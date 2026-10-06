<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Extended scripting, slice 1a: module reference (`Core`, `Store`, `Commands`, `UI`, `Input`, `World`, `Entity`)

| Field | Value |
|---|---|
| Status | **Review** |
| Last updated | 2026-10-07 |
| Author role | Design (Tessera's own API, written from the accepted decision 0017 and Tessera's own specs; no dirty-room input), AI-assisted |
| Reviewer | Independent clean-room review session, 2026-10-07 (first review, re-review of revision 2 and re-review of revision 3): **changes requested**, status stays Review |
| Review record | Independent review (clean room: D17, 0004, 0014–0016, the ext, plugin, keymap and UI specs, LEGAL §18), 2026-10-07: **changes requested**, status stays Review. (1) Authority: the spec cites a non-existent `ext-api.md` for its general rules and never cites `ext-threat-model.md`, whose §7–§8 rules are declared normative for D17's open items and "other improvements". This spec closes items 2, 5 and 6 with different rules. Either adopt the threat model's rules or record an agreed amendment in both specs, and say whether this file is `ext-api.md` (ext-api-1b calls it "EA"). (2) Open item 2 (§7.6–§7.7) conflicts with TM §7.2. Under TM a "solid press" excludes text and borders, a non-solid press deactivates the active window like a world click, notifications carry `gesture = false`, and focus under case 1 is refused outside a gesture turn. This spec allows a timer to focus during case 1. (3) The gesture window is narrowed to one call (§2.1, §7.7). TM §8.13 and D17 "Other improvements" define it as the whole turn that delivers the gesture. (4) Composition-area bound: §7.10 gives 2.0/6.0 screens, TM §7.1 gives 1.25/2.0 (the badge counts, and nested entries count again), and 1b §6.4 gives 2·S/3·S physical pixels. The spec has no rule for user drags or for host resizes and UI-scale changes, and no GPU budget row or E9. (5) Entity filtering (§10.3) differs from TM §7.5: per-subscription delivery calls versus one batched filter call that queues notifications, `dropped` versus `unfiltered`, a different cost formula, and a different handling of suspension. The 15 MiB retained-record bound is not mentioned. (6) `Store` read-your-writes (§5.2) disagrees with both TM §7.6 and 1b §8.6 on whether a read of a pending value is an I/O-queue job and whether it counts as an outstanding request. The rule that a wipe clears the pending map and the counting of pending entries toward size totals are missing. (7) A refused `Get` reservation raises here but returns `nil, "out of memory"` in TM §8.14/L28. (8) The `Store` byte write rate here is 1 MiB/s and 4 MiB/s, but TM §8.7/L44 applies 32 KiB/s and 128 KiB/s with buckets until the owner decides. Temporary-file counting and crash-leftover removal are missing. (9) The ownership badge here shows only while a field is focused. TM §8.3 requires it on every shown frame that has a catching node or a field. (10) A busy image decode answers `Loaded{ok = false}` here but `nil, "busy"` in TM §8.5. (11) OPEN-9 says there is no liveness floor, but TM §7.4 already defines one (and ext-manifest O-5 asks this spec to carry it). The gate's clock origin (TM §8.12) is missing. (12) Retained snapshot memory (TM §8.10, ext-manifest O-4) is undecided, and the §11 memory row omits it. (13) Missing rules: what `Ext.Use` does with the manifest's `[modules]` and minor versions, including an undeclared module (ext-manifest §4.8, O-2), and how ext context priorities are assigned (O-3). (14) §2.8 "Order" says "in the order they arose" but puts this frame's step-1 input ahead of notifications caused in the previous frame's 6b. (15) `World.Clock` and `World.Sky` pass through values computed by algorithms marked BIN (environment.md §11 "BIN unless stated", sound.md §10.3.5), yet §0 names only two such results. Give them contract limits like OPEN-3 (LEGAL §18.3 rule 1). (16) The log record format (§4.3) differs from TM §8.1 (prefix, `\xNN` escaping, cut mark). Minor: the value of `V` is 295,112 units, not 295,000; `Input.Standard("mouselook")` is ambiguous because KM §8 binds it in both `camera` and `movement`; state the angle and quaternion conventions and the handler pattern as general rules so that 1b's OPENs can align. **Revision 2 (2026-10-07)** answers every point by adopting `ext-threat-model.md` (TM) revision 2 and closing EM OPEN O-2 and O-3 (change log, §16); awaiting re-review. **Re-review of revision 2** (clean room: D17 in full, TM revision 2 §0–§14, EM and EA1b revision 2, KM §3 and §8–§9.1, `coordinates.md` §4), 2026-10-07: **changes requested**, status stays Review. All sixteen points and the minor items of the first review are fixed: TM is cited as normative throughout, every repeated figure matches TM §10 (L3–L12, L17, L18, L22–L31, L33–L45, L48–L51, L55–L62, L68–L71, L81), every cited X-number exists with matching scope, and every EA1a row of TM §13 is carried. Remaining issues: (1) **Blocking, consistency with EA1b.** §2.10 makes yaw and pitch radians and "no argument is ever a heading" a general rule, and says "this is the form EA1b's `Camera` takes"; EA1b §3.1 (which defers to EA1a on general rules) takes a compass **heading in degrees** (any finite value, modulo 360), pitch and field of view in degrees, builds the view direction from the heading, and cites a non-existent "EA1a §9 'Heading'". A script passing `World.Here().yaw` to `Camera` gets a different direction under each spec. Settle one form in §2.10 and make EA1b follow, or correct §2.10's claim. (2) Consistency with EA1b, smaller: EA1b still names `Core.OnUnload` and `Core.OnTick`, which revision 2 renamed to `Core.On("Unload" \| "Tick", fn)`; EA1b §14 asks EA1a §0/§3 to name the generated reference `docs/specs/ext-api-reference.md`, which this spec does not (§2.5 says only "generates the reference"). (3) **Fairness (§2.6, 1a rule "the group position").** A split turn resumes as the first script of its group. A script whose own calls fill the 2 ms cap (for example a 1,024-entry delivery set) is split again every frame and so runs first in every frame until its turn ends, and other scripts of its group get no 6b time in those frames, against D17 §4's rotating round robin and TM X17 ("others starved beyond round robin"); the rolling-time throttle reacts only after its 1 s window. Resume the turn at the script's ordinary round-robin position, or give the head position only when another script's calls or the gate split the turn. (4) Locale suspension: D17 §1.5 ("The process locale stays C") suspends every ext script when the locale is not "C", and EM §10.5 points to EA1a §7.11 and §2.9 for it, but §7.11 lists only watchdog, panic chord and revocation, and `Resumed.reason` (§4.1) has no value for it. (5) TM §13 asks the API specs to carry TM §8.15 (the ext profile's standard-library tables, the removed compatibility names, `math.max`/`math.min` charged by argument count; X8); this spec, the home of the general rules, neither states nor cites it (nor D17 §1.2's environment, e.g. `print`'s replacement). (6) §2.8 does not say what happens when a state notification's coalesced entry is already in the current turn's delivery set and the script's own call changes the value again: updating that entry would deliver a self-caused value in the same turn (against D17 §4, "next turn"); a new entry would break "one entry per node and kind". State which, since invariant 38's traces depend on it. (7) D17 §3's stated exception says the user removes a pointer-catching ext window "with the panic chord, the script manager or by closing the window"; this spec gives the user no host route to close or hide one frame (no host close control; the badge is never a pointer target). Define one (for example a "hide windows" action per script in the manager) or state that the routes are the chord and the manager. Minor: §2.6's 250 ms floor condition counts from the **start** of the last frame in which ext ran, and a floor frame also runs 6c with its 1 ms slice (TM rule 7.4.1); a read answered from the pending map is delivered in the script's **next** turn (TM rule 7.6.3), not "a later turn" (§5.2); TM §8.24 also wipes on an **unreadable** owner tag (§5.2); invariant 8 cites TM X24 for the 33rd command, but X24 does not cover commands. **Revision 3 (2026-10-07)** answers every point of this re-review and carries TM revision 3 (the §8.7 re-dirtying rule for `Store` writes, the at-most-once head position of §8.13, the reset of rule 7.4.1, the per-apartment file-index charge of rule 7.6.1, the store no-follow rule of §8.8 and the grant-store re-read route of §8.23); change log, §16. Awaiting re-review. **Re-review of revision 3** (clean room: D17 in full, including "Open security and design items", "Other improvements" and the M5.5 deliverables; TM revision 3 §0–§14; EM and EA1b revision 3; KM §3, §8, §9.1), 2026-10-07: **changes requested**, status stays Review. Every point of the re-review of revision 2 is fixed. §2.10's angle form is now the one EA1b §3.1, §4 and §7.2 follow. EA1b uses `Core.On("Tick" \| "Unload")` and `docs/specs/ext-api-reference.md`. The split-turn head position is at most once (TM §8.13, X17; invariant 42). The locale suspension and `Resumed.reason = "locale"` are in place (invariant 43). §2.11 carries D17 §1.2 and TM §8.15 (invariant 46). Frozen delivery-set entries are fixed (invariant 44). The window-removal routes are in §7.11. The floor-frame reset, the next-turn pending read, the unreadable owner tag and invariant 8 are corrected. TM revision 3 is carried: the taken/untaken pending entries and full re-payment of §8.7/L44 (invariant 41, X57); the per-apartment file-index charge of rule 7.6.1; the no-follow store directory of §8.8 (X94); and the grant-store re-read route of §8.23 (invariant 45, X91). Every repeated figure checked matches TM §10 (L3–L12, L17, L18, L22–L31, L33–L45, L48–L51, L55, L57–L62, L68–L71, L81). Every cited X-number exists with matching scope. The worked examples' arithmetic holds (heading 270.3 for yaw 3.1364; 10,240 and 2,560 units per filter batch; invariant 12's ≤ 20 KiB per flush under 32 KiB/s). Remaining issues: (1) **Blocking, consistency with TM §0.1 rule 5 and this spec's own §0 ("Every numeric limit is TM's").** TM rule 5 (revision 3) says every count, size or range of extended scripting has an L-number. A companion spec that needs one requests it as an OPEN; EA1b does this for its `Overlay.Lines` cap. This spec states caps that have no L-number and makes no request: command arguments at most 1,024 bytes (§6.2, invariant 17) and `help` at most 96 bytes (§6.1); at most 16 edges per action per frame (§8.2); `Entity.Read` at most 512 handles, a `near` or `around_self` radius of at most 500 and a `box` side of at most 1,000 (§10.2); a timer delay of at most 10^6 s (§4.2); position magnitudes of at most 10^7 (§2.3); node `x`, `y`, `width` and `height` of at most 16,384 and `name` of at most 64 bytes (§7.4); world-label width 512, `region` 1…255, offsets of at most 256 and `max_distance` 1…500 (§7.9); `LocText` at most 256 bytes (§9.1, §9.2); a ray over at most 25 landblocks (§9.1). Fix: add an OPEN asking TM §10.6 to register these (or the subset TM counts as limits), then cite their L-numbers; or have TM state which argument ranges rule 5 leaves to the API specs, and correct §0's sentence to match. (2) §7.11 says "the grant dialog's text and the user documentation name the chord and the manager", but EM §6.1's `ui` dialog text ("Show its own windows and labels. They take clicks and the mouse wheel where they lie.") names neither, and this spec records no change request to EM. Either drop the claim or request the wording from EM. (3) §7.10 "Frame budget" and the §11 L56 row describe the TM L56 row as 1.0 ms of GPU time. TM L56 and rule 7.1.6 give the row as 1.6 ms (0.6 ms overlay encoding plus 1.0 ms GPU). Say "the 1.0 ms GPU part of the 1.6 ms row", so that the repeated figure matches TM §10 (TM §0.1 rule 1). Minor: `e(n)` counts keys and values (§2.5), yet `Store.Usage` (5 fields) charges `e(8)`, `World.Here` (7 fields) `e(9)` and `World.Clock` (7 fields) `e(7)`. Count fields in one consistent way even though the figures are provisional (OPEN-1). |
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

- **Limits.** Every numeric limit is TM's, cited as "TM L*n*". A number repeated here next
  to its citation is for the reader only; if it differs from TM §10, TM wins and this spec
  has a defect.
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
(§9.2), never a formula, a rounding, a threshold or a format (LEGAL §18.3 rule 1, OPEN-3).

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
| Every numeric limit (§11 and wherever a number is cited as TM L*n*) | TM §10, from D17 §8 and TM's changes | Decision (TM, normative) |
| The rules for D17's open items 1 (composition part), 2, 4, 5, 6 and 8 and the "other improvements" this slice ships: composition coverage (§7.10), solid presses and gesture-window focus (§7.6–§7.7), the liveness floor and the gate's clock origin (§2.6), entity filtering (§10.3), `Store` read-your-writes, reservations, disk rates and the no-follow store directory (§5), world labels (§7.6), log format (§4.3), the ownership badge (§7.7), selection and copy (§7.8), image decodes (§7.10), the sticky abort (§2.4), split turns, their place in the round robin and gesture windows (§2.6), the ext profile's library tables (§2.11), revocation and the grant-store re-read (§2.9) | TM §7.1, §7.2, §7.4, §7.5, §7.6, §7.8, §8.1, §8.3–§8.5, §8.7, §8.8, §8.10–§8.15, §8.23, §8.24 | Decision (TM, normative); the script-visible form is this spec's |
| The ext environment: the globals and libraries of an apartment, `print` to the script log, `tostring` without addresses, per-apartment random numbers (§2.11); the process locale pinned to "C" and the suspension when it is not (§2.9) | D17 §1.2, §1.5 | Decision (D17) |
| Activation and focus cases 1 and 2, the visible-and-uncovered focus rule, the ext z-range, pointer notifications only over the script's own catching nodes (§7.6–§7.7) | D17 §4, §5.6, as TM §7.2 tightens them | Decision (D17, TM) |
| Ext keymap names `ext.<id>.<name>`, contexts in the ext partition, resolution after every non-ext context (§8) | D17 §2, §5.5; KM §2, §3, §7 | Decision (D17); KM rules as KM marks them |
| Region frame: f64 (160 · X + x, 160 · Y + y, z), +X east, +Y north, +Z up, unit = 1 m by convention; yaw counter-clockwise from +X (§2.10) | ARCH §9.1; `coordinates.md` §2–§3; KM §9.1 | as marked there (Confirmed; KM Decision) |
| Function names, argument shapes, result tables, notification names, element kinds and their properties, reason strings, provisional unit costs, the handler pattern, `"not declared"` and the ext context priority (closing EM OPEN O-2 and O-3), and every rule marked **(1a rule)** | This spec, 2026-10-07 (revisions 2 and 3 the same day): designed to satisfy D17 and to state TM's rules in script-visible form | Decision (this spec; subject to review) |

---

<a id="s2"></a>
## 2. Conventions

<a id="s2-1"></a>
### 2.1 Terms

| Term | Meaning |
|---|---|
| **Script** | One loaded ext script: one apartment (D17 §1.2) for one (folder kind, id). |
| **Call** | One metered entry into the script's Lua code: the top-level run, the filter call (§10.3), one handler call, one timer call, `Tick`, the `Unload` handler, and (slice 1b, developer mode) one developer REPL run (EA1b §9.4, TM L85), as TM §2 lists them. Each call has 800,000 work units and a 4 ms wall-clock deadline (TM L3, L4). |
| **Turn** | The script's share of 6b for one round-robin visit (TM §2): its filter call (§10.3), its queued notifications in order, its due timers, then `Tick`. Every item of a turn is its own call. A turn can be **split** between frames (§2.6, TM §8.13). |
| **Notification** | A queued event for the script, delivered as one call in a turn (D17 §4, §2.8). |
| **Intent** | A queued effect that a later frame step applies (D17 §4). In slice 1a the only intents are `Input` context changes (§8). |
| **Gesture** | One of the user's own inputs that D17 roots trust in (TM §2): a **solid press** (§7.6) on one of the script's frames, the start of one of its own actions (§8.2), or an `/ext:` line for it (§6). |
| **Gesture window** | The whole turn that delivers a gesture, from the start of the call that delivers the gesture's notification to the end of that turn's `Tick`, whatever frames a split turn spans (TM §2, §8.13). Every API spec uses exactly this window. |
| **Solid press** | A user press whose catching node draws its own background at the press point with alpha × effective opacity ≥ 0.5 (TM rule 7.2.1, L57; §7.6 says what "own background" covers for each kind). |
| **Node** | One element of the script's UI: a frame, a control inside a frame, or a world label (§7). |
| **Frame** | A top-level ext window (element kind `Frame`). The word "window" in D17 means a frame. |
| **Catching node** | A node that catches the pointer at a point (§7.6). D17 and UB call this "mouse-visible". |
| **UI unit** | One logical pixel at UI scale 1.0. The host multiplies by the user's UI scale. Origin at the top-left of the game window's client area, +x right, +y down. |
| **Region frame** | Positions as f64 (x, y, z) in metres: x = 160 · X + local x, y = 160 · Y + local y, z up, in **the player's current region** (ARCH §9.1). |
| **Resident** | Present in memory now: a landblock streamed and integrated, a collision set installed, an entity in the store (D17 §8, "Natives never wait"). |
| **Screen**, **coverage** | As TM §2 defines them: one full viewport in physical pixels (in deterministic mode the harness's virtual surface, 1920 × 1080 unless the walk file sets another, EM §11.5), and a conservative pixel count divided by one screen. |
| **Steps** | The frame steps of ARCH §20.3 as IB D8 and D17 §4 refine them: step 1 input; steps 2–6 session, simulation, streaming, completions, animation; step 7 items 1–6 (UI tick and plugins), then **6a** (ext queue), **6b** (ext tick), **6c** (ext collector), then item 7 (the outbox drain, "**item-7 drain**"); step 8 layout and paint; step 10 render. |

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
  script holds always re-checks the grant (§2.9).

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
| `position` | three numbers `x, y, z` passed as separate arguments (region frame, §2.1), each with absolute value at most 10^7 | non-finite or larger values |
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

1. **Lua error** (catchable with `pcall`): a programming error. Wrong type, a value out of
   range, an unknown name (property, context, action, setting), a call on a stale handle
   (§2.7), a call on a module whose grant was revoked (§2.9), a ray longer than 500 m, a
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
| `e(n)` | 20 · n | `n` elements (keys and values) of a table built or walked |
| `h(n)` | 50 · n | `n` handles returned (the interned handle lookup of §2.7) |
| `N` | 2,000 | Creating one node (element-tree insert, accounting) |
| `P(n)` | 100 · n | Setting or reading `n` node properties (plus `b` for strings) |
| `Q` | 500 | One point query of `World` |
| `V` | 295,112 | The reservation of one maximal value: `e(8,192)` + `b(1 MiB)` + `B` = 163,840 + 131,072 + 200 (TM L34, L35). Refunded down to the real size after the walk |

<a id="s2-6"></a>
### 2.6 Timing

| "Applies" value | Meaning |
|---|---|
| **at once** | Inside the call. A getter called right after returns the new state (D17 §4, "Notifications a script causes itself"). |
| **next turn** | A notification the change causes is appended to the script's queue and delivered in its **next** turn, normally the next frame's 6b, never re-entrantly in the same call or turn. |
| **same-frame 6b** | A notification produced by native work before 6b of this frame (step 1 routing of the user's input; the hit test and hover refresh of step 7 item 1) is delivered in this frame's turn (D17 §4). |
| **item-7 drain** | Queued as an intent and applied at IB D8's item 7 of this frame, with the band requests (D17 §4, intents table). |
| **step 8** | Laid out and painted in step 8 of this frame. |
| **worker** | Queued as a job on an asset worker; its result is a notification delivered in the first turn after the job completes. |

**Split turns** (TM §8.13). D17 §4 checks the 2 ms per-frame cap (TM L5) before each call.
When the cap or the gate stops a script's turn partway, the rest of that turn resumes in the
next frame that reaches that script, at the first call not yet made (undelivered
notifications of the turn's delivery set in order, §2.8, then timers not yet run, then
`Tick`), **before** any notification that arose later.

**Where a split turn resumes in the round robin** (TM §8.13). The round robin visits the
groups in order (lease holders first, then the others; D17 §4) and rotates within each
group. A split turn may resume as the **first script of its group at most once**: in the
next frame that runs 6b, it goes before the group's ordinary rotation. If that resumed
part is split again, the rest of the turn waits until the ordinary rotation next reaches
the script, and never takes the head again. So a script whose own calls fill the 2 ms cap
every frame (for example a 1,024-entry delivery set) runs first in at most one frame of
that turn, and every other script of its group gets its turn in rotation (TM X17). A script resumed at the head is not visited again at its rotation place in that
frame, and the rotation itself (D17 §4: it starts one script later each frame) is not
changed by the resumption.

`Tick`'s `dt` is the time since the last `Tick` the script received, so a split or skipped
turn gives the summed time. A gesture's **gesture window** (§2.1) is the whole delivering
turn, across the frames a split turn spans, so a split turn never loses a gesture's effect
(TM X89).

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
  still returns the kind (D17 §3). Batched reads (§10) answer `false` for a stale entry
  instead of raising.
- **Identity (1a rule).** Within one apartment the host returns **the same userdata** for the
  same object for as long as the script holds a reference to it, so handles compare with
  `==` and serve as table keys. The host keeps the mapping in a weak table charged to the
  script's memory (`h(n)` covers the lookup). A handle the script has dropped may come back
  as a new userdata later; it still compares equal only to handles held at the same time.
- **Owned by the apartment.** A handle passed to another apartment is impossible (there is no
  route), and a handle kept across an unload is gone with its apartment.

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
  set when it arises, with one exception: a **gesture** (§2.1) is always queued, so that its
  gesture window opens at its place in the turn; without a handler its delivery makes no
  call. Per-request callbacks are always queued.
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
  together (TM L30). **State notifications** (§7.5 marks them) are coalesced per node and kind
  to one entry with the latest value (outside the current delivery set, above); the handler may therefore see a value equal to the one
  it saw last. While the queue is full, new notifications are dropped and counted per kind,
  and the next delivered notification of that kind carries `dropped = n` (D17 §8).
- **Suspension.** A script is suspended by exactly these causes: the watchdog (the time
  throttle or three aborts, TM L17, L18), the panic chord (D17 §5.5), a grant revocation
  by any route (§2.9, TM §8.23) and a process locale other than "C" (§2.9, D17 §1.5). A
  suspended script accrues nothing: no notifications, no timers, no subscription events.
  Its pending notifications are dropped with counts, and its first turn after the user
  resumes it begins with `Resumed` (§4.1) carrying the counts and the cause (D17 §4).

<a id="s2-9"></a>
### 2.9 Capability checks

- `Ext.Use(name, 1)` hands out a module table only for a granted capability (§3).
- Every native of a granted module **re-checks the grant and the backend class on every
  call** (D17 §5.1), so a module table kept in a global, a closure or a table fails closed.
- **Revocation** (TM §8.23). Revoking a grant of a loaded script, required or optional, by
  any route, suspends it in that frame: pending intents are discarded and nothing accrues.
  If the revoked capability is required, the script cannot be resumed and is unloaded (a
  close item, no `Unload` handler; D17 §2, "Refusal is whole"). If it is optional, the user
  may resume the script; from then on every function of the revoked module raises a Lua
  error `"not granted"`, `Ext.Has` answers `false` for it, and the host has removed what the
  module made **(1a rule for what is removed)**: for `ui`, every node and world label, whose
  handles are stale; for `input`, the contexts are deactivated and the action handlers
  cleared; for `entity.read`, every subscription, whose handles are stale (entity handles
  stay valid objects but every method raises `"not granted"`); for `world.read`, nothing.
  Escape test: TM X91.
- **The grant-store re-read is a route** (TM §8.23). When the script manager re-reads
  `ext-grants.toml` (EM §8.5) and the result leaves a **loaded** script with fewer grants
  than it runs with (its record removed, made invalid under EM §8.3, or narrowed by a
  capability), that is a revocation: the script is suspended in the frame the re-read
  result is applied, exactly as above, required and optional alike. A re-read that would
  **widen** a loaded script's grants changes nothing while it runs: `Ext.Has` and
  `Ext.Use` keep answering from the grants it was loaded with, and the wider record is
  honoured only at its next load (EM §9.2). A record that breaks EM §8.3's rules grants
  nothing.
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
| `"Resumed"` | `{counts, reason}` | The first call of the first turn after the user resumes the suspended script. `counts` maps each dropped notification kind (for example `"TextChanged"`, `"command"`, `"timer"`, and `"entity"`, the subscription records skipped while suspended, §10.3) to the number dropped; `reason` is the cause of the suspension (§2.8): `"panic"`, `"watchdog"` (the time throttle or three aborts, TM L17, L18), `"grant"` (a revocation by any route, D17 §4, TM §8.23, §2.9) or `"locale"` (the process locale was not "C", D17 §1.5, §2.9). |
| `"SettingsChanged"` | `{keys}` | In the turn after the user applies a change on the script's settings page; `keys` is an array of the changed setting keys in manifest order. |

<a id="s4-2"></a>
### 4.2 Clock and timers

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Core.Time()` | number | `B` | read | Seconds from the start of the frame in which the script's top-level run ran, to the start of this frame (step 1). **One value per frame**: every call in one frame returns the same number (D17 §3). Never decreases. In deterministic mode (`scripttool test`) it is the sum of the walk file's frame times. |
| `Core.Frame()` | integer | `B` | read | Frames since the frame of the top-level run (that frame is 0). |
| `Core.Remaining()` | integer | `B` | read | Work units left in the current call: the smaller of the units left and the remaining wall time ÷ 5 ns. In deterministic mode, units only. Lets a script cut long work into pieces. **(1a rule)** |
| `Core.After(seconds, fn)` | timer \| `nil, "limit"` | `B` + `h(1)` | at once | A one-shot timer. `seconds` ≥ 0 and ≤ 10^6. `fn(timer)` runs in the first turn whose `Core.Time()` is at least the creation time + `seconds`, and never in the turn that created it. |
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
- **Limits** (TM L45): 64 KiB per second per script, beyond which records are dropped and
  counted, then one host record "N records dropped" when the rate allows; one file of at most
  1 MiB rotated once. The developer console shows records as EA1b §9.2 and TM §8.1 say.

<a id="s4-4"></a>
### 4.4 Settings and identity

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Core.Setting(key)` | boolean \| number \| string | `B` + `b(n)` | read | The current value of a manifest `[[settings]]` entry: `bool` → boolean, `number` → number in its range, `choice` → the chosen option's string, `text` → string within its length cap. An unknown key raises. Values are set only by the user; no call writes them (D17 §5.2). |
| `Core.Settings()` | table | `B` + `e(n)` + `b(bytes)` | read | All settings as a new table key → value. |
| `Core.Script()` | table | `B` + `b(bytes)` | read | `{id, kind, name, version, api}`: the script id, `"installed"` or `"developer"`, the manifest's `name` and `version`, and the `Ext` major it targets. No path, no user or host name (D17 §5.6). |

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
  (EM §8.8), lists the key files and builds the **file index**: key → counted size of its
  file. The file index is charged to the memory cap of **every apartment that has the store
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
  - if the key has a pending entry, the answer is a **copy of that entry taken at the call**;
    a pending deletion answers "absent" (`fn(nil)`). No worker job and no file read is made;
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
  newer `Set` moves the entry's charge to the apartment that made it.

<a id="s5-3"></a>
### 5.3 Functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Store.Get(key, fn)` | `true` \| `nil, reason` | `B` + `b(key)`; at delivery, the conversion `e(n)` + `b(bytes)` charged first in the delivery call | the pending map (`fn` in the next turn) or worker (`fn` in the first turn after the job completes), §5.2 | `fn(value)` on success, with `nil` when the key does not exist or its deletion is pending; `fn(nil, reason)` on failure with `"invalid"`, `"too large"` or `"io"`. Reasons at the call: `"busy"` (the read rate, TM L42, or the outstanding-request cap, TM L27), `"out of memory"` (the result reservation, below), `"reserved key"`. A refusal queues nothing. |
| `Store.Set(key, value [, done])` | `true` \| `nil, reason` | `B` + `b(key)` + reservation `V`, refunded to the real walk | at once in the pending map; file within about 1 s (worker) | Serialises the value **inside this call** (D17 §8, "Value conversion is not a residual"). Reasons: `"too large"`, `"cyclic"`, `"invalid"`, `"full"`, `"busy"` (the write buckets, below), `"reserved key"`. A refusal changes nothing and debits nothing. `value = nil` is `Delete`. `done(true)` is delivered in the turn after this value, or a later value of the same key, is on disk; `done(false, "io")` if that write failed. A `done` of a value replaced before its write is called when the replacing write completes. |
| `Store.Delete(key [, done])` | `true` \| `nil, reason` | `B` + `b(key)` | as `Set` | Removes the key's file. Deleting an absent key succeeds. |
| `Store.Has(key)` | boolean | `B` + `b(key)` | read (index) | The pending map over the file index. |
| `Store.Keys()` | array of strings | `B` + `e(n)` + `b(bytes)` | read (index) | Every key, sorted byte-wise. |
| `Store.Usage()` | table | `B` + `e(8)` | read (index) | `{keys, bytes, max_keys, max_bytes, pending}`: keys held, counted bytes (each key file, pending entry and temporary file counted as TM §8.7 says), the caps (TM L40, L41: 1,024 and 16 MiB), and the number of pending entries. |

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
- **Result reservations** (TM §8.14, L28). A `Get` reserves, against the script's memory cap
  at the call, the stored size of the value (from the pending entry, otherwise from the file
  index) times the neutral reader's declared expansion ratio (TM OPEN O-4), never the 1 MiB
  maximum; a key with neither reserves nothing. A reservation the cap refuses answers
  `nil, "out of memory"` (**returned, not raised**) and queues nothing. So 32 outstanding
  reads of small values do not exhaust a 32 MiB script, and the 33rd answers `nil, "busy"`
  (TM X25).
- **Space** (TM L40, L41): 1,024 keys and 16 MiB per script, 16,384 keys and 256 MiB for all
  scripts with orphans included; temporary files of flushes in progress and pending entries
  count. A write refused by the all-scripts total is reported in the script manager naming
  the orphans (D17 §5.2).
- **Lifetime.** The store belongs to the (kind, id)'s grant record and carries its owner tag
  (TM §8.24): it survives unloads, updates with the same `author`, and orphaning; it is
  wiped when the `author` changes, when its owner tag does not match the loading script's
  `author`, when the user resets it, or before a `session.server` holder opens a local-only
  store (D17 §5.2). Slice 1a has no local-only capability; the I/O-queue test of D17 §6.6
  (TM X63) uses a test-only one.
- **At unload** every pending value is handed to the final flush (D17 §8), including values
  set in the `Unload` handler; `done` callbacks are not called after the apartment is gone.
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
| `Commands.Register(name, fn [, help])` | `true` \| `nil, "limit"` | `B` + `b(help)` | at once (the next step 1 dispatches to it) | `name`: KM name grammar without `.`, 1–32 bytes of `a-z`, `0-9`, `_` (D17 §3); outside it raises. Registering an existing name replaces its handler and help. `help`: a string of at most 96 bytes shown by `/ext:help <id>` (longer raises). At most 32 commands per script (TM L68). |
| `Commands.Unregister(name)` | nothing | `B` | at once | An unknown name does nothing. |
| `Commands.List()` | array of strings | `B` + `e(n)` + `b(bytes)` | read | Registered names, sorted. |

<a id="s6-2"></a>
### 6.2 Dispatch and delivery

- **Matching.** `ext:`, the id and the command name are matched ASCII case-insensitively, so
  `/EXT:Notes Add milk` reaches the command `add` of script `notes` (D17 §3).
- **Arguments.** The text after the command word, with leading and trailing ASCII
  whitespace removed and invalid UTF-8 replaced; at most 1,024 bytes, cut at a UTF-8
  boundary beyond that **(1a rule)**. The host does not split it: the script parses it.
  Arguments may come from a shortcut a plugin built, so a handler treats them as hostile
  input (D17 §7).
- **The call.** `fn(args, info)` in the script's turn of the same frame (the dispatch is
  native work in step 1, delivered **same-frame 6b**). `info` = `{command, truncated}`.
  The line is a **gesture**: the turn that delivers it is a gesture window (§2.1, §7.7).
- **No command word.** `/ext:<id>` alone opens the script manager at the script's help page,
  as `/ext:help <id>` does; the script gets nothing **(1a rule)**.
- **Unknown or unloaded.** A line naming no loaded script, or a command the script has not
  registered at the time of dispatch, shows a notice in the reserved layer (its text through
  TM §8.2, L66, L67) and a script-log entry, never a chat line (D17 §3; TM X73). A command unregistered between dispatch and delivery is
  dropped with a log entry.
- **Queue.** Command lines are notifications: they count toward the 1,024-entry queue and are
  never coalesced.
- **Management words.** `/ext:list`, `/ext:reload <id>` and `/ext:help [<id>]` belong to the
  host; ids `list`, `reload` and `help` are reserved (D17 §2–§3). Their output goes to the
  script manager and, in developer mode, the developer console, never to chat (D17 §3).
  `/ext:reload` works only in developer mode; with developer mode off it is refused with a
  reserved-layer notice "Developer mode is off" (D17 §3, EA1b §8.1). In developer mode,
  `/ext:reload` of an id that is not loaded queues nothing and shows a reserved-layer
  notice that the script is not loaded; a reload never starts a first load **(1a rule)**.
  Which reloads are hot or full is EA1b §8's. No management word is delivered to any
  script.

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
  are ordered by the last user press on any frame of that script (most recent on top); a newly
  loaded script's block goes on top. Inside a block, frames are ordered by the script's own
  `Raise`, shows and the user's presses. A script's call can reorder only its own block, so no
  script can raise its frame over another script's frame.
- **World labels** lie below every ext frame, in the order they were created (§7.9).
- **Ownership badges** (§7.7) are the top layer of the ext range: every badge is drawn above
  every ext frame of every script, badges among themselves in the z-order of their frames
  (TM §8.3). Plugin windows, native panels, the developer overlay and the reserved layer
  stay above them.
- **Units.** Geometry is in UI units (§2.1). Frame positions are relative to the screen;
  control positions are relative to their parent's top-left corner. Children are clipped to
  their parent's box.
- **Changes apply at once** in the element tree; they are painted in step 8 of this frame
  (D17 §4, intents table). Geometry a getter returns is the geometry the next hit test uses
  (IB D8, "Geometry used for hit tests").
- **While a consent prompt shows**, ext frames and world labels are neither drawn nor
  hit-tested; the script keeps running and its nodes keep their state; nothing is notified
  (D17 §4).

<a id="s7-2"></a>
### 7.2 Module functions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `UI.Frame(props)` | frame \| `nil, "limit"` | `B` + `N` + `P(n)` + `b(bytes)` | at once; painted step 8 | Creates a frame from the property table (§7.4, §7.5). It starts **hidden** unless `props.visible` is `true`, in which case it is shown under the rules of `frame:Show()`. `"limit"`: the element counts (§7.10), the ext surface pool, or, when `visible` is `true`, the composition pool or a coverage bound (§7.10); then nothing is created. |
| `UI.WorldLabel(props)` | label \| `nil, "limit"` | `B` + `N` + `P(n)` + `b(bytes)` | at once; placed step 8 | §7.9. `"limit"`: the element counts, or, for a shown label, a coverage bound (§7.10); then nothing is created. |
| `UI.Screen()` | width, height, scale | `B` | read | The game window's client area in UI units and the user's UI scale. |
| `UI.On(name, fn)` | nothing | `B` | at once | Sets or clears the handler of `UI`'s one module notification (§2.8, form 1): `"ScreenChanged"`, `fn({width, height, scale, by = "host"})` in the turn after the screen size or UI scale changes. State notification. Any other name raises. |
| `UI.MeasureText(text, font [, wrap])` | width, height | `B` + `b(text)` | read | The box `text` needs in `font` (§7.4); with `wrap` (a width in UI units) it wraps at word boundaries as a `Text` node does. Results depend on the fonts available (OPEN-6) and are not stable across installs. Text over 16 KiB raises. |
| `UI.Frames()` | array of frames | `B` + `h(n)` | read | The script's frames in their z-order, bottom first. |
| `UI.Usage()` | table | `B` + `e(12)` | read | `{nodes, nodes_max, labels, labels_max, area, area_max}`: the script's node and label counts and caps (§7.10), and its composition coverage and its bound in screens (§7.10, TM L55). |

<a id="s7-3"></a>
### 7.3 Node methods (every kind)

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `node:Set(props)` | `true` \| `false [, "busy"]` | `B` + `P(n)` + `b(bytes)` | at once | Sets the listed properties in one step; an unknown property or a value of the wrong type raises, and then nothing is changed. Returns `false`, changing nothing, when the change would pass a composition-coverage bound or the composition pool (§7.10; no reason, the script log says why), and `false, "busy"`, changing nothing, when it sets an `Image`'s `file` while the outstanding-request cap is reached (§7.10, TM §8.5). |
| `node:Get(name)` | the property's value | `B` + `P(1)` + `b(bytes)` | read | The current value; geometry after this frame's layout of that node (§7.4). |
| `node:On(name, fn)` | nothing | `B` | at once | Sets or clears (`fn = nil`) the handler of a notification the kind has (§7.5; §2.8, form 2). A name the kind does not have raises. `fn(node, args)`. |
| `node:Add(kind, props)` | node \| `nil, "limit"` \| `nil, "busy"` | `B` + `N` + `P(n)` + `b(bytes)` | at once | Creates a child; `"busy"` only for an `Image` with a `file` while the outstanding-request cap is reached (§7.10), and then nothing is created. Only `Frame`, `Group` and `Scroll` hold children; any other parent raises. `kind` is a kind name of §7.5 other than `Frame` and `WorldLabel`. |
| `node:Remove()` | nothing | `B` + `N` per node removed | at once | Removes the node and its descendants; their handles become stale. Removing the focused `Field` or the active frame applies §7.7. |
| `node:Children()` | array of nodes | `B` + `h(n)` | read | In drawing order, bottom first. |
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
| `x`, `y` | number, \|v\| ≤ 16,384 | 0 | Position relative to the parent (the screen for a frame). A `Group` with a layout (below) sets its children's `x` or `y`. |
| `width`, `height` | number, 0 … 16,384 | kind's default | Size. Changing it causes `Resized` (state). |
| `visible` | boolean | `true` (controls), `false` (frames, until shown) | Whether the node and its subtree are shown. A frame's `visible` is set through `Show`/`Hide` or `Set`, with the same rules. Changing it causes `VisibilityChanged` (state). |
| `opacity` | number 0 … 1 | 1 | Multiplies the node's and its subtree's drawing. The **effective opacity** is the product along the path from the frame. |
| `enabled` | boolean | `true` | A disabled node is drawn dimmed, still catches the pointer if it would otherwise, but has no native behaviour and sends no pointer notifications. |
| `background` | colour or `nil` | `nil` | Fill of the node's box. |
| `border` | colour or `nil` | `nil` | A 1-UI-unit frame line inside the box. |
| `catch` | boolean | `true` for `Frame`, `PushButton`, `Toggle`, `Slider`, `Field`, `List`, `Scroll`; `false` for `Group`, `Text`, `Gauge`, `Image` | Whether the node catches the pointer (§7.6). |
| `name` | string ≤ 64 bytes or `nil` | `nil` | A free label for the script's own use; the host never shows it. |

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
has `by` (§2.8). The args of `Pressed`, `Released` and `Clicked` also have `gesture`
(§7.6).

| Kind | Own properties (type, default) | Native behaviour | Own notifications |
|---|---|---|---|
| `Frame` | `movable` (boolean, `false`); `resizable` (boolean, `false`); `min_width`, `min_height` (number, 48, 32); `max_width`, `max_height` (number, the screen) | A frame that is `movable` follows a drag that starts with the primary button on a point where the frame itself is the catching node (not a catching descendant) and moves at least 4 UI units; it is kept so that its top 24 units and at least 24 units of its width stay on screen. A `resizable` frame gets a host-drawn grip of 12 × 12 units in its bottom-right corner; dragging it resizes within the min and max, and stops growing at the largest size that keeps both composition-coverage bounds (§7.10, TM rule 7.1.5). A drag that moves a frame is never limited by coverage. | `ActiveChanged` (state, `{active}`) |
| `Group` | `layout` (`"none"`, `"column"`, `"row"`; `"none"`); `gap` (number ≥ 0, 0); `padding` (number ≥ 0, 0) | A container. With `"column"`, each shown child in order gets `y` = `padding` for the first and the previous child's `y` + `height` + `gap` for the next; its `x` stays as set. `"row"` does the same along `x`. A child's own `y` (column) or `x` (row) is then overwritten, and `Moved` reports it. | — |
| `Text` | `text` (string ≤ 16 KiB, `""`); `wrap` (boolean, `false`) + text properties | Draws the text in the box, clipped; with `wrap`, breaks lines at spaces, or inside a word that does not fit. No editing, no selection. | — |
| `Field` | `text` (string, `""`); `max_bytes` (integer 1 … 4,096, 256); `multiline` (boolean, `false`); `placeholder` (string ≤ 256 bytes, `""`); `read_only` (boolean, `false`) + text properties | Single- or multi-line text entry with the editing behaviour of §7.8. Takes the keyboard focus only under §7.7. | `TextChanged` (state, `{text}`); `FocusChanged` (state, `{focused}`); `Submitted` (`{text}`, single-line only) |
| `PushButton` | `text` (string ≤ 256 bytes, `""`) + text properties | Drawn by the host in the ext theme with a hover and a pressed look. | (its `Clicked` is the pointer notification) |
| `Toggle` | `text` (string ≤ 256 bytes); `checked` (boolean, `false`) + text properties | A box with a check mark and its text; a `Clicked` with the primary button flips `checked`. | `Toggled` (state, `{checked}`) |
| `Slider` | `min`, `max` (number, 0, 1; `min` < `max`); `step` (number ≥ 0, 0 = continuous); `value` (number, clamped into [min, max] and rounded to `min` + k · `step`); `vertical` (boolean, `false`) | A track and a thumb drawn by the host. A press on the track moves the thumb to the point; a drag moves it. No wheel and no keys. | `ValueChanged` (state, `{value}`) |
| `Gauge` | `value` (number 0 … 1, 0); `fill` (colour, theme); `vertical` (boolean, `false`) | A bar filled to `value` of its length, from the left (or the bottom). No input. | — |
| `Image` | `file` (string: a path relative to the script folder, `nil`); `tint` (colour, `#FFFFFFFF`); `fit` (`"stretch"`, `"contain"`, `"none"`; `"stretch"`) | Shows a picture from the script's folder snapshot (§7.10, "Images"). Draws nothing until decoded. | `Loaded` (`{ok, reason}`; `reason` `"too large"`, `"invalid"` or `"limit"` when `ok` is `false`) |
| `List` | `rows` (array of strings, at most 1,024 rows of at most 256 bytes; `{}`); `selected` (integer 1 … #rows or `nil`); `offset` (number ≥ 0, 0) + text properties | Rows of one line each, row height = the font's line height + 4 units, scrolled by `offset` (UI units). A primary-button click on a row selects it. While the pointer is over it, the wheel scrolls it by 3 rows per notch instead of zooming the camera (D17 §4, "The wheel over ext scrollables"). A host-drawn thumb on the right edge when the rows overflow; dragging it scrolls. | `SelectionChanged` (state, `{index}`); `Scrolled` (state, `{offset}`) |
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
    `Released` carries its press's value;
  - `Clicked` (`{button, x, y, gesture}`): a press and a release of the same button both on
    the same node, with no drag of a frame or slider in between; `gesture` as its press.
- **Capture.** After a press on a node, that node receives the pointer's moves and its
  release until the button is released, even outside its box (its coordinates may then lie
  outside the box).
- **Solid presses** (TM §7.2, D17 open item 2; L57). A user's press on an ext frame is a
  **solid press** only when the catching node at the press draws **its own background** at
  that point with alpha × effective opacity ≥ 0.5, where effective opacity is the product of
  the node's `opacity` and every ancestor's, the frame's included. "Its own background" is
  **(1a rule, the per-kind form of TM rule 7.2.1)**:
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
step 1, as plugin time; a solid press on a `Field` that passes the visible-focus rule below
also focuses it. A press elsewhere (the world, a plugin window, a native panel, another
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
§8.13, X89). A timer or `Tick` of a later turn does not qualify. There is no exception for
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
   developer overlay, no surface of the reserved layer and **no frame of another script
   (1a rule)** draws anything inside it or catches the pointer there. The transparent,
   non-catching area of a full-screen native root does not cover; world labels never cover
   (TM §7.8).

The host checks the rule again at the end of every step that can change it (step 1's user
drags and toggles, step 7's plugin items, 6b, the item-7 drain, step 8's layout). A field
that fails it loses the focus at once, as a hide does; from that moment no key is routed to
it (D17 §4; TM X39).

**What the host draws, not the script** (D17 §4; TM §8.3):

- the caret, and a focus mark (a border in Tessera's focus colour) around the focused field,
  drawn over the script's own drawing;
- an **ownership badge** on **every shown frame that has at least one catching node (the
  frame itself included) or any `Field`**: the script id and the words "ext script" (for
  example "notes · ext script") in Tessera's fixed badge style, attached outside the frame's
  top edge (inside the frame when that edge is off screen), at full opacity whatever the
  frame's opacity. It is shown whenever the frame is shown, not only while a field has
  focus, and is drawn in Tessera's focus colour as well while a `Field` of that frame has
  the keyboard focus. Badges are the top layer of the ext range (§7.1), so no ext frame of
  any script can be placed over one. The script cannot style, move, cover, hide or measure
  the badge; it is not a node, never a pointer target, and not part of any `node:Box()`.
  Its area counts toward the frame's composition coverage (§7.10). Escape test: TM X49.

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
  (`opaque_keys`, IB D6); ext and game bindings do not fire while the user types (D17 §5.5).
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
  copy or cut copies only text the user selected, and only if the field's text has not
  changed since the selection was made; otherwise it does nothing (TM X52).
- **Notifications.** `TextChanged` (state) after the user's edits, at most one per frame with
  the latest text; also after the script's own `Set{text = …}`, in its next turn with
  `by = "script"`.

<a id="s7-9"></a>
### 7.9 World labels

A world label is a short text placed at a point of the world.

| Property | Type | Default | Meaning |
|---|---|---|---|
| `text` | string ≤ 256 bytes | `""` | One line; cut at the end with an ellipsis when wider than 512 UI units. |
| `x`, `y`, `z` | number (region frame) | required | The point. |
| `region` | integer 1 … 255 | the player's region at creation | The label is shown only while the player is in this region. |
| `offset_x`, `offset_y` | number, \|v\| ≤ 256 | 0, 0 | A screen offset in UI units after projection. |
| `max_distance` | number 1 … 500 | 100 | Hidden beyond this distance (metres) from the camera. |
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
- Methods: `Set`, `Get`, `Remove`, `Exists`, `Kind` and `On` (only `VisibilityChanged`, for
  the script's own `visible` changes; projection hiding is not notified).

<a id="s7-10"></a>
### 7.10 Limits and pools

| Limit | Value | When reached |
|---|---|---|
| Nodes (frames, controls, labels) | TM L30: 2,048 per script, of which at most 512 in one frame and 256 world labels; 8,192 for all ext scripts, of which 1,024 world labels | creation answers `nil, "limit"` |
| Ext surfaces, composition entries, UI memory | TM L49–L51: 128 surfaces, 256 entries and a quarter of each memory pool per script; 512, 1,024 and 32/64/64 MiB for all, on top of UR's caps | creation answers `nil, "limit"`; a show answers `false` |
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
- **Counting.** Each entry counts min(one screen, its displayed rectangle counted outward),
  in physical pixels (UI units × the UI scale) divided by one screen, **unclipped and
  independent of its position** (TM rule 7.1.2). So moving a frame or a label, by the
  script, by the user's drag or by step 8's projection, never changes coverage, and a frame
  created mostly off screen costs as much as one on screen. `UI.Usage().area` reports the
  script's sum.
- **Check points** (TM rule 7.1.5): a frame or label created shown, `Show`, `visible = true`
  on a frame or label, a change of a shown frame's `width` or `height` or of a shown label's
  displayed size, a frame gaining its badge (its first catching node or `Field`, or `catch`
  turned on), and every screen resize or UI-scale change. Position changes are not checked.
- **A script call** that would take the script past 1.25 screens or all ext scripts past 2.0
  is refused and changes nothing: `Show` returns `false` and the frame stays hidden; a
  creation answers `nil, "limit"`; a `Set` returns `false`. The script log says why.
- **A user resize** of a `resizable` frame stops at the largest size that keeps both bounds;
  a user drag is never limited.
- **A host change** (a screen resize, a UI-scale change) that puts a script or the total
  over its bound hides ext frames and labels, across all scripts, in **reverse order of
  their last show** (the most recently shown first) until every script and the total fit.
  Each hidden node's script gets `VisibilityChanged{visible = false, by = "host"}` in its
  next turn (TM rule 7.1.5 calls it `VisibleChanged`), and the script log records it.
- **Frame budget** (TM rule 7.1.6, L56). The GPU time of ext composition at these bounds is
  part of the "Ext overlays and composition" row (1.0 ms of GPU time for overlays and
  composition together, measured as E9 at the reference display's native 3840 × 2160;
  slice 1a measures the composition part). If E9 measures more, the bounds shrink in
  proportion (TM OPEN O-1). While the row's funding is open (TM OPEN O-2) the ext Lua
  ceiling of slice 1a is 12 ms (TM L12).
- Escape test: TM X31 (with E9).

**Images** (TM §8.5). `file` names a file of the script's folder snapshot (D17 §2); a missing
file or a path outside the folder grammar raises. Decoding runs on an asset worker at the
lowest priority, as an **outstanding worker request** of the script (TM L27: 32 per script,
256 in all). A creation of an `Image` with a `file`, or a `Set` that changes its `file`,
that would pass that cap answers `nil, "busy"` (creation) or `false, "busy"` (`Set`) **at
the call** and creates or changes nothing; no decode is queued for later. Requests for the
same file of the same script are joined. Before any pixel is decoded the header is
checked, an image over 4,096 pixels on an axis is refused (TM L48), and its decoded size is
charged to the script's share of ext UI memory (TM L51); `Loaded{ok = false, reason}`
reports `"too large"`, `"invalid"` (a broken or unsupported file) or `"limit"`. The formats
accepted are those of OPEN-5. A successful decode delivers `Loaded{ok = true}`. Escape
tests: TM X26, X33.

<a id="s7-11"></a>
### 7.11 Suspension, unload and the panic chord

- When a script is suspended (watchdog, panic chord, grant revocation, a locale other than
  "C"; §2.8), the host hides all its frames and labels at once; its focused field loses the focus and its frame is
  deactivated. Nothing is notified while it is suspended (§2.8). When the user resumes it,
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
  control of its own that hides the frame; nothing in this spec guarantees one, so the
  grant dialog's text and the user documentation name the chord and the manager.
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
| `Input.Activate(context)` | `true` \| `nil, "busy"` | `B` | item-7 drain | `context` is a short name the manifest declares; another name raises. Rate: 4 activations or deactivations per script per second (TM L62); changes of one frame are coalesced, only the last state of each context is applied, and all ext registry changes of one frame are applied as one recompilation (TM §8.18). Counts as one pending intent (TM L30: 256 per script per frame). |
| `Input.Deactivate(context)` | `true` \| `nil, "busy"` | `B` | item-7 drain | As above. Deactivating releases the context's holds (KM §3); a pending sequence is dropped only if it belongs to this context (D17 §5.5). |
| `Input.IsActive(context)` | boolean | `B` | read | The state the script last asked for, including a change not yet applied. |

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

<a id="s8-2"></a>
### 8.2 The script's own actions

| Signature | Returns | Cost | Applies | Notes |
|---|---|---|---|---|
| `Input.On(action, fn)` | nothing | `B` | at once | Sets or clears the handler of a declared action (§2.8, form 1: `Input`'s notification names are the script's declared short action names; any other name raises). `fn(ev)` with `ev = {action, kind, value, dropped}`: `kind` `"start"` or `"end"`; `value` the action's value at that edge (1 or 0 for a button; for an axis the value after the edge). Edges are recorded in step 1 in the order they happened, at most 16 per action per frame (more are counted in `dropped`), and delivered **same-frame 6b**. A `"start"` is a **gesture**: its turn is a gesture window (§2.1, §7.7). |
| `Input.State(action)` | table | `B` + `e(5)` | read | This frame's snapshot (KM §7.1): `{pressed, presses, releases, value, delta}` (KM §2). |
| `Input.Actions()` | array of tables | `B` + `e(n)` + `b(bytes)` | read | One `{name, kind, context}` per declared action, in manifest order; `kind` `"button"` or `"axis"`. |
| `Input.Bindings(action)` | array of strings | `B` + `e(n)` + `b(bytes)` | read | The binding texts (KM §5) now in effect for the action: the user's from the ext keymap document, or the suggested default when registration did not withhold it. So a script can say "press F7". Empty when unbound. |

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
| `Input.Standard(name)` | table | `B` + `e(5)` | read | This frame's snapshot of one standard action, as `Input.State`. `name` must be on the list below; any other name raises. |

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
| `World.Here()` | table \| `nil, "not loaded"` | `B` + `e(9)` | read | `{region, x, y, z, yaw, heading, mode}`: the player character's position and facing (§2.10) as of step 3 of this frame, `mode = "walk"`; in fly mode without a character, the free-fly viewpoint, `mode = "fly"` (OPEN-7). `nil, "not loaded"` before the player is placed. |
| `World.Landblock(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` | read | `{region, bx, by, cell, indoors}`: the landblock (X, Y) and, from the resident collision data, the interior cell index containing the point or `nil` outdoors (`collision.md` §5.8.1); `indoors` is `cell ~= nil`. Counts as a point query. |
| `World.Area(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` + `b(bytes)` | read | `{area, territory, region}`: the names of the area at the point and of its parents, each a string or `nil`, in the client's language, from the area lookup over resident data (ARCH §9.2, "Lookups"). |
| `World.MapCoords(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` + `b(bytes)` | read | `{north, east, text}`: the point's map coordinates as Tessera's own map display computes them. **Contract:** `north` grows toward +Y and `east` toward +X, both are finite numbers, and `text` is the string Tessera's client shows for them. Their formula, rounding and the format of `text` are **not** part of the ext contract (§9.2, OPEN-3); scripts display `text` and must not parse it. |
| `World.LocText()` | string \| `nil, "not loaded"` | `B` + `b(bytes)` | read | The position text that Tessera's own `/loc` command reports for the player now, at most 256 bytes. Same contract limit as `MapCoords.text`: display it, do not parse it. |
| `World.Height(x, y)` | number \| `nil, "not loaded"` | `B` + `Q` | read | The terrain surface's z at (x, y), from the resident terrain of the point's landblock (`tessera-terrain` height query). Ignores objects and interiors. |
| `World.Water(x, y, z)` | table \| `nil, "not loaded"` | `B` + `Q` | read | `{surface, under}`: the z of the water surface above or at (x, y) or `nil` when there is none, and whether (x, y, z) is under water (ARCH §9.2, "Lookups"; `water.md`). |
| `World.Ray(x1, y1, z1, x2, y2, z2 [, opts])` | table \| `false` \| `nil, reason` | `B` + 2,000 + 2,000 per landblock the segment's bounding box overlaps (at most 25) | read | Casts a segment against the **installed collision sets** (ARCH §20.3, "Collision-set residency"). Its length must be ≤ 500 m (longer raises). `opts.water` (boolean, `false`) includes water surfaces. Returns the nearest hit `{x, y, z, nx, ny, nz, distance, what}` with `what` one of `"terrain"`, `"object"`, `"cell"`, `"water"`; `false` when nothing is hit; `nil, "not loaded"` when any landblock the segment's bounding box overlaps has no installed set; `nil, "busy"` past the ray caps. |
| `World.Clock()` | table | `B` + `e(7)` | read | The game clock Tessera's client uses in this frame: `{seconds, time_of_day, day, year, day_of_year, month, night}`. Only the contract of §9.2 is promised. |
| `World.Sky()` | table | `B` + `e(12)` | read | The sky state Tessera's client computed for this frame: `{sun_x, sun_y, sun_z, sun_color, ambient_color, fog_near, fog_far, fog_color, day}`. Only the contract of §9.2 is promised. A field Tessera does not compute in a frame (no sky loaded) is `nil`. |

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
reviewer's confirmation of this reading is OPEN-3.

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
| `Entity.Find(filter)` | array of entities, truncated \| `nil, "busy"` | `B` + 20 per candidate + (100 + `h(1)`) per result; the candidates are reserved from the counts known at the call's start and refunded | read | Batched query. `filter = {kinds, near, box, limit}`: `kinds` an array of kind names (default all three); exactly one of `near = {x, y, z, radius}` (radius 0 … 500) or `box = {x1, y1, z1, x2, y2, z2}` (each side ≤ 1,000), or neither (every entity of those kinds); `limit` 1 … 512 (default 512). Results are sorted by distance from `near`'s point (or from the box's centre, or from the player), ties in a stable order that deterministic mode repeats. Second result `truncated` is `true` when more entities matched than were returned (D17 §3, "API rules"). Caps (TM L71): 8 queries per script per frame, 32 for all scripts. The candidates are the entities of the landblocks the area overlaps (all resident ones without an area), so a query whose reservation exceeds the call's remaining units raises before it starts (§2.5): narrow it with `near` or `box`, or leave out `"static"`. |
| `Entity.Read(list, fields)` | array of tables | `B` + per entry (`e(#fields)` + `b(bytes)`) | read | Batched read: `list` an array of at most 512 entity handles, `fields` an array of field names (§10.1). Entry *i* is a table of the requested fields of `list[i]`, or `false` when that handle is stale. An unknown field name raises. |
| `e:Get(field)` | the field's value | `B` + `b(bytes)` | read | One field of one entity; raises on a stale handle. |
| `e:Position()` | x, y, z | `B` | read | Raises on a stale handle. |
| `e:Exists()` | boolean | `B` | read | |
| `e:Kind()` | string | `B` | read | Works on a stale handle. |
| `Entity.Subscribe(filter, fn)` | subscription \| `nil, "limit"` | `B` + `h(1)` | next turn onward | `filter = {kinds, near, around_self, box}`: `kinds` as above; at most one of `near = {x, y, z, radius}`, `around_self = radius` (radius ≤ 500, measured from the player at the frame of the event), `box`. `fn(sub, batch)` per §10.3 (a per-request callback, §2.8). At most 16 subscriptions per script, 64 for all scripts (TM L30). |
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
  **filter call**: a call like any other (TM L3, L4), counted against the 2 ms cap and
  split like any turn (§2.6). It filters records from the cursor in batches of **64**,
  charging before each batch 64 × (the script's number of subscriptions) × `c_f` units
  (`c_f` the declared cost of one record-filter pair, TM OPEN O-4). It stops, keeping the
  cursor, when the next batch's charge would pass the call's remaining units or deadline,
  when the script's events matched in this frame reach **1,024** (TM L31), or at the end of
  the retained records. No filter work runs outside this call: not in steps 2–5 and not in
  6c. A record older than 60 frames when the cursor reaches it is skipped and counted as
  dropped.
- **What matches.** A spawn record matches a subscription when its kind is in `kinds` and
  its position at spawn is inside the area (if any; `around_self` measured from the player's
  position in the record's frame); a despawn record the same with the position at despawn.
  Moving into or out of an area is not an event; scripts query for that.
- **Matches become notifications.** At the end of the filter call, each subscription that
  has matches, or a count to report, gets **one** batch notification appended to the queue
  (§2.8; it is part of the turn's delivery set, so it is delivered later in the same turn if
  the cap allows). Each batch notification takes one entry of the 1,024-entry queue; while
  the queue is full, the batch is dropped and its events are counted.
- **The batch.** `fn(sub, batch)` with `batch = {spawned, despawned, dropped, unfiltered}`:
  arrays of handles in record order (converted inside the delivery call, `h(n)` charged
  first); `dropped`, the events lost since this subscription's last delivery (queue cap,
  records older than 60 frames); and `unfiltered`, the overflow counts of every frame the
  cursor passed since that delivery (the script cannot know whether those records would
  have matched, so every subscription of the script reports them). Handles in `despawned`
  are stale on delivery; they compare equal to handles the script still holds (§2.7), so it
  can remove them from its own tables.
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
| Queued notifications, timers, subscriptions, nodes, pending intents | L30 | queue 1,024 per script (state notifications coalesced); timers 64; subscriptions 16 per script, 64 for all; nodes 2,048 per script (512 per frame, 256 labels), 8,192 for all (1,024 labels); intents 256 per script per frame | dropped and counted; `nil, "limit"`; `nil, "busy"` |
| Commands | L68 | 32 per script; names 1–32 bytes | `nil, "limit"`; a bad name raises |
| Actions, contexts | L60 | 64 actions and 2 contexts per script; 256 and 16 for all; priority −500 (§8.1) | the load is refused |
| Context changes | L62 | 4 per script per second, coalesced per frame | `nil, "busy"` |
| Ext holds | L61 | 64 for all ext scripts | the hold does not start |
| Ext UI pools | L49–L51 | 128 surfaces, 256 entries, a quarter of each memory pool per script; 512, 1,024, 32/64/64 MiB for all | `nil, "limit"` or `false` |
| Composition coverage | L55 | 1.25 screens per script, 2.0 for all; per entry min(one screen, unclipped displayed rectangle); badges and labels count | `false` / `nil, "limit"`; a user resize stops; a host change hides (§7.10) |
| Ext overlay and composition row | L56 | 1.0 ms GPU at 3840 × 2160 (E9; composition part in slice 1a) | the bounds shrink |
| Solid press, focus box, case 1 | L57–L59 | alpha × effective opacity ≥ 0.5; the visible-focus rule; case 1's three conditions and gesture-window focus | not a gesture; `false` |
| UI text | L81 | `Text` 16 KiB, `Field` ≤ 4,096 bytes, `List` 1,024 × 256 bytes, labels, buttons, `placeholder` 256 bytes | Lua error |
| Images | L48 | ≤ 4,096 px per axis, charged before decode | `Loaded{ok = false, reason = "too large"}` |
| Outstanding worker requests | L27 | 32 per script, 256 for all (`Store` reads, pending reads included, and image decodes) | `nil, "busy"` (`Get`, image creation); `false, "busy"` (image `Set`) |
| Result reservations | L28 | stored size × the declared expansion ratio | `nil, "out of memory"` |
| `Store` values | L33–L35, L39 | 32 levels, 8,192 elements, 1 MiB | `nil, "too large"` |
| `Store` space | L40, L41 | 1,024 keys and 16 MiB per script; 16,384 keys and 256 MiB for all; temporary files and pending entries count | `nil, "full"` |
| `Store` reads | L42 | 64 reads and 4 MiB per second per script; 16 MiB per second for all | `nil, "busy"` |
| `Store` writes, keys | L43 | 64 newly dirty keys per second per script (10 s bucket); one flush per script per second | `nil, "busy"` |
| `Store` writes, bytes | L44 | 32 KiB/s with a 10 MiB bucket per script; 128 KiB/s with a 40 MiB bucket for all; debited at the call, in full when a call makes a clean key dirty (no entry, or an entry a flush has taken), growth only over an untaken value (TM OPEN O-3) | `nil, "busy"` at the call |
| Script log | L45 | 64 KiB per second per script; 1 MiB file rotated once; record ≤ 4,096 bytes | dropped and counted; cut with `…[cut]` |
| `World` rays | L69 | 64 per script per frame, 256 for all; ≤ 500 m | `nil, "busy"`; longer raises |
| `World` point queries | L70 | 256 per script per frame | `nil, "busy"` |
| `Entity` queries | L71 | 8 per script per frame, 32 for all; ≤ 512 results | `nil, "busy"`; `truncated` |
| Entity events | L31 | 1,024 per script per frame; 4,096 records per frame; 60 frames retained; filter batch 64 | dropped, `unfiltered` and counted |
| Retained entity records | L25 | ≤ 245,760 records, ≤ 15 MiB host-wide | overflow counted |

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
| *L* (load) | 6a runs the top-level run. It takes `Core`, `UI` and `World` with `Ext.Use`; creates a frame of 220 × 56 units at the top-left, `movable = true`, with a `Group` (`layout = "column"`) holding two `Text` nodes; calls `frame:Show()`, which returns `true` (coverage about 0.007 screens at 1920 × 1080 and UI scale 1: the frame's 12,320 pixels plus its badge, §7.10). It registers `Core.Every(0.25, …)`. It has no `VisibilityChanged` handler, so nothing is queued for it (§2.8). Step 8 paints the frame and its badge "hud · ext script". |
| *L* + 1 | No timer is due yet; the turn is empty. |
| *L* + 15 (0.25 s at 60 fps) | The timer call runs: `World.Here()` returns `{region = 1, x = 20,240.0, y = 22,420.0, z = 312.5, yaw = 3.1364, heading = 270.3, mode = "walk"}`; `World.MapCoords(x, y, z)` returns a table whose `text` the script puts in the first `Text`; it formats `heading` as "W" (the nearest of eight compass points) in the second. Two `Set` calls change text; they paint in this frame's step 8. |
| any frame while a plugin window overlaps the frame | Nothing changes for the script; the plugin window is drawn above it and takes the pointer there (§7.1). |
| a frame where the player stands in a landblock not yet streamed | `World.MapCoords` answers `nil, "not loaded"`; the script keeps the old text. |

<a id="s13-2"></a>
### 13.2 Nearest-entity tracker

Manifest: `ui`, `world.read`, `entity.read`, with the matching `[modules]`. The grant dialog
shows the `entity.read` + `ui` warning (D17 §5.2).

- The top-level run creates a frame with a `List` and one `WorldLabel` (hidden), subscribes
  with `Entity.Subscribe({kinds = {"character"}, around_self = 60}, fn)`, and sets
  `Core.Every(0.5, refresh)`.
- `refresh` calls `Entity.Find({kinds = {"character"}, near = {x, y, z, 60}, limit = 10})`
  with the position from `World.Here()`. Say it returns 4 handles and `truncated = false`; the
  cost is `B` + 20 × (the candidates in the overlapping landblocks) + 4 × 150 units. It then
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
| *S* (the user types `/ext:notes add buy bread` in chat) | Step 1 intercepts the `ext:` word; no chat line, no echo. 6b calls `onAdd("buy bread", {command = "add", truncated = false})`; the turn is a gesture window, but the chat window is still active, so case 2 does not hold and a `field:Focus()` here returns `false` (no exception for the chat window, §7.7). |

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

These are this spec's functional checks, run with `scripttool test` (deterministic mode) or
the unit tests of `tessera-ext` and `tessera-ext-host`. They are **not** an escape
catalogue: where an invariant covers ground of an escape case, the case is TM's and is named
in brackets; its procedure and "Fails if" are TM §11's, and EM's `escape` runner runs it
under that number (TM §0.1 rule 3).

1. `Ext.Use` of `UI`, `Input`, `World` or `Entity` without the grant answers
   `nil, "not granted"`; with it, two calls in one apartment return the same table; for a
   module the manifest does not declare it answers `nil, "not declared"`.
2. After a grant is revoked the script is suspended in that frame and runs no code until the
   user resumes it; after an optional revocation every function of a module table of that
   capability that the script kept raises `"not granted"`; after a required one the script
   is unloaded without its `Unload` handler [TM X91].
3. Every function of §4–§10 raises a Lua error, never panics, for: a NaN or infinite number
   where a finite one is required, a non-integer where an integer is required, a DID outside
   0 … 2^32 − 1, a value of the wrong type, an unknown property, context, action, setting,
   notification or field name (fuzz target "Lua-side argument validation").
4. A handle compares equal to every other handle of the same live object held at the same
   time; after removal, `Exists()` is `false`, `Kind()` still answers, and every other method
   raises.
5. `getmetatable` of any handle returns a string, never a table [TM X2].
6. `Core.Time()` returns the same number for every call within one frame and never
   decreases; `Core.Frame()` rises by one per frame.
7. A timer created in a turn never fires in that turn; a repeating timer fires at most once
   per turn.
8. A 65th timer, a 33rd command (TM L68) and a 17th subscription answer `nil, "limit"`
   [TM X24, which covers `/ext:` commands at L68].
9. A log call whose text holds CR, LF, ESC, a C1 control or U+2028 produces exactly one
   record in TM §8.1's format, in which none of those characters appears unescaped
   [TM X51].
10. No function or report of these modules produces a chat line or a plugin chat event; an
    `/ext:` line, known or unknown, in any letter case, produces none either [TM X73, X66].
11. `Store.Get(k)` called after `Store.Set(k, v)` answers `v`, whether or not the flush ran,
    including in the call of the `Set`; a `Set` after a `Get` does not change what the `Get`
    answers; after `Store.Delete(k)` it answers absent [TM X88].
12. A script that sets one key to a value under 16 KiB in every frame for 10 s causes at
    most 11 writes of that key's file and is never answered `"busy"` for it.
13. A value with a NaN, a cycle, a 33rd level, an 8,193rd element or more than 1 MiB
    serialised answers `"invalid"`, `"cyclic"` or `"too large"`, and nothing is stored
    [TM X20].
14. 32 outstanding `Store.Get` calls for 1-KiB values succeed in a script with a 32 MiB cap,
    whether answered from the pending map or from disk; the 33rd answers `nil, "busy"`; a
    reservation over the cap answers `nil, "out of memory"` and raises nothing [TM X25].
15. A `Store` file edited to hold a NaN key, a nil key or 10^6 entries is answered
    `fn(nil, "invalid")` or `fn(nil, "too large")` from the worker, without a panic
    [TM X20, X59].
16. `Commands.Register` of a name with `.`, an upper-case letter or over 32 bytes raises.
17. A command line's arguments over 1,024 bytes arrive cut at a UTF-8 boundary with
    `truncated = true`.
18. An ext frame is never drawn above, and never catches a point of, a plugin window or a
    native panel, whatever their z-orders.
19. A script's `Raise`, `Show` or `Activate` never moves its frame above another script's
    frame.
20. `frame:Activate()` and `field:Focus()` return `false`: with no window active outside a
    gesture window; with the chat window active in the turn of an `/ext:` line; after the
    user's solid press on the frame followed by a key press elsewhere, by the pointer leaving
    the script's frames, or by 5 s without input there. `field:Focus()` (and the focus part
    of `Activate`) also fails under case 1 outside a gesture window, and for a hidden,
    zero-size, off-screen, clipped, below-0.5-opacity or covered field (under the chat
    window, an opaque plugin window or another script's frame) [TM X37, X39].
21. In the turn that delivers the `"start"` of the script's own action with no window active,
    `Focus` on a visible, uncovered field returns `true`, from the action's handler, from a
    later handler of that turn or from its `Tick`, also when the turn is split [TM X89].
22. A press on a catching node whose own background at the press point is absent or has
    alpha × effective opacity below 0.5 (text and borders only, opacity 0.49, an image texel
    of alpha 0) delivers `Pressed`, `Released` and `Clicked` with `gesture = false`,
    activates nothing, deactivates the window that was active, and a timer `Focus`
    afterwards returns `false` [TM X38].
23. A focused field over which a plugin window is moved loses the focus before the next key
    is routed; the next key reaches the plugin window or the game [TM X39].
24. Every shown frame with a catching node or a field carries the host-drawn badge with its
    script id and "ext script", whether or not a field has focus; no ext frame of any script
    draws over a badge; while a field has the focus the caret, the focus mark and the
    focus-coloured badge are drawn [TM X49].
25. A script's `Set{text = …}` on a field clears its selection, and a copy after a text
    change copies nothing [TM X52].
26. World labels are never hit-tested: a world label over a plugin button changes neither
    the button's hover nor its click, and receives no notification [TM X45].
27. A show, creation or size change that would take a script's composition coverage over
    1.25 screens, or all ext over 2.0, is refused and changes nothing; moving a shown frame
    never changes coverage; a user resize stops at the bound; a screen resize that passes a
    bound hides frames in reverse order of their last show [TM X31].
28. An `Input.Activate` made in 6b of the frame before a backend switch is never applied;
    nor is one made before a suspension or unload (D17 §6.6, the slice-1a intent test)
    [TM X80].
29. An ext binding never fires while a text field (chat, plugin or ext) has the focus, and
    never for an input that a non-ext binding takes in either pass (D17 exit criterion 2)
    [TM X34, X36].
30. `Input.Standard` of any name off the list of §8.3 raises; `Input.Standard("mouselook")`
    is pressed exactly while the `mouselook` action has a hold from either context.
31. A script that toggles its context every frame never cancels a pending sequence of a
    non-ext context (D17 exit criterion 2) [TM X40].
32. Every `World` function answers `nil, "not loaded"` for a point in a landblock that is
    not resident, and never causes that landblock to be requested.
33. `World.Ray` answers `nil, "not loaded"` when any landblock its bounding box overlaps
    has no installed collision set, and never a partial result.
34. The 65th ray, the 257th point query and the 9th entity query of a script in one frame
    answer `nil, "busy"`; the next frame they succeed.
35. `Entity.Find` never returns more than `limit` results and sets `truncated` exactly when
    more matched.
36. With 10,000 spawns in one frame, 4,096 are recorded, every subscription's next batch
    reports `unfiltered = 5,904`, and no filter work happens outside a filter call
    [TM X19].
37. A script starved for 10 s while 1,024 entity events arrive per frame keeps its memory
    within its cap and its frames within the ext ceiling of TM L12 [TM X19, X24].
38. In deterministic mode, two runs with the same seed and walk give the same calls in the
    same frames, the same notification order (§2.8) and the same answers to every query,
    while real worker timing varies (D17 exit criterion 7).
39. Under the "every frame is heavy" hook in deterministic mode, a queued `/ext:` line is
    delivered within 16 frames (plus the round robin) [TM X85].
40. `World.Here().heading` equals (90 − `yaw` · 180 / π) wrapped into [0, 360), and every
    `yaw` the host returns lies in (−π, π].
41. A `Store.Set` of a key whose previous value a flush has taken and is still writing
    (the harness holds the flush in flight) debits one key token and the new value's full
    counted size; a script rewriting 1,024 keys every frame for 60 s, with every flush
    held in flight, writes at most the bound of TM X57 [TM X57].
42. A script whose own calls fill the 2 ms cap every frame resumes its split turn first
    in its group at most once per turn; every later part of that turn runs at the
    script's ordinary rotation place, so the other scripts of its group are never starved
    beyond the round robin [TM X17].
43. With the process locale set to a non-"C" value in a test build, every loaded ext
    script is suspended in the frame of the next check, no load or resume succeeds until
    the locale is "C" again, and the first turn after a resume carries
    `Resumed{reason = "locale"}` [TM X22].
44. A state notification frozen in a turn's delivery set is delivered with the value it
    had when the turn started; a change the script's own call makes during that turn is
    delivered as a separate entry in the next turn, never in the same turn.
45. A re-read of the grant store that removes, invalidates or narrows a loaded script's
    record suspends it in the frame the result is applied; one that widens it changes
    nothing until the next load [TM X91].
46. In the ext profile, `table.foreach`, `table.foreachi`, `table.getn`, `table.setn`,
    `string.gfind` and `math.mod` are `nil`, and every global and library function outside
    §2.11's lists is absent [TM X8].

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

> **OPEN-3 (values Tessera computes):** `World.MapCoords`, `World.LocText`, `World.Clock`
> and `World.Sky` pass through values Tessera's client computes with rules some of which
> rest on disassembly (`coordinates.md` §8–§9; `environment.md` §11, "BIN unless stated";
> `sound.md` §10.3.5). §9.2 promises only units, ranges and directions, so no such rule
> becomes part of the ext contract. The reviewer is asked to confirm that this satisfies
> D17 §12 and LEGAL §18.3 rule 1. If not: `MapCoords` keeps `north` and `east` from a
> Tessera-defined linear function of the region frame and drops `text`; `LocText` is
> withdrawn from the major; `Clock` keeps only `seconds` and `time_of_day`; `Sky` keeps only
> the sun vector and drops `day`.

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

> **OPEN-7 (`World.Here` in fly mode):** whether the free-fly viewpoint may be reported
> under `world.read` once slice 1b's `Camera` module (group A) reads the camera pose; if the
> review decides it may not, `Here` answers `nil, "not loaded"` in fly mode without a
> character.

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

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (design, from accepted decision 0017): §0–§16. The slice-1a module reference for `Core`, `Store`, `Commands`, `UI`, `Input`, `World` and `Entity`, with types, results, failures, costs, timing and capabilities; closes for these modules D17's open items 2 (presses that do not activate), 5 (subscription filtering inside metered calls with a cursor and a recorded-list cap), 6 (`Store` read-your-writes with a per-(kind, id) pending map), 8 (world labels never catch the pointer) and the composition half of item 1 (composited-area bound), and the "Other improvements" on log escaping, the ownership badge, clipboard selection clearing, image decodes as outstanding requests, sticky aborts, split turns and gesture calls, and `Store` reservations from the recorded size. |
| 2026-10-07 | Revision 2 (Review), answering the independent review by adopting `ext-threat-model.md` revision 2 as normative. **Authority** (§0, header): TM cited for every limit (TM L*n*, §11 rewritten with an L column), every rule of its §7–§9 and every escape test (TM X*n*); this spec and EA1b together are D17's `ext-api.md`, and the general rules live here; no escape catalogue of its own (§14's invariants name the TM tests they overlap). **Open item 2** (§7.6, §7.7): "visible painter" replaced by TM's solid press (own background or image only, with a per-kind form for host-drawn faces); a non-solid press is a world click for activation and focus; `Pressed`, `Released`, `Clicked` carry `gesture`; under case 1 a `Field` is focused only inside a gesture window. **Gesture window** (§2.1, §2.6, §7.7): the whole delivering turn (TM §8.13), not one call; solid presses are gestures. **Composition** (§7.10, §7.9, §7.2, §11): 1.25 and 2.0 screens, unclipped and position-independent per-entry counting, badges and labels counted, no nested surfaces in slice 1a, user resizes stop at the bound, host changes hide in reverse show order, the TM L56 budget row and E9; OPEN-4 closed. **Entity filtering** (§10.3): one batched filter call per turn costing 64 × subscriptions × `c_f`, matches queued as batch notifications, overflow reported as `unfiltered`, cursor to the newest frame at resume with the gap in `Resumed`, the 15 MiB retained bound. **`Store`** (§5): TM §7.6 read-your-writes (pending reads answered from a copy at the call, no worker job but counted as outstanding and against the read rate, delivered in a later turn; pending deletion answers absent; wipes clear the map; pending entries count to the totals and the writer's cap); TM §8.14 reservations from the pending entry or file index, refusals return `nil, "out of memory"` (new reason, §2.4); TM §8.7 byte buckets (32 KiB/s and 10 MiB per script, 128 KiB/s and 40 MiB for all, debited at the call, per session), temporary files counted and removed at open; the owner tag of TM §8.24. **Badge** (§7.1, §7.7): on every shown frame with a catching node or a field, "ext script", top layer of the ext range, counted in coverage, no longer part of a field's focus area. **Images** (§7.10, §7.3): a decode at the cap answers `nil, "busy"` / `false, "busy"` at the call. **Liveness** (§2.6, §4.1): floor frames, the switch timeout (close without `Unload`) and the gate's clock origin; OPEN-9 closed. **Memory** (§11): kept snapshot data files, the file index and pending entries in the per-script row. **New general rules** (§2.8, §2.10, §3): one handler pattern (`Module.On`, `obj:On`, per-request callbacks; `Core.OnTick`/`OnUnload`/`OnResumed`/`OnSettingsChanged` became `Core.On("Tick"|"Unload"|"Resumed"|"SettingsChanged")`, `UI.OnScreenChanged` became `UI.On("ScreenChanged")`, `Input.On` handlers take one table); notifications queued only with a handler, gestures always; a FIFO queue with a fixed delivery set per turn (the §2.8 order contradiction fixed); yaw in radians per KM §9.1 beside `heading` in degrees, quaternions `{w, x, y, z}` (`yaw` added to `World.Here` and entity fields); `Ext.Use` checks the manifest's module tables and answers `"not declared"` for an undeclared module (closing EM OPEN O-2); every ext context has priority −500 (closing EM OPEN O-3). **BIN pass-through** (§0, §9.2, OPEN-3): `World.Clock` and `World.Sky` get contract limits like `MapCoords`. **Log** (§4.3): TM §8.1's one format. **Minor**: `V` = 295,112; `Input.Standard("mouselook")` defined as the one registry action; revocation removes subscriptions too (§2.9); OPEN-2 and OPEN-10 closed; examples §13 and invariants §14 updated (invariants 39 and 40 added). |
| 2026-10-07 | Revision 3 (Review), answering the re-review of revision 2 and carrying TM revision 3. **Angles** (§2.10, blocking): one rule for every API spec: every direction angle (yaw, pitch, an angle about +Z relative to a yaw) is in radians per KM §9.1, `heading` is a result only, no argument is ever a heading or takes degrees for a direction; the claim about EA1b restated as a requirement EA1b follows (`Camera` pose, `Follow`, key frames, `Sandbox.Teleport`); a field of view is an extent, stated by its module; §0 names §2.10 as the only angle section. **Generated reference** (§0, §2.5): `docs/specs/ext-api-reference.md` (EA1b §10.4, §10.6), derived, never hand-edited, adds no rule. **Fairness** (§2.6): a split turn resumes first in its group at most once; a part split again waits for its ordinary rotation place (TM §8.13, X17); invariant 42. **Locale** (§2.8, §2.9, §4.1, §7.11): a process locale other than "C" suspends every ext script (D17 §1.5, TM T22, X22); no load or resume until it is "C" again (1a rule); `Resumed.reason = "locale"`; §2.8 lists the four suspension causes; invariant 43. **Ext environment** (new §2.11): D17 §1.2's globals, absent names and replacements (`print`, `tostring`, per-apartment random numbers) and TM §8.15's library tables, removed compatibility names and `math.max`/`math.min` charged by argument count (X8); invariant 46. **Coalescing** (§2.8): entries of a turn's fixed delivery set are frozen; a later change, self-caused or from the next frame's input in a split turn, is a new entry after the set, so self-caused values arrive in the next turn and at most one entry per node and kind waits outside the set; invariant 44. **Window removal** (§7, §7.11): no host close control; the host routes are the panic chord and the script manager (unload, revoke `ui`); D17 §3's "closing the window" only through the script's own control. **`Store`** (§5, §11, §13.3; TM §8.7, L44, X57, rules 7.6.1–7.6.3, §8.8, §8.24): taken and untaken pending entries; a key is clean once a flush has taken its value, even in flight, so a `Set` or `Delete` landing during an in-flight flush pays a key token and the full counted size, and only a replacement of an untaken value pays growth alone (invariant 41); the file index charged in full to every apartment that has the store open; reads from the pending map are delivered in the **next** turn, disk reads in the first turn after their job; an unreadable owner tag also wipes; the store directory opened without following links, temporary files named `t_<key>.tmp` (EM §3.1, §3.6, §8.8, TM X94). **Floor** (§2.6, §11): 250 ms counted from the start of the last frame in which ext ran; a floor frame resets both conditions and runs 6c with its 1 ms slice. **Revocation** (§2.9): the grant-store re-read is a route (narrowing suspends, widening waits for the next load; TM §8.23); invariant 45. **Smaller** (§2.1, §6.2, §10.3, §14): "Call" includes REPL runs (TM §2, L85); `/ext:reload` refused with "Developer mode is off" outside developer mode, and for an id not loaded queues nothing with a notice (1a rule); the entity worked example and X19 within L30 (4 × 16 and 16 × 4); invariant 8 cites L68 with X24, which now covers commands. |
