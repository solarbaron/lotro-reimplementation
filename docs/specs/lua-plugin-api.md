<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Lua plugin API (compatible plugin surface)

| Field | Value |
|---|---|
| Status | **Accepted** |
| Last updated | 2026-10-05 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | Claude review session (spec-writer agent) |
| Review record | Claude review session (spec-writer agent), 2026-10-05: changes requested (named-colour component values in §13.9/A.5 do not match the original, which uses byte/255 rounded to 3 decimals; the plugin-data key character rule in §10.2/P14/invariant 12 is locale-dependent in the original, not ASCII-only; O4 key count 472 should be 471; unsourced behaviour statements in §13.1, §13.10, §14.2, §14.5; OPEN count in change log is 22, not 23; naming note cites `LEGAL.md` §10, which has no interoperability record). Writer's revision 2026-10-05 addresses every item (change log). Re-review: Claude review session (spec-writer agent), 2026-10-05: **passed, Accepted** (every §5.3 item true; spot checks against the install, the owner's settings folder, S3 and targeted BIN all passed: 141 named colours / 423 components follow the 3-decimal rule, O4 and §15 counts, game-language setlocale and both `isalnum` checks, `GetScriptVersion` = 13, 0, 48 font records, Appendix A.1 vs S3, `PluginOptions.xml` structure; 25 precise OPENs, none blocking) |
| Implements into | `crates/script` (Lua host, sandbox), `crates/plugins` (discovery, apartments, API surface), `crates/ui` (controls) |
| Related specs | [`plugin-descriptor.md`](plugin-descriptor.md) (disk side: discovery, descriptor, `PluginOptions.xml`; see §20 for corrections to it), dat container spec (block header, DID routing), `records.md` §5 (0x41 render surfaces), §7 (enum mappers), §8 (DID maps), [`keymap.md`](keymap.md) (input actions), `ui-layout.md` (native panels) |

> **Naming note (interoperability).** The namespace, class, method, event, field, enum member,
> XML element/attribute and file names in this document are the identifiers that existing
> third-party plugins and their files use. They are reproduced only because a plugin cannot
> run without them (see `docs/ARCHITECTURE.md` §18.2 and the *Google v. Oracle* row of
> `docs/LEGAL.md` §4.1; the written interoperability record that ARCHITECTURE §18.2 asks for
> is still pending as its "OPEN (legal)" item). They must appear
> in Tessera only inside the plugin subsystem, never as product names, crate names or UI
> strings. Descriptive text is our own summary. No documentation prose, client message text
> or game text is reproduced: error conditions are described, never quoted, and game-data
> enum entries are referred to by DID and key, not by their internal names.

---

## 1. Purpose

This spec lets an implementer build a Lua 5.1 plugin host that runs existing user-interface
plugins written for the original client, without reading any third-party code. It covers:

- the runtime and sandbox the original gives plugins (§3);
- where plugins live, how they are discovered and how the `.plugin` descriptor is read (§4,
  §5);
- the apartment (script-state) model and the plugin load/unload lifecycle (§6);
- `import`, package resolution and the per-package environments (§7);
- the native object model that Lua code subclasses, argument passing and string encoding
  (§8);
- the event model: handler forms, dispatch, argument tables (§9);
- plugin-data persistence: calls, timing, validation, location and the exact file format
  (§10);
- the shell/chat, engine, UI and gameplay namespaces (§11–§15);
- a complete API inventory of client build 4906.0071.1794.4035 (§16), implementation tiers
  (§17), invariants (§18) and the remaining open questions (§19).

Out of scope: UI skinning (`SkinDefinition.xml`), the music (ABC) system, third-party plugin
managers' own files, and the network-side meaning of gameplay values (those come from the
session layer).

---

## 2. Provenance

Source keys:

| Key | Source |
|---|---|
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, PE file version **4906.0071.1794.4035** (`lotro_live`, built 2026-09-27), and its `x64/lua51.dll`, read in the dirty room on 2026-10-05 with a targeted capstone disassembly of individual functions (no whole-image dump kept). Code read: the Lua host (state creation, allocator, protected-call wrapper, error reporting, per-frame step), `import` and the package resolvers, plugin discovery and descriptor parsing, plugin load and unload, the plugin-data load/save/serialiser/path code, the shell and shell-command bindings, the engine functions, the event dispatcher and the control event-argument builders, the class and enumeration registration code (names, implementations, enum values), the text-markup tag parser, the `PluginOptions.xml` writer and the chat listener entry. A follow-up repair session on the same day read, again function by function: the start-up call that sets the C runtime locale (the only other locale calls switch the numeric category to the C locale for the duration of two value-conversion functions that the plugin code does not call, then restore it), the number formatter used by the plugin-data writer, the plugin-data key validator, the package-name check of the disk resolver, the named-colour registration, and the function bound as `GetScriptVersion` (questions: P5, P14, P35, P43, P45; M1–M3 could not answer them because they are code paths, and S3 shows only resulting values). **Questions:** everything marked BIN below (sandbox contents, resolution order, environments, re-import, load/unload sequence, dispatch, argument fields, plugin-data timing, key rules, file format, shell semantics, engine values, API surface and enum values). **Why M1–M3 were not enough:** all of these are code, not data (M1 can observe only the data files and the owner's settings folder, which holds no plugins); the published references (S2, S3) list names but not behaviour, and are older than this build; M3 would need the official client running third-party plugins on the owner's account, which is not available to this session and could not expose internal decisions (resolution order, sandbox contents, serialiser rules) exhaustively. Nothing about the network protocol, login, keys, the launcher or anti-tamper was read. Numeric constants are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). |
| **S1** | `research/notes/engine-client-re.md` §5 (dirty-room notes) |
| **S2** | Official "Update 25" Lua API reference, HTML, © 2016 publisher, lotrointerface.com file 1054 (version 2500.0062.0883); mirrored in the `lunarwtr/lotro-api-docs` repository (`U25/`) |
| **S3** | `lunarwtr/vscode-lotro-api` (MIT): `lotro-docs/TurbineDump.json` (a dump of the live in-game `Turbine` table, v1.0.18, 2026-03), `TurbineEventsDump.json` (event argument tables captured in game), `TurbineDumper.lua` (a lightly edited `.plugindata` file), `resources/xsds/lotroplugin.xsd`, EmmyLua stubs generated from S2 |
| **S4** | The publisher's 2010 beta sample plugins (`Turbine/` utility package and `TurbinePlugins/`), archived as `dist/Beta_LuaPlugins.zip` in `Goldbishop/Lotro_LUA`; S2 also mirrored there (U24) |
| **S5** | lotro-wiki.com pages "Lua Plugins", "Lua Scripting", "Plugin manager" (Wayback copies), which quote the 2010–2011 developer forum posts; lotrointerface.com file page 1054 |
| **O1** | Observed in user install 2026-10-03, static inspection of files only: `lua51.dll` (32- and 64-bit) version string and export table; `x64/lotroclient64.exe` and `lotroclient.exe` import tables and literal strings |
| **O2** | Observed in the owner's settings folders (read-only). 2026-10-03: a `Plugins/` tree with two third-party plugins (descriptors, file names and call patterns read statically, nothing copied). 2026-10-05: that `Plugins/` tree is no longer present; `PluginData/<account>/<server>/PluginOptions.xml` exists for three worlds (folder names are the world display names, one with a bracketed prefix and a space) |
| **O3** | Observed in user install data files 2026-10-03 (read-only probes): font records `0x42xxxxxx` in `client_local_English.dat` and `client_general.dat`; image records `0x41xxxxxx` in `client_surface.dat` |
| **O4** | Observed in user install data files 2026-10-05 (read-only, M1): the named-enum-mapper DID map 0x28000001 (`records.md` §8) and the enum mappers it names (`records.md` §7), decoded with their base chains: input actions 0x230000AF (290 own keys) with base 0x23000176 (181 keys), 471 keys in all, no key in both; character classes 0x2300000E (no base, 212 keys); species 0x2300000B (no base, 123); languages 0x230000B1 (3 own keys) with base 0x23000186 (2), 5 in all; shortcut types 0x23000137 (no base, 13); item qualities 0x23000067 (no base, 6); item classes 0x23000036 (no base, 279); craft tiers 0x23000063 (no base, 18); chat text types 0x230002A8 (39 own keys) with base 0x230002A9 (9), 48 in all. Key 0 is present in every one of these chains. Counts re-taken 2026-10-05 by the repair session; they are observations of this install, read at run time |
| **O5** | Observed 2026-10-05 (read-only): the owner's `lotro.keymap` in the Proton prefix binds the scroll-up input action to the mouse's Z axis in the positive direction and the scroll-down action to the negative direction |

Marks: **Confirmed** = observed in the owner's install (data, settings folder or the client
binary), with date; BIN facts are Confirmed by disassembly on 2026-10-05 and are marked
"BIN"; facts that rest only on BIN are flagged as such in the row. **Reported** = from
published sources only.

| # | Fact or group of facts | Sources | Mark |
|---|---|---|---|
| P1 | Embedded Lua is the unmodified reference 5.1.4 interpreter in both clients | O1, BIN (`lua51.dll` carries the 5.1.4 identification string and the stock export set) | **Confirmed** 2026-10-05 |
| P2 | The client opens exactly base (with `coroutine`), `string`, `table`, `math` and `debug`; not `io`, `os`, `package` | O1 (imports), BIN (state creation calls the five openers and nothing else) | **Confirmed** 2026-10-05 (BIN) |
| P3 | After opening, the client sets the globals `dofile`, `loadfile`, `require`, `module` to nil, moves the `debug` table into a host-private slot (unreachable from Lua) and removes the global `debug` | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P4 | `.plugin` descriptor element and attribute names | O1, O2, S3 xsd, S4, BIN | **Confirmed** |
| P5 | Package resolution is folder-first: `Plugins/C1/…/Ck` as a directory runs its `__init__.lua`, otherwise `Ck.lua`; package names are dot-separated non-empty components whose bytes pass the C runtime's `isalnum` test (the byte passed sign-extended) or are `_` | BIN (package-name check of the disk resolver, re-read 2026-10-05 by the repair session: one `isalnum` call per byte of the UTF-8 name, `.` as separator, `_` accepted); S1, S3 xsd, S4 | **Confirmed** 2026-10-05 (BIN); class of bytes 0x80–0xFF OPEN (§7.2) |
| P6 | Package and file lookup is case-insensitive (Windows file system) | O2 | **Confirmed** 2026-10-03 |
| P7 | Native package names are `Turbine`, `Turbine.Gameplay`, `Turbine.Gameplay.Attributes`, `Turbine.UI`, `Turbine.UI.Lotro`; `Turbine` is imported automatically when an apartment is created | O1, S2, S3, BIN | **Confirmed** 2026-10-05 |
| P8 | A chunk runs with its package's namespace table as its environment; the namespace table falls back to the real globals through a metatable `__index` | BIN (exact mechanism, §7.2); S4, O2 (behaviour) | **Confirmed** 2026-10-05 (BIN) |
| P9 | The global `Plugins` maps a plugin's `Name` to its `Turbine.Plugin` object; the global `plugin` is the plugin being loaded, and is reset to nil when loading ends | O1, O2, BIN | **Confirmed** 2026-10-05 |
| P10 | Apartments are separate Lua states; plugins without `Configuration` share the apartment `Default`; unloading is per apartment; an apartment cannot be unloaded while any of its Lua code is running (which includes a call from inside itself) | BIN; S2, S3 xsd, S5, O2 | **Confirmed** 2026-10-05 (BIN) |
| P11 | Default apartment name `Default`; apartment names compare case-insensitively | BIN | **Confirmed** 2026-10-05 (BIN) |
| P12 | Font enum values are DIDs of type `0x42`; all 48 non-zero values exist as records; each record's payload starts with its own DID then the point size | O3, S3, BIN (values) | **Confirmed** 2026-10-03 |
| P13 | Image DIDs passed to `SetBackground` are render-surface records in `client_surface.dat` | O3, O2, S2, BIN | **Confirmed** |
| P14 | Plugin-data keys are 1 to 64 UTF-16 units, each passing the C runtime's `isalnum` test (unit value passed as is) or equal to `_`, stored as `<key>.plugindata`; account scope in `AllServers`, server scope in `<server>/AllCharacters`, character scope in `<server>/<character>` under `PluginData/<account>/` | BIN (key validator re-read 2026-10-05 by the repair session: length check, then one `isalnum` call per UTF-16 unit, `_` accepted), O2 | **Confirmed** 2026-10-05 (corrects the 63-character limit of the first draft; the first revision's "ASCII only" was an overclaim, see P43) |
| P15 | `.plugindata` files are a Lua chunk `return …` in the layout of §10.4, CRLF line ends | BIN (writer and reader), S3 | **Confirmed** 2026-10-05 (BIN) |
| P16 | Event handlers are fields on objects; a field may hold a function or a table of functions | BIN (dispatcher, §9.2), S2, O2 | **Confirmed** 2026-10-05 (BIN) |
| P17 | `ShellCommand:Execute` is called with the command word and the argument text, both strings | S4, O2, BIN (always two string arguments) | **Confirmed** 2026-10-05 |
| P18 | `Plugin:GetConfiguration` and `Shell.Execute` do not exist | S3, O1, BIN (not registered) | **Confirmed** absent 2026-10-05 |
| P19 | Member lists, signatures and enum values of §11–§16 and Appendix A | BIN (registration code: names, implementations, every enum value), S2 (parameter types), S3 | **Confirmed** 2026-10-05 (names and values, BIN); Reported (S2 parameter types) |
| P20 | Every non-zero `Turbine.UI.Lotro.Action` value is a key of the input-action enum mapper (0x230000AF and its base chain) | O4, BIN, O2 | **Confirmed** 2026-10-05 (269 of 269) |
| P21 | Newer API members exist in this build: Control edge attachments and min/max sizes, Window global-scaling members, `Display.GetUIGroupScale`, `Plugin:GetUsesGlobalScaling`, event `UIScaleChanged`, enums `UIScalingGroupType` and `EdgeAttachmentType`. `ComboBox` is named but not registered; `LuaScriptLog` is a log channel, not an event | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P22 | Plugins cannot issue game commands or automate input | S5, P18, BIN (no such entry point registered) | **Confirmed** (absence) |
| P23 | `PluginOptions.xml` per account and world holds the manager's autoload and scaling choices; its structure is §5.4 | O2, BIN (writer) | **Confirmed** 2026-10-05 |
| P24 | Lua strings cross the API as UTF-8, converted to and from the client's UTF-16 text | BIN (every string conversion in the binding layer uses code page 65001) | **Confirmed** 2026-10-05 (BIN only) |
| P25 | Protected calls into Lua use no message handler, so reported errors carry no traceback; runtime and syntax errors go to the apartment's error listeners | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P26 | The host allocator supports a per-apartment byte limit, but no code that sets a limit was found; no instruction-count hook is installed | BIN (allocator; `lua_sethook` not imported) | **Confirmed** 2026-10-05 (BIN only); limit value OPEN |
| P27 | Discovery reads only `Plugins/<folder>/*.plugin`, one level deep; the first descriptor with a given `Name` wins | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P28 | Descriptor parse rules, field limits and rejections of §5.2 | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P29 | Plugin load sequence, `Load` only on success, failed plugins removed from `Plugins`; `LoadPlugin` is deferred; `UnloadScriptState` is immediate | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P30 | `import` returns nothing; each package name runs at most once per apartment; native and disk packages of the same name are merged | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P31 | Event argument field names and the events that carry them (§9.3) | BIN; S3 (captured values) | **Confirmed** 2026-10-05 |
| P32 | `MouseWheel.Direction` is +1 for the scroll-up input action (key 0x82) and −1 for scroll-down (key 0x10) | BIN, O4, O5 | **Confirmed** 2026-10-05 |
| P33 | `PluginData.Load` is synchronous only while a plugin is loading; otherwise it needs a handler and delivers later. `Save` serialises at once and writes later | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P34 | Shell command rules: names split on `;`, `:`, `,`, trimmed, case-insensitive; counts returned; default help texts empty | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P35 | Engine values: `GetDate` bases, `GetLocalTime` 32-bit, `GetLocale` from the OS locale, `GetScriptVersion` = 13, 0 | BIN (`GetScriptVersion`: the repair session followed the function object bound under that name on 2026-10-05; its body stores the constant 13 in the first half of a pair of 32-bit integers whose second half is 0) | **Confirmed** 2026-10-05 (BIN only) |
| P36 | Lists are 1-based; `Get` out of range raises; `IndexOf` gives −1 when absent | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P37 | `Turbine.UI.Color()` with no or wrong argument count is transparent black; 3 arguments give alpha 1; named colours P45 | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P38 | Text markup tags `b`, `i`, `u`, `p`, `li`, `rgb=#RRGGBB` | BIN (the client's only markup tag parser), S2 | **Confirmed** 2026-10-05 (BIN) |
| P39 | `GetMoneyComponents` returns copper, silver (0–999), gold, in that order | BIN | **Confirmed** 2026-10-05 (BIN only; corrects S2) |
| P40 | Shortcut data string formats per shortcut type (§14.6) | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P41 | `Class`, `Race`, `Language`, `ShortcutType`, `ItemQuality`, `ItemCategory`, `CraftTier` and `ChatType` values are keys of the matching data enum mappers; `Equipment` slot values are not | O4, BIN | **Confirmed** 2026-10-05 |
| P42 | `SetBackground` raises a Lua error when an image file cannot be opened or has an unsupported format | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P43 | During start-up the client sets the C runtime locale for all categories to a name chosen by its game language: `English` for `Turbine.Language` 2 and for any value not listed, `english-uk` for 0x10000001, `french` for 0x10000003, `german` for 0x10000004. That locale governs the `isalnum` tests of P5 and P14 and the decimal separator of the plugin-data writer (§3.4, §10.4) | BIN (repair session 2026-10-05: the single call that sets all categories, and its four name choices keyed by the language value; the plugin-data number formatter passes no explicit locale, so it uses this global one). **Why M1–M3 were not enough:** locale use is code, not data; S3 only reports that comma-separated files exist | **Confirmed** 2026-10-05 (BIN only) |
| P44 | Under the standard Windows locale data these four names select code page 1252; `English` and `english-uk` use `.` as decimal separator, `french` and `german` use `,`. In code page 1252 `isalnum` is true for some units 0x80–0xFF (accented letters such as 0xE9) | General platform knowledge (Windows C runtime locale names and code-page 1252 character classes), consistent with S3's reports of comma files | **Reported** |
| P45 | The 141 named colours: each of R, G, B equals the colour's standard web byte value divided by 255 and rounded to 3 decimals, stored as a 32-bit float; A is 1, except `Transparent` (A 0, R/G/B 1) | BIN (repair session 2026-10-05: all 141 constructor calls of the colour registration read, 564 of 564 float constants match the rule); S3 (live dump: all 423 R/G/B values match the rule, 312 of them differ from byte/255) | **Confirmed** 2026-10-05 |
| P46 | UI behaviour from the official reference: controls are transparent until given a background image or colour; `SetZOrder` higher draws on top; `Window:Activate` brings a window to the front; opacity is used by windows to blend with the game; a control that the mouse does not see passes mouse input to what is below; `IsDisplayed` rule; `SetParent` moves a control; `ContextMenu:ShowMenu` opens at the mouse, is moved back on screen when its right or bottom edge would fall off, and first closes any other open context menu; only one context menu shows at a time and input outside it closes it; the `Turbine.UI.Lotro` skinned classes are the game's silver (and gold) styled window, button, check box, text box and scroll bar; `ItemControl`/`ItemInfoControl` show an item the user can use as from any game panel; `EntityControl` gives the game's entity selection and context menu; `EffectDisplay` shows an effect applied to an actor; `EquipmentSlot` lets the user equip items | S2 (class and member pages of `Control`, `Window`, `ContextMenu` and the `Turbine.UI.Lotro` classes), S3 stubs (`EntityControl` flags) | **Reported** |

> **Process note.** Static reading of client literals (O1) is superseded by BIN for every fact
> it supported; O1 rows are kept for history.

---

## 3. Runtime and sandbox

<a id="3-1"></a>
### 3.1 Lua version

- Lua **5.1** semantics (P1). The original uses the reference 5.1.4 interpreter unchanged:
  `number` is a C `double`, strings are 8-bit clean, `#` is the 5.1 border operator,
  `table.getn`, `table.setn`, `math.mod`, `string.gfind`, `unpack`, `setfenv`/`getfenv`,
  `newproxy`, `gcinfo` and `string.dump` behave as in 5.1.4. Real plugins call
  `table.getn`, `getfenv(0)` and `unpack` (O2), so these must exist.
- File loading uses the stock 5.1.4 file loader (BIN): a first line starting with `#` is
  skipped; a file whose first byte is the bytecode signature byte (ESC) is loaded as a
  precompiled chunk; a UTF-8 byte-order mark is **not** skipped, so a file that starts with
  one fails with a syntax error.
- A vendored PUC-Rio Lua 5.1.x is the compatible choice. LuaJIT differs in corner cases
  (e.g. `#` on tables with holes); not recommended.

<a id="3-2"></a>
### 3.2 Standard libraries and removed globals

| Library / global | Original client (BIN, P2, P3) | Tessera requirement |
|---|---|---|
| base (`print`, `pairs`, `ipairs`, `next`, `type`, `tostring`, `tonumber`, `select`, `unpack`, `pcall`, `xpcall`, `error`, `assert`, `rawget`, `rawset`, `rawequal`, `setmetatable`, `getmetatable`, `getfenv`, `setfenv`, `collectgarbage`, `gcinfo`, `newproxy`, `load`, `loadstring`, `_G`, `_VERSION`) | opened, unmodified | Provide, with the exceptions in the rows below. |
| `print` | stock: writes to the process's standard output, which the windowed client never shows | Provide. Route the text to the script log (§12) at debug level; never to chat. |
| `load`, `loadstring` | present (stock). They compile text **and precompiled binary** chunks; the new function's environment is the thread's current global table (§7.2 explains why that is the package namespace during a file's top-level run) | Provide for **text** chunks only. Reject a chunk whose first byte is ESC (0x1B) with an error: precompiled 5.1 bytecode is not verified and is a sandbox-escape risk. This is a deliberate deviation. |
| `coroutine` | opened as part of base | Provide. |
| `string` (incl. `string.dump`), `table`, `math` | opened, complete | Provide in full. |
| `debug` | opened, then removed from the globals and kept by the host for `Turbine.Engine.GetCallStack` (§12); plugins cannot reach it | Must be absent (`nil`). |
| `io`, `os`, `package` | not opened | Must be absent. |
| `dofile`, `loadfile`, `require`, `module` | set to nil | Must be `nil`. |
| `import` | added by the host (§7) | Provide. |
| `Turbine` and its core classes | present from apartment creation (§7.4) | Provide. |

<a id="3-3"></a>
### 3.3 Limits and scheduling

- **Memory.** The original's allocator counts the bytes of each apartment and refuses an
  allocation that would push the count over a per-apartment limit; a refused allocation
  becomes a Lua memory error and an out-of-memory report to the apartment's error listeners
  (§6.6). No code that sets a limit was found in this build, so the limit is presumably
  unset (unlimited) (P26). > **OPEN:** whether some path sets a limit, and its value.
  Tessera: a configurable per-apartment cap (default 64 MiB) — a deliberate safety
  deviation.
- **Run time.** The original installs no instruction-count or time hook: a plugin that
  loops forever freezes the client. Tessera adds an instruction budget per host-to-Lua call
  (default 500 million VM instructions, coroutines included) — a deliberate safety
  deviation; when it trips, the call is aborted with an error reported like any other.
- **Garbage collection.** Once per frame the original runs one incremental collection step
  in every apartment (BIN). Tessera may do the same or rely on the default collector.
- All plugin code runs on the main (UI) thread, between frames (Tessera architecture §18.1).
- Every host-to-Lua call is protected; an error in one callback never aborts the frame or
  affects other apartments (§6.6).

<a id="3-4"></a>
### 3.4 Numbers, strings and locale

- Lua numbers are doubles; DIDs (e.g. `0x41007DEA`) and enum values up to `0x7FFFFFFF` are
  represented exactly. Values the API receives as integers are truncated toward zero.
- **Text encoding (P24).** Every string the API receives from Lua is interpreted as UTF-8 and
  converted to the client's UTF-16 text; every string it returns is UTF-8. Lua code itself
  handles strings as bytes. Tessera: UTF-8 on both sides; invalid UTF-8 from a plugin is
  converted with replacement characters, never rejected.
- **Locale (P43, P44).** At start-up the original sets the C runtime locale, all categories,
  from its **game language** (the `Turbine.Language` value, §12), not from the user's
  operating-system settings: `English` for English (2) and for any unlisted value,
  `english-uk` for EnglishGB, `french` for French, `german` for German. Two plugin-visible
  behaviours depend on it: the character class of `import` names and plugin-data keys
  (§7.2, §10.2) and the decimal separator of numbers in `.plugindata` files.
- The plugin-data writer formats numbers with the C library's fixed-point conversion and no
  explicit locale, so it uses that game-language locale (§10.4). With the standard Windows
  locale data, English and EnglishGB clients write `.`, French and German clients write `,`
  (Reported, P44; S3 reports comma files). Tessera always writes `.` and reads both forms
  (§10.4). (`Turbine.Engine.GetLocale`, by contrast, reports the operating system's locale,
  §12.)

<a id="3-5"></a>
### 3.5 Anti-automation boundary

Plugins can display information and offer **user-clicked** shortcuts (Quickslot, §14.6) but
cannot themselves execute chat or slash commands, use skills or items, move the character, or
send keystrokes (P18, P22). Tessera keeps this boundary: no API converts a script call into a
game action without a user input event on a plugin control.

---

## 4. Files and folders

<a id="4-1"></a>
### 4.1 User folder

The original client's per-user folder is `Documents/The Lord of the Rings Online/` (S1, S5;
location rules: `install-layout.md` §8). Observed contents (O2): `UserPreferences.ini`,
`lotro.keymap`, `PluginData/`, `CEF/`; on 2026-10-03 also `Plugins/`, `Music/`, `ui/`. The
client does not create `Plugins/`; the user does (S5). The plugin manager keeps the path of
`Plugins/`; `PluginData/` is its sibling (BIN).

Tessera reads an existing original `Plugins/` folder **read-only** (Architecture §18.2) and
writes its own plugin data under its own `data/` directory (§10.7).

<a id="4-2"></a>
### 4.2 `Plugins/` layout

| Path pattern | Meaning |
|---|---|
| `Plugins/<Folder>/<Any>.plugin` | Descriptor (§5). The **only** place the original looks for descriptors (§4.3). Convention: `<Folder>` is the author's folder. |
| `Plugins/<Folder>/<Name>/…` | The plugin's Lua files and resources, addressed by dotted package paths. |
| `Plugins/<Folder>/<Any>.plugincompendium` (and `.skincompendium`, `.musiccompendium`) | Third-party manager metadata. The client never reads them (BIN: no such names in the client). Tessera ignores them. |
| `Plugins/Turbine/…`, `Plugins/TurbinePlugins/…` | The publisher's 2010 sample utility package and samples, installed by users when an older plugin needs them (S5). Ordinary user content, **not** part of the client; Tessera must not ship them. A `Plugins/Turbine/` folder is merged into the native `Turbine` package (§7.4). |

<a id="4-3"></a>
### 4.3 Discovery algorithm (P27)

Discovery runs at client start and on `/plugins refresh` or
`PluginManager.RefreshAvailablePlugins()` (S2, S5). It rebuilds the list of **available**
plugins from scratch:

1. List the entries directly inside `Plugins/`. Files there are ignored; only
   subdirectories are kept. `.` and `..` are skipped.
2. For each subdirectory, in the order the directory listing returns them, list the files
   directly inside it whose name matches `*.plugin` (case-insensitive). Deeper
   subdirectories are **not** searched.
3. Parse each such file (§5.2). A descriptor that fails is dropped silently (Tessera: log a
   warning).
4. Add a valid descriptor to the available map under its `Name`, compared
   case-insensitively. If the map already holds that name, the **new descriptor is dropped**:
   the first one found wins. On Windows the listing order is the file system's (on NTFS,
   case-insensitive name order). Tessera: process subdirectories and files in
   case-insensitive byte order of their names, so the first in that order wins, and log the
   duplicates.
5. Refresh does not unload plugins whose descriptor vanished; they just stop being listed
   (S2).

The available map is unordered: `PluginManager.GetAvailablePlugins()` returns its rows in an
unspecified order (§6.7).

<a id="4-4"></a>
### 4.4 Path rules

- All plugin paths (packages, images, descriptors' `Image`) are relative to `Plugins/`.
  Packages use `.` as the separator; resource paths may use `/` or `\` (S2, S4).
- Resolution is **case-insensitive** (P6). On case-sensitive file systems Tessera must match
  each component case-insensitively (pick the first match in byte order if several exist).
- No component may be empty, `.` or `..`; absolute paths, drive letters and NUL bytes are
  rejected. Resolution never leaves `Plugins/` (Architecture §18.2), including through
  symlinks. (The original does not enforce containment for image paths; Tessera does.)

---

## 5. The `.plugin` descriptor

<a id="5-1"></a>
### 5.1 Elements

An XML 1.0 document, parsed by a general-purpose DOM parser (BIN). Observed files carry
`<?xml version="1.0"?>` with no encoding declaration, tab or space indentation, and
multi-line text (O2).

| Element path | Occurs | Content | Meaning |
|---|---|---|---|
| `/Plugin` | 1 (root) | element | Root; its name must be exactly `Plugin`. |
| `Information` | exactly 1 in the document | element | Metadata block. |
| `Information/Name` | 1 | text | Plugin name: the key for `/plugins load`, `PluginManager.LoadPlugin`, `Plugins[...]` and the manager list. 1–120 characters after trimming; may contain spaces (O2). |
| `Information/Version` | 0..1 | text | Free-form; returned by `Plugin:GetVersion()`. If present: 1–120 characters after trimming. |
| `Information/Author` | 0..1 | text | Returned by `Plugin:GetAuthor()`. If present: non-empty after trimming; longer than 120 characters is cut to 120. |
| `Information/Description` | 0..1 | text | Shown in the manager; may span many lines (O2). If present: non-empty after trimming; cut to 2048 characters. |
| `Information/Image` | 0..1 | text | Path (relative to `Plugins/`) of a JPG or TGA icon, shown 32×32: larger images are cropped, smaller ones tiled (S3 xsd). If present: non-empty after trimming; cut to 120 characters. |
| `Package` | exactly 1 in the document | text | Dotted package path of the main chunk (§7). 1–120 characters after trimming. |
| `Configuration` | 0..1 in the document | element | Present only to choose an apartment. Its content is ignored. |
| `Configuration/@Apartment` | optional | attribute | Apartment name (§6.1). |

<a id="5-2"></a>
### 5.2 Parsing rules (P28)

"Trimming" removes leading and trailing characters from the set space, line feed, carriage
return, tab and form feed. Element names compare **case-sensitively**. The descriptor is
accepted only if every step passes:

1. The document parses as XML and its root element is named `Plugin`.
2. The whole document contains **exactly one** element named `Information` (counted at any
   depth below the root).
3. The direct children of `Information` are visited in document order. A child named
   `Name`, `Version`, `Author`, `Description` or `Image` is handled as below; any other
   child (unknown elements, comments, text) is ignored.
   - If that field already holds a value, the descriptor is **rejected** (a repeated field
     element is an error).
   - The field takes the element's text content, trimmed.
   - `Name`, `Version`: rejected if the result is longer than 120 characters or empty.
   - `Author`, `Image`: cut to the first 120 characters if longer; rejected if empty.
   - `Description`: cut to the first 2048 characters if longer; rejected if empty.
     Internal line breaks are kept.
4. After the loop, `Name` must be non-empty (a descriptor without `Name` is rejected).
5. The document contains **exactly one** element named `Package` (any depth). Its trimmed
   text must be 1–120 characters; otherwise the descriptor is rejected.
6. The document contains **at most one** element named `Configuration`. If it has an
   `Apartment` attribute, the apartment is that attribute's value **as written** (not
   trimmed). Otherwise the apartment is `Default`.

Encoding: the parser honours an encoding declaration and a byte-order mark; with neither, the
bytes are UTF-8 (rule of the XML parser used). Malformed XML rejects the descriptor.

Tessera additionally: reject files over 64 KiB, DTDs and external entities; bound element
counts and nesting depth.

<a id="5-3"></a>
### 5.3 Invariants

- An accepted descriptor has a `Name` of 1–120 characters with no surrounding whitespace.
- A document with a repeated `Name`, `Version`, `Author`, `Description` or `Image` child is
  rejected; so is one with zero or two `Information` or `Package` elements, or two
  `Configuration` elements.
- A `Description` of 3000 characters reads back as its first 2048.
- With no `Configuration`, or a `Configuration` without `Apartment`, the apartment is
  `Default`.

<a id="5-4"></a>
### 5.4 Plugin-manager options file (`PluginOptions.xml`, P23)

The in-game plugin manager stores its choices per account and world at
`PluginData/<account>/<world>/PluginOptions.xml` (O2). The writer (BIN) produces:

| Path | Kind | Meaning |
|---|---|---|
| `/PluginOptions` | root element | |
| `/PluginOptions/AutoloadOptions` | element | Autoload choices |
| `…/AutoloadOptions/Plugin` | element, repeated | One per plugin to autoload |
| `…/AutoloadOptions/Plugin/@Name` | attribute | Plugin `Name` |
| `…/AutoloadOptions/Plugin/@AllCharacters` | attribute, `true`/`false` (lower case) | Autoload for every character of the account on that world |
| `…/AutoloadOptions/Plugin/Character` | element, repeated | One per character for which the plugin autoloads |
| `…/AutoloadOptions/Plugin/Character/@Name` | attribute | Character name |
| `/PluginOptions/ScalingOptions` | element | UI-scaling choices |
| `…/ScalingOptions/Plugin` | element, repeated | One per plugin |
| `…/ScalingOptions/Plugin/@Name` | attribute | Plugin `Name` |
| `…/ScalingOptions/Plugin/@UseScaling` | attribute, `True`/`False` (capitalised) | Value returned by that plugin's `Plugin:GetUsesGlobalScaling()` |

The file is written pretty-printed with CRLF line ends and an XML declaration with
`encoding="UTF-8" standalone="no"`; observed files written by older clients contain only
`AutoloadOptions`, newer ones also an empty `ScalingOptions` (O2). Tessera keeps its own
autoload and scaling state; an optional importer may read this file read-only.

---

## 6. Apartments and plugin lifecycle

<a id="6-1"></a>
### 6.1 Apartments

- An **apartment** (script state) is one Lua state (P10). Every plugin runs in exactly one
  apartment.
- A descriptor with `Configuration/@Apartment = X` runs in apartment `X`; one without runs in
  `Default` (§5.2). All plugins naming the same apartment share its state, and therefore its
  globals, native singletons (§9.5) and class-level handlers.
- Apartment names compare **case-insensitively** (P11).
- An apartment is created on first use: a new Lua state with the libraries of §3.2, the
  removals of §3.2, `import`, and an automatic `import "Turbine"` (§7.4).

<a id="6-2"></a>
### 6.2 Loading a plugin (P9, P29)

Triggered by `/plugins load <Name>`, `PluginManager.LoadPlugin(name)`, the manager UI, or
autoload at character login.

`PluginManager.LoadPlugin(name)` only **queues** the request: it checks that an available
plugin of that name exists (case-insensitive) and that no plugin of that name is loaded in
any apartment, and if both hold, appends the request to a queue that the plugin manager
works through later (between frames). It returns nothing, raises nothing, and does nothing
if the checks fail. Processing a queued request (and the other triggers) does:

1. Get the plugin's apartment by name, creating it if needed (§6.1).
2. Create the `Turbine.Plugin` object carrying the descriptor's name, author and version, and
   the plugin's scaling choice (§5.4; `Plugin:GetUsesGlobalScaling()`).
3. If the global `Plugins` is not a table, set it to a new empty table. Set
   `Plugins[<Name>]` to the plugin object, and set the global `plugin` to it.
4. Mark the apartment as **loading a plugin** (this changes `PluginData.Load`, §10.1).
5. Run `import "<Package>"` (§7.2). Top-level code of the main file and everything it
   imports runs now.
6. If step 5 succeeded, fire `Load` on the plugin object (§9) with an empty argument table.
7. Clear the loading mark; set the global `plugin` to nil.
8. If step 5 failed, set `Plugins[<Name>]` to nil (the plugin is not loaded, and `Load` was
   not fired). The failure was reported by the import (§6.6).

So `plugin` is usable during the main package's top-level run and inside its `Load`
handlers, and is nil afterwards; code that needs its plugin object later must keep its own
reference (or use `Plugins[name]`).

> **OPEN:** the order in which autoloaded plugins load. Tessera: the order of its own
> autoload list.

<a id="6-3"></a>
### 6.3 Unloading an apartment (P10, P29)

Triggered by `/plugins unload [<apartment>]`, `PluginManager.UnloadScriptState(name)`, the
manager's unload action, logout, or client exit.

`PluginManager.UnloadScriptState(name)` acts **immediately**:

1. Find the apartment by name (case-insensitive). If there is none, do nothing.
2. If Lua code of that apartment is currently running — which is always the case when the
   call comes from a plugin in that same apartment — **refuse silently**: no error, no
   unload, no return value.
3. For each plugin of the apartment, in the apartment's plugin order (load order): fire
   `Unload` on its plugin object with an empty argument table, then set `Plugins[<Name>]` to
   nil.
4. Destroy the plugins' native objects: windows, controls, shell commands (§11), context
   menus, drag operations.
5. Close the Lua state. While the state is closing, `PluginData.Save` writes synchronously
   (§10.6).

Real plugins work around rule 2 with a helper plugin in another apartment (O2).
`/plugins unload` with no argument unloads every apartment (S5).

<a id="6-4"></a>
### 6.4 Chat command `/plugins`

| Sub-command | Effect | Source |
|---|---|---|
| `load <Name>` | §6.2 | S5 |
| `unload [<apartment>]` | §6.3 | S5, S3 xsd |
| `list` | Writes the available and loaded plugins to chat | S5 |
| `refresh` | Rediscovers (§4.3) | S5 |
| `manager` | Opens the plugin manager window | S5 |

The client's command words are localized text taken from the game data (BIN: none of them
are literals in the client), and the list output names a plugin in a non-`Default`
apartment together with its apartment. Tessera uses its own wording and accepts
`/plugins`; `/plugin` is not an alias. > **OPEN:** the localized aliases of the original
command.

<a id="6-5"></a>
### 6.5 Options panel

When the manager shows a plugin's details, it reads the field `GetOptionsPanel` of the
plugin object (normal indexing); if that is a Lua function, it calls it in protected mode
with the plugin object as its **only** argument and takes the first result. A result that is
not a `Turbine.UI.Control` (or a failed call) gives no panel; a control is re-parented into
the manager's options area and sized by it (S2, S3, BIN). The manager calls the function each
time it shows that plugin's details; it keeps no cache (BIN, inferred from the call sites).
Tessera: call once each time the options page for that plugin is opened.
`PluginManager.ShowOptions(pluginObject)` opens the manager on that page (S2, S3).

<a id="6-6"></a>
### 6.6 Error reporting (P25)

Every host-to-Lua call (file loads, event handlers, shell commands, plugin-data callbacks,
the options panel) is a protected call **without** a message handler. When it fails with a
runtime or syntax error, the error value converted to a string is passed to the apartment's
error listeners; the original shows it in the chat window (S5). The message is what Lua
produced: for a file, `<chunk name>:<line>: <message>`, where the chunk name is the file's
full path abbreviated by Lua's usual source-name rule; no traceback. Load failures (file
missing or unreadable, syntax error) are reported the same way.

The original also reports, through the same listeners and without raising an error:
allocation failures (§3.3) and a control resized beyond the graphics device's largest texture
size (§13.2).

Tessera format: plugin name, apartment, the Lua error message; a traceback may be added in
the log only. > **OPEN:** the original's chat colour and prefix.

<a id="6-7"></a>
### 6.7 Plugin manager rows

`PluginManager.GetAvailablePlugins()` and `GetLoadedPlugins()` return an array (keys 1…n, in
the unordered map order) of row tables with string fields (BIN):

| Field | Available rows | Loaded rows |
|---|---|---|
| `Name` | yes | yes |
| `Author` | yes | yes |
| `Version` | yes | yes |
| `Package` | yes | yes |
| `ScriptState` | yes (the apartment the plugin would load into) | yes |
| `Description` | yes | no |
| `Image` | yes | no |

`RefreshAvailablePlugins()` reruns discovery (§4.3).

---

## 7. `import` and package environments

<a id="7-1"></a>
### 7.1 Call form

`import` is a global function. Its first argument names the package; both `import "A.B"` and
`import(expr)` forms occur, and the name may be built at run time (O2).

- No argument: raises an error.
- A string is used as is; a number is converted to text with up to 14 significant digits
  (C `%.14g`); any other value becomes the empty name, which fails to resolve.
- On failure, raises an error naming the package. On success, returns **no values** (P30).

<a id="7-2"></a>
### 7.2 Resolution and execution (P5, P8, P30)

Given `N` = `C1.C2.….Ck`:

**Name check.** Components are separated by single dots; no leading or trailing dot, no
empty component. Each byte of the (UTF-8) name other than a separating dot must be `_` or
pass the C runtime's `isalnum` test in the game-language locale (§3.4); the original passes
each byte **sign-extended**, so bytes 0x80–0xFF reach `isalnum` as values −128 to −1. A name
that fails this check does not resolve from disk. ASCII letters, ASCII digits and `_` always
pass.

> **OPEN:** how the original's C runtime classifies the negative values that bytes 0x80–0xFF
> become here, and units above 0xFF in plugin-data keys (§10.2); the answer depends on the C
> runtime build the client runs against, not on the client. Tessera: reject both (package
> names and keys are ASCII letters, ASCII digits and `_` only), a stated deviation that
> affects only names outside ASCII.

**Resolution.** The apartment keeps a cache of resolved packages by name; a name resolved
before is not looked up again. Otherwise:

1. Ask the native registry for `N` (§7.4).
2. Look on disk (plugin apartments always do, even when step 1 found a native package):
   1. Let `D` be `Plugins/C1/C2/…/Ck` (case-insensitive, §4.4).
   2. If `D` exists **and is a directory**, the chunk is `D/__init__.lua` and the package is a
      *folder package*. A folder without `__init__.lua` does not fall back to `D.lua`.
   3. Otherwise the chunk is `D.lua` and the package is a *file package*.
   4. The chunk must exist and be readable; else the disk part is absent.
3. If both parts exist, the package is the **merge** of the two: installing it installs the
   native part, then runs the disk chunk (§7.4). If only one exists, that one is the package.
   If neither exists, `import` fails.

**Namespace.** For a folder package and for a native package, the namespace is `C1.….Ck`;
for a file package it is `C1.….C(k-1)` (for `k = 1`, the top-level namespace itself). The
namespace path is walked from the **top-level namespace** — the real global table of the
apartment, even when `import` is called from inside a file that is being imported — creating
an empty table for each missing component; an existing non-table component makes `import`
fail.

**Run once.** The apartment remembers which package names have been run. If `N` is
recorded, `import` returns at once (a second `import` of the same name runs nothing). If not:

1. Record `N` as run (so a package that imports itself does not recurse).
2. Set the field `_` of the namespace table to a new table `{ Name = N }` (the package name
   as passed). A file can read its own package name as `_.Name` during its top-level run.
3. Install the package (below). If installing fails, un-record `N`, so a later `import` of
   the same name tries again.

**Running a disk chunk** with namespace table `T`:

1. If `T` is a distinct table (not the global table): remember `T`'s current metatable, and
   set `T`'s metatable to a new table whose `__index` is the real global table.
2. Make `T` the thread's global table for the duration of steps 3–4 (so the chunk's
   environment, and every function the chunk creates, has `T` as its environment; and
   `getfenv(0)`, `loadstring` and `load` see `T` while the top-level code runs).
3. Load the file as a Lua chunk with the stock loader (§3.1) — Tessera: text only, chunk name
   = path relative to `Plugins/`, BOM accepted (Appendix B) — and call it with no
   arguments, in protected mode (§6.6).
4. Restore the real global table.
5. If `T` had a metatable before step 1 that was a table, restore it; otherwise **keep** the
   new metatable. Normally a namespace has no metatable, so after an import it permanently
   falls back to the globals through `__index`.

Effects implementers must reproduce:

- File `A/B/X.lua` assigning a bare global `X` creates `A.B.X`; another file of package
  `A.B` sees it as a bare name.
- Files in different packages can each define a bare global of the same name without
  clashing.
- A file sees names defined only in the real globals (`class`, `Turbine`, `plugin`) through
  the fallback, at load time **and** later (its functions keep `T` as environment, and `T`
  keeps the `__index` metatable). `A.B.Turbine` is therefore readable as a field.
- Assigning a bare name always writes into `T`, never into the real globals; code reaches the
  real globals through `_G`.

<a id="7-3"></a>
### 7.3 Re-import

Each package name runs at most once per apartment (§7.2 "Run once"). Distinct names that
resolve to the same file (e.g. different letter case) are distinct names and would run the
file again. Tessera: key the record by the name as passed.

<a id="7-4"></a>
### 7.4 Native packages

| Package | Contents |
|---|---|
| `Turbine` | Core classes and enums of §11, §12, plus `Object`, `Plugin`, `PluginData`, `PluginManager` |
| `Turbine.Gameplay` | §15 |
| `Turbine.Gameplay.Attributes` | Per-class and per-race attribute classes and stance enums |
| `Turbine.UI` | §13 |
| `Turbine.UI.Lotro` | §14 |

- Every new apartment runs `import "Turbine"` before any plugin code (P7). So `Turbine`
  (with `Turbine.Engine`, `Turbine.PluginData`, …) is always present; `Turbine.UI`,
  `Turbine.UI.Lotro`, `Turbine.Gameplay` and `Turbine.Gameplay.Attributes` appear when
  imported. Tessera may install all five at creation (plugins only observe the extra
  tables).
- Merge rule (§7.2): a user folder `Plugins/Turbine/` with an `__init__.lua` (the 2010
  sample utility package, S4) is run with the `Turbine` table as its namespace **when each
  apartment is created**, after the native classes are installed. Its sub-packages
  (`Turbine.Class`, `Turbine.UI.Extensions`, …) resolve from disk as usual.

---

## 8. Object model

<a id="8-1"></a>
### 8.1 Class tables

- Every API class (e.g. `Turbine.UI.Window`) is a plain Lua **table** in its namespace. It
  holds the class's own methods and reaches its base class's methods through its metatable's
  `__index` (`Window` → `Control` → `Object`).
- Instantiable classes have a `Constructor` function and are **callable**: `Class(...)`
  creates a new instance (S2, S4).
- Every class table and every enum table has an `IsA` function (S3, BIN).
- Static classes (`Engine`, `Shell`, `PluginData`, `PluginManager`, `Display`, `LotroUI`)
  are called with dot syntax and are never instantiated.
- **Arguments are positional from the first** (BIN, applies to statics and methods alike):
  argument 1 of a static function is its first parameter. A static called with colon syntax
  (`Turbine.Gameplay.LocalPlayer:GetInstance()`, seen in O2) receives the class table as its
  first argument; for zero-parameter statics this is harmless because extra arguments are
  ignored. Tessera follows the original; it does not strip a leading class table.

<a id="8-2"></a>
### 8.2 Instances

- An instance is a Lua **table** whose lookup chain ends at its class table. Plugins store
  their own fields on it and assign event handlers to it (§9).
- The native object behind an instance is bound to the table by the host, not through a field
  a plugin can see. Native methods find their native object from the table passed as
  `self`, whatever that table's metatable is (needed for §8.3).
- Calling a native method with a `self` that has no bound native object of a compatible
  class raises a Lua error (never a crash).
- Identity: a control keeps a reference to its own table, so `GetParent()`, `GetControls()`
  and list getters return the same tables that were created. Gameplay entities are kept in a
  per-apartment weak-valued cache, so asking twice for the same live entity yields the same
  table while plugin code holds a reference (BIN, structure observed; the cache key was not
  traced). Tessera: weak-valued map from native identity to table.
- `obj:IsA(C)` is true when `C` is the object's class or one of its bases, native or Lua.

<a id="8-3"></a>
### 8.3 Subclassing from Lua

Plugins subclass native classes with a small Lua helper, conventionally a global function
`class(base)` that each plugin (or the sample package) defines itself (S4, O2). Tessera does
**not** provide `class`. What the host must support so those helpers work:

1. A Lua class table `L` whose metatable `__index` is a native class table `N` sees all of
   `N`'s methods (and `N`'s bases') through ordinary indexing.
2. An instance is a fresh table whose metatable `__index` is `L` (owned by the helper).
3. The Lua constructor calls `N.Constructor(self, ...)` on that table. This binds a new native
   object of class `N` to `self` **without** replacing `self`'s metatable.
4. After that, native methods called on `self` work (§8.2), events fire on `self`, and
   methods the subclass defines (including overrides of native method names) take priority,
   because lookup reaches `L` before `N`.
5. Nested subclassing (Lua class of a Lua class of a native class) works the same way.

<a id="8-4"></a>
### 8.4 Enumerations

An enumeration is a table of name → number (plus `IsA`). Several names may share a value
(e.g. `BlendMode.None` and `BlendMode.Undefined` are both 8). Values are in Appendix A.

<a id="8-5"></a>
### 8.5 Conventions

- Getter/setter pairs: `GetX()`/`SetX(v)`; booleans use `IsX()`/`SetX(b)`.
- Functions that return several values return them as separate Lua values, in the order
  listed (e.g. `GetSize()` → width, height; `GetPosition()` → left, top;
  `GetMoneyComponents()` → copper, silver, gold).
- **List indices are 1-based** (P36): `ControlList.Get(i)`, `MenuItemList.Get(i)`,
  `TreeNodeList.Get(i)`, `ListBox:GetItem(i)` accept 1…count and raise an error for any
  other index. `IndexOf(x)` / `IndexOfItem(x)` return the 1-based index, or **−1** when `x`
  is not in the list. `ListBox:GetSelectedIndex()` returns 0 when nothing is selected.
  `Equipment:GetItem` takes a slot enum value instead (Appendix A).
- Colours are `Turbine.UI.Color` objects (§13.9); blend modes, alignments, fonts and so on
  are enum numbers.

---

## 9. Event model

<a id="9-1"></a>
### 9.1 Handlers are fields

An event `E` of an object `O` is not registered through a function: the plugin assigns a value
to the field `O.E` (S2, S4, O2). Forms seen in real code: a function assigned to a control's
`Click`; the class-level `Turbine.Plugin.Load`, which then applies to every plugin object of
the apartment; `function W:MouseDown(args)` on a Lua subclass.

<a id="9-2"></a>
### 9.2 Dispatch (P16)

When the host fires `E` on `O` (BIN):

1. Create a fresh argument table `A` and fill the event's fields (§9.3); events without
   fields get an empty table.
2. Read `O.E` with **normal** indexing (`__index` honoured, so class-level and subclass-level
   handlers count).
3. If the value is a **function**: call it as `handler(O, A)` in protected mode; an error is
   reported (§6.6).
4. If the value is a **table**:
   1. First call the table itself as `table(O, A)` in protected mode, **without reporting** a
      failure. If this succeeds (the table has a `__call` metamethod that ran without error),
      dispatch ends.
   2. Otherwise visit every entry of the table with `next` (array part first in index order,
      then the hash part in its internal order) and, for each **value that is a function**,
      call `value(O, A)` in protected mode, reporting errors and continuing with the next
      entry. Keys are ignored; non-function values (including callable tables) are skipped.
5. Any other value (nil, boolean, number, string, userdata): nothing happens.
6. Return values of handlers are ignored. Results are passed back only through fields of `A`
   (`Window.Closing`'s `Cancel`, §13.3) or objects in it (`DragDropInfo`, §13.10).

So a handler's first parameter is the sender (`O`); a handler written as a method
(`function obj:Click(args)`) receives `self = O`. Handlers declared with an extra leading
parameter on a dot-assigned class field (seen in O2) receive `O` and `A` in their first two
parameters; this must keep working. Note the edge case of step 4.1: a callable table whose
`__call` raises is retried as a plain array, silently.

<a id="9-3"></a>
### 9.3 Argument tables (P31)

Field names and the events that carry them (BIN; values observed in S3):

| Event(s) | Fields of `A` |
|---|---|
| `Turbine.Chat.Received` | `ChatType` (number, §11.3), `Message` (string), and `Sender` (string) when the line has a sender; system lines have no `Sender` |
| `Control.MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick` | `Button` (`MouseButton` value), `X`, `Y` (pixels, relative to the control) |
| `Control.MouseMove`, `MouseHover` | `X`, `Y` |
| `Control.MouseWheel` | `X`, `Y`, `Direction`: **+1** for the scroll-up input action (key 0x82), **−1** for scroll-down (key 0x10) (P32). The sign follows the bound action, not the wheel. In the owner's keymap (O5; not shown to be the shipped default) scroll-up is bound to the wheel's positive Z direction, which on Windows is rotation away from the user. |
| `Control.KeyDown`, `KeyUp` | `Action` (an input-action key, §14.4), `Alt`, `Control`, `Shift` (booleans) |
| `Control.DragDrop` (and the `Quickslot`, `EquipmentSlot` overrides) | `DragDropInfo` (object, §13.10), `X`, `Y` |
| `Window.Closing` | `Cancel` (boolean, initially false) |
| `ListBox.ItemAdded`, `ItemRemoved` | `Item` (the control) |
| `Backpack`, bank-type and wallet `ItemAdded`, `ItemRemoved` | `Index`, `Item` |
| `Backpack`, bank-type `ItemMoved` | `OldIndex`, `NewIndex`, `Item` |
| `Equipment.ItemEquipped`, `ItemUnequipped` | `Index` (slot), `Item` |
| `Bank.ChestNameChanged` | `Index` |
| `EffectList.EffectAdded`, `EffectRemoved` | `Index` |
| `ProfessionInfo.RecipeAdded`, `RecipeRemoved` | `Index` |
| `SkillList.SkillAdded` | `Index`, `Skill` |
| `SkillList.SkillRemoved` | `Index` |
| `Party.AssistTargetAdded` | `Player` (S3) |
| `Window.UIScaleChanged` | `Index` (the `UIScalingGroupType` that changed), `Value` (the new scale, number) |
| all other events (`Click`, `Load`, `Unload`, `Update`, every `…Changed` without a row above, `EffectsCleared`, `ItemsRefreshed`, …) | empty table |

> **OPEN:** whether `Party.MemberAdded`/`MemberRemoved` and `AssistTargetRemoved` carry a
> field (no field name is wired to them in the binary; Tessera: empty table).

<a id="9-4"></a>
### 9.4 Per-frame update

`Control:SetWantsUpdates(true)` subscribes the control to the UI's per-frame tick; the host
then fires `Update` on it once per frame with an empty `A` (S2, BIN). It stays on until set
false or the control is destroyed. > **OPEN:** whether hidden controls still receive
`Update`. Recommended: yes (plugins use it for timers on hidden controls).

<a id="9-5"></a>
### 9.5 Shared singletons

Objects such as `LocalPlayer.GetInstance()`, `Turbine.Chat` and `Turbine.Plugin` (class level)
are shared by every plugin **in the same apartment**; one plugin replacing a handler removes
another's (S2). Different apartments get different tables. Plugins cope by using handler
tables (§9.2 step 4) or private apartments; the host must support both.

<a id="9-6"></a>
### 9.6 Re-entrancy and ordering

- Events are delivered on the main thread. Events directly caused by a script call (e.g.
  `Close()` firing `Closing`) are delivered synchronously before the call returns.
- `Turbine.Chat.Received` is not re-entered: a chat line produced while a `Received`
  dispatch is running (for example by `Shell.WriteLine` in a handler) is not delivered to
  that apartment's `Received` handlers (BIN).
- A handler that destroys or re-parents the sender must not crash the host; remaining
  handlers of that firing still run.
- > **OPEN:** which setters fire their own change events (e.g. `SetText` → `TextChanged`).

---

## 10. Plugin data (`Turbine.PluginData`)

<a id="10-1"></a>
### 10.1 Calls (P33)

`scope` is a `Turbine.DataScope` value: `Account` = 1, `Server` = 2, `Character` = 3. All
validation errors below are raised as Lua errors in the caller.

**`PluginData.Load(scope, key [, handler])`**

1. If the calling apartment is **loading a plugin** (§6.2 steps 4–7: the main package's
   top-level code and its `Load` handlers), the call is **synchronous**:
   1. Validate the key (§10.2) and scope; build the path (§10.3).
   2. If no file exists, the result is nil. Otherwise load the file as a Lua chunk (stock
      loader) and call it with no arguments, keeping one result. A load failure raises an
      error; a run-time failure raises an error.
   3. If `handler` is a Lua function, call `handler(result)` now (protected; errors
      reported). A non-function `handler` is ignored.
   4. Return `result`.
2. Otherwise the call is **asynchronous**:
   1. `handler` must be a Lua function; nil or any other value raises an error.
   2. Validate, build the path and read the file exactly as in step 1 (the data is read
      **now**).
   3. Queue `handler(result)` to be called later (between frames). Return nothing.

So the common pattern `local v = Turbine.PluginData.Load(scope, key)` works only during
loading; at any other time it raises an error.

**`PluginData.Save(scope, key, data [, onDone])`**

1. Validate the key and scope; build the path (this fails, with an error, when the account,
   world or character name is not available — e.g. character scope at character selection).
2. Serialise `data` now (§10.4–§10.5); failure raises an error.
3. If the apartment's state is closing (§6.3 step 5), write the file now; otherwise queue the
   write for later.
4. When the write has been attempted, if `onDone` is a Lua function, call it as
   `onDone(success, message)`: `success` is true and `message` is the empty string when the
   file was written; otherwise `success` is false and `message` describes the failure. A
   non-function `onDone` is ignored. `Save` returns nothing.

<a id="10-2"></a>
### 10.2 Validation (P14)

- `scope` must be 1, 2 or 3.
- `key` is converted to text (a number becomes its string form; nil or another non-string
  becomes empty). It must have **1 to 64** UTF-16 units. Each unit must be `_` or pass the
  C runtime's `isalnum` test, called with the unit's value, in the game-language locale
  (§3.4, P14, P43). Anything else is an error.
- What passes therefore depends on that locale. ASCII letters, ASCII digits and `_` pass in
  every locale. Units 0x80–0xFF pass when the locale's code page classifies them as letters
  or digits; under code page 1252 (all four game languages, P44) that includes accented
  letters such as U+00E9 (Reported). Units above 0xFF: OPEN (§7.2).
- **Tessera deviation:** accept only ASCII letters, ASCII digits and `_` (so `"é"` is
  rejected in Tessera although the original, under its code-page-1252 locales, accepts it,
  P44). Keys are file names; a fixed ASCII rule keeps them portable across file systems and
  independent of locale. A plugin that uses a non-ASCII key gets an error in Tessera.
- Character scope needs a character in the world; with none, building the path fails and
  the call raises an error.

<a id="10-3"></a>
### 10.3 Storage location (original layout)

`PluginData/` is a sibling of `Plugins/` in the user folder (§4.1):

| Scope | Folder | Source |
|---|---|---|
| Account | `PluginData/<account>/AllServers/` | BIN, S1, S3 |
| Server | `PluginData/<account>/<world>/AllCharacters/` | BIN, O2 |
| Character | `PluginData/<account>/<world>/<character>/` | BIN, S1 |

File name: `<key>.plugindata`. `<account>` is the account name, `<world>` the world's display
name as shown in the client (it may contain spaces and brackets, O2), `<character>` the
character name. Missing folders are created when writing.

<a id="10-4"></a>
### 10.4 File format (P15)

The file is a Lua chunk that returns the saved value. The writer (BIN) emits, with `⏎` meaning
CR LF (the file is written in text mode, which turns each line feed into CR LF):

1. The text `return ` (with the trailing space), then the value.
2. A **table** value is written as: `⏎`, then *d* tabs, then `{⏎`, where *d* is the current
   depth (0 for the top-level table); the depth increases by one; then its entries; then
   `⏎`, *d* tabs (after the depth decreased back) and `}`.
3. Each **entry** is: *d* tabs (the inner depth), `[`, the key written as a scalar, `]`,
   ` = `, then the value. Entries are separated by `,⏎`; there is no comma after the last.
   Entries appear in the order `next` visits the table.
4. **Scalars**: nil as `nil`; booleans as `true`/`false`; numbers with the C fixed-point
   format with six decimals (`%lf`: 9 → `9.000000`, 0.5 → `0.500000`, 1e-7 → `0.000000`,
   1e20 → `100000000000000000000.000000`), using the decimal separator of the game-language
   locale (§3.4); strings
   in double quotes, with line feed written as `\n`, carriage return as `\r`, `"` as `\"`,
   `\` as `\\`, and every other byte written unchanged. So numeric keys look like
   `[1.000000]`.
5. An empty table is written as `{`, an empty line, `}` (because of rules 2 and 3).

Example (synthetic): saving a table with key `"a"` → true and key 1 → the string `x"y`, at the
top level, produces these lines (⇥ = tab, entry order as `next` returns it):

| Line | Content |
|---|---|
| 1 | `return ` |
| 2 | `{` |
| 3 | `⇥[1.000000] = "x\"y",` |
| 4 | `⇥["a"] = true` |
| 5 | `}` |

Consequences: fractional parts beyond six decimals are lost on save; values with magnitude
below 0.0000005 become 0; NaN and infinities are written as the C library's text for them,
which reads back as nil (an unknown global name).

**Reading (Tessera).** Parse the file as a restricted Lua expression after an optional
`return`, **without executing code**:

| Grammar element | Allowed |
|---|---|
| value | `nil`, `true`, `false`, number, string, table |
| number | Lua 5.1 numeral (decimal, exponent, hex `0x…`), optional leading `-`; inside `[…]` and after `=` also accept a comma decimal separator (files written under a comma locale) |
| string | double- or single-quoted with Lua 5.1 escapes; long brackets `[[…]]`, `[=[…]=]` |
| table | `{` fields `}`; field = `[` key `] =` value, or `name =` value, or value (array item); separators `,` or `;`, trailing separator allowed |
| key | number or string (not nil, not NaN, not table) |

Accept LF or CRLF and any whitespace. A file that does not fit the grammar loads as nil and is
logged (the original would run it as code; Tessera never does). Limits: file size cap
(default 16 MiB), nesting depth cap (default 64), entry count cap.

**Writing (Tessera).** Emit the layout above, except: numbers with integral values as
integers, other numbers with 17 significant digits, always `.` as decimal separator; keys
sorted (numbers ascending first, then strings bytewise) for stable diffs. Files Tessera
writes are therefore readable by the original.

<a id="10-5"></a>
### 10.5 What can be saved

Booleans, numbers, strings, nil and tables of those (BIN):

- A **top-level** value that is a function, userdata, thread or light userdata makes `Save`
  fail (error raised, nothing written).
- Inside a table, an entry whose key **or** value is a function, userdata, thread or light
  userdata is **skipped** silently.
- A table that has already been written once in the same `Save` — whether through a cycle or
  simply shared by two entries — is written as `nil` the second time (the entry stays, with
  value nil, and disappears on reload).
- Metatables are not saved.

<a id="10-6"></a>
### 10.6 Timing

- Data is read (`Load`) and serialised (`Save`) at call time; the file write and the
  asynchronous `Load` handler run later, between frames (§10.1). Writes to the same key keep
  call order.
- During state close (§6.3 step 5) `Save` writes immediately.
- The original truncates and rewrites the file in place (not atomic).
- Tessera: write on a worker thread if desired, but preserve per-key order, make a later
  `Load` of the same key see the last `Save`, flush everything before an apartment closes
  (data saved in `Unload` handlers must reach disk), and write atomically (temporary file in
  the same folder, then rename).

> **OPEN:** whether the original still calls an `onDone` callback for a write that was
> queued before its apartment closed. Tessera: drop such callbacks after the apartment has
> closed.

<a id="10-7"></a>
### 10.7 Tessera storage

Tessera stores plugin data under its own `data/plugindata/` directory using the same
three-level layout (account, world, character names sanitized for the file system), never in
the original client's folders. A one-time read-only import of an original `PluginData/` tree
is allowed.

---

## 11. Shell and chat (`Turbine.Shell`, `Turbine.ShellCommand`, `Turbine.Chat`)

<a id="11-1"></a>
### 11.1 `Turbine.Shell` (static, P34)

| Function | Parameters → result | Behaviour |
|---|---|---|
| `AddCommand(names, cmd)` | string, ShellCommand → number | Splits `names` at every `;`, `:` and `,`; trims each piece of spaces, tabs, CR, LF and form feeds; ignores empty pieces. Registers `cmd` under each piece not already a command name — built-in or plugin, compared case-insensitively. Returns how many names were registered (0 if `cmd` is nil). Names are typed in chat as `/name`. |
| `RemoveCommand(cmd)` | ShellCommand → number | Unregisters every name `cmd` holds; returns how many were removed (0 if `cmd` was not registered). |
| `IsCommand(name)` | string → boolean | True if `name` is any known command name, built-in or plugin, case-insensitive. Plugins probe localized built-in names to detect the client language (S5), so built-in names must be answered. |
| `GetCommands()` | → array of strings | Every command name (built-in and plugin, aliases included), keys 1…n, unspecified order. |
| `WriteLine(text)` | string → nothing | Adds a line to the chat window with chat text type 8 (a system-log type that has no name in `Turbine.ChatType`). |

Registrations of an apartment are removed when it unloads (§6.3).

> **OPEN:** whether `WriteLine` lines reach `Turbine.Chat.Received` handlers (type 8 is not in
> the `ChatType` enum). Tessera: deliver them with `ChatType = 8`, no `Sender`, on the next
> pump, never inside `WriteLine` and never re-entrantly (§9.6).

<a id="11-2"></a>
### 11.2 `Turbine.ShellCommand`

A class (instantiate with `Turbine.ShellCommand()` or subclass it). The host looks up three
methods on the command object, so plugins override them:

| Method | Called as | Default (not overridden) | Expected result |
|---|---|---|---|
| `Execute` | `cmd:Execute(command, arguments)` (P17) | does nothing | ignored |
| `GetHelp` | `cmd:GetHelp()` | returns `""` | string (may contain newlines), shown by help |
| `GetShortHelp` | `cmd:GetShortHelp()` | returns `""` | one-line string, shown in command lists |

`command` is the command word as typed (without `/`), `arguments` the text after it with the
separating space removed; **both are always strings** — `arguments` is `""` when nothing
follows (BIN). > **OPEN:** whether surrounding whitespace in `arguments` is trimmed and
whether `command` keeps the user's letter case. Tessera: keep case, remove exactly one
separating whitespace character, keep trailing whitespace.

<a id="11-3"></a>
### 11.3 `Turbine.Chat`

Static event source: `Turbine.Chat.Received = handler` receives the chat lines the client
shows, with the fields of §9.3. `ChatType` is the client's chat text type; the named values
are in Appendix A (they are keys of the chat-text-type enum 0x230002A8, P41), other values
can occur. Message text may contain the client's markup (§13.4).
> **OPEN:** the full grammar of markup in `Message` (item and other links).

---

## 12. Engine (`Turbine.Engine`, static)

| Function | Result |
|---|---|
| `GetDate()` | Table, local time of the current moment (P35): `Year` (four-digit), `Month` 1–12, `Day` 1–31, `Hour` 0–23, `Minute` 0–59, `Second` 0–60, `DayOfWeek` **1 = Sunday … 7 = Saturday**, `DayOfYear` **1-based** (1–366), `IsDST` boolean. |
| `GetLocalTime()` | Seconds since 1970-01-01 00:00 UTC, as a **32-bit signed** integer (wraps after 2038-01-19). |
| `GetGameTime()` | The client's frame clock in seconds (fractional); constant within one frame; only differences matter. |
| `GetLanguage()` | The client's language, a `Turbine.Language` value (Appendix A). |
| `GetLocale()` | The two-letter ISO 639 language code of the **operating system's** current locale (e.g. `en`, `de`), not of the game client; `""` if unavailable. |
| `GetScriptVersion()` | Two numbers: **13, 0** (in that order) in this build. |
| `GetCallStack(...)` | Calls the host's private `debug.traceback` with up to three of the given arguments and returns its result; equivalent to `debug.traceback(...)`. |
| `ScriptLog(message)` | Appends `message` to a log shared by all scripts: the client's script-log channel, written to a file named `Script.log`. Tessera: the `logs/` directory and a `tracing` target. |

`GetLanguage` and `GetScriptVersion` are added to `Turbine.Engine` by the game layer; they are
present in every apartment (BIN).

---

## 13. User interface (`Turbine.UI`)

<a id="13-1"></a>
### 13.1 Concepts

Sources are given per item; "Reported" items come from the official reference (S2, P46) and
have not been observed in this build.

- **Coordinates:** pixels; a control's position is relative to its parent's top-left; a
  `Window`'s position is relative to the screen (`Display`) (S2, Reported).
- **Tree:** a control is shown only inside a `Window` chain (S2, Reported); top-level windows
  have no parent. `SetParent(nil)` detaches; parenting a control to itself is an error (BIN).
  Setting a new parent removes the control from the old one (S2, Reported).
- **Visibility:** new controls and windows start hidden; plugins call `SetVisible(true)`
  (S2/S4, Reported). `IsDisplayed()` = own visibility and every ancestor's (S2, Reported).
- **Transparency:** a bare `Control` draws nothing until it gets a background image
  (`SetBackground`) or colour (`SetBackColor`) (S2, Reported).
- **Stacking:** among siblings a higher `ZOrder` draws above a lower one (S2, Reported);
  `Window:Activate()` brings a window to the front (S2, Reported).
- **Opacity:** 0.0 (fully transparent) to 1.0 (opaque). S2 says only some controls use it,
  namely windows, to decide how they blend with the game (Reported). > **OPEN:** whether a
  window's opacity also applies to its child controls, and whether opacity set on a
  non-window control has any visible effect. Tessera: a window's opacity multiplies the
  alpha of its whole content; on other controls it is stored and returned only.
- **Mouse:** a control with `SetMouseVisible(false)` does not take mouse input; the controls
  underneath handle it (S2, Reported).
- **Keys:** only controls with `SetWantsKeyEvents(true)` receive `KeyDown`/`KeyUp` (S2,
  Reported). Key events carry key-binding **actions**, not raw keys (BIN, §9.3, §14.4).
- **Focus:** `Focus()` asks for focus within the control's window (S2, Reported);
  `HasFocus()`, events `FocusGained`/`FocusLost` (BIN registration). Tessera: a text box
  takes focus when clicked (a Tessera rule, not a sourced fact).
- **Drag and drop:** `SetAllowDrop(true)` makes the control handle drag-and-drop events
  `DragEnter`/`DragLeave`/`DragDrop` (S2, Reported); drags of game shortcuts produce
  `Turbine.UI.Lotro.DragDropInfo` (§14.6).
- **Mouse buttons:** button events convert the client's button to `MouseButton` (Left = 1,
  Right = 2, Middle = 4) (BIN).

<a id="13-2"></a>
### 13.2 `Control` (base of all controls)

| Group | Members (instance methods unless marked) |
|---|---|
| Geometry | `GetLeft/SetLeft`, `GetTop/SetTop`, `GetPosition()`→l,t / `SetPosition(l,t)`, `GetWidth/SetWidth`, `GetHeight/SetHeight`, `GetSize()`→w,h / `SetSize(w,h)`, `PointToClient(x,y)`→x,y, `PointToScreen(x,y)`→x,y, `GetMousePosition()`→x,y (relative to the control) |
| Size limits | `Get/SetMinimumWidth`, `Get/SetMinimumHeight`, `GetMinimumSize()`→w,h / `SetMinimumSize(w,h)`, same for `Maximum` — members of **`Control`** in this build (earlier dumps list them on `Window`; windows inherit them, so existing plugins are unaffected) |
| Edge attachment | `AttachEdges(a, b, c, d)` takes four `EdgeAttachmentType` numbers, one per edge; `GetEdgeAttachments()` returns four numbers. > **OPEN:** which edge each position means, and the layout rules of each attachment type. |
| Tree | `GetParent/SetParent(control or nil)`, `GetControls()`→ControlList (live view of children), `GetZOrder/SetZOrder(n)` |
| State | `IsVisible/SetVisible(b)`, `IsDisplayed()`, `IsEnabled/SetEnabled(b)`, `GetOpacity/SetOpacity(0..1)`, `IsMouseVisible/SetMouseVisible(b)`, `GetWantsKeyEvents/SetWantsKeyEvents(b)`, `GetWantsUpdates/SetWantsUpdates(b)`, `GetAllowDrop/SetAllowDrop(b)`, `Focus()`, `HasFocus()` |
| Look | `GetBackColor/SetBackColor(Color)`, `GetBackColorBlendMode/SetBackColorBlendMode(BlendMode)`, `GetBackground/SetBackground(path, DID or Graphic)` (§13.8), `GetBlendMode/SetBlendMode(BlendMode)` (applies to the background image), `GetStretchMode/SetStretchMode(n)` (§13.8). An invalid blend-mode number raises an error. |
| Static | `Control.IsAltKeyDown()`, `Control.IsControlKeyDown()`, `Control.IsShiftKeyDown()` |
| Events | `MouseEnter`, `MouseLeave`, `MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick`, `MouseHover`, `MouseMove`, `MouseWheel`, `KeyDown`, `KeyUp`, `FocusGained`, `FocusLost`, `EnabledChanged`, `VisibleChanged`, `PositionChanged`, `SizeChanged`, `DragStart`, `DragEnter`, `DragLeave`, `DragDrop`, `Update` |

**Size warning.** When a control becomes wider or taller than the graphics device's largest
texture dimension, the original reports a warning (with the place in the Lua source where the
control was created) to the error listeners; it does not clamp the size and raises no error
(BIN). Tessera: same, using the renderer's maximum texture size.

<a id="13-3"></a>
### 13.3 `Window` (extends `Control`)

Top-level container with no frame of its own (S2). Members: `GetText/SetText(title)`,
`Activate()` (brings the window to the front, S2), `Close()`, `GetRotation()`/`SetRotation(t)`, and the
global-scaling members `RegisterForGlobalScaling()`, `UnregisterForGlobalScaling()`,
`SetUIScalingGroup(group)`/`GetUIScalingGroup()` (a `UIScalingGroupType`),
`SetScale(n)`/`GetScale()`, `SetScalingOriginPoint(…)`/`GetScalingOriginPoint()` (BIN).
Events: `Activated`, `Deactivated`, `Closing`, `Closed`, `UIScaleChanged`.

- **Rotation:** `SetRotation` takes a table with number fields `x`, `y`, `z` (lower case);
  `GetRotation` returns the last value set (BIN). > **OPEN:** units and pivot.
- **Scaling:** `SetScale` with a value that is not greater than 0 is an error. A window
  registered for global scaling receives `UIScaleChanged` (§9.3) when the scale of its group
  or of the `Global` group (60) changes. > **OPEN:** the units of `SetScale`/`GetScale`, the
  form of the scaling origin point, and how a window's own scale combines with its group's.

`Close()` algorithm (S2): fire `Closing` with `Cancel = false`; if a handler set `Cancel`
true, stop; otherwise make the window not visible and fire `Closed`. Closing does not destroy
the window. > **OPEN:** whether Escape closes plugin windows automatically. Recommended: no;
plugins handle the `Escape` action themselves.

<a id="13-4"></a>
### 13.4 `Label` (extends `ScrollableControl`)

Text display. Members: `GetText/SetText(s)`, `AppendText(s)`, `InsertText(s)` (at the
selection), `GetTextLength()`, `GetFont/SetFont(Font)` (§14.1), `GetFontStyle/SetFontStyle`
(`FontStyle.None`/`Outline`), `GetForeColor/SetForeColor(Color)`,
`GetOutlineColor/SetOutlineColor(Color)`, `GetTextAlignment/SetTextAlignment(ContentAlignment)`,
`IsMultiline/SetMultiline(b)`, `IsMarkupEnabled/SetMarkupEnabled(b)`,
`IsSelectable/SetSelectable(b)`, selection: `GetSelectedText`, `SetSelectedText`,
`GetSelectionStart/SetSelectionStart`, `GetSelectionLength/SetSelectionLength`,
`SetSelection(start, length)`, `SelectAll()`, `DeselectAll()`.

**Markup** (when enabled; P38). Tags are written `<name>` and closed `</name>`; names compare
case-insensitively:

| Tag | Effect |
|---|---|
| `b` | bold on / off |
| `i` | italic on / off |
| `u` | underline on / off |
| `p` | paragraph break (opening tag only) |
| `li` | list item on / off (a new item starts at the opening tag) |
| `rgb=…` | text colour. With `#` the value is 6 hex digits `RRGGBB`; without `#` the value is read as a C integer literal (decimal, `0x` hex, or octal with a leading 0), so `<rgb=FF0000>` is **not** a colour. The colour is always opaque. Colours nest: `</rgb>` returns to the colour before the matching opening tag. A value that is not entirely a number makes the tag ignored. |

Any other tag is not markup. Tessera renders unrecognised and malformed tags literally.

> **OPEN:** whether selection offsets are 0- or 1-based; default font, colour and alignment.

<a id="13-5"></a>
### 13.5 `TextBox` (extends `Label`)

Editable text. Members: `IsReadOnly/SetReadOnly(b)`. Events: `TextChanged`,
`EnabledChanged`. Typing in a focused text box must not trigger game key bindings.

<a id="13-6"></a>
### 13.6 `Button` and `CheckBox` (extend `Label`)

`Button`: events `Click` (empty argument table, BIN), `EnabledChanged`. Unskinned;
`Turbine.UI.Lotro.Button` is the skinned variant. > **OPEN:** which mouse buttons activate
`Click`. Tessera: the left button, on release inside the button.
`CheckBox`: `IsChecked/SetChecked(b)`, `GetCheckAlignment/SetCheckAlignment(ContentAlignment)`;
events `CheckedChanged`, `EnabledChanged`.

<a id="13-7"></a>
### 13.7 Scrolling and lists

- `ScrollBar` (extends `Control`): `Get/SetMinimum`, `Get/SetMaximum`, `Get/SetValue`,
  `Get/SetSmallChange`, `Get/SetLargeChange`, `Get/SetOrientation(Orientation)`, and
  replaceable parts `Get/SetDecrementButton`, `Get/SetIncrementButton` (Buttons),
  `Get/SetThumbButton` (Control). While a scroll bar is bound to a `ScrollableControl`,
  getting or setting its minimum, maximum, value or large change, and setting its small
  change, raise errors (BIN). Events `ValueChanged`, `OrientationChanged`.
- `ScrollableControl` (extends `Control`): `Get/SetHorizontalScrollBar(ScrollBar or nil)`,
  `Get/SetVerticalScrollBar(…)` — attaches an external scroll bar that the control then
  drives.
- `ListBox` (extends `ScrollableControl`): items are **controls**. `AddItem(c)`,
  `InsertItem(i,c)`, `RemoveItem(c)`, `RemoveItemAt(i)`, `ClearItems()`, `GetItem(i)`,
  `SetItem(i,c)`, `GetItemCount()`, `IndexOfItem(c)`, `ContainsItem(c)`, `GetItemAt(x,y)`,
  `EnsureVisible(i)`, selection `Get/SetSelectedIndex`, `Get/SetSelectedItem`; layout
  `Get/SetOrientation`, `Get/SetMaxItemsPerLine`, `Get/SetMaxRows`, `Get/SetMaxColumns`,
  `Get/SetReverseFill`, `Get/SetFlippedLayout`, `Get/SetWrapIndentAmount`,
  `Get/SetIgnoreCellAlignment`; `Sort(compare)` where `compare(a, b)` returns true when `a`
  sorts first (an inconsistent comparator must not crash Tessera). Index rules: §8.5. Events
  `ItemAdded`, `ItemRemoved`, `SelectedIndexChanged`.
- `ControlList` (and `MenuItemList`, `TreeNodeList`): `Add`, `Insert(i,x)` (not on
  `TreeNodeList`), `Remove(x)`, `RemoveAt(i)`, `Clear`, `Get(i)`, `Set(i,x)` (not on
  `TreeNodeList`), `GetCount`, `IndexOf(x)`, `Contains(x)`. Index rules: §8.5.
- `TreeView` / `TreeNode` / `TreeNodeList` (tier 2): nodes are controls; `GetNodes()`,
  `GetSelectedNode/SetSelectedNode`, `ExpandAll/CollapseAll`, `Get/SetIndentationWidth`,
  `Get/SetFilter(f)` (`f(node)` → keep?), `Get/SetSortMethod(f)`, `Refresh()`,
  `GetItemAt(x,y)`; node: `GetChildNodes()`, `GetParentNode()`, `Expand`, `Collapse`,
  `ExpandAll`, `CollapseAll`, `IsExpanded/SetExpanded`, `IsSelected`; events
  `SelectedNodeChanged` (view), `SelectedChanged` (node). (`TreeNode` has no
  `EnsureVisible` in this build.)

<a id="13-8"></a>
### 13.8 Images (P42)

- `SetBackground(v)` (BIN):
  - `v` a **number**: the integer part is a DID of a client image (render surface
    `0x41xxxxxx`, P13; `records.md` §5). The DID's existence is not checked at call time.
  - `v` a **string**: a path relative to `Plugins/` (§4.4). If the file cannot be opened,
    the call **raises an error**. JPG and TGA are the documented formats.
  - `v` a `Turbine.UI.Graphic`: that image.
  - nil or the empty string: no background.
  - other types: no background, no error.
  - If the loaded image has an unsupported format, the call **raises an error**.
- `Turbine.UI.Graphic`: `Graphic()` empty, `Graphic(did)` or `Graphic(path)` (S2);
  accepted wherever a background is.
- Stretch modes (S3 stubs, community-observed, Reported): 0 = no scaling, image cropped or
  tiled to the control and clipped by the parent; 1 = scale the image by the ratio between
  the control's size when the mode was set and its current size; 2 = resize the control to
  the image's size when set, stretch afterwards; 3 = like 0 but not clipped by the parent;
  4 = like 1 but the control ignores the mouse. > **OPEN:** confirm by observation; mode 0
  reportedly also resets alpha.

<a id="13-9"></a>
### 13.9 `Color` (P37)

Constructor (BIN):

| Call | Result (A, R, G, B) |
|---|---|
| `Color(r, g, b)` | (1, r, g, b) |
| `Color(a, r, g, b)` | (a, r, g, b) |
| any other number of arguments, including `Color()` | (0, 0, 0, 0) — transparent black |

Non-number arguments count as 0. Components are stored as single-precision floats, so reading
a field back gives the value rounded to single precision (e.g. 0.1 reads back as
0.100000001490116…). Fields `R`, `G`, `B`, `A` are readable and writable; values are not
clamped.

**Named colours (P45).** 141 named colours are fields of the class table (Appendix A.5), e.g.
`Turbine.UI.Color.Red`: the standard web named colours plus `Transparent`. Appendix A.5 gives
each as the web byte triple `#RRGGBB`, but the original's components are **not** byte/255.
Each component is computed as:

1. Take the byte (0–255) and divide by 255.
2. Round to **3 decimal places** (no value falls exactly half-way, so the rounding mode does
   not matter).
3. Store as a 32-bit float (so reading it back in Lua gives that float's exact value).

A is 1 for every named colour except `Transparent`, which is A 0 with R, G and B 1.
Worked examples: AliceBlue `#F0F8FF` gives R = 240/255 = 0.94118 → **0.941** (reads back as
0.94099998474121…), G = 248/255 = 0.97255 → **0.973**, B = 1; Gray `#808080` gives 0.50196 →
**0.502** for each of R, G, B; Black gives 0, 0, 0. For 312 of the 423 R/G/B components the
result differs from byte/255 rounded to single precision, so an implementation that stores
byte/255 fails equality tests that plugins may make against `Color` fields.

<a id="13-10"></a>
### 13.10 Other `Turbine.UI` classes

- `Display` (static): `GetWidth()`, `GetHeight()`, `GetSize()`→w,h of the UI area in pixels;
  `GetMouseX()`, `GetMouseY()`, `GetMousePosition()`→x,y in screen pixels;
  `GetUIGroupScale(group)` → the current scale of a `UIScalingGroupType` (1.0 by default).
  > **OPEN:** its exact semantics for `Global` and for groups that were never scaled.
- `DragDropInfo`: `IsSuccessful()`, `SetSuccessful(b)` (a drop handler marks the drop as
  accepted).
- `ContextMenu` (tier 2): `GetItems()`→MenuItemList, `ShowMenu()`, `ShowMenuAt(x, y)`,
  `Close()`. Behaviour (S2, Reported, P46): `ShowMenu` opens the menu at the current mouse
  position; if its right or bottom edge would fall off the screen, the menu is moved so that
  it is fully on screen; a context menu that is already open is closed first, so at most one
  shows at a time; any user input outside the open menu closes it.
  `MenuItem(text [, enabled [, checked]])`: `Get/SetText`, `IsEnabled/SetEnabled`,
  `IsChecked/SetChecked`, `GetItems()` (sub-menu), event `Click`.
- Enums `BlendMode`, `ContentAlignment`, `EdgeAttachmentType`, `FontStyle`, `MouseButton`,
  `Orientation`, `HorizontalLayout`, `VerticalLayout`: Appendix A.

---

## 14. Game-styled UI (`Turbine.UI.Lotro`)

<a id="14-1"></a>
### 14.1 Fonts

`Turbine.UI.Lotro.Font` values are DIDs of font records (P12). `Label:SetFont` takes one of
them. 48 fonts plus `Undefined = 0` (Appendix A.2). For the font record format see
[`fonts.md`](fonts.md).

Font record payload prefix (record type `0x42`, Confirmed 2026-10-03 for all 48 records; the
payload is what follows the 8-byte block header described in the dat container spec):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 LE | The record's own DID (`0x42xxxxxx`) |
| 4 | 4 | u32 LE | Nominal point size; equals the trailing number of the enum name (e.g. 20 for `TrajanPro20`) |
| 8 | … | — | Glyph metrics and texture references: `fonts.md` |

Location: 45 font records in `client_local_English.dat`, 3 (`0x42000027`, `0x42000028`,
`0x420000FD`) in `client_general.dat`. Records were stored uncompressed (directory flags 2).

<a id="14-2"></a>
### 14.2 Skinned controls

Class and base names are BIN (registration); the look descriptions are S2 (Reported, P46).

| Class | Base | Look / extra members |
|---|---|---|
| `Window` | `Turbine.UI.Window` | The game's standard silver-styled window; `SetText` sets the caption text (S2); `IsResizable/SetResizable(b)` (S2, BIN). |
| `GoldWindow` | `Turbine.UI.Lotro.Window` | The gold-styled variant (S2). |
| `Button` | `Turbine.UI.Button` | The game's standard silver-styled button (S2). |
| `GoldButton` | `Turbine.UI.Lotro.Button` | The gold-styled variant (S2). |
| `CheckBox` | `Turbine.UI.CheckBox` | The game's standard check box (S2). |
| `TextBox` | `Turbine.UI.TextBox` | The game's standard text box (S2). |
| `ScrollBar` | `Turbine.UI.ScrollBar` | The game's standard scroll bar (S2); event `OrientationChanged` (S2, BIN). |

Art comes from the client's UI skin data (see `ui-layout.md`). Tessera may draw them with its
own neutral frame art of the same metrics. > **OPEN:** frame border widths and title-bar
height (plugins lay out content assuming them), and which frame parts exist (for example
whether the skinned window has a close button); no source describes them.

<a id="14-3"></a>
### 14.3 Native panel control (`LotroUI`)

`LotroUI.IsEnabled(element)`, `LotroUI.SetEnabled(element, b)`, `LotroUI.Reset()` (re-enable
all). `element` is a `LotroUIElement` value: `PluginsManager`, `Party`, `Backpack1`–`Backpack6`,
`Vitals`, `Target` (Appendix A). Disabling hides the native panel so a plugin can replace it
(S4). Tessera maps these to its own native panels; until they exist, store and report the
flag.

<a id="14-4"></a>
### 14.4 Key-binding actions (`Action`, P20)

270 members (Appendix A.3): `Undefined = 0`, three small keys (`CameraInstantMouseLook = 92`,
`CaptureScreenshot = 116`, `Escape = 145`) and the rest in `0x10000002…0x10000246`. Every
non-zero value is a key of the client's **input-action enum mapper**, the one the EMAPPER DID
map labels for input actions (0x230000AF in this install, with its base chain; read the label
at run time, `records.md` §8.2) — Confirmed for all 269 (O4). The plugin API names are its
own; they are not the data's symbolic names.

`KeyDown`/`KeyUp` deliver the input action bound to the pressed key, which may be any key of
that mapper, including ones without an API name (S3 observed `0x100001D8`). The mouse wheel
does not produce key events; it produces `MouseWheel` with the scroll-up (0x82) or
scroll-down (0x10) action translated to `Direction` (§9.3). Tessera's input layer maps its
own actions onto these keys (Architecture §16, `keymap.md`).

<a id="14-5"></a>
### 14.5 Game-object controls (tier 2)

Class, base and member names are BIN (registration); the behaviour notes are S2 (Reported,
P46) unless marked.

| Class | Base | Members |
|---|---|---|
| `BaseItemControl` | `Control` | — |
| `ItemControl` | `BaseItemControl` | `GetItem/SetItem(Item)`: displays an item the player has; the user can interact with it and use it as from any game panel (S2) |
| `ItemInfoControl` | `BaseItemControl` | `GetItemInfo/SetItemInfo(ItemInfo)`, `GetQuantity/SetQuantity(n)`: displays item information (S2) |
| `EquipmentSlot` | `BaseItemControl` | `GetEquipmentSlot/SetEquipmentSlot(slot)`; event `DragDrop`: a slot through which the user can equip items (S2) |
| `EntityControl` | `Control` | `GetEntity/SetEntity`, `IsSelectionEnabled/SetSelectionEnabled(b)`, `IsContextMenuEnabled/SetContextMenuEnabled(b)`: gives the game's standard entity selection and the entity's context menu, each switchable by its flag (S2 class summary, S3 stubs) |
| `EffectDisplay` | `Control` | `GetEffect/SetEffect(Effect)`: displays an indicator for an effect applied to an actor (S2) |

> **OPEN:** the interaction details of these controls: which mouse gesture selects the entity
> or opens its context menu on an `EntityControl`, what an `EffectDisplay` draws besides the
> effect's icon (for example a remaining-time display), and which tooltip and click
> behaviour `ItemControl` and `ItemInfoControl` show. Tessera (tier 2): left click selects,
> right click opens the context menu, `EffectDisplay` draws the effect's icon only.

<a id="14-6"></a>
### 14.6 Quickslots and shortcuts (tier 2)

- `Shortcut()`, `Shortcut(item)`, `Shortcut(type, data)`: `GetType/SetType(ShortcutType)`,
  `GetData/SetData(string)`, `GetItem()`. `ShortcutType`: `Item = 2`, `Alias = 4`,
  `Emote = 5`, `Skill = 6`, `Pet = 7`, `Hobby = 9` (values are keys of the data enum
  0x23000137, P41).
- **Data string per type** (P40), as the client produces it when it converts a native
  shortcut (e.g. on a drag):

  | Type | `GetData()` text |
  |---|---|
  | `Item` (2) | `0x` + 16 upper-case hex digits (64-bit item instance id) + `,0x` + 8 upper-case hex digits (a 32-bit companion value of the item reference) |
  | `Alias` (4) | the alias text itself |
  | `Emote` (5), `Skill` (6), `Hobby` (9) | `0x` + 8 upper-case hex digits (the 32-bit id) |
  | `Pet` (7) | `0x` + 8 upper-case hex digits (the 32-bit pet command id) |
  | 3 | not a valid shortcut; the type reads back as 0 |
  | 8, 10 | the data is left unchanged |

  > **OPEN:** what the 32-bit companion value of an item shortcut is, and how `SetData`
  > parses each form.
- `Quickslot` (extends `Control`): `GetShortcut/SetShortcut`, `IsUseOnRightClick/
  SetUseOnRightClick(b)`; events `ShortcutChanged`, `DragDrop`. An invalid shortcut passed to
  `SetShortcut` raises an error (BIN). A **user click** on a quickslot activates its
  shortcut; this is the only way a plugin control causes a game action (§3.5).
- `Turbine.UI.Lotro.DragDropInfo` (extends `Turbine.UI.DragDropInfo`): `GetShortcut()`.

---

## 15. Gameplay views (`Turbine.Gameplay`)

Read-only views of client-side game state, fed in Tessera by the session layer. Every getter
returns the current value; every `XChanged` event fires after the value changed. The full
member list is in §16; the main entry points:

| Entry point | Notes |
|---|---|
| `LocalPlayer.GetInstance()` | The local character (singleton per apartment, §9.5). Extends `Player` → `Actor` → `Entity`. Real plugins call it at load time, so it must exist as soon as a character is in the world. |
| `Entity` | `GetName()`, `IsLocalPlayer()`, event `NameChanged`. |
| `Actor` | Morale and power (current, max, base max, temporary, max temporary), `GetLevel()`, `GetTarget()` (an `Entity` or nil), `GetEffects()` (`EffectList`); one `…Changed` event per value. |
| `Player` | `GetClass()`, `GetRace()`, `GetAlignment()` (enum numbers), `GetParty()`, `GetPet()`, `GetReadyState()`, `IsLinkDead()`, `IsVoiceEnabled()`, `IsVoiceActive()`; events `PartyChanged`, `RaidChanged`, `PetChanged`, `ReadyStateChanged`, … |
| `LocalPlayer` | `GetAttributes()` (`FreePeopleAttributes` or `MonsterPlayerAttributes`), `GetClassAttributes()`, `GetRaceAttributes()`, `GetBackpack()`, `GetEquipment()`, `GetVault()`, `GetSharedStorage()`, `GetWallet()`, `GetMount()`, `GetTrainedSkills()`, `GetUntrainedSkills()`, `IsInCombat()`; events `InCombatChanged`, `MountChanged`. |
| `Item` / `ItemInfo` | Names, description, category (`ItemCategory`), quality, durability, wear state, quantity and stack limits, and image DIDs (icon, background, underlay, shadow, quality overlay) that plugins pass to `SetBackground`. |
| `FreePeopleAttributes` | Stats (might, agility, vitality, will, fate and base values), mitigations, resistances, critical/defence ratings, regeneration, healing, `GetMoney()` (copper), `GetMoneyComponents()`, `GetProfessionInfo(profession)`, `GetVocation()`. |

**`GetMoneyComponents()`** (P39) returns three numbers, in this order: copper = money mod
100; silver = (money div 100) mod 1000; gold = money div 100000. Example: 123456789 copper →
89, 567, 1234. (S2's order "gold, silver, copper" is wrong.)

**Enum values and game data** (P41, O4). These API enums use the game's own enum keys, so
Tessera can obtain their meaning from the data (read the enum mappers by their EMAPPER labels
at run time, `records.md` §7–§8):

Columns: *names* = members of the API enum (Appendix A), *non-zero values* = distinct
non-zero values among them (aliases such as two names for one value count once), *keys* =
how many of those non-zero values are keys of the mapper with its base chain. Value 0 is
left out of the count; it is also a key of every mapper listed (O4).

| API enum | Data enum mapper (DID in this install; base) | Names | Non-zero values | Keys |
|---|---|---|---|---|
| `Class` | character classes (0x2300000E) | 22 | 21 | 21 |
| `Race` | species (0x2300000B) | 9 | 8 | 8 |
| `Language` | languages (0x230000B1; base 0x23000186) | 5 | 4 | 4 |
| `ShortcutType` | shortcut types (0x23000137) | 8 | 6 | 6 |
| `ItemQuality` | item qualities (0x23000067) | 6 | 5 | 5 |
| `ItemCategory` | item classes (0x23000036) | 187 | 186 | 186 |
| `CraftTier` | craft tiers (0x23000063) | 10 | 8 | 8 |
| `ChatType` | chat text types (0x230002A8; base 0x230002A9) | 34 | 33 | 33 |
| `Action` | input actions (0x230000AF; base 0x23000176) | 270 | 269 | 269 (§14.4) |

`Equipment` slot values do not match a data enum (they are the API's own numbering);
`EffectCategory` matches only partly. > **OPEN:** whether `Alignment`, `Profession`,
`Vocation`, `ItemDurability`, `ItemWearState`, `ReadyState`, `SkillType`, `EffectCategory`
and the stance enums follow a data enum.

Differences of this build from S2 and from the S3 dump: see §16.

---

## 16. API inventory (client build 4906.0071.1794.4035)

Columns: namespace; class; base class; kind (`class` = instantiable or returned objects,
`static` = function table, `enum`); own members (inherited ones omitted); events (own);
implementation tier (§17). The member lists are the registration of this build (BIN, P19);
rows marked "changed" differ from S2 or from the 2026-03 S3 dump.

| Namespace | Class | Base | Kind | Own members | Events | Tier |
|---|---|---|---|---|---|---|
| `Turbine` | **Chat** | Object | class | (none of its own) | Received | 1 |
| `Turbine` | **ChatType** | — | enum | 34 values (Appendix A) | — | 1 |
| `Turbine` | **DataScope** | — | enum | 3 values | — | 1 |
| `Turbine` | **Engine** | — | static | static: GetCallStack, GetDate, GetGameTime, GetLanguage, GetLocalTime, GetLocale, GetScriptVersion, ScriptLog | — | 1 |
| `Turbine.Gameplay` | **ActiveSkill** | Skill | class | GetBaseCooldown, GetCooldown, GetResetTime, IsUsable | ResetTimeChanged | 3 |
| `Turbine.Gameplay` | **Actor** | Entity | class | GetBaseMaxMorale, GetBaseMaxPower, GetEffects, GetLevel, GetMaxMorale, GetMaxPower, GetMaxTemporaryMorale, GetMaxTemporaryPower, GetMorale, GetPower, GetTarget, GetTemporaryMorale, GetTemporaryPower | BaseMaxMoraleChanged, BaseMaxPowerChanged, LevelChanged, MaxMoraleChanged, MaxPowerChanged, MaxTemporaryMoraleChanged, MaxTemporaryPowerChanged, MoraleChanged, PowerChanged, TargetChanged, TemporaryMoraleChanged, TemporaryPowerChanged | 1 |
| `Turbine.Gameplay` | **Alignment** | — | enum | 3 values | — | 1 |
| `Turbine.Gameplay` | **Attributes** | PropertyHandler | class | (none of its own) | — | 2 |
| `Turbine.Gameplay.Attributes` | **BeorningAttributes** | ClassAttributes | class | GetWrath, IsInBearForm | FormChanged, WrathChanged | 3 |
| `Turbine.Gameplay.Attributes` | **BlackArrowAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **BrawlerAttributes** | ClassAttributes | class | GetStance | — | 3 |
| `Turbine.Gameplay.Attributes` | **BrawlerStance** | — | enum | 1 value | — | 3 |
| `Turbine.Gameplay.Attributes` | **BurglarAttributes** (changed) | ClassAttributes | class | GetStance (IsCriticalTier1Available and IsCriticalTier2Available are **not** registered) | IsCriticalTier1Changed, IsCriticalTier2Changed, StanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **BurglarStance** | — | enum | 1 value | — | 3 |
| `Turbine.Gameplay.Attributes` | **CaptainAttributes** | ClassAttributes | class | IsInEnemyDefeatResponse, IsInFellowDefeatResponse, IsReadiedTier1Available, IsReadiedTier2Available | IsInEnemyDefeatResponseChanged, IsInFellowDefeatResponseChanged, IsReadiedTier1Changed, IsReadiedTier2Changed | 3 |
| `Turbine.Gameplay.Attributes` | **ChampionAttributes** | ClassAttributes | class | GetFervor, GetStance | FervorChanged, StanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **ChampionStance** | — | enum | 1 value | — | 3 |
| `Turbine.Gameplay.Attributes` | **ChickenAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **DefilerAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **DwarfAttributes** | Attributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **ElfAttributes** | Attributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **FreePeopleAttributes** | Attributes | class | GetAgility, GetArmor, GetBaseAgility, GetBaseArmor, GetBaseFate, GetBaseMight, GetBaseResistance, GetBaseVitality, GetBaseWill, GetDestinyPoints, GetDiseaseResistance, GetFate, GetFearResistance, GetMight, GetMoney, GetMoneyComponents, GetPoisonResistance, GetProfessionInfo, GetVitality, GetVocation, GetWill, GetWoundResistance, GetAcidMitigation, GetBaseCriticalHitAvoidance, GetBaseCriticalHitChance, GetBlock, GetCommonMitigation, GetEvade, GetFinesse, GetFireMitigation, GetFrostMitigation, GetInCombatMoraleRegeneration, GetInCombatPowerRegeneration, GetIncomingHealing, GetLightningMitigation, GetMeleeCriticalHitAvoidance, GetMeleeCriticalHitChance, GetMeleeDamage, GetMeleeDefence, GetOutOfCombatMoraleRegeneration, GetOutOfCombatPowerRegeneration, GetOutgoingHealing, GetParry, GetPhysicalMitigation, GetRangeCriticalHitAvoidance, GetRangeCriticalHitChance, GetRangeDamage, GetRangeDefence, GetShadowMitigation, GetTacticalCriticalHitAvoidance, GetTacticalCriticalHitChance, GetTacticalDamage, GetTacticalDefence, GetTacticalMitigation, CanBlock, CanEvade, CanParry | — | 2 |
| `Turbine.Gameplay.Attributes` | **GuardianAttributes** | ClassAttributes | class | GetStance, IsBlockTier1Available, IsBlockTier2Available, IsBlockTier3Available, IsParryTier1Available, IsParryTier2Available, IsParryTier3Available | IsBlockTier1AvailableChanged, IsBlockTier2AvailableChanged, IsBlockTier3AvailableChanged, IsParryTier1AvailableChanged, IsParryTier2AvailableChanged, IsParryTier3AvailableChanged, StanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **GuardianStance** | — | enum | 1 value | — | 3 |
| `Turbine.Gameplay.Attributes` | **HighElfAttributes** | Attributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **HobbitAttributes** | Attributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **HunterAttributes** | ClassAttributes | class | GetFocus, GetStance | FocusChanged, StanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **HunterStance** | — | enum | 4 values | — | 3 |
| `Turbine.Gameplay.Attributes` | **LoreMasterAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **ManAttributes** | Attributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **MarinerAttributes** | ClassAttributes | class | GetBalance | BalanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **MinstrelAttributes** | ClassAttributes | class | GetStance, IsSerenadeTier1Available, IsSerenadeTier2Available, IsSerenadeTier3Available | IsSerenadeTier1Changed, IsSerenadeTier2Changed, IsSerenadeTier3Changed, StanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **MinstrelStance** | — | enum | 3 values | — | 3 |
| `Turbine.Gameplay.Attributes` | **MonsterPlayerAttributes** | Attributes | class | GetDestinyPoints, GetMoney, GetMoneyComponents | — | 2 |
| `Turbine.Gameplay.Attributes` | **RangerAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **ReaverAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **RiverHobbitAttributes** | Attributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **RuneKeeperAttributes** | ClassAttributes | class | GetAttunement, IsCharged | AttunementChanged, IsChargedChanged | 3 |
| `Turbine.Gameplay.Attributes` | **StalkerAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | ~~StoutAxeAttributes~~ (changed) | — | — | the name exists in the client but the class is **not registered**; `Turbine.Gameplay.Attributes.StoutAxeAttributes` is nil | — | — |
| `Turbine.Gameplay.Attributes` | **TrollAttributes** | ClassAttributes | class | GetRampage | RampageChanged | 3 |
| `Turbine.Gameplay.Attributes` | **WarLeaderAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay.Attributes` | **WardenAttributes** | ClassAttributes | class | GetGambit, GetGambitCount, GetMaxGambitCount, GetStance, GetTrainedGambits, GetUntrainedGambits | GambitChanged, MaxGambitCountChanged, StanceChanged | 3 |
| `Turbine.Gameplay.Attributes` | **WardenGambit** | — | enum | 4 values | — | 3 |
| `Turbine.Gameplay.Attributes` | **WardenStance** | — | enum | 3 values | — | 3 |
| `Turbine.Gameplay.Attributes` | **WeaverAttributes** | ClassAttributes | class | (none of its own) | — | 3 |
| `Turbine.Gameplay` | **Backpack** | Object | class | GetItem, GetSize, PerformItemDrop, PerformShortcutDrop | ItemAdded, ItemMoved, ItemRemoved, SizeChanged | 2 |
| `Turbine.Gameplay` | **Bank** | Object | class | GetCapacity, GetChestCount, GetChestName, GetCount, GetItem, IsAvailable, PerformShortcutDrop | CapacityChanged, ChestNameChanged, CountChanged, IsAvailableChanged, ItemAdded, ItemMoved, ItemRemoved, ItemsRefreshed | 3 |
| `Turbine.Gameplay` | **BankItem** | Item | class | GetQuantity, GetWearState, GetBoundOwner, GetChest | ChestChanged, QuantityChanged | 3 |
| `Turbine.Gameplay` | **BasicMount** | Mount | class | GetMaxMorale, GetMorale | MaxMoraleChanged, MoraleChanged | 3 |
| `Turbine.Gameplay` | **Class** | — | enum | 22 values | — | 1 |
| `Turbine.Gameplay` | **ClassAttributes** | Attributes | class | (none of its own) | — | 2 |
| `Turbine.Gameplay` | **CombatMount** | Mount | class | GetAgility, GetArmor, GetBaseAgility, GetBaseArmor, GetBaseMaxMorale, GetBaseMaxPower, GetBaseStrength, GetFury, GetMaxFury, GetMaxMorale, GetMaxPower, GetMaxTemporaryMorale, GetMaxTemporaryPower, GetMorale, GetPower, GetStrength, GetTemporaryMorale, GetTemporaryPower, GetEvasion, GetTurnRate | BaseMaxMoraleChanged, BaseMaxPowerChanged, FuryChanged, MaxMoraleChanged, MaxPowerChanged, MaxTemporaryMoraleChanged, MaxTemporaryPowerChanged, MoraleChanged, PowerChanged, TemporaryMoraleChanged, TemporaryPowerChanged | 3 |
| `Turbine.Gameplay` | **CraftTier** | — | enum | 10 values | — | 3 |
| `Turbine.Gameplay` | **Effect** (changed) | Object | class | GetCategory, GetDescription, GetDuration, GetID, GetIcon, GetName, GetStartTime, IsCurable, IsDebuff (no IsDispellable) | CategoryChanged, DescriptionChanged, DurationChanged, IconChanged, IsCurableChanged, IsDebuffChanged, IsDispellableChanged, NameChanged, StartTimeChanged | 2 |
| `Turbine.Gameplay` | **EffectCategory** | — | enum | 11 values | — | 2 |
| `Turbine.Gameplay` | **EffectList** | Object | class | Contains, Get, GetCount, IndexOf | EffectAdded, EffectRemoved, EffectsCleared | 2 |
| `Turbine.Gameplay` | **Entity** | PropertyHandler | class | GetName, IsLocalPlayer | NameChanged | 1 |
| `Turbine.Gameplay` | **EntityReference** | Object | class | (none of its own) | — | 3 |
| `Turbine.Gameplay` | **Equipment** | Object | class / enum | the table holds the slot enumeration (Appendix A); the object from `LocalPlayer:GetEquipment()` has GetItem(slot), GetSize | ItemEquipped, ItemUnequipped | 2 |
| `Turbine.Gameplay` | **GambitSkill** | Skill | class | (none of its own) | — | 3 |
| `Turbine.Gameplay` | **GambitSkillInfo** | SkillInfo | class | GetGambit, GetGambitCount | — | 3 |
| `Turbine.Gameplay` | **Item** | Entity | class | GetItemInfo, GetBackgroundImageID, GetCategory, GetDurability, GetIconImageID, GetMaxStackSize, GetQuality, GetQualityImageID, GetQuantity, GetShadowImageID, GetUnderlayImageID, GetWearState, IsMagic | QuantityChanged | 2 |
| `Turbine.Gameplay` | **ItemCategory** | — | enum | 187 values | — | 2 |
| `Turbine.Gameplay` | **ItemDurability** | — | enum | 8 values | — | 2 |
| `Turbine.Gameplay` | **ItemInfo** | Object | class | GetBackgroundImageID, GetCategory, GetDescription, GetDurability, GetIconImageID, GetMaxQuantity, GetMaxStackSize, GetName, GetNameWithQuantity, GetQuality, GetQualityImageID, GetShadowImageID, GetUnderlayImageID, IsMagic, IsUnique | — | 2 |
| `Turbine.Gameplay` | **ItemQuality** | — | enum | 6 values | — | 2 |
| `Turbine.Gameplay` | **ItemWearState** | — | enum | 5 values | — | 2 |
| `Turbine.Gameplay` | **LocalPlayer** | Player | class | static: GetInstance; GetAttributes, GetBackpack, GetClassAttributes, GetEquipment, GetMount, GetRaceAttributes, GetSharedStorage, GetTrainedSkills, GetUntrainedSkills, GetVault, GetWallet, IsInCombat | InCombatChanged, MountChanged | 1 |
| `Turbine.Gameplay` | **Mount** | Entity | class | GetName | — | 3 |
| `Turbine.Gameplay` | **Party** | Object | class | GetAssistTarget, GetAssistTargetCount, GetLeader, GetMember, GetMemberCount, IsAssistTarget, IsMember | AssistTargetAdded, AssistTargetRemoved, LeaderChanged, MemberAdded, MemberRemoved | 2 |
| `Turbine.Gameplay` | **PartyMember** | Player | class | (inherits; S2 lists GetClass, GetLevel, GetName, GetParty, GetRaid, GetReadyState, IsLinkDead, IsVoiceEnabled) | — | 2 |
| `Turbine.Gameplay` | **Pet** | Actor | class | (none of its own) | — | 2 |
| `Turbine.Gameplay` | **Player** | Actor | class | GetAlignment, GetClass, GetParty, GetPet, GetRace, GetReadyState, IsLinkDead, IsVoiceActive, IsVoiceEnabled | IsLinkDeadChanged, IsVoiceActiveChanged, IsVoiceEnabledChanged, PartyChanged, PetChanged, RaidChanged, ReadyStateChanged | 1 |
| `Turbine.Gameplay` | **Profession** | — | enum | 11 values | — | 3 |
| `Turbine.Gameplay` | **ProfessionInfo** | Object | class | GetMasteryExperience, GetMasteryExperienceTarget, GetMasteryLevel, GetMasteryTitle, GetName, GetProficiencyExperience, GetProficiencyExperienceTarget, GetProficiencyLevel, GetProficiencyTitle, GetRecipe, GetRecipeCount | MasteryExperienceChanged, MasteryExperienceTargetChanged, MasteryLevelChanged, MasteryTitleChanged, ProficiencyExperienceChanged, ProficiencyExperienceTargetChanged, ProficiencyLevelChanged, ProficiencyTitleChanged, RecipeAdded, RecipeRemoved | 3 |
| `Turbine.Gameplay` | **PropertyHandler** | EntityReference | class | (none of its own) | — | 3 |
| `Turbine.Gameplay` | **Race** | — | enum | 9 values | — | 1 |
| `Turbine.Gameplay` | **Raid** | Object | class | (none of its own) | — | 2 |
| `Turbine.Gameplay` | **ReadyState** | — | enum | 3 values | — | 2 |
| `Turbine.Gameplay` | **Recipe** | Object | class | GetBaseCriticalSuccessChance, GetCategory, GetCategoryName, GetCooldown, GetCriticalResultItemInfo, GetCriticalResultItemQuantity, GetExperienceReward, GetIngredient, GetIngredientCount, GetIngredientPack, GetName, GetOptionalIngredient, GetOptionalIngredientCount, GetProfession, GetResultItemInfo, GetResultItemQuantity, GetTier, HasCriticalResultItem, IsSingleUse | — | 3 |
| `Turbine.Gameplay` | **RecipeIngredient** | Object | class | GetCriticalChanceBonus, GetItemInfo, GetRequiredQuantity | — | 3 |
| `Turbine.Gameplay` | **SharedStorage** | Bank | class | (none of its own) | — | 3 |
| `Turbine.Gameplay` | **Skill** | Object | class | GetSkillInfo | — | 3 |
| `Turbine.Gameplay` | **SkillInfo** | Object | class | GetDescription, GetIconImageID, GetName, GetType | — | 3 |
| `Turbine.Gameplay` | **SkillList** | Object | class | GetCount, GetItem | SkillAdded, SkillRemoved | 3 |
| `Turbine.Gameplay` | **SkillType** | — | enum | 3 values | — | 3 |
| `Turbine.Gameplay` | **UntrainedSkill** | Skill | class | GetPrice, GetRequiredLevel, GetRequiredRank | — | 3 |
| `Turbine.Gameplay` | **Vault** | Bank | class | (none of its own) | — | 3 |
| `Turbine.Gameplay` | **Vocation** | — | enum | 8 values | — | 3 |
| `Turbine.Gameplay` | **Wallet** | Object | class | GetItem, GetSize | ItemAdded, ItemRemoved | 2 |
| `Turbine.Gameplay` | **WalletItem** | Object | class | GetDescription, GetImage, GetMaxQuantity, GetName, GetQuantity, GetSmallImage, IsAccountItem | MaxQuantityChanged, QuantityChanged | 2 |
| `Turbine` | **Language** | — | enum | 5 values | — | 1 |
| `Turbine` | **Object** | — | class | IsA | — | 1 |
| `Turbine` | **Plugin** (changed) | Object | class | GetAuthor, GetName, GetVersion, GetUsesGlobalScaling | Load, Unload; callback GetOptionsPanel | 1 |
| `Turbine` | **PluginData** | — | static | static: Load, Save | — | 1 |
| `Turbine` | **PluginManager** (changed) | — | static | static: GetAvailablePlugins, GetLoadedPlugins, LoadPlugin, RefreshAvailablePlugins, UnloadScriptState, ShowOptions (added to the same table by the game layer; there is no separate `LotroPluginManager` table) | — | 1 |
| `Turbine` | **Shell** | — | static | static: AddCommand, GetCommands, IsCommand, RemoveCommand, WriteLine | — | 1 |
| `Turbine` | **ShellCommand** | Object | class | Execute, GetHelp, GetShortHelp | — | 1 |
| `Turbine.UI` | **BlendMode** | — | enum | 9 values | — | 1 |
| `Turbine.UI` | **Button** | Label | class | (none of its own) | Click, EnabledChanged | 1 |
| `Turbine.UI` | **CheckBox** | Label | class | GetCheckAlignment, IsChecked, SetCheckAlignment, SetChecked | CheckedChanged, EnabledChanged | 1 |
| `Turbine.UI` | **Color** | Object | class | fields A, R, G, B; 141 named colours | — | 1 |
| `Turbine.UI` | ~~ComboBox~~ (changed) | — | — | the name exists in the client but the class is **not registered** | — | — |
| `Turbine.UI` | **ContentAlignment** | — | enum | 10 values | — | 1 |
| `Turbine.UI` | **ContextMenu** | Object | class | Close, GetItems, ShowMenu, ShowMenuAt | — | 2 |
| `Turbine.UI` | **Control** (changed) | Object | class | static: IsAltKeyDown, IsControlKeyDown, IsShiftKeyDown; Focus, GetAllowDrop, GetBackColor, GetBackColorBlendMode, GetBackground, GetBlendMode, GetControls, GetHeight, GetLeft, GetMousePosition, GetOpacity, GetParent, GetPosition, GetSize, GetStretchMode, GetTop, GetWantsKeyEvents, GetWantsUpdates, GetWidth, GetZOrder, HasFocus, IsDisplayed, IsEnabled, IsMouseVisible, IsVisible, PointToClient, PointToScreen, SetAllowDrop, SetBackColor, SetBackColorBlendMode, SetBackground, SetBlendMode, SetEnabled, SetHeight, SetLeft, SetMouseVisible, SetOpacity, SetParent, SetPosition, SetSize, SetStretchMode, SetTop, SetVisible, SetWantsKeyEvents, SetWantsUpdates, SetWidth, SetZOrder, GetMinimumWidth, SetMinimumWidth, GetMinimumHeight, SetMinimumHeight, GetMinimumSize, SetMinimumSize, GetMaximumWidth, SetMaximumWidth, GetMaximumHeight, SetMaximumHeight, GetMaximumSize, SetMaximumSize, GetEdgeAttachments, AttachEdges | DragDrop, DragEnter, DragLeave, DragStart, EnabledChanged, FocusGained, FocusLost, KeyDown, KeyUp, MouseClick, MouseDoubleClick, MouseDown, MouseEnter, MouseHover, MouseLeave, MouseMove, MouseUp, MouseWheel, PositionChanged, SizeChanged, Update, VisibleChanged | 1 |
| `Turbine.UI` | **ControlList** | Object | class | Add, Clear, Contains, Get, GetCount, IndexOf, Insert, Remove, RemoveAt, Set | — | 1 |
| `Turbine.UI` | **Display** (changed) | — | static | static: GetHeight, GetMousePosition, GetMouseX, GetMouseY, GetSize, GetWidth, GetUIGroupScale | — | 1 |
| `Turbine.UI` | **DragDropInfo** | Object | class | IsSuccessful, SetSuccessful | — | 2 |
| `Turbine.UI` | **EdgeAttachmentType** (new) | — | enum | 7 values (Appendix A.6) | — | 3 |
| `Turbine.UI` | **FontStyle** | — | enum | 2 values | — | 1 |
| `Turbine.UI` | **Graphic** | Object | class | (none of its own) | — | 1 |
| `Turbine.UI` | **HorizontalLayout** | — | enum | 2 values | — | 1 |
| `Turbine.UI` | **Label** | ScrollableControl | class | AppendText, DeselectAll, GetFont, GetFontStyle, GetForeColor, GetOutlineColor, GetSelectedText, GetSelectionLength, GetSelectionStart, GetText, GetTextAlignment, GetTextLength, InsertText, IsMarkupEnabled, IsMultiline, IsSelectable, SelectAll, SetFont, SetFontStyle, SetForeColor, SetMarkupEnabled, SetMultiline, SetOutlineColor, SetSelectable, SetSelectedText, SetSelection, SetSelectionLength, SetSelectionStart, SetText, SetTextAlignment | — | 1 |
| `Turbine.UI` | **ListBox** | ScrollableControl | class | AddItem, ClearItems, ContainsItem, EnsureVisible, GetFlippedLayout, GetIgnoreCellAlignment, GetItem, GetItemAt, GetItemCount, GetMaxColumns, GetMaxItemsPerLine, GetMaxRows, GetOrientation, GetReverseFill, GetSelectedIndex, GetSelectedItem, GetWrapIndentAmount, IndexOfItem, InsertItem, RemoveItem, RemoveItemAt, SetFlippedLayout, SetIgnoreCellAlignment, SetItem, SetMaxColumns, SetMaxItemsPerLine, SetMaxRows, SetOrientation, SetReverseFill, SetSelectedIndex, SetSelectedItem, SetWrapIndentAmount, Sort | ItemAdded, ItemRemoved, SelectedIndexChanged | 1 |
| `Turbine.UI.Lotro` | **Action** | — | enum | 270 values | — | 1 |
| `Turbine.UI.Lotro` | **BaseItemControl** | Control | class | (none of its own) | — | 2 |
| `Turbine.UI.Lotro` | **Button** | UI.Button | class | (none of its own) | — | 1 |
| `Turbine.UI.Lotro` | **CheckBox** | UI.CheckBox | class | (none of its own) | — | 1 |
| `Turbine.UI.Lotro` | **DragDropInfo** | UI.DragDropInfo | class | GetShortcut | — | 2 |
| `Turbine.UI.Lotro` | **EffectDisplay** | Control | class | GetEffect, SetEffect | — | 2 |
| `Turbine.UI.Lotro` | **EntityControl** | Control | class | GetEntity, IsContextMenuEnabled, IsSelectionEnabled, SetContextMenuEnabled, SetEntity, SetSelectionEnabled | — | 2 |
| `Turbine.UI.Lotro` | **EquipmentSlot** | BaseItemControl | class | GetEquipmentSlot, SetEquipmentSlot | DragDrop, EquipmentSlotChanged | 2 |
| `Turbine.UI.Lotro` | **Font** | — | enum | 49 values | — | 1 |
| `Turbine.UI.Lotro` | **GoldButton** | Lotro.Button | class | (none of its own) | — | 1 |
| `Turbine.UI.Lotro` | **GoldWindow** | Lotro.Window | class | (none of its own) | — | 1 |
| `Turbine.UI.Lotro` | **ItemControl** | BaseItemControl | class | GetItem, SetItem | — | 2 |
| `Turbine.UI.Lotro` | **ItemInfoControl** | BaseItemControl | class | GetItemInfo, GetQuantity, SetItemInfo, SetQuantity | EnabledChanged | 2 |
| `Turbine.UI.Lotro` | **LotroUI** | — | static | static: IsEnabled, Reset, SetEnabled | — | 1 |
| `Turbine.UI.Lotro` | **LotroUIElement** | — | enum | 10 values | — | 1 |
| `Turbine.UI.Lotro` | **Quickslot** | Control | class | GetShortcut, IsUseOnRightClick, SetShortcut, SetUseOnRightClick | DragDrop, ShortcutChanged | 2 |
| `Turbine.UI.Lotro` | **ScrollBar** | UI.ScrollBar | class | (none of its own) | OrientationChanged | 1 |
| `Turbine.UI.Lotro` | **Shortcut** | Object | class | GetData, GetItem, GetType, SetData, SetType | — | 2 |
| `Turbine.UI.Lotro` | **ShortcutType** | — | enum | 8 values | — | 2 |
| `Turbine.UI.Lotro` | **TextBox** | UI.TextBox | class | (none of its own) | — | 1 |
| `Turbine.UI.Lotro` | **UIScalingGroupType** (new) | — | enum | 57 values (Appendix A.7) | — | 3 |
| `Turbine.UI.Lotro` | **Window** | UI.Window | class | IsResizable, SetResizable | — | 1 |
| `Turbine.UI` | **MenuItem** | Object | class | GetText, IsChecked, IsEnabled, SetChecked, SetEnabled, SetText, GetItems | Click | 2 |
| `Turbine.UI` | **MenuItemList** | Object | class | Add, Clear, Contains, Get, GetCount, IndexOf, Insert, Remove, RemoveAt, Set | — | 2 |
| `Turbine.UI` | **MouseButton** | — | enum | 3 values | — | 1 |
| `Turbine.UI` | **Orientation** | — | enum | 2 values | — | 1 |
| `Turbine.UI` | **ScrollBar** | Control | class | GetDecrementButton, GetIncrementButton, GetLargeChange, GetMaximum, GetMinimum, GetOrientation, GetSmallChange, GetThumbButton, GetValue, SetDecrementButton, SetIncrementButton, SetLargeChange, SetMaximum, SetMinimum, SetOrientation, SetSmallChange, SetThumbButton, SetValue | OrientationChanged, ValueChanged | 1 |
| `Turbine.UI` | **ScrollableControl** | Control | class | GetHorizontalScrollBar, GetVerticalScrollBar, SetHorizontalScrollBar, SetVerticalScrollBar | — | 1 |
| `Turbine.UI` | **TextBox** | Label | class | IsReadOnly, SetReadOnly | EnabledChanged, TextChanged | 1 |
| `Turbine.UI` | **TreeNode** (changed) | Control | class | Collapse, CollapseAll, Expand, ExpandAll, GetChildNodes, GetParentNode, IsExpanded, IsSelected, SetExpanded | SelectedChanged | 2 |
| `Turbine.UI` | **TreeNodeList** | Object | class | Add, Clear, Contains, Get, GetCount, IndexOf, Remove, RemoveAt | — | 2 |
| `Turbine.UI` | **TreeView** | ScrollableControl | class | CollapseAll, ExpandAll, GetFilter, GetIndentationWidth, GetItemAt, GetNodes, GetSelectedNode, GetSortMethod, Refresh, SetFilter, SetIndentationWidth, SetSelectedNode, SetSortMethod | SelectedNodeChanged | 2 |
| `Turbine.UI` | **VerticalLayout** | — | enum | 2 values | — | 1 |
| `Turbine.UI` | **Window** (changed) | Control | class | Activate, Close, GetText, SetText, GetRotation, SetRotation, RegisterForGlobalScaling, UnregisterForGlobalScaling, SetUIScalingGroup, GetUIScalingGroup, SetScale, GetScale, SetScalingOriginPoint, GetScalingOriginPoint (min/max sizes moved to Control) | Activated, Closed, Closing, Deactivated, UIScaleChanged | 1 |

Not listed: the namespace tables themselves. Events listed per class are those whose names the
client defines for that class's notifications (S2, S3, BIN); the event table of §9.3 gives
their arguments.

---

## 17. Implementation tiers

Tiers are steps **inside** the plugin-host work (Architecture: `tessera-plugins`, roadmap
M5).

<a id="17-1"></a>
### 17.1 Tier 1 — "hello window" plugins run

Goal: a typical small plugin (a window with labels, buttons and a text box, a slash command,
saved settings, player name/level/morale display) loads, works and unloads cleanly.

- Runtime: §3 sandbox; §4 discovery; §5 descriptor; §6 apartments and lifecycle including
  `Plugins`/`plugin` globals, `Load`/`Unload`, `/plugins load|unload|list|refresh`; §7
  `import` with namespace environments; §8 object model and subclassing; §9 event dispatch
  including handler tables and `Update`; §10 plugin data (all scopes, sync `Load` during
  loading, async `Load` with handler, deferred `Save` with flush on unload).
- `Turbine`: `Object`, `Plugin`, `PluginData`, `PluginManager` (all functions;
  `ShowOptions` may be a no-op), `Shell`, `ShellCommand`, `Engine`, `Chat` (fed by Tessera's
  chat and system messages), enums `ChatType`, `DataScope`, `Language`.
- `Turbine.UI`: `Control` (without edge attachments), `Window` (without scaling), `Label`
  (full markup), `TextBox`, `Button`, `CheckBox`, `ScrollBar`, `ScrollableControl`, `ListBox`
  (incl. `Sort`), `ControlList`, `Color` (with named colours), `Display`, `Graphic`, and all
  `Turbine.UI` enums.
- `Turbine.UI.Lotro`: `Font` (rendered with Tessera's font pipeline from the `0x42`
  records), `Window`, `GoldWindow`, `Button`, `GoldButton`, `CheckBox`, `TextBox`,
  `ScrollBar`, `LotroUI` (flag store), `LotroUIElement`, `Action`.
- `Turbine.Gameplay`: `LocalPlayer.GetInstance()` with the `Entity`, `Actor` and `Player`
  members (others may return defaults), enums `Class`, `Race`, `Alignment`.
- Every other class of §16 exists as a table (so `import` and `IsA` work) and its methods
  return nil/defaults without raising, logged once per method at debug level.

<a id="17-2"></a>
### 17.2 Tier 2 — inventory and group plugins

`ContextMenu`/`MenuItem`/`MenuItemList`, `TreeView`/`TreeNode`/`TreeNodeList`,
`DragDropInfo` (both), the plugin manager window with options panels (`GetOptionsPanel`,
`ShowOptions`), `Backpack`, `Item`, `ItemInfo`, `Equipment`, `Wallet`, `WalletItem`,
`Effect`, `EffectList`, `Party`, `PartyMember`, `Raid`, `Pet`, `FreePeopleAttributes`,
`MonsterPlayerAttributes`, the item/effect/entity controls, `Quickslot`, `Shortcut`, and
their enums.

<a id="17-3"></a>
### 17.3 Tier 3 — everything else

Per-class attributes and stances, mounts, bank/vault/shared storage, skills and gambits,
professions and recipes, `Window` rotation, stretch modes 1–4, global UI scaling
(`UIScalingGroupType`, `Window` scaling members, `UIScaleChanged`,
`Display.GetUIGroupScale`, `Plugin:GetUsesGlobalScaling`), edge attachments.

---

## 18. Invariants and conformance checks

Each is checkable with a synthetic plugin tree (generated by test code, never shipped
third-party plugins) or against the user install.

1. **Sandbox:** in a fresh apartment, `io`, `os`, `package`, `require`, `module`, `dofile`,
   `loadfile`, `debug` are nil; `string`, `table`, `math`, `coroutine`, `getfenv`, `setfenv`,
   `pcall`, `unpack`, `table.getn`, `loadstring`, `string.dump`, `newproxy`, `gcinfo` exist;
   `_VERSION == "Lua 5.1"`; `Turbine.Engine` and `Turbine.PluginData` exist without any
   `import`.
2. **Discovery:** with descriptors at `Plugins/a.plugin`, `Plugins/X/b.plugin` and
   `Plugins/X/Y/c.plugin`, only `b` is available. Two descriptors named `Foo` and `FOO` yield
   one available plugin (the first in order).
3. **Descriptor:** a descriptor with two `Name` children, with a 121-character `Name`, or
   without `Name` is rejected; a 200-character `Author` reads back as its first 120
   characters; a descriptor without `Configuration` loads into `Default`.
4. **Resolution precedence:** with both `Plugins/A/B/` (containing `__init__.lua`) and
   `Plugins/A/B.lua`, `import "A.B"` runs the folder's `__init__.lua`; with an empty folder
   `Plugins/A/B/` and `Plugins/A/B.lua`, `import "A.B"` fails.
5. **Environment isolation:** packages `A.P` and `B.P` each defining a bare global `T`
   produce `A.T` and `B.T`, and `_G.T` stays nil. After the import, a function defined in
   `A/P.lua` can still read `Turbine` as a bare name, and `A.Turbine == Turbine`.
6. **Run once:** a file imported twice under the same name runs once; `import` returns no
   values (`select("#", import "A.P") == 0`).
7. **Package name:** inside `A/B/Main.lua` imported as `A.B.Main`, `_.Name == "A.B.Main"`
   during its top-level run.
8. **Apartment isolation:** a global set by a plugin in apartment `X` is not visible in
   apartment `Y`; `Turbine.Chat` in `X` and `Y` are different tables. Apartments `Foo` and
   `foo` are the same apartment.
9. **Self-unload refusal:** `UnloadScriptState(own apartment)` leaves the apartment loaded and
   raises no error.
10. **Load bookkeeping:** after a plugin's main file raises an error, `Plugins[name]` is nil
    and its `Load` handler did not run; after a successful load, `plugin` is nil.
11. **Plugin data timing:** inside top-level code, `Load(1, "k")` returns the stored value;
    inside a later event handler, `Load(1, "k")` without a handler raises an error, and with
    a handler returns nothing and calls the handler on a later pump with the value.
12. **Key rule (Tessera):** keys of length 0 and 65 are rejected; lengths 1 and 64 are
    accepted; `"a-b"` is rejected; `"A_1"` is accepted; `"é"` is rejected. The last is the
    stated Tessera deviation of §10.2: the original's rule is `isalnum` in its game-language
    locale, which accepts `"é"` under code page 1252 (P44). Package names follow the same
    ASCII rule (§7.2).
13. **Plugin data round trip:** for random nested tables of booleans, finite numbers and
    strings (including quotes, backslashes, newlines, NUL, non-ASCII) with string and number
    keys, Tessera's `Load(Save(v))` equals `v` (deep equality); the files parse with the
    §10.4 grammar. A file written in the original's exact layout (§10.4 example) loads as
    the table it describes.
14. **Shared table:** saving `{a = t, b = t}` with a non-empty `t` and loading it back gives a
    table with exactly one of `a`, `b` present.
15. **Handler tables:** with `obj.Click = {f1, f2}` both run, in order, each with
    `(obj, args)`; an error in `f1` does not stop `f2`; a string element is skipped.
16. **Class-level handler:** assigning `Turbine.Plugin.Load` before load fires it for the
    plugin with `self`/sender = the plugin object.
17. **Subclassing:** a Lua subclass of `Turbine.UI.Window` built with a user-defined `class`
    helper can call `SetSize`, receives `MouseClick`, and `IsA(Turbine.UI.Control)` is true.
18. **Lists:** `ControlList.Get(0)` and `Get(count + 1)` raise errors; `IndexOf` of an
    absent control returns −1.
19. **Colour:** `Turbine.UI.Color().A == 0`; `Turbine.UI.Color(1, 0, 0).A == 1`. For every
    named colour of Appendix A.5 and each of R, G, B, the field equals the 32-bit float
    nearest to (byte / 255 rounded to 3 decimals); e.g. `Turbine.UI.Color.AliceBlue.R` equals
    the float nearest 0.941 and **not** the float nearest 240/255; `Transparent` is A 0,
    R/G/B 1; every other named colour has A 1 (§13.9).
20. **Money:** for money 123456789, `GetMoneyComponents()` returns 89, 567, 1234.
21. **Shell:** `AddCommand("a; b :c,", cmd)` returns 3 and registers `a`, `b`, `c`;
    `IsCommand("A")` is then true; `RemoveCommand(cmd)` returns 3.
22. **Date:** on a Sunday, `GetDate().DayOfWeek == 1`; on 1 January, `DayOfYear == 1`.
23. **Fonts:** for every non-zero `Font` value `v`, the record `v` exists in the archive set
    and its payload's first u32 equals `v` and second u32 equals the name's trailing number
    (holds for all 48 in the user install, 2026-10-03).
24. **Actions:** every non-zero `Action` value is a key of the input-action enum mapper read
    through EMAPPER (holds for all 269, 2026-10-05).
25. **No crash on hostile input:** malformed descriptors, binary or huge Lua files, deep
    recursion, `Sort` with an inconsistent comparator, invalid DIDs and image files, cyclic
    tables passed to `Save`, and malformed `.plugindata` files all produce errors or empty
    results, never a panic.

---

## 19. Open questions (summary)

All `> **OPEN:**` callouts above:

- Sandbox: whether any path sets an apartment memory limit (§3.3).
- Names: C-runtime class of package-name bytes 0x80–0xFF and of key units above 0xFF (§7.2).
- Lifecycle: autoload order (§6.2); localized aliases of `/plugins` (§6.4); chat colour and
  prefix of reported errors (§6.6).
- Events: fields of `Party.MemberAdded/MemberRemoved`, `AssistTargetRemoved` (§9.3);
  `Update` on hidden controls (§9.4); setters that fire their own change events (§9.6).
- Plugin data: callbacks of writes queued before an apartment closed (§10.6).
- Shell and chat: `WriteLine` lines and `Chat.Received` (§11.1); whitespace and case in
  `Execute` arguments (§11.2); markup in chat `Message` (§11.3).
- UI: opacity of child and non-window controls (§13.1); edge-attachment order and rules
  (§13.2); window rotation units and pivot, scaling units and origin point, Escape (§13.3);
  `Display.GetUIGroupScale` semantics (§13.10); label selection base and defaults (§13.4);
  which buttons activate `Click` (§13.6); stretch modes (§13.8); skinned frame metrics and
  parts (§14.2); interaction details of the game-object controls (§14.5).
- Lotro and gameplay: item shortcut companion value and `SetData` parsing (§14.6); which of
  the remaining enums follow a data enum (§15).

---

## 20. Corrections to other specs

These rest on the disassembly of client build 4906.0071.1794.4035 (BIN) and on O4. They are
recorded here, not applied, because this session owns only this file.

| Target | Current text | Correction | Basis |
|---|---|---|---|
| `plugin-descriptor.md` §4.2 step 2, D3, §13 | Discovery descends into subfolders; maximum depth OPEN | Discovery reads exactly `Plugins/<folder>/*.plugin`: files directly in `Plugins/` and anything deeper than one folder are ignored (both directory listings in the discovery routine are non-recursive). See §4.3 here. | BIN |
| `plugin-descriptor.md` §4.2 last OPEN, §13 | Duplicate `Name` behaviour OPEN | The available map is keyed by `Name` compared case-insensitively; a later descriptor with a name already present is dropped (first found wins). | BIN |
| `plugin-descriptor.md` §5.2 steps 3–4, D6, D7, §5.4 | Repeated elements keep the last value; `Name` and `Version` are truncated to 120; all five fields optional | A repeated `Name`, `Version`, `Author`, `Description` or `Image` child **rejects** the descriptor. `Name` is **required**. `Name` and `Version` longer than 120 characters (after trimming) **reject**; `Author` and `Image` are cut to 120; `Description` is cut to 2048; any of the five that is present but empty after trimming rejects. `Package` must be 1–120 characters after trimming. See §5.2 here. | BIN |
| `plugin-descriptor.md` §7.2 step 1, D11, §7.3 last invariant | Native names never touch the disk | In plugin apartments a native name is **also** looked up on disk; when both exist the package is the merge (native part installed, then the disk chunk run in the same namespace). `Plugins/Turbine/__init__.lua` therefore runs whenever an apartment is created. See §7.2, §7.4 here. | BIN |
| `plugin-descriptor.md` §6.2 OPEN | Case-folding of apartment names OPEN | Apartment names compare case-insensitively (the apartment map hashes lower-cased characters and compares case-insensitively). | BIN |
| `plugin-descriptor.md` §10 OPEN, D13 | Inner structure of `PluginOptions.xml` OPEN | Structure resolved: `Plugin` elements with `Name` and `AllCharacters` (`true`/`false`) attributes and `Character` child elements with `Name` under `AutoloadOptions`; `Plugin` elements with `Name` and `UseScaling` (`True`/`False`) attributes under `ScalingOptions`. See §5.4 here. | BIN |
| `plugin-descriptor.md` §11, D14 | Keys rejected when they contain "characters invalid in a file name" | The original accepts a key unit when it is `_` or passes the C runtime's `isalnum` in the locale the client sets from its game language (code page 1252 for all four languages, so accented letters such as U+00E9 pass); everything else is rejected. Tessera accepts only ASCII letters, ASCII digits and `_` (stated deviation). See §10.2 here. | BIN (P14, P43), P44 |
| `keymap.md` OPEN (table mapping Tessera actions to the plugin API's action IDs) | OPEN | The plugin API's `Action` values are keys of the input-action enum mapper (EMAPPER label for input actions; 0x230000AF with base chain in this install; all 269 non-zero values present). The scroll-up and scroll-down actions are keys 0x82 and 0x10. See §14.4 here. | O4, BIN |

---

## Appendix A. Enumeration values

All values below were read from the client's registration code (BIN, 2026-10-05) and agree
with the live dump (S3) where S3 has them; Font values are also Confirmed against data (P12).
Exception: the four single-value stance enums (`BurglarStance`, `ChampionStance`,
`GuardianStance`, `BrawlerStance`, each `None = 0`) are from S3 only (Reported); the BIN
extraction did not isolate them.
`Turbine.Language` in S2 also lists `Russian = 0x10000007`, which this build does not
register.

### A.1 Small enumerations

| Enumeration | Values (name = value) |
|---|---|
| `Turbine.ChatType` | Undef = 0, Error = 1, Admin = 3, Standard = 4, Say = 5, Tell = 6, Emote = 7, Fellowship = 11, Kinship = 12, Officer = 13, Advancement = 14, Trade = 15, LFF = 16, OOC = 18, Regional = 19, Death = 20, Quest = 21, Raid = 23, Unfiltered = 25, Roleplay = 27, UserChat1 = 28, UserChat2 = 29, UserChat3 = 30, UserChat4 = 31, Tribe = 32, PlayerCombat = 34, EnemyCombat = 35, SelfLoot = 36, FellowLoot = 37, World = 38, UserChat5 = 39, UserChat6 = 40, UserChat7 = 41, UserChat8 = 42 |
| `Turbine.DataScope` | Account = 1, Server = 2, Character = 3 |
| `Turbine.Language` | Invalid = 0, English = 2, EnglishGB = 0x10000001, French = 0x10000003, German = 0x10000004 |
| `Turbine.UI.BlendMode` | Color = 1, Normal = 2, Multiply = 3, AlphaBlend = 4, Overlay = 5, Grayscale = 6, Screen = 7, None = 8, Undefined = 8 |
| `Turbine.UI.ContentAlignment` | Undefined = 0, TopLeft = 1, TopCenter = 2, TopRight = 3, MiddleLeft = 4, MiddleCenter = 5, MiddleRight = 6, BottomLeft = 7, BottomCenter = 8, BottomRight = 9 |
| `Turbine.UI.FontStyle` | None = 0, Outline = 8 |
| `Turbine.UI.MouseButton` | Left = 1, Right = 2, Middle = 4 |
| `Turbine.UI.Orientation` | Horizontal = 0, Vertical = 1 |
| `Turbine.UI.HorizontalLayout` | LeftToRight = 0, RightToLeft = 1 |
| `Turbine.UI.VerticalLayout` | TopToBottom = 0, BottomToTop = 1 |
| `Turbine.UI.Lotro.LotroUIElement` | PluginsManager = 0x10000001, Party = 0x10000002, Backpack1 = 0x10000003, Backpack2 = 0x10000004, Backpack3 = 0x10000005, Backpack4 = 0x10000006, Backpack5 = 0x10000007, Backpack6 = 0x10000008, Vitals = 0x10000009, Target = 0x1000000B |
| `Turbine.UI.Lotro.ShortcutType` | Undef = 0, Undefined = 0, Item = 2, Alias = 4, Emote = 5, Skill = 6, Pet = 7, Hobby = 9 |
| `Turbine.Gameplay.Alignment` | Undefined = 0, FreePeople = 1, MonsterPlayer = 2 |
| `Turbine.Gameplay.Class` | Undefined = 0, Guardian = 23, Captain = 24, Minstrel = 31, Burglar = 40, WarLeader = 52, Reaver = 71, Stalker = 126, Weaver = 127, Defiler = 128, Hunter = 162, Champion = 172, BlackArrow = 179, LoreMaster = 185, Troll = 190, Ranger = 191, Chicken = 192, RuneKeeper = 193, Warden = 194, Beorning = 214, Brawler = 215, Mariner = 216 |
| `Turbine.Gameplay.Race` | Undefined = 0, Man = 23, Elf = 65, Dwarf = 73, Hobbit = 81, Beorning = 114, HighElf = 117, StoutAxe = 120, RiverHobbit = 125 |
| `Turbine.Gameplay.Equipment` | Undefined = 0, Head = 1, Chest = 2, Legs = 3, Gloves = 4, Boots = 5, Shoulder = 6, Back = 7, Bracelet1 = 8, Bracelet2 = 9, Necklace = 10, Ring1 = 11, Ring2 = 12, Earring1 = 13, Earring2 = 14, Pocket = 15, PrimaryWeapon = 16, SecondaryWeapon = 17, Shield = 17, Instrument = 18, RangedWeapon = 18, CraftTool = 19, Class = 20, Bridle = 21 |
| `Turbine.Gameplay.EffectCategory` | Undefined = 0, Disease = 2, Physical = 4, Wound = 8, Cry = 16, Song = 32, Fear = 64, Poison = 128, Elemental = 512, Corruption = 1024, Dispellable = 1226 |
| `Turbine.Gameplay.ItemQuality` | Undefined = 0, Legendary = 1, Rare = 2, Incomparable = 3, Uncommon = 4, Common = 5 |
| `Turbine.Gameplay.ItemDurability` | Undefined = 0, Substantial = 1, Brittle = 2, Normal = 3, Tough = 4, Flimsy = 5, Indestructible = 6, Weak = 7 |
| `Turbine.Gameplay.ItemWearState` | Undefined = 0, Damaged = 1, Pristine = 2, Broken = 3, Worn = 4 |
| `Turbine.Gameplay.ReadyState` | Unset = 0, Ready = 1, NotReady = 2 |
| `Turbine.Gameplay.SkillType` | Normal = 0, Mount = 1, Gambit = 2 |
| `Turbine.Gameplay.Profession` | Undefined = 0, Metalsmith = 1, Forester = 2, Scholar = 3, Jeweller = 4, Tailor = 5, Weaponsmith = 6, Prospector = 7, Woodworker = 8, Farmer = 9, Cook = 10 |
| `Turbine.Gameplay.Vocation` | None = 0, Explorer = 1, Tinker = 2, Yeoman = 3, Historian = 4, Armsman = 5, Woodsman = 6, Armorer = 7 |
| `Turbine.Gameplay.CraftTier` | Undefined = 0, Apprentice = 1, Journeyman = 2, Expert = 3, Artisan = 4, Master = 5, Supreme = 6, Westfold = 7, Eastemnet = 8, Westemnet = 8 |
| `Turbine.Gameplay.Attributes.HunterStance` | None = 0, Precision = 1, Strength = 2, Endurance = 3 |
| `Turbine.Gameplay.Attributes.MinstrelStance` | None = 0, WarSpeech = 1, Harmony = 2 |
| `Turbine.Gameplay.Attributes.WardenStance` | None = 0, Determination = 2, Assailment = 3 |
| `Turbine.Gameplay.Attributes.WardenGambit` | Spear = 1, Shield = 2, Fist = 3, Javelin = 4 |
| `Turbine.Gameplay.Attributes.BurglarStance` | None = 0 |
| `Turbine.Gameplay.Attributes.ChampionStance` | None = 0 |
| `Turbine.Gameplay.Attributes.GuardianStance` | None = 0 |
| `Turbine.Gameplay.Attributes.BrawlerStance` | None = 0 |

### A.2 `Turbine.UI.Lotro.Font`

| Name | DID | Name | DID | Name | DID |
|---|---|---|---|---|---|
| Undefined | 0 | TrajanPro20 | 0x42000000 | Verdana14 | 0x42000001 |
| TrajanPro13 | 0x42000002 | TrajanPro15 | 0x42000007 | Verdana12 | 0x42000008 |
| TrajanPro18 | 0x42000009 | TrajanPro21 | 0x4200000A | TrajanPro16 | 0x4200000B |
| TrajanPro14 | 0x4200000C | TrajanPro23 | 0x4200000D | BookAntiqua20 | 0x4200000E |
| BookAntiquaBold22 | 0x4200000F | BookAntiquaBold24 | 0x42000010 | TrajanProBold30 | 0x42000011 |
| TrajanProBold25 | 0x42000012 | TrajanProBold22 | 0x42000013 | TrajanProBold36 | 0x42000014 |
| TrajanPro24 | 0x42000015 | Verdana20 | 0x42000016 | Verdana10 | 0x42000017 |
| Verdana16 | 0x42000018 | Verdana18 | 0x42000019 | Verdana22 | 0x4200001A |
| Verdana23 | 0x4200001B | TrajanProBold24 | 0x42000024 | TrajanPro26 | 0x42000025 |
| TrajanPro28 | 0x42000026 | FixedSys15 | 0x42000027 | LucidaConsole12 | 0x42000028 |
| Arial12 | 0x42000029 | TrajanProBold16 | 0x4200002A | TrajanPro19 | 0x42000035 |
| BookAntiquaBold14 | 0x420000F0 | BookAntiqua22 | 0x420000F1 | BookAntiquaBold18 | 0x420000F2 |
| BookAntiquaBold12 | 0x420000F3 | BookAntiqua24 | 0x420000F4 | BookAntiquaBold19 | 0x420000F5 |
| BookAntiqua12 | 0x420000F6 | TrajanPro25 | 0x420000F7 | BookAntiqua16 | 0x420000F8 |
| BookAntiqua14 | 0x420000F9 | BookAntiqua18 | 0x420000FA | VerdanaBold16 | 0x420000FD |
| BookAntiqua28 | 0x42000168 | BookAntiqua26 | 0x42000169 | BookAntiqua32 | 0x4200016A |
| BookAntiqua36 | 0x4200016B |  |  |  |  |

### A.3 `Turbine.UI.Lotro.Action` (270 values)

Undefined = 0, CameraInstantMouseLook = 92, CaptureScreenshot = 116, Escape = 145, Quickbar3Visibility = 0x10000002, Quickslot_63 = 0x10000003, Quickslot_47 = 0x10000004, Quickslot_30 = 0x10000005, ToggleQuickslotLock = 0x10000006, VendorFullStack = 0x10000007, SelectFellowFive = 0x10000008, Quickslot_64 = 0x10000009, Quickslot_48 = 0x1000000A, Quickslot_31 = 0x1000000B, SelectionAssist = 0x1000000C, SelectionNearestPlayer = 0x1000000D, Quickslot_65 = 0x1000000E, Quickslot_49 = 0x1000000F, Quickslot_32 = 0x10000010, Quickslot_20 = 0x10000011, PreviousAttacker = 0x10000012, SelectionNextPlayer = 0x10000013, ToggleRadar = 0x10000014, ToggleStatusbarPanel = 0x10000015, ToggleBag1 = 0x10000016, Quickslot_66 = 0x10000017, Quickslot_33 = 0x10000018, Quickslot_21 = 0x10000019, Tooltip_Detach = 0x1000001A, ToggleSkillPanel = 0x1000001B, Quickbar4Visibility = 0x1000001D, ToggleBag2 = 0x1000001E, Quickslot_67 = 0x1000001F, Quickslot_50 = 0x10000020, Quickslot_34 = 0x10000021, Quickslot_22 = 0x10000022, SelectionPreviousFoe = 0x10000023, ToggleBag3 = 0x10000025, Quickslot_68 = 0x10000026, Quickslot_51 = 0x10000027, Quickslot_35 = 0x10000028, Quickslot_23 = 0x10000029, Quickslot_1 = 0x1000002A, SelectFellowOne = 0x1000002C, ToggleBag4 = 0x1000002D, Quickslot_69 = 0x1000002E, Quickslot_52 = 0x1000002F, Quickslot_36 = 0x10000030, Quickslot_24 = 0x10000031, Quickslot_2 = 0x10000032, SelectionPreviousCreature = 0x10000033, SelectionSelf = 0x10000034, ToggleSocialPanel = 0x10000035, ToggleTraitPanel = 0x10000036, ToggleOptionsPanel = 0x10000038, ToggleBag5 = 0x10000039, QuickSlot_Chat = 0x1000003A, Quickslot_53 = 0x1000003B, Quickslot_37 = 0x1000003C, Quickslot_25 = 0x1000003D, Quickslot_3 = 0x1000003E, SelectionPreviousItem = 0x1000003F, ToggleCraftingPanel = 0x10000040, MapPanel = 0x10000041, SelectFellowSix = 0x10000043, Quickslot_54 = 0x10000044, Quickslot_38 = 0x10000045, Quickslot_26 = 0x10000046, Quickslot_4 = 0x10000047, TitlesPanel = 0x10000048, ToggleJournalPanel = 0x10000049, ToggleQuestPanel = 0x1000004A, Quickslot_55 = 0x1000004C, Quickslot_39 = 0x1000004D, Quickslot_27 = 0x1000004E, Quickslot_10 = 0x1000004F, Quickslot_5 = 0x10000050, SelectFellowThree = 0x10000052, Quickbar5Visibility = 0x10000053, Quickslot_56 = 0x10000054, Quickslot_28 = 0x10000055, Quickslot_11 = 0x10000056, Quickslot_6 = 0x10000057, SelectionNearestFellow = 0x10000058, ChatModeReply = 0x1000005A, Quickslot_57 = 0x1000005C, Quickslot_29 = 0x1000005D, Quickslot_12 = 0x1000005E, Quickslot_7 = 0x1000005F, Logout = 0x10000060, ToggleComboUI = 0x10000061, VoiceChat_Talk = 0x10000063, Quickbar2Visibility = 0x10000064, Quickslot_58 = 0x10000065, Quickslot_13 = 0x10000066, Quickslot_8 = 0x10000067, ShowDamage = 0x10000069, ToggleAccomplishmentPanel = 0x1000006A, Quickslot_59 = 0x1000006F, Quickslot_14 = 0x10000070, Quickslot_9 = 0x10000071, Quickbar1Visibility = 0x10000077, Quickslot_15 = 0x10000078, SelectionNearestCreature = 0x10000079, Start_Command = 0x1000007A, ToggleHiddenDragBoxes = 0x1000007B, ToggleInputPanel = 0x1000007C, ToggleToolbar = 0x10000081, Quickslot_16 = 0x10000082, SelectionPreviousFellow = 0x10000083, SelectionNextCreature = 0x10000084, Use = 0x10000085, ToggleItemSellLock = 0x10000086, ToggleQuestJournal = 0x10000087, SelectFellowFour = 0x1000008B, SelectFellowTwo = 0x1000008C, Quickslot_40 = 0x1000008D, Quickslot_17 = 0x1000008E, PreviousSelection = 0x1000008F, ToggleToolbarPanel = 0x10000090, ToggleBags = 0x10000094, Quickslot_41 = 0x10000095, Quickslot_18 = 0x10000096, SelectionNearestFoe = 0x10000097, SelectionPreviousPlayer = 0x10000098, TopFellowshipManoeuvre = 0x10000099, Quickslot_70 = 0x1000009C, Quickslot_42 = 0x1000009D, Quickslot_19 = 0x1000009E, BottomFellowshipManoeuvre = 0x1000009F, Quickslot_71 = 0x100000A2, Quickslot_43 = 0x100000A3, SelectionOff = 0x100000A4, SelectionNextFellow = 0x100000A5, SelectionNextFoe = 0x100000A6, LeftFellowshipManoeuvre = 0x100000A8, Quickslot_72 = 0x100000AB, Quickslot_60 = 0x100000AC, Quickslot_44 = 0x100000AD, RightFellowshipManoeuvre = 0x100000AE, Quickslot_61 = 0x100000AF, Quickslot_45 = 0x100000B0, SelectionNearestItem = 0x100000B1, SelectionNextItem = 0x100000B2, UI_Toggle = 0x100000B3, EnterChatMode = 0x100000B4, ToggleAssistancePanel = 0x100000B5, QuickSlot_SkillMode = 0x100000B7, Quickslot_62 = 0x100000B8, Quickslot_46 = 0x100000B9, Show_Names = 0x100000BA, DressingRoom = 0x100000BB, AutoLootAll = 0x100000BD, Music_Ab4 = 0x100000BE, Music_Bb2 = 0x100000BF, Music_Bb3 = 0x100000C0, Music_B2 = 0x100000C1, Music_Bb4 = 0x100000C2, Music_B3 = 0x100000C3, Music_G2 = 0x100000C4, Music_B4 = 0x100000C5, Music_G3 = 0x100000C6, Music_G4 = 0x100000C7, Music_Db2 = 0x100000C8, Music_Db3 = 0x100000C9, Music_Db4 = 0x100000CA, Music_A2 = 0x100000CB, Music_A3 = 0x100000CC, Music_F2 = 0x100000CD, Music_Eb2 = 0x100000CE, Music_A4 = 0x100000CF, Music_F3 = 0x100000D0, Music_Eb3 = 0x100000D1, Music_D2 = 0x100000D2, Music_F4 = 0x100000D3, Music_Eb4 = 0x100000D4, Music_D3 = 0x100000D5, Music_D4 = 0x100000D6, Music_Gb2 = 0x100000D7, Music_Gb3 = 0x100000D8, Music_Gb4 = 0x100000D9, Music_E2 = 0x100000DA, Music_E3 = 0x100000DB, Music_C2 = 0x100000DC, Music_E4 = 0x100000DD, Music_C3 = 0x100000DE, Music_C4 = 0x100000DF, Music_Ab2 = 0x100000E0, Music_C5 = 0x100000E1, Music_Ab3 = 0x100000E2, ToggleMusicMode = 0x100000E3, SelectionNextTracked = 0x100000E4, SelectionPreviousTracked = 0x100000E5, FellowshipSkillAssist = 0x100000E6, AssistFellowThree = 0x100000E8, AssistFellowTwo = 0x100000E9, AssistFellowSix = 0x100000EA, AssistFellowFive = 0x100000EB, AssistFellowFour = 0x100000EC, MusicEndSong = 0x100000EF, ReputationPanel = 0x100000F0, SpearMark = 0x100000F1, ArrowMark = 0x100000F2, SunMark = 0x100000F3, SwordsMark = 0x100000F4, MoonMark = 0x100000F5, StarMark = 0x100000F6, ClawMark = 0x100000F7, SkullMark = 0x100000F8, LeafMark = 0x100000F9, ShieldMark = 0x100000FA, HousingPanel = 0x100000FB, PresentOutfit1 = 0x100000FC, PresentOutfit2 = 0x100000FD, PresentMainInventory = 0x100000FE, ToggleItemAdvancementPanel = 0x1000012A, ToggleGambitPanel = 0x1000015F, ItemLinkToChat = 0x10000160, VendorQuantity = 0x1000017B, ToggleStackDisplay = 0x1000017C, ToggleThreatTrackerPanel = 0x1000018D, ToggleSkirmishPanel = 0x1000018E, ToggleWorldJoin = 0x100001B0, ToggleWebStore = 0x100001B1, MainMenu = 0x100001BB, SystemMenu = 0x100001BC, ToggleMountsPanel = 0x100001BD, ToggleCraftingProf2Panel = 0x100001BE, ToggleCraftingProf1Panel = 0x100001BF, ToggleCraftingProf3Panel = 0x100001C0, KinshipPanel = 0x100001C1, GroupStagePanel = 0x100001C2, ToggleSocialFellowingPanel = 0x100001C3, RaidPanel = 0x100001C4, FriendsPanel = 0x100001C5, HobbyPanel = 0x100001C6, DestinyPointPerksPanel = 0x100001C9, DismountRemount = 0x100001CC, TogglePaperItemPanel = 0x100001CD, ClearAllFilters = 0x100001CE, TogglePVPPanel = 0x100001CF, ToggleBioPanel = 0x100001D0, PresentOutfit3 = 0x100001D1, PresentOutfit4 = 0x100001D2, PresentOutfit5 = 0x100001D3, ToggleInstanceFinderPanel = 0x100001D4, PresentOutfit6 = 0x100001D5, PresentOutfit7 = 0x100001D6, TogglePluginManagerUI = 0x100001D7, TogglePluginManager = 0x100001D8, TrackNearbyQuests = 0x100001D9, ToggleBag6 = 0x1000022F, ToggleMountedCombatUI = 0x10000230, ShowRemoteQuestActions = 0x10000233, QuickslotPageDown = 0x10000235, QuickslotPageUp = 0x10000236, TogglePendingLoot = 0x10000237, ToggleMithrilCoinsMenu = 0x10000238, ToggleMithrilCoinsAuctionUI = 0x1000023A, ToggleMountedXpUI = 0x1000023C, FollowSelection = 0x1000023D, FindItems = 0x1000023E, ToggleAlertOverflow = 0x1000023F, ToggleRandomItemRewardUI = 0x10000240, ToggleBigBattlesUI = 0x10000241, ToggleMailUI = 0x10000242, ToggleCollectionUI = 0x10000243, MultiUseItem = 0x10000244, ToggleFilterUI = 0x10000245, PresentOutfit8 = 0x10000246

### A.4 `Turbine.Gameplay.ItemCategory` (187 values)

Undefined = 0, Bow = 1, Treasure = 2, Chest = 3, Minstrel = 4, Hands = 5, Shoulders = 6, Head = 7, Mounts = 8, Pennant = 9, Dagger = 10, Instrument = 11, Axe = 12, Captain = 13, Oil = 14, Legs = 15, Effect = 16, Hunter = 17, Armor = 18, Loremaster = 19, Trap = 20, Decoration = 21, Champion = 22, Feet = 23, Hammer = 24, Healing = 25, Guardian = 26, Quest = 27, Potion = 28, Crossbow = 29, Mace = 30, Key = 31, Tool = 32, Shield = 33, Staff = 34, Crafting = 35, Halberd = 36, Component = 37, Ingredient = 38, Clothing = 39, Club = 40, Dye = 41, Weapon = 42, KinshipCharter = 43, Sword = 44, Back = 45, Spear = 46, Trophy = 47, Burglar = 48, Jewelry = 49, Device = 50, Relic = 51, Book = 52, NonInventory = 53, Thrown = 54, Food = 55, Resource = 56, Scroll = 57, ApprenticeTailorScroll = 58, ApprenticeJewellerScroll = 59, ApprenticeProspectingScroll = 60, ApprenticeFarmerScroll = 61, ApprenticeScholarScroll = 62, ApprenticeForestryScroll = 63, ApprenticeMetalworkScroll = 64, ApprenticeCookScroll = 65, ApprenticeWeaponsmithScroll = 66, ApprenticeWoodworkScroll = 67, MusicDecoration = 82, SurfacePaintDecoration = 83, TrophyDecoration = 84, FurnitureDecoration = 85, FloorDecoration = 86, YardDecoration = 87, WallDecoration = 88, Reputation = 89, Implement = 91, CosmeticHeld = 96, CosmeticBack = 97, SpecialDecoration = 98, CeilingDecoration = 99, FishingBait = 100, FishingOther = 101, Fish = 102, FishingPole = 103, Orb = 104, Warden = 105, Runekeeper = 106, LegendaryWeaponExperience = 107, LegendaryWeaponReset = 108, Deconstructable = 109, Javelin = 110, GuardianBelt = 111, JourneymanCookScroll = 113, ExpertCookScroll = 114, ArtisanCookScroll = 115, MasterCookScroll = 116, SupremeCookScroll = 117, MasterWoodworkScroll = 118, MasterWeaponsmithScroll = 119, ArtisanTailorScroll = 120, ExpertFarmerScroll = 121, JourneymanForestryScroll = 122, SupremeWoodworkScroll = 123, SupremeWeaponsmithScroll = 124, MasterTailorScroll = 125, ArtisanFarmerScroll = 126, ExpertForestryScroll = 127, SupremeTailorScroll = 128, MasterFarmerScroll = 129, ArtisanForestryScroll = 130, SupremeFarmerScroll = 131, MasterForestryScroll = 132, SupremeForestryScroll = 133, JourneymanProspectingScroll = 134, ExpertProspectingScroll = 135, ArtisanProspectingScroll = 136, MasterProspectingScroll = 137, SupremeProspectingScroll = 138, JourneymanScholarScroll = 139, ExpertScholarScroll = 140, JourneymanMetalworkScroll = 141, ArtisanScholarScroll = 142, JourneymanJewellerScroll = 143, ExpertMetalworkScroll = 144, MasterScholarScroll = 145, ExpertJewellerScroll = 146, JourneymanWoodworkScroll = 147, JourneymanWeaponsmithScroll = 148, ArtisanMetalworkScroll = 149, SupremeScholarScroll = 150, ArtisanJewellerScroll = 151, ExpertWoodworkScroll = 152, ExpertWeaponsmithScroll = 153, MasterMetalworkScroll = 154, JourneymanTailorScroll = 155, MasterJewellerScroll = 156, ArtisanWoodworkScroll = 157, ArtisanWeaponsmithScroll = 158, SupremeMetalworkScroll = 159, ExpertTailorScroll = 160, SupremeJewellerScroll = 161, JourneymanFarmerScroll = 162, MinstrelBook = 163, Misc = 164, LegendaryWeaponReplaceLegacy = 166, LegendaryWeaponIncreaseMaxLevel = 167, LegendaryWeaponUpgradeLegacy = 168, CraftingTrophy = 170, LoremasterFood = 171, SpecialTrophy = 172, Special = 173, Social = 174, Map = 175, LegendaryWeaponIncreaseItemLevel = 176, Skirmish = 177, Barter = 178, Shieldspike = 179, CosmeticFeet = 180, CosmeticShoulders = 181, CosmeticChest = 182, CosmeticHead = 183, CosmeticHands = 184, CosmeticLegs = 185, Horn = 187, OptionalIngredient = 188, Perk = 189, Tome = 190, Travel = 191, CosmeticClass = 192, LegendaryWeaponLegacyReveal = 193, LegendaryWeaponUnslotRelics = 194, WestfoldMetalworkScroll = 195, WestfoldJewellerScroll = 196, WestfoldWoodworkScroll = 197, WestfoldWeaponsmithScroll = 198, WestfoldTailorScroll = 199, WestfoldFarmerScroll = 200, WestfoldForestryScroll = 201, WestfoldProspectingScroll = 202, WestfoldCookScroll = 203, WestfoldScholarScroll = 204, Festival = 205, LegendaryWeaponAddLegacy = 206, Beorning = 233, Brawler = 288, Mariner = 294

### A.5 `Turbine.UI.Color` named colours (141)

Listed as web byte triples `#RRGGBB` for compactness only. The component values are **not**
byte/255: each is byte/255 rounded to 3 decimals and stored as a 32-bit float (§13.9, P45;
checked for all 141 colours, 564 of 564 components, BIN, and all 423 R/G/B values against
S3). Alpha is 1 for all but `Transparent`.

AliceBlue #F0F8FF, AntiqueWhite #FAEBD7, Aqua #00FFFF, Aquamarine #7FFFD4, Azure #F0FFFF, Beige #F5F5DC, Bisque #FFE4C4, Black #000000, BlanchedAlmond #FFEBCD, Blue #0000FF, BlueViolet #8A2BE2, Brown #A52A2A, BurlyWood #DEB887, CadetBlue #5F9EA0, Chartreuse #7FFF00, Chocolate #D2691E, Coral #FF7F50, CornflowerBlue #6495ED, Cornsilk #FFF8DC, Crimson #DC143C, Cyan #00FFFF, DarkBlue #00008B, DarkCyan #008B8B, DarkGoldenrod #B8860B, DarkGray #A9A9A9, DarkGreen #006400, DarkKhaki #BDB76B, DarkMagenta #8B008B, DarkOliveGreen #556B2F, DarkOrange #FF8C00, DarkOrchid #9932CC, DarkRed #8B0000, DarkSalmon #E9967A, DarkSeaGreen #8FBC8B, DarkSlateBlue #483D8B, DarkSlateGray #2F4F4F, DarkTurquoise #00CED1, DarkViolet #9400D3, DeepPink #FF1493, DeepSkyBlue #00BFFF, DimGray #696969, DodgerBlue #1E90FF, Firebrick #B22222, FloralWhite #FFFAF0, ForestGreen #228B22, Fuchsia #FF00FF, Gainsboro #DCDCDC, GhostWhite #F8F8FF, Gold #FFD700, Goldenrod #DAA520, Gray #808080, Green #008000, GreenYellow #ADFF2F, Honeydew #F0FFF0, HotPink #FF69B4, IndianRed #CD5C5C, Indigo #4B0082, Ivory #FFFFF0, Khaki #F0E68C, Lavender #E6E6FA, LavenderBlush #FFF0F5, LawnGreen #7CFC00, LemonChiffon #FFFACD, LightBlue #ADD8E6, LightCoral #F08080, LightCyan #E0FFFF, LightGoldenrodYellow #FAFAD2, LightGray #D3D3D3, LightGreen #90EE90, LightPink #FFB6C1, LightSalmon #FFA07A, LightSeaGreen #20B2AA, LightSkyBlue #87CEFA, LightSlateGray #778899, LightSteelBlue #B0C4DE, LightYellow #FFFFE0, Lime #00FF00, LimeGreen #32CD32, Linen #FAF0E6, Magenta #FF00FF, Maroon #800000, MediumAquamarine #66CDAA, MediumBlue #0000CD, MediumOrchid #BA55D3, MediumPurple #9370DB, MediumSeaGreen #3CB371, MediumSlateBlue #7B68EE, MediumSpringGreen #00FA9A, MediumTurquoise #48D1CC, MediumVioletRed #C71585, MidnightBlue #191970, MintCream #F5FFFA, MistyRose #FFE4E1, Moccasin #FFE4B5, NavajoWhite #FFDEAD, Navy #000080, OldLace #FDF5E6, Olive #808000, OliveDrab #6B8E23, Orange #FFA500, OrangeRed #FF4500, Orchid #DA70D6, PaleGoldenrod #EEE8AA, PaleGreen #98FB98, PaleTurquoise #AFEEEE, PaleVioletRed #DB7093, PapayaWhip #FFEFD5, PeachPuff #FFDAB9, Peru #CD853F, Pink #FFC0CB, Plum #DDA0DD, PowderBlue #B0E0E6, Purple #800080, Red #FF0000, RosyBrown #BC8F8F, RoyalBlue #4169E1, SaddleBrown #8B4513, Salmon #FA8072, SandyBrown #F4A460, SeaGreen #2E8B57, SeaShell #FFF5EE, Sienna #A0522D, Silver #C0C0C0, SkyBlue #87CEEB, SlateBlue #6A5ACD, SlateGray #708090, Snow #FFFAFA, SpringGreen #00FF7F, SteelBlue #4682B4, Tan #D2B48C, Teal #008080, Thistle #D8BFD8, Tomato #FF6347, Transparent #FFFFFF (A = 0), Turquoise #40E0D0, Violet #EE82EE, Wheat #F5DEB3, White #FFFFFF, WhiteSmoke #F5F5F5, Yellow #FFFF00, YellowGreen #9ACD32

### A.6 `Turbine.UI.EdgeAttachmentType` (new in this build)

None = 0, Center = 1, Mirror = 2, Proportional = 3, Opposite = 4, Same = 5, RatioProportional = 6

### A.7 `Turbine.UI.Lotro.UIScalingGroupType` (new in this build, 57 values)

None = 0, QuickslotBars = 1, PetQuickslotBar = 2, PetVitals = 3, OpponentVitals = 4, Radar = 5, Toolbar = 6, QuestTracker = 10, Vitals = 11, Alerts = 14, FellowshipVitals1 = 15, FellowshipVitals2 = 16, FellowshipVitals3 = 17, FellowshipVitals4 = 18, ClassAbilities = 22, MountDisplay = 23, SoldierVitals = 24, EscortVitals = 25, AllegianceProgress = 26, InventoryBags = 27, Character = 29, Options = 30, Panels = 31, BarberShop = 32, Tooltips = 33, Deeds = 34, TraitTree = 36, HobbitPresents = 37, Wallet = 38, ItemWear = 39, MountedCombat = 40, BigBattles = 41, Skirmish = 43, Mail = 44, Chat = 45, DressingRoom = 46, Help = 47, Webstore = 48, Vault = 49, Inspect = 51, Wardrobe = 52, Barter = 53, ProgressBar = 54, Menus = 55, HudText = 56, Auction = 57, QuestBestow = 58, Global = 60, Pregame = 61, MapQuestGuideFrame = 62, QuestActionButton = 63, Tutorials = 64, TargetProgressBar = 65, Map = 66, TargetAssist = 67, FellowshipManoeuvres = 70, LootRoll = 71

---

## Appendix B. Implementation notes from `crates/script` (clean room, 2026-10-04), answered

Choices the first implementation made where the first draft was silent, with the answer from
this revision. Implementers should re-check the affected code.

| Implementation choice | Answer (this revision) |
|---|---|
| `PluginData.Load(scope, key, handler)` with data available: return the value **and** call the handler on the next pump | Original: during loading, `Load` calls a function `handler(value)` **immediately**, then returns the value; outside loading, the handler is required and is called later, and `Load` returns nothing (§10.1). Change: call the handler synchronously in the loading case. |
| `PluginData.Save` with a fourth argument that is neither nil nor callable: ignore it | Correct: a non-function `onDone` is ignored (§10.1). A validation or serialisation failure raises an error whether or not `onDone` is given. |
| Rows of `GetAvailablePlugins()` / `GetLoadedPlugins()`: `Name`, `Author`, `Version`, `Description` (when present), plus `ScriptState` on loaded rows only | Original: both have `Name`, `Author`, `Version`, `Package`, `ScriptState`; available rows also `Description` and `Image` (§6.7). |
| `LoadPlugin`, `UnloadScriptState`, `RefreshAvailablePlugins` queued until after the current script call | `LoadPlugin` is queued (§6.2). `UnloadScriptState` is immediate but refused for an apartment with running Lua code (§6.3); queuing it is still acceptable for Tessera as long as a self-unload is refused. `RefreshAvailablePlugins` reruns discovery (§6.7); immediate or deferred is not observable to plugins except through `GetAvailablePlugins`. |
| `Shell.WriteLine` echo to `Chat.Received` with `Sender = ""`, on the next pump | Original posts the line as chat type 8; delivery to `Received` is OPEN (§11.1). Tessera: deliver with `ChatType = 8` and **no** `Sender` field (system lines have none, §9.3), never re-entrantly. |
| Default `GetHelp()` / `GetShortHelp()` return nil | Original returns the empty string `""` (§11.2). Change. |
| `Execute(command, arguments)`: case preserved, one separating whitespace removed, trailing whitespace kept, nil when nothing follows; `plugins` reserved; `/plugin` not an alias | `arguments` is `""` (never nil) when nothing follows (§11.2). Change. The rest stays (whitespace and case OPEN). |
| `GetScriptVersion()` returns 0, 0 | Original: 13, 0 (§12). Change. Default `GetDate()` without time-zone data: the original uses local time; `DayOfWeek` 1 = Sunday and 1-based `DayOfYear` are confirmed. |
| Strip a UTF-8 byte-order mark at the start of a `.lua` file | The original (stock 5.1.4 loader) does not strip it, so such a file fails there. Tessera may keep stripping it: a deliberate leniency that makes no working plugin fail. |
| Instruction budget (500 million per host-to-Lua call, coroutines included), 64 MiB per apartment | The original has neither (§3.3); keep both as deliberate safety deviations. |
| Vendored PUC-Rio Lua 5.1.5 | The original is 5.1.4 (P1). The 5.1.5 changes are bug fixes; none is known to change behaviour plugins rely on. |
| Static functions ignore a leading argument equal to their own class table | The original does not (§8.1). Change to positional arguments, or keep as a deliberate leniency only for zero-parameter statics (indistinguishable from the original there). |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-04 | First draft (observations made 2026-10-03). |
| 2026-10-05 | Revision from method M4 (BIN, client 4906.0071.1794.4035, decision 0011) and new M1 observations (O4, O5); status Draft → Review. Of the 63 OPEN callouts of the first draft, 22 callouts remained (listed in §19; first stated as 23, which counted the §19 heading). New or changed: sandbox removals and `debug` hiding, stock 5.1.4 loader behaviour, no memory/time limits in the original (§3); discovery depth and duplicates (§4.3); descriptor rules and limits (§5.2); `PluginOptions.xml` structure (§5.4); load/unload sequences, `plugin` reset, deferred `LoadPlugin`, immediate `UnloadScriptState` (§6); `import` mechanics, folder-first resolution, run-once, native/disk merge, `_` package-name field (§7); positional arguments, list indices, UTF-8 (§3.4, §8); dispatcher and argument tables, wheel direction (§9); plugin-data timing, key rule (64), path, exact file format, saveable values (§10); shell rules and defaults (§11); engine values (§12); Control min/max sizes and edge attachments, Window scaling, markup tags, Color constructor, image errors (§13); action IDs and enum values against game data, shortcut data formats (§14, §15); money order (§15); inventory of this build (§16); new enums (Appendix A.6, A.7); answers to Appendix B; corrections to `plugin-descriptor.md` and `keymap.md` (§20). Corrections to this spec's first draft: key limit 63 → 64, resolution order (file-first → folder-first), `plugin` stays set → reset to nil, `Color()` default, `GetMoneyComponents` order, `LuaScriptLog` is not an event, `ComboBox`/`StoutAxeAttributes` not registered, `LotroPluginManager` is not a separate table. |
| 2026-10-05 | Revision answering the review of 2026-10-05 (status stays Review). **Accuracy:** named colours are byte/255 rounded to 3 decimals and stored as 32-bit floats, not byte/255 (§13.9, A.5 note, invariant 19, P45; BIN 564/564 components, S3 423/423). **Overclaim:** the plugin-data key rule and the package-name rule are the C runtime's `isalnum` (plus `_`) in a locale the client sets from its game language, not ASCII-only; ASCII-only is now a stated Tessera deviation (§7.2, §10.2, invariant 12, P5, P14, P43, P44, §20 D14 row); the plugin-data decimal separator follows that game-language locale (§3.4, §10.4). **Minor:** O4 counts corrected (input actions 471 = 290 + 181; language mapper base 0x23000186 and chat-type base 0x230002A9 added; key 0 present in every chain); §15 table split into names / non-zero values / keys; §9.3 no longer calls the owner's keymap the default; sources or marks added for §13.1, §13.3 `Activate`, §13.10 `ContextMenu`, §14.2 and §14.5 (P46, Reported, from S2/S3), with unsourced claims removed or moved into OPEN callouts (opacity of children §13.1, close button §14.2, control interaction §14.5); naming note cites `LEGAL.md` §4.1 (*Google v. Oracle* row) and the pending ARCHITECTURE §18.2 legal item instead of `LEGAL.md` §10; `GetScriptVersion` = 13, 0 now traced (P35). OPEN callouts: 25 (22 before, plus §7.2 non-ASCII classification, §13.1 opacity, §14.5 control interaction); listed in §19. |
