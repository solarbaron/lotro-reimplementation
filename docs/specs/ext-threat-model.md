<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Extended scripting: threat model, limits register and escape-test catalogue (M5.5)

| Field | Value |
|---|---|
| Status | **Accepted** (2026-10-07) |
| Last updated | 2026-10-07 |
| Author role | Design (original Tessera design, clean room, no dirty-room input), AI-assisted |
| Reviewer | Independent clean-room review |
| Review record | Review of revision 6, 2026-10-07: **passed**, Accepted, with minor non-blocking follow-ups. Full record moved to the review log (decision 0018). |
| Implements into | `crates/script` (the ext profile, the patched interpreter's ext modes, the hook, the sticky abort and the C-call bound), `crates/ext` (limits, value model, scheduling, gesture and focus rules, bridge resolution rules), `crates/ext-host` (ports, all ext file I/O, the `Store` file index and pending-write map, the store owner tag, the log writer, the headless harness), `crates/input` (ext holds pool, suggested-default rule, coalesced recompilation, the `Input.Drive` allow-list), `crates/net` (backend class), `crates/plugins` (registration serial numbers for the bridge, §7.3; the plugin side of X6 and X74), `crates/settings` (the install guard on `keymap-ext.toml` saves, X61), `crates/ui` (the solid-press hit test, §7.2; selection and copy rules, §8.4; composition coverage of ext entries, §7.1), `crates/render` and `apps/client` (overlay coverage, the ownership badge, the reserved layer, the heavy-frame gate and its floor, pointer picking under a lease), `apps/scripttool` (`test` and the `escape` runner), `tools/` (the build-level check), `fuzz/` (the parsers named in §11) |
| Related records and specs | [0017](../decisions/0017-extended-scripting.md) (the accepted design; abbreviated **D17**), [0004](../decisions/0004-server-strategy.md), [0014](../decisions/0014-ui-input-bridge.md) (**IB**), [0015](../decisions/0015-ui-rendering.md) (**UR**), [0016](../decisions/0016-plugin-class-helper.md), [`lua-plugin-api.md`](lua-plugin-api.md) (**LPA**), [`plugin-runtime.md`](plugin-runtime.md) (**PR**), [`keymap.md`](keymap.md) (**KM**), [`chat-window.md`](chat-window.md) (**CW**), [`ui-behaviour.md`](ui-behaviour.md) (**UB**), [`ui-layout.md`](ui-layout.md), [`ui-widgets.md`](ui-widgets.md), `docs/LEGAL.md` §9.5 and §18; the M5.5 companion specs [`ext-manifest.md`](ext-manifest.md) (**EM**), [`ext-api-1a.md`](ext-api-1a.md) (**EA1a**) and [`ext-api-1b.md`](ext-api-1b.md) (**EA1b**), which cite this spec as **TM** (§0.1). "The API specs" means EA1a, EA1b and the API specs of later slices; D17's working name `ext-api.md` means these files |

> **Amended by [decision 0034](../decisions/0034-official-server-compatibility.md) §4
> (2026-10-11); not yet carried into this text.** The rule "extended scripts never run
> against an official server" is replaced: on an official-server session (a backend class
> of its own, named by the M8 and M9 specs) extended scripts run only after the user's
> explicit opt-in, which is off by default, shows the risks, is stored per user and can be
> set by nothing but the user in the client's own interface; per-script grants still apply.
> Automation (`input.drive` and every module that issues movement, input or game commands)
> stays offline only and is never enabled on an official server. `Other` stays fail-closed.
> D17 §6.3's build-level exclusion (`gated-compat` tag, `links` guard, CI check) is
> withdrawn. The M9 revision of this spec carries the change into its text; where the two
> differ before then, decision 0034 wins.

---

<a id="s0"></a>
## 0. Purpose

D17 accepted extended scripting with eight "open security and design items" and a list of
"other improvements", and said that each becomes a requirement of the M5.5 threat model
and an escape test in `scripttool test` before the first slice ships. This spec is that
threat model. With it an implementer and a reviewer can:

- name what is protected (§3), from whom (§4) and across which boundaries (§5);
- find, for every threat, the requirement that answers it and the test that proves it
  (§6);
- read the concrete rule chosen for each D17 open item (§7) and each "other improvement"
  (§8), and the restated rules for the earlier hardening items: `string.rep`, hash
  flooding, exfiltration routes, disk caps and per-apartment random numbers (§9);
- look up every resource limit of extended scripting with its number in one register
  (§10);
- run the escape-test catalogue (§11) and check the invariants (§12).

It is a design document. It records no fact about the original game client, and it adds
no BIN fact to the ext contract (D17 §12, LEGAL §18.3 rule 1). Where it names plugin or
chat behaviour that other specs record from the original (for example how the chat
command line re-tokenizes a line, CW §7.3), it does so only to say which route ext must
**not** use; none of that behaviour becomes part of what ext scripts may rely on.

**Normative status.** D17 delegates its open items to this spec, so for those items the
rules of §7 and §8 are normative, and the API specs and EM take them over word for word in
meaning. Where a companion spec settled a point differently, this revision has adopted
the stronger security rule of the two and lists the companion's change in §13. Where a
rule here changes a figure that D17 states as accepted (the frame budget, §7.1.6; the
`Store` write rate, §8.7), the change is listed in §13 for the accepting session and
carries an OPEN (§14) until the owner confirms it at the M5.5 plan review. Until then the
stricter of the two figures applies.

<a id="s0-1"></a>
### 0.1 How other specs cite this spec

This spec is the **single home** of two things: every resource limit of extended
scripting (the register of §10, rows **L1**…; rule 6 says which numbers that covers) and
the escape-test catalogue (§11, tests **X1**…).
The companion specs follow these rules:

1. **Limits.** A spec that needs a limit cites it as "TM L*n*" and may repeat its number
   for the reader only next to that citation (for example "32 outstanding requests, TM
   L27"). If a repeated number differs from §10, §10 wins and the other spec has a
   defect. A spec that needs a new limit, or a different value, proposes it here first;
   it gets the next free L-number.
2. **Rules.** A rule of §7–§9 is cited as "TM §*n.m*" or "TM rule *n.m.k*". An API spec
   states the script-visible behaviour (names, arguments, results) in its own words and
   cites the rule; it may make a rule stricter, never weaker.
3. **Escape tests.** Escape cases are named only by their X-number ("TM X38"). EM's
   `escape` runner (EM §11.8) and the client's integration tests run them under those
   numbers; no other spec keeps its own escape-case IDs (§11.10 maps EM's earlier IDs).
   A spec that finds a new escape case proposes it here; it gets the next free X-number.
   A spec's own functional tests (not escape cases) keep their own names. An X-test with
   parts under several harness codes passes only when every part passes (§11).
4. **Stability.** L-, X-, T- and invariant numbers are never reused or renumbered. A
   retired entry stays in its table marked "retired" with the reason. A row's value may
   be restated (with a change-log entry); its number stays.
5. **Requests.** A companion spec that needs TM to change marks the request OPEN in its
   own text; TM answers it in §13 (the companion's change) or §10 (a new or restated
   row), and the companion then closes its OPEN.
6. **What the register holds.** Every **resource limit** of extended scripting has an
   L-number here: a count, size, rate, time or per-call cap that bounds work, memory,
   disk, GPU time, queue length or the user's control (objects, entries, elements,
   records, outstanding requests, bytes held, read, written or displayed, calls or
   commands per second, deadlines, and a per-call cap that keeps a native's cost inside
   L3 and L4). Two kinds of number stay in the defining spec and need no L-number:
   (a) **argument-validity domains**, the range of one scalar argument or field that
   bounds no cost by itself (coordinate magnitudes, angles, sizes and offsets of one UI
   node, distances of one camera or label setting, enumerated integers such as a region
   number, numeric bounds of a setting, the integer bound of a grant record, a timer's
   longest delay); (b) **grammar lengths**, the length of an identifier or name that
   only fits it to its grammar (setting keys, capability names, module-name segments,
   version parts, action and context names, a walk file's entity name). A domain or
   grammar length on which a TM rule or bound depends is registered all the same (for
   example L38, L68, L83). A file read bounded at "cap + 1 bytes" to detect an oversized
   file belongs to the cap's own row. A companion spec claims an L-number only for the
   numbers this rule registers.

---

<a id="s1"></a>
## 1. Provenance

| Material | Source | Mark |
|---|---|---|
| Everything in §3–§14 | Original design by the Tessera project, 2026-10-07, from D17 (all sections, including "Open security and design items", "Other improvements", "Required follow-ups" and its revisions 1–6), 0004, 0014–0016, LEGAL §9.5 and §18, LPA, PR, KM, CW, UB and `docs/perf/reference-machine.md` | **Tessera Decision** |
| GPU cost of one blended full-screen layer at 3840 × 2160 on the reference machine (about 0.3 ms) | UR "Open review items" (the U2 note: an 8.3 Mpx blended quad is about 100 MB of traffic per frame, close to 0.3 ms on the reference GPU) | Estimate; E9 measures it (§7.1, OPEN O-1) |
| Reference display and GPU | `docs/perf/reference-machine.md` (RTX 2070 Super Max-Q; 3840 × 2160 panel) | Recorded |
| Revision 2: reconciliation with EM, EA1a and EA1b (§0.1, §7.1, §7.6, §8.1, §8.2, §8.10, §8.14, §8.17, §8.21, §8.23–§8.25, §10.6, §11.10, §13) | Original design by the Tessera project, 2026-10-07, from the review of the four M5.5 specs; where two specs differed, the stronger security rule was kept | **Tessera Decision** |
| Revision 3: the re-review of the four M5.5 specs (§0.1, §2, §7.4, §7.5, §7.6, §8.7, §8.8, §8.13, §8.21, §8.23, §10, §11, §12, §13) | Original design by the Tessera project, 2026-10-07; where a companion's re-review found TM open, the stronger security rule was adopted | **Tessera Decision** |
| Revision 4: the re-review of revision 3 and the follow-up reviews of the four M5.5 specs (§0.1, §2, §6, §7.1, §7.2, §7.4, §7.5, §7.6, §8.3, §8.4, §8.7, §8.8, §8.13, §8.14, §8.16, §8.17, §8.23, new §8.26, §10, §11, §12, §13, §14) | Original design by the Tessera project, 2026-10-07; where a companion's review found TM open, the stronger security rule was adopted | **Tessera Decision** |
| Revision 5: the re-review of revision 4 and the companions' re-reviews (§8.2, §8.5, §8.8, §8.14, §8.23, §10, §11, §12, §13, §14) | Original design by the Tessera project, 2026-10-07; where a companion's review found TM open, the stronger security rule was adopted | **Tessera Decision** |
| Revision 6: the re-review of revision 5 and the companions' re-reviews (§6.8, §7.2, §7.5, §7.6, §8.3, §8.5, §8.7, §8.13, §8.23, §10, §11, §12, §13) | Original design by the Tessera project, 2026-10-07; where a companion's review found TM open, the stronger security rule was adopted | **Tessera Decision** |
| Pixel-count bound of rule 7.1.2 (a convex region covers at most area + half perimeter + 1 lattice points) | Standard plane geometry | Public mathematics |

No dirty-room material, no third-party source and no decompiler output was read. Lua
5.1.5 behaviour cited here (the count hook, `table.foreach`, `string.gfind`, the C-stack
limit, the nested C-call counter and its per-coroutine count at a resume) is public
MIT-licensed library behaviour, which D17 §1.5 allows implementers to read.

---

<a id="s2"></a>
## 2. Terms and conventions

| Term | Meaning |
|---|---|
| **Ext script**, **script** | A script for Tessera's extended API (global `Ext`). "Plugin" always means a standard plugin (D17 abbreviations). |
| **Apartment** | One Lua state of one ext script (D17 §1.2). |
| **Call** | One metered entry into an apartment: a 6b turn's filter call (§7.5), a 6b delivery, a timer, `Tick`, a top-level run, an `Unload` handler or a developer REPL run (L85). Each has a 4 ms deadline and 800,000 work units (D17 §8). |
| **Turn** | All calls one script gets in 6b for one round-robin visit: its filter pass (§7.5), its queued notifications in order, its due timers, then `Tick`. A turn may be split across frames by the 2 ms cap (§8.13). |
| **Gesture** | A user input that D17 roots trust in: the start of one of the script's own actions, an `/ext:` line for it, or a **solid press** (§7.2) on one of its windows. Of a solid press, only the delivery of its **press** notification is a gesture; the release and click that follow carry the press's `gesture` flag for information only and open no window (§8.13). |
| **Gesture window** | The turn that delivers a gesture, from the call that delivers it to the end of that turn's `Tick`, across frames if the turn is split, and closed earlier by the bounds of §8.13 (the user's next input elsewhere, or 1 s, L93). Every spec uses exactly this window (D17 "Other improvements"). |
| **Solid press** | A user press whose hit element in an ext window paints its own background colour or image at the press point with alpha × effective opacity ≥ 0.5 (§7.2). |
| **Screen** | One full viewport of the current presentation: width × height in physical pixels. In deterministic mode and in CI the screen is the harness's virtual surface, **1920 × 1080** unless the walk file sets another (EM §11.5), so every count repeats. Coverage is measured in screens. |
| **Coverage** | A conservative count of the physical pixels that a set of overlay primitives or composition entries can write, before depth testing, divided by one screen (§7.1). Conservative means never below the pixels actually written. |
| **File index** | The per-(kind, id) map from `Store` key to the counted size of the key's file on disk (§7.6). |
| **Floor frame** | A frame in which ext runs although the heavy-frame gate would skip it (§7.4). |
| **Pending-write map** | The per-(kind, id) map of `Store` values written by a script and not yet renamed into place on disk (§7.6). |
| **Drivable action** | An action on the closed allow-list of §7.7. |
| **Harness codes** (§11) | **U**: unit test in the named crate, CI. **ST**: `tessera-scripttool escape` (EM §11.8), which runs the case with the `test` machinery in deterministic mode on a synthetic world, CI. **CI**: client integration test, headless, synthetic world, test hooks, CI. **RM**: run on the reference machine (E-budgets). **FZ**: fuzz target in `fuzz/`. |
| **Slice** | D17's delivery slices: 1a (M5.5), 1b, 2 (after M6), 3 (with M7), 4. A test runs in the slice that ships the module it tests. |

Numbers: MiB = 2^20 bytes, KiB = 2^10 bytes. "Per frame" means per pass of D17 §4's frame
model. Section numbers of D17 are written "D17 §n"; this spec's own are "§n".

---

<a id="s3"></a>
## 3. Assets

| ID | Asset | Why it matters | Main owners |
|---|---|---|---|
| A1 | **The user's control** of the character, camera, keyboard focus and pointer | Group O must never take a control over; group A only by lease and grant (D17 §3, §5.4) | `input`, `client`, `ext` |
| A2 | **Frame time and GPU time** (no frame over 50 ms in the run D17 §8 judges) | Responsiveness is the product; the budget is fully allocated (D17 §8 "The frame budget") | `ext`, `client`, `render` |
| A3 | **Memory, worker and cache capacity** of the client | Host exhaustion takes down plugins and the client too | `script`, `ext`, `ext-host`, `assets` |
| A4 | **The user's files and disks**: the install (read-only), Tessera's `data/` and `logs/`, every other file, free space and SSD wear | Loss or corruption is unrecoverable; the install must never be written (CLAUDE.md) | `ext-host`, `settings` |
| A5 | **Decoded data of the user's install** | Personal use only; no export route (LEGAL §11, §18.4) | `ext-host`, `client` |
| A6 | **Plugins**: their behaviour, data, command table, queue, slice and what they can observe | The plugin surface must not grow (LEGAL §17.2 rule 5); plugins behave as with no ext script (D17 §7) | `plugins`, `client` |
| A7 | **Other ext scripts' state**: `Store`, apartment, windows, random generator, client-local entities | Scripts are isolated from each other (D17 §5.6) | `ext`, `ext-host` |
| A8 | **The trust UI**: consent prompts, script manager, lease indicator, notices | Grants are only as good as the prompt's integrity (D17 §5.3) | `client` |
| A9 | **Grants and content pinning** | A grant covers exactly the hashed snapshot (D17 §5.2) | `ext`, `ext-host` |
| A10 | **What reaches a server**, and the official-server rule | LEGAL §9.5 is binding policy; local-only data never leaves (LEGAL §18.4) | `net`, `client`, `ext` |
| A11 | **The user's privacy**: OS identity, paths, clipboard, typed text | D17 §5.6 "Input and the user" | `ext`, `client` |
| A12 | **Process integrity**: memory safety, address layout, no crash, no hang | Failure stays local (D17 §8) | `script`, `ext` |
| A13 | **Liveness of management**: the panic chord, `/ext:` lines, backend switches, unloads | The user must always be able to stop and remove scripts (D17 §5.5) | `client`, `ext` |

---

<a id="s4"></a>
## 4. Attackers and assumptions

<a id="s4-1"></a>
### 4.1 Attackers in scope

| ID | Attacker | Capabilities assumed |
|---|---|---|
| AT1 | **Hostile script author** | Writes any Lua the loader accepts, any manifest, and receives **every** capability the user may grant (group O and group A, `memory_mib` = 64). Runs as many scripts as the limits allow. This is the default attacker for every test. |
| AT2 | **Hostile update** | A script that was benign when granted and changes its content later (a new version, a hand edit, a swapped folder). |
| AT3 | **Cooperating programs** | Two or more ext scripts by the same author, or an ext script and a plugin, that try to pass data or effects between each other. |
| AT4 | **Hostile plugin** | A standard plugin that targets ext: registers crafted command names for the bridge, writes plugin data for bridge reads, builds `Alias` quickslots with `/ext:` lines, echoes bridged arguments. |
| AT5 | **Hostile file content** | Hand-edited `Store` files, manifest, grant store, `keymap-ext.toml`; a file the user picks; archives, links and special files inside a script folder; another program that changes the folder while the host reads it. |
| AT6 | **Server operator** (slice 3) | The project-owned Tessera-protocol test server, or a proxy the user runs, sending hostile `Server` messages and policies. |
| AT7 | **Local reader** | Another program or a terminal that follows the script log (`tail -f`), or reads `data/script-store/`. |

<a id="s4-2"></a>
### 4.2 Out of scope (stated limits)

- **Modified builds** and user-run proxies that translate Tessera's protocol to another
  service (LEGAL §9.5 item 3). The design answer is "automation offline only" (D17 §6.5).
- **Malware with the user's OS privileges**, physical access, and the user deliberately
  acting against their own interest (for example binding an ext action over `W` by hand,
  D17 §5.5).
- **Micro-architectural side channels.** Scripts get one frame-clock value per frame and
  no high-resolution timer (`os` is absent, D17 §1.2), which keeps timing resolution at
  frame granularity; this spec claims nothing finer.
- **Covert channels between cooperating programs** that the user runs together, listed
  exactly in §6.8 and LEGAL §18.4. They are residuals, not threats this spec closes.

<a id="s4-3"></a>
### 4.3 Assumptions

1. The patched Lua 5.1.5 build behaves as D17 §1.5 states (verified by OPEN items of D17
   and by X12–X14 here).
2. The OS random source is sound (hash secrets, salts, per-apartment generators).
3. Tessera's own settings, keymap file and grant store are written only by Tessera and the
   user (they are still parsed as hostile, AT5).
4. M5 delivers the `script`/`plugins` split and IB D8's frame model (D17 §1.1).

---

<a id="s5"></a>
## 5. Trust boundaries

| ID | Boundary | Untrusted side | What crosses it | Guard |
|---|---|---|---|---|
| TB1 | Script ↔ host natives | Lua code and every value it passes | Calls, arguments, tables, returned values | Argument validation from the registry, charge-before-work, bounded value model, sealed metatables (D17 §1.2, §1.3, §8) |
| TB2 | Apartment ↔ apartment | Each script | Nothing directly; shared engine state and pools only | Separate states, generators, `Store`, UI pools per script; residuals in §6.8 |
| TB3 | Ext ↔ plugins | Both sides | Bridge actions and reads (slice 2), `/ext:` lines from `Alias` quickslots, effects on shared state | One-way bridge, direct handler call (§7.3), z-range (§8.26) and focus domain (D17 §4) |
| TB4 | Ext ↔ the user's input and the trust UI | Scripts | Key resolution, pointer routing, activation and focus, prompts | Ext passes last, gesture-rooted focus (§7.2), reserved layer, panic chord (D17 §5.3, §5.5) |
| TB5 | Ext ↔ disk | Scripts and file content (AT5) | Snapshot reads, `Store` files, the script log, `Files.Pick` | Directory-handle opens, folder rules, per-store I/O queue, caps and rates (D17 §2, §8; §7.6, §8.5–§8.8) |
| TB6 | Ext ↔ backend and server | Scripts; the server (AT6) | Intents, `Server` messages, policy | Backend class gate, local-only rule, session generations (D17 §5.2, §6) |
| TB7 | Ext ↔ frame, GPU and workers | Scripts | Work in 6a–6c, ext UI time, overlay and composition GPU work, worker jobs | Deadlines, units, caps, gate and floor, coverage bounds (D17 §4, §8; §7.1, §7.4, §7.5) |
| TB8 | Ext ↔ other local programs | The script log and `Store` files as output | Bytes another program reads | Log sanitising (§8.1), caps |
| TB9 | Build ↔ compatibility tier | Future gated code | Linking | `gated-compat` tag, CI check, runtime gate (D17 §6.3) |

---

<a id="s6"></a>
## 6. Threat catalogue

Each row names one threat, the assets (§3), attackers (§4), boundary (§5), the
requirement that answers it and its escape test (§11). "D17 §n" points to an accepted
requirement; "§n" points to a rule of this spec.

<a id="s6-1"></a>
### 6.1 Code execution and sandbox escape

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T1 | Data becomes code: `load`, `loadstring`, `string.dump`, binary chunks, a string from `Store`, `Files.Pick`, plugin data or `Server` | A9, A12 | AT1, AT6 | TB1 | Globals exactly D17 §1.2's allow-list; text-only compilation of snapshot files; no compiling of strings (D17 §5.6) | X1 |
| T2 | A Lua function placed on a host metatable runs unmetered in 6c or `lua_close` | A2, A12 | AT1 | TB1 | Sealed host metatables, no script finalizers, no `newproxy` (D17 §1.2) | X2 |
| T3 | `tostring` leaks heap addresses (defeats address randomisation) | A12 | AT1 | TB1 | Per-apartment numbering (D17 §1.2) | X3 |
| T4 | The host runs script code to describe an error value after the call's deadline | A2 | AT1 | TB1 | No metamethods on host description paths (D17 §1.2) | X4 |
| T5 | A script replaces globals or the string metatable that a host shim written in Lua uses | A12, A9 | AT1 | TB1 | Shims capture at creation; never call string methods (D17 §1.2) | X5 |
| T6 | Plugin names reach ext, or `Ext` reaches a plugin | A6 | AT1, AT4 | TB3 | Separate profiles; tests in `tessera-ext` and `tessera-plugins` (D17 §12) | X6 |
| T7 | A stock function reached through the string metatable bypasses its wrapper | A2 | AT1 | TB1 | One wrapped `string` table as `__index` (D17 §8) | X7 |
| T8 | Compatibility functions (`table.foreach`, `table.foreachi`, `string.gfind`, `math.mod`, `table.getn`, `table.setn`) loop natively without charge | A2 | AT1 | TB1 | §8.15 (removed from the ext profile) | X8 |
| T9 | `pcall`, `xpcall` or a coroutine catches the budget abort and keeps running | A2 | AT1 | TB1 | §8.11 (sticky abort) | X9 |

<a id="s6-2"></a>
### 6.2 Time and CPU

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T10 | Endless loop or stalled tick | A2 | AT1 | TB7 | 4 ms deadline, 800,000 units (D17 §8) | X10 |
| T11 | Costly single instructions (concatenation, comparison, resize, `next`) between hook checks, in the main thread or a coroutine | A2 | AT1 | TB7 | Work-proportional hook counting; coroutines inherit the hook (D17 §1.5, §8) | X11 |
| T12 | Degenerate wrapper arguments (`string.rep("", 2^31−1)` and kin) | A2 | AT1 | TB1 | §9.1 | X12 |
| T13 | Hash flooding of Lua tables, the interning table or the compiler | A2 | AT1 | TB1 | §9.2 | X13 |
| T14 | Hash flooding moved into host maps (keys from scripts, files or messages) | A2, A3 | AT1, AT5, AT6 | TB1, TB5 | §9.2 (keyed hashers) | X14 |
| T15 | Deep C recursion (callbacks, metamethods, nested coroutine resumes, deep parse nesting) overflows the native stack and crashes the process | A12 | AT1 | TB1 | §9.6 (nested C-call bound counted across resumes, stack reserve); Lua error, never a crash (D17 §8, E6) | X15 |
| T16 | Uninterruptible spikes: collector atomic step (S), state close (C), compilation (K), worst operation (R) | A2 | AT1 | TB7 | Measured bounds of L12 (7 ms + R + 2·S and 2 ms + K + S, or C for K, within the ceiling in force) and of L14–L16 for the slice, recorded fallback (D17 §8) | X16 |
| T17 | Many scripts together overrun the frame | A2 | AT1, AT3 | TB7 | 2 ms cap, 1 ms slices, rolling throttle, suspension (D17 §8) | X17 |
| T18 | Repeated aborting calls | A2 | AT1 | TB7 | 3 aborts in 10 s suspends (D17 §8) | X18 |
| T19 | Entity-subscription filtering outside every bound (D17 open item 5) | A2 | AT1 | TB7 | §7.5 | X19 |
| T20 | Costly value conversion (maximal values, shared subtables, deep chains, metamethods) | A2, A12 | AT1, AT5 | TB1 | Bounded value model, conversion inside metered calls (D17 §8) | X20 |
| T21 | Keymap recompilation storms from ext registrations | A2 | AT1 | TB7 | §8.18 | X21 |
| T22 | A locale change makes string comparison slow or changes order | A2 | AT1 | TB7 | Byte-wise order, locale pinned to "C" (D17 §1.5) | X22 |

<a id="s6-3"></a>
### 6.3 Memory and host objects

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T23 | Lua heap growth and single huge allocations | A3 | AT1 | TB1 | 32/64 MiB cap, 4 MiB single allocation (D17 §8) | X23 |
| T24 | Host objects grow outside the memory cap | A3 | AT1 | TB1 | Charged host objects with count caps (D17 §8) | X24 |
| T25 | Worker-request floods; result reservations that collide with the memory cap | A3 | AT1 | TB7 | Outstanding caps (D17 §8) and §8.14 | X25 |
| T26 | Image decode floods outside the outstanding cap | A3 | AT1 | TB7 | §8.5 | X26 |
| T27 | The snapshot and the retained entity-event lists are uncharged host memory | A3 | AT1 | TB1 | §8.10 and §7.5 | X27 |
| T28 | Many scripts or a hot reload exceed the memory sum | A3 | AT1 | TB7 | 512 MiB sum, 64 MiB reload allowance, 16 scripts, 17 apartments (D17 §8) | X28 |

<a id="s6-4"></a>
### 6.4 GPU and UI resources

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T29 | Overlay overdraw: full-screen quads and near-camera boxes (D17 open item 1) | A2 | AT1 | TB7 | §7.1 | X29, X30 |
| T30 | Ext composition overdraw: many full-screen ext surfaces (D17 open item 1) | A2 | AT1 | TB7 | §7.1 | X31 |
| T31 | Ext UI resources starve plugins and native panels | A6, A2 | AT1 | TB7 | Separate ext pools on top of UR's caps (D17 §8) | X32 |
| T32 | Image bombs: huge dimensions or decoded sizes | A3 | AT1, AT5 | TB5 | Header checks, 4,096 px per axis, charged before decoding (D17 §8) | X33 |

<a id="s6-5"></a>
### 6.5 Input, focus and control takeover

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T33 | An ext binding takes a press that a built-in binding matches (either pass) | A1 | AT1 | TB4 | Ext passes after both non-ext passes; collision rule (D17 §5.5) | X34 |
| T34 | The same at releases (`release`, `tap`) | A1 | AT1 | TB4 | D17 §5.5 release order | X35 |
| T35 | An ext binding fires while the user types | A1, A11 | AT1 | TB4 | `opaque_keys` band end stops ext passes (D17 §5.5) | X36 |
| T36 | A script focuses its own text box by itself and swallows movement keys | A1, A11 | AT1 | TB4 | Gesture-rooted activation and focus (D17 §4) and §7.2 | X37 |
| T37 | A transparent full-screen element turns every world click into an activation, then a timer takes the keyboard (D17 open item 2) | A1, A11 | AT1 | TB4 | §7.2 | X38 |
| T38 | Focus moves into a hidden, transparent, off-screen or covered text box | A1, A11 | AT1 | TB4 | Visible-and-uncovered rule with re-checks (D17 §4) | X39 |
| T39 | Ext context toggling cancels the user's pending sequences | A1 | AT1 | TB4 | D17 §5.5 and rate limit | X40 |
| T40 | Ext holds exhaust KM §7.3's 256 holds (D17 other item) | A1 | AT1 | TB4 | §8.16 | X41 |
| T41 | The panic chord is consumed, rebound or unreachable | A1, A13 | AT1 | TB4 | Chord tier first; leases end on focus loss (D17 §5.4, §5.5) | X42 |
| T42 | `Input.Drive` presses actions outside movement: screenshot files, menus, chat focus, plugin handlers in step 3 (D17 open item 7) | A1, A4, A6 | AT1 | TB4 | §7.7 | X43 |
| T43 | A lease outlives the user's ability to end it (stall, starvation, window focus loss, a queued backend switch, a revoked grant, an unload, a lease taken in an `Unload` handler), or steers what the user's own click picks | A1 | AT1 | TB4 | 350 ms expiry, end on focus loss, watchdog, chord (D17 §5.4) and §8.17 | X44 |
| T44 | World labels catch the pointer and move boxes in step 8 (D17 open item 8) | A1, A6 | AT1 | TB4 | §7.8 | X45 |
| T45 | A script synthesises input that presses "Allow" | A8, A9 | AT1 | TB4 | No synthesised input; pointer-only Allow (D17 §5.3) | X46 |

<a id="s6-6"></a>
### 6.6 Spoofing, the reserved layer and output integrity

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T46 | Manifest or error strings impersonate Tessera: markup, bidi, invisible and format characters, stacked combining marks | A8 | AT1, AT4 | TB4 | Plain text rules (D17 §5.3) and §8.2 | X47 |
| T47 | Prompt clickjacking: a resting pointer, lured rapid clicks, a key answer | A8, A9 | AT1 | TB4 | 500 ms delay, pointer entry, hidden ext output, keys only refuse (D17 §5.3) | X48 |
| T48 | An ext window imitates a native panel or a Tessera dialog to capture typed secrets, also by placing its badge under a plugin window or native panel, or off screen | A11, A8 | AT1 | TB4 | §8.3 (host-drawn ownership badge, placed wholly on screen; no focus while it is covered) | X49 |
| T49 | Script-caused reserved-layer surfaces occlude plugin controls | A6 | AT1 | TB3 | Hit- and hover-transparent notices and indicator (D17 §5.3) | X50 |
| T50 | Log injection: terminal escapes (OSC 52 clipboard writes), forged host lines (D17 other item) | A11, A8 | AT1, AT7 | TB8 | §8.1 | X51 |
| T51 | Pastejacking: the script changes a box's text between the user's selection and the copy, or fills it beforehand with text the user cannot see (clipped, scrolled out, transparent, invisible characters) (D17 other item) | A11 | AT1 | TB4 | §8.4 | X52 |

<a id="s6-7"></a>
### 6.7 Files and disks

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T52 | Path escape: `..`, absolute paths, Windows device names, script-chosen bare file names | A4 | AT1 | TB5 | Fixed prefixes, grammars, directory-handle opens (D17 §2) | X53 |
| T53 | Links, hard links, reparse points and swaps during the walk; a symlinked script folder root (D17 other item) | A4, A9 | AT5 | TB5 | No-follow walk (D17 §2) and §8.8 | X54 |
| T54 | Folder-walk exhaustion (millions of entries, huge directories) | A2, A3 | AT5 | TB5 | 512 entries, 64 directories, counted as read (D17 §2) | X55 |
| T55 | Disk exhaustion through `Store`, the log or leftover temporary files | A4 | AT1 | TB5 | Key, size and log caps (D17 §8) and §8.7 | X56 |
| T56 | Disk wear: sustained `Store` rewrites and log floods by many scripts (D17 other item) | A4 | AT1, AT3 | TB5 | §8.7; the all-scripts log rate (L45) | X57, X56 |
| T57 | `Files.Pick` on a FIFO or device blocks a worker forever (D17 other item) | A3, A13 | AT1 | TB5 | §8.6 | X58 |
| T58 | Hostile parsed files: `Store` values, manifest, grant store, `keymap-ext.toml`, bridge plugin data | A12, A3 | AT5, AT4 | TB5 | Bounded strict parsers with fuzz targets (D17 §2, §5.2, §8) | X59 |
| T59 | A crash leaves half-written values or values with no local-only mark | A4, A5 | AT1 | TB5 | Temp file and rename; durable marker before the top-level run (D17 §5.2, §8) | X60 |
| T60 | `keymap-ext.toml` written through a link into the install or elsewhere (D17 other item) | A4 | AT5 | TB5 | §8.19 | X61 |
| T93 | A link, reparse point, subdirectory, special file, hard-linked file or unknown entry at a store directory `s_<id>`, inside it, or at a script log path leads a store open, read, flush, wipe or log write outside Tessera's directories, blocks a worker, or deletes the user's files | A4, A7, A13 | AT5 | TB5 | §8.8 (store directories, their entries and log files are opened without following links; entry-kind rule) | X94 |

<a id="s6-8"></a>
### 6.8 Exfiltration, privacy and cross-script isolation

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T61 | One script holds local-only data and the server channel | A5, A10 | AT1 | TB6 | Local-only capabilities never with `session.server` (D17 §5.2 rule 1) | X62 |
| T62 | Local-only `Store` content reaches a later `session.server` holder (late write after the wipe) | A5, A10 | AT1, AT2 | TB5, TB6 | Durable marker, wipe, one I/O queue per store (D17 §5.2 rule 3) | X63 |
| T63 | A hot-reload hand-off table crosses a change of grants | A5, A10 | AT2 | TB6 | No hand-off across a capability change (D17 §5.2 rule 4, §10) | X64 |
| T64 | The bridge reaches a built-in command (for example a chat channel) on a server (D17 open item 3) | A10, A6 | AT1, AT4 | TB3, TB6 | §7.3 | X65 |
| T65 | `/ext:` lines (possibly private text) reach a backend | A10, A11 | AT1, AT4 | TB6 | Never forwarded (D17 §3 `Commands`) | X66 |
| T66 | OS identity leaks: home paths in chunk names, errors or `Files.Pick` results | A11 | AT1 | TB1 | Relative chunk names, basename only (D17 §1.2, §5.6) | X67 |
| T67 | Shared decode cache tells one script what another requested (D17 other item) | A7 | AT3 | TB2 | §8.9 (per-script visibility) | X68 |
| T68 | A script reads another script's `Store`, windows, generator or local entities | A7 | AT1 | TB2 | D17 §5.6 | X69 |
| T69 | Pointer notifications reveal other windows' shapes (layout residual) | A6, A7 | AT1 | TB2, TB3 | Notifications only over own pointer-catching elements; residual stated (D17 §5.6) and §8.9 | X70 |

**Residuals, stated exactly (not closed by this spec).** As D17 §5.2 and LEGAL §18.4
record: (1) cooperating scripts through shared engine state (game clock, native panel
positions), through global pools answering "busy" (the all-scripts log rate of L45
included: a script's log records can be dropped, and counted, while other scripts fill
it), and across sessions through the `Store` totals answering "full"; (2) a script and a plugin through bridged command
arguments a plugin keeps, an `Alias` quickslot the user later clicks, plugin data, frame
timing and window layout; (3) the layout residual of D17 §5.6, which also holds **between
ext scripts** (a lower ext window learns where higher ext windows catch the pointer,
§8.9); (4) timing of shared decode work: a request that joins another script's
in-flight decode, or whose entry another script already made resident, can complete
sooner than a fresh decode would, at frame granularity (§8.9); (5) the number of ext
scripts drawing overlays in a frame, which sets each script's fair share (§7.1.4); (6) the
end reasons of a camera lease (EA1b §4.5, §4.10): a script holding only `camera` learns,
from `LeaseEnded{reason = "region"}` or `"focus"`, that the player's region changed or that
the game window lost keyboard focus, at frame granularity, at most once per lease and only
while it holds one. It learns no region number, position or other application. This is
accepted: each is one event per lease at frame granularity carrying no number; the lease
ends at these events so that it fails safe (§8.17, D17 §5.4 for the focus loss), and
hiding the reason would not hide the end itself, which a renewing script observes as its
lease ending early.
The decode-cache **content** channel of T67 is closed, not residual. Overlay coverage is
**not** a channel to the camera: the coverage-dependent overlay statistics (culled,
drawn, coverage, drops by coverage) reach only a script that holds `world.read`
(rule 7.1.4).

<a id="s6-9"></a>
### 6.9 Plugins and coexistence

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T70 | Ext work runs plugin code inside ext time or outside steps 1 and 7 (including through game-driven UI changes after `Sandbox` or `Input.Drive` effects) | A6, A2 | AT1 | TB3 | D17 §4, §7; §7.7; §8.20 | X71 |
| T71 | Ext changes the plugins' command table, lists or `IsCommand` answers | A6 | AT1 | TB3 | `ext:` words outside the plugin lookup (D17 §3) | X72 |
| T72 | Ext text reaches chat and so `Turbine.Chat.Received` | A6 | AT1 | TB3 | Ext reports never in chat (D17 §7 "Chat") | X73 |
| T73 | Random sequences cross between ext and plugins or between scripts | A6, A7 | AT1, AT3 | TB2, TB3 | §9.5 | X74 |
| T74 | Bridge floods the plugin queue | A6, A2 | AT1 | TB3 | 1 action/s, load caps (D17 §7) | X75 |
| T75 | A hostile plugin's data overwhelms a bridge read | A3, A12 | AT4 | TB3 | Value-model caps on the worker (D17 §7 item 4) | X76 |
| T76 | A plugin's `Alias` `/ext:` line injects hostile arguments | A9 | AT4 | TB3 | Arguments are data; the line never reaches a backend or chat (D17 §7) | X77 |
| T94 | An ext window, nested surface, label or badge is drawn or hit-tested above a plugin window or a native panel (through `ZOrder`, `Activate`, re-parenting, a nested surface or a show while a prompt shows), covering it, taking its clicks, or making it look like part of the ext window | A6, A1, A8 | AT1 | TB3, TB4 | §8.26 (the ext z-range is below every plugin window and native panel for drawing and hit testing) | X95 |

<a id="s6-10"></a>
### 6.10 Backend gate and servers

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T77 | Ext runs on an `Other` backend (and so, potentially, an official server) | A10 | AT1 | TB6 | Fail-closed gate, no override (D17 §6.2; LEGAL §9.5) | X78 |
| T78 | A wrapper backend reports `Offline` by delegation | A10 | AT1 | TB6 | Class decided by `Session` from a sealed trait (D17 §6.1) | X79 |
| T79 | An intent outlives its apartment, session or backend | A10, A1 | AT1 | TB6 | Session generations, apply-time checks (D17 §4) | X80 |
| T80 | A build links ext and a compatibility backend | A10 | — | TB9 | `gated-compat` check (D17 §6.3) | X81 |
| T81 | Hostile `Server` messages (size, rate, depth, keys) | A3, A12 | AT6 | TB6 | Value model, 64 KiB, rates (D17 §8) | X82 |
| T82 | Through-wall views on a server | A10 | AT1 | TB6 | Residual; only a policy without `entity.read` stops it client-side (D17 §6.5) | X83 |
| T83 | Automation (`Input.Drive`, `Sandbox`) on a server | A10 | AT1, AT6 | TB6 | Offline only, no policy can enable (D17 §6.5) | X84 |

<a id="s6-11"></a>
### 6.11 Liveness and consistency

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T84 | The heavy-frame gate starves ext forever; backend switches never finish (D17 open item 4) | A13 | AT1 (or plain load) | TB7 | §7.4 | X85 |
| T85 | The gate counts idle time between frames (D17 other item) | A13, A2 | — | TB7 | §8.12 | X86 |
| T86 | Partially loaded apartments at a backend switch (D17 other item) | A13, A10 | — | TB6 | §8.21 | X87 |
| T87 | `Store` reads return stale values, including across a hot reload (D17 open item 6) | A7, A9 | — | TB5 | §7.6 | X88 |
| T88 | A gesture is lost when the 2 ms cap or the gate defers the script's turn (D17 other item) | A1, A13 | — | TB4 | §8.13 | X89 |
| T89 | One id loaded from two folder kinds; grants reused for new code | A9 | AT2 | TB4 | One script per id; re-ask group A on content change (D17 §2, §5.2) | X90 |

<a id="s6-12"></a>
### 6.12 Grants, authors and the user's files at run time

| ID | Threat | Assets | By | TB | Requirement | Test |
|---|---|---|---|---|---|---|
| T90 | A revoked grant keeps working: a kept module table, a running lease, queued intents, live subscriptions, a `Follow` of an entity | A1, A9, A10 | AT1 | TB1, TB4 | §8.23 (revocation suspends in that frame) | X91 |
| T91 | A `Store` or a hot-reload hand-off reaches a script by another `author` (a developer hot reload across an `author` change, a store left without its grant record) | A7, A9 | AT2, AT5 | TB5 | §8.24 | X92 |
| T92 | Ext registrations and bindings change or corrupt the user's own keymap file | A1, A4 | AT1, AT5 | TB5 | §8.19 (ext bindings only in `keymap-ext.toml`) | X93 |

---

<a id="s7"></a>
## 7. Requirements for D17's open security and design items

Each subsection states the problem in one paragraph, the rule (normative), the numbers,
what it changes in D17's budget or enumeration, and its tests.

<a id="s7-1"></a>
### 7.1 GPU overdraw of ext overlays and ext composition (open item 1)

**Problem.** D17 limits overlays by primitive count only (10,000 per script, 20,000 in
all) and ext composition by entry count only (1,024 entries). A screen quad, or a world
box near the camera, can each cover the whole screen, and every ext composition entry can
be a full-screen surface. 20,000 blended full-screen quads at 1920 × 1080 are about 41
Gpixels per frame, hundreds of milliseconds of GPU time in step 10, which no ext time
measure sees and the throttle never suspends.

**Rule 7.1.1 (coverage is counted).** The host counts **coverage** (§2) for every overlay
primitive when the overlay lists are encoded for step 10, and for every ext composition
entry at the check points of rule 7.1.5. Coverage is counted before depth testing: a
depth-tested primitive hidden behind the world still counts, because rejected fragments
still cost rasterisation. Every count is **conservative**: never below the pixels the GPU
can write for it.

**Rule 7.1.2 (how each primitive and entry counts).** In physical pixels of the current
viewport (§2 "Screen"), then divided by one screen. "Counted outward" means a real
rectangle [x0, x1] × [y0, y1] counts (⌈x1⌉ − ⌊x0⌋) × (⌈y1⌉ − ⌊y0⌋) pixels (0 if empty),
which covers every pixel whose centre lies in it. "Counted by size" means a rectangle of
width *w* and height *h* counts (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1) pixels (0 if *w* or *h* is 0):
an interval of length *w* contains at most ⌊*w*⌋ + 1 pixel centres wherever it lies, so
this count is never below the pixels the rectangle covers at any position, sub-pixel
phases included, and it does not depend on the position at all.

| Primitive or entry | Counted pixels |
|---|---|
| Any overlay primitive whose colour alpha is 0 | 0; it is not drawn |
| Any overlay primitive entirely outside the view frustum (every point outside one frustum plane, the near plane included) | 0; it is culled and not drawn |
| Screen quad | Its rectangle clipped to the viewport, counted outward |
| Line (screen or world) of width *w* pixels (*w* ≥ 1) | Clip the segment to the near plane, project both ends, and clip the 2D segment to the viewport **expanded by *w*/2 on each side** (so a wide line whose centre line lies just outside the viewport still counts the pixels it paints inside); with *L* the clipped length in pixels plus *w* (the end caps), count ⌈*L*·*w* + *L* + *w* + 1⌉. This is area + half perimeter + 1 of the line's rectangle, an upper bound on the pixel centres any convex shape covers, so it holds at every angle. A line whose segment misses the expanded viewport counts 0 |
| World box, drawn as edges | The sum over its twelve edges, each counted as a line (an edge entirely outside the frustum counts 0). A straddling outline box is therefore a set of clipped lines, never a full screen |
| World box, drawn filled, eye inside the box | 0: the overlay pipeline draws filled boxes with back faces culled, so a box seen from inside writes nothing (EA1b §5.4). A renderer that does not cull back faces of filled boxes counts **one full screen** here instead |
| World box, drawn filled, eye outside, all eight corners in front of the near plane | The axis-aligned rectangle around its eight projected corners, clipped to the viewport, counted outward. With back faces culled a convex box writes each pixel at most once |
| World box, drawn filled, eye outside, some corner on or behind the near plane, not culled | **One full screen** |
| Ext composition entry: a window, a nested surface, a world label, an ownership badge (§8.3) | min(one screen, its **displayed rectangle counted by size**), where the displayed rectangle is the axis-aligned rectangle around the entry after its size, scale and rotation, in physical pixels, **not clipped to the screen**; the count depends only on its width and height, never on its position or sub-pixel phase. Nested entries count again, because they are drawn again. A badge counts its fixed box (§8.3) |

Because an entry's count depends only on its size, moving a window or label (by the
script, by the user's drag, or by step 8 projecting a world label, by whole or fractional
pixels) can never raise coverage: a script cannot create windows that are mostly off
screen and then slide them on screen unchecked. The count is never below the pixels the
entry can cover at any position, so it stays conservative.

**Rule 7.1.3 (bounds).**

| Bound | Value |
|---|---|
| Overlay coverage per script per frame | **0.5 screen** |
| Overlay coverage, all ext scripts, per frame | **1.0 screen** |
| Ext composition coverage per script | **1.25 screens** |
| Ext composition coverage, all ext scripts | **2.0 screens** |

These replace EA1b §6.3's 2·S/3·S figures (the stricter values are kept, §13).

**Rule 7.1.4 (overlays: fair shares, drop and go on).** In step 10, let *n* be the number of
ext scripts whose shown overlay list is non-empty in this frame. Each such script gets a
**count share** *c* = min(10,000, ⌊20,000 / *n*⌋) primitives and a **coverage share**
*A* = min(0.5, 1.0 / *n*) screens. For each script, independently of every other script:

1. Only the first *c* primitives of its shown list, in list order, are considered; the rest
   are dropped (D17 §8, "the newest are dropped").
2. The considered primitives are visited in list order with a running sum Σ = 0. A
   primitive counted 0 by rule 7.1.2 is skipped and not drawn. Otherwise, if Σ + a(p) ≤ *A*
   the primitive is drawn and Σ grows by a(p); if not, that primitive alone is dropped and
   the visit goes on (a later, smaller primitive may still fit).
3. A primitive is never partly drawn.

So no script's overlay passes 0.5 screen, all together never pass 1.0 screen and 20,000
primitives, and one script's result depends on the others only through *n* (a residual,
§6.8). **Reporting:** the counts dropped by the count share and by the coverage share are
kept per frame, shown in the script's profiler line, and written to its script log at most
once per 10 s (L91). The script's own overlay statistics (EA1b states how a script reads
them) give it the count-share drops, which do not depend on the view; the figures that do
(primitives culled or drawn, coverage, drops by coverage) are given only to a script that
holds `world.read`, because a script with `overlay` alone could otherwise probe them frame
by frame for the camera's position and facing. Drops are not aborts: they never throttle
or suspend the script, because the bound already caps the cost.

**Rule 7.1.5 (composition past the bound is refused).** The composition check runs at
creation, at every show, at every change that makes hidden content visible (D17's entry
check points), at every change of an entry's size, scale or rotation, at every
re-parenting, when a window gains its badge (§8.3), at every show of a world label, and at
every screen resize or UI scale change. Position changes need no check (rule 7.1.2).

- A **script call** that would pass either bound is refused and changes nothing: a show
  leaves the window hidden and returns `false`; a size, scale, rotation or re-parenting
  change leaves the geometry unchanged and returns `false`; a creation of a shown entry,
  or an addition that gives a shown window its badge (its first pointer-catching element
  or text box), answers `nil, "limit"` and creates or adds nothing; the script log says
  why.
- A **user resize** of an ext window stops growing the window at the largest size that
  keeps both bounds. A user drag is never limited (position does not count).
- When a **host change** (a screen resize, a UI scale change) puts a script or the total
  over its bound, the host hides ext **windows and world labels**, across all scripts, in
  reverse order of their last show (the most recently shown first), until every script
  and the total fit. Hiding every entry brings any script to 0, so the rule always ends,
  also for a script whose coverage is mostly labels. Each hidden window or label's script
  gets `VisibilityChanged{visible = false, by = "host"}` (EA1a §7.10's name) in its next
  turn, and the script log records it. This order replaces EA1b §6.4's "lowest in the ext
  z-order first" (§13).

**Rule 7.1.6 (budget row and E9).** D17 §8's frame-budget table gains the row **"Ext
overlays and composition: 1.6 ms"**: **0.6 ms** of CPU encoding of up to 20,000 overlay
primitives in step 10 (twice E4's 0.3 ms per 10,000; from slice 1b) and **1.0 ms** of GPU
time for overlays and composition at the coverage bounds (E9). It is funded from the
margin, which becomes **2.6 ms**, and the margin's M5 condition reads "if the non-paint UI
work needs more than 0.6 ms of the margin, the ext ceiling drops by the excess" (leaving
2.0 ms for noise and overhead; OPEN O-2 for the owner's confirmation). Derivation of the
GPU part: the four bounds allow at most 3.0 screens of blended layers per frame. Coverage
is counted in screens, so the pixel cost grows with the display; the worst case is the
reference display's native 3840 × 2160 (8,294,400 pixels), where one blended screen costs
about 0.3 ms on the reference GPU (§1), so 3.0 screens are about 0.9 ms. A new budget
**E9**: "GPU time of ext overlays and ext composition at the coverage bounds, at the
reference display's native 3840 × 2160, timestamp queries: ≤ 1.0 ms" (composition in slice
1a, overlays in slice 1b). If E9 measures more, the coverage bounds shrink in proportion
until it holds; the row never grows without the owner's decision. This row and E9 replace
EA1b §6.5's 1.8 ms row, its 2.4 ms margin and E4b (§13).

**Effect on D17's claims.** With these bounds the "no frame over 50 ms with ext" claim of
the M4.5 walking run holds for a plain group-O `overlay` grant, because ext GPU work and
overlay encoding now have a row. E6 gains the two floods below.

**Worked example.** A waypoint script draws 500 outline boxes and one filled box that the
player stands next to, with one corner behind the near plane. Every other script draws
nothing, so *n* = 1, *c* = 10,000 and *A* = 0.5 screen. The filled box counts one full
screen, does not fit, and is dropped alone; the outline boxes behind the camera are culled
and count 0; the remaining outline boxes count a few thousand pixels each and are drawn.
The profiler line shows "1 dropped (coverage)". A second script now starts drawing: *n* = 2,
so each gets *A* = 0.5 screen and *c* = 10,000 again. With three drawing scripts *A* is 1/3
screen each.

**Tests.** X29 (full-screen quad flood), X30 (near-camera box flood, wide lines just
outside the viewport), X31 (composition flood, off-screen windows moved on screen,
sub-pixel moves, user resizes at the bound, a labels-only script at a host change), all
also run inside E6 and E9.

<a id="s7-2"></a>
### 7.2 Transparent full-screen element, activation and keyboard focus (open item 2)

**Problem.** Under D17 §4 case 1, a press on an ext window activates it. A mouse-visible
ext element that paints nothing and covers the screen turns every world click into such
a press; the pointer never leaves the script's window, so case 1 stays true, and a timer
can then focus a small opaque text box elsewhere whose `text_entry` swallows `W`. That
contradicts why `ui` is group O. This spec adopts **both** fixes D17 offers.

**Rule 7.2.1 (only a solid press activates).** A user press that lands on an ext window
counts as an **activating press** only when it is a **solid press**: the hit element at
the press point draws there its own background colour or image, with alpha (for an
image, the alpha of the texel under the press point) times the element's effective
opacity (its own opacity times every ancestor's, the window's included) **at least 0.5**,
**and** no other element of any ext window (the script's own later siblings, its higher
windows and other scripts' windows included, whether or not they catch the pointer)
paints over the hit element at that point. The test is judged by the **topmost painted
element** at the press point: a press that falls through a non-catching picture onto a
catching element below it is not solid. Text, borders, the host's focus mark and the
ownership badge (§8.3) do not make a press solid. A press on a catching element that sends
no pointer notifications (EA1a's `enabled = false`) is solid or not by the same test and
activates by the same rule, but it delivers no notification and so opens no gesture window.
A press that is not solid:

- is still caught by the ext element (it does not reach the world), and the script still
  receives its pointer notification, marked `gesture = false`;
- for activation and focus is treated **exactly as a click on the world**: it activates
  nothing, deactivates whatever window was active (as a world click does), starts no
  case 1, refreshes no case-1 timer, **raises nothing in the ext z-order** (only an
  activating press moves a script's windows to the top of the ext range), and is not a
  gesture for `Files.Pick`.

**Rule 7.2.2 (a script focuses a text box only in a gesture turn).** A script request that
would give keyboard focus to an ext text box (its `Focus`, an `Activate` or show of a
window whose remembered focus control is a text box) is honoured only during the turn
(§8.13) that delivers a gesture for that script: a solid press on one of its windows, a
start of one of its actions, or an `/ext:` line for it, subject to D17 §4's other
conditions (case 2 needs no active window; the visible-and-uncovered rule always
applies, and "covered" includes the script's own content: any element of its own tree drawn
above the box, and any of its own higher windows, that paints inside the box's focus area,
as well as the box's window badge being covered or off screen, §8.3; and any **badge**
whose fixed box overlaps the box's focus area with a positive area, except the badge of
the box's own window when it is attached **outside** that window's top edge, §8.3). Under case 1 outside
such a turn, requests may still move **activation** among
the script's own windows, but a request that would focus a text box only remembers the
focus control and returns `false`. This replaces D17 §4 case 1's sentence "inside those
limits a timer can still focus a box before the user's next key".

**Rule 7.2.3 (no other route).** No ext call sets a control's text-entry state, pushes
`text_entry` or `ui.clipboard`, or changes the solid test's inputs for a press already
routed. Changing an element's opacity or background after a press does not make that
press solid after the fact.

**Worked example.** A script shows a window covering the screen with one transparent
element that takes pointer input, and a 200 × 30 opaque text box in a corner. The user
clicks the world at (900, 500): the press lands on the transparent element, is not solid,
and is handled for activation exactly as a world click (any active window is deactivated,
nothing is activated); the script receives it with `gesture = false`. Two seconds later a timer calls `Focus` on the box:
it returns `false`. The user presses `W` and walks. If the user instead clicks the opaque
box itself, that solid press focuses it natively in step 1, as D17 §4 allows.

**Tests.** X38 (the transparent full-screen element; also at opacity 0.49, and with an
image of texel alpha 0), and X37 (timer focus after a solid press outside the gesture
turn returns `false`; focus inside the gesture turn returns `true`).

<a id="s7-3"></a>
### 7.3 The plugin-command bridge (open item 3)

**Problem.** D17 §7 runs a plugin's command by queueing a line for step 1 and routing it
like a typed line. That routing re-tokenizes the line and looks the first token up in a
registry that also holds built-ins (CW §7.1–§7.4), and `Shell.AddCommand` keeps inner
whitespace and quotes in names (LPA §11.1). A plugin can register a name such as
`"say"` (with its quotes) or `say x`; the re-tokenized line then yields the built-in word
`say`, which on `TesseraProtocol` sends script-chosen text to the server. The registration
can also change between the queue and step 1.

**Rule 7.3.1 (resolve at queue time to an identity).** `Plugins.Run(name, arguments)`
resolves `name` when the script calls it, against **plugin registrations only** (names
that a plugin apartment registered with `AddCommand`), ASCII case-insensitively. The
result is a **registration identity**: the plugin apartment's instance number, the
registration's serial number and the registered spelling. The serial number is assigned
**per successful `AddCommand` call**, not per name: the plugin host draws it from one
counter for the client session that only grows, so a name removed and registered again,
even with the same spelling by the same apartment, gets a new serial, and no serial is
ever reused. Built-in names, `ext:` words and names that
no plugin registered resolve to nothing: the call returns `nil, "unknown command"`.

**Rule 7.3.2 (refused names).** A `name` that is empty, longer than 64 bytes, not valid
UTF-8, or that contains any Unicode White_Space character, `"` (U+0022), `'` (U+0027),
`/`, `;`, `:`, `,` or any character of general category Cc or Cf, is refused with
`nil, "invalid command name"` before any lookup. A plugin registration whose name contains
any of these characters can never be run through the bridge.

**Rule 7.3.3 (re-check at step 1, call directly).** In step 1 of the next frame the host
checks that the identity still holds (same apartment instance, same serial, still
registered, apartment loaded). If it does not, the action is dropped with `nil,
"command changed"` delivered in the script's next turn. If it does, the host calls that
registration's handler **directly**, as the plugin host calls any command handler
(`Execute(command, arguments)`, LPA §11.2), with `command` = the registered spelling and
`arguments` = the script's string unchanged. It counts as plugin time in step 1.

**Rule 7.3.4 (never the line route).** The bridge never builds a command line, never
tokenizes, never runs `;` substitutions, never looks a word up in the shared registry,
never reaches a built-in command, an `/ext:` word, the chat entry, the history or any
backend. `arguments` is at most **4,096 bytes** of UTF-8 (invalid sequences replaced as
D17's API rules say); a longer string is refused with `nil, "too large"`.

**Rule 7.3.5 (rate and channel).** D17's rate limits stay: one bridge action per script
per second. The one channel left is the one D17 §7 lists: a plugin that echoes the
arguments with `WriteLine` posts a local chat line; `WriteLine` lines are local and never
sent to a backend.

**Worked example.** Plugin P calls `AddCommand('"say"', cmd)`; the plugin layer registers
the name `"say"` with its quotes. Script S calls `Plugins.Run('"say"', "hello")`: refused
before lookup (`"` in the name). S calls `Plugins.Run("greet", '"say" hello')`, where
`greet` is P's command: in step 1 P's handler runs with `command = "greet"` and
`arguments = '"say" hello'`; no tokenizer runs, and nothing reaches the backend. If P
unregistered `greet` and registered it again between the call and step 1, the serial
differs and S gets `nil, "command changed"`.

**Tests.** X65 (slice 2, §6.6-style): a plugin that registers `"say"`, `say x`, `Say`
(refused by `AddCommand` as a built-in duplicate) and a name with a tab; a registration
swapped between queue and step 1; a backend stub that fails the test on any submitted
`Chat` command.

<a id="s7-4"></a>
### 7.4 Liveness floor for the heavy-frame gate (open item 4)

**Problem.** The gate skips every frame in which step 7 is reached more than 22 ms after
the frame began. On a slower machine, a debug build or during sustained streaming every
frame may be gated; ext then never runs, `/ext:` lines are never served, and a backend
switch, which needs its unload items, never finishes. Deterministic mode turns the gate
off, so CI cannot see it.

**Rule 7.4.1 (floor frame).** A frame that the gate would skip is run as a **floor frame**
when either holds: the previous **15** frames were all gated, or at least **250 ms** of
wall time have passed since the start of the last frame in which ext ran. In a floor
frame ext runs with its ordinary caps and slices (2 ms cap, 1 ms 6a slice, 1 ms 6c
slice), except that 6a starts **exactly one item** if one is queued. A floor frame **is**
a frame in which ext ran: it resets both conditions, so the count of consecutive gated
frames starts again at 0 after it and the 250 ms are counted from its start (read without
this reset, every frame after the first floor frame would qualify). Under wall time,
floor frames therefore occur at most once per 250 ms, at most four times per second (a
gated frame takes more than 22 ms, so 15 gated frames always span more than 250 ms and
the time condition is met first). In **deterministic mode** the wall clock is off: only the 15-frame
condition applies, so under permanent gating every 16th frame is a floor frame whatever
the simulated frame time.

**Rule 7.4.2 (switch timeout).** A switch is **queued** at the moment EM's backend gate
enters `Draining` (EM §10.3). When a backend switch or session end has been queued for
**2 s** of wall time (or **120 frames** in deterministic mode) and some apartments have
not yet started their `Unload` handler, those apartments are closed **without** their
`Unload` handler, exactly as for a suspended script (D17 §4): their pending intents are
already discarded, their `Store` writes are flushed at the close, and the script log
records "unloaded without Unload: backend switch timed out". An `Unload` call already
running when the timeout passes finishes within its own deadline. Close items still obey
the 6a slice and the floor. Loads in progress follow §8.21; leases have already ended at
the queueing (§8.17).

**Rule 7.4.3 (what the gate never stops).** The gate and the floor apply to 6a–6c only.
The panic chord, the native routing of ext UI in step 1, the dropping of intents at
suspension and the expiry of leases are native and run in every frame.

**Rule 7.4.4 (bounds).** Under permanent gating at any frame rate, with *F* the length of
one frame: a queued `/ext:` line or notification reaches its script within 250 ms + *F*
plus that script's position in the round robin; a backend switch with *a* apartments to
close completes within **2 s + *a* × (250 ms + *F*)** (at least one unload or close item per
floor frame). In slice 1a at most 16 apartments exist (L26; a 17th exists only during a hot
reload, which ships in slice 1b), so the bound is **2 s + 16 × (250 ms + *F*)**, about 6 s
when *F* is small against 250 ms; from slice 1b it is **2 s + 17 × (250 ms + *F*)**, about
6.25 s. In deterministic mode the same bounds in frames are **16 frames** for a queued line
or notification (plus the round robin) and **120 + *a* × 16** frames for a switch: **120 +
16 × 16 = 376 frames** in slice 1a and **120 + 17 × 16 = 392 frames** from slice 1b. Without
gating D17's bound (34 ungated frames, about 0.6 s) holds.

**Rule 7.4.5 (worst-frame arithmetic).** A floor frame's ext time obeys D17 §8's ceiling
(7 ms + R + 2·S, or 2 ms + K + S after a compilation and 2 ms + C + S after a close, each
within 13 ms; the lower interim ceilings of L12 and OPEN O-2 while it is open), but it
lands on a
frame that is already 22 ms old at step 7. D17 §8 "The frame budget" therefore gains the
sentence: "Floor frames (§7.4 of the threat model) can add up to the ext ceiling to a
heavy frame at most four times per second; they are outside the walking-run claim only if
E6 observes one there." E6 records the number of gated and floor frames in the walking run
and in M3-a and M3-d; **a floor frame in the M4.5 walking run fails E6** (the gate is
meant never to engage there persistently), and in the flights floor frames are recorded
with the flight residual.

**Rule 7.4.6 (testable in CI).** `scripttool test` and the client harness offer a test
hook "every frame is heavy" that turns the gate on in deterministic mode with a fixed
simulated frame time; the floor's frame-count condition (15 frames) makes the result
deterministic.

**Worked example.** In an M3-d-like flight every frame reaches step 7 after 25 ms and
lasts 30 ms. Ext last ran in frame 0. Frames 1–8 are gated; frame 9 starts 270 ms after
frame 0 started, so the 250 ms condition holds before the 15-frame one and frame 9 is a
floor frame: one 6a item runs, the 6b round robin runs up to the 2 ms cap and delivers a
queued `/ext:notes add milk` line, and 6c steps one apartment. The user starts a backend
switch with 16 apartments loaded at frame 10. Every ninth frame (about every 270 ms) is a
floor frame and runs one unload item; 2 s after the switch was queued, the apartments
whose `Unload` has not started are closed without it, one close per floor frame, and
`Connect` is submitted after the last close, within 2 s + 16 × 270 ms ≈ 6.3 s of the
switch. In deterministic mode with the "every frame is heavy" hook, the same run has a
floor frame every 16th frame and completes within 376 frames. With a hot reload in
progress (slice 1b) the switch has 17 apartments and the bounds become 2 s + 17 × 270 ms
≈ 6.6 s and 392 frames.

**Tests.** X85, slice-1a part: the "every frame is heavy" hook in deterministic mode with
a queued `/ext:` line (served within 16 frames) and a backend switch with 16 apartments
(completes within 376 frames, `Connect` after the last close, no `Unload` starts after
frame 120 of the switch, each late apartment's log has the timeout record). Slice-1b part:
the same switch with a hot reload in progress, so 17 apartments (completes within 392
frames), and a camera lease held when the switch is queued (ends in that frame, §8.17).

<a id="s7-5"></a>
### 7.5 Bounded entity-subscription filtering (open item 5)

**Problem.** Subscription filters run natively at the start of a 6b turn over every
recorded spawn and despawn of up to 60 retained frames, times up to 16 subscriptions. The
recorded list is uncapped (a landblock integration can spawn thousands of statics), the
work is neither a 6b call nor a 6a item, and "charged to that turn's units" has no
refuse-or-resume mechanism.

**Rule 7.5.1 (the record list is capped).** Steps 2–5 record at most **4,096** spawn and
despawn records per frame for ext consumers. Records past the cap are not kept; the
frame's entry carries an **overflow count**, and every script with at least one
subscription receives that count as `unfiltered` drops in its next delivery (it cannot
know whether they matched). Retained lists are kept at most **60 frames**, so the shared
store holds at most 245,760 records; at the declared record size (E5 measures it; at most
64 bytes) that is at most **15 MiB**, a host-wide fixed bound listed with the memory sum
(§8.10).

**Rule 7.5.2 (filtering is a metered call with a cursor).** Each script with
subscriptions has a **cursor** into the retained lists (frame, index). Its turn starts
with a **filter call**: a call like any other (4 ms deadline, 800,000 units, counted
against the 2 ms cap, ends a turn split as §8.13 says). The call filters records from the
cursor in batches of **64**, charging before each batch `64 × (number of the script's
subscriptions) × c_f` units, where `c_f` is the declared cost of one record-filter pair
(E7 calibrates it). It stops and keeps the cursor when the next batch's charge would pass
the call's remaining units or deadline, or when the script's events delivered in this
frame reach **1,024**. Matches become ordinary subscription notifications in the script's
queue (within the 1,024-entry queue cap). The rest of the turn follows in the same frame
if the cap allows. A record older than 60 frames when the cursor reaches it is skipped
and counted as a drop. A subscription matches only records of frames **after** the frame
in which it was created, so a subscription made while the cursor lags never matches
records from before it existed.

**Rule 7.5.2a (the filter call, the rolling time and aborts).** A filter call that ends by
its own stopping rule (the next batch would pass its units or deadline, or the 1,024
delivered events are reached) is **never an abort** and never counts under L18; one
batch is at most 64 × 16 × `c_f` units (about 51 µs at `c_f` = 10), so the call stops at a
batch boundary before its deadline. Filter-call time is **excluded** from L17's rolling
mean, throttle and suspension: its amount is set by the world's record rate (L31) times
the script's subscriptions (L30), which a script in a dense streaming area, or under an
entity flood from a server (AT6, slice 3), does not control. It still counts against the
2 ms cap (L5) and the frame's ext time (L12, E6), the profiler line shows it apart from the
script's own time, and the rest of the turn's calls count under L17 as before. A throttled
script's turns, its filter call included, run at half rate (L17); its cursor then lags,
and records older than 60 frames are dropped and counted, never filtered late.

**Rule 7.5.3 (enumeration).** D17 §8 "The worst ext time in one frame" now covers the
pass: a filter call is a 6b call, so it is inside the "6b call started just under 2 ms"
case. No native filter work runs outside a call, in steps 2–5, or in 6c.

**Rule 7.5.4 (suspension).** A suspended script records nothing and its cursor jumps to
the newest frame when it is resumed; the skipped records are counted once in its
`Resumed` notification (D17 §4).

**Worked example.** One frame integrates a landblock with 10,000 statics. The list keeps
4,096 records with overflow 5,904. Four scripts each hold 16 subscriptions, the 64 that
L30 allows for all scripts together; with `c_f` = 10 units, one batch costs 64 × 16 × 10 =
10,240 units, so one 800,000-unit call can filter about 78 batches (4,992 records), the
whole frame. With sixteen scripts of 4 subscriptions each (again 64 in all), a batch
costs 2,560 units and one call filters the whole frame in 64 batches (163,840 units).
With the 2 ms cap, the scripts not reached this frame keep their cursors and filter next
frame. Every script's next delivery carries `unfiltered = 5,904`.

**Tests.** X19 (E6 case: a 10,000-entity integration per frame for 10 s, run twice, with 4
scripts × 16 subscriptions and with 16 scripts × 4, both at L30's 64 in all; frame rule and
ceiling hold; drops counted; cursors never skip a record except past 60 frames; no script
is throttled, suspended or charged an abort by its filter calls alone; a subscription made
while its script's cursor lags matches no earlier record). Its ST part feeds the records
through the harness's **record-flood hook** (EM §11.5's `[[flood]]`, a test hook that adds
spawn and despawn records to steps 2–5's list in deterministic mode, within L86's flood
bounds: at most 16,384 spawns per frame and 65,536 flood entities alive in any frame;
X19 uses `{ from = 60, to = 659, spawn = 10000 }`, 10,000 spawns and 10,000 despawns per
frame with 10,000 alive), not through walk-file entities, whose count L86 caps at 4,096
per run.

<a id="s7-6"></a>
### 7.6 `Store` read-your-writes (open item 6)

**Problem.** Writes are written behind (at most one flush per second) and reads read the
file, so `Store.Set(k, v)` followed within about a second by `Store.Get(k)` returns the
old value. In a hot reload the new apartment opens while the old one still runs, and
values the old `Unload` saved are still pending, so the new apartment reads stale data.

**Rule 7.6.1 (the file index).** When a store is opened (D17 §8, "`Store` I/O": on the
store's I/O queue, inside the load's snapshot job, before the top-level run), the host
first removes temporary files left by a crash (§8.7), then lists the store's key files and
builds the **file index**: key → the file's exact byte size and its counted size (§8.7
"Counted bytes"); the exact size bounds every later read of that file (§8.14). It is
kept in host memory and charged to the memory cap of **every apartment that has the store
open**: in a hot reload, where both apartments share the store, it counts in full toward
both caps (conservatively, twice in the sum) until the old apartment's cap leaves the sum
(rule 7.6.5), and from then on toward the new apartment's cap only. It is kept current only
by work on the store's
I/O queue: a flush that renames a value into place sets the key's entry to the new file's
byte size and counted size; a flushed deletion removes the entry; a wipe and an orphan removal empty
it. The file index describes the disk; what the script has written and not yet flushed is
in the pending-write map.

**Rule 7.6.2 (one pending-write map per store).** The host keeps one **pending-write map**
per (kind, id) store, keyed by `Store` key, holding the serialised bytes of the latest
value written and not yet renamed into place (a deletion is a pending "absent"). Both
apartments of a hot reload share it, because they share the (kind, id). A flush **takes**
an entry's value when its job starts writing that value's temporary file (or starts the
removal, for a deletion); from then until a newer `Set` or `Delete`, the entry is
**taken**, otherwise it is **untaken**. An entry is removed only when the flush job that
took **that** value has renamed it into place (or removed the file, for a deletion); a
newer `Set` replaces the entry's bytes and makes it untaken again, and the flush job in
flight for the older bytes does not remove the newer entry. The write buckets of §8.7
read this state: a call on a taken entry pays as for a clean key. Calls that answer from
the index (`Has`, `Keys`, `Usage`) see the pending map over the file index.

**Rule 7.6.3 (reads see pending values).** A `Store.Get(k)` whose key has an entry in the
pending-write map is answered from **that entry's bytes as they are when the `Get` is called**
(program order): a `Set` or `Delete` after the `Get` does not change the answer. The host
keeps each pending value's bytes immutable and the `Get` shares them (a newer `Set` makes
new bytes, never changes the shared ones), so the call copies nothing; an implementation
that copies instead charges the copied bytes, b(stored size), at the call before it
copies (D17 §8, charge before work). A pending
deletion answers "absent", exactly as a key with no file. Such a read needs no worker job
and no file read, but in every way the script can observe it is an ordinary read: it is
delivered as a notification in the script's **next** turn (never in the calling turn) and
converted inside that metered call with its cost charged first (D17 §4); it counts
against the read rate (L42) and as one **outstanding request** (L27) from the call until it
is delivered or dropped; and it reserves its result (§8.14). A `Get` whose key has no
pending entry is a job on the store's I/O queue, behind every earlier flush of that key.
This rule replaces both EA1a §5.2's "one I/O-queue job either way" and EA1b §8.6's "not
counted as outstanding" (§13).

**Rule 7.6.4 (ordering with wipes).** A wipe (D17 §5.2 rule 3) and an orphan removal clear
the pending map and the file index for that store on the store's I/O queue, after the final
flush. Because a load's open waits for that final flush (D17 §8), a new apartment that
triggers a wipe never sees an entry from before the wipe.

**Rule 7.6.5 (accounting).** Pending entries, and the bytes a flush job in flight holds,
count toward the `Store` size totals at their counted size (so writes in flight cannot
pass the 16 MiB or 256 MiB totals, L41) and toward the memory cap of the apartment that
wrote them, until those bytes are renamed into place or discarded. A newer `Set` moves the
entry's charge to the apartment that made it. An apartment's cap leaves the 512 MiB sum
(or the 64 MiB reload allowance) only after its close item has run **and** every flush
holding bytes it wrote has completed; at a close, the apartment's pending entries go to
its final flush (D17 §8). In a hot reload, therefore, the old apartment's pending entries
stay charged to the old cap, which stays in the sum until the old apartment's final flush
completes; only then is the reload allowance released (§8.10). The new apartment can read
those entries (rule 7.6.3) without being charged for them.

A pending **deletion** adds **no key and no byte** of its own to the key and byte totals
(L40, L41), whether its key is present or absent: the key's file keeps counting through the
file index until the flush that removes it completes, and the value a deletion replaces
keeps counting while a flush job in flight holds its bytes. So a `Store.Delete`, which has
no `full` answer, never raises a total and can never pass a space cap; only a `Set` can,
and a `Set` that would pass one answers `nil, "full"` at the call. (The 4 KiB a deletion
takes from the write buckets, §8.7, is wear, not space.)

**Worked example.** Developer script `notes` (scripts-dev) saves `draft` in its `Unload`
during a hot reload. The bytes enter the pending-write map. The next 6a item runs the new
apartment's top level, which calls `Store.Get("draft")`; the answer comes from the map in
its first turn, the new text, although the flush job writes the file 400 ms later.

**Tests.** X88: `Set` then `Get` in the same call and the next frame return the new value;
`Get` then `Set` in the same call returns the old value (program order); `Delete` then `Get`
returns absent; a `Set` during an in-flight flush of an older value, then `Get`, returns the
newest; 32 `Get`s of pending keys make the 33rd `Get` answer `nil, "busy"`; a hot reload
whose old `Unload` writes a key and whose new top level reads it gets the new value; after
a wipe, `Get` returns absent.

<a id="s7-7"></a>
### 7.7 The `Input.Drive` allow-list (open item 7)

**Problem.** "Press standard game-band actions" includes `global` (`menu`, `open_chat`,
`screenshot`, `toggle_fly`, `toggle_debug`) and `pointer`. A driven `screenshot` writes
image files, contrary to "the only file output is the capped `Store` and log". Driven
presses carry no command name and go straight to game code, which on `menu` or
`open_chat` shows, activates or focuses native panels in step 3, running plugin
`Deactivated` and `FocusLost` outside IB D8's steps 1 and 7; `open_chat` pushes
`text_entry` without a user gesture.

**Rule 7.7.1 (closed allow-list).** `Input.Drive` may press, hold or set only these
**drivable actions** (KM §8 names):

| Context | Drivable actions |
|---|---|
| `movement` | `move_forward`, `strafe`, `turn` (axis values in −1..1), `jump`, `walk`, `autorun` |
| `fly` | `fly_vertical` (axis), `fly_boost`, `fly_slow`; only while fly mode is active by the user's own choice |

Every other action is refused with a Lua error naming it ("not drivable"): every action
of `global`, `camera` (including `mouselook`, which also starts the pointer lock),
`pointer`, every UI-band context, `fly_speed`, every plugin action and every ext action.
The list is closed: a new built-in action becomes drivable only by an API-spec
change (a new minor version of `Input`) reviewed against this section.

**Rule 7.7.2 (no mode or UI effect).** `Input.Drive` never activates or deactivates a
context, never switches walk or fly mode, never shows, hides, activates or focuses any
panel or window, and never produces an event with a command name. A driven press is
action state latched per controller step (D17 §3).

**Rule 7.7.3 (consistency).** The allow-list is stated in the slice-2 API spec (`Input.Drive`),
and LEGAL §18.6's open item on driven screenshots is closed by it (§13).

**Tests.** X43 (slice 2, Offline): each excluded action (and specifically `screenshot`,
`menu`, `open_chat`, `toggle_fly`, `toggle_debug`, `mouselook`, `select_left`) raises
"not drivable"; after 10 s of driving every allowed action no new file exists anywhere
under the user's data, log and screenshot directories except the script's `Store` and log;
an instrumented plugin records no handler call outside steps 1 and 7; no `text_entry` or
other UI-band entry was pushed.

<a id="s7-8"></a>
### 7.8 World labels never catch the pointer (open item 8)

**Problem.** World labels are UI elements placed at a projected point in step 8. If one
could catch the pointer, step 8 would move a box that the next hit test and hover refresh
read, breaking IB D8's "step 8 changes no box that a hit test reads", which D17 says it
keeps.

**Rule 7.8.1.** Ext world labels never catch the pointer: they are never pointer targets,
never block clicks, never hover targets, never receive pointer notifications and never
count as covering for the focus rule (they are in the ext range, below everything that
can cover a text box anyway). No API spec offers a property that changes this. Hit
tests and hover refresh skip them entirely, so moving them in step 8 changes nothing a
hit test reads.

**Tests.** X45: a world label placed under the pointer on an entity: a world click passes
to the world, a plugin control's hover under it is unchanged, the script receives no
pointer notification for the label, and the hit-test result of each frame is identical
with and without the label.

---

<a id="s8"></a>
## 8. Requirements for D17's other improvements

<a id="s8-1"></a>
### 8.1 Script log sanitising

This is the one log format; EA1a §4.3 and EA1b §9.2 cite it (§13).

- **Record.** Every record the host writes to a script log is **one line**, ended by LF:
  `<time> <level> <source>[ <chunk>:<line>]: <text>`, where `<time>` is UTC in the form
  `2026-10-07T14:03:05.123Z`; `<level>` is `debug`, `info`, `warn` or `error`; `<source>`
  is `print`, `log` (a `Core.Log` call), `error` (an error or abort the host reports) or
  `host` (a record the host writes itself, such as "N records dropped"); and
  `<chunk>:<line>` is the relative chunk name and line of the calling code when known
  (never an absolute path, T66). Everything after the prefix that a script can influence,
  the chunk name included, is escaped as below.
- **Escaping.** Invalid UTF-8 is written as U+FFFD. Every C0 control character except tab
  (U+0000–U+0008, U+000A–U+001F), U+007F and every C1 control (U+0080–U+009F) is written as
  `\xNN` (two upper-case hexadecimal digits of the code point, so U+0085 is `\x85`);
  U+2028 and U+2029 are written as `\u{2028}` and `\u{2029}`; a literal backslash is
  written as `\\`. So CR, LF and the Unicode line separators never start a new line, and
  ESC, OSC and CSI sequences never reach a terminal or an editor.
- **Cap.** A record is at most **4,096 bytes** after escaping, prefix included; longer text
  is cut at a UTF-8 boundary that does not split an escape and ends with the marker
  `…[cut]`.
- **Rate.** Two token buckets of escaped record bytes, each holding one second of its
  rate and starting full: D17 §8's **64 KiB/s per script**, and a new **128 KiB/s for all
  scripts together** (L45). A record is written only when both buckets hold its bytes,
  which it then takes from both; otherwise it is dropped and counted in that script's
  drop count. When both buckets next allow it, the host writes that script's counted
  "N records dropped" record (source `host`, itself paid from both buckets) and resets the
  count. So the bytes all script logs write in any interval of *t* seconds are at most
  128 KiB × (*t* + 1), whatever the number of scripts. This changes D17 §8's script-log row,
  which calls the per-script rate "the only rate limit on the log" (§13, OPEN O-3).
- **The developer console** (EA1b §9.2) shows log records as text of the reserved layer:
  each displayed line passes §8.2's steps 1–5 with the "error or log line" cap, so the
  console and the reserved layer show script text the same way (D17 §5.3). The log
  **file** keeps the full escaped record; the console's cut does not apply to it.
- **Test X51:** `print("\27]52;c;cm0gLXJmIH4=\7")`, `print("ok\ngrant approved")`,
  `print("a\rError in other · installed")` and `print("x\226\128\168y")` (U+2028): the log
  file contains no byte below 0x20 except tab and LF, each `print` is one record with the
  host prefix, the text shows the escapes, and a 10 KiB `print` is one record of at most
  4,096 bytes ending in `…[cut]`.

<a id="s8-2"></a>
### 8.2 Plain text in the reserved layer

Every string in the reserved layer that is not Tessera's own (D17 §5.3), and every line of
the developer console (§8.1), is processed in this order when it is drawn:

1. Invalid UTF-8 becomes U+FFFD.
2. Remove every character of general category Cc, Cf, Zl, Zp, Cs, Co and Cn, and every
   noncharacter. This covers bidi controls, U+200B–U+200D, U+2060, U+FEFF, U+2028,
   U+2029 and tag characters. Single exception: U+000A in a manifest `description` is
   kept as a line break (at most 8 lines, EM §4.2).
3. Keep at most **2** combining marks (general category M: Mn, Mc and Me, variation
   selectors included) after each base character; drop the rest. A mark with no base
   (at the start, or after a removed character or a line break) is dropped.
4. Cut at the cap in **code points and in bytes**, whichever comes first, with an
   ellipsis: `name` and `author` 48 code points and 192 bytes; `reason` 160 and 640;
   a setting `description` 160 and 640 (L80); the manifest `description` 512 and 2,048;
   an id or command word 32 and 128; a command's **help line** (`Commands.Register`'s
   `help`, drawn by `/ext:help`) 96 and 96, its L91 cap, so a `help` string that EA1a
   accepted is never cut and only steps 1–3 and 5 change it; an error or log line 256 and
   1,024; a label or choice label 48 and 192. Any other string that is not Tessera's takes
   the cap of an error or log line, so every string has a category.
5. Draw each string clipped to its own box, so no glyph draws outside it.

**Manifest strings are refused earlier (the stricter rule, resolving EM OPEN O-9).** A
manifest display string (EM §4.4) that steps 1–3 would change is refused when the
manifest is parsed: invalid UTF-8, any character step 2 removes (the description's line
feeds excepted), more than 2 marks of category M after a base or a mark with no base, or
a string over its cap in code points or bytes. EM may refuse more (for example the
variation selectors) but never less. A refused manifest's strings are never displayed:
the report names the script by its id and folder kind only. So for a manifest that loads,
the display rule changes nothing, and the display rule remains the only guard for strings
that do not come from a manifest: error messages, log lines, `/ext:` words and ids the
user typed, command words.

**Test X47** has three parts. (a) **ST/U:** a manifest whose `name` is "Tessera" with
U+202E, U+200B, tag characters and 40 combining marks on one letter, one with markup
(`<b>Tessera</b>`, which is allowed and shown as plain text) and one with 3 marks on one
letter: the first and third are refused at parse time and the report shows the id and
kind but not the name. (b) **U (`ui`):** the display filter applied directly to the same
hostile string gives the expected plain text: none of the removed characters, at most two
marks per base, cut at the cap. (c) **CI (golden image):** an error message raised with
the same content and an unknown `/ext:` id typed with the same content: the drawn text has
none of the removed characters, at most two marks per base, stays inside its box, and sits
next to the id and folder kind; the markup name of (a) is drawn as literal text.

<a id="s8-3"></a>
### 8.3 Host-drawn ownership badge

- Every shown ext window that has at least one element that catches the pointer, or any
  text box, carries a **badge** drawn by the host: the text "⟨id⟩ · ext script" in
  Tessera's fixed badge style, at full opacity whatever the window's opacity.
- **Fixed box.** The badge is drawn in a box of fixed size, **20 UI units high and
  16 + 8 × (*n* + 13) UI units wide**, where *n* is the id's length in bytes (ids are
  ASCII, at most 32, L38), so at most 376 UI units wide; its text is clipped to that box.
  The box, never the drawn glyphs, is what composition coverage counts (rule 7.1.2,
  counted by size in physical pixels: UI units × the UI scale), so the count does not
  depend on fonts and `UI.Usage().area` is deterministic.
- **Placement: wholly on screen.** The host attaches the badge outside the window's top
  edge, left-aligned with the window, when that position lies wholly inside the screen;
  otherwise (a window at y = 5, a window partly off screen at the top, left or right) the
  host places it **inside** the window along its top edge, shifted horizontally so that
  the whole box lies on screen. Where the window itself is too small or too far off screen
  for the box to lie wholly on screen inside it, the badge counts as **off screen**.
- The badge is shown whenever the window is shown, not only while a text box has focus
  (this replaces EA1a §7.7's focused-only badge, §13).
- The script cannot style, move, cover, hide or measure the badge; it is not an element
  of the script's tree and it is never a pointer target. **All badges are drawn above
  every ext window of every script**, as the top layer of the ext range (badges among
  themselves in the z-order of their windows), so no ext window, of the same script or of
  a cooperating one, with or without a catching element, can be placed over a badge.
  Plugin windows, native panels and the reserved layer are above the ext range (§8.26) and
  may cover a badge; none of them is controlled by an ext script. Its area counts toward
  the window's composition coverage (§7.1).
- **No focus behind a hidden badge.** A script can read native panel positions (§6.8
  residual 1) and place its window so that the badge falls under an always-visible
  toolbar or chat window while its text box stays visible. So keyboard focus enters a
  window's text box (by the user's own solid press, or by a script request in a gesture
  window) only while that window's badge is **wholly on screen and uncovered** by any
  plugin window, native panel or reserved-layer surface, and the host **takes the focus
  back** at the same re-checks as for a covered text box (D17 §4, L58) as soon as the
  badge becomes covered or off screen. A press on the box while its badge is hidden is
  still a press, but focus stays where it was.
- **Badges cover text boxes** (adopted from EA1a §7.7). Although badges are drawn by the
  host, a badge whose fixed box overlaps a text box's **focus area** (its box grown by the
  width of the focus mark) with a positive area **covers** that box for rule 7.2.2 and L58:
  the badge of any other window, of the same script or another (for example one attached
  above a lower window of another script), and the badge of the box's own window when the
  host has placed it **inside** that window (a window at y = 5, along its top edge). The
  one exception is the badge of the box's own window attached **outside** the window's
  top edge: it lies wholly outside the window, so only the focus mark of a box at the
  window's top edge can reach it, and that badge is drawn in the focus colour while the
  box has the focus anyway. A frame move, a layout change or a show that brings a
  covering badge into the area takes the focus at the next re-check (D17 §4, L58).
- **The caret and focus mark are drawn above every badge.** The caret and the focus mark
  of the focused ext text box are host-drawn in the badge layer, the top layer of the ext
  range, **above every badge**. With the cover rule above, no content of any script and no
  badge can draw over the caret or the mark of a box that holds the focus.
- While an ext text box has keyboard focus, its window's badge is drawn with Tessera's
  focus colour as well.
- **Test X49:** a window styled like a native panel with a text box: the badge shows the
  id; script calls that try to restyle or overlap it change nothing in the badge's
  pixels (golden image); a second, higher ext window of the same script and one of
  another script, both without catching elements or text boxes, placed over the badge:
  the badge's pixels are unchanged; a fully transparent window with a pointer-catching
  element shows the badge too; a window at y = 5 and one partly off screen at the left:
  the badge is drawn inside the window, wholly on screen; a window placed so that its
  badge lies under a native panel (the chat window) and, in a second run, under a plugin
  window, while its text box is uncovered: a user's solid press on the box and a script
  `Focus` in a gesture window both leave the focus where it was; a plugin window dragged
  over the badge of a focused box takes the focus back before the next key; the counted
  badge area is the same under two different fonts. **Badge over a box:** a text box whose
  focus area is overlapped by the badge of another window of the same script, by the badge
  of another script's window, and by its own window's inside badge (a window at y = 5,
  the box at its top edge): a solid press and a gesture-window `Focus` both leave the focus
  where it was, and moving such a badge over a focused box takes the focus back before the
  next key; a box at its window's top edge with the window's badge attached outside takes
  the focus, and its focus mark and caret are drawn over that badge (golden image).

<a id="s8-4"></a>
### 8.4 Selection and copy (pastejacking)

- Any script change to an ext text box's text clears its selection.
- A script can set the caret but **cannot create a selection**; only the user's own
  pointer and keys select text.
- A copy (`Ctrl+C`, `Ctrl+X`, the platform's equivalents) copies only text the user
  selected and only if the box's text has not changed since the selection was made; if
  it changed, the copy does nothing.
- **A copy takes only text that was on screen** (D17's wording). A script can fill a box
  before the user selects, so a copy is made only when, from the selection to the copy,
  the **whole selected range was displayed**: every selected character drawn inside the
  box's visible area (none clipped by the box, its window or the screen, none scrolled
  out), the box visible and uncovered as for focus (L58), and the text drawn with colour
  alpha × effective opacity **≥ 0.5**. Otherwise the copy does nothing. So `Ctrl+A`,
  `Ctrl+C` in a `Field` holding "ls" followed by 300 spaces and a command scrolled out of
  view copies nothing.
- **Invisible characters are stripped.** The copied text has every character of general
  category Cc, Cf, Zl and Zp removed (bidi controls, U+200B–U+200D, U+2060, U+FEFF, tag
  characters), except LF and tab in a box that displays them as line breaks and tabs;
  in a single-line box LF and tab are removed too. Text whose colour merely matches the
  background is not detected; that is a stated residual of the copy rule (the user still
  sees the box's extent).
- **Test X52:** the user selects "abc"; in the same frame and the next, the script
  replaces the text with a shell command; `Ctrl+C` puts nothing on the clipboard (the
  test's clipboard stub stays unchanged). A script call that tries to select text has no
  effect. Then, with text set before the user selects: "ls" + 300 spaces + "; curl x|sh"
  in a `Field` and `Ctrl+A`, `Ctrl+C` (nothing copied); text at colour alpha 0 and at
  0.49 (nothing copied); a selection partly under a plugin window (nothing copied); text
  with U+200B, U+202E and U+0007 inside it, fully displayed (copied without them).

<a id="s8-5"></a>
### 8.5 Image decodes count as outstanding requests

Decodes of snapshot images and of `Files.Pick` bytes for UI images count toward the
outstanding-request caps (32 per script, 256 in all), are de-duplicated like other
decode requests (§8.9 for visibility) and charge their decoded size to the script's UI
memory share before decoding (D17 §8). A creation (or a change of an element's image)
that would pass a cap answers `nil, "busy"` at the call and creates nothing; no decode is
queued for later (this replaces EA1a §7.10's delayed `Loaded{ok = false, reason = "busy"}`,
§13). A decode counts from the call until its worker job has **ended** (finished, or
cancelled before it started); destroying the element, or changing its image, before then
does not free the slot early (L27), so a loop of creations and destructions can never have
more than 32 decode jobs of one script queued or running. **Joined requests count once
each.** When several requests of one script for the same file are joined to one decode job
(EA1a §7.10), each request (each creation of an image element, or change of its image)
counts toward L27 by itself, from its call until its own result is delivered or discarded
**and** the shared job has ended; joining saves decode work, never slots. So every
`Loaded` delivery, joined or not, belongs to a counted request (invariant 36). **Test
X26:** a loop that creates and destroys image elements every frame for 10 s keeps
outstanding requests at or below 32 and the worker queue does not grow; it runs twice,
once with a distinct file per element and once with every element naming the same file
(joined requests), and in the second run the 33rd creation while 32 are outstanding answers
`nil, "busy"` although only one decode job exists.

<a id="s8-6"></a>
### 8.6 `Files.Pick` reads only regular files

The chosen path is opened non-blocking (POSIX `O_NONBLOCK`; on Windows a share-read open
that never waits), its type is checked **on the handle** (`fstat` regular file; on
Windows a disk file that is not a directory or reparse point), and at most 3 MiB + 1 bytes
are read to detect an oversized file. Anything else (FIFO, socket, device such as
`/dev/zero`, directory) answers `nil, "not a regular file"`. **Test X58** (slice 2): a
FIFO, `/dev/zero`, a directory and a 3 MiB + 1 file each fail at once; no worker stays
busy.

<a id="s8-7"></a>
### 8.7 Disk caps and disk wear

- **Counted bytes.** Each `Store` file write counts its size rounded up to 4 KiB plus
  4 KiB, the same counting as D17's size totals; the temporary file of a write-behind
  flush is that write (the rename writes no data).
- **Sustained write rate (lowered).** Per script a token bucket of **10 MiB** capacity
  refilled at **32 KiB/s**; for all scripts **40 MiB** refilled at **128 KiB/s**. The
  capacities are D17's (10 s of its rates), so both the burst and the rate are at most
  D17's figures. These byte rates change D17 §8's `Store` writes row (§13, OPEN O-3).
  The key rate of D17 stays, counted as **64 newly dirty keys per second per script**
  (10 s bucket, L43): a key token is taken only when a call makes a clean key dirty
  (below), never for a replacement of an untaken value.
- **Worst sustained wear, all scripts, no grant needed.** `Store` writes at 128 KiB/s
  (≈ 10.5 GiB per day) plus script logs at their all-scripts rate of 128 KiB/s (L45,
  §8.1; ≈ 10.5 GiB per day), so about **21 GiB per day** of hostile scripts running all
  day, plus the one-off bursts of the `Store` buckets (40 MiB) and the log buckets
  (128 KiB). D17's figures allowed 86–345 GB per day for `Store` and, with 16 scripts at
  64 KiB/s, about 84 GiB per day for the log.
- **When the buckets are debited.** At the `Store.Set` or `Store.Delete` call, so that a
  refusal can be answered there. A key is **dirty** while it has an **untaken** entry in
  the pending-write map (rule 7.6.2), and **clean** otherwise: with no entry, or with an
  entry whose value a flush has already **taken**, even while that flush is still in
  flight. A call that makes a clean key dirty takes one key token and the **full** counted
  size of its new value (a deletion counts 4 KiB) from both byte buckets. A call on a key
  that is already dirty (it replaces an untaken value) takes only the growth of the
  counted size over that untaken value, if any; a replacement that does not grow takes
  nothing. If any bucket lacks the tokens, the call answers `nil, "busy"`, debits nothing
  and changes nothing. So every value a flush takes was paid in full: the bytes paid for a
  dirty key are never less than its untaken value's counted size, and a `Set` or `Delete`
  that lands while the flush of the key's previous value is in flight pays again in full.
  A key rewritten every frame therefore pays one key token and its full counted size once
  per flush that writes it, as write-behind writes it once per second, and the bytes all
  flushes write in any interval never exceed the buckets' capacity plus their refill. A
  flush is never refused or delayed by the buckets, because its bytes were paid for at
  the call.
- **Bucket lifetime.** The buckets belong to the (kind, id) for the client session; an
  unload, a reload or a backend switch neither refills nor resets them, so reloading a
  script cannot buy a new burst. They start full when the session starts.
- **Temporary files count.** Temporary files of flushes in progress count toward the
  16 MiB and 256 MiB totals. When a store is opened, temporary files left by a crash are
  removed on its I/O queue before the open completes.
- **Log.** 1 MiB per file, rotated once (at most 2 MiB on disk per script); 64 KiB/s per
  script and 128 KiB/s for all scripts (§8.1, L45).
- **Totals, orphans included (L40, L41, L92).** The all-scripts totals count every store
  of a (kind, id), loaded or not. A loaded or loading store contributes its file index and
  pending entries (rule 7.6.1, 7.6.5). Every other store, an orphan or a script not
  loaded, contributes the result of the **store-root scan**: the host lists
  `data/script-store/<kind>/` through its directory handle (§8.8 item 1) and, for each
  entry named `s_<id>` with a valid id that is a plain directory (checked with
  `fstatat` without following, never opened if it is a link, reparse point or other
  kind), lists that directory through a no-follow handle and adds, for each entry, its
  counted size from `fstatat` without following (a regular file by its size, any other
  entry as 4 KiB), and one key per regular file whose name is a key file; nothing in it
  is opened for reading. Entries of `<kind>` that are not such directories count nothing
  and are reported by the script manager. The scan reads at most **1,024** entries of
  each `<kind>` listing and at most **2,064** entries of each store (1,024 key files, as
  many temporary files left by a crash, and the host files); past either
  cap the listing stops, the store or the remaining stores count as **full** (16 MiB and
  1,024 keys each, or the whole all-scripts totals for a `<kind>` listing that passes
  its cap), so the totals stay conservative, and the script manager reports it. The scan
  runs on the grant-store queue (never an asset worker, never the main thread) **every
  time** EM's gate enters `Starting` (each session start, so after every backend switch
  too, not once per client run), before any load of that session, and again after every
  Reset, Remove and orphan removal; a store leaves the scanned set when a load opens it (its file index
  then counts) and joins it again, with its file index's last figures, when its
  apartment's final flush completes.
- **Test X56:** new 1-byte keys every frame stop at 1,024 keys and 16 MiB counted;
  leftover temp files are removed at the next open; with 300 orphan stores of 1 MiB each
  present at start-up, a new script's writes answer `nil, "full"` at the 256 MiB total;
  a `<kind>` directory of 10^5 entries is listed only up to 1,024 and the totals count as
  full. **Log flood:** one script printing a 4 KiB line every frame for 60 s keeps its
  log files within 2 MiB on disk and its rate within 64 KiB/s; then 16 scripts doing so
  together write at most 128 KiB × 61 bytes of log in 60 s in all, and each script's log
  carries its counted "N records dropped" records. **Test X57:** a script rewriting
  1,024 keys every frame for 60 s writes at most 10 MiB + 60 × 32 KiB of counted bytes,
  its refused calls answer `nil, "busy"` at the call, and a reload in the middle of the
  run does not raise that bound. The same bound holds in a second run where the harness
  holds every flush in flight for several frames (a test hook on the store's I/O queue)
  so that a `Set` of each key lands while the flush of its previous value is in flight:
  every such `Set` debits the full counted size, and the counted bytes written over 60 s
  stay within the bound.

<a id="s8-8"></a>
### 8.8 Script folders, store directories and log files are opened without following links

**The snapshot root.** The host opens `data/scripts` (or `data/scripts-dev`) as a directory
handle, then the script folder `<id>` **relative to that handle** with
`O_NOFOLLOW | O_DIRECTORY` (POSIX), or relative to the parent handle with
`FILE_FLAG_OPEN_REPARSE_POINT`, refusing any reparse point (Windows). A symbolic link or
junction at `<id>` refuses the script. The data directory itself may be a link (it is the
user's configuration). Every entry of the walk (EM §3.4, `scripttool check` included) is
**classified before it is opened**, with `fstatat(…, AT_SYMLINK_NOFOLLOW)` relative to its
directory handle (Windows: the directory listing's attributes); a link, FIFO, socket or
device node refuses the script without being opened, so no open blocks and no device is
opened with side effects. A regular file is then opened with `O_NOFOLLOW | O_NONBLOCK`
and checked again **on the handle** (regular, one link, the same file as classified); a
swap between the two checks refuses the script. **Test X54** includes a script folder that
is a symlink to a directory outside, a FIFO and a device node in the folder: each refused,
and no asset worker blocks.

**Store directories and log files (closes T93).** The same holds for every path below
Tessera's own directories whose name a script influences: the store directory
`data/script-store/<kind>/s_<id>/` and every entry in it (key files, the write-behind
temporary files, the owner tag `owner`, the marker `local-only`), and the script logs
`logs/scripts/<kind>/s_<id>.log` and its rotated file `s_<id>.1.log`. EM §3.6 names the
files (key files `k_<key>.store`, the write-behind temporary files `t_<key>.tmp`, a
pattern that no key file and no host file matches, and the host files) and states the
per-entry actions; they must satisfy all of these:

1. **Handles, never paths.** `data/script-store/`, `logs/scripts/` and each `<kind>`
   directory are opened as directory handles (the first two may be links, as the data
   directory may; `<kind>` may not), and `s_<id>` is opened relative to its parent handle
   with `O_NOFOLLOW | O_DIRECTORY` (POSIX) or as a non-reparse-point directory (Windows).
   Every entry of the store is opened, created, renamed or removed relative to the
   `s_<id>` handle, every log file relative to its `<kind>` handle, never by a path.
2. **No link is followed.** Every open of an entry uses `O_NOFOLLOW` (POSIX) or
   `FILE_FLAG_OPEN_REPARSE_POINT` with a reparse-point refusal (Windows), and its kind is
   checked **on the opened handle**: a key file, temporary file, owner tag, marker or log
   file is used only if it is a regular file with a link count of 1. New files are
   created exclusively (`O_CREAT | O_EXCL`, or `CREATE_NEW`), so a create never writes
   through an existing entry, and a rename only replaces a directory entry, never writes
   through it. An existing entry is **classified before it is opened** (`fstatat` without
   following), and only a regular file is opened; **every** open, for reading or for
   writing (the log's append open included), is non-blocking (`O_NONBLOCK`; a write open of
   a FIFO then fails with `ENXIO` instead of waiting for a reader), so a FIFO never blocks
   a worker or the log writer and a device node is never opened. A rename that would
   replace an entry that is not a plain regular file (a rotation onto a linked or special
   `s_<id>.1.log`, a flush onto a special key file) is not made; the entry is first
   removed as in item 4, or the operation fails closed as in item 3.
3. **A store directory that is not a plain directory is never used.** If `s_<id>` (or its
   `<kind>` directory) is a symbolic link, a junction or other reparse point, or not a
   directory, the store open fails closed: nothing in or behind it is read, written or
   removed, the load is refused, and the script manager names the path. The same holds
   for a log directory; a log file that is not a plain regular file with one link is not
   written, and the log is unavailable for that script until it is fixed (the script
   still loads; the script manager reports it).
4. **Unexpected entries inside the store** (a link, a reparse point, a subdirectory, a
   special file, a regular file with more than one link, a name that is no key file,
   temporary file or host file) are **never opened** and never listed into the file index.
   An entry that is not a directory is removed at the store open, before the file index
   is built, by a no-follow removal of the **entry itself** (`unlinkat` without
   following; on Windows deleting the reparse point, never its target), so a link's
   target and a hard link's other names are never touched. A subdirectory, or any entry
   that cannot be removed this way, makes the open fail closed as in item 3; the host
   never recurses into it.
5. **A wipe removes only entries of the store directory**, each by the no-follow removal
   of item 4, after the checks of item 3; it never follows a link, never enters a
   subdirectory, and never deletes anything outside `s_<id>`. A wipe that finds an entry
   it may not remove fails closed: the open fails, the load is refused, and nothing is
   opened that the wipe should have removed.

**Tessera's own files.** The grant store `ext-grants.toml` and `keymap-ext.toml` may be
links (they follow the configured paths, §8.19, EM §8.5), but every read of them is a
non-blocking open whose kind is checked **on the handle** (a regular file; a FIFO,
socket, device or directory is treated as unreadable, never waited on), and reads at most
the file's cap + 1 bytes (L79, L47). A copy of a damaged document (EM §8.6's `.corrupt`
backup) is made of those bytes only, or by a rename, never by an unbounded copy of the
target. An advisory lock on such a file is taken without blocking, retried for at most
**1 s** (L94), and then treated as a failed write (the decision stays for the session, EM
§8.5).
The grant-store queue therefore never waits on another program, and EM's gate cannot stay
in `Starting` because of it.

**The harness's tool root.** `scripttool`'s tool root (EM §11.1) holds X94's links and
junctions on purpose. Every removal EM §11.1 makes at exit (of the tool root it made,
and of a user-given `--root` if EM decides to remove one) is a no-follow recursive
removal by directory handles, as item 4 removes entries: a link or
reparse point is removed as an entry, never followed, and nothing outside the root is
removed.

These rules make invariant 15 concrete for the store and the log. **Test X94.**

<a id="s8-9"></a>
### 8.9 Cross-script channels: decode cache and layout

- **Decode cache: per-script visibility (closes T67).** Ext decode results are held in the
  ext partition (16 MiB per script, 64 MiB in all), but a script sees a decoded DID,
  setup or sound only when **its own** request for it has completed; until then `Data`,
  `Audio` and `Entity.Local` answer `nil, "not loaded"` to it even if another script's
  result is resident. In-flight requests may still be joined (one worker job), and the
  joined result may be shared in memory, but each script's view is recorded per script
  and charged to that script's share. So a hit or miss never tells script B what script A
  asked for. A request whose entry is already resident is still an outstanding request
  (L27) whose completion is delivered no earlier than the requesting script's next turn,
  like any worker result. What remains is timing: a request that joins an in-flight job,
  or finds its entry resident, can complete sooner than a fresh decode would, which tells
  B at frame granularity that someone asked for the entry recently. It is listed as
  residual (4) in §6.8 and goes to LEGAL §18.4 (§13).
- **Layout between scripts (residual, listed).** A lower ext window with a pointer-catching
  element learns where higher ext windows catch the pointer, as D17 §5.6 says for plugin
  windows. It is a channel of layout only and is listed in §6.8; LEGAL §18.4 and §18.5
  name it too (§13).
- **Test X68:** script A requests DID d and waits until loaded; script B then asks for d:
  "not loaded" until B's own request completes. **Test X70:** pointer notifications reach
  a window only over its own pointer-catching elements.

<a id="s8-10"></a>
### 8.10 Uncharged host memory

- **The snapshot is charged.** After the compile items, the host keeps only the data files
  (non-`.lua`) of the snapshot and the content hash; those kept bytes are charged to the
  script's memory cap. During the load, before the apartment exists, the whole snapshot
  (at most 8 MiB) is held under the load's reservation of the script's cap, which counts
  toward the 512 MiB sum from the start of the load. There is no separate bound for
  retained snapshots outside the caps (this resolves EM OPEN O-4 against EM §3.4's
  136 MiB bound, §13).
- **A hot reload fits its allowance.** The new apartment of a hot reload reserves its own
  cap (its manifest's `memory_mib`, at most 64 MiB) from the **64 MiB reload allowance**;
  its snapshot, its retained data files and the hand-off bytes it receives are charged
  **inside** that cap, never on top of it. So a reload always fits the allowance, whatever
  the cap (D17 §8). The old apartment's cap stays in the 512 MiB sum until its close item
  has run and its final flush has completed (§7.6 rule 7.6.5); then the new cap takes its
  place in the sum and the allowance is free again. A hand-off table that would not fit
  the new cap is refused as too large and the reload continues without it.
- **The retained entity lists** have the fixed host-wide bound of §7.5 (at most 15 MiB),
  listed beside the 512 MiB sum and the 64 MiB reload allowance in E5.
- **Test X27:** a script whose data files are 8 MiB and whose heap then grows hits its cap
  8 MiB earlier than one without data files; a hot reload of a script with
  `memory_mib = 64`, 8 MiB of data files and a 1 MiB hand-off stays within the allowance
  and the 512 MiB sum at every item; E5 reports the retained-list peak.

<a id="s8-11"></a>
### 8.11 The abort is sticky

Once a call's deadline has passed or its units are spent, every later hook check in that
apartment, in any of its threads, aborts again until the call has returned to the host. A
`pcall` or `xpcall` that catches nothing (the abort is not catchable, D17 §8) cannot run a
loop past it, and neither can a coroutine resumed from a handler. **Test X9:** a call
that, inside `pcall` inside a coroutine, loops forever and on any error resumes another
coroutine that loops: the call ends at its deadline with one abort.

<a id="s8-12"></a>
### 8.12 The heavy-frame gate's clock origin

The gate measures from the **frame's work start**: the start of the frame's redraw handling
minus the main-thread time spent in this frame's step-1 callbacks since the previous
present (IB D8 runs step-1 work in callbacks as events arrive). Idle time between frames
(pacing, vsync waits) is never counted. This matches the wall-time rule the perf records
measure. In deterministic mode the gate is off (D17 §4) except under the "every frame is
heavy" hook (§7.4). **Test X86:** with 30 ms of idle pacing between frames and 5 ms of
step-1 callback work, a frame reaching step 7 15 ms after its redraw started is 20 ms old
and is not gated; the same frame reaching step 7 18 ms after its redraw started is 23 ms
old and is gated. The idle 30 ms appear in neither figure.

<a id="s8-13"></a>
### 8.13 Turns split by the 2 ms cap, and gesture windows

- A turn is a sequence of calls (filter, notifications in order, due timers, `Tick`).
  When the 2 ms cap or the gate stops it, it resumes in the next frame that reaches that
  script, at the first call not yet made, before any notification that arose later.
- **Where a split turn resumes in the round robin.** A split turn may resume as the
  first script of its group (EA1a §2.6: lease holders, then the others) **at most once**.
  If that resumed part is split again, the rest of the turn resumes only when the
  ordinary rotation of the round robin next reaches the script, never at the head. So a
  script whose own calls fill the 2 ms cap every frame (for example a 1,024-entry
  delivery set) runs first in at most one frame of that turn, and every other script of
  its group gets its turn in rotation (D17 §4; X17).
- `Tick(dt)` receives the wall time (deterministic mode: simulated time) since the last
  `Tick` that script received, so `dt` sums correctly over split and skipped turns.
- **The gesture window** of a gesture is the turn that delivers it, from the start of the
  call that delivers the gesture notification to the end of that turn's `Tick`, whatever
  frames that turn spans. D17 §4 case 2, §7.2 and `Files.Pick` read "in this frame's
  6b turn" as this window. EA1a's narrower "gesture call" (EA1a §2.1, §7.7) is replaced
  by it (§13), so that a gesture's effect does not depend on whether the script acts in
  the delivering handler or later in the same turn.
- **What opens it.** Only the delivery of a gesture opens a window: an action start's
  notification, an `/ext:` line's notification, or the **press** notification of a solid
  press. The release and click of that press carry its `gesture` flag for information
  only and open nothing, so a press held for 10 s does not open a second window when it
  is released.
- **How long it lasts (L93).** A split turn can span many frames (its rest waits for the
  ordinary rotation, and a throttled script runs at half rate). So the window closes at
  the **first** of: the end of the delivering turn; the first user key press, pointer
  press or wheel step after the gesture that resolves outside that script's own windows
  and actions (as a key elsewhere ends D17 §4 case 1); **1 s** of wall time after the
  delivering call started (**60 frames** in deterministic mode). A request made after it
  closed is treated as outside a gesture window (rule 7.2.2: a focus request returns
  `false`). The 1 s covers a turn split by the 2 ms cap and resumed at the next floor
  frame (at most 250 ms + one frame later, §7.4), so X89 still holds under gating.
- **Known limit: a gesture turn split twice under permanent gating (fails safe).** Under
  permanent gating ext runs only in floor frames, about every 250 ms (every 16th frame in
  deterministic mode, §7.4). A gesture turn split once resumes at the head of its group in
  the next floor frame, within the 1 s. If that resumed part is split again, the rest of
  the turn waits for the **ordinary rotation** (above), which advances only in floor frames;
  with several scripts ahead of it that each fill the 2 ms cap, the rest can start more
  than 1 s (60 frames) after the gesture (four floor frames are already 64 frames in
  deterministic mode). The window has then closed: a focus request in that rest returns
  `false` and focuses nothing, as for any request outside a gesture window, and the user
  presses the key again. This is accepted: it fails safe (no focus without a live
  gesture), it needs both permanent gating and a turn that fills the 2 ms cap twice, and
  lengthening the window would weaken L93. X89 covers the split over two frames, which
  must succeed, and this case, which must fail safe.
- **Test X89:** with the 2 ms cap forced by heavier scripts so that a note-taker's turn is
  split over two frames, its action start still opens its window with the text box
  focused, every time; also under the "every frame is heavy" hook. A split gesture turn
  whose `Focus` call comes after the user pressed `W` (resolved to the world), or more
  than 60 frames after the gesture, returns `false`; a solid press held for 10 s opens one
  window, at the press. **Twice-split case:** under the "every frame is heavy" hook, with
  four other scripts whose turns fill the 2 ms cap ahead of the note-taker in the
  rotation, a gesture turn split twice whose `Focus` comes in the rest of the turn more
  than 60 frames after the gesture returns `false`, no text box is focused, and `W`
  pressed afterwards moves the player.

<a id="s8-14"></a>
### 8.14 Outstanding reservations and the memory cap

Every outstanding request reserves, against the script's memory cap at the call, the
memory its result will hold until delivery, never a flat maximum:

- A `Store` read of *n* stored bytes, taken from the pending-write map (§7.6) when the
  key has an entry there and otherwise from the **file index**'s exact byte size (§2,
  rule 7.6.1), reserves **ρ × *n* + ε × min(*n*, 8,192)**: ρ is the neutral reader's
  declared parsed bytes per stored byte and ε its declared host memory per value-model
  element, counted exactly as the value model counts them: **every key and every value
  counts one**, booleans, numbers, strings and tables alike, a nested table being one
  value of its parent whose own keys and values count again (D17 §8, EA1a §2.5; at most
  8,192 elements, L34). The neutral format spends at least one stored byte per element
  counted this way (a table's opening bracket, a separator or closing bracket after each
  entry, an implicit array index included), so min(*n*, 8,192) bounds the count; E5
  checks this with ρ and ε (OPEN O-4). Both are
  **worst-case bounds over the whole value model**, not typical figures: E5 declares them
  as the maxima over hostile values built to maximise each (for example 8,192 empty
  tables, or thousands of 1-byte strings, in the fewest bytes), at least 1.0 and 0
  respectively (EA1a's OPEN-2). A key with no file and no pending entry reserves nothing
  and answers absent.
- **The read never passes its reservation (AT5).** Another program may replace or grow a
  key file after the file index was built. The worker opens the key file as §8.8 says,
  checks the size **on the handle** against the file index's exact size, and reads at
  most that many bytes plus one; a file whose size on the handle differs from the index,
  or that yields more bytes, answers `fn(nil, "invalid")` and is not parsed (the script
  log names the key; the file stays for the next open's checks). The parse runs under the
  reservation as its memory limit: a parsed result whose measured size passes its
  reservation is discarded and answers `fn(nil, "too large")`, **never delivered**. So a
  delivered value never holds more than the reservation charged at the call. A pending
  entry's bytes cannot change (rule 7.6.3), and its parse is bounded the same way.
- A bridge read reserves by the same formula over the plugin-data file's size (at most
  1 MiB), checks that size on the handle and reads at most it plus one byte; a
  `Files.Pick` reserves 3 MiB.
- A reservation that would pass the script's memory cap answers `nil, "out of memory"`
  (**returned, not raised**) and queues nothing. This replaces EA1a §5.3's raised memory
  error (§13): a refused reservation is a resource answer like "busy", not a fault in the
  script's code.

So 32 outstanding small reads answer `nil, "busy"` at the 33rd, as D17's exit criterion 2
expects. **Test X25** (with the hostile-file cases: a key file replaced by a 1 MiB file, or
grown, after the file index was built; a value of 8,192 empty tables; a value of 1-byte
strings: no delivered value passes its reservation, no read passes the indexed size plus
one byte, and the script's memory stays under its cap).

<a id="s8-15"></a>
### 8.15 Compatibility functions removed from the ext profile

The ext profile's standard library tables hold exactly:

| Table | Functions |
|---|---|
| `string` | `byte`, `char`, `find`, `format`, `gmatch`, `gsub`, `len`, `lower`, `match`, `rep`, `reverse`, `sub`, `upper` |
| `table` | `concat`, `insert`, `maxn`, `remove`, `sort` |
| `math` | `abs`, `acos`, `asin`, `atan`, `atan2`, `ceil`, `cos`, `cosh`, `deg`, `exp`, `floor`, `fmod`, `frexp`, `huge`, `ldexp`, `log`, `log10`, `max`, `min`, `modf`, `pi`, `pow`, `rad`, `random`, `randomseed`, `sin`, `sinh`, `sqrt`, `tan`, `tanh` |
| `coroutine` | `create`, `resume`, `running`, `status`, `wrap`, `yield` |

`table.foreach`, `table.foreachi`, `table.getn`, `table.setn`, `string.gfind` and
`math.mod` are absent. `math.max` and `math.min` charge their argument count (as
`select` does, D17 §8). **Test X8:** each removed name is nil; `table.foreach` cannot be
reached by any route.

<a id="s8-16"></a>
### 8.16 Holds: a separate ext pool and one suggested default per stroke

- Holds of ext actions come from their own pool of **64** for all ext scripts, on top of
  KM §7.3's 256, which stay whole for non-ext contexts. A binding whose hold would pass
  the ext pool does not start (the press resolves to nothing further).
- A manifest may suggest **at most one** default binding per stroke per script, and no two
  suggested defaults of one script in one context may **overlap**: one stroke's completion
  can complete the other. That covers equal strokes after KM §5.1 normalisation, a chord
  and a single button that is one of its buttons, or two chords of which one's buttons are
  a subset of the other's (a simultaneous chord and a single-button binding both fire when
  the chord completes, KM §7.2), two strokes with the same last button whose modifier
  conditions one press can meet together (KM's modifier matching), and for axis actions
  any two whose modifier sets can match at once (a subset-modifier match). An overlapping pair is refused by `scripttool check` and at load (EM's
  finding code), so one press never starts two holds of one script through its defaults.
  The user's own bindings are not limited by this rule (§4.2).
- **Test X41:** a script binding one stroke to 64 actions by hand, held with four other
  such keys: built-in movement still starts and stops normally; manifests suggesting
  `A+B` and `B`, and two axis actions on `Shift+Wheel` and `Wheel`, each pair in one
  context: refused.

<a id="s8-17"></a>
### 8.17 Leases end in the frame of the event; picking follows the screen

D17 §5.4 ends leases "in the same frame" as the events that take control back. Every lease
(the camera lease of EA1b §4 and any later one) therefore ends at the event, and the
user's view or control returns at step 3 of that frame (or of the next frame when the
event comes after step 3, "Event time" below), at the first of:

- the 350 ms expiry (L63);
- the game window losing keyboard focus, including when a `Files.Pick` dialog opens;
- the script's suspension for any reason: watchdog, aborts, the panic chord, or a grant
  revocation (§8.23);
- the frame in which a backend switch or session end is **queued** (rule 7.4.2), not the
  later frame in which that apartment's unload item runs; this adds a "switch" end reason
  to EA1b §4.5 (§13);
- the frame in which the apartment's unload is **requested**, for any other cause (the
  user's unload in the script manager, `Core.Unload`, developer mode turned off, a full
  or hot reload, the client's exit), not the later frame in which its unload item runs
  (D17 §5.4 "in the same frame").

**No lease or intent from an `Unload` handler.** The "unload" end and the discard of
intents happen when the unload is requested, before the handler runs, so the handler must
not start new ones: inside an `Unload` handler, a lease request (`Camera.Take` and any
later lease) is refused as if taken, lease and overlay intents are refused, `Sandbox`
commands answer a refusal, and every intent the handler queues anyway is discarded when it
returns. During a hot reload the old apartment's handler therefore never holds the global
lease slot (L82) against the new apartment.

**Event time.** Every event ends the lease at once. For an event in or before step 3 the
user's view and control come back in step 3 of that frame. For an event after step 3
(for example a suspension after a third abort in 6b, or a revocation applied after
step 3) no later renewal, intent or apply of the lease takes effect, and the user's view
and control come back at step 3 of the **next** frame, the first step 3 after the event.

**Picking under a lease.** While a lease changes the rendered view, every pick that the
user's own pointer makes (clicks, selection, hover, and IB's pointer actions) uses the
view and projection of the frame being presented, the one the user sees, so a click
selects what is under the pointer on screen. A script can neither aim the user's click at
something the user does not see nor learn more from it than from an unleased click.
Native UI anchored in the world follows the same presented view.

Covered by X44.

<a id="s8-18"></a>
### 8.18 Coalesced keymap recompilation

All ext registry changes of one frame (registrations, unregistrations and ext context
changes) are applied at the item-7 drain as **one** recompilation, never one per script.
That recompilation is ext UI time (D17 §4) and E8 measures it at the registry caps (256
actions, 16 contexts) and for a backend switch that unregisters 16 scripts in one frame.
**Test X21:** 16 scripts unloaded in one frame cause exactly one recompilation.

<a id="s8-19"></a>
### 8.19 Where `keymap-ext.toml` lives

It lives in the directory of the **configured** keymap path (the link's directory when the
keymap file is a symbolic link; `$TESSERA_KEYMAP_FILE`'s directory when set). Its own
reading and saving follow KM §6.5, and the settings crate's install guard applies to the
**resolved** target of every write, so a link can never lead a save into the install.
Ext bindings and ext contexts are saved **only** to `keymap-ext.toml`; the user's own
keymap file is never written because of an ext registration, binding or unload, and its
built-in rebindings are saved and read back as KM §6.5 says whatever ext holds. EM §3.1
places the file the same way (§13).
**Test X61:** a `keymap-ext.toml` link that points into a fake install directory: the save
is refused with the install-guard report; with `$TESSERA_KEYMAP_FILE` pointing outside the
settings directory, `keymap-ext.toml` is created beside it. **Test X93:** bindings saved
for 70 script ids: only `keymap-ext.toml` grows; a built-in rebinding made in the same
session is still saved to the user's keymap file and read back strictly; a damaged
`keymap-ext.toml` never changes the user's keymap file.

<a id="s8-20"></a>
### 8.20 Inherited 0014 open item 2

Ext-caused state changes applied in steps 2 and 3 (`Sandbox` teleports and clock changes,
`Input.Drive` movement) must run plugin code only where M5's resolution of IB open review
item 2 lets the user's own equivalent changes run it. Until M5 resolves it, this is a
known gap recorded here, and X71 asserts the stronger property for the driven actions,
which §7.7 makes possible: no plugin handler runs outside steps 1 and 7 during driving.

<a id="s8-21"></a>
### 8.21 Partially loaded apartments at a backend switch

A load in progress when a switch is queued: its snapshot job, if not yet delivered, is
cancelled and its result discarded; its pending compile items and top-level run are
dropped; if its state exists it gets one close item (no `Unload`, its top level never
ran). It counts among the at most 17 apartments from the moment its load reserved its cap.
**Store file work is never cancelled half-way:** a store open (temporary-file removal,
the file index, a wipe) that has started on the store's I/O queue, inside the snapshot job
or as EM's phase C, completes in order on that queue, and only its result is discarded.
"Cancelled" above means only that no later step of the load runs. A grant dialog showing
or queued for that load closes as "Not now" and writes nothing (EM §10.4). A load whose
dialog was already answered, or that needed none, and that is waiting on its grant-record
write or its store probe when the switch is queued: that write or probe completes in
order on its queue (a record the user decided is written whole, never half), and the load
is then dropped with its reservation released, before its store open and top level.
**Test X87.**

<a id="s8-22"></a>
### 8.22 Wording of D17 §5.4

"A throttled script's half-rate tick at 8 frames per second" means a client running at
8 fps, where half rate is one tick every two frames, 250 ms. The API specs use that
wording. No test.

<a id="s8-23"></a>
### 8.23 Grant revocation suspends in that frame

Revoking a grant of a loaded script, required or optional, from the script manager or by
any other route, **suspends the script in that frame** (D17 §4 lists grant revocation as a
suspension reason; EA1a §2.9 agrees). Suspension does what it always does: pending intents
are discarded, leases end (§8.17), no notification, timer or subscription event accrues,
and no call of the script runs until the user resumes it. Then:

- a revoked **required** capability cannot be resumed: the script is unloaded (close
  item, no `Unload` handler, as for any suspended script), because refusal is whole
  (D17 §2);
- after a revoked **optional** capability the user may resume the script; from then on
  every native of that module raises `"not granted"` (natives re-check the grant on every
  call, D17 §5.1, so a module table kept anywhere fails closed), `Ext.Has` answers
  `false`, and what the module made is gone (EA1a §2.9 lists it per module); an entity
  `Follow` or subscription made under `entity.read` does not survive.

**What natives check: the apartment's effective set.** Every native, and every apply step
of an intent, checks the capability against the **apartment's effective set**: the
capabilities granted when that apartment was loaded, shrunk by every revocation since, and
**never grown** during the apartment's life. It never reads the live grant table. So after
the user revokes optional `camera`, resumes the script and then widens `camera` again in
the script manager, the kept `Camera` natives still raise `"not granted"` until the
script's next load (a widening applies only at a load, EM §9.3, §9.7).

**Handles carry their module's capability.** A handle (an entity, a node, any object a
module mints) stays a valid Lua object after its module's capability is revoked, but every
native that takes a handle as an argument, of any module, checks the capability of the
module that **minted** it against the effective set and raises `"not granted"` if it is
gone; an intent that holds a handle is re-checked the same way when it is applied. So a
kept entity handle cannot be passed to `lease:Follow` (or any later native) after
`entity.read` is revoked, and a `Follow` already running ends at the revocation's
suspension and is not restarted by a resume.

**Suspension and input.** Suspension, for any reason, deactivates the script's ext
contexts and releases their holds (L61) in that frame; its bindings consume no input and
record no action edges or gestures while it is suspended (a press of one of its strokes
resolves as if the binding did not exist; the panic chord is resolved before any of them,
L64). On resume the contexts are **not** restored: they stay inactive, `Input.IsActive`
answers `false`, and the script activates them again itself (within L62's rate).

There is no "refuse from the next call without suspension" path (this replaces EM §9.7's
optional-capability rule, §13).

**Re-reading the grant store is a route.** The user may edit `ext-grants.toml` by hand
(§4.3 assumption 3), and EM §8.5 re-reads it when the script manager opens. When a
re-read leaves a **loaded** script with fewer grants than it runs with (its record
removed, made invalid under EM §8.3, or narrowed by a capability), that is a revocation
by another route: the script is suspended in the frame the re-read result is applied,
exactly as above, required and optional alike. A re-read that would **widen** a loaded
script's grants changes nothing while it runs; the wider record is honoured only at the
script's next load, which goes through EM §9.2's checks (pinned capability list and
content hash) like any load. A record that breaks EM §8.3's rules (for example one that
grants `session.server` with a local-only capability) is ignored and grants nothing,
however it was written.

**A failed write never undoes an earlier session decision.** The overlay holds one entry
per key, and a newer decision of a key supersedes the entry before it. When a write fails,
a decision made in the script manager (a revocation, a widen, Reset, Remove, a local-only
switch, a manager-started dialog's Allow) stays in the overlay for the session. A load's
own record write (EM §9.2 step 6: the record update at a load, a load dialog's Allow
included) is **taken back** when it fails and the load is cancelled (EM §8.5); taking it
back **restores the overlay entry that write superseded**, or leaves no entry if there was
none, and never simply removes the key's entry (a newer decision of the key made
meanwhile stays, as EM §8.5 says). So a revocation, or
Reset's or Remove's "no record", whose own write failed earlier is still in force after
a later failed load write of the same key, and the grant table never falls back to a
document view that still grants what the user revoked or still holds the record the user
reset. (Example: with the disk full, the user revokes optional `b`, whose write fails; a
later load of the script with a changed hash writes its update, which fails and is taken
back: `b` stays revoked, and no later successful write puts it back on disk.)

**Developer mode turned off.** Developer scripts load only with developer mode on (D17 §2).
Turning it off unloads every loaded developer script (EM §9.8) and, in the same frame,
**cancels every developer load in progress** as a narrowing does (below and §8.21): a grant
dialog showing for it closes as "Not now" and writes no decision, store work in progress
completes on its queue, the state gets one close item if it exists, and the top level never
runs; a developer load still waiting (a user-started load queued in `Starting`, a reload
request) is refused with "Developer mode is off". The user's unload of a script whose load
is in progress (the unload the script manager offers for a loading copy) cancels that load
the same way. So no developer script finishes loading, under a grant decided in developer
mode, while developer mode is off.

**The session overlay.** A re-read must not undo what the user decided in this session.
The host keeps, for the client session, an **overlay** of the user's decisions that the
document does not hold: those whose write has not completed or failed, and all of them
while the document is not writable (a newer `version`, EM §8.6). Every re-read result is
combined with it: narrowings of the overlay (a revocation, Reset, Remove, a local-only
switch) always apply over the re-read result, so a revoked grant never returns without a
dialog; widenings of the overlay (an Allow in the dialog this session) apply too, because
the user made them. A narrowing by another route (below) still narrows an overlay entry of
its key, so a hand edit that narrows always narrows (EM §8.5).

**What narrows by another route.** Only a change between two documents that Tessera
**read and understood as a whole**: readable, of a version it supports, not damaged as a
whole (EM §8.6); a missing file reads as an understood, empty document. A key narrows when
the newer such document grants less for it than the previous document view did. So:

- **A re-read that cannot be used changes nothing.** A newer `version` (whose records are
  all ignored), a document damaged as a whole, and a failed open, kind check or read (for
  example `EACCES`, `EIO`, or on Windows a sharing violation while an editor or a virus
  scanner holds the file) are neither a narrowing nor a widening and suspend nothing: the
  previous document view stays the base of the grant table and of the next comparison.
  A transient failure when the script manager opens therefore never suspends a script.
- **Tessera's own writes never narrow the user's decisions.** The completion of a write
  is applied as a read result only for the keys the write did **not** carry. For each key
  it carried, the record it wrote is not a narrowing, and a **newer** decision of the same
  key, applied while the write was in flight, stays exactly as the user made it: per key,
  the newest decision wins over the completion of every earlier write. Examples: a
  manager revocation of `b`, then a widen of `b` (or a first-enable Allow after a Reset or
  Remove) before the revocation's write completes: `b` stays granted, no load that decided
  on `b` is cancelled, and no newly loaded script is suspended by the older write's
  completion.
- **Replacing a damaged document narrows nothing.** When a write replaces a document
  damaged as a whole (after EM §8.6's backup), it writes every record of the grant table
  as it stands, not only the keys it carries, so its completion grants no less than the
  grant table did.

Only a re-read of an understood document can narrow by another route as above.

**Loads in progress.** A narrowing that reaches a (kind, id) whose load is in progress
(past its grant decision, before its top-level run: a manager revocation, Reset, Remove,
a local-only switch or a narrowing re-read) cancels that load as §8.21 cancels one at a
switch: store work in progress completes on its queue, the state gets one close item if it
exists, and the top level never runs. Phase D's delivery and the top-level item each
re-check the load's grant set against the current record before they run, so a load never
proceeds without a capability that was revoked after its dialog (D17 §2, "never loads
degraded").

**One id, one apartment.** A load of an id is refused while the same id of the other
folder kind is loaded **or loading**, from the moment that load took its reservation
(EM phase A) until its close item has run (L26). So two loads of one id from both kinds
that start together (for example two `autoload = true` records at world entry with
developer mode on) cannot both pass: the second is refused. **Test X91:** revoke an optional `camera` grant while a
lease and a path are active and an optional `entity.read` grant while a `Follow` runs:
the script is suspended in that frame, the lease ends in that frame, no intent of the
script is applied after it, and after the user resumes the script every call of either
module raises `"not granted"`; revoke a required grant: suspended, then unloaded, with no
`Unload` call. Further cases: revoke optional `camera`, resume, then widen `camera` in the
manager: the kept `Camera` natives still raise `"not granted"` until the next load; revoke
optional `entity.read`, resume, then pass a kept entity handle to `lease:Follow` (1b):
`"not granted"`; a manager revocation whose write fails, then a manager open that
re-reads the document: the grant stays revoked; a newer-`version` document re-read while
scripts run on session-only grants: nothing suspended; a re-read of a document damaged as
a whole, and (U) a re-read whose open fails, while scripts run: nothing suspended, and a
later write that replaces the damaged document suspends nothing; a manager revocation of
an optional grant and a widen of it in one frame with the grant-store write latency at
2 frames (EM `--latency 2`): the grant table grants it after both writes complete (an
`[[expect]]` of granted); a Reset, then a first-enable Allow and load before the Reset's
write completes: the new load is neither cancelled nor suspended; a suspended script's ext
context and its holds are released in that frame, and stay inactive after resume. **Failed
load-write rollback** (ST, with EM §11.8's hook that makes grant-store writes fail): the
user revokes optional `b` and its write fails; a load of the script with a changed content
hash writes its record update, which also fails and is taken back: `b` stays revoked (an
`[[expect]]` of not granted), and after writes succeed again the next load writes no
record granting `b`; likewise a Reset whose write fails, then a first-enable Allow whose
load write fails: the grant table holds no record for the key, and the old record does not
return.

<a id="s8-24"></a>
### 8.24 No store or hand-off crosses an `author` change

A script's `Store` and any hot-reload hand-off belong to the `author` the user granted
(D17 §5.2: an `author` change re-asks every grant and empties the store, in every mode).

- **Hot reload is full on any trust-relevant change.** Before comparing, the host
  validates the new manifest exactly as a load would (EM §4.14, including the registry
  check of required module majors and minors); a manifest a load would refuse ends the
  reload as a failure. A developer hot reload is hot (keeps the store open and passes a
  hand-off) only when the new manifest has the same `manifest` format version, `author`,
  `api`, required and optional capability lists, `memory_mib`, `[[actions]]`,
  `[[contexts]]`, `[modules]`, `[optional_modules]` and `[server]` table as the running
  one. Any difference makes it an unload, the grant rows of EM §9.2, and a load without a
  hand-off; an `author` difference also empties the store before the new apartment opens
  it. This extends D17 §10's capability rule and EA1b §8.1 (§13).
- **The store carries its owner.** When a store directory is created, the host writes in
  it an owner tag holding the `author` it was created under (a host file that is not a
  key file and cannot be read or written through `Store`). When a store is opened, on its
  I/O queue, a tag that is missing, unreadable or names another `author` than the loading
  script's causes a **wipe** before the open completes, and the script manager reports
  "Stored values of another author were erased". So a store left without its grant record
  (a damaged or duplicated grant document, or a crash between a record's removal and the
  store's), or a folder copied in by hand, never reaches a script by another author. This
  replaces EM's "Stored values found … They will be kept" for a store whose owner does
  not match (§13). Removal (EM §8.7) deletes the store before the record, so a crash
  between them leaves a record with no store, which is harmless.

**Test X92:** a developer script hot-reloaded with only its `author` changed: no hand-off
is delivered and `Store.Keys()` is empty in the new apartment; a store directory whose
owner tag names another author, and one with no tag, present when a script with no grant
record first loads: both are wiped before its top level runs; a hot reload whose new
manifest fails validation keeps the old apartment running.

<a id="s8-25"></a>
### 8.25 Consent controls need a fresh pointer entry

D17 §5.3's prompt accepts a press on "Allow", on a capability box or on "Load
automatically" only when **all** hold: the press comes at least 500 ms after the dialog
appeared; the pointer **entered** that control (a pointer move observed from outside the
control to inside it) **after** the 500 ms delay ended; and the pointer stayed inside the
control from that entry to the press. A pointer that entered during the first 500 ms and
rests there, or that was over the control when the dialog appeared, must leave and enter
again. Keys only refuse. **Test X48** covers it.

<a id="s8-26"></a>
### 8.26 The ext z-range is below plugin windows and native panels

D17 §4 places ext windows in their own z-range below plugin windows and native panels, and
its M5.5 exit criterion 2 requires that "an ext window never covers a plugin window,
whatever their `ZOrder`". The rule (closes T94):

- **One range, below.** Every ext composition entry (windows, nested surfaces of any later
  kind, world labels and badges) is drawn in the ext range, and the whole ext range is
  drawn **below** every plugin window, every native panel and the reserved layer, and
  above the world. Hit testing and hover use the same order, so a point covered by a
  plugin window or native panel never reaches an ext element.
- **Nothing a script does leaves the range.** A script's `ZOrder` (any value), `Activate`,
  a solid press that raises the script's block (§7.2), re-parenting, a nested surface, a
  show, and a show while a consent prompt or another reserved-layer surface shows all order
  entries **within** the ext range only. A nested surface is drawn inside its window's
  place in the range, never outside it. No API offers a way to place an ext entry in the
  plugin or native range, and the host clamps every ordering value it receives into the
  ext range.
- **While a prompt shows** the ext range is not drawn at all (D17 §5.3), and an ext show
  in that time changes only the ext range's order for when it is drawn again.

**Test X95** (CI, golden image and hit test): a plugin window at its lowest `ZOrder` over
part of an ext window at its highest, then after the ext window's `Activate`, a solid press
on it, a re-parenting of its content, and (in the slice that adds one) a nested surface
inside it; and an ext window shown while a prompt shows, then after the prompt closes. In
every frame the plugin window's pixels are unchanged against a run without the ext script,
and a click and a hover on the plugin window's area reach the plugin window. The same with
a native panel (the chat window) in place of the plugin window.

---

<a id="s9"></a>
## 9. The earlier hardening items, restated as requirements

<a id="s9-1"></a>
### 9.1 `string.rep` and degenerate wrapper arguments

Every wrapped function charges, before it starts, the largest of bytes read, bytes
produced and loop iterations its arguments imply (D17 §8). `string.rep(s, n)` charges
max(n, result size); `n` ≤ 0 or empty `s` returns `""` without calling the stock loop; a
result over 4 MiB is refused before it starts. `table.concat` charges the range;
`unpack` the range before the stack check; `table.insert`/`remove` with a position below
1 are refused. **Test X12:** `string.rep("", 2^31−1)`, `("x"):rep(2^31−1)`,
`table.concat` over 10^8 empty strings, `unpack({}, 1, 10^8)`, `table.insert(t, −2^31, v)`:
each returns or errors within one call's units, no frame passes 50 ms.

<a id="s9-2"></a>
### 9.2 Hash flooding

In ext states: keyed full-length string hashing with a 128-bit per-state secret; keyed
main positions by multiply-shift of the whole hash and a per-table salt (XOR or addition
before a low-bit modulo forbidden); a 32-node chain bound with re-salting for tables and
the interning table; chain steps past 32 charged (D17 §1.5). On the host: every map keyed
by script, file or message input uses a randomly keyed hasher (D17 §8). **Test X13:** the
E6 flooding set (stock-sampled collisions, stock number-hash collisions, `next`-order
learning, harness-built keys sharing their low 17 bits in fresh tables of every size,
a 128 KiB file of colliding constants): no chain walk over 32 nodes, no frame over 50 ms.
**Test X14:** 10^5 `Store` keys, command names and subscription filters built to collide
under a fixed-seed hasher: per-operation time stays flat.

<a id="s9-3"></a>
### 9.3 Exfiltration routes

Every route by which data can leave a script, and its status:

| Route | Status | Test |
|---|---|---|
| `Server` messages | Only for `session.server` holders, never with a local-only capability | X62 |
| `Store` lineage | Local-only mark, wipe before a server holder opens, one I/O queue | X63 |
| Hot-reload hand-off | Never across a capability change | X64 |
| Plugin bridge | Local-only; direct handler call, never the line route (§7.3) | X65 |
| `/ext:` lines | Never sent to a backend | X66 |
| Chat | No direct route; plugin echo of bridged arguments is local only | X73 |
| Screenshot files | Closed by the `Input.Drive` allow-list (§7.7) | X43 |
| Clipboard | No API; copy only of user-selected, unchanged text (§8.4) | X52 |
| Script log | Local file, capped and sanitised; readable by local programs (residual, §6.8) | X51 |
| Decode cache | Closed by per-script visibility (§8.9) | X68 |
| Shared quotas, engine state, layout, timing | Residuals between cooperating programs (§6.8) | — |

<a id="s9-4"></a>
### 9.4 Disk caps

`Store`: 1 MiB per value as written, 1,024 keys and 16 MiB per script, 16,384 keys and
256 MiB in all (orphans and temporary files included), each key counted rounded up to
4 KiB plus 4 KiB; script log 1 MiB rotated once; `keymap-ext.toml` 128 KiB; write rates of
§8.7. **Tests X56, X57.**

<a id="s9-5"></a>
### 9.5 Per-apartment random numbers

Each ext apartment has its own 64-bit-state generator, seeded from the OS at creation
(from the test seed in deterministic mode); `math.randomseed` reseeds only that
apartment; PR §4.5's shared generator belongs to the plugin profile and no ext call draws
from or reseeds it (D17 §1.2). **Test X74:** an ext `math.randomseed(1)` changes neither a
plugin's sequence nor another script's; a plugin's `math.randomseed(1)` changes no ext
sequence; two scripts seeded alike draw independent sequences after one of them reseeds.

<a id="s9-6"></a>
### 9.6 Deep C recursion

Lua 5.1.5 bounds two different things. `LUAI_MAXCSTACK` (8,000 stack slots) bounds how
many values a C function may push, which limits argument copies (`unpack`, `select`,
`string.byte`); D17 §8 charges them. Recursion **depth** through C is bounded by the
nested C-call counter `LUAI_MAXCCALLS` (200), which stock 5.1.5 keeps **per coroutine**:
a resume starts the resumed coroutine's count afresh, so a chain of nested `resume` or
`wrap` calls, each recursing through C, can overflow the native stack. The ext build
therefore requires:

1. **One counter per apartment, carried across resumes.** Every C boundary the
   interpreter crosses (a C function calling back into Lua, a metamethod call, a `gsub`
   replacement function, a `sort` comparator, a `pcall`/`xpcall`, a `resume` or a `wrap`
   call, a nested parser level while compiling) increments one counter shared by all
   threads of the apartment, and a resume continues the resumer's count instead of
   starting at zero. Past **200** the operation raises the ordinary catchable Lua error
   "C stack overflow" in the calling thread. Host natives that recurse over script values
   (value conversion, the neutral reader) are iterative or bounded by the value model's 32
   levels and do not depend on this counter.
2. **A stack reserve.** Every thread that runs ext calls reserves at least **8 MiB** of
   native stack (the client's main thread included, which on Windows needs a linker
   setting; the harness's threads likewise). E6 measures the largest native stack one
   nesting level uses on each path of item 1 and requires 200 levels plus the host's own
   frames to stay below half the reserve; if they do not, the counter's limit is lowered
   until they do, and the new figure is recorded in L21.

**Test X15:** recursion to the bound through `gsub` callbacks, `sort` comparators,
`__index` and `__lt` metamethods, `pcall` chains, and 1,000 coroutines each resuming the
next (by `resume` and by `wrap`), each also recursing through C, and a source file nested
to the compiler's level limit: every case ends with a catchable Lua error, never a crash,
on the client's thread and on the harness's.

---

<a id="s10"></a>
## 10. Limits register

Every resource limit of extended scripting (§0.1 rule 6), with its number. This register is
the single home of these numbers (§0.1): other specs cite the L-number. Argument-validity
domains and grammar lengths stay in their defining specs (§0.1 rule 6). "Source" names the defining text;
"§" rows are new or changed here; an EM or EA row names the companion spec that states
the limit's script-visible form. "Exceed" is what happens past the limit.

<a id="s10-1"></a>
### 10.1 Interpreter, calls and frame time

| # | Limit | Value | Exceed | Source | Threats |
|---|---|---|---|---|---|
| L1 | Work unit | ≈ 5 ns on the reference machine | — | D17 §8 | T10–T12 |
| L2 | Hook interval | 1,000 units | — | D17 §1.5, §8 | T11 |
| L3 | Per-call wall deadline | 4 ms (every call, top level and `Unload` included) | Abort at next instruction, sticky (§8.11) | D17 §8 | T10, T9 |
| L4 | Per-call units | 800,000 | As L3 | D17 §8 | T10 |
| L5 | Ext time started per frame (6a–6b) | 2 ms (400,000 units deterministic) | No further call or item | D17 §4, §8 | T17 |
| L6 | 6a slice | 1 ms (200,000 units), whole items, ≥ 1 item per ungated frame | Wait | D17 §8 | T16 |
| L7 | 6c slice | 1 ms, ≥ 1 apartment per ungated frame; `LUA_GCSTEP` 20 | Wait | D17 §4 | T16 |
| L8 | Heavy-frame gate | 22 ms from the frame's work start (§8.12) | Nothing starts | D17 §4 | T84, T85 |
| L9 | Floor frame | after 15 gated frames or 250 ms (deterministic mode: 15 frames only); one 6a item | — | §7.4 | T84 |
| L10 | Switch timeout | 2 s (120 frames deterministic); then close without `Unload` | — | §7.4 | T84 |
| L11 | Switch completion under permanent gating | with *a* apartments ≤ 2 s + *a* × (250 ms + one frame): slice 1a *a* ≤ 16, about 6 s, deterministic mode ≤ 376 frames; from slice 1b *a* ≤ 17 (a hot reload), about 6.25 s, ≤ 392 frames; a queued line ≤ 250 ms + one frame (16 frames) plus the round robin | — | §7.4 | T84 |
| L12 | Worst ext time per frame | Both branches within the ceiling: 7 ms + R + 2·S, and 2 ms + K + S (or 2 ms + C + S) after a compilation or a close. Ceiling 13 ms (D17 §8); while OPEN O-2 is open, **12 ms** in slice 1a (so S ≤ 1 ms at R = 3 ms, and K, C ≤ 10 ms − S, so ≤ 9 ms) and **11.4 ms** from slice 1b (S ≤ 0.7 ms, and K, C ≤ 9.4 ms − S, so ≤ 8.7 ms) | Fallback lowers caps: the 4 MiB allocation (R), the 128 KiB source cap (K), the 64 MiB maximum (C) | D17 §8, §7.1.6 | T16 |
| L13 | R (one uninterruptible operation) | ≈ 3 ms at 4 MiB (measured in slice 1a) | Fallback | D17 §8 | T11, T16 |
| L14 | S (one collector step) | ≤ 1.5 ms required | Fallback | D17 §8 | T16 |
| L15 | K (one compilation) | ≤ 9 ms (13 ms ceiling, and slice 1a's 12 ms with S ≤ 1 ms); ≤ 8.7 ms from slice 1b while OPEN O-2 is open (11.4 ms with S ≤ 0.7 ms); in general K ≤ ceiling − 2 ms − S | Lower the 128 KiB source cap | D17 §8, L12 | T16 |
| L16 | C (one state close) | ≤ 9 ms (13 ms ceiling, and slice 1a's 12 ms with S ≤ 1 ms); ≤ 8.7 ms from slice 1b while OPEN O-2 is open; in general C ≤ ceiling − 2 ms − S | Lower the 64 MiB maximum | D17 §8, L12 | T16 |
| L17 | Rolling time per script | mean ≤ 1 ms per frame over 1 s; throttle to half rate until < 0.5 ms; over for 5 s within 10 s while throttled → suspend; filter-call time excluded (rule 7.5.2a), a throttled script's filter call runs at half rate with its turn | Throttle, suspend | D17 §8, §7.5 | T17, T19 |
| L18 | Aborts | 3 in 10 s; a filter call ended by its stopping rule is never an abort | Suspend | D17 §8, §7.5 | T18 |
| L19 | Chain bound (tables, interning) | 32 nodes, re-salt | Re-salt; steps past 32 charged | D17 §1.5 | T13 |
| L20 | Hash secret | 128 bits per state | — | D17 §1.5 | T13 |
| L21 | C stack | `LUAI_MAXCSTACK` 8,000 slots (argument copies, charged ≤ 8,000 units); nested C calls `LUAI_MAXCCALLS` 200 per apartment, counted across coroutine resumes; ≥ 8 MiB native stack on every thread that runs ext calls | Catchable Lua error | D17 §8, §9.6 | T15 |
| L22 | Largest single allocation | 4 MiB (2^18 array slots, 2^16 hash nodes) | Catchable memory error | D17 §8 | T23 |

<a id="s10-2"></a>
### 10.2 Memory, scripts and workers

| # | Limit | Value | Exceed | Source | Threats |
|---|---|---|---|---|---|
| L23 | Memory per script | 32 MiB default, manifest ≤ 64 MiB; heap + host objects (EA1a §11 lists them) + kept snapshot data files + the file index + pending `Store` entries and flush bytes it wrote + outstanding reservations | Refusal, catchable error | D17 §8, §8.10, §7.6 | T23, T24, T27 |
| L24 | Memory sum | 512 MiB of loaded scripts' caps + 64 MiB reload allowance (the new apartment's whole cap, its snapshot and hand-off inside it); a closing apartment's cap stays until its final flush completes | Load refused | D17 §8, §8.10 | T28 |
| L25 | Retained entity-record store | ≤ 245,760 records, ≤ 15 MiB host-wide | Overflow counted | §7.5 | T19, T27 |
| L26 | Loaded scripts | 16; ≤ 17 apartments (the 17th only during a hot reload, slice 1b); one per id: a load is refused while the same id of the other kind is loaded or loading (from its reservation to its close item, §8.23); from slice 3 a script's server-side half counts as a **second script** against the 16 and against L24's sum with its own cap, so the bound reads "at most 16 loaded apartments plus one reload" (D17 §8, §11) | Load refused | D17 §8, §2 | T28, T89 |
| L27 | Outstanding worker requests | 32 per script, 256 in all. "Per script" is per (kind, id): both apartments of a hot reload share the 32. A request counts from its call until its result is delivered or discarded **and** any worker job it started has ended, even after the element, apartment or handle that made it is gone. Counted: worker reads, pending `Store` reads (rule 7.6.3), image decodes (§8.5; a destroyed or re-imaged element's decode until its job ends; joined requests for one file count once each, per request, not once per job), and every `Store.Set` or `Store.Delete` with a `done` callback until that `done` is delivered or discarded (a replaced value's `done` included). So every never-dropped result delivery of L30 belongs to a counted request | `nil, "busy"` | D17 §8, §8.5 | T25, T26 |
| L28 | Result reservation | `Store`: ρ × *n* + ε × min(*n*, 8,192) for *n* stored bytes (pending entry or the file index's exact size), ρ and ε worst-case bounds over the value model; the read stops at *n* + 1 bytes and a size mismatch answers `invalid`; a parsed result over its reservation is never delivered (`too large`). Bridge: the same formula over the plugin-data size ≤ 1 MiB. `Files.Pick`: 3 MiB | `nil, "out of memory"`, returned | §8.14 | T25 |
| L29 | Ext decode-cache partition | 16 MiB per script, 64 MiB in all; per-script visibility | LRU eviction | D17 §8, §8.9 | T67 |
| L30 | Host object counts | elements 2,048 per script (512 per window, that is per ext Frame, not per time frame; 256 labels), 8,192 in all (1,024 labels); timers 64; subscriptions 16 (64 in all); notification queue 1,024, plus room reserved for the results of outstanding requests (L27) and for due timers, which are never dropped; pending intents 256 per frame | `nil, "limit"`, `nil, "busy"`, or dropped and counted (EA1a §11) | D17 §8 | T24 |
| L31 | Entity events | 1,024 per script per frame; records 4,096 per frame; 60 frames retained; filter batch 64 | Dropped and counted | D17 §8, §7.5 | T19 |
| L32 | `tostring` numbers | charged to memory | — | D17 §1.2 | T3 |

<a id="s10-3"></a>
### 10.3 Values, folders and files

| # | Limit | Value | Exceed | Source | Threats |
|---|---|---|---|---|---|
| L33 | Value model depth | 32 levels | `nil, "too large"` | D17 §8 | T20 |
| L34 | Value model elements | 8,192 (`Store`, hand-off, `Server`); natives declare theirs (camera path 256) | As above | D17 §8 | T20 |
| L35 | Value model bytes | 1 MiB (`Store`, hand-off, bridge read), 64 KiB (`Server`) | As above | D17 §8 | T20, T81 |
| L36 | Folder | 256 files, 8 MiB, 4 directory levels below the script folder (a file at `a/b/c/d/x.lua` is allowed), 512 entries, 64 directories, file ≤ 3 MiB, `.lua` ≤ 128 KiB, segment 1–64 bytes | Script refused | D17 §2, EM §3.3 | T53, T54 |
| L37 | Manifest | 64 KiB; nesting ≤ 4 levels of tables or arrays below the document root; ≤ 1,024 keys | Refused | D17 §2, EM §4.1 | T58 |
| L38 | Script id, `Store` key | 1–32 bytes `a-z0-9_`; reserved names | Refused | D17 §2 | T52 |
| L39 | `Store` per value | 1 MiB as written | `nil, "too large"` | D17 §8 | T55 |
| L40 | `Store` keys | 1,024 per script, 16,384 in all | `nil, "full"` | D17 §8 | T55 |
| L41 | `Store` bytes | 16 MiB per script, 256 MiB in all; key = ⌈size⌉4 KiB + 4 KiB; temp files and pending entries count; a pending deletion adds no key and no byte of its own (the deleted file counts until its flush completes), so a `Delete` never passes L40 or L41 (rule 7.6.5) | `nil, "full"` | D17 §8, §8.7, §7.6 | T55 |
| L42 | `Store` reads | 64 reads and 4 MiB/s per script (10 s bucket); 16 MiB/s in all | `nil, "busy"` | D17 §8 | T25 |
| L43 | `Store` writes, keys | 64 newly dirty keys/s per script (10 s bucket): a key token is taken only when a call makes a clean key dirty (§8.7); one flush per script per second | `nil, "busy"` | D17 §8, §8.7 | T56 |
| L44 | `Store` writes, bytes | 10 MiB bucket at 32 KiB/s per script; 40 MiB at 128 KiB/s in all; debited at the call: in full when the call makes a clean key dirty (no entry, or an entry a flush has already taken, rule 7.6.2), growth only over an untaken value; buckets per (kind, id) for the session | `nil, "busy"` at the call | §8.7 (changes D17, OPEN O-3) | T56 |
| L45 | Script log | 1 MiB per file, rotated once; 64 KiB/s per script and **128 KiB/s for all scripts** (token buckets of one second, escaped record bytes); record ≤ 4,096 bytes after escaping, prefix included | Dropped, counted, "N records dropped" record; cut with `…[cut]` | D17 §8, §8.1 (all-scripts rate changes D17, OPEN O-3) | T50, T55, T56 |
| L46 | `Files.Pick` | one dialog in all; 1 per 5 s per script; 3 MiB; regular files only; gesture only | `nil, "busy"` / refused | D17 §8, §8.6 | T57 |
| L47 | `keymap-ext.toml` | 128 KiB, 128 contexts, 64 actions per context | Save refused | D17 §2 | T58, T60 |
| L48 | Images | ≤ 4,096 px per axis; charged before decode | Refused | D17 §8 | T32 |

<a id="s10-4"></a>
### 10.4 UI, GPU, input and trust UI

| # | Limit | Value | Exceed | Source | Threats |
|---|---|---|---|---|---|
| L49 | UI surfaces | 128 per script, 512 in all (ext pool) | Window not created | D17 §8 | T31 |
| L50 | Composition entries | 256 per script, 1,024 in all (ext pool) | Show refused | D17 §8 | T31 |
| L51 | UI memory | 32 MiB CPU, 64 MiB GPU, 64 MiB pinned (ext pool); ¼ each per script | Refused | D17 §8 | T31, T32 |
| L52 | Ext paint and upload | 1 ms paint, 1 MiB upload per frame | Wait | D17 §4 | T31 |
| L53 | Overlay primitives | 10,000 per script per frame, 20,000 in all; per script per frame the count share min(10,000, ⌊20,000 / *n*⌋) | Newest dropped | D17 §8, §7.1 | T29 |
| L54 | Overlay coverage | 0.5 screen per script, 1.0 in all; per script per frame the share min(0.5, 1.0 / *n*) | That primitive dropped, the visit goes on | §7.1 | T29 |
| L55 | Composition coverage | 1.25 screens per script, 2.0 in all; each entry min(one screen, (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1) of its unclipped displayed rectangle); badges (fixed box, §8.3) and world labels count | Script change refused; user resize stops; host change hides windows and labels | §7.1 | T30 |
| L56 | Ext overlay and composition row | 1.6 ms: 0.6 ms overlay encoding (E4, 20,000 primitives) + 1.0 ms GPU at 3840 × 2160 (E9) | Bounds shrink | §7.1 | T29, T30 |
| L57 | Solid-press threshold | alpha × effective opacity ≥ 0.5 | Not a gesture | §7.2 | T37 |
| L58 | Focus box | shown, on screen, ≥ full height and 16 logical px wide, opacity ≥ 0.5, uncovered (also by the script's own higher content, rule 7.2.2, and by every badge whose fixed box overlaps its focus area, except its own window's badge attached outside the window, §8.3); its window's badge wholly on screen and uncovered (§8.3); the caret and focus mark drawn above every badge | Focus refused or taken | D17 §4, §7.2, §8.3 | T38, T48 |
| L59 | Activation case 1 | ends on a key elsewhere, pointer leaving, or 5 s idle; text-box focus only in a gesture turn | Request returns `false` | D17 §4, §7.2 | T36, T37 |
| L60 | Actions, contexts | 64 actions and 2 contexts per script; 256 and 16 in all; ext priority −1,000..−1 | Load refused | D17 §8 | T33 |
| L61 | Ext holds | 64 in all (own pool); one suggested default per stroke per script, and no two overlapping suggested defaults of one script in one context (§8.16); released at suspension | Hold not started; manifest refused | §8.16, §8.23 | T40 |
| L62 | Ext context changes | 4 per script per second, coalesced per frame | `nil, "busy"` | D17 §8 | T39 |
| L63 | Lease | 350 ms; ends at focus loss, suspension (revocation included), a queued switch, an unload request, chord; control back by step 3 of that frame, or of the next frame after step 3; none from an `Unload` handler | Lease ends | D17 §5.4, §8.17 | T43 |
| L64 | Panic chord | default Ctrl+Shift+F12; rebindable only in Tessera's settings | — | D17 §5.5 | T41 |
| L65 | Prompt | 500 ms dead time for every control of the prompt (Deny and "Not now" included); the pointer must enter the control after it and stay inside until the press (Allow, the boxes, "Load automatically"); keys only refuse; ≤ 8 reserved surfaces | Click ignored | D17 §5.3, §8.25 | T47 |
| L66 | Reserved-layer text | code points and bytes per §8.2; ≤ 2 combining marks (category M); manifest strings that would change refused at parse time | Cut, stripped; manifest refused | D17 §5.3, §8.2 | T46 |
| L67 | Notices | one per script and message per 10 s | Counted | D17 §8 | T49 |
| L68 | Commands | 32 per script; names 1–32 bytes | Refused | D17 §3 | T71 |

<a id="s10-5"></a>
### 10.5 Modules of later slices

| # | Limit | Value | Exceed | Source | Threats |
|---|---|---|---|---|---|
| L69 | `World` rays | 64 per script per frame, 256 in all, ≤ 500 m | `nil, "busy"`; longer is an error | D17 §8 | T17 |
| L70 | `World` point queries | 256 per script per frame | `nil, "busy"` | D17 §8 | T17 |
| L71 | `Entity` queries | 8 per script per frame (32 in all), ≤ 512 results | `nil, "busy"`; truncated flag | D17 §8 | T17 |
| L72 | `Data` lookups | 16 per script per frame | `nil, "busy"` | D17 §8 | T25 |
| L73 | `Audio` | 8 voices per script, 32 in all; 8 starts/s; gain ≤ bus volume (all ext ≤ 2×) | Refused | D17 §8 | — |
| L74 | Client-local entities | 256 per script | Spawn refused | D17 §8 | — |
| L75 | `Sandbox` | 1 command/s per script; clock changes (time of day, sky) at most 4 per second for all scripts together; one outstanding teleport in all. Its slot is released at the first of: the sandbox's outcome **arrives**; the intent is discarded; its submission fails (the request number is not consumed); the session generation it was submitted under **rises**, in the frame a backend switch or session end is **queued** (EM §10.3, D17 §4), since no ext intent of that generation can be applied after it. Requests and outcomes are numbered per `Session` connection (reset when `Connect` is submitted) and matched by (connection, request number); an outcome of an earlier connection, or one arriving after its slot was released, releases and answers nothing and is dropped. So the slot is never held by a suspended or unloaded script and never carried into the next session | `nil, "busy"` / `nil, "pending"` | D17 §8, EA1b §7.4 | T83 |
| L76 | Bridge | 1 action/s per script; one load outstanding per script, two in all; name rules and 4,096-byte arguments (§7.3) | Refused | D17 §7, §7.3 | T64, T74 |
| L77 | `Server` | 64 KiB per message; 32 messages and 256 KiB/s per script; 64 queued; 1 MiB/s in all | `nil, "busy"` | D17 §8 | T81 |
| L78 | `Input.Drive` | the drivable actions of §7.7 only; Offline only | Lua error | §7.7, D17 §6.5 | T42, T83 |

<a id="s10-6"></a>
### 10.6 Limits stated by the companion specs

These limits are stated with their script- or user-visible form in EM, EA1a or EA1b; they
are listed here so that every number has one home.

| # | Limit | Value | Exceed | Source | Threats |
|---|---|---|---|---|---|
| L79 | Grant store `ext-grants.toml` | ≤ 4 MiB; nesting ≤ 4; ≤ 256 records; `order` ≤ 256 entries; per record ≤ 16 required and 16 optional capabilities, ≤ 32 settings, setting strings ≤ 256 bytes; ≤ 8 `.corrupt` backups | Records ignored (fail closed); oldest backup removed | EM §8.3–§8.6 | T58 |
| L80 | Manifest display strings | `name`, `author`, labels and choice labels 48 cp; reasons and setting descriptions 160 cp; `description` 512 cp and ≤ 8 lines; ≤ 4 bytes per cp of the cap; ≤ 2 marks per base | Manifest refused | EM §4.4, §8.2 | T46 |
| L81 | UI text | `Text` 16 KiB; `Field` `max_bytes` ≤ 4,096; `List` 1,024 rows × 256 bytes; labels, buttons and `placeholder` 256 bytes; `UI.MeasureText` 16 KiB | Lua error | EA1a §7.10 | T24 |
| L82 | Camera | 1 lease for all scripts; a path ≤ 256 key frames and ≤ 300 s; the leased eye ≤ 500 m (3D distance) from the reference point *P*: the player character's interpolated position of the frame when a character is placed, otherwise (before placement, or free fly without a character) the eye of the user's own view of that frame (EA1b §4.6) | `nil, "taken"`; Lua error; eye clamped | EA1b §4 | T43 |
| L83 | Overlay world box | extent ≤ 2,000 m per axis | Lua error | EA1b §5.3 | T29 |
| L84 | Hot reload | one **hot** reload at a time (a full reload is an unload and a load and is not counted, so a full reload waiting on a grant dialog blocks no other reload). A full reload's load (EM phase A, with its reservation) starts only after the old apartment's close item has run **and** the old cap has left L24's sum at its final flush (rule 7.6.5), as a hot reload's item 6 waits for it (EA1b §8.3); so it never adds an apartment (L26's 17 holds) and, with the sum full (16 × 32 MiB), it waits for that flush instead of being refused. This **reads** D17 §8's "at most one reload at a time, so at most 17 apartments exist" and D17 §10 as the limit on hot reloads, the only reloads that hold two apartments, rather than changing an accepted figure (§13 notes the reading by status line; no OPEN is needed). Watcher: quiet period 300 ms; a fixed table of at most 16 dirty flags (one per loaded developer script, L26) instead of D17 §10's event channel, so a folder changing without pause costs at most 16 flags (EA1b §8.2; a design narrowing, §13); developer scripts only | Coalesced | EA1b §8 | T28 |
| L85 | Developer REPL | one submission ≤ 8 KiB; each run is a call under L3–L5 and L23 | Refused | EA1b §9.4 | T10 |
| L86 | Walk file (`scripttool test`) | ≤ 1 MiB; ≤ 4,096 synthetic entities; ≤ 65,536 `[[input]]` entries; ≤ 4,096 `[[expect]]` entries; `frames` 1–216,000; `[screen]` `width` 320–7,680 and `height` 200–4,320 physical pixels (default 1920 × 1080); `[world]` `size` odd, 1–9 landblocks per side; the record-flood hook `[[flood]]` (TM §7.5, X19): ≤ 64 `[[flood]]` entries, `spawn` 1–16,384 flood entities per frame, `life` 1–600 frames, ≤ 65,536 flood entities alive in any frame (checked when the walk file is read); flood entities do not count toward the 4,096 `[[entity]]` entries (registered in revision 6, EM OPEN O-13; `radius` 1–500 m is an argument-validity domain and stays in EM) | Refused | EM §11.5 | T58 |
| L87 | Manifest per-field counts | `[capabilities]` and `[optional_capabilities]` ≤ 16 entries together; `[modules]` and `[optional_modules]` ≤ 32 entries together; `[[settings]]` ≤ 32; `choices` 1–32 per setting; a `text` setting's `max_length` 1–256 bytes (default 64); `version` ≤ 64 bytes, each numeric identifier ≤ 9 digits; `license` ≤ 64 bytes | Manifest refused | EM §4.2, §4.7, §4.8, §4.12 | T58 |
| L88 | Script-folder listing | ≤ 512 entries read per listing of `data/scripts/` or `data/scripts-dev/` (L36's entry cap applied to the listing) | Listing stops; the script manager reports it | EM §3.5 | T54 |
| L89 | Store owner tag `owner` | ≤ 256 bytes (the `author`'s UTF-8 bytes) | Wipe before the open (§8.24) | EM §8.8 | T91 |
| L90 | `Overlay.Lines` per call | ≤ 2,048 lines (12,288 numbers) in one call. Its cost is `B` + e(2*m*) for *m* numbers, since an array of *m* entries counts 2*m* elements (every key and every value counts one, D17 §8, EA1a §2.5), so at most 200 + 20 × 24,576 = **491,720 units**, inside L3 and L4 (restated in revision 5; revision 4's 245,960 counted one element per number) | Lua error before any element is read | EA1b §5.3, §5.5 | T29, T10 |
| L91 | Per-call and per-object caps of the API specs | Command arguments ≤ 1,024 bytes (cut) and `help` ≤ 96 bytes (EA1a §6.1, §6.2); ≤ 16 action edges per action per frame (EA1a §8.2); `Entity.Read` ≤ 512 handles per call, a `near` or `around_self` radius ≤ 500 m, a `box` side ≤ 1,000 m (EA1a §10.2); a `World` ray crosses ≤ 25 landblocks (EA1a §9.1, follows from L69's 500 m); `LocText` ≤ 256 bytes (EA1a §9.1); REPL result description ≤ 32 entries (EA1b §9.4); overlay drop record in the script log ≤ 1 per 10 s (rule 7.1.4, EA1b §5) | Cut, Lua error, or dropped and counted, as the API spec states | EA1a, EA1b | T10, T17, T24 |
| L92 | Store-root scan | ≤ 1,024 entries per `data/script-store/<kind>/` listing; ≤ 2,064 entries per store directory; sizes from no-follow `fstatat`, nothing opened; on the grant-store queue every time the gate enters `Starting` (each session start) and after Reset, Remove and orphan removal | Listing stops; the stores not scanned count as full; the script manager reports it | §8.7 | T54, T55 |
| L93 | Gesture window | the delivering turn, closed earlier by the user's next key, press or wheel step outside the script's windows and actions, or 1 s after the delivering call started (60 frames deterministic); only a press notification of a solid press, an action start or an `/ext:` line opens one; a turn split twice under permanent gating can outlast it and then finds it closed (a known limit that fails safe, §8.13) | Requests treated as outside a gesture window | §8.13 | T36, T88 |
| L94 | Advisory-lock wait on Tessera's own files | a lock on `ext-grants.toml` (on the grant-store queue) or `keymap-ext.toml` held by another process is retried without blocking for at most **1 s** of wall-clock time per write job; the job never waits otherwise (§8.8 "Tessera's own files", EM §8.5) | The write fails: a grant decision stays in the session overlay (EM §8.5); a `keymap-ext.toml` save is reported as failed; the queue's next job runs | §8.8 | T58, T93 |

---

<a id="s11"></a>
## 11. Escape-test catalogue

Every threat of §6 has at least one test. This catalogue is the only escape catalogue
(§0.1): EM §11.8's `escape` runner runs the **ST** cases under these numbers, and the
client's integration tests run the **CI** cases under the same numbers (EM lists them with
its `client` runner). Tests needing a GPU or the reference machine are **RM** and are part
of E6, E8 or E9. A slice ships only when every test **part** that runs in that slice or an
earlier one passes; a test tagged "1a, 2" blocks slice 1a only through its slice-1a part,
and its slice-2 part blocks slice 2. "Fails if" names the observable failure.

A test with parts under several harness codes (for example "ST, CI") passes only when
every part passes. Each runner reports the parts it runs under the X-number and the part
(for example "X62 ST: PASS") and reports the parts it does not run as left to their runner
(EM §11.8's `escape` reports a CI part as `CLIENT` and a U, FZ or RM part as
`ELSEWHERE`), never as passed. Where a slice column names
several slices, each part runs in the slice that ships what it tests (§2 "Slice").

<a id="s11-1"></a>
### 11.1 Sandbox and code

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X1 | T1 | 1a | U (`ext`), ST | Compare an apartment's globals and library tables with D17 §1.2 and §8.15; put a binary chunk in a snapshot; `Store` a string of Lua source and read it back | Any extra name; the chunk loads; any route runs the string |
| X2 | T2 | 1a | U (`ext`) | `getmetatable(handle)`; `setmetatable(handle, …)`; `rawset` on every reachable table looking for a host metatable; collect and close | A table is returned; any field is set; Lua runs in 6c or close |
| X3 | T3 | 1a | U (`ext`) | `tostring` of a table, function, coroutine, userdata; compare with the heap | Any output contains an address |
| X4 | T4 | 1a | U (`ext`) | `error(t)` where `t.__tostring` loops forever; read the log and script manager | The metamethod runs after the call |
| X5 | T5 | 1a | ST | Replace `pairs`, `type`, `tostring`, `string.format` and the string metatable's `__index`, then call every module function that has a Lua shim | Any host behaviour changes |
| X6 | T6 | 1a | U (`ext`, `plugins`) | Globals of an ext apartment against the allow-list (no interop names spelled); `Ext` absent in a plugin apartment | Either fails |
| X7 | T7 | 1a | U (`ext`) | Call every wrapped function through the library and through `("")`'s metatable; compare identities | A stock function is reachable |
| X8 | T8 | 1a | U (`ext`) | The removed functions of §8.15 | Any is non-nil or reachable |
| X9 | T9 | 1a | ST | §8.11's nested `pcall`/coroutine loop | The call runs past its deadline, or more than one abort is counted |

<a id="s11-2"></a>
### 11.2 Time, CPU and memory

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X10 | T10 | 1a | ST, RM (E6) | Endless loop in `Tick`, top level and `Unload`; a stalled tick | Any call exceeds 4 ms + one operation; no throttle or suspension |
| X11 | T11 | 1a | RM (E6) | 1,000 maximal concatenations and comparisons, in the main thread and in coroutines made early and late, via `resume` and `wrap`; comparison also under a UTF-8 collation locale (test build) | Ext time over L12's ceiling for the slice (12 ms in 1a, 11.4 ms from 1b while OPEN O-2 is open, 13 ms after); R changes with the locale |
| X12 | T12 | 1a | ST, RM | §9.1's degenerate arguments | A call exceeds its units; a frame over 50 ms |
| X13 | T13 | 1a | ST (seeded harness), RM | §9.2's flooding set | A chain walk over 32 nodes; a frame over 50 ms |
| X14 | T14 | 1a | U (`ext-host`) | §9.2's host-map flood | Time per operation grows with the key count |
| X15 | T15 | 1a | ST, U (`script`) | §9.6: recursion to the bound through `gsub` callbacks, `sort` comparators, metamethods, `pcall` chains, 1,000 nested `resume`/`wrap`, deep source nesting | A crash instead of a catchable Lua error; the counter restarts at a resume |
| X16 | T16 | 1a, 1b (the reload apartment) | RM (E6) | Heaps built to maximise S, an interning rehash, worst K file, worst C; the two backend-switch cases of D17 E6, in 1a with 16 apartments and from 1b with a hot reload's new apartment as the 17th | Either branch of L12 over the slice's ceiling (12 ms in 1a, 11.4 ms from 1b while OPEN O-2 is open, 13 ms after); K or C over L15 or L16 for the slice (9 ms in 1a, 8.7 ms from 1b while O-2 is open); any of these without the fallback applied |
| X17 | T17 | 1a | ST, RM | 16 scripts each using its full call every frame; one alternating heavy and light; one whose own calls fill the 2 ms cap every frame (a 1,024-entry delivery set) | Ext time over the cap plus one call; no throttle; others starved beyond round robin; a split turn resumes first in its group more than once (§8.13) |
| X18 | T18 | 1a | ST | A script aborting 3 times in 10 s, every 10 s | Not suspended; notice not hit-transparent |
| X19 | T19 | 1a | ST (record-flood hook `[[flood]]` within L86's bounds, §7.5), RM | §7.5's 10,000-entity flood for 10 s (`{ from = 60, to = 659, spawn = 10000 }`: 10,000 spawns and 10,000 despawns per frame, 10,000 alive, inside L86's 16,384 per frame and 65,536 alive), with 4 scripts × 16 subscriptions and with 16 scripts × 4 (64 in all, L30); a subscription created while its cursor lags | Filter work outside a call; frame rule broken; a record skipped by a cursor within 60 frames; drops uncounted; a script throttled, suspended or charged an abort by filter calls alone (rule 7.5.2a); a new subscription matches an older record |
| X20 | T20 | 1a, 1b (hand-off) | ST, RM | Maximal `Store` value read back; maximal hand-off (1b, with hot reload); shared-subtable graph; 10^5-deep chain; cycle; `__index`/`__len` metamethods; NaN key | Over-cap value accepted; a metamethod runs; a panic |
| X21 | T21 | 1a | CI | Unload 16 scripts in one frame; count recompilations; E8 timing | More than one recompilation; E8 over 2 ms |
| X22 | T22 | 1a, 2 (`Files.Pick`) | CI | Change the locale in a test build; run an ext load (1a) and a `Files.Pick` dialog return (2) | Ext not suspended; `Files` still served after the dialog returns (2) |
| X23 | T23 | 1a | ST | Heap growth to the cap; a 5 MiB string | No catchable error; host memory over the cap |
| X24 | T24 | 1a | ST | Elements, labels, timers, subscriptions, notifications, intents and `/ext:` commands (the 33rd, L68) at and past their caps | Host memory outside the cap; a count cap passed |
| X25 | T25 | 1a, 2 (`Data`) | ST | 33 small `Store` reads in one frame; 33 `Store.Set` calls with a `done` callback on one key before its flush (L27); reads of 1 MiB values; the hostile-file reads of §8.14 (a key file replaced or grown after indexing, 8,192 empty tables, 1-byte strings) (1a); `Data` lookups of new DIDs at the cap for 10 s (2, the `Data` module) | 33rd read not `busy`; a 33rd pending `done` accepted; worker queue grows; `out of memory` raised instead of returned; a read past the indexed size; a delivered value over its reservation |
| X26 | T26 | 1a | ST | §8.5's image create/destroy loop, with distinct files and with one file for every element (joined requests) | Outstanding over 32; queue grows; a destroyed element's decode slot freed before its job ends; a joined request not counted by itself (a 33rd creation accepted) |
| X27 | T27 | 1a, 1b (hot reload) | ST | §8.10's snapshot-charge check and E5 peak report (1a); the hot reload of a `memory_mib = 64` script with 8 MiB of data files and a 1 MiB hand-off (1b) | Data files not charged; retained store over 15 MiB; the reload outside the allowance or the sum (1b) |
| X28 | T28 | 1a, 1b (reload) | ST | 16 scripts at 32 MiB; 8 at 64 MiB; a 17th load (1a); each case plus a hot reload (1b) | Sum exceeded; the 17th loads |

<a id="s11-3"></a>
### 11.3 GPU and UI resources

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X29 | T29 | 1b | CI (counting), RM (E6, E9) | 20,000 full-screen screen quads shown in one frame by two scripts (each list built over as many calls and frames as the add costs need, then shown) | More than 1.0 screen encoded; drops not reported; E9 over 1.0 ms; a frame over 50 ms in the walking run |
| X30 | T29 | 1b | CI, RM | 10,000 world boxes, filled and as edges, straddling the near plane, around the camera, behind it and with the eye inside; then 500 outline waypoint boxes after one straddling filled box; screen lines of width 4 whose centre line lies 1 px outside each viewport edge | A filled box straddling the near plane drawn; a primitive outside the frustum counted; the waypoints dropped because of the straddling box; coverage over bound; a wide line near the edge counted below the pixels it paints |
| X31 | T30 | 1a (windows, labels, badges), the slice that adds nested surfaces, scale or rotation for those parts | CI, RM (E9) | 16 scripts each showing full-screen windows and world labels; windows created mostly off screen, then moved on screen by the script and dragged by the user, also by fractional pixels at UI scale 1.25; a user resize at the bound; a script whose coverage is labels only; then a screen resize and a UI scale change. In the slice that adds them: nested surfaces, scale and rotation changes (EA1a §7.10 has none in 1a) | A show, size, scale or rotation change past 1.25/2.0 screens succeeds; a moved window or label raises coverage (whole or fractional pixels); a user resize passes the bound; a host change leaves a script or the total over its bound; labels not hidden; hidden entries not in reverse order of last show or not notified with `VisibilityChanged{by = "host"}` |
| X32 | T31 | 1a | CI | Ext at every pool cap while a plugin creates windows, images and registrations | Any plugin call fails that succeeds with no ext script |
| X33 | T32 | 1a | U (`ext-host`) | Images with 65,535 px axes, DDS sizes overflowing 32 bits, truncated headers | Decoded before refusal; a panic |
| X95 | T94 | 1a (windows, labels, badges), the slice that adds nested surfaces for that part | CI (golden image, hit test) | §8.26: a plugin window at its lowest `ZOrder` against an ext window at its highest, after `Activate`, a solid press, re-parenting and (later slice) a nested surface; an ext show during a prompt; the same with a native panel | The plugin window's or panel's pixels differ from a run without ext; a click or hover on its area reaches an ext element |

<a id="s11-4"></a>
### 11.4 Input and focus

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X34 | T33 | 1a | U (`input`), CI | Ext `Shift+W`, `Shift+Space`; suggested defaults on built-in strokes | Walking or jumping changes; default not withheld |
| X35 | T34 | 1a | U (`input`) | Exact ext `Shift+MouseLeft` `on = "tap"` | `tap_left` blocked |
| X36 | T35 | 1a | CI | Ext bindings on K and F5 with chat input or a plugin `TextBox` focused | The ext action fires |
| X37 | T36 | 1a | CI | D17 exit criterion 2's focus list; plus a solid press, then a timer `Focus` on a text box in a later turn | Any request returns `true` outside a gesture turn; `W` stops moving |
| X38 | T37 | 1a | CI | §7.2: transparent full-screen element (also opacity 0.49, image alpha 0), world click, timer `Focus`; an opaque non-catching image of the script over a catching element, pressed; repeated non-solid presses on a full-screen catching frame of one script over another script's window | `Focus` returns `true`; the window becomes active; `W` is swallowed; a press through a non-catching picture counts as solid; a non-solid press raises its script in the ext z-order |
| X39 | T38 | 1a | CI | Box under the chat window, under an opaque plugin window, hidden, zero-size, off screen; under the script's own later sibling and its own higher window; a plugin window moved over a focused box; a badge over the box's focus area (another window's of the same script, another script's, and its own window's inside badge, §8.3), and a window moved so that a badge comes over a focused box | Focus granted; focus kept past the next key |
| X40 | T39 | 1a | U (`input`) | Toggle an ext context every frame during `Ctrl+K Ctrl+C` | The sequence is cancelled |
| X41 | T40 | 1a | U (`input`), ST | §8.16 (also overlapping suggested defaults: `A+B` with `B`, and subset-modifier axis pairs, in one context) | Built-in movement holds fail; an overlapping pair of suggested defaults accepted |
| X42 | T41 | 1a | U (`input`), CI | Chord with a focused text box, an imported `F12` binding, a plugin `KeyDown`, a pending sequence | Chord not consumed first; any other receiver sees it |
| X43 | T42 | 2 | ST, CI | §7.7 | An excluded action is accepted; a file appears; a plugin handler runs outside steps 1 and 7; a UI-band entry is pushed |
| X44 | T43 | 1b, 2 (`Files.Pick`) | CI | Lease held, then: script stalls; window loses focus; `Files.Pick` opens (2); suspension by the watchdog, and by a third abort in 6b (after step 3); grant revocation; a backend switch queued; a user unload and `Core.Unload`; chord. `Camera.Take` and a `Sandbox` command inside an `Unload` handler, during a user unload and during a hot reload whose new apartment then calls `Take`. Under a `Hold`, the user clicks an entity visible under the pointer | Lease survives longer than 350 ms after the last renewal; a lease not ended at its event, or the user's view and control not back by step 3 of that frame (of the next frame for an event after step 3); a lease or intent from an `Unload` handler takes effect, or the new apartment's `Take` answers "taken"; the click picks anything but what is under the pointer on the presented frame |
| X45 | T44 | 1a | CI | §7.8 | A label takes a click or hover, or a notification |
| X46 | T45 | 1a, 2 (`Input.Drive`) | CI | Every ext call attempted while a prompt shows (1a); `Input.Drive` attempted while a prompt shows (2) | "Allow" is pressed |

<a id="s11-5"></a>
### 11.5 Spoofing and output

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X47 | T46 | 1a | ST, U (`ui`), CI (golden image) | §8.2 parts (a)–(c) | A hostile manifest string loads or is shown; removed characters drawn; more than 2 marks; text outside its box |
| X48 | T47 | 1a | CI | §8.25: pointer resting on "Allow" at show; pointer entering "Allow" at 100 ms and pressing at 600 ms; clicks during 500 ms; Enter and Space; rapid clicking before the prompt; the same on a capability box and "Load automatically"; a script drawing under the dialog | Any of these allows or ticks a box; ext windows visible while the dialog shows |
| X49 | T48 | 1a | CI (golden image) | §8.3 (with the badge under a native panel and under a plugin window, a window at y = 5 and one partly off screen; a badge over a box's focus area; a focused box at its window's top edge with the badge attached outside) | Badge missing, restyled, covered by an ext window or not wholly on screen; focus enters a box, or stays in it, while its badge is covered or off screen, or while a badge other than its window's outside badge overlaps its focus area; a badge drawn over a caret or focus mark; counted badge area depends on the font |
| X50 | T49 | 1a (notices), 1b (lease indicator) | CI | A suspension notice over a plugin button (1a, D17 M5.5 exit criterion 2); the lease indicator over a plugin button (1b) | Click or hover on the button blocked |
| X51 | T50 | 1a | U (`ext-host`) | §8.1 | Control bytes in the file; one `print` makes two records |
| X52 | T51 | 1a | CI | §8.4 (changed text; text clipped, scrolled out, transparent, under a plugin window; Cc and Cf characters) | Changed, script-selected, undisplayed or faint text reaches the clipboard; a Cc or Cf character (other than a displayed LF or tab) is copied |

<a id="s11-6"></a>
### 11.6 Files and disks

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X53 | T52 | 1a | U (`ext-host`), ST | Ids and keys `con`, `nul`, `com1`, `lpt1`; a folder segment `aux.lua`; `Ext.Load("..x")`; segments with `\` or `:` | Any accepted (keys answer `nil, "reserved key"`); a file with a bare script-chosen name created |
| X54 | T53 | 1a | U (`ext-host`) | Symlink, hard link (link count 2), FIFO, device node, swap of a file for a link mid-walk, a file growing mid-read, a symlinked script folder (§8.8) | Script loads; a worker blocks; a special file is opened |
| X55 | T54 | 1a | U (`ext-host`) | 10^6 empty directories; one directory of 10^6 entries | More than 512 entries read |
| X56 | T55, T56 | 1a | ST | §8.7 (keys, sizes, temp files, orphan totals, the store-root scan cap); the log flood of one script and of 16 scripts | Caps passed; temp files left after open; the totals below the orphans' counted sizes; more than 1,024 entries of a `<kind>` listing read; a script's log files over 2 MiB on disk or its rate passed; all logs together over 128 KiB × (*t* + 1) in *t* s; drops not recorded |
| X57 | T56 | 1a | ST | §8.7 | Counted bytes over the bucket bound |
| X58 | T57 | 2 | CI | §8.6 | A worker blocks; a special file is read |
| X59 | T58 | 1a | FZ, U | Fuzz the manifest, grant store, `Store` reader (via the neutral reader), `keymap-ext.toml` reader; slice 2 adds bridge reads | A panic, hang or over-cap allocation |
| X60 | T59 | 1a | CI | Kill the process during flushes (test hook) at every step | A half-written value; values without the local-only mark |
| X61 | T60 | 1a | U (`settings`) | §8.19 | A save lands in the install |
| X94 | T93 | 1a | U (`ext-host`), ST | §8.8 store and log rules: `s_<id>` as a symlink and a junction to a directory outside holding a canary file, then a load that needs a wipe (an `author` change); inside `s_<id>`: a key file that is a symlink to a canary outside, a hard link to a canary outside (link count 2), a FIFO, a subdirectory, an unknown regular file, `owner` and `local-only` as symlinks; a log file and a log `<kind>` directory as symlinks; a log file and a rotated log file that are FIFOs or device nodes; the grant store as a FIFO and as a link to `/dev/zero`; the grant store's lock held by another process for 5 s (U: the write fails after at most 1 s, L94, and the queue's next job runs); each entry swapped for a link between the listing and the open | A canary outside is read, written, truncated or removed; a link followed; a worker or the log writer blocks on a FIFO; a device node opened; the grant-store queue waits; an unexpected entry listed into the file index or opened; a wipe enters a subdirectory or removes anything outside `s_<id>`; the load not refused when item 3 or 5 of §8.8 says so |

<a id="s11-7"></a>
### 11.7 Exfiltration and isolation

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X62 | T61 | 1a (stand-in), 2 | U (`ext`), CI, ST | Manifests requiring `session.server` with `data.read` or `bridge.plugins`; optional mixes in the grant dialog; a hand-edited grant store (seeded by the harness, EM §11.4) granting both, with duplicate records and a newer `version` | Both granted; a record that breaks EM §8.3 (granting both, or a duplicate) honoured instead of ignored; a newer document overwritten |
| X63 | T62 | 1a (stand-in), 2 | CI | D17 §6.6's I/O-queue test: one write pending, one in flight, grant switched | The server holder reads a key; a file appears after the wipe |
| X64 | T63 | 1b | ST | Hot reload changing the capability list | A hand-off table is passed |
| X65 | T64 | 2 | CI | §7.3 | Any built-in runs; anything reaches the backend; a swapped registration runs |
| X66 | T65 | 1a | CI | `/ext:` lines for known and unknown ids on a backend stub that records submissions | Any submission |
| X67 | T66 | 1a | U (`ext`) | Errors, tracebacks and chunk names from a script under a home path | An absolute path appears |
| X68 | T67 | 2 | ST | §8.9 | B sees "loaded" before its own request completes |
| X69 | T68 | 1a | ST | Script B tries every API to reach A's `Store`, windows, generator, local entities | Any access |
| X70 | T69 | 1a | CI | Pointer sweep over a lower window's transparent background and over its elements | Notifications over non-catching areas; more than one move per frame |

<a id="s11-8"></a>
### 11.8 Plugins, backends and liveness

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X71 | T70 | 1a, 2 | CI (instrumented plugin) | Ext `Activate`/`Focus` while a plugin window is active; a prompt raised in 6a; driving (slice 2) | Plugin code runs in ext time or outside steps 1 and 7 (except the inherited 0014 item, §8.20) |
| X72 | T71 | 1a | CI | Ext commands registered; plugin `IsCommand`, `GetCommands`, `AddCommand` before and after | Any answer differs |
| X73 | T72 | 1a | CI | `error("x")`, `/ext:list`, `/ext:help`, `/EXT:Unknown` | A chat line or `Received` event |
| X74 | T73 | 1a | U (`ext`, `plugins`) | §9.5 | Any sequence changes across the boundary |
| X75 | T74 | 2 | CI | Bridge actions every frame from 16 scripts | More than the rate; more than two loads queued |
| X76 | T75 | 2 | CI | Bridge read of a 16 MiB, 10^6-entry plugin-data file | Parsed past the cap; a panic |
| X77 | T76 | 1a | CI | A plugin `Alias` with `/ext:notes` and markup, bidi and 64 KiB arguments; a user click | Anything but a command notification with the arguments as data |
| X78 | T77 | 1a | CI | Stub backend `Other`; test-only `TesseraProtocol` session | Any ext apartment exists |
| X79 | T78 | 1a | U (`net`) | Compile-fail test for the sealed trait; a forwarding wrapper in another crate | Class not `Other`; an apartment loads |
| X80 | T79 | 1a, 1b | CI | Intent queued in 6b before a switch; after suspension; under a newer generation; `Sandbox` teleport before a switch to `Other`; a teleport outstanding when its script is suspended, then another script's teleport after the sandbox answers; a teleport outstanding when a switch to a new `Offline` session is queued, whose outcome never arrives, then a teleport in the new session; a late outcome of the old connection; a teleport whose submission fails | Any intent applied; the teleport slot still held after the outcome arrived, after the frame its switch was queued, or after its submission failed; a late outcome releases or answers anything; request numbers not reset at `Connect` or consumed by a failed submission |
| X81 | T80 | 1a | U (`tools/`) | D17 §6.6's synthetic manifest sets | Check passes a bad graph |
| X82 | T81 | 3 | ST (loopback), FZ | Messages over 64 KiB, deep, NaN keys, floods | Accepted; panic; rate passed |
| X83 | T82 | 3 | CI | Policy without `entity.read` | `Entity` available |
| X84 | T83 | 1b, 2 | CI | `Input.Drive`, `Sandbox`, `Data` on a `TesseraProtocol` stub with a permissive policy | Any is available |
| X85 | T84 | 1a, 1b | ST, CI | §7.4 with the "every frame is heavy" hook (deterministic): a queued line and a switch with 16 apartments (1a); a switch with a hot reload in progress (17 apartments) and a camera lease held when it is queued (1b) | A line waits over 16 frames; a switch takes over 376 frames (1a) or 392 frames (1b); an `Unload` starts after frame 120 of the switch; a lease survives the frame the switch was queued |
| X86 | T85 | 1a | CI | §8.12 (15 ms and 18 ms after redraw start, 5 ms of callbacks, 30 ms idle) | Idle time counted; the 20 ms frame gated or the 23 ms frame not gated |
| X87 | T86 | 1a, 1b (hot reload) | CI, ST | Switch queued with one load at a grant dialog, one waiting on its grant-record write, one at its store probe, one at its store open, one at snapshot, one mid-compile, one before top level (1a), and a hot reload in progress (1b) | A compile or top level runs; a state leaks; count over 17; a store open, probe or record write left half-done; a dialog writes a decision |
| X88 | T87 | 1a, 1b | ST | §7.6 (hot reload part in 1b) | A stale value returned |
| X89 | T88 | 1a | ST | §8.13 (split turn, also under the "every frame is heavy" hook; a `Focus` after the user's `W`, or after 60 frames; a held solid press; the twice-split turn behind four cap-filling scripts under that hook) | Focus not granted in a turn split over two frames; granted after the window closed (L93), the twice-split case included; a release or click opens a window |
| X90 | T89 | 1a | CI, ST | D17 exit criterion 5's content-change and second-copy cases (the ST part answers each dialog as EM §11.4's harness policy says and expects it in the trace); two loads of one id from both kinds started in the same frame (two `autoload = true` records at world entry with developer mode on); developer mode turned off (EM `developer_mode = false` walk event) while one developer load shows its grant dialog, one is in phase C and one waits in `Starting`, and a user unload of a loading copy (§8.23) | Group A kept on change outside dev mode; second copy loads; both loads of one id pass; a developer load finishes, or its dialog writes a decision, after developer mode is off; a waiting developer load not refused with "Developer mode is off"; an unloaded loading copy reaches its top level |

<a id="s11-9"></a>
### 11.9 Grants, authors and the keymap file

| Test | Threats | Slice | Harness | Procedure | Fails if |
|---|---|---|---|---|---|
| X91 | T90 | 1a, 1b | ST, U (`ext-host`) | §8.23: revoke optional and required grants (manager test hook), with a lease, intents and a `Follow` active (1b); a re-read of a hand-edited grant store that removes, invalidates, narrows and widens loaded scripts' records; a revoke-resume-widen of optional `camera` (1b); a kept entity handle passed to `lease:Follow` after an `entity.read` revocation and resume (1b); a failed-write revocation followed by a re-read; a newer-`version` document re-read; a re-read of a document damaged as a whole, then a write that replaces it; a re-read whose open fails (U); a manager revoke and widen of one grant in one frame under `--latency 2`, with an `[[expect]]` of granted; a Reset, then a first-enable Allow and load before the Reset's write completes; the failed load-write rollback cases of §8.23 (a revocation whose write fails, then a failed load write of the same key; a failed Reset write, then a failed first-enable Allow); a required grant revoked while a load is past its dialog and before its top level; a suspended script's ext context | Not suspended in that frame (a narrowing re-read included); a widening applied before the next load (the live table read instead of the effective set included); an intent applied after it; a lease survives the frame; a revoked module, or a handle it minted, answers anything but `"not granted"` after resume; a required revocation not unloaded; a revoked grant back after a re-read; a newer, damaged or unreadable document, or the replacement of a damaged one, suspends anything; an older write's completion narrows a newer decision of the same key (the widen lost, the new load cancelled or suspended); a failed load write taken back restores anything but the entry it superseded (a revoked grant granted again, a reset record back); the load's top level runs; a context or hold of a suspended script still active |
| X92 | T91 | 1a, 1b | ST | §8.24: `author`-only hot reload (1b); stores with a foreign, missing or 257-byte owner tag; an invalid new manifest at a hot reload (1b); a full reload across an `author` change, with its dialog answered per EM §11.4 (1b) | A hand-off delivered or a key readable across the `author` change; a foreign store opened without a wipe; the old apartment stopped by an invalid manifest |
| X93 | T92 | 1a | U (`input`, `settings`) | §8.19 | The user's keymap file written because of ext; a built-in rebinding lost or read back loosely |

<a id="s11-10"></a>
### 11.10 Earlier case IDs of EM §11.8

EM's first version named its `escape` cases with its own IDs. They are retired in favour
of the X-numbers; each maps as below, and each case's expected outcome is the "Fails if"
column of the X-tests named (EM's procedures are folded into them).

| EM ID | X-tests | EM ID | X-tests |
|---|---|---|---|
| X-FS-1 | X53 | X-STORE-4 | X56, X57 |
| X-FS-2 | X54 | X-GRANT-1 | X91 |
| X-FS-3 | X54 (§8.8) | X-GRANT-2 | X90 |
| X-FS-4 | X54 | X-GRANT-3 | X62, X59 |
| X-FS-5 | X55 | X-GATE-1 | X78 |
| X-LUA-1 | X1, X6 | X-GATE-2 | X87, X80 |
| X-LUA-2 | X1 | X-GATE-3 | X85 |
| X-LUA-3 | X3 | X-SUB-1 | X19 |
| X-LUA-4 | X2, X4 | X-INPUT-1 | X34, X35 |
| X-LUA-5 | X7, X12 | X-INPUT-2 | X40 |
| X-LUA-6 | X74 | X-INPUT-3 | X93 |
| X-CPU-1 | X10, X11, X18 | X-INPUT-4 | X43 |
| X-CPU-2 | X13 | X-UI-1 | X39 |
| X-MEM-1 | X24 | X-UI-2 | X38 |
| X-VAL-1 | X20, X59 | X-UI-3 | X37 |
| X-STORE-1 | X25 | X-UI-4 | X45 |
| X-STORE-2 | X88 | X-UI-5 | X29, X30, X31 |
| X-STORE-3 | X63 | X-PROMPT-1 | X48 |
| X-CHAT-1 | X73 | X-PROMPT-2 | X47 |
| X-BRIDGE-1 | X65 | X-DET-1 | none: a determinism check of `test` (two runs, equal traces), not an escape case; it stays an ordinary EM `test` check |

---

<a id="s12"></a>
## 12. Invariants

1. No ext code runs except the hashed snapshot's compiled files (and, in developer mode,
   the REPL bound to a `scripts-dev` apartment).
2. No ext Lua runs outside a metered call in 6a or 6b; 6c and state closes run no Lua.
3. Every native's cost is charged or reserved before it works; every value crossing TB1
   is bounded during its walk.
4. Ext Lua time per frame stays within the ext ceiling of L12 (13 ms, D17 §8; while
   OPEN O-2 is open, 12 ms in slice 1a and 11.4 ms from slice 1b) in every frame, on both
   of L12's branches (7 ms + R + 2·S, and 2 ms + K + S or 2 ms + C + S), floor frames
   included; ext GPU time ≤ 1.0 ms at the coverage bounds (E9).
5. Coverage drawn per frame never exceeds 0.5 screen per script and 1.0 screen in all
   for overlays, and 1.25 and 2.0 screens for composition, counted conservatively
   (§7.1.2); no movement of a window or label, by whole or fractional pixels, raises
   composition coverage.
6. No ext binding consumes a press or release that a non-ext context would consume or
   that an `opaque`/`opaque_keys` stop swallows.
7. Keyboard focus enters an ext text box only by the user's own **solid** press on that
   box, or in a gesture window of that script, and only while the box is visible and
   uncovered (also by the script's own content, and by every badge overlapping its focus
   area except its own window's badge attached outside the window) and its window's badge
   is wholly on screen and uncovered.
8. A press that is not solid never activates an ext window and never opens a gesture
   window.
9. The panic chord is resolved before anything else while ext is present, and after it no
   ext code runs until the user resumes.
10. No ext call, item or bridge action reaches a backend except a `session.server`
    holder's `Server` messages and, offline, `Sandbox` commands and the movement that
    `Input.Drive` latches, which the sandbox backend receives as the player's ordinary
    movement commands.
11. The bridge never tokenizes, substitutes or looks up a command line, and never runs a
    built-in.
12. No script holds a local-only capability together with `session.server`; no value
    written under a local-only capability is readable by a `session.server` holder.
13. A `Store.Get` returns the latest value written before it in program order (absent
    after a `Delete`), across a hot reload too; a read answered from the pending-write
    map is counted, rate-limited and delivered exactly like a read from disk.
14. Ext file output is only the script's `Store` and its log, inside Tessera's
    directories; `Input.Drive` cannot cause any other file.
15. Every host file open under `data/` and `logs/` is relative to a directory handle and
    never follows a link at a script-influenced name (the script folder, the store
    directory and every entry in it, the script log files); such an entry is used only
    when it is a regular file with one link, and a wipe removes only entries of the store
    directory itself, never anything a link points to (§8.8).
16. The script log contains no control byte other than tab and LF, and one record per
    line, each with the host prefix.
17. No reserved-layer string that is not Tessera's carries a removed character or more
    than two marks per base, or draws outside its box.
18. A script never observes another script's `Store`, apartment, generator, local
    entities, decoded-cache view or window content.
19. World labels and script-caused reserved-layer surfaces never catch the pointer.
20. Under permanent gating, `/ext:` lines are served within 250 ms plus one frame plus
    round robin, and backend switches complete within the bound of L11.
21. Plugins observe ext only through D17 §7's listed channels.
22. Ext apartments never exist on an `Other` session, nor on `TesseraProtocol` before
    slice 3.
23. A revoked grant is never used again: the script is suspended in the frame of the
    revocation, and every later call of the revoked module raises `"not granted"`.
24. No `Store` value and no hand-off table reaches a script whose `author` differs from
    the one it was written under.
25. Every lease ends at the event that ends it, and the user's view and control are back
    by step 3 of that frame, or of the next frame for an event after step 3 (§8.17); no
    lease or intent comes from an `Unload` handler; the user's pointer always picks from
    the presented frame.
26. No ext window of any script draws over an ownership badge, and no badge and no content
    of any script draws over the caret or focus mark of a focused ext text box (§8.3).
27. No native stack overflow is reachable from ext code; deep recursion through C ends
    in a catchable Lua error.
28. Every value a `Store` flush writes was paid in full in the write buckets at the call
    that made its key dirty or grew it (§8.7), so the counted bytes written over any
    interval never exceed the buckets' capacity plus their refill.
29. No ext composition entry is drawn or hit-tested above a plugin window, a native panel
    or the reserved layer, whatever its `ZOrder`, activation, parent or nesting (§8.26).
30. A copy from an ext text box puts on the clipboard only text the user selected, that
    did not change since, that was wholly displayed at opacity ≥ 0.5 from the selection to
    the copy, and that holds no Cc, Cf, Zl or Zp character other than a displayed LF or
    tab (§8.4).
31. Every native and every intent apply checks the apartment's effective set, which only
    shrinks during the apartment's life; a handle is usable only while the capability of
    the module that minted it is in that set (§8.23).
32. A `Store` or bridge read never reads past the indexed size plus one byte and never
    delivers a value larger than the reservation charged at the call (§8.14).
33. All script logs together write at most 128 KiB per second plus one second's burst
    (§8.1, L45).
34. A gesture window never outlasts the user's next input elsewhere or 1 s (60 frames in
    deterministic mode) (§8.13, L93).
35. Only the user narrows grants: a revocation, Reset, Remove or local-only switch, or a
    hand edit between two documents Tessera read and understood. An unusable re-read
    (newer, damaged as a whole, or unreadable), Tessera's replacement of a damaged
    document and the completion of an earlier write never narrow a grant or suspend a
    script; a failed write never undoes an earlier session decision (a failed load write
    taken back restores the overlay entry it superseded); and no developer load finishes
    while developer mode is off (§8.23).
36. Every result delivery that is never dropped (L30) belongs to a request counted under
    L27 (a `Store.Set` or `Store.Delete` `done` included) or to a due timer, so the
    reserved room of the notification queue is bounded by 32 results and 64 timers per
    script.

---

<a id="s13"></a>
## 13. Changes other documents need (for the accepting session)

This spec edits no other file. The session that accepts it, and the authors of the
companion specs, make these changes. Rows marked **(companion)** bring EM, EA1a or EA1b in
line with this spec; in each, the companion's own rule was weaker or conflicting, and
this spec keeps the stronger one.

| Document | Change |
|---|---|
| D17 (status line only; accepted records are not rewritten) | "Open security and design items resolved by `ext-threat-model.md` §7; other improvements by §8." |
| D17 §8 frame-budget table (by status line and in ARCH §18.3) | New row "Ext overlays and composition: 1.6 ms" (0.6 ms encoding, E4; 1.0 ms GPU, E9); margin 2.6 ms; the M5 non-paint-UI condition at 0.6 ms (§7.1.6; OPEN O-2); until the owner decides, the interim ceilings of L12 |
| D17 §8 limits table | Overlay and composition coverage (L53–L55); floor and switch timeout (L9–L11); entity record cap and filter call (L25, L31); `Store` byte bucket (L44, OPEN O-3); ext holds pool (L61); reservation rule (L28); C-call bound (L21); compatibility functions removed (§8.15) |
| D17 §9 | New E9 (§7.1.6); E6 gains X29, X30, X19's flood, the floor-frame count and §9.6's stack measurement; E8 gains the coalesced recompilation; E5 gains the reader's expansion ratio (§8.14) |
| The API specs (EA1a, EA1b, slice 2's) | Carry §7.2, §7.3 (slice 2), §7.5, §7.6, §7.7 (slice 2), §7.8, §8.3, §8.4, §8.5, §8.13, §8.15, §8.17 and §8.23 as their own rules, in their own words, citing the TM sections |
| EM header, Related row, every "`ext-api.md` defines/decides" **(companion)** | Cite TM, EA1a and EA1b by their real names and sections; cite limits as TM L-numbers (§0.1) |
| EM §3.1 **(companion)** | `keymap-ext.toml` lives in the directory of the configured keymap path (§8.19), not `<config>` |
| EM §3.4, OPEN O-4 **(companion)** | Retained snapshot data files are charged to the script's cap; the whole snapshot is held under the load's reservation, counted in the 512 MiB sum; no separate 136 MiB bound (§8.10). O-4 closed |
| EM §4.4, OPEN O-9 **(companion)** | Parse-time refusal kept (the stricter rule); its mark rule aligned to at most 2 marks of category M per base and no mark without a base, and Cn and noncharacters refused too (§8.2, L80); X-PROMPT-2 becomes X47 part (a). O-9 closed |
| EM §9.1, §9.3 "Controls" **(companion)** | Pointer entry observed after the 500 ms delay ended, for Allow, the boxes and "Load automatically" (§8.25) |
| EM §9.2, §8.4, §8.7 **(companion)** | A store found without a valid record is kept only when its owner tag matches the loading script's `author`; otherwise it is wiped before the open (§8.24). Remove deletes the store before the record. The store directory gains the host's owner tag, which EM §11.8's `no_file_outside` set allows |
| EM §9.7 **(companion)** | Revocation of any grant, optional or required, suspends in that frame (§8.23); X-GRANT-1 becomes X91 |
| EM §9.8 **(companion)** | The declarations whose change forces a full reload include `author`, `api`, `manifest`, `[modules]`, `[optional_modules]` and `[server]`; the new manifest is fully validated first (§8.24) |
| EM §10.3, §10.4, invariant 18, OPEN O-5 **(companion)** | Cite §7.4 and L9–L11; add the switch-timeout row (close without `Unload` after 2 s or 120 frames); phase C store work completes and only its result is discarded (§8.21); the 34-frame bound holds only without gating. O-5 closed |
| EM §11.5 **(companion)** | Walk-file keys for the virtual surface size (default 1920 × 1080, §2 "Screen"), a reload from a second folder version and a simulated loss of window focus (requested by EA1b §14). **Done** in EM revision 2 (`[screen]`, `reload`, `focus`); EA1b §6.1, §8.8 and §14 mark their requests done |
| EM §3.1, §8.8, §12.8 **(companion)** | The no-follow and entry-kind rule of §8.8 for `s_<id>/`, its entries and the log files (items 1–5: handle-relative opens, `O_NOFOLLOW` with the kind checked on the handle, exclusive creates, a non-plain store directory fails closed, unexpected entries never opened and removed as entries, a subdirectory fails closed, the wipe never follows or recurses); a named temporary-file pattern that no key file or host file matches, so "temporary files left by a crash", the file-index listing and `no_file_outside` are exact. Escape case X94 (T93) |
| EM §3.5, §4 preamble, §4.2, §4.7, §4.8, §4.12, §8.8, §11.5, OPEN O-11 **(companion)** | Cite the new and restated rows: L86 (walk file: inputs, expectations, frames, `[screen]` and `size` ranges), L87 (manifest per-field counts), L88 (folder listing, 512), L89 (owner tag, 256 bytes). §0.1 rule 6 (revision 4) answers the rest of O-11: settings `min`/`max` ±1e15 and the grant-record integer bound 2^53 are argument-validity domains, and setting keys, capability names, module-name segments, version parts, action and context name lengths and the walk file's `[[entity]] name` are grammar lengths, all staying in EM; the `--grants`/`grants` copy bound of 4 MiB + 1 belongs to L79. The §4 preamble claims an L-number only for what rule 6 registers; O-11 closed |
| EM §8.4, §8.5, invariants 9 and 13 **(companion)** | The re-read rule of §8.23: a re-read that removes, invalidates or narrows a loaded script's record suspends it in that frame; a widening is honoured only at the next load; a record that breaks §8.3 grants nothing. Invariant 9 reads "Tessera never grants a capability except by an Allow (or the `test` tool root)"; invariant 13's "hand-widened record" means a record that breaks §8.3, or the word is dropped |
| EM §10.4 **(companion)** | A row for a load past its dialog that waits on its grant-record write or its store probe when `Draining` begins: the write or probe completes, then the load is dropped (§8.21) |
| EM §11.4–§11.6 **(companion)** | The harness's dialog policy at every load that §9.2 would ask at (first load, `reload` with a changed hash, full reload, `manager` `load` after a required revocation, a widen): an answer per dialog from the walk file, or a stated default; a `dialog` trace event (kind, asked set, answer) and expectation; a grant-store seed option for the tool root (hand-edited, duplicate and newer-`version` documents), so the ST parts of X62, X90 and X92 can be built |
| EM §11.8 **(companion)** | Specifies only the `escape` runner; its cases are TM's X-numbers (§11.10 maps its earlier IDs); expected outcomes are TM's "Fails if"; a case with ST and CI parts reports each part, the CI part as `CLIENT`, never the whole case as PASS (§11); `no_file_outside` allows every lock and temporary file that the grant-store and `keymap-ext.toml` saves create, named exactly, and X94's canaries |
| EA1a header and §0 **(companion)** | Cite TM as normative for §7–§9; say that EA1a and EA1b together are D17's `ext-api.md` for slices 1a and 1b |
| EA1a §2.1, §2.6, §7.7 **(companion)** | Gesture window = the whole delivering turn (§8.13), not one call |
| EA1a §2.6, OPEN-9 **(companion)** | Carry the floor frame (with its reset of both conditions, counted from the start of the last frame in which ext ran, 6c's 1 ms slice included) and switch timeout (§7.4) and the gate's clock origin (§8.12); OPEN-9 closed |
| EA1a §2.6 "the group position" **(companion)** | A split turn resumes first in its group at most once; a part split again resumes at the ordinary rotation (§8.13, X17) |
| EA1a §2.9 | Agrees with §8.23 (kept); adds the grant-store re-read as a revocation route (§8.23) |
| EA1a §2.9, §4.1, §7.11 **(companion)** | A process locale other than "C" suspends every ext script (D17 §1.5, T22, X22), with its own `Resumed.reason` value |
| EA1a invariant 8 | Its citation of X24 for the 33rd command now holds: X24 covers `/ext:` commands at L68 |
| EA1a §4.3 **(companion)** | The log record format of §8.1 (prefix, `\xNN`, `\u{2028}`, `…[cut]`, 4,096 bytes) |
| EA1a §5.2, §5.3, §11 **(companion)** | Read-your-writes as §7.6 (no I/O job for a pending read, but counted, rate-limited and delivered next turn; pending deletion answers absent; wipes clear the map; pending entries count to the totals); a refused reservation returns `nil, "out of memory"` (§8.14); write rate 32 KiB/s and 128 KiB/s with 10 MiB and 40 MiB buckets debited at the call (§8.7, L44), where a `Set` or `Delete` on a key whose pending value a flush has already taken pays one key token and the full counted size, and only a replacement of an untaken value pays growth alone; the file index counts toward every apartment that has the store open (rule 7.6.1); temporary files count and are removed at open |
| EA1a §7.6, §7.7, invariants 20 and 22, example 13.3 **(companion)** | Solid press as §7.2.1 (only background colour or image; text and borders do not count); a non-solid press deactivates as a world click; pointer notifications carry `gesture = false`; no text-box focus under case 1 outside a gesture window (§7.2.2) |
| EA1a §7.7 badge **(companion)** | The badge of §8.3 on every shown window with a catching node or a field, "ext script", above all ext windows |
| EA1a §7.10, §11, OPEN-4 **(companion)** | Composition coverage as §7.1 (1.25 and 2.0 screens, unclipped and position-independent, badge and nested entries counted, user resize and host changes handled); image decode at the cap answers `nil, "busy"` at the call (§8.5); OPEN-4 closed |
| EA1a §10.3 **(companion)** | Entity filtering as §7.5 (one batched filter call per turn, matches queued as notifications, overflow as `unfiltered`, cursor to the newest frame at resume with the gap in `Resumed`, the 15 MiB retained bound) |
| EA1a §11 memory row **(companion)** | Adds the kept snapshot data files (§8.10) |
| EA1b header, §1, Related row **(companion)** | Cite TM; TM, not EA1b, wins on the GPU bound and on every limit (§0.1) |
| EA1b §4.5, invariant 2 **(companion)** | A "switch" end reason in the frame a switch or session end is queued; revocation ends leases through suspension (§8.17, §8.23) |
| EA1b §4.6 **(companion)** | User pointer picking under a lease uses the presented view (§8.17) |
| EA1b §5.4 and `crates/render` **(companion)** | Filled boxes drawn with back faces culled (rule 7.1.2), or counted as a full screen with the eye inside |
| EA1b §6 and its §14 OPENs on the budget row and the composition rule **(companion)** | Replaced by §7.1: bounds 0.5/1.0 and 1.25/2.0 screens (not 2·S/3·S); conservative pixel-centre counting (lines, quads, boxes), frustum culling first; fair shares with drop-and-go-on (kept from EA1b); host changes hide in reverse order of last show; the 1.6 ms row and E9 replace the 1.8 ms row, the 2.4 ms margin and E4b; drops reported in stats, profiler and log |
| EA1b §7.4 **(companion)** | The teleport slot is released when the sandbox's outcome arrives (not when it is delivered), when the intent is discarded, or when its session generation ends; outcomes are matched by (generation, request number), and a late outcome of an ended generation is dropped (L75, X80) |
| EA1b §13 **(companion)** | Its escape list adds the slice-1b parts of X20 (maximal hand-off), X28 (a hot reload) and X87 (a hot reload in progress), which TM now tags 1a, 1b |
| EA1b §8.1, §8.8 **(companion)** | The full-reload declaration set and validation of §8.24; snapshot and hand-off bytes inside the new apartment's cap (§8.10) |
| EA1b §8.6, invariant 21, its §14 OPEN **(companion)** | Replaced by §7.6: a pending read counts as outstanding and against the read rate; program order; pending deletion answers absent |
| EA1b §9.2 **(companion)** | Log file format and escaping of §8.1 (escape, not remove); the console shows lines through §8.2 with the log-line cap |
| D17 §8 limits table, script-log row (by status line) | The log keeps 64 KiB/s per script and gains **128 KiB/s for all scripts together**, with the counted "N records dropped" record; "the only rate limit on the log" no longer holds (§8.1, L45, OPEN O-3) |
| D17 M5.5 exit criterion 2 (by status line) | Its `Data` item is slice 2 (`Data` ships in slice 2); in slice 1a the item covers the `Store` read flood only (X25) |
| EM §3.4, §3.6, §12 **(companion)** | Snapshot walk (`check` included) and every store, log and grant-store open classify an entry with `fstatat` without following **before** opening it, open only regular files, and open non-blocking for reads and writes (a FIFO log write open fails with `ENXIO`); a rotation never renames over a non-plain `s_<id>.1.log` (§8.8, X54, X94) |
| EM §8.5, §8.6, invariants 9 and 13 **(companion)** | The session overlay of §8.23 (failed-write and session-only decisions narrow, or for dialog Allows widen, every re-read result; a newer-`version` document changes nothing and suspends nothing); grant-store reads non-blocking, regular-file checked on the handle, bounded at 4 MiB + 1; `.corrupt` backups of those bytes or by rename; the advisory lock taken without blocking, retried for 1 s, then a failed write (§8.8 "Tessera's own files") |
| EM §7.4, §9.7, §10.4 **(companion)** | A load is refused while the same id of the other kind is loaded or loading (L26, X90); a narrowing (revocation, Reset, Remove, local-only switch, narrowing re-read) during a load past its dialog cancels it, with phase D and the top-level item re-checking the record (§8.23, X91) |
| EM §4.11, M17, invariant 22 **(companion)** | Overlapping suggested defaults of one script in one context are refused, not only equal ones (§8.16, L61, X41) |
| EM §8.7 **(companion)** | The store-root scan of §8.7 (L92) gives the orphan totals (when, on which queue, no-follow `fstatat` sizes, caps, how it stays current) |
| EM §11.1, §11.5, §11.8 **(companion)** | Tool-root removal is a no-follow recursive removal inside the root (§8.8); the harness offers a record-flood hook for X19's ST part (L86's 4,096 entities stay); TM §11's intro now names `ELSEWHERE` too. **Done** in EM revision 5 (`[[flood]]`, §11.5, §11.8); its bounds are registered in L86 (revision 6, next row) |
| EM §11.5, OPEN O-13 **(companion)** | The `[[flood]]` bounds are L86's (at most 64 entries, `spawn` 1–16,384 per frame, `life` 1–600 frames, at most 65,536 flood entities alive in any frame); EM cites L86 for them, drops "interim" and closes O-13; `radius` 1–500 m stays in EM as an argument-validity domain (§0.1 rule 6). Answered in revision 6 |
| EA1a §0, §2.3, §4.2, §6.1, §6.2, §7.4, §7.9, §8.2, §9.1, §10.2 **(companion)** | Its resource caps are L91 (command arguments, `help`, edges, `Entity.Read` handles, query radius and box side, ray landblocks, `LocText`); positions, node sizes and offsets, label width, `region`, `max_distance`, timer delay and node names are domains or grammar lengths that stay in EA1a (§0.1 rule 6); §0's sentence says "every resource limit is TM's" |
| EA1a §2.1, §2.6, §2.8, §7.6, §7.7 **(companion)** | Only a press notification of a solid press, an action start or an `/ext:` line opens a gesture window, which closes at the user's next input elsewhere or after 1 s (§8.13, L93); a solid press is judged by the topmost painted element, a disabled node's press opens no window, and only a solid press raises a script's block (rule 7.2.1); per-request results and due timers are never dropped (L30) |
| EA1a §2.7, §2.9, §7.11, §8.1 **(companion)** | Natives and applies check the apartment's effective set, never the live grant table; a handle is checked against the capability of the module that minted it; suspension deactivates the script's ext contexts and releases their holds, and resume does not restore them (§8.23) |
| EA1a §5.2, §5.3 **(companion)** | A pending `Get` shares the entry's immutable bytes (or charges a copy at the call, rule 7.6.3); a read reserves ρ × *n* + ε × min(*n*, 8,192), reads at most the indexed size + 1, answers `fn(nil, "invalid")` for a size mismatch and `fn(nil, "too large")` for a result over its reservation (§8.14) |
| EA1a §7.7, §7.10, §11, §13.1 **(companion)** | Badge text "⟨id⟩ · ext script" in a fixed box (20 × (16 + 8 × (*n* + 13)) UI units), placed wholly on screen, no focus while it is covered or off screen (§8.3); entries counted by size, (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1) (rule 7.1.2), so §13.1's coverage figure is recomputed; an addition that gives a shown frame its badge answers `nil, "limit"` past the bound (rule 7.1.5); the §11 L30 row reads "512 per Frame (window)"; the L56 row is "the 1.0 ms GPU part of the 1.6 ms row" |
| EA1a §4.3, §10.3 **(companion)** | The all-scripts log rate (L45); a subscription matches only records of frames after its creation, a filter call ended by its stopping rule is never an abort, and filter time is outside L17 (rule 7.5.2a) |
| EA1b §4.4, §4.5, §4.7, invariant 2 **(companion)** | `Follow` with an entity handle checks `entity.read` at the call and at every apply (§8.23 handles); the "unload" end in the frame the unload is requested; no lease, lease intent or `Sandbox` command from an `Unload` handler, its intents discarded at its return (§8.17); correct the §4.7 citation of EA1a §2.9 |
| EA1b §5.3, §5.5, §6.4, §14 **(companion)** | The `Lines` cap is L90 (its §14 request answered); coverage-dependent `GetStats` fields only with `world.read` (rule 7.1.4); *n* counts shown lists that are non-empty, drawn or not, as rule 7.1.4 says |
| EA1b §7.4 **(companion)** | Teleports numbered per `Session` connection, reset at `Connect`; the slot released in the frame a switch or session end is queued and at a failed submission (number not consumed); clock changes at most 4 per second for all scripts (L75, X80) |
| EA1b §8.2, §8.3, §8.8, §13 **(companion)** | L84 counts only hot reloads; the file index counts toward the new apartment's cap during a hot reload (rule 7.6.1); X44 is tagged 1b, 2 and X85, X27 and X16 1a, 1b in TM |
| D17 §8 "Loaded scripts" row and §10 (by status line) | A **reading**, not a change of an accepted figure: "at most one reload at a time, so at most 17 apartments exist" limits **hot** reloads, the only reloads that hold two apartments (L84). A full reload is an unload and a load; its load (EM phase A) starts only after the old apartment's close item has run and the old cap has left the sum at its final flush, so it never adds an apartment (L26) and is not counted (requested by EA1b's revision-4 review; restated in revision 6). No OPEN is needed |
| D17 §10 watcher (by status line) | **Design narrowing** (requested by EA1b's revision-5 review): the watcher sends no channel of events; it sets per-script dirty flags in a fixed table of at most 16 slots, read by the main thread once per frame, and drops events for folders of scripts that are not loaded (EA1b §8.2, L84). It still never reads file content; its memory is bounded whatever the event rate. ARCH §21's watcher row reads "sets coalesced per-script flags that main reads" instead of "sends events over a channel" |
| EA1a §6.1, §6.2, OPEN-11 **(companion)** | `help` is §8.2 step 4's **help line** category, 96 code points and 96 bytes (its L91 cap), so an accepted `help` is never cut; EA1a drops its interim "error or log line" cap. OPEN-11 closed (revision 5) |
| EM §4.4 | No change: §8.2 step 4 now names the setting `description` category (160 and 640) that EM §4.4 and L80 already state, and gives every other non-Tessera string the error-or-log-line cap |
| EA1a §2.4, §2.8, §5.3, §11 **(companion)** | Every `Store.Set` or `Store.Delete` with a `done` callback counts toward L27 from the call until its `done` is delivered or discarded (a replaced value's pending `done` included), answering `nil, "busy"` at a stated step of §2.4's order; §11's L27 row lists it with reads and image decodes; an image decode counts until its worker job ends, also after its element is destroyed (§8.5, L27, X25, X26, invariant 36) |
| EA1b §8.5 **(companion)** | During a hot reload, the old apartment's outstanding requests count toward the script's one L27 count, shared by both apartments ("per script" is per (kind, id)), until their results are discarded and their worker jobs have ended |
| EA1b §4.6, §5.3, §14 **(companion)** | L82 restated with the reference point *P* of EA1b §4.6 (the placed character, otherwise the user's own eye); L90's cost restated as `B` + e(2*m*), at most 491,720 units. Both §14 OPENs on TM closed (revision 5) |
| EM §8.5, §8.6, invariants 9 and 13 **(companion)** | §8.23 revision 5: a re-read that is unreadable or finds the document damaged as a whole narrows nothing and suspends nothing, and the previous document view stays the base (EM §8.5 step 2 no longer narrows for those; a missing file still reads as empty); a write's completion is applied as a read result only for the keys it did not carry, and a newer decision of a carried key stays as made; a write that replaces a damaged document writes every grant-table record; the lock retry is L94. X91 gains the ST cases (revoke and widen in one frame under `--latency 2` with an `[[expect]]` of granted; Reset, then a first-enable Allow; a damaged-document re-read and replacement) and a U case (an unreadable re-read); invariant 35 |
| EM §8.5, invariant 25 or 33 **(companion)** | A failed load write taken back restores the overlay entry it superseded, or none, so it never undoes an earlier session decision whose own write failed (a revocation, Reset's or Remove's "no record"); X91's ST part gains the rollback cases (§8.23, invariant 35). Requested by EM's revision-5 review |
| EM §9.8, §7.5, §11.4 **(companion)** | Turning developer mode off cancels every developer load in progress by §7.5's rows (a showing dialog closes as "Not now", store work completes, phase D gets one close item) and refuses waiting developer loads with "Developer mode is off"; the user's unload of a loading copy cancels its load the same way; the trace of the `developer_mode = false` walk event follows (§8.23, X90). Requested by EM's revision-5 review |
| EM §8.7, §10.3 **(companion)** | The store-root scan runs every time the gate enters `Starting` (each session start), not once per client run (§8.7, L92). Requested by EM's revision-5 review |
| EM §9.8 **(companion)** | A full reload's load (phase A, its reservation) starts after the old apartment's close item has run and its cap has left the sum at its final flush (L84), so §7.4's one-id rule and the sum never refuse it |
| EA1a §7.1 **(companion)** | The reason that no badge lies over the caret or focus mark is that both are drawn above every badge (§8.3), not that a badge in the focus area covers the field (the own window's outside badge is the exception). TM §8.3, rule 7.2.2, L58 and invariants 7 and 26 now adopt EA1a §7.7's badge cover and caret rules (revision 6) |
| EA1a §7.10 **(companion)** | The host-change notification is `VisibilityChanged` (rule 7.1.5), not `VisibleChanged`; joined decode requests for one file count toward L27 once each, per request (§8.5, X26 runs with distinct files and with one file) |
| EA1a §5.3 **(companion)** | A pending deletion adds no key and no byte of its own to L40 and L41, present or absent; the deleted file counts until its flush completes (rule 7.6.5), so `Store.Delete` needs no `full` answer |
| EA1b §8.1, §8.2 **(companion)** | A full reload's load waits until the old cap has left the 512 MiB sum (its final flush completed), as item 6 of a hot reload does, and is never refused for the old copy's memory (L84); the watcher's flag table is recorded as a design narrowing of D17 §10 (row above) |
| EA1b §4.10 **(companion)** | The `"region"` and `"focus"` end reasons are §6.8 residual (6); EA1b cites it |
| keymap.md §7.3 | Notes the separate ext holds pool of 64 on top of the 256 (§8.16) |
| keymap.md §6.5 | Notes that `keymap-ext.toml` follows the configured keymap path and that the install guard applies to the resolved target (§8.19) |
| LEGAL §18.4, §18.5, §18.6 | The driven-screenshot item closed by §7.7; the decode-cache content channel closed by §8.9; residuals (3)–(6) of §6.8 (layout between ext scripts, decode timing including resident hits, the overlay share count, a lease's `"region"` and `"focus"` end reasons) added to §18.4's stated limit |
| ROADMAP M5.5 | Exit criterion 2 gains X38, X50 (notices), X65 (slice 2), X85, X88, X51, X91 and X95 (an ext window never covers a plugin window, §8.26); its `Data` item ("`Store` read flood and `Data` lookups of new DIDs") is tested in slice 2, where `Data` ships (X25's slice-2 part); deliverables name this spec |

<a id="s14"></a>
## 14. Open items

> **OPEN O-1:** the GPU cost of one blended screen at 3840 × 2160 on the reference GPU is
> an estimate (about 0.3 ms). E9 measures overlays and composition at the coverage bounds
> in slice 1a (composition) and 1b (overlays); if the 1.0 ms row does not hold, the
> bounds of §7.1.3 shrink in proportion. Owner: slice 1a/1b implementer.

> **OPEN O-2:** funding the 1.6 ms row of §7.1.6 (0.6 ms overlay encoding, 1.0 ms GPU)
> from the frame-budget margin (4.2 → 2.6 ms) changes an accepted D17 figure and tightens
> M5's non-paint-UI condition to 0.6 ms. The owner confirms it at the M5.5 plan review;
> the alternative is to lower the 13 ms ext Lua ceiling by the row. Until then the
> stricter reading applies: the row is counted **and** the 4.2 ms margin is kept, so the
> ext Lua ceiling is 13 ms − row: **12 ms** in slice 1a (composition only, 1.0 ms) and
> **11.4 ms** from slice 1b. Both of D17 §8's worst-frame branches must fit (L12): with
> R = 3 ms the first, 7 ms + R + 2·S, needs S ≤ 1 ms in slice 1a and S ≤ 0.7 ms from
> slice 1b; the second, 2 ms + K + S (or C), needs **K, C ≤ 9 ms** in slice 1a and
> **K, C ≤ 8.7 ms** from slice 1b (L15, L16). If E6 cannot show these, D17 §8's recorded
> fallback lowers the caps: the 4 MiB allocation for R, the 128 KiB source cap for K and
> the 64 MiB memory maximum for C.

> **OPEN O-3:** the lower `Store` byte rate of §8.7 (32 KiB/s per script, 128 KiB/s in
> all, with D17's 10 MiB and 40 MiB bucket capacities, debited at the call) changes D17
> §8's row, and the all-scripts script-log rate of 128 KiB/s (§8.1, L45) changes D17 §8's
> script-log row ("the only rate limit on the log"). The owner confirms both at the M5.5
> plan review. Each is stricter than D17's figures and applies meanwhile; together they
> bound sustained wear to about 21 GiB per day (§8.7).

> **OPEN O-4:** `c_f`, the declared cost of one record-filter pair (§7.5), the declared
> size of one retained entity record (at most 64 bytes assumed), and the neutral reader's
> declared worst-case bounds ρ (parsed bytes per stored byte) and ε (host memory per
> element, every key and every value counting one) for result reservations (§8.14; EA1a
> OPEN-2), which are maxima over hostile values, not typical figures, and the check that
> the neutral format spends at least one stored byte per element (§8.14). E7 and E5
> measure them in slice 1a.

> **OPEN O-5:** the inherited 0014 open review item 2 (§8.20). When M5 resolves it, X71's
> exception is removed or restated to match.

> **OPEN O-6:** whether an image texel's alpha is available to the solid-press test
> (§7.2) for every image format ext accepts without a second decode. If not, an image
> counts as solid only where its whole-image minimum alpha × effective opacity ≥ 0.5
> (the safe direction: fewer presses activate).

---

<a id="s15"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Assets (13), attackers (7) and trust boundaries (9); 89 threats in 11 groups, each with a requirement and an escape test (X1–X90); concrete rules for D17's eight open items (§7: overlay and composition coverage with a GPU row and E9, solid presses and gesture-turn focus, the bridge's direct handler call, the gate's floor frame and switch timeout, metered entity filtering with a record cap, `Store` read-your-writes through a shared pending-write map, the `Input.Drive` allow-list, pointer-transparent world labels) and for its "other improvements" (§8); the earlier items restated (§9); a limits register of 78 rows (§10); 22 invariants; six OPEN items. |
| 2026-10-07 | Revision 2 (Review), answering the independent review and reconciling with EM, EA1a and EA1b. New §0.1 (how other specs cite TM: L-, X-, T- and invariant numbers, never renumbered). §7.1 rewritten: conservative pixel-centre counting, frustum culling first, filled boxes with the eye inside, unclipped position-independent composition counting (closes the move-on-screen hole), fair shares with drop-and-go-on from EA1b, user resizes clamped, host changes hide in reverse show order across scripts, a 1.6 ms row (encoding + GPU) with E9 replacing EA1b's row and E4b. §7.3.1 serial per `AddCommand` call. §7.4: deterministic mode uses the frame condition only; switch bound 392 frames. §7.6: file index defined; pending reads counted and delivered next turn; pending deletion; charging across a hot reload. §8.1 one pinned log format (adds U+2028/U+2029). §8.2: manifest parse-time refusal kept (EM O-9), marks aligned, X47 split in three parts. §8.3 badges above all ext windows. §8.7: buckets 10/40 MiB, debited at the call, per session. §8.9 resident-hit timing residual. §8.10 hot-reload allowance contents (EM O-4 closed). §8.12 X86 figures fixed. §8.14 reservation from pending map or file index times the reader's ratio, returned. §8.17 leases end at a queued switch and at revocation; picking under a lease. §8.19 ext bindings only in `keymap-ext.toml`. §8.21 store work never cancelled half-way. New §8.23 (revocation suspends), §8.24 (no store or hand-off across an `author` change; full-reload set; store owner tag), §8.25 (consent pointer entry), §9.6 (nested C-call bound across resumes, 8 MiB stack). Threats T90–T92; limits L79–L86 and many rows restated (§10.6); tests X91–X93, X50 split by slice, EM's earlier case IDs mapped (§11.10); invariants 7, 10, 13 restated, 23–27 added; §13 lists every companion change; O-2, O-3, O-4 restated. |
| 2026-10-07 | Revision 3 (Review), answering the re-review of revision 2 and the companions' re-reviews. §8.7 (blocking): a key is clean again once a flush has **taken** its pending value (rule 7.6.2 defines taken and untaken), so a `Set` or `Delete` landing during an in-flight flush pays a key token and the full counted size; only a replacement of an untaken value pays growth alone; X57 gains the in-flight run; L44 restated; new invariant 28. §7.5 worked example and X19 within L30 (4 × 16 and 16 × 4). §10: L86 restated (inputs, expectations, frames, `[screen]` and `size` ranges), new L87 (manifest per-field counts), L88 (folder listing), L89 (owner tag), so EM O-11 is answered; §0.1 rule 5 (every count has an L-number; requests answered in §13 or §10) and rule 3's §11.10 citation fixed. Rule 7.4.1: a floor frame resets both conditions; the "at most once per 15 gated frames" sentence corrected. Rule 7.6.1: the file index counts toward every apartment that has the store open. §2 "Call" includes the filter call and REPL runs. Adopted from the companions' re-reviews: §8.8 extended to store directories, their entries and log files (no-follow, entry kinds, fail closed, wipes never follow or recurse; new T93 and X94; invariant 15 restated); §8.13 a split turn resumes first in its group at most once (X17); §8.21 a load waiting on its record write or store probe at a switch; §8.23 a grant-store re-read that narrows suspends, a widening waits for the next load, a record breaking EM §8.3 grants nothing (X91, X62 restated); L75 teleport slot released when its session generation ends (X80); X20, X28 and X87 tagged 1a, 1b; X24 covers commands; X29 builds its lists over several calls; §11 reports multi-harness tests per part; §13 rows for EM §3.1/§8.8, O-11, §8.4/§8.5, §10.4, §11.4–§11.6, §11.8, EA1a §2.6, §2.9, §5.2/§5.3, and EA1b §7.4, §13 (EM §11.5 marked done). |
| 2026-10-07 | Revision 4 (Review), answering the re-review of revision 3 and the follow-up review of the same day, and settling what the companions' reviews refer to TM. **Register** (§0.1, §10): new rule 6, so the register holds every resource limit, and argument-validity domains and grammar lengths stay in their defining specs (EM O-11 answered); new L90 (`Overlay.Lines` per-call cap, EA1b §14 answered), L91 (the API specs' per-call and per-object caps), L92 (store-root scan), L93 (gesture window); L11, L12, L15–L18, L26, L28, L30, L43, L45, L55, L58, L61, L63, L75 and L84 restated. **Slices** (§7.4, §11): X16, X27 and X85 tagged 1a, 1b, with X85's and L11's slice-1a figures for 16 apartments (2 s + 16 × (250 ms + *F*), 376 frames); X22, X25 and X46 tagged 1a, 2; X44 tagged 1b, 2; X31's nested, scale and rotation parts moved to the slice that adds them; a slice ships on the parts that run in it; D17 exit criterion 2's `Data` item noted as slice 2 (§13). **Wear** (§8.1, §8.7): an all-scripts log rate of 128 KiB/s (OPEN O-3), the worst sustained wear restated as about 21 GiB per day, X56 gains the 16-script log flood. **Counting** (rule 7.1.2): composition entries counted by size, (⌈*w*⌉ + 1) × (⌈*h*⌉ + 1), independent of position and sub-pixel phase; wide lines clipped against the viewport expanded by *w*/2; the badge has a fixed counted box. **Frame ceiling** (L12, L15, L16, O-2, T16, X11, X16): the K and C bounds of each interim ceiling (9 ms in 1a, 8.7 ms from 1b) and the fallbacks; X11 and X16 fail on L12's ceilings. **`Store` reads** (§8.14, rule 7.6.1, 7.6.3): reservations from worst-case ρ and ε, reads bounded by the indexed size, a size mismatch answers `invalid`, a result over its reservation is never delivered; a pending `Get` shares immutable bytes. **Badge** (§8.3): placed wholly on screen, inside the window when needed; no focus while it is covered or off screen. **Copy** (§8.4): only wholly displayed text at opacity ≥ 0.5, Cc, Cf, Zl and Zp stripped. **Host change** (rule 7.1.5): windows and labels hidden, `VisibilityChanged{visible = false, by = "host"}`. **Z-range**: new §8.26, T94, X95, invariant 29. **Leases** (§8.17, invariant 25, X44): ended at the event, control back at step 3 of that frame or the next; ended when an unload is requested; nothing from an `Unload` handler. **Filtering** (rule 7.5.2, 7.5.2a, X19): a filter call is never an abort, its time is outside L17, a subscription matches only later frames, X19's ST part uses a record-flood hook. **Settled for the companions:** solid press by the topmost painted element, only a solid press raises a block, disabled nodes (rule 7.2.1); covering by the script's own content (rule 7.2.2, L58); gesture windows opened only by a press notification and bounded (§8.13, L93); effective-set checks, handles checked against their minting module, suspension releases contexts and holds, the session overlay of grant decisions, a newer document changes nothing, loads in progress cancelled by a narrowing, one id loaded or loading (§8.23, L26, X90, X91); overlapping suggested defaults refused (§8.16, X41); classify-before-open and non-blocking opens for the walk, store, log and Tessera's own files, bounded grant-store reads, the tool root's no-follow removal (§8.8, X54, X94); the store-root scan for orphan totals (§8.7, L92); teleports numbered per connection with the slot released at the queued switch or a failed submission and a global clock-change rate (L75, X80); L84 counts hot reloads only; coverage-dependent overlay statistics only with `world.read` (rule 7.1.4, §6.8); per-request results never dropped (L30); TM §11's intro names `ELSEWHERE`. Invariants 4, 5, 7 and 25 restated; 29–34 added; §13 rows for D17's log row and exit criterion 2, EM, EA1a and EA1b. |
| 2026-10-07 | Revision 5 (Review), answering the re-review of revision 4 and the requests that the companions' re-reviews address to TM. **Register** (§10): L90's cost restated as `B` + e(2*m*), at most 491,720 units (every key and every value counts one; the 2,048-line cap unchanged), answering EA1b §5.3 and §14; L82's leased-eye bound measured from EA1b §4.6's reference point *P* (the placed character, otherwise the user's own eye), answering EA1b's review; L84 notes that a full reload's load starts after the old close item and that it narrows D17 §10's "one reload at a time" (new §13 row); L27 restated: per (kind, id), shared by both apartments of a hot reload, a request counted until its result is delivered or discarded and its worker job has ended, and every `Store.Set` or `Store.Delete` with a `done` counted (EA1a's review, items 2 and 9); new **L94**, the advisory-lock wait of at most 1 s on Tessera's own files (§0.1 rule 6). **Display text** (§8.2 step 4): categories for a setting `description` (160 and 640) and a command's help line (96 and 96, L91), and the error-or-log-line cap for any other non-Tessera string; EA1a OPEN-11 answered. **Reservations** (§8.14, O-4): ε is per value-model element counted as D17 §8 and EA1a §2.5 count (every key and every value, booleans included), with the neutral format's one byte per element checked by E5. **Citations** (§8.8): EM §3.6 names the store files (`t_<key>.tmp` included). **Image decodes** (§8.5): a decode counts until its worker job ends, also after its element is destroyed; X26's fail condition extended. **Grants** (§8.23): only a change between two documents Tessera read and understood narrows by another route; an unreadable re-read and a document damaged as a whole change nothing and keep the previous document view as the base; a write's completion is applied only for keys it did not carry, and the newest decision of a key wins; a write replacing a damaged document writes every grant-table record (EM's review, items 1 and 4). **Tests**: X25 gains 33 pending `done` callbacks; X91 gains the revoke-and-widen case under `--latency 2` with an `[[expect]]` of granted, Reset then first-enable Allow, the damaged-document re-read and replacement (ST) and an unreadable re-read (U); X94 gains a held grant-store lock (U). New invariants 35 (only the user narrows grants) and 36 (never-dropped deliveries are counted requests or due timers). §13 rows for D17 §10, EA1a §2.4/§2.8/§5.3/§11 and §6.1/§6.2/OPEN-11, EM §4.4 and §8.5/§8.6, EA1b §4.6/§5.3/§14 and §8.5. |
| 2026-10-07 | Revision 6 (Review), answering the re-review of revision 5 and the requests that the companions' re-reviews address to TM. **Register** (§10): L86 restated with the record-flood hook `[[flood]]`'s bounds (at most 64 entries, `spawn` 1–16,384 per frame, `life` 1–600 frames, at most 65,536 flood entities alive in any frame; `radius` stays in EM as a domain), answering EM OPEN O-13; §7.5's Tests and X19 cite them instead of "as many per frame as the test asks", and §13's row "EM §11.1, §11.5, §11.8" is marked done. L84 restated: it **reads** D17 §8's and §10's "one reload at a time" as the hot-reload limit (no accepted figure changes, so no OPEN); a full reload's load waits until the old cap has left L24's sum at its final flush (EA1b's review); the watcher's fixed table of at most 16 dirty flags replaces D17 §10's event channel (a design narrowing, §13). L26 gains slice 3's server-side half counted as a second script (D17 §8, §11). L27 and §8.5: joined image decode requests count once each, per request; X26 runs with distinct files and with one file. L41 and rule 7.6.5: a pending deletion adds no key and no byte, so `Delete` never passes a space cap. L58, §8.3, rule 7.2.2, invariants 7 and 26: a badge whose fixed box overlaps a text box's focus area covers it, except the box's own window's badge attached outside, and the caret and focus mark are drawn above every badge (adopted from EA1a §7.7); X39 and X49 gain the badge-over-box cases. L65: the 500 ms dead time covers every prompt control, Deny and "Not now" included. L92 and §8.7: the store-root scan runs every time the gate enters `Starting`. L93 and §8.13: a gesture turn split twice under permanent gating is a stated known limit that fails safe, with an X89 case. **Grants** (§8.23): a failed load write taken back restores the overlay entry it superseded, so it never undoes an earlier session decision (EM's review, X91's rollback cases, invariant 35); turning developer mode off cancels every developer load in progress and refuses waiting ones, and a user unload of a loading copy cancels its load (EM's review, X90). **Residual** (§6.8): (6) a lease's `"region"` and `"focus"` end reasons (EA1b §4.10). §13 rows for EM §8.5, §9.8, §7.5, §8.7, §10.3, §11.5 and O-13, EA1a §5.3, §7.1 and §7.10 (`VisibilityChanged`), EA1b §4.10, §8.1 and §8.2, D17 §8 and §10 by status line, and LEGAL §18.4's residual list. |
| 2026-10-11 | Amendment note under the header for [decision 0034](../decisions/0034-official-server-compatibility.md) §4 (extended scripts on official servers by the user's opt-in, automation offline only, D17 §6.3's build-level exclusion withdrawn; TB9, T80 and the `gated-compat` check are superseded by the runtime gate tests of 0034 §4 item 6). Not yet carried into the text; the M9 revision does that. Status unchanged. |
