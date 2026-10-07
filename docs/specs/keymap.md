<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Input: actions, contexts, Tessera's keymap file, movement controllers

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | §0–§11: Implementer (clean room); everything there is a **Tessera design decision**, not a description of another program. §12–§13 and the 2026-10-05 revisions of §1 and §11: Spec writer (dirty room); §12 describes the original client's key-mapping file and key-map data, with provenance (§1.2). |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review 2026-10-06: **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/input` (`tessera-input`); the importer of §12 also reads records through `crates/dat`/`crates/formats` |
| **Companion docs** | `docs/ARCHITECTURE.md` §16 (input pipeline), §17 (settings directories), §20.3 (frame loop, 30 Hz fixed tick); `mesh.md` §11 (axes, units, handedness, model facing); `install-layout.md` §8.1 (settings folder), §10 (`--keymap`); `records.md` §1 (VLE, TSize, Pascal string), §7 (enum mappers), §8 (DID maps); `property-system.md` (DID-map groups). |

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
- the optional winit adapter (§10);
- a **read-only importer** for the original client's key-mapping file and its key-map
  records (§12): the file syntax, the records' byte layouts, how the original combines
  them into its effective bindings, the meaning class of every command name, and the
  translation of the result into a Tessera keymap.

§0–§11 are **not** a description of another program: nothing in them is taken from another
program's key map, and the default layout of §8 is a generic choice for a third-person
walker. §12 is the only part that describes the original client; every fact in it carries a
provenance row (§1.2), and the importer it specifies never writes the original's file or the
install.

---

<a id="s1"></a>
## 1. Provenance

<a id="s1-1"></a>
### 1.1 Tessera design (§2–§11)

| Fact | Source | Status |
|---|---|---|
| Pipeline: window/device events → raw state → contexts → actions | ARCH §16 | Decision |
| Keymap is a TOML file in Tessera's config directory | ARCH §16, §17 | Decision |
| Simulation runs at a fixed 30 Hz tick, interpolated for rendering | ARCH §20.3 | Decision |
| World space is `+Z` up | ARCH §11.1 | Decision |
| World space is right-handed, units are metres, character models face `+Y` | `mesh.md` §11.1–11.2 | Confirmed (data) |
| Everything else in §2–§11 (syntax, limits, defaults, algorithms, tuning numbers) | Tessera design, this document | Decision |

<a id="s1-2"></a>
### 1.2 The original client's key map (§12)

Sources:

| Key | Source |
|---|---|
| **OBS** | The owner's install and settings folder, read **read-only** on 2026-10-05: the four key-map records (prefix 0x1D) and two action-map records (prefix 0x51) in full; the enum mappers 0x230000AF, 0x23000176, 0x230000B0, 0x2300019E, 0x230001C0, 0x230001C1, 0x230001C2; the DID maps 0x28000000, 0x28000001, 0x2800000A, 0x2800000C; the owner's own key-mapping file (written by the original client, 254 bindings) and the `[Input]` section of the owner's preferences file. Probes were throwaway scripts in the dirty scratch area. |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md): disassembly of the offline client `x64/lotroclient64.exe`, file version **4906.0071.1794.4035**, 2026-10-05, confined to its key-map reader, writer, merge and binding-selection code, the generic text-tree parser that reader uses, the device-name tables, the XInput translation and the registration of the `[Input]` preferences. **Questions:** (a) the lexical grammar of the key-mapping file; (b) how the four parts and each binding are interpreted, and which errors reject the whole file; (c) the start-up load-and-merge sequence and the conflict rule; (d) the accepted control-name vocabulary; (e) the format the client writes; (f) how one binding is chosen at run time; (g) how XInput pads are presented as controls; (h) the initial values of the `[Input]` preferences. **Why M1–M3 were not enough:** the format is undocumented (research notes: "not found"); the install holds only one file written by the client (M1), which cannot show error handling, case rules, the accepted vocabulary, defaults merging or rejection reasons; black-box tests (M3) would need many edited files fed to the running official client, rewriting the owner's settings each time, and still could not show merge order. The BIN model was then checked against M1 data: rebuilding the owner's file from its user entries and the install's records reproduced it **byte for byte** (§12.17, I1). **Revision session (same build, 2026-10-05)**, answering a review, confined to the same parser, reader and writer code plus the construction of the control-name tables, the string-to-number conversion of node texts, the splitting and writing of activation words, and the loading of the XInput state function. Questions, all within (a), (b), (e) and (g): what an opening bracket with no token before it produces; what bytes 0xFF and 0x00 do; what the original does with extra children of the name node and of device entries, and with empty tokens where numbers, activations and sub-controls are expected; which of the two spellings of each joystick code the writer emits; what the writer emits for a control code without a name and for activations without a word; which XInput state call supplies the pad buttons. Why M1–M3 were not enough: the owner's file uses none of these cases, and feeding hand-edited files to the official client (M3) would rewrite the owner's settings each time and would not show the writer's choice for controls nobody has bound. **Second revision session (same build, 2026-10-06)**, answering the re-review, confined to the same text-tree parser (its node-list loop, directive handler and the check made before parsing), the key-map reader's four part readers, its binding and control-specification readers, and the node accessors they call (child count, child by index, text, number). Questions, all within (a) and (b): what a top-level part without a child list gives; whether children of nodes read only for their text are looked at; how directives are recognised, matched and handled when unknown or without a child; what a device index that is not a number does. Why M1–M3 were not enough: as for the first revision (none of these cases occurs in the owner's file, and M3 would rewrite the owner's settings). Network, login and launcher code were not examined (decision 0004). |
| **DX** | Public DirectInput 8 and XInput header constants (key, mouse and joystick offset names and values, the two DirectInput system-device GUIDs, XInput button bits). Public C runtime behaviour (text-mode reading, `strtoul`). General engineering knowledge, not dirty (CLEAN_ROOM.md §2). |
| **PUB** | Public developer discussion of XInput (SDL project forum, "SDL Archaeology: Where does XINPUT_GAMEPAD_EX come from?", read 2026-10-05): the guide button is bit 0x0400, it is reported only by an undocumented extended state call (ordinal 100 of later XInput versions), not by the documented `XInputGetState`, and the oldest XInput version has no such call. Facts only. |
| **PLG** | Third-party dump of the plugin API (vscode-lotro-api `TurbineDump.json`, March 2026): the `Action` enumeration. Facts only. |
| **HELP** | The original client's command-line help text as recorded by OneLauncher (`install-layout.md` P30). |
| **AC** | OpenAC (MIT) retail keymap reader and DatReaderWriter's `MasterInputMap` description (Asheron's Call). Lineage only: the same file family with an empty origin token. Facts only. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| K1 | The file is `lotro.keymap` in the per-user settings folder; the client reads another file instead when started with `--keymap <file>`, but always writes the settings-folder file (§12.2) | OBS (location); BIN | Confirmed (location), BIN |
| K2 | The client writes ASCII text with CRLF line ends and no byte-order mark, starting with `#` comment lines (§12.13) | OBS (owner's file); BIN (text-mode write) | Confirmed |
| K3 | Lexical grammar: separators, comments, quoted and bare tokens, backslash escapes, bracket pairs, end of input (§12.3) | BIN; reproduction I1 | BIN; the parts the owner's file uses Confirmed by I1 |
| K4 | Node tree, bracket matching, directives, fatal and reported parse errors (§12.4) | BIN | BIN |
| K5 | Four top-level parts identified by position, not by name (§12.5) | BIN; OBS | BIN, Confirmed (layout) |
| K6 | Device entries, device type words, GUID forms, standard devices appended after the file's list (§12.5.2) | BIN; OBS (device lists of all four 0x1D records) | BIN, Confirmed (lists) |
| K7 | GUID values of the eight standard devices and their symbolic names (§12.5.2) | OBS (data device lists); BIN (names); DX (the two system GUIDs) | Confirmed |
| K8 | Meta-key table: index 1–32; a binding's mask bit *i* (value 2^(i−1)) is held internally as 2^(32−i) (§12.5.3, §12.9) | BIN; OBS (records) | Confirmed |
| K9 | Section names are input-map enum names, matched without regard to case; an unknown or repeated section rejects the file (§12.5.4) | BIN; OBS (16 names) | BIN, Confirmed (names) |
| K10 | Binding entry layout and the origin token (`User` versus anything else) (§12.5.5) | BIN; OBS | BIN, Confirmed (layout) |
| K11 | Control specification: 2 or 3 children, device index below 256, control name looked up in all four name tables with case, alias names (§12.5.6, §12.6) | BIN | BIN |
| K12 | The accepted control-name vocabulary and codes (§12.6) | BIN (accepted set, spellings written); DX (meanings and values); OBS (names in the owner's file, codes in records) | BIN, Confirmed (used subset) |
| K13 | Activation words, flag values, `/` combination, default Down + Up, writer spelling (§12.5.7) | BIN; OBS (records and file) | BIN, Confirmed (used values) |
| K14 | Sub-control words and values (§12.5.8) | BIN; OBS | BIN, Confirmed (AxisPositive, AxisNegative) |
| K15 | Numbers use C `strtoul` base-0 rules; numeric action and section names are accepted with a warning when the ID exists (§12.5.9) | BIN | BIN |
| K16 | Which errors reject the whole file and which drop one binding (§12.7) | BIN | BIN |
| K17 | Actions of toggle type 3 or 6 need a down-type activation (§12.7) | BIN; OBS (types) | BIN |
| K18 | Start-up sequence: reset, load the file, drop every non-user binding, merge four default key maps in a fixed order (§12.8) | BIN; reproduction I1 | Confirmed |
| K19 | Conflict rule and resulting order (§12.8) | BIN; reproduction I1 | Confirmed |
| K20 | Key-map record (0x1D) byte layout (§12.9) | OBS (4 of 4 records, no bytes left over); BIN (string and table headers) | Confirmed |
| K21 | DID-map keys the client uses for KEYMAP, ACTIONMAP and the two enum mappers (§12.2) | OBS (labels); BIN (keys) | Confirmed |
| K22 | Action-map record (0x51) byte layout; table A is the toggle type (§12.10) | OBS (2 of 2 records, no bytes left over, bucket order); BIN (table A) | Confirmed |
| K23 | Labels of toggle types (0x230001C1), action classes (0x230001C2) and movement commands (0x230001C0); their link to tables A and B (§12.10) | OBS (mappers; membership census); BIN (table A) | Confirmed (mappers); link by census |
| K24 | 471 command names in the InputActions mapper chain; 16 input-map names (§12.11) | OBS | Confirmed |
| K25 | The meaning-class rules of §12.11 cover every one of the 471 names | OBS (census) | Confirmed |
| K26 | The original's best-match rule for one input event within one input map (§12.12) | BIN | BIN |
| K27 | The writer's layout, quoting, escaping and spellings (§12.13) | BIN; reproduction I1 | Confirmed (layout and the spellings in the owner's file, including `DIMOFS_Z` for the wheel); BIN (letter case of hexadecimal digits, quoting, the joystick spellings of §12.6.3) |
| K28 | XInput pads are presented as DirectInput-style joystick controls; the state comes from the documented state call, so bits 0x0400 and 0x0800 are never set (§12.6.3) | BIN; DX; PUB | BIN; Reported (what the documented call reports) |
| K29 | Every value of the plugin API's `Action` enumeration (270 values) is an InputActions ID (§12.15) | PLG; OBS | Confirmed (IDs), Reported (plugin names) |
| K30 | `[Input]` preference initial values (§12.16) | BIN; OBS (owner's file holds the same values) | Confirmed |
| K31 | The `<name>c<ext>` and `<name>s<ext>` companion files named by the help text are not read (§12.2) | BIN; HELP (states the opposite) | BIN (corrects HELP) |
| K32 | Asheron's Call retail key maps use the same file family (§12.1) | AC | Reported |
| K33 | An opening bracket with no token before it makes a node with empty text (§12.4) | BIN | BIN |
| K34 | Byte 0xFF stops tokens and lists like the end of input without being the end of input (§12.3) | BIN | BIN |
| K35 | Extra children of the name node, of device entries and of bindings are ignored; so are the children of every node the reader uses only for its text or number (GUID, device index, control name, sub-control word, meta-key mask, activation) (§12.7) | BIN | BIN |
| K36 | Empty tokens: a number reads as 0, an activation as 0; empty command, section, control and sub-control names are unknown (§12.5.7, §12.5.9, §12.7) | BIN; DX (`strtoul`) | BIN |
| K37 | The writer emits an empty control name for a code without a name in the table of the device's type and for every virtual-device control; activations with no word are written empty or left out (§12.13) | BIN | BIN |
| K38 | A top-level `Devices`, `MetaKeys` or `Bindings` node without a child list is an empty part, as with an empty list (§12.5) | BIN | BIN |
| K39 | Directives are recognised by the first byte of the node text, at every level, quoted or not; their names are compared without regard to ASCII case; an unknown directive stays in the tree as an ordinary node; `!error` and `!warning` without a child use an empty message; an `!include` file that is missing or fails to parse is only reported (§12.4) | BIN | BIN |
| K40 | A device index that is not a number makes the control specification invalid, like an index of 256 or more: a binding is dropped, a meta key rejects the file (§12.7) | BIN | BIN |
| K41 | A file whose first four bytes are all 0xAA is rejected before it is parsed (§12.3 rule 8) | BIN | BIN |

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

> **OPEN:** synthetic keyboard events. On focus gain some platforms report every key that
> is already physically down as a *synthetic* press (with text). This section does not say
> what to do with them. The implementation (2026-10-06, engineering choice) drops synthetic
> presses and their text and forwards synthetic releases, so a key held across a focus change
> does not fire edge-triggered actions (`menu`, `open_chat`) or type text, and acts again
> only after it is released and pressed. The alternative, recording such keys as held
> without resolving their bindings (so a held movement key resumes at once), would need a
> new raw event. Confirm or choose.

---

<a id="s11"></a>
## 11. Open items

Resolved on 2026-10-05 (kept so that earlier references still lead somewhere):

- **Resolved:** importing the original client's key-mapping file. See §12.
- **Resolved:** the table between Tessera actions and the plugin API's key-binding action
  IDs. See §12.14.2 (counterparts) and §12.15 (IDs).
- **Resolved:** the terrain height scale. `landblock.md` §3.4: height in world units =
  stored value × 2,000 / 65,535, and world units are metres (`mesh.md` §11.1). The tuning
  numbers of §9.3 were chosen in metres and stay as they are.
- **Resolved (by reference):** Tessera's yaw and the game's compass heading. With +X east and
  +Y north (`coordinates.md` §3) and the heading rule of `coordinates.md` §11.4 (a compass
  bearing measured **clockwise from north**, east = 90°; BIN there, row C11), a controller
  yaw ψ (§9.1, radians, counter-clockwise from +X) shows the compass heading
  h = (90° − ψ · 180/π) mod 360°. Checks: ψ = 0 faces +X (east), h = 90°; ψ = π/2 faces +Y
  (north), h = 0°; ψ = π faces −X (west), h = 270°. A model drawn for the controller is turned
  by ψ − π/2 (§9.1), which is the object yaw of `coordinates.md` §11.3, and that section's
  rule h = (360° − object yaw) mod 360° gives the same value.

> **OPEN:** gamepads: no gilrs adapter yet; all pads are merged into one; stick `+Y`
> direction and trigger ranges need checking per backend. For comparison: the original
> presents each of four XInput pads as a joystick with fixed control codes (§12.6.3), its
> default key maps bind no pad control (§12.9), and its pad dead-zone preference starts at
> 0.25 (§12.16) where Tessera's defaults use 0.15.

> **OPEN:** the walk controller only knows the height field. Collision with static
> objects, water/swimming, and per-race or mounted speeds come with later milestones.

> **OPEN:** IME composition, key auto-repeat for UI actions, and localised key labels for
> display are not covered. For comparison: the original marks its auto-repeating text-cursor,
> delete and focus commands with toggle type 3 (AutoRepeat, §12.10).

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

The open items of the importer are collected in §12.19.

---

<a id="s12"></a>
## 12. Importing the original client's key map

Everything in this section describes the original client and its data. Each statement rests
on a provenance row of §1.2 (K1–K41). Subsections marked **Decision** are Tessera's choices
about how to use those facts.

<a id="s12-1"></a>
### 12.1 Scope

The importer turns the bindings the original client would use for this user into a Tessera
keymap document (§6). It reads, **read-only**:

1. the user's key-mapping file (§12.2), parsed by §12.3–§12.7;
2. the four default key-map records (DID prefix 0x1D, §12.9);
3. the default action-map record (prefix 0x51, §12.10), for one validity rule (§12.7);
4. the enum mappers that name commands and sections (§12.5.4, §12.11).

It builds the original's **effective key map** the way the original does at start-up
(§12.8), then translates the part that has Tessera counterparts (§12.14). It never writes the
original's file, the settings folder or the install. Its result becomes Tessera's keymap only
through an explicit save (§6.5).

The file belongs to a family shared with an older game on the same engine (K32): Asheron's
Call retail key maps have the same four parts and binding layout, with an empty origin token
where this client writes `Old` or `User`.

<a id="s12-2"></a>
### 12.2 Where the data lives

| Item | Location | Notes |
|---|---|---|
| User file | `lotro.keymap` in the per-user settings folder (`install-layout.md` §8.1) | Observed in the owner's Proton prefix (OBS). The file name is an interoperability identifier. |
| Alternative source | the file named by the original's `--keymap <file>` option | Read **instead of** the user file (BIN). If it cannot be loaded, the original logs it and runs on the defaults. |
| What the original writes | always `lotro.keymap` in the settings folder, also when `--keymap` named another file | BIN. Written on shutdown and from the options user interface (BIN; the exact triggers are OPEN). |
| Companion files `<name>c<ext>`, `<name>s<ext>` | not read | The help text describes them (HELP); no load path of this build derives or opens them (K31). |
| Default key maps | DID map **KEYMAP**: root DID map 0x28000000, key 8 (label `KEYMAP`) → 0x2800000C | The original loads these keys, in this order: **0x10000002** (label `gmPartner`) → 0x1D000004, stored in the language archive; **0x10000005** (`gmPCDefault`) → 0x1D000009; **0x10000001** (`gmDefault`) → 0x1D000000; **0x2** (`Default`) → 0x1D000001. Keys 0x1 (`Autotest` → 0x1D00000B) and 0x3 (`Preproc` → 0x1D00000A) name records absent from this install and are not loaded. |
| Default action map | DID map **ACTIONMAP**: root key 6 (label `ACTIONMAP`) → 0x2800000A, key **1** (label `default`) → 0x51000000 | The key-map code of the client uses key 1 only (BIN). Key 2 (`Preproc` → 0x51000001) covers the tool commands of §12.11. |
| Command names | DID map **EMAPPER** (root key 7 → 0x28000001), key **0x1B** (label `InputActions`) → 0x230000AF, whose base mapper is 0x23000176 | Key 0x10000135 (label `InputAction`) names the same mapper. 471 names (§12.11). |
| Section names | EMAPPER key **0x3E** (label `InputMap`) → 0x230000B0, base 0x2300019E | 16 names (§12.5.4). |
| Toggle types, action classes, movement commands | enum mappers 0x230001C1 (toggle types), 0x230001C2 (action classes), 0x230001C0 (movement commands) | Not labelled in DID map 0x28000001; found by DID (OBS). |

The original resolves DID maps by **key** (BIN, K21). The importer uses the same keys and
checks the label; a missing record or a label mismatch is reported and that source is
skipped. DIDs are read at run time, never hard-coded (`records.md` §8.2); the DIDs above are
observations of this install.

The importer opens the user file read-only with shared access and never locks it (the
original denies write sharing while it reads; BIN).

<a id="s12-3"></a>
### 12.3 Lexical syntax

The file is a sequence of bytes. All rules are BIN (K3); I1 (§12.17) confirms the ones the
owner's file exercises.

1. **End of input** is the end of the file or a byte 0x00. The original opens the file in C
   text mode, so a byte 0x1A also ends the input and each CR LF pair arrives as a single LF; a
   CR that is not followed by LF stays a CR (DX: C runtime text-mode reading). A reader that
   reads the raw bytes must apply the same translation itself **before** the rules below:
   stop at the first 0x1A, and replace every CR LF pair by LF. Without it the tokens differ
   in these cases (elsewhere CR is a separator and the tokens agree):
   - a quoted token left unterminated at a line end keeps a trailing CR (rule 4);
   - a `\` directly before a line end escapes the CR instead of the LF: with the
     translation, a bare token `ab\` at a line end continues on the next line with an LF
     byte inside it, and a quoted token continues past the line end; without it, the CR
     becomes a token byte and the LF then ends the bare token or leaves the quoted token
     unterminated (rules 4 and 5);
   - bytes after a 0x1A would be read.
2. **Separators** between tokens: space, TAB (0x09), CR (0x0D), LF (0x0A) and comma (`,`), in
   any number. Commas therefore carry no meaning.
3. **Comments**: outside a quoted token, `#` starts a comment that runs to the end of the
   line. A `#` directly after a bare token ends that token. Comments carry no meaning.
4. **Quoted tokens** start with `"` and end at the next unescaped `"`. Inside, `\` makes the
   next byte literal: `\"` is `"`, `\\` is `\`, and `\n` is the letter `n` (there are no
   other escapes; an escaped LF or 0xFF is part of the token). A quoted token may hold
   separators, `#` and brackets, and may be empty (`""`). A quoted token that reaches an
   unescaped LF, an unescaped 0xFF or the end of input is **unterminated**: the original
   reports it, ends the token there (the LF or 0xFF is not part of it and is still the next
   byte) and continues. A `\` as the last byte of the input is reported the same way.
5. **Bare tokens** run up to the next separator, `"`, `#`, bracket (`[ ] ( ) { }`), byte
   0xFF or the end of input. Inside, `\` makes the next byte part of the token (an LF too);
   a `\` followed by 0xFF or by the end of input is reported, the 0xFF is dropped and the
   token ends there. Every other byte is a token byte, including VT (0x0B), FF (0x0C), `~`
   and the bytes 0x80–0xFE.
6. **Brackets**: `[ ]`, `( )` and `{ }` delimit child lists (§12.4). The three pairs are
   equivalent, but a list must close with the partner of the bracket that opened it.
7. **Byte 0xFF** (K34), outside a comment and not escaped, is not a separator and not the
   end of input, but wherever the parser expects a node or a closing bracket it ends the
   current list as the end of input would. The input has not ended, so the result is always
   fatal (§12.4): *unexpected close* at the top level, *mismatched brackets* inside a list.
   A file holding such a byte is therefore rejected. Inside a comment a 0xFF byte is
   consumed and ends the comment as an LF would; it does no harm there.
8. **Binary marker** (K41). Before parsing, the original reads the first four bytes of the
   file in binary mode, without the translation of rule 1. If there are four and all four
   are 0xAA, the file is rejected without being parsed (the original reports that binary
   node files are not supported). A file shorter than four bytes, or with any other first
   four bytes, is parsed by rules 1–7. Nothing else about binary node files is used here.

Consequences:

- The bare text `DIJOFS_BUTTON(5)` is **not** one token: it is the token `DIJOFS_BUTTON`
  followed by a child list holding `5`. The original writes such names quoted (§12.13).
- A UTF-8 byte-order mark at the start of a file is not a separator. It becomes a token of its
  own, which breaks the four-part rule (§12.5), so the original rejects such a file. The
  original never writes one (K2, `install-layout.md` P40).

<a id="s12-4"></a>
### 12.4 Node tree

- A **node** is a token, optionally followed (after separators and comments) by a bracketed
  list of child nodes. The token is the node's **text**.
- **A node without a token** (K33). Where a node may start (at the top level, as the first
  item of a list, or directly after another node's closing bracket; not directly after a
  token, where the bracket opens that token's list), an opening bracket starts a node whose
  text is **empty**; the bracketed list is its children. This is not an error and is not
  reported: the node is the same as one written `"" [ … ]`. So `A [ x ] [ y ]` is two nodes, `A` with the child `x` and an
  empty-text node with the child `y`; `A [ [ x ] ]` is `A` with one empty-text child, whose
  child is `x`; `[ ]` alone is an empty-text node without children. A token is followed by at
  most one list: in `A B [ x ]`, `A` has no children and `B` has the child `x`. What an
  empty text means in each place of the key-map structure is listed in §12.7.
- The file is a list of **top-level nodes**.
- A closing bracket that is not the partner of the open one, or the end of input inside a
  list, is the fatal error *mismatched brackets*. A closing bracket at the top level is the
  fatal error *unexpected close*. Either one rejects the whole file.
- The original sets no nesting limit. Files it writes nest four levels below a top-level
  node (section, binding, origin node, control parts).
- **Directives** (K39). After a node has been parsed, with its child list if it has one, the
  parser looks at the first byte of its **text**. If that byte is `!`, the node is a
  directive candidate. Text is what rules 4 and 5 of §12.3 produce, so a quoted token
  (`"!error"`) and an escaped one (`\!error`) count the same as a bare one; an empty-text
  node is never a directive. This happens in every list, at the top level and inside child
  lists alike. The whole text is compared with the three directive names below, without
  regard to the case of ASCII letters (`!ERROR` is `!error`; `!errorx` matches nothing):

  | Text | Effect |
  |---|---|
  | `!include` | Its children name another file; that file's top-level nodes take the directive's place in the current list (at whatever level the directive stands). The directive node itself is not kept. A file that cannot be found, or that fails to parse, is **reported and parsing continues** (not fatal); what a failed include contributes is not described here, because the importer never follows `!include` (§12.14.1). Where files are searched is OPEN (§12.19). |
  | `!error` | Fatal error: the file is rejected. The message is the text of the first child; with no child it is empty, and the file is still rejected. Further children are ignored. The node is not kept. |
  | `!warning` | Reported, parsing continues. The message is the text of the first child, or empty with no child. The node is not kept. |
  | any other text starting with `!` | Not an error and not reported: the node **stays in the tree** as an ordinary node, with its text (including the `!`) and its children. The key-map structure then treats it like any other node in that position (§12.7). |

  Because the effect comes after the node is complete, a directive's child list must still be
  well formed: a mismatched bracket inside it is fatal as anywhere else. Directive nodes
  that are not kept do not count as nodes, for example toward the four top-level parts of
  §12.5. The original never writes directives.

<a id="s12-5"></a>
### 12.5 Document structure

The file must have **exactly four** top-level nodes. They are taken by **position**; their
texts are not checked (K5). Any other count rejects the file.

| Position | Text written by the original | Content |
|---|---|---|
| 1 | the key map's name, quoted | name node (§12.5.1) |
| 2 | `Devices` | device list (§12.5.2) |
| 3 | `MetaKeys` | meta-key table (§12.5.3) |
| 4 | `Bindings` | sections of bindings (§12.5.4–§12.5.6) |

Parts 2, 3 and 4 are read through their child lists only. A part **without a child list**
(for example `MetaKeys` with no bracket after it) is the same as a part with an empty list: no devices
(the eight standard devices are still appended, §12.5.2), no meta keys, no sections. It is
not an error and is not reported (K38). The name node differs: it needs a child (§12.5.1).

<a id="s12-5-1"></a>
#### 12.5.1 Name node

Its text is the key map's name. Its first child must be a GUID in one of the forms of
§12.5.2; a missing or unreadable GUID rejects the file. Name and GUID have no other effect:
files written by the original always carry the same fixed name and the all-zero GUID.

<a id="s12-5-2"></a>
#### 12.5.2 Devices

Each child is `<type word> [ <GUID> ]`. A binding or meta key names a device by its **device
index**, the child's 0-based position in this list. After the file's entries, each of the
eight standard devices below whose GUID the file did not list is appended, in table order.
A file with an empty device list therefore gets indices 0–7 in table order.

Type words (matched without regard to case; K6):

| Word | Type code |
|---|---|
| `Keyboard` | 1 |
| `Mouse` | 2 |
| `Joystick` | 3 |
| `Virtual` | 4 |

GUID forms (BIN):

- a symbolic name from the table below, matched without regard to case;
- otherwise the hexadecimal 8-4-4-4-12 form without braces
  (`XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX`, digits in either case), read like a Windows GUID
  string.

An entry with an unknown type word or an unreadable GUID rejects the file.

Standard devices (K7). Every 0x1D record lists exactly these eight, in this order (OBS):

| Order | Symbolic name | Type | GUID |
|---|---|---|---|
| 0 | `GUID_SysKeyboard` | Keyboard | 6F1D2B61-D5A0-11CF-BFC7-444553540000 (DX) |
| 1 | `GUID_SysMouse` | Mouse | 6F1D2B60-D5A0-11CF-BFC7-444553540000 (DX) |
| 2 | `GUID_Virtual` | Virtual | C8854010-4D36-445B-85ED-5DA62CBA27E4 |
| 3 | `GUID_SysJoystick` | Joystick | 1E55E717-F1D3-47AF-8321-996C5CE0C8FD |
| 4 | `GUID_XInput1` | Joystick | 7C20E1A8-78B5-4D0A-83D3-E1D4ACF2B862 |
| 5 | `GUID_XInput2` | Joystick | 57F7ADAD-9A51-4037-ABD1-E9C3F54C9CF9 |
| 6 | `GUID_XInput3` | Joystick | AD847413-69AD-4F20-B003-CC44E72AE90E |
| 7 | `GUID_XInput4` | Joystick | 0113552B-7D27-49E4-AFC7-8B47B54FB2B7 |

`GUID_XInput1`–`4` are the four XInput pad slots (§12.6.3). `GUID_SysJoystick` stands for a
generic DirectInput joystick. The importer reads the GUIDs at run time from the device list of
the KEYMAP records (positions 0–7, in this order) instead of hard-coding them; the values
are listed for identification and for synthetic tests.

<a id="s12-5-3"></a>
#### 12.5.3 Meta keys

Each child is `<n> [ <device> <control> [<sub-control>] ]`: the children of the node `<n>`
form a control specification (§12.5.6). `<n>` is a number (§12.5.9) from 1 to 32 and names
**meta-key bit n**; the device index must exist in the device list. Several controls may
define the same bit, such as the left and right Shift keys. An index outside 1–32, an invalid
control specification or a device index beyond the list rejects the file. The original does
not check that a meta-key control is a button (BIN).

In a binding (§12.5.5), meta-key bit *n* has the value 2^(n−1) in the binding's **meta-key
mask**: bit 1 is 0x1, bit 2 is 0x2, and so on. The original holds masks **bit-reversed**:
bit *n* becomes 2^(32−n), so bit 1 is 0x80000000 (K8). The binary records store the reversed
form (§12.9).

Meta keys of the default key maps (Confirmed; codes per §12.6):

| Bit | Controls | Meaning | In records |
|---|---|---|---|
| 1 | keyboard 0x2A, 0x36 | Shift, left and right | all with meta keys |
| 2 | keyboard 0x1D, 0x9D | Ctrl | all with meta keys |
| 3 | keyboard 0x38, 0xB8 | Alt | all with meta keys |
| 4 | keyboard 0xDB, 0xDC | Windows keys | all with meta keys |
| 5 | mouse 0x0F | fourth mouse button | `gm` records only |
| 6 | mouse 0x10 | fifth mouse button | `gm` records only |
| 7 | mouse 0x0C | left mouse button | `gm` records only |
| 8 | mouse 0x0D | right mouse button | `gm` records only |

<a id="s12-5-4"></a>
#### 12.5.4 Sections (input maps)

Each child of the fourth top-level node is a **section**. Its text names an input map and its
children are bindings (§12.5.5). A section without a child list is valid and empty. The name is
looked up in the InputMap mapper chain without regard to case (BIN); a decimal number that is
an existing input-map ID is accepted with a warning (§12.5.9). An unknown name, or a section
whose input map appeared earlier in the file, **rejects the file** (K9).

Input maps (OBS). The names are interoperability identifiers, read at run time from the
mapper chain of §12.2:

| ID | Name | Apparent use (from the name; Reported) |
|---|---|---|
| 0x0 | `Invalid` | placeholder |
| 0x1 | `DialogBoxes` | dialog navigation |
| 0x2 | `ScrollableControls_Mouse` | wheel over scrollable widgets |
| 0x3 | `DebugConsole` | developer console |
| 0x4 | `IgnoreRemainingMaps` | marker; no bindings in the data |
| 0x5 | `ScrollableControls` | scrollable widgets |
| 0x6 | `EditControls` | text-entry fields |
| 0x7 | `StringTokenDebugger` | developer tool |
| 0x8 | `CopyAndPasteControls` | clipboard in text fields |
| 0x9 | `EatKeyboardInput` | marker; no bindings in the data |
| 0xA | `Main` | the game world |
| 0x10000001 | `SelectionUI` | target selection |
| 0x10000002 | `MusicUI` | music mode |
| 0x10000003 | `MapPanel` | map window |
| 0x10000004 | `VendorBuyUI` | vendor window |
| 0x1000003D | `PVPGameplayUI` | no bindings in the data |

<a id="s12-5-5"></a>
#### 12.5.5 Binding entries

Each child of a section is a binding:

| Part | Content | Required |
|---|---|---|
| node text | the **command name** (§12.11), matched without regard to case in the InputActions mapper chain; a decimal number that is an existing command ID is accepted with a warning | yes |
| child 1 | the **origin node**: its text is the origin token, its children are the control specification (§12.5.6) | yes |
| child 2 | the **meta-key mask** (§12.5.3), a number | no; default 0 |
| child 3 | the **activation** (§12.5.7) | no; default 0x03 (Down + Up) |
| further children | ignored | — |

The **origin token** decides persistence (K10). `User`, matched without regard to case, marks
a binding the user made. Any other text marks a binding copied from the defaults: the
original writes `Old`, and files of the older engine use the empty token `""`. Only `User`
bindings survive loading (§12.8).

An activation can only follow a mask: child 3 needs child 2, so "no meta keys, activation
Tap" is written `0x00000000 Tap`.

<a id="s12-5-6"></a>
#### 12.5.6 Control specification

The origin node (or the meta-key node) has 2 or 3 children:

| Child | Content |
|---|---|
| 1 | device index, a number below 256 (§12.5.9) |
| 2 | control name (§12.6), matched **with** regard to case |
| 3 (optional) | sub-control word (§12.5.8) |

The control name is looked up in all four name tables (keyboard, mouse, joystick, virtual)
whatever the device's type, and its code is used on the indexed device (K11). The binding's
**control value** is a u32: byte 0 = device index, byte 1 = sub-control, bytes 2–3 = control
code as a little-endian u16. The original does not reject a device index beyond the device
list in a binding (BIN); the importer drops such bindings (§12.14.3).

<a id="s12-5-7"></a>
#### 12.5.7 Activations

An activation is one word or several words joined by `/` (`Up/Tap`); its value is the
bitwise OR of the words' values. Words are matched without regard to case; an unknown word
**rejects the file** (§12.7). Values (K13):

| Word | Value | Meaning |
|---|---|---|
| `Down` | 0x01 | button pressed |
| `Up` | 0x02 | button released |
| `Tap` | 0x04 | released after a short press (that release also counts as Up) |
| `DblClickDown` | 0x08 | press that completes a double click |
| `DblClickUp` | 0x10 | release that completes a double click |
| `NearbyDown` | 0x20 | as `DblClickDown`, but only if the pointer stayed close |
| `NearbyUp` | 0x40 | as `DblClickUp`, but only if the pointer stayed close |
| `Click` | 0x03 | Down + Up |
| `Analog` | 0x03 | the same value as `Click` |
| `DblClick` | 0x18 | DblClickDown + DblClickUp |
| `MouseDblClick` | 0x60 | NearbyDown + NearbyUp |

An **empty** activation token (`""`, or an empty-text node in that place, §12.4) has no words
and gives the activation value **0** without any report (K36). Such a binding shares no bit
with any event or any other binding, so it never conflicts (§12.8) and never matches
(§12.12); a command of toggle type 3 or 6 with it is dropped (§12.7).

> **OPEN:** an activation token with an empty word between or after slashes (`Up//Tap`,
> `Up/`, `/Up`). The importer treats such a token like an unknown word (file rejected).

Values found in the default records: 0x03 in 246 bindings, 0x06 in 5, 0x04 in 3, 0x60 in 2
(OBS). The meaning column is functional naming; the double-click and tap timings and the
"close" distance are OPEN (§12.19).

<a id="s12-5-8"></a>
#### 12.5.8 Sub-controls

A sub-control turns part of an analog control into a button (K14). Words are matched without
regard to case:

| Word | Value | Meaning |
|---|---|---|
| `None` | 0 | the whole control |
| `AxisPositive` | 1 | the axis moving in its positive direction |
| `AxisNegative` | 2 | the axis moving in its negative direction |
| `POVUp` | 3 | hat switch up |
| `POVRight` | 4 | hat switch right |
| `POVDown` | 5 | hat switch down |
| `POVLeft` | 6 | hat switch left |

An unknown word makes the control specification invalid. Observed: the mouse wheel with
`AxisPositive` and `AxisNegative` (OBS).

<a id="s12-5-9"></a>
#### 12.5.9 Numbers

Device indices, meta-key indices and masks are read like C `strtoul` with base 0: optional
leading white space (C `isspace`: TAB, LF, VT, FF, CR, space; in a token only VT and FF can
occur unescaped), an optional `+` or `-` (a `-` negates modulo 2^32), then decimal,
hexadecimal after `0x` or `0X`, or octal when the digits start with `0`. The whole token must
be used and the value must fit in 32 bits (K15). An **empty** token passes both checks and
reads as **0** (K36); a token of only white space or only a sign fails. The original writes
masks as `0x` and eight lower-case hexadecimal digits.

A command or section name that is not found by name, is not empty, and is made only of
decimal digits and `-` is converted the same way, but **without** the whole-token and range
checks: conversion stops at the first byte that does not fit (`12-3` is 12, `09` is 0, `-` is
0) and an overflow gives 0xFFFFFFFF. The name is accepted, with a warning, when an entry with
that ID exists; otherwise it is an unknown name. An empty command or section name is always
unknown.

<a id="s12-6"></a>
### 12.6 Control names and codes

Control names are matched **with** regard to case. A name shown as an *alias* is accepted but
never written; for keyboard and mouse the original writes the first name of each row (K12),
for joysticks the spelling given in §12.6.3. The Tessera names are those of §4.

<a id="s12-6-1"></a>
#### 12.6.1 Keyboard (type 1)

The codes are the DirectInput key codes (DX). 137 names plus 15 aliases are accepted:

| Name(s) | Code(s) | Tessera key |
|---|---|---|
| `DIK_ESCAPE` | 0x01 | `Escape` |
| `DIK_1` … `DIK_9`, `DIK_0` | 0x02 … 0x0A, 0x0B | `1` … `9`, `0` |
| `DIK_MINUS`, `DIK_EQUALS` | 0x0C, 0x0D | `Minus`, `Equal` |
| `DIK_BACK` (alias `DIK_BACKSPACE`) | 0x0E | `Backspace` |
| `DIK_TAB` | 0x0F | `Tab` |
| `DIK_Q`, `W`, `E`, `R`, `T`, `Y`, `U`, `I`, `O`, `P` | 0x10 … 0x19 | the same letters |
| `DIK_LBRACKET`, `DIK_RBRACKET` | 0x1A, 0x1B | `BracketLeft`, `BracketRight` |
| `DIK_RETURN` | 0x1C | `Enter` |
| `DIK_LCONTROL` | 0x1D | `LCtrl` |
| `DIK_A`, `S`, `D`, `F`, `G`, `H`, `J`, `K`, `L` | 0x1E … 0x26 | the same letters |
| `DIK_SEMICOLON`, `DIK_APOSTROPHE`, `DIK_GRAVE` | 0x27, 0x28, 0x29 | `Semicolon`, `Quote`, `Backquote` |
| `DIK_LSHIFT`, `DIK_BACKSLASH` | 0x2A, 0x2B | `LShift`, `Backslash` |
| `DIK_Z`, `X`, `C`, `V`, `B`, `N`, `M` | 0x2C … 0x32 | the same letters |
| `DIK_COMMA`, `DIK_PERIOD`, `DIK_SLASH` | 0x33, 0x34, 0x35 | `Comma`, `Period`, `Slash` |
| `DIK_RSHIFT` | 0x36 | `RShift` |
| `DIK_NUMPADSTAR` (alias `DIK_MULTIPLY`) | 0x37 | `NumpadMultiply` |
| `DIK_LALT` (alias `DIK_LMENU`) | 0x38 | `LAlt` |
| `DIK_SPACE` | 0x39 | `Space` |
| `DIK_CAPSLOCK` (alias `DIK_CAPITAL`) | 0x3A | `CapsLock` |
| `DIK_F1` … `DIK_F10` | 0x3B … 0x44 | `F1` … `F10` |
| `DIK_NUMLOCK`, `DIK_SCROLL` | 0x45, 0x46 | `NumLock`, `ScrollLock` |
| `DIK_NUMPAD7`, `DIK_NUMPAD8`, `DIK_NUMPAD9` | 0x47, 0x48, 0x49 | `Numpad7`, `Numpad8`, `Numpad9` |
| `DIK_NUMPADMINUS` (alias `DIK_SUBTRACT`) | 0x4A | `NumpadSubtract` |
| `DIK_NUMPAD4`, `DIK_NUMPAD5`, `DIK_NUMPAD6` | 0x4B, 0x4C, 0x4D | `Numpad4`, `Numpad5`, `Numpad6` |
| `DIK_NUMPADPLUS` (alias `DIK_ADD`) | 0x4E | `NumpadAdd` |
| `DIK_NUMPAD1`, `DIK_NUMPAD2`, `DIK_NUMPAD3`, `DIK_NUMPAD0` | 0x4F, 0x50, 0x51, 0x52 | `Numpad1`, `Numpad2`, `Numpad3`, `Numpad0` |
| `DIK_NUMPADPERIOD` (alias `DIK_DECIMAL`) | 0x53 | `NumpadDecimal` |
| `DIK_OEM_102` | 0x56 | `IntlBackslash` |
| `DIK_F11`, `DIK_F12` | 0x57, 0x58 | `F11`, `F12` |
| `DIK_F13`, `DIK_F14`, `DIK_F15` | 0x64, 0x65, 0x66 | `F13`, `F14`, `F15` |
| `DIK_NUMPADENTER`, `DIK_RCONTROL` | 0x9C, 0x9D | `NumpadEnter`, `RCtrl` |
| `DIK_NUMPADSLASH` (alias `DIK_DIVIDE`) | 0xB5 | `NumpadDivide` |
| `DIK_SYSRQ` | 0xB7 | `PrintScreen` |
| `DIK_RALT` (alias `DIK_RMENU`) | 0xB8 | `RAlt` |
| `DIK_PAUSE` | 0xC5 | `Pause` |
| `DIK_HOME`, `DIK_END` | 0xC7, 0xCF | `Home`, `End` |
| `DIK_UPARROW` (alias `DIK_UP`), `DIK_DOWNARROW` (alias `DIK_DOWN`) | 0xC8, 0xD0 | `Up`, `Down` |
| `DIK_LEFTARROW` (alias `DIK_LEFT`), `DIK_RIGHTARROW` (alias `DIK_RIGHT`) | 0xCB, 0xCD | `Left`, `Right` |
| `DIK_PGUP` (alias `DIK_PRIOR`), `DIK_PGDN` (alias `DIK_NEXT`) | 0xC9, 0xD1 | `PageUp`, `PageDown` |
| `DIK_INSERT`, `DIK_DELETE` | 0xD2, 0xD3 | `Insert`, `Delete` |
| `DIK_LWIN`, `DIK_RWIN`, `DIK_APPS` | 0xDB, 0xDC, 0xDD | `LSuper`, `RSuper`, `ContextMenu` |
| `DIK_ABNT_C1` 0x73, `DIK_YEN` 0x7D, `DIK_ABNT_C2` 0x7E, `DIK_NUMPADEQUALS` 0x8D, `DIK_PREVTRACK` 0x90, `DIK_AT` 0x91, `DIK_COLON` 0x92, `DIK_UNDERLINE` 0x93, `DIK_STOP` 0x95, `DIK_AX` 0x96, `DIK_UNLABELED` 0x97, `DIK_NEXTTRACK` 0x99, `DIK_MUTE` 0xA0, `DIK_CALCULATOR` 0xA1, `DIK_PLAYPAUSE` 0xA2, `DIK_MEDIASTOP` 0xA4, `DIK_VOLUMEDOWN` 0xAE, `DIK_VOLUMEUP` 0xB0, `DIK_WEBHOME` 0xB2, `DIK_NUMPADCOMMA` 0xB3, `DIK_WEBSEARCH` 0xE5, `DIK_WEBFAVORITES` 0xE6, `DIK_WEBREFRESH` 0xE7, `DIK_WEBSTOP` 0xE8, `DIK_WEBFORWARD` 0xE9, `DIK_WEBBACK` 0xEA, `DIK_MYCOMPUTER` 0xEB, `DIK_MAIL` 0xEC, `DIK_MEDIASELECT` 0xED | as given | none: §4.1 has no such key |

Public DirectInput names missing from this list are **not** accepted by the original (for
example the Japanese input keys, `DIK_CIRCUMFLEX`, `DIK_POWER`, `DIK_SLEEP`, `DIK_WAKE`): a
binding using one has an invalid control specification.

<a id="s12-6-2"></a>
#### 12.6.2 Mouse (type 2)

Codes are DirectInput mouse-state offsets (DX).

| Name(s) | Code | Kind | Tessera |
|---|---|---|---|
| `DIMOFS_X` | 0x00 | relative axis, positive to the right | axis `MouseX` |
| `DIMOFS_Y` | 0x04 | relative axis, positive downwards | axis `MouseY` |
| `DIMOFS_Z`, `DIMOFS_WHEEL` (both accepted; `DIMOFS_Z` is written) | 0x08 | wheel, positive when turned away from the user | axis `WheelY`; with `AxisPositive` the click `WheelUp`, with `AxisNegative` the click `WheelDown` |
| `DIMOFS_BUTTON0` … `DIMOFS_BUTTON7` | 0x0C … 0x13 | buttons | `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`, `Mouse6`, `Mouse7`, `Mouse8` |

<a id="s12-6-3"></a>
#### 12.6.3 Joystick (type 3) and XInput pads

Codes are the DirectInput joystick-state offsets (DX), plus two codes of the original's own:

| Name(s) | Code(s) |
|---|---|
| `DIJOFS_X`, `DIJOFS_Y`, `DIJOFS_Z` | 0x00, 0x04, 0x08 |
| `DIJOFS_RX`, `DIJOFS_RY`, `DIJOFS_RZ` | 0x0C, 0x10, 0x14 |
| `DIJOFS_SLIDER0` or `DIJOFS_SLIDER(0)`; `DIJOFS_SLIDER1` or `DIJOFS_SLIDER(1)` | 0x18; 0x1C |
| `DIJOFS_POV0` … `DIJOFS_POV3` or `DIJOFS_POV(0)` … `DIJOFS_POV(3)` | 0x20, 0x24, 0x28, 0x2C |
| `DIJOFS_BUTTON0` … `DIJOFS_BUTTON31` or `DIJOFS_BUTTON(0)` … `DIJOFS_BUTTON(31)` | 0x30 … 0x4F |
| `DIJOFS_STICK0`, `DIJOFS_STICK1` | 0x400, 0x404: a whole stick as one two-dimensional control (not DirectInput names) |

Both spellings of a code are accepted. The writer emits, for each code, the first name it
finds in its joystick name table, whose traversal order is unrelated to the code (K27; BIN).
The result per code:

| Codes | Written spelling |
|---|---|
| 0x00–0x14, 0x400, 0x404 | the only name (`DIJOFS_X` … `DIJOFS_RZ`, `DIJOFS_STICK0`, `DIJOFS_STICK1`) |
| 0x18, 0x1C | `"DIJOFS_SLIDER(0)"`, `"DIJOFS_SLIDER(1)"` |
| 0x20, 0x24, 0x28, 0x2C | `DIJOFS_POV0`, `"DIJOFS_POV(1)"`, `DIJOFS_POV2`, `DIJOFS_POV3` |
| buttons 0, 3, 4, 6, 7, 8, 10, 12, 15, 17, 18, 19, 22, 28, 29, 31 (code 0x30 + n) | with brackets, quoted: `"DIJOFS_BUTTON(n)"` |
| buttons 1, 2, 5, 9, 11, 13, 14, 16, 20, 21, 23, 24, 25, 26, 27, 30 (code 0x30 + n) | without brackets: `DIJOFS_BUTTONn` |

Spellings with brackets are written quoted (§12.13). The same traversal rule gives `DIMOFS_Z`
rather than `DIMOFS_WHEEL` for the mouse wheel, which the owner's file confirms (OBS); the
joystick rows are BIN only, because no default binding and no binding in the owner's file
uses a joystick. A Tessera writer that wants files identical to the original's uses this
table; a reader accepts both spellings either way.

For the four XInput slots (`GUID_XInput1`–`4`) the original presents pad state as these
controls (K28; XInput bit values are DX):

| Pad element (XInput button bit) | Control | Tessera |
|---|---|---|
| A (0x1000) | `DIJOFS_BUTTON0` | `PadSouth` |
| B (0x2000) | `DIJOFS_BUTTON1` | `PadEast` |
| X (0x4000) | `DIJOFS_BUTTON2` | `PadWest` |
| Y (0x8000) | `DIJOFS_BUTTON3` | `PadNorth` |
| left shoulder (0x0100) | `DIJOFS_BUTTON4` | `PadL1` |
| right shoulder (0x0200) | `DIJOFS_BUTTON5` | `PadR1` |
| Back (0x0020) | `DIJOFS_BUTTON6` | `PadSelect` |
| Start (0x0010) | `DIJOFS_BUTTON7` | `PadStart` |
| left stick press (0x0040) | `DIJOFS_BUTTON8` | `PadL3` |
| right stick press (0x0080) | `DIJOFS_BUTTON9` | `PadR3` |
| bit 0x0400 | `DIJOFS_BUTTON10` | `PadMode` (**Decision**, see below) |
| bit 0x0800 | `DIJOFS_BUTTON11` | none |
| D-pad (bits 0x0001–0x0008) | hat `DIJOFS_POV0`: hundredths of a degree clockwise from up (up 0, up-right 4,500, right 9,000, …, up-left 31,500), −1 when centred or for impossible combinations | `POVUp`, `POVRight`, `POVDown`, `POVLeft` → `PadUp`, `PadRight`, `PadDown`, `PadLeft` |
| left trigger | `DIJOFS_Z` | axis `PadLeftTrigger`; with `AxisPositive` the button `PadL2` |
| right trigger | `DIJOFS_RZ` | axis `PadRightTrigger`; with `AxisPositive` the button `PadR2` |
| left stick | `DIJOFS_X`, `DIJOFS_Y`, and `DIJOFS_STICK0` | axes `PadLeftX`, `PadLeftY`; `STICK0`: none |
| right stick | `DIJOFS_RX`, `DIJOFS_RY`, and `DIJOFS_STICK1` | axes `PadRightX`, `PadRightY`; `STICK1`: none |

The sticks use the standard XInput thumb dead zones (left 7,849, right 8,689; DX) before
rescaling. The value ranges and the sign of the stick Y axes are OPEN (§12.19).

**Bits 0x0400 and 0x0800** (K28). The original maps them to buttons 10 and 11 (BIN), but it
reads pad state through the documented XInput state call, looked up by name (BIN). The
public XInput header names neither bit; bit 0x0400 is the guide (centre) button, which only
an undocumented extended state call reports, and 0x0800 is unused (PUB, Reported). So in the
original these two controls never become pressed, and a binding on them never fires.

**Decision** (`DIJOFS_BUTTON10` → `PadMode`): the importer still translates a binding on
button 10 of an XInput slot to Tessera's `PadMode`, because bit 0x0400 is the guide button,
which is what `PadMode` names (§4.2), and a user who wrote such a binding by hand meant that
button. The import report marks each such entry with a warning that the original would
never have fired it. Button 11 has no Tessera counterpart and such a binding is dropped.

<a id="s12-6-4"></a>
#### 12.6.4 Virtual device (type 4)

One name: `DIV_MOUSELOOK`, code 0x01, a control that follows the mouse-look state (meaning
Reported from the name). No binding in the data uses it. Tessera: none.

<a id="s12-7"></a>
### 12.7 Errors

What the original does on each problem (K16, K17):

| Problem | Effect |
|---|---|
| Mismatched brackets, a closing bracket at the top level, an `!error` directive (with or without a child) | file rejected |
| An unescaped byte 0xFF outside a comment (§12.3 rule 7) | file rejected |
| First four bytes all 0xAA (§12.3 rule 8) | file rejected, not parsed |
| `!warning` directive (with or without a child); an `!include` file missing or failing to parse | reported; parsing continues (§12.4) |
| A node whose text starts with `!` but is not one of the three directive names | not reported; kept as an ordinary node (§12.4), then treated by the rows below like any node in its position |
| Not exactly four top-level nodes | file rejected |
| Top-level part 2, 3 or 4 without a child list | empty part, not reported (§12.5, K38) |
| Name node: missing child list, so no GUID; unreadable GUID | file rejected |
| Name node: children after the first; children of the GUID node; empty text | ignored, not reported (K35) |
| Device entry: unknown or empty type word; missing child list, so no GUID; unreadable GUID | file rejected |
| Device entry: children after the GUID; children of the GUID node | ignored, not reported (K35) |
| Device entry: a GUID listed twice in the file | not checked: each entry gets its own index |
| Meta key: index outside 1–32 (an empty index reads as 0; an index that is not a number counts as 0), invalid control specification (including a device index that is not a number, K40), device index beyond the list | file rejected |
| Section: unknown or empty name, or an input map that already had a section | file rejected |
| Binding: meta-key mask that is not a number (for example an activation word in its place) | file rejected |
| Binding: unknown activation word | file rejected |
| Binding: empty mask, empty device index, empty activation | read as 0 (§12.5.7, §12.5.9); not reported |
| Binding: unknown or empty command name | binding dropped, reported |
| Binding: missing origin node; control specification with other than 2 or 3 children, device index that is not a number (`User [ x DIK_A ]`, K40) or is 256 or more, unknown or empty control name, unknown or empty sub-control word | binding dropped, reported |
| Children attached to a node that is read only for its text or number: a device index, control name, sub-control word, meta-key mask or activation node (`DIK_A ( x )`, `0x00000000 [ y ]`), and a GUID node | ignored, not reported; the node's own text is used as if it had no children (K35) |
| Binding: children after the third (§12.5.5) | ignored, not reported (K35) |
| Binding: empty origin token | accepted as a non-user binding (as in the older engine's files, §12.5.5) |
| Binding: command of toggle type 3 (AutoRepeat) or 6 (Continuous) (§12.10) whose activation has none of Down, DblClickDown, NearbyDown | binding dropped, reported |
| Binding that conflicts with an earlier one (§12.8) | later binding dropped, not reported |
| Unterminated quoted token; `\` at the end of input; `\` followed by 0xFF in a bare token | reported; parsing continues |
| Numeric command or section name that is an existing ID | accepted, warning |

Empty texts arise from `""` or from a node without a token (§12.4); the table treats both
alike. An empty-text node in a position of the structure gets the effect of an empty text in
that position: as the name node it is accepted, as a device entry or section it rejects the
file, as a meta key it rejects the file (index 0), as a binding it is dropped (unknown
command), as an origin node it gives a non-user binding.

Child lists are looked at only where the structure has them: the name node (first child),
the three parts, device entries (first child), meta-key nodes, sections, bindings (up to
three children) and origin nodes (2 or 3 children). Every other node is read for its text or
number alone, and a child list attached to it changes nothing (K35).

An unknown directive kept as an ordinary node (§12.4) is handled by its position: as a fifth
top-level node it makes the count wrong (file rejected); as a device entry, section or meta
key it is an unknown type word, section name or index (file rejected); as a binding it is an
unknown command (binding dropped, reported); as an origin token it gives a non-user binding;
as a control name it is unknown (binding dropped).

When the file is rejected, the original runs on the default key maps alone (§12.8, step 2
contributes nothing).

<a id="s12-8"></a>
### 12.8 The effective key map

At start-up the original builds its key map like this (K18):

1. Start from an empty key map.
2. Load the user file (or the `--keymap` file) into a separate key map by §12.3–§12.7. If the
   file is missing or rejected, go to step 4. Otherwise merge it into the empty key map with
   the rules of step 5.
3. Remove every binding whose origin is not `User`. Devices and meta keys from the file stay.
4. Merge the default key maps, in the order of §12.2: KEYMAP keys 0x10000002, 0x10000005,
   0x10000001, 0x2. A missing record is skipped.
5. Merging a source key map into the target, in source order:
   1. **Devices.** A source device whose GUID equals a target device's GUID is mapped to it
      (if their type codes differ, the source's type replaces the target's); otherwise it is
      appended to the target's list. The source's meta keys and bindings use the mapped
      indices.
   2. **Meta keys.** A source meta key whose control the target already has is skipped (the
      target's bit stays); otherwise it is appended.
   3. **Sections.** A section the target lacks is appended, empty, at the end of the target's
      section list.
   4. **Bindings.** Each source binding is added to its section by step 6.
6. **Adding binding N to a section.** If an existing binding E of that section **conflicts**
   with N, N is discarded and E moves to the **end** of the section's list, keeping its origin.
   Otherwise N is appended. Two bindings conflict when they have the same control value
   (§12.5.6), the same meta-key mask, and activation values that share at least one bit (K19).
   Loading the file in step 2 adds its bindings with the same rule, so of two conflicting
   bindings in a file the first one is kept.

Consequences of steps 1–6 (the owner's file shows the first, second and third):

- A user binding replaces every default binding it conflicts with and takes the position of
  the last default it displaced.
- A default binding is still added when its control is free, even if the user has bound the
  same command elsewhere. To remove a default binding, the original binds the control to the
  command `DoNothing` with origin `User` (observed in the owner's file). The importer treats
  `DoNothing` as "no command" (§12.14).
- Bindings with origin `Old` in the file never count: step 3 drops them and step 4 recreates
  the current defaults.
- The file's meta keys and devices persist; default meta keys for other controls are added.
- Sections appear in the order first seen: the file's sections, then each record's new
  sections in record order.

Observed result without a user file (I2; observations of this install, not constants): 12
sections and 254 bindings (`Main` 186, `EditControls` 18, `SelectionUI` 1, `MusicUI` 25,
`MapPanel` 0, `VendorBuyUI` 6, `ScrollableControls` 0, `ScrollableControls_Mouse` 2,
`CopyAndPasteControls` 6, `DialogBoxes` 7, `DebugConsole` 2, `StringTokenDebugger` 1), built
from 256 record bindings of which 2 are dropped as conflicts, and 12 meta-key entries.

<a id="s12-9"></a>
### 12.9 Key-map records (prefix 0x1D)

All integers are little-endian. Layout (K20):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | u32 | own DID (`records.md` §1.6) |
| 2 | name | Pascal string (`records.md` §1.4: VLE length, then bytes) | internal name; not used |
| 3 | guid | 16 bytes | key-map GUID in Windows byte order (u32, u16, u16, 8 bytes); not used |
| 4 | device_count | u32 | number of device entries |
| 5 | devices | device × device_count | device list (§12.5.2) |
| 6 | meta_keys | table of meta-key entries | meta-key table (§12.5.3) |
| 7 | input_maps | table of input-map entries | sections (§12.5.4) |

Device entry (17 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 1 | u8 | type code (§12.5.2) |
| 1 | 16 | GUID | device GUID |

**Table** (the key-map records' own collection header; not the TSize of `records.md` §1.3):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | buckets | VLE | the writer's hash-bucket count (11, 23 or 89 observed) |
| 2 | n | VLE | element count |
| 3 | elements | element × n | in list order (the order of §12.8), **not** bucket order |

Readers use only *n*. The observed bucket counts are, for every table, the smallest of 11,
23, 47, 89, … with 2 × buckets ≥ *n* (OBS; it matters only to a writer).

Meta-key entry (8 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 | control value (§12.5.6) |
| 4 | 4 | u32 | meta-key bit in reversed form: exactly one bit set, bit *n* stored as 2^(32−n) |

Input-map entry:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | id | u32 | input-map ID (§12.5.4) |
| 2 | mappings | table of mapping entries | the section's bindings |

Mapping entry (16 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 | control value (§12.5.6) |
| 4 | 4 | u32 | meta-key mask, reversed form |
| 8 | 4 | u32 | activation (§12.5.7) |
| 12 | 4 | u32 | command ID (§12.11) |

Bindings read from records have origin `Old`.

Census (Confirmed, every 0x1D record of this install):

| DID | Archive | KEYMAP key (label) | Devices | Meta keys | Sections | Bindings |
|---|---|---|---|---|---|---|
| 0x1D000000 | general | 0x10000001 (`gmDefault`) | 8 | 12 | 6 | 178 |
| 0x1D000001 | general | 0x2 (`Default`) | 8 | 8 | 8 | 77 |
| 0x1D000004 | local_English | 0x10000002 (`gmPartner`) | 8 | 12 | 0 | 0 |
| 0x1D000009 | general | 0x10000005 (`gmPCDefault`) | 8 | 12 | 1 | 1 |

All 256 record bindings are on device 0 (keyboard, 233) or device 1 (mouse, 23); no record
binds a pad, joystick or virtual control.

<a id="s12-10"></a>
### 12.10 Action-map records (prefix 0x51)

Layout (K22):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | u32 | own DID |
| 2 | strings | DID | a string table (prefix 0x25) holding the texts of table C |
| 3 | n_A | TSize (`records.md` §1.3) | |
| 4 | table A | (u32 command ID, u32 toggle type) × n_A | toggle type per command |
| 5 | n_B | TSize | |
| 6 | table B | (u32 command ID, u32 action class, u32 target, f32 scale) × n_B | 16 bytes each |
| 7 | n_C | TSize | |
| 8 | table C | (u32 command ID, u32 token, u32 second token) × n_C | 12 bytes each |

Census (Confirmed): 0x51000000 (ACTIONMAP key 1): A 364 (hint 4), B 364 (hint 4), C 93
(hint 2). 0x51000001 (key 2): A 53, B 53, C 0 (hints 2, 2, 0). Both parse with no bytes left
over, every table is in bucket order of its command IDs, and tables A and B have the same
command set (I4).

**Toggle types** (table A; labels from enum mapper 0x230001C1, K23):

| Code | Label | Behaviour | Commands in 0x51000000 |
|---|---|---|---|
| 0 | `Invalid` | the command is not in the map | — |
| 1 | `Momentary` | active while held | 91 |
| 2 | `Impulse` | acts once per activation | 250 |
| 3 | `AutoRepeat` | repeats while held; needs a down-type activation (§12.7) | 16 |
| 4 | `Analog` | proportional input | 4 |
| 5 | `Toggle` | each activation flips a state | 3 |
| 6 | `Continuous` | needs a down-type activation (§12.7) | 0 |

Only the down-type rule is BIN; the other behaviours are read from the labels and the
membership: forward, backward, strafe, turn and jump commands are Momentary, run lock and
panel switches are Impulse, the turn-or-strafe and roll commands Analog, text-cursor keys
AutoRepeat, walk mode Toggle.

**Action classes** (table B, second field; labels from 0x230001C2): 0 `Invalid`, 1 `weenie`,
2 `emote`, 3 `movement`, 4 `UI`, 5 `camera`. In 0x51000000: movement 22, UI 321, camera 21;
0x51000001: UI 53.

**Target** (third field): for class 3 a movement command from mapper 0x230001C0 (below); for
classes 4 and 5 a command ID, the entry's own ID in 341 of 342 entries (one developer
command targets another). **Scale**: 1.0, except one movement entry with 0.5.

Movement commands (labels in 0x230001C0; descriptions here are functional): 1 jump, 2 stop,
3 turn left, 4 strafe left, 5 dodge left, 6 dodge right, 7 turn around, 8 forward, 9 turn or
strafe right, 10 flip right, 11 backward, 12 run lock, 13 turn or strafe left, 14 sneak (the
walk-mode command targets it), 15 strafe right, 16 flip left, 17 turn right.

**Table C** pairs commands with text tokens of the string table named in field 2. Only 35 of
the 93 first tokens exist in that table in this install; the role of table C is OPEN. The
importer does not use it.

<a id="s12-11"></a>
### 12.11 Command names and meaning classes

The **InputActions** chain (K24): mapper 0x230000AF holds 290 names with IDs from 0x10000001
up (game commands), its base 0x23000176 holds 181 names with IDs 0x0–0xD0 (engine commands);
no ID is in both. Of the 471 commands, 364 are in the default action map (with a class and a
toggle type), 53 in the tool map (51 tool-only, plus the two pointer axes), and 56 in neither.

The **meaning class** of a command name follows from the rules below, applied in order; the
first rule that matches wins. Names are compared as stored (with case). The rules cover every
one of the 471 names (K25; I5). The class is Tessera's vocabulary for deciding what to
import.

| # | Class | Rule on the command name | Count | Default action map | Tessera use |
|---|---|---|---|---|---|
| 1 | none | `Invalid`, `DoNothing` | 2 | absent | `DoNothing` occupies a control; never an action |
| 2 | tool | starts with `Preproc` | 51 | tool map only | ignored |
| 3 | legacy | starts with `WEENIE_`, `Alternate`, `Test`, `AbsPointer` or `RelPointer`; or `EmoteWave` | 29 | absent | ignored |
| 4 | developer | starts with `ADMIN_`; or one of `ToggleAdminPanel`, `ToggleDebugConsole`, `ToggleDebugHUD`, `ToggleProfiler`, `TogglePerfGraph`, `ToggleMemoryGraph`, `ToggleBlockUI`, `ToggleEntityNodeLabels`, `ToggleConsolePanel`, `InstantStringTokenDebugger`, `ToggleStringTokenDebugger` | 12 | 11 UI, 1 absent | `ToggleDebugHUD` only |
| 5 | movement | starts with `Movement` | 22 | 22 movement | §12.14.2 |
| 6 | camera | starts with `Camera` | 25 | 21 camera, 3 UI, 1 absent | §12.14.2 |
| 7 | pointer | `PointerX`, `PointerY` | 2 | UI (also in the tool map) | look axes |
| 8 | mouse click | `SelectLeft`, `SelectRight`, `SelectMid`, `SelectDblLeft`, `SelectDblRight`, `SelectDblMid`, `TapLeft`, `TapRight`, `TapMid` | 9 | UI | none (user-interface clicks) |
| 9 | quick slots | starts with `QUICKSLOT_` or `Quickslot_Page_` | 88 | UI | none yet |
| 10 | music | starts with `MUSIC_`; `MusicEndSong`, `ToggleMusicMode` | 39 | UI | none yet |
| 11 | targeting | starts with `SELECTION_`, `SelectFellow`, `AssistFellow`, `ToggleTargetMark` or `Conjunction_Contribution_`; `FOLLOW` | 49 | 46 UI, 3 absent | none yet |
| 12 | text and dialogs | starts with `Cursor`, `Focus`, `Scroll` or `DIALOGBOX_`; `CopyText`, `CutText`, `PasteText`, `EscapeKey`, `DeleteKey`, `BackspaceKey`, `AcceptKey`, `AcceptInput` | 34 | 28 UI (16 AutoRepeat), 6 absent | part, §12.14.2 |
| 13 | chat | `START_COMMAND`, `START_ALIAS`, `ChatModeReply`, `EnterChatMode`, `VOICECHAT_TALK`, `ItemLinkToChat` | 6 | 5 UI, 1 absent | none yet |
| 14 | session | `LOGOUT`, `EXITGAME`, `AltTab`, `LeaveSessionPlay`, `RESURRECT` | 5 | 3 UI, 2 absent | none |
| 15 | panels | starts with `Toggle`, `Show`, `SHOW_` or `SelectOutfit`; `TITLE_VIEW`, `UI_TOGGLE`, `TOOLTIP_DETACH`, `DisplayActiveQuest`, `CloseBankPanel`, `COMBOUI_TOGGLE`, `PendingLootUI`, `ClearFilters`, `FindNearbyQuests`, `RemoteQuestAction` | 88 | 78 UI, 10 absent | none yet |
| 16 | other game commands | everything else | 10 | 9 UI, 1 absent | `CaptureScreenshot` only |

The names themselves are read at run time from the mapper chain; only those Tessera maps
(§12.14.2) and those the rules name appear in this spec.

<a id="s12-12"></a>
### 12.12 How the original picks a binding (reference)

For one input event on a control with activation bits *A* (for example Down for a press),
while the meta-key controls held give the meta state *S*, the original looks in **one** input
map for bindings whose control value matches, whose mask is a subset of *S*, and whose
activation shares a bit with *A*. Among those it takes the one with the numerically largest
activation value; then the one with more mask bits; then the one whose reversed mask is
numerically larger, which is the one whose lowest meta-key bit has the lower number (K26).
So with bindings for X, Shift+X and Ctrl+X, Shift+Ctrl+X selects Shift+X (Shift is bit 1,
Ctrl bit 2), and Alt+X selects X.

> **OPEN:** when each input map is active and in which order the original consults them
> (BIN not done). Not needed by the importer, which takes bindings by section.

<a id="s12-13"></a>
### 12.13 Files the original writes

The original writes its effective key map (§12.8) as follows (K27; I1 confirms every point
the owner's file exercises; the letter case of hexadecimal digits, quoting and escaping are
BIN only):

- ASCII text, CRLF line ends (written in C text mode), no byte-order mark, final CRLF.
- First the original's fixed explanatory comment, one `#` per line (an empty line is a lone
  `#`). Its text is not needed and not reproduced here.
- Then the four parts: the name node (a fixed name, quoted, then `[`, the all-zero GUID in
  8-4-4-4-12 form, `]`), and `Devices`, `MetaKeys` and `Bindings`, each on its own line,
  followed by `[` on the next line, its entries, and `]`. An empty line separates the four
  parts.
- Device and meta-key entries are indented by two spaces; section names by two spaces,
  with their `[`, bindings indented by four, and `]`; an empty line separates sections. A
  section without bindings is written as its name alone.
- Entries are written on one line with single spaces: `Keyboard [ GUID_SysKeyboard ]`,
  `1 [ 0 DIK_LSHIFT ]`, `Command [ Origin [ device control ] ]`.
- A binding gets ` 0x` + eight lower-case hexadecimal digits (the mask in file form) when its
  mask is not zero **or** its activation is not 0x03, and then the activation word(s) when
  the activation is not 0x03. A sub-control word follows the control name inside the inner
  brackets.
- Activation spelling: the words of §12.5.7 are tried in the order `Click`, `Analog`,
  `DblClick`, `MouseDblClick`, `Down`, `Up`, `Tap`, `DblClickDown`, `DblClickUp`,
  `NearbyDown`, `NearbyUp`; a word is used when all its bits are still present, and its bits
  are then removed; the words used are joined with `/`. So 0x06 is written `Up/Tap`, 0x60
  `MouseDblClick` and 0x07 `Click/Tap`.
- Bit 31 of an activation is ignored. Bits outside 0x7F that no word covers are dropped
  when at least one word was written; when no word applies at all, an activation of 0 is
  written as the empty token `""` (which reads back as 0, §12.5.7), and an activation made
  only of bits outside 0x7F is not written, so it reads back as the default 0x03 (K37).
- Control names: the first name of each row of §12.6 for keyboard and mouse, the spelling of
  the table in §12.6.3 for joysticks, looked up **only** in the name table of the device's
  type (§12.5.2); device types as the words of §12.5.2; device GUIDs as symbolic names, other
  GUIDs in 8-4-4-4-12 form with upper-case hexadecimal digits.
- A control code without a name in that table, and **every** control of a virtual device
  (type 4; the writer has no name table for it, so `DIV_MOUSELOOK` is never written), is
  written as the empty control name `""` (K37). Reading such a file back drops the binding
  (unknown control name, §12.7). A meta-key entry written this way makes the whole file
  rejected on the next start (§12.7). Examples: `DIV_MOUSELOOK` on device 2; a keyboard
  device bound with a joystick-only name such as `DIJOFS_STICK0`, which the reader accepts
  because it searches all four tables (§12.5.6).
- A token is written in double quotes when it is empty or contains a space, TAB, `#`, `,` or
  a bracket; `"` and `\` inside a token are preceded by `\`.

<a id="s12-14"></a>
### 12.14 Import into Tessera (Decision)

<a id="s12-14-1"></a>
#### 12.14.1 Procedure

1. Find the source file: the user file of §12.2, or the file given to Tessera through the
   original's `--keymap` option (see §13, item 2, for the option's name clash). Read at most
   1 MiB; a larger file is not imported.
2. Check the binary marker of §12.3 rule 8 on the raw bytes (four bytes 0xAA: rejected).
   Then apply the text-mode translation of §12.3 rule 1 to the bytes read (stop at 0x1A; CR LF
   becomes LF), then parse by §12.3–§12.7, with two limits of Tessera's own: nesting depth 16
   (empty-text nodes of §12.4 count as levels like any other node) and token length 4,096
   bytes; exceeding either rejects the file. Directives are recognised and matched as in
   §12.4. `!include`, in any letter case and at any level, is not followed: the file is
   treated as rejected and the report says why. `!warning` is reported with its message
   (empty if it has no child) and removed; `!error` rejects the file, with or without a
   child, as in the original. Any other node whose text starts with `!` is kept as an
   ordinary node, as in the original, and gives the result §12.7 states for its position.
3. Build the effective key map by §12.8 from the file (or none, if rejected) and the four
   records reached through the DID maps.
4. Translate the bindings of the sections named in §12.14.2, in effective order, by
   §12.14.3–§12.14.5.
5. Produce a Tessera keymap document (§6.1): `format = "tessera-keymap"`, `version = 1`, and
   a list for each Tessera (context, action) of §12.14.2:
   - the translated keyboard and mouse entries, in effective order;
   - then the pad entries: the translated ones when the effective map binds a pad control for
     the action's commands, otherwise the pad entries of Tessera's defaults (§8), because the
     original's default key maps bind no pad control (§12.9);
   - exact duplicates removed, at most 16 entries (§6.2; the rest dropped with a warning).

   When none of the action's commands has any binding in the effective map, the list holds
   only those default pad entries; it may be empty, which unbinds the action as in the
   original. When the commands have bindings but none of them translates, the action is
   **left out**, so Tessera's whole default list stays, and a warning names it.
6. Return the document and a report: whether the file was rejected and why, the counts of
   imported and dropped bindings, and the warnings (unknown commands, each dropped binding
   with its reason, and each stroke an imported entry shares with a default that another
   action of the same context keeps), capped at 64 as in §6.3. Nothing is saved (§6.5).
   Tessera actions outside §12.14.2 keep their defaults; shared strokes then resolve by §7.

<a id="s12-14-2"></a>
#### 12.14.2 Commands

Command names are compared without regard to case. "Value" is the entry value of §5.3 for
axis actions.

| Original section | Original command(s) | Tessera context | Tessera action | Value |
|---|---|---|---|---|
| `Main` | `MovementForward`, `MovementForwardCameraMovement`, `MovementForwardCameraMovementToggleMouseSteer` | movement | `move_forward` | +1 |
| `Main` | `MovementBackup` | movement | `move_forward` | −1 |
| `Main` | `MovementStrafeRight` | movement | `strafe` | +1 |
| `Main` | `MovementStrafeLeft` | movement | `strafe` | −1 |
| `Main` | `MovementTurnLeft`, `MovementTurnOrStrafeLeft` | movement | `turn` | +1 |
| `Main` | `MovementTurnRight`, `MovementTurnOrStrafeRight` | movement | `turn` | −1 |
| `Main` | `MovementLongJump`, `MovementHighJump` | movement | `jump` | — |
| `Main` | `MovementRunLock` | movement | `autorun` | — |
| `Main` | `CameraInstantMouseSteer` | camera **and** movement | `mouselook` | — |
| `Main` | `PointerX` | camera | `look_x` | axis entry |
| `Main` | `PointerY` | camera | `look_y` | axis entry |
| `Main` | `CameraInstantMoveToward`, `CameraMoveToward` | camera | `zoom` | +1 (closer) |
| `Main` | `CameraInstantMoveAway`, `CameraMoveAway` | camera | `zoom` | −1 |
| `Main` | `CameraReset` | camera | `reset_camera` | — |
| `Main` | `CameraFlightMode` | global | `toggle_fly` | — |
| `Main` | `CaptureScreenshot` | global | `screenshot` | — |
| `Main` | `ToggleDebugHUD` | global | `toggle_debug` | — |
| `Main` | `EscapeKey` | global | `menu` | — |
| `Main` | `AcceptInput` | global | `open_chat` | — |
| `EditControls` | `AcceptInput` | text_entry | `text_submit` | — |
| `EditControls` | `EscapeKey` | text_entry | `text_cancel` | — |
| `EditControls` | `BackspaceKey` | text_entry | `text_backspace` | — |
| `EditControls` | `CursorPreviousLine` | text_entry | `text_history_prev` | — |
| `EditControls` | `CursorNextLine` | text_entry | `text_history_next` | — |

Not translated, with the reason:

- `MovementWalkMode`: its toggle type is Toggle (a latch), while Tessera's `walk` is held
  (§8). OPEN (§12.19).
- `CameraInstantMouseLook`: orbits the camera without turning the character; Tessera has no
  such action.
- `MovementTurnOrStrafeLeft`/`Right` turn the character, and strafe while the character is
  steered with the mouse; they are imported as plain turning.
- Camera rotation, roll, field-of-view and first-person commands, and every class of §12.11
  not named above: no Tessera action yet. Tessera's `fly_*` actions have no original
  counterpart and keep their defaults.

<a id="s12-14-3"></a>
#### 12.14.3 Controls

- Keyboard device: the code maps by §12.6.1; a code with no Tessera key drops the binding.
- Mouse device: buttons by §12.6.2. `DIMOFS_X` and `DIMOFS_Y` without a sub-control become
  the analog entries `{ axis = "MouseX" }` and `{ axis = "MouseY" }` (scale 1: DirectInput and
  §4.3 agree on +X right and +Y down), and only for `look_x`/`look_y`. The wheel: §12.14.5.
- XInput slots (`GUID_XInput1`–`4`): by §12.6.3. All four pads become Tessera's merged pad
  (§11), so the same binding on several slots collapses into one entry.
- `GUID_SysJoystick`, the virtual device, devices with other GUIDs, and device indices beyond
  the effective device list: the binding is dropped (their control layout is device-specific
  or has no Tessera input).

<a id="s12-14-4"></a>
#### 12.14.4 Meta keys

For each bit set in a binding's mask, take the controls that the **effective** meta-key table
(§12.8) defines for that bit:

1. Left and right keys of one modifier, both present (Shift 0x2A and 0x36, Ctrl 0x1D and
   0x9D, Alt 0x38 and 0xB8, Windows 0xDB and 0xDC): the Tessera modifier `Shift`, `Ctrl`,
   `Alt` or `Super`.
2. Exactly one control that §12.6 maps to a Tessera button (for example bit 8 = the right
   mouse button, or only the left Shift key): that button joins the stroke as a chord button
   (`MouseRight`, `LShift`).
3. Anything else, or no control for the bit: the binding is dropped.

The stroke (§5.1) is the modifiers in §4.4 order, then the buttons. More than four buttons
drops the binding. An analog entry (§12.14.3, §12.14.5) with any mask bit is dropped, because
analog entries take no modifiers (§5.3).

<a id="s12-14-5"></a>
#### 12.14.5 Activations and sub-controls

- A binding is translated only when its activation has the Down bit (0x01): `Click`,
  `Down` and combinations with them. Release-only, tap and double-click bindings have no
  Tessera form and are dropped.
- Sub-control 0 is the control itself.
- The mouse wheel (code 0x08): `AxisPositive` bound to a +1 zoom command, or `AxisNegative`
  bound to a −1 zoom command, becomes `{ axis = "WheelY", scale = 1.0 }`; the opposite pairing
  becomes scale −1.0. Both directions collapse into one entry. Any other wheel binding is
  dropped.
- Pad hat and trigger sub-controls by §12.6.3; any other sub-control drops the binding.

<a id="s12-14-6"></a>
#### 12.14.6 Known behavioural differences

Tessera keeps its own resolution rules (§7) and modifier policies (§2); the original's
best-match rule (§12.12) is not reproduced. Consequences:

- An *Exact* Tessera button action does not fire while an extra modifier is held, where the
  original would fall back to the binding with fewer meta keys (for example Escape with Ctrl
  held).
- Tessera chords complete in any press order (§5.1). The original needs a meta button held
  **before** the control is pressed (the left-plus-right mouse run).
- Imported pad bindings use the dead zone of §12.16 when the preferences file is imported,
  else Tessera's 0.15.

<a id="s12-15"></a>
### 12.15 Plugin API action IDs

All 270 values of the plugin API's `Action` enumeration (PLG) are IDs of the InputActions
chain (OBS, K29); 64 enumeration members have the mapper's name, 206 use names of their own.
The ID is therefore the stable link between plugins and key bindings.

**Decision:** each imported entry remembers the original command it came from. When Tessera
reports a key-binding action to plugins (`lua-plugin-api.md` §14.4), it reports the
InputActions ID of that command, looked up by name in the data at run time.

An entry that remembers no command (Tessera's built-in defaults, entries the user creates in
Tessera, and entries read back from a saved Tessera keymap, which does not store the command)
reports a command chosen from the rows of the §12.14.2 table for its **(context, action)**
pair, in table order:

1. **Button actions** (rows whose Value is "—"): the first row's command. So `jump` reports
   `MovementLongJump`, `mouselook` reports `CameraInstantMouseSteer` in both contexts,
   `text_submit` reports `AcceptInput`.
2. **Digital entries of axis actions** (`"text"` or `{ input, value }`, §5.3): the first row
   whose Value has the sign of the entry's value (a plain `"text"` entry has value +1,
   §5.3). A value of exactly 0 reports nothing. Rows whose Value is "axis entry" have no
   sign and are never chosen here, so a digital entry of `look_x` or `look_y`, whose only
   rows are "axis entry" rows, reports nothing.
3. **Analog entries** (`{ axis = … }`): the first row whose Value is "axis entry" when the
   pair has one (`look_x` → `PointerX`, `look_y` → `PointerY`), whatever the scale.
   Otherwise the first row whose Value has the sign of the stored scale (negative after
   `invert`); a scale of exactly 0 reports nothing. So `{ axis = "WheelY", scale = 1.0 }` on
   `zoom` reports `CameraInstantMoveToward`, and the default `PadLeftY` entry of
   `move_forward` reports `MovementForward`.
4. Pairs without rows (`fly_*`, and every other Tessera action not in the table) report
   nothing.

The same choice is made for every entry of one kind and sign, so a plugin cannot tell two
such entries apart by the ID; it does not need to, because the ID names a command, not a
key.

<a id="s12-16"></a>
### 12.16 Related input preferences

The `[Input]` section of the original's preferences file (`install-layout.md` §8.3) holds
these keys. The original's initial values (BIN) equal the values in the owner's file (OBS,
K30):

| Key | Initial value | Unit |
|---|---|---|
| `InvertMouseLookYAxis` | False | flag |
| `JoystickDeadZone` | 0.25 | fraction of stick travel (Reported from the name) |
| `MouseLookSensitivity` | 0.10 | OPEN |
| `MouseLookSmoothingAmount` | 0.00 | OPEN |
| `PitchKeySpeed` | 60 | degrees per second |
| `RollKeySpeed` | 60 | degrees per second |
| `TurnKeySpeed` | 150 | degrees per second |
| `XInputDetection` | False | flag |

**Decision** (optional companion import, read-only like the rest): `TurnKeySpeed` sets the
controllers' keyboard turn rate (§9.2, §9.3) as value × π / 180 rad/s (150 → 2.618; Tessera's
default is 2.5); `InvertMouseLookYAxis` sets the inverted-vertical-look flag of §8;
`JoystickDeadZone` becomes the dead zone of imported pad axis entries. The other keys have no
Tessera use yet.

<a id="s12-17"></a>
### 12.17 Invariants

- **I1** (Confirmed 2026-10-05, manual check on the owner's file): parsing the owner's file,
  building the effective key map from its `User` bindings and the install's four records
  (§12.8), and writing it by §12.13 reproduces the file byte for byte (16,613 bytes, 254
  bindings, 6 `User` bindings that displaced 6 defaults). The settings folder is outside
  `TESSERA_GAME_DIR`, so automated tests use I2–I6 instead.
- **I2** (real data, `TESSERA_GAME_DIR`): the effective key map built from the records alone
  has the sections, section order and counts listed at the end of §12.8, and exactly two
  record bindings are dropped as conflicts. The numbers are observations of this install:
  tests report them, and fail only on a structural difference (for example a record that no
  longer parses).
- **I3** (real data): every 0x1D record parses with no bytes left over; every device list is
  the eight standard devices in §12.5.2 order; every meta-key entry has exactly one bit set;
  every control value names a device in the list and a code with a name for that device's
  type; every command ID is in the InputActions chain and every input-map ID in the InputMap
  chain; record bindings use only sub-controls 0–2 and the activations 0x03, 0x04, 0x06 and
  0x60.
- **I4** (real data): both 0x51 records parse with no bytes left over; each table is in
  bucket order (`records.md` §1.3.1); tables A and B have the same command set; every toggle
  type is 1–5 and every class is 3, 4 or 5.
- **I5** (real data): the InputActions chain has 471 names with distinct IDs; the rules of
  §12.11 give each name exactly one class, with the counts in the table.
- **I6** (synthetic): writing a key map by §12.13 and reading it back by §12.3–§12.7 gives
  the same sections, bindings (order, origin, control value, mask, activation) and meta keys,
  including control names with brackets, which the writer quotes, and activation 0, written
  as `""`. This holds for key maps whose controls all have a name in the table of their
  device's type (no virtual-device control, no joystick-only name on a keyboard device) and
  whose activations have no bits outside 0x7F (§12.13, K37); for the excluded cases the
  read-back result is the one §12.13 states (binding dropped, or activation 0x03).
- **I7** (synthetic): the merge examples of §12.18 give exactly the orders shown; a file with
  five top-level nodes, a leading byte-order mark, a mismatched bracket, an unknown section,
  an unknown activation word, an unescaped 0xFF byte outside a comment, an empty-text
  device entry, section or meta key, an `!error` directive with or without a child (in any
  letter case, quoted or not, at any level), or four 0xAA bytes at the start is rejected; a
  file with an unknown command name, an unknown control name, an empty-text binding, a
  binding whose device index is not a number, or a binding named by an unknown directive
  (`!foo`) is accepted without that binding; a file whose `Devices`, `MetaKeys` and
  `Bindings` parts have no child list is accepted as an empty key map with the eight
  standard devices; a child list attached to a device index, control name, sub-control,
  mask or activation node changes nothing; the examples of Example H (§12.18) give exactly
  the trees and results shown.
- **I8**: the importer opens no file for writing, refuses a source larger than 1 MiB, and never
  touches the settings folder or the install.

<a id="s12-18"></a>
### 12.18 Worked examples (synthetic)

**Example A: tokens and tree.** The line `CameraReset [ Old [ 0 DIK_NUMPAD0 ] ] # note`
gives the tokens `CameraReset`, `[`, `Old`, `[`, `0`, `DIK_NUMPAD0`, `]`, `]`, and a comment.
The tree is: `CameraReset` with one child `Old`, whose children are `0` and `DIK_NUMPAD0`.

**Example B: quoting.**

| Input | Result |
|---|---|
| `"a \"b\" c"` | one token: `a "b" c` |
| `x\#y` | one bare token: `x#y` |
| `DIJOFS_BUTTON(5)` | token `DIJOFS_BUTTON` with a child list holding `5` (an invalid control name) |
| `"DIJOFS_BUTTON(5)"` | one token, the control name with code 0x35 |
| `a,b` | two tokens, `a` and `b` |

**Example C: one binding.** With the default meta keys (§12.5.3), the binding
`MovementForwardCameraMovement [ User [ 1 DIMOFS_BUTTON0 ] 0x00000080 ]` decodes to: device 1
(Mouse), code 0x0C, sub-control 0, control value 0x000C0001; file mask 0x80 = bit 8, reversed
mask 0x01000000; activation 0x03; origin User. Bit 8 is the right mouse button, so the import
(§12.14) gives `move_forward` the entry `"MouseLeft+MouseRight"` (value +1). The binding
`CameraInstantMoveToward [ Old [ 1 DIMOFS_Z AxisPositive ] ]` gives `zoom` the entry
`{ axis = "WheelY", scale = 1.0 }`.

**Example D: merging.** One default record has, in section `Main`, in order: D1
`MovementForward` on key W, D2 `MovementTurnOrStrafeLeft` on A, D3 `MovementStrafeLeft` on Q
(all mask 0, activation 0x03). The user file's `Main` holds U1 `MovementStrafeLeft [ User [ 0
DIK_A ] ]` and U2 `CameraReset [ Old [ 0 DIK_HOME ] ]`.

| Step | `Main` afterwards |
|---|---|
| file loaded | U1, U2 |
| non-user bindings removed | U1 |
| D1 merged | U1, D1 |
| D2 merged: conflicts with U1 (key A, mask 0, activations share bits), D2 discarded, U1 moved to the end | D1, U1 |
| D3 merged | D1, U1, D3 |

Import: `move_forward` gets `"W"` followed by Tessera's default pad entry for it
(`{ axis = "PadLeftY", deadzone = 0.15 }`, §8); `strafe` gets `{ input = "A", value = -1.0 }`,
`{ input = "Q", value = -1.0 }` and its default pad entry; `turn` and `reset_camera` get only
their default pad entries, which for both is none, so they become empty lists (unbound),
because none of their commands keeps a binding.

**Example E: picking a binding (§12.12).** `Main` holds `SELECTION_NEXT_FOE` on Tab (mask 0),
`SELECTION_PREVIOUS_FOE` on Tab with mask 0x1 (Shift) and `SELECTION_NEAREST_PC` on Tab with
mask 0x2 (Ctrl), all activation 0x03. Tab pressed with Shift and Ctrl held: all three are
candidates; the two masked ones tie on one bit; the reversed masks are 0x80000000 and
0x40000000, so `SELECTION_PREVIOUS_FOE` wins. Tab with Alt held: only the unmasked binding
qualifies.

**Example F: writer spellings.** A binding with mask 0 and activation 0x04 is written
`TapLeft [ Old [ 1 DIMOFS_BUTTON0 ] 0x00000000 Tap ]`; with file mask 0x6 (Ctrl + Alt) and
activation 0x03, `... ] 0x00000006 ]`; activation 0x07 is written `Click/Tap`; activation 0
is written `0x00000000 ""`. By the table of §12.6.3, joystick button 0 on the first XInput
slot (device 4) is written `[ 4 "DIJOFS_BUTTON(0)" ]` and button 1 `[ 4 DIJOFS_BUTTON1 ]`;
the virtual control `DIV_MOUSELOOK` on device 2 is written `[ 2 "" ]` (§12.13).

**Example H: nodes without tokens, empty tokens, raw bytes** (§12.3, §12.4, §12.7).

| Input | Result |
|---|---|
| `A [ x ] [ y ]` | two nodes: `A` with child `x`; an empty-text node with child `y` |
| `A [ [ x ] ]` | `A` with one empty-text child, whose child is `x` |
| `[ GUID_Virtual ] Devices [ ] MetaKeys [ ] Bindings [ ]` | four top-level nodes; the empty-text name node is accepted; devices 0–7 are the standard eight; a valid, empty key map |
| `Main [ [ User [ 0 DIK_A ] ] ]` inside `Bindings` | a binding with an empty command name: dropped, reported |
| `CameraReset [ [ 0 DIK_HOME ] ]` inside a section | origin token empty: kept while loading as a non-user binding, removed at start-up step 3 (§12.8) |
| `CameraReset [ User [ "" DIK_HOME ] 0x00000000 "" ]` | device index 0, activation 0: accepted, never fires |
| `MetaKeys [ [ 0 DIK_LSHIFT ] ]` | meta-key index empty, read as 0: file rejected |
| bytes `61 62 5C 0D 0A 63 64` (`ab\`, CR LF, `cd`) | with the text-mode translation: one token of 5 bytes `61 62 0A 63 64`; without it: tokens `61 62 0D` and `63 64` |
| bytes `22 61 62 0D 0A` (`"ab`, CR LF) | unterminated, reported; token `61 62` with the translation, `61 62 0D` without |
| bytes `41 20 5B 20 78 FF 20 5D` (`A [ x`, 0xFF, ` ]`) | the list of `A` ends at 0xFF, its closing bracket is missing there: mismatched brackets, file rejected |
| `"" [ GUID_Virtual ] Devices MetaKeys Bindings` | parts 2–4 without child lists: a valid, empty key map with the eight standard devices (§12.5) |
| `CameraReset [ User [ 0 DIK_HOME ( x ) ] 0x00000000 [ y ] Down [ z ] ]` inside a section | the children `x`, `y`, `z` are ignored: control DIK_HOME on device 0, mask 0, activation Down |
| `CameraReset [ User [ x DIK_HOME ] ]` inside a section | device index not a number: binding dropped, reported; the file is still accepted |
| `"!WARNING" [ note ]` as an extra top-level node | reported with the message `note` and removed; the remaining four nodes are the parts |
| `!error` as an extra top-level node | file rejected (empty message) |
| `!foo [ User [ 0 DIK_A ] ]` inside a section | not a directive name: kept as a binding with command `!foo`, unknown command, dropped, reported |
| bytes `AA AA AA AA` at the start of the file | rejected before parsing (§12.3 rule 8) |

**Example G: a minimal key-map record** (DID and command ID invented for the example):

| Offset | Bytes | Field |
|---|---|---|
| 0 | CD AB 00 1D | DID 0x1D00ABCD |
| 4 | 01 78 | name: length 1, `x` |
| 6 | 00 × 16 | key-map GUID (zero) |
| 22 | 01 00 00 00 | one device |
| 26 | 01 | type Keyboard |
| 27 | 61 2B 1D 6F A0 D5 CF 11 BF C7 44 45 53 54 00 00 | `GUID_SysKeyboard` |
| 43 | 0B 01 | meta-key table: 11 buckets, 1 entry |
| 45 | 00 00 2A 00 | control value: device 0, sub-control 0, code 0x2A |
| 49 | 00 00 00 80 | bit 1, reversed |
| 53 | 0B 01 | input-map table: 11 buckets, 1 entry |
| 55 | 0A 00 00 00 | input map 0xA (`Main`) |
| 59 | 0B 01 | mapping table: 11 buckets, 1 entry |
| 61 | 00 00 39 00 | control value: device 0, code 0x39 |
| 65 | 00 00 00 00 | mask 0 |
| 69 | 03 00 00 00 | activation 0x03 |
| 73 | 01 00 00 10 | command ID 0x10000001 (invented) |

The record is 77 bytes long. As a binding in a written file it reads
`<command name> [ Old [ 0 DIK_SPACE ] ]`.

<a id="s12-19"></a>
### 12.19 Open items of the importer

> **OPEN:** when each input map is active and the order in which the original consults the
> active maps (§12.5.4, §12.12).

> **OPEN:** the exact user actions after which the original writes the file (§12.2).

> **OPEN:** pad value ranges, the stick Y sign, the trigger threshold and the effect of
> `XInputDetection` (§12.6.3, §12.16).

> **OPEN:** the tap and double-click timings and the "close" distance of `NearbyDown`/
> `NearbyUp` (§12.5.7).

> **OPEN:** the role of action-map table C and of its tokens missing from the named string
> table (§12.10).

> **OPEN:** the contents of the `gmPartner` record (0x1D000004) in the German and French
> language archives; it is empty in `local_English` (§12.9).

> **OPEN:** the toggle-type behaviours other than the down-type rule are read from labels and
> membership, not from code (§12.10).

> **OPEN:** importing `MovementWalkMode` needs a Tessera walk action that latches (§12.14.2).

> **OPEN:** the unit of `MouseLookSensitivity` and `MouseLookSmoothingAmount` relative to §8
> (§12.16).

> **OPEN:** where `!include` searches for files (§12.4). The importer does not follow it.

> **OPEN:** activation tokens with an empty word between or after slashes (§12.5.7). The
> importer rejects such a file, as for an unknown word.

---

<a id="s13"></a>
## 13. Corrections to other specs

These specs are owned elsewhere; the corrections are recorded here, not made there.

1. **`install-layout.md` §10** (`--keymap` row), **P30**, and **§8.2** (`lotro.keymap` row):
   the `<name>c<ext>` and `<name>s<ext>` companion files that the help text describes are not
   read by build 4906.0071.1794.4035 (K31). With `--keymap`, the original loads the named file
   but still writes `lotro.keymap` in the settings folder (K1). The file format is now
   §12.3–§12.13 of this spec.
2. **Option clash.** `install-layout.md` §10 lists `--keymap` as an option of the original
   that Tessera's client uses ("use"), while `walker.md` §1 and the client's command line use
   `--keymap` for Tessera's own TOML keymap. One name now carries two file formats. Proposal
   for the owners of both specs: keep `--keymap` with the original's meaning (an
   original-format file, imported by §12.14 at start-up) and give Tessera's own keymap a
   `--tessera-keymap` option, as `install-layout.md` §10 reserves the `--tessera-` prefix for
   Tessera's additions.
3. **`records.md` §2.1**, rows 0x1D and 0x51 ("L, BIN", layout not specified): the layouts are
   §12.9 and §12.10 of this spec, Confirmed for every record (status C). Note that the 0x1D
   tables start with a VLE bucket count and keep list order; the TSize rule of `records.md`
   §1.3 does not apply to them, while the 0x51 tables are ordinary TSize collections.
4. **`lua-plugin-api.md`** P20, §14.4 and appendix A.3: every value of the `Action`
   enumeration is an InputActions ID (all 270; 64 names identical, 206 renamed), so plugin
   key events can carry the IDs of §12.15.
5. **`docs/ARCHITECTURE.md`** §16 (OPEN on the original keymap syntax) and §25 row 12: answered
   by §12 of this spec (a neutral document; the owner should update it).

---

<a id="s14"></a>
## 14. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First version (clean room): §0–§11. |
| 2026-10-05 | Spec writer (dirty room): new §12 (the original client's key-mapping file and key-map records: syntax, structure, control vocabulary, errors, effective key map, record layouts, command classes, writer format, import into Tessera, plugin action IDs, related preferences, invariants, examples), §13 and §14; provenance split into §1.1 and §1.2 (K1–K32, with the BIN record); §0 and the header updated; §11 revised (import, plugin IDs, height scale and yaw relation resolved; gamepad and auto-repeat notes extended). §2–§10 unchanged. |
| 2026-10-05 | Spec writer (dirty room), answering the review of the same day; section numbers unchanged. (1) §12.4: an opening bracket with no token before it makes an empty-text node (K33), with the consequences in §12.7 and Example H. (2) §12.6.3: per-code table of the joystick spellings the writer emits (K27, BIN; the same rule confirmed for the mouse wheel by OBS); §12.6 intro and Example F aligned. (3) §11: the compass heading is clockwise from north per `coordinates.md` §11.4 (row C11); stale OPEN remark removed, check values added. (4a) §12.3 rule 1: a raw-byte reader must apply the text-mode translation first; the three cases where tokens differ are listed; §12.14.1 step 2 applies it. (4b) §12.7: name node and device entries with extra children, children of GUID nodes, duplicate device GUIDs, empty texts in every position (K35, K36). (4c) §12.15: the reported command for entries without a remembered command is defined for button actions, digital and analog entries. (4d) §12.6.3: bits 0x0400/0x0800 are never set in the original (documented XInput call; K28, PUB); `PadMode` for button 10 marked as a Decision with its basis. Also found while answering: byte 0xFF stops tokens and lists and makes a file rejected (§12.3 rules 4, 5, 7; K34; corrects "0x80–0xFF are token bytes"); empty tokens read as number 0 and activation 0; numeric command and section names are converted without whole-token checks (§12.5.9); the writer emits an empty control name for unnamed codes and every virtual-device control and handles word-less activations as stated (§12.13, K37); I6 restricted accordingly, I7 extended; new OPEN on empty activation words. Provenance: BIN row extended with the revision session; new source PUB; rows K33–K37. Status stays Review; Reviewer reset to none yet. |
| 2026-10-06 | Spec writer (dirty room), answering the re-review of 2026-10-05; section numbers unchanged, status stays Review. (1) §12.5: a `Devices`, `MetaKeys` or `Bindings` part without a child list is an empty part, not an error (K38); §12.7 row added. (2) §12.7: children attached to any node read only for its text or number (device index, control name, sub-control, mask, activation, GUID) are ignored, as are children of a binding after the third; the nodes whose child lists are read are listed (K35 widened). (3) §12.4: directives are recognised by the first byte of the node text after the node is complete, quoted or escaped alike, in every list; names compared without regard to ASCII case; an unknown `!` name stays as an ordinary node; `!error` without a child still rejects the file, `!warning` without a child reports an empty message; a missing or unparsable `!include` file is only reported (K39); §12.7 rows and a paragraph on unknown directives by position; §12.14.1 step 2 states the importer's handling. (4) §12.7: a device index that is not a number makes the control specification invalid: binding dropped and reported, meta key rejects the file (K40). Nit: §12.15 rule 2 states that digital `look_x`/`look_y` entries report nothing. Also found: a file whose first four bytes are all 0xAA is rejected before parsing (§12.3 rule 8, K41; §12.14.1 step 2). I7 and Example H extended. Provenance: BIN row extended with the second revision session; rows K38–K41. |
