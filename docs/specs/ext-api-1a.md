<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Extended scripting, slice 1a: module reference (`Core`, `Store`, `Commands`, `UI`, `Input`, `World`, `Entity`)

| Field | Value |
|---|---|
| Status | **Accepted** |
| Last updated | 2026-10-07 |
| Author role | Design (Tessera's own API, written from the accepted decision 0017 and Tessera's own specs; no dirty-room input), AI-assisted |
| Reviewer | Independent clean-room review session |
| Review record | Re-review of revision 6, 2026-10-07: **accepted**, with minor follow-ups (G1–G6) for the next revision. Full record moved to the review log (decision 0018). |
| Implements into | `crates/ext` (`tessera-ext`: the binding descriptions of these modules, argument validation, the value model, scheduling), `crates/ext-host` (`tessera-ext-host`: the ports of these modules over `world`, `physics`, `entity`, `input`, `ui`, `settings`; `Store` file I/O; the headless harness), `apps/client` (feature `ext`: the frame steps, the heavy-frame gate and its floor, the reserved layer, ext UI routing, the ownership badge), `crates/ui` (the solid-press test, selection and copy, composition coverage of ext entries), `crates/input` (the ext context priority), `apps/scripttool` (`check`, `test`) |
| Related records and specs | [decision 0017](../decisions/0017-extended-scripting.md) (abbreviated **D17**; the architecture this spec details, including its "Open security and design items" and "Required follow-ups"); [`ext-threat-model.md`](ext-threat-model.md) (**TM**: **normative** for every limit, cited as TM L*n*, for every rule of its §7–§9, cited as TM §*n.m*, and for the escape tests, cited as TM X*n*; TM §0.1); [`ext-manifest.md`](ext-manifest.md) (**EM**: manifest grammar, folder rules, grant store, backend gate states, the script tool); [`ext-api-1b.md`](ext-api-1b.md) (**EA1b**: the slice-1b modules); [decision 0014](../decisions/0014-ui-input-bridge.md) (**IB**: frame steps D8, UI band D5, `opaque_keys` D6); [decision 0015](../decisions/0015-ui-rendering.md) (**UR**: surfaces, composition, U1); [decision 0004](../decisions/0004-server-strategy.md) (server scope); [decision 0016](../decisions/0016-plugin-class-helper.md) item 2; [`keymap.md`](keymap.md) (**KM**: names, actions, contexts, bindings, resolution, KM §9.1 angle conventions); [`plugin-runtime.md`](plugin-runtime.md) (**PR**: the shared runtime core only); [`coordinates.md`](coordinates.md) (frames); [`environment.md`](environment.md) §11 (game clock and sky, BIN); `docs/ARCHITECTURE.md` (**ARCH** §9.1 coordinates, §20.3 frame loop, §21 threading); `docs/LEGAL.md` §18 |

---

<a id="s0"></a>
## 0. Purpose and scope

This spec is the **reference of the seven modules that slice 1a (milestone M5.5) ships**:
`Core`, `Store`, `Commands`, `UI`, `Input`, `World` and `Entity`. For every function it
states the arguments and their types, the results, what fails and how, the limits and the
work-unit cost, the frame step in which its effect applies, and the capability it needs. An
implementer of `tessera-ext` and `tessera-ext-host` builds the slice-1a modules from it; a
script author reads it as the contract.

**This file and `ext-api.md`.** D17 names one follow-up spec `ext-api.md`. It is written as
two files: this spec (**EA1a**, which EA1b calls "EA") and `ext-api-1b.md` (**EA1b**).
Together they are D17's `ext-api.md` for slices 1a and 1b. The **general rules** every
module follows (§2: terms, value types, failures, costs, timing, handles, the handler
pattern, capability checks and suspension, angles and rotations, the ext environment; §3:
the `Ext` root) live here, and EA1b and the API specs of later slices follow them. A
companion spec that states one of these subjects differently has a defect; this section
is the place it cites (there is no other "heading" or angle section in this spec).

**The generated reference.** The module registry generates the reference
**`docs/specs/ext-api-reference.md`** (EA1b §10.4, D17 §10) from the signatures of this
spec and EA1b and from the calibrated costs (OPEN-1). It is part of what D17 calls
`ext-api.md`, is never edited by hand (a CI check compares it with the registry's output,
EA1b §10.6), and adds no rule: where it and these specs differ in anything but a
calibrated cost, the spec wins and the registry has a defect.

**Authority.** TM is normative for the subjects it delegates (TM §0 "Normative status",
§0.1):

- **Limits.** Every **resource limit** is TM's, cited as "TM L*n*" (TM §0.1 rule 6: a
  count, size, rate, time or per-call cap that bounds work, memory, disk, GPU time, queue
  length or the user's control). This spec's own per-call and per-object caps are TM L91.
  A number repeated here next to its citation is for the reader only; if it differs from
  TM §10, TM wins and this spec has a defect. Two kinds of number stay here and need no
  L-number (TM §0.1 rule 6): **argument-validity domains** (the range of one scalar
  argument or field that bounds no cost by itself: position magnitudes, §2.3; a timer's
  longest delay, §4.2; node positions and sizes, §7.4; a world label's width, `region`,
  offsets and `max_distance`, §7.9) and **grammar lengths** (a node's `name`, §7.4). Each
  is marked "domain" or "grammar" where it is stated. A new resource limit is requested of
  TM as an OPEN first (TM §0.1 rule 5).
- **Rules.** For D17's open security and design items and its "other improvements", the
  rules of TM §7–§9 apply. This spec states their script-visible form (names, arguments,
  results, notifications) in its own words and cites the rule; it may make a rule
  stricter, never weaker.
- **Escape tests.** This spec keeps no escape catalogue. Escape cases are TM's X-numbers;
  §14 lists this spec's invariants and names the TM tests that cover the same ground.

What it does **not** define, because D17 or a companion spec does:

| Subject | Where |
|---|---|
| The architecture: apartments, the patched Lua build, the frame steps 6a–6c, the backend gate, grants and prompts, leases, the panic chord | D17 §1–§8 |
| The threat model, the limits register, the escape-test catalogue, the rules for D17's open items and "other improvements" | TM |
| The manifest `script.toml`, folder rules, content hash, grant store, backend gate states, the script tool (`check`, `test`, `escape`, the walk file) | EM, from D17 §2 and §5.2 |
| Modules of slice 1b (`Camera`, `Overlay`, `Sandbox`), hot reload, the developer console, the generators | EA1b |
| Modules of later slices (`Files`, `Data`, `Audio`, `Input.Drive`, `Entity.Local`, `UI.Native`, `Plugins`, `Server`) | their slices' specs |

Where this spec restates a rule of D17 or TM, that document is the source; where it adds a
rule, the rule is marked **(1a rule)** and the provenance table says why.

**No facts from disassembly.** The ext contract is Tessera's own design (D17 §12, LEGAL
§18.3 rule 1). Nothing here adopts behaviour that the plugin specs or the UI specs record
from the original client as the ext contract: the ext widgets, their input behaviour, their
notifications and their drawing are defined below in their own terms. Four `World`
results pass through text or numbers that Tessera's own client computes with rules some of
which are recorded from the original (BIN): the map-coordinate value, the `/loc` text, the
game clock (`environment.md` §11.1, "BIN unless stated", and the night rule of `sound.md`
§10.3.5) and the sky state (the day-description choice of `environment.md` §11.2 and the
blending of §11.3). For each, the ext contract promises only units, ranges and directions
(§9.2), never a formula, a rounding, a threshold or a format (LEGAL §18.3 rule 1; OPEN-3,
closed by the reviewer's confirmation).

**Names.** `Ext`, the module names and every function, property, notification and element
kind name in this spec are Tessera's own choices. Element kinds deliberately avoid every
class name of the plugin API's interoperability record (LEGAL §17, D17 §12: "Ext UI classes
never reuse the interop names"). A word that also appears in another API (for example
`Exists`) implies no shared behaviour.

---

<a id="s1"></a>
## 1. Provenance

This is a design spec. It records decisions, not observations, so its facts carry the mark
**Decision** with their source. Facts it cites from other specs keep the marks they have
there.

| Facts | Source | Mark |
|---|---|---|
| Module set of slice 1a, capabilities, groups, backends, apply steps (§3–§10 headers); the API rules: chunky calls, region frame in f64, stale handles raise, every argument validated, per-frame caps answer `nil, "busy"` (§2) | D17 §3 table, "API rules" | Decision (D17, accepted 2026-10-07) |
| Frame steps 6a–6c, turns, notification order, self-caused notifications in the next turn, conversion inside metered calls, intents and their apply steps (§2.6, §2.8) | D17 §4 | Decision (D17) |
| Every resource limit (§11 and wherever a number is cited as TM L*n*), this spec's per-call caps included (TM L91) | TM §10, from D17 §8 and TM's changes | Decision (TM, normative) |
| Argument-validity domains and grammar lengths marked "domain" or "grammar" (§2.3, §4.2, §7.4, §7.9) | This spec, under TM §0.1 rule 6 | Decision (this spec) |
| The rules for D17's open items 1 (composition part), 2, 4, 5, 6 and 8 and the "other improvements" this slice ships: composition coverage (§7.10), solid presses and gesture-window focus (§7.6–§7.7), the liveness floor and the gate's clock origin (§2.6), entity filtering (§10.3), `Store` read-your-writes, reservations, disk rates and the no-follow store directory (§5), world labels (§7.6), log format (§4.3), the ownership badge (§7.7), selection and copy (§7.8), image decodes (§7.10), the sticky abort (§2.4), split turns, their place in the round robin, gesture windows, what opens them and their bound (§2.6), the ext profile's library tables (§2.11), revocation, the effective set, handles checked against their minting module, suspension of input and the grant-store re-read (§2.7, §2.9, §8.1), results of outstanding requests never dropped (§2.8) | TM §7.1, §7.2, §7.4, §7.5, §7.6, §7.8, §8.1, §8.3–§8.5, §8.7, §8.8, §8.10–§8.15, §8.23, §8.24; L30, L91, L93 | Decision (TM, normative); the script-visible form is this spec's |
| The ext environment: the globals and libraries of an apartment, `print` to the script log, `tostring` without addresses, per-apartment random numbers (§2.11); the process locale pinned to "C" and the suspension when it is not (§2.9) | D17 §1.2, §1.5 | Decision (D17) |
| Activation and focus cases 1 and 2, the visible-and-uncovered focus rule, the ext z-range, pointer notifications only over the script's own catching nodes (§7.6–§7.7) | D17 §4, §5.6, as TM §7.2 tightens them | Decision (D17, TM) |
| Ext keymap names `ext.<id>.<name>`, contexts in the ext partition, resolution after every non-ext context (§8) | D17 §2, §5.5; KM §2, §3, §7 | Decision (D17); KM rules as KM marks them |
| Region frame: f64 (160 · X + x, 160 · Y + y, z), +X east, +Y north, +Z up, unit = 1 m by convention; yaw counter-clockwise from +X (§2.10) | ARCH §9.1; `coordinates.md` §2–§3; KM §9.1 | as marked there (Confirmed; KM Decision) |
| Function names, argument shapes, result tables, notification names, element kinds and their properties, reason strings, provisional unit costs, the handler pattern, `"not declared"` and the ext context priority (closing EM OPEN O-2 and O-3), and every rule marked **(1a rule)** | This spec, 2026-10-07 (revisions 2–6 the same day): designed to satisfy D17 and to state TM's rules in script-visible form | Decision (this spec; subject to review) |

---

<a id="s2"></a>
## 2. Conventions

<a id="s2-1"></a>
### 2.1 Terms

| Term | Meaning |
|---|---|
| **Script** | One loaded ext script: one apartment (D17 §1.2) for one (folder kind, id). |
| **Call** | One metered entry into the script's Lua code: the top-level run, the filter call (§10.3), one handler call, one timer call, `Tick`, the `Unload` handler, and (slice 1b, developer mode) one developer REPL run (EA1b §9.4, TM L85), as TM §2 lists them. Each call has 800,000 work units and a 4 ms wall-clock deadline (TM L3, L4). |
| **Turn** | The script's share of 6b for one round-robin visit (TM §2), in this order: in the first turn after a resume only, its `Resumed` call (§4.1); its filter call (§10.3), when it has a subscription; its queued notifications in order; its due timers; then `Tick`. Every item of a turn is its own call. A turn can be **split** between frames (§2.6, TM §8.13). |
| **Notification** | A queued event for the script, delivered as one call in a turn (D17 §4, §2.8). |
| **Intent** | A queued effect that a later frame step applies (D17 §4). In slice 1a the only intents are `Input` context changes (§8). |
| **Gesture** | One of the user's own inputs that D17 roots trust in (TM §2): a **solid press** (§7.6) on one of the script's frames that delivers a `Pressed` notification, the start of one of its own actions (§8.2), or an `/ext:` line for it (§6). |
| **Gesture window** | Opened **only** by the delivery of a gesture's own notification: the `Pressed` of a solid press, an action's `"start"`, or an `/ext:` line's call (TM §8.13, L93). The `gesture` flag of the matching `Released` and `Clicked` is information only; their delivery opens nothing, so a press held for 10 s opens one window, at the press. A press on an `enabled = false` node delivers no notification and so opens no window (§7.6). The window runs from the start of the delivering call and closes at the **first** of: the end of that turn's `Tick`, whatever frames a split turn spans; the user's first key press, pointer press or wheel step after the gesture that resolves outside this script's own frames and actions; **1 s** of wall time after the delivering call started (**60 frames** in deterministic mode) (TM §8.13, L93). Every API spec uses exactly this window. |
| **Solid press** | A user press whose catching node draws its own background at the press point with alpha × effective opacity ≥ 0.5 and is the **topmost painted node** there, so that no other node of any ext frame paints over it at that point (TM rule 7.2.1, L57; §7.6 says what "own background" covers for each kind). |
| **Node** | One element of the script's UI: a frame, a control inside a frame, or a world label (§7). |
| **Frame** | A top-level ext window (element kind `Frame`). The word "window" in D17 means a frame. |
| **Catching node** | A node that catches the pointer at a point (§7.6). D17 and UB call this "mouse-visible". |
| **UI unit** | One logical pixel at UI scale 1.0. The host multiplies by the user's UI scale. Origin at the top-left of the game window's client area, +x right, +y down. |
| **Region frame** | Positions as f64 (x, y, z) in metres: x = 160 · X + local x, y = 160 · Y + local y, z up, in **the player's current region** (ARCH §9.1). |
| **Resident** | Present in memory now: a landblock streamed and integrated, a collision set installed, an entity in the store (D17 §8, "Natives never wait"). |
| **Screen**, **coverage** | As TM §2 defines them: one full viewport in physical pixels (in deterministic mode the harness's virtual surface, 1920 × 1080 unless the walk file sets another, EM §11.5), and a conservative pixel count divided by one screen. |
| **Steps** | The frame steps of ARCH §20.3 as IB D8 and D17 §4 refine them: step 1 input; steps 2–6 session, simulation, streaming, completions, animation; step 7 items 1–6 (UI tick and plugins), then **6a** (ext queue), **6b** (ext tick), **6c** (ext collector), then item 7 (the outbox drain, "**item-7 drain**"); step 8 paint (for ext: painting and world-label placement only; ext layout runs inside the call that changes it, §7.1); step 10 render. |
| **Effective set** | The capabilities an apartment runs with: those granted when it was loaded, shrunk by every revocation since, **never grown** during its life (TM §8.23). Every grant check of this spec reads it, never the live grant table. |

<a id="s2-2"></a>
### 2.2 How the tables read

- **Signature.** `Module.Name(a, b [, c])`: brackets mark optional arguments. A method on a
  handle is written `h:Name(...)`. A table argument is written `{field, ...}` and its fields
  are listed below the table.
- **Returns.** "`true` \| `nil, reason`" means the function returns `true` on success, and
  `nil` plus a reason string (§2.4) on a soft failure.
- **Cost.** Work units charged before the work starts (§2.5).
- **Applies.** When the effect takes place (§2.6). "Read" means the function only reads
  state as it stood after steps 1–6 of this frame (D17 §4, "Reads"), or the script's own
  state at the moment of the call.
- **Capability.** The grant the module needs (D17 §3). A function of a module whose table the
  script holds always re-checks it against the apartment's effective set (§2.1, §2.9).

<a id="s2-3"></a>
### 2.3 Value types

| Type | Accepted | Refused with a Lua error |
|---|---|---|
| `number` | a finite Lua number | NaN or ±∞, unless the row says otherwise; a string is **not** converted to a number |
| `integer` | a finite number with no fractional part in the stated range | a fraction; out of range |
| `DID` | an integer in 0 … 4,294,967,295 | anything else ("out-of-range DID", D17 §3) |
| `string` | a Lua string. Text that the host interprets (labels, names, command text) is UTF-8; an invalid sequence is replaced by U+FFFD **when the host reads it**, never an error (D17 §3) | a non-string, unless the row allows a number |
| `boolean` | `true`, `false` | anything else, including `nil` where a boolean is required |
| `colour` | a string `#RRGGBB` or `#RRGGBBAA` (hexadecimal digits, either case); RGB is sRGB, AA is straight alpha, omitted = FF | any other form |
| `position` | three numbers `x, y, z` passed as separate arguments (region frame, §2.1), each with absolute value at most 10^7 (domain, TM §0.1 rule 6) | non-finite or larger values |
| `handle` | a handle of the stated kind (§2.7) | another kind; a non-handle |
| `function` | a Lua function | a callable table (no metamethod is called by the host) |
| `table` | a Lua table walked with raw access only (D17 §8, "The bounded value model") | a non-table; a table over the native's declared caps is refused with `nil, "too large"` where the row says so, otherwise a Lua error |

- Strings returned by the host are valid UTF-8, except `Store` values, which come back
  byte-for-byte as stored.
- Every number the host returns is finite unless the row says otherwise.
- Tables the host returns are new tables that the script owns; changing them changes nothing
  in the host.

<a id="s2-4"></a>
### 2.4 Three ways to fail

1. **Lua error** (catchable with `pcall`): a programming error. Wrong type, a missing or
   extra argument, a value out of
   range, an unknown name (property, context, action, setting), a call on a stale handle
   (§2.7), a call on a module whose grant was revoked or with a handle whose minting
   module's grant was revoked (§2.7, §2.9), a ray longer than 500 m, a
   native whose cost exceeds what is left of the call (§2.5), and a refused allocation or
   host-object creation over the memory cap ("not enough memory", TM L22, L23). A result
   **reservation** the memory cap refuses is not an error but a soft failure (below). The
   message names
   the script's file and line (relative paths only, D17 §1.2); its exact text is not part of
   the contract.
2. **Soft failure** `nil, reason` (or `false, reason` where the table says so): an expected
   condition the script should handle. Reasons are fixed strings:

   | Reason | Meaning |
   |---|---|
   | `"busy"` | A per-frame cap, a rate limit or an outstanding-request cap is reached; try again in a later frame (D17 §3, "API rules") |
   | `"limit"` | A count cap is reached (nodes, timers, subscriptions, commands): creation refused until something is removed **(1a rule: the name)** |
   | `"not loaded"` | The data needed is not resident (D17 §8, "Natives never wait") |
   | `"too large"` | A value over the bounded value model's caps (D17 §8) |
   | `"cyclic"` | A value with a cycle (D17 §8) |
   | `"invalid"` | A stored file that does not parse, or holds a nil or NaN key or a duplicate key (D17 §8); a NaN in a value to store **(1a rule)** |
   | `"full"` | `Store` space: the script's 1,024 keys or 16 MiB, or the 256 MiB or 16,384 keys of all scripts (TM L40, L41) |
   | `"reserved key"` | A `Store` key that is a reserved device name (D17 §2) |
   | `"io"` | A file-system operation failed on the worker **(1a rule)** |
   | `"out of memory"` | A result reservation (`Store.Get`) would pass the script's memory cap; nothing is queued (TM §8.14, L28). **Returned, never raised** |
   | `"not granted"`, `"unknown module"`, `"not declared"`, `"major not supported"`, `"unavailable on this backend"` | `Ext.Use` only (§3) |

3. **Abort**: the call passed its 4 ms deadline or its 800,000 units (TM L3, L4). The call
   ends at the next instruction; `pcall` and `xpcall` cannot catch it, in the main thread or
   in any coroutine. **The abort is sticky** (TM §8.11): every later hook check in that
   apartment, in any of its Lua threads, aborts again until the call has returned to the
   host, so no handler, `pcall` chain or coroutine can run on past it. The host writes one
   log record naming the call (source `error`, §4.3). Three aborts in 10 s suspend the
   script (TM L18).

**Order of checks (general rule, 1a rule; every API spec follows it, EA1b §10.3 included).**
Every native, of every slice, checks in this order, and the first check that fails
decides:

1. **Grant and backend.** The module's capability is in the apartment's effective set and
   the backend class admits the module (§2.9). Otherwise a Lua error `"not granted"` or
   "unavailable on this backend".
2. **Arity and types.** No required argument is missing and **no extra argument** is
   passed: a call with more arguments than the signature lists raises, an explicit
   trailing `nil` included (the native sees how many arguments were passed), so a script
   written for a later minor fails loudly on an older host. Every argument has its type
   (§2.3). A handle, whether an argument or `self`, is checked in this order **(general
   rule; the order of EA1b §10.2's `handle(K)` row)**: (a) the module that minted it
   still has its capability in the effective set (§2.7), else a Lua error
   `"not granted"`, whatever the handle's kind and whether or not it is stale; (b) it is
   of the stated kind; (c) it is not stale (§2.7), else a Lua error, except for
   `Exists()` and `Kind()`, which answer. So the minting rule wins over the stale-handle
   rule: `Exists()` and `Kind()` raise `"not granted"` on a stale handle whose minting
   capability is gone, and no answer tells whether the object is gone. In slice 1a every
   handle method belongs to the module that minted its handle, so step 1 decides first
   for `self`, and no slice-1a native takes a handle minted by another module; the order
   decides only which error a value with several faults raises, and later slices'
   natives (EA1b's `lease:Follow` first) follow it.
3. **Constraints** of what is known **without reading a table argument's contents**: the
   domains, ranges, finiteness and lengths of scalar arguments, enumerations, an unknown
   name passed as a scalar argument (a property, context, action, setting, notification
   or field name), and a table argument's type. A failure is a Lua error. Then the soft
   constraint a row lists before any cap (`"reserved key"`).
4. **Caps.** Count caps, per-frame and per-second caps and the outstanding-request cap:
   `nil, "limit"` or `nil, "busy"` (or the row's form, for example `false, "busy"`).
5. **Charge.** The row's cost or reservation (§2.5). A cost over what is left of the call
   raises before any work; a memory reservation the cap refuses answers
   `nil, "out of memory"` (TM §8.14).
6. **The operation.** It starts with the **walk of the call's table arguments** (property
   tables, option tables, filters, arrays of names or handles), under the step-5 charge:
   each element is charged before it is read (§2.5), so no element of a table argument is
   read before it is paid for. The walk applies the checks that need the table's
   contents (unknown property and option fields, field types, domains and ranges of
   fields, a table's element cap, and for a handle inside a table the checks of step 2 in
   step 2's order: minting capability, kind, then staleness where the row raises for a
   stale handle, a batched read answering `false` for a stale entry instead, §2.7), each
   a **Lua error** as at steps 2–3, raised before any effect: the
   call then changes nothing and the walk's units stay charged. Then come the soft
   failures only the operation can find (`"not loaded"`, a `false` result, a refused
   coverage check, the value walk's `"too large"`, `"cyclic"` and `"invalid"`).

Steps 1–4 are paid by `B`. A refusal at steps 1–4 charges `B` only and changes nothing. A
cap's count or token (step 4) is taken only once the call is past step 6's walk of its
table arguments: a call refused at step 4, or one that raises in that walk, takes none; a
call past it takes its counts and tokens whatever its operation then answers, unless its
row says otherwise (`Store.Set`'s refusals after its value walk take nothing, below)
**(1a rule: field checks in the walk, answering EA1b's revision-4 review, so that EA1b
§10.3's generated validation and this order are one rule)**.

**Exceptions, where a cap needs what only the walk finds (1a rule).**

- `Store.Set(key, value [, done])`: steps 1–3 (the key grammar raises; then
  `"reserved key"`); step 4, only when `done` is passed, the outstanding-request cap
  (`nil, "busy"`, TM L27: a pending `done` is an outstanding request, §5.3); step 5
  reserves `V`; then the walk serialises the value and answers the **first** of
  `"too large"`, `"cyclic"` or `"invalid"` it meets, or raises `"not enough memory"` when
  the serialised bytes would pass the script's memory cap (§5.2 "Accounting"; a refused
  allocation, item 1 above) (the walk's visiting order is not part of the contract: a
  value with several faults may report any of them); then `"full"` (TM L40, L41, with the
  new counted size); then `"busy"` (the key bucket, then the script's byte bucket, then
  the all-scripts byte bucket, TM L43, L44); then the value enters the pending map, the
  buckets are debited, a `done` starts to count toward L27 and the reservation is refunded
  down to the real walk. A refusal after the walk, and the memory error, change nothing in
  the store, debit no bucket and count nothing toward L27; the units of the walk stay
  charged. `value = nil` follows `Store.Delete`'s order.
- `Store.Delete(key [, done])`: `"reserved key"`, then `"busy"` (only when `done` is
  passed, the outstanding-request cap, TM L27; then the buckets, a deletion counting
  4 KiB).
- `Store.Get(key, fn)`: `"reserved key"`, then `"busy"` (the read rate, TM L42, then the
  outstanding-request cap, TM L27), then `"out of memory"` (the reservation, TM L28).
- `World.Ray`: a segment over 500 m raises at step 3; `"busy"` at step 4; the step-5
  reservation covers 25 landblocks (TM L91) and is refunded to the landblocks overlapped;
  `"not loaded"`, `false` or the hit at step 6.

<a id="s2-5"></a>
### 2.5 Costs

Every native charges its cost in work units **before** it does the work; one unit is about
5 ns on the reference machine (D17 §8, "Work units"). A native whose cost exceeds the call's
remaining units, or whose cost × 5 ns exceeds the call's remaining wall time, raises a Lua
error before it starts. A native that walks a Lua value reserves the cost of its caps and
refunds what it did not use.

The figures are **provisional** (OPEN-1): slice 1a measures every native (E7) and writes the
calibrated costs into the registry, which then generates the reference
`docs/specs/ext-api-reference.md` (§0); the shape of each formula stays.

| Symbol | Units | What it covers |
|---|---|---|
| `B` | 200 | Every native call: argument checks, grant check, scalar results (E2's 1 µs) |
| `b(n)` | ⌈n / 8⌉ | `n` bytes of string crossing the boundary in either direction, including the UTF-8 check |
| `e(n)` | 20 · n | `n` elements of a table built or walked, counted by the rule below |
| `h(n)` | 50 · n | `n` handles returned (the interned handle lookup of §2.7); the array holding them is counted by `e` as well |
| `N` | 2,000 | Creating one node (element-tree insert, accounting) |
| `P(n)` | 100 · n | Setting or reading `n` node properties, walking each property's key and scalar value in the argument table included (plus `b` for strings; a property whose value is a table adds `e` for that table) |
| `Q` | 500 | One point query of `World` |
| `V` | 295,112 | The reservation of one maximal value: `e(8,192)` + `b(1 MiB)` + `B` = 163,840 + 131,072 + 200 (TM L34, L35). Refunded down to the real size after the walk |

**Counting elements (general rule, 1a rule; every row of every API spec follows it).** `n`
in `e(n)` counts **every key and every value** of every table built or walked, nested tables
included, exactly as the bounded value model counts elements (D17 §8, §5.1): a record of *k*
fields is *2k*, an array of *m* entries is *2m*, and a nested table is one value of its
parent whose own keys and values count again. So `{x, y, z}` is `e(6)`, an array of 4 such
records is `e(8 + 4 · 6)` = `e(32)`, and an entity entry with `rotation` adds `e(2 + 8)` for
that field. A result of **fixed shape** is charged for every field of the shape, including a
field that is `nil` in this answer, and its row states the figure. A table whose size varies
is charged its real count, reserved at the cap where the size is not known before the walk
(§2.5, first paragraph). Argument tables are counted the same way when they are walked (a
filter, an option table), unless the row charges them through `P(n)`.

<a id="s2-6"></a>
### 2.6 Timing

| "Applies" value | Meaning |
|---|---|
| **at once** | Inside the call. A getter called right after returns the new state (D17 §4, "Notifications a script causes itself"). |
| **next turn** | A notification the change causes is appended to the script's queue and delivered in its **next** turn, normally the next frame's 6b, never re-entrantly in the same call or turn. |
| **same-frame 6b** | A notification produced by native work before 6b of this frame (step 1 routing of the user's input; the hit test and hover refresh of step 7 item 1) is delivered in this frame's turn (D17 §4). |
| **item-7 drain** | Queued as an intent and applied at IB D8's item 7 of this frame, with the band requests (D17 §4, intents table). |
| **step 8** | Painted in step 8 of this frame (world labels are also placed there). Layout is not deferred to step 8: geometry changes apply **at once** (§7.1). |
| **worker** | Queued as a job on an asset worker; its result is a notification delivered in the first turn after the job completes. |

**Split turns** (TM §8.13). D17 §4 checks the 2 ms per-frame cap (TM L5) before each call.
When the cap or the gate stops a script's turn partway, the rest of that turn resumes in the
next frame that reaches that script, at the first call not yet made (undelivered
notifications of the turn's delivery set in order, §2.8, then timers not yet run, then
`Tick`), **before** any notification that arose later.

**Where a split turn resumes in the round robin** (TM §8.13). The round robin visits the
groups in order (lease holders first, then the others; D17 §4) and rotates within each
group. A split turn may resume as the **first script of its group at most once**: in the
next frame that runs 6b, it goes before the group's ordinary rotation. If that resumed part
is split again, the rest of the turn waits until the ordinary rotation next reaches the
script, and never takes the head again. So a script whose own calls fill the 2 ms cap every
frame (for example a 1,024-entry delivery set) runs first in at most one frame of that turn,
and every other script of its group gets its turn in rotation (TM X17). A script resumed at
the head is not visited again at its rotation place in that frame, and the rotation itself
(D17 §4: it starts one script later each frame) is not changed by the resumption.

`Tick`'s `dt` is the time since the last `Tick` the script received, so a split or skipped
turn gives the summed time. A gesture's **gesture window** (§2.1) lasts to the end of the
delivering turn, across the frames a split turn spans, so a split turn never loses a
gesture's effect (TM X89), but never past the user's next input elsewhere or 1 s (60
frames in deterministic mode) after the delivering call started (TM §8.13, L93). The 1 s
covers a turn split by the 2 ms cap and resumed at the next floor frame (below), so a
note-taker's split gesture turn still focuses its field under gating, while a call late
in a slow turn cannot focus one more than 1 s after the gesture, or after the user has
pressed `W` and is walking. **Known limit, failing safe (TM §8.13, L93).** Under permanent
gating, a gesture turn split **twice** waits for its rest at the script's ordinary
rotation place, which advances only in floor frames (about every 250 ms, every 16th frame
in deterministic mode); behind several scripts that each fill the 2 ms cap, that rest can
start more than 1 s (60 frames) after the gesture. The window has then closed: a `Focus`
there returns `false` and focuses nothing, and the user presses the key again. TM X89
covers both the turn split over two frames, which must focus, and this twice-split case,
which must fail safe.

**The heavy-frame gate** (TM L8, §8.12). The gate skips 6a–6c in a frame whose step 7 is
reached more than 22 ms after the frame's **work start**: the start of the frame's redraw
handling minus the main-thread time spent in this frame's step-1 callbacks since the previous
present. Idle time between frames is never counted. In a skipped frame no call runs;
notifications, timers and intents wait within their caps.

**The liveness floor** (TM rule 7.4.1, L9–L11). A frame the gate would skip still runs ext
as a **floor frame** when the previous 15 frames were all gated or at least 250 ms of wall
time have passed since the **start** of the last frame in which ext ran (in deterministic
mode only the 15-frame condition applies). A floor frame runs 6a–6c with their ordinary
caps and slices (the 2 ms cap, TM L5; 6c's 1 ms slice, TM L7), except that 6a starts
**exactly one** item if one is queued. A floor frame is a frame in which ext ran, so it
**resets both conditions**: the count of consecutive gated frames starts again at 0 after
it, and the 250 ms are counted from its start. Under permanent gating a floor frame
therefore comes at most once per 250 ms of wall time, and in deterministic mode every 16th
frame. So a queued notification or `/ext:` line therefore reaches its script
within the bound of TM L11 (250 ms plus one frame, or 16 frames in deterministic mode, plus
the script's place in the round robin). A backend switch or session end that has been
queued for 2 s (120 frames in deterministic mode, TM L10) closes the apartments whose
`Unload` handler has not started **without** it (§4.1). The gate and the floor never stop
the panic chord, the native routing of ext UI in step 1, the dropping of intents at
suspension or the expiry of leases (TM rule 7.4.3).

<a id="s2-7"></a>
### 2.7 Handles

- **What they are.** Nodes, timers, subscriptions and entities are referred to by handles:
  host userdata with sealed metatables (D17 §1.2: `getmetatable(h)` returns a constant
  string, `setmetatable` is refused).
- **Generational.** A handle names one object for its life. After the object is gone (a
  removed node, a cancelled timer, a despawned entity), the handle is **stale**: it never
  refers to a later object that reuses the slot (D17 §3, "API rules"; `tessera-entity`
  generational IDs).
- **Stale handles raise.** Every method on a stale handle raises a Lua error naming the
  script's file and line, except `h:Exists()`, which returns `false`, and `h:Kind()`, which
  still returns the kind (D17 §3), both while the module that minted the handle keeps its
  capability (below). Batched reads (§10) answer `false` for a stale entry instead of
  raising.
- **Identity (1a rule).** Within one apartment the host returns **the same userdata** for the
  same object for as long as the script holds a reference to it, so handles compare with
  `==` and serve as table keys. The host keeps the mapping in a weak table charged to the
  script's memory (`h(n)` covers the lookup). A handle the script has dropped may come back
  as a new userdata later; it still compares equal only to handles held at the same time.
- **Owned by the apartment.** A handle passed to another apartment is impossible (there is no
  route), and a handle kept across an unload is gone with its apartment.
- **Handles carry their module's capability (general rule, TM §8.23; every API spec follows
  it).** Every handle records the module that **minted** it (`UI` for nodes and labels,
  `Core` for timers, `Entity` for entities and subscriptions, and so on for later slices).
  After that module's capability leaves the apartment's effective set (an optional
  revocation followed by a resume, §2.9), the handle stays a valid Lua object (it still
  compares equal and serves as a table key, which needs no native), but **every native of
  every module that takes it as an argument or as `self`**, `Exists()` and `Kind()`
  included (an `Exists()` answer would still tell the script whether an entity
  despawned), raises a Lua error `"not granted"` (§2.4, step 2), whatever the module of that native
  and whatever capabilities that module needs itself. **This rule wins over the
  stale-handle rule above:** §2.4 step 2 checks the minting capability before the kind
  and before staleness, so a stale handle whose minting capability is gone raises
  `"not granted"` from `Exists()` and `Kind()` too. An intent that holds a handle is
  re-checked the same way at its apply step and discarded if the minting capability is
  gone. So a kept entity handle can never reach EA1b's `lease:Follow`, or any later
  native, after `entity.read` is revoked. Modules without a capability (`Core`, `Store`,
  `Commands`) mint handles that this rule never stops. Escape test: TM X91 (its sub-case
  of a kept entity handle passed to a native after an `entity.read` revocation and a
  resume: in slice 1a `Entity.Read` and `e:Get`, from slice 1b `lease:Follow`).

<a id="s2-8"></a>
### 2.8 Handlers and notifications

- **The handler pattern (general rule, 1a rule; every API spec follows it).** There are
  exactly three forms:
  1. **Module notifications:** `Module.On(name, fn)` sets the handler of one of the
     module's notification names (`Core.On("Tick", fn)`, `UI.On("ScreenChanged", fn)`,
     `Input.On("toggle", fn)` where `Input`'s notification names are the script's declared
     actions). `fn` receives **one** table of the notification's fields.
  2. **Object notifications:** `obj:On(name, fn)` on a handle (`node:On("Clicked", fn)`).
     `fn` receives the handle and one table of the notification's fields.
  3. **Per-request callbacks:** a function passed to the call that makes the request
     (`Core.After`, `Core.Every`, `Store.Get`, the `done` of `Store.Set` and
     `Store.Delete`, `Commands.Register`, `Entity.Subscribe`); it takes the arguments its row
     states and lives as long as the request.

  An unknown name raises a Lua error. `fn` is a function or `nil` (which clears the
  handler). One handler per name per module or object; a second `On` replaces the first.
  Handlers are never table fields read by the host.
- **What is queued.** A module or object notification is queued only if a handler for it is
  set when it arises, with one exception: a notification that **opens a gesture window**
  (§2.1: the `Pressed` of a solid press, an action's `"start"`, an `/ext:` line) is always
  queued, so that its window opens at its place in the turn; without a handler its
  delivery makes no call. Per-request callbacks are always queued.
- **One call per notification** (D17 §4). The host builds the argument table, charges its
  conversion first, and calls the handler. An error in a handler is logged and ends only that
  call; the next notification is delivered normally. An abort counts toward suspension
  (§2.4).
- **`by`.** Every UI notification's table has `by`: `"user"` (the user's input), `"script"`
  (the script's own call) or `"host"` (the host, for example a suspension that hid the frame,
  a screen resize or a coverage hide, §7.10) **(1a rule)**.
- **Order (1a rule, making D17 §4's "in the order they arose" exact).** Each script has one
  queue, **first in, first out by the moment the host appends an entry**. Entries are
  appended when the host produces them: notifications the script's own calls cause in 6b
  (appended during that 6b, so they precede the next frame's input), then in the next frame
  the step-1 input notifications in the order step 1 routes them, worker completions in the
  order the host collects them (in deterministic mode, in request order), and the
  hover-refresh notifications of step 7 item 1. A coalesced state notification keeps the
  position of its first entry and carries the latest value. A turn's **delivery set** is
  fixed when the turn starts: the entries queued before it, plus the matches its filter call
  appends (§10.3). Entries appended during the turn (for example by the script's own calls)
  wait for the next turn, so no notification is delivered re-entrantly.
- **Coalescing never reaches into a delivery set (1a rule).** Once a turn's delivery set
  is fixed, its entries are **frozen**: their values do not change. A state change that
  arises after that moment (the script's own call in this turn, or, in a turn split over
  frames, the next frame's step-1 input) never updates a frozen entry; it is appended as a
  **new** entry after the delivery set, and later changes of the same node and kind
  coalesce into that new entry until it is itself frozen in a later turn. So a node and
  kind have at most one entry outside the current delivery set (and at most one inside
  it), a self-caused value is always delivered in the next turn (D17 §4), and the handler
  may see the frozen value and then the newer one.
- **Queue cap and coalescing.** At most 1,024 queued notifications per script, all kinds
  together (TM L30). **State notifications** (§7.5 marks them) are coalesced per node and
  kind to one entry with the latest value (outside the current delivery set, above); the
  handler may therefore see a value equal to the one it saw last. A coalesced change is not
  a drop.
- **What is never dropped (TM L30).** Beyond the 1,024 entries the queue keeps **reserved
  room** for the results of the script's outstanding requests (TM L27: `Store.Get`'s `fn`,
  the `done` of `Set` and `Delete`, an `Image`'s `Loaded`) and for its due timers. These
  are never dropped, by a full queue or by a suspension (below), and no script waits
  forever for one. Every outstanding request **ends exactly once**, in exactly one of
  these ways: (i) its one result is delivered; (ii) an `Image`'s `Loaded` with no
  `Loaded` handler set when it arises is not queued, as any object notification ("What
  is queued", above), and its request ends then; (iii) the `Loaded` of an `Image` that
  was removed, or whose `file` was changed, before its result arose is never
  delivered (§7.10: a removed node's handle is stale, and a node reports only the
  decode of its current `file`), and its request ends when its result arises; (iv) at
  unload every result not yet delivered (the `done`s of §5.3, a `Store.Get`'s `fn`, a
  `Loaded`) is discarded with the apartment, and its request ends then. Whatever the
  way, the request stays counted under TM L27 until any worker job it started has ended
  (below).
  Every such result belongs to a request **counted under TM L27** from its call until it
  is delivered or discarded (and any worker job it started has ended): a `Store.Get`, an
  image decode, and every `Store.Set` or `Store.Delete` passed a `done`, a replaced
  value's pending `done` included (§5.3, TM invariant 36). L27 is per (kind, id), shared
  by both apartments of a slice-1b hot reload. So their room is bounded by TM L27
  (32 results) and TM L30 (64 timers, each queued at most once), and a loop of
  `Store.Set(k, v, done)` calls on one key is answered `nil, "busy"` at its 33rd pending
  `done` instead of growing the queue (TM X25).
- **What is dropped, and how it is reported (1a rule, making D17 §8's "counted" exact).**
  Every other notification that arises while the queue is full is dropped and counted.
  **Every queued module and object notification table carries `dropped`** (an integer ≥ 0, 0
  when nothing was lost; `Tick`, `Unload` and `Resumed` are turn items, never queued or
  dropped, and carry none): the number of notifications of the **same name, for the same
  module or object**, dropped since the last one of that name for it was delivered. The
  other forms carry their count in a field of their own: an `/ext:` command's `info.dropped`
  (command lines for this script dropped since the last delivered one, whatever their
  command word), an action's `ev.dropped` (§8.2: its edges past 16 in a frame and the edges
  of its dropped notifications), and a subscription batch's `dropped` (§10.3: its events). A
  count whose notification never comes (a node removed, a handler cleared) is lost with it;
  a count still held when the script is suspended moves into `Resumed.counts`.
- **Suspension.** A script is suspended by exactly these causes: the watchdog (the time
  throttle or three aborts, TM L17, L18), the panic chord (D17 §5.5), a grant revocation
  by any route (§2.9, TM §8.23) and a process locale other than "C" (§2.9, D17 §1.5). A
  suspended script accrues nothing: no notifications, no timers, no subscription events,
  no command lines, no action edges and no gestures. Its ext contexts are deactivated and
  their holds released in the frame of the suspension (§8.1, TM §8.23). Its pending
  notifications are dropped with counts, **except** the results of outstanding requests,
  which are kept with their reservations and delivered after `Resumed` in the first turn
  after the resume (a `done` whose write completes meanwhile is kept the same way). Its
  first turn after the user resumes it begins with `Resumed` (§4.1) carrying the counts
  and the cause (D17 §4).

<a id="s2-9"></a>
### 2.9 Capability checks

- `Ext.Use(name, 1)` hands out a module table only for a granted capability (§3).
- Every native of a granted module **re-checks the capability against the apartment's
  effective set (§2.1) and the backend class on every call** (D17 §5.1, TM §8.23), so a
  module table kept in a global, a closure or a table fails closed, and so does a handle
  that module minted (§2.7). The apply step of every intent re-checks the same way. No
  check reads the live grant table: a capability revoked and then widened again in the
  script manager stays refused until the script's next load.
- **Revocation** (TM §8.23). Revoking a grant of a loaded script, required or optional, by
  any route, suspends it in that frame: pending intents are discarded and nothing accrues.
  If the revoked capability is required, the script cannot be resumed and is unloaded (a
  close item, no `Unload` handler; D17 §2, "Refusal is whole"). If it is optional, the user
  may resume the script; from then on every function of the revoked module raises a Lua
  error `"not granted"`, `Ext.Has` answers `false` for it, every handle the module minted
  is refused by every native (§2.7), and the host has removed what the module made **(1a
  rule for what is removed)**: for `ui`, every node and world label, whose handles are
  stale; for `input`, the action handlers are cleared (its contexts are already inactive,
  as after every suspension, §8.1); for `entity.read`, every subscription, whose handles
  are stale (entity handles stay Lua objects, refused by every native, §2.7); for
  `world.read`, nothing. Escape test: TM X91.
- **The grant-store re-read is a route** (TM §8.23). When the script manager re-reads
  `ext-grants.toml` (EM §8.5) and the result leaves a **loaded** script with fewer grants
  than it runs with (its record removed, made invalid under EM §8.3, or narrowed by a
  capability), that is a revocation: the script is suspended in the frame the re-read
  result is applied, exactly as above, required and optional alike. A re-read that would
  **widen** a loaded script's grants changes nothing while it runs: `Ext.Has` and
  `Ext.Use` keep answering from the grants it was loaded with, and the wider record is
  honoured only at its next load (EM §9.2). A record that breaks EM §8.3's rules grants
  nothing. **Only the user narrows** (TM §8.23, TM invariant 35): a re-read narrows only
  when it compares two documents Tessera **read and understood as a whole** (readable, of
  a supported version, not damaged as a whole; a missing file reads as an understood,
  empty document). A re-read that cannot be used (a newer `version`, a document damaged as
  a whole, a failed open, kind check or read such as `EACCES`, `EIO` or a Windows sharing
  violation) changes nothing and suspends nothing. Tessera's own grant-store writes never
  narrow: a write's completion is applied as a read result only for the keys it did not
  carry, a newer decision of a carried key (a widen, or a first-enable Allow after a
  Reset or Remove, made while the older write was in flight) stays as the user made it,
  and a write that replaces a damaged document writes every record of the grant table, so
  its completion narrows nothing. So no completion of an earlier write cancels a load or
  suspends a script that a newer decision allowed.
- **The process locale** (D17 §1.5, TM T22, X22). The client checks that the process
  locale is "C" at every ext load and after every dialog returns. If it is not, **every**
  ext script is suspended in that frame with a reserved-layer notice, the event is
  logged as a defect, and (from slice 2) `Files` answers `nil, "unavailable on this
  backend"` for the rest of the session. While the locale is not "C", no ext load starts
  and the user cannot resume a script: the host checks the locale again at each load and
  each resume and refuses it with the same notice **(1a rule: no load or resume while the
  locale is not "C")**. A script resumed
  afterwards receives `Resumed{reason = "locale"}` (§4.1).
- `Core`, `Store` and `Commands` need no grant and are present on every backend that admits
  ext (D17 §3). Like every module they must be declared in the manifest (§3).
- In slice 1a the backend gate admits `Offline` only (D17 §6.2). The "T" column of D17 §3
  applies from slice 3.

<a id="s2-10"></a>
### 2.10 Angles and rotations (general rule, 1a rule; every API spec follows it)

This section is the one home of the angle forms of every API spec (EA1b and later
slices' specs cite it; there is no other "heading" section). Every **direction angle** an
ext function takes or returns, that is every angle that says where something faces or
looks, is in **radians** in KM §9.1's convention:

- **Yaw** (any argument or result named `yaw`): radians, counter-clockwise from +X seen
  from above, yaw 0 facing +X and π/2 facing +Y (KM §9.1). Arguments accept any finite
  value (read modulo 2π); results are wrapped into (−π, π].
- **Pitch** (any argument or result named `pitch`): radians above the horizontal (KM
  §9.1). The range an argument accepts, or the clamp applied to it, is the function's
  (stated in its row).
- **Angles about +Z relative to a yaw** (for example an orbit or bearing offset around a
  target): radians, counter-clockwise seen from above, like yaw.
- **Heading** (any **result** named `heading`): a display convenience only, the compass
  bearing in degrees in [0, 360) of the same direction: 0 = north (+Y), 90 = east (+X),
  clockwise. heading = (90 − yaw · 180 / π) wrapped into [0, 360). **No argument of any
  API spec is ever a heading** and no argument takes degrees for a direction.

So a `yaw` read from `World.Here()` or an entity, and any `pitch` a module returns, passes
**unchanged** to every
function of every slice that takes a direction: EA1b's `Camera` pose, `Follow`, camera
path key frames and `Sandbox.Teleport` take `yaw` and `pitch` in exactly this form (EA1b
§3.1 follows this section; `tessera-net`'s heading already uses yaw's convention, so the
`Sandbox` port only narrows it to `f32`). An angle that is an **extent**, not a direction
(a field of view), is not covered by this rule; the module that takes it states its unit.
- **Rotation** (any field named `rotation`): a unit quaternion `{w, x, y, z}` (scalar
  first, Hamilton convention) that rotates the object's model frame into the region frame
  (right-handed, +Z up). Results are normalised; arguments of later slices that take one
  normalise it and raise a Lua error for a zero or non-finite quaternion.
- **Facing of an object** (the direction its `yaw` and `heading` describe): the horizontal
  projection of its model +Y axis rotated by `rotation` (character models face +Y, KM §9.1).
  When that projection is shorter than 10⁻⁶ (the axis points straight up or down), yaw is 0
  and heading is 90 **(1a rule)**.

<a id="s2-11"></a>
### 2.11 The ext environment (D17 §1.2, §1.5; TM §8.15)

Every apartment is created from the **ext profile** of the shared core (D17 §1.1–§1.2).
What a script finds there is D17's and TM's; this section states it for the script author
and adds nothing.

- **Globals** (D17 §1.2, exactly these): `_G`, `_VERSION`, `assert`, `error`, `gcinfo`,
  `getmetatable`, `ipairs`, `next`, `pairs`, `pcall`, `print`, `rawequal`, `rawget`,
  `rawset`, `select`, `setmetatable`, `tonumber`, `tostring`, `type`, `unpack`, `xpcall`;
  the `coroutine`, `string`, `table` and `math` libraries; `collectgarbage` with the
  `"count"` option only; and `Ext` (§3).
- **Library tables** (TM §8.15, exactly these functions): `string`: `byte`, `char`, `find`,
  `format`, `gmatch`, `gsub`, `len`, `lower`, `match`, `rep`, `reverse`, `sub`, `upper`;
  `table`: `concat`, `insert`, `maxn`, `remove`, `sort`; `math`: `abs`, `acos`, `asin`,
  `atan`, `atan2`, `ceil`, `cos`, `cosh`, `deg`, `exp`, `floor`, `fmod`, `frexp`, `huge`,
  `ldexp`, `log`, `log10`, `max`, `min`, `modf`, `pi`, `pow`, `rad`, `random`,
  `randomseed`, `sin`, `sinh`, `sqrt`, `tan`, `tanh`; `coroutine`: `create`, `resume`,
  `running`, `status`, `wrap`, `yield`. The Lua 5.1 compatibility names `table.foreach`,
  `table.foreachi`, `table.getn`, `table.setn`, `string.gfind` and `math.mod` are
  **absent** and unreachable by any route (TM X8). Wrapped functions charge their work
  (D17 §8); `math.max` and `math.min` charge by their argument count, as `select` does
  (TM §8.15).
- **Absent** (D17 §1.2): `load`, `loadstring`, `setfenv`, `getfenv`, `string.dump`,
  `newproxy`, `debug`, `io`, `os`, `package`, `require`, `dofile`, `loadfile`, `module`,
  `import`, `Turbine`, `Plugins`, `plugin`. There is no way to compile a string (§12);
  other files of the script's folder are reached only with `Ext.Load` (§3).
- **Replacements.** `print` writes to the script log, never to a console or chat (§4.3).
  `tostring` (and so `print`) shows a table, function, thread or userdata as `table: #17`,
  a per-apartment counter, never an address. `math.random` and `math.randomseed` use a
  generator of the apartment's own, with Lua 5.1's argument forms; `scripttool test`
  seeds it from the test's seed (D17 §1.2, TM §9.5).
- **Strings and numbers.** String comparison is byte-wise in every state, whatever the
  process locale; number conversions assume the "C" locale, which the client keeps for
  its whole life (D17 §1.5; §2.9 says what happens when it is not).
- **Handles** are host userdata with sealed metatables (§2.7); the script cannot make
  userdata of its own and has no finalizers.

---

<a id="s3"></a>
## 3. The `Ext` root (summary)

The root of `api` major 1 has these four entries (EM §12.2); later slices add to it only
by EM §12's rules. They need no grant.

| Signature | Returns | Cost | Notes |
|---|---|---|---|
| `Ext.Use(name, major)` | module table \| `nil, reason` | `B` | `name` a module name (§4–§10), `major` an integer (1 in slice 1a). Answers in the order below. Repeated calls in one apartment return the same table. The table belongs to the apartment; changing it changes only that apartment's copy, and every function in it still re-checks its grant (§2.9). |
| `Ext.Has(name, major)` | boolean | `B` | Whether `Ext.Use` would succeed now. Grants nothing. |
| `Ext.Load(dotted)` | the value the file's chunk returned | `B` + the chunk's own run | Resolves `a.b.c` to `a/b/c.lua` of the snapshot, runs its precompiled chunk once per apartment, caches the result (D17 §2). A missing file, a segment outside the folder grammar, or a file being loaded that loads itself again raises a Lua error. Never compiles at call time. |
| `Ext.Version` | string | — | Tessera's version, `"major.minor.patch"`. A field, not a function. |

**`Ext.Use` and the manifest's module tables (1a rule, closing EM OPEN O-2).** The load has
already checked `[modules]` and `[optional_modules]` against the build's registry, minors
included (EM §4.8): a required module whose major is not served, or whose served minor is
lower than the manifest asks, refused the load. At run time `Ext.Use(name, major)` answers,
checking in this order:

1. `name` is not a module of the registry: `nil, "unknown module"`.
2. `name` is not declared in `[modules]` or `[optional_modules]`, or is declared with
   another major: `nil, "not declared"`. This is a fifth reason beside D17 §2's four, so
   that `scripttool check` (EM §11.3) and the run time cannot diverge: a script uses only
   what its manifest states. `Core`, `Store` and `Commands` are no exception.
3. An optional module whose major is not served, or whose served minor is lower than the
   manifest asks: `nil, "major not supported"` (EM §4.8's table).
4. The module's capability is not in the effective rights (EM §7.1): `nil, "not granted"`.
5. The backend class does not admit the module: `nil, "unavailable on this backend"`.
6. Otherwise the module table of that major. Its functions are those of the **served**
   minor, which is at least the one the manifest asks; a script that needs a later
   addition declares that minor (`Core = "1.2"`).

A module whose major is deprecated (EM §12.3) is answered as served; the load already logged
its warning.

---

<a id="s4"></a>
## 4. `Core`

Capability: none. Group: —. Backends: Off (T from slice 3). Slice 1a. Obtained with
`Ext.Use("Core", 1)`.

`Core` gives the script its lifecycle handlers, its clock and timers, its log and its
settings.

<a id="s4-1"></a>
### 4.1 Lifecycle handlers

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Core.On(name, fn)` | nothing | `B` | at once | Sets or clears the handler of one of `Core`'s notifications (§2.8, form 1): `"Tick"`, `"Unload"`, `"Resumed"`, `"SettingsChanged"` (below). Any other name raises. |
| `Core.Unload()` | nothing | `B` | after this call | Queues the script's own unload: the current call finishes, no further call of this turn runs, and the unload items (`Unload` handler, then state close) follow in 6a. Not a user gesture. **(1a rule)** |

| Notification | `fn`'s table | When |
|---|---|---|
| `"Tick"` | `{dt}` | Once per turn, after the script's notifications and due timers (D17 §4). `dt` (seconds, ≥ 0) is the time since the last `Tick` the script received (TM §8.13; deterministic mode: simulated time); the first `Tick` gets the time since the start of the frame of the top-level run. A throttled script is ticked at half rate (TM L17) and gets the summed `dt`. |
| `"Unload"` | `{reason}` | Runs as the `Unload` item before the state closes (D17 §4, "Unload items"), with its own 4 ms deadline (TM L3). `reason`: `"user"` (the user or the script unloaded it), `"reload"` (a reload, hot or full, D17 §10; EA1b §8.4 adds `Core.SetHandOff` for a hot reload), `"session"` (backend switch or session end), `"exit"` (client exit). It is **not** called for a suspended script (D17 §5.5), for a script whose required grant was revoked (§2.9), or for an apartment closed by the switch timeout (TM rule 7.4.2, §2.6); a call already running when the timeout passes finishes within its deadline. Its return value is ignored. `Store.Set` calls made in it are flushed by the unload's final flush (§5). |
| `"Resumed"` | `{counts, reason}` | The first call of the first turn after the user resumes the suspended script: it comes **before** that turn's filter call (§10.3), so the filter call, the notifications and the results kept over the suspension (§2.8) follow it in the order of §2.1's "Turn"; it is a turn item, never queued, so it is called even with the queue full, and with no handler set no call is made **(1a rule: the order)**. `counts` maps each kind of loss to its number, with **exactly these keys**, each present only when its number is above 0 **(1a rule)**: a **notification name** of §4.1, §7.2, §7.5, §7.7–§7.9 (CamelCase, for example `"TextChanged"`, `"Pressed"`, `"SettingsChanged"`), counting the module and object notifications of that name dropped at or during the suspension, over all objects, plus the drop counts still held for that name (§2.8); `"action"`, the action edges dropped (past 16 per frame, or with their notifications) whose counts were still held or that were discarded at the suspension, all actions together (§8.2; while suspended, no edge is recorded and none is counted, §8.1); `"command"`, the `/ext:` lines for the script dropped at the suspension or refused while it was suspended (§6.2); `"entity"`, the subscription records skipped by the cursor's jump (§10.3) plus the events of dropped batches; `"timer"`, the timer firings that did not happen while suspended, beyond the one each due timer makes after the resume (§4.2). Lowercase keys cannot collide with notification names, which are CamelCase. Results of outstanding requests are never counted: they are delivered (§2.8). `reason` is the cause of the suspension (§2.8): `"panic"`, `"watchdog"` (the time throttle or three aborts, TM L17, L18), `"grant"` (a revocation by any route, D17 §4, TM §8.23, §2.9) or `"locale"` (the process locale was not "C", D17 §1.5, §2.9). |
| `"SettingsChanged"` | `{keys}` | In the turn after the user applies a change on the script's settings page; `keys` is an array of the changed setting keys in manifest order. |

<a id="s4-2"></a>
### 4.2 Clock and timers

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Core.Time()` | number | `B` | read | Seconds from the start of the frame in which the script's top-level run ran, to the start of this frame (step 1). **One value per frame**: every call in one frame returns the same number (D17 §3). Never decreases. In deterministic mode (`scripttool test`) it is the sum of the walk file's frame times. |
| `Core.Frame()` | integer | `B` | read | Frames since the frame of the top-level run (that frame is 0). |
| `Core.Remaining()` | integer | `B` | read | Work units left in the current call: the smaller of the units left and the remaining wall time ÷ 5 ns. In deterministic mode, units only. Lets a script cut long work into pieces. **(1a rule)** |
| `Core.After(seconds, fn)` | timer \| `nil, "limit"` | `B` + `h(1)` | at once | A one-shot timer. `seconds` ≥ 0 and ≤ 10^6 (domain, TM §0.1 rule 6). `fn(timer)` runs in the first turn whose `Core.Time()` is at least the creation time + `seconds`, and never in the turn that created it. |
| `Core.Every(seconds, fn)` | timer \| `nil, "limit"` | `B` + `h(1)` | at once | A repeating timer, same rules. After it fires, its next due time is the larger of (due + `seconds`) and (now + `seconds` ÷ 2), so a late timer never fires twice in one turn (no catch-up bursts). `seconds` = 0 fires once in every turn. |
| `timer:Cancel()` | nothing | `B` | at once | The handle becomes stale. Cancelling inside its own call is allowed. |
| `timer:Exists()` | boolean | `B` | read | |
| `timer:Kind()` | `"Timer"` | `B` | read | |

- **Limit:** 64 live timers per script (TM L30); a 65th answers `nil, "limit"`.
- **Order:** due timers run in due-time order, ties in creation order, before `Tick`.
- **Suspension:** timers do not fire while the script is suspended; on resume, each fires at
  most once in the first turn, then follows its rule.

<a id="s4-3"></a>
### 4.3 Log

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Core.Log(level, ...)` | nothing | `B` + `b(n)` | buffered; written by a worker within 1 s | `level`: `"debug"`, `"info"`, `"warn"`, `"error"`. The other arguments are turned into text as the stock `print` does (each through `tostring`, which may run the script's own `__tostring` inside this call), joined by a tab. |
| `print(...)` | nothing | `B` + `b(n)` | as above | The same as `Core.Log("info", ...)` (D17 §1.2). |

- **Where it goes.** The script log `logs/scripts/<kind>/s_<id>.log` and, in developer mode,
  the developer console. **Never the chat window** (D17 §7, "Chat").
- **One record per call, in TM §8.1's one log format.** Each call is one record on one line:
  the host prefix `<time> <level> <source>[ <chunk>:<line>]: ` (UTC time; the level; source
  `print` for `print`, `log` for `Core.Log`; the relative chunk and line of the caller when
  known), then the text, with every control character escaped as TM §8.1 says (`\xNN`,
  `\u{2028}`, `\u{2029}`, `\\`; invalid UTF-8 as U+FFFD). A record is at most 4,096 bytes
  after escaping, prefix included, and longer text ends in `…[cut]` (TM L45). Errors and
  aborts the host reports use source `error`; the host's own records (such as "N records
  dropped") use `host`. So no script text can end a line, start a forged record or carry a
  terminal escape sequence (TM X51).
- **Limits** (TM L45, §8.1): two token buckets of escaped record bytes, each holding one
  second of its rate: **64 KiB per second per script** and **128 KiB per second for all
  scripts together**. A record is written only when both buckets hold its bytes; otherwise
  it is dropped and counted in that script's drop count, and when both buckets next allow
  it the host writes that script's record "N records dropped" (source `host`, paid from
  both buckets). So all script logs together write at most 128 KiB × (*t* + 1) in *t*
  seconds, whatever the number of scripts (TM invariant 33, X56). The all-scripts rate
  changes D17 §8's script-log row and awaits the owner's confirmation (TM OPEN O-3); until
  then it applies. One file of at most 1 MiB, rotated once. The developer console shows
  records as EA1b §9.2 and TM §8.1 say.

<a id="s4-4"></a>
### 4.4 Settings and identity

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Core.Setting(key)` | boolean \| number \| string | `B` + `b(n)` | read | The current value of a manifest `[[settings]]` entry: `bool` → boolean, `number` → number in its range, `choice` → the chosen option's string, `text` → string within its length cap. An unknown key raises. Values are set only by the user; no call writes them (D17 §5.2). |
| `Core.Settings()` | table | `B` + `e(2n)` + `b(bytes)` | read | All *n* settings as a new table key → value. |
| `Core.Script()` | table | `B` + `e(10)` + `b(bytes)` | read | `{id, kind, name, version, api}`: the script id, `"installed"` or `"developer"`, the manifest's `name` and `version`, and the `Ext` major it targets. No path, no user or host name (D17 §5.6). |

---

<a id="s5"></a>
## 5. `Store`

Capability: none. Group: —. Backends: Off (T from slice 3). Slice 1a. `Ext.Use("Store", 1)`.

`Store` keeps the script's own values across loads in `data/script-store/<kind>/s_<id>/`,
one file `k_<key>.store` per key (with at most one write-behind temporary file
`t_<key>.tmp` per key, EM §3.1, §3.6), written in the core's neutral Lua-table data format
and read back by the neutral reader, **never executed** (D17 §1.1, §2, §3). The store
directory and every entry in it are opened relative to directory handles and **never
through a link** (TM §8.8, EM §8.8): a store directory that is a link or not a plain
directory refuses the load, and an unexpected entry inside it is never opened or listed
(TM X94). Nothing of this is visible to the script except as a refused load or
`"io"`.

<a id="s5-1"></a>
### 5.1 Keys and values

- **Keys** follow the script-id grammar: 1–32 bytes of `a-z`, `0-9`, `_` (TM L38). A key
  outside the grammar raises a Lua error. The device names `con`, `prn`, `aux`, `nul`,
  `com0`–`com9`, `lpt0`–`lpt9` answer `nil, "reserved key"` (D17 §2, TM X53).
- **Values** follow the bounded value model (D17 §8): `nil` (meaning "delete"), booleans,
  numbers, strings (any bytes), and tables of those; at most 32 levels, 8,192 elements (every
  key and every value counts one), 1 MiB as serialised in the file (TM L33–L35, L39). Table
  keys are booleans, strings or numbers that are not NaN. Walks use raw access and ignore
  metatables; a table met twice on the current path is a cycle. A table reachable by two
  paths is stored twice and comes back as two tables.
- **NaN values (1a rule).** A NaN anywhere in a value to store answers `nil, "invalid"`, so
  every stored file reads back exactly. ±∞ values and keys are stored and read back.
- A value read back is a **new** tree; no identity is kept between reads.

<a id="s5-2"></a>
### 5.2 The file index, the pending-write map and read-your-writes (TM §7.6)

This section states TM §7.6 (D17 open item 6) and TM §8.14 in script-visible form.

- **Opening the store** (TM rule 7.6.1, §8.7, §8.8, §8.24). On the store's I/O queue,
  inside the load's snapshot job and before the top-level run, the host opens the store
  directory without following links (TM §8.8), checks the store's owner tag (a tag that is
  missing, **unreadable** or names another `author` causes a wipe first, and the script
  manager reports it), removes temporary files a crash left behind and unexpected entries
  (EM §8.8), lists the key files and builds the **file index**: key → its file's **exact
  byte size** and its **counted size** (TM rule 7.6.1, §8.7 "Counted bytes"). The exact
  size bounds every later read of that file and sizes its result reservation (§5.3, TM
  §8.14); the counted size enters the space totals and `Store.Usage` (§5.3). The file index is charged to the memory cap of **every apartment that has the store
  open**: during a slice-1b hot reload, when both apartments share the store, it counts in
  full toward both caps until the old apartment's cap leaves the sum (rule 7.6.5), then
  toward the new apartment's only. It changes only by work on the store's I/O queue (a
  completed flush, a flushed deletion, a wipe).
- **The pending-write map** (TM rule 7.6.2). One map per **(kind, id)**, not per apartment,
  so both apartments of a slice-1b hot reload share it. It holds, per key, the serialised
  bytes of the latest value written and not yet renamed into place, or a pending "absent"
  for a deletion. A flush **takes** an entry's value when its job starts writing that
  value's temporary file (or starts the removal, for a deletion); from then until a newer
  `Set` or `Delete` of the key the entry is **taken**, otherwise **untaken**. A newer
  `Set` or `Delete` replaces the entry's bytes and makes it untaken again. An entry leaves
  the map only when the flush that took **that** value has renamed it into place (or
  removed the file); the flush in flight for older bytes never removes a newer entry. The
  write buckets (§5.3) read this state. `Has`, `Keys` and `Usage` answer from the pending
  map over the file index, without I/O.
- **Read-your-writes** (TM rule 7.6.3). `Store.Get(key, fn)` answers the value of the
  **latest `Set` or `Delete` of that key made before the `Get` call, in program order**:
  - if the key has a pending entry, the answer is **that entry's bytes as they are at the
    call**: the host keeps every pending value's bytes immutable (a newer `Set` makes new
    bytes, never changes the shared ones), and the `Get` **shares** them, copying nothing
    (TM rule 7.6.3). An implementation that copies instead charges the copied bytes,
    `b(stored size)`, at the call before it copies (D17 §8, "charge before work"). A
    pending deletion answers "absent" (`fn(nil)`). No worker job and no file read is made;
  - otherwise the read is a job on the store's I/O queue, behind every earlier flush of that
    key, so the file it reads is current.

  Either way the script sees an ordinary read: `fn` is never called in the calling turn; a
  read answered from the pending map is delivered in the script's **next** turn (TM rule
  7.6.3), and a read from disk in the first turn after its job completes (§2.6, "worker").
  The value is converted inside that call with its cost charged first, and the read counts
  against the read rate (TM L42) and as one outstanding request (TM L27) from the call
  until it is delivered or dropped. A `Set` or `Delete` after the `Get` does not change its
  answer.
- **Wipes** (TM rule 7.6.4). A wipe or an orphan removal clears the pending map and the file
  index of that store on its I/O queue, after the final flush; a new apartment never sees an
  entry from before a wipe.
- **Accounting** (TM rule 7.6.5). Pending entries, and the bytes a flush in flight holds,
  count toward the `Store` size totals at their counted size (TM L41) and toward the memory
  cap of the apartment that wrote them until they are renamed into place or discarded; a
  newer `Set` moves the entry's charge to the apartment that made it. A `Set`'s
  serialised bytes are charged to that cap **as the walk produces them**, before they are
  allocated, on top of everything the apartment already holds (the untaken value it
  replaces is freed only once the new value enters the map). Bytes that would pass the
  cap stop the walk with the Lua error `"not enough memory"` (§2.4 item 1 and its
  `Store.Set` exception: after the step-5 charge, in the walk, before `"full"` and
  `"busy"`); nothing enters the map and no bucket is debited **(1a rule)**. This is a
  refused allocation, not a result reservation, so it raises; only `Store.Get`'s
  reservation answers `nil, "out of memory"` (TM §8.14).

<a id="s5-3"></a>
### 5.3 Functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Store.Get(key, fn)` | `true` \| `nil, reason` | `B` + `b(key)` (a pending entry's bytes are shared, not copied, §5.2); at delivery, the conversion `e(n)` + `b(bytes)` charged first in the delivery call | the pending map (`fn` in the next turn) or worker (`fn` in the first turn after the job completes), §5.2 | `fn(value)` on success, with `nil` when the key does not exist or its deletion is pending; `fn(nil, reason)` on failure with `"invalid"` (a file that does not parse, or whose size on the handle differs from the file index, below), `"too large"` (a parsed result over its reservation, never delivered) or `"io"`. Reasons at the call, in this order (§2.4): `"reserved key"`, `"busy"` (the read rate, TM L42, or the outstanding-request cap, TM L27), `"out of memory"` (the result reservation, below). A refusal queues nothing. `fn` is never dropped (§2.8). |
| `Store.Set(key, value [, done])` | `true` \| `nil, reason` | `B` + `b(key)` + reservation `V`, refunded to the real walk | at once in the pending map; file within about 1 s (worker) | Serialises the value **inside this call** (D17 §8, "Value conversion is not a residual"). Reasons, in the order of §2.4's `Store.Set` exception: `"reserved key"`; `"busy"` when `done` is passed and the script already has 32 outstanding requests (TM L27); the first of `"too large"`, `"cyclic"` and `"invalid"` the walk meets (or the raised `"not enough memory"`, §5.2 "Accounting"); `"full"`; `"busy"` (the write buckets, below). A refusal changes nothing, debits no bucket and counts nothing. `value = nil` is `Delete`. `done(true)` is delivered in the turn after this value, or a later value of the same key, is on disk; `done(false, "io")` if that write failed. A `done` of a value replaced while its pending entry was still **untaken** (§5.2) is delivered when the replacing write completes, with that write's result. A `done` of a value that a flush had already **taken** when it was replaced is delivered when **that** flush's own write completes (`done(true)` once the value is on disk, `done(false, "io")` if that write failed), whatever becomes of the replacing value **(1a rule)**. **Every accepted call with a `done` is one outstanding request** (TM L27) from the call until its `done` is delivered or discarded, a replaced value's pending `done` included, so at most 32 `done`s (with the script's reads and decodes) are pending at once and the never-dropped room of §2.8 stays bounded. |
| `Store.Delete(key [, done])` | `true` \| `nil, reason` | `B` + `b(key)` | as `Set` | Removes the key's file. Deleting an absent key succeeds. Reasons, in the order of §2.4's `Store.Delete` exception: `"reserved key"`; `"busy"` (with a `done`, the outstanding-request cap, TM L27; then the write buckets). A `done` counts toward L27, and is delivered, as `Set`'s does. **No `"full"` answer** (TM L41, rule 7.6.5): a pending deletion adds **no key and no byte** of its own to the space totals (TM L40, L41), whether the key is present or absent; a present key's file keeps counting through the file index until the flush that removes it completes, and a value the deletion replaces keeps counting while a flush in flight holds its bytes. So a deletion never raises a total and can never pass a space cap; only a `Set` can, and it answers `"full"`. (The 4 KiB a deletion takes from the write buckets is wear, not space.) |
| `Store.Has(key)` | boolean | `B` + `b(key)` | read (index) | The pending map over the file index. |
| `Store.Keys()` | array of strings | `B` + `e(2n)` + `b(bytes)` | read (index) | Every key (*n* of them), sorted byte-wise. |
| `Store.Usage()` | table | `B` + `e(10)` | read (index) | `{keys, bytes, max_keys, max_bytes, pending}`: keys held, counted bytes (each key file, pending entry and temporary file counted as TM §8.7 says), the caps (TM L40, L41: 1,024 and 16 MiB), and the number of pending entries. |

- **Write-behind** (D17 §8, TM L43): a script's dirty keys are flushed at most once per
  second, each to its temporary file `t_<key>.tmp` in the store's directory, created
  exclusively and renamed over the key's file, so a crash leaves each file with its old or
  its new value. A key set again before a flush takes its value replaces the pending value
  and causes no extra file write.
- **Write buckets, debited at the call** (TM §8.7, L43, L44). A key is **dirty** while it
  has an **untaken** pending entry (§5.2) and **clean** otherwise: with no entry, or with an
  entry whose value a flush has already taken, **even while that flush is still in
  flight**. A `Set` or `Delete` that makes a clean key dirty takes one token from the
  script's key bucket and the **full** counted size of its new value (a deletion counts
  4 KiB) from the script's byte bucket and from the all-scripts byte bucket. Only a call on
  a key that is already dirty (it replaces an untaken value) takes just the growth of the
  counted size over that untaken value, if any; a replacement that does not grow takes
  nothing. If any bucket lacks the tokens the call answers `nil, "busy"`, debits nothing
  and changes nothing. So every value a flush takes was paid in full, and a `Set` that
  lands while the flush of the key's previous value is in flight pays again in full. A
  flush is never refused or delayed by the buckets. The buckets belong to the (kind, id)
  for the client session, so an unload, a reload or a backend switch does not refill them.
  The figures (TM L44: 32 KiB/s per script with a 10 MiB bucket, 128 KiB/s for all with a
  40 MiB bucket) are lower than D17 §8's row and await the owner's confirmation (TM OPEN
  O-3); until then TM's apply. So a script that rewrites one small key every frame pays a
  key token and the value's full counted size **once per flush that takes it**, and causes
  one file write per second for it; a script rewriting its whole live set every frame is
  held to the bound of TM X57 (the bucket's capacity plus its refill), whatever the timing
  of its calls against the flushes.
- **Read rate** (TM L42): per script 64 reads and 4 MiB per second (buckets holding 10 s),
  and 16 MiB per second for all scripts; the bytes counted are the stored size TM §8.14 names.
- **Result reservations** (TM §8.14, L28). A `Get` of *n* stored bytes (from the pending
  entry, otherwise the file index's exact size) reserves, against the script's memory cap
  at the call, **ρ × *n* + ε × min(*n*, 8,192)**, where ρ and ε are the neutral reader's
  declared worst-case parsed bytes per stored byte and host memory per element, every key
  and every value counting one element as §2.5 counts them (TM §8.14, OPEN O-4), never the 1 MiB maximum; a key with neither reserves nothing. A reservation the cap
  refuses answers `nil, "out of memory"` (**returned, not raised**) and queues nothing.
  The worker checks the file's size on the handle against the file index and reads at most
  that size plus one byte; a mismatch answers `fn(nil, "invalid")` without parsing. The
  parse runs under the reservation: a result that would pass it answers
  `fn(nil, "too large")` and is never delivered (TM §8.14, TM invariant 32). So 32
  outstanding reads of small values do not exhaust a 32 MiB script, and the 33rd answers
  `nil, "busy"` (TM X25).
- **Space** (TM L40, L41): 1,024 keys and 16 MiB per script, 16,384 keys and 256 MiB for all
  scripts with orphans included; temporary files of flushes in progress and pending entries
  count, a pending deletion with 0 keys and 0 bytes of its own (`Store.Delete`, TM rule
  7.6.5). A write refused by the all-scripts total is reported in the script manager naming
  the orphans (D17 §5.2).
- **Lifetime.** The store belongs to the (kind, id)'s grant record and carries its owner tag
  (TM §8.24): it survives unloads, updates with the same `author`, and orphaning; it is
  wiped when the `author` changes, when its owner tag does not match the loading script's
  `author`, when the user resets it, or before a `session.server` holder opens a local-only
  store (D17 §5.2). Slice 1a has no local-only capability; the I/O-queue test of D17 §6.6
  (TM X63) uses a test-only one.
- **At unload** every pending value is handed to the final flush (D17 §8), including values
  set in the `Unload` handler; `done` callbacks are not called after the apartment is gone:
  they are discarded with it (§2.8, way (iv) of a request's end), which ends their L27
  count (in a slice-1b hot reload, the
  old apartment's discarded `done`s stop counting toward the (kind, id)'s one L27 count
  that both apartments share, TM L27).
  A closing apartment's cap stays in the memory sum until that flush completes (TM L24).
- **Tests.** TM X88 (read-your-writes), X25 (reservations), X56 and X57 (disk caps and
  rates, X57 including the run with flushes held in flight), X60 (crash during flushes),
  X92 (owner tag), X94 (no link followed in the store directory).

---

<a id="s6"></a>
## 6. `Commands`

Capability: none. Group: —. Backends: Off (T from slice 3). Slice 1a.
`Ext.Use("Commands", 1)`.

The user types **`/ext:<id> <command> [arguments]`** in the chat input (or clicks a
shortcut that holds such a line). The client's command dispatch handles any word starting
with `ext:` (ASCII case-insensitive) in step 1, before the plugin command lookup, and queues
the line for the script; the line is **not echoed**, produces **no chat line**, never reaches
plugins and is **never sent to a backend** (D17 §3, "`Commands` in detail"; TM X66, X77).

<a id="s6-1"></a>
### 6.1 Functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Commands.Register(name, fn [, help])` | `true` \| `nil, "limit"` | `B` + `b(help)` | at once (the next step 1 dispatches to it) | `name`: KM name grammar without `.`, 1–32 bytes of `a-z`, `0-9`, `_` (D17 §3); outside it raises. Registering an existing name replaces its handler and help. `help`: a string of at most 96 bytes (TM L91; longer raises) shown by `/ext:help <id>` in the script manager. It is script text, so it is drawn as reserved-layer text through TM §8.2's steps 1–5, in step 4's **help line** category (96 code points and 96 bytes, its L91 cap), so an accepted `help` is never cut and only steps 1–3 and 5 change it (OPEN-11, closed by TM revision 5). At most 32 commands per script (TM L68). |
| `Commands.Unregister(name)` | nothing | `B` | at once | An unknown name does nothing. |
| `Commands.List()` | array of strings | `B` + `e(2n)` + `b(bytes)` | read | The *n* registered names, sorted. |

<a id="s6-2"></a>
### 6.2 Dispatch and delivery

- **Matching.** `ext:`, the id and the command name are matched ASCII case-insensitively, so
  `/EXT:Notes Add milk` reaches the command `add` of script `notes` (D17 §3).
- **Arguments.** The text after the command word, with leading and trailing ASCII
  whitespace removed and invalid UTF-8 replaced; at most 1,024 bytes (TM L91), cut at a
  UTF-8 boundary beyond that. The host does not split it: the script parses it.
  Arguments may come from a shortcut a plugin built, so a handler treats them as hostile
  input (D17 §7).
- **The call.** `fn(args, info)` in the script's turn of the same frame (the dispatch is
  native work in step 1, delivered **same-frame 6b**). `info` = `{command, truncated,
  dropped}` (`dropped`: the script's command lines lost since the last delivered one,
  §2.8).
  The line is a **gesture**: the turn that delivers it is a gesture window (§2.1, §7.7).
- **No command word.** `/ext:<id>` alone opens the script manager at the script's help page,
  as `/ext:help <id>` does; the script gets nothing **(1a rule)**.
- **Unknown or unloaded.** A line naming no loaded script, or a command the script has not
  registered at the time of dispatch, shows a notice in the reserved layer (its text through
  TM §8.2, L66, L67) and a script-log entry, never a chat line (D17 §3; TM X73). A command
  unregistered between dispatch and delivery is dropped with a log entry.
- **Suspended (1a rule).** A line for a loaded script that is **suspended** is not queued
  (a suspended script accrues nothing, §2.8) and is not a gesture. It shows the
  reserved-layer notice "⟨id⟩ is suspended: resume it first" (the wording of EM §9.8, its
  text through TM §8.2, rate-limited by TM L67) and counts once under
  `Resumed.counts.command` (§4.1).
- **Queue.** Command lines are notifications: they count toward the 1,024-entry queue and are
  never coalesced.
- **Management words.** `/ext:list`, `/ext:reload <id>` and `/ext:help [<id>]` belong to the
  host; ids `list`, `reload` and `help` are reserved (D17 §2–§3). Their output goes to the
  script manager and, in developer mode, the developer console, never to chat (D17 §3).
  `/ext:reload` follows EM §9.8, which this spec does not restate: it works only in
  developer mode (otherwise the notice "Developer mode is off", D17 §3, EA1b §8.1); an id
  that names no **loaded** script queues nothing and shows "No loaded extended script
  '⟨id⟩'" (a reload never starts a first load); a loaded but **suspended** script is not
  reloaded and shows "⟨id⟩ is suspended: resume it first". Which reloads are hot or full
  is EA1b §8's and EM §9.8's. No management word is delivered to any script.

---

<a id="s7"></a>
## 7. `UI`

Capability: `ui`. Group: O. Backends: Off (T from slice 3). Slice 1a. `Ext.Use("UI", 1)`.

The grant dialog says that the script "can show windows that take clicks and the mouse
wheel where they lie" (D17 §3, the stated exception). How the user removes such a window
is §7.11.

<a id="s7-1"></a>
### 7.1 Model

- **Ext widget set.** The script builds **frames** (top-level ext windows) that hold
  **controls**, plus free-standing **world labels**. The kinds are listed in §7.5. They are
  Tessera's ext widget set, built on `tessera-ui`'s element tree and painter but promising
  only what this section states (D17 §12).
- **Where frames lie.** Every ext frame lies in the **ext z-range**: above the 3D world (and
  slice 1b's ext overlays), **below every plugin window and every native panel**, whatever
  their z-order, and below the developer overlay and the reserved layer (D17 §4, §5.3). An ext
  frame can never cover a plugin window or a native panel.
- **Order inside the ext range (1a rule).** The frames of one script form a **block**. Blocks
  are ordered by the last user **solid** press (§7.6) on any frame of that script (most
  recent on top); a newly loaded script's block goes on top. Only a solid press raises a
  block or a frame: a non-solid press raises nothing in the ext z-order (TM rule 7.2.1), so
  a transparent full-screen catching frame cannot climb to the top of the range on every
  world click and take the clicks meant for other scripts' frames (TM X38). Inside a block,
  frames are ordered by the script's own `Raise`, shows and the user's solid presses. A
  script's call can reorder only its own block, so no script can raise its frame over
  another script's frame.
- **World labels** lie below every ext frame, in the order they were created (§7.9).
- **Ownership badges** (§7.7) are the top layer of the ext range, the **badge layer**: every
  badge is drawn above every ext frame of every script, badges among themselves in the
  z-order of their frames (TM §8.3). The caret and focus mark of a focused `Field` are drawn
  in the same layer, **above every badge**, so no badge is ever drawn over them, the
  field's own frame's badge attached outside the frame included (the one badge that may
  overlap a focused field's focus area, §7.7, visible-focus item 3; TM §8.3, L58).
  Plugin windows, native panels, the developer overlay and the reserved layer stay above
  them.
- **Units.** Geometry is in UI units (§2.1). Frame positions are relative to the screen;
  control positions are relative to their parent's top-left corner. Children are clipped to
  their parent's box.
- **Changes apply at once, layout included (1a rule, IB D8, TM §7.8).** A call that changes
  a node's geometry, or anything a layout depends on (a `Group`'s `layout`, `gap` or
  `padding`, a child's size or visibility, a `List`'s `rows` or font), recomputes the
  affected layout **inside that call**: the `Group` positions of §7.5 and the `Scroll`
  and `List` offset clamping. A getter called right after returns the new
  geometry, which is the geometry the next hit test uses (IB D8, "Geometry used for hit
  tests"). The `Moved` and `Resized` this causes are self-caused (`by = "script"`) and
  arrive in the next turn (§2.6). User changes (a drag, a resize grip) and host changes
  (a screen resize, a UI-scale change) lay out in the step that makes them (step 1)
  and are notified **same-frame 6b**. Step 8 only paints the tree and places world labels
  (§7.9); it never moves a box that a hit test reads. Text wrapping and `List` row heights
  change only what is drawn inside a box, never a box.
- **Surfaces (TM L49, L50).** Each frame takes one ext surface from its creation to its
  removal, shown or hidden, and one composition entry while shown. Controls draw into
  their frame's surface. A world label takes **no** surface of its own: the host paints
  shown labels into the ext label layer, and each shown label takes one composition entry
  (TM L50) and counts toward coverage (§7.10).
- **While a consent prompt shows**, ext frames and world labels are neither drawn nor
  hit-tested; the script keeps running and its nodes keep their state and visibility, and
  no `VisibilityChanged` is sent for it (D17 §4). The prompt takes the keyboard focus: a
  focused `Field` loses it, and the script gets `FocusChanged{focused = false,
  by = "host"}` in its next turn **(1a rule)**. The focus does **not** come back when the
  prompt closes; the field stays its frame's remembered focus, and only the user's solid
  press or a request in a gesture window (§7.7) focuses it again.

<a id="s7-2"></a>
### 7.2 Module functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `UI.Frame(props)` | frame \| `nil, "limit"` | `B` + `N` + `P(n)` + `b(bytes)` | at once; painted step 8 | Creates a frame from the property table (§7.4, §7.5). It starts **hidden** unless `props.visible` is `true`, in which case it is shown under the rules of `frame:Show()`. `"limit"`: the element counts (§7.10), the ext surface pool, or, when `visible` is `true`, the composition pool or a coverage bound (§7.10); then nothing is created. |
| `UI.WorldLabel(props)` | label \| `nil, "limit"` | `B` + `N` + `P(n)` + `b(bytes)` | at once; placed step 8 | §7.9. `"limit"`: the element counts, or, for a shown label, a coverage bound (§7.10); then nothing is created. |
| `UI.Screen()` | width, height, scale | `B` | read | The game window's client area in UI units and the user's UI scale. |
| `UI.On(name, fn)` | nothing | `B` | at once | Sets or clears the handler of `UI`'s one module notification (§2.8, form 1): `"ScreenChanged"`, `fn({width, height, scale, by = "host"})` in the turn after the screen size or UI scale changes. State notification. Any other name raises. |
| `UI.MeasureText(text, font [, wrap])` | width, height | `B` + `b(text)` | read | The box `text` needs in `font` (§7.4); with `wrap` (a width in UI units) it wraps at word boundaries as a `Text` node does. Results depend on the fonts available (OPEN-6) and are not stable across installs. Text over 16 KiB raises. |
| `UI.Frames()` | array of frames | `B` + `h(n)` + `e(2n)` | read | The script's frames in their z-order, bottom first. |
| `UI.Usage()` | table | `B` + `e(12)` | read | `{nodes, nodes_max, labels, labels_max, area, area_max}`: the script's node and label counts and caps (§7.10), and its composition coverage and its bound in screens (§7.10, TM L55). |

<a id="s7-3"></a>
### 7.3 Node methods (every kind)

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `node:Set(props)` | `true` \| `false [, "busy"]` | `B` + `P(n)` + `b(bytes)`, plus `e(2 · #rows)` when it sets a `List`'s `rows` | at once (layout included, §7.1) | Sets the listed properties in one step; an unknown property, a value of the wrong type or out of its range (judged on the node's state after the whole `Set`, so `Set{min = 5, max = 10}` is one valid step) raises, and then nothing is changed. Returns `false`, changing nothing, when the change would pass a composition-coverage bound or the composition pool (§7.10: a size change of a shown frame or label, `catch = true` or a `Field` that gives a shown frame its badge; no reason, the script log says why), and `false, "busy"`, changing nothing, when it sets an `Image`'s `file` while the outstanding-request cap is reached (§7.10, TM §8.5). |
| `node:Get(name)` | the property's value | `B` + `P(1)` + `b(bytes)`, plus `e(2 · #rows)` for a `List`'s `rows` | read | The current value; for geometry, the value the next hit test uses, with layout already applied (§7.1). |
| `node:On(name, fn)` | nothing | `B` | at once | Sets or clears (`fn = nil`) the handler of a notification the kind has (§7.5; §2.8, form 2). A name the kind does not have raises. `fn(node, args)`. |
| `node:Add(kind, props)` | node \| `nil, "limit"` \| `nil, "busy"` | `B` + `N` + `P(n)` + `b(bytes)` (plus `e(2 · #rows)` for a `List`'s `rows`) | at once (layout included, §7.1) | Creates a child. `"limit"`: the element counts or pools (§7.10), or a child that would give a **shown** frame its badge (its first catching node or `Field`) when the badge's coverage would pass a bound (§7.10, TM rule 7.1.5); then nothing is created. `"busy"` only for an `Image` with a `file` while the outstanding-request cap is reached (§7.10), and then nothing is created. Only `Frame`, `Group` and `Scroll` hold children; any other parent raises. `kind` is a kind name of §7.5 other than `Frame` and `WorldLabel`. |
| `node:Remove()` | nothing | `B` + `N` per node removed | at once | Removes the node and its descendants; their handles become stale. Removing the focused `Field` or the active frame applies §7.7. |
| `node:Children()` | array of nodes | `B` + `h(n)` + `e(2n)` | read | In drawing order, bottom first. |
| `node:Parent()` | node \| `nil` | `B` + `h(1)` | read | `nil` for frames and labels. |
| `node:Box()` | x, y, width, height | `B` | read | The node's box on screen in UI units after layout and clipping are applied, as the hit test sees it; `nil` when it is not shown. |
| `node:Exists()` | boolean | `B` | read | |
| `node:Kind()` | string | `B` | read | The kind name. Works on a stale handle. |

<a id="s7-4"></a>
### 7.4 Common properties

Every node kind except `WorldLabel` (§7.9) has these. "State" marks a property whose change
causes a state notification (coalesced, §2.8).

| Property | Type | Default | Meaning |
|---|---|---|---|
| `x`, `y` | number, \|v\| ≤ 16,384 (domain, TM §0.1 rule 6) | 0 | Position relative to the parent (the screen for a frame). A `Group` with a layout (below) sets its children's `x` or `y`. |
| `width`, `height` | number, 0 … 16,384 (domain) | kind's default | Size. Changing it causes `Resized` (state). |
| `visible` | boolean | `true` (controls), `false` (frames, until shown) | Whether the node and its subtree are shown. A frame's `visible` is set through `Show`/`Hide` or `Set`, with the same rules. Changing it causes `VisibilityChanged` (state). |
| `opacity` | number 0 … 1 | 1 | Multiplies the node's and its subtree's drawing. The **effective opacity** is the product along the path from the frame. |
| `enabled` | boolean | `true` | A disabled node is drawn dimmed, still catches the pointer if it would otherwise, but has no native behaviour and sends no pointer notifications. |
| `background` | colour or `nil` | `nil` | Fill of the node's box. |
| `border` | colour or `nil` | `nil` | A 1-UI-unit frame line inside the box. |
| `catch` | boolean | `true` for `Frame`, `PushButton`, `Toggle`, `Slider`, `Field`, `List`, `Scroll`; `false` for `Group`, `Text`, `Gauge`, `Image` | Whether the node catches the pointer (§7.6). |
| `name` | string ≤ 64 bytes (grammar, TM §0.1 rule 6) or `nil` | `nil` | A free label for the script's own use; the host never shows it. |

Text-bearing kinds also take:

| Property | Type | Default | Meaning |
|---|---|---|---|
| `font` | `"body"`, `"small"`, `"large"` | `"body"` | A font role. Which font face and size each role uses is Tessera's choice (OPEN-6). |
| `color` | colour | the ext theme's text colour | Text colour. |
| `align` | `"left"`, `"center"`, `"right"` | `"left"` | Horizontal alignment inside the box. |
| `valign` | `"top"`, `"middle"`, `"bottom"` | `"top"` (`"middle"` for `PushButton`, `Toggle`) | Vertical alignment. |

<a id="s7-5"></a>
### 7.5 Node kinds

Pointer notifications (§7.6) exist on every **catching** node: `PointerEntered`,
`PointerLeft`, `PointerMoved` (state), `Pressed`, `Released`, `Clicked`. Every node except a
world label has `Moved` (state, `{x, y}`), `Resized` (state, `{width, height}`) and
`VisibilityChanged` (state, `{visible}`). The table lists the rest. Every args table also
has `by` and `dropped` (§2.8). The args of `Pressed`, `Released` and `Clicked` also have
`gesture` (§7.6).

| Kind | Own properties (type, default) | Native behaviour | Own notifications |
|---|---|---|---|
| `Frame` | `movable` (boolean, `false`); `resizable` (boolean, `false`); `min_width`, `min_height` (number, 48, 32); `max_width`, `max_height` (number, the screen) | A frame that is `movable` follows a drag that starts with the primary button on a point where the frame itself is the catching node (not a catching descendant) and moves at least 4 UI units; it is kept so that its top 24 units and at least 24 units of its width stay on screen. A `resizable` frame gets a host-drawn grip of 12 × 12 units in its bottom-right corner; dragging it resizes within the min and max, and stops growing at the largest size that keeps both composition-coverage bounds (§7.10, TM rule 7.1.5). A drag that moves a frame is never limited by coverage. | `ActiveChanged` (state, `{active}`) |
| `Group` | `layout` (`"none"`, `"column"`, `"row"`; `"none"`); `gap` (number ≥ 0, 0); `padding` (number ≥ 0, 0) | A container. With `"column"`, each shown child in order gets `y` = `padding` for the first and the previous child's `y` + `height` + `gap` for the next; its `x` stays as set. `"row"` does the same along `x`. A child's own `y` (column) or `x` (row) is then overwritten, and `Moved` reports it. | — |
| `Text` | `text` (string ≤ 16 KiB, `""`); `wrap` (boolean, `false`) + text properties | Draws the text in the box, clipped; with `wrap`, breaks lines at spaces, or inside a word that does not fit. No editing, no selection. | — |
| `Field` | `text` (string, `""`); `max_bytes` (integer 1 … 4,096, 256); `multiline` (boolean, `false`); `placeholder` (string ≤ 256 bytes, `""`); `read_only` (boolean, `false`) + text properties | Single- or multi-line text entry with the editing behaviour of §7.8. Takes the keyboard focus only under §7.7. `text` must fit `max_bytes` on the node's state after the whole `Set` **(1a rule)**: a `Set` whose resulting text is longer raises and changes nothing, so lowering `max_bytes` below the current text's length raises unless the same `Set` also gives a text that fits; the host never cuts a script's text. | `TextChanged` (state, `{text}`); `FocusChanged` (state, `{focused}`); `Submitted` (`{text}`, single-line only) |
| `PushButton` | `text` (string ≤ 256 bytes, `""`) + text properties | Drawn by the host in the ext theme with a hover and a pressed look. | (its `Clicked` is the pointer notification) |
| `Toggle` | `text` (string ≤ 256 bytes); `checked` (boolean, `false`) + text properties | A box with a check mark and its text; a `Clicked` with the primary button flips `checked`. | `Toggled` (state, `{checked}`) |
| `Slider` | `min`, `max` (number, 0, 1; `min` < `max`); `step` (number ≥ 0, 0 = continuous); `value` (number, clamped into [min, max] and rounded to `min` + k · `step`); `vertical` (boolean, `false`) | A track and a thumb drawn by the host. A press on the track moves the thumb to the point; a drag moves it. No wheel and no keys. **(1a rule)** `min` ≥ `max` on the node's state after the whole `Set` raises and changes nothing. A `Set` of `min`, `max` or `step` clamps and rounds the current `value` again; a change of it is a self-caused `ValueChanged` (`by = "script"`, next turn). | `ValueChanged` (state, `{value}`) |
| `Gauge` | `value` (number 0 … 1, 0); `fill` (colour, theme); `vertical` (boolean, `false`) | A bar filled to `value` of its length, from the left (or the bottom). No input. | — |
| `Image` | `file` (string: a path relative to the script folder, `nil`); `tint` (colour, `#FFFFFFFF`); `fit` (`"stretch"`, `"contain"`, `"none"`; `"stretch"`) | Shows a picture from the script's folder snapshot (§7.10, "Images"). Draws nothing until decoded. | `Loaded` (`{ok, reason}`; `reason` `"too large"`, `"invalid"` or `"limit"` when `ok` is `false`) |
| `List` | `rows` (array of strings, at most 1,024 rows of at most 256 bytes; `{}`); `selected` (integer 1 … #rows or `nil`); `offset` (number ≥ 0, 0) + text properties | Rows of one line each, row height = the font's line height + 4 units, scrolled by `offset` (UI units, clamped to the rows' height). A primary-button click on a row selects it. **(1a rule)** A `Set` of `selected` outside 1 … #`rows` (after the whole `Set`) raises; a `Set` of `rows` that leaves the current selection past the new #`rows` clears it to `nil`, a self-caused `SelectionChanged{index = nil, by = "script"}` in the next turn, and clamps `offset` again. While the pointer is over it, the wheel scrolls it by 3 rows per notch instead of zooming the camera (D17 §4, "The wheel over ext scrollables"). A host-drawn thumb on the right edge when the rows overflow; dragging it scrolls. | `SelectionChanged` (state, `{index}`); `Scrolled` (state, `{offset}`) |
| `Scroll` | `content_width`, `content_height` (number ≥ 0, the box size); `scroll_x`, `scroll_y` (number, clamped to the content) | A container whose children lie on a content area of the given size, shown through the box at (`scroll_x`, `scroll_y`) and clipped to it inside its frame's surface (not a nested composition surface, §7.10). The wheel scrolls `scroll_y` by 40 units per notch while the pointer is over it (with Shift held, `scroll_x`); host-drawn thumbs on the right and bottom edges when the content overflows. | `Scrolled` (state, `{x, y}`) |
| `WorldLabel` | §7.9 | §7.9 | none |

Default sizes **(1a rule)**: `Frame` 320 × 200; `Text`, `PushButton`, `Toggle` 120 × 24;
`Field` 160 × 24 (multi-line 160 × 96); `Slider` 120 × 16; `Gauge` 120 × 12; `Image`,
`Group`, `List`, `Scroll` 100 × 100.

<a id="s7-6"></a>
### 7.6 The pointer

- **Where the pointer reaches ext frames.** A point is tried against the reserved layer
  first, then plugin windows and native panels, then the ext range, then the world (D17 §4,
  amending UB §4). A point reaches an ext frame only where nothing above it catches the
  pointer.
- **Hit test inside the ext range (1a rule).** Frames from the top of the range down; in a
  frame, its subtree from the topmost child down (later children above earlier ones), each
  child's subtree before the child itself, and the frame last. The first node that is
  shown, has a positive size, contains the point inside every ancestor's clip, and has
  `catch = true` **catches** it. Opacity does not matter: a transparent node with
  `catch = true` catches (D17 §5.6). A point that no ext node catches goes on to the world.
- **World labels never catch** (TM §7.8, D17 open item 8): they are not hit-tested, never
  hovered, get no pointer notifications, never count as covering for the focus rule, and
  moving them in step 8 changes no box that a hit test reads (TM X45). No property changes
  this.
- **Notifications** (all **same-frame 6b**, D17 §4):
  - `PointerEntered` and `PointerLeft` (`{}`) when the catching node under the pointer
    changes;
  - `PointerMoved` (`{x, y}`, state, at most once per frame per node): only while the node
    catches the pointer, in the node's own coordinates (D17 §5.6). A frame's background that
    does not catch receives nothing (TM X70);
  - `Pressed` and `Released` (`{button, x, y, gesture}`): `button` is `"primary"`,
    `"secondary"`, `"middle"` or `"other"`, for the pointer presses that reach the node;
    `gesture` is `true` for a solid press (below) and `false` otherwise (TM rule 7.2.1); a
    `Released` carries its press's value. Only the delivery of the `Pressed` of a solid
    press opens a gesture window; the flag on `Released` and `Clicked` is information
    only (§2.1, TM §8.13);
  - `Clicked` (`{button, x, y, gesture}`): a press and a release of the same button both on
    the same node, with no drag of a frame or slider in between; `gesture` as its press.
- **Capture.** After a press on a node, that node receives the pointer's moves and its
  release until the button is released, even outside its box (its coordinates may then lie
  outside the box).
- **Solid presses** (TM §7.2, D17 open item 2; L57). A user's press on an ext frame is a
  **solid press** only when the catching node at the press draws **its own background** at
  that point with alpha × effective opacity ≥ 0.5, where effective opacity is the product of
  the node's `opacity` and every ancestor's, the frame's included, **and** that node is the
  **topmost painted node** at the point: no other node of any ext frame paints over it
  there, whether or not that node catches the pointer (the script's own later siblings and
  their subtrees, later siblings of any ancestor, its own higher frames and other scripts'
  frames all included; TM rule 7.2.1). "Paints" means draws anything at that point with
  alpha × effective opacity above 0: a background, an image texel, text, a border or a
  host-drawn face. So a press that falls through a non-catching `Image` or `Group` onto a
  catching `Field` below it is **not** solid, even where the picture is opaque and the
  field's face is too (TM X38). "Its own background" is **(1a rule, the per-kind form of TM
  rule 7.2.1)**:
  - its `background` colour over its box;
  - for an `Image`, the alpha of the decoded texel under the point times the alpha of
    `tint` (where texel alpha is not available without a second decode, the image's
    minimum alpha, TM OPEN O-6); nothing before the image is decoded;
  - for `PushButton`, `Field` and `List` whose `background` is `nil`, the face the host draws
    over the whole box in the ext theme (an opaque colour); for `Toggle`, its host-drawn
    check box only; for `Slider`, its track and thumb; for `Scroll`, its thumbs.

  Text, borders, a `Gauge`'s fill, the caret, the focus mark and the ownership badge never
  make a press solid. A press that is **not** solid is still caught (it does not reach the
  world) and still notified with `gesture = false`, but for activation and focus it is
  treated **exactly as a click on the world**: it activates nothing, deactivates whatever
  window was active (a plugin window or native panel in step 1, as plugin time), starts no
  case 1 and refreshes no case-1 timer, focuses no `Field`, and is not a gesture. The test
  uses the node's state as step 1's hit test saw it; a later change of opacity or background
  does not make that press solid after the fact (TM rule 7.2.3). So a transparent
  full-screen catching node can stop world clicks (D17's stated exception) but cannot win
  the keyboard through them (TM X38).
- **Disabled nodes** (TM rule 7.2.1). A press on a catching node with `enabled = false` is
  solid or not by the same test, and a solid one activates its frame and raises its
  script's block by the same rule as any solid press. It delivers **no** notification, so
  it opens no gesture window (§2.1), and it has no native behaviour: a disabled `Field` is
  never focused by it, and a disabled `Frame` is not dragged.

<a id="s7-7"></a>
### 7.7 Activation and focus

There is one active window and one keyboard focus, shared with plugin windows and native
panels (D17 §4). Ext frames form their own domain for **requests**:

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `frame:Show()` | `true` \| `false` | `B` | at once; painted step 8 | Shows the frame **inactive**. `false` when the ext composition pool or a composition-coverage bound refuses it (§7.10); the frame stays hidden. Showing never activates. |
| `frame:Hide()` | nothing | `B` | at once | Hiding the active frame deactivates it and activates nothing else; its focused `Field` loses the focus (D17 §4). |
| `frame:Raise()` | nothing | `B` | at once | Moves the frame to the top of the script's own block (§7.1). Changes no activation or focus. |
| `frame:Activate()` | `true` \| `false` | `B` | at once | Honoured only under case 1 or case 2 below; otherwise remembers nothing, changes nothing, returns `false`. When honoured, the frame becomes active and the call returns `true`; its remembered `Field`, if any, is focused only if the focus rule below allows it, and otherwise stays remembered. |
| `frame:IsActive()` | boolean | `B` | read | |
| `field:Focus()` | `true` \| `false` | `B` | at once | Honoured only when the focus rule below allows it; then the field's frame becomes active if it was not. Otherwise the field becomes the frame's **remembered focus** and the call returns `false` (D17 §4). |
| `field:Unfocus()` | nothing | `B` | at once | Drops the focus if the field has it; the frame stays active. |
| `field:HasFocus()` | boolean | `B` | read | |

The `false` results carry no reason; the reason goes to the script log only **(1a rule)**,
so a script learns nothing about what covers its field.

**The user's own input** (step 1, native, D17 §4) moves activation and focus as for any
window: a **solid** press (§7.6) on an ext frame activates it and raises the script's block
to the top of the ext range, and the active plugin window or native panel is deactivated in
step 1, as plugin time; a solid press on an enabled `Field` that passes the visible-focus
rule below also focuses it. A press elsewhere (the world, a plugin window, a native panel, another
script's frame), and a non-solid press anywhere, deactivates the script's frame as it would
any window.

**Case 1: the user's attention is on this script** (D17 §4, as TM §7.2 tightens it). One of
the script's own frames is the active window, it became active by the user's solid press or
by a request honoured under case 1 or 2, and since then:

- no key press of the user has resolved outside the script's frames (in the game band, in
  another window, in the chat input);
- the pointer has not left all of the script's frames;
- no more than 5 s have passed since the user's last solid press on them, its release, or
  a key on them (TM L59).

Under case 1 a request may move **activation** among that script's own frames. A request
that would **focus** a `Field` (`Focus`, or an `Activate` of a frame whose remembered focus
is a `Field`) is honoured only inside a **gesture window** (§2.1) of this script; outside
one it only remembers the field and `Focus` returns `false` (TM rule 7.2.2, which replaces
D17 §4's "a timer can still focus a box before the user's next key").

**Case 2: a gesture window with no window active.** The request is made inside a gesture
window (§2.1) opened by the start of one of the script's own actions (§8.2) or by an `/ext:`
line for it (§6), and **no window is active** at the time of the request. Both activation
and focus may then be given. This is how a key-bound note-taker opens its frame with the
caret in its field; it works whether the script acts in the action's handler, in a later
handler of the same turn or in that turn's `Tick`, and in a turn split over frames (TM
§8.13, X89), within the window's 1 s (a turn split twice under permanent gating may
outlast it and then fails safe, §2.6). A timer or `Tick` of a later turn does not qualify. There is no exception for
the chat window: a line submitted from the chat input while the chat window is still
active does not qualify.

**The focus rule.** A `Field` is focused by a request only under case 1 or 2, only inside a
gesture window of its script, and only while the visible-focus rule holds.

**Never otherwise.** In the idle state of play (no window active) outside a gesture window,
and whenever a plugin window, a native panel or another script's frame is active, requests
return `false`. No call of this module runs plugin code (D17 §4; TM X71).

**The visible-focus rule** (D17 §4, TM L58). A `Field` can receive or keep the focus only
while:

1. it and every ancestor are shown, its box lies inside the screen, and no ancestor clips it
   to less than its full height and 16 units of width;
2. its effective opacity, and its frame's, is at least 0.5;
3. its **focus area** (its box grown by the width of the focus mark) is **not covered**: no
   shown element of a plugin window or a native panel (at any opacity), no part of the
   developer overlay, no surface of the reserved layer, **no frame of another script (1a
   rule)** and **none of the script's own content drawn above the field** draws anything
   inside it or catches the pointer there. The script's own content above the field is
   (TM rule 7.2.2, L58): every node of the field's frame that is drawn after it (a later
   sibling of the field or of any of its ancestors, with its subtree), and every higher
   frame of the same script, whether or not it catches the pointer; a node counts as
   drawing inside the area where it paints anything with alpha × effective opacity above 0
   (§7.6). **Ownership badges cover too (TM rule 7.2.2, §8.3, L58, which TM revision 6
   adopted from this spec):** a
   badge whose fixed box (below) overlaps the focus area with a positive area covers the
   field, although badges are drawn by the host: the badge of any other frame, of the same
   script or another (for example one attached above another script's frame), and the
   badge of the field's own frame when it is placed **inside** that frame (a frame at
   y = 5, along its top edge). The one exception is the field's own frame's badge attached
   **outside** the frame's top edge: it lies wholly outside the frame, so only the focus
   mark of a field at the frame's top edge can reach it, and that badge is drawn in the
   focus colour while the field has the focus anyway. The transparent, non-catching
   area of a full-screen native root does not cover; world labels never cover (TM §7.8);
4. its frame's **ownership badge** (below) lies wholly on screen and is not covered by any
   plugin window, native panel or reserved-layer surface (TM §8.3, L58). A press on the
   field while its badge is hidden is still a press, but the focus stays where it was.

The host checks the rule again at the end of every step that can change it (step 1's user
drags, toggles and host changes, step 7's plugin items, 6b, the item-7 drain). Layout
never runs in step 8 (§7.1), so step 8 cannot change it. A field that fails it loses the
focus at once, as a hide does; from that moment no key is routed to it (D17 §4; TM X39,
X49).

**What the host draws, not the script** (D17 §4; TM §8.3):

- the caret, and a focus mark (a border in Tessera's focus colour) around the focused field,
  drawn in the **badge layer**, the top layer of the ext range (§7.1), above every ext
  frame of every script and above every badge (TM §8.3, L58, adopted from this spec in
  TM revision 6).
  Item 3 of the visible-focus rule (other scripts' frames and the script's own higher
  content cover) means that no content of any script, its own included, draws over the
  caret or the mark of a field that holds the focus; the drawing order means that no
  badge does, the own frame's outside badge included; so with the focus area uncovered,
  both are on screen (D17 §4). Item 3's badge cover adds that no other badge even lies
  in the focus area: a frame move, a layout change or a show that brings such a badge
  into the area takes the focus at the next re-check below;
- an **ownership badge** on **every shown frame that has at least one catching node (the
  frame itself included) or any `Field`**: the text "⟨id⟩ · ext script" (for example "notes
  · ext script") in Tessera's fixed badge style, at full opacity whatever the frame's
  opacity, clipped to a **fixed box 20 UI units high and 16 + 8 × (*n* + 13) UI units
  wide**, *n* the id's length in bytes (at most 376 units, TM §8.3). The host attaches the
  box outside the frame's top edge, left-aligned with the frame, when that position lies
  wholly inside the screen; otherwise (a frame at y = 5, or partly off screen at the top,
  left or right) **inside** the frame along its top edge, shifted horizontally so that the
  whole box lies on screen; where the frame is too small or too far off screen for that, the
  badge counts as **off screen** (and no field of that frame can hold the focus, rule item
  4). It is shown whenever the frame is shown, not only while a field has focus, and is
  drawn in Tessera's focus colour as well while a `Field` of that frame has the keyboard
  focus. Badges are the top layer of the ext range (§7.1), so no ext frame of any script can
  be placed over one. The script cannot style, move, cover, hide or measure the badge; it is
  not a node, never a pointer target, and not part of any `node:Box()`. Its fixed box, never
  its drawn glyphs, counts toward the frame's composition coverage (§7.10), so the count
  does not depend on fonts. Escape test: TM X49.

**Notifications.** `ActiveChanged` and `FocusChanged` are state notifications delivered in
the script's next turn when its own request caused them, and in the same frame's 6b when
step 1's routing of the user's input caused them (D17 §4). Only the script's own frames and
fields are notified; plugins are notified of activation changes by the user's press only,
in step 1, as plugin time (D17 §7).

<a id="s7-8"></a>
### 7.8 Text entry in a `Field`

- **What the keyboard reaches.** While a field has focus it pushes `text_entry` and
  `ui.clipboard` into the UI band (IB D5 as D17 extends it), so the user's text-editing
  bindings drive it and every other key or pad press is swallowed at the band end
  (`opaque_keys`, IB D6), so no ext or game binding fires on a **key or pad press** while
  the user types (D17 §5.5). Mouse buttons, the wheel and axes are not stopped by
  `opaque_keys` (IB D6) and pass to the game band, so an ext binding on a mouse button can
  still fire while a field has the focus.
  The script never receives raw keys: it sees only the field's text (D17 §3).
- **Editing operations (Tessera's own set, 1a rule).** Insert typed text at the caret,
  replacing the selection; delete the character or selection before or after the caret; move
  the caret by character, word, line (multi-line) and to the start or end; extend the
  selection with the same moves while Shift is held; select by pressing and dragging; copy,
  cut and paste through the system clipboard (below); **submit** (the user's Enter: in a
  single-line field, `Submitted` with the text; the field keeps the focus; in a multi-line
  field, a line break); **cancel** (the user's Escape: the field loses the focus,
  `FocusChanged` with `focused = false` and `by = "user"`). Which keys perform them follows
  the user's keymap for those UI actions.
- **What is accepted.** Typed and pasted text is UTF-8; control characters are dropped except
  the line break in a multi-line field. An insertion that would pass `max_bytes` is cut at the
  last whole character that fits. `read_only` fields allow caret moves, selection and copy
  only.
- **Clipboard** (D17 §5.6; TM §8.4). No function of any module reads or writes the
  clipboard. Copy, cut and paste happen only by the user's keys in a field the user
  focused. A script **cannot create a selection** and has no caret function: only the
  user's own pointer and keys select text. **Any change the script makes to the field's
  `text` clears the selection and moves the caret to the end** **(1a rule: the caret)**. A
  copy or cut (TM §8.4) puts text on the clipboard only when **all** of these hold, and
  otherwise does nothing (the clipboard keeps what it held; a cut then removes nothing):
  1. **Selected by the user, unchanged.** The text is what the user selected, and the
     field's text has not changed since the selection was made.
     **The moment the selection was made (1a rule)** is the **last change of the selected
     range** (its start or its end) before the copy: the end of the user's drag, or the
     last Shift move that extended or shrank it. A drag or a Shift extension that scrolls
     the field while it runs therefore counts from its final range, and the host tracks
     the display of that final range in every frame from that change to the copy (item
     2). A caret move without Shift clears the selection.
  2. **Only text that was on screen.** From the moment the selection was made to the copy,
     the **whole selected range was displayed**: every selected character drawn inside the
     field's visible area, none clipped by the field, an ancestor, its frame or the screen,
     and none scrolled out of view; the field shown and **uncovered** as the visible-focus
     rule's items 1–4 say for focus (§7.7, TM L58); and the text drawn with its `color`'s
     alpha × the field's effective opacity **≥ 0.5**. A selection that was ever partly out
     of view, under a covering element, or drawn fainter in that time copies nothing. So
     `Ctrl+A`, `Ctrl+C` in a `Field` holding "ls" followed by 300 spaces and a command
     scrolled out of view copies nothing.
  3. **Invisible characters stripped.** The copied text has every character of general
     category Cc, Cf, Zl and Zp removed (bidi controls, U+200B–U+200D, U+2060, U+FEFF, tag
     characters, U+2028, U+2029), except LF and tab in a `multiline` field, which displays
     them as line breaks and tabs; in a single-line field LF and tab are removed too.

  Text whose colour merely matches the field's background is not detected; that is TM
  §8.4's stated residual (the user still sees the field's extent). Escape test: TM X52.
- **Notifications.** `TextChanged` (state) after the user's edits, at most one per frame with
  the latest text; also after the script's own `Set{text = …}`, in its next turn with
  `by = "script"`.

<a id="s7-9"></a>
### 7.9 World labels

A world label is a short text placed at a point of the world.

| Property | Type | Default | Meaning |
|---|---|---|---|
| `text` | string ≤ 256 bytes (TM L81) | `""` | One line; cut at the end with an ellipsis when wider than 512 UI units (domain, TM §0.1 rule 6). |
| `x`, `y`, `z` | number (region frame) | required | The point. |
| `region` | integer 1 … 255 (domain; 0 means "no region" in `coordinates.md` §4 and is refused) | the player's region at creation | The label is shown only while the player is in this region. |
| `offset_x`, `offset_y` | number, \|v\| ≤ 256 (domain) | 0, 0 | A screen offset in UI units after projection. |
| `max_distance` | number 1 … 500 (domain) | 100 | Hidden beyond this distance (metres) from the camera. |
| `font`, `color`, `background` | as §7.4 | | |
| `visible` | boolean | `true` | |

- **Placement** in step 8: the point is projected with this frame's camera; the label is
  hidden when the point is behind the camera, beyond `max_distance` or off screen. It is not
  depth-tested and is drawn below every ext frame (§7.1). Nodes persist: a frame in which the
  script is not ticked shows them unchanged (D17 §3).
- **Never catches the pointer** (§7.6).
- **Coverage.** A shown label is a composition entry (§7.10). Creating it shown, showing it
  (`visible = true`) and a `Set` that changes its displayed size (`text`, `font`) are
  checked against the bounds; a refused creation answers `nil, "limit"`, a refused `Set`
  answers `false` and changes nothing. Projection in step 8 never changes coverage.
- Methods: `Set`, `Get`, `Remove`, `Exists`, `Kind` and `On` (only `VisibilityChanged`).
  A label's `VisibilityChanged` reports changes of its `visible` property and of whether
  the host shows it: the script's own `visible` changes (`by = "script"`, next turn), a
  host change's hide (§7.10, `by = "host"`), and the coalesced state notifications that
  follow `Resumed` when the user resumes the script (§7.11, `by = "host"`, for example a
  label that no longer fits the coverage bound and stays hidden). Projection hiding in step 8 (behind the
  camera, beyond `max_distance`, off screen, another region) and a consent prompt (§7.1)
  are **not** notified **(1a rule)**.

<a id="s7-10"></a>
### 7.10 Limits and pools

| Limit | Value | When reached |
|---|---|---|
| Nodes (frames, controls, labels) | TM L30: 2,048 per script, of which at most 512 per `Frame` (in one ext window, its own node included; not a rate per time frame) and 256 world labels; 8,192 for all ext scripts, of which 1,024 world labels | creation answers `nil, "limit"` |
| Ext surfaces, composition entries, UI memory | TM L49–L51: 128 surfaces, 256 entries and a quarter of each memory pool per script; 512, 1,024 and 32/64/64 MiB for all, on top of UR's caps. Only a `Frame` takes a surface (TM L49); a world label takes none, and a shown frame, its badge and a shown label each take one entry (§7.1) | creation answers `nil, "limit"`; a show answers `false` |
| Composition coverage | TM L55, rule 7.1.3: **1.25 screens** per script, **2.0 screens** for all ext scripts | below |
| Text | TM L81: `Text` 16 KiB, `Field` `max_bytes` ≤ 4,096, `List` 1,024 rows × 256 bytes, labels and buttons 256 bytes, `placeholder` 256 bytes, `UI.MeasureText` 16 KiB | longer raises a Lua error |
| Notifications | the 1,024-entry queue and coalescing of §2.8 (TM L30) | dropped and counted |

**Composition coverage** (TM §7.1, the composition half of D17 open item 1; this section
states its script-visible form).

- **Entries.** In slice 1a the script's composition entries are its shown frames, its shown
  world labels and the badges of its shown frames (§7.7). No slice-1a node kind is composed
  as a nested surface (none scales, rotates or stretches its content; a `Scroll` clips
  inside its frame's surface); a later kind that is composed as one counts again, as TM rule
  7.1.2 says for nested entries.
- **Counting by size** (TM rule 7.1.2, L55). An entry of displayed width *w* and height
  *h*, in physical pixels (UI units × the UI scale), counts min(one screen,
  (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1)) pixels (0 when *w* or *h* is 0), divided by one screen,
  **unclipped**. The count depends only on the entry's size, never on its position or its
  sub-pixel phase, and it is never below the pixels the entry can cover at any position.
  So moving a frame or a label, by the script, by the user's drag or by step 8's
  projection, by whole or fractional pixels and at any UI scale (1.25 included), never
  changes coverage, and a frame created mostly off screen costs as much as one on screen.
  A frame's displayed size is its `width` and `height`; a label's is its drawn text box
  (its text cut to 512 UI units, §7.9, in its `font`); a **badge** counts its fixed box of
  §7.7, 20 × (16 + 8 × (*n* + 13)) UI units, never its glyphs, so its count does not
  depend on fonts (TM §8.3). `UI.Usage().area` reports the script's sum; for frames and
  badges it is deterministic, and it depends on the fonts (OPEN-6) only through shown
  labels.
- **Check points** (TM rule 7.1.5): a frame or label created shown, `Show`, `visible = true`
  on a frame or label, a change of a shown frame's `width` or `height` or of a shown label's
  displayed size, a shown frame gaining its badge (a `node:Add` of its first catching node
  or `Field`, or a `Set` that turns `catch` on), and every screen resize or UI-scale
  change. Position changes are not checked.
- **A script call** that would take the script past 1.25 screens or all ext scripts past 2.0
  is refused and changes nothing: `Show` returns `false` and the frame stays hidden; a
  creation answers `nil, "limit"`; a `node:Add` that would give a shown frame its badge
  answers `nil, "limit"` and adds nothing; a `Set` returns `false`. The script log says
  why.
- **A user resize** of a `resizable` frame stops at the largest size that keeps both bounds;
  a user drag is never limited.
- **A host change** (a screen resize, a UI-scale change) that puts a script or the total
  over its bound hides ext frames and labels, across all scripts, in **reverse order of
  their last show** (the most recently shown first) until every script and the total fit.
  Each hidden node's script gets `VisibilityChanged{visible = false, by = "host"}` in its
  next turn (the name TM rule 7.1.5 uses too), and the script log records it.
- **Frame budget** (TM rule 7.1.6, L56). The GPU time of ext composition at these bounds is
  part of **the 1.0 ms GPU part of the 1.6 ms "Ext overlays and composition" row** (the
  row is 0.6 ms of overlay encoding, from slice 1b, plus 1.0 ms of GPU time for overlays
  and composition together, measured as E9 at the reference display's native 3840 ×
  2160; slice 1a measures the composition part). If E9 measures more, the bounds shrink in
  proportion (TM OPEN O-1). While the row's funding is open (TM OPEN O-2) the interim ext
  ceiling of TM L12 applies.
- Escape test: TM X31, its **slice-1a part** (frames, world labels and badges: windows
  created off screen and moved on, user drags by fractional pixels at UI scale 1.25, a user
  resize at the bound, a labels-only script at a host change), with E9. X31's parts for
  nested surfaces and for scale and rotation changes belong to the slice that adds them
  (TM revision 4): slice 1a has none of them, so they cannot be set up here.

**Images** (TM §8.5). `file` names a file of the script's folder snapshot (D17 §2); a missing
file or a path outside the folder grammar raises. Decoding runs on an asset worker at the
lowest priority, as an **outstanding worker request** of the script (TM L27: 32 per script,
256 in all). A creation of an `Image` with a `file`, or a `Set` that changes its `file`,
that would pass that cap answers `nil, "busy"` (creation) or `false, "busy"` (`Set`) **at
the call** and creates or changes nothing; no decode is queued for later. A decode counts
toward L27 from the call until its worker job has **ended** (finished, or cancelled before
it started), even after its `Image` is removed or its `file` changed: removing the node, or
setting another `file`, frees no slot before then (TM §8.5, L27), and a removed node's
`Loaded` is never delivered (its handle is stale). A node reports only the decode of its
current `file`: the `Loaded` of a `file` that a later `Set` replaced is never delivered
either, and the node's next `Loaded` is that of its new `file` **(1a rule)**. So a loop
that creates and removes `Image`s every frame never has more than 32 of the script's
decodes queued or running (TM X26). **Joined requests (TM §8.5, L27).** Requests of one
script for the same file are joined to one decode job, but each request (each creation
of an `Image` with a `file`, and each `Set` that changes a `file`) counts toward L27
**once by itself, per request, not once per job**, from its call until its own `Loaded`
is delivered or discarded (§2.8) **and** the shared job has ended. Joining saves decode
work, never slots: with 32 requests outstanding for one file, a 33rd `Image` of that file
answers `nil, "busy"` although only one decode job exists. Before any pixel is decoded the header is
checked, an image over 4,096 pixels on an axis is refused (TM L48), and its decoded size is
charged to the script's share of ext UI memory (TM L51); `Loaded{ok = false, reason}`
reports `"too large"`, `"invalid"` (a broken or unsupported file) or `"limit"`. The formats
accepted are those of OPEN-5. A successful decode delivers `Loaded{ok = true}`. Escape
tests: TM X26 (run with a distinct file per `Image` and with one file for every `Image`,
the joined case), X33.

<a id="s7-11"></a>
### 7.11 Suspension, unload and the panic chord

- When a script is suspended (watchdog, panic chord, grant revocation, a locale other than
  "C"; §2.8), the host hides all its frames and labels at once; its focused field loses
  the focus and its frame is deactivated, and its ext contexts are deactivated with their
  holds released (§8.1). Nothing is notified while it is suspended (§2.8). When the user resumes it,
  frames and labels that were shown are shown again in their earlier show order, each
  subject to the coverage check of §7.10 (one that does not fit stays hidden), and the
  first turn carries `Resumed` (§4.1) followed by the coalesced state notifications with
  `by = "host"`.
- **How the user removes a window that catches the pointer (1a rule, stating D17 §3's
  routes).** Ext frames have **no host close control**: the host draws no close button on
  them, and the ownership badge is never a pointer target (§7.7). The host routes are
  exactly two, and both work whatever the script does: the **panic chord** (D17 §5.5),
  which suspends every ext script and so hides all their frames at once; and the
  **script manager**, where the user unloads the script or revokes its `ui` grant (§2.9,
  EM §9.7), either of which removes every frame of that script in the frame the action is
  applied. D17 §3's "by closing the window" is available only when the script offers a
  control of its own that hides the frame; nothing in this spec guarantees one. The grant
  dialog's `ui` text names both routes: "To remove them, press ⟨chord⟩ (stops every
  extended script) or unload the script in the script manager." (EM §6.1, the `ui` row,
  EM revision 4). The user documentation names them too.
- The suspension notice and every other reserved-layer surface are hit-transparent, so a
  notice over a plugin button never stops its click or hover (TM X50, slice-1a part).
- At unload every node is removed with the apartment. No geometry or visibility is kept for
  the next load (D17 §5.2: a script that wants its frame where the user left it saves the
  geometry in `Store`).

---

<a id="s8"></a>
## 8. `Input`

Capability: `input`. Group: O. Backends: Off (T from slice 3). Slice 1a.
`Ext.Use("Input", 1)`.

The manifest declares the script's actions and at most two contexts (D17 §2). The **host**
registers them at load as `ext.<id>.<name>` in the ext partition of the key registry and
unregisters them at unload; a script never registers anything (D17 §2). In this module a
script names its own actions and contexts by their **short** names (`<name>`).

<a id="s8-1"></a>
### 8.1 Contexts

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Input.Activate(context)` | `true` \| `nil, "busy"` | `B` | item-7 drain | `context` is a short name the manifest declares; another name raises. Rate: 4 activations or deactivations per script per second (TM L62), counted in **calls, not in applied changes** **(1a rule)**: every call that returns `true` takes one token from a bucket that holds 4 and refills at 4 per second, also a call that changes nothing (activating an active context) or whose change is later coalesced away; a refused call takes nothing. Changes of one frame are coalesced, only the last state of each context is applied, and all ext registry changes of one frame are applied as one recompilation (TM §8.18). Counts as one pending intent (TM L30: 256 per script per frame). |
| `Input.Deactivate(context)` | `true` \| `nil, "busy"` | `B` | item-7 drain | As above. Deactivating releases the context's holds (KM §3); a pending sequence is dropped only if it belongs to this context (D17 §5.5). |
| `Input.IsActive(context)` | boolean | `B` | read | The state the script last asked for, including a change not yet applied, **unless the host has deactivated the context since** (a suspension, below): then `false` until the script activates it again. |

- At load every context of the script is **inactive**; the script activates what it needs,
  typically in its top-level run (D17 §10).
- **Priority (1a rule, closing EM OPEN O-3).** Every ext context of every script gets the
  same priority, **−500**, inside KM's reserved range −1,000 … −1 (TM L60). Ext contexts are
  therefore ordered among themselves only by recency of activation (KM §3: among equal
  priorities the most recently activated comes first; re-activating moves a context to the
  top of its band), and no script can rank its context by declaration above another
  script's.
- Ext contexts sit in the game band, are never `opaque`, `opaque_keys`, `text` or
  pass-through, and resolve only **after both passes over every non-ext context**, at
  presses and at releases, and never for a key that an `opaque` context or an
  `opaque_keys` band end stops (D17 §5.5; TM X34–X36). So a script's binding fires only for
  an input that no built-in, user or plugin binding takes. Holds of ext actions come from
  their own pool (TM §8.16, L61); a binding whose hold would pass it does not start (TM X41).
- An intent made before a suspension, unload or backend switch is discarded and never applied;
  the applying step re-checks the apartment, the session generation, the backend class and the
  grant (D17 §4).
- **Suspension and input** (TM §8.23; every cause of §2.8, the panic chord included). In the
  frame of the suspension the host deactivates **every** ext context of the script and
  releases their holds (TM L61), and discards its pending context intents. While the
  script is suspended its bindings consume no input and record no action edges and no
  gestures: a press of one of its strokes resolves as if the binding did not exist, so it
  reaches whatever lies after the ext partition (usually nothing). The panic chord is
  resolved before every ext binding (TM L64), so no ext context can hold it. On resume the
  contexts are **not** restored: they stay inactive, `Input.IsActive` answers `false` for
  each, and the script activates them again itself, within the rate above (for example in
  its `Resumed` handler). Edges lost meanwhile are not counted (nothing accrues); edges
  dropped before the suspension count under `Resumed.counts.action` (§4.1).

<a id="s8-2"></a>
### 8.2 The script's own actions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Input.On(action, fn)` | nothing | `B` | at once | Sets or clears the handler of a declared action (§2.8, form 1: `Input`'s notification names are the script's declared short action names; any other name raises). `fn(ev)` with `ev = {action, kind, value, dropped}`: `kind` `"start"` or `"end"`; `value` the action's value at that edge (1 or 0 for a button; for an axis the value after the edge). Edges are recorded in step 1 in the order they happened, at most 16 per action per frame (TM L91; more are counted in `dropped`), and delivered **same-frame 6b**. A `"start"` is a **gesture**: its turn is a gesture window (§2.1, §7.7). |
| `Input.State(action)` | table | `B` + `e(10)` | read | This frame's snapshot (KM §7.1): `{pressed, presses, releases, value, delta}` (KM §2). |
| `Input.Actions()` | array of tables | `B` + `e(8n)` + `b(bytes)` | read | One `{name, kind, context}` per declared action (*n* of them), in manifest order; `kind` `"button"` or `"axis"`. |
| `Input.Bindings(action)` | array of strings | `B` + `e(2n)` + `b(bytes)` | read | The *n* binding texts (KM §5) now in effect for the action: the user's from the ext keymap document, or the suggested default when registration did not withhold it. So a script can say "press F7". Empty when unbound. |

- **Start and end.** For a button action, `"start"` is a transition to pressed and `"end"` a
  transition to released (KM §7.3). For an axis action, the same transitions of `pressed`
  (a digital binding held, or \|value\| ≥ 0.5, KM §2). Release-class bindings (`on =
  "release"` or `"tap"`, IB D3) give a start and an end in the same frame.
- **Never offered, never seen by plugins.** Ext actions carry no command name, so they are
  never offered to the UI and never reach a plugin's key broadcast (D17 §5.5).

<a id="s8-3"></a>
### 8.3 Standard actions (read only)

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Input.Standard(name)` | table | `B` + `e(10)` | read | This frame's snapshot of one standard action, as `Input.State`. `name` must be on the list below; any other name raises. |

The readable list is **closed in major 1** (additions only within the major, D17 §2)
**(1a rule)**: `move_forward`, `strafe`, `turn`, `jump`, `walk`, `autorun`, `mouselook`
(KM §8, `movement` and `camera`), and `fly_vertical`, `fly_boost`, `fly_slow` (KM §8,
`fly`).

Each name is **one action** of the key registry (KM §2: actions are registered once by
name; contexts only hold bindings to them). `mouselook` is bound in both `camera` and
`movement` (KM §8), and its snapshot is that of the one action: it is pressed while any of
its holds, from either context, exists (KM §7.3), and its edges count transitions of that
combined state. Reading it is read only; it does not start the pointer lock and is not
drivable (TM §7.7).

Deliberately absent: `look_x`, `look_y`, `zoom` and `fly_speed` (their deltas are the
user's raw mouse motion and wheel everywhere on screen, including over plugin windows);
the `pointer` context's actions (they would report clicks on plugin windows); `global`
actions (`open_chat` and `menu` share their names with text-entry bindings, so they would
report Enter and Escape while the user types); every UI-band action; plugin actions; and
other scripts' actions (D17 §5.6). There is no pointer position and no modifier state
in this module: a script learns about the pointer only over its own catching nodes (§7.6).

---

<a id="s9"></a>
## 9. `World`

Capability: `world.read`. Group: O. Backends: Off (T from slice 3). Slice 1a.
`Ext.Use("World", 1)`.

Read-only queries of `tessera-world` and `tessera-physics`, answered **only from resident
data** (D17 §3, §8). Positions are in the region frame of the player's current region
(§2.1); a point whose landblock is not resident answers `nil, "not loaded"`, never a guess.

<a id="s9-1"></a>
### 9.1 Functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `World.Here()` | table \| `nil, "not loaded"` | `B` + `e(14)` | read | `{region, x, y, z, yaw, heading, mode}`: the player character's position and facing (§2.10) as of step 3 of this frame, `mode = "walk"`; in fly mode without a character, the eye of the user's own free-fly view of this frame (never a leased view), `mode = "fly"`, the same point EA1b §4.6 uses as its reference point *P* and EA1b §4.2 reports under `world.read` (OPEN-7, closed). `nil, "not loaded"` before the player is placed. |
| `World.Landblock(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` + `e(10)` | read | `{region, bx, by, cell, indoors}`: the landblock (X, Y) and, from the resident collision data, the interior cell index containing the point or `nil` outdoors (`collision.md` §5.8.1); `indoors` is `cell ~= nil`. Counts as a point query. |
| `World.Area(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` + `e(6)` + `b(bytes)` | read | `{area, territory, region}`: the names of the area at the point and of its parents, each a string or `nil`, in the client's language, from the area lookup over resident data (ARCH §9.2, "Lookups"). |
| `World.MapCoords(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` + `e(6)` + `b(bytes)` | read | `{north, east, text}`: the point's map coordinates as Tessera's own map display computes them. **Contract:** `north` grows toward +Y and `east` toward +X, both are finite numbers, and `text` is the string Tessera's client shows for them. Their formula, rounding and the format of `text` are **not** part of the ext contract (§9.2, OPEN-3); scripts display `text` and must not parse it. |
| `World.LocText()` | string \| `nil, "not loaded"` | `B` + `b(bytes)` | read | The position text that Tessera's own `/loc` command reports for the player now, at most 256 bytes (TM L91). Same contract limit as `MapCoords.text`: display it, do not parse it. |
| `World.Height(x, y)` | number \| `nil, "not loaded"` | `B` + `Q` | read | The terrain surface's z at (x, y), from the resident terrain of the point's landblock (`tessera-terrain` height query). Ignores objects and interiors. |
| `World.Water(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` + `e(4)` | read | `{surface, under}`: the z of the water surface above or at (x, y) or `nil` when there is none, and whether (x, y, z) is under water (ARCH §9.2, "Lookups"; `water.md`). |
| `World.Ray(x1, y1, z1, x2, y2, z2 [, opts])` | table \| `false` \| `nil, reason` | `B` + 2,000 + 2,000 per landblock the segment's bounding box overlaps (at most 25, TM L91), + `e(16)` + `b(bytes)` for a hit | read | Casts a segment against the **installed collision sets** (ARCH §20.3, "Collision-set residency"). Its length must be ≤ 500 m (TM L69; longer raises). `opts.water` (boolean, `false`) includes water surfaces. Returns the nearest hit `{x, y, z, nx, ny, nz, distance, what}` with `what` one of `"terrain"`, `"object"`, `"cell"`, `"water"`; `false` when nothing is hit; `nil, "not loaded"` when any landblock the segment's bounding box overlaps has no installed set; `nil, "busy"` past the ray caps. |
| `World.Clock()` | table | `B` + `e(14)` | read | The game clock Tessera's client uses in this frame: `{seconds, time_of_day, day, year, day_of_year, month, night}`. Only the contract of §9.2 is promised. |
| `World.Sky()` | table | `B` + `e(18)` + `b(bytes)` | read | The sky state Tessera's client computed for this frame: `{sun_x, sun_y, sun_z, sun_color, ambient_color, fog_near, fog_far, fog_color, day}`. Only the contract of §9.2 is promised. A field Tessera does not compute in a frame (no sky loaded) is `nil`. |

- **Facing.** `yaw` and `heading` follow §2.10 (yaw in radians counter-clockwise from +X;
  heading the compass bearing in degrees). They are Tessera's own definitions; `heading`
  may differ from the heading inside `LocText()` by rounding and for tilted rotations.
- **Caps** (TM L69, L70): 64 rays per script per frame and 256 for all scripts; 256 point
  queries per script per frame (`Landblock`, `Area`, `MapCoords`, `Height`, `Water`). Past a
  cap: `nil, "busy"` until the next frame. `Here`, `LocText`, `Clock` and `Sky` are not
  capped.
- **Region changes.** After the player changes region, every position argument refers to the
  new region; positions the script kept from the old one simply name other places.
- Nothing in this module waits, triggers streaming, decodes or reads files (D17 §8).

<a id="s9-2"></a>
### 9.2 Contract limits of values Tessera computes (LEGAL §18.3 rule 1)

`MapCoords`, `LocText`, `Clock` and `Sky` hand the script values that Tessera's client
computes with rules recorded partly from the original client (`coordinates.md` §8–§9;
`environment.md` §11, "BIN unless stated"; `sound.md` §10.3.5). None of those rules becomes
part of the ext contract. The contract is only **(1a rule)**:

| Value | Promised | Not promised |
|---|---|---|
| `MapCoords.north`, `.east` | finite numbers; `north` grows toward +Y and `east` toward +X | the formula, the origin, the scale, the rounding |
| `MapCoords.text`, `LocText()` | the string Tessera's client shows for that point or for `/loc`, valid UTF-8, at most 256 bytes | its format; scripts display it and must not parse it |
| `Clock.seconds` | finite game seconds, ≥ 0; within one session they do not decrease unless something sets the clock (slice 1b's `Sandbox`) | the length of a game day or year, the epoch |
| `Clock.time_of_day` | a number in [0, 1), the fraction of the current game day | where dawn, noon or dusk fall |
| `Clock.day`, `.year`, `.day_of_year` | integers ≥ 0 that Tessera's client uses for its calendar | the calendar's rules, the days per year |
| `Clock.month` | an integer ≥ 0, or `nil` when Tessera's client has no month for that day | the month lengths, the month names |
| `Clock.night` | a boolean: whether Tessera's client treats this frame as night | the night rule and its thresholds |
| `Sky.sun_x`, `.sun_y`, `.sun_z` | a unit vector (region frame) toward the sun as drawn this frame | the sun's path |
| `Sky.sun_color`, `.ambient_color`, `.fog_color` | `#RRGGBB` strings (sRGB) as used this frame | the blending, the keyframes |
| `Sky.fog_near`, `.fog_far` | finite distances in metres, 0 ≤ `fog_near` ≤ `fog_far` | how they are chosen |
| `Sky.day` | the DID of the day description Tessera uses this frame, or `nil` | which description is chosen and when it changes |

A script may display these values, compare them with each other in the same frame, and
react to their changes; it must not depend on any figure in the right column. The values
may change between Tessera releases without a major version change of `World`. The
re-review of revision 3 confirmed that this promise-only contract satisfies D17 §12 and
LEGAL §18.3 rule 1 for all four functions (OPEN-3, closed); OPEN-3 keeps the fallback in
case the owner decides otherwise.

---

<a id="s10"></a>
## 10. `Entity`

Capability: `entity.read`. Group: O. Backends: Off (T from slice 3). Slice 1a.
`Ext.Use("Entity", 1)`.

Reads of the client's entity store (`tessera-entity`) through generational handles (§2.7),
batched queries, and filtered spawn and despawn subscriptions (D17 §3). The grant dialog
warns about `entity.read` with `ui` (D17 §5.2).

<a id="s10-1"></a>
### 10.1 Kinds and fields

| Kind | What it is |
|---|---|
| `"self"` | The user's own character. |
| `"character"` | Another entity the session reports (networked), other than the user's character. In slice 1a every such entity is a `"character"`; a finer split comes with the session layer of M7 (OPEN-8). |
| `"static"` | A streamed static of a resident landblock, including procedurally generated scenery objects placed as entities (ARCH §9.2). |

Client-local entities of a later slice never appear (D17 §3, `Entity.Local`).

| Field | Type | Meaning |
|---|---|---|
| `kind` | string | As above. |
| `name` | string or `nil` | The name the session gives the entity; `nil` for statics and unnamed entities. |
| `x`, `y`, `z` | number | Position, region frame, as of step 2–3 of this frame. |
| `yaw` | number | Facing in radians counter-clockwise from +X (§2.10), of the entity's model +Y axis. |
| `heading` | number | The same facing as a compass bearing in degrees (§2.10). |
| `rotation` | table `{w, x, y, z}` | The entity's rotation as a unit quaternion, scalar first, model frame to region frame (§2.10). |
| `setup` | DID or `nil` | The setup the entity is drawn from, when it has one. |
| `scale` | number | Its uniform scale (1 when it has none). |
| `drawn` | boolean | Whether it has a render instance now. |
| `landblock` | table `{bx, by}` or `nil` | For statics, the landblock it belongs to. |

<a id="s10-2"></a>
### 10.2 Functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Entity.Self()` | entity \| `nil` | `B` + `h(1)` | read | The user's character, or `nil` when there is none (before placement, free fly without a character). |
| `Entity.Find(filter)` | array of entities, truncated \| `nil, "busy"` | `B` + `e(n)` for the walked filter + 20 per candidate + (60 + `e(2)` + `h(1)`) per result (60 for the distance and the sort); the candidates are reserved from the counts known at the call's start and refunded | read | Batched query. `filter = {kinds, near, box, limit}`: `kinds` an array of kind names (default all three); exactly one of `near = {x, y, z, radius}` (radius 0 … 500, TM L91) or `box = {x1, y1, z1, x2, y2, z2}` (each side \|x2 − x1\|, \|y2 − y1\|, \|z2 − z1\| ≤ 1,000, TM L91; the two corners may be given in any order: the box is, on each axis, the closed interval between the two values, so x1 > x2 names the same box as x2 > x1, and a side of 0 is allowed **(1a rule)**), or neither (every entity of those kinds); `limit` 1 … 512 (default 512). Results are sorted by distance from `near`'s point, or from the box's centre, or, with neither, from the player's character; ties, and every result when there is neither an area nor a placed character, in the stable order of the entity store, which deterministic mode repeats **(1a rule)**. Second result `truncated` is `true` when more entities matched than were returned (D17 §3, "API rules"). Caps (TM L71): 8 queries per script per frame, 32 for all scripts. **Candidates (1a rule):** the resident entities **of the requested kinds** in the landblocks the area overlaps (in every resident landblock without an area); an entity of another kind is never a candidate and costs nothing. A `"self"` or `"character"` entity belongs to the landblock that holds its position in this frame, a `"static"` to its own landblock. So a query whose reservation exceeds the call's remaining units raises before it starts (§2.5): narrow it with `near` or `box`, or leave out `"static"`, usually by far the most numerous kind. |
| `Entity.Read(list, fields)` | array of tables | `B` + `e(2 · #fields)` for walking `fields`, then per entry `e(2)` (its array slot) + `e(2 · #fields)` + `e(8)` when `rotation` is requested + `e(4)` when `landblock` is requested + `b(bytes)`, by §2.5's counting rule (a nested table's keys and values count again; a fixed-shape field is charged even when it is `nil`); the whole is reserved from `#list` at the call and refunded for stale entries | read | Batched read: `list` an array of at most 512 entity handles (TM L91), `fields` an array of field names (§10.1). Entry *i* is a table of the requested fields of `list[i]`, or `false` when that handle is stale. An unknown field name raises. |
| `e:Get(field)` | the field's value | `B` + `b(bytes)`, plus `e(8)` for `rotation` and `e(4)` for `landblock` (fixed shapes of 4 and 2 fields, §2.5, charged even when the answer is `nil`) | read | One field of one entity; raises on a stale handle. An unknown field name raises. |
| `e:Position()` | x, y, z | `B` | read | Raises on a stale handle. |
| `e:Exists()` | boolean | `B` | read | |
| `e:Kind()` | string | `B` | read | Works on a stale handle. |
| `Entity.Subscribe(filter, fn)` | subscription \| `nil, "limit"` | `B` + `h(1)` + `e(n)` for the walked filter | records of frames **after** this one (§10.3) | `filter = {kinds, near, around_self, box}`: `kinds` as above; at most one of `near = {x, y, z, radius}`, `around_self = radius` (radius 0 … 500, as `near`'s, TM L91; outside it raises; measured from the player's character in the frame of each record; a record of a frame without a placed character matches no `around_self` subscription), `box` (as `Find`'s: each side ≤ 1,000, TM L91, corners in any order). Both functions check these fields in the walk of `filter` (§2.4 step 6). It matches only records of frames after the frame in which it was created, however far the script's cursor lags (TM rule 7.5.2). `fn(sub, batch)` per §10.3 (a per-request callback, §2.8). At most 16 subscriptions per script, 64 for all scripts (TM L30). |
| `sub:Cancel()` | nothing | `B` | at once | Stale afterwards; undelivered events are discarded. |
| `sub:Exists()`, `sub:Kind()` | boolean, `"Subscription"` | `B` | read | |

<a id="s10-3"></a>
### 10.3 Subscriptions and the filter call (TM §7.5)

This section states TM §7.5 (D17 open item 5) in script-visible form.

- **Recording** (TM rule 7.5.1, L25, L31). Steps 2–5 record each frame's spawns and
  despawns **once**, in one list for all consumers, each record holding the entity, its
  kind, its position at that moment and the player's position in that frame. A frame's list
  holds at most **4,096 records**; records past that are not kept, and the frame carries an
  **overflow count**. Lists are retained for at most **60 frames**, so the shared store holds
  at most 245,760 records and 15 MiB, a host-wide bound outside the scripts' caps (TM
  §8.10). Recording is native bookkeeping with a fixed cost per record, outside every
  script's call.
- **The filter call** (TM rule 7.5.2, 7.5.3). Each script with at least one subscription has
  one **cursor** (frame, index) into the retained lists, and its turn **starts** with a
  **filter call** (after the `Resumed` call in the first turn after a resume, §2.1, §4.1):
  a call like any other (TM L3, L4), counted against the 2 ms cap and
  split like any turn (§2.6). It filters records from the cursor in batches of **64**,
  charging before each batch 64 × (the script's number of subscriptions) × `c_f` units
  (`c_f` the declared cost of one record-filter pair, TM OPEN O-4). It stops, keeping the
  cursor, when the next batch's charge would pass the call's remaining units or deadline,
  when the script's events **delivered** in this frame reach **1,024** (TM L31, rule 7.5.2;
  the events of the batches this frame's filter calls append count as delivered in this
  frame), or at the end of the retained records. No filter work runs outside this call:
  not in steps 2–5 and not in 6c. A record older than 60 frames when the cursor reaches it
  is skipped and counted as dropped.
- **The filter call is never an abort** (TM rule 7.5.2a). A filter call that ends by its
  own stopping rule is not an abort and never counts toward the three aborts of TM L18;
  its time is **excluded** from the rolling mean, the throttle and the suspension of TM
  L17, because its amount is set by the world's record rate, not by the script. It still
  counts against the 2 ms cap (TM L5) and the frame's ext time (TM L12), and the profiler
  shows it apart from the script's own time. A throttled script's turns, its filter call
  included, run at half rate; its cursor then lags, and records older than 60 frames are
  dropped and counted, never filtered late.
- **What matches.** A spawn record matches a subscription when the record's frame is
  **later than the frame in which the subscription was created** (TM rule 7.5.2: a
  subscription made while the cursor lags never matches an older record), its kind is in
  `kinds`, and its position at spawn is inside the area (if any; `around_self` measured
  from the player's character's position in the record's frame, and never matched by a
  record of a frame without a placed character); a despawn record the same with the
  position at despawn. Moving into or out of an area is not an event; scripts query for
  that.
- **Matches become notifications.** At the end of the filter call, each subscription that
  has matches, or a count to report, gets **one** batch notification appended to the queue
  (§2.8; it is part of the turn's delivery set, so it is delivered later in the same turn if
  the cap allows). Each batch notification takes one entry of the 1,024-entry queue; while
  the queue is full, the batch is dropped and its events are counted.
- **The batch.** `fn(sub, batch)` with `batch = {spawned, despawned, dropped, unfiltered}`:
  arrays of handles in record order (converted inside the delivery call, `h(n + 1)` and `e(8
  + 2n)` charged first, *n* the handles of both arrays, §2.5); `dropped`, the events lost
  since this subscription's last delivery (queue cap, records older than 60 frames); and
  `unfiltered`, the overflow counts of every frame the cursor passed since that delivery
  (the script cannot know whether those records would have matched, so every subscription of
  the script reports them). Handles in `despawned` are stale on delivery; they compare equal
  to handles the script still holds (§2.7), so it can remove them from its own tables.
- **Suspension** (TM rule 7.5.4). A suspended script records nothing. When the user resumes
  it, its cursor jumps to the newest frame, and the records it skipped are counted once, as
  `counts.entity` of its `Resumed` notification (§4.1), not in a batch.
- **Worked example** (TM §7.5). One frame integrates a landblock with 10,000 statics: 4,096
  records are kept, overflow 5,904. A script with 16 subscriptions and `c_f` = 10 pays
  10,240 units per batch of 64, so one 800,000-unit call filters about 78 batches (4,992
  records), the whole frame; a script with 4 subscriptions pays 2,560 units per batch and
  filters the frame in 64 batches (163,840 units). Each of its subscriptions' next batches
  carries `unfiltered = 5,904`. Escape test: TM X19, run with 4 scripts × 16 subscriptions
  and with 16 scripts × 4, the 64 that TM L30 allows for all scripts together.

---

<a id="s11"></a>
## 11. Limits of slice 1a (summary)

Every number here is TM's (TM §10, the single home of the limits; TM §0.1 rule 1). The
values are repeated for the reader; if one differs from TM §10, TM wins. The "When reached"
column gives what the script sees.

| Resource | TM | Value | When reached |
|---|---|---|---|
| Memory per script | L23 | 32 MiB (manifest may ask up to 64): Lua heap plus host objects (nodes, labels, badges' bookkeeping, timers, subscriptions, queued notifications, pending intents, buffered log records, the handle map, `tostring` numbers), the **kept snapshot data files** (TM §8.10), the `Store` file index (in full for every apartment that has the store open, TM rule 7.6.1), the pending `Store` entries and flush bytes it wrote, and outstanding requests with their reservations | memory error (catchable); a reservation answers `nil, "out of memory"` |
| Memory sum | L24 | 512 MiB of loaded caps + the 64 MiB reload allowance; a closing apartment's cap stays until its final flush completes | the load is refused |
| Largest single allocation | L22 | 4 MiB | memory error |
| Per call | L3, L4 | 4 ms and 800,000 units | abort (§2.4) |
| Ext time started per frame | L5 | 2 ms in 6a–6b for all scripts | rest of the turn waits (§2.6) |
| Heavy-frame gate, floor, switch timeout | L8–L11 | 22 ms from the frame's work start; floor after 15 gated frames or 250 ms from the start of the last frame in which ext ran, each floor frame resetting both; switch timeout 2 s (120 frames deterministic) | §2.6 |
| Worst ext time per frame | L12 | 13 ms; 12 ms in slice 1a while TM OPEN O-2 is open | fallback lowers caps |
| Rolling mean per script | L17 | ≤ 1 ms per frame over 1 s | throttled to half rate; suspended after 5 s over within 10 s |
| Aborts | L18 | 3 in 10 s | suspended |
| Queued notifications, timers, subscriptions, nodes, pending intents | L30 | queue 1,024 per script (state notifications coalesced); timers 64; subscriptions 16 per script, 64 for all; nodes 2,048 per script (512 per `Frame`, that is per ext window, not per time frame; 256 labels), 8,192 for all (1,024 labels); intents 256 per script per frame; reserved queue room for the results of outstanding requests (L27) and for due timers, which are never dropped | dropped and counted; `nil, "limit"`; `nil, "busy"` |
| Commands | L68 | 32 per script; names 1–32 bytes | `nil, "limit"`; a bad name raises |
| Actions, contexts | L60 | 64 actions and 2 contexts per script; 256 and 16 for all; priority −500 (§8.1) | the load is refused |
| Context changes | L62 | 4 per script per second, coalesced per frame | `nil, "busy"` |
| Ext holds | L61 | 64 for all ext scripts | the hold does not start |
| Ext UI pools | L49–L51 | 128 surfaces, 256 entries, a quarter of each memory pool per script; 512, 1,024, 32/64/64 MiB for all | `nil, "limit"` or `false` |
| Composition coverage | L55 | 1.25 screens per script, 2.0 for all; per entry min(one screen, (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1) of its unclipped displayed rectangle); badges (fixed box) and labels count | `false` / `nil, "limit"`; a user resize stops; a host change hides (§7.10) |
| Ext overlay and composition row | L56 | the 1.0 ms GPU part of the 1.6 ms row (0.6 ms overlay encoding from slice 1b + 1.0 ms GPU at 3840 × 2160, E9; slice 1a measures the composition part) | the bounds shrink |
| Solid press, focus box, case 1 | L57–L59 | alpha × effective opacity ≥ 0.5 on the topmost painted node; the visible-focus rule, the script's own higher content and the badge's placement included; case 1's three conditions and gesture-window focus | not a gesture; `false` |
| Gesture window | L93 | the delivering turn, closed earlier by the user's next input outside the script's frames and actions or 1 s after the delivering call started (60 frames deterministic); opened only by a solid press's `Pressed`, an action start or an `/ext:` line; a turn split twice under permanent gating can outlast it (a known limit that fails safe, §2.6) | requests treated as outside a gesture window |
| UI text | L81 | `Text` 16 KiB, `Field` ≤ 4,096 bytes, `List` 1,024 × 256 bytes, labels, buttons, `placeholder` 256 bytes | Lua error |
| Images | L48 | ≤ 4,096 px per axis, charged before decode | `Loaded{ok = false, reason = "too large"}` |
| Outstanding worker requests | L27 | 32 per script, 256 for all; "per script" is per (kind, id), shared by both apartments of a hot reload. Counted from the call until the result is delivered or discarded **and** any worker job it started has ended: `Store` reads (pending reads included), every `Store.Set` or `Store.Delete` with a `done` until that `done` is delivered or discarded (a replaced value's included), and image decodes (each request once, joined requests for one file included, until its own `Loaded` is delivered or discarded and the job ends, also after the `Image` is removed) | `nil, "busy"` (`Get`, `Set` or `Delete` with a `done`, image creation); `false, "busy"` (image `Set`) |
| Result reservations | L28 | ρ × *n* + ε × min(*n*, 8,192) for *n* stored bytes (the pending entry or the file index's exact size), ε per element as §2.5 counts them (TM §8.14) | `nil, "out of memory"` |
| `Store` values | L33–L35, L39 | 32 levels, 8,192 elements, 1 MiB | `nil, "too large"` |
| `Store` space | L40, L41 | 1,024 keys and 16 MiB per script; 16,384 keys and 256 MiB for all; temporary files and pending entries count; a pending deletion adds no key and no byte | `nil, "full"` (`Set` only) |
| `Store` reads | L42 | 64 reads and 4 MiB per second per script; 16 MiB per second for all | `nil, "busy"` |
| `Store` writes, keys | L43 | 64 newly dirty keys per second per script (10 s bucket); one flush per script per second | `nil, "busy"` |
| `Store` writes, bytes | L44 | 32 KiB/s with a 10 MiB bucket per script; 128 KiB/s with a 40 MiB bucket for all; debited at the call, in full when a call makes a clean key dirty (no entry, or an entry a flush has taken), growth only over an untaken value (TM OPEN O-3) | `nil, "busy"` at the call |
| Script log | L45 | 64 KiB per second per script and 128 KiB per second for all scripts (TM OPEN O-3); 1 MiB file rotated once; record ≤ 4,096 bytes | dropped and counted; cut with `…[cut]` |
| `World` rays | L69 | 64 per script per frame, 256 for all; ≤ 500 m | `nil, "busy"`; longer raises |
| `World` point queries | L70 | 256 per script per frame | `nil, "busy"` |
| `Entity` queries | L71 | 8 per script per frame, 32 for all; ≤ 512 results | `nil, "busy"`; `truncated` |
| Entity events | L31 | 1,024 per script per frame; 4,096 records per frame; 60 frames retained; filter batch 64 | dropped, `unfiltered` and counted |
| Retained entity records | L25 | ≤ 245,760 records, ≤ 15 MiB host-wide | overflow counted |
| Per-call and per-object caps of this spec | L91 | command arguments 1,024 bytes; `help` 96 bytes; 16 action edges per action per frame; `Entity.Read` 512 handles; a `near` or `around_self` radius 500 m and a `box` side 1,000 m; a ray over 25 landblocks; `LocText` 256 bytes | cut, Lua error, or dropped and counted, as each section states |

---

<a id="s12"></a>
## 12. Not offered by these modules

D17 §5.6 and TM §9.3 list what no module offers. For the modules of this slice that means
in particular:

- no pointer position, no raw keys, no modifier state, no mouse motion outside the
  script's own catching nodes (§7.6, §8.3);
- no clipboard function; the user's own copy, cut and paste in a focused field only (§7.8);
- no reading of plugin windows, native panels, other scripts' frames, other scripts'
  actions or stores, or plugin actions (§8.3);
- no chat line from any function, report or command (§4.3, §6);
- no path beyond the script's folder and its own store; `Core.Script()` and every message
  use relative names (§4.4);
- no world or entity data that is not resident, and no request that makes it resident
  (§9, §10);
- no compiling of strings: `Store` values, command arguments and field text are data only
  (D17 §1.2).

---

<a id="s13"></a>
## 13. Worked examples (prose)

These follow the three reference scripts of the M5.5 exit criteria (D17, "Roadmap placement
proposal"). They are described in words; the reference scripts themselves are original CC0
works written in M5.5.

<a id="s13-1"></a>
### 13.1 HUD panel with coordinates and compass

Manifest: capabilities `ui` and `world.read`; `[modules]` `Core = 1`, `UI = 1`, `World = 1`
(§3); no actions.

| Frame | What happens |
|---|---|
| *L* (load) | 6a runs the top-level run. It takes `Core`, `UI` and `World` with `Ext.Use`; creates a frame of 220 × 56 units at the top-left, `movable = true`, with a `Group` (`layout = "column"`) holding two `Text` nodes; calls `frame:Show()`, which returns `true` (coverage about 0.0075 screens at 1920 × 1080 and UI scale 1, counted by size, §7.10: the frame's (220 + 1) × (56 + 1) = 12,597 pixels plus its badge "hud · ext script", whose fixed box of 20 × (16 + 8 × (3 + 13)) = 20 × 144 units counts (144 + 1) × (20 + 1) = 3,045, together 15,642 of 2,073,600). It registers `Core.Every(0.25, …)`. It has no `VisibilityChanged` handler, so nothing is queued for it (§2.8). Step 8 paints the frame and its badge "hud · ext script". |
| *L* + 1 | No timer is due yet; the turn is empty. |
| *L* + 15 (0.25 s at 60 fps) | The timer call runs: `World.Here()` returns `{region = 1, x = 20,240.0, y = 22,420.0, z = 312.5, yaw = 3.1364, heading = 270.3, mode = "walk"}`; `World.MapCoords(x, y, z)` returns a table whose `text` the script puts in the first `Text`; it formats `heading` as "W" (the nearest of eight compass points) in the second. Two `Set` calls change text; they paint in this frame's step 8. |
| any frame while a plugin window overlaps the frame | Nothing changes for the script; the plugin window is drawn above it and takes the pointer there (§7.1). |
| a frame where the player stands in a landblock not yet streamed | `World.MapCoords` answers `nil, "not loaded"`; the script keeps the old text. |

<a id="s13-2"></a>
### 13.2 Nearest-entity tracker

Manifest: `ui`, `world.read`, `entity.read`, with `[modules]` `Core = 1`, `UI = 1`,
`World = 1` and `Entity = 1` (§3: `Core` must be declared like any module, or
`Ext.Use("Core", 1)` answers `nil, "not declared"`). The grant dialog shows the
`entity.read` + `ui` warning (D17 §5.2).

- The top-level run creates a frame with a `List` and one `WorldLabel` (hidden), subscribes
  with `Entity.Subscribe({kinds = {"character"}, around_self = 60}, fn)`, and sets
  `Core.Every(0.5, refresh)`.
- `refresh` calls `Entity.Find({kinds = {"character"}, near = {x, y, z, 60}, limit = 10})`
  with the position from `World.Here()`. Say it returns 4 handles and `truncated = false`; the
  cost is `B` + `e(16)` for the filter (its 3 fields, the 1-entry `kinds` and the 4-entry
  `near`) + 20 × (the `"character"` candidates in the overlapping landblocks) + 4 × 150
  units. It then
  calls `Entity.Read(list, {"name", "x", "y", "z"})` once, builds 4 row strings with the
  distances and sets `rows`. It moves the label to the nearest entity's position and shows
  it.
- When a character despawns, the next turn's filter call matches the record and queues a
  batch, delivered later in that turn as `{spawned = {}, despawned = {h}, dropped = 0,
  unfiltered = 0}`; the script removes `h` from its own table
  (it compares equal to the handle it kept). Calling `h:Position()` would raise; `h:Exists()`
  is `false`.
- If 30 queries were attempted in one frame, the 9th would answer `nil, "busy"`.
- When the user clicks a row, `SelectionChanged{index = 2, by = "user"}` arrives in the same
  frame's 6b. The `List`'s host-drawn face makes the press solid (§7.6), so the click
  activates the script's frame; the plugin window that was active receives its deactivation
  in step 1, as plugin time, as for a click on a native panel (D17 §7).

<a id="s13-3"></a>
### 13.3 Key-bound note-taker

Manifest: `ui` and `input`; `[modules]` `Core`, `Store`, `Commands`, `UI` and `Input`, each
`1`; one context `main`, one button action `toggle` with suggested default `F7`; settings:
none.

| Frame | What happens |
|---|---|
| *L* | The top-level run calls `Input.Activate("main")` (applied at this frame's item-7 drain), `Input.On("toggle", onToggle)`, `Commands.Register("add", onAdd, "add a line to the notes")`, builds a hidden frame with a multi-line `Field`, and calls `Store.Get("notes", onNotes)` (one outstanding request; `true`). |
| *L* + 1 or later (worker) | The read's result arrives: `onNotes("…previous text…")` runs inside a metered delivery call, the conversion charged first; the script sets the field's text. |
| *P* (the user presses F7 while walking, no window active) | Step 1 resolves `ext.notes.toggle` after every non-ext context declined F7. 6b delivers `onToggle({action = "toggle", kind = "start", value = 1, dropped = 0})`; this turn is a gesture window. Inside that call the script calls `frame:Show()` (`true`) and `field:Focus()`: case 2 holds (gesture window, no window active), the field is visible and uncovered, so it returns `true`. (Had the script done this in its `Tick` of the same turn, or had the 2 ms cap split the turn over two frames, the result would be the same, TM X89.) The field pushes `text_entry`; from the next raw event on, W types "w" instead of walking. The host draws the caret, the focus mark and the badge "notes · ext script" in the focus colour. |
| *P* + n | The user types; each frame with edits delivers one `TextChanged` (state). The script calls `Store.Set("notes", text)`; the first call after a flush has taken the pending value (even while that flush is still writing) makes the key dirty again: one key token and the value's **full** counted size from the byte buckets, debited at the call. Later calls before the next flush takes the value replace an untaken value and pay only growth. The flush writes the file within a second. |
| *P* + m | A `Store.Get("notes", …)` issued now is answered from the pending-write map with the latest text, even though the file is a second behind; it still counts as one outstanding request and one read until `fn` runs in the script's next turn (§5.2). |
| *Q* (the user presses Escape) | The field loses the focus (`FocusChanged{focused = false, by = "user"}`, same-frame 6b); `text_entry` is popped; the frame stays active. |
| *R* (2 s after *Q*, a timer of the script calls `field:Focus()`) | Case 1 still holds (the frame is active, the user's last key, Escape, was on it, the pointer is over it, under 5 s), but a timer call is outside any gesture window, so the request only remembers the field and returns `false` (TM rule 7.2.2, X37). The user presses W and walks; case 1 ends. |
| *S* (the user types `/ext:notes add buy bread` in chat) | Step 1 intercepts the `ext:` word; no chat line, no echo. 6b calls `onAdd("buy bread", {command = "add", truncated = false, dropped = 0})` (§6.2); the turn is a gesture window, but the chat window is still active, so case 2 does not hold and a `field:Focus()` here returns `false` (no exception for the chat window, §7.7). |

<a id="s13-4"></a>
### 13.4 Edge cases

| Situation | Result |
|---|---|
| A script creates a frame covering the whole screen with `catch = true` and no background, then the user clicks the world | The click is caught by the frame (`Pressed`, `Released`, `Clicked`, each with `gesture = false`), so the world gets no click; for activation it counts as a world click: whatever window was active is deactivated and the frame is **not** activated (§7.6), and a later timer `field:Focus()` returns `false` (TM X38). |
| A script creates ten 1920 × 1080 frames at x = 10,000 (off screen) and shows them, then moves them on screen | Each counts one screen wherever it lies (§7.10): the second `Show` already returns `false` (over 1.25 screens), and moving the shown one never changes coverage (TM X31). |
| A focused field is covered when a plugin window is dragged over it in step 1 | The re-check at the end of step 1 takes the focus away at once; the next key is not routed to the field; the script gets `FocusChanged{focused = false, by = "host"}` in this frame's 6b. |
| `Store.Set("Notes", …)` | Lua error: `Notes` is outside the key grammar. |
| `Store.Set("con", …)` | `nil, "reserved key"`. |
| `Store.Set("t", v)` where `v` contains itself | `nil, "cyclic"`; nothing is stored. |
| `World.Ray` of 600 m | Lua error. |
| A script holds the `UI` table, then the user switches the optional `ui` grant off | The script is suspended in that frame and its frames are removed. When the user resumes it, `Resumed{reason = "grant"}` comes first, and the next `UI` call raises `"not granted"` (§2.9, TM X91). |
| `Ext.Use("Entity", 1)` by a script whose manifest does not declare `Entity` | `nil, "not declared"`, even if `entity.read` is granted (§3). |
| 33 `Store.Get` calls of small keys in one turn | The 33rd answers `nil, "busy"`; a `Get` whose reservation would pass the memory cap answers `nil, "out of memory"` instead of raising (TM X25). |
| A tick handler loops forever | The call aborts at its 4 ms deadline; `pcall` around the loop does not catch it; three such aborts in 10 s suspend the script with a notice. |

---

<a id="s14"></a>
## 14. Invariants

These are this spec's functional checks. They are **not** an escape catalogue: where an
invariant covers ground of an escape case, the case is TM's and is named in brackets; its
procedure and "Fails if" are TM §11's, and EM's `escape` runner runs it under that number
(TM §0.1 rule 3).

**Harness codes** (as TM §11 uses them). Every invariant carries one or more:

- **U**: a unit test of `tessera-ext`, `tessera-ext-host`, `tessera-input` or `tessera-ui`
  (the crate is named where it is not `tessera-ext`);
- **ST**: `scripttool test` in deterministic mode with EM §11.5's walk file. The walk file
  has no plugins, plugin windows or native panels, moves the pointer only together with a
  press, has no drag or held button, and allows at most 4,096 synthetic entities per run
  plus the flood entities of the record-flood hook `[[flood]]` (EM §11.5) within TM L86's
  flood bounds (at most 16,384 spawns per frame and 65,536 flood entities alive in any
  frame); an invariant that needs more is not ST;
- **CI**: the client's integration tests (EM's `client` runner), with plugins, native
  panels, pointer moves, drags, held buttons and golden images;
- **FZ**: a fuzz target in `fuzz/`; **RM**: the reference machine (E6, E9).

An invariant with several codes passes only when every part passes. Every ST part runs
on inputs EM §11.5 defines: the record-flood hook `[[flood]]` for invariants 36 and 37,
and the walk file's `[world] start` `yaw` (radians, §2.10) for invariant 40 (OPEN-12 (d)
and (e), closed).

1. **ST.** `Ext.Use` of `UI`, `Input`, `World` or `Entity` without the grant answers
   `nil, "not granted"`; with it, two calls in one apartment return the same table; for a
   module the manifest does not declare it answers `nil, "not declared"`.
2. **ST** (the walk file's `manager` input). After a grant is revoked the script is
   suspended in that frame and runs no code until the user resumes it; after an optional
   revocation every function of a module table of that capability that the script kept
   raises `"not granted"`; after a required one the script is unloaded without its `Unload`
   handler [TM X91].
3. **U, FZ.** Every function of §4–§10 raises a Lua error, never panics, for: a NaN or
   infinite number where a finite one is required, a non-integer where an integer is
   required, a DID outside 0 … 2^32 − 1, a value of the wrong type, an unknown property,
   context, action, setting, notification or field name, a missing argument and an extra
   argument (an explicit trailing `nil` included); and the first failing check of §2.4's
   order decides the result (fuzz target "Lua-side argument validation").
4. **ST.** A handle compares equal to every other handle of the same live object held at
   the same time; after removal, `Exists()` is `false`, `Kind()` still answers, and every
   other method raises.
5. **U.** `getmetatable` of any handle returns a string, never a table [TM X2].
6. **ST.** `Core.Time()` returns the same number for every call within one frame and never
   decreases; `Core.Frame()` rises by one per frame.
7. **ST.** A timer created in a turn never fires in that turn; a repeating timer fires at
   most once per turn.
8. **ST.** A 65th timer, a 33rd command (TM L68) and a 17th subscription answer
   `nil, "limit"` [TM X24, which covers `/ext:` commands at L68].
9. **U** (`tessera-ext-host`). A log call whose text holds CR, LF, ESC, a C1 control or
   U+2028 produces exactly one record in TM §8.1's format, in which none of those
   characters appears unescaped [TM X51].
10. **CI.** No function or report of these modules produces a chat line or a plugin chat
    event; an `/ext:` line, known or unknown, in any letter case, produces none either
    [TM X73, X66].
11. **ST.** `Store.Get(k)` called after `Store.Set(k, v)` answers `v`, whether or not the
    flush ran, including in the call of the `Set`; a `Set` after a `Get` does not change
    what the `Get` answers; after `Store.Delete(k)` it answers absent [TM X88].
12. **ST.** A script that sets one key to a value under 16 KiB in every frame for 10 s
    causes at most 11 writes of that key's file and is never answered `"busy"` for it.
13. **ST.** A value with a NaN, a cycle, a 33rd level, an 8,193rd element or more than
    1 MiB serialised answers `"invalid"`, `"cyclic"` or `"too large"`, and nothing is
    stored and no bucket is debited [TM X20].
14. **ST.** 32 outstanding `Store.Get` calls for 1-KiB values succeed in a script with a
    32 MiB cap, whether answered from the pending map or from disk; the 33rd answers
    `nil, "busy"`; a reservation over the cap answers `nil, "out of memory"` and raises
    nothing; 33 `Store.Set(k, v, done)` calls on one key before its flush (and the same
    with `Store.Delete`) leave 32 `done`s pending and answer the 33rd `nil, "busy"`, each
    pending `done` being delivered exactly once; a `Set` whose serialised bytes would pass
    the memory cap raises `"not enough memory"` and changes nothing [TM X25].
15. **U** (`tessera-ext-host`), **FZ.** A `Store` file edited to hold a NaN key, a nil key
    or 10^6 entries is answered `fn(nil, "invalid")` or `fn(nil, "too large")` from the
    worker, without a panic [TM X20, X59].
16. **ST.** `Commands.Register` of a name with `.`, an upper-case letter or over 32 bytes
    raises.
17. **ST.** A command line's arguments over 1,024 bytes (TM L91) arrive cut at a UTF-8
    boundary with `truncated = true`.
18. **CI.** An ext frame is never drawn above, and never catches a point of, a plugin
    window or a native panel, whatever their z-orders [TM X95].
19. **ST, CI.** A script's `Raise`, `Show` or `Activate` never moves its frame above
    another script's frame; a non-solid press never raises a script's block or frame, so
    repeated world clicks on a transparent full-screen catching frame leave another
    script's higher frame above it and catching its clicks (CI part) [TM X38].
20. **CI.** `frame:Activate()` and `field:Focus()` return `false`: with no window active
    outside a gesture window; with the chat window active in the turn of an `/ext:` line;
    after the user's solid press on the frame followed by a key press elsewhere, by the
    pointer leaving the script's frames, or by 5 s without input there. `field:Focus()`
    (and the focus part of `Activate`) also fails under case 1 outside a gesture window,
    and for a hidden, zero-size, off-screen, clipped, below-0.5-opacity or covered field
    (under the chat window, an opaque plugin window, another script's frame, its own later
    sibling or its own higher frame), for a field whose focus area a badge overlaps (its
    own frame's badge placed inside a frame at y = 5, or another script's badge attached
    above that script's frame), and for a field whose frame's badge is off screen or
    under a native panel or plugin window [TM X37, X39, X49].
21. **ST.** In the turn that delivers the `"start"` of the script's own action with no
    window active, `Focus` on a visible, uncovered field returns `true`, from the action's
    handler, from a later handler of that turn or from its `Tick`, also when the turn is
    split over two frames; under the "every frame is heavy" hook, with four other scripts
    whose turns fill the 2 ms cap ahead of it, a gesture turn split twice whose `Focus`
    comes more than 60 frames after the gesture returns `false` and focuses nothing
    [TM X89, its twice-split case].
22. **CI.** A press on a catching node whose own background at the press point is absent
    or has alpha × effective opacity below 0.5 (text and borders only, opacity 0.49, an
    image texel of alpha 0), or that lies under another painted node of any ext frame (an
    opaque non-catching `Image` of the same script over a `Field`), delivers `Pressed`,
    `Released` and `Clicked` with `gesture = false`, activates nothing, raises nothing,
    deactivates the window that was active, focuses no field, and a timer `Focus`
    afterwards returns `false` [TM X38].
23. **CI.** A focused field over which a plugin window is moved loses the focus before the
    next key is routed; the next key reaches the plugin window or the game [TM X39].
24. **CI** (golden image). Every shown frame with a catching node or a field carries the
    host-drawn badge with "⟨id⟩ · ext script" in its fixed box, wholly on screen (inside
    the frame for a frame at y = 5), whether or not a field has focus; no ext frame of any
    script draws over a badge; while a field has the focus the caret, the focus mark and
    the focus-coloured badge are drawn, and no node of any script and no badge draws over
    the caret or the mark: a focused field loses the focus before the next key when a
    frame move brings another frame's badge, or its own frame's inside badge, into its
    focus area, while its own frame's outside badge never takes it [TM X49].
25. **CI.** A script's `Set{text = …}` on a field clears its selection, and a copy after a
    text change copies nothing; a script call never creates a selection. With text set
    before the user selects, a copy puts nothing on the clipboard (the test's clipboard
    stub stays unchanged) when any selected character was clipped or scrolled out of view
    at any time between the selection and the copy ("ls" followed by 300 spaces and
    "; curl x|sh" in a `Field`, then `Ctrl+A`, `Ctrl+C`), when the text is drawn at colour
    alpha 0 or 0.49 or the field's effective opacity puts it below 0.5, or when a plugin
    window or another frame covers part of the selection; text with U+200B, U+202E and
    U+0007 inside it, fully displayed, is copied without them, and LF and tab are kept
    only in a `multiline` field [TM X52].
26. **CI.** World labels are never hit-tested: a world label over a plugin button changes
    neither the button's hover nor its click, and receives no notification [TM X45].
27. **U** (`tessera-ui`), **CI.** A show, creation, size change or `node:Add` that would
    take a script's composition coverage over 1.25 screens, or all ext over 2.0, is
    refused and changes nothing; every entry counts (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1) physical
    pixels capped at one screen, and a badge its fixed box under any font (U); moving a
    shown frame, by the script or by the user's drag, by whole or fractional pixels at UI
    scale 1.25, never changes coverage; a user resize stops at the bound; a screen resize
    that passes a bound hides frames and labels in reverse order of their last show (CI)
    [TM X31, slice-1a part].
28. **CI.** An `Input.Activate` made in 6b of the frame before a backend switch is never
    applied; nor is one made before a suspension or unload (D17 §6.6, the slice-1a intent
    test) [TM X80].
29. **CI.** An ext binding on a **key or pad** stroke never fires while a text field (chat,
    plugin or ext) has the focus, and no ext binding fires for an input that a non-ext
    binding takes in either pass (D17 exit criterion 2). An ext binding on a mouse button
    may fire while a field has the focus (IB D6, §7.8) [TM X34, X36].
30. **ST.** `Input.Standard` of any name off the list of §8.3 raises;
    `Input.Standard("mouselook")` is pressed exactly while the `mouselook` action has a
    hold from either context.
31. **U** (`tessera-input`). A script that toggles its context every frame never cancels a
    pending sequence of a non-ext context (D17 exit criterion 2) [TM X40].
32. **ST.** Every `World` function answers `nil, "not loaded"` for a point in a landblock
    that is not resident, and never causes that landblock to be requested.
33. **ST.** `World.Ray` answers `nil, "not loaded"` when any landblock its bounding box
    overlaps has no installed collision set, and never a partial result.
34. **ST.** The 65th ray, the 257th point query and the 9th entity query of a script in one
    frame answer `nil, "busy"`; the next frame they succeed.
35. **ST.** `Entity.Find` never returns more than `limit` results and sets `truncated`
    exactly when more matched.
36. **ST** (the record-flood hook `[[flood]]` of EM §11.5 within TM L86, for example
    `{ from = 60, to = 60, spawn = 10000, life = 600 }`). With 10,000 spawns in one frame, 4,096 are
    recorded, every subscription's next batch reports `unfiltered = 5,904`, no filter work
    happens outside a filter call, and a subscription created while its script's cursor
    lags matches no record of its own or an earlier frame [TM X19].
37. **ST** (the record-flood hook `[[flood]]` of EM §11.5 within TM L86, for example
    `{ from = 60, to = 659, spawn = 512 }`), **RM.** A script starved for 10 s while 1,024
    entity events arrive per frame keeps its memory within its cap and its frames within
    the ext ceiling of TM L12, and its filter calls alone never throttle, suspend or abort
    it [TM X19, X24].
38. **ST.** In deterministic mode, two runs with the same seed and walk give the same calls
    in the same frames, the same notification order (§2.8) and the same answers to every
    query, while real worker timing varies (D17 exit criterion 7).
39. **ST.** Under the "every frame is heavy" hook in deterministic mode, a queued `/ext:`
    line is delivered within 16 frames (plus the round robin) [TM X85].
40. **ST** (the walk file's `[world] start` `yaw`, EM §11.5). `World.Here().yaw` equals
    `start.yaw` wrapped into (−π, π], within 10⁻⁶ rad compared modulo 2π, until the character turns; `World.Here().heading`
    equals (90 − `yaw` · 180 / π) wrapped into [0, 360); and every `yaw` the host returns
    lies in (−π, π].
41. **ST.** A `Store.Set` of a key whose previous value a flush has taken and is still
    writing (the harness holds the flush in flight) debits one key token and the new
    value's full counted size; a script rewriting 1,024 keys every frame for 60 s, with
    every flush held in flight, writes at most the bound of TM X57 [TM X57].
42. **ST.** A script whose own calls fill the 2 ms cap every frame resumes its split turn
    first in its group at most once per turn; every later part of that turn runs at the
    script's ordinary rotation place, so the other scripts of its group are never starved
    beyond the round robin [TM X17].
43. **CI.** With the process locale set to a non-"C" value in a test build, every loaded
    ext script is suspended in the frame of the next check, no load or resume succeeds
    until the locale is "C" again, and the first turn after a resume carries
    `Resumed{reason = "locale"}` [TM X22].
44. **ST.** A state notification frozen in a turn's delivery set is delivered with the
    value it had when the turn started; a change the script's own call makes during that
    turn is delivered as a separate entry in the next turn, never in the same turn.
45. **ST** (the walk file's `manager` input), **U** (`tessera-ext-host`, the failed
    open). A re-read of the grant store that removes, invalidates or narrows a loaded
    script's record suspends it in the frame the result is applied; one that widens it
    changes nothing until the next load; a re-read of a newer-`version` document, of a
    document damaged as a whole, or whose open fails (U) suspends nothing; a manager
    revocation and a widen of one optional grant in one frame under `--latency 2` leave it
    granted, and a Reset followed by a first-enable Allow and load before the Reset's
    write completes neither cancels nor suspends the new load [TM X91].
46. **U.** In the ext profile, `table.foreach`, `table.foreachi`, `table.getn`,
    `table.setn`, `string.gfind` and `math.mod` are `nil`, and every global and library
    function outside §2.11's lists is absent [TM X8].
47. **ST** (the walk file's `manager` input). After an optional `entity.read` revocation and
    a resume, a kept entity handle passed to `Entity.Read`, `e:Get`, `e:Exists()` or
    `e:Kind()` raises `"not granted"` while still comparing equal to itself as a table key,
    also when the entity despawned before the revocation (the minting check comes first,
    §2.4 step 2);
    after an optional `ui` revocation every node handle is stale; an intent holding a handle
    whose minting capability is gone is never applied (from slice 1b also `lease:Follow`)
    [TM X91, its kept-handle sub-case].
48. **CI.** After a solid press, the script's turn may reorder, show or hide its own
    frames and nodes, but no node of the script drawn above a focused field (a later
    sibling, a later sibling of an ancestor, a higher frame of the script), whether it
    catches the pointer or not, can overlap the field's focus area without the field
    losing the focus before the next key; a `Focus` request while such a node overlaps it
    returns `false` [TM X38, X39].
49. **ST.** A script suspended by any cause (the panic chord included) has every ext
    context inactive and its holds released in that frame; a stroke bound only by it then
    records no edge and opens no gesture window; after the resume `Input.IsActive` answers
    `false` for every context until the script activates it again [TM X91, its suspended-
    context sub-case].
50. **ST, CI.** A gesture window closes at the end of its turn, at the user's first key,
    press or wheel step resolved outside the script's frames and actions (a `W` to the
    world), or 60 frames after the delivering call started in deterministic mode,
    whichever comes first; a `Focus` request after it returns `false` (ST); a solid press
    held for 10 s opens one window, at its `Pressed`, and its `Released` and `Clicked`
    open none (CI: the walk file has no held button) [TM X89].
51. **ST.** With the notification queue full (1,024 entries) and 32 `Store.Get` requests
    outstanding, every request's `fn` is delivered exactly once, also across a suspension
    and resume, and no other dropped notification goes unreported: every delivered table
    of that name carries `dropped`, or `Resumed.counts` carries it under its key (§2.8,
    §4.1).
52. **ST.** In the first turn after a resume of a script with a subscription, `Resumed` is
    the first call, before the filter call, the kept results and the queued
    notifications (§2.1, §4.1).
53. **U, ST.** A call whose table argument holds an unknown field, a field of the wrong
    type or out of its range (a `node:Set`, an `Entity.Find` filter, a `World.Ray`
    `opts`) raises only after the step-5 charge, changes nothing, keeps the walk's units
    charged and takes no per-frame count: the 8 `Entity.Find` calls a frame allows all
    still succeed after any number of such raises (§2.4).
54. **ST.** An `Entity.Find` or `Entity.Subscribe` `box` with x1 > x2 (or y1 > y2,
    z1 > z2) matches exactly what the same box with its corners swapped matches; an
    `around_self` radius below 0 or above 500 raises.
55. **ST.** A loop that creates `Image`s with a `file` and removes them in the same turn,
    every frame for 10 s, run once with a distinct file per `Image` and once with one file
    for every `Image` (joined requests), never has more than 32 requests of the script
    outstanding nor more than 32 decodes queued or running, is answered `nil, "busy"`
    while 32 requests are outstanding (in the joined run although only one decode job
    exists), and never receives a `Loaded` for a removed node [TM X26].

---

<a id="s15"></a>
## 15. Open items

Numbers are never reused; a closed item stays listed with the place that closed it.

> **OPEN-1 (costs):** every unit cost of §2.5 and of the function tables is provisional.
> Slice 1a measures each native with the patched build (D17 E7) and writes the calibrated
> costs into the registry; the reference is then generated from it (D17 §10). If the
> maximal value conversion `V` exceeds half of a call's 800,000 units, D17 §8's fallback
> lowers the element cap and this spec follows. `c_f` (§10.3) is TM OPEN O-4.

**OPEN-2 (Store reservation ratio): closed here, tracked as TM OPEN O-4.** The reservation
rule is TM §8.14 (§5.3); the neutral reader's declared expansion ratio is measured in E5.

**OPEN-3 (values Tessera computes): closed by the reviewer's confirmation** (re-review of
revision 3): §9.2's promise-only contract (units, ranges, directions; no formula, rounding
or format) satisfies D17 §12 and LEGAL §18.3 rule 1 for `World.MapCoords`,
`World.LocText`, `World.Clock` and `World.Sky`. The fallback stays recorded as the plan if
the owner disagrees: `MapCoords` keeps `north` and `east` from a Tessera-defined linear
function of the region frame and drops `text`; `LocText` is withdrawn from the major;
`Clock` keeps only `seconds` and `time_of_day`; `Sky` keeps only the sun vector and drops
`day`.

**OPEN-4 (composited-area bound): closed by TM §7.1** (1.25 and 2.0 screens, TM L55;
§7.10). E9 and the bounds' possible shrinking are TM OPEN O-1; the budget row's funding is
TM OPEN O-2.

> **OPEN-5 (image formats):** which file formats an ext `Image` accepts. Tessera's UI image
> decoders (UR §4) cover DDS and TGA; whether slice 1a adds PNG (a new decoder dependency,
> the format script authors most expect) is a decision for M5.5's plan. Until then the
> accepted set is whatever the UI image decoder accepts, and anything else answers
> `Loaded{ok = false, reason = "invalid"}`. Whether a texel's alpha is available to the
> solid-press test for each accepted format is TM OPEN O-6.

> **OPEN-6 (fonts):** which font face and size each role `"body"`, `"small"`, `"large"`
> uses, and what the headless harness (no install) uses so that `UI.MeasureText` is
> deterministic in `scripttool test`.

**OPEN-7 (`World.Here` in fly mode): closed in revision 5, aligned with EA1b.** EA1b §4.2
reports the eye position of the user's own view (`Camera.GetUserPose`, and the leased view
of `Camera.GetPose`) only under `world.read`, and EA1b §4.6 and TM L82 measure the leased
eye from that point when no character is placed. `World.Here` follows the same rule: in
free fly without a character it reports the eye of the user's own view under `world.read`
(§9.1), never a leased view, so the two slices answer one question one way. A review that
decides the free-fly eye must not be reported under `world.read` changes both specs
together (`Here` would answer `nil, "not loaded"`, and EA1b §4.2 would drop its position
fields without a character).

> **OPEN-8 (entity kinds):** how networked entities split into kinds once the M7 session
> layer reports entity types; the split is an addition within major 1 (new kind names), and
> scripts must treat unknown kind names as "other".

**OPEN-9 (liveness under the heavy-frame gate): closed by TM §7.4 and §8.12** (the floor
frame, the switch timeout and the gate's clock origin, TM L8–L11; §2.6, §4.1).

**OPEN-10 (consistency with `ext-api.md`): closed.** This spec and EA1b together are D17's
`ext-api.md` (§0); the general rules are here and EA1b follows them. The narrow
gesture-call reading is withdrawn in favour of TM §8.13's gesture window; the split-turn
rule is TM §8.13's; the reason `"limit"`, the frame-block ordering (§7.1), the handler
pattern (§2.8) and the angle and rotation forms (§2.10) are general rules of this spec.
Revision 3 confirms the angle form against EA1b's re-review: direction angles are radians
in every API spec, `heading` is a result only, and EA1b §3.1, §4.2, §4.7, §4.8 and §7.2
take `yaw` and `pitch` as §2.10 states (EA1b's citation of an "EA1a §9 'Heading'" means
§2.10). EA1b's handler names follow §2.8 (`Core.On("Tick")`, `Core.On("Unload")`), and the
generated reference is named in §0.


**OPEN-11 (display category of `help`): closed by TM revision 5.** TM §8.2 step 4 names
the **help line** category for `Commands.Register`'s `help` (96 code points and 96 bytes,
its L91 cap), so an accepted `help` is never cut and only steps 1–3 and 5 change it (§6.1;
TM §13). The interim "error or log line" cap is dropped.

**OPEN-12 (requests to and from the companion specs, TM §0.1 rule 5's form): closed in
revision 6.** Every part is answered; the record stays for its history.

- (a) **EA1b §3.1** gave `Region` as an integer 0–255, while this spec's `region` is
  1 … 255 (§7.9) and `coordinates.md` §4 makes 0 "no region". **Answered by EA1b revision
  5** (§3.1, §7.2): every region argument refuses 0, and 0 is only reported, for a sandbox
  without world data.
- (b) and (c), **answered by EA1b revision 4**: EA1b refuses a kept entity handle in
  `Follow` at the call and at every apply under §2.4 step 2 and §2.7, cites §2.4's order
  of checks and extra-argument rule in §10.3, and cites the handle rule as §2.7.
- (d) **EM §11.5** gave the walk file's `[world] start` "heading" in degrees without its
  form. **Answered by EM revision 5** (§11.5): `start` takes `yaw` in radians in §2.10's
  form and has no `heading` key; invariant 40 uses `start.yaw`.
- (e) **EM §11.5 and §11.8** had no record-flood hook, which TM rule 7.5.2's tests (X19's
  ST part) and invariants 36 and 37 need because L86 caps walk-file entities at 4,096 per
  run. **Answered by EM revision 5's `[[flood]]`** (§11.5), whose bounds TM revision 6
  registers in L86 (at most 64 entries, `spawn` 1–16,384 per frame, `life` 1–600 frames,
  at most 65,536 flood entities alive in any frame), closing EM OPEN O-13; invariants 36
  and 37 and §14's preamble cite it.
- (f) **From EA1b to this spec, answered in revision 5 (EA1b §10.3 OPEN part (a)):** the
  field checks of table arguments run in the walk after the charge, not at §2.4 step 3
  (EA1b §10.3, invariant 23): §2.4 step 3 checks only what is known without reading a
  table's contents, and step 6 starts with the walk of the table arguments, which raises
  for unknown fields, field types and ranges and element caps before any effect. EA1b
  §4.2's free-fly eye under `world.read` settles OPEN-7 one way for both specs (closed
  above).
- (g) **From EA1b to this spec, answered in revision 6 (EA1b §10.3 OPEN part (b), EA1b
  §14 row "EA1a §2.4 steps 2 and 3"):** §2.4 step 2 now checks a handle, argument or
  `self`, in EA1b §10.2's `handle(K)` order: the minting capability first (`"not
  granted"`, whatever the kind and whether or not the handle is stale), then the kind,
  then staleness; step 6 applies the same order to handles inside table arguments. So
  §2.7's minting rule wins over its stale-handle rule, and `Exists()` and `Kind()` raise
  `"not granted"` even on a stale handle whose minting capability is gone. No outcome
  differs today: every slice-1a handle method belongs to its handle's minting module
  (step 1 decides first), and EA1b already applies this order to its own entries.

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (design, from accepted decision 0017): §0–§16. The slice-1a module reference for `Core`, `Store`, `Commands`, `UI`, `Input`, `World` and `Entity`, with types, results, failures, costs, timing and capabilities; closes for these modules D17's open items 2 (presses that do not activate), 5 (subscription filtering inside metered calls with a cursor and a recorded-list cap), 6 (`Store` read-your-writes with a per-(kind, id) pending map), 8 (world labels never catch the pointer) and the composition half of item 1 (composited-area bound), and the "Other improvements" on log escaping, the ownership badge, clipboard selection clearing, image decodes as outstanding requests, sticky aborts, split turns and gesture calls, and `Store` reservations from the recorded size. |
| 2026-10-07 | Revision 2 (Review), answering the independent review by adopting `ext-threat-model.md` revision 2 as normative. **Authority** (§0, header): TM cited for every limit (TM L*n*, §11 rewritten with an L column), every rule of its §7–§9 and every escape test (TM X*n*); this spec and EA1b together are D17's `ext-api.md`, and the general rules live here; no escape catalogue of its own (§14's invariants name the TM tests they overlap). **Open item 2** (§7.6, §7.7): "visible painter" replaced by TM's solid press (own background or image only, with a per-kind form for host-drawn faces); a non-solid press is a world click for activation and focus; `Pressed`, `Released`, `Clicked` carry `gesture`; under case 1 a `Field` is focused only inside a gesture window. **Gesture window** (§2.1, §2.6, §7.7): the whole delivering turn (TM §8.13), not one call; solid presses are gestures. **Composition** (§7.10, §7.9, §7.2, §11): 1.25 and 2.0 screens, unclipped and position-independent per-entry counting, badges and labels counted, no nested surfaces in slice 1a, user resizes stop at the bound, host changes hide in reverse show order, the TM L56 budget row and E9; OPEN-4 closed. **Entity filtering** (§10.3): one batched filter call per turn costing 64 × subscriptions × `c_f`, matches queued as batch notifications, overflow reported as `unfiltered`, cursor to the newest frame at resume with the gap in `Resumed`, the 15 MiB retained bound. **`Store`** (§5): TM §7.6 read-your-writes (pending reads answered from a copy at the call, no worker job but counted as outstanding and against the read rate, delivered in a later turn; pending deletion answers absent; wipes clear the map; pending entries count to the totals and the writer's cap); TM §8.14 reservations from the pending entry or file index, refusals return `nil, "out of memory"` (new reason, §2.4); TM §8.7 byte buckets (32 KiB/s and 10 MiB per script, 128 KiB/s and 40 MiB for all, debited at the call, per session), temporary files counted and removed at open; the owner tag of TM §8.24. **Badge** (§7.1, §7.7): on every shown frame with a catching node or a field, "ext script", top layer of the ext range, counted in coverage, no longer part of a field's focus area. **Images** (§7.10, §7.3): a decode at the cap answers `nil, "busy"` / `false, "busy"` at the call. **Liveness** (§2.6, §4.1): floor frames, the switch timeout (close without `Unload`) and the gate's clock origin; OPEN-9 closed. **Memory** (§11): kept snapshot data files, the file index and pending entries in the per-script row. **New general rules** (§2.8, §2.10, §3): one handler pattern (`Module.On`, `obj:On`, per-request callbacks; `Core.OnTick`/`OnUnload`/`OnResumed`/`OnSettingsChanged` became `Core.On("Tick"|"Unload"|"Resumed"|"SettingsChanged")`, `UI.OnScreenChanged` became `UI.On("ScreenChanged")`, `Input.On` handlers take one table); notifications queued only with a handler, gestures always; a FIFO queue with a fixed delivery set per turn (the §2.8 order contradiction fixed); yaw in radians per KM §9.1 beside `heading` in degrees, quaternions `{w, x, y, z}` (`yaw` added to `World.Here` and entity fields); `Ext.Use` checks the manifest's module tables and answers `"not declared"` for an undeclared module (closing EM OPEN O-2); every ext context has priority −500 (closing EM OPEN O-3). **BIN pass-through** (§0, §9.2, OPEN-3): `World.Clock` and `World.Sky` get contract limits like `MapCoords`. **Log** (§4.3): TM §8.1's one format. **Minor**: `V` = 295,112; `Input.Standard("mouselook")` defined as the one registry action; revocation removes subscriptions too (§2.9); OPEN-2 and OPEN-10 closed; examples §13 and invariants §14 updated (invariants 39 and 40 added). |
| 2026-10-07 | Revision 3 (Review), answering the re-review of revision 2 and carrying TM revision 3. **Angles** (§2.10, blocking): one rule for every API spec: every direction angle (yaw, pitch, an angle about +Z relative to a yaw) is in radians per KM §9.1, `heading` is a result only, no argument is ever a heading or takes degrees for a direction; the claim about EA1b restated as a requirement EA1b follows (`Camera` pose, `Follow`, key frames, `Sandbox.Teleport`); a field of view is an extent, stated by its module; §0 names §2.10 as the only angle section. **Generated reference** (§0, §2.5): `docs/specs/ext-api-reference.md` (EA1b §10.4, §10.6), derived, never hand-edited, adds no rule. **Fairness** (§2.6): a split turn resumes first in its group at most once; a part split again waits for its ordinary rotation place (TM §8.13, X17); invariant 42. **Locale** (§2.8, §2.9, §4.1, §7.11): a process locale other than "C" suspends every ext script (D17 §1.5, TM T22, X22); no load or resume until it is "C" again (1a rule); `Resumed.reason = "locale"`; §2.8 lists the four suspension causes; invariant 43. **Ext environment** (new §2.11): D17 §1.2's globals, absent names and replacements (`print`, `tostring`, per-apartment random numbers) and TM §8.15's library tables, removed compatibility names and `math.max`/`math.min` charged by argument count (X8); invariant 46. **Coalescing** (§2.8): entries of a turn's fixed delivery set are frozen; a later change, self-caused or from the next frame's input in a split turn, is a new entry after the set, so self-caused values arrive in the next turn and at most one entry per node and kind waits outside the set; invariant 44. **Window removal** (§7, §7.11): no host close control; the host routes are the panic chord and the script manager (unload, revoke `ui`); D17 §3's "closing the window" only through the script's own control. **`Store`** (§5, §11, §13.3; TM §8.7, L44, X57, rules 7.6.1–7.6.3, §8.8, §8.24): taken and untaken pending entries; a key is clean once a flush has taken its value, even in flight, so a `Set` or `Delete` landing during an in-flight flush pays a key token and the full counted size, and only a replacement of an untaken value pays growth alone (invariant 41); the file index charged in full to every apartment that has the store open; reads from the pending map are delivered in the **next** turn, disk reads in the first turn after their job; an unreadable owner tag also wipes; the store directory opened without following links, temporary files named `t_<key>.tmp` (EM §3.1, §3.6, §8.8, TM X94). **Floor** (§2.6, §11): 250 ms counted from the start of the last frame in which ext ran; a floor frame resets both conditions and runs 6c with its 1 ms slice. **Revocation** (§2.9): the grant-store re-read is a route (narrowing suspends, widening waits for the next load; TM §8.23); invariant 45. **Smaller** (§2.1, §6.2, §10.3, §14): "Call" includes REPL runs (TM §2, L85); `/ext:reload` refused with "Developer mode is off" outside developer mode, and for an id not loaded queues nothing with a notice (1a rule); the entity worked example and X19 within L30 (4 × 16 and 16 × 4); invariant 8 cites L68 with X24, which now covers commands. |
| 2026-10-07 | Revision 4 (Review), answering the re-review of revision 3 and its follow-up review (items 1–25) and carrying TM revision 4. **(1) Register** (§0, §1, §2.3, §4.2, §6.1, §6.2, §7.4, §7.9, §8.2, §9.1, §10.2, §11, invariant 17): under TM §0.1 rule 6 every resource limit is TM's; this spec's per-call and per-object caps are TM L91 (command arguments, `help`, action edges, `Entity.Read` handles, query radius and box side, ray landblocks, `LocText`) and are cited so; positions, timer delay, node positions and sizes, label width, `region`, offsets and `max_distance` are marked "domain", node `name` "grammar"; §11 gains the L91 and L93 rows. **(2) Removal routes** (§7.11): the `ui` dialog text of EM §6.1 (EM revision 4) names the chord and the manager; cited. **(3) L56** (§7.10, §11): "the 1.0 ms GPU part of the 1.6 ms row". **(4) Costs** (§2.5 and every row): one counting rule, every key and every value, nested tables again, fixed shapes charged in full: `Store.Usage` and `Core.Script` `e(10)`, `World.Here` and `World.Clock` `e(14)`, `World.Sky` `e(18)`, `Input.State` and `Input.Standard` `e(10)`, `UI.Usage` `e(12)`; arrays `e(2n)` (`Core.Settings`, `Store.Keys`, `Commands.List`, `Input.Bindings`), `Input.Actions` `e(8n)`; `World` point-query results and ray hits charged; `node:Set`/`Get`/`Add` of `rows` `e(2 · #rows)`; `Entity.Read` per entry `e(2)` + `e(2 · #fields)` + `e(8)` for `rotation` + `e(4)` for `landblock`; walked filters charged; batch tables `h(n + 1)` + `e(8 + 2n)`; §13.2's cost restated. **(5) Handles across modules** (§2.4, §2.7, §2.9): every handle records its minting module; after that module's capability leaves the effective set every native that takes it raises `"not granted"` (`Exists`, `Kind` included) and every apply re-checks it (TM §8.23); invariant 47, X91's kept-handle sub-case. **(6) Own content** (§7.6, §7.7): a solid press is judged by the topmost painted node (TM rule 7.2.1); the visible-focus rule counts the script's own later nodes and higher frames as covering and requires the badge wholly on screen and uncovered (TM rule 7.2.2, §8.3, L58); the caret and focus mark are drawn in the badge layer (1a rule, stricter); the re-check list no longer names step 8; invariants 20, 22, 48. **(7) Block order** (§7.1): only a solid press raises a block or frame; invariants 19, 22. **(8) Coverage** (§7.3, §7.7, §7.10, §11, §13.1, invariant 27): entries counted by size, (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1), independent of position and sub-pixel phase (TM rule 7.1.2); the badge's fixed box 20 × (16 + 8 × (*n* + 13)) UI units and its on-screen placement (TM §8.3); `node:Add` that gives a shown frame its badge answers `nil, "limit"` past a bound; §13.1 recomputed (about 0.0075 screens). **(9) What opens a gesture window** (§2.1, §2.8, §7.6): only the `Pressed` of a solid press, an action start or an `/ext:` line; `gesture` on `Released` and `Clicked` is information only; a press on a disabled node is solid or not by the same test, activates by the same rule and opens no window. **(10) Window bound** (§2.1, §2.6): closed at the user's next input outside the script's frames and actions or 1 s (60 frames) after the delivering call (TM L93, which answers the request to TM); invariant 50. **(11) Suspension and input** (§2.8, §7.11, §8.1): contexts deactivated and holds released in the frame of the suspension, no input consumed and no edge or gesture recorded while suspended, the panic chord resolved first, contexts not restored on resume and `Input.IsActive` answering `false` (TM §8.23); invariant 49. **(12) Drops** (§2.8, §4.1, §5.3, §7.5): reserved queue room for results of outstanding requests and due timers, never dropped (TM L30); `dropped` on every queued module and object notification table; `Resumed.counts`' exact keys; invariant 51. **(13) Order of checks** (§2.4): the general order (grant and backend, arity and types with no extra argument, constraints, caps, charge, operation) and its exceptions for `Store.Set`, `Store.Delete`, `Store.Get` and `World.Ray`, with `Store.Set`'s precedence (walk faults, then `"full"`, then `"busy"`); EA1b §10.3 cites it. **(14)** Invariant 29 and §7.8 narrowed to key and pad presses (IB D6). **(15) Harness codes** (§14): U, ST, CI, FZ, RM on every invariant; the walk file's limits named; invariants needing plugins, drags or held buttons are CI; X19's ST part needs EM's record-flood hook (OPEN-12). **(16) X31** (§7.10): its slice-1a part only; nested, scale and rotation parts belong to the slice that adds them (TM revision 4). **(17) Entity subscriptions** (§10.2, §10.3): a subscription matches only records of frames after its creation (TM rule 7.5.2); `around_self` never matches a record without a placed character; `Find` without an area sorts from the character, or in the store's stable order without one; candidates are the entities of the requested kinds only; "delivered", not "matched"; the filter call is never an abort and is outside L17 (TM rule 7.5.2a). **(18) Layout** (§2.1, §2.6, §7.1, §7.3): layout runs inside the call that changes it; step 8 only paints and places labels. **(19)** "512 per `Frame` (window)" (§7.10, §11); only frames take a surface (TM L49). **(20) Pending reads** (§5.2, §5.3): a pending `Get` shares immutable bytes, or charges `b(stored size)` before copying (TM rule 7.6.3). **(21)** `help` drawn through TM §8.2 (OPEN-11 asks TM for its category); an `/ext:` line for a suspended script is not queued, shows EM §9.8's notice and counts under `Resumed.counts.command` (§6.2). **(22) Unspecified results** (§7.1, §7.5, §8.1): `Field` text must fit `max_bytes` (raise, never cut); `Slider` `min` ≥ `max` raises and `value` is re-clamped; a `List` selection past the new rows clears to `nil`; a consent prompt takes the focus with `FocusChanged{by = "host"}` and does not give it back; `Input`'s rate counts calls. **(23) Companions**: OPEN-12 asks EA1b for its region range and EM for the walk file's `start.heading` form and a record-flood hook; §6.2 cites EM §9.8's reload notices; §13.2 lists `Core` in `[modules]`. **(24)** The all-scripts log rate of 128 KiB/s (§4.3, §11; TM L45, OPEN O-3). **(25)** OPEN-3 closed by the reviewer's confirmation, the fallback kept (§0, §9.2, §15). |
| 2026-10-07 | Revision 5 (Review), answering the re-review of revision 4 (items 1–11), carrying TM revision 5 and answering EA1b's revision-4 review requests addressed to this spec. **(1) Copy** (§7.8, blocking; TM §8.4): a copy or cut also requires that the whole selected range was displayed from the selection to the copy (not clipped, not scrolled out, uncovered as for focus under L58, alpha × effective opacity ≥ 0.5), and strips Cc, Cf, Zl and Zp characters (LF and tab kept only in a `multiline` field); TM's colour-matches-background residual stated; invariant 25 extended to X52's clipped, scrolled-out, transparent, covered and invisible-character cases. **(2) Pending `done`s** (§2.4, §2.8, §5.3, §11, blocking; TM L27, invariant 36): every `Store.Set` or `Store.Delete` with a `done` is one outstanding request from the call until its `done` is delivered or discarded, a replaced value's included; `nil, "busy"` at §2.4 step 4 (before the value walk for `Set`; before the buckets for `Delete`); §2.8's never-dropped room is bounded by it; §11's L27 row lists it, per (kind, id) and shared across a hot reload; invariant 14 gains the 33rd pending `done` (X25). **(3) Focus and badges** (§7.1, §7.7): a badge whose fixed box overlaps a field's focus area covers it (any other frame's badge, and the own frame's badge placed inside the frame), except the own frame's badge attached outside the frame; the caret and focus mark are drawn above every badge; invariants 20 and 24. **(4)** A label's `VisibilityChanged` covers host hides and resume notifications with `by = "host"` (§7.9). **(5)** `e:Get` charges `e(8)` for `rotation` and `e(4)` for `landblock` (§10.2). **(6)** The file index holds each file's exact byte size and counted size (§5.2; TM rule 7.6.1). **(7)** `Resumed` is the first call of the first turn after a resume, before the filter call (§2.1, §4.1, §10.3); invariant 52. **(8)** `around_self` radius 0 … 500; a `box` takes its corners in any order (closed interval per axis, side 0 allowed) (§10.2); invariant 54. **(9)** An image decode counts toward L27 until its worker job ends, also after its `Image` is removed or re-imaged, and a removed node's `Loaded` is never delivered (§7.10; TM §8.5, X26); invariant 55. **(10)** A `Store.Set` whose serialised bytes would pass the memory cap raises `"not enough memory"` in the value walk, after the step-5 charge and before `"full"` and `"busy"`, changing nothing (§2.4, §5.2). **(11)** §13.3 row *S* shows `info.dropped`. **TM revision 5**: OPEN-11 closed (TM §8.2 step 4's help-line category, 96 code points and 96 bytes; §6.1); ε counted per value-model element (§5.3, §11 L28); the grant-store re-read narrows only between two documents Tessera understood, an unusable re-read suspends nothing, and Tessera's own write completions never narrow a newer decision (§2.9; TM §8.23, invariant 35); invariant 45 gains X91's new cases. **EA1b's requests**: the field checks of table arguments run in step 6's walk after the step-5 charge, a raise there changing nothing and taking no cap token (§2.4; EA1b §10.3, invariant 23); invariant 53; OPEN-7 closed by aligning `World.Here` in free fly with EA1b §4.2 and §4.6 (the user's own eye under `world.read`, §9.1); OPEN-12 records that (a), (d) and (e) are still with EA1b and EM. |
| 2026-10-07 | Revision 6 (status stays Accepted), answering the follow-ups F1–F8 of the re-review of revision 5, carrying TM revision 6 and closing OPEN-12 on the companions' revision-5 answers. **(F1) Handle check order** (§2.4 steps 2 and 6, §2.7, invariant 47): a handle, argument or `self`, is checked minting capability first, then kind, then staleness (EA1b §10.2's `handle(K)` order), also inside table arguments; the minting rule wins over the stale-handle rule, so `Exists()` and `Kind()` raise `"not granted"` on a stale handle whose minting capability is gone; answers EA1b §10.3 OPEN part (b) and EA1b §14's row "EA1a §2.4 steps 2 and 3" (OPEN-12 (g)). **(F2) OPEN-12 closed** (§14 preamble, invariants 36, 37, 40, §15): (a) by EA1b revision 5 (§3.1, §7.2 refuse region 0), (d) by EM revision 5 (`[world] start` takes `yaw` in radians; invariant 40 uses `start.yaw`), (e) by EM revision 5's `[[flood]]` with its bounds in TM L86 (TM revision 6, EM O-13); the preamble no longer says EM lacks the hooks; the ST harness code names the flood entities within L86. **(F3)** §7.1 and §7.7: the caret and focus mark are safe from badges because they are drawn above every badge, the own frame's outside badge included; the badge-cover and caret rules are now TM's too (TM rule 7.2.2, §8.3, L58, adopted in TM revision 6), so §7.7 no longer marks them stricter than TM. **(F4)** §2.8, §5.3: an outstanding request ends exactly once, by delivery, by a `Loaded` with no handler, by the never-delivered `Loaded` of a removed `Image` or of a replaced `file` (1a rule: a node reports only its current `file`, §7.10), or by the discard of every undelivered result at unload. **(F5)** §7.10, §11, invariant 55: joined decode requests for one file count toward L27 once each, per request, until their own `Loaded` is delivered or discarded and the shared job ends (TM §8.5, L27); TM X26 and invariant 55 run with distinct files and with one file. **(F6)** §5.3: a `done` of a value replaced while untaken comes with the replacing write; one whose value a flush had already taken comes when that flush's own write completes (1a rule). **(F7)** §7.8 copy item 1: "the moment the selection was made" is the last change of the selected range, so a drag or Shift extension that scrolls counts from its final range. **(F8)** §5.3, §11: a pending deletion adds no key and no byte to TM L40 and L41, present or absent (TM rule 7.6.5), so `Store.Delete` needs no `"full"` and never passes a space cap. **TM revision 6**: §7.10's host-change notification is `VisibilityChanged` under TM rule 7.1.5's own name; L93's known limit of a gesture turn split twice under permanent gating, which fails safe (§2.6, §7.7 case 2, §11, invariant 21 with X89's twice-split case). |
