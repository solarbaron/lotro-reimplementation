<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0017 — Extended scripting: Lua 5.1 ext apartments on the shared script core, capability modules behind a fail-closed backend gate

**Status:** Accepted by the owner, 2026-10-07, as written (revision 6; all owner questions
answered "yes"). The Lua 5.1.5 hardening patch (owner question 1) lives in a pinned fork of the
Lua source crate outside this repository, so `REUSE.toml` stays unchanged. The open security
and design items below are M5.5 threat-model requirements and escape tests. The "Required
follow-ups" (Consequences) are carried out in separate commits. **Amended 2026-10-11** by
[decision 0034](0034-official-server-compatibility.md) §4 (extended scripts on official
servers by the user's opt-in; automation offline only): see "Amendment" below the open items.

This is the architecture that [0016](0016-plugin-class-helper.md) item 2 promises. The owner
asked for "much more extended scripting support that would allow much more stuff", with the
standard plugins still working. Three clean-room designs were written: A (modder first),
B (security first) and C (integration first). Two independent judges scored them, and both
chose **C**. This record is C, with the ideas the judges recommended taking from A and B
grafted in. Each graft is listed with where it lands in "Grafts and where they land".

**How it was written.** Clean-room architect work. Inputs: decisions 0004, 0011–0016;
`lua-plugin-api.md`, `plugin-runtime.md`, `plugin-descriptor.md`, `plugin-api-usage.md`, the
`ui-*.md` specs, `keymap.md`; ARCHITECTURE, ROADMAP, `CLAUDE.md`; and the code of `crates/script`,
`net`, `entity`, `world`, `input`, `render`. No dirty-room material was read. The extended
API is Tessera's own design. Its spec (`ext-api.md`) must stay free of BIN facts: it may not
adopt behaviour that the plugin specs or the UI specs record from disassembly as the ext
contract (§12). Under that rule it has no provenance problem. Timings
marked "measured 2026-10-06" were taken in this session on the reference machine
(`docs/perf/reference-machine.md`, i9-10885H) with the operating system's stock Lua 5.1.5,
not the project's patched build (§1.5). They are indicative, and slice 1a re-measures them
with the patched build (E2, E6, E7).

Abbreviations: LPA = `lua-plugin-api.md`, PR = `plugin-runtime.md`, PD =
`plugin-descriptor.md`, UB = `ui-behaviour.md`, IB = [0014](0014-ui-input-bridge.md), UR =
[0015](0015-ui-rendering.md), ARCH = `docs/ARCHITECTURE.md`, KM = `keymap.md`. "Ext script"
means a script written for the extended API. "Plugin" always means a standard plugin
written for the original client's API.

## Owner summary

**The decision in six lines.**
1. Ext scripts are Lua 5.1 programs in **their own apartments** on the same sandboxed runtime
   core as plugins, loaded from their own folder, under **one neutral global, `Ext`**.
   Plugins never see `Ext`, and ext scripts never see `Turbine`. Ext apartments have **no
   `load` or `loadstring`**: the only code a script runs is the files of its own folder.
   The interpreter is a **project-patched Lua 5.1.5**: in ext states only, it hashes with
   a secret key, places table keys by a keyed function of the whole hash and a per-table
   salt (so scripts cannot build colliding keys), and counts the work of costly
   instructions toward the time check (§1.5). Plugin states run the stock behaviour. In
   every state it orders strings byte by byte whatever the process locale, which is what
   PR §4.4 already requires of plugins. The plugin API and every plugin-visible behaviour
   stay as M5 defines them.
2. Nothing is allowed by default. Each power is a versioned **module** from
   `Ext.Use(name, major)`, handed out only for a granted capability. Rights are the
   intersection of the manifest, the user's grant, the backend class and the server's policy.
3. Ext Lua runs only in frame step 7, **after** all plugin work of that step, with its own
   queue, its own time slice and a hard per-frame cap, and nothing it does runs plugin code
   inside ext time. In a frame that is already late when step 7 reaches it, ext starts
   nothing. Effects are **intents** that the owning step applies. Camera and
   movement overrides are **leases** with a fixed lifetime. Ext key bindings resolve only
   after every built-in binding, and never while a text box or a prompt holds the
   keyboard. A **panic chord** suspends every ext script.
4. A **fail-closed backend gate** with no override: ext scripts run only on the sandbox
   and, from slice 3 (with M7), on the **project-owned** test server that speaks
   Tessera's own protocol, as decision 0004 limits the client. They run
   **never against an official server** (later amended by decision 0034). Automation
   (`input.drive`) runs **offline only**.
5. The module registry is **language-neutral** (Rust port traits plus a binding
   description). Lua 5.1 is the first front end, so a later Luau or Wasm tier is a new
   binding, not a new API. The same registry generates the API reference, the editor
   annotations and the runtime validation.
6. **M5.5** follows M5 and is slice 1a: the patched Lua build, the core, manifest and
   grants, the gate, `Core`, `Store`, `Commands`, `UI`, `Input`, `World`, `Entity`, the
   panic chord, `check` and `test`. Slice 1b adds `Camera` with leases, `Overlay`, `Sandbox`, hot reload, the
   console and the generators. Later slices add audio, movement, local entities and the
   plugin bridge (after M6), and the server channel (with M7).

**Questions the owner must answer.**
1. **Language.** Do you accept Lua 5.1 on the shared core for M5.5? Luau or Wasm would come
   later as a tier behind its own decision. One build can link only one Lua version through
   mlua (Decision §1.4), so a second language means a separate process or a binding of our
   own.
   - **The patched build (§1.5).** Do you accept a small project patch to Lua 5.1.5 (MIT
     licence) in place of mlua's stock vendored sources? Without it, a script can build
     colliding keys that make one instruction walk a hash chain as long as its memory
     allows, and a single costly instruction stays invisible to the time check. The patch
     changes nothing in plugin states. It needs a home: `REUSE.toml` says that no
     third-party material is committed, and a patch carries lines of Lua's source as
     context. Choose either a fork of the Lua source crate kept outside this repository
     and used as a pinned dependency, or a `REUSE.toml` exception for one MIT-licensed
     patch directory.
2. **Names and crates.** Do you accept the global `Ext`, the `scripts/` folder in Tessera's
   data directory, a new L4 crate `tessera-ext` with an `ext` → `script` sibling allowance,
   and a new L5 library `tessera-ext-host` that holds the engine ports (§1.1)? ROADMAP M5
   already lists `tessera-plugins` and ARCH §3.1 places it at M5, so the
   `script`/`plugins` split is M5 work. Do you accept that M5's split must leave a core
   with no `Turbine`, `import` or chat state (§1.1 sizes it), so that M5.5 does not have to
   redo it?
3. **Which servers.**
   - **Official servers.** Do you accept the gate with no setting, flag or variable to
     change it, and the rule "extended scripts never run against an official server" as
     binding policy, with the threat-model limit stated in §6.5? Any M8 decision would have
     to address the rule explicitly.
   - **Third-party Tessera-protocol servers.** 0016 says extended scripts run "offline and
     on Tessera's own server", and 0004 limits M0–M7 to the sandbox and a project-owned
     server. This record keeps that: ext support on `TesseraProtocol` targets the
     **project-owned** test server only (§6.5). Do you accept that third-party servers need
     a separate decision that amends 0004 first?
   - **Default server policy.** If the server states no policy, do you accept every
     group-O capability that a server session offers (all but `data.read`, which is
     offline only) plus `session.server` (itself group A, needed for the script's own
     server channel) as the default, with `ui.native`, `camera` and every other group-A
     capability off unless the policy allows them (§6.5)? The default lets a script
     report every entity the server sends, through walls too, by any output it has
     (a window, an overlay, a sound, even its log file read by another program). The
     only client-side measure is a policy that leaves `entity.read` out of `allow`;
     forbidding combinations cannot close it. The real defence is the server's own
     interest management (§6.5).
4. **Trust model.** Do you accept these parts of it?
   - two grant groups ("observe and show" and "act for you"), with `camera`,
     `session.sandbox`, `input.drive`, `entity.local`, `ui.native`, `bridge.plugins` and
     `session.server` in "act for you" (§3);
   - re-asking for the "act for you" grants whenever the script's content changes, except in
     the developer folder with developer mode on, which is also the only folder where a
     hot reload keeps the running state; at most one script per id is loaded, whichever
     folder it comes from (§10);
   - warnings for dangerous combinations of grants, and for `camera` on a server (§5.2);
   - leases with a 350 ms lifetime and an on-screen indicator that the pointer passes
     through;
   - ext key bindings that resolve only after both passes over every built-in context,
     for presses and for `release`/`tap` bindings alike, and never for a key that a
     focused text box or a prompt swallows, so no ext binding can take an input that a
     built-in binding would match (§5.5; amendments to KM §7.2 and IB D4);
   - ext windows that sit below every plugin window and native panel, and that take
     activation or keyboard focus only after the user's own gesture: a click on the
     script's window (until the user's next key press that goes elsewhere, the pointer
     leaving the script's windows, or 5 s without the user's input there), a press of
     that script's own action, or an `/ext:` line for it while no window is active. A
     script can never take them from a plugin window or a native panel (the chat window
     included), and never from the idle state of play. A focused ext text box must be
     visible, on screen and **not covered** by any plugin window, native panel or the
     reserved layer, with a caret and focus mark the host draws; it loses the focus as
     soon as anything covers it (§4);
   - **separate pools:** ext gets its own surfaces, composition entries, UI memory, paint
     slice, upload share, key-registry space and **keymap document**
     (`keymap-ext.toml`) **on top of** the caps that 0015 and the keymap spec give native
     panels and plugins, never a part of them, so loading ext scripts, or binding keys
     for them, never makes a plugin's window, registration or keymap save fail (§2, §8);
     ext asset decoding has its own cache partition, the lowest worker priority and a cap
     on outstanding requests, so the only effect left on plugins is timing (a plugin
     decode can wait for one ext job a worker has already started), listed in §7;
   - **local-only capabilities** (`data.read` and `bridge.plugins`) never granted to the
     same script as `session.server`; `data.read` offered offline only; a script's
     `Store` that was written under a local-only capability wiped before the script
     opens it with `session.server`, with every file operation of one script's store
     done in order on one queue, so no late write can land after the wipe; and hot
     reload never crossing a change of capabilities. Together these close every route by
     which one script could send decoded data of your install, or your plugins' data, to
     a server. Programs that you install and run together could still pass data to each
     other: two scripts through shared engine state (for example the game clock), even
     across sessions through shared quotas (the total `Store` space answering "full"),
     and a script and a plugin through a bridged command whose arguments the plugin
     keeps, or through layout and timing. §5.2 says so;
   - a panic chord that only Tessera's own settings can rebind (proposed default
     Ctrl+Shift+F12), active only while an ext script is loaded, and checked against
     every key binding when it is set (§5.5);
   - a reserved top layer for consent prompts, the lease indicator and the script manager,
     where every string that is not Tessera's own is shown as plain text, and while a
     prompt shows, ext windows are hidden and "Allow" counts only after the pointer
     has entered it (§5.3);
   - the "never offered" list (§5.6), including "no compiling of strings".
5. **Plugin bridge.** Do you accept a one-way bridge (slice 2)? An ext script may list, load
   and unload plugins, read plugin data, and run a plugin's `/` command, each replayed as
   the user's own action and rate-limited. It may **not** write plugin data or inject chat
   lines that plugins receive; the one indirect route, a plugin that echoes a bridged
   command's arguments to chat, is listed in §7. This is the one exception to "no state
   of plugins" (§5.6). `bridge.plugins` is local-only: never held with `session.server`.
   A two-way bridge would need its own decision.
6. **Automation.** Do you accept that `input.drive` (movement intents and pressing standard
   actions) works **in the sandbox only**? Allowing it on any server, even under an
   operator's policy, needs a separate decision (§6.5).
7. **Placement.** Do you accept M5.5 (slice 1a) after M5, slice 1b right after it, slice 2
   after M6 and slice 3 with M7, with the exit criteria in "Roadmap placement proposal"?
8. **Defaults.** Do you accept these limits (§8)? Measured 2026-10-06 on the reference
   machine, Lua 5.1 runs 100–240 million VM instructions per second in plain loops, so the
   earlier figure of 50 million instructions per call (210–490 ms) could not keep a frame
   under 50 ms. The limits are therefore wall-clock first:
   - 32 MiB of memory per script (a manifest may ask for up to 64 MiB, and the prompt shows
     it), counting both its Lua heap and the host objects it creates (windows, controls,
     labels, timers, subscriptions, queued notifications, intents and results still on
     their way from a worker), at most 16 loaded scripts, and at most 512 MiB as the sum
     of the loaded scripts' caps (so 16 scripts at the default fit, or 8 at the
     maximum), plus one 64 MiB allowance for the second apartment of a hot reload; hard
     counts on those host objects as well (§8);
   - no single string or table part over **4 MiB**, which keeps one uninterruptible Lua
     instruction to about 3 ms;
   - `Store`: 1 MiB per value as written to disk (at most 8,192 elements and 32 levels
     deep), 16 MiB and 1,024 keys per script (each key counts at least 8 KiB of disk),
     256 MiB for all scripts, reads and writes rate-limited, writes written behind; the
     script log at most 1 MiB per file, rotated once, and rate-limited;
   - at most 32 worker requests outstanding per script and 256 for all scripts
     (`Store` reads, decode requests, bridge reads), so no script can grow the worker
     queue;
   - per ext call, the top-level run included, a **4 ms** wall-clock deadline and
     **800,000** work units (the same budget in two measures: 800,000 × 5 ns = 4 ms);
   - at most **2 ms** of ext time started per frame, for all scripts together, plus a
     1 ms collector slice, and nothing started at all in a frame that is already 22 ms
     old when step 7 reaches ext;
   - a worst ext time of **13 ms** in one frame (E6), backend switches included. 13 ms is
     what the 50 ms frame rule leaves after the worst walking frame of the M4.5 exit
     re-run (21.8 ms), the UI paint slice, the plugin slice, a reserve for plugin
     handlers and the plugin collector, ext UI time and a margin (§8, "The frame
     budget"). That arithmetic holds for the M4.5 walking run, which E6 judges. M3's
     fast flights already reach 28.8–34.5 ms with no UI at all, so for them the record
     promises only that ext starts nothing in a frame that is late by step 7; a flight
     frame whose cost comes after step 7 can still pass 50 ms with ext at its ceiling.
     That residual is recorded (§8). The bound depends on figures that slice 1a must
     measure with the patched build: one collector step, one state close and the worst
     compilation (§8, "Residual"). If they do not fit, the 64 MiB memory maximum (or the
     4 MiB allocation cap, or the 128 KiB file cap) is lowered until they do, and you are
     told the result at the M5.5 exit;
   - in the key registry, 256 actions and 16 contexts for all ext scripts together, and
     at most 64 actions and 2 contexts per script. So the registry, not the 16-script limit, is what
     binds scripts that declare many actions or contexts: at most 8 scripts with two
     contexts each, or 4 scripts with 64 actions each, can be loaded together. Scripts
     that declare few actions and at most one context fit 16 at a time.


## Open security and design items (2026-10-06)

After eight revise-and-review rounds (security and architecture reviewers), the items below
were still open. None changes the chosen option; each becomes a requirement of the M5.5
threat model and an escape test in `scripttool test` before the first slice ships.

1. GPU overdraw from ext overlays has no limit and sits outside every budget (§3 Overlay, §8 table row 'Overlay primitives' at line 2290, the frame-budget table at lines 2416-2425, E4/E6). The only overlay limit is a primitive count: 10,000 per script and 20,000 in all. Screen quads and world boxes near the camera can each cover the whole screen. 20,000 alpha-blended full-screen quads at 1080p is about 41 Gpixels per frame, which takes hundreds of ms on the reference laptop GPU. That cost is in step 10, not ext time, so the rolling-window throttle never sees it and the script is never suspended. The 13 ms + 2 ms ext rows of the budget table leave it out, so the record's 'no frame over 50 ms with ext' claim is false for a plain group-O `overlay` grant. Ext UI composition has the same gap: 1,024 ext entries, each a full-screen surface. Fix: bound the summed projected screen area (for example at most k times the screen per script and in total), counted when the overlay list is encoded, with primitives past the bound dropped. Bound the composited area of ext surfaces the same way. Add an overlay and composition GPU row to the budget table. Add a full-screen-quad flood and a near-camera-box flood to E6's hostile scripts.
2. A group-O `ui` script can take the keyboard after any world click (§4 activation case 1 at lines 1068-1079; §3 'Group O vs. group A' at lines 931-935; the exception at lines 937-946). §3 and §7 allow a mouse-visible ext element that paints nothing ('a large, even transparent, ext window'). If that element covers the screen, every click the user means for the world presses the ext window. That press activates the window, and because the pointer never leaves it and every click lands on it, case 1 stays true indefinitely. A timer can then focus a small, opaque, uncovered text box in a corner. The box passes the visible-focus rule, its `text_entry` swallows W and the other movement keys, and the user's typing goes into the script's box. This contradicts the stated reason why `ui` is group O ('a script cannot by itself push `text_entry` and swallow the user's movement keys'). The group-O exception only admits clicks and the wheel. Fix options: (a) a press on an ext element that paints nothing, or paints below 0.5 opacity, catches the click but does not activate the window or start case 1; or (b) in case 1, a script may focus only the control the user pressed, or only in the 6b turn that delivers that press. Add an escape test: a transparent full-screen mouse-visible element, a world click, then a timer `Focus` that must return false.
3. The bridge's 'run a plugin's registered command' contradicts its own guarantee and opens a server route for a local-only capability (§7 bridge item 3 at lines 2041-2045 vs lines 2062-2063; `bridge.plugins` is 'Off, T' in §3 and is local-only under §5.2). The command is 'queued for step 1 of the next frame and run there through the same routing as the user's ... typed line'. That routing re-tokenizes the line and looks the first token up in the shared registry, which holds built-ins too (chat-window.md §7.1-§7.4). `Shell.AddCommand` splits names only at `;`, `:` and `,` and trims only the ends of each piece (LPA §11.1), so a plugin can register a name such as `say x` or a name in literal quotes such as `"say"`. The re-tokenized line then gives W = `say`, a built-in channel command, which on `TesseraProtocol` sends script-chosen text to the server with no user click. The registration can also change between the queue and step 1. So the claim 'it never runs a built-in command' does not hold, and a `bridge.plugins` script (allowed on T when the policy lists it) gains a direct route to the server that §5.2 says it cannot have. Fix: resolve the command to the plugin's registration (its identity) when it is queued, check again at step 1 that the same registration still holds, and call that handler directly with the command and arguments the host builds (`ShellCommand:Execute`). Never re-tokenize, never look the word up in the registry, never run substitutions, and never reach a built-in or the backend. Refuse names that contain whitespace or quotes. Add a §6.6 test with a plugin that registers `"say"`.
4. Heavy-frame gate has no liveness floor (§4 'The heavy-frame gate', §4 'Backend switch', §8 limits table). Unload items obey the gate with 'no exception', and Connect is submitted only after the last unload item. So a backend switch takes 'at most 34 frames that the gate does not skip', and that is not a time bound. On any machine or build where step 7 is routinely reached more than 22 ms after the frame began, the gate skips every frame. That covers a debug build, a machine slower than the reference laptop, and sustained streaming as in the M3-d flight phases (terrain plus object updates of about 25 ms before step 7). In those frames ext never runs, `/ext:` lines are never served, and a user-started backend switch or reconnect waits indefinitely. This contradicts the '~0.6 s, acceptable for a rare user-started action' claim. `scripttool test` turns the gate off, so CI cannot catch it. Fix: add a floor. For example, after N consecutive gated frames or 250 ms, run at least one 6a item and one 6c step anyway; or let a switch blocked past a timeout close states without their `Unload` handlers, as is already done for suspended scripts. State the floor in §8's worst-frame arithmetic.
5. The entity-subscription filter pass is outside the 13 ms bound (§4 6b 'Entity subscriptions are filtered in the script's turn', §8 'The worst ext time in one frame'). The filters run natively at the start of a 6b turn over every recorded spawn and despawn of up to 60 retained frames, times up to 16 subscriptions. The recorded list is not capped: only delivered events are capped, at 1,024 per script per frame. Streamed statics are `tessera-entity` entities (ARCH §3.1), so a landblock integration can spawn hundreds to thousands per frame. This work is neither a 6b call nor one of the 'exactly five' 6a item kinds, so the enumeration that yields 7 ms + R + 2·S does not cover it. 'Charged to that turn's units' has no refuse-or-resume mechanism, because a turn is not a call. Fix: make filtering incremental with a cursor, inside a metered delivery call bounded by the call's units and deadline and by the 2 ms cap; or cap the recorded list per frame and count the excess as drops. Then add the pass to §8's enumeration and to E6, which currently tests only 1,024 events per frame.
6. `Store` write-behind has no read-your-writes rule (§3 `Store`, §8 '`Store` I/O', the `Store` writes row). Dirty keys are flushed at most once per second, and reads are jobs on the store's I/O queue that read the file. So `Store.Set(k, v)` followed within about 1 s by `Store.Get(k)` returns the old value. In a hot reload (§10) the new apartment's open happens in its snapshot job while the old apartment still runs. The new top-level run follows the old `Unload` in the next item, so values the old `Unload` saved are still pending in memory, and the new apartment reads stale data. That defeats the stated 'state survives through `Store` and the hand-off table'. Fix: answer a read of a key that has a pending or in-flight write from that pending value (charged as a conversion), and keep the pending-write map per (kind, id) so that both apartments of a hot reload share it. Alternatively, queue a flush of that key ahead of the read.
7. The `Input.Drive` action set is unbounded (§3 `Input.Drive`: 'press standard game-band actions'). The game band includes `global` (`menu`, `open_chat`, `screenshot`, `toggle_debug`, `toggle_fly`, KM §8) and `pointer` (IB D2). (a) A driven `screenshot` writes image files, which contradicts §5.6 and §12's 'the only file output … is the script's own capped `Store` and its capped script log'. (b) Driven presses carry no command name, so they go 'straight to game code' (IB D1). Game code acting on `menu` or `open_chat` at the apply step, next frame's step 3, shows, activates or focuses native panels. Activation and focus are shared with plugin windows (LPA §13.14) and notifications are synchronous (LPA §9.7). The result is plugin `Deactivated` and `FocusLost` running in step 3, outside IB D8's steps 1 and 7. That is exactly 0014's unresolved open must-fix item 2, and it contradicts §7's 'plugins see only their effects'. (c) `open_chat` pushes `text_entry` without the user's gesture, which contradicts §4's rule that a script never moves the keyboard by itself. Fix: give `Input.Drive` an explicit allow-list (the `movement`, and if wanted `fly`, context actions only) in §3 and in `ext-api.md`, and exclude `global`, `pointer` and every UI-band action.
8. Ext world labels contradict IB D8's step-8 invariant (§3 `UI` row, §4 'Ext UI time', the Consequences row for '0014 D8, Budget', which asserts 'step 8 still changes no box that a hit test reads'). World labels are UI elements (they count in the 2,048-element and 8,192-element caps) that are placed at the projected point in step 8. Nothing says they are hit- or hover-transparent. If a label can be mouse-visible, step 8 moves a box that the next step-1 hit test and the step-7 hover refresh read, which breaks the invariant the record says it keeps. Fix: state that world labels never catch the pointer (not mouse-visible, never hover or hit targets, no pointer notifications), or position them before D8 step 7 item 1.

**Other improvements:**

- Nothing sanitises the script log (§2 table at line 667, §8 'Script log', §5.2 'entity.read on a server, alone'). The record itself expects other programs to follow the log. `print` text with ESC/OSC sequences can inject terminal escapes into a terminal that runs `tail -f`, for example OSC 52 to write the clipboard. Embedded CR/LF can also forge lines that look like the host's own entries ('grant approved', 'Error in other · installed'). Escape C0/C1 control characters (keep tab) when writing the log, and give every record a host prefix, as §5.3 already does for display.
- The plain-text rule for the reserved layer is incomplete (§5.3, lines 1450-1469). 'Cut at 48 characters' does not say whether it counts code points or grapheme clusters. Stacks of combining marks ('Zalgo') can draw far outside a string's box, over the id/kind label or the Allow and Deny buttons. Only Cc and bidi controls are removed. Other invisible or format characters stay: U+200B-U+200D, U+2060, U+FEFF, U+2028/U+2029, tag characters and variation selectors. Cap strings by code points or bytes as well, limit combining marks per base character, clip each string to its own box, and strip every Cf, Zl and Zp character.
- Ext windows carry no host-drawn mark of ownership. A group-O `ui` script can draw a window that looks like a native panel or a Tessera dialog (for example 'session expired, re-enter your password') and capture what the user types into its text box. Consider an unstylable badge with the script's id on every ext window, or at least on a window that holds a focused text box, alongside the host-drawn focus mark that already exists.
- Clipboard copy invites pastejacking (§5.6, lines 1696-1702). A script can change a box's text between the user's selection and Ctrl+C, so the clipboard gets text the user never saw (for example a shell command). State that any script change to a box's text clears the selection, and that a copy takes only text that was on screen when the user selected it.
- Ext image decodes are missing from the outstanding-worker-request cap (§8, lines 2187-2209 list Store reads, Data, Audio, Entity.Local, bridge reads and Files.Pick). Decodes of snapshot images and of `Files.Pick` bytes for UI images are not in the list. A loop that creates and destroys image elements could keep queueing decode jobs. Add them to the per-script and total caps and to the de-duplication rule.
- `Files.Pick` does not require a regular file (§3 `Files`, §8 row). If the user picks a FIFO or a device such as /dev/zero or a named pipe, a blocking open or read can hold a worker forever. Open the chosen path non-blocking, check on the handle that it is a regular file, and read at most 3 MiB, as the folder walk already does.
- The `Store` disk-wear budget is high for a module that needs no grant (§8 rows at lines 2304-2306). It allows 1 MiB/s per script and 4 MiB/s in all, sustained, which is about 86-345 GB of SSD writes per day. Consider a lower sustained rate or a daily write budget. Count temporary files from write-behind in the totals, and remove temp files a crash left behind when the store is opened.
- The snapshot root is not covered by the no-follow rule (§2 'No time-of-check gap', lines 707-715). The text applies `O_NOFOLLOW`/`FILE_FLAG_OPEN_REPARSE_POINT` to the walk inside the folder. State that `data/scripts/<id>` itself is opened with `O_NOFOLLOW|O_DIRECTORY` (and that reparse points are refused on Windows), relative to a handle of `data/scripts`, so a symlinked script folder from an unpacked archive cannot bring outside files into the snapshot.
- The cross-script channels in the residual list (§5.2 'What remains', §5.6 'No state of other ext scripts') miss two cases. (1) The shared ext decode-cache partition and cross-script de-duplication: a cache hit versus 'not loaded' tells script B which DIDs or setups script A requested. (2) The layout residual of §5.6 applies between scripts as well: a full-screen mouse-visible element in a lower ext window learns the shapes of other scripts' windows above it. List both, or partition the cache per script.
- The folder snapshot, up to 8 MiB per script and kept for serving data files, is not among the host objects charged to the script's memory cap (§8 'Memory per script'). Either charge it, or state its separate bound (17 apartments x 8 MiB) in the memory sum.
- Not a defect, for the record: the current core already guards `coroutine.resume` against catching the budget error (crates/script/src/runtime.rs:43-62), and `coroutine.wrap` re-raises, so §8's 'pcall cannot catch it' also holds for coroutines. It would still help if `ext-api.md` stated that the abort is sticky, so that once the deadline has passed every later hook check in any thread aborts again.
- The heavy-frame gate's clock origin is unclear. §4 measures from 'the start of its step 1', but IB D8 runs step-1 work in winit callbacks as raw events arrive, before `RedrawRequested`. With any pacing that waits between frames, a frame's step 1 can begin long before its redraw, so the gate would count idle time. Pin the origin to the previous frame's present, or to the redraw start plus the measured step-1 callback time, so that it matches what the 50 ms rule measures in the perf records.
- The granularity of the 2 ms cap inside a 6b turn needs stating. §4 checks the cap per call ('no ext call starts'), but each notification delivery is its own 4 ms call and §4 also speaks of 'a script not reached in this frame'. State that a turn can be split between calls and resumes in order next frame, and how `dt` is summed then. Relatedly, activation case 2 and `Files.Pick` require the gesture to be 'from this frame's step 1'. If the cap or the gate defers that script's turn to a later frame, a key-bound note-taker intermittently fails to focus. Define the condition as 'the turn that delivers the gesture', whatever its frame.
- Outstanding-request reservations collide with the memory cap (§8 'Outstanding worker requests are capped'). Each `Store` read reserves 1 MiB, so 32 outstanding reads reserve the entire 32 MiB default cap. With any heap at all, a pure `Store` read flood hits the memory refusal before 'busy'. Exit criterion 2 and E6 ('answer nil, "busy" at 32 outstanding requests') then cannot pass as written. State what a failed reservation returns, and either reserve the stored value's actual size (known on disk) or adjust the criterion.
- `table.foreach` and `table.foreachi` exist in the stock 5.1.5 table library and are missing from §8's wrapped-function table. With a native callback (`type`, `rawequal`) they make up to about 330k native calls with no VM instruction in between, so the count hook never fires. That is roughly 10 ms of uninterruptible work, above R = 3 ms. The rule in §8 already requires wrapping them; list them, charging n, or drop them from the ext profile. Audit the compatibility names such as `string.gfind` too.
- KM §7.3's cap of 256 holds is a shared pool that the record does not separate. KM §7.2 fires every complete match in the consuming context, and the collision rule for suggested defaults checks only non-ext contexts. So one manifest can suggest the same stroke for all 64 of its actions, and one press then creates 64 holds. A few such keys held at once exhaust the holds available to built-in movement. Allow at most one suggested default per stroke per script, or give ext holds their own cap.
- Uncharged host memory. The snapshot (up to 8 MiB per script, up to 17 apartments) and the retained entity-event lists (up to 60 frames) are not in the per-script memory accounting or in E5's list. Charge the snapshot to the script, or list it with its total.
- Keymap recompilation on registration and unregistration (KM §2), applied at the item-7 drain, is native main-thread work that grows with the registry. It is not in ext time, E8 or §8's frame arithmetic. A backend switch unregisters up to 16 scripts. Coalesce all ext registry changes of one frame into one recompilation and budget it.
- The record inherits 0014's open review item 2 (game- or session-driven UI changes outside steps 1 and 7 run plugin handlers synchronously). `Sandbox` teleports and clock changes, applied in step 2, can lead game code to show or hide native panels. §7's channel list and the 'no plugin code outside steps 1/7' claims should cite that open item and follow whatever M5 decides.
- `keymap-ext.toml` sits 'next to the keymap file', but KM §6.5 follows symbolic links when saving. State whether the directory is the link's or the resolved target's, and that the settings crate's install guard applies to the ext document as it does to the keymap file.
- §5.4 wording: 'A throttled script's half-rate tick at 8 frames per second comes every 250 ms' means a client running at 8 fps (half rate gives one tick every 2 frames, which is 250 ms). Reword it, because 8 ticks per second would be every 125 ms.
- Partially loaded apartments at a backend switch: say what happens to 6a items of a load that is still in progress (snapshot or compile items not yet run) when a switch is queued. Are they dropped, or are their states counted among the 17 apartments and closed? The gate says no apartment is created until the new session closes.

## Amendment: extended scripts on official servers (2026-10-11, decision 0034)
[Decision 0034](0034-official-server-compatibility.md) §4 replaces the rule "extended
scripts never run against an official server" wherever this record states it (owner summary
item 4, owner question 3, Context, §6.2, §6.4, §6.5, §6.7, §12, and the scheduling decision
below). In short:

- Extended scripts may run on an official-server session only after the user's explicit
  opt-in, which is off by default; it is stored per user, can be withdrawn, and no flag,
  variable, plugin, script or server can set it. The opt-in grants nothing by itself: the
  user's per-script grants (§5) still apply.
- Official-server sessions get a backend class of their own (named by the M8 and M9 specs);
  `Other` stays fail-closed with no override (§6.2).
- **Automation stays offline only** (project policy kept by the maintainer): `input.drive`
  and every module that issues movement, input or game commands on the user's behalf are
  never enabled on any official server, with or without the opt-in. §6.5's rule for every
  server is unchanged.
- Unchanged: the local-only rules (§5.2), `data.read` offline only, the never-offered list,
  `Sandbox` sandbox-only, `Server` only on Tessera's own protocol.
- §6.3's build-level exclusion (the `links` guard and the CI check that keep `tessera-ext`
  and a `gated-compat` crate apart) is withdrawn; the compatibility backend stays its own
  crate, and the M9 slice plan replaces the check with tests of the runtime gate and the
  opt-in.
- "The M8 gate" in this record now reads "M8 (decision 0034)".

Plugins are unaffected (§6.7): on an official server they run as in the original client.

## Owner scheduling decision (2026-10-08)
Extended scripting is not a priority. The milestone planned here as **M5.5** moves to the end
of the roadmap as **M9**. It starts only after full parity with the original client and
compatibility with the server (M8).

What does not change:
- The design and its slices, in their order (1a, 1b, 2, 3).
- The Accepted specs.
- The core/plugins split M5 made for it (decision 0023).

Where this record says "M5.5", read "M9 (slice 1a)". The external Lua fork is created only
when M9 starts, after the owner confirms it.

## Context

- **What the owner asked for.** "Much more extended scripting support that would allow
  much more stuff", while the standard plugins keep working (0016 item 2). Decision 0016
  fixes four constraints:
  - a separate namespace;
  - the M5 compatibility API unchanged;
  - extended scripts never against official servers;
  - a placement after M5.
- **The plugin API cannot grow.** Plugins have only what existing plugins call, with no
  capability the original API lacks. Compatible names stay separable in `tessera-plugins`,
  so the compatibility surface is isolated from the rest of the engine, and BIN facts in
  plugin specs are used only for compatibility with users' plugins.
  Extended features therefore cannot be added to `Turbine.*`, must not leak into plugin
  apartments by any route, and must not change what a plugin can observe.
- **What exists in code (2026-10-06).**
  - `crates/script` (`tessera-script`, about 5,400 lines) holds both the runtime core and
    the plugin surface, entangled:
    - `Apartment::new` (`runtime.rs`) takes the plugins root and the shared plugin state
      (`Shared`: available and loaded plugin rows, the command table, the request queue,
      the `Turbine.Chat.Received` line queue), and installs `import` and the `Turbine`
      packages while it creates the state;
    - `HostApi` (`host.rs`) is plugin-shaped: `write_line`, `plugin_data_dir`,
      `engine_date`, `language`, `builtin_commands`, `install_natives`;
    - `NativeApi` (`api.rs`) registers classes in the `Turbine` object model (LPA §8);
    - the reusable core parts are the sandbox install (`install_sandbox`), `Limits`, the
      instruction `Budget` with its hook every 1,000 instructions, the bounded pattern
      matcher that charges backtracking to the budget (`pattern.rs`), the metered
      finalizer proxy, the `pcall` guard, text-only compilation and error description;
    - `load` and `loadstring` are installed for every apartment. `load` with a reader
      function is capped at 16 MiB (`max_source_bytes`); `loadstring` has no cap.
      Compilation runs outside the instruction hook.
  - `Limits::default()` is 64 MiB and 500 million instructions per host call. PR §9.4 sets
    the plugin memory default to 512 MiB.
  - It pins `mlua` 0.12 with the `lua51`, `vendored` and `send` features.
  - `tessera-input` registers actions and contexts at run time
    (`InputMapper::register_action`, `register_context`). KM §2 caps a registry at 1,024
    actions and 64 contexts, and IB D2 adds the `pointer` context, nine pointer actions,
    seven UI-band contexts and their actions to the built-ins.
  - `tessera-render` offers `Pass::line`, `aabb` and `quad`. Its `PassHook` trait is
    `pub(crate)`, so there is no public way to inject a render stage.
  - `tessera-entity` has generational IDs. A stale ID never reaches a newer occupant.
  - `tessera-world` holds the game clock (`environment::clock::GameClock`, with
    `set_game_seconds` and `set_time_of_day`), and `testkit::SyntheticWorld` builds a
    complete `MemoryWorldSource` from synthetic records.
  - `tessera-net`'s `Command` (`session.rs`) is `#[non_exhaustive]` with `Connect`,
    `EnterWorld`, `Move`, `Teleport`, `QueryLocation`, `Chat` and `Disconnect`. The
    sandbox backend answers `Teleport` and `QueryLocation` partly with **system chat
    lines** ("Teleport cancelled: superseded by a newer request.", "Teleport failed: …",
    place descriptions; `sandbox/mod.rs`), and takes its world view through the
    `WorldSource` trait that the client implements. The `Backend` trait has `name`,
    `as_any`, `submit` and `poll`, and nothing that says which kind of world source it is.
    `net/src/compat.rs` is an empty, documented placeholder that describes a compatibility
    adapter as "one more `Backend`" **inside** `tessera-net`.
  - The client (`apps/client`) already has a library target, `tessera_client`.
- **Frame model.** IB D8 confines plugin code to steps 1 and 7. Step 7 runs the UI tick
  (items 1–4), the plugin host's deferred work sliced at 2 ms of plugin time by whole items
  (item 5), the collector step (item 6) and the outbox drain (item 7). Plugin time counts
  toward the frame rule (no frame over 50 ms) but not toward UR's U1.
- **Limits measured.**
  - PR §9.4: the largest plugin host call uses 12.8 million instructions; compiling 33.7 MB
    of plugin source took 1.2–1.7 s.
  - Measured 2026-10-06 (reference machine, stock Lua 5.1.5, count hook every 1,000
    instructions as in `runtime.rs`):
    - plain loops: 100–240 million instructions per second (field access 125 M/s, table
      writes 100 M/s, calls 185 M/s, arithmetic 215 M/s), so 50 million instructions take
      210–490 ms;
    - string building (`"k" .. i`): about 3 million instructions per second, because each
      instruction does native work;
    - compiling ordinary code: 45–52 ms per MiB of source;
    - one concatenation to a 64 MiB string: 48.7 ms (32 MiB: 24.6 ms), a single VM
      instruction;
    - `table.sort` of one million numbers without a comparator: 490 ms in one native call;
    - a full collection with two million live tables: 99 ms.
  - So instruction counts do not bound time: one instruction or one native call can do
    tens or hundreds of milliseconds of work.
- **The mlua constraint, checked 2026-10-06 against mlua-sys 0.13.0 in the local cargo
  registry.**
  - Its build script stops with "You can enable only one of the features: lua55, lua54,
    lua53, lua52, lua51, luajit, luajit52, luau".
  - Its manifest declares `links = "lua"`. Cargo allows only one package with a given
    `links` value in a dependency graph, so not even two mlua-sys versions can coexist.
  - PUC Lua and Luau both export `lua_*` C symbols.
  - A second Lua dialect in the client binary is therefore not a feature flag. It is a
    second process, or a binding of our own with renamed symbols.

## Options considered

### Designs

| Design | Core idea | Judge 1 (cap / safety / fit / deliver) | Judge 2 (cap / safety / fit / deliver) |
|---|---|---|---|
| **A. Modder first** | Luau in-process, one VM per mod, `@host/*` modules, a wide API (camera stack, scene authoring, shaders and asset overrides later), a two-way plugin bridge, hot reload, a modtool, generated types | 9 / 7 / 7 / 5 | 9 / 5 / 6 / 4 |
| **B. Security first** | Luau in a separate hardened `tessera-scripthost` process, capability objects, snapshot in and commands out (one frame of latency), leases, a panic chord, content-pinned grants, a one-way bridge | 6 / 9 / 6 / 4 | 7 / 9 / 5 / 3 |
| **C. Integration first (chosen)** | Lua 5.1 ext apartments on the shared `tessera-script` core, `Ext.Use` capability modules, intents applied in owning steps, a fail-closed backend gate, M5.5 | 7 / 7 / 9 / 9 | 7 / 7 / 9 / 8 |

- **A.**
  - *For:* the broadest and most ergonomic API. Batched queries, clear errors on stale
    handles, settings pages from a manifest schema, a server-side half run in-process so
    network code works offline, generated types, and a headless test command.
  - *Against:*
    - Slice 1 rests on a linking spike that the mlua facts above show will fail as stated.
    - Slice 1 is very large: nine modules, a console, hot reload, types and a modtool.
    - Its two-way bridge gives plugins a capability the original lacks, which breaks
      plugin-compatibility isolation.
    - Mods pushed by a server are drive-by remote code delivery.
    - The WGSL and asset-override tiers have no gate.
    - Dev mode skips every prompt.
- **B.**
  - *For:* the strongest trust model. No ambient authority, rights as an intersection,
    leases, a panic chord resolved before the UI band, prompts that cannot be spoofed,
    grants pinned to content, an explicit "never offered" list, a one-way bridge, fail-soft
    watchdogs, and a CI check against the compatibility backend.
  - *Against:*
    - Before the first useful script it needs a second binary, a hand-written IPC schema
      with fuzzing, per-OS hardening (Landlock, seccomp, job objects, a macOS profile) and
      a parallel tick.
    - Every query and UI event takes a one-frame round trip.
    - The snapshot is a second world representation beside D8's synchronous model.
    - Slice 1 ships three read-only examples.
    - It is the right long-term home for a Luau or Wasm tier, but too heavy to start with.
- **C.**
  - *For:* it reuses the audited core and its M5 escape tests, the input registry, the
    render primitives, generational IDs, the `Command` vocabulary and the no-execution
    data reader. Its only refactor is the split that ARCH and ROADMAP M5 already plan. It
    has the smallest useful first slice and the closest fit to the accepted decisions.
  - *Against, as judged:*
    - Lua 5.1 with no types.
    - Code changes never re-ask for grants.
    - No leases, panic chord or reserved prompt layer.
    - Its bridge writes plugin data and injects chat lines.
    - 128 MiB default memory.
  - Every one of these is fixed by a graft below or by revision 1.

### Runtime

| Option | Verdict |
|---|---|
| **Lua 5.1 ext apartments on the shared core** | **Chosen for slice 1.** No new VM. The sandbox, allocator cap, instruction hook, `__gc` handling and the M5 criterion-5 escape tests are reused, with three ext-only additions to the core: a wall-clock deadline in the hook, native-call charging, and an apartment profile without `load`, `loadstring`, `setfenv` and `getfenv` (§1.2), plus a small patch to Lua 5.1.5 whose keyed hashing and work counting are on in ext states only (§1.5). Plugin authors already write 5.1. Costs: no integers, no types, slower vector maths. That is acceptable for UI, overlay and tooling scripts. |
| Share the plugins' VMs | Rejected. It breaks apartment isolation (PR §5.3) and reaches plugin globals, so plugins would gain capabilities indirectly. |
| Luau in the client process | Rejected for now. It needs symbol renaming or a hand-written binding (mlua constraint above), which means a second sandbox, second limits and second escape tests. |
| Luau in a separate process (B) | The best route **if** a second language is wanted. It is kept as a later tier behind its own decision (§11). |
| Wasm (wasmtime) | The strongest isolation and many languages, but a heavy toolchain for authors. Kept as a later tier, like Luau. |
| JavaScript (QuickJS) | No advantage, and unfamiliar to this community. Rejected. |

## Decision (proposed)

### 1. Runtime, crates and the language-neutral registry

1. **Crates.**
   - **`tessera-script` keeps the core**, with no plugin names and no plugin state:
     - the link to the project's patched Lua 5.1.5 build (§1.5);
     - state creation from an **apartment profile** (which base functions exist, limits,
       chunk naming, the hashing and work-counting mode of §1.5, the random generator),
       with the allow-listed libraries;
     - `Limits`, protected calls, error description and the `pcall` guard;
     - the work budget with the instruction hook, plus two optional, ext-only additions
       that plugin apartments leave off: a **wall-clock deadline** checked in the same
       hook, and a **charge API** through which natives pay their work (§8);
     - text-only compilation of host-supplied source (binary chunks rejected);
     - the bounded pattern matcher, the metered finalizer proxy and collector stepping;
     - the **neutral Lua-table data reader**: the no-execution reader of a Lua table
       constructor of plain values, with its value model and bounds (caller-supplied
       limits, so the ext host can pass the bounded value model's caps of §8). It
       carries no plugin file names, key grammar or locale rule.
   - **`tessera-plugins` takes the plugin surface:** `Turbine` and its packages, `import`,
     `NativeApi` and the LPA §8 object model, descriptors, `PluginManager`, the command
     table, the request queue and the chat-line queue, the plugin `HostApi`, PR §4.5's
     random generator shared by every plugin apartment (it belongs to the plugin profile;
     ext apartments never reach it, §1.2), and the
     `.plugindata` wrapper over the neutral reader (file naming, the LPA §10.2 key grammar,
     the comma decimal separator of LPA §3.4, the three scopes).
   - **`tessera-ext` (new, L4) is the ext host:** manifest, the grant record's format
     (parsing and validation only), module registry and its Lua 5.1 front end,
     scheduling, limits, leases. It depends on `script` only, through a new sibling
     allowance `ext` → `script` with the same shape as `plugins` → `script`. It never
     depends on `plugins`, so building without the plugin compatibility surface
     leaves it intact. It does no file or settings I/O of its own: it takes bytes and
     returns values (a snapshot it is handed, a `Store` value's bytes, a log line to
     write), so it needs no `ext` → `settings` allowance.
   - **`tessera-ext-host` (new, L5 library) holds the port implementations** over `world`,
     `physics`, `entity`, `input`, `ui`, `net` and `settings`, the **persistence of
     grants** in Tessera's settings and the script manager's settings page, **all file
     I/O of ext**: the folder walk that reads a snapshot, the `Store` files with their
     write-behind and their per-script I/O queue (§8), the local-only marker and wipes,
     and the rotated script log, and the headless harness of §10. (The ext keymap
     document of §2 is read and saved by the same code as the keymap file, KM §6.5.) It does not depend on `render` or `audio`: the `Overlay` port produces neutral
     primitive lists that the client turns into `Pass` calls, and the `Audio` port of
     slice 2 is a trait the client implements. The client and `tessera-scripttool` both use
     it, so no tool depends on an app (ARCH §3). ARCH §3 gains it as the first library at
     L5 ("application glue shared by the client and the script tool").
     - Its one link to the plugin surface, the `Plugins` bridge port (slice 2), sits behind
       an **optional feature `bridge`** of `tessera-ext-host`, the only thing that makes
       `tessera-plugins` a dependency. The client turns it on when it links both. A build
       without `tessera-plugins` turns it off, and `Ext.Use("Plugins", 1)` then answers
       `nil, "unavailable on this backend"`. So `tessera-ext` and `tessera-ext-host` build
       without the plugin compatibility surface (§7 "Independence").
   - **The client links ext behind a default-on feature `ext`** (optional dependencies on
     `tessera-ext` and `tessera-ext-host`). A build with the feature off has no ext code.
     This is what lets a gated compatibility build exclude ext without restructuring the
     client (§6.3, decision 0004). No workspace package has a compatibility feature; how a
     compatibility binary is built is §6.3.
   - **Size of the split.** Of the 5,400 lines of `crates/script`, roughly 2,000 stay in
     the core (`install_sandbox`, `Budget`, `pattern.rs`, the proxy and guard sources,
     `compile_text`, error description, the reader of `plugindata.rs`) and roughly 3,400
     move (`turbine.rs`, `manager.rs`, `descriptor.rs`, `paths.rs`, `api.rs`, the plugin
     parts of `runtime.rs` and `host.rs`). `Apartment::new` must be cut in two so that the
     core builder takes no plugins root and no `Shared`, and does not run `import` or the
     `Turbine` install. `HostApi` splits into a small core trait (script log, error
     reports) and the plugin `HostApi`. The client's `plugins.rs` and the M5 escape and
     behaviour tests move with it and must pass unchanged. This is M5 work (ROADMAP M5
     lists `tessera-plugins`); if M5 ships a split that leaves any of these in the core,
     slice 1a finishes it first and exit criterion 6 applies.
2. **Apartments.**
   - One apartment per ext script, never shared. There are no apartment names in the
     manifest.
   - **Globals:** `_G`, `_VERSION`, `assert`, `error`, `gcinfo`, `getmetatable`, `ipairs`,
     `next`, `pairs`, `pcall`, `print` (to the script log), `rawequal`, `rawget`,
     `rawset`, `select`, `setmetatable`, `tonumber`, `tostring`, `type`, `unpack`,
     `xpcall`; the `coroutine`, `string`, `table` and `math` libraries (with the wrapped
     functions of §8 and the generator below); `collectgarbage` with the `"count"` option
     only; and `Ext`.
   - **Random numbers: one generator per apartment.** In the ext profile `math.random`
     and `math.randomseed` are host natives over a generator that belongs to that one
     apartment (a 64-bit-state generator such as xoshiro256**, seeded from the operating
     system's random source when the apartment is created). `math.randomseed(x)` reseeds
     only that apartment. The argument forms are Lua 5.1's (`random()` in [0, 1),
     `random(m)` in 1..m, `random(m, n)` in m..n), and each call costs a constant.
     `scripttool test` seeds every apartment from the test's seed, so runs repeat. PR
     §4.5's generator, shared by all plugin apartments, belongs to the plugin profile
     only: no ext call draws from it or reseeds it, and no plugin or other ext script can
     change the sequence an ext script draws.
   - **`tostring` shows no addresses.** Stock `tostring` (and so `print`) shows a table,
     function, thread or userdata as its heap address, which would give scripts layout
     information about the process (defeating address randomisation). In the ext profile
     it shows `table: #17` instead: a per-apartment counter assigned on the first
     `tostring` of that value and kept in a weak host table charged to the apartment's
     memory. Two values never share a number within an apartment, and the numbers say
     nothing about memory. The plugin profile keeps stock `tostring`.
   - **Absent:** `load`, `loadstring`, `setfenv`, `getfenv`, `string.dump`, `newproxy`,
     `debug`, `io`, `os`, `package`, `require`, `dofile`, `loadfile`, `module`, `import`,
     `Turbine`, `Plugins`, `plugin`.
     - **No script finalizers.** In Lua 5.1 only userdata run `__gc`, and `newproxy` is
       the one way Lua code makes a userdata with a metatable it controls. Without it no
       ext Lua code ever runs inside a collector step (6c, a script's own allocations) or
       inside `lua_close` (unload, backend switch, client exit). The only finalizers in
       an ext apartment are those of host userdata (module handles), which are native,
       constant-cost and never call Lua. The plugin profile keeps the core's metered
       `newproxy` view.
     - **Host userdata metatables are sealed (requirement).** Lua 5.1 looks `__gc` up in
       a userdata's metatable when it is collected and runs it with hooks off, so a Lua
       function that a script added to a host metatable would run unmetered in 6c or in
       `lua_close`. Every host userdata of an ext apartment therefore has a metatable
       that carries a `__metatable` field: `getmetatable(handle)` returns that field's
       value (a constant string), never the real table, and `setmetatable` on a userdata
       is refused by Lua itself. No native returns a host metatable or lets a script
       reach one through `debug` (absent) or any other route. mlua does this for its
       userdata today; the ext profile states it as a requirement for every host
       userdata, whatever binding produces it, and escape tests check it
       (`getmetatable(handle)` is not a table; no route sets `__gc` or any other field
       of a host metatable).
     - Without `load` and `loadstring` no string becomes code, so the code that runs is
       exactly the folder snapshot that the content hash covers (§5.2), and no data
       received later (from `Store`, `Files.Pick`, plugin data or `Server`) can run.
       Compilation, which no hook can interrupt, happens only for the script's own files,
       in bounded pieces (§2, "Loading").
     - `setfenv` and `getfenv` have no use without a loader, and `setfenv` on a host shim
       written in Lua could redirect its globals. `string.dump` is useless without a
       loader.
     - `collectgarbage("collect")` is a full collection in one native call (99 ms with two
       million live tables, measured); the core steps the collector instead (§4).
   - The host keeps `debug` privately for tracebacks, as for plugins.
   - **The host never runs script code to describe a value.** An error value that is not
     a string (a table thrown by `error(t)`, perhaps with a `__tostring` metamethod) is
     described by the host's error path without metamethods: its type name and its
     `tostring` number (`table: #17`, above), never through `luaL_tolstring` or
     `__tostring`. The same holds for every host path that turns a value into text
     outside a script's own call: the script log, the script manager, notices and the
     developer console. Inside a script's call, `tostring` and `print` may run the
     script's `__tostring` as stock Lua does, which is the script's own metered work. So
     no error report runs script code after its call has ended or passed its deadline.
   - **Host shims written in Lua capture what they use.** A script can replace any global
     (`pairs`, `type`, `tostring`, `string.format`) and, through `getmetatable("")`, the
     string metatable of its own apartment. Every host function written in Lua therefore
     captures each global and library function it uses as an upvalue when the apartment
     is created, before any script code runs, and never calls a method on a string value
     (`s:sub(...)`). Natives written in Rust do not look globals up at all.
   - **Chunk names** are the file's path relative to the script folder (`main.lua`,
     `ui/panel.lua`), so error messages and tracebacks never show a home path or user name
     (§5.6).
3. **Language-neutral module registry (graft from A).**
   - Each `Ext` module is a Rust **port trait**: the engine-side operations, implemented in
     `tessera-ext-host`.
   - Each module also has a **binding description**: its name, major and minor version,
     capability, group, and functions with typed parameters, return types, errors, the
     frame step that applies them, their **work cost** (§8) and their per-frame caps.
   - The Lua 5.1 binding is generated from that description and is one **front end**. A
     later Luau or Wasm front end binds the same ports and the same descriptions, so it
     adds a language, not a new API.
   - The registry is the single source of truth (§10): the reference, the annotation file
     and the runtime argument validation all come from it, so they cannot drift.
4. **Recorded constraint.**
   - mlua enables one Lua version per build. Its sys crate declares `links = "lua"`, so one
     dependency graph holds one Lua.
   - PUC Lua and Luau both export `lua_*` symbols.
   - A second language is therefore either B's separate process or a thin binding of our own
     with renamed symbols. Either is a decision of its own (§11), taken knowingly, never
     slipped in as a feature flag.
5. **The project's Lua 5.1.5 build.** Stock Lua 5.1.5 has two properties that no host-side
   rule can repair, so the workspace links a patched build instead of mlua's stock
   vendored sources.
   - **Why.**
     - *Hash flooding.* Stock 5.1 hashes a long string from at most about 32 sampled
       bytes, with no secret, and walks the interning chain with a byte comparison of
       every entry of the same length. Number keys hash from the sum of the words of the
       double, pointer keys from the address. So a script can build thousands of keys
       that share one chain. Then every string creation (a concatenation, `sub`,
       `tostring`, a received value) or every get or set of such a key walks a chain
       that grows with the apartment's memory, and compiling a file full of colliding
       constants is superlinear. The 4 MiB single-allocation cap does not shrink this.
     - *Costly instructions are invisible to the count hook.* The hook counts
       instructions, not work. One concatenation, one string comparison, one table
       resize or one `next` over a sparse table does work that grows with its operands,
       and 1,000 of them can run between two hook checks.
     - *String order depends on the locale.* Stock `l_strcmp` orders strings with
       `strcoll`, whose cost and result depend on the process's collation locale. Under
       a UTF-8 collation locale glibc's `strcoll` is one to two orders of magnitude
       slower than a byte comparison, and any in-process component that calls
       `setlocale` (for example a GUI toolkit) would change both the cost of one
       uninterruptible comparison and the order that scripts and plugins see.
   - **What the patch does, in ext states only.** The patch is a mode chosen per state at
     creation, before the state's first string, by the apartment profile.
     - **Keyed string hash.** Every string's hash covers every byte and is keyed with a
       128-bit secret drawn from the operating system's random source when the state is
       created (SipHash-1-3 is the reference function; slice 1a may choose another keyed
       function of the same strength and records its cost in E2 and E7). Interning
       compares the stored hash before the length and the bytes. Number keys (with −0
       taken as 0) and pointer keys are hashed with the same key. No script can read the
       secret.
     - **Salted tables with a chain bound.** A table's main positions depend on the key's
       hash and on a salt of that table, drawn again each time its hash part is
       allocated or resized. An insertion that finds the chain at its main position longer
       than **32 nodes** rehashes the table at the same size with a fresh salt. In Lua
       5.1 every node of a chain shares its main position, so a get or set walks at most
       32 nodes, except by the negligible chance that a fresh random salt itself makes a
       longer chain, which the next insertion checks again. The rehash is a table resize,
       already part of R (§8).
     - **How the salt is mixed (normative).** Stock 5.1 takes a main position from the
       low bits of the hash (`lmod(h, sizenode)`, or a modulo for numbers and pointers).
       If the salt were XORed or added to `h` before that, keys whose stored hashes share
       their low *k* bits would collide in **every** table of 2^*k* nodes whatever the
       salt, a script could learn such sets from one table's `next` order, and no
       re-salt would ever break the chain. So in ext states the main position of every
       key kind (strings, numbers, pointers, booleans) is a **keyed pseudo-random
       function of the whole 32-bit stored hash and the table's salt**, such that for two
       different hash values the chance over the salt that they share a main position in
       a table of 2^*k* nodes is at most about 2 / 2^*k*. The reference function is
       multiply-shift: the 32-bit hash, widened to 64 bits, times the table's salt (a
       random **odd** 64-bit multiplier), taking the **high** *k* bits of the 64-bit
       product. A keyed SipHash of (salt, hash) is an accepted alternative. XOR or
       addition followed by a low-bit modulo is forbidden, and `ext-api.md` and the
       patch's tests say so. With this rule, what one table's order reveals says nothing
       about the next salt, so collisions that a script learns stop working once they
       reach 33 nodes. Keys whose whole 32-bit hashes are equal still share a chain, but
       the script cannot see the keyed hash and would need on the order of 2^32 strings
       per colliding pair to find them. The interning table of strings uses the same
       mixing with a per-state salt drawn at creation and at every resize of that table,
       so no bits that a table's order shows are the bits the interning table uses.
     - **The interning table has the same chain bound.** An interning that finds the
       chain of its bucket longer than **32 nodes** rehashes the interning table at the
       same size with a fresh salt, exactly as a table does. So every interning lookup
       walks at most 32 nodes (with the same negligible exception), and native work that
       interns strings (value conversion, compilation, string-producing natives) is
       charged each interned string at that 32-node worst case, which its pre-charge
       includes. A rehash of the interning table, at its stock growth or at a re-salt,
       walks every interned string of the state, so its time grows with the heap; it
       is one of the operations that R measures under the 64 MiB cap (§8, "Residual").
     - **Chain walks are charged (defence in depth).** Every step of a get, a set or an
       interning lookup beyond the 32nd node of a chain subtracts one work unit per node
       from the count-hook counter (below), so even a chain that the bound misses cannot
       hide work from the hook.
     - **Work-proportional hook counting.** Every operation whose work grows with an
       operand (concatenation, string creation with its hashing, string comparison, table
       allocation, resize and rehash, `next` skipping empty slots, a chain walk beyond 32
       nodes) subtracts its work, in
       work units of §8, from the running thread's count-hook counter, stopping at one, so
       the hook fires at the next instruction once the work has used up the interval. The
       count hook charges the call's units and checks its wall-clock deadline (§8), so
       between two checks at most one hook interval of work (1,000 units, about 5 µs)
       plus one operation passes. It runs on the main thread, in the thread that did the
       work. The same charge makes the work units honest for VM work, not only for
       natives.
   - **Official bug fixes, in every state.** The build carries every fix that lua.org
     publishes for 5.1.5 on its bugs page, in both profiles, because they fix crashes and
     memory errors that hostile code can reach. They change no defined behaviour; M5's
     plugin tests confirm it (exit criterion 6). The patch's documentation lists each
     fix it carries.
   - **Byte-wise string order, in every state.** The patch replaces `strcoll` in
     `l_strcmp` with an unsigned byte comparison (the shorter string first when one is a
     prefix of the other, embedded zero bytes compared like any byte). This is the
     C-locale result that PR §4.3 and §4.4 already require of plugins ("never use
     locale collation"), so no plugin-visible order changes, but it no longer depends on
     the process locale. In ext states its work is charged by the compared length, like
     every costly operation.
   - **The process locale stays "C" (requirement).** Lua's number conversions
     (`tonumber`, number-to-text) and character classes still use the C library, so
     the client keeps `LC_ALL` (and so `LC_NUMERIC`, `LC_COLLATE` and `LC_CTYPE`) at
     "C" for its whole life, as PR §4.3 already requires, and **no in-process component
     may change it**. That rules out, for `Files.Pick`, any dialog that runs a toolkit
     in-process which calls `setlocale` (for example a GTK backend that calls
     `setlocale(LC_ALL, "")`): the dialog is a desktop portal or another out-of-process
     or locale-neutral dialog (§8, `Files.Pick`). The client checks the locale after
     every dialog returns and at every ext load; if it is not "C", every ext script is
     suspended with a notice, `Files` answers `nil, "unavailable on this backend"` for
     the rest of the session, and the event is logged as a defect.
   - **What it does not change.** In plugin states the mode is off: stock hashing (so
     `pairs` order and every other plugin-visible behaviour are exactly stock 5.1.5's),
     stock hook counting, PR §4's build limits. M5's plugin escape and behaviour tests pass
     unchanged on the patched build (exit criterion 6). The Lua C API and the bytecode
     format are unchanged; mlua sees an ordinary Lua 5.1.
   - **How it is linked.** One Lua per dependency graph (item 4) still holds: the patched
     sources replace the stock ones in the source crate that mlua's `vendored` feature
     builds, or mlua's external-library mode links a static library the workspace builds.
     Slice 1a chooses and verifies the mechanism. Where the patch lives is owner question
     1: `REUSE.toml` says no third-party material is committed, and a patch carries lines
     of Lua's MIT-licensed source as context. Lua's source is a public library, not
     dirty-room material, so implementers may read it.

### 2. Packaging, namespace and versioning

- **Folders.** Everything lives in Tessera's own data directory (ARCH §17), never in the
  original client's `Plugins/` tree, which stays read-only and plugin-only. The **folder
  kind** is `scripts` or `scripts-dev`.

  | Path | Holds |
  |---|---|
  | `data/scripts/<id>/` | Installed scripts |
  | `data/scripts-dev/<id>/` | Developer scripts. Only with developer mode on |
  | `data/script-store/<kind>/s_<id>/` | Each script's own stored values (`Store`), one file `k_<key>.store` per key (at most 1,024, §8), plus a marker file `local-only` when the store is **local-only** (§5.2); keys follow the script-id grammar, so no key can leave the folder |
  | `logs/scripts/<kind>/s_<id>.log` | The script log (`print`, `Core` log, error reports), at most 1 MiB, rotated once to `s_<id>.1.log` (§8) |

- **No script-chosen string is a bare file name.** Store keys and script ids become parts
  of file names only behind a fixed prefix (`s_`, `k_`), so no name the host creates can be
  a Windows device name (`con`, `prn`, `aux`, `nul`, `com0`–`com9`, `lpt0`–`lpt9`, with or
  without an extension), which a Win32 path open would send to a serial port, a printer
  or the null device instead of a file. As a second layer, the host opens every file
  under `data/` and `logs/` relative to an open directory handle (`openat` on POSIX;
  `NtCreateFile` relative to the parent handle on Windows, which never interprets device
  names) or through `\\?\` paths, never through a plain Win32 path, and the id and key
  grammars below reserve those names too.
- **Script id.** `<id>` follows the KM name grammar without `.` (`a-z`, `0-9`, `_`, 1–32
  bytes), so it can prefix action names. The manifest's `id` **must equal the folder
  name**, or the script is refused. The ids `list`, `reload` and `help` are reserved for
  the `/ext:` management words (§3, `Commands`). The Windows device names `con`, `prn`,
  `aux`, `nul`, `com0`–`com9` and `lpt0`–`lpt9` are reserved as ids and as `Store` keys
  on every platform, so a script works the same everywhere; `scripttool check` and the
  load refuse such an id ("reserved name"), and `Store` refuses such a key with
  `nil, "reserved key"`.
- **Folder rules.** A script folder holds at most 256 regular files and 8 MiB in total,
  at most 4 directory levels deep, at most **512 directory entries of any kind** in total
  (files, directories, links and everything else, refused kinds included) with at most
  64 directories, no file over 3 MiB (so any file fits one string under the 4 MiB
  allocation cap of §8), and no `.lua` file over 128 KiB. Relative paths use `/`, and each
  segment is 1–64 bytes of `a-z`, `A-Z`, `0-9`, `_`, `-` and `.`, not `.` or `..`, and
  not a Windows device name (with or without an extension, compared ASCII
  case-insensitively), so a folder packed on one platform reads the same on another. The
  walk counts every entry it reads and stops reading a listing as soon as the entry cap
  is reached, so a folder of millions of empty directories, or one huge directory, costs
  at most 512 entries of reading. A symbolic link, a special file, a **regular file with
  more than one hard link** (link count above 1 on the open handle: `st_nlink` on POSIX,
  `NumberOfLinks` on Windows), a path outside the grammar or a folder over a cap refuses
  the whole script. The hard-link rule is defence in depth beside the link and
  reparse-point refusal: a hard link in an unpacked script folder would otherwise bring
  a file from outside the folder into the snapshot. The 128 KiB cap bounds one compilation of ordinary code to
  about 6.5 ms (45–52 ms per MiB, measured). Hostile source cannot make it superlinear
  through colliding constants or names, because the compiler's constant table and string
  interning use the keyed hashing of §1.5; E6 compiles the worst file it can build
  (colliding constants under the stock hash, the deepest nesting the parser allows) and
  slice 1a records the worst compile item (K, §8).
- **No time-of-check gap.** The reader runs on a worker over a tree the user (or another
  program) can change while it reads, so it never checks a path and then opens it. It
  opens the script folder once and walks it with directory-relative opens that refuse to
  follow links (`openat` with `O_NOFOLLOW`, and `O_DIRECTORY` for directories, on POSIX;
  on Windows, opens relative to the parent handle with `FILE_FLAG_OPEN_REPARSE_POINT`,
  refusing any reparse point). It checks the type of what it opened on the open handle
  (`fstat` or the handle's file information), never on the path, and counts the caps on
  the bytes actually read, not on reported sizes. A link swapped in after the listing is
  therefore refused, and a file that grows while it is read refuses the script.
- **Loading.** On an asset worker the host reads the whole folder once into memory (the
  **snapshot**) and computes the content hash (§5.2) over exactly those bytes. Then, in the
  ext queue (§4), it compiles each `.lua` file of the snapshot as one item, so compilation
  spans frames and never reads the disk again. Finally one item runs the entry file's top
  level. Images and other data files are served to the script from the snapshot. A file
  changed on disk after the snapshot changes nothing until the next load.
- **Manifest `script.toml`.** It is TOML, like the keymap file, parsed bounded and strictly
  (at most 64 KiB). It is hostile input, so it gets a fuzz target.

  | Field | Meaning |
  |---|---|
  | `id`, `name`, `version`, `author`, `description`, `license` | Identity and display. `version` is semver. `id` equals the folder name. |
  | `api` | `Ext` major version the script targets (first release: 1). |
  | `entry` | Main file, relative to the folder. |
  | `capabilities` | Capability names (§3), each with a one-line `reason` shown in the prompt. |
  | `optional_capabilities` | Capabilities the script can live without; it probes with `Ext.Has`. |
  | `memory_mib` | Optional request above the 32 MiB default, at most 64. The prompt shows it. |
  | `[[contexts]]` | The script's input contexts (at most 2): name. Never opaque, `opaque_keys`, text or pass-through (§5.5). |
  | `[[actions]]` | The script's actions: name, kind (button or axis), the declared context its binding belongs to, and an optional suggested default binding (one stroke, never a sequence; it may carry an `on` activation as KM allows; withheld when it collides, whatever its `on`, §5.5). Every binding the user makes goes through the ext keymap document (below, "Ext bindings have their own keymap document"). |
  | `[[settings]]` | Settings schema: key, type (bool, number with range, choice, text with length cap), default, label. The host shows it as a settings page automatically (graft from A) and hands the values to the script read-only. Key bindings are not a settings type; they are `[[actions]]`. |
  | `autoload` | Whether to load at world entry once granted. |

- **Actions and contexts come only from the manifest.** The manifest declares every action
  and context a script can have. The **host** registers all of them, under their declared
  names and kinds, when the script loads (applied at the item-7 drain of that frame), and
  unregisters them when it unloads, through the **unregistration** operation that this
  record adds to KM §2 (below, "Unregistration in the key registry"). They live in the
  registry's separate ext partition (§8), never in the space KM §2 gives everything
  else. A script never registers anything: at run time
  `Input` only activates and deactivates its own declared contexts, within the rate limit
  of §8. So the binding UI and the ext keymap document can show a script's full set before it ever
  runs, and registration never churns while a script runs.
- **Unregistration in the key registry (extends KM §2).** KM §2 defines registration on a
  running mapper but no removal, and `tessera-input` has only `register_action` and
  `register_context`. Without removal, loading different scripts or reloading after
  manifest changes would use up the registry. This record adds one operation, used only
  by the ext host, and slice 1a implements it:
  - **What it removes.** A set of actions and contexts in the ext partition, all of one
    script, in one operation, applied at the item-7 drain like registration. The
    script's contexts are deactivated first (without dropping any non-ext pending
    sequence, §5.5).
  - **IDs.** A removed entry's slot is free for a later registration. Action and context
    handles are generational, as `tessera-entity`'s IDs are: a handle of a removed entry
    never resolves to a later occupant of its slot; it reads as absent.
  - **Holds and edges.** The holds of bindings of a removed action end at once with no
    release edge (nothing reads that action any more). The buttons stay in the held set
    for press suppression (KM §7.3), as for any hold that ends without a release. A
    pending sequence is dropped only when it belongs to a removed context (§5.5).
  - **Recompilation.** The keymap in use is recompiled against the smaller registry, as
    KM §2 does for a grown one: active contexts, held buttons and the holds of unchanged
    bindings are kept.
  - **Bindings are kept.** The user's bindings for `ext.<id>.<name>` stay in the ext
    keymap document (below). While the action is not registered they compile to
    nothing, as KM §6.1 and §6.3 do for names absent from the registry, and they apply
    again when the script loads.
  - Only the ext host may unregister, and only names with the reserved `ext.` prefix. No
    plugin or built-in entry can be removed.
- **Ext bindings have their own keymap document (extends KM §6).** KM §6.2 caps the keymap
  document at 64 contexts and 1 MiB, and KM §6.3's serialiser refuses a keymap that would
  not read back. If ext bindings shared that document, a user who bound keys for a few
  dozen scripts over time would push it past 64 contexts, after which no rebinding of a
  built-in or plugin action could be saved and the strict reader would fail. So ext
  bindings never enter it:
  - **The document.** `keymap-ext.toml`, next to the keymap file (KM §6.5, and
    `$TESSERA_KEYMAP_FILE`'s directory when that is set), with KM §6.1's schema, KM
    §6.3's strict and lenient reading and KM §6.5's reading and saving rules. It holds
    **only** contexts and actions named `ext.<id>.…`. Its limits: 128 KiB, **128
    contexts** (two for each of 64 ids), 64 actions per context, and KM §6.2's per-action
    and per-stroke limits. Its overrides apply over the suggested defaults that
    registration did not withhold, as KM §6.4 does over built-in defaults.
  - **The keymap file never holds ext names.** The binding UI and the serialiser write
    `ext.` bindings only to the ext document. A hand-edited `ext.` section in the keymap
    file is kept and reported as KM §6.1 says for unknown names, but never compiled
    against the ext partition. So the keymap file, its caps and what its strict reader
    accepts are exactly as KM defines them, whatever ext scripts the user has bound.
  - **Bounded, and cleaned with the script.** Removing an orphan (§5.2, "orphans") also
    removes the bindings of its id from the ext document, unless another folder kind's
    grant record for the same id remains. When a save would pass a limit, the binding
    UI refuses that binding with a report that lists the ids whose scripts are not
    installed and offers to remove their bindings. The lenient reader of an ext
    document over a limit keeps the contexts of installed scripts first and drops the
    others, in reverse sorted order, with a warning. Nothing it does touches the keymap
    file.
- **Refusal is whole.** An unknown capability, an unknown module major, a manifest that
  fails validation or a folder that breaks a folder rule refuses the whole script with a
  clear report. Nothing is partly loaded (graft from B). The same holds for every layer
  of §5.1: a capability that the manifest does not mark optional and that the user's
  grant, the backend class or the server's policy removes refuses the whole script, and
  the report names the layer ("not granted", "unavailable on this backend", "not allowed
  by this server", "not allowed together with …"). A removed optional capability only
  makes `Ext.Has` false. A script never loads degraded on a required capability.
- **Mutually exclusive capabilities.** The **local-only** capabilities, `data.read` and
  `bridge.plugins`, are never held by one script together with `session.server` (§5.2).
  A manifest that requires `session.server` and a local-only capability is refused by
  `scripttool check` and at load. If any of them is optional, the user's grant may hold
  `session.server` or local-only capabilities, never both; the dialog says why.
- **One loaded script per id.** Grants and `Store` are keyed by (kind, id), but action
  names (`ext.<id>.<name>`), keymap bindings and `/ext:<id>` lines carry only the id. So
  at most one script per id is loaded at a time, across both folder kinds. A load of
  `data/scripts-dev/notes/` while `data/scripts/notes/` is loaded (or the reverse) is
  refused with a report naming the loaded copy, and the script manager offers to unload
  it first. The user's bindings, `/ext:` lines and the gestures they carry (activation,
  focus, `Files.Pick`) therefore always reach the one loaded copy. Both copies keep their
  own grants and `Store`; the user's bindings for that id serve whichever copy is loaded,
  as they name an id, not a folder.
- **Namespace.** The single global is `Ext`.
  - It is neutral and does not use the codename, so a product rename
    breaks no script.
  - Module names are PascalCase, so authors who know other Lua APIs feel at home. This is
    a naming style, not an adoption of any plugin-API behaviour (§12).
  - `Ext.Use("World", 1)` returns the `World` module table of major 1. Otherwise it returns
    `nil` plus a reason: "not granted", "unknown module", "major not supported" or
    "unavailable on this backend".
- **Other files.** `Ext.Load("sub.module")` resolves a dotted name to a `.lua` file of the
  snapshot (each segment a path segment, no escape from the folder), runs its precompiled
  chunk once and caches the result per apartment. It never compiles at call time.
- **Versioning.**
  - Each module has its own major. Within a major only additions are allowed. A rename or a
    behaviour change bumps the major.
  - The previous major stays served for at least one Tessera minor release, with a log
    warning at every call of a deprecated name.
  - `Ext.Version` gives Tessera's version.
  - `Ext.Has(name, major)` tests support without granting anything.

### 3. Capability modules

"Apply" is the step of ARCH §20.3 where the intent takes effect (§4). "Group" is the prompt
group of §5.2:
- **O** = observe and show;
- **A** = act for you;
- **—** = always present, no grant.

"Backends" says where a module is available (§6): **Off** = `Offline` only, **T** = also
`TesseraProtocol` (subject to the server's policy), **from slice 3 only**: until the
script-policy handshake ships, the gate admits `Offline` alone (§6.2). Caps per frame are
in §8.

| Module | What a script can do | Capability | Group | Backends | Apply | Slice |
|---|---|---|---|---|---|---|
| `Core` | Log at levels to the script log and the developer console; `Tick(dt)` and `Unload` handlers; the frame clock (one value per frame) and timers (at most 64, §8); settings values from the manifest schema | — | — | Off, T | step 7 (6b) | 1a |
| `Store` | Save and load its own values in `data/script-store/<kind>/s_<id>/`, through the core's neutral Lua-table data reader (never executed). Values follow the bounded value model of §8 (1 MiB as written, 8,192 elements, 32 levels, no cycles); 16 MiB and 1,024 keys per script, reads and writes rate-limited, writes written behind (§8); every file operation of one (kind, id) runs in order on that store's own I/O queue on a worker, results delivered as notifications in the script's 6b turn and turned into Lua values inside that metered call (§4). A local-only store is wiped before a `session.server` holder opens it, on the same queue, after every earlier write (§5.2) | — | — | Off, T | worker; delivered in 6b | 1a |
| `Commands` | Register commands typed as **`/ext:<id> <command> [arguments]`**. They never enter the plugin-visible command table (below) | — | — | Off, T | ext queue → 6b | 1a |
| `UI` | Windows and controls of Tessera's ext widget set, defined in `ext-api.md` in its own terms (§12); world labels as UI text nodes placed at the projected point in step 8 (nodes persist until the script changes them, so a frame without a tick shows them unchanged). Built on `tessera-ui`'s element tree and painter, not on the plugin control layer. Ext windows sit in their own z-range **below every plugin window and native panel** and form their own activation and focus domain (§4, "Ext UI, z-order, activation and focus"). Elements, surfaces, entries, UI memory, paint slice and uploads come from ext's own pools (§8) | `ui` | O | Off, T | 6b; paint step 8 | 1a |
| `Input` | Activate and deactivate the contexts that the manifest declares; the host registers the declared actions and contexts at load (§2), as `ext.<id>.<name>`. They show in the ext keymap document (§2) and the binding UI, resolve only **after both passes over every non-ext context**, at presses and at releases, and never for a key that an `opaque_keys` entry swallows (§5.5). They can never be opaque, `opaque_keys`, text or pass-through contexts. Read the state of standard actions. A script never receives raw keys, except typed text while its own text box has focus | `input` | O | Off, T | step 1 (native resolution of its actions); 6b (reads); D8's item-7 drain (context changes, and the host's registrations and unregistrations) | 1a |
| `World` | Region, landblock, area name, map coordinates and `/loc` text, terrain height, water at a point, game clock and sky state, rays against installed collision sets (at most 500 m long). All are read-only queries of `world` and `physics`, answered from resident data only (§8, "Natives never wait") | `world.read` | O | Off, T | 6b (reads) | 1a |
| `Entity` | Generational handles; kind, name, transform, appearance summary; batched queries by radius, kind or box (at most 512 results); filtered subscriptions for spawn and despawn (at most 16 per script), whose filters run in the script's own 6b turn, never at spawn time (§4) | `entity.read` | O | Off, T | 6b (reads) | 1a |
| `Camera` | Read the pose. `Take()` returns a **lease** (§5.4) that allows follow-entity, a fixed pose, a keyframed path the client interpolates, and the field of view. The lease overrides the user's camera control | `camera` | **A** | Off, T (only if the server's policy allows it, §6.5) | next frame's step 3 | 1b |
| `Overlay` | World-space lines, boxes and screen quads, rebuilt every tick, handed to step 10 as `Pass` primitives. A frame in which the script is not ticked (throttled, or not reached under the cap) draws the list of its last tick, so overlays never flicker; suspension or the panic chord clears it. Depth-tested or on top. Drawn under the game UI (UR) | `overlay` | O | Off, T | this frame's step 10 | 1b |
| `Sandbox` | Sandbox commands: `Teleport` and `QueryLocation` (`tessera-net` `Command`s), and setting the game clock through the **clock port**, which `tessera-ext-host` implements over `tessera-world`'s `GameClock` (`set_game_seconds`, `set_time_of_day`); the clock is not a `Command`. Rate-limited (§8). On any backend other than `Offline`, `Ext.Use` returns `nil, "unavailable on this backend"` | `session.sandbox` | A | Off | next frame's step 2 | 1b |
| `Files` | `Pick()` opens an OS file dialog. The user's choice in that dialog is the grant, per call. The result is one file's bytes (at most 3 MiB) and its **basename** only, never its directory or full path (graft from A). The dialog is **asynchronous** (never a modal call that blocks the main thread), at most one is open for all scripts, and `Pick()` is honoured only in a 6b turn that delivers a user gesture to that script (§8). The result is delivered in the script's 6b turn, inside that metered call | `files.pick` | O | Off, T | delivered in 6b | 2 |
| `Data` | Decoded records by DID: property values by name through the registry, string-table text, enum labels, icons. For the user's local use: no raw bytes, no enumeration by DID range, no writes. Answers only from records already decoded and cached; otherwise `nil, "not loaded"` and a lowest-priority request (§8). Local-only (§5.2): offline only, never granted together with `session.server`, and a store it wrote is wiped before a `session.server` holder opens it | `data.read` | O | **Off only** | 6b | 2 |
| `Audio` | Sounds by DID from the user's archives, 2D or at an entity; bus volumes, never louder than the user's bus volume, with voice, start-rate and summed-gain caps (§8). A sound not yet decoded answers `nil, "not loaded"` and is requested at the lowest priority | `audio` | O | Off, T | step 9 | 2 (after M6) |
| `Input.Drive` | A movement **lease**: a movement intent latched per controller step like real input; press standard game-band actions at the controller level. These presses are action state, not key events: they carry no command name, are never offered to the UI and never reach a plugin `KeyDown` or `KeyUp` broadcast; plugins see only their effects (§7) | `input.drive` | A | **Off only** (§6.5) | next frame's step 3 (1/60 s step) | 2 |
| `Entity.Local` | Spawn and move client-local entities from setup DIDs in the user's install. Only its own, at most 256. A setup not yet decoded answers `nil, "not loaded"` and is requested at the lowest priority. They are drawn only: never targetable or pickable (a click passes through them), never in a plugin-visible gameplay view or entity list, never in another script's `Entity` results | `entity.local` | A | Off, T | next frame's step 2 tail | 2 |
| `UI.Native` | Show, hide, move and query Tessera's native panels by Tessera panel ID. It never reads their content (§5.6), a show never activates a panel, and a move is never saved to the user's layout. Plugins observe the effect (§7) | `ui.native` | A | Off, T | 6b, at once (as UI changes; never in step 8) | 2 |
| `Plugins` | The one-way bridge of §7. Plugin-data reads are bounded by the value model of §8 and delivered as notifications in the script's 6b turn, converted inside that metered call (§4). Local-only: never granted together with `session.server` (§5.2) | `bridge.plugins` | A | Off, T | next frame's step 1, or the plugin queue; reads on a worker, delivered in 6b | 2 |
| `Server` | Typed messages on a channel that Tessera's own protocol defines, to the script's server-side half (§11). Size and rate limits in §8; messages follow the bounded value model and are turned into Lua values inside the receiving script's metered 6b call. Never granted together with a local-only capability (`data.read`, `bridge.plugins`) | `session.server` | A | T (and the in-process loopback offline) | next frame's step 2; received in 6b | 3 (with M7) |

**`Commands` in detail (decides the former OPEN).**
- The command word is `ext:<id>`, followed by one space, the script's command name and its
  arguments. `Shell.AddCommand` splits names at every `:` (LPA §11.1), so no plugin can
  register a name containing `:`, and no Tessera built-in contains one. The `ext:` words
  therefore never collide with a plugin or built-in name in either order, and plugin names
  always win: nothing an ext script does can make a plugin's `AddCommand` return fewer
  registrations.
- Ext commands never appear in the plugins' `Shell.IsCommand` or `Shell.GetCommands`, and
  `ext:` words are not Tessera built-ins in the LPA §11.1 sense. So the language detection
  that plugins do with built-in names (S5) and every plugin-visible command list are
  exactly as M5 defines them.
- Management words: `/ext:list` lists scripts; `/ext:reload <id>` reloads one (developer
  mode; a hot reload only for a `data/scripts-dev/` script, a full unload, re-check and
  load for an installed one, §10); `/ext:help [<id>]` lists commands. They are why `list`, `reload` and `help` are
  reserved ids. Their output goes to the **script manager** (which `/ext:list` and
  `/ext:help` open at the matching page) and, in developer mode, the console. It never
  goes to the chat window, so script-provided command names never become chat lines.
- **Case.** Plugin command lookup is case-insensitive (LPA §11.1), so the `ext:` prefix,
  the script id and the script's command name are all matched ASCII case-insensitively:
  `/EXT:Notes Add` reaches the command `add` of script `notes`. Ids and command names are
  lowercase by grammar, so no two can differ only in case. A word whose prefix is `ext:`
  in any case never reaches the plugin lookup, which loses nothing: no plugin or built-in
  name contains `:`.
- **Unknown or unloaded id.** A `/ext:` line naming no loaded script, or a command the
  script has not registered, produces **no chat line**: a short notice in the reserved
  layer ("No loaded extended script 'foo'") and a script-log entry. So plugins see nothing,
  whatever the user types after `/ext:`.
- The client's command dispatch handles an `ext:` word before the plugin lookup and queues
  the line, unchanged, for that script (§4). The typed line is not echoed into the chat
  log. This supersedes in part the chat-window rules for such lines (`chat-window.md`
  §7.4 step 6, §7.6, §10 items 4 and 6): today `/ext:notes add` would give the command
  word `ext:notes`, the lookup would fail with result 3, and the client would post the
  unknown-command line as a type 1 chat line, which plugins receive. Under this record
  the word is intercepted before step 6, and no unknown-command line is posted for it
  (Consequences).
- **Never sent to a server.** An `/ext:` line, for a known or an unknown id, is never
  forwarded to any backend as a command or as chat, on `TesseraProtocol` included. Its
  arguments can be private (a note's text); only the script's own `Server` messages
  reach a server. Command names follow the KM name grammar without `.`, 1–32 bytes; at most 32 per
  script.

**API rules (graft from A).**
- Calls are chunky. Queries return arrays, and events are filtered subscriptions, never one
  callback per entity.
- Positions use one documented frame: the region frame of ARCH §9.1, as `f64` numbers.
- A call on a stale handle raises a Lua error naming the script's file and line, never a
  silent `nil`. `handle:Exists()` is the probe.
- Every argument is validated. Non-finite numbers, out-of-range DIDs and wrong types are Lua
  errors, never panics. Strings are UTF-8 with replacement characters.
- A per-frame cap that is reached returns `nil, "busy"` until the next frame, never a
  silent truncation (except entity query results, which carry a `truncated` flag).

**Group O vs. group A.** Group A ("act for you") means the script can change the game state,
take over any of the user's controls (camera, movement, native panels), change other
software's state or what a server receives on the user's behalf. Group O can only observe
and draw: it reads state and shows the script's own windows, labels and overlays. `input`
is group O because its actions resolve only for presses that no built-in binding matches
(§5.5), so it adds controls and never takes one over. `ui` is group O because an ext
window takes activation or keyboard focus only after the user's own gesture (§4), so a
script cannot by itself push `text_entry` and swallow the user's movement keys. `camera`
is group A because its lease replaces the user's camera control for as long as the script
renews it.

**The stated exception to "group O only observes and draws": an ext window's own area.**
A window takes the pointer where it lies, as every window does. So a mouse-visible ext
window takes world clicks inside its area (one that covers the screen takes them all),
and a hovered ext scrollable takes the wheel from camera zoom (§4). This is the one way a
group-O script affects a user control. It reaches no plugin window or native panel (ext
windows lie below them, §4), it acts only where the script's own window is drawn, so the
user can see what takes the click, and the user removes it with the panic chord, the
script manager or by closing the window. The grant dialog's text for `ui` says so ("can
show windows that take clicks and the mouse wheel where they lie"). No ext call can
otherwise take over a control in group O.

### 4. Execution model

- **Where ext Lua runs: step 7 only, after every plugin item.** IB D8's step 7 keeps items
  1–6 exactly as they are (UI tick, plugin `Update`, the plugin host's deferred work with
  its 2 ms plugin slice, the plugin collector step). Three ext items follow, before D8's
  item 7 (the outbox drain), which stays last:
  - **6a. The ext queue.** Ext deferred work, in queue order, by whole items. It has
    **exactly five item kinds**: a snapshot result (native, constant), one compilation
    (K, §8), a top-level run, an `Unload` handler and a state close (C). A hot reload is
    a sequence of these. Results that carry values for a script (`Store` reads and write
    completions, `Files.Pick` results, bridge plugin-data reads, `Server` messages) are
    **not** 6a items: they are notifications delivered in the script's 6b turn, where
    turning them into Lua values is part of a metered call (below). File work that a
    load needs (the folder walk, the `Store` open with its local-only marker or wipe) is
    done on workers before the snapshot result is delivered (§8, "`Store` I/O"), so it
    is not an item either. A new item kind must be added to §8's worst-frame bound
    before it ships. Its own slice: no further item starts once the ext queue has taken
    1 ms in this frame. At least one item runs in each frame that the heavy-frame gate
    (below) does not skip. The rule has **no exception**: the unload items of a backend
    switch or a session end obey it too (Lifecycle, below). It is **separate from the
    plugin queue**: no ext item ever waits behind a plugin item or delays one. A
    suspended script's items that would run its code (a pending top-level run) do not
    start while it is suspended (§5.5).
  - **6b. The ext tick.** Scripts that hold a lease (§5.4) come first, then the others;
    within each group the order is round-robin and starts one script later each frame. For
    each script: first its queued notifications in the order they arose (its action starts
    and ends from step 1, its `/ext:` command lines, its UI control notifications, its
    entity subscription events, its `Store`, `Files.Pick`, bridge-read and `Server` results), then its
    due timers, then `Tick(dt)`. A script not reached in this frame keeps its
    notifications in order, within the queue cap of §8, and gets the summed `dt` at its
    next tick.
    - **Values are converted inside a metered call.** A notification that carries a value
      (a `Store` read, a `Files.Pick` result, a bridge plugin-data read, a `Server`
      message) is delivered as one call: the host first builds the Lua value, then runs
      the script's callback. The worker that read or received the value has already
      parsed it into the bounded value model (§8) and counted its elements and bytes,
      refusing it as "too large" past the model's caps, so the cost of building it is
      known before the call starts and is charged first, against the call's units and
      its 4 ms deadline, like any native (§8, "Work units"). The value model's caps keep
      that cost far below the deadline (E7 calibrates it, E6 tests the maximal value).
      So no native value conversion runs outside a metered call.
    - **Entity subscriptions are filtered in the script's turn.** Steps 2–5 only record
      each frame's spawns and despawns once, in one list for all consumers. A script's
      subscription filters run over that list at the start of its 6b turn, charged to
      that turn's units, so no ext filter work runs in steps 2–5 or outside ext time. The
      host keeps each frame's list until every subscribed script has filtered it, for at
      most 60 frames; a script not ticked within that window loses the older frames'
      events, counted as drops in its next delivery, and what it filters still counts
      against the queue cap of §8.
    - **Suspension stops accrual.** While a script is suspended (watchdog, panic chord,
      grant revocation), it accrues nothing and runs nothing: its subscriptions record
      no events, its timers do not fire, its UI produces no notifications, its pending
      notifications are dropped, each kind with a count, and none of its queued 6a
      items that run script code starts (§5.5). When the user resumes it, its
      first turn gets one `Resumed` notification with those counts.
    - **Notifications a script causes itself.** A UI call never runs Lua re-entrantly. The
      change applies at once (a getter returns the new size at once), and the
      notification it causes (its own `SizeChanged`, `VisibleChanged` and so on) is
      appended to that script's queue and delivered in its **next** 6b turn, normally the
      next frame. The same holds for notifications caused in 6a (a top-level run that
      builds windows) and for notifications that the host's own changes cause (a
      suspension that hides the script's windows).
      Notifications that native work produces **before** 6b are delivered in the same
      frame's 6b: step 1's routing of the user's input, and the hit test and hover
      refresh of D8 step 7 item 1 (for example the pointer entering an ext control, even
      when an earlier ext geometry change is what moved the control under it). Everything
      produced from 6b on is delivered in the script's next turn. So one turn never chains
      into another, and each notification is delivered once.
  - **6c. The ext collector step:** `lua_gc` with `LUA_GCSTEP` and 20, one apartment at a
    time in round-robin order. Its own slice: no further apartment is stepped once 6c has
    taken **1 ms** in this frame, and at least one apartment is stepped in each frame
    that the heavy-frame gate does not skip, so with 16 scripts each is stepped at least
    every 16th such frame. Allocation in a script's own
    calls also advances its collector (Lua's own steps on allocation), and that time is part
    of the call. 6c is ext time; it is outside the 2 ms cap, so a busy frame never starves
    the collector, and inside the residual arithmetic of §8. 6c runs no ext Lua code:
    ext apartments have no script finalizers (§1.2), so a collector step does native
    work only.
- **No ext Lua in step 1.** Step 1 does only native ext work: the panic chord (§5.5); the
  mapper's resolution of ext actions (their starts and ends are recorded per script); the
  native routing of ext UI (hit tests, capture, focus, text editing, dragging and resizing
  of ext windows); and queueing `/ext:` lines. Ext controls do not veto routing: their
  input behaviour is declarative and native (`ext-api.md`), and the script hears about it
  in 6b of the same frame, before layout and paint. So there are no "step-1 intents" and
  no step-1 reads. IB D8's step 1 gains only native work: the panic chord and the
  showing of consent prompts deferred from the previous frame (below).
- **The per-frame ext cap.** No ext item (6a, beyond the first) and no ext call (6b) starts
  once this frame's ext time in 6a and 6b has reached **2 ms**. With the per-call deadline
  (§8) the worst 6a–6b is the cap plus one call or item that overruns its deadline by one
  uninterruptible step, and 6c adds its 1 ms slice plus one collector step. §8, "Residual",
  gives the sum and what it rests on.
- **The heavy-frame gate.** If, when 6a would begin, more than **22 ms** (a Tessera
  constant; slice 1a may tune it from E6) of wall time has passed since the frame began
  (the start of its step 1), ext starts **nothing** in that frame: no 6a item, no 6b call
  and no 6c step. Its notifications, timers and queue wait for the next frame, within
  their caps. So ext never stacks its ceiling on a frame that engine work, streaming or
  plugins have already made late before step 7. The gate only removes ext work, so the
  worst-frame bound of §8 holds with or without it; what it adds is protection for the
  heavy frames that bound does not cover (§8, "The frame budget", the flight residual).
  The price is liveness under sustained load: while every frame is heavy, ext makes no
  progress, its leases expire (the safe direction, §5.4), notifications past the queue
  cap are dropped and counted, and a backend switch waits (Lifecycle). Skipped frames
  count as zero ext time in the rolling-window rule. Deterministic mode turns the gate
  off (§8).
- **Ext UI, z-order, activation and focus.** Ext windows are top-level windows, but they
  sit in their **own z-range below every plugin window and every native panel**, whatever
  `ZOrder` those have, and above only the 3D world and ext overlays. Within the range, ext
  windows are ordered among themselves. So an ext window can never cover, hide or
  clickjack a plugin control or a native panel, and never changes what a hit test finds
  over one. Ext windows also form their **own activation and focus domain** for
  programmatic requests. The one active window and the one keyboard focus stay shared
  with plugin windows and native panels (LPA §13.13–§13.14), so the domain is a rule
  about who may move them:
  - **The user's input** (step 1, native) moves activation and focus as for any window: a
    press on an ext window activates it, and the active plugin window gets `Deactivated`
    and its focused text box `FocusLost`, exactly as for a press on a native panel. A
    press on an ext text box focuses it. This happens in step 1, where plugin code is
    allowed and counts as plugin time.
  - **A script's request** (`Activate`, `Focus`, or showing a window with activation) is
    honoured at once only in two cases, both rooted in the user's own gesture:
    1. one of the **same script's** windows is the active window at the time of the
       request (it became active by the user's press, or by a request honoured under
       case 1 or 2), **and the user's attention is still there**: since it became
       active, no key press of the user has resolved outside that script's windows (in
       the game band, in another window, or in the chat input), the pointer has not
       left all of that script's windows, and no more than **5 s** have passed since the
       user's last press, release or key on them. The request may move activation or
       focus among that script's own windows and controls. So after one click on a
       script's window, the user who walks off with W, moves the mouse elsewhere or
       simply waits ends case 1, and a timer later cannot focus a text box and swallow
       W. Inside those limits a timer can still focus a box before the user's next key;
       the visible-focus rule below makes that visible where the user is looking;
    2. the request is made in the 6b turn that delivers a **user gesture for that
       script** from this frame's step 1: a start of one of its own ext actions (the user
       pressed the script's key) or an `/ext:` line for it that the user entered, **and
       no window is active**. This is what lets a key-bound note-taker open its window
       with the cursor in its text box. Only that one turn counts; a timer or a later
       tick does not. There is no exception for the chat window: an `/ext:` line
       submitted from the chat input while the chat window is still active opens the
       script's window inactive, and the user clicks it to type. So the absolute rule
       below holds as stated.

    In every other state, including the ordinary idle state of play where **no** window
    is active (a click on the world deactivates everything, LPA §13.14), the request only
    remembers the focus control in the script's window (as LPA §13.14's `Focus` does on
    an inactive window), shows the window inactive if it was a show, and returns `false`.
    So a script can never take activation or focus from a plugin window, a native panel
    (the chat window included) or another script's window, and can never focus its own
    text box by itself, which would push `text_entry` (`opaque_keys`, IB D6) and swallow
    the user's movement keys.
  - **A focused ext text box is always visible and uncovered.** Even under case 1 or 2,
    a request to focus an ext text box (or to activate a window whose remembered focus
    control is one) is refused unless the box is shown, inside the screen, not clipped
    by an ancestor to less than its full height and 16 logical pixels of width, in a
    window and ancestors whose opacity is at least 0.5, **and not covered**. The box's
    **focus area** is its rectangle grown by the width of the focus mark. It is covered
    when any higher layer draws or catches the pointer anywhere inside that area: any
    shown element of a plugin window or a native panel (at any opacity, whatever its
    `ZOrder`), the developer overlay, or any surface of the reserved layer. An element
    counts when it paints anything there (a background, an image, text, a border) or is
    mouse-visible or blocks clicks (UB §4); the transparent, non-mouse-visible area of a
    full-screen native root panel does not cover, as it does not catch the pointer
    either. The host draws the caret and a focus mark (a border in Tessera's focus
    colour) itself, over the script's own drawing, and the script cannot style them
    away; with the box uncovered, both are on screen where the user can see them.
    - **Re-checked whenever anything can change it.** The host checks the conditions
      again at the end of every step that can move, show, resize, raise or restyle the
      box or anything above the ext range: step 1's routing of the user's own drags and
      toggles, the plugin items of step 7, 6b (the script's own changes and `UI.Native`),
      the item-7 drain and step 8's layout. If the box fails, the host takes the focus
      away at once, as a hide does: from that moment step 1 offers it no key, and its
      `text_entry` and `ui.clipboard` entries are popped where band requests apply (the
      next item-7 drain). Until that pop a key that the band end swallows reaches no
      one, which is the safe direction. Since the last check of a frame comes after
      step 8's layout, no key of the next frame is routed to a box that is covered.
    - So after one click a script cannot move the keyboard into a hidden, zero-size,
      transparent or covered box (for example one placed under the chat window, whose
      caret and focus mark the chat window would hide) and capture the next chat
      message. An escape test places a box under the chat window and under an opaque
      plugin window: the request returns `false`, and a box that a plugin window is
      moved over loses the focus before the next key.
  - **Hiding or closing** a script's own active window deactivates it and no other window
    is activated (the Tessera rule of LPA §13.14). Only the script's own controls get
    `Deactivated` or `FocusLost`.
  - **Consent prompts** are modal and hold keyboard focus (§5.3), so showing one takes the
    focus from whatever holds it. A prompt raised outside step 1 (an autoload in 6a, a
    content change found while loading) is queued and shown **at the start of the next
    frame's redraw, immediately before IB D8's `offer_pending`**. The raw events that
    arrived earlier in winit callbacks (step 1 of that frame, IB D8) were resolved
    without the prompt; every later raw event sees it. A plugin text box's `FocusLost`
    is therefore delivered in step 1, as plugin time. A prompt raised by the user's own
    action in step 1 shows at once. The lease indicator, notices and the script manager
    never take activation or focus by themselves. **While a consent prompt shows, ext
    windows, ext world labels and ext overlays are neither drawn nor hit-tested** (the
    scripts keep running; their windows reappear when the prompt closes), so no script
    can draw a lure next to or under a prompt (§5.3).
  - **Where the pointer reaches ext windows.** Hit tests walk plugin windows and native
    panels first, as UB §4 defines (an element catches the pointer only when it is
    mouse-visible or blocks clicks). A point that none of them catches reaches the ext
    range next, and only then the world. So the transparent area of a full-screen native
    root panel leaves ext windows under it clickable, exactly as it leaves the world
    clickable, and an area that blocks clicks hides them from input as it hides the
    world. The UB §4 order is extended accordingly: the reserved layer first, then
    plugin windows and native panels, then the ext range, then the world (Consequences,
    UB §4).
  - **The wheel over ext scrollables.** A hovered ext scrollable (an ext list or scroll
    view) pushes `ui.scroll_mouse` for itself, as a native scrollable does (IB D5 "Who
    pushes"), so the wheel over an ext list scrolls it instead of zooming the camera, and
    pops it when the hover leaves.
  - So no ext call and no ext item ever runs plugin code, and nothing in the `UI` module
    alters what plugin hit tests or hover find: ext windows are below every plugin window,
    and the reserved-layer surfaces that a script can cause (the lease indicator,
    notices) let the pointer through (§5.3). The one module that does change plugin hit
    tests is the group-A `UI.Native`, which moves native panels; §7 lists it.
- **Intents.** Natives never touch the renderer, physics or session directly. They append
  intents, and the step named in §3 applies them:

  | Made in 6a–6b | Applies |
  |---|---|
  | UI changes to the script's own nodes | at once in the element tree; painted in step 8 of this frame; activation and focus only as §4 "Ext UI, z-order, activation and focus" allows; the script's own notifications in its next 6b turn |
  | `UI.Native` show, hide and move (slice 2) | at once in 6b, as UI changes are, never in step 8; like a plugin's own geometry change in step 7, it reaches hit tests and hover at the next hit test or hover refresh (IB D8), and step 8 still changes no box that a hit test reads |
  | `Input` context activations and deactivations (and the host's registrations at load and unload) | queued and applied at D8's item-7 drain, with the band requests (D5), when the mapper is not borrowed; they never drop a pending sequence of a non-ext context (§5.5) |
  | Consent prompts raised in 6a–6b | shown at the start of the next frame's redraw, immediately before `offer_pending` (step 1) |
  | Overlay primitives | step 10 of this frame |
  | Camera lease intents | step 3 of the next frame |
  | Movement lease intents (slice 2) | latched per 1/60 s controller step from step 3 of the next frame, so the determinism of ARCH §20.3 holds |
  | `Sandbox` commands, `Server` messages | submitted in step 2 of the next frame, before `Session::poll` |
  | `Entity.Local` edits | the tail of step 2 of the next frame |
  | Bridge actions (slice 2) | step 1 of the next frame, or the plugin queue (§7) |

- **Pending intents are capped** per script and frame (§8); a call that would exceed the
  cap returns `nil, "busy"`.
- **Intents never outlive their apartment or their session.** Every queued intent carries
  its apartment and the **session generation** (a counter that rises at every `Connect`
  and every disconnect) under which it was made. Two rules hold:
  - **Drop on unload or suspension.** When an apartment unloads (for any reason,
    including a backend switch, session end and hot reload's old apartment) or is
    suspended (watchdog, panic chord, grant revocation), every intent it queued and not
    yet applied is discarded in that same frame, as its leases end (§5.4).
  - **Check at apply time.** The applying step checks again, for each intent, that its
    apartment is still loaded and not suspended, that the session generation is the
    current one, that the backend class still admits the module (§3, "Backends") and
    that the grant still holds (§5.1). An intent that fails any check is discarded with a
    script-log entry. So an intent made in 6b of the frame before a backend switch can
    never be submitted beside, or after, the new backend's `Connect`, and a `Sandbox`
    intent can never reach an `Other` or `TesseraProtocol` backend.

- **Reads.** Reads in 6a–6c see the state after steps 1–6 of this frame: this frame's
  action state, session events, controller and camera results, streaming and animation.
- **Ext time.** Time inside ext Lua calls and the host's 6a–6c work. It is measured apart
  from plugin time and from U1, is never charged to the plugin slice, and counts toward
  the frame rule. Because no ext call runs plugin code (above), no plugin handler time is
  ever inside ext time, and a slow plugin cannot make an ext call miss its deadline.
- **Ext UI time (steps 1, 7 and 8).** All of Tessera's native work for ext UI runs no
  Lua and is measured as **ext UI time**, apart from U1: in step 1, the hit-test work
  that reaches the ext range, routing to ext windows, ext text editing and the dragging
  and resizing of ext windows; in step 7 item 1, the hover refresh and hover check over
  ext windows; and in step 8, the layout, paint and upload encoding of ext windows and
  the placing of ext world labels. Ext paint jobs have **their own paint slice of
  1 ms per frame** and **their own upload share of 1 MiB per frame**; they never use
  0015 U4 (b)'s 4 ms slice or U3's 8 MiB cap, so ext windows never delay a native or
  plugin first paint (U4 (c), (d)). This changes 0015 §6's definition of U1, which
  counts "Tessera's own work inside the offers" and step 8 of all UI: U1 now counts
  native panels and plugin UI only (Consequences), so U1 measures them exactly as M5
  defines them, whatever ext windows are open. Ext UI time counts toward the frame rule
  and the profiler line, is outside the 2 ms cap of 6a–6b (it is native work bounded by
  ext's pools of elements, surfaces, entries and UI memory, §8), and has its own budget,
  E8 (§9).
- **Lifecycle (ARCH §20.1).**
  - Ext scripts load only after the backend is connected (start-up step 8), the session's
    backend class is known, and the UI is ready (step 9). Autoload is queued at world
    entry, one load per script in the order of the user's script list. **World entry**
    means the session's first entry into the world after its `Connect` (the
    `EnterWorld` that the user's login or the sandbox start issues), once per session.
    No ext action counts as world entry: a `Sandbox` teleport, a clock change or any
    other script effect never queues an autoload, so no script can raise a prompt by
    what it does (§5.3).
  - **Unload items.** Every unload, for any reason, is two ext items: the `Unload`
    handler (with its 4 ms deadline), then the state close (`lua_close`); for a
    suspended script it is the close alone (below). The close runs
    no ext Lua code (§1.2, no script finalizers); its native time C grows with the heap
    and is measured in slice 1a (§8).
  - **Backend switch.** Before `Command::Connect` on any new backend, every ext apartment
    is unloaded. The switch is queued: the unload items go to the head of the ext queue,
    and `Connect` is submitted only after the last one. From the frame the switch is
    queued, no ext tick (6b) runs and every apartment's pending intents are discarded, so
    no new intent can be made while the unloads span frames. The unload items obey the
    1 ms slice of 6a like every other item (at least one item per frame that the
    heavy-frame gate does not skip), so a switch adds nothing to §8's worst frame. At
    most 17 apartments exist (16 loaded plus the new apartment of one hot reload, §8),
    so a switch is at most 34 items and takes at most **34 frames that the gate does not
    skip** (about 0.6 s at 60 fps) before `Connect`, which is acceptable for a rare,
    user-started action. The memory sum allows either 16 apartments at 32 MiB or 8 at
    64 MiB (plus the reload allowance's one apartment of up to 64 MiB), so the
    longest switch is the 17 small apartments and the one with the largest closes is
    the 9 at 64 MiB; E6 runs both.
  - **Unload of a suspended script.** A script that is suspended when it is unloaded
    (panic chord, watchdog, revocation) gets no `Unload` handler: its unload is the state
    close alone, so no script code runs after the chord until the user resumes it (§5.5).
    Its `Store` writes already made are flushed as at any unload (§8, "`Store` I/O").
  - **Client exit.** Like IB D8's plugin exit unload: after the last frame, before the
    renderer and window are destroyed, each ext apartment's `Unload` handler runs with its
    deadline, then the state closes. No frame follows, so the frame rule does not apply.
    Every script's pending `Store` writes and buffered log lines are then handed to their
    I/O queues, and the client waits for those queues to finish (at most 16 MiB of
    `Store` values per script and one log buffer each) before it exits; what happens
    on a crash is in §8 ("`Store` I/O").
  - **Session end.** When the session disconnects, ext apartments unload as at a backend
    switch.

### 5. Security and trust

#### 5.1 Rights are an intersection (graft from B)

The effective rights of a script are manifest request ∩ user grant ∩ backend class (§6) ∩
server policy (§6). Each layer can only remove rights. Natives check the grant **again on
every call**, so a module table smuggled through `Store`, a global or a closure is useless
once a grant is revoked or the backend changes.

#### 5.2 Prompts and grant pinning

- **First enable.** One dialog lists the requested capabilities in plain words, in the two
  groups of §3, each with the manifest's `reason`. The user may grant a subset. A script
  that lacks a capability it did not mark optional does not load.
- **Combinations.** The dialog also warns about grant pairs that are dangerous together,
  whatever the groups of their parts, and about single grants that are dangerous on a
  server:

  | Combination | Warning |
  |---|---|
  | `entity.read` + `overlay` | Can draw markers on entities through walls. Harmless offline; on a server it shows every entity the server sends, seen or not (§6.5) |
  | `entity.read` + `ui` | The same through world labels placed at entity positions, or a radar or list in a window (the nearest-entity reference script is one). Harmless offline; on a server, as above |
  | `entity.read` + `audio` | The same by sound: an alert for an entity the character cannot see. Harmless offline; on a server, as above |
  | `entity.read` on a server, alone | Even with no output module, a script can write what it reads to its script log, which another program can follow as a radar (and the developer console shows it in game). Only a server policy that leaves `entity.read` out stops this (§6.5) |
  | `session.server` + `files.pick` or `ui` | Can send what you pick or what you type to the server's operator |
  | `input` + `input.drive` | Can bind keys that drive your character |
  | `camera` on a `TesseraProtocol` server | Can move the view anywhere, including places your character cannot see; the server's policy decides whether it is allowed at all (§6.5) |
  | `camera` + `entity.read` | Can point the view at entities automatically |

- **Local-only capabilities are refused with `session.server`, not warned about.**
  `data.read` (decoded data of the user's install) and `bridge.plugins` (the user's
  plugins and their data) are **local-only**: what they return must not reach a server's
  operator. Decoded data sent there would be an export to a third party, contrary to
  "no export" (§3 `Data`, §5.6). Four rules close every
  route that one script could use:
  1. **Per script.** No grant holds a local-only capability together with
     `session.server` (§2, "Mutually exclusive capabilities").
  2. **Per session.** `data.read` is offered offline only (§3), so no apartment of a
     `TesseraProtocol` session holds it. An offline session can hold `data.read` in one
     script and `session.server` (over the in-process loopback, §3) in another; what
     those two can pass to each other is the cooperating-programs residual below. This
     rule is about live apartments only; sessions do not isolate what persists (below,
     "What remains").
  3. **Across a change of grants.** A script's grant can change between loads (a new
     manifest, or the user switching an optional grant in the script manager). A grant
     change between a local-only capability and `session.server` takes effect only at a
     full unload and load, never on a live apartment: switching it in the script
     manager unloads the script first. A `Store` is **opened** when an apartment is
     loaded for its (kind, id), before its top-level run, whether or not the script
     ever calls `Store`. A store opened by an apartment holding a local-only capability
     is marked **local-only** (the marker file of §2), and the mark is cleared only by
     wiping the store. The marker is written **durably** (written, flushed to disk with
     its directory entry) at that load, before the apartment's top-level run and so
     before any value file of a local-only holder can be flushed; a crash can therefore
     leave a marked store with no values, never values with no mark. Before an apartment
     holding `session.server` opens a local-only store, the host wipes it. The grant
     dialog that the change raises says "This script's stored values will be erased",
     with no option to keep them.
     - **No write can land after the wipe.** `Store` writes are written behind, on a
       worker (§8), so a write of the old local-only apartment could otherwise still be
       pending or in flight when the next load wipes and opens the store, and land in
       the unmarked store afterwards. Every file operation on one (kind, id) store
       therefore goes through **that store's own I/O queue**, which runs its jobs one at
       a time in the order they were queued (§8, "`Store` I/O"): reads, write-behind
       flushes, the open with its marker, the wipe and orphan removal alike. At unload,
       in the same frame as the apartment's close item and before the apartment counts
       as gone, the host queues one final flush of every pending write of that
       apartment; jobs already queued or running are ahead of it. A later load's open
       (with its marker or its wipe) is queued after all of them, and its snapshot
       result is delivered only when that open has completed. So the wipe always comes
       after the old apartment's last write, and the new apartment reads nothing that a
       local-only holder wrote. A §6.6 test switches the grant with a write pending and
       another one in flight.
  4. **Across a hot reload.** A reload whose manifest changes the capability list is a
     full unload and load with no hand-off table (§10), so no hand-off table crosses a
     change of grants.

  The bridge cannot launder data through plugins either: a script that runs a plugin
  command with decoded data as its arguments may lead that plugin to save them in plugin
  data, but reading plugin data needs `bridge.plugins`, which is itself local-only, so
  the reader can never hold `session.server` and its own `Store` is local-only.

  **What remains, stated exactly.** No module sends local-only output off the machine, and
  no single script can **on its own**. Programs that the user installs and runs together
  could still pass data to each other, and one of them could later send it to a server:
  - **two or more ext scripts**, through engine state that one can change and the other
    can read (for example the game clock set by `Sandbox` and read by `World`, or a
    native panel's position), or through the shared pools that answer "busy" to one
    script according to what others use (the global caps on rays, entity queries, UI
    elements, subscriptions and outstanding worker requests, §8, which are four or more
    times the per-script caps, so four scripts can exhaust one);
  - **an ext script and a plugin.** A `bridge.plugins` script can pass decoded data as
    the arguments of a plugin's command; an ordinary plugin (an alias or macro plugin)
    can keep them, or turn them into an `Alias` quickslot that the user later clicks
    while on a server, and a plugin can signal to a `session.server` script through
    frame timing or window layout (§5.6, the layout residual). The plugin reads nothing
    of the user's install that the plugin API does not already give it, but it is a
    cooperating party all the same.

  That is a covert channel between programs the user chose to run, as between any two
  programs on a computer; Tessera neither offers nor documents it, and
  this record's §12 records it.
  The channel also **persists across sessions**: shared quotas that survive a restart
  (the 256 MiB and 16,384-key `Store` totals, orphans included) answer "full" to one
  script according to what another stored, so an offline `data.read` script could pass
  bits to a later `session.server` script days afterwards, and plugin data persists by
  design. Rule 2 does not prevent this.

- **No other per-script state persists.** Apart from the grant record and the `Store`, the
  host keeps **nothing** for a script across loads that the script can influence: no
  window geometry or visibility, no last active context, no lease or UI preference. A
  script that wants its window where the user left it saves the geometry in its own
  `Store`, which the wipe of rule 3 covers. What the host does keep across loads is set
  only by the user (the grants, the `[[settings]]` values, the keymap bindings, the
  script list and its autoload flags), and no script call can write any of it.

- **Grant store.** Grants live in Tessera's settings, keyed by **(folder kind, id)**, so a
  developer copy and an installed copy never share grants. Each grant records:
  - the manifest's capability list and `author`;
  - the **content hash** of the snapshot (§2): SHA-256 over this canonical encoding:
    the ASCII tag `tessera-ext-content-v1` and a zero byte; the file count as a 32-bit
    little-endian number; then, for each file in byte-wise order of its relative path, the
    path's length (32-bit little-endian), the path bytes, the content length (64-bit
    little-endian) and the content bytes. Every boundary is length-prefixed, so bytes
    cannot move between a path and a content, or between files, without changing the
    hash. The manifest is one of the files.
- **When grants are asked again (graft from B):**
  - a changed capability list or a changed `author` asks again for all of them;
  - a changed content hash keeps group O grants, shows a notice that names the kept grants
    with a "Review" button, and asks again for every **group A** grant before the script
    next loads;
  - **exception:** scripts in `data/scripts-dev/` with developer mode on keep their grants
    across content changes, so hot reload stays cheap. Developer mode shows a permanent
    badge. Developer scripts still need their grants once; nothing is granted without a
    prompt. Hot reload exists **only** for this kind (§10): `/ext:reload` of an installed
    script is a full unload, a content check (with the re-ask above when the hash
    changed) and a load, so new code of an installed script never runs under grants
    that were given to older code.
- **The `Store` belongs to the grant record.** A (kind, id) has one grant record and one
  `Store`, created and removed together:
  - a changed `author` asks again for every grant (above) and starts an **empty** `Store`;
    the old one is wiped;
  - a changed content hash or capability list with the same `author` keeps the `Store`
    (an update of a note-taker keeps its notes), except rule 3 of the local-only rules
    above. The content-change notice names the kept grants **and** the kept stored
    values, and its "Review" page has a "Reset" button that clears both;
  - until signing (§5.7), a new folder that copies an installed script's id, `author` and
    capability list cannot be told from an update, so it keeps group-O grants and the
    `Store`, and the notice is what tells the user. With signing, a different key is a
    different script: empty grants and an empty `Store`;
  - **orphans:** when a script's folder is gone, its grant record and `Store` stay (a
    reinstall keeps them), and the script manager lists them with their size and a
    "Remove" button that deletes both (the store on its own I/O queue, on a worker) and
    the id's bindings in the ext keymap document (§2). Orphaned stores count toward the
    256 MiB total;
    a write refused by the total names the orphans in its report, so the user can free
    the space.
- **What pinning covers.** The hash pins the code: no ext apartment can compile a string
  (§1.2), so the code that runs is the snapshot that was hashed. The one exception is the
  developer console's REPL, which binds only to `data/scripts-dev/` apartments, where the
  person typing is the developer (§10). It does not pin behaviour
  that the code derives from data. A script whose shipped code interprets data it receives
  later (an interpreter written in Lua) behaves as that data says; that design is visible
  in its shipped code, and the grants it needs to receive such data (`session.server`,
  `files.pick`, `bridge.plugins`) are group A or warned about above.
- **The grant store is hostile input** (users edit settings files), so it is parsed bounded
  and gets a fuzz target.

#### 5.3 Prompts cannot be spoofed (graft from B)

- Consent prompts, the lease indicator and the **script manager** draw in a **reserved top
  layer**: composed above every root panel (native panels, plugin windows at any z-order,
  ext windows) **and above the developer overlay** (the egui pass of 0015 Decision 1.6,
  which holds the ext developer console of §10), below only the software cursor. So the
  composition order after the world becomes: the UI pass, the developer overlay, the
  reserved layer, the software cursor (Consequences, 0015). While a consent prompt
  shows, the developer console is hidden and takes no input, so text a script prints
  there can never sit over a prompt or take its clicks. Ext UI and plugin UI cannot cover
  it, read it or focus it. Hit tests try it first. A consent prompt is modal: while it shows, it
  holds keyboard focus. A prompt raised outside step 1 is shown at the start of the next
  frame's redraw, immediately before `offer_pending` (§4, "Ext UI, z-order, activation
  and focus"), so the focus change it causes never runs plugin code inside ext time. The
  script manager takes no activation or focus unless the user clicks it.
- **What a script can cause in the reserved layer is hit-transparent.** No script call
  raises a consent prompt (prompts come only from loads that the user or autoload starts)
  or opens the script manager (only the user's `/ext:list`, `/ext:help` or a Tessera
  key opens it). What a script can cause there, the lease indicator and notices
  (suspension, unknown `/ext:` id, `Store` wiped), is **hit- and hover-transparent**: hit
  tests and hover refresh pass through as if it were absent, and it takes no activation
  or focus. So it never occludes a plugin control for input. Its Restart and Disable
  buttons live in the script's own windows and in the script manager, not in the notice.
  Notices are rate-limited per script and message (§8).
- **Script-supplied text cannot pass for Tessera's.** The prompts, notices, the lease
  indicator and the script manager show manifest strings (`name`, `description`,
  `author`, each capability's `reason`). They are drawn as **plain text**: no markup is
  interpreted, control characters and Unicode bidirectional controls (U+202A–U+202E,
  U+2066–U+2069, U+200E, U+200F, U+061C) are removed, and each string is cut at a fixed
  length (`name` and `author` 48 characters, `reason` 160, `description` 512) with an
  ellipsis. Next to every free-form name the layer shows the script's **id and folder
  kind** in a fixed style ("notes · installed", "notes · developer"), and the prompt's
  title and buttons are Tessera's own strings, never the script's. So a script named
  "Tessera" or named after another script cannot pass itself off in the trusted layer.
- **The same rules for every string that does not come from Tessera.** Manifest strings
  are not the only text a script or a plugin can influence there: error messages and
  log lines in the script manager, command names in `/ext:help`, settings labels, and
  the id echoed by the notice "No loaded extended script '…'" (which a plugin's `Alias`
  quickslot can supply, §7) are all drawn as plain text with control and bidi
  characters removed and a length cap (an id or command word 32 characters, an error
  or log line 256, a label 48), always inside a frame or prefix that is Tessera's own
  string ("Error in notes · installed: …"). The script manager and the console show
  log text the same way. No reserved-layer string comes from a script, a plugin or a
  typed line without these rules.
- **Prompts appear while the user is playing** (an autoload at world entry, a re-ask
  after a content change), so a key the user is pressing at that moment must not answer
  them:
  - **"Allow" needs a pointer click** on its button: a press **and** a release, both with
    the pointer on the button, and the press starting at least 500 ms after the prompt
    appeared. No key activates it: not Enter, not Space, not a focus move followed by a
    key. Every button of a prompt ignores input for its first 500 ms on screen (a
    Tessera constant), and a press that began earlier (or off the button) never
    completes a click, so a click already on its way does not land on it.
  - **Against lured clicks, not only accidental ones.** The delay defeats a click
    already on its way, not a user whom a running script keeps clicking rapidly at the
    place where a prompt will appear (an autoload or a re-ask prompt for another script
    of the same author). So, in addition: the press counts only if the pointer has
    **entered** the button after its 500 ms ended (a pointer move from outside the
    button to inside it, seen after the delay; a pointer resting on the button when
    the prompt appeared does not count until it leaves and re-enters), and while a
    prompt shows, ext windows, world labels and overlays are not drawn or hit-tested
    (§4), so nothing a script draws stays beside the prompt to aim the user. No script
    can raise a prompt or choose when one appears (prompts come only from user-started
    loads and from the autoload at world entry, which no ext action can cause, §4
    "Lifecycle").
  - Keys may only refuse: Escape is "Deny" (or "Not now" for a re-ask), the safe
    direction.
  - **Which contexts.** While a prompt shows, the client pushes `ui.dialog` for the
    prompt (owner: the prompt, priority above every other band entry) and
    `ui.eat_keyboard` (`opaque_keys`, IB D6). So a key no UI-band context takes is
    swallowed at the band end, as with a focused text box, and pushing the first
    `opaque_keys` entry releases the game-band and `pointer` holds whose last stroke is a
    key (IB D6): a user walking with W stops walking when a prompt appears, rather than
    walking on unseen. Mouse buttons, the wheel and axes pass as D6 says. Both entries
    are popped when the prompt closes.
- Its surfaces are few (at most 8) and fixed in size, and are exempt from the surface and
  memory caps (§8), so no amount of plugin or ext UI can make a prompt draw nothing.
- The game's plugin manager panel stays an ordinary native panel, exactly as M5 builds it.
  Scripts are managed in the separate script manager, so M5's plugin manager, UB's
  activation rules and the plugin-visible z-order are unchanged.
- No module synthesises raw input or presses UI controls. `Input.Drive` produces movement
  intents and standard-action presses as game-band action state only: no key events,
  never UI-band commands, never pointer events, never offered to the UI (§3).
- So no script can click "Allow". An escape test checks this.

#### 5.4 Leases (graft from B)

- Every override is a lease: the camera (`Camera.Take`), movement (`Input.Drive`), and later
  the avatar.
- **Lifetime.** A lease has an expiry time. Taking it, renewing it (`lease:Renew()`) or
  issuing an intent through it sets the expiry to now + **350 ms** (a Tessera constant).
  The client checks expiry at the start of the step that applies the lease's intents.
- **Throttled and starved scripts.** Lease holders are ticked first in 6b (§4). A
  throttled script's half-rate tick at 8 frames per second comes every 250 ms, so 350 ms
  leaves 100 ms for frame-time jitter, and throttling alone does not drop a lease that
  the script keeps renewing at 8 frames per second or more. If lease holders together
  use the whole 2 ms cap, or the heavy-frame gate (§4) skips frames for longer than 350 ms,
  one of them can go unticked for longer than 350 ms; its
  lease then expires. **Expiry under starvation is the accepted fail-safe:** control
  returns to the user, which is the safe direction, and the script can take a new lease
  at its next tick.
- **A lease ends** at the first of:
  - its expiry passing without renewal (a stalled, looping or aborting script stops
    renewing, so its lease ends at most 350 ms after the last renewal);
  - a **watchdog stop**: the host suspends the script under §8 (abort count or the
    rolling-window rule). The lease ends in the same frame;
  - unload, revocation of the grant, a backend change, or the panic chord, in the same
    frame;
  - **the game window losing keyboard focus** (the user switching away, or an OS file
    dialog of `Files.Pick` opening), in the frame the client learns of it. While the
    window has no keyboard focus it gets no key events, so the panic chord cannot reach
    it; no lease runs while the chord cannot end it. A script may take a new lease once
    the window has the focus again.
- When it ends, control returns to the user's controller at the next step that applies it.
- While any lease is active, an indicator in the reserved layer names the script.

#### 5.5 Panic chord and ext contexts (graft from B)

- A reserved chord suspends every ext script, ends every lease, drops every pending ext
  intent (§4) and clears every ext overlay. It also stops every queued 6a item that
  would run a suspended script's code (a pending top-level run), and an unload of a
  suspended script skips its `Unload` handler (§4, "Lifecycle"). So after the chord no
  ext script code runs at all until the user resumes that script in the script manager;
  only native work (compilation, state closes, `Store` flushes) continues.
- **A new mapper tier, active only while ext is present.** While the ext host has at
  least one loaded apartment (running or suspended) or one live lease, the chord is
  tested at each press **before any event of this press is resolved or offered**:
  before IB D7's `handle_with` steps 1 and 2 (which offer the end queue and a pending
  move), before KM §7.2 step 1 (the pending sequence), before every IB D5 UI-band entry
  and before game contexts. The press that
  completes it is consumed: it resolves nothing else, is offered to nobody, and a pending
  sequence is dropped. Its buttons stay in the held set for press suppression and the
  64-button cap. So no script, plugin or focused text box can consume it. While no ext
  script is loaded, the tier does not exist and every press resolves exactly as M5
  defines; a user with no ext scripts sees no change at all.
  - **D7's own steps still run.** The end queue and the pending move that D7's steps 1
    and 2 offer belong to **earlier** events, not to the chord press. So the chord test
    decides first, and then, in the same `handle_with` call, steps 1 and 2 run exactly
    as D7 defines (every offered Start still gets exactly one End, and the pending move
    is still offered before any later event); only the chord press itself is then
    consumed instead of going on to D7's later steps. A consumed chord press therefore
    changes nothing in D7's ordering; it only removes one press.
- **Checked against every binding when it is set.** When the chord is set (the default
  at first start, or a change in Tessera's settings) and whenever the keymap changes
  (an imported original key map, a user edit), the client checks the chord against
  every binding of every built-in, imported, user and plugin context, active or not, in
  pass 1 **or** pass 2: so an imported `F12` binding, which matches `Ctrl+Shift+F12` by
  modifier subset (KM §12.12, IB D2), collides, and so does a user binding on
  `Ctrl+Shift+F12` itself. The settings page refuses a colliding new chord. A collision
  found later (a keymap import) is shown in the binding UI and the script manager with
  a request to pick another chord; until the user does, the chord keeps working, so
  safety never depends on the keymap. The remaining effect is listed in §7.
- It is rebindable only in Tessera's own settings, never through the keymap API that ext
  scripts use.
- Proposed default: Ctrl+Shift+F12. Ctrl+Alt+Backspace is avoided because some Linux
  display servers kill the session on it.
- The script manager resumes suspended scripts.
- **Ext contexts resolve after every built-in binding (amends KM §7.2 and IB D4).** A
  priority below `global` is not enough. KM §7.2 step 2 runs two passes, exact-modifier
  matches first and subset matches second, so an ext exact binding `Shift+W` would win
  pass 1 before the built-in `move_forward` (`W`, subset policy) is tried in pass 2, and
  would take walking forward while `walk` (a `Shift` modifier hold) is held. The rule is
  therefore:
  - Every ext context is registered in the game band with a priority in the reserved range
    −1,000 to −1 (below `global` at 0, KM §8), is never `opaque`, `opaque_keys`, `text` or
    `pass_through`, and is ordered only among ext contexts.
  - **Resolution order of a press:** the panic-chord tier (while present); KM §7.2 step 1
    (a pending sequence); pass 1 and then pass 2 over every active **non-ext** context
    (the UI band and the game band, in IB D5's order, with IB D4's pass-through rules);
    and only then, if the press is still unresolved, pass 1 and then pass 2 over the
    active **ext** contexts. A press is unresolved when neither non-ext pass consumed it
    **and neither non-ext pass ended early** at a stop:
    - an `opaque` non-ext context ends the pass there (KM §7.2). No built-in context is
      `opaque` (IB D6); a plugin's or the user's context may be;
    - **an `opaque_keys` band end (IB D6):** a key or pad press that no UI-band context
      took while any UI-band entry is `opaque_keys` ends the pass at the band end. D6
      swallows it after both passes, and it **never reaches the ext passes**. The
      `opaque_keys` contexts are `text_entry` (a focused text field: the chat input, a
      plugin `TextBox`, an ext text box) and `ui.eat_keyboard` (a consent prompt, §5.3).
      So an ext binding on K or F5 never fires while the user types in chat or in a
      plugin's text box. Mouse buttons, the wheel and axes are not stopped by
      `opaque_keys` (D6) and go on as usual.
  - **Resolution order of a release (`on = "release"` or `"tap"`, IB D4).** Release-class
    resolution follows the same order: D4's two passes over every non-ext context first,
    with D4's pass-through rules and its "release after a UI-owned press" rule, and the
    two passes over the ext contexts only if no non-ext context fired a release-class
    binding that ended the resolution, and neither pass ended at an `opaque` context or
    an `opaque_keys` band end. A pass-through context whose release pulse the UI declined
    continues past as D4 says, so it does not block the ext passes, and it has already
    fired. After a UI-owned press, D4 considers only UI-band and pass-through contexts;
    ext contexts are neither, so such a release never reaches them. Example: an exact ext
    `Shift+MouseLeft` with `on = "tap"`, released over the world, is tried only after
    `pointer`'s Subset `tap_left` has fired in pass 2 (declined over the world) and the
    game band has had both passes, so `tap_left` is never blocked.
  - Analog axes (KM §7.5) already go to the first context that binds them, and ext contexts
    come last, so an ext axis binding gets an axis only when no active non-ext context
    binds it. Modifier holds (KM §7.4) consume nothing, so an ext modifier hold takes
    nothing from a built-in one; while an `opaque_keys` entry is in the band, D6 stops
    the evaluation of game-band modifier holds, ext ones included.
  - Ext actions have no command name, so they are never offered to the UI and plugins
    cannot see them (IB D2). A press or release that an ext binding takes is one that
    no non-ext context consumed and that no stop swallowed, so it would have resolved to
    nothing and reached no plugin. The one exception is the next press after the first
    stroke of a sequence that the user bound in an ext context (KM §7.2 step 1, below);
    §7 lists it as a channel caused by the user's own keymap edit.
  - **Ext context changes never disturb a non-ext pending sequence (amends KM §2, KM §3
    and IB D5).** KM §2 drops the pending sequence at every run-time registration, and
    KM §3 with IB D5 ("on a game-band context, `activate` keeps today's rule") drops it
    at every game-band activation. Under those rules a script that toggled its own
    context every frame would cancel every user-bound or imported sequence (for example
    `Ctrl+K Ctrl+C`) before it could complete, and plugins would see the missing
    command. So the host's registration and unregistration of a script's actions and
    contexts (at load and unload, §2) and every activation or deactivation of an ext
    context drop a pending sequence **only** when it belongs to a context of that same
    script that the change deactivates or unregisters. A pending sequence of any non-ext
    context, or of another script, is kept; the recompilation keeps its candidates as KM
    §2 keeps the holds of unchanged bindings. Ext context changes are also rate-limited
    (§8) and coalesced per frame, so they cannot churn the context stack either.
  - **Collision**, for suggested defaults: an ext suggested binding collides when its
    stroke would be matched, in pass 1 **or** pass 2, by any binding of any built-in or
    plugin context **whether that context is active or not** (so `fly`'s `C` collides
    in walk mode too), including subset matches with extra modifiers (so `Shift+W`
    collides with `W`), and **whatever the `on` of either binding**: a suggested `tap`
    on a stroke that a built-in binds with `press` collides, and so does the reverse. A
    colliding suggested default is withheld at registration; the binding UI shows the
    conflict if the user binds it by hand. Suggested defaults are single strokes and may
    carry any `on` that KM allows; a sequence can only be bound by the user.
  - **The guarantee, stated exactly:** an ext binding fires only for a press or release
    that no active non-ext context consumes, that no `opaque` context or `opaque_keys`
    band end stops, and (for a release) at which no non-ext release-class binding ended
    the resolution. While a built-in context is inactive (`fly` in walk mode), an ext
    binding the user made by hand on one of its inputs does fire, as the user chose. A
    pending sequence that the user bound in an ext context takes the next press in KM
    §7.2 step 1, as any user-bound sequence does; a push of the first `opaque_keys`
    entry drops it (IB D5), so it cannot be completed by typing. No ext context change
    drops a non-ext pending sequence (above). So an `input` grant cannot take over or
    disrupt movement, camera, menu or sequence controls and never sees typed keys; only
    the user's own keymap edits can put an ext action on such an input.

#### 5.6 Never offered (graft from B and A)

`docs/specs/ext-api.md` lists these, and no module, slice or grant provides them:
- **Code.** No compiling of strings: no `load`, `loadstring` or any equivalent. No binary
  chunks or bytecode. The only code is the script's own folder snapshot.
- **Files and the network.** General file paths outside the script's folder and its
  `Store`, other than the `Files.Pick` dialog. No sockets, HTTP, processes, native code or
  FFI.
- **The install and game data.** No write access to the install. No raw archive bytes and
  no enumeration by DID range. No export of decoded assets: no module sends decoded data
  off the user's machine, and no single script can on its own (the four local-only rules
  of §5.2); the only file output of it is the script's own capped `Store` and its capped
  script log, both in Tessera's directories. §5.2 states the one residual (cooperating
  programs: other ext scripts, or plugins).
- **Other software's state.** No state of other ext scripts: their `Store`, their
  apartments, their random generators (§1.2), their windows' content and their
  client-local entities (§3 `Entity.Local`) are out of reach. No state of plugins, **except**
  the group-A `bridge.plugins` exception of §7: the list of loaded plugins (name, version,
  author, as the plugin manager shows them) and read-only plugin data of a named plugin.
  No content of UI nodes the script does not own: no scraping of plugin windows or native
  panels.
  - **One residual: layout, not content.** Ext windows lie below plugin windows and native
    panels, and a point reaches them only where nothing above catches the pointer (§4).
    So a large, even transparent, ext window that receives pointer notifications learns,
    as the user moves the mouse, **where** plugin windows and native panels catch the
    pointer: their positions and mouse-visible shapes, never their content. To keep this
    small, an ext window hears pointer moves and hover changes only over its own
    mouse-visible elements, at most once per frame, as coordinates inside that element;
    a window's transparent, non-mouse-visible background receives nothing. A script
    whose window covers the screen with a mouse-visible element still learns that
    layout; this is stated here, and it is a channel of layout only.
- **Input and the user.** No synthesised raw input. No clipboard **API**: no module reads
  or writes the clipboard. Native text editing in an ext text box keeps the usual
  Ctrl+C, Ctrl+X and Ctrl+V (and the platform's equivalents) as **user gestures**: a
  paste puts clipboard text into the box, where the script can read it as the box's
  text, and a copy puts text the user selected in the box onto the clipboard. Both
  happen only by the user's keys in a box the user focused (§4); `ext-api.md` documents
  them, and no script call can trigger either. No OS identity: no user
  name, home path or host name. Paths reach a script only relative to its folder (chunk
  names, error messages, tracebacks), and `Files.Pick` gives a basename only.
- **Rendering.** No custom shaders or render stages. `PassHook` stays crate-private, and a
  shader tier needs its own decision.
- **Server-delivered code.** No code is ever pushed from a server to a client. This is
  enforceable: a received message is data, and no ext apartment can turn data into code.

#### 5.7 Signing (slice 4)

- An optional detached Ed25519 signature over the content hash.
- Trust on first use: the first key seen for a (kind, id) is pinned. A different key means a
  different script, with empty grants.
- A signature proves authorship only. It **never** raises trust or skips a prompt.
- Unsigned scripts work and are labelled "unsigned".
- Tessera hosts, bundles and downloads no third-party scripts.

### 6. The backend gate and the official-server rule (decision 0004)

1. **Backend class.** A `Session` in `tessera-net` gains `fn class(&self) -> BackendClass`
   with:
   - `Offline`: the in-process sandbox;
   - `TesseraProtocol`: the backend that speaks Tessera's own protocol (M7). Under
     decision 0004 its
     target is the **project-owned** test server; the client cannot tell who runs the
     server behind it (§6.5);
   - `Other`: the default, and anything added later.

   **The class is decided by `Session`, never reported by a backend.** A public trait
   method that returned the class would leak by delegation: any crate that holds a
   sandbox backend (a wrapper, a recorder, a hybrid backend) could call `inner.class()`
   and return the `Offline` value it got, whatever token the value carries. So the
   `Backend` trait gets **no** class method. Instead:
   - `tessera-net` has a **sealed, crate-private** trait, implemented only for its own
     concrete sandbox backend (class `Offline`) and, from M7, its own Tessera-protocol
     backend (class `TesseraProtocol`). Nothing outside `tessera-net` can name or
     implement it;
   - `Session` records the class when it is constructed: `tessera-net`'s constructors
     that take one of those **concrete types by value** record its class; the general
     constructor that takes any `Box<dyn Backend>` records `Other`, whatever that box
     holds. A wrapper around a sandbox backend is therefore `Other`;
   - `BackendClass` is a plain enum for matching; holding one grants nothing, because
     the ext gate reads the class only from the `Session` it gates.

   So a backend in any other crate (a compatibility backend included), or any wrapper of
   a `tessera-net` backend, can only run in an `Other` session; the M7 Tessera-protocol
   backend therefore lives in `tessera-net`. It is an additive change to
   `crates/net/src/session.rs`.
2. **Fail closed, no override.**
   - The ext host is active only while the session's backend class is `Offline`, or
     `TesseraProtocol` **from slice 3 on**. Until slice 3 ships the script-policy
     handshake and its gating, the gate admits `Offline` alone, whatever M7 ships first:
     no ext script ever runs on a Tessera-protocol server without a policy. The slice-1a
     gate therefore treats `TesseraProtocol` like `Other` (unload before `Connect`).
   - Before `Command::Connect` on any other backend, the client unloads every ext apartment
     (§4, "Backend switch") and creates none until that session is closed.
   - No setting, flag or environment variable changes this.
3. **Build-level exclusion (graft from A and B).**
   - **The compatibility backend must be its own crate**, never a module of `tessera-net`.
     `crates/net/src/compat.rs` is reworded to say so (Consequences), and ARCH §19.3 says
     that a compatibility backend is a separate crate that
     depends on `tessera-net` and is **excluded from the workspace** (`[workspace]
     exclude`), like the binary that uses it (below). A workspace member could not hold it
     anyway: the `links` guard below makes the workspace's single resolve fail.
   - **The marker.** That crate declares `[package.metadata.tessera] tier = "gated-compat"`
     in its manifest. The check matches this tag, not a crate name.
   - **How a compatibility binary is built.** Never as a feature of a workspace package:
     the check below resolves graphs under `--all-features`, so a client feature `compat`
     beside the default-on `ext` would always fail it. And never as a workspace member:
     `cargo build --workspace` unifies the features of `tessera-client` across members, so
     a member that depends on the client without `ext` would still get `ext` linked in
     whenever another member enables it, while a per-package check passed. A compatibility
     binary is therefore **its own package listed in the root
     manifest's `[workspace] exclude`**, built from its own directory with its own lock
     file, and depending on `tessera-client` with `default-features = false`.
   - **A hard guard in Cargo itself.** `tessera-ext` and every `gated-compat` crate declare
     the same `links` value (`links = "tessera-ext-xor-compat"`, with an empty build
     script, which Cargo requires for `links`). Cargo allows one package per `links` value
     in a dependency graph (the mechanism that keeps two Lua versions apart, Context), so
     Cargo refuses to resolve any graph that holds both, whatever the features. Slice 1a
     confirms this with a synthetic fixture in the check's tests, which also proves the
     other half: a synthetic excluded "compatibility binary" that depends on a synthetic
     "client" with `default-features = false` (whose optional `ext` dependency pulls in
     the `links` holder) and on a `gated-compat` package **still resolves and builds**.
     That shows that the optional dependency does not enter the binary's graph through
     its lock-file resolution. If Cargo does count the unactivated optional dependency
     for `links`, the `links` guard is dropped, the build-level layer rests on the check
     below alone (the runtime gate of §6.2 is unchanged), and slice 1a records this here.
     **That is the expected outcome.** Cargo builds `Cargo.lock` from a resolve that
     includes every optional dependency of the packages in the graph, and the `links`
     conflict is detected during that resolve, so the client's optional `ext`
     dependency will most likely conflict with a `gated-compat` package even with
     `default-features = false`. Slice 1a therefore plans for the CI check alone: the
     check, its tests and its CI job are the deliverable, and the `links` fixture is a
     short experiment that keeps the guard only if it passes.
   - **The check** (`tools/`, run in CI) reports the rule in words:
     - it scans every `Cargo.toml` in the repository, workspace member or not;
     - it resolves the **unified graph of the whole workspace** (every member, under
       default features and under `--all-features`) and fails if a `gated-compat` package
       appears in it at all;
     - for each package tagged `gated-compat`, and for every package outside the workspace
       whose own resolved graph (default features, `--all-features`, and each feature set
       it declares for release builds) contains one, it fails if that graph contains
       `tessera-ext` or `tessera-ext-host`;
     - it fails if a workspace package declares a feature that enables a `gated-compat`
       dependency.
   - **No restructuring.** Because the client links ext only through its default-on `ext`
     feature (§1.1), a compatibility build turns that feature off. It changes no client
     code, as decision 0004's consequence requires. The runtime gate (§6.2) still holds in
     every build; the build-level layer is defence in depth.
4. **Official-server boundary (graft from B).** Extended scripts never
   run against an official server, so any M8 design must preserve that boundary explicitly.
5. **Tessera-protocol servers (M7), and the limit of the gate.**
   - **What the gate can and cannot do.** The class is what a backend compiled into the
     build reports about itself. Tessera's own builds keep the rule. A modified build, or a
     proxy run by the user that translates Tessera's protocol to another service, defeats
     any client-side gate: the client cannot know what lies behind a `TesseraProtocol`
     connection, and such a user is their own "operator". This is the limit of the
     threat model. The design answer is to keep automation out of reach of any server:
   - **Offline only, until a separate decision:** `input.drive`, and every later module
     that issues game-state commands or movement on the user's behalf (any `Command`
     other than `Server` messages). No server policy can turn them on. A future decision
     that wants them on a server must address the proxy case above.
   - **On `TesseraProtocol` (slice 3):** the handshake carries the server's **script
     policy**, with two parts:
     - `allow`: the allowed **capability names** (§3). The modules without a capability
       (`Core`, `Store`, `Commands`) need no grant and are always present.
     - `forbid`: **forbidden combinations**, each a set of two to four capability names
       that no one script may hold together, for example `["camera", "entity.read"]`.
       Forbidding combinations does **not** stop through-wall views (below). A script
       that requires every member of a forbidden
       set is refused as a whole (§2, "Refusal is whole"); if some members are
       optional, the optional ones are withheld until no forbidden set is complete.

     If the server states no policy, the default `allow` is every group-O capability
     offered on `TesseraProtocol` (`ui`, `input`, `world.read`, `entity.read`, `overlay`,
     `files.pick`, `audio`; `data.read` is offline only) plus `session.server`, which is
     group A but is the script's channel to that same server, and the default `forbid` is
     empty: the client refuses every local-only capability with `session.server` on every
     backend anyway (§5.2), whatever a policy says. Every other group-A capability
     (`camera`, `ui.native`, `entity.local`, `bridge.plugins`) is off unless the policy
     lists it, and the Offline-only ones can never be listed. Owner question 3
     asks the owner to confirm this default. The client grants the intersection (§5.1).
   - **What a policy cannot do: the real defence is the server's.** Any script that can
     read entity positions and draw anything (overlay markers, world labels at the
     projected points, a radar or a list in its own window) can show the user entities
     that the user's character cannot see. A per-module restriction such as "no world
     labels on entities" would not help, because a label or a radar dot can be placed at
     any point the script computes. Forbidding `entity.read` with `ui` and `overlay` does
     not close it either: `audio` (group O, in the default `allow`) can sound an alert
     for a hidden entity, and `print` and the `Core` log write to the script log, a file
     that another program on the machine can follow as a radar (and in developer mode
     the console shows those lines in game). The **only** client-side measure is a policy
     whose `allow` leaves out `entity.read`, and a modified build ignores even that
     (above). So the
     defence against through-wall views is the server's own **interest management**: a
     Tessera-protocol server sends a client only the entities its character may know
     about. `tessera-protocol.md` (slice 3, with M7) states this as a server
     requirement. The server stays authoritative and trusts no client.
   - **Which servers: the project-owned test server only.** Decision 0004 targets M0–M7
     at "the in-process sandbox backend and a project-owned server". This record stays inside that scope: ext support
     on `TesseraProtocol`, its script policy and its default exist for the project-owned
     test server, and the project neither designs for, documents nor tests third-party
     operators. The client cannot verify who runs a server (a user can point it at any
     address), which is the threat-model limit above, not a supported use. Supporting
     third-party Tessera-protocol servers, for ext scripts or for the client at all,
     needs its own decision that amends 0004
     first (owner question 3). 0016 item 2 ("offline and on Tessera's own server")
     holds unchanged.
6. **Tests.** Slice 1a runs the ones marked **1a**; the others come with the modules and
   the policy they test.
   - **1a:** a stub backend of class `Other`, and in slice 1a also a session of class
     `TesseraProtocol` (built by a test-only constructor inside `tessera-net`), gets no
     ext apartment.
   - **1a:** a mid-session switch unloads all of them before the new session's `Connect`.
   - **1a:** intents never outlive the gate: a script that queues an intent in 6b of the
     frame before a switch is queued has it discarded, and no intent is applied after a
     suspension or unload of its apartment or under a newer session generation (§4). In
     slice 1a the intents are `Input` context activations and deactivations; slice 1b adds
     a `Sandbox` teleport queued just before a switch to an `Other` stub.
   - **1a:** a compile-fail test shows that a crate outside `tessera-net` cannot implement
     or name the sealed class trait, and a test shows that a `Session` built around a
     wrapper of the sandbox backend (a forwarding `Backend` in another crate) reports
     `Other` and gets no ext apartment (§6.1).
   - **1b and 2:** `Input.Drive`, `Sandbox`, `Data` and every Offline-only module answer
     "unavailable on this backend" on a `TesseraProtocol` stub, whatever its policy says.
   - **2 and 3:** the local-only rules of §5.2: a manifest requiring `session.server` with
     `data.read` or `bridge.plugins` is refused; a grant switched from `data.read` to
     `session.server` in the script manager wipes the local-only `Store` before the next
     load opens it; a hot reload that changes the capability list passes no hand-off
     table.
   - **1a (with a stand-in local-only grant) and 2:** the `Store` I/O queue orders the
     wipe after the old apartment's writes. A `data.read` holder writes a key, writes
     another so that one flush is in flight on a worker (held there by a test hook) and
     a second value is pending, and the grant is switched in that frame. The test
     checks that the final flush is queued at unload, that the next load's open waits
     for both writes and then wipes, that the `session.server` holder reads no key, and
     that no file appears in the store after the wipe. Slice 1a runs it with a
     test-only local-only capability, because `data.read` ships in slice 2.
   - **3:** a policy's `allow` and `forbid` sets are applied, and a required capability
     removed by them refuses the script as a whole.
   - **1a:** the CI dependency check fails on a synthetic manifest set where a
     `gated-compat` package and `tessera-ext` share a graph, fails when a `gated-compat`
     package is a workspace member or the target of a workspace feature, and passes for
     an excluded package that depends on the client with the `ext` feature off. Cargo
     itself refuses to resolve the synthetic graph that holds both `links` declarations,
     and resolves the excluded compatibility binary of §6.3's fixture.
7. **Plugins are unaffected.** They are the original's own feature and follow whatever the
   M8 gate decides.

### 7. Coexistence with standard plugins

- **Separate everything.** Folders, apartments, globals, grants, limits, queues, time
  slices, command words and manager windows are all separate. No ext script changes the
  plugins' command table, command lists, deferred-work queue, plugin slice or plugin
  limits.
- **What a plugin can observe.** A plugin cannot detect an ext script except through
  channels the original API already has. This is the complete list for the modules of this
  record; a later module that adds a channel must add it here:
  - the effects of the user's own actions that a script performs through the bridge
    (below);
  - **chat lines a plugin writes when the bridge runs its command:** the bridge runs a
    plugin's registered `/` command with **arguments the script chooses**. A plugin that
    echoes its arguments, or anything derived from them, through `Shell.WriteLine`
    (LPA §11.1) thereby posts a chat line whose text the script controls, and that line
    reaches every plugin's `Turbine.Chat.Received` like any `WriteLine` line. The user
    could type the same line; the bridge rate limit (one action per script per second)
    bounds it, and the grant is group A (slice 2);
  - system chat lines that a `Sandbox` command causes (below);
  - **world state that a `Sandbox` command changes:** a teleport moves the player and a
    clock change moves the game time and sky. Plugins see these through whatever the
    plugin API reports of them (the gameplay views and their `…Changed` events, LPA §15),
    queued as game-raised events and delivered in step 7 item 5 (IB D8), as for the user's
    own teleport;
  - **the user's clicks on ext windows:** a press on an ext window activates it, so the
    active plugin window gets `Deactivated` and its focused text box `FocusLost`, as for a
    press on a native panel (LPA §13.14). A script's own request can never do this (§4,
    "Ext UI, z-order, activation and focus");
  - **an active ext window or a focused ext text box:** after the user's gesture (§4),
    key actions are offered first to the focused ext control or the active ext window
    (LPA §13.13 step 1), as for a native panel, and while an ext text box has focus
    `text_entry` swallows keys that no UI context binds (IB D6). Keys consumed or
    swallowed this way reach no plugin `KeyDown` or `KeyUp` broadcast, exactly as with
    the chat input or a native text field. It needs the user's click on the script's
    window, or the user's press of that script's own action or `/ext:` line; the script
    cannot cause it from the idle state of play;
  - **ext windows and world clicks:** ext windows lie below every plugin window and
    native panel, so they never change what a hit test finds over one. They do take
    pointer presses over the world inside their own area, as any window does, so a large
    (even transparent) ext window can stop world clicks there; plugins observe this only
    as the absence of whatever those clicks would have caused. The panic chord and the
    script manager remove such a window;
  - **consent prompts:** a modal prompt takes keyboard focus, so a focused plugin text box
    gets `FocusLost` when one appears, in step 1 (§5.3), and its `ui.eat_keyboard`
    swallows keys that reach no plugin broadcast while it shows;
  - **the panic chord, only while an ext script is loaded:** the press that completes it
    is consumed before every context and every offer (§5.5), so it reaches no plugin
    `KeyDown` and no binding, even one that an imported key map or the user put on that
    stroke. The client warns about every such binding (§5.5). With no ext script loaded
    the tier does not exist and nothing changes;
  - a plugin can itself build an `Alias` quickslot shortcut (LPA §14.6) whose text is an
    `/ext:` line. Only a user click runs it, so the plugin's chosen arguments reach the
    ext script's command handler, which must treat them as hostile input;
  - **native panels moved by `UI.Native` (group A, slice 2):** native panels share
    z-order and hit testing with plugin windows (LPA §13.14), so showing, hiding or
    moving one changes what plugin hit tests and hover find, as the user's own toggle or
    drag of that panel would. The change applies at once in 6b and reaches plugin hit
    tests and hover at the next hit test or hover refresh, as a plugin's own geometry
    change in step 7 does (§4). A show never activates the panel (§3), so no plugin window
    gets `Deactivated` from it; a panel hidden while active deactivates as when the
    user closes it;
  - **the effects of `Input.Drive` (group A, Offline only, slice 2):** the movement and
    the standard actions it presses change the player's state, which plugins see through
    the gameplay views and their `…Changed` events (LPA §15), as for the user's own
    movement. Its presses are action state, not key events: they reach no plugin
    `KeyDown` or `KeyUp` broadcast and are never offered to the UI (§3).

  - **a pending sequence that the user bound in an ext context:** KM §7.2 step 1 gives
    the next press to a pending sequence before any context is tried (§5.5), so once the
    user has typed the first stroke of a sequence that **the user** bound to an ext
    action, the next press goes to that sequence, even if a built-in binding or a
    plugin `KeyDown` would otherwise have had it. This is caused only by the user's own
    keymap edit (no suggested default is a sequence, §2), exactly as a user-bound
    sequence in a non-ext context takes such a press, and the sequence timeout or any
    `opaque_keys` push ends it.

  **Ruled out, so not channels:**
  - ext key bindings otherwise: a single-stroke ext binding takes only presses and
    releases that no non-ext context consumes and that no `opaque` or `opaque_keys` stop
    swallows, which would have reached no plugin (§5.5), and ext context changes never
    drop a non-ext pending sequence (§5.5);
  - reserved-layer surfaces a script can cause (the lease indicator and notices): they
    are hit- and hover-transparent (§5.3), so they change no plugin hit test or hover;
  - client-local entities (`Entity.Local`): never targetable or pickable, never in a
    plugin-visible gameplay view or entity list (§3);
  - random numbers: ext apartments draw from their own generators, never from PR §4.5's
    shared plugin generator (§1.2);
  - **shared pools:** ext's surfaces, composition entries, UI memory, paint slice, upload
    share and key-registry space are **separate pools on top of** 0015's and KM §2's
    caps (§8). A plugin's window, image or registration therefore succeeds or fails, and
    UR's draw-nothing fallback applies, exactly as with no ext script loaded;
  - **asset decoding:** ext requests (`Entity.Local` setups, `Audio` and `Data` decodes,
    ext images) use their own partition of the decoded-asset cache and the lowest worker
    priority (§8), so they never evict a plugin's or a native panel's decoded asset and
    never queue ahead of one. A plugin request can still wait for an ext job that a
    worker has already started, at most one job per worker; like the frame rate, that
    is timing, not a channel the API reports;
  - ext time, beyond the frame rate that any engine load changes.
- **Chat.** Ext scripts have **no direct route** to post a chat line in any slice of this
  record: every line the chat window shows reaches plugins as `Turbine.Chat.Received`
  (IB D8; for error lines, the Tessera rule of the LPA §6.6 OPEN). So:
  - **Ext reports never go to the chat window.** Error reports, warnings, `print`, `Core`
    log lines, suspension notices and the output of `/ext:list`, `/ext:help` and
    `/ext:reload` go only to the script log, the developer console and the script manager
    (and notices in the reserved layer). This differs on purpose from plugin errors,
    which LPA §6.6 shows as chat type 1 lines; an ext `error("any text")` therefore never
    becomes a line that plugins receive. A typed `/ext:` line is not echoed to the chat
    log, and an unknown `/ext:` id produces no chat line (§3, `Commands`).
  - **Two indirect sources**, both listed above: chat lines that a plugin itself writes
    when the bridge runs its command (slice 2), and sandbox system lines. A
    `Sandbox` command makes the sandbox emit the same system lines it emits for the user's
    own command (a teleport cancelled or failed, a place description). Those lines reach
    plugins like any system line. They are bounded by the `Sandbox` rate limits (§8): at most
    one command per second per script, and an ext teleport request is refused before it
    reaches the sandbox while **any** teleport is pending, the user's own included. So an ext
    request never supersedes another request. A teleport the user issues while an ext
    teleport is pending does supersede it, and the sandbox then answers the ext request with
    its "Teleport cancelled" line (`Command::Teleport` in `crates/net/src/session.rs`). That
    line, and the failure and place lines, are ordinary sandbox lines that the user's own
    commands also cause.
- **One-way bridge (`bridge.plugins`, slice 2; graft from B).** Each action is replayed as
  the user's own action, so plugins see nothing the user could not cause:
  1. **list** loaded and available plugins (what the plugin manager shows);
  2. **load** a plugin: queued as the same load item that the plugin manager's load action
     queues (LPA §3.3), in the **plugin** queue, where it is plugin time like any load;
  3. **unload** a plugin's apartment, and **run a plugin's registered `/` command** with
     arguments the script chooses (a channel listed above): queued for step 1 of the
     next frame and run there through the same routing as the user's
     manager click or typed line, so plugins see an ordinary user action at an ordinary
     time;
  4. **read** a named plugin's plugin data through the plugin-data reader of
     `tessera-plugins` (LPA §10.7), on a worker. There are **no writes**. A read never
     sees a half-written file: it is started only after step 7 item 5 of its frame, and
     the plugin-data layer serialises it after any write of the same file already
     started, so it reads each file as of the last completed write.
     - **Bounded like every value ext receives.** The plugin-data reader's own limits
       (LPA §10.4, and `ParseLimits::default()` in `crates/script`: 16 MiB of file,
       depth 64, 1,000,000 entries) are for
       plugins and far above ext's caps. The bridge port calls it with the **bounded
       value model's caps** of §8 instead: at most 1 MiB of file, 32 levels and 8,192
       elements, and the result must also fit 1 MiB as ext would serialise it. A file
       over any of them is refused as `nil, "too large"` on the worker, before it is
       parsed past the cap. The parsed value is delivered as a notification in the
       script's 6b turn and turned into Lua values inside that metered call, with its
       cost charged first (§4, §8, "Value conversion is not a residual"). A read counts
       as one outstanding worker request of the script (§8).
  - The bridge runs only a plugin's registered `/` command; it never runs a built-in
    command or an `/ext:` line, so no script reaches another script through it.
  - **Rate limits:** at most one bridge action per script per second, at most one load
    outstanding in the plugin queue per script and two for all scripts. So ext can never
    flood the plugin queue or delay `LoadPlugin`, `PluginData` callbacks, game-raised
    events or the logout unload by more than two load items.
  - The bridge port lives in `tessera-ext-host` behind its optional feature `bridge`, the
    only dependency of ext on `tessera-plugins` (§1.1). If the plugin compatibility surface is
    disabled, the feature is off and `Plugins` answers "unavailable on this backend".
- **Excluded.** Plugin-data writes and direct chat-line injection are excluded (the
  indirect line a plugin itself writes for a bridged command is listed above). They would let a
  script forge plugin input and poison plugin data. A two-way bridge needs its own decision. None is proposed.
- **Script manager.** A Tessera window in the reserved layer (§5.3) lists scripts with their
  grants, leases, signature state and profiler line (§8), and resumes suspended ones.
- **Independence.** Building without the plugin compatibility surface leaves ext
  scripting intact. `tessera-ext` never depends on `tessera-plugins`; `tessera-ext-host`
  depends on it only through the optional `bridge` feature, which a build without the
  plugin surface turns off (§1.1); no ext name or module uses the plugin names; and `Store`
  uses the core's neutral reader with its own file names (`.store`, never `.plugindata` or
  `PluginData`).

### 8. Limits and graceful failure

**Work units.** One unit is about 5 ns of work on the reference machine (one VM instruction
takes 4–10 ns at the measured 100–240 million per second). The instruction hook charges one
unit per instruction, and the patched build charges the work of every costly VM operation
to the same counter (§1.5), so the units cover VM work too. Every native, including the wrapped standard-library functions below,
charges its declared cost before it does the work. Costs are part of the binding
description (§1.3) and are calibrated so that charged units × 5 ns is at least the native's
measured worst-case time (E7). A native whose cost exceeds the call's remaining units, or
whose cost × 5 ns exceeds the call's remaining wall time, is refused with a Lua error before
it starts. A native whose exact work is known only as it goes (one that walks a Lua value:
`Store.Set`, the hand-off table, `Server` sends, natives that take tables such as camera
paths and overlay lists) **reserves the cost of its cap** before it starts, which is known
from the bounded value model below, walks with that reservation, and refunds what it did
not use. So "charged before the work" holds for every native. Native work that interns
strings (value conversion, compilation, string-producing natives) charges each interned
string at the 32-node chain bound of §1.5, so its pre-charge covers the worst lookup.

**One budget in two measures.** A call's work-unit cap and its wall-clock deadline are the
same budget: 800,000 units × 5 ns = 4 ms. In the client both apply, and whichever is
reached first stops the call; in deterministic mode (below) only the units apply. Because
every charge is calibrated so that units × 5 ns is at least the worst measured time (E7),
a call that stays within its units in `scripttool test` normally stays within its
deadline on the reference machine. On a slower machine the deadline can stop it first,
which is the intended direction.

**The bounded value model.** Every Lua value that crosses the host boundary as data (a
`Store` value, the hand-off table, a `Server` message, a plugin-data value read through
the bridge, a table argument of a native) is a tree of plain values (nil, booleans,
numbers, strings, tables of those) with these bounds, checked **during** the walk, which
stops at the first bound it reaches:
- **raw access only:** the walk over a Lua value uses `lua_next` and raw gets
  (`rawget`), never `__index`, `__len`, `__tostring`, `__eq`, `__lt` or any other
  metamethod, and ignores metatables entirely (they are not part of the value), so no
  script code runs during a walk, and what is walked is exactly what is stored;
- **valid keys:** a key is a boolean, a finite or infinite number that is not NaN, or a
  string. In the other direction, a parsed `Store` file, a `Server` message or a
  plugin-data file whose key is nil or NaN (possible in a hand-edited file or a hostile
  message) is refused on the worker as a conversion error, `nil, "invalid"`, with a
  script-log entry; never a panic, never a partial value. A duplicate key in a parsed
  file is refused the same way;
- **no cycles and no sharing beyond the caps:** a table met twice on the current path is a
  cycle and refuses the value; a table reachable twice by different paths is walked (and
  counted) each time, so a graph of shared subtables cannot expand past the element cap
  before the check;
- **at most 32 levels** deep, so a `t = {t}` chain cannot overflow the Rust stack of a
  recursive walk (the walk is iterative in any case);
- **at most 8,192 elements** (every key and every value counts one) for a `Store` value, the
  hand-off table and a `Server` message; a native that takes a table declares its own
  element cap in its binding description (an overlay list: its primitive cap of the
  table below; a camera path: 256 keyframes);
- **bytes counted as serialised**, in the file encoding with its escapes: at most 1 MiB for
  a `Store` value and the hand-off table, 64 KiB for a `Server` message, and the walk stops
  as soon as the serialised size passes the cap, so no value is ever expanded in memory
  before its size is known. The `Store` totals (16 MiB per script, 256 MiB in all) are
  counted on the bytes of the files as written.

A value over a bound is refused with `nil, "too large"` (or `"cyclic"`), never truncated.
Reading a value back (a worker parses the file) enforces the same bounds, so a file edited
by hand cannot exceed them either. The same holds for a plugin-data file read through the
bridge (§7): the worker calls the plugin-data reader with these caps (1 MiB of file, 32
levels, 8,192 elements), never with the plugin reader's own larger defaults.

**`Store` I/O.** Every file operation of `Store` runs on a worker, never on the main
thread (ARCH §21), through **one I/O queue per (kind, id) store**, which runs its jobs
one at a time in the order they were queued:
- **the open** at load: it writes the durable local-only marker (written, flushed with
  its directory entry) or, for a `session.server` holder of a marked store, wipes the
  store (which also removes the marker). It runs inside the load's snapshot job, and the
  snapshot result reaches 6a only when it has completed, so neither the marker nor the
  wipe is a main-thread step or a new 6a item;
- **reads**, each answered as a 6b notification;
- **write-behind flushes**: a script's dirty keys at most once per second, each value
  written to a temporary file in the store's directory (relative to its handle, §2) and
  renamed over the key's file, so a crash never leaves a half-written value;
- **the final flush at unload**, queued in the frame of the apartment's close item,
  before the apartment counts as gone, with every value that was still pending; jobs
  already queued or running stay ahead of it;
- **wipes and orphan removal**.

A later load of the same (kind, id) queues its open after all of these, so a wipe always
follows the last write of the apartment before it (§5.2 rule 3). At client exit the
host queues every final flush and waits for the queues to finish before it exits (§4).
**On a crash**, writes not yet flushed, at most the last second of each script's writes,
are lost; every file on disk is either its old or its new value, and the local-only
marker, written before the top-level run, is never missing for values a local-only
holder wrote. The script log is written the same way: lines are buffered in memory
(charged to the script) and appended by a worker, at least once per second and at
unload and exit, never from the main thread. Orphan removal runs on the store's queue
too, so it can never race a load.

**Natives never wait.** The cost of a native must be known before it starts, so no native
does work that depends on what is loaded. `World` rays and point queries, `Entity` queries,
`Data` lookups, `Audio` and `Entity.Local` answer **only from resident or cached data**:
when the data they need is not resident (a landblock not streamed, a record not decoded),
they return `nil, "not loaded"` and, where it makes sense, queue a request. They never
start a synchronous decode, never trigger streaming and never do I/O on the main thread
(ARCH §21). A ray is at most 500 m long, so its cost is bounded by the cells it can cross.
Ext asset requests run at the **lowest worker priority**, below every native and plugin
request, and their results go to an **ext partition** of the decoded-asset cache (16 MiB
per script, 64 MiB for all ext scripts, least recently used first), which never evicts a
native or plugin entry.

**Outstanding worker requests are capped.** Lowest priority bounds the delay a native job
sees, not the queue's length or the worker time ext uses. So every request that ext puts
on a worker, `Store` reads, decode requests from `Data`, `Audio` and `Entity.Local`,
bridge plugin-data reads and `Files.Pick` dialogs, counts as **outstanding** from the
moment it is queued until its result has been delivered or dropped:
- at most **32 outstanding per script** and **256 for all ext scripts**; a call that would
  pass either answers `nil, "busy"` (a `Data`, `Audio` or `Entity.Local` miss then
  answers `nil, "not loaded"` without queueing anything);
- **de-duplicated:** a decode request for a DID, setup or sound that is already pending
  (from any script) joins that request instead of queueing another; it still counts once
  toward the asking script's own cap, and a script asking again for what it already
  waits for adds nothing;
- **charged before it is queued:** a request that will bring a value back to the script
  (a `Store` read, a bridge read, a `Files.Pick` result) reserves the largest result it
  can deliver (1 MiB for a `Store` or bridge value, 3 MiB for a `Files.Pick` file)
  against the script's memory cap before it is queued, and the reservation becomes the
  delivered value's real size, or is released when the result is dropped. Decode
  results are charged to the ext cache partition. So results waiting for delivery can
  never grow host memory beyond the script's cap or the partition;
- `Store` reads are also **rate-limited** (table below), like writes.

So worker queue length, parsed results in flight and worker time are bounded per script
and in total, whatever a script asks for.

**Host-side maps use a keyed hasher.** Every Rust map whose keys a script, a hand-edited
file or a server chooses uses a randomly keyed hasher (std `RandomState`), never FxHash or
a fixed-seed hasher, so hash flooding cannot move from Lua, a file or a message into the
host. That covers the ext host's maps (`Store` keys, command names, subscription filters,
the `Ext.Load` cache, the `tostring` numbering), the core's neutral Lua-table data reader
(its duplicate-key and table maps, for `Store` files and plugin-data files alike), the
`Server` message parser, the ext keymap document's reader, and every de-duplication or
lookup map on a worker path (the outstanding-request map above, the per-store I/O
queues).

**Wrapped standard functions in ext apartments.** The rule: **every standard-library
function whose work grows with its arguments is wrapped, and charges, before it starts,
the largest of the bytes it reads, the bytes it produces and the number of loop
iterations that its arguments imply**, computed from the arguments alone, never from the
result size alone. Arguments for which the stock function would loop without producing
anything (an empty string or a count of zero or less for `string.rep`, an empty range) are
answered by the wrapper itself without calling the stock function, and arguments outside
the range the stock function handles correctly (a position below 1 for `table.insert` or
`table.remove`) are refused with a Lua error, as later Lua versions do. E7 checks every
wrapper, and E6 and E7 include the degenerate arguments. A function absent from this list
may be exposed only if its work is bounded by a constant. Slice 1a starts from this list,
and the registry review adds any function it finds missing:

| Functions | Charged by |
|---|---|
| `table.sort` | n·⌈log₂ n⌉ × 5 units (490 ms for one million numbers, measured) |
| `table.insert(t, pos, v)`, `table.remove(t, pos)` | the number of elements moved, n − pos + 1 with n the length the stock function reads; a `pos` below 1 is refused; the append forms `table.insert(t, v)` and `table.remove(t)` are constant |
| `table.maxn` | the size of the table (it walks the whole table) |
| `table.concat(t, sep, i, j)` | the larger of the result size and the range j − i + 1 (a range of empty strings still loops) |
| `unpack(t, i, j)` | the range j − i + 1 from its arguments, before the stack check |
| `string.byte(s, i, j)`, `string.char` | the number of values (the range clipped to the string, or the argument count) |
| `string.rep(s, n)` | **max(n, result size)**: the stock function loops n times even when `s` is empty. `n` ≤ 0 or an empty `s` returns `""` in the wrapper; a result over 4 MiB is refused before it starts |
| `string.format` | the larger of the result size and the argument count |
| `string.upper`, `string.lower`, `string.reverse`, `string.sub` | result size (equal to or below the input size they read) |
| `string.find`, `string.match`, `string.gmatch`, `string.gsub` | the core's bounded matcher charges its steps, including the plain (non-pattern) search; `gsub` also charges its result size |
| `tonumber` on a string, `print` | input or output size |
| `select`, `coroutine.resume`, the functions `coroutine.wrap` returns, `pcall`, `xpcall` | the number of arguments and results they copy. Lua 5.1's C-stack limit (`LUAI_MAXCSTACK`, 8,000 slots) bounds that count by a constant, so the charge is at most 8,000 units, but it is charged so that the units stay honest |
| `next`, and so every step of `pairs` | a constant per call, **bounded explicitly by the allocation cap**: one call may scan every empty slot of a sparse table, so its worst case is one scan of the largest table the 4 MiB cap allows (2^18 array slots plus 2^16 hash nodes). The patched build charges the slots it skips (§1.5), so the hook fires right after such a call. That worst call is part of R below and is measured by E6 and E7 |
| `collectgarbage` | `"count"` only, constant |
| `math.random`, `math.randomseed` | constant (the per-apartment generator, §1.2) |

**Wrapping reaches every route to a function.** String values find their methods through
the string metatable's `__index` (`(""):rep(n)`). The ext profile builds its `string`
table from the wrappers and sets the string metatable's `__index` to **that same table**
before any script code runs, so `s:rep(n)` and `string.rep(s, n)` reach the same wrapper,
and no table that a script can reach holds a stock function that should be wrapped. A
script may later change `__index` or the `string` table in its own apartment, but only
to values it can already reach. The escape tests check both routes.

Lua operators are VM instructions, not functions; the patched build charges their
operand-dependent work (§1.5), and the one uninterruptible instruction is the residual
below. A Lua-to-Lua call copies its arguments within the same C-stack bound.

| Resource | Default | When exceeded |
|---|---|---|
| Memory per script | 32 MiB. A manifest may ask for up to 64 MiB, and the prompt shows it. The cap covers the Lua heap **and the host objects the script creates**: each element, world label, timer, subscription, queued notification, pending intent, pending `Store` write, buffered log line, outstanding worker request with its result reservation (above) and `tostring` number is charged a declared size (its Rust state, measured in E5) plus its variable bytes (text, strings) through the same accounting as Lua allocations, and released when the object goes | Allocation or creation refused; the script gets a memory error it can catch; reported once |
| Largest single allocation | 4 MiB (one string, table part, buffer or stack) | As above |
| Memory, all ext scripts | 512 MiB as the sum of the caps of the loaded scripts (16 at the 32 MiB default, or 8 at the 64 MiB maximum), plus a separate **64 MiB reload allowance** that holds the new apartment of the one hot reload in progress (§10) | A load that would push the sum over is refused with a report. A reload always fits its allowance, because no cap exceeds 64 MiB |
| Loaded scripts | 16 (a script in hot reload counts once; at most one reload at a time, so at most 17 apartments exist; from slice 3 a script's server-side half counts as a second script, §11); at most one per id across both folder kinds (§2) | Further loads refused |
| Per ext call: wall time | 4 ms for every call: a 6b call, a top-level run and an `Unload` handler alike (a script that builds more at load continues in its first ticks) | Checked in the count hook, which fires after every 1,000 units of instructions and costly VM operations (§1.5), and after every native; so the call stops at the **next instruction** after the operation that passes the deadline ("Deadline at the next instruction" below). The call aborts; `pcall` cannot catch it, as for the budget |
| Per ext call: work units | 800,000 (= the 4 ms deadline at 5 ns per unit, above, "One budget in two measures"), the top-level run included. Deterministic runs (`scripttool test`) use units only and switch the wall clock off (below, "Deterministic mode") | As above |
| Ext time started per frame, all scripts | 2 ms in 6a–6b (§4); nothing at all in a frame that the heavy-frame gate skips (22 ms elapsed when 6a would begin, §4) | No further ext call or item starts this frame |
| Ext collector slice (6c) | 1 ms per frame, at least one apartment stepped in each frame the heavy-frame gate does not skip | The rest waits for the next frame |
| Ext queue slice | 1 ms per frame, by whole items, at least one item in each frame the heavy-frame gate does not skip | The rest waits for the next frame |
| Ext time per script, rolling | mean ≤ 1 ms per frame over the last 1 s (half the per-frame cap) | **Throttled**: ticked at half rate until the mean is below 0.5 ms. Over the limit for a total of 5 s within the last 10 s while throttled: **suspended** with a notice, and Restart and Disable buttons in its panels and the script manager |
| Aborted calls (deadline, units, errors that abort) | 3 in 10 s | Suspended, as above |
| `World` rays | 64 per script per frame; 256 for all scripts; at most 500 m long; resident data only | `nil, "busy"`; a longer ray is a Lua error; `nil, "not loaded"` past resident data |
| `World` point queries (height, water, area) | 256 per script per frame | `nil, "busy"` |
| `Entity` queries | 8 per script per frame (32 for all scripts), at most 512 results each | `nil, "busy"`; results truncated with a flag |
| `Entity` subscriptions | 16 per script; 64 for all scripts; filters run in the script's 6b turn (§4) | Creation refused |
| `Entity` subscription events | 1,024 per script per frame | Dropped, with a count in the next delivery |
| Queued notifications | 1,024 per script, all kinds together. State notifications (size, visibility, focus, activation) are **coalesced** per control to the latest value and never take more than one entry each; while the queue is full, new events are dropped and counted per kind, and the next delivery carries the counts. A suspended script queues nothing (§4) | Dropped and counted |
| `Core` timers | 64 per script | Creation refused |
| Pending intents | 256 per script per frame (overlay primitives and `Store` writes have their own rows) | `nil, "busy"` |
| UI elements (windows, controls, world labels) | 2,048 per script, of which at most 512 in one window and 256 world labels; **8,192 for all ext scripts**, of which at most 1,024 world labels. These counts bound step-8 layout (E8) | Creation refused |
| `Audio` (slice 2) | 8 voices per script and 32 for all scripts; 8 starts per script per second; the summed gain of one script's voices on a bus never above that bus's volume, and the summed gain of all ext voices on a bus never above twice it | `nil, "busy"`; a start that would pass the gain cap is refused |
| `Data` lookups | 16 per script per frame | `nil, "busy"` |
| Outstanding worker requests | 32 per script, 256 for all ext scripts (`Store` reads, decode requests of `Data`, `Audio` and `Entity.Local`, bridge reads, `Files.Pick`); decode requests de-duplicated; each result reserved against the script's memory before it is queued (above, "Outstanding worker requests are capped") | `nil, "busy"` (a decode miss answers `nil, "not loaded"` and queues nothing) |
| Client-local entities | 256 per script | Spawn refused |
| Overlay primitives | 10,000 per script per frame; 20,000 for all scripts | The newest are dropped |
| `Sandbox` commands | 1 per script per second; one outstanding teleport for all scripts | Refused before submission |
| `Files.Pick` (slice 2) | one dialog open for all scripts; only in a 6b turn that delivers a user gesture to that script (its action start, its `/ext:` line, or a click notification from its own control) in this frame; at most one per script per 5 s. The dialog is asynchronous (a desktop portal or a non-blocking OS dialog), so frames keep running while it is open, and it never runs an in-process toolkit that changes the process locale (§1.5); the client checks the locale when it returns. It counts as one outstanding worker request of the script while open (above). While the dialog holds keyboard focus the game window gets no key events, so the panic chord cannot reach it; therefore every lease ends when the dialog opens (§5.4), and the chord works again once the game window has the focus. A platform with only a blocking dialog has no `Files` (`nil, "unavailable on this backend"`) | `nil, "busy"` |
| `Server` messages (slice 3) | 64 KiB per message; per script 32 messages and 256 KiB per second, at most 64 queued; for all scripts 1 MiB per second. The same caps hold on the in-process loopback. `tessera-protocol.md` may lower them, never raise them without amending this record | `nil, "busy"`; nothing is dropped silently |
| Bridge actions (slice 2) | 1 per script per second; one load outstanding per script, two for all | Refused |
| **Separate UI pools** | Every ext UI resource comes from an **ext pool on top of** 0015's caps, never from them: UR's 4,096 surfaces, 8,192 composition entries, U5 memory, U4 (b) paint slice and U3 upload cap stay whole for native panels and plugins. The renderer's absolute limits are UR's plus ext's; slice 1a confirms it handles the sum (E8), and if it cannot, the ext pools shrink, never UR's | — |
| UI surfaces | 128 per script; **512 for all ext scripts** (ext pool) | Creation refused (the ext window is not created; UR's draw-nothing fallback is never the ext path) |
| UI composition entries | 256 per script; **1,024 for all ext scripts** (ext pool). Entries are counted per frame (UR §7), so the check runs where an ext window would add entries: at creation, at every show (`SetVisible(true)` or a show of an ancestor) and at every change that makes hidden ext content visible. Plugins and native panels never compete for this pool, so ext never has to yield | The show is refused: the window stays hidden, the call returns `false` and the script log says why |
| UI memory | ext pool: 32 MiB of CPU canvases, 64 MiB of GPU textures and 64 MiB of pinned images, on top of U5; per script at most a quarter of each. An ext image (a snapshot file, or bytes from `Files.Pick`) passes SK D4 and D18's header checks (dimensions, and DDS sizes in 64-bit arithmetic) before any decoding, is refused above 4,096 pixels on an axis, and its decoded size is charged to the script's share **before** it is decoded | Creation refused |
| Ext paint and uploads | a paint slice of 1 ms per frame and an upload share of 1 MiB per frame for all ext windows, separate from U4 (b) and U3 (§4, "Ext UI time") | The rest waits for the next frame |
| Reserved-layer surfaces | at most 8, fixed size | **Exempt** from every surface, entry and U5 cap |
| Registered actions | 64 per script; **256 for all ext scripts**, in a separate **ext partition** of the registry on top of KM §2's 1,024, which stay whole for built-ins, the UI band and plugins. Freed at unload by unregistration (§2) | The load is refused with a report (the host registers the manifest's whole set at load, §2) |
| Registered contexts | **2 per script; 16 for all ext scripts**, in the ext partition on top of KM §2's 64. With the actions row this means that at most 8 scripts with two contexts each, or 4 with 64 actions each, can be loaded together, below the 16-script limit (owner question 8) | The load is refused with a report |
| Action and context names | `ext.<id>.<name>`: the prefix takes 5 + length of `<id>` bytes of KM's 64, so `<name>` has at most 59 − length of `<id>` bytes (27 for a 32-byte id). The `ext.` prefix is reserved: only the ext host may register such names | `scripttool check` and the load refuse the manifest |
| `Store` size | 1 MiB per value **as serialised in its file**, 8,192 elements and 32 levels (the bounded value model above); **1,024 keys per script**, 16,384 for all scripts; 16 MiB per script and 256 MiB for all scripts (orphans included, §5.2), where each key counts its file size rounded up to 4 KiB **plus 4 KiB** of file overhead, so a 1-byte value counts 8 KiB | Write refused with `nil, "full"` (or `"too large"`) and a report |
| `Store` reads | per script at most 64 reads and 4 MiB per second (a token bucket over 10 s); all scripts 16 MiB per second; each read is also an outstanding request (above) | `nil, "busy"` |
| `Store` writes | written behind, on the store's own I/O queue (above, "`Store` I/O"): a key written again before its file is written replaces the pending value, and each script's dirty keys are flushed at most once per second, so a script that writes a key every frame causes at most one file write per key per second. Per script at most 64 key writes and 1 MiB per second (a token bucket over 10 s); all scripts 4 MiB per second | `nil, "busy"` |
| Script log | per script one file of at most 1 MiB, rotated once (at most 2 MiB on disk); at most 64 KiB per second per script, beyond which lines are dropped and counted, and a "N lines dropped" line is written when the rate allows. This is the only rate limit on the log | Lines dropped and counted |
| Ext context changes | 4 activations or deactivations per script per second, coalesced per frame (only the last state of each context in a frame is applied) | `nil, "busy"` |
| Reserved-layer notices | one per script and message per 10 s; hit-transparent (§5.3) | Counted in the script manager |
| Commands | 32 per script | Registration refused |

- **Deadline at the next instruction.** A check every 1,000 instructions is not enough on
  its own with stock Lua: some single instructions grow with their operands, and 1,000 of
  them in a row could run for seconds between two checks. The patched build closes this
  **synchronously** (§1.5): every costly VM operation subtracts its work from the running
  thread's count-hook counter, so the count hook, which charges the call's units and
  compares the wall clock with the call's deadline, fires at the next instruction after
  any operation that uses up the interval. A call whose deadline has passed therefore
  aborts at most one operation plus one hook interval (1,000 units, about 5 µs) after the
  deadline. Two facts make this hold everywhere:
  - **Coroutines carry the hook.** In Lua 5.1 hooks are per Lua thread, and a coroutine
    copies its creator's hook settings when it is created. The ext profile installs its
    hook on the main thread once, when the apartment is created and before any script
    code runs, and never changes it: the deadline and the units live in host data that
    the hook reads, not in the hook settings. So every coroutine of an ext apartment,
    however and whenever it was made, and whether it runs through `coroutine.resume` or a
    `coroutine.wrap` function, is metered like the main thread. E6 has both cases.
  - **Nothing is asynchronous.** The hook runs on the main thread, inside the Lua thread
    that did the work, at an instruction boundary. No watchdog thread, signal or
    suspension touches a `lua_State`, no hook field is written while another write is in
    progress, and no record of "the running thread" is kept that could outlive a
    coroutine or an apartment. PR §10.3 ("Lua states are never touched from another OS
    thread") holds unchanged.

  Natives cannot run past the deadline by more than their charge either: a native whose
  charged cost × 5 ns exceeds the call's remaining wall time is refused before it starts
  (Work units, above).
- **Largest single allocation: 4 MiB** in an ext apartment (any one string, table part,
  buffer or stack). Larger requests are refused with a memory error the script can catch.
  So no string is longer than about 4 MiB and no table has more than 2^18 array slots or
  2^16 hash nodes (64-bit builds). Every operation whose cost grows with **the size of** a
  string or a table is therefore bounded by that size rather than by the 64 MiB apartment
  cap. Operations whose cost would grow with the **number** of colliding keys (a string
  creation walking the interning chain, a get or set walking a table chain, the
  compiler's constant table) are bounded by the keyed hashing and the 32-node chain bound
  of §1.5 (for tables and the interning table alike), not by the allocation cap: with
  them, no chain walk grows with the apartment's memory. Data files of a script folder
  and `Files.Pick` results are capped to fit (§2: no file over 3 MiB, 8 MiB in total;
  `Files.Pick` refuses files over 3 MiB). The interning table's own bucket array is not a
  script allocation: it grows with the number of interned strings, which the apartment's
  memory cap bounds, and its rehash is part of R below. (The cap was 8 MiB until revision
  6; it was halved to fit the frame budget below.)
- **Residual.** Four kinds of work cannot be interrupted, because the hook can only stop a
  call after them:
  - **One VM operation, or one bounded native (R).** The worst one measured is a string
    concatenation, whose cost grows with its result: 0.75 ms per MiB, measured with the
    stock build, so about **3 ms** for a 4 MiB result; the keyed hash of the result adds
    a pass over its bytes, which slice 1a measures. Other operations that grow with their
    operands are not measured yet: a store that resizes or rehashes a table at the
    allocation cap (every entry is moved), an order comparison (`<`, `<=`) of two 4 MiB
    strings (byte-wise in every state, §1.5, so its cost no longer depends on the
    locale), a `next` call on the sparsest table the cap allows (§8's table), and a rehash
    of the string interning table under the 64 MiB apartment cap (its stock growth or a
    re-salt, §1.5), which walks every interned string. A walk of a 32-node chain is far
    below all of them. Slice 1a measures each, the comparison also with the process
    locale set to a UTF-8 collation locale in a test build (R must not change), and R is
    the largest.
  - **One collector step (S).** A step of Lua 5.1's incremental collector is normally
    small, but the step that finishes a mark phase (the atomic part) re-marks every table
    written since it was marked and cannot be split, so it grows with the apartment's
    heap, not with one allocation. It runs in 6c, or inside an allocating operation of a
    script's own call (so one overrunning operation can carry one S). S is **not
    measured yet**. Slice 1a measures the worst S under the 64 MiB cap with a script
    built to maximise it.
  - **A compilation (K)**, which runs as a whole 6a item: about 6.5 ms for 128 KiB of
    ordinary code (§2), and linear for hostile source thanks to the keyed hashing; slice
    1a measures the worst file E6 can build.
  - **A state close (C).** `lua_close` at an unload frees the apartment's heap and drops
    its host userdata. It runs no ext Lua (no script finalizers, and sealed host
    metatables, §1.2), but its native time grows with the heap. It is an ext item of its
    own, after the `Unload` handler's item (§4, "Unload items"). Slice 1a measures C
    under the 64 MiB cap.
  - **Finalizers are not a residual.** Ext apartments have no script finalizers, and no
    script can put a Lua `__gc` on a host metatable (§1.2), so neither 6c nor
    `lua_close` ever runs ext Lua code, and no finalizer time needs a budget of its own.
  - **Value conversion is not a residual.** Turning a `Store` value, a `Files.Pick`
    result, a bridge plugin-data read, a `Server` message or the hand-off table into Lua
    values, and serialising a Lua value for `Store.Set`, a `Server` send or the hand-off
    table, always runs inside a metered call (a 6b delivery call, the `Unload` handler or
    the top-level run, §4), with its cost charged or reserved before it starts from the
    bounded value model (above, "Work units" and "The bounded value model"). So it obeys
    the call's units and deadline like any native, and never runs as an uncharged item.
    Parsing on a worker is bounded by the same caps and is worker time, not ext time.
- **The worst ext time in one frame.** 6a and 6b stop starting work at the 2 ms cap, and
  6a also at its 1 ms slice, with no exception (a backend switch's unload items obey it,
  §4). 6a has exactly five item kinds (§4), so the last thing started is one of:
  - a 6b call (4 ms deadline) started just under 2 ms: it ends by 6 ms + R + S;
  - a 6a item started under 1 ms: a top-level run or an `Unload` handler (4 ms deadline
    each) ends by 5 ms + R + S; a compilation by 1 ms + K; a state close by 1 ms + C; a
    snapshot result is constant.

  With K ≤ 9 ms and C ≤ 9 ms, 6a–6b take at most **max(6 ms + R + S, 10 ms)**. 6c adds
  its 1 ms slice plus one S. The total is **7 ms + R + 2·S** (or 11 ms + S after a
  compilation or a close); with R = 3 ms it is 10 ms + 2·S, so the E6 ceiling of
  **13 ms** holds if S ≤ 1.5 ms and the unmeasured operations stay under the
  concatenation. (The hook interval of about 5 µs is below the precision of these
  figures.) In a frame that the heavy-frame gate skips (§4), ext time is zero.
- **The frame budget: where 13 ms comes from.** The frame rule is "no frame over 50 ms"
  (U4 (a), IB D8), ext time included, and it is a rule on wall time. The ext ceiling is
  what the rule leaves, in the M4.5 walking run, after everything else that one frame can
  hold on the reference machine. The engine figure is the **re-run of the M4.5 exit
  verification** (`docs/perf/reference-machine.md`, "Re-run in the M4.5 exit
  verification"), which supersedes that day's first run (P4 15.5 ms, the figure that
  revision 5 used):

  | Share of one frame | Budget | Source |
  |---|---|---|
  | Engine work with no UI, worst frame | 21.8 ms | M4.5 exit re-run, walk spot P4 maximum (P2 19.8 ms, P3 15.7 ms) |
  | Native and plugin UI paint, event frames | 4 ms | 0015 U4 (b) paint slice. U4 (b) bounds paint only |
  | Plugin deferred work | 2 ms | IB D8 item 5 slice |
  | Plugin `Update`, input handlers, the overrun of the last plugin item, and the plugin collector step (IB D8 step 7 item 6: `LUA_GCSTEP` in every plugin apartment, unsliced, and growing with heaps that PR §9.4 allows up to 512 MiB) | 3 ms reserve | unsliced in IB D8; M5 measures the M5 plugin set (ROADMAP M5 exit criterion 4) against it |
  | Ext UI time (steps 1, 7 and 8, E8) | 2 ms | §4, E8 |
  | Ext Lua time, worst frame (E6) | **13 ms** | this section |
  | Margin: measurement noise, the frame's own overhead, and the **non-paint native and plugin UI work of event frames** (offers, hit tests, the UI tick and layout in panel-open, resize and scale frames, where U4 (f) does not apply) | 4.2 ms | M5 measures the non-paint UI work; if it needs more than 2 ms of the margin, the ext ceiling drops by the excess |
  | **Total** | **50 ms** | |

  So the per-frame cap is 2 ms (4 ms in revision 5), every call's deadline is 4 ms
  (5 ms) and 800,000 units (2 million), the largest single allocation is 4 MiB (8 MiB),
  and the E6 ceiling and the fallback target are 13 ms (20 ms). With the first-run
  figure gone, the old rows added up to 21.8 + 4 + 2 + 3 + 2 + 20 = 52.8 ms, over the
  rule before any margin; the new rows leave a 4.2 ms margin.
  **Which frames E6 judges:** every frame of the M4.5 walking run (walk spots P1–P5)
  with the M5 plugin set open, the UI event script of 0015 (panel opens, scroll steps,
  chat lines) running, and the hostile scripts loaded. The "no frame over 50 ms with ext"
  claim is made **for that run only**.
  **Residual risk, recorded:**
  - *Plugins.* Plugin `Update`, handler and collector time is unsliced (IB D8) and plugin
    limits allow far more than the 3 ms reserve; a plugin set that uses more can break
    the frame rule with or without ext, and with ext at its ceiling it breaks it sooner.
    If M5 measures more than 3 ms for the M5 plugin set, the ext ceiling drops by the
    difference, and the fallback below applies to the new ceiling.
  - *Fast flights.* The 50 ms rule covers M3's flights too, and ext scripts run in fly
    mode. The M4.5 exit re-run's flights reach 28.8 ms (M3-a), 25.8 ms (M3-b) and
    34.5 ms (M3-d) with no UI at all (the first run had 35.5, 42.8 and 55.3 ms). 34.5 ms
    plus the UI, plugin and ext rows above is far over 50 ms, so the ext ceiling cannot
    be promised there, and no lower ceiling would make it hold for every flight frame.
    The heavy-frame gate (§4) is the measure: in a frame that is already 22 ms old when
    ext would start, ext starts nothing. The perf record attributes the heavy flight
    frames to terrain and object updates while landblocks arrive and to time the main
    thread did not run; the first happen before step 7, so the gate catches them. A
    flight frame whose cost comes **after** step 7 (render submission, a GPU wait, the
    main thread descheduled) can still pass 50 ms with ext at its ceiling. E6 runs M3-a
    and M3-d with the hostile scripts loaded and **records** the frames over 50 ms with
    and without ext, as a measurement, not a pass criterion, and the owner is told the
    result at the M5.5 exit.
  - *The measure.* The budget is wall time. Most of a walking frame on the reference
    machine is submission and the wait for the GPU (main-thread work had a median of
    about 2 ms), and main-thread ext work may overlap some of that wait, but the record
    does not count on it: it budgets ext as if every millisecond of it added to the
    frame.
- **The recorded fallback.** If slice 1a measures a larger R, S, K or C, it lowers the
  single-allocation cap (which shrinks R), the per-script memory maximum (which shrinks
  S and C) and the per-file source cap (which shrinks K) until 7 ms + R + 2·S ≤ 13 ms,
  C ≤ 9 ms and K ≤ 9 ms; and if a maximal value conversion (E6) takes more than half
  of a call's 4 ms deadline, it lowers the value model's element cap. The owner is told
  the result at the M5.5 exit. S growing with the heap is also why the per-script maximum
  is 64 MiB, not 256. E6 tests the bound with hostile scripts.
- **Deterministic mode (`scripttool test`).** The guarantee is: given the same scripts,
  walk file and seed, which items and calls run in each frame, every `dt` sum, every
  answer of a query and the frame in which every notification is delivered repeat
  exactly, as `pairs` order does. Four rules make it hold, because otherwise worker
  timing (ARCH §21) would decide when results arrive and what is resident:
  - **Units, not time.** With the wall clock off, every per-frame slice is counted in
    units, mirroring the client's mapping of 5 ns per unit: the 6a slice is 200,000
    units (1 ms), the 6b cap 400,000 units (2 ms), a call's limit its 800,000 units
    (4 ms, the same as in the client, so a script that passes `scripttool test` does
    not abort in the client on its units), and 6c steps one apartment per frame,
    round-robin. The heavy-frame gate is off.
  - **Seeds.** Table salts, interning salts, hash secrets and random generators are all
    derived from the test's seed.
  - **A fixed-latency completion queue.** Every worker job that ext causes (a snapshot,
    a `Store` open, read or write, a decode request, a bridge read) and every job whose
    result an ext query reads (landblock and collision-set integration, entity
    integration) completes, as far as the harness is concerned, at frame N + k for a
    job submitted in frame N (k = 1 by default; the walk file may set it), in
    submission order, whatever the real worker time: the harness waits for a job that
    has not finished by then and holds back a result that finished early.
  - **Residency follows the walk.** Streaming is gated on residency, as ARCH §20.3 does
    for collision sets: the walk's frame N starts only when everything the streamer
    asked for by frame N − k is integrated, so the resident set at each frame, and with
    it every `World`, `Entity`, `Data`, `Audio` and `Entity.Local` answer ("not loaded"
    included), is a function of the walk alone.

  What it does not promise: the timings of a run in the client, where the wall clock,
  real worker latency and the heavy-frame gate apply.
- **Profiler line (graft from B).** Each script reports tick time, rolling mean, memory,
  units, intents, leases and limit hits. The script manager and the console show it.
- **Rate-limited reports.** Repeated reports for one script and message are rate-limited in
  the script manager's and the console's display, in the manner of PR §11.6. In the log
  only the log's own byte rate and size cap apply (table above), and dropped lines are
  counted, never lost silently. Unlike plugin reports (LPA §6.6), ext reports are **never shown in the chat
  window** (§7, "Chat").
- **Failure stays local.** A failing script never takes down the client, plugins or other
  scripts.
- **Suspension** ends the script's leases at once and its overlays in the same frame.

### 9. Performance budgets (reference machine, `docs/perf/reference-machine.md`)

These are measured with the reference scripts of the slice's exit criteria during the M4.5
walking run with the M5 plugin set (ROADMAP M5 exit criterion 4) open.

| # | Measure | Budget |
|---|---|---|
| E1 | Ext time per frame, all scripts, steady state | mean ≤ 1.0 ms, p99 ≤ 2.0 ms |
| E2 | Host overhead of one native call (argument check, grant check, conversion, charge) | ≤ 1 µs mean |
| E3 | Entity query, radius 100 m, 500 entities in range, one batched call | ≤ 0.2 ms |
| E4 | Overlay: 10,000 primitives (slice 1b) | CPU encode ≤ 0.3 ms, GPU ≤ 0.3 ms |
| E5 | Memory of all ext apartments, host objects included | within the sum of their caps (enforced); measured peak recorded; the declared size of each host object kind (element, label, timer, subscription, notification, intent) at least its measured Rust state |
| E6 | Frame rule with hostile scripts: an endless loop, a stalled tick, a loop of 1,000 maximal concatenations and one of 1,000 maximal string comparisons (the next-instruction deadline; the comparison loop also run with the process locale set to a UTF-8 collation locale in a test build, where R must not change), **the same two loops run inside a coroutine** (made at the start of the call and in an earlier call, and driven through `coroutine.resume` and through a `coroutine.wrap` function), a `pairs` walk over the sparsest table the cap allows, the largest table resize the allocation cap allows, a maximal `table.sort`, `table.insert(t, 1, v)` and `table.remove(t, 1)` on the largest array; **degenerate wrapper arguments** (`string.rep("", 2^31 - 1)`, `("x"):rep(2^31 - 1)` through the string metatable, `table.concat` over a range of 10^8 empty strings, `unpack({}, 1, 10^8)`, `table.insert(t, -2^31, v)`); **hash flooding** (thousands of same-length strings that share every byte stock 5.1 samples, number keys that collide under the stock number hash, a script that learns main positions from `next` order and builds the longest chain it can, **a set of keys whose stored hashes share their low bits, inserted into fresh tables of every size after re-salting** (the test harness, which knows the test seed's secret, builds keys sharing their low 17 bits), with 1,000 gets of such a key between two hook checks, and a 128 KiB file of constants that collide under the stock hash); **value conversion** (a maximal `Store` value read back, a maximal hand-off table, a `Store.Set` of a graph of shared subtables that would expand past the caps, a `t = {t}` chain 10^5 deep, a cyclic table, a table with `__index` and `__len` metamethods that must not run, a hand-edited `Store` file with a NaN key, and a maximal plugin-data file read through the bridge, slice 2); **worker floods** (a script issuing `Store` reads of 1 MiB values and `Data` lookups of new DIDs at their per-frame caps for 10 s: outstanding requests stay at their caps and the worker queue does not grow); **deep C recursion** (nested `coroutine.resume` and `coroutine.wrap`, and recursion through `string.gsub` callbacks, `table.sort` comparators and metamethods, each stopping with a Lua error, never a crash); **host-object floods** (elements, world labels, timers, subscriptions and queued notifications at and past their caps, and a script starved for 10 s while 1,024 entity events per frame arrive); a heap built to maximise the collector's atomic step (S); a heap built to maximise one rehash of the string interning table; **two backend switches within §8's memory sum**: 8 apartments at 64 MiB plus a hot reload's new apartment at 64 MiB in the reload allowance (9 closes at the worst C), and 16 apartments at 32 MiB plus a reload's new apartment (34 items, the longest switch, at most 34 frames that the heavy-frame gate does not skip); rays and queries at their caps; a script alternating heavy and light frames; and a script that aborts three times in 10 s every 10 s (notices stay hit-transparent) | no frame over 50 ms, ext time included, in the run that §8 "The frame budget" defines (the M4.5 walking run); ext time per frame ≤ 13 ms; in M3-a and M3-d with the same scripts, the frames over 50 ms with and without ext recorded (not a pass criterion, §8); no chain walk over 32 nodes; host memory of every script within its cap; R, S, K and C measured and recorded against §8's sum |
| E7 | Native cost calibration: for every native, including every wrapped standard function of §8 (`table.insert` and `table.remove` with a position, `table.maxn`, the size-proportional string functions and the degenerate arguments of E6 among them), the measured worst time ≤ charged units × 5 ns; and for every costly VM operation of the patched build (§1.5), the measured time ≤ the work it subtracts from the hook counter × 5 ns; and for value conversion in both directions (§8, the bounded value model), the measured time ≤ the charged or reserved units × 5 ns | holds for every registry entry and every operation (an indicative CI benchmark; met on the reference machine) |
| E8 | Ext UI time in steps 1, 7 and 8 (§4): native routing, text editing and dragging of ext windows, hover over them, and layout, paint and world-label placement of ext windows at the ext caps (8,192 elements with 512 in one window, 1,024 world labels, 512 surfaces, 1,024 entries), with the pointer sweeping the ext windows, measured apart from U1 | ≤ 2.0 ms per frame at the caps, the 1 ms ext paint slice included; U1 measured with the same ext windows open meets M5's figure; the renderer handles UR's caps plus ext's pools together |

### 10. Tooling

- **Hot reload (developer scripts only).**
  - Only a script of the `scripts-dev` kind, with developer mode on, is ever hot
    reloaded: by `/ext:reload <id>` or by the file watcher for `data/scripts-dev/`. Only
    that kind keeps its grants across content changes (§5.2), so only there can new code
    take over a running script's state without a re-ask.
  - **`/ext:reload` of an installed script** (developer mode) is a **full reload**: the
    old apartment unloads (its `Unload` handler and state close), the new snapshot is
    read, its content hash is checked, every group-A grant is asked again if it changed
    (§5.2), and only then is it loaded, with **no hand-off table**. New code of an
    installed script therefore never runs under grants given to older code, and no
    prompt shows over a running apartment.
  - **The watcher** is a non-main thread (or the platform's notification callback) that
    only sends "something under this script folder changed" events, coalesced per
    folder, over a channel to the main thread; it never reads file content. The reload
    itself reads the folder on an asset worker, as every load does (§2).
  - It spans frames. The new snapshot is read and compiled in ext items while the old
    apartment keeps running. Then one item runs the old `Unload` (4 ms deadline) and keeps
    its hand-off table, and the next item runs the new top level (4 ms deadline), which
    receives the table.
  - State survives through `Store` and the hand-off table (the bounded value model of §8:
    at most 1 MiB serialised, 8,192 elements, 32 levels, no cycles). The table is
    serialised inside the old `Unload` call with its maximum cost reserved, and turned
    into Lua values at the start of the new top-level run with its cost charged first
    (§8), so neither step runs outside a metered call.
  - A compile error keeps the old apartment running and shows the error.
  - **Only a reload with the same declarations is hot.** The new manifest is read first.
    If it changes the capability list (required or optional), `memory_mib`,
    `[[actions]]` or `[[contexts]]`, the reload is **not** hot: the old apartment is
    unloaded (its `Unload` handler and state close, as at any unload), the grant dialog
    asks again as §5.2 says (for a capability change, every grant, in developer mode too),
    and only then is the new snapshot loaded, with **no hand-off table**. So no prompt
    shows while the old apartment runs with the old grants, no hand-off table crosses a
    change of grants (rule 4 of §5.2), and the registry never holds two sets of
    declarations for one script.
  - **What a hot reload keeps.** With the same declarations, the script's actions and
    contexts stay registered (they belong to the (kind, id), §2) and count once against
    the registry caps. At the swap, after the old `Unload`, the host deactivates every
    context the old apartment had active (applied at the item-7 drain, without dropping
    any non-ext pending sequence, §5.5); the new apartment starts with none active and
    activates what it needs in its top-level run. The old apartment keeps its contexts
    until its `Unload` has run.
  - **Limits while both apartments exist.** The script counts once against the 16-script
    limit, and at most one reload runs at a time (a second `/ext:reload` waits in the ext
    queue), so at most 17 apartments exist. The old apartment stays inside the 512 MiB
    sum of caps, and the new one is held by the separate 64 MiB reload allowance (§8),
    which every cap fits. So a hot reload never has to fall back to unload-then-load, and
    with 16 scripts loaded at the default cap a reload still works. When the old
    apartment unloads, the new one takes its place in the sum with the same cap (a
    changed `memory_mib` makes the reload a full one, above).
- **Errors.** The core's error path: chunk (relative path) and line, a traceback from the
  host's private `debug`, and the script log.
- **Console.** In developer mode, an egui overlay (the tool UI of ARCH §15, never game UI).
  It has a REPL bound to one ext apartment, the per-script profiler lines, and Tracy zones.
  The REPL's input is compiled by the **host** as a developer tool, never through a
  function the script can reach, only in developer mode, and runs under the same limits.
  The REPL binds **only to apartments of `data/scripts-dev/`**, never to an installed
  script, so the statement "the code that runs is the hashed snapshot" (§5.2) holds for
  every script of `data/scripts/` without exception. (Developer scripts already keep their
  grants across content changes, §5.2.) While a consent prompt shows, the console is
  hidden and takes no input (§5.3). A debug adapter (DAP) on localhost is slice 4, with
  these requirements fixed now: developer mode only; bound to `127.0.0.1` (and `::1`)
  only, never to another interface; a random per-session token that the client shows to
  the user and every connection must present first; DNS-rebinding protection (a
  connection that carries a `Host` or `Origin` other than the loopback address and the
  adapter's port is refused, and the adapter speaks no browser-reachable protocol
  without the token); and it binds only to `data/scripts-dev/` apartments, like the
  REPL.
- **One source of truth (graft from A).** The binding registry (§1.3) generates:
  - `docs/specs/ext-api.md`'s reference tables, in our own words;
  - a LuaLS-style annotation file (`--ext-dump-api`), so editors complete and check `Ext.*`;
  - the runtime validation.

  A CI check fails if the committed reference differs from the generated one.
- **`tessera-scripttool` (new app; graft from A and B).** It uses `tessera-ext-host`.

  | Command | Slice | What it does |
  |---|---|---|
  | `check` | 1a | Lint the manifest, apply the folder rules (reserved device names included, §2), check capability and module names against the registry, syntax-check every file |
  | `test` | 1a | Run the script headless on a **synthetic world** with no install and no GPU, along a walk file, with the script's own assertions, in **deterministic mode** (§8): every apartment's generator (§1.2), hash secret and table and interning salts (§1.5) derived from the test's seed (so `pairs` order repeats too), the wall clock off, every per-frame slice counted in units, worker results delivered through the fixed-latency completion queue and streaming gated on residency, so which work runs in which frame, and every query's answer, repeat too; authors can test in CI (CLAUDE.md content rules) |
  | `init` | 1b | Create a script skeleton with a manifest |
  | `pack` | 1b | Make a zip of exactly the files the loader would snapshot (the folder rules of §2). The **content guard**: it refuses any `.dat` or `.datx` file, any file that starts with the archive container's header as `tessera-dat` recognises it, and any file over the folder caps; it warns that extracted game assets must not be packed |

- **The headless harness** (in `tessera-ext-host`, slice 1a). The pieces exist:
  `tessera-world::testkit::SyntheticWorld` builds a `MemoryWorldSource` that
  `WorldContext::load` accepts, and the sandbox backend takes its world through the
  `WorldSource` trait. Today's `--walk-path` cannot be reused as it is, because it opens
  the install (`walker.md` §13.6). Slice 1a builds the driver: the sandbox backend over a
  synthetic world, `physics`, `entity`, the input mapper without a window, `tessera-ui`
  without a painter, and the ext host, stepped per frame with the walk file's inputs,
  with the worker model of §8 "Deterministic mode" (results at frame N + k in
  submission order; each frame starts only when what the streamer asked for by frame
  N − k is integrated).
- **Examples.** The reference scripts are original and CC0.

### 11. The path to "much more": later tiers

These keep the owner's wider goal open without blocking slice 1. Each needs its own decision
before any code lands.

- **A second language (Luau or Wasm).** It would be a new front end on the same registry and
  ports (§1.3). The decision must choose between B's separate `tessera-scripthost` process
  (IPC schema plus fuzz target, OS hardening, one frame of latency for queries) and a thin
  in-process binding with renamed symbols (a second sandbox and escape-test suite). It must
  also show that the grant, lease, deadline and "no compiling of strings" rules hold
  unchanged.
- **The server-side half of `Server` (graft from A, slice 3).**
  - A script may declare a `server` entry.
  - In the sandbox that file runs in a second ext apartment, connected to the client half
    through an in-process loopback, so the same script works offline.
  - **What must be defined before slice 3 ships** (an addition to this record or to
    `ext-api.md`, reviewed with slice 3's plan), within these fixed constraints:
    - **accounting:** the server half's apartment counts against the 16-script limit as
      a second script and against the 512 MiB sum with its own cap, so §8's "at most 17
      apartments" becomes "at most 16 loaded apartments plus one reload"; it is never
      outside the counts;
    - **grants:** it holds no capability of its own beyond `Core`, `Store` and the
      server end of its channel; it is shown in the client half's grant dialog, and it
      can never hold a local-only capability, because its client half holds
      `session.server` (§5.2);
    - **`Store`:** its own store, keyed (kind, id, `server`), never local-only, wiped
      with the client half's grant record;
    - **limits:** the same per-call, per-frame and memory limits as any ext apartment,
      inside the same 2 ms cap and slices.
  - On the project-owned Tessera-protocol test server, the project installs the same file
    in the server's ext host (`apps/server`). Third-party servers are outside this record
    (§6.5).
  - Servers never push code to clients (§5.6).
- **Automation on servers.** `input.drive` and other game-command modules beyond the sandbox
  (§6.5). It must answer the proxy case of §6.5.
- **Render hooks or custom shaders.** naga validation cannot prevent GPU hangs, and
  `PassHook` is crate-private. This needs its own decision.
- **Asset overrides** (in-memory replacement of a DID's decoded content). This is a
  content-substitution surface and needs its own decision.
- **Two-way plugin bridge.** It needs its own decision. None is
  proposed.
- **Network access** (HTTP to an allow-list). It is not proposed, and it stays on the "never
  offered" list until a decision moves it.

### 12. Content and isolation rules

- **Backend scope.** Ext support targets the offline sandbox and the project-owned
  Tessera-protocol test server (§6.5): third-party servers need their own
  decision amending 0004. Ext scripts never run
  against an official server in Tessera's builds (§6).
- **No BIN facts in the ext contract.** `ext-api.md` defines the ext
  widgets, their input behaviour, events and drawing in its own terms, from Tessera's
  design and general GUI conventions. It cites no BIN fact as part of the ext contract:
  neither LPA §13's plugin control behaviour nor the BIN rows of UB and UL. The
  implementation may share `tessera-ui`'s element tree and painter, which also serve native
  panels and plugin controls, but the ext contract promises only what `ext-api.md` states.
  Where the shared code's behaviour rests on a BIN fact that `ext-api.md` does not state,
  ext scripts may not rely on it, and a change to it is not an ext API change. A wish to
  give ext widgets plugin-control parity would need its own decision;
  none is proposed.
- **No game content.** Scripts refer to DIDs that resolve in the user's install. `Data`
  returns decoded values for the user's local use, with no enumeration by DID range.
  **Assessment of `data.read`:** it is a viewer of the user's own install inside the user's own client, like
  the explorer, and is opt-in per script. Tessera offers **no export route**, and that is
  made true by design, not just stated: `data.read` is offered offline only; it is
  local-only and never held by one script with `session.server`; a store it wrote is
  wiped before a `session.server` holder opens it; no hand-off table crosses a change
  of grants (§5.2); `bridge.plugins`, the one way to put data into plugin data, is
  local-only too; no other module reaches the network; and `Data` gives no raw bytes.
  The file output of a script is its own `Store` (capped at 16 MiB) **and its script
  log** (whatever it prints, decoded values included; capped at 2 MiB per script), both
  in Tessera's directories on the user's machine. **The stated limit:** no single
  script can send it off the machine on its own, but programs that the user installs
  and runs together could pass data to each other and one of them could send it to a
  server (§5.2): two or more scripts through shared engine state, shared pools, or
  across sessions through shared quotas such as the total `Store` space; and a script
  and a plugin, through a bridged command whose arguments the plugin keeps or turns
  into an `Alias` quickslot that the user later clicks on a server, or through frame
  timing and window layout. Tessera neither offers nor documents that; it is the user running cooperating programs of
  their choice, comparable to any other software on their computer. Nothing in Tessera
  advertises scripts as extraction tools, and `pack` refuses archive files and warns
  about extracted assets (§10). Examples embed no extracted data, names or strings.
- **Naming.** `Ext`, the module names, `tessera-ext`, `tessera-ext-host`,
  `tessera-scripttool`, `scripts/`, `.store` and every UI string are neutral (CLAUDE.md Naming).
  Ext UI classes never reuse the interop names. Those names stay inside the plugin
  subsystem as string data. The ext escape tests therefore
  name no interop name: in `tessera-ext` they check that an ext apartment's globals are
  **exactly** the allow-list of §1.2 (which excludes every other name, the interop ones
  included, without spelling them), and the test that a plugin apartment has no `Ext`
  lives in `tessera-plugins`' own tests.
- **Automation.**
  - LPA §3.5's anti-automation boundary stays for plugins.
  - `input.drive` exists only offline, where automation harms no one (§6.5).
- **Third-party scripts** are their authors' works. Tessera does not host, bundle or download
  them.

## Roadmap placement proposal

*This section is written to be copied into `docs/ROADMAP.md` by the orchestrator once the
owner accepts this record. It adds a milestone. M5–M8 keep their numbers.*

<a id="m5-5"></a>
## M5.5: Extended scripting, slice 1a (core)

Placed after M5. It needs M5's `tessera-ui`, IB D8's frame placement and M5's
`script`/`plugins` split. It does not need M6 or M7, and it can run in parallel with M6.
Sandbox only in this slice; the gate admits `Offline` alone until slice 3.

**Goal:** users and authors can write scripts in Tessera's own extended API, beside the
unchanged standard plugins. Scripts draw their own windows, bind their own actions and
commands, store values, and read the world and its entities. They are capability-gated,
testable headless in CI, and never run against any backend other than the sandbox.

**Deliverables**

- The project's patched Lua 5.1.5 build (§1.5 of the decision): keyed hashing, salted
  tables whose main positions are a keyed function of the whole hash and the salt
  (multiply-shift, high bits), the 32-node chain bound, chain walks and every other
  costly operation charged to the hook in ext states, stock behaviour in plugin states,
  every official lua.org fix for 5.1.5 in both, the interning table's 32-node re-salt in
  ext states, byte-wise string order in every state, linked through mlua as one Lua per
  graph, in the home the owner chose (owner question 1).
- `tessera-script`: the runtime core only, after the split (§1.1 of the decision), with
  apartment profiles, the optional wall-clock deadline and the charge API. If M5's split
  left plugin state in the core, this slice finishes it first, and criterion 6 applies.
- `tessera-ext` (new, L4; sibling allowance `ext` → `script`):
  - the `script.toml` parser and the folder rules (checked on entries it is handed);
  - the canonical content hash over a snapshot it is handed, and the grant store's
    format (parsing and validation) keyed by (kind, id);
  - the language-neutral module registry and its Lua 5.1 front end, with runtime
    validation generated from it;
  - `Ext.Use`, `Ext.Has`, `Ext.Load` over precompiled files;
  - apartments without `load`, `loadstring`, `setfenv`, `getfenv` and `string.dump`, with
    `tostring` showing no addresses, sealed host metatables, error values described
    without metamethods, the 4 MiB single-allocation cap and the
    next-instruction deadline (§8);
  - the bounded value model (cycles, depth, elements, serialised bytes) with conversion
    inside metered calls; host objects charged to the script's memory cap, with their
    count caps and the notification queue cap; deterministic mode;
  - the ext queue, the ext tick, the per-frame cap and the heavy-frame gate; limits,
    rolling-window throttle and suspension; native costs and the wrapped standard
    functions (string metatable included); the per-apartment random generator;
  - the `Store` rules that need no I/O: key, size, read-rate and write-rate caps, the
    coalescing of pending writes, the caps on outstanding worker requests with their
    result reservations; the script log's line buffering and rate limit.
- `tessera-ext-host` (new, L5 library): the ports of `Core`, `Store`, `Commands`, `UI`,
  `Input`, `World`, `Entity` (answering from resident data only); grant persistence and
  the settings page; **all ext file I/O** (§1.1 of the decision): the folder walk that reads the
  snapshot (directory-relative, no links, no hard-linked files), `Store` files behind
  fixed prefixes with write-behind on one I/O queue per (kind, id), the durable
  local-only marker, wipes and orphan removal on that queue, and the rotated script log
  written by a worker; files opened relative to directory handles; host maps, the
  neutral reader's maps and worker-path maps with a keyed hasher; the headless harness
  with the deterministic worker model of §8.
- `tessera-net`: `Session::class()` and `BackendClass`, decided by `Session` from a sealed,
  crate-private trait of `tessera-net`'s own concrete backends; any other backend, or a
  wrapper of one of them, is `Other` (§6.1). `compat.rs` reworded (a compatibility backend
  is a separate crate). The slice-1a gate admits `Offline` only, even if M7's backend
  already exists (§6.2).
- `tessera-input`: the panic-chord tier (present only while ext scripts are loaded) and
  its collision check against every binding, the reserved `ext.` prefix, the ext context
  priority range, the ext passes after both non-ext passes at presses and at releases,
  with the `opaque` and `opaque_keys` stops (§5.5), the collision rule for suggested
  defaults, the separate ext partition of the registry, **unregistration** with
  generational handles (§2), ext registrations, unregistrations and context changes
  that never drop a non-ext pending sequence (§5.5), and the ext keymap document
  `keymap-ext.toml` with its own limits, read and saved like the keymap file (§2).
- `tessera-client` (feature `ext`, on by default):
  - the backend gate, with session generations and the intent checks at apply time, and
    the ext steps 6a–6c;
  - the reserved top layer above the developer overlay (consent prompt with its
    pointer-only "Allow", plain-text manifest strings with id and folder kind, script
    manager, hit-transparent lease indicator and notices);
  - ext UI time in steps 1, 7 and 8 with its own paint slice and upload share, measured
    apart from U1 (E8), and the separate ext UI pools;
  - the ext z-range below every plugin window and native panel, the gesture-rooted
    activation and focus rule (case 1 ending on a key elsewhere, the pointer leaving or
    5 s idle) and the visible-and-uncovered focus rule with its re-checks; ext windows
    hidden while a prompt shows, and "Allow" counting only after the pointer entered it;
  - the process locale pinned to "C", checked after dialogs and at every ext load;
  - the panic chord, and leases that end when the game window loses keyboard focus;
  - `/ext:` command dispatch (never sent to a backend), and at most one loaded script
    per id; the removal of an orphan's bindings from the ext keymap document.
- `tessera-scripttool` (`apps/scripttool`): `check`, `test`.
- Fuzz targets: the `script.toml` parser, the grant store, and Lua-side argument validation
  of the module natives. The neutral data reader keeps the target it has.
- CI: the build-level exclusion check, and the `links` guard only if its fixture shows it works (expected not to, §6.3 of the decision).
- Three original CC0 reference scripts:
  - a HUD panel with coordinates and compass;
  - a nearest-entity tracker panel;
  - a key-bound note-taker that writes `Store` once per second and has `/ext:` commands.

**Specs needed:**
- `ext-api.md`: normative, Tessera's own design, with no BIN facts as contract (§12 of the
  decision). Modules, capabilities, groups, backends, the ext widget set, manifest, folder
  rules, versioning, the never-offered list, limits and costs, leases, the panic chord,
  and the reference tables generated from the registry.
- `ext-manifest.md`, or a section of `ext-api.md`: the `script.toml` grammar, the content
  hash encoding and the grant store format.
- The amendments listed under Consequences, made by the accepting session.

**RE tasks:** none. This milestone is design work, not reverse engineering.

**Exit criteria**

1. The three reference scripts run in the walker and meet E1–E3 and E5–E8 of the
   decision record on the reference machine.
2. Escape tests pass:
   - no file access outside the script's snapshot and `Store`, including through the
     Windows device names: an id or `Store` key `con`, `nul`, `com1` or `lpt1` is refused,
     and no file the host creates under `data/` or `logs/` has a script-chosen bare name;
   - `tostring` of a table, function, coroutine or userdata shows no address;
   - `getmetatable` of a module handle is not a table, and no route sets `__gc` or any
     other field of a host metatable; an `error(t)` with a `__tostring` metamethod that
     loops is reported by type name and number, and the metamethod never runs after the
     call;
   - a folder holding a hard-linked file is refused;
   - an ext apartment's globals are exactly the allow-list of §1.2 (so no `load`,
     `loadstring`, `setfenv`, `getfenv`, `string.dump`, `debug`, `io`, `os`, `package`
     or plugin names), and no binary chunk loads (tests in `tessera-ext`, naming no
     interop name); no `Ext` in a plugin apartment (a test in `tessera-plugins`);
   - `string.rep` and every other wrapped function is the wrapper through the library
     table and through the string metatable (`(""):rep(n)`);
   - an ext `math.randomseed` changes neither a plugin's sequence nor another ext
     script's, and a plugin's `math.randomseed` changes no ext sequence;
   - a string received from `Store` cannot be run;
   - memory, units, the deadline and the per-frame caps hold;
   - a revoked capability fails closed on the next call;
   - no script can draw over, read or focus a prompt, or synthesise the input that presses
     "Allow";
   - no ext call or item runs plugin code: a script's `Activate` or `Focus` while a plugin
     window is active changes nothing a plugin sees, and a prompt raised in 6a reaches a
     focused plugin text box as `FocusLost` in the next step 1;
   - no ext binding takes a press that a built-in binding matches in either pass: an ext
     `Shift+W` or `Shift+Space` leaves walking forward and jumping while walking intact,
     and such suggested defaults are withheld;
   - the same at releases: an exact ext `Shift+MouseLeft` with `on = "tap"` leaves
     `pointer`'s `tap_left` firing on a Shift-click;
   - no ext binding fires while a text box has focus: with the chat input or a plugin
     `TextBox` focused, an ext binding on K or F5 does not fire and the key is swallowed
     as IB D6 says;
   - a script cannot take the keyboard by itself: its `Activate` or `Focus` on its own
     text box while no window is active, outside the 6b turn of its own action start or
     `/ext:` line, returns `false`, and W still moves the character; after the user's
     press of the script's action it is honoured; a request in the turn of an `/ext:`
     line submitted from the chat input, while the chat window is active, returns
     `false`; after a click on the script's window followed by the user walking with W,
     a timer's `Focus` returns `false`, and so does one after the pointer left the
     script's windows or 5 s passed; a `Focus` on a hidden, zero-size, off-screen,
     transparent or **covered** text box (one placed under the chat window, and one
     under an opaque plugin window) returns `false` even right after a click; a focused
     box made invisible, or one that a plugin window is moved over, loses the focus
     before the next key is routed, and the next chat message reaches the chat input;
   - a manifest `name` of "Tessera" with bidi controls and markup is shown as plain
     text, without the controls, next to its id and folder kind, and so are an error
     message with bidi controls and an `/ext:` line naming an unknown id with markup;
   - while a consent prompt shows, ext windows are not drawn or hit-tested, and a click
     on "Allow" by a pointer that rested on it since the prompt appeared does not
     count;
   - after the panic chord, no ext script code runs (a queued top-level run does not
     start, and an unload of a suspended script runs no `Unload` handler) until the
     user resumes the script;
   - a script's `Store` read flood and `Data` lookups of new DIDs answer `nil, "busy"`
     at 32 outstanding requests, and the worker queue does not grow;
   - a `Store` file edited by hand with a NaN key, a nil key or a table with 10^6
     entries is refused on the worker as invalid or too large, without a panic;
   - binding keys for 70 different script ids fills only the ext keymap document; a
     rebinding of a built-in action is still saved and read back strictly;
   - a script that toggles its own context every frame does not cancel a user-bound
     `Ctrl+K Ctrl+C` sequence in a non-ext context;
   - the lease indicator and a suspension notice over a plugin button do not stop a
     click on that button or its hover;
   - an ext window never covers a plugin window, whatever their `ZOrder`;
   - an ext `error("text")` and the output of `/ext:list` and `/ext:help` produce no
     chat line and no `Turbine.Chat.Received` event, and neither does `/EXT:Unknown`;
   - "Allow" cannot be pressed with a key, and ignores clicks in its first 500 ms;
   - no error message, chunk name or traceback contains an absolute path.
3. The slice-1a gate tests of §6.6 of the decision pass (the ones marked **1a**: no ext
   apartment on an `Other` or `TesseraProtocol` stub, unload before `Connect` at a
   mid-session switch, no intent applied after its apartment's unload or suspension or
   under a newer session generation, the `Store` I/O-queue test with a write pending
   and one in flight at a grant switch, and the CI dependency check, with the `links`
   fixture's result recorded either way).
4. Failure tests pass: the hostile scripts of E6 each keep every frame under 50 ms and ext
   time under 13 ms in the M4.5 walking run, throttle or suspend that script, and leave plugins and other scripts
   working. The measured R, S, K and C are recorded, and the fallback of §8 is applied if
   the sum needs it. The frames over 50 ms in M3-a and M3-d with and without the hostile
   scripts are recorded and reported to the owner (§8, the flight residual). A script that writes new 1-byte `Store` keys and rewrites them every
   frame stops at 1,024 keys and 16 MiB of counted size, causes at most one file write
   per key per second, and a script that prints every frame keeps its log within 2 MiB.
5. A content change re-asks for group A grants outside developer mode, and keeps them in
   `data/scripts-dev/` with developer mode on. A copy of a script in the other folder kind
   has no grants, and cannot be loaded while the first copy is loaded. `/ext:reload` of a
   changed installed script is a full reload that re-asks group A before the new code
   runs. Two snapshots that differ only by moving bytes between a path and a content hash
   differently. Loading and unloading 64 different scripts in turn, each with the
   maximum actions and contexts, leaves the registry as it was (unregistration).
6. The M5 plugin set (M5 exit criterion 4) behaves identically with the reference scripts
   loaded, on the patched Lua build (plugin states in stock mode, the M5 escape and
   behaviour tests unchanged), and after the `script`/`plugins` split: the same `Shell.IsCommand` and
   `GetCommands` answers, the same command registrations, the same deferred-work order and
   plugin time per frame within noise.
7. `tessera-scripttool test` runs the three reference scripts headless on a synthetic world
   in CI, with no install, and two runs with the same seed give identical traces (the
   frame of every call, item and notification, and every query's answer) while the real
   worker timing is varied (a test hook delays worker jobs at random).

**Later slices** (each with its own short plan and exit check):
- **Slice 1b, right after M5.5:** `Camera` with leases and the lease indicator, `Overlay`,
  `Sandbox` (with the clock port and its rate limits), hot reload with the watcher and the
  hand-off table, the developer console, `scripttool init` and `pack`, the reference and
  annotation generators with their CI check, and three more reference scripts (a waypoint
  overlay of about 500 primitives, a camera-follow script under a lease, a key-bound
  sandbox teleport bookmark). Exit: E4 and E6 with these scripts; every lease ends within
  350 ms of its last renewal and in the same frame on suspension or the panic chord; hot
  reload of a UI script keeps its window state through the hand-off table and keeps the
  frame rule.
- **Slice 2, after M6:** `Audio`, `Data` (Offline only), `Files`, `Input.Drive` (Offline
  only), `Entity.Local`, `UI.Native`, the one-way `Plugins` bridge with its rate limits,
  and the local-only rules of §5.2 with their §6.6 tests.
- **Slice 3, with M7:**
  - `Server` and its server-side half (in-process in the sandbox), after its accounting,
    grants, local-only status and `Store` are defined within §11's constraints;
  - the script policy (`allow` and `forbid`) in `tessera-protocol.md`'s handshake, the
    interest-management requirement on the server, and the `Server` size and rate
    limits;
  - the gate admits `TesseraProtocol` (the project-owned test server only, §6.5), and
    the §6.6 tests marked **3** pass.
- **Slice 4:** signing, a debug adapter (developer mode only, loopback only, a
  per-session token, DNS-rebinding protection, §10), and the separate decisions of §11 (second language,
  render hooks, automation on servers) if the owner wants them.

## Consequences

### Amended records and sections

Accepted records are not rewritten (`docs/decisions/README.md`). When this record is
accepted, it **supersedes in part** the decision points marked "supersedes" below. The
accepting session adds one status line to 0014 and to 0015, "Amended in part by NNNN
(…)", naming the points, and changes their README rows to "Accepted; amended in part by
NNNN". 0004 and 0016 are not amended (rows below).
Points marked "extends" add to a record without contradicting it; specs and ARCH are edited
in place as usual.

| Record or spec, section | Change | Kind |
|---|---|---|
| 0014 D5 ("The band is consulted before every game-band context"), D7 (`handle_with` steps 1 and 2) | While an ext script is loaded, the panic-chord tier is tested before any event of the press is resolved or offered: before D7's steps 1 and 2, before the band and before KM §7.2 step 1 (§5.5). D7's steps 1 and 2 still run in that call for the earlier events they offer; only the chord press itself is consumed, so D7's ordering is unchanged | supersedes in part |
| 0014 D5 ("Band pushes and the pending sequence": "On a game-band context, `activate` keeps today's rule") | Activating or deactivating an ext context, and the host's registration of a script's actions and contexts, drop a pending sequence only when it belongs to a context of that same script that the change deactivates or unregisters (§5.5) | supersedes in part |
| 0014 D5 ("Who pushes") | New pushers: a focused ext text box pushes `text_entry` (`EditControls`), priority 50, and `ui.clipboard` (`CopyAndPasteControls`, the same element, right after `EditControls`), as a plugin `TextBox` does, and pops both when it loses focus, so the Ctrl+C, Ctrl+X and Ctrl+V gestures of §5.6 reach it instead of being swallowed at the `opaque_keys` band end; it can gain focus only by the gesture-rooted and visible-and-uncovered focus rules of §4. A hovered ext scrollable pushes `ui.scroll_mouse` for itself, as a native scrollable does, so the wheel over it scrolls instead of zooming | extends |
| 0014 D4 (release-class resolution) | Ext contexts are tried only after D4's two passes over every non-ext context, only if no non-ext release-class binding ended the resolution and no `opaque` context or `opaque_keys` band end stopped it; a release after a UI-owned press never reaches them (§5.5) | supersedes in part (the resolution order of release-class bindings) |
| 0014 D6 (`opaque_keys`) | A key or pad press that ends at the band end because of `opaque_keys`, in either pass, never reaches the ext passes; consent prompts push `ui.eat_keyboard`, its first user (§5.3, §5.5) | extends |
| 0014 D8, step 7 list | Items 6a–6c (ext queue, ext tick, ext collector) between item 6 and item 7. Items 1–6, the plugin queue, its 2 ms plugin slice and the plugin-time rules are **unchanged** | supersedes in part (the list) |
| 0014 D8, "Budget" | Two new time categories: ext time (6a–6c) and ext UI time (native ext UI work in steps 1, 7 and 8), both outside U1 and plugin time, inside the frame rule. D8's step 8 is unchanged: `UI.Native` applies in 6b and ext never hides windows in step 8, so step 8 still changes no box that a hit test reads | extends |
| 0014 D8, client-exit unload | Ext apartments unload at the same point, after the plugins | extends |
| 0014 D8, step 1 | Consent prompts raised outside step 1 are shown at the start of the next frame's redraw, immediately before `offer_pending`; ext windows' activation and focus follow §4's gesture-rooted rule, so no ext call runs plugin code | extends |
| 0004 (scope of M0–M7), 0016 item 2 | **Unchanged.** Ext support on `TesseraProtocol` targets the project-owned test server only, as 0004 says (§6.5); third-party servers need their own decision amending 0004 | none |
| 0015 §7, surface and entry caps; §6 U5 | Ext has **separate pools on top of** UR's caps (512 surfaces, 1,024 entries, 32/64/64 MiB of UI memory); UR's caps stay whole for native panels and plugins, and the renderer's absolute limits become UR's plus ext's; ext entries checked at creation, at every show and at every visibility change; reserved-layer surfaces exempt from every cap and from U5 refusal | supersedes in part (the totals the renderer handles) |
| 0015 §6, row U1 | U1 counts native panels and plugin UI only: Tessera's native work for ext windows in steps 1, 7 and 8 is ext UI time, measured as E8 (§4) | supersedes in part |
| 0015 §6, rows U3 and U4 (b) | Ext paint jobs have their own 1 ms paint slice and 1 MiB upload share; they never use U4 (b)'s slice or U3's cap, so U4 (c) and (d) hold for native and plugin surfaces whatever ext windows do | extends |
| 0015 composition (Decision 1, Decision 1.6, §8 "Order") | The screen-space passes after the world become: **ext overlays** (§3 `Overlay`: the depth-tested ones are drawn with the world's depth at the end of the world passes, the "on top" ones right after the world passes, so both lie under the walker's overlays and the UI), the walker's overlays, the UI pass (with ext windows composed first, in their own z-range below every plugin window and native panel), the developer overlay, the **reserved layer**, the software cursor. The developer console is hidden and takes no input while a consent prompt shows, and ext windows, labels and overlays are not drawn while a prompt shows (§4) | supersedes in part (the order of the screen-space passes) |
| UB §4 (hit test, hover, activation) | The hit-test order gains the reserved layer first and the ext range after plugin windows and native panels, before the world; the one active window and focus are shared with ext windows under §4's rules; ext windows hear pointer moves only over their own mouse-visible elements | extends |
| `chat-window.md` §7.4 step 6, §7.6, §10 items 4 and 6 | A command word starting with `ext:` (ASCII case-insensitive) is intercepted before step 6's lookup and handed to the ext host; it gets no result 3, posts no unknown-command line, is not echoed and is never sent to a backend (§3, `Commands`) | supersedes in part |
| ARCH §3, §3.1 | Allowance `ext` → `script`; rows for `tessera-ext` (L4), `tessera-ext-host` (L5 library), `tessera-scripttool` (app); the `tessera-script` row becomes "runtime core"; L5 may hold a library shared by the client and a tool | extends |
| ARCH §17 | `data/scripts/`, `data/scripts-dev/`, `data/script-store/` (with its local-only marker), `logs/scripts/` | extends |
| ARCH §18 | §18.1 states that the core serves both hosts, that `load`/`loadstring` exist only in the plugin profile, and that the workspace links the project's patched Lua 5.1.5 build whose ext-only modes are off in plugin states (§1.5); new §18.3 "Extended scripts (`tessera-ext`)" summarising §1–§8 | extends |
| ARCH §19.1, §19.3 | `Session::class()`, decided by `Session` from a sealed, crate-private trait of `tessera-net`'s own concrete backends, so only they give `Offline` or `TesseraProtocol` and any wrapper is `Other` (so the M7 backend lives in `tessera-net`); a compatibility backend, and any binary that uses it, is a separate package tagged `gated-compat` and listed in `[workspace] exclude`; the `links` guard shared with `tessera-ext`; the build-level check | extends; §19.3's "could be added later without changes above `tessera-net`" now reads "as a separate crate outside the workspace" |
| ARCH §20.1 | Ext scripts load after start-up steps 8 and 9, at world entry | extends |
| ARCH §20.3 | Step 7 gains 6a–6c; step 1 gains the panic chord | extends |
| ARCH §21 | Script folders are read and hashed on a worker; every `Store` file operation on a worker through one I/O queue per (kind, id), the durable local-only marker and the wipe inside the load's snapshot job (before its result reaches 6a), orphan removal on that queue; script-log lines buffered and appended by a worker; pending writes and log buffers flushed and awaited at client exit, at most the last second of writes lost on a crash; ext's outstanding worker requests capped per script and in total; no new main-thread I/O; new rows for the developer-mode file watcher of `data/scripts-dev/` (a thread or OS callback that sends coalesced change events over a channel to main) and the asynchronous `Files.Pick` dialog (a desktop-portal call or an OS dialog thread whose result comes back over a channel to main) | extends |
| KM §2 | The `ext.` prefix is reserved to the ext host; a separate ext partition of 256 actions and 16 contexts on top of the 1,024 and 64; a new **unregistration** operation for the ext host (removal of one script's set, generational handles, holds ended without edges, recompilation, bindings kept in the ext keymap document, §2) | extends |
| KM §2 ("a pending sequence is dropped" at run-time registration), KM §3 (activate) | The host's registration or unregistration of a script's actions and contexts, and the activation or deactivation of an ext context, drop a pending sequence only when it belongs to a context of that same script that the change deactivates or unregisters; ext context changes are rate-limited (§5.5, §8) | supersedes in part |
| KM §3, §8 | The ext context priority range −1,000 to −1, never opaque or text | extends |
| KM §7.2 | The panic-chord tier before step 1, only while an ext script is loaded, with its collision check against every binding (imported ones included, §12.12 subset matching); ext contexts resolve only after both passes over every non-ext context have failed to consume the press and neither stopped at an `opaque` context or an `opaque_keys` band end; the same order at releases (§5.5) | supersedes in part (the resolution order of step 2) |
| KM §6 (§6.1, §6.2, §6.5) | A second document, `keymap-ext.toml` beside the keymap file, holds every `ext.` binding, with KM §6.1's schema, §6.3's reading and §6.5's reading and saving rules and its own limits (128 KiB, 128 contexts, 64 actions per context); the keymap file never compiles `ext.` names, so its 64-context and 1 MiB caps and its strict reading are unchanged by ext; an orphan's removal removes its id's ext bindings; the lenient reader of an over-limit ext document keeps installed scripts' contexts first (§2) | extends |
| KM §6, binding UI | The collision rule for ext suggested defaults (either pass, any built-in or plugin context, active or not, whatever the `on`); suggested defaults are single strokes and may carry `on`; panic-chord collisions are shown | extends |
| PR §8.4, LPA §3.3 | A note: ext items run after the plugin collector step and never share the plugin queue or slice. No plugin rule changes | extends (note only) |
| PR §4.1–§4.2 (interpreter build and limits) | A note: the workspace links the project's patched Lua 5.1.5 build; its keyed hashing and work counting are off in plugin states, so every plugin-visible behaviour, `pairs` order included, is stock 5.1.5's (§1.5) | extends (note only) |
| PR §4.3, §4.4 (locale) | A note: the patched build orders strings byte-wise in every state, which is §4.4's required result, now independent of the process locale; the client keeps the locale at "C" as §4.3 requires and no in-process component may change it (so `Files.Pick` uses an out-of-process or locale-neutral dialog); the client checks it after every dialog and at every ext load (§1.5) | extends (note only) |
| PR §4.5 (random numbers) | A note: the shared generator belongs to the plugin profile; ext apartments have their own generators and never draw from or reseed it (§1.2) | extends (note only) |
| PR §10.3 (threads) | **Unchanged.** The ext deadline is enforced synchronously on the main thread (§8); no Lua state is touched from another OS thread | none |
| LPA §11.1 | A note: `ext:` command words are handled before the plugin lookup, case-insensitively; they cannot collide with plugin names (which cannot contain `:`), are not built-ins for `IsCommand`/`GetCommands`, are not echoed, and an unknown id posts no chat line | extends (note only) |
| LPA §6.6, PR §11.6 | A note: ext reports never go to the chat window, so they never reach `Turbine.Chat.Received`; plugin reporting is unchanged | extends (note only) |
| LPA §13.14 | A known difference: the reserved layer stays above every plugin window whatever its z-order or activation; the surfaces a script can cause there (lease indicator, notices) are hit- and hover-transparent (§5.3) | extends |
| LPA §13.14, §15 | A note: the group-A `UI.Native` (slice 2) changes native panel positions and visibility as the user's own toggle or drag would, and `Input.Drive` (slice 2, Offline only) changes the player's state that the gameplay views report; neither produces key events (§7) | extends (note only) |
| LPA §13.13, §13.14 | A note: ext windows lie below every plugin window and native panel and share the one active window and the one focus; a script's request never takes them from a plugin window or a native panel, and moves them only after the user's gesture (§4) | extends (note only) |
| `REUSE.toml` (and, if the owner picks it, `LICENSES/MIT.txt`) | The home of the Lua patch (owner question 1): either nothing changes (a fork outside the repository) or one annotated MIT-licensed patch directory is excepted from "no third-party material" | owner's choice |
| ROADMAP | §1 gains the M5.5 row (theme "Extended scripting, slice 1a", new crates `ext`, `ext-host`, `scripttool`); the milestone section is copied from "Roadmap placement proposal"; M5 notes that its split must leave a plugin-free core, and that it measures the M5 plugin set's `Update`, handler and collector time against the 3 ms reserve and the non-paint UI work of event frames against the margin of §8 | extends |
| `crates/net/src/compat.rs` (doc comment) | "one more `Backend`" becomes "a separate crate implementing `Backend`, tagged `gated-compat`" | implementation follow-up |

0016 item 2 is fulfilled by this record once it is accepted. 0004 is unchanged: the
compatibility backend still plugs in without restructuring the client (§6.3), and ext
stays inside 0004's sandbox and project-owned-server scope (§6.5).

### Required follow-ups

This proposal edits no file but itself. When the owner accepts it, the accepting session
makes these edits, each traced to a row of the table above. None of them is made before
acceptance.

1. **`docs/decisions/README.md`:** the new record's row; the 0014 and 0015 rows become
   "Accepted; amended in part by NNNN".
2. **0014, 0015:** one status line each, "Amended in part by NNNN (…)", naming the
   superseded points (0014 D4; D5's band order and its "`activate` keeps today's rule"
   for ext contexts; D7's `handle_with` order for the panic-chord tier; the D8 step-7
   list; 0015 §7 and U5's totals with ext's separate pools; 0015 §6 row U1; 0015
   Decision 1.6 and §8 "Order" for the ext overlays at the start of the screen-space
   passes and the reserved layer above the developer overlay) and the extensions (0014
   D5's new "Who pushes" rows for the ext text box, with `text_entry` and
   `ui.clipboard`, and ext scrollables, D6, D8's time categories; 0015 U3 and U4 (b) with ext's own slice and
   upload share); no other text changes. 0004 and 0016 get no edit.
3. **`docs/ARCHITECTURE.md`:**
   - §3 and §3.1: the allowance `ext` → `script`; rows for `tessera-ext` (L4),
     `tessera-ext-host` (L5 library, with its optional `bridge` feature) and
     `tessera-scripttool`; the `tessera-script` row becomes "runtime core"; L5 may hold a
     library shared by the client and a tool;
   - §17: `data/scripts/`, `data/scripts-dev/`, `data/script-store/` (with the local-only
     marker), `logs/scripts/`;
   - §18.1: the core serves both hosts, `load`/`loadstring` exist only in the plugin
     profile, and the workspace links the project's patched Lua 5.1.5 build with its
     ext-only modes off in plugin states; new §18.3 "Extended scripts (`tessera-ext`)"
     summarising §1–§8;
   - §19.1 and §19.3: `Session::class()`, decided from a sealed, crate-private trait of
     `tessera-net`'s own concrete backends (any wrapper is `Other`); a compatibility
     backend and its binary are
     packages tagged `gated-compat` and listed in `[workspace] exclude`; the `links` guard;
     the build-level check;
   - §20.1, §20.3: ext load after start-up steps 8–9; step 7 items 6a–6c; step 1's panic
     chord (only while ext scripts are loaded) and the consent prompts shown before
     `offer_pending`; intents dropped on unload and checked at apply time (session
     generation, backend class, grant);
   - §21: script folders hashed on a worker; every `Store` file operation on a worker
     through one I/O queue per (kind, id), with the local-only marker and the wipe done
     in the load's snapshot job; script-log appends on a worker; the exit flush and
     what a crash loses; ext's capped outstanding worker requests; rows for the
     developer-mode file watcher and the asynchronous `Files.Pick` dialog, each with its
     channel to the main thread.
4. **`docs/specs/keymap.md`:**
   - §2: the reserved `ext.` prefix; the separate ext partition of 256 actions and 16
     contexts on top of the 1,024 and 64; the **unregistration** operation of this
     record's §2 (one script's set at once, generational handles, holds ended without
     release edges, recompilation, bindings kept in the ext keymap document); the host's registration
     and unregistration of a script's set drop only that script's pending sequence,
     never a non-ext one;
   - §3: activating or deactivating an ext context drops only a pending sequence of a
     context of that same script that it deactivates; ext context changes are
     rate-limited;
   - §3 and §8: the ext priority range −1,000 to −1, never opaque or text;
   - §6 (binding UI and suggested defaults): the collision rule of §5.5, whatever the
     `on`; suggested defaults may carry `on`; panic-chord collisions shown;
   - §6.1, §6.2 and §6.5: the second document `keymap-ext.toml` for every `ext.`
     binding, with its own limits (128 KiB, 128 contexts, 64 actions per context), the
     keymap file never compiling `ext.` names, the removal of an orphan's bindings, and
     what the lenient reader of an over-limit ext document drops first (this record's
     §2);
   - §7.2: the panic-chord tier before step 1, present only while ext scripts are
     loaded, and its collision check against every binding; the ext passes after both
     non-ext passes, with the `opaque` and `opaque_keys` stops; the same order for
     release-class resolution (§5.5; IB D4 and D6).
5. **`docs/ROADMAP.md`:** §1 gains the M5.5 row (theme "Extended scripting, slice 1a",
   new crates `ext`, `ext-host`, `scripttool`); the milestone section is copied from
   "Roadmap placement proposal"; M5 notes that its split must leave a plugin-free core,
   that its U1 criterion counts native panels and plugin UI only, and that it measures
   the M5 plugin set's (M5 exit criterion 4) plugin `Update`, handler and collector-step
   time against the 3 ms reserve, and the non-paint native and plugin UI work of event
   frames against the margin, of this record's §8 ("The frame budget").
6. **Plugin specs (notes only, no rule changes):** PR §8.4 and LPA §3.3 (ext items after
   the plugin collector step, separate queue and slice); LPA §11.1 (`ext:` words, case,
   no echo, no chat line for an unknown id); LPA §6.6 and PR §11.6 (ext reports never in
   chat); LPA §13.13–§13.14 (the reserved layer and its hit-transparent surfaces, the
   ext z-range below plugin windows, and the ext activation and focus rule); LPA §13.14
   and §15 (`UI.Native` and `Input.Drive` effects); PR §4.1–§4.2 (the patched build,
   stock in plugin states), PR §4.3–§4.4 (byte-wise string order in every state; the
   locale kept at "C" with no in-process component allowed to change it) and PR §4.5
   (the shared generator is the plugin profile's). PR §10.3 gets no edit.
7. **`docs/specs/tessera-protocol.md` (written with M7; slice 3):** the script policy with
   `allow` and `forbid`, the default policy (and that only leaving `entity.read` out of
   `allow` stops a client-side through-wall view, §6.5), the server's
   interest-management requirement, the `Server` message limits of §8, and that `/ext:`
   lines are never sent to the server (§3).
8. **New specs, written in M5.5:** `ext-api.md` and `ext-manifest.md` (or a section of
   `ext-api.md`), as listed under "Specs needed".
9. **Implementation follow-up (a clean-room implementation session, not the accepting
   session):** the doc comment of `crates/net/src/compat.rs`.
10. **The Lua patch's home (owner question 1):** if the owner picks the in-repository
    option, `REUSE.toml` gains one annotation for the MIT-licensed patch directory and its
    comment's "no third-party material" sentence names that exception, and
    `LICENSES/MIT.txt` is added; if the owner picks the external fork, nothing in this
    repository changes beyond the pinned dependency that slice 1a adds.
11. **`docs/specs/chat-window.md`:** §10 items 4 and 6 gain the Tessera rule for `ext:`
    words (intercepted before the lookup, no result 3, no unknown-command line, no echo,
    never sent to a backend), and §7.4 step 6 and §7.6 get a note pointing to it; the BIN
    descriptions of the original stay as they are.
12. **`docs/specs/ui-behaviour.md`:** §4 gets a Tessera note: the reserved layer first,
    the ext range after plugin windows and native panels, the shared active window and
    focus under this record's §4, and pointer notifications to ext windows only over
    their own mouse-visible elements.

### Effects

- **Plugins.** The plugin API, its limits (512 MiB, 500 million instructions), its queue,
  its 2 ms slice, its command table and lists, the plugin manager panel and every
  plugin-visible behaviour are unchanged; plugin states run the patched Lua build in its
  stock mode (§1.5). Plugins can observe ext scripts only through the
  channels of §7.
- **Shared engine limits.** Ext takes nothing from UR's surface, entry and memory caps,
  its paint slice and upload cap, or KM's registry: it has **separate pools on top of
  them** (512 surfaces, 1,024 entries, 32/64/64 MiB of UI memory, a 1 ms paint slice, a
  1 MiB upload share, 256 actions and 16 contexts), so native panels and plugins keep
  every unit they have without ext. Ext bindings live in their own keymap document, so
  the keymap file's caps are untouched (§2). Ext asset requests run at the lowest worker
  priority in their own cache partition, with capped outstanding requests. The reserved
  layer is exempt (§8).
- **Input with no ext script loaded.** Nothing changes: the panic-chord tier exists only
  while an ext script is loaded, and ext contexts exist only for loaded scripts.
- **Frame time.** Ext adds at most 2 ms of started work and a 1 ms collector slice per
  frame, plus bounded overruns: 7 ms + R + 2·S in all, at most 13 ms once slice 1a's
  measurements are applied (§4, §8), including during a backend switch, and nothing in a
  frame that is 22 ms old when step 7 reaches ext. Ext UI time in steps 1, 7 and 8 (no
  Lua) comes on top, within E8's 2 ms at the ext caps. Together they fit the 50 ms frame
  rule in the M4.5 walking run beside its worst re-run frame (21.8 ms), the UI paint
  slice and the plugin slice and reserve, with a 4.2 ms margin (§8, "The frame budget").
  In M3's fast flights the rule is not promised with ext at its ceiling; the residual is
  recorded and measured (§8). U1 counts native panels and plugin UI only.
- **Plugin code and ext time.** No ext call or item runs plugin code; plugin handlers that
  ext activity leads to run in step 1 or step 7's plugin items, as plugin time (§4, §7).

## Open items

> **OPEN:** the panic chord default (proposed Ctrl+Shift+F12) and whether it should also be
> on a gamepad chord.

> **OPEN:** the exact cost of every native and of every costly VM operation of the patched
> build (E7). Slice 1a measures them with the patched build and writes them into the
> registry and the patch's per-operation factors; the figures of this record come from
> the operating system's Lua 5.1.5.

> **OPEN:** the patched Lua build (§1.5): its home (owner question 1), how mlua links it
> (the source crate of its `vendored` feature patched, or its external-library mode), how
> the apartment profile selects the mode before the state's first string, and the keyed
> hash function and its cost (SipHash-1-3 is the reference). Slice 1a settles these and
> shows that M5's plugin tests pass unchanged on it.

> **OPEN:** whether M5's `script`/`plugins` split already leaves a core with no `Turbine`,
> `import`, `Shared` or plugin-shaped `HostApi` (§1.1). If it does not, slice 1a finishes
> it first.

> **OPEN:** the uninterruptible residuals of §8: the worst concatenation with its keyed
> hash, table resize or rehash, long-string comparison (also under a UTF-8 collation
> locale, where it must not change) and sparse-table `next` under the 4 MiB allocation
> cap, and the worst interning-table rehash under the 64 MiB apartment cap (R), the
> worst atomic collector step under the 64 MiB apartment cap (S), the worst compilation
> of a 128 KiB file (K) and the worst state close (C). Slice 1a measures them with the
> patched build and applies the recorded fallback if 7 ms + R + 2·S exceeds 13 ms, or K
> or C exceeds 9 ms. It also measures the cost of converting a maximal value of the
> bounded value model in both directions (E7) and lowers the element cap if it takes
> more than half of a 4 ms call.

> **OPEN:** the 3 ms reserve for plugin `Update`, input handlers, the last plugin item's
> overrun and the plugin collector step (IB D8 step 7 item 6, unsliced, growing with
> plugin heaps up to PR §9.4's 512 MiB) in §8's frame budget, and the non-paint native
> and plugin UI work of event frames (offers, hit tests, the tick, layout) that the
> 4.2 ms margin holds. M5 measures the M5 plugin set (ROADMAP M5 exit criterion 4)
> against both; if the reserve needs more, or the non-paint UI work more than 2 ms, the
> ext ceiling of 13 ms drops by the difference before M5.5 ships.

> **OPEN:** the heavy-frame gate's threshold (22 ms elapsed when 6a would begin, §4) and
> how many frames of M3's fast flights it skips. Slice 1a records, for M3-a and M3-d
> with the hostile scripts, the frames over 50 ms with and without ext and the frames
> the gate skipped, and tunes the threshold; the flight residual of §8 stays recorded
> whatever the result.

> **OPEN:** that the renderer handles UR's caps plus ext's separate pools together
> (surfaces, entries, GPU memory). Slice 1a confirms it with E8; if it cannot, the ext
> pools shrink, never UR's.

> **OPEN (before slice 3):** the server-side half's accounting, grants, `Store` and limits,
> within the constraints of §11.

> **OPEN:** that the patched build's work counting reaches every costly VM operation
> (§1.5, §8 "Deadline at the next instruction"). Slice 1a verifies it with E6's hostile
> scripts, in the main thread and in coroutines made at any time and driven through
> `coroutine.resume` and `coroutine.wrap`. If an operation is found that it misses, it is
> added to the patch before any ext script ships. No asynchronous arming is used, so PR
> §10.3 needs no amendment.

> **OPEN:** whether Cargo leaves an unactivated optional dependency out of the `links`
> check of an excluded package's own resolve (§6.3). Expected: it does not, because the
> lock file is resolved with every optional dependency, so slice 1a plans for the CI
> check alone and keeps the `links` guard only if its fixture passes.

## Grafts and where they land

| # | Graft (source) | Where |
|---|---|---|
| 1 | Leases for every override, with a 350 ms lifetime (250 ms before revision 4) renewed by the script, ending on expiry, watchdog stop, unload, revocation, backend change or the panic chord; on-screen indicator (B) | §5.4, §3 `Camera`, `Input.Drive` |
| 2 | Panic chord as a mapper tier before the pending sequence, offers and every IB D5 band; rebindable only in Tessera's settings (B) | §5.5 |
| 3 | Prompts and the script manager in a reserved top layer, exempt from caps; no synthesised input; escape test (B) | §5.3, criterion 2 |
| 4 | Grants pinned to the capability list, author and a canonical content hash, keyed by (kind, id); group A re-asked on change except in dev mode (B) | §5.2, criterion 5 |
| 5 | One-way plugin bridge replayed as user actions; no plugin-data writes, no chat injection (B) | §7 |
| 6 | Explicit "never offered" list, whole refusal of unknown capability or major (B, A) | §5.6, §2 |
| 7 | Backend class plus CI dependency check on a tagged separate crate; the official-server boundary (A, B) | §6 |
| 8 | Language-neutral registry (ports plus binding description, with costs); the mlua constraint recorded (A) | §1.3, §1.4, §11 |
| 9 | Chunky API, one frame, clear errors on stale handles with `Exists()` (A) | §3 API rules |
| 10 | Generated reference and annotations; headless check and test tool on a synthetic world (A, B) | §10 |
| 11 | Manifest settings schema shown as a settings page; `Ext.Has` and version probes (A, C) | §2 |
| 12 | Server-side half run in-process in the sandbox (A) | §11, slice 3 |
| 13 | Per-script profiler line; throttle, then suspend with Restart and Disable; rate-limited reports (B, C) | §8 |
| 14 | Confirm the `script`/`plugins` split in M5, or finish it in slice 1a (judge 1) | Owner question 2, §1.1, criterion 6, Open items |
| 15 | Low default memory; `input.drive` offline only; fuzz targets for the manifest and the grant store (judge 2) | §8, §6.5, §2, §5.2, deliverables |
| 16 | `Files.Pick` where the OS dialog is the grant (A) | §3 |
| 17 | `pack` refuses `.dat`/`.datx` and archive-headed files (A) | §10 |

## Revision 6: answers to the sixth review (2026-10-06)

As before, where a fix could remove a class of problems rather than one case, this
revision took it: one ordered I/O queue per store instead of a special case for the wipe,
one budget in two measures for units and deadlines, a separate keymap document instead of
a shared allowance, byte-wise string order in the interpreter instead of a locale check
alone, a fixed-latency worker model for deterministic runs, and caps on everything ext
puts on a worker.

### Must-fix items

1. **A focused ext text box could lie under the chat window.** Fixed (§4, "A focused ext
   text box is always visible and uncovered"). A box can be focused only when its focus
   area is not covered by anything a higher layer draws or that catches the pointer
   (plugin windows and native panels at any opacity, the developer overlay, the
   reserved layer); the transparent area of a full-screen root panel does not count.
   The conditions are re-checked at the end of every step that can change them
   (step 1's user drags and toggles, step 7's plugin items, 6b, the item-7 drain and
   step 8's layout), and a box that fails loses the focus at once, so no key of the next
   frame is routed to a covered box. Exit criterion 2 places a box under the chat window
   and under an opaque plugin window, and moves a plugin window over a focused box.
2. **The write-behind `Store` could undo the local-only wipe.** Fixed structurally (§5.2
   rule 3, §8 "`Store` I/O"): every file operation of one (kind, id) store (open with
   marker or wipe, reads, write-behind flushes, the final flush at unload, wipes,
   orphan removal) runs in order on that store's own worker queue. At unload the final
   flush is queued in the frame of the close item, before the apartment counts as gone;
   the next load's open is queued after it, and its snapshot result waits for that open.
   Writes are written to a temporary file and renamed. A §6.6 test switches the grant
   with one write pending and one in flight.
3. **Bridge reads were outside the value model and outside metered delivery.** Fixed
   (§7 bridge item 4, §4, §8): the bridge calls the plugin-data reader with the value
   model's caps (1 MiB, 32 levels, 8,192 elements; "too large" past them, on the
   worker), and delivers the value as a 6b notification converted inside the metered
   call. Bridge reads are in §4's list, the value model's list and "Value conversion is
   not a residual"; E6 reads a maximal plugin-data file.
4. **In-flight worker requests had no cap.** Fixed (§8, "Outstanding worker requests are
   capped", and the table): at most 32 outstanding requests per script and 256 in all
   (`Store` reads, decode requests, bridge reads, `Files.Pick`), refused with
   `nil, "busy"`; decode requests de-duplicated across scripts; results that come back
   to a script reserved against its memory cap before they are queued; `Store` reads
   rate-limited (64 reads and 4 MiB per second per script, 16 MiB per second in all).
   E6 floods reads and lookups; exit criterion 2 checks the cap.
5. **String order depended on the locale.** Fixed in the interpreter (§1.5): the patched
   build orders strings byte-wise in every state, the result PR §4.4 already requires,
   charged by length in ext states. The client keeps the locale at "C" as PR §4.3
   requires, no in-process component may change it (so `Files.Pick` never runs a
   toolkit that calls `setlocale`), and the client checks it after every dialog and at
   every ext load, suspending ext if it ever changed. E6 runs the comparison loop under
   a UTF-8 collation locale in a test build. A PR §4.3–§4.4 note is a follow-up.
6. **The frame budget used the superseded first-run figure.** Re-derived (§8, "The frame
   budget") from the M4.5 exit re-run: P4's 21.8 ms. With the old rows the sum was
   52.8 ms. The ext ceiling is now **13 ms**, reached by halving the per-frame cap to
   **2 ms**, cutting the call deadline to **4 ms** (with the units cap at 800,000, the
   same budget) and halving the largest single allocation to **4 MiB** (R ≈ 3 ms), so
   the bound is 7 ms + R + 2·S, with K and C at most 9 ms. Rows are 21.8 + 4 + 2 + 3 + 2
   + 13 = 45.8 ms, leaving a 4.2 ms margin. E6's 50 ms claim is scoped to the M4.5
   walking run. M3's fast flights (28.8–34.5 ms in the re-run) cannot hold the ceiling,
   so the record adds a **heavy-frame gate** (§4: ext starts nothing in a frame already
   22 ms old when 6a would begin) and records the residual for flight frames whose cost
   comes after step 7. E6 measures those flights without making them a pass criterion.
   The budget stays defined on wall time. Main-thread time was rejected as the measure,
   because the frame rule is a wall-time rule and the overlap with GPU waits is not
   guaranteed.
7. **The E6 backend-switch case did not fit the memory sum.** Restated (§4 "Backend
   switch", E6) as two cases inside §8's 512 MiB sum plus the reload allowance: 8 at
   64 MiB plus a 64 MiB reload apartment (9 closes at the worst C), and 16 at 32 MiB
   plus a reload apartment (34 items). The switch bound is now derived from that:
   at most 34 frames that the heavy-frame gate does not skip.
8. **Ext bindings could fill the shared keymap document.** Fixed by a separate document
   (§2, "Ext bindings have their own keymap document"; KM §6 row and follow-up 4):
   `keymap-ext.toml` holds every `ext.` binding, with its own limits (128 KiB, 128
   contexts, 64 actions per context). The keymap file never compiles `ext.` names, so
   its caps and strict reading are exactly KM's. An orphan's removal removes its id's
   bindings. The binding UI refuses a save over a limit and offers to remove bindings of
   scripts that are not installed. The lenient reader keeps installed scripts' contexts
   first. Exit criterion 2 binds keys for 70 ids and still saves a built-in rebinding.
9. **Deterministic mode depended on worker timing.** Fixed (§8 "Deterministic mode",
   §10): the harness delivers every worker result at frame N + k in submission order
   (a fixed-latency completion queue), and gates streaming on residency as ARCH §20.3
   does for collision sets, so the resident set and every query's answer are functions
   of the walk. The call limit is 800,000 units, the same as in the client. Exit
   criterion 7 varies real worker timing and expects identical traces.

### Other items

- **`Store` path.** §3's row now says `data/script-store/<kind>/s_<id>/`, as §2 does.
- **Host userdata metatables.** Sealed with `__metatable` as a requirement for every
  host userdata, with escape tests (§1.2).
- **Error values with `__tostring`.** The host describes non-string values without
  metamethods (type name and `tostring` number) on every path outside a script's own
  call (§1.2).
- **Raw walks, invalid keys.** The value-model walk uses raw access only and ignores
  metatables; nil or NaN keys (and duplicate keys) in a parsed file or message are
  refused as `nil, "invalid"` on the worker (§8).
- **Keyed hashers** now cover the neutral reader, the `Server` parser, the ext keymap
  reader and every worker-path map (§8).
- **Interning table.** It gets the 32-node re-salt too; interning work is charged at the
  32-node bound; its rehash is part of R (§1.5, §8).
- **§5.2 rule 2** reworded: no apartment of a `TesseraProtocol` session holds
  `data.read`; the offline loopback case belongs to the residual.
- **Plugins as cooperating parties.** Named in §5.2, §5.6, §12 and owner question 4;
  "no single script can" became "no single script can on its own".
- **Ext sequences.** §7 lists the press that a user-bound ext sequence takes in KM §7.2
  step 1 as a channel caused by the user's keymap edit; §5.5 points to it.
- **Reserved-layer text.** Every string there that is not Tessera's (errors, log lines,
  command names, labels, an echoed `/ext:` id) is plain text, stripped and capped, inside
  a Tessera frame (§5.3).
- **Prompt clickjacking.** Ext windows, labels and overlays are hidden while a prompt
  shows, and "Allow" counts only after the pointer entered the button after the delay
  (§4, §5.3). World entry is defined; no ext action queues an autoload (§4).
- **Activation case 1** also ends when the pointer leaves the script's windows or after
  5 s without the user's input there (§4).
- **Hard links** in a script folder refuse the script (§2).
- **Global pools** as a cross-script influence channel are listed in §5.2's residual.
- **Group O and pointer capture.** Stated as the one exception, with the `ui` grant text
  saying so (§3).
- **Panic chord.** It stops queued items that would run a suspended script's code, and a
  suspended script unloads without its `Unload` handler (§4, §5.5).
- **Budget table.** The plugin collector step is in the 3 ms reserve row and its OPEN;
  non-paint UI work of event frames is named in the margin, with an M5 measurement.
- **Deterministic units.** 800,000 per call, mirroring the client (must-fix 9).
- **ARCH §21.** Marker and wipe in the snapshot job, log lines appended by a worker,
  exit flush and crash behaviour stated (§4, §8, Consequences, follow-up 3).
- **IB D5 "Who pushes".** An ext text box pushes `ui.clipboard` beside `text_entry`.
- **0015 §8 "Order".** Ext overlays come first among the screen-space passes, before the
  walker's overlays and the UI pass (Consequences, follow-up 2).
- **§1.1 and the deliverables.** All ext file I/O is in `tessera-ext-host`;
  `tessera-ext` only handles bytes it is handed.
- **`links` guard.** The expected outcome (the guard fails) is now the plan; the CI check
  is the deliverable (§6.3, OPEN).
- **D7 and the chord.** D7's steps 1 and 2 still run for earlier events; only the chord
  press is consumed (§5.5, Consequences).
- **Registry arithmetic.** Owner question 8 and the contexts row say that at most 8
  two-context scripts or 4 scripts with 64 actions each fit together.
- **"M5 HUD set"** replaced by "the M5 plugin set (ROADMAP M5 exit criterion 4)".

## Revision 5: answers to the fifth review (2026-10-06)

As in revision 4, where the review offered a choice, this revision took the fix that
removes a class of problems: separate pools instead of shares and yields, conversion
inside metered calls instead of a new residual, a keyed mixing rule instead of a case
list, prefixed file names instead of a list of bad names alone, and hot reload limited to
the one folder kind whose grants allow it.

### Must-fix items

1. **Salt mixing left flooding open.** Fixed by a normative rule (§1.5, "How the salt is
   mixed"): in ext states the main position of every key kind is a keyed pseudo-random
   function of the **whole** 32-bit hash and the table's salt (reference: multiply-shift
   with a random odd 64-bit multiplier, high bits; a keyed SipHash of (salt, hash) is
   accepted); XOR or addition followed by a low-bit modulo is forbidden. The interning
   table uses the same mixing with its own salt. As defence in depth, every chain-walk
   step beyond 32 nodes (get, set, interning) is charged to the hook counter. E6 gains
   keys whose stored hashes share their low 17 bits, inserted into fresh tables of every
   size after re-salting, with 1,000 gets between two hook checks.
2. **Host objects outside every cap.** Fixed both ways (§4, §8). The per-script memory cap
   now covers the host objects a script creates (elements, labels, timers,
   subscriptions, queued notifications, intents, pending `Store` writes, `tostring`
   numbers), each charged a declared size plus its variable bytes (E5 checks the
   declared sizes). Count caps bound the rest: 2,048 elements per script (512 per
   window, 256 world labels) and 8,192 in all (1,024 labels), 64 timers, 16
   subscriptions (64 in all), a 1,024-entry notification queue with coalescing of state
   notifications and counted drops, and 256 pending intents per frame. Subscription
   filters run in the script's own 6b turn, never at spawn time, so no ext filter work
   runs outside ext time. Suspension stops accrual (no events recorded, timers stopped,
   pending notifications dropped with counts, a `Resumed` notification on resume). E8
   is measured at the element caps, which bound step-8 layout.
3. **Windows device names.** Fixed structurally and by grammar (§2). Script-chosen strings
   are never bare file names: `s_<id>` folders and logs, `k_<key>.store` files. Every
   file under `data/` and `logs/` is opened relative to a directory handle (`openat`;
   `NtCreateFile` relative to the parent handle) or through `\\?\` paths. As a second
   layer, `con`, `prn`, `aux`, `nul`, `com0`–`com9` and `lpt0`–`lpt9` are reserved as
   ids, `Store` keys and folder path segments on every platform, with clear reports from
   `scripttool check`, the load and `Store`. Exit criterion 2 has the escape test.
4. **Hot reload against grant pinning; ids across folder kinds.** Fixed (§2, §5.2, §10).
   Hot reload exists only for the `scripts-dev` kind with developer mode on, the one
   kind that keeps grants across content changes. `/ext:reload` of an installed script
   is a full unload, content check (re-asking group A on a changed hash) and load, with
   no hand-off table. At most one script per id is loaded across both kinds; loading
   the other copy is refused with a report, so bindings, `/ext:` lines and the gestures
   they carry always reach the one loaded copy. Exit criterion 5 tests both.
5. **Contention on shared pools.** Fixed by removing the sharing (§8, §7). Ext's surfaces,
   composition entries, UI memory, paint slice, upload share and key-registry space are
   **separate pools on top of** 0015's and KM §2's caps, which stay whole for native
   panels and plugins; the renderer's absolute limits become the sum (an OPEN that E8
   confirms). So a plugin's window, image or registration succeeds or fails exactly as
   with no ext script, and the composition-entry "yield" is gone. Ext asset requests run
   at the lowest worker priority with their own cache partition; the remaining effect,
   a plugin decode waiting for one ext job a worker already started, is timing and is
   listed in §7 and owner question 4.
6. **The worst-frame bound missed 6a item kinds.** Fixed by moving every value-carrying
   result out of 6a (§4): 6a now has exactly five kinds (snapshot result, compilation,
   top-level run, `Unload`, close), and a new kind must join the bound first. `Store`
   results, `Files.Pick` results and `Server` messages are notifications delivered in
   6b, converted inside the metered delivery call with their cost charged first.
7. **Activation case 2 contradicted the invariant.** Fixed by dropping the chat-window
   exception (§4): case 2 requires that no window is active. The absolute rule ("never
   from a plugin window or a native panel, the chat window included") now holds in §4,
   §7, owner question 4 and the Consequences row as written; exit criterion 2 tests that
   an `/ext:` line from the chat input does not focus the script's text box.
8. **The frame budget did not fit 50 ms.** Fixed by deriving the ext ceiling (§8, "The
   frame budget"): 50 ms minus the worst measured walking frame (15.5 ms), the UI paint
   slice (4 ms), the plugin slice (2 ms) and a 3 ms plugin reserve, ext UI time (2 ms,
   now covering native ext work in steps 1, 7 and 8) and a 3.5 ms margin leaves
   **20 ms**. To fit it, the per-frame cap drops from 6 ms to **4 ms** and the top-level
   run's deadline from 10 ms to **5 ms** (2 million units, like any call), so the bound
   is 10 ms + R + 2·S and the fallback target is 20 ms. The run E6 judges is stated, and
   so is the residual risk of unsliced plugin time (with an OPEN for M5 to measure the
   reserve).
9. **Native value conversion was uninterruptible.** Fixed by the bounded value model and
   metered conversion (§8): every value crossing the host boundary is a tree with no
   cycles, at most 32 levels, 8,192 elements and a byte cap counted on the serialised
   form, all checked during an iterative walk that stops at the first bound. Reading
   converts inside a metered call with its cost known from the worker's counts;
   serialising (`Store.Set`, the hand-off table, `Server` sends, table arguments)
   reserves the cost of its cap first and refunds the rest. "Value conversion is not a
   residual" is stated in §8; E6 (a maximal value read back, a maximal hand-off table, a
   shared-subtable graph, a 10^5-deep chain, a cycle) and E7 (conversion calibration)
   test it, and the fallback lowers the element cap if needed.
10. **Unregistration did not exist in KM.** Fixed by an "extends" amendment to KM §2 (§2,
    "Unregistration in the key registry"): removal of one script's set in the ext
    partition, generational handles, holds ended without release edges, recompilation
    as for registration, keymap-file entries kept. Slice 1a implements it in
    `tessera-input`; exit criterion 5 loads and unloads 64 scripts in turn.
11. **Two step-8 changes conflicted with IB D8.** Both removed (§3, §4, §8): `UI.Native`
    applies at once in 6b, like other UI changes, and reaches hit tests at the next hit
    test or hover refresh; the composition-entry yield no longer exists (must-fix 5).
    Step 8 again changes no box that a hit test reads; the D8 "Budget" row says so.
12. **The E6 backend-switch case could not be built.** Restated as 8 apartments at 64 MiB
    plus 8 at 32 MiB, with the worst C measured at 64 MiB.
13. **Consequences missed two accepted texts.** Added: `chat-window.md` §7.4 step 6, §7.6
    and §10 items 4 and 6 (supersedes in part; follow-up 12), and 0015 §6 row U1
    (supersedes in part), with rows for U3 and U4 (b) (ext's own slice and share), 0015's
    composition order (the reserved layer above the developer overlay), UB §4 (extends;
    follow-up 13) and D7. Follow-up 2 names them.

### Other items

- **Through-wall views (§6.5, §5.2, owner question 3).** Only a policy that leaves
  `entity.read` out of `allow` works on the client side; `audio` alerts and the script
  log (followed by another program, or the developer console) defeat combination bans.
  The warnings gain `entity.read` + `audio` and `entity.read` alone on a server; the
  `forbid` example is now `["camera", "entity.read"]`.
- **Serialisation bounds.** The bounded value model (must-fix 9): cycles refused, depth
  32, elements capped, walk stopped at the byte cap, sizes measured on serialised bytes.
- **Native costs and world state.** "Natives never wait" (§8): resident or cached data
  only, `nil, "not loaded"` otherwise, no synchronous decode, streaming or main-thread
  I/O; rays at most 500 m; ext asset requests at the lowest priority with a cache
  partition (16 MiB per script, 64 MiB in all).
- **Persistence outside the Store.** No other per-script state persists (§5.2). A grant
  change between local-only and `session.server` takes effect only at a full unload and
  load; "opened" means loaded; the local-only marker is written durably before the
  top-level run.
- **Silent keystroke capture.** A focused ext text box must be visible, on screen,
  unclipped and at least half opaque, with a caret and focus mark the host draws; focus
  is refused or taken away otherwise (§4).
- **Spoofing.** Manifest strings are plain text, stripped of control and bidi
  characters, length-capped, and shown beside the id and folder kind; "Allow" needs a
  press and a release on the button, the press starting after 500 ms (§5.3).
- **`Files.Pick` and the panic chord.** Reworded: the chord cannot reach a window without
  keyboard focus, so every lease ends when the game window loses keyboard focus,
  including when the dialog opens (§5.4, §8).
- **Layout channel.** Listed in §5.6 as layout, not content, and narrowed: ext windows
  hear pointer moves only over their own mouse-visible elements, once per frame.
- **Persistent covert channels.** Named in §5.2 ("persists across sessions"), owner
  question 4 and §12.
- **Patched-build hygiene.** `tostring` shows a per-apartment number, never an address
  (§1.2); the build carries every official lua.org fix for 5.1.5 in both profiles
  (§1.5); E6 covers deep C recursion through coroutines, `gsub`, `sort` and metamethods.
- **Host-side maps** use std `RandomState` (§8).
- **Folder walk.** At most 512 entries of any kind and 64 directories, counted as read,
  and the listing stops at the cap (§2).
- **`Audio` caps.** Voices, starts per second and summed gain per script and in total
  (§8).
- **`/ext:` lines** are never sent to any backend (§3).
- **Slice 3's server half.** Constraints fixed in §11 (counted as a second script and in
  the memory sum, no local-only capability, its own non-local-only `Store`), with an
  OPEN to define the rest before slice 3; §8's "Loaded scripts" row counts it.
- **Clipboard.** No clipboard API; Ctrl+C, Ctrl+X and Ctrl+V in an ext text box are user
  gestures, documented (§5.6).
- **Images and the debug adapter.** Ext images pass SK D4 and D18's header checks, are
  refused above 4,096 pixels on an axis, and are charged before decoding (§8). The slice-4
  debug adapter's requirements are fixed now (§10, slice 4).
- **`BackendClass` leaked by delegation.** `Session` decides the class from a sealed,
  crate-private trait of `tessera-net`'s concrete backends; the `Backend` trait has no
  class method; a wrapper is `Other`, and a slice-1a test checks it (§6.1, §6.6).
- **Reserved layer and the developer overlay.** The reserved layer is above the
  developer overlay; the console is hidden while a prompt shows (§5.3, §10).
- **U1.** All native ext UI work in steps 1, 7 and 8 is ext UI time (E8), with its own
  1 ms paint slice and 1 MiB upload share (§4).
- **Determinism.** Deterministic mode counts every per-frame slice in units and derives
  every salt from the seed (§8, §10).
- **ARCH §21.** The watcher and the `Files.Pick` dialog are listed with their channels
  (Consequences, follow-up 3).
- **Overlays and labels.** An unticked frame draws the last tick's overlay list; world
  labels are persistent nodes (§3).
- **Panic-chord order.** "Before any event of this press is resolved or offered", before
  IB D7's steps 1 and 2 (§5.5).
- **`Input` apply column** aligned with the intents table (§3).
- **UI links.** Hovered ext scrollables push `ui.scroll_mouse` (§4, D5 row); UB §4 has an
  "extends" row.
- **`tessera-ext` and settings.** Grant persistence and the settings page are in
  `tessera-ext-host`; `tessera-ext` only parses and validates (§1.1).
- **"(§12)" references.** Now "this record's §12" (§12).

## Revision 4: answers to the fourth review (2026-10-06)

Where the review offered a choice, this revision took the fix that removes a whole class
of problems rather than the single case.

### Must-fix items

1. **`string.rep` charged by result size only.** Fixed by a rule, not a row (§8): every
   wrapper charges the largest of bytes read, bytes produced and the loop iterations its
   arguments imply, computed from the arguments; degenerate arguments (empty `s`, `n` ≤ 0,
   empty ranges) are answered by the wrapper without calling the stock loop, and
   out-of-range positions are refused. `string.rep` is charged max(n, result size).
   Rechecked every wrapper: `table.concat` (range j − i + 1, which loops over empty
   strings), `unpack` (range before the stack check), `table.insert`/`table.remove`
   (n − pos + 1; a position below 1, which made stock 5.1 loop toward −2^31, is refused),
   `string.format` (argument count). E6 and E7 gain the degenerate cases.
2. **Hash flooding.** Fixed structurally by the project's patched Lua 5.1.5 build (new
   §1.5): in ext states every string's hash covers every byte and is keyed with a
   per-state secret, number and pointer keys use the same key, interning compares the
   stored hash before the bytes, and tables have per-table salts with a 32-node chain
   bound (a longer chain re-salts the table). So no chain walk grows with the
   apartment's memory, and compiling hostile source stays linear. §8 "Largest single
   allocation" now separates size-bounded operations (allocation cap) from
   collision-bounded ones (§1.5); R, the compile residual (now K) and E6 carry the
   flooding cases. The plugin profile keeps stock hashing, so `pairs` order and every
   plugin-visible behaviour are unchanged. The patch needs a home outside or excepted
   from `REUSE.toml`'s "no third-party material"; owner question 1 asks.
3. **Backend switch broke the frame bound.** Fixed by removing the exception (§4): every
   unload is two ordinary ext items (the `Unload` handler, then the state close) under
   the 1 ms slice with the at-least-one-item rule. §8's worst frame now lists every item
   kind (top-level run, `Unload` handler, compilation K, close C) and holds at
   12 ms + R + 2·S with K ≤ 10 ms and C ≤ 10 ms; the fallback and the OPEN say so. A
   switch with 16 apartments takes at most 32 frames before `Connect`; E6 tests it.
4. **"No route off the user's machine" was false.** Fixed by four rules and an exact
   statement (§5.2). (a) `Store` lineage: a store opened under a local-only capability is
   marked and is wiped before a `session.server` holder opens it, with no keep option.
   (b) A hot reload that changes the capability list (or `memory_mib`, actions or
   contexts) is a full unload and load with no hand-off table (§10). (c) `bridge.plugins`
   joins `data.read` as **local-only**, exclusive with `session.server`, so plugin data
   can no longer launder decoded data to a server script. (d) `data.read` is offline
   only, so no live session holds it beside a server channel. The assessment
   (§12, §5.6, §3 `Data`) now says what is true: no module and no single script can send
   local-only output off the machine; two cooperating scripts the user installs could
   still use shared engine state as a covert channel, which Tessera neither offers nor
   documents. The default server policy no longer lists
   `data.read`, and its default `forbid` is empty (§6.5).
5. **Random numbers were shared.** Fixed (§1.2): each ext apartment has its own generator
   (host natives, seeded from the OS at creation, reseeded only by its own
   `math.randomseed`); `scripttool test` seeds it (and the hash secret) from the test's
   seed. §1.1 says that PR §4.5's shared generator belongs to the plugin profile; §7 lists
   random numbers as ruled out; an escape test checks both directions.
6. **The deadline mechanism contradicted PR §10.3 and could tear hook state.** Fixed by
   choosing a main-thread-only, synchronous mechanism on every platform (§8): the patched
   build subtracts the work of every costly VM operation from the running thread's
   count-hook counter, so the ordinary count hook fires at the next instruction after it.
   The ext hook is installed once at apartment creation and never changed (the deadline
   is data the hook reads), so every coroutine inherits it. There is no watchdog thread,
   no signal, no `SuspendThread`, no asynchronous arm and no recorded running thread, so
   no torn hook fields and no stale pointer. PR §10.3 is unchanged (a Consequences row
   says so). The ext replacement of `coroutine.wrap` is no longer needed. The OPEN is now
   a verification item.
7. **Unlisted plugin-observable channels.** Fixed (§3, §4, §5.3, §7). (a) The lease
   indicator and every notice a script can cause are hit- and hover-transparent; no script
   can raise a prompt or open the script manager; Restart and Disable live in the
   script's windows and the manager. (b) `UI.Native` is listed in §7 (a show never
   activates; moves are not saved). (c) `Input.Drive` is listed in §7, and its presses are
   defined as controller-level action state: no command names, never offered to the UI,
   never in `KeyDown`. (d) `Entity.Local` entities are ruled out: never targetable or
   pickable, never in plugin-visible views or other scripts' `Entity` results. §4's claim
   is narrowed to the `UI` module, and §7 has a "ruled out" list.
8. **Ext context toggling cancelled user sequences.** Fixed both ways (§2, §5.5, §8):
   the host registers a script's whole declared set at load and unregisters it at
   unload, so scripts never register at run time; ext registrations and ext context
   (de)activations drop a pending sequence only when it belongs to a context of that
   same script; and ext context changes are rate-limited (4 per script per second,
   coalesced per frame). Consequences lists KM §2, KM §3 and IB D5 as superseded in part.
9. **Disk DoS without a grant.** Fixed (§2, §8): `Store` has 1,024 keys per script and
   16,384 in all, each key counted as its value rounded up to 4 KiB plus 4 KiB of file
   overhead, write-behind with one flush per second per script, and a write-rate token
   bucket (64 keys and 1 MiB per second per script, 4 MiB per second in all). The script
   log is one file of at most 1 MiB per script, rotated once, rate-limited to 64 KiB per
   second with dropped lines counted. Exit criterion 4 tests both.

### Other items

- **Activation case 1** now lapses as soon as a key press of the user resolves outside the
  script's windows (§4), so a timer cannot focus a text box after the user walked off.
- **Activation case 2** accepts the chat window as the active window when the gesture is
  the `/ext:` line just submitted from it; owner question 4 names the `/ext:` line.
- **D5 "Who pushes".** A Consequences row adds the ext text box as a `text_entry` pusher
  (priority 50), and follow-up 2 names it.
- **Hover-refresh notifications** from D8 step 7 item 1 are delivered in the same frame's
  6b, like step 1's; everything from 6b on goes to the next turn (§4).
- **String metatable.** The ext `string` table is built from the wrappers and the string
  metatable's `__index` points to that same table (§8); an escape test checks
  `(""):rep(n)`.
- **Id reuse and orphans.** The `Store` belongs to the grant record: a changed `author`
  starts an empty store; the content-change notice names kept values and offers Reset;
  orphaned stores are listed with a Remove button and named when they block a write; the
  pre-signing limit is stated (§5.2).
- **§12 file output** now names the script log.
- **Global allow-list.** The ext escape test checks the exact global allow-list, which
  names no interop name; the "no `Ext` in a plugin apartment" test lives in
  `tessera-plugins` (§12, exit criterion 2).
- **Lease margin.** 350 ms (100 ms over a half-rate tick at 8 fps); owner question 4,
  §5.4 and slice 1b's exit check use it.
- **Hot reload and manifest changes.** Defined (§10): only a reload with the same
  capabilities, `memory_mib`, actions and contexts is hot; any other is unload, re-ask,
  load, without a hand-off table, so no prompt shows over a running old apartment and the
  registry holds one set. On a hot reload the old apartment keeps its contexts until its
  `Unload`, then they are deactivated and the new one starts with none active.
- **U1 and ext windows.** Layout and paint of ext windows in step 8 are ext UI time, apart
  from U1, inside the frame rule, with a new budget E8 (§4, §9, Effects).
- **Native panels over ext windows.** Hit testing is defined (§4): a point reaches ext
  windows only when no plugin or native element catches it (UB §4), as for the world.
  "HUD docking slots" were undefined and are removed from `UI`; docking can come later
  as an addition within the major.
- **Bridge read ordering.** A bridge read starts after step 7 item 5 and is serialised
  after any write of the same file, so it reads the last completed write (§7). The bridge
  never runs a built-in or an `/ext:` line.
- **Backend class enforcement.** Structural instead of a test only: `Offline` and
  `TesseraProtocol` carry tokens that only `tessera-net` can construct (§6.1), checked by
  a slice-1a compile-fail test (§6.6).
- **Host shims** capture every global and library function they use at apartment
  creation and never call methods on strings (§1.2).

## Revision 3: answers to the third review (2026-10-06)

### Must-fix items

1. **`opaque_keys` let ext bindings fire while the user types.** Fixed (§5.5): a key or
   pad press that ends at the band end because of an `opaque_keys` entry (IB D6), in
   either pass, never reaches the ext passes; D6 swallows it as before. The wording that
   called `text_entry` opaque is gone: no built-in context is `opaque`, and `text_entry`
   and `ui.eat_keyboard` are `opaque_keys`. Ext contexts can be neither. Listed under
   Consequences (0014 D6, KM §7.2) and in exit criterion 2 (an ext binding on K or F5
   with the chat input or a plugin `TextBox` focused).
2. **Release-class resolution was left out.** Fixed (§5.5): `release` and `tap` bindings
   follow the same order. D4's two passes over every non-ext context run first, the ext
   passes only if no non-ext release-class binding ended the resolution and no `opaque`
   or `opaque_keys` stop applied, and a release after a UI-owned press never reaches ext
   contexts (they are game-band and never pass-through). The `Shift+MouseLeft` `tap`
   example is worked through, and exit criterion 2 tests it. Suggested defaults may carry
   `on`, and the collision rule ignores `on` on both sides. Consequences adds 0014 D4 as
   "supersedes in part".
3. **A group-O script could take the keyboard from the idle state.** Fixed (§4, "Ext UI,
   z-order, activation and focus"): a script's `Activate`, `Focus` or show-with-activation
   is honoured only (1) while one of its own windows is already the active window, or
   (2) in the 6b turn that delivers the user's press of one of its own actions or an
   `/ext:` line for it, with no window active. The "no active window" state alone no
   longer suffices. So a script cannot focus its own text box by itself and push
   `text_entry`. The group-O definition (§3), owner question 4, §5.5's guarantee and
   §7's list now agree, and §7 lists the remaining, gesture-rooted effect (key actions
   offered to an active ext window or a focused ext text box, as for a native panel).
4. **Wallhack mitigation.** Fixed (§5.2, §6.5): the script policy has `allow` and
   `forbid` (forbidden combinations of two to four capability names). The warnings table
   gains `entity.read` + `ui` (world labels, radar, the nearest-entity reference script).
   §6.5 says plainly that no client-side rule can stop a script that reads positions and
   draws anything, that a per-module restriction would not help, and that the real
   defence is the server's interest management, which `tessera-protocol.md` must state as
   a server requirement (follow-up 7).
5. **`data.read` + `session.server` was an export.** Fixed by making them mutually
   exclusive (§2, §5.2): a manifest that requires both is refused, and a grant holds at
   most one. With no other network route, "no export" is now true and its meaning is
   stated (§3 `Data`, §5.6). §12 records the assessment. The default server policy's `forbid` lists the pair too.
6. **Third-party servers conflicted with 0004.** Fixed by the second option:
   ext support on `TesseraProtocol` targets the project-owned test server only (§6.1,
   §6.5, §12), inside 0004's scope. Third-party servers need a
   separate decision that amends 0004 first.
   The 0016 "supersedes" row is removed; Consequences says
   0004 and 0016 are unchanged; owner question 3 asks the owner to confirm.
7. **The deadline missed coroutines and raced across threads.** Fixed (§8, "Deadline at
   the next instruction"): the host tracks the running Lua thread through every ext entry
   point and the guarded `coroutine.resume` (the ext profile also replaces
   `coroutine.wrap` with one built on it), installs the current hook on a coroutine
   before each resume, and arms the hook of the recorded running thread. No `lua_State`
   is touched from another OS thread (PR §10.3): the OPEN names the candidates (a
   thread-directed signal whose handler only calls `lua_sethook`, as `lua.c` does; on
   Windows, suspend, arm, resume) and forbids a plain cross-thread write. E6 gains the
   coroutine cases.
8. **Finalizers were outside the time model.** Fixed by removing `newproxy` from the ext
   profile (§1.2). In Lua 5.1 only userdata run `__gc`, so ext Lua code never runs in a
   collector step or in `lua_close`; host userdata finalizers are native and constant.
   The native time of `lua_close` enters §8 as a fourth residual, C, measured in slice 1a
   with a fallback.
9. **Intents could outlive the gate.** Fixed (§4): an apartment's pending intents are
   discarded when it unloads or is suspended, and each intent is checked again at apply
   time against its apartment, the session generation, the backend class and the grant.
   A queued backend switch stops 6b at once. §6.6 has the test, which slice 1a runs.
10. **"No route to post a chat line" was contradicted.** Fixed (§7, "Chat"; §8; §3
    `Commands`): ext reports and management output never go to the chat window, only to
    the script log, the console, the script manager and reserved-layer notices; a typed
    `/ext:` line is not echoed and an unknown id posts no chat line. The bridge's
    plugin-command run with script-chosen arguments is listed as a channel. LPA §6.6 and
    PR §11.6 get notes.
11. **The panic chord changed plugin-visible input.** Fixed (§5.5, §7): the tier exists
    only while the ext host has a loaded apartment or a live lease; it is checked against
    every built-in, imported and user binding (subset matches included) when it is set
    and when the keymap changes; and its remaining effect while ext scripts are loaded is
    listed in §7. With no ext script loaded nothing changes (Effects).

### Other items

- **Memory limits.** The sum of caps is now 512 MiB for loaded scripts plus a separate
  64 MiB reload allowance (§8, §10), so 16 scripts at the default fit and a reload never
  falls back. Owner question 8 says so.
- **Slice 1a gate.** `Offline` only until slice 3 ships the policy handshake (§3, §6.2,
  deliverables); slice 3 admits `TesseraProtocol`.
- **Exit criterion 3.** Names the slice-1a tests of §6.6, which are now marked by slice.
- **Wrapped functions.** `select`, `coroutine.resume`, `coroutine.wrap` functions,
  `pcall` and `xpcall` are charged per value copied (bounded by `LUAI_MAXCSTACK`); `next`
  is bounded explicitly by the allocation cap and counted in R (§8, E6).
- **Composition entries.** Checked at creation, at every show and at every visibility
  change; ext yields (its windows are hidden, newest first) when plugins grow (§8).
- **`Files.Pick`.** Asynchronous dialog, one open for all scripts, only on a user gesture,
  one per script per 5 s (§3, §8).
- **`Server` limits.** Size, rate and queue caps (§8), before slice 3.
- **Consent prompts.** "Allow" needs a pointer click, buttons ignore input for 500 ms,
  Escape only refuses; the prompt pushes `ui.dialog` and `ui.eat_keyboard`, which
  releases key holds as IB D6 says (§5.3).
- **Prompt timing.** Shown at the start of the next redraw, immediately before
  `offer_pending`; raw events that arrived earlier were resolved without it (§4).
- **Rights intersection.** A required capability removed by any layer refuses the script
  as a whole, naming the layer; optional ones only make `Ext.Has` false (§2).
- **Folder snapshot.** Directory-relative, no-follow opens with type checks on the handle
  and caps counted on bytes read (§2).
- **Ext windows and z-order.** Ext windows sit in their own z-range below every plugin
  window and native panel, so they cannot cover or clickjack them; the hover channel of
  revision 2 is gone, and §7 lists that an ext window takes world clicks in its own area.
- **`links` fixture.** Also proves that an excluded compatibility binary depending on the
  client with `default-features = false` still resolves, with a recorded fallback (§6.3,
  Open items).
- **`/ext:` case and unknown ids.** Case-insensitive matching; no chat line for an
  unknown id (§3).

## Revision 2: answers to the second review (2026-10-06)

### Must-fix items

1. **Ext UI could run plugin Lua inside ext time.** Decided: ext windows form their own
   activation and focus domain for programmatic requests (§4, "Ext UI, activation and
   focus"). The active window and the focus stay shared (LPA §13.13–§13.14), but a
   script's `Activate`, `Focus` or show-with-activation is honoured only while no window
   is active or the active window is the same script's; otherwise it only remembers the
   focus control and returns `false`. Hiding a script's own active window activates
   nothing else. Consent prompts raised outside step 1 are shown at the start of the next
   frame's step 1, so a plugin text box's `FocusLost` is delivered there as plugin time
   (§5.3). Ext geometry changes reach plugin hover only at the next hit test or hover
   refresh (IB D8). So no ext call or item runs plugin code, ext time never contains
   plugin handler time, and the 5 ms deadline cannot be spent by a plugin (§4, "Ext
   time"). The resulting channels (the user's clicks on ext windows, prompts, covered
   windows) are in §7's list, and an escape test covers them (criterion 2).
2. **Ext bindings could beat built-in subset bindings.** Fixed by an amendment to KM §7.2,
   listed under Consequences as "supersedes in part": ext contexts resolve only after both
   passes over every non-ext context have failed to consume the press (§5.5). Suggested
   defaults collide when any built-in or plugin binding, in an active context or not,
   would match the stroke in either pass, including subset matches with extra modifiers
   (`Shift+W` against `W`); colliding defaults are withheld, and suggested defaults are
   single strokes. The guarantee is restated exactly: an ext binding fires only for a
   press that no active non-ext context consumes, and a hand-made binding on an input of
   an inactive built-in context fires as the user chose. Axes and modifier holds are
   covered too.
3. **`camera` in group O.** Moved to group A (§3), with the definition of group A saying
   "take over any of the user's controls" and a sentence on why `input` stays group O. A
   content change now re-asks for `camera`. The warnings table gains `camera` on a server
   and `camera` + `entity.read` (§5.2); owner question 4 lists the group-A capabilities;
   a server's default policy leaves `camera` off (§6.5).
4. **E6 did not follow from §8.** Fixed in three parts (§8). (a) The rule is now "every
   standard-library function whose work grows with its input or result is wrapped and
   charged", with a table that includes `table.insert` and `table.remove` with a position,
   `table.maxn`, the size-proportional string functions, `string.byte` and `string.char`
   with many values, plain searches and `tonumber` on long strings; E7 checks all of them.
   (b) 6c has its own 1 ms slice, round-robin and at least one apartment per frame, and
   one collector step (S) enters the residual arithmetic, both in 6c and inside an
   allocating instruction. (c) Two holes the review did not name are closed: a new 8 MiB
   single-allocation cap bounds every instruction that grows with a string or a table
   (the worst concatenation drops from 16 ms to about 6 ms), and the deadline now takes
   effect at the next instruction through a watchdog that arms the count hook, because
   1,000 expensive instructions between two hook checks could otherwise run for seconds.
   The sum is 12 ms + R + 2·S = 18 ms + 2·S with today's measurement; S and the two
   unmeasured instructions are OPEN for slice 1a, with a recorded fallback (lower caps)
   if the sum exceeds 30 ms. E6 lists the new hostile cases.

### Other items

- **Build-level exclusion (§6.3).** A compatibility binary is never a feature of a
  workspace package and never a workspace member: it and the compatibility backend are
  packages in `[workspace] exclude`, the binary depends on the client with
  `default-features = false`. `tessera-ext` and every `gated-compat` crate share a `links`
  value, so Cargo refuses any graph with both. The check also resolves the unified graph
  of the whole workspace and rejects features that enable a `gated-compat` dependency.
- **Independence.** `tessera-ext-host`'s dependency on `tessera-plugins` is optional behind
  its `bridge` feature (§1.1, §7).
- **"Never a superseded line".** Reworded (§7): ext refuses while any teleport is pending,
  the user's included; a user teleport issued after an ext one still cancels it with the
  ordinary "Teleport cancelled" line.
- **§7's list.** Completed with the world-state effects of `Sandbox` (gameplay views,
  LPA §15), ext windows covering plugin windows (hover and hit tests), the user's clicks
  on ext windows and consent prompts, and a note that ext bindings and ext time are not
  channels.
- **KM registry "last part".** A reserve rule is added (§8): ext never leaves fewer than
  256 actions or 16 contexts free. Effects now lists every reserve.
- **Leases and the cap.** Lease holders are ticked first in 6b, and expiry under
  starvation is stated as the accepted fail-safe (§5.4).
- **Default server policy.** Written as capability names; `Commands` and `Store` need no
  grant; `ui.native` is no longer in the default; `session.server` stays, and owner
  question 3 asks the owner to confirm (§6.5).
- **0016 and third-party servers.** Owner question 3 now asks explicitly; Consequences
  lists 0016 item 2 as superseded in part if the owner accepts.
- **Developer REPL.** Bound only to `data/scripts-dev/` apartments, so the "hashed
  snapshot" statement holds without exception for installed scripts (§5.2, §10).
- **Actions and contexts.** The manifest gains `[[contexts]]`; each action names its
  context; run-time `Input` registers only what the manifest declares (§2, §3).
- **Hot reload and limits.** Counts once against the 16-script limit, at most one reload
  at a time, both caps against the 256 MiB sum, with an unload-then-load fallback (§8,
  §10).
- **Self-caused notifications.** Delivered in the script's next 6b turn, never
  re-entrantly; only step 1's native input produces notifications delivered in the same
  frame (§4).
- **Follow-ups.** Edits to other files are now collected under "Required follow-ups"
  (Consequences), and the status line points there.

## Revision 1: answers to the review (2026-10-06)

### Must-fix items

1. **Text `load`/`loadstring` in ext apartments.** Fixed by removal. Ext apartments have no
   `load`, `loadstring`, `setfenv`, `getfenv` or `string.dump` (§1.2). Code comes only from
   the folder snapshot, which is read once, hashed, and compiled one file per ext item with
   a 128 KiB file cap (§2, "Loading"); `Ext.Load` runs precompiled chunks only. So (a)
   pinning covers the code that runs, and §5.2 now says plainly what it does not cover
   (behaviour a script derives from data); (b) "no code pushed from a server" is
   enforceable and stated as such (§5.6); (c) no compilation happens inside an ext call,
   and one compilation item costs about 6.5 ms at most (45–52 ms per MiB, measured). The
   developer console's REPL is compiled by the host, in developer mode only, never through
   a function a script can reach (§10).
2. **Exit criterion 4 / E6 did not follow from the limits.** Fixed. The record now cites the
   measured rates (Context; owner question 8). Limits are wall-clock first: a 5 ms deadline
   per call checked in the instruction hook and after every native (§8); natives charge
   calibrated work units before they run, including wrapped `table.sort`, `table.concat`,
   `string.rep`, `string.format` and `unpack`, and `collectgarbage` is reduced to
   `"count"` (§1.2, §8); per-frame caps for rays, point queries, entity queries and
   subscription events (§8); a 6 ms cap on ext time started per frame (§4); the throttle
   uses a rolling 1 s mean, and suspension a total over 10 s, so alternating heavy and
   light frames does not escape it (§8). The one uninterruptible residual (a single
   concatenation) is measured and bounded by lowering the memory maximum to 64 MiB (§8,
   "Residual"). E6 now names the hostile cases and an ext ceiling of 30 ms per frame, and
   E7 checks the cost calibration.
3. **The shared deferred-work queue.** Fixed. Ext has its own queue (6a) with its own 1 ms
   slice, run after D8 item 6 and measured as ext time (§4); D8 items 1–6, the plugin queue
   and the plugin slice are unchanged. Bridge actions are replayed as the user's own
   actions: loads as the manager's load item (at most two outstanding for all scripts), and
   unloads and commands in the next frame's step 1 (§7), so ext cannot starve `LoadPlugin`,
   `PluginData` callbacks, game-raised events or the logout unload. `Sandbox` commands are
   rate-limited, a teleport while one is pending is refused before submission (so no
   "superseded" line; narrowed in revision 2), and §7 now admits the system lines that
   sandbox commands cause.
4. **Ext commands visible to plugins.** Decided, OPEN removed. Ext commands are
   `/ext:<id> <command> …`. `AddCommand` splits names at `:`, so no plugin or built-in name
   can collide; ext commands never appear in `IsCommand`/`GetCommands`; plugin names always
   win (§3, `Commands`). I chose the `ext:` word over `/ext <id>` because a plugin could
   register `ext` itself, which would either break the plugin-wins rule or make ext
   commands unreachable.
5. **Amended accepted decisions not named.** Fixed. Consequences now lists every amended
   record and section with "supersedes in part" or "extends", and says how the accepting
   session marks 0014 and 0015 without rewriting them. Two of the listed amendments were
   removed instead of recorded: D8's shared queue and plugin-time rules are no longer
   touched (item 3), and the plugin manager panel no longer moves into the reserved layer;
   a separate script manager lives there, so M5's panel, UB's activation rules and UL are
   unchanged (§5.3). PR §8.4/§10.1 and LPA §3.3/§11.1 get notes only, because no plugin rule
   changes.
6. **Shared global caps not partitioned.** Fixed (§8): ext pools of 512 surfaces and 1,024
   composition entries, a native reserve (ext never takes the last 512 surfaces, the last
   1,024 entries, or the last 16/32 MiB of U5), ext memory shares inside U5, refusal at
   creation instead of UR's draw-nothing fallback, and reserved-layer surfaces exempt from
   every cap. Actions: 64 per script and 256 for all ext; contexts: 2 per script and 16 for
   all ext. The `ext.` prefix is reserved to the ext host, and the name budget is stated (27
   bytes after a 32-byte id).
7. **Contradictory lease rules.** Fixed (§5.4): a 250 ms lifetime set by take, renew or
   intent; expiry is checked where the intents apply; 250 ms covers a half-rate tick down
   to 8 fps; a watchdog stop is defined as suspension under §8 and ends leases in the same
   frame. The "30 frames after a watchdog stop" rule is gone.
8. **Build-level exclusion against the code and 0004.** Fixed (§6.3): the compatibility
   backend must be a separate crate tagged `[package.metadata.tessera] tier =
   "gated-compat"`; the check matches the tag across every manifest in the repository;
   `tessera-ext` is a default-on client feature, so a compatibility build drops it without
   restructuring the client. `compat.rs` and ARCH §19.3 are listed for rewording
   (Consequences); this proposal edits neither.
9. **"Never against an official server" only as strong as the class.** Fixed (§6.5, §12):
   `input.drive` and every later game-command or movement module are Offline only until a
   separate decision that must answer the proxy case; the class is renamed `TesseraProtocol` because it covers any server speaking our
   protocol. Owner question 6 changed accordingly.
10. **§5.6 contradicted the bridge.** Fixed: §5.6 states the bridge as the one group-A
    exception (list and read-only plugin data).
11. **BIN facts in the ext contract.** Fixed by the first option: `ext-api.md` defines ext UI
    independently and cites no BIN fact (LPA §13, UB, UL BIN rows) as contract; shared
    `tessera-ui` code is allowed, but ext may rely only on what `ext-api.md` states (§12).
    The "no provenance problem" claim in the header is qualified by that rule.
12. **Grant-pinning defects.** Fixed (§2, §5.2): a canonical, length-prefixed, tagged
    encoding; grants keyed by (folder kind, id); manifest `id` must equal the folder name;
    folder caps of 256 files, 8 MiB, depth 4 and 128 KiB per `.lua` file; symbolic links
    and special files refused. A changed `author` also re-asks every grant, and kept group-O
    grants are announced with a Review button, which narrows the "unrelated script reuses an
    id" case further.

### Other items

- **Slice 1 not small.** Adopted the suggested split with two changes: `Sandbox` moves to 1b
  with `Camera` and `Overlay`, and 1a keeps three reference scripts that need only 1a
  modules. M5.5 is slice 1a; 1b follows at once with its own exit check.
- **Owner question 2 and the split.** Reworded: ROADMAP M5 and ARCH §3.1 already place
  `tessera-plugins` at M5. §1.1 sizes the split from the code (about 3,400 of 5,400 lines
  move; `Apartment::new`, `HostApi` and `NativeApi` are the entangled parts). The OPEN now
  asks only whether M5's split reaches that shape.
- **Clock port.** Named (§3, `Sandbox`): the clock port over `tessera-world`'s `GameClock`;
  `Sandbox` is Offline only and answers "unavailable on this backend" elsewhere.
- **`scripttool test` needs the ports.** The ports and the harness live in the new L5 library
  `tessera-ext-host`, used by both the client and the tool (§1.1, §10). I preferred a crate
  to documenting the client's lib target as a library, because ARCH §3 lets tools use
  library crates, and an app's internals should stay free to change.
- **OS identity.** Fixed (§1.2, §3, §5.6): relative chunk names and messages; `Files.Pick`
  gives a basename only.
- **Aggregate caps.** Added (§8): 256 MiB for all ext apartments as the sum of caps, 16
  loaded scripts, 256 MiB of `Store` on disk, 1 MiB per value.
- **Lifecycle points.** Added (§4, "Lifecycle"): load after start-up steps 8–9 at world
  entry; backend-switch and session-end unloads as queued head items with `Connect`
  waiting, recorded as exceptions; the client-exit unload as in D8; hot reload spans frames
  (§10).
- **Ext contexts shadowing built-ins.** Fixed both ways (§5.5): ext contexts have a priority
  range below every built-in and are never opaque or text, and colliding suggested defaults
  are not applied.
- **Dangerous combinations.** Added to the prompt (§5.2).
- **Alias quickslots.** Added to §7's list of what a plugin can observe or cause.
- **`Data` "never bulk".** Reworded as local use, with enumeration by DID
  range kept off; the 16-per-frame cap stays as a load limit (§3,
  §12).
- **`Store` and `.plugindata`.** Fixed (§1.1, §2, §7): the core holds a neutral Lua-table
  data reader; `Store` files are `<key>.store` under `script-store/<kind>/<id>/`; the
  `.plugindata` naming, key grammar and comma rule stay in `tessera-plugins`.
- **§5.7 citation and the content guard.** Fixed: the content guard is defined (§10,
  `pack`).
- **Step-1 intents and reads.** Answered by design: no ext Lua runs in step 1 any more; the
  script hears about its step-1 input in 6b of the same frame (§4).
- **`[[settings]]` key-binding type.** Removed; key bindings are `[[actions]]` only (§2).
- **Measured instruction rate.** Cited in Context, owner question 8 and §8, with the caveat
  that the vendored build is re-measured in slice 1a.
