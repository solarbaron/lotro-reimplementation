<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Input: actions, contexts, Tessera's keymap file, movement controllers

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Implementer (clean room). Everything here is a **Tessera design decision**, not a description of another program. |
| **Reviewer** | none yet |
| **Implements into** | `crates/input` (`tessera-input`) |
| **Companion docs** | `docs/ARCHITECTURE.md` §16 (input pipeline), §17 (settings directories), §20.3 (frame loop, 30 Hz fixed tick); `mesh.md` §11 (axes, units, handedness, model facing). |

---

<a id="s0"></a>
## 0. Purpose

This spec defines Tessera's **own** input model and keymap file format:

- named **actions** and stacked **contexts** (§2, §3);
- the device vocabulary and binding syntax: keys, mouse buttons, wheel clicks, gamepad
  buttons, analog axes, modifiers, simultaneous chords and stroke sequences (§4, §5);
- the TOML keymap file: schema, limits, lenient loading, overrides over built-in defaults,
  location and safe saving (§6);
- the runtime resolution rules that turn raw events into per-frame action states (§7);
- the built-in actions, contexts and default bindings (§8);
- the free-fly and walk controllers driven at a fixed time step (§9);
- the optional winit adapter (§10).

It is **not** a description of the original client's key-mapping file, whose syntax is
unknown to the clean room (ARCH §16 OPEN; see §11). Nothing in this document is taken
from another program's key map; the default layout is a generic choice for a
third-person walker.

---

<a id="s1"></a>
## 1. Provenance

| Fact | Source | Status |
|---|---|---|
| Pipeline: window/device events → raw state → contexts → actions | ARCH §16 | Decision |
| Keymap is a TOML file in Tessera's config directory | ARCH §16, §17 | Decision |
| Simulation runs at a fixed 30 Hz tick, interpolated for rendering | ARCH §20.3 | Decision |
| World space is `+Z` up | ARCH §11.1 | Decision |
| World space is right-handed, units are metres, character models face `+Y` | `mesh.md` §11.1–11.2 | Confirmed (data) |
| Everything else in this spec (syntax, limits, defaults, algorithms, tuning numbers) | Tessera design, this document | Decision |

---

<a id="s2"></a>
## 2. Actions

An **action** is a name that code reacts to (`jump`, `move_forward`). Code registers
actions in a registry; keymaps refer to them by name.

- **Names**: 1–64 bytes of `a-z`, `0-9`, `_`, `.`. A `.` is allowed so plugins can
  namespace their actions (`myplugin.toggle`); in TOML such names must be quoted (§6).
- **Kinds**:
  - *Button*: `pressed` (held now), `presses` / `releases` (edge counts in the frame),
    `value` = 1 while pressed, else 0.
  - *Axis*: `value` in `-1..=1` = sum of held digital contributions and absolute analog
    axes, clamped; `delta` = sum of scaled relative-axis movement during the frame
    (mouse motion, wheel); `pressed` = a digital binding is held, or `|value| ≥ 0.5`.
- **Modifier policy** (per action, set by code):
  - *Exact* (default for buttons): the held modifiers must equal the binding's.
  - *Subset* (default for axes; used for movement): the binding's modifiers must be held,
    extra ones are ignored, but any exact match wins (§7.2).
- Limits: 1024 actions, 64 contexts per registry.
- Actions and contexts may also be registered on a running mapper (plugins): the keymap in
  use is recompiled against the grown registry; active contexts, held buttons and the holds
  of unchanged bindings are kept, a pending sequence is dropped.

---

<a id="s3"></a>
## 3. Contexts

A **context** is a named group of bindings that can be switched on and off (`movement`,
`text_entry`). Each has:

| Field | Meaning |
|---|---|
| `priority` (signed integer) | Active contexts are consulted from the highest priority down; among equal priorities the most recently activated comes first. |
| `opaque` | While active, inputs it does not bind do not reach lower contexts (text entry, modal dialogs). |
| `text` | It receives typed characters when it is the first context in resolution order. |

The set of active contexts is the **context stack**. Operations: activate (re-activating
moves a context to the top of its priority band), deactivate, query.

- Deactivating a context releases every hold it owns (release edges are reported).
- Activating an **opaque** context releases holds owned by contexts below it, so movement
  stops when a chat line opens.

---

<a id="s4"></a>
## 4. Device vocabulary

Key names are **physical positions named after a US layout** (the key that types `W` on a
US layout is `W` on every layout), so bindings keep their shape on other layouts. All
names are matched ASCII case-insensitively; the canonical spelling (first column) is what
Tessera writes.

### 4.1 Keys

| Canonical names | Aliases also accepted |
|---|---|
| `A` … `Z` | `KeyA` … `KeyZ` |
| `0` … `9` | `Digit0` … `Digit9` |
| `F1` … `F24` | |
| `Escape`, `Tab`, `CapsLock`, `Space`, `Enter`, `Backspace` | `Esc`, `Return` |
| `Insert`, `Delete`, `Home`, `End`, `PageUp`, `PageDown` | `Ins`, `Del`, `PgUp`, `PgDn` |
| `Up`, `Down`, `Left`, `Right` | `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight` |
| `LShift`, `RShift`, `LCtrl`, `RCtrl`, `LAlt`, `RAlt`, `LSuper`, `RSuper` | `ShiftLeft`, `ControlLeft`, `CtrlLeft`, `AltLeft`, `SuperLeft`, `MetaLeft` (and the `Right` forms), `AltGr` |
| `Backquote`, `Minus`, `Equal`, `BracketLeft`, `BracketRight`, `Backslash`, `Semicolon`, `Quote`, `Comma`, `Period`, `Slash`, `IntlBackslash` | `Grave`, `Tilde`, `Equals`, `LBracket`, `RBracket`, `Apostrophe`, `Dot` |
| `Numpad0` … `Numpad9`, `NumpadAdd`, `NumpadSubtract`, `NumpadMultiply`, `NumpadDivide`, `NumpadDecimal`, `NumpadEnter` | `NumpadPlus`, `NumpadMinus`, `NumpadStar`, `NumpadSlash`, `NumpadPeriod` |
| `NumLock`, `ScrollLock`, `Pause`, `PrintScreen`, `ContextMenu` | `PrtSc`, `Menu` |

Punctuation is always written as a word; `+` is the token separator and has no key name.

### 4.2 Mouse, wheel, gamepad

| Kind | Names |
|---|---|
| Mouse buttons | `MouseLeft` (`Mouse1`), `MouseRight` (`Mouse2`), `MouseMiddle` (`Mouse3`), `MouseBack` (`Mouse4`), `MouseForward` (`Mouse5`), `Mouse6` … `Mouse65535` (any other platform button number, written in decimal, at most 5 digits). Other button numbers 0–5 have no name and cannot be bound. |
| Wheel clicks (momentary buttons) | `WheelUp`, `WheelDown`, `WheelLeft`, `WheelRight` |
| Gamepad buttons (positional) | `PadSouth` (`PadA`), `PadEast` (`PadB`), `PadWest` (`PadX`), `PadNorth` (`PadY`), `PadL1`, `PadR1`, `PadL2`, `PadR2`, `PadSelect` (`PadBack`), `PadStart`, `PadMode` (`PadGuide`), `PadL3`, `PadR3`, `PadUp`, `PadDown`, `PadLeft`, `PadRight` |

### 4.3 Analog axes

| Name | Kind | Range / unit |
|---|---|---|
| `MouseX`, `MouseY` | relative | raw device motion units per frame; `+X` right, `+Y` down |
| `WheelX`, `WheelY` | relative | notches per frame; `+Y` away from the user, `+X` right |
| `PadLeftX`, `PadLeftY`, `PadRightX`, `PadRightY` | absolute | `-1..=1`; `+Y` = stick pushed away from the player |
| `PadLeftTrigger` (`PadLT`), `PadRightTrigger` (`PadRT`) | absolute | `0..=1` |

### 4.4 Modifiers

`Ctrl` (`Control`), `Alt`, `Shift`, `Super` (`Meta`, `Cmd`). Left and right keys are
merged. Canonical order when written: `Ctrl`, `Alt`, `Shift`, `Super`.

---

<a id="s5"></a>
## 5. Binding syntax

<a id="s5-1"></a>
### 5.1 Strokes

A **stroke** is `+`-separated tokens with no whitespace: zero or more modifiers and zero
to four buttons (keys, mouse buttons, wheel clicks, pad buttons).

- Button order does not matter and duplicates collapse: `MouseRight+MouseLeft` is
  `MouseLeft+MouseRight`. Written form: modifiers in canonical order, then buttons in a
  fixed internal order.
- A stroke with **several buttons** is a *simultaneous chord*: it completes when the last
  of its buttons is pressed while the others are held, in any order.
- A stroke with **modifiers only** (`Shift`, `Ctrl+Alt`) is a *modifier hold*: the binding
  is held while those modifiers are held (§7.4). It cannot be part of a sequence.
- A modifier **key** used as a button (`LCtrl`, `LShift+A`) does not also require its own
  modifier: when matching, modifiers contributed by the stroke's own buttons are ignored.
- Errors: empty stroke or token, unknown token, more than four buttons, axis names, a
  button without a name (§4.2) when a stroke is built in code.

<a id="s5-2"></a>
### 5.2 Sequences

A binding is 1–4 strokes separated by ASCII whitespace: `G G`, `Ctrl+K Ctrl+C`. Each
stroke after the first must complete within the **sequence timeout** of the previous one
(default 1000 ms, `options.sequence_timeout_ms`, range 50–10000). Binding text is at most
256 bytes.

Within one context, a binding whose strokes start with the **complete** strokes of another
binding (the shorter would always fire first) is disabled when the keymap is compiled,
with a warning: `G` and `G H` in one context → `G H` is dropped.

<a id="s5-3"></a>
### 5.3 Binding entries

| Entry | Meaning |
|---|---|
| `"text"` | Digital binding (strokes as above), axis value `1.0`. |
| `{ input = "text", value = v }` | Digital binding with axis value `v` in `-1..=1` (button actions ignore `value`). |
| `{ axis = "Name", scale = s, deadzone = d, invert = b }` | Analog binding. `scale` default 1, `|s| ≤ 10000`; `deadzone` default 0, range `0..=0.95` (absolute axes only); `invert = true` negates `scale` (stored as a negative scale). |

Numbers are stored as 32-bit floats and the ranges are checked on the stored value, so the
bounds as written (`deadzone = 0.95`) are accepted.

Unknown fields, both or neither of `input`/`axis`, `value` with `axis`, or `scale`/
`deadzone`/`invert` with `input` make the entry invalid.

---

<a id="s6"></a>
## 6. The keymap file

<a id="s6-1"></a>
### 6.1 Schema

```toml
format = "tessera-keymap"     # optional tag; any other value is a warning
version = 1                   # format version; newer than supported: see §6.3

[options]
sequence_timeout_ms = 1000

[bindings.movement]           # [bindings.<context>]
move_forward = ["W", "Up", { input = "S", value = -1.0 }, { axis = "PadLeftY", deadzone = 0.15 }]
jump = ["Space"]
autorun = []                  # explicitly unbound

[bindings."ui.dialog"]        # names containing '.' are quoted
"myplugin.close" = ["Ctrl+K Ctrl+W"]
```

- Each action maps to a list of entries (§5.3); a single string is accepted as a list of
  one. Exact duplicate entries collapse.
- Context and action names follow §2's name rule; names the running build does not
  register are **kept** in the document (so a save does not lose them) and reported when
  compiled.
- Unknown top-level keys are kept and written back unchanged. Unknown keys inside
  `[options]` are warnings and are dropped.

<a id="s6-2"></a>
### 6.2 Limits

| Limit | Value |
|---|---|
| Document size | 1 MiB |
| Contexts per keymap | 64 |
| Actions per context | 1024 |
| Bindings per action | 16 (after removing duplicates) |
| Strokes per binding / buttons per stroke | 4 / 4 |

<a id="s6-3"></a>
### 6.3 Strict and lenient reading

- **Lenient** (used at start-up): never fails. A document that is not TOML or exceeds the
  size limit yields an empty keymap; each bad section, entry or value is dropped and
  reported as a warning; a newer `version` is a warning and the document is read
  best-effort. An action whose list is non-empty but has **no usable entry** (a typo, or
  device names from a newer build) is left out with a warning, so the overlay (§6.4) keeps
  its defaults; only a literally empty list unbinds. At most 64 warnings are recorded (the
  same cap applies when compiling against a registry); past that one "truncated" warning is
  added and the rest are dropped.
- **Strict**: any warning is an error; a newer version is an error.
- Serialisation is deterministic (sorted context and action names, entry order kept) and
  the strict reader returns exactly the keymap that was written. To keep that promise for
  keymaps built in code: setting an action's bindings normalises every number into its
  range (non-finite `value` → 1, non-finite `scale` → 1, non-finite `deadzone` → 0, others
  clamped) and refuses a binding with an unnamed button; the serialiser refuses (rather
  than writes) an out-of-range `sequence_timeout_ms` or any binding that would not read
  back. At run time an out-of-range timeout set in code is clamped into 50–10000.

<a id="s6-4"></a>
### 6.4 Overrides over built-in defaults

The user's file holds **only overrides** of the built-in defaults (§8):

- *overlay* `defaults ⊕ user`: every `(context, action)` the user file mentions replaces
  the default list (an empty list unbinds); options set in the file win; unknown keys
  come from the file.
- *diff* `effective ⊖ defaults`: pairs whose lists differ, plus empty lists for default
  pairs that are missing; the timeout when it differs. Saving writes the diff, so changed
  defaults in later versions reach users who never rebound those actions.

<a id="s6-5"></a>
### 6.5 Location and saving

- Path: `$TESSERA_KEYMAP_FILE` when set and non-empty, else `keymap.toml` next to the
  settings file (on Linux `~/.config/tessera/keymap.toml`).
- Reading is bounded to the size limit and refuses non-regular files and non-UTF-8. The
  type is checked before opening and again on the open handle, and on Unix the file is
  opened non-blocking, so a FIFO or device node is refused without blocking.
- Saving follows symbolic links in the final path component (at most 40) to the file that
  will really be replaced, writes a temporary file in **that** file's directory and renames
  it over that file; the link itself is left alone. Before anything is written, the settings
  crate's read-only install guard must accept the resolved destination (it refuses any path
  inside a recorded game directory, inside `TESSERA_GAME_DIR`, or under a directory holding
  `client_general.dat`); the check runs again on the canonical directory after it is
  created. An existing destination that is not a regular file is not replaced.
- Nothing is saved automatically; a save follows an explicit rebinding by the user.

---

<a id="s7"></a>
## 7. Runtime resolution

<a id="s7-1"></a>
### 7.1 Frame protocol

1. Raw events are fed in arrival order, each with a monotonic timestamp.
2. Once per frame, *update* publishes a snapshot of every action.
3. Readers use the snapshot until the next update.

Edges and relative deltas that occur between two updates appear in the second snapshot;
a press and release inside one frame yields `presses = 1, releases = 1, pressed = false`.

Raw events: key (with an auto-repeat flag), mouse button, raw mouse motion, cursor
position, cursor left, wheel (notches), gamepad button, gamepad axis position, the window
system's modifier state, typed character, focus lost.

<a id="s7-2"></a>
### 7.2 Resolving a press

Auto-repeat presses and presses of buttons already held are ignored. At most 64 buttons are
tracked as held; further presses are ignored. Then:

1. **Pending sequence.** If a sequence is waiting and has not timed out, the press is
   tried only against the remaining candidates of that sequence's context at the next
   stroke position. On a match the sequence advances (or completes). Otherwise, a press of
   a **modifier key**, or of one button of a candidate's next stroke whose other buttons are
   not all held yet (a simultaneous chord in progress), leaves the sequence waiting and is
   not resolved further (so `Ctrl+K`, release, `Ctrl+C` and `Ctrl+K Shift+C` work; while a
   sequence waits, a modifier key bound as a plain button does not fire). Any other press
   cancels the sequence and continues with step 2.
2. **Two passes over the active contexts** in resolution order (§3): the first pass
   accepts only *exact-modifier* matches, the second also *subset* matches (for actions
   with the subset policy). In each pass, the first context that has a binding whose first
   stroke is completed by this press **consumes** the press. An opaque context ends the
   pass (inputs below it are never seen).
3. In the consuming context, every matching binding that is complete fires (becomes a
   *hold*); if none is complete, the matches become the candidates of a new pending
   sequence with deadline `now + timeout`.

A stroke is *completed by* a press when the pressed button is one of its buttons, all its
other buttons are held, and the held modifiers (minus those contributed by its own
buttons) equal its modifiers (exact) or contain them (subset).

Consequences: a simultaneous chord and a single-button binding in the **same** context both
fire when the chord completes (the chord does not consume the single binding); in
different contexts only the higher one fires. Modifiers released while a hold is active do
not end it.

<a id="s7-3"></a>
### 7.3 Holds and releases

A hold remembers the buttons of its binding's **last** stroke and ends when any of them is
released, when its context is deactivated (or covered by an opaque context, §3), on focus
loss, or when the keymap is replaced. An action is pressed while it has at least one hold,
an analog press (§7.5) or a modifier hold (§7.4); edges are counted on transitions of
that combined state. Wheel clicks are momentary: their hold starts and ends at once.
At most 256 holds exist at a time.

When a hold ends, buttons that are still held are **not** resolved again: a press consumed
by a chord in one context does not later start a single-button binding in a lower context.
Keymaps that want "release one button of the chord, keep the other's action" bind that
single button in the chord's context, where it fires together with the chord (§7.2); the
defaults do this for `mouselook` (§8).

<a id="s7-4"></a>
### 7.4 Modifier holds

At each update, for contexts in resolution order up to and including the first opaque one,
every modifier-hold binding whose modifiers match the held modifiers (exact or subset by
policy) holds its action. Modifier holds do not consume anything. Held modifiers are the
union of modifier keys held and the window system's last report.

<a id="s7-5"></a>
### 7.5 Analog sources

At each update, each analog axis is routed to the **first** context in resolution order
that binds it (an opaque context that does not bind it swallows it). In that context:

- relative axes add `movement × scale` to the `delta` of each bound axis action (button
  actions ignore relative axes);
- absolute axes contribute `deadzone(v) × scale` to the `value` of axis actions, where
  `deadzone(v) = sign(v) · (|v| − d)/(1 − d)` above `d` and 0 inside it; for button actions
  the largest contribution presses the action at ≥ 0.5 and releases it below 0.4.

Wheel events also generate one momentary `WheelUp`/`WheelDown`/`WheelLeft`/`WheelRight`
click per whole notch (fractions accumulate; at most 16 clicks per event). Non-finite
inputs are ignored; one motion event is clamped to ±100000 units; gamepad positions are
clamped to `-1..=1`; focus loss zeroes them.

<a id="s7-6"></a>
### 7.6 Text

Typed characters are collected for the frame (at most 4096 bytes, control characters
dropped) only while the first context in resolution order has `text` set.

---

<a id="s8"></a>
## 8. Built-in actions, contexts and defaults

| Context | Priority | Flags |
|---|---|---|
| `global` | 0 | |
| `camera` | 10 | |
| `movement` | 20 | |
| `fly` | 30 | |
| `text_entry` | 100 | opaque, text |

| Context | Action | Kind (policy) | Default bindings |
|---|---|---|---|
| global | `menu` | button | `Escape` |
| global | `screenshot` | button | `PrintScreen`, `F12` |
| global | `toggle_fly` | button | `Ctrl+F` |
| global | `toggle_debug` | button | `F3` |
| global | `open_chat` | button | `Enter`, `NumpadEnter` |
| camera | `mouselook` | button (subset) | `MouseRight` |
| movement | `mouselook` | | `MouseRight` |
| camera | `look_x` | axis | `MouseX`; `PadRightX` (dead zone 0.15) |
| camera | `look_y` | axis | `MouseY`; `PadRightY` (scale −1, dead zone 0.15) |
| camera | `zoom` | axis | `WheelY`; `PageUp` (+1), `PageDown` (−1) |
| camera | `reset_camera` | button | `Home` |
| movement | `move_forward` | axis | `W`, `Up`, `MouseLeft+MouseRight` (+1); `S`, `Down` (−1); `PadLeftY` (dead zone 0.15) |
| movement | `strafe` | axis | `E` (+1, right), `Q` (−1); `PadLeftX` (dead zone 0.15) |
| movement | `turn` | axis | `A`, `Left` (+1, left); `D`, `Right` (−1) |
| movement | `jump` | button (subset) | `Space`, `PadSouth` |
| movement | `walk` | button (subset) | `Shift` (modifier hold) |
| movement | `autorun` | button (subset) | `NumLock`, `R`, `PadL3` |
| fly | `fly_vertical` | axis | `Space`, `PadR1` (+1); `C`, `PadL1` (−1) |
| fly | `fly_boost` | button (subset) | `Shift` |
| fly | `fly_slow` | button (subset) | `Alt` |
| fly | `fly_speed` | axis | `WheelY` |
| text_entry | `text_submit` | button | `Enter`, `NumpadEnter` |
| text_entry | `text_cancel` | button | `Escape` |
| text_entry | `text_backspace` | button | `Backspace` |
| text_entry | `text_history_prev` / `text_history_next` | button | `Up` / `Down` |

Mode helpers: *walk mode* activates `global`, `camera`, `movement` and deactivates `fly`;
*fly mode* activates all four with `fly` on top (its `Space` shadows `jump`).

Look glue: mouse deltas turn the view only while `mouselook` is pressed (or the caller
captured the cursor). `look_x`/`look_y` use the mouse convention (`+` = right / down) for
every source, which is why `PadRightY` (`+` = pushed away) has scale −1. Yaw change
`= −look_x.delta·s − look_x.value·r·dt`, pitch change `= −look_y.delta·s −
look_y.value·r·dt` (negated again when vertical look is inverted), with `s = 0.0025
rad/unit` and `r = 2.5 rad/s` by default: moving the mouse away and pushing the stick away
both look up; mouse or stick right both turn right. The `MouseRight` binding of `mouselook`
in `movement` fires together with the `MouseLeft+MouseRight` run chord, so looking stays
active while running with both buttons and after letting go of the left one, in either
press order (§7.3).

---

<a id="s9"></a>
## 9. Movement controllers

<a id="s9-1"></a>
### 9.1 Conventions and fixed step

- World: right-handed, `+Z` up, metres (`mesh.md` §11.1–11.2). **Yaw** is the heading
  counter-clockwise from `+X` seen from above (yaw 0 faces `+X`, π/2 faces `+Y`); **pitch**
  is the angle above the horizontal. Right of yaw = `(sin yaw, −cos yaw, 0)`.
- Character models face `+Y` (`mesh.md` §11.1), i.e. yaw π/2. A model drawn for a
  controller is rotated about `+Z` by **`yaw − π/2`** (wrapped), not by `yaw`.
- A *move intent* per tick: `forward`, `strafe` (+ right), `turn` (+ left), `vertical`
  (fly), each sanitised to `-1..=1` (non-finite → 0), plus `jump`, `walk`, `boost`, `slow`.
- **Fixed step**: an accumulator turns frame times into whole ticks of `1/30 s` by default
  (rate clamped to 1–1000 Hz). At most 8 ticks run per frame; excess time is dropped and
  counted (a remainder that rounding makes slightly negative counts as zero, so no time is
  created at the cap). `alpha` = leftover fraction of a tick, used to interpolate rendering
  between the previous and current controller **positions and yaws** (yaw along the
  shorter arc, so ±π is crossed correctly). Keyboard turning is applied per tick and is
  therefore interpolated.
- **Intent latch**: axes come from the latest frame; a jump press is kept until the next
  tick consumes it (a frame without ticks does not lose it; a frame with two ticks does
  not jump twice).
- Look changes (mouse) are applied every frame, outside the tick, for responsiveness; they
  shift both the previous and the current yaw, so they show at once and are not
  interpolated twice. Setting the yaw or teleporting disables interpolation for that step.
- Positions are clamped to ±10⁷ units; non-finite state is reset to safe values.

<a id="s9-2"></a>
### 9.2 Free-fly controller

Per tick: yaw += `turn × turn_rate × dt`; wish = `forward × view_dir + strafe × right +
vertical × Z`, normalised if longer than 1; speed = base × (`boost` ? 5 : 1) × (`slow` ?
0.2 : 1); velocity approaches `wish × speed` by at most `acceleration × dt` (instant when
acceleration ≤ 0); position += velocity × dt. Pitch is clamped to ±89°. A speed wheel
multiplies the base speed by `2^(notches/4)` within `min_speed..=max_speed`.

Defaults (Tessera tuning): speed 40, boost ×5, slow ×0.2, acceleration 400 u/s², turn rate
2 rad/s, speed range 1–2000.

<a id="s9-3"></a>
### 9.3 Walk controller

The ground is a callback `height(x, y) → Option<h>` (`None` = no data loaded; non-finite
values are treated as `None`), optionally `normal(x, y)`. Without an exact normal it is
estimated by central differences at `±normal_probe` (one-sided where a neighbour is
missing, `+Z` if both are). Ground is *walkable* when `normal.z ≥ cos(max_slope)`.

States: **Grounded**, **Sliding** (on ground steeper than the slope limit), **Airborne**,
**Unsupported** (no data under the feet: the character is held still until data arrives).

Per tick (`dt` capped at 0.25 s):

1. yaw += `turn × turn_rate × dt`. A jump press sets a jump buffer of
   `max(jump_buffer, dt)`.
2. No height under the feet → Unsupported, velocity zero, done.
3. If Unsupported before, or below the surface: stand on the surface if within
   `step_height` of it, else become Airborne. If Airborne, not rising, and within 10⁻⁴ of
   the surface: land. If Grounded or Sliding but more than 10⁻⁴ above the surface (the
   ground dropped away under a character standing still: refined heights, a terrain or LOD
   swap): snap down when the drop is within `step_height`, else become Airborne (vertical
   speed 0).
4. Target horizontal velocity = (`forward × heading + strafe × right`, normalised if longer
   than 1) × (`walk` ? walk_speed : run_speed) × (backward ? backward_multiplier : 1).
5. By state:
   - Grounded: horizontal velocity approaches the target by `ground_acceleration × dt`;
     vertical 0. With a buffered jump: vertical = `jump_speed`, Airborne, buffer cleared.
   - Sliding: approach by `air_acceleration × dt`, then add the downhill direction ×
     `gravity × sinθ × dt`; horizontal speed capped at `max_fall_speed`; vertical 0.
   - Airborne: horizontal approaches by `air_acceleration × dt` (air control).
6. Horizontal displacement `v × dt` is cut into ≤ 64 equal sub-steps of at most
   `max_substep`. Each sub-step to candidate `c` with ground `h_c`:
   - no data at `c` → blocked;
   - Grounded/Sliding: if `h_c` rises above the feet, it is allowed only when the rise
     ≤ `step_height` **and** either the ground at `c` is walkable or (*step-up*) the ground
     `step_probe` further along the motion is walkable and within `step_height` of the
     feet. After moving: if the drop to `h_c` ≤ `step_height + |sub-step| × tan(max_slope)`
     the feet snap to `h_c` (state from walkability at `c`), else the character walked off
     a ledge and becomes Airborne;
   - Airborne: blocked if `h_c` > feet + `step_height`; otherwise move, and touching the
     ground while not rising lands.
   - When a sub-step is blocked, the axis-aligned components are tried (larger first); the
     velocity along the blocked axis is zeroed; if both fail, horizontal velocity is
     zeroed and movement stops for this tick.
7. Airborne vertical motion integrates constant gravity exactly over the tick:
   `v₁ = max(v₀ − g·dt, −max_fall_speed)`, `z += (v₀ + v₁)/2 · dt` (so a jump's apex is
   `jump_speed²/(2g)` at any tick rate). Reaching the ground (within 10⁻⁴) lands, reports
   the impact speed, and sets Grounded or Sliding by walkability.

Defaults (Tessera tuning, in world units assumed to be metres — see §11): run 6, walk 2,
backward ×0.6, ground acceleration 60, air acceleration 6, gravity 20, jump speed 7
(apex ≈ 1.23), max fall speed 60, step height 0.5, max slope 50°, turn rate 2.5 rad/s, jump
buffer 0.15 s, normal probe 0.25, step probe 0.3, max sub-step 0.25. Every tunable is
sanitised into a sane range each tick.

---

<a id="s10"></a>
## 10. winit adapter (feature `winit`)

- Keyboard: physical key codes map one-to-one onto §4.1 (unmapped keys are dropped);
  the repeat flag is passed through; on a press, the event's text yields up to 32 typed
  characters.
- Window modifier changes, mouse buttons, cursor moves, cursor leaving, and focus loss map
  to the raw events of §7.1.
- Wheel: line deltas are notches; pixel deltas are divided by 40 pixels per notch. winit's
  positive horizontal delta scrolls content right, which is a tilt to the **left**, so the
  horizontal value is negated to match `WheelX` `+` = right (§4.3); the vertical value
  (positive = away from the user) is used as is.
- Raw device motion (`DeviceEvent::MouseMotion`) feeds `MouseX`/`MouseY`.

---

<a id="s11"></a>
## 11. Open items

> **OPEN:** importing the original client's key-mapping file (read-only) waits for a
> dirty-room spec of its syntax (ARCH §16, decision 12, M5).

> **OPEN:** the table mapping Tessera actions to the plugin API's key-binding action IDs
> (Reported as about 270 actions, ARCH §16) is not defined here.

> **OPEN:** metres are Confirmed for meshes (`mesh.md` §11.1). What remains is the terrain
> height scale (landblock height table → metres) needed for the walk controller to stand on
> the terrain at the right height (`landblock.md`/`coordinates.md`); tuning numbers may then
> need adjusting.

> **OPEN:** mapping between Tessera's yaw convention (§9.1) and the game's heading/map
> conventions belongs in `coordinates.md` (not owned by this spec); the model-facing offset
> (`yaw − π/2`) is fixed here.

> **OPEN:** gamepads: no gilrs adapter yet; all pads are merged into one; stick `+Y`
> direction and trigger ranges need checking per backend.

> **OPEN:** the walk controller only knows the height field. Collision with static
> objects, water/swimming, and per-race or mounted speeds come with later milestones.

> **OPEN:** IME composition, key auto-repeat for UI actions, and localised key labels for
> display are not covered.

> **OPEN:** layering. `tessera-input` (L4) depends on its sibling `tessera-settings` (L4) for
> the read-only install guard and the config directory; ARCH §3 lists no such allowance yet
> and should either record it or move the guard lower. The movement controllers
> (`MoveIntent`, `HeightQuery`, `FixedTimestep`, fly/walk) depend only on `glam` and could
> move to a small lower-layer crate shared with world collision and a future server
> (M7); until then the world layer plugs in through a closure (`Fn(x, y) → Option<h>`), so
> it does not depend on `tessera-input`.

> **OPEN:** entries the lenient reader cannot parse are dropped, so saving after a load
> loses them (the defaults stay in effect for such an action). Keeping their raw text for
> a later save would need a separate "unparsed entries" store.
