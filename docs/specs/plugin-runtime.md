<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Plugin runtime: the Lua interpreter, apartments, limits and errors

| Field | Value |
|---|---|
| Status | **Accepted** (revision 3, 2026-10-06: applies the Accepted decisions 0014 and 0015, see the change log; repaired twice the same day after its reviews; accepted after revision for decisions 0014/0015, review record below). **Amended 2026-10-06** for decision 0014's Amendment 1 (§8.4, §10.3, §16 Q11–Q13; last change-log row): Tessera rules only, awaiting review of this change |
| Last updated | 2026-10-06 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | Claude review session (spec-writer agent) |
| Review record | 2026-10-06, Claude review session (spec-writer agent), a different session from the author: **passed** `docs/CLEAN_ROOM.md` §5.3, every item. Spot checks against the install (client 4906.0071.1794.4035 and its `lua51.dll`, read-only) and the C1 snapshot all matched. **R1:** 123 exports with the io/os/package openers, 64-bit, a 5.1.4 identification string, side-by-side assembly 8.0.50727.762. **R3:** 43 imports, and none of the listed functions. **R4:** only the interpreter and the XML library import `MSVCR80`, and only the interpreter imports its locale setter. **R6:** the formatter has two 32-bit conversions and one 64-bit conversion. **R7:** remainder 32767, divisor 32767.0, and the LCG gives 41, 18467, 6334. **R12:** one `lua_gc` call site with option 5, called from a loop over all apartments, and the step field is set to 20 in the manager constructor. **R25:** 155 / 7,722 / 218, 5,319 CRLF files, 168 descriptors with an apartment, `getfenv` 66, `setfenv` 12, `table.getn` 68, `newproxy` 2, 7 hexadecimal numerals above 32 bits. Non-blocking notes for the next revision: (1) the §4.4 hexadecimal row's "wrap a leading minus" applies only to text conversion (`tonumber`, coercion); a source `-0x10` is unary minus applied to 16, so it gives −16, and saturation is the only rule that applies to source numerals; (2) "Derived" is a third mark that §5.1 does not define; it is defined in §2 and used consistently, but each Derived row could also carry Confirmed or Reported; (3) the §9.2 rule to step the collector "after the tick's event dispatch" is Tessera's own choice and has no provenance row for the original's order; (4) invariant 2's `tostring(-0) == "-0"` depends on the chunk having no other zero constant, because 5.1 shares numerically equal constants. **Revision 2 (2026-10-06, spec writer):** applies notes (1)–(4), corrects the per-tick collector arithmetic (§9.2), sets Tessera's default memory cap from a measurement of the M5 plugin set (§9.1, §9.4) and updates the naming note; awaiting re-review. **Re-review of revision 2: 2026-10-06, Claude review session (spec-writer agent), a different session from the author, against `CLEAN_ROOM.md` §5.3. Result: passed; status Accepted.** All four earlier notes are applied. The collector arithmetic of §9.2 agrees with the stock 5.1 `lua_gc`/`luaC_step` rules: 20 KiB of debt paid in 1 KiB per basic step of 10 × 200 work units, about 20 steps, stopping at the end of a cycle. Install spot checks (read-only): R1 (`lua51.dll`: 123 exports including the io/os/package openers, AMD64, `Lua 5.1.4`, 8.0.50727.762, imports only `MSVCR80` and `KERNEL32`); R3 (the client imports 43 interpreter functions, none of the listed ones and no raw set); R4 (only `lua51.dll` and `xerces-c_2_6.dll` import `MSVCR80`; only the interpreter imports `setlocale`); R7 (`rand` result reduced modulo 0x7FFF and divided by 32767.0; the generator gives 41, 18467, 6334); R12 (one reference to the `lua_gc` import, through one thunk with one caller that passes option 5). Measurements re-run in the dirty scratch area: R26 (37, 572 and 4,571 ticks for 1.6, 25.1 and 201.6 MiB; heap at most 1.59 times live with 64 KiB of garbage per tick); R27 / §9.4 (#2 peaks at 167,942,126 bytes, live 131,198,857, refused during load at 159 MiB; #14 peaks at 78,862,389 bytes and is refused at 75 MiB; the synthetic 1,000,000-table file peaks at 170.8 MiB with 154.2 MiB live and 9.0 M instructions, and is refused at 128 MiB). Non-blocking note for the next revision: the §9.4 rows "smallest cap under which #2 / #14 loads" are about the load call only. At caps of 160 MiB and 75.2 MiB the load succeeds, but a later allocation in the same run is refused, because the run's peaks are 160.16 and 75.21 MiB. Say so, so that nobody reads those rows as caps that pass invariant 9 of `plugin-api-usage.md`. **Review of revision 3 (after its second repair): 2026-10-06, Claude review session (spec-writer agent), a different session from the author, against `CLEAN_ROOM.md` §5.3 (every item) and the Tessera rules of the Accepted decisions 0014 (D8, open review items 1 and 2, the ARCHITECTURE §21 item) and 0015 (§4.3–§4.4, §6 U1 and U4). Result: passed; accepted after revision for decisions 0014/0015; status Accepted.** §8.4, §9.2, §10.1 and §10.3 agree with D8 (step-1 handler list, step-7 items 1–7, slicing by whole items at 2 ms, the collector step as item 6 before the outbox drain, the unload timing, the plugin-time accounting outside U1 and the U4 paint slice); the two stated exceptions (the first-index wait from 0015 §4.4, the `Received` skip set from 0014 open review item 1) are explicit and labelled; invariants 19–22 were worked through and are consistent with the rules. No fact about the original changed. Install spot checks (read-only, client 4906.0071.1794.4035): R1 (`lua51.dll` AMD64, 123 exports with the io/os/package openers, `Lua 5.1.4`, VC80 CRT 8.0.50727.762, imports only `MSVCR80` and `KERNEL32`); R3 (43 interpreter imports, none of the listed ones, no raw set); R4 (only `lua51.dll` and `xerces-c_2_6.dll` import `MSVCR80`; the XML library imports no `setlocale`); R7 (`math.random` reduces the `rand` result modulo 0x7FFF and divides by 32767.0); R12 (one reference to the `lua_gc` import, one thunk, one caller passing option 5 inside a loop over all apartments, its argument read from a manager field; one immediate store of 20 at the matching field offset). Non-blocking notes for the next revision: (1) R4 and §4.3 say "two C runtimes are in the process"; `OpenAL32.dll` and `soft_oal.dll` import a third one, the system `msvcrt.dll` (and `soft_oal.dll` its `setlocale`); this does not touch `MSVCR80`'s state and changes no plugin-visible behaviour, but the wording should say "the interpreter's runtime is shared only with the XML library"; (2) the §12 image row applies 0015 §4.4 to `SetBackground(path)` only; `Graphic(path)` and the descriptor `Image` are left unstated, as 0015's own open review item notes; add an OPEN or a cross-reference once `lua-plugin-api.md` §13.8 settles it; (3) Q13 raises ARCHITECTURE §21 only for synchronous saves at unload; the synchronous reads of Lua source by `import` and by load items in step 7 are the same kind of main-thread I/O (0015 open review item on its `import` precedent) and could be named in the same OPEN; (4) the last change-log row says the §17 load-timing row has one remaining request, while the row itself (correctly, `lua-plugin-api.md` §3.3 now states the head-of-queue wait) says all its requests are applied; align the change-log wording; (5) because queued game-raised events go to the handlers set at delivery, a plugin loaded by an earlier queue item receives `Received` lines raised before its load ran (including lines raised during the first-index wait), which the original's synchronous delivery would not give; inherited from 0014 D8, worth a sentence in §8.4 or a known-difference note. |
| Implements into | `crates/script` (Lua host: interpreter build, number and string conversions, allocator, collector stepping, protected calls, error routing), `crates/plugins` (apartment lifecycle, autoload, file access) |
| Related specs | [`lua-plugin-api.md`](lua-plugin-api.md) (API surface; its §3 sandbox, §6 lifecycle, §7 `import`, §9 dispatch, §10 plugin data and §12 engine functions are the detailed sources this spec builds on), [`plugin-descriptor.md`](plugin-descriptor.md) (discovery, descriptor, package grammar, `PluginOptions.xml`); decision records [`0014-ui-input-bridge.md`](../decisions/0014-ui-input-bridge.md) (D8: where plugin code runs in the frame loop, the deferred-work slice, game-raised events, unload timing) and [`0015-ui-rendering.md`](../decisions/0015-ui-rendering.md) (§6: the UI budget that plugin time is kept out of; §4.3–§4.4: plugin image formats, the whole-`Plugins/` file index and the call-time header read used in §12, and the rule that no plugin loads before the first index is ready, §8.2, §8.4) |

> **Naming note (interoperability).** The global, namespace, function, event and file names in
> this document (`import`, `Plugins`, `plugin`, `Turbine`, `Turbine.Engine`, `Turbine.PluginData`,
> `Turbine.PluginManager`, `Load`, `Unload`, `GetOptionsPanel`, `Default`, `Plugins/`,
> `PluginData/`, `.plugindata`, `__init__.lua`, `Script.log`, `PluginOptions.xml`) are the
> identifiers that existing third-party plugins and their files use. They are reproduced only
> because a plugin cannot run without them. The project's interoperability record for these
> names is `docs/LEGAL.md` §17 (it answers the "OPEN (legal)" item of `docs/ARCHITECTURE.md`
> §18.2); its §17.2 rules bind every use: the names appear in Tessera only inside the plugin
> subsystem, as string data, never as product, crate, module or UI names. Lua's own function
> names, C API names and build options (`lua_gc`, `LUAI_MAXCCALLS`, …) are general knowledge of
> the MIT-licensed reference interpreter. No client message text, documentation prose or plugin
> code is reproduced: messages are described, never quoted.

---

<a id="1-purpose"></a>
## 1. Purpose

This spec lets an implementer build the **runtime** underneath the plugin API so that existing
plugins see the same Lua they see in the original client. It covers:

- which interpreter the original embeds, its build options, and the C library behaviour that
  plugin code can observe through it (§4);
- how an apartment's Lua state is set up and what it contains, how apartments are isolated
  from each other, and which environments code runs in (§5, §6);
- package naming and `import` resolution, in summary (§7);
- load, autoload, unload and the callbacks a plugin receives, with their order (§8);
- memory accounting, garbage collection, finalizers (§9) and execution limits, threads and
  coroutines (§10);
- protected calls, error reporting, unprotected paths and what happens to a failing plugin
  (§11);
- every way a plugin can reach the file system (§12).

Where `lua-plugin-api.md` already states a runtime fact in detail, this spec summarises it and
points there; the two agree. New facts in this spec are marked in its provenance table.

---

<a id="2-provenance"></a>
## 2. Provenance

Source keys:

| Key | Source |
|---|---|
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, PE file version **4906.0071.1794.4035** (`lotro_live`, built 2026-09-27), and the interpreter `x64/lua51.dll` shipped with it, read in the dirty room on **2026-10-06** with a targeted capstone disassembly of individual functions (no whole-image listing kept). Code read in the client: the apartment constructor (state creation, panic-handler registration, library opening), the host allocator, the panic handler, the protected-call wrapper, the host's error-raising helper, the apartment's error and out-of-memory notifiers and the apartment registry's fan-out to listeners, the per-tick collector step and the constructor that sets its step size, the apartment destructor, the generic file loader and its plugin-data caller, the event dispatcher's call site for table handlers, the `PluginOptions.xml` autoload reader and the keyed container it fills, the helper that records a control's creation location, and the error-guard list used by the panic handler. Code read in `lua51.dll`: `lua_newstate`, the C-call depth and call-info limit checks, the pattern capture limit, the upvalue limit, the chunk-name buffer argument, `math.random` and the integer conversions of `string.format`. Also the PE import tables of every module in `x64/` (static, 2026-10-06). **Questions:** interpreter build and patches, collector stepping, allocator limit, error filtering, panic handling, floating-point environment, C-runtime split, autoload container. **Why M1–M3 were not enough:** these are code paths, not data (M1 sees no plugin code; the owner's settings folder holds no plugins on 2026-10-06); published references (S2, S3 of `lua-plugin-api.md`) list API names but say nothing about the interpreter build, the collector or the error paths; M3 (running small plugins on the official client) would show results but not the internal decisions (step size, which error kinds are reported, panic handling) and was not available to this session. Nothing about the network protocol, login, keys, the launcher or anti-tamper was read. Numeric constants are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). |
| **LPA** | [`lua-plugin-api.md`](lua-plugin-api.md) (Accepted), whose own BIN reading of the same client build is dated 2026-10-05; cited by section and provenance row (P*n*) |
| **PD** | [`plugin-descriptor.md`](plugin-descriptor.md) (Accepted); cited by section and row (D*n*) |
| **LUA** | General knowledge of the Lua 5.1 reference manual and the behaviour of the reference interpreter 5.1.4 (MIT licence; `docs/CLEAN_ROOM.md` §2 rule 4: not dirty) |
| **CRT** | General platform knowledge of the Microsoft C runtimes: the Visual C++ 2005 runtime (`MSVCR80`, version 8.0) and the Universal C runtime (`api-ms-win-crt-*`); documented behaviour of their number formatting and parsing, `rand`, and locale handling |
| **C1** | Census of third-party plugins (method M2, facts only, counts only): a snapshot taken 2026-10-06 of **155 plugin packages** (downloaded archives and public repositories gathered in the dirty scratch area; some are different versions of the same plugin), **7,722 Lua files**, **218 `.plugin` descriptors**. Nothing was copied; only which runtime features occur and how often |

Marks: **Confirmed** = observed in the owner's install (data, settings folder or the client
binary) with date; facts resting only on BIN say so. **Reported** = published sources or
general platform knowledge only. **Derived** = follows from other rows by the stated Lua/CRT
rule; the row names both. For the review checklist of `docs/CLEAN_ROOM.md` §5.3 a Derived row
counts as Confirmed when every row it rests on is Confirmed, and as Reported otherwise (each
Derived row below says which applies).

**Tessera rules from decision records.** Some Tessera rules in this spec are design
decisions recorded in `docs/decisions/` (key **DEC**: [0014](../decisions/0014-ui-input-bridge.md),
Accepted 2026-10-06, chiefly its D8; [0015](../decisions/0015-ui-rendering.md), Accepted
2026-10-06, its §6 and its §4.3–§4.4). They are not facts about the original, so they get no
row and no mark below; each place that uses one cites the record and its section. A decision changes what
Tessera does, never what this spec says the original does.

| # | Fact or group of facts | Sources | Mark |
|---|---|---|---|
| R1 | The interpreter is the reference Lua 5.1.4, built as a separate 64-bit DLL against the Visual C++ 2005 C runtime (`MSVCR80`, side-by-side assembly version 8.0.50727.762); it exports the stock 5.1 API (123 functions, `luaopen_io`, `luaopen_os`, `luaopen_package` included) | BIN (import, export and version resources of `lua51.dll`; its identification string names 5.1.4), LPA P1 | **Confirmed** 2026-10-06 |
| R2 | No patch visible: every text literal in `lua51.dll` is a literal of the reference 5.1 sources, and the build limits checked equal the reference defaults: nested C calls 200, call-info entries 20000, pattern captures 32, upvalues per function 60, chunk-name buffer 60 bytes, collector pause 200 and step multiplier 200 | BIN (string table diffed against the reference source literals; the limit constants read in the functions that enforce them; `lua_newstate` initial collector parameters) | **Confirmed** 2026-10-06 (BIN only) |
| R3 | The client links only 43 of the interpreter's functions; it never imports `lua_sethook`, `lua_resume`, `lua_yield`, `lua_setfenv`, `lua_load`, `luaL_loadbuffer`, `luaL_openlibs` or `luaopen_io/os/package` | BIN (import table of the client) | **Confirmed** 2026-10-06 |
| R4 | Two C runtimes are in the process: the client and its own modules use the Universal C runtime; the interpreter uses `MSVCR80`. The client's start-up `setlocale` (LPA P43) acts on the Universal runtime only. No module calls `MSVCR80`'s `setlocale` (the interpreter imports it only for `os.setlocale`, which is never opened), so the interpreter's conversions and character classes run in the **C locale** whatever the game language | BIN (import tables of all modules in `x64/`: only `lua51.dll` and the XML library import `MSVCR80`, and the XML library imports no locale setter), CRT (a runtime starts in the C locale) | **Confirmed** 2026-10-06 (imports); Derived (behaviour; rests on CRT: counts as Reported) |
| R5 | Lua-visible consequences of R4 and of `MSVCR80`: number-to-text uses `%.14g` with **three-digit exponents**; pointer text (`%p`) is 16 upper-case hexadecimal digits without prefix; text-to-number accepts no `inf`, `nan` or hexadecimal floating forms; hexadecimal integers go through a 32-bit unsigned conversion (values above 0xFFFFFFFF become 4294967295, a leading minus wraps modulo 2^32); character classes and case mapping are ASCII-only; string ordering is bytewise unsigned | R1, R4, LUA (`luaO_str2d`, `lua_number2str`, `l_strcmp`, `lstrlib` classes use the C library), CRT | **Derived** (rests on CRT: counts as Reported); exponent width, `%p` and non-finite text **Reported** (CRT) |
| R6 | `string.format` converts `%d`, `%i` and `%c` arguments to a **32-bit** signed integer (out-of-range values become −2147483648) and `%o`, `%u`, `%x`, `%X` arguments to a 64-bit integer of which the formatter uses the **low 32 bits** | BIN (the three conversion sites in the formatter: two 32-bit, one 64-bit), LUA (the formatter's `long` argument type), CRT (`long` is 32-bit on 64-bit Windows) | **Confirmed** 2026-10-06 (BIN) |
| R7 | `math.random` uses `MSVCR80`'s `rand` with range 0–32767: the result is (`rand()` mod 32767) / 32767. The generator state is per OS thread and starts at seed 1; every apartment runs on the main thread, so all apartments share one generator | BIN (`math.random` divides by 32767 after a remainder by 32767; host calls run on the main thread, R13), CRT (per-thread state, initial seed 1, linear congruential step: multiply by 214013, add 2531011, result = bits 16–30) | **Confirmed** 2026-10-06 (range, BIN); **Reported** (generator constants and per-thread state, CRT) |
| R8 | Around every protected call into Lua the host masks the floating-point divide-by-zero exception and clears pending floating-point status; after the call it unmasks it again only when a client configuration flag is set (the flag's meaning was not traced). Division by zero in plugin code therefore yields ±infinity or NaN, never a trap | BIN (protected-call wrapper and its floating-point helper) | **Confirmed** 2026-10-06 (BIN only) |
| R9 | State creation: `lua_newstate` with the host allocator and the apartment as its user value; a panic handler is installed; base (with `coroutine`), `table`, `string`, `math`, `debug` are opened by direct calls in that order; then `import` is registered and the removals of LPA §3.2 follow | BIN, LPA P2, P3 | **Confirmed** 2026-10-06 |
| R10 | Host bookkeeping per apartment (the loading-a-plugin mark, the plugin-data scheduler, references to Lua values held by native objects) lives in the Lua registry. With `debug` removed, no plugin code can reach the registry | BIN (lookups with the registry pseudo-index), LPA P3 | **Confirmed** 2026-10-06 (BIN only) |
| R11 | Allocator: counts the bytes of each apartment; when a limit is set and an allocation would exceed it, or when the system allocation fails, it notifies the apartment's out-of-memory listeners and refuses. Every resize allocates a new block, copies and frees the old one. No code that sets the limit was found (a further scan of the plugin code region on 2026-10-06 found no writer of the limit field after construction) | BIN, LPA P26 | **Confirmed** 2026-10-06 (BIN only); limit value OPEN |
| R12 | Garbage collection: on each plugin-manager tick the host performs, in **every** apartment, exactly the operation `lua_gc(LUA_GCSTEP, 20)` (the same as `collectgarbage("step", 20)`); it ignores the result. This is the client's only call of `lua_gc`: it never stops, restarts, forces a full cycle or changes pause or step multiplier | BIN (the single `lua_gc` call site, its argument read from a manager field that the manager's constructor sets to 20 and nothing else writes), LPA §3.3 (once per frame) | **Confirmed** 2026-10-06 (BIN only) |
| R13 | All host-to-Lua calls (file loads, event handlers, shell commands, plugin-data callbacks, options panel) use the apartment's **main** Lua thread, never a plugin coroutine | BIN (the call context built at apartment creation holds the main state; the protected-call wrapper uses it), LPA §9.6 | **Confirmed** 2026-10-06 (BIN only) |
| R14 | Protected-call wrapper: increments the apartment's running-code counter (which is what makes `UnloadScriptState` refuse, LPA §6.3), calls `lua_pcall` with no message handler, decrements the counter. If the status is a run-time error or a syntax error, and the caller did not ask for silence, the error value converted with `lua_tolstring` is passed to the apartment's error listeners. A memory error is not reported here (the allocator already reported it, R11); an error-in-error-handling status is not reported at all. A value that is neither a string nor a number is reported as an empty message | BIN | **Confirmed** 2026-10-06 (BIN only) |
| R15 | Exactly one host call site asks for silence: the dispatcher's first attempt to call a table handler as a function (LPA §9.2 step 4.1) | BIN (10 call sites of the wrapper; one passes the silent flag) | **Confirmed** 2026-10-06 (BIN only) |
| R16 | Errors raised by host API functions are raised with `luaL_error` from inside the C function, so they carry **no** `chunk:line:` prefix; the message is the host's text converted to UTF-8 | BIN (the error-raising helper), LUA (`luaL_where` at level 1 is empty for a C function) | **Confirmed** 2026-10-06 |
| R17 | Errors in chunks carry Lua's position prefix; the chunk name of a file is its path, shortened by the stock rule with a 60-byte buffer: a path longer than 52 bytes is shown as `...` followed by its last 52 bytes | R2 (buffer 60), LUA (`luaO_chunkid`), LPA §6.6 | **Derived** (from R2, Confirmed: counts as Confirmed) |
| R18 | The host's panic handler reports the error value (as text) to the apartment's error listeners. If a host error guard is active it then unwinds to that guard (the guarded host operation fails); if none is active it returns, and the stock interpreter then terminates the process. Guards are pushed by a small set of host operations (apartment creation, plugin loading and a few others); the per-tick collector step pushes none | BIN (panic handler, guard list and the functions that push it), LUA (`luaD_throw` after a returning panic handler) | **Confirmed** 2026-10-06 (BIN); termination **Derived** (from BIN and the unpatched interpreter of R2: counts as Confirmed) |
| R19 | Host reads of plugin tables (event handler fields, the options-panel field, many API arguments) use normal indexing (`lua_gettable`) outside any protected call; the client links no raw-set function, so all its table writes go through `lua_settable` and honour `__newindex` | BIN (the generic field-read helper, 304 call sites, and its use by the dispatcher path; import table), LPA §9.2 step 2 | **Confirmed** 2026-10-06 (BIN only) |
| R20 | `PluginData.Load` runs a `.plugindata` file through the interpreter's file loader and calls it with the thread's **current** global table as environment (the host sets no environment; `lua_setfenv` is not even linked). A load failure (syntax error or unreadable file) is reported to the error listeners **and** then raised as an error in the caller | BIN (the generic loader reports run-time, syntax and file errors; the plugin-data reader raises its own error afterwards), R3, LPA §10.1 | **Confirmed** 2026-10-06 (BIN only) |
| R21 | Under the French and German game languages the plugin-data writer emits `,` as decimal separator (LPA P43, P44); the reader is the interpreter's compiler in the C locale (R4). So in the original such a file does not read back as written: a numeric key makes the whole file a syntax error, and a fractional value splits into two entries | R4, R20, LPA §10.4, LUA (numeral lexing stops at `,`) | **Derived** (from Confirmed rows: counts as Confirmed); consistent with LPA S3's reports of comma files |
| R22 | Autoload choices read from `PluginOptions.xml` are kept in a hash map keyed by the plugin `Name` compared **exactly** (case-sensitive); a later `Plugin` element whose `Name` is already present is dropped (first wins). Scaling choices go to a second map | BIN (the options reader and the keyed container: look-up before insert, new entry discarded on a hit) | **Confirmed** 2026-10-06 (BIN only) |
| R23 | Apartment close: the destructor marks the apartment as closing (the mark that makes `PluginData.Save` write synchronously, LPA §10.6), then calls `lua_close`, which runs every pending `__gc` finalizer; finalizer errors during close are discarded by the stock interpreter | BIN (destructor), LUA (`lua_close`) | **Confirmed** 2026-10-06 (sequence, BIN); discard **Reported** (LUA) |
| R24 | Each control records the source position (`chunk:line`) of the Lua code that created it, for the client's diagnostic when a control is resized beyond the graphics device's texture limit (LPA §6.6) | BIN (creation-location helper using `lua_getstack`/`lua_getinfo` at level 1) | **Confirmed** 2026-10-06 (BIN only) |
| R26 | The per-tick step of R12 under the stock 5.1.4/5.1.5 collector rules: one `LUA_GCSTEP` call with 20 lowers the threshold 20 KiB below the allocation and then runs basic collector steps until less than 1 KiB of that debt is left, about 20 basic steps of 2,000 work units each (step size 1 KiB ÷ 100, rounded down to 10, times the step multiplier 200); when the collector is idle the call starts a new cycle. Measured with stock Lua 5.1.5 (64-bit) on heaps of small tables: a full cycle with no new garbage took 37, 572 and 4,571 ticks for 1.6, 25.1 and 201.6 MiB of live heap (about 45 KiB of heap per tick); with 64 KiB of new garbage per tick the heap stayed below 1.6 times its live size | LUA (`lua_gc`, `luaC_step` and `singlestep` rules of the reference interpreter), measurement 2026-10-06 in the dirty scratch area | **Derived** (rests on R12, Confirmed, and LUA: counts as Reported for the arithmetic); measurement **Reported** |
| R27 | Memory and instruction cost of loading every apartment of the M5 exit-criterion-4 plugin set (`plugin-api-usage.md` §9.2), measured with stock Lua 5.1.5 (64-bit), a stub host API and the apartment layout of the descriptors: largest peak 160.2 MiB (live 125.1 MiB after a full collection), second 75.2 MiB (live 60.6 MiB), all others below 19 MiB; largest host call 12.8 M VM instructions; a synthetic plugin building 1,000,000 two-element tables peaks at 170.8 MiB (154.2 MiB live) and executes 9.0 M instructions | `plugin-api-usage.md` §6.12 (U12), LUA; no client code was run | **Reported** 2026-10-06 (published third-party files and our own synthetic file, not the install) |
| R25 | Corpus facts (C1): 168 of 218 descriptors (77 %) name an apartment; no Lua file starts with a UTF-8 byte-order mark, a `#` line or precompiled bytecode; 5,319 of 7,722 files use CRLF line ends; feature use in packages: `getfenv` 66, `setfenv` 12 (the dominant form gives a compiled chunk the caller's `getfenv(0)`), `loadstring` 35, `load` 4, `string.dump` 0, `coroutine` 1, `newproxy` 2 (one plugin, two versions, which only sets `__gc` on the proxy's metatable through `getmetatable`), `collectgarbage` 16 (2 of them call `"stop"` and `"restart"`), `math.randomseed` 6 (all seeded from a `Turbine.Engine` time), `math.random` 22, `table.getn` 68, `math.mod` 17, `unpack` 57, `string.format` with `%d`/`%i` 44 and with `%x`/`%X` 47, `Turbine.Engine.GetCallStack` 17, `ScriptLog` 6, `PluginManager.LoadPlugin` 37, `UnloadScriptState` 33, `import` with a computed name 30; 7 hexadecimal numerals above 0xFFFFFFFF occur in 4 packages | C1 | **Reported** (third-party census, 2026-10-06) |

---

<a id="3-scope"></a>
## 3. Scope and relation to other specs

<a id="3-1"></a>
### 3.1 Which spec says what

| Topic | Authoritative section |
|---|---|
| Libraries opened, removed globals, stock file loader | `lua-plugin-api.md` §3.1–§3.2 (summary in §5.2 here) |
| Discovery, descriptor, package grammar on disk | `plugin-descriptor.md` §4, §5, §7.1 |
| `import` resolution, namespaces, run-once | `lua-plugin-api.md` §7 (summary in §6–§7 here) |
| Load and unload sequences, `/plugins`, options panel | `lua-plugin-api.md` §6 (summary and additions in §8 here) |
| Event dispatch | `lua-plugin-api.md` §9 |
| Plugin data calls, timing, file format | `lua-plugin-api.md` §10 (runtime consequences in §6.3, §11.5 here) |
| Interpreter build, C-library behaviour, collector, allocator, limits, protected calls, panic path, file access summary | **this spec** |

<a id="3-2"></a>
### 3.2 Terms

- **Apartment**: one Lua state (`lua-plugin-api.md` §6.1).
- **Tick**: one pass of the plugin manager's per-frame update (`lua-plugin-api.md` §3.3). In
  Tessera the tick is frame-loop step 7 (decision 0014 D8): the UI tick, then the plugin
  host's deferred work, then the collector step (§8.4).
- **Frame-loop step**: a numbered step of Tessera's frame loop, `docs/ARCHITECTURE.md` §20.3
  as decision 0014 D8 rewords it. Step 1 is input, step 7 the UI tick and plugin host, step 8
  UI layout and paint.
- **Host call**: any call the client (or Tessera) makes into Lua: running a file, firing an
  event, running a shell command, calling a plugin-data handler, asking for the options panel.
- **Report**: passing a message to the apartment's error listeners, which the original shows in
  the chat window (`lua-plugin-api.md` §6.6).

---

<a id="4-interpreter"></a>
## 4. The interpreter

<a id="4-1"></a>
### 4.1 Version and build (R1, R2)

- Reference **Lua 5.1.4**, unpatched, as a separate DLL built against the Visual C++ 2005 C
  runtime. Number type `double`; integers for the C API are 64-bit (`ptrdiff_t`).
- The 5.1 compatibility options are the reference defaults: `table.getn`, `math.mod`, `unpack`,
  `gcinfo`, `newproxy` exist; `table.setn` raises an error; `string.gfind` raises an error
  that names its replacement.
- Tessera: vendor PUC-Rio Lua **5.1.x** (5.1.5 is acceptable, `lua-plugin-api.md` Appendix B)
  with the same build limits (§4.2), and apply the conversion rules of §4.4 so that plugin-visible
  results match the original on every operating system.

<a id="4-2"></a>
### 4.2 Build limits (R2)

| Limit | Value | Effect a plugin sees |
|---|---|---|
| Nested C calls (`LUAI_MAXCCALLS`) | 200 | The 200th nested C-level call (through `pcall`, metamethods, host calls, the parser's nesting) fails with Lua's C-stack-overflow error. Beyond 225 the interpreter raises an error-in-error-handling status, which the host does not report (R14). The parser uses the same counter: source nested more than 200 levels deep fails to compile. |
| Active call-info entries (`LUAI_MAXCALLS`) | 20000 | Unbounded Lua recursion (non-tail calls) fails with Lua's stack-overflow error when the call-info array would grow past 20,000 entries; with the stock doubling from 8 entries that happens when 16,384 calls are active. It is a run-time error and is reported. |
| Pattern captures (`LUA_MAXCAPTURES`) | 32 | A pattern with more than 32 captures raises an error. |
| Upvalues per function (`LUAI_MAXUPVALUES`) | 60 | A function closing over more than 60 locals fails to compile. |
| Chunk-name buffer (`LUA_IDSIZE`) | 60 bytes | File chunk names longer than 52 bytes are shortened (§11.3). |
| Collector pause / step multiplier | 200 / 200 | The automatic incremental collector runs with the reference defaults (§9.2). |

Tessera's vendored build must use these values (they are also the reference defaults).

<a id="4-3"></a>
### 4.3 Two C runtimes (R4)

The original process holds two C runtimes with separate state:

| Runtime | Used by | Locale |
|---|---|---|
| Universal C runtime | the client: descriptor parsing, `import` name check, plugin-data key check and writer, path handling | Set at start-up from the game language (`lua-plugin-api.md` §3.4, P43) |
| `MSVCR80` | the interpreter: Lua's number/text conversions, `string` library character classes and case mapping, `string.format`, string ordering, `math.random`, `print` | Never changed: the **C locale** |

Consequences:

1. Inside Lua, numbers always format and parse with `.` as decimal separator, on every game
   language.
2. Lua patterns `%a %c %d %l %p %s %u %w %x` and `string.upper`/`string.lower` treat only ASCII
   as letters, digits, punctuation or space; bytes 0x80–0xFF belong to none of these classes
   and are not case-mapped.
3. `<`, `<=`, `>`, `>=` on strings compare bytes as unsigned values (C-locale collation).
4. By contrast, `import` package names and plugin-data keys are checked by the **client's**
   runtime in the game-language locale (`lua-plugin-api.md` §7.2, §10.2), and the plugin-data
   writer's decimal separator follows that locale (§6.3, R21).

**Tessera requirement:** Lua's conversions and character classes must behave as in the C
locale regardless of the user's environment. The process must not switch the C runtime's
global locale away from "C" (or the vendored Lua must be built so that its conversions and
`ctype` calls ignore the process locale).

<a id="4-4"></a>
### 4.4 C-library behaviour visible to plugins (R5, R6)

The table lists every behaviour of the original that differs from a typical Linux build of the
reference interpreter. "Linux default" means PUC-Rio 5.1 built against glibc on x86-64.

| Operation | Original (Windows, `MSVCR80`, C locale) | Linux default | Tessera |
|---|---|---|---|
| `tostring(n)`, `..` with a number, `print`, `%g`/`%e` in `string.format` | `%.14g`; exponent has **at least three digits**: `1e+020`, `1.5e-007`, `1.2345678901234e+014` | two-digit exponent: `1e+20` | **Should** produce the original's text: pad the exponent to three digits. |
| Same, non-finite values | The runtime's own text, e.g. `1.#INF`, `-1.#INF`, and for NaN `-1.#IND` (the NaN that `0/0` yields) or `1.#QNAN` (Reported, CRT) | `inf`, `-inf`, `nan`, `-nan` | > **OPEN:** confirm the exact strings `tostring` gives for `1/0`, `-1/0`, `0/0` and `-(0/0)` (owner black-box check, §16). Until then Tessera uses the four strings shown. |
| `tostring` of a table, function, userdata or thread | `<type>: ` followed by 16 upper-case hexadecimal digits, no `0x` (Reported, CRT) | `<type>: 0x…` lower case | **Should** use the original's form; plugins rely on uniqueness, not on the digits (54 of 155 C1 packages call `tostring(self)`). |
| `tonumber(s)` and string-to-number coercion, decimal | C-locale `strtod`: no `inf`, `infinity`, `nan`, no hexadecimal floating form (`0x1p4`); surrounding ASCII white space allowed | glibc accepts `inf`, `nan`, `0x1p4` | **Must** reject the forms the original rejects. |
| Same, hexadecimal (`"0x…"`, also numerals in source code) | After `strtod` stops at `x`, the text is converted with a 32-bit `strtoul`: `0x10` → 16; `0x100000000` → **4294967295** (saturated); `-0x10` → **4294967280** (wrapped) | 64-bit `strtoul` or glibc `strtod` hex: `0x100000000` → 4294967296, `-0x10` → −16 | **Must** reproduce the original's results: saturate at 4294967295 (in `tonumber`, string coercion and source numerals alike); wrap a leading minus modulo 2^32 (only in text conversion, `tonumber` and coercion, where the minus is part of the text). A source expression `-0x10` is the unary minus applied to the numeral 16 and gives −16. |
| `string.format("%d" / "%i" / "%c", x)` | `x` truncated toward zero to a **32-bit** signed integer; outside −2^31…2^31−1 the value becomes −2147483648 | 64-bit `long`: exact up to 2^63 | **Must** reproduce the 32-bit result. |
| `string.format("%o" / "%u" / "%x" / "%X", x)` | `x` truncated to a 64-bit signed integer, then only the **low 32 bits** are formatted: `%x` of −1 → `ffffffff`, of 2^32 + 5 → `5`; values beyond the 64-bit range give `0` | 64-bit unsigned long | **Must** reproduce the low-32-bit result. |
| `math.random`, `math.randomseed` | §4.5 | glibc `rand`, range 0–2^31−1 | **Should** reproduce (§4.5). |
| String ordering | bytewise unsigned | bytewise under the C locale | Same; never use locale collation. |
| `string.upper/lower`, pattern classes | ASCII only | ASCII only in the C locale | Same (§4.3). |
| `print` | Writes to the process's standard output, which the windowed client discards | — | `lua-plugin-api.md` §3.2: route to the script log at debug level. |

> **OPEN (implementation, M5, `crates/script`):** the M5 core applies the rows above by
> replacing the library functions `tostring`, `tonumber`, `string.format`, `math.random`
> and `math.randomseed` (and `print`, which uses that `tostring`). Conversions done inside
> the vendored interpreter itself keep the platform's results: the `..` operator and
> `table.concat` with numbers (`1e20 .. ""` gives `1e+20`), string coercion of number
> arguments by other library functions, text-to-number coercion in arithmetic
> (`"0x100000000" + 0` gives 4294967296), and hexadecimal numerals in source text
> (`0x100000000` gives 4294967296). Matching those needs a patched interpreter, which
> decision 0023 leaves out of M5. Also unconfirmed: the text `string.format` gives for a
> non-finite value with `%e`, `%f` or `%g` (the M5 core passes the platform's `inf`/`nan`
> through), and the result of a hexadecimal text with a leading minus whose magnitude
> exceeds 32 bits (the M5 core gives 4294967295, the saturated value, without wrapping).

<a id="4-5"></a>
### 4.5 Random numbers (R7)

- `math.random()` returns *r* = (*g* mod 32767) / 32767, where *g* is the next value of the C
  runtime's generator (0–32767). So there are only 32767 distinct results, the largest is
  32766/32767, and `math.random(m, n)` is ⌊*r* · (*n* − *m* + 1)⌋ + *m* (arguments truncated to
  32-bit integers).
- The generator is the runtime's linear congruential generator: state *s* (32-bit, wrapping)
  becomes *s* · 214013 + 2531011; *g* is bits 16–30 of the new state. `math.randomseed(x)` sets
  *s* to the low 32 bits of the integer part of *x*. The initial state is 1 (Reported, CRT).
- **One generator for the whole client:** all apartments run on the main thread (R13) and share
  it. Seeding in one apartment changes the sequence every other apartment sees, and without any
  seeding the sequence starts the same in every session. In C1, the packages that seed do so
  from a `Turbine.Engine` time value.
- **Tessera:** should implement the generator exactly as above, as one generator shared by all
  apartments of the client, initial state 1, so that seeded plugins draw the same numbers as in
  the original.

<a id="4-6"></a>
### 4.6 Floating-point environment (R8)

During host calls the divide-by-zero exception is masked: `1/0` is +infinity, `-1/0` is
−infinity, `0/0` and `x % 0` are NaN, and none of these interrupts the plugin. Tessera's
default IEEE environment already behaves this way; Tessera must not enable floating-point traps
around Lua calls.

---

<a id="5-apartments"></a>
## 5. Apartment states

<a id="5-1"></a>
### 5.1 Creation sequence (R9; `lua-plugin-api.md` §6.1, §7.4)

When an apartment is first needed (first plugin naming it, `lua-plugin-api.md` §6.2 step 1):

1. Create a Lua state with the host allocator (§9.1) and install the panic handler (§11.4).
2. Open, in this order: base (which also creates `coroutine`), `table`, `string`, `math`,
   `debug`. Nothing else.
3. Register the global `import` (§7).
4. Remove `dofile`, `loadfile`, `require`, `module` from the globals and move `debug` into a
   host-private slot (`lua-plugin-api.md` §3.2, P3).
5. Run `import "Turbine"` (native classes, then a merged disk package `Plugins/Turbine/` if one
   exists, `lua-plugin-api.md` §7.4).

Apartment creation runs inside the host's get-or-create-by-name operation, which holds a host
error guard (R18): a failure there abandons the operation instead of terminating the client.

<a id="5-2"></a>
### 5.2 Globals a plugin's first line sees

| Global | Present | Notes |
|---|---|---|
| `_G`, `_VERSION` (`"Lua 5.1"`) | yes | |
| `assert`, `collectgarbage`, `error`, `gcinfo`, `getfenv`, `getmetatable`, `ipairs`, `load`, `loadstring`, `newproxy`, `next`, `pairs`, `pcall`, `print`, `rawequal`, `rawget`, `rawset`, `select`, `setfenv`, `setmetatable`, `tonumber`, `tostring`, `type`, `unpack`, `xpcall` | yes | Stock 5.1.4 behaviour, with §4.4. Tessera rejects binary chunks in `load`/`loadstring` (`lua-plugin-api.md` §3.2). |
| `coroutine`, `string`, `table`, `math` | yes | Complete libraries. |
| `debug`, `io`, `os`, `package`, `dofile`, `loadfile`, `require`, `module` | **no** (nil) | |
| `import` | yes | §7. |
| `Turbine` | yes | With `Turbine.Engine`, `Turbine.PluginData`, `Turbine.PluginManager`, … |
| `Plugins`, `plugin` | during a plugin load | `lua-plugin-api.md` §6.2. |

<a id="5-3"></a>
### 5.3 Isolation between apartments

- Each apartment is a **separate Lua state**: its own globals, registry, string metatable,
  collector and memory count. No Lua value is ever shared or passed between apartments.
- `Plugins` lists only the plugins of its own apartment; `PluginManager.GetLoadedPlugins()`
  lists all apartments (with `ScriptState`, `lua-plugin-api.md` §6.7).
- Native singletons (`Turbine.Chat`, `LocalPlayer`, class-level handlers) are distinct per
  apartment (`lua-plugin-api.md` §9.5).
- Shared between apartments, outside Lua: the random generator (§4.5); the file system (plugin
  data files written by one apartment are readable by another under the same key and scope; the
  common `Script.log`); the shell command table and chat; the plugin manager calls (`LoadPlugin`,
  `UnloadScriptState`), which is how one apartment unloads another (`lua-plugin-api.md` §6.3).
- Within one apartment nothing is isolated except package namespaces (§6.1); every plugin of the
  apartment can read and replace every other's globals and handlers.

<a id="5-4"></a>
### 5.4 Host bookkeeping (R10)

The host keeps its per-apartment state (the loading-a-plugin mark, the plugin-data queue,
references from native objects to Lua handlers) in the registry. Plugins cannot observe or
modify it. Tessera may keep the same state anywhere outside the Lua-visible globals.

---

<a id="6-environments"></a>
## 6. Environments inside an apartment

<a id="6-1"></a>
### 6.1 Package namespaces (summary of `lua-plugin-api.md` §7.2)

While a file's top-level code runs, the thread's global table **is** the file's namespace table
`T`, which falls back to the real globals through an `__index` metatable; functions created
then keep `T` as environment. Bare assignments land in `T`; the real globals are reached
through `_G`.

<a id="6-2"></a>
### 6.2 `getfenv(0)`, `loadstring`, `load`

- `getfenv(0)` returns the thread's current global table: `T` during a top-level run, the real
  global table at any other time (event handlers, shell commands, plugin-data callbacks, the
  options panel all run with the real global table current, R13).
- `loadstring`/`load` give the new function the thread's current global table (stock rule).
- The common idiom in C1 (R25) is to compile user text with `loadstring` and then set the new
  function's environment to the caller's `getfenv(0)`. Tessera must implement `getfenv(0)` as
  the thread's current global table, swapped exactly as in `lua-plugin-api.md` §7.2.

<a id="6-3"></a>
### 6.3 Plugin-data chunks (R20, R21)

In the original, a `.plugindata` file is **Lua code** run in the apartment: it is compiled with
the thread's current global table as its environment (the real globals, or `T` when `Load` is
called from top-level code) and called with no arguments. Nothing restricts what it does. It is
compiled by the interpreter in the C locale, so files written with `,` decimals by French or
German clients do not read back as written (R21; worked example §15.4).

Tessera never executes these files; it parses them with the restricted grammar of
`lua-plugin-api.md` §10.4, which accepts both decimal separators. This is a deliberate safety
and portability deviation.

---

<a id="7-packages"></a>
## 7. Packages and `import` (summary)

| Rule | Where specified |
|---|---|
| Name grammar: dot-separated non-empty components of `_` and bytes passing the client's `isalnum` in the game-language locale; Tessera accepts ASCII letters, digits and `_` only | `lua-plugin-api.md` §7.2, `plugin-descriptor.md` §7.1 |
| Resolution: native registry, then disk (`Plugins/C1/…/Ck/__init__.lua` if `Ck` is a directory, else `Ck.lua`), merge when both exist, case-insensitive paths | `lua-plugin-api.md` §7.2, §7.4 |
| Run once per apartment per name; `import` returns nothing; failure raises an error naming the package | `lua-plugin-api.md` §7.1–§7.3 |
| Chunk loader: stock file loader; first-line `#` skipped; byte-order mark not skipped; precompiled chunks accepted (Tessera: rejected; BOM accepted) | `lua-plugin-api.md` §3.1, §7.2 |
| A file's own package name is readable as `_.Name` during its top-level run | `lua-plugin-api.md` §7.2 |

Runtime notes added here:

- The name check runs in the **client's** C runtime (§4.3), so it is the only place where the
  game-language locale affects which `import` names work. The open question about bytes
  0x80–0xFF (`lua-plugin-api.md` §7.2) is therefore a property of the platform's Universal C
  runtime (or of Wine's implementation of it under Proton), not of the client or the
  interpreter.
- In C1, 30 of 155 packages compute the `import` name at run time; no file uses a byte-order
  mark, a `#` first line or bytecode (R25), so Tessera's loader deviations affect no observed
  plugin.

---

<a id="8-lifecycle"></a>
## 8. Lifecycle

<a id="8-1"></a>
### 8.1 Callbacks a plugin receives

| Callback | When | Arguments | Run as | On error |
|---|---|---|---|---|
| Top-level code of the main package and everything it imports | Plugin load, `lua-plugin-api.md` §6.2 step 5 | none | protected host call, environment `T` (§6.1) | Reported; the load fails (§11.5) |
| `Load` event on the plugin object | Right after a successful step 5 | sender, empty table | event dispatch (`lua-plugin-api.md` §9.2) | Reported per handler; the plugin stays loaded |
| `PluginData.Load` handler | During loading: immediately; otherwise on a later tick (Tessera: a step-7 deferred-work item, §8.4) | the loaded value | protected host call | Reported |
| `PluginData.Save` completion | After the queued write is attempted (Tessera: a step-7 deferred-work item, §8.4) | success flag, message | protected host call | Reported |
| `ShellCommand:Execute` | User types the command | command word, argument text | protected host call | Reported |
| `GetOptionsPanel` | Each time the manager shows the plugin's details | plugin object | protected host call | Not shown as a panel; reported |
| Event handlers (`Update`, mouse, chat, gameplay events) | When the host fires them (Tessera: input handlers in frame-loop step 1, `Update` in step 7, events raised by game or session state queued to step 7; §8.4) | sender, argument table | event dispatch | Reported per handler |
| `Unload` event on each plugin object | Apartment unload, logout, client exit (`lua-plugin-api.md` §6.3; Tessera timing §8.4) | sender, empty table | event dispatch | Reported; unloading continues |
| `__gc` finalizers | Collector steps (§9.2), apartment close | the object | inside the collector | §9.3 |

<a id="8-2"></a>
### 8.2 Loading and its order

The load sequence is `lua-plugin-api.md` §6.2. Ordering facts:

- `PluginManager.LoadPlugin` only queues; requests are processed in a later tick, one after
  another, in queue order (`lua-plugin-api.md` §6.2). Tessera: each request is one step-7
  deferred-work item (§8.4).
- **Autoload (R22):** the original keeps the autoload choices of `PluginOptions.xml` in a hash
  map keyed by the exact `Name` text; a repeated `Plugin` element with the same `Name` is
  ignored. The order in which autoloaded plugins load is therefore not the file order.
  > **OPEN:** the iteration order of that map when autoload fires, and whether autoload goes
  > through the `LoadPlugin` queue or loads directly. Tessera: load autoloaded plugins in the
  > order of its own autoload list (`lua-plugin-api.md` §6.2), one at a time, each fully
  > (top-level code and `Load`) before the next, each as one step-7 deferred-work item (a
  > Tessera reading of decision 0014 D8, which does not name autoload; §8.4), and none before
  > the first plugin file index is ready (decision 0015 §4.4; §8.4).
- **Loads asked for interactively** (`/plugins load`, the manager UI's load action). Where in
  its frame the original runs them is not known. Tessera choice (§8.4): each is queued as one
  step-7 deferred-work item, in the same queue as `LoadPlugin` requests, so every load runs
  in step 7 in the order in which it was asked for.
- **No load before the first file index** (Tessera rule, decision 0015 §4.4). Tessera checks
  plugin image paths against an index of the whole `Plugins/` tree (§12;
  `plugin-descriptor.md` §4.2), built on an asset worker when the plugin host starts. No
  plugin is loaded, by any trigger, before that first index is ready (§8.4).
- Plugins sharing an apartment load into one state in sequence: a later plugin's top-level
  code sees every global an earlier one created.

<a id="8-3"></a>
### 8.3 Unload and close (R23; `lua-plugin-api.md` §6.3)

1. Refused, silently, while any Lua code of the apartment is running (R14's counter).
2. `Unload` fires on each plugin of the apartment in load order; each `Plugins[name]` is
   cleared.
3. Native objects (windows, controls, shell commands, menus) are destroyed.
4. The apartment is marked closing; from now on `PluginData.Save` writes synchronously.
5. `lua_close`: every object with a pending `__gc` finalizer has it run, in the stock 5.1
   order; an error raised by a finalizer here is discarded and the remaining finalizers still
   run.
6. The state's memory is released; the apartment no longer exists. A later load naming it
   creates a fresh state (§5.1).

When these steps run in Tessera's frame loop (an unload asked for by a plugin or the user,
the logout unload, the unload at client exit) is §8.4, from decision 0014 D8. The steps
themselves are unchanged.

<a id="8-4"></a>
### 8.4 Where host calls run in Tessera's frame (decision 0014 D8)

This section is Tessera's rule, not a fact about the original. The original runs every host
call on its main thread (R13); where in its frame each kind runs is stated only in part
(`lua-plugin-api.md` §3.3: the UI tick order, input outside the tick; its OPEN on the order
of the deferred work). The rule lives in decision 0014 D8 and is restated here for the
plugin host; if the two ever differ, D8 governs and this section must be revised, with
the explicit exception named below. The autoload loads as deferred items and the
queuing of interactive loads go beyond D8's text without contradicting it (Tessera readings
of D8, marked where they appear). One rule **differs** from D8's text and is an explicit
exception to "D8 governs" (until decision 0014's Amendment 1 of 2026-10-06 there were two;
that amendment made the second, the skip set of queued `Received` events, part of D8):

- the **wait for the first plugin file index** (below), grounded in the Accepted decision
  0015 §4.4 (the host loads no plugin before the first index is ready). While the head of
  the deferred-work queue is a load and that index is not ready, no item runs in the frame,
  which lifts D8's rule that at least one item runs in each frame (D8 step 7, the slice
  rule). Where the two Accepted decisions meet, 0015 §4.4 governs for this case only.

**Placement.** Every host call runs on the main thread, in frame-loop step 1 or step 7, with
one exception, the unload at client exit (below). No host call runs in steps 2–6
(session, simulation) or in step 8 onward (UI layout and paint, rendering), and none runs on
another thread (§10.3). UI notifications that game or session code causes are no exception:
the UI changes behind them are applied in step 7 (below, "UI changes made outside steps 1
and 7").

**Step 1, input.** The plugin input handlers (`MouseDown`, `MouseUp`, `MouseClick`,
`MouseDoubleClick`, `MouseMove`, `MouseEnter`, `MouseLeave`, `MouseWheel`, `KeyDown`, `KeyUp`,
`FocusGained`, `FocusLost`) and the notifications they cause run while the input mapper
offers events to the UI (0014 D7, D8). Raw events that arrive between redraws are handled in
the window system's callbacks as they arrive; that work, plugin calls included, counts
toward step 1 of the following frame. Input handler calls are never deferred, dropped or cut
by a per-frame slice.

**Step 7, the tick.** It begins with the queued UI changes (below, "UI changes made outside
steps 1 and 7"), then, in this order:

1. the UI tick of `lua-plugin-api.md` §3.3: if the pointer needs re-evaluating, the hit test
   and the hover refresh (with its `MouseLeave` and `MouseEnter`); then the hover check
   (`MouseHover`), the `Update` broadcast to every subscriber, then the rest of the UI's
   per-frame work except layout and paint. Plugin timers are `Update` handlers that read
   the game time; the API has no other timer. The broadcast is never cut by a slice;
2. the plugin host's **deferred work**: queued `LoadPlugin` requests (§8.2), the autoload
   loads and the interactive loads (§8.2), asynchronous `PluginData.Load` handlers and
   plugin-data writes with their completion callbacks (§8.1), the game-raised plugin events
   (below) and the logout unload (below);
3. the collector step of §9.2 in every apartment, once per tick, whatever the deferred work
   left queued;
4. then non-plugin work (the UI's input-map changes are handed to the input mapper, 0014 D5).

**Slicing of the deferred work** (Tessera rule, 0014 D8). The host runs the queued items in
queue order. Once the deferred work of the current frame has taken 2 ms of plugin time, it
starts no further item; the rest waits for the next frame's step 7. At least one item runs
in every frame in which items are queued (with the one exception of the wait for the first
file index, below), and an item, once started, is never interrupted (each host call inside
it keeps its own instruction budget, §10.1). Because queue order is kept, load order
(`lua-plugin-api.md` §6.2) and the per-key order of plugin-data operations
(`lua-plugin-api.md` §10.6) are unchanged. Only the deferred work is sliced: the input
handlers, the `Update` broadcast and the collector step are not.

The autoload loads are not named in 0014 D8. They are plugin-host work of the same kind as
queued `LoadPlugin` requests, and D8 allows plugin code only in steps 1 and 7, so Tessera
queues one deferred item per autoloaded plugin, in the order of its autoload list, when the
session reports the login (Tessera reading of D8). Loads asked for with `/plugins load` or
the manager UI's load action are queued the same way, one item each, when the chat line or
click that asks for them is handled in step 1 (Tessera choice; D8 names only unloads for
these triggers). So every load, whatever its trigger, is a step-7 item and keeps the order in
which it was asked for.

**Wait for the first file index** (Tessera rule, decision 0015 §4.4; the first explicit
exception to "D8 governs" above). The index of the whole
`Plugins/` tree (§12, `plugin-descriptor.md` §4.2) is built on an asset worker when the
plugin host starts; the main thread never waits for it. Until that first index is ready, no
load item starts: when the item at the head of the queue is a load, that frame's deferred
work stops there, the rule "at least one item runs in every frame" does not apply, and the
items behind it wait too, so queue order is kept. A walk that fails, or finds no `Plugins/`
folder, still yields an index (empty), which is logged. Refreshed indexes (on a refresh of
the available list) never hold back loads: until a rebuilt index is swapped in, lookups use
the previous one.

**Game-raised plugin events** (Tessera rule, 0014 D8 and its Known difference 21).
`Turbine.Chat.Received` for every chat line the client shows (lines that arrive from the
session, Tessera's own system notices, lines written by `Turbine.Shell.WriteLine`, and the
plugin error lines of §11.6; `lua-plugin-api.md` §6.6, §11.1, §11.3), and the `…Changed`
events of the gameplay views (`lua-plugin-api.md` §15), are not delivered when the state changes:

- the code that raises such an event (session code, the chat window's line intake, the
  gameplay views) never calls Lua. It appends the event, with its argument values taken at
  that moment, to the deferred-work queue;
- one queued event is one item. At delivery it goes to the handlers that are set at that
  moment; an apartment closed in between gets nothing;
- a getter called inside such a handler returns the value at delivery, which may already
  include a later change made in the same frame. Two changes of one value in one frame still
  give two events, in order;
- an event of this kind raised while plugin code runs (in step 1 or step 7) is queued the
  same way, so it never re-enters Lua from inside a handler;
- the native side (the chat window's own log, for example) is not delayed; only the plugin
  delivery is;
- events that are UI notifications keep the delivery of `lua-plugin-api.md` §9.7 and are not
  queued.

**Skip set of queued `Received` events** (Tessera rule, 0014 D8 as amended on 2026-10-06,
Amendment 1 item 1; this closes the former OPEN on the re-entrancy of `Received`, Q11).
`lua-plugin-api.md` §9.6 (BIN) says that a chat line produced while a `Received` dispatch is
running (for example by `Shell.WriteLine` in a handler) is not delivered to that apartment's
`Received` handlers. Queued without a further rule, such a line would reach that apartment one
item later, and a handler that writes a line for every line it receives would feed itself
without end. So:

- each queued `Received` event carries a **skip set** of apartments. A line raised while a
  queued `Received` event is being delivered to an apartment gets the skip set of that
  event plus that apartment (the apartment whose `Received` dispatch is running). A line
  raised at any other time (a session line, a system notice, a `WriteLine` from an input
  handler, an `Update` handler or another deferred item) gets an empty skip set;
- delivery skips every apartment in the event's skip set; every other apartment gets the
  line as usual;
- so a chain of lines that echoing apartments write in answer to one another visits each
  apartment at most once: a chain in which each line is written in answer to the one
  before it has at most as many written lines as there are apartments, so the lines end.
  This is what §9.6's nested synchronous delivery gives: there, an apartment whose
  dispatch is still running further up the chain gets no line from below it.

`lua-plugin-api.md` §11.3 states the same rule and tests the two-apartment case in its
invariant 65; invariants 20 and 21 below test it here.

**Unload timing** (Tessera rule, 0014 D8; the steps are §8.3).

- An unload that a plugin or the user asks for (`PluginManager.UnloadScriptState`,
  `/plugins unload`, the manager's unload action) runs where the request is made: inside the
  plugin call, or inside the step-1 handling of the chat line or click that asked for it. So
  it runs in step 1 or step 7. §8.3 step 1 still applies (refused while the apartment's code
  runs).
- The **logout** unload (every apartment) is queued by the session code that reports the
  logout and runs in step 7 as deferred work, one item per apartment, in queue order and
  sliced like the other items, so plugin-data writes queued before it keep their order.
- The **client-exit** unload is the one exception to the placement rule. When the event loop
  exits, after the last frame has been presented and before the renderer and the window are
  destroyed, the host unloads every apartment on the main thread: the `Unload` handlers, then
  the close of §8.3 with its synchronous `PluginData.Save` (`lua-plugin-api.md` §10.6). Every
  handler call keeps its instruction budget. No frame follows, so the frame-time rule does
  not apply.

**Writes at unload** (Tessera rule, 0014 Amendment 1 item 9, `docs/ARCHITECTURE.md` §21
exception (b); this closes the former OPEN on blocking writes at unload, Q13). An unload in
step 1 or step 7 closes Lua states, and §8.3 step 4 makes `PluginData.Save` write
synchronously while a state closes. That blocking file I/O on the main thread is a stated
exception to ARCHITECTURE §21's no-blocking-I/O rule: the synchronous writes and the flush
when a Lua state closes, for an unload asked for by the user or a plugin (step 1 or 7), the
logout unload (step 7) and the client-exit unload (after the last frame). The close keeps the
synchronous write that §8.3 and `lua-plugin-api.md` §10.6 require.

**Accounting.** Time inside plugin Lua calls is measured as plugin time: the step-1 input
handlers wherever in the event loop they run, and in step 7 the plugin notifications raised
when the queued UI changes are applied, the `Update` broadcast, the deferred work and the
collector step. Plugin time is kept out of Tessera's own UI budget
(row U1 of decision 0015 §6) and out of its paint slice, but it counts toward the frame-time
rule (no frame over 50 ms). The client-exit unload is plugin time outside any frame.

**UI changes made outside steps 1 and 7** (Tessera rule, 0014 D8 as amended on 2026-10-06,
Amendment 1 item 2; this closes the former OPEN on UI notifications outside steps 1 and 7,
Q12). `lua-plugin-api.md` §9.7 delivers UI notifications synchronously, so a UI change made
outside steps 1 and 7 would run plugin handlers there: for example a native panel shown with
activation by a session event in step 2, which deactivates the active plugin window and takes
focus from its `TextBox`, or a window resize or scale change handled in a window-system
callback, which re-lays out root panels and raises `SizeChanged` and `PositionChanged`.
Decision 0014 settles this by moving the changes, not by widening the placement rule:

- session code (step 2), simulation and game code (steps 3–6) and the window system's size and
  scale-factor callbacks never make such a UI change directly; they queue a UI change request
  in `tessera-ui` (show and hide, `HideWhenField`, activation, focus, z-order, moves and
  resizes, the screen size and scale factor);
- the queue is applied at the start of step 7, before the hit test and hover refresh, in
  request order and not sliced; the plugin notifications it raises run there, each one host
  call with its own instruction budget, and count as plugin time like input handlers;
- changes made in step 1 or step 7 (input routing, plugin code, the UI tick, deferred work)
  apply at once, as before; step 8 raises no plugin notification.

So the placement rule above holds for every host call, UI notifications included.

---

<a id="9-memory"></a>
## 9. Memory and garbage collection

<a id="9-1"></a>
### 9.1 Accounting and limits (R11)

- The original counts each apartment's allocated bytes. A limit field exists, and when it is
  non-zero an allocation that would push the count past it is refused (Lua then raises its
  memory error) after the apartment's out-of-memory listeners are notified. A failed system
  allocation is handled the same way.
- No code that sets the limit was found, so in practice the original has **no** per-apartment
  memory limit.
  > **OPEN:** whether any path sets the limit, and its value (narrowed 2026-10-06: none in the
  > apartment creation path or in the plugin code region).
- `collectgarbage("count")` reports the interpreter's own count for the calling apartment only.
- **Tessera:** a configurable per-apartment cap, a deliberate safety deviation. On refusal:
  notify as an out-of-memory report for that apartment, let Lua raise its memory error, and
  keep the client running.
  - The cap counts the bytes the apartment's Lua state requests from its allocator (block
    sizes as passed to the allocator, new size minus old size on a resize), not allocator
    overhead and not memory the host keeps outside the Lua state for native objects. Source
    text the host reads before compiling counts only if the host stores it in the Lua state.
  - **Default: 512 MiB per apartment** (Tessera choice, revision 2; it replaces the 64 MiB that
    `lua-plugin-api.md` §3.3 names, which the M5 plugin set does not fit, §9.4). A user setting
    may raise or lower it per apartment name.
  - The limit must be checked against the peak, not the live size: Lua 5.1 has no emergency
    collection, so garbage not yet collected counts until the collector reaches it (§9.4).

<a id="9-2"></a>
### 9.2 Collector stepping (R12)

- The automatic incremental collector runs with pause 200 and step multiplier 200 (§4.2).
- In addition, on every tick the host performs `lua_gc(LUA_GCSTEP, 20)` in **every** apartment,
  loaded plugins or not (R12). Under the 5.1 rules (R26) this call:
  1. sets the collection threshold 20 KiB below the current allocation, which adds 20 KiB of
     collector debt;
  2. runs basic collector steps, each worth 2,000 work units (the step size of 1 KiB divided by
     100, rounded down to 10, times the step multiplier 200), and after each step pays 1 KiB of
     the debt, until less than 1 KiB of debt is left — about **20 basic steps per call**, not
     one; it stops early if a cycle completes during the call;
  3. starts a new cycle if the collector was idle, so with the host step cycles follow one
     another without waiting for the pause threshold.

  Measured effect with stock Lua 5.1.5 on heaps of small tables (R26): about 45 KiB of heap is
  traversed per tick, so a full cycle over *L* KiB of live heap takes roughly *L* / 45 ticks
  (37 ticks at 1.6 MiB, 572 at 25.1 MiB, 4,571 at 201.6 MiB); with 64 KiB of new garbage per
  tick the heap stayed below 1.6 times its live size. These figures are informative; the rule
  is the call itself.
- The step runs **outside** any protected call and outside any host error guard (R18).
- The host never stops or restarts the collector. A plugin's `collectgarbage("stop")` lasts only
  until the next tick: the host's step resets the threshold, so automatic collection resumes
  (stock 5.1 rule). `collectgarbage("collect")`, `"step"`, `"setpause"` and `"setstepmul"` from a
  plugin act on its own apartment only.
- **Tessera:** must perform the same call, `lua_gc` with `LUA_GCSTEP` and 20, in every
  apartment once per tick, and must not otherwise stop, restart, force or re-parameterise the
  collector (pause 200, step multiplier 200 stay as in §4.2). `lua-plugin-api.md` §3.3 still
  says Tessera "may rely on the default collector"; this section is authoritative for the
  collector (§3.1) and that wording is superseded (§17).
- Where in the tick the step runs is not known for the original. Tessera choice, now fixed
  by decision 0014 D8 (step 7, item 6 there): in frame-loop step 7, after the UI tick (with
  the `Update` broadcast) and after the plugin host's deferred work, and before the UI's
  input-map changes, layout, paint and rendering (§8.4). It runs in every tick, whatever
  deferred work the 2 ms slice left queued, it is never cut by that slice, and its time
  counts as plugin time.

> **Note (implementation, M5):** a state at its memory cap cannot run the `lua_gc` call
> itself (the call needs a few bytes). The M5 core therefore lifts the cap by 1 MiB for the
> duration of the per-tick step; finalizers run in that step can allocate at most that much
> more. Without that, a state that hit its cap stays at it, because Lua 5.1 has no
> emergency collection.

<a id="9-3"></a>
### 9.3 Finalizers

- Plugins can create finalizable objects only through `newproxy(true)` and setting `__gc` on
  the proxy's metatable (tables cannot have finalizers in 5.1; host userdata finalizers are the
  host's). In C1 exactly one plugin (two versions) does this, and only by assigning
  `getmetatable(proxy).__gc` (R25).
- When a finalizer runs inside a collector step that a plugin's own code triggered (an
  allocation or `collectgarbage`), an error it raises propagates into that code like any other
  error and is reported by the enclosing host call.
- When it runs inside the host's per-tick step (§9.2), there is no protected call: the error
  reaches the panic handler, is reported, and — with no guard active — the stock interpreter
  terminates the client (R18, Derived).
- At apartment close, finalizer errors are discarded (§8.3).
- **Tessera:** never terminates. A finalizer error in any context is reported for its
  apartment like a run-time error and does not stop other finalizers. Running finalizers under
  the instruction budget is allowed (answers the implementer question in
  `lua-plugin-api.md` §3.3): the only observed pattern assigns `__gc` through `getmetatable`,
  so a metatable view that forwards writes and installs a metered dispatcher is compatible with
  every plugin in C1; raw access to a proxy's metatable (`rawget`, `rawset`, `next`) does not
  occur in C1.

<a id="9-4"></a>
### 9.4 Limits measured against the M5 plugin set (R27)

Method: every apartment of the exit-criterion-4 set (`plugin-api-usage.md` §9.2, its apartment
layout from the descriptors) was loaded with stock Lua 5.1.5 built for 64-bit, the globals of
§5.2, `import` as in `lua-plugin-api.md` §7.2 and a stub host API; then the `Load` event,
queued plugin-data callbacks and 1,000 ticks of `Update` delivery, each tick followed by the
§9.2 step. Bytes are allocator requests as defined in §9.1; instructions are VM instructions
of one host call. Full table: `plugin-api-usage.md` §6.12.

| Quantity | Largest value in the set | Where |
|---|---|---|
| Peak bytes of one apartment | 160.2 MiB (live 125.1 MiB after a full collection) | the apartment of #2, at its load, which imports four databases of 33.7 MB of Lua source in one host call |
| Second-largest peak | 75.2 MiB (live 60.6 MiB) | #14, at its load |
| Smallest cap under which #2 loads | between 159 and 160 MiB (refused at 159 MiB) | — |
| Smallest cap under which #14 loads | between 75 and 75.2 MiB | — |
| Every other apartment | below 19 MiB | — |
| VM instructions of one host call | 12.8 M | a #1 helper's load with every annotation cell requested; #2's load executes 4.0 M |

Derivation of the defaults:

1. **Instruction budget: keep 500 million per host call** (`lua-plugin-api.md` §3.3). The
   largest call uses 12.8 M, a margin of 39 times.
2. **Memory cap: 512 MiB per apartment** (§9.1). 64 MiB refuses #2 and #14 during their load.
   A cap must cover the peak including uncollected garbage (no emergency collection in 5.1).
   After a load the collector's pause of 200 lets a heap grow to about twice its live size
   before the next automatic cycle, and the §9.2 step bounds steady state lower (below 1.6
   times live in R26); twice the largest live heap measured is 250 MiB. 512 MiB covers that
   with a margin of 2, and the largest measured peak with a margin of 3.2.
3. **Compilation is outside the budget.** Instructions are counted only while Lua code runs.
   Compiling #2's 33.7 MB of source took 1.2–1.7 s of wall time on a 2020 laptop CPU (Intel
   i9-10885H) for 4.0 M VM instructions, and the parser held up to 7.6 times a file's size at
   its compile peak. The source a plugin can compile is bounded by its own files and by the
   strings it builds, which the memory cap bounds; no separate compile limit is required for
   the M5 set.

> **OPEN:** steady-state memory after user interaction (searching #2's databases, browsing #14)
> was not measured, because the stub host cannot drive those windows. The exit-criterion-4 run
> records the peak per apartment; if any exceeds 256 MiB the 512 MiB default must be revisited.

---

<a id="10-execution"></a>
## 10. Execution limits, threads and coroutines

<a id="10-1"></a>
### 10.1 Time

The original has **no** instruction-count or time limit: no hook is ever installed (R3). A
plugin that loops forever freezes the client. Tessera adds an instruction budget per host call
(`lua-plugin-api.md` §3.3, ARCHITECTURE §18.1), a deliberate safety deviation; when it trips,
the call fails with an error reported like a run-time error, and the apartment stays loaded.
The default stays **500 million VM instructions per host call**; nested `import` calls and
coroutines resumed inside a host call count towards that call's budget. The M5 plugin set uses
at most 12.8 M in one call (§9.4).

The budget is per host call, not per frame. Tessera's per-frame rules for plugin code are
decision 0014 D8 (§8.4): input handlers, the `Update` broadcast and the collector step are
never cut by a per-frame slice; only the step-7 deferred work is sliced, by whole items, at
2 ms per frame; and plugin time counts toward the frame-time rule (no frame over 50 ms) but
not toward the UI budget of decision 0015 §6.

<a id="10-2"></a>
### 10.2 Depth

Only the build limits of §4.2 apply. Tessera must keep them and must turn each into a Lua
error, never a crash; a native stack overflow on deep recursion through host functions is not
acceptable.

<a id="10-3"></a>
### 10.3 Threads

In the original every host call runs on the main thread and uses the apartment's **main** Lua
thread (R13).

Tessera (decision 0014 D8; this replaces the earlier wording "between frames", which is how
`docs/ARCHITECTURE.md` §18.1 puts it; `lua-plugin-api.md` §3.3 now gives D8's reading too):
all plugin code runs
on the client's main thread, in frame-loop step 1 (the input handlers) and step 7 (the UI tick
with the `Update` broadcast; the deferred work, which includes the plugin events raised by game
and session state and the logout unload; the collector step), with one exception, the unload
of every apartment at client exit, which runs after the last frame. It never runs during steps
2–6 or 8 onward (session, simulation, layout, paint, rendering) and never on another thread.
UI notifications caused by game or session code follow the same rule, because the UI changes
behind them are applied at the start of step 7 (§8.4, decision 0014 Amendment 1 item 2).
Host calls always use the apartment's main Lua thread (R13). Lua states are never touched
from another OS thread.

<a id="10-4"></a>
### 10.4 Coroutines

- The `coroutine` library is complete and stock. A plugin may create coroutines and resume them
  from any handler (the typical form is resuming a worker coroutine from `Update`).
- A host call cannot be yielded across: calling `coroutine.yield` directly in an event handler,
  shell command or other host call fails with Lua's error about yielding across a C-call
  boundary (stock 5.1), reported like any error.
- `pcall`, metamethods and iterators also cannot be yielded across (stock 5.1).
- > **OPEN:** whether every API function behaves the same when called from inside a plugin
  > coroutine as from the main thread (it then receives the coroutine's state). Only 1 of 155
  > C1 packages uses coroutines. Tessera: API functions must work from any coroutine of the
  > apartment, and any event they fire synchronously is dispatched on the main thread.

---

<a id="11-errors"></a>
## 11. Errors

<a id="11-1"></a>
### 11.1 Protected host calls (R14, R15)

Every host call goes through one wrapper:

1. Mask the divide-by-zero trap (§4.6).
2. Increment the apartment's running-code counter.
3. Call the function with no message handler, so no traceback is produced.
4. Decrement the counter.
5. Classify the status:
   - success: return true;
   - run-time error or syntax error: report the error value converted to text, unless the caller
     asked for silence (only the table-handler probe, `lua-plugin-api.md` §9.2 step 4.1, does);
   - memory error: not reported here (the allocator reported it, §9.1);
   - error while handling an error (C-stack overflow past 225, §4.2): not reported.
6. Return false on any failure. The caller decides what failure means (§11.5).

Error values: a string is reported as is; a number is reported in its `%.14g` text (§4.4);
**any other value** (nil, boolean, table, userdata, function) is reported as an **empty**
message.

<a id="11-2"></a>
### 11.2 Errors raised by the API (R16)

API functions raise errors from inside the host function, so the message has no
`chunk:line:` prefix even though `error` in Lua code at level 1 would have one. A plugin that
catches such an error with `pcall` receives a plain string. Tessera's API errors must also be
plain Lua strings (never userdata wrappers) without a position prefix; the wording is Tessera's
own.

<a id="11-3"></a>
### 11.3 Message format (R17)

- Errors from Lua code carry the stock prefix `<chunk name>:<line>: `.
- For a file the chunk name is its path as passed to the loader. If that path is longer than 52
  bytes, the name is `...` followed by the path's last 52 bytes (example §15.1). Strings compiled
  by `loadstring` use the stock `[string "…"]` form.
- Tessera passes the path relative to `Plugins/` as chunk name (`lua-plugin-api.md` §7.2 step
  3) and applies the same 52-byte rule.

<a id="11-4"></a>
### 11.4 Unprotected paths and the panic handler (R18, R19)

Some host work touches Lua outside any protected call:

| Unprotected work | Can raise because |
|---|---|
| The per-tick collector step (§9.2) | A `__gc` finalizer raises |
| Host reads of plugin tables with normal indexing: handler fields before dispatch, the options-panel field, table arguments | An `__index` metamethod raises (plugin-set metatables on controls, class tables or argument tables) |
| Host writes with normal assignment into plugin tables | A `__newindex` metamethod raises |
| Any allocation by the host | Memory refused (§9.1) |

In the original such an error reaches the panic handler: the message is reported to the
apartment's listeners; then, if a host error guard is active (apartment creation, plugin
loading and a few other operations push one), the guarded operation is abandoned; if none is
active, the interpreter terminates the client.

> **OPEN:** whether a guard is active during event dispatch (which decides whether a raising
> `__index` on a control terminates the original client or only abandons the dispatch). The
> per-tick collector step has none.

**Tessera:** must perform every host access to Lua values in protected mode (or an equivalent)
so that none of these cases can terminate or corrupt the process. Such an error is reported for
its apartment like a run-time error; the event, field read or step it interrupted is abandoned
for that apartment only.

<a id="11-5"></a>
### 11.5 What happens to a failing plugin

- **Load failure** (missing or unreadable main file, syntax error, run-time error in top-level
  code or in anything it imports, failed `import`): reported; `Load` does not fire;
  `Plugins[name]` becomes nil (`lua-plugin-api.md` §6.2 step 8). There is **no rollback**: globals,
  windows, controls, shell commands and handlers created before the error stay alive and
  working until the apartment is unloaded. The apartment itself stays, even if no plugin in it
  loaded. The plugin can be loaded again later; packages that failed are not marked as run and
  are retried (`lua-plugin-api.md` §7.2), packages that succeeded are not re-run.
- **Errors after loading** (handlers, commands, callbacks): each failure is reported and that
  call ends; the plugin stays loaded, its handlers stay assigned and are called again on the
  next event. A handler that fails every frame produces a report every frame. Neither the
  wrapper nor the dispatcher counts failures or disables anything (BIN).
  > **OPEN:** whether any error listener (chat display, log) acts beyond showing the message.
  > Tessera: listeners only display and log.
- **Out of memory:** reported; the allocation fails with Lua's memory error inside the plugin's
  code, which can catch it with `pcall`.
- **Plugin data:** a `.plugindata` file that fails to compile or run is reported **and** the
  `Load` call raises an error in the caller (R20); a top-level `PluginData.Load` therefore turns
  a broken data file into a load failure unless the plugin wraps it in `pcall`.
- **Tessera:** same outcomes, plus: budget exhaustion (§10.1) and the cases of §11.4 behave as a
  run-time error at that point; nothing a plugin does may terminate the client.

<a id="11-6"></a>
### 11.6 Reporting in Tessera

Report format, channel and log routing are `lua-plugin-api.md` §6.6 (plugin name, apartment,
the Lua message; a traceback in the log only). Reports for the same apartment and message
repeated every tick may be rate-limited in the chat display (never in the log) — a
presentation choice the original does not make.

---

<a id="12-files"></a>
## 12. File-system access available to plugins

A plugin has no general file access: `io`, `os`, `package`, `dofile`, `loadfile` and
`require` are absent, and `load`/`loadstring` compile strings only. The complete list of file
operations a plugin can cause:

| Operation | Trigger | Files | Original rule | Tessera |
|---|---|---|---|---|
| Read and run Lua source | `import`, plugin load | `Plugins/…/*.lua`, `__init__.lua` | Package grammar keeps the path inside `Plugins/` (`lua-plugin-api.md` §7.2) | Same; resolve through the plugin tree's virtual file system, never leaving it |
| Read an image | `SetBackground(path)`, `Graphic(path)`, descriptor `Image` | relative to `Plugins/` | Not confined: `..` is not rejected (`lua-plugin-api.md` §4.4) | Confined to `Plugins/` (stated deviation). For `SetBackground(path)`, decision 0015 §4.3–§4.4: existence is checked against an index of the whole `Plugins/` tree, built on an asset worker; the format is checked at call time by one header read of at most 64 bytes on the main thread (a stated exception to ARCHITECTURE §21); `.png` and `.bmp` are refused; the full decode runs on the asset workers |
| Read plugin data | `PluginData.Load` | `PluginData/<account>/…/<key>.plugindata` | Key grammar prevents traversal; the file is executed (§6.3) | Own data directory; parsed, never executed (`lua-plugin-api.md` §10.7) |
| Write plugin data | `PluginData.Save` | same | Truncate and rewrite in place | Atomic write (`lua-plugin-api.md` §10.6) |
| Append to the shared script log | `Turbine.Engine.ScriptLog` | `Script.log` (all plugins, all apartments) | — | Tessera's log directory (`lua-plugin-api.md` §12) |
| Write to standard output | `print` | process output (discarded) | — | Script log at debug level |
| Indirect | `PluginManager.LoadPlugin`, options panel scaling choice | `PluginOptions.xml` is written by the manager, never by plugin code | — | Tessera's own settings |

No other file is opened on a plugin's behalf. Paths given by plugins are UTF-8 Lua strings
(`lua-plugin-api.md` §3.4).

> **OPEN:** which code page the original uses for the full path it hands to the interpreter's
> file loader (it matters only when the settings-folder path contains non-ASCII characters).
> Tessera opens files by native path and is not affected.

---

<a id="13-census"></a>
## 13. Corpus observations (informative, R25)

From C1 (155 packages, 7,722 Lua files, 218 descriptors, 2026-10-06), useful for priorities:

| Observation | Count | Consequence |
|---|---|---|
| Descriptors naming an apartment | 168 of 218 | Most descriptors choose an apartment other than `Default`; the number of states, and with it per-apartment memory and step costs, grows accordingly. |
| Files with BOM / `#` line / bytecode | 0 / 0 / 0 | Loader deviations are harmless. |
| Files with CRLF line ends | 5,319 of 7,722 | The loader must accept CRLF (stock does). |
| `getfenv` / `setfenv` / `loadstring` packages | 66 / 12 / 35 | §6.2 must be exact. |
| `table.getn` / `math.mod` / `unpack` packages | 68 / 17 / 57 | 5.1 compatibility functions are required. |
| `string.format` with `%d`/`%i` / with `%x`/`%X` | 44 / 47 | §4.4 integer conversions matter. |
| Hexadecimal numerals above 0xFFFFFFFF | 7 in 4 packages | §4.4 saturation is observable. |
| `math.random` / `math.randomseed` packages | 22 / 6 | §4.5. |
| `collectgarbage` packages (2 use stop/restart) | 16 | §9.2. |
| `newproxy` with `__gc` | 1 plugin | §9.3. |
| `coroutine` packages | 1 | §10.4. |
| `GetCallStack` / `ScriptLog` packages | 17 / 6 | `lua-plugin-api.md` §12. |
| `LoadPlugin` / `UnloadScriptState` packages | 37 / 33 | Cross-apartment management is common (§5.3). |
| Computed `import` names | 30 packages | §7. |

References to `io`, `os`, `debug` and `require` occur only in a few packages (8–14 each),
almost all in bundled stand-alone tools or behind presence checks; in the original they fail
with an error if reached.

---

<a id="14-invariants"></a>
## 14. Invariants

Checkable with synthetic plugins generated by test code (never shipped third-party plugins).
"Both" means the original and Tessera must agree.

1. **Build limits (both):** a Lua function recursing without bound fails with a reported
   stack-overflow error, not a crash; nesting `pcall` 300 deep fails with the C-stack-overflow
   error once about 200 levels are nested (the host call itself counts as one level); a pattern with 33 captures raises an error; a function using 61
   upvalues fails to compile.
2. **Number text (both):** `tostring(1e20) == "1e+020"`, `tostring(1.5e-7) == "1.5e-007"`,
   `tostring(0.1) == "0.1"`, `tostring(2^53) == "9.007199254741e+015"`, and `tostring(-0) == "-0"`
   when evaluated in a chunk that has no other numeric constant equal to zero (5.1 folds `-0` to
   the constant −0 and shares numerically equal constants within a function, so a chunk that
   also contains `0` gives `"0"`).
3. **Number parsing (both):** `tonumber("inf")`, `tonumber("nan")`, `tonumber("0x1p4")`,
   `tonumber("1,5")` are nil; `tonumber("0x10") == 16`; `tonumber("0x100000000") == 4294967295`;
   `tonumber("-0x10") == 4294967280`; `tonumber(" 12 ") == 12`; the source numeral
   `0x100000000` evaluates to 4294967295, and the source expression `-0x10` to −16.
4. **Integer formats (both):** `string.format("%d", 2^31) == "-2147483648"`;
   `string.format("%d", -2.7) == "-2"`; `string.format("%x", -1) == "ffffffff"`;
   `string.format("%x", 2^32 + 5) == "5"`; `string.format("%X", 0x80000000) == "80000000"`.
5. **Locale independence (both):** with the game or OS language set to French or German,
   `tostring(0.5) == "0.5"`, `string.upper("\233") == "\233"`, `string.find("\233", "%a")` is
   nil, and `"\233" > "z"` is true.
6. **Random (both):** after `math.randomseed(1)`, three calls of `math.random()` return
   41/32767, 18467/32767 and 6334/32767; after `math.randomseed(1)` in apartment A, the next
   `math.random()` in apartment B returns 41/32767 (shared generator).
7. **Division (both):** `1/0 > 1e308`, `0/0 ~= 0/0`, and neither raises.
8. **Collector (both):** in an apartment that called `collectgarbage("stop")` and then produces
   garbage every tick, `collectgarbage("count")` stays bounded over 1,000 ticks.
9. **Error values (both):** a handler that raises `error({})` produces a report with an empty
   message; `error(42)` reports `42`; `pcall` around an API call with an invalid argument
   returns false and a string without a `chunk:line:` prefix.
   > **OPEN (implementation, M5):** with the stock `error`, a number at level 1 gets the
   > position prefix like a string (`lua_isstring` is true for numbers), so `error(42)` in a
   > handler file reports `<chunk>:<line>: 42`; only `error(42, 0)` (or a call whose level
   > has no line) reports `42`. The M5 core keeps the stock rule; confirm in the original
   > whether the invariant means level 0.
10. **No rollback (both):** a plugin whose main file creates a window, registers a shell
    command and then raises an error leaves the window shown and the command working;
    `Plugins[name]` is nil; unloading its apartment removes both.
11. **Persistent handlers (both):** a `Click` handler that always raises produces one report per
    click and stays assigned.
12. **Isolation (both):** a global set in apartment `X` is nil in apartment `Y`; `Plugins` in `Y`
    has no entry for a plugin loaded in `X`.
13. **getfenv (both):** inside a file's top-level run `getfenv(0)` is the file's namespace table;
    inside a later event handler it is `_G`.
14. **Tessera only, robustness:** a `__gc` finalizer that raises during the per-tick step, an
    `__index` metamethod on a control that raises when a handler field is read, and an
    infinite loop in a handler each produce one report and leave the client and all other
    apartments running.
15. **Tessera only, plugin data:** a `.plugindata` file whose content would call a function when
    executed loads as nil (it is never run) and is logged.
16. **Tessera only, default limits:** a synthetic plugin whose main file builds a global table
    of 1,000,000 entries, entry *i* being a new two-element table holding *i* and the string
    `"s"` followed by the decimal digits of *i*, loads under the default limits (it peaks at
    about 171 MiB of allocator bytes on a 64-bit build and executes about 9 M VM instructions);
    with the per-apartment cap set to 128 MiB the same load is refused with Lua's memory error,
    one out-of-memory report is produced for that apartment, and the client and every other
    apartment keep running.
17. **Collector step (both):** in an apartment holding about 25 MiB of live small tables and
    producing no garbage, the per-tick step of §9.2 alone completes a collection cycle (the
    `lua_gc` call reports the end of a cycle) within 1,000 ticks; R26 measured 572.
18. **Tessera only, placement (§8.4, decision 0014 D8 invariant (y)):** a chat line posted in
    frame-loop step 2, and a system notice posted at the start of the redraw, reach
    `Turbine.Chat.Received` handlers in that frame's step 7 (or a later frame's, when the
    deferred-work slice is used up), after that frame's `Update` broadcast; no host call runs
    in steps 2–6 or 8 onward for them. At client exit, `Unload` handlers run after the last
    frame, and a `PluginData.Save` they make is written before the process ends.
19. **Tessera only, deferred-work slice (§8.4):** with ten queued deferred items of 0.8 ms
    of plugin time each, a frame's step 7 runs three of them: they start at 0, 0.8 and 1.6 ms,
    and the fourth, which would start at 2.4 ms, at or past the 2 ms slice, does not start in
    that frame. A single item of 5 ms runs alone and is not interrupted. When a frame's slice
    is used up, the collector step of §9.2 still runs in that frame, and the remaining items
    run in later frames in queue order.
20. **Tessera only, `Received` re-entrancy (§8.4, skip set):** an apartment whose
    `Received` handler calls `Shell.WriteLine` once per line it receives gets each of its own
    written lines **not** delivered to its `Received` handlers, while another apartment's
    handlers get them; a single incoming line therefore causes exactly one written line and
    the deferred-work queue empties.
21. **Tessera only, two echoing apartments (§8.4, skip set):** apartments `A` and `B`
    each have a `Received` handler that calls `Shell.WriteLine` once per line it receives, and
    apartment `C` has a handler that only counts lines. One session line `L0` arrives. `A`
    and `B` each get `L0` and each write one line (`L1` from `A` with skip set {`A`}, `L1'`
    from `B` with skip set {`B`}); `B` gets `L1` and writes `L2` with skip set {`A`, `B`};
    `A` gets `L1'` and writes `L2'` with skip set {`A`, `B`}; neither `A` nor `B` gets `L2` or
    `L2'`. `C` counts exactly five lines (`L0`, `L1`, `L1'`, `L2`, `L2'`), the chat window
    shows the same five, and the deferred-work queue is empty after them, in whatever frames
    the slice spreads them over.
22. **Tessera only, first file index (§8.2, §8.4):** with the first `Plugins/` index held
    back (a test hook that delays the asset worker's walk) and one autoload plugin queued,
    the plugin's top-level code does not run in any frame until the index is ready, other
    frames still run their `Update` broadcast and collector step, and the plugin loads in the
    first step 7 after the index is ready.

---

<a id="15-examples"></a>
## 15. Worked examples (synthetic)

<a id="15-1"></a>
### 15.1 Chunk-name shortening

A plugin file at `C:\Users\Someone\Documents\Game Settings\Plugins\Acme\Clock\Main.lua`
(68 bytes) raises an error on line 7. The chunk name keeps the last 52 bytes, so the reported
message starts with `...\Documents\Game Settings\Plugins\Acme\Clock\Main.lua:7: ` followed by
the Lua error text. A 40-byte path would appear in full. In Tessera the chunk name is
`Acme\Clock\Main.lua` or `Acme/Clock/Main.lua` (relative to `Plugins/`, 19 bytes), shown in
full.

<a id="15-2"></a>
### 15.2 Random sequence

Seeding with 1 sets the state to 1. First step: 1 · 214013 + 2531011 = 2745024; bits 16–30 give
41; `math.random()` = 41/32767 ≈ 0.0012513, and `math.random(6)` = ⌊0.0012513 · 6⌋ + 1 = 1.
The next two generator outputs are 18467 and 6334. A plugin in another apartment drawing next
continues this same sequence.

<a id="15-3"></a>
### 15.3 Conversions

| Lua expression | Original result | Typical Linux 5.1 result |
|---|---|---|
| `tostring(1e20)` | `1e+020` | `1e+20` |
| `string.format("%d", 3000000000)` | `-2147483648` | `3000000000` |
| `string.format("%x", 4294967301)` | `5` | `100000005` |
| `tonumber("0x100000000")` | `4294967295` | `4294967296` |
| `tonumber("-0x10")` | `4294967280` | `-16` |
| `tonumber("inf")` | nil | infinity |

<a id="15-4"></a>
### 15.4 A comma plugin-data file in the original

A plugin on a German client saves the table {[1] = 1.5}. The writer (client runtime, German
locale) produces the lines `return `, `{`, a tab then `[1,000000] = 1,500000`, `}`. Loading it
back, the interpreter (C locale) reads `[1` and then expects `]` but finds `,`: a syntax error.
The error is reported and `PluginData.Load` raises an error. If the key had been the string
`"a"`, the entry line `["a"] = 1,500000` would compile as field `a` = 1 plus a positional
entry 500000, so the table would read back as {a = 1, [1] = 500000}. Tessera writes `1.5` with a
`.` and reads both forms correctly.

<a id="15-5"></a>
### 15.5 Lifecycle of a failing plugin

Apartment `X` does not exist. The user loads plugin `Clock` (package `Acme.Clock.Main`,
apartment `X`):

1. Apartment `X` is created (§5.1); `Turbine` is imported.
2. `Plugins.Clock` and `plugin` are set; the loading mark is set.
3. `Main.lua` runs: it imports `Turbine.UI`, creates and shows a window, adds a shell command,
   then calls a nil value on line 30.
4. The error is reported with chunk name and line 30. `Load` does not fire. The loading mark and
   `plugin` are cleared; `Plugins.Clock` becomes nil.
5. The window stays on screen and the command still works (no rollback). `Acme.Clock.Main` is
   not marked as run, so a second load attempt runs `Main.lua` again (and creates a second
   window), while `Turbine.UI` is not re-run.
6. `/plugins unload X` destroys the window and the command and closes the state.
   > **OPEN:** whether the failed plugin's object is still in the apartment's plugin list, so
   > that `Unload` fires on it at this point. Tessera: a plugin whose load failed is not in the
   > list and receives no `Unload`.

---

<a id="16-open"></a>
## 16. Open questions (summary)

| # | Question | Where | Suggested resolution |
|---|---|---|---|
| Q1 | Exact `tostring` text of `1/0`, `-1/0`, `0/0`, `-(0/0)` in the original | §4.4 | Owner black-box check (method M3): a small plugin that writes these four strings to the chat window. |
| Q2 | Whether any path sets an apartment memory limit, and its value | §9.1 | Further BIN only if a plugin is seen to hit a limit. |
| Q3 | Autoload iteration order and whether autoload uses the `LoadPlugin` queue | §8.2 | BIN on the login-time autoload trigger. |
| Q4 | Whether a host error guard is active during event dispatch | §11.4 | BIN on the dispatcher's callers; does not change Tessera's behaviour. |
| Q5 | API behaviour when called from a plugin coroutine | §10.4 | BIN on a few bindings, or owner M3 check. |
| Q6 | Whether an error listener does more than display | §11.5 | BIN on the chat listener. |
| Q7 | Code page of the loader path | §12 | BIN on the disk resolver; does not affect Tessera. |
| Q8 | Classification of bytes 0x80–0xFF in `import` names (carried from `lua-plugin-api.md` §7.2) | §7 | Narrowed: a property of the platform's Universal C runtime; Tessera's ASCII rule stands. |
| Q9 | Whether a plugin whose load failed still receives `Unload` when its apartment is unloaded | §15.5 | BIN on the load-failure path; Tessera: no `Unload` for it. |
| Q10 | Peak memory of the largest M5 apartments after user interaction | §9.4 | Exit-criterion-4 run records the peak per apartment; revisit the 512 MiB default if any exceeds 256 MiB. |
| Q11 | `Received` re-entrancy under the step-7 queue of game-raised events (`lua-plugin-api.md` §9.6 against decision 0014 D8) | §8.4 | **Closed 2026-10-06** by decision 0014 Amendment 1 item 1: the skip set, inherited along a chain of written lines, is now D8's rule. |
| Q12 | UI notifications that `lua-plugin-api.md` §9.7 delivers synchronously when game or session code changes the UI outside steps 1 and 7 | §8.4, §10.3 | **Closed 2026-10-06** by decision 0014 Amendment 1 item 2: such UI changes are queued and applied at the start of step 7. |
| Q13 | Synchronous `PluginData.Save` during an unload inside a frame against `docs/ARCHITECTURE.md` §21 | §8.4 | **Closed 2026-10-06** by decision 0014 Amendment 1 item 9: ARCHITECTURE §21 exception (b). |

---

<a id="17-other-specs"></a>
## 17. Notes for other specs

Recorded here, not applied, because this session owns only this file.

| Target | Addition or correction | Basis |
|---|---|---|
| `lua-plugin-api.md` §3.3 "Garbage collection" | State the step precisely: `lua_gc(LUA_GCSTEP, 20)` per apartment per tick, outside any protected call; it runs about 20 basic collector steps per call and starts a new cycle when the collector is idle; the host never changes collector parameters; a plugin's `"stop"` lasts one tick. Tessera's step "may rely on the default collector" must become "must perform the same step" (§9.2 here). **Applied in substance** in the working-tree revision of `lua-plugin-api.md` of 2026-10-06 (pending its review): its §3.3 now says Tessera "must make the same step" and that §9.2 here governs; the step details stay here. | R12, R26 |
| `lua-plugin-api.md` §3.3 "Memory" and §20 table row on limits | Replace the 64 MiB default with 512 MiB per apartment and state what the cap counts (§9.1 here); keep 500 million instructions per host call. Until applied, §9.1 and §10.1 here govern. | R27 |
| `lua-plugin-api.md` naming note | Cite the interoperability record `docs/LEGAL.md` §17 instead of the pending ARCHITECTURE §18.2 item (critique B10). | — |
| `lua-plugin-api.md` §3.3 implementer OPEN on `newproxy(true)` | Answer: the metatable-view deviation is acceptable; the only observed use assigns `__gc` through `getmetatable`; no raw access occurs in C1 (§9.3 here). | R25 |
| `lua-plugin-api.md` §3.4 | Add that Lua's own conversions, character classes and string ordering run in the interpreter's separate C runtime in the C locale, unaffected by the game-language locale (§4.3 here). | R4 |
| `lua-plugin-api.md` §6.2 autoload OPEN | Narrow: the original keeps autoload choices in a hash map keyed by exact `Name`, first `Plugin` element wins; order is not file order (§8.2 here). | R22 |
| `lua-plugin-api.md` §6.6 | Add: non-string, non-number error values are reported as an empty message; error-in-error-handling statuses are not reported; API errors carry no position prefix; errors on unprotected paths go to the panic handler (§11 here). | R14, R16, R18 |
| `lua-plugin-api.md` §7.2 OPEN (bytes 0x80–0xFF) | Narrow: the check uses the client's Universal C runtime `isalnum`, so the answer is a platform property (§7 here). | R4, BIN imports |
| `lua-plugin-api.md` §10.4 | Add the original's read-back behaviour for comma files (§15.4 here). | R21 |
| `plugin-descriptor.md` §8.2, §10.2 | Autoload: exact-name map, first `Plugin` element with a given `Name` wins; order not file order. | R22 |
| `lua-plugin-api.md` §3.3, §6.3, §11.3, §15 | Decision 0014's owners list asks that spec's owner to reword "between frames" as D8 does (main thread, frame-loop steps 1 and 7, the client-exit unload after the last frame), to add the game-raised events and the logout unload to its list of deferred work, and to state when unloads and game-raised events run. **Applied** in the working-tree revision of `lua-plugin-api.md` of 2026-10-06, pending its review (its §3.3 Tessera rule and deferred-work list, §6.3, §11.3). §8.4 and §10.3 here follow D8 too. | DEC (0014 D8) |
| `lua-plugin-api.md` §3.3, §6.2, §11.3 | (1) Name the autoload loads and the interactive loads (`/plugins load`, the manager UI) as step-7 deferred items, and the wait for the first plugin file index (decision 0015 §4.4), as §8.2 and §8.4 here do. (2) In §11.3, let a line raised while a queued `Received` event is delivered inherit that event's skip set plus the delivering apartment (§8.4 OPEN, invariant 21 here), pending the revision of 0014 that settles its open review item 1. **Both applied** in the working-tree revision of `lua-plugin-api.md` of 2026-10-06, pending its review: (1) in its §3.3 deferred-work list and §6.2, (2) in its §11.3 OPEN with its invariant 65. Its §3.3 now also states how the two meet (the head-of-queue wait of §8.4 here, which suspends the at-least-one-item rule), so **all requests of this row are applied**, pending that spec's review. (3) **New, 2026-10-06:** decision 0014's Amendment 1 makes the skip set D8's rule (item 1), applies UI changes made outside steps 1 and 7 at the start of step 7 (item 2) and steps the frame clock right after present (item 4); LPA §11.3 can drop "interim … until decision 0014 is revised", and the two §3.3 OPENs (UI notifications outside steps 1 and 7; when the frame clock steps) can close with those rules. Not applied there yet. | DEC (0014 D8, open review item 1 and Amendment 1; 0015 §4.4) |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First version (status Review). New BIN reading of client 4906.0071.1794.4035 and its `lua51.dll` (R1–R24), census C1 (R25). Consolidates the runtime facts of `lua-plugin-api.md` §3, §6, §7, §9, §10 by reference and adds: interpreter build and limits, the two-runtime split and its plugin-visible conversions, `string.format` integer widths, the random generator, the floating-point mask, the collector step size and its consequences, allocator behaviour, protected-call filtering, API error form, chunk-name rule, panic handling and unprotected paths, plugin-data chunk environment and comma read-back, autoload container, close sequence, failing-plugin outcomes, the file-access list, corpus counts, 15 invariants, 5 worked examples. 9 open questions (§16), none blocking. |
| 2026-10-06 | Revision 2 (spec writer, dirty room); status Accepted → Review. **Collector (critique B7):** §9.2 corrects "in practice one incremental step per tick": one `LUA_GCSTEP` call with 20 runs about 20 basic steps and starts a new cycle when the collector is idle, with measured cycle lengths (new R26); the step position in the tick is marked as Tessera's choice (review note 3); §17 restates that `lua-plugin-api.md` §3.3 must say "must". **Limits (critique A7):** §9.1 states what the cap counts and raises Tessera's default to 512 MiB per apartment; new §9.4 records the measurement of the M5 plugin set (new R27) and derives the defaults; §10.1 keeps 500 million instructions per host call; new invariants 16 (synthetic large heap, refused at 128 MiB) and 17 (step completes a cycle); new OPEN in §9.4 and Q10 in §16; §17 asks `lua-plugin-api.md` §3.3 to change its 64 MiB default. **Review notes of the Accepted version:** (1) §4.4 hexadecimal row: wrap of a leading minus applies to text conversion only, a source `-0x10` is −16, invariant 3 extended; (2) the Derived mark is mapped to Confirmed or Reported in §2 and on each Derived row; (3) see collector; (4) invariant 2 states the constant-sharing condition for `tostring(-0)`. **Naming note (critique B10):** cites `docs/LEGAL.md` §17. Interpreter facts R1–R25 unchanged. |
| 2026-10-06 | Revision 3 (spec writer, dirty room); status Accepted → Revised, awaiting review of this change. Applies the Accepted decisions 0014 (UI input bridge, D8) and 0015 (UI rendering, §6) to Tessera's rules only; no fact about the original (R1–R27, their marks and the sections that state them) changed. **§2:** a note on Tessera rules taken from decision records (key DEC, no provenance mark). **§3.2:** "Tick" is frame-loop step 7 in Tessera; new term "frame-loop step". **§8.1, §8.2, §8.3:** cross-references to the new §8.4 for the timing of plugin-data callbacks, event handlers, `LoadPlugin` requests, autoload and unloads. **New §8.4** (where host calls run, from 0014 D8): placement in steps 1 and 7 with the client-exit exception; step-7 order; deferred work sliced by whole items at 2 ms; autoload as deferred items (Tessera reading of D8); game-raised plugin events queued to step 7; unload timing (requested, logout, client exit); plugin-time accounting against 0015 §6 U1 and the 50 ms rule; three OPENs taken from 0014's open review items (`Received` re-entrancy against `lua-plugin-api.md` §9.6, with an interim Tessera rule that keeps §9.6; synchronous saves at unload against ARCHITECTURE §21; synchronous UI notifications outside steps 1 and 7). **§9.2:** the collector step's place is now fixed by 0014 D8 (step 7, after the deferred work, every tick, not sliced); the statement that the original's place is unknown is kept. **§10.1:** per-frame rules for plugin code point to §8.4. **§10.3:** "between frames" reworded as 0014 D8 and its owners list require; the original's main-thread fact (R13) is stated separately. **§14:** invariants 18–20. **§16:** Q11–Q13. **§17:** a row for the `lua-plugin-api.md` sections that 0014's owners list addresses. **§12:** the image row of the file table points to decision 0015 §4.3–§4.4 (whole-tree index, 64-byte header read at call time, `.png`/`.bmp` refused, decode on workers). |
| 2026-10-06 | Repair of revision 3 after its review (spec writer, dirty room); status stays Revised, awaiting review. No fact about the original (R1–R27, their marks) changed. **Header, §2:** decision 0015 is cited for §4.3–§4.4 as well as §6. **§8.2:** autoload as step-7 items labelled a Tessera reading of 0014 D8; new bullets: interactive loads (`/plugins load`, manager UI) are queued step-7 items (Tessera choice), and no plugin loads before the first `Plugins/` index is ready (0015 §4.4). **§8.4:** the "D8 governs" sentence names the rules that go beyond D8 and makes the `Received` skip set an explicit exception, pending the revision of 0014; the tick's first item runs the hit test and hover refresh only if the pointer needs re-evaluating (D8 step 7 item 1); the game-raised `Received` sources now include `Shell.WriteLine` lines and plugin error lines; new paragraph on waiting for the first file index; interactive loads queued like autoload; the interim `Received` rule now gives a line raised during a queued `Received` delivery the delivered event's skip set plus the delivering apartment, so that echoing apartments cannot feed each other (the first pass covered only one apartment). **§10.3:** `lua-plugin-api.md` §3.3 no longer says only "between frames". **§14:** invariant 19 reworded (the fourth item, due at 2.4 ms, does not start); new invariants 21 (two echoing apartments) and 22 (first file index). **§16:** Q11 updated. **§17:** the collector row and the 0014 owners-list row are marked applied in the working-tree `lua-plugin-api.md`, pending its review; a new row asks that spec for the load timing, the index wait and the inherited skip set. |
| 2026-10-06 | Second repair of revision 3 after its re-review (spec writer, dirty room); status stays Revised, awaiting review. No fact about the original changed; no section number changed. **§8.4:** the "D8 governs" sentence now names two explicit exceptions, the wait for the first file index (grounded in the Accepted decision 0015 §4.4; it lifts D8's "at least one item runs in each frame" while the head of the queue is a load) and the `Received` skip set; the index-wait paragraph is labelled as the first exception; the end of the re-entrancy OPEN no longer says that `lua-plugin-api.md` §11.3 lacks the inherited skip set (its working tree states it, with invariant 65). **§17:** the load-timing and skip-set row is marked applied in the working-tree `lua-plugin-api.md`, pending its review; its only remaining request is that LPA §3.3 state the head-of-queue wait and the suspension of "at least one item". |
| 2026-10-06 | Amendment for decision 0014's Amendment 1 (spec writer, dirty room); awaiting review of this change. No fact about the original changed; no section number changed. **§8.4:** the interim skip set of queued `Received` events is now D8's rule (0014 Amendment 1 item 1), so the OPEN on `Received` re-entrancy becomes a stated rule and "D8 governs" keeps one explicit exception (the wait for the first file index); the OPEN on UI notifications outside steps 1 and 7 becomes the rule that UI changes made by game or session code and by the window system's size and scale-factor callbacks are queued and applied at the start of step 7 (item 2), and step 7 begins with them; the OPEN on blocking writes at unload becomes ARCHITECTURE §21 exception (b) (item 9). **§10.3:** the qualification by the §8.4 OPEN is replaced by the step-7 rule. **Invariants 20, 21:** labels no longer say "interim". **§16:** Q11–Q13 closed. **§8.4 Accounting:** names the notifications of queued UI changes. **§17:** the LPA row asks LPA §3.3 and §11.3 to close their matching OPENs. |
