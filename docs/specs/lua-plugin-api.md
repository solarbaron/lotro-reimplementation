# Lua plugin API (compatible plugin surface)

| Field | Value |
|---|---|
| Status | **Draft** |
| Last updated | 2026-10-04 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | — (unreviewed) |
| Implements into | `crates/script` (Lua host, sandbox), `crates/plugins` (discovery, apartments, API surface), `crates/ui` (controls) |
| Related specs | dat container spec (block header, DID routing), `render-surface.md` (0x41 images), `ui-layout.md` (native panels), `enum-mapper.md` |

> **Naming note (interoperability).** The namespace, class, method, event, field and enum
> member names in this document are the identifiers that existing third-party plugins call.
> They are reproduced only because a plugin cannot run without them (see
> `docs/ARCHITECTURE.md` §18 and the OPEN (legal) item there). They must appear in Tessera
> only inside the Lua API surface, never as product names, crate names or UI strings.
> Descriptive text in this spec is our own summary; no documentation prose is reproduced.

---

## 1. Purpose

This spec lets an implementer build a Lua 5.1 plugin host that runs existing user-interface
plugins written for the original client, without reading any third-party code. It covers:

- where plugins live on disk and how the `.plugin` XML descriptor is read (§4, §5);
- the apartment (script-state) model and the plugin load/unload lifecycle (§6);
- the `import` mechanism and the per-package global environments (§7);
- the native object model that Lua code subclasses (§8);
- the event model (§9);
- plugin-data persistence (§10);
- the shell/chat, engine, UI and gameplay namespaces (§11–§15);
- a complete API inventory with an implementation tier for each class (§16, §17);
- the sandbox: which Lua standard libraries exist (§3).

Out of scope: UI skinning (`SkinDefinition.xml`), the music (ABC) system, third-party plugin
managers' own files, and network-side semantics of gameplay values (those come from the
session layer).

---

## 2. Provenance

Source keys used below:

| Key | Source |
|---|---|
| **S1** | `research/notes/engine-client-re.md` §5 (dirty-room notes) |
| **S2** | Official "Update 25" Lua API reference, HTML, © 2016 publisher, lotrointerface.com file 1054 (version 2500.0062.0883); mirrored in the `lunarwtr/lotro-api-docs` repository (`U25/`) |
| **S3** | `lunarwtr/vscode-lotro-api` (MIT): `lotro-docs/TurbineDump.json` (a dump of the live in-game `Turbine` table, v1.0.18, 2026-03), `TurbineEventsDump.json` (event argument tables captured in game), `TurbineDumper.lua` (a lightly edited `.plugindata` file), `resources/xsds/lotroplugin.xsd`, EmmyLua stubs generated from S2 |
| **S4** | The publisher's 2010 beta sample plugins (`Turbine/` utility package and `TurbinePlugins/`), archived as `dist/Beta_LuaPlugins.zip` in `Goldbishop/Lotro_LUA`; S2 also mirrored there (U24) |
| **S5** | lotro-wiki.com pages "Lua Plugins", "Lua Scripting", "Plugin manager" (Wayback copies), which quote the 2010–2011 developer forum posts; lotrointerface.com file page 1054 |
| **O1** | Observed in user install 2026-10-03, static inspection of files only (no disassembly, no execution): `lua51.dll` (32- and 64-bit) version string and export table; `x64/lotroclient64.exe` and `lotroclient.exe` PE import tables and literal string table |
| **O2** | Observed in the user's own settings folders 2026-10-03 (read-only): the `Plugins/` tree holding two installed third-party plugins (descriptors, file names, and their API call patterns read statically, nothing copied), and `PluginData/<account>/<server>/PluginOptions.xml` |
| **O3** | Observed in user install data files 2026-10-03 (read-only probes): font records `0x42xxxxxx` in `client_local_English.dat` and `client_general.dat`; image records `0x41xxxxxx` in `client_surface.dat` |

> **Process note.** O1 reads literal strings and the import table of the client executable.
> That is not disassembly (CLEAN_ROOM.md M4) but it is also not a data file (M1). The
> reviewer should confirm this is acceptable; every O1 fact below is restated as a
> functional fact, and no client strings, keys or tables are reproduced beyond identifier
> names needed for interoperability.

| # | Fact or group of facts | Sources | Mark |
|---|---|---|---|
| P1 | Embedded Lua is version 5.1, precisely 5.1.4, in both the 32- and 64-bit clients | O1 (version string in both `lua51.dll`), S1 | **Confirmed** 2026-10-03 |
| P2 | The client opens exactly these standard libraries: base (which in 5.1 includes `coroutine`), `string`, `table`, `math`, `debug`. It does **not** open `io`, `os` or `package` | O1 (the client imports only `luaopen_base`, `luaopen_string`, `luaopen_table`, `luaopen_math`, `luaopen_debug` from `lua51.dll`; `luaL_openlibs` is not imported) | **Confirmed** 2026-10-03 (corrects S1, which omitted `debug`) |
| P3 | The globals `dofile`, `require`, `module` are removed from plugin states | O1 (the three names sit together, next to the root namespace name, in the client's Lua-host string block); S1 | Reported (inferred from O1) |
| P4 | `.plugin` descriptor element and attribute names: `Plugin`, `Information`, `Name`, `Author`, `Version`, `Description`, `Image`, `Package`, `Configuration`, `Apartment` | O1 (client string table), O2 (three real descriptors), S3 xsd, S4 | **Confirmed** 2026-10-03 |
| P5 | Package paths are dotted, relative to the `Plugins` folder; the last component names a `.lua` file, or a folder whose `__init__.lua` is run | S1, S3 xsd, S4, O1 (`__init__.lua` and `.lua` strings) | **Confirmed** (strings) / Reported (algorithm) |
| P6 | Package and file lookup is case-insensitive (a real plugin's descriptor names `….Main` while the file on disk is `main.lua`) | O2 | **Confirmed** 2026-10-03 |
| P7 | Native package names are `Turbine`, `Turbine.Gameplay`, `Turbine.Gameplay.Attributes`, `Turbine.UI`, `Turbine.UI.Lotro` | O1, S2, S3 | **Confirmed** 2026-10-03 |
| P8 | Each file loaded through `import` runs with its package namespace table as its global environment; names fall back to the real globals | S4 (two sample files define the same bare name `Table` in different packages; a sample helper writes `_G.class` explicitly), O2 (a real plugin copies its globals into `_G` "for other modules") | Reported (inferred, consistent across S4 and O2) |
| P9 | The global `Plugins` maps a plugin's `Name` to its `Turbine.Plugin` object; the global `plugin` is the plugin object of the plugin being loaded | O1 (both names in the API string block), O2 (real plugin reads `Plugins["<its name>"]:GetVersion()` and assigns `plugin.GetOptionsPanel`) | **Confirmed** 2026-10-03 (corrects S2, which names the table `__plugins`) |
| P10 | Apartments are separate Lua states; plugins without `Configuration` share one default apartment; unloading works per apartment; a state cannot unload itself | S2 (`UnloadScriptState`), S3 xsd, S5, O2 (a real plugin ships separate "reloader"/"unloader" plugins in their own apartments for exactly this reason) | Reported; self-unload rule corroborated by O2 |
| P11 | Default apartment name is `Default` | O1 (string adjacent to `Apartment` in the descriptor-parsing string block) | Reported (inferred) |
| P12 | Font enum values are DIDs of type `0x42`; all 48 non-zero values exist as records; each record's payload starts with its own DID then the point size, which equals the number at the end of the enum name | O3 (45 records in `client_local_English.dat`, 3 in `client_general.dat`), S3 dump | **Confirmed** 2026-10-03 |
| P13 | Image DIDs that plugins pass to `SetBackground` (e.g. `0x41007DEA`) are render-surface records in `client_surface.dat` | O3, O2 (DIDs used by a real plugin), S2 (`SetBackground` accepts "an id") | **Confirmed** 2026-10-03 |
| P14 | Plugin-data keys must be 1 to 63 characters, are checked for invalid characters, and are stored as `<key>.plugindata` files; account scope uses a folder `AllServers`, server scope a folder `AllCharacters` | O1 (validation and path strings), O2 (`PluginData/<account>/<server>/` exists), S1, S3 | **Confirmed** (rules exist) / Reported (exact path assembly) |
| P15 | `.plugindata` files contain a Lua table constructor in text form, tab-indented, CRLF line ends | S3 (`TurbineDumper.lua`, with the leading `return` replaced by an assignment), O1 (`return ` and `%.14g` strings in the serializer block) | Reported |
| P16 | Event handlers are fields on objects; a field may hold one function or an array of functions (multicast) | S2, O2 (an actively maintained installed plugin builds handler arrays and relies on the client calling each), O1 (a "delegate still has handlers" diagnostic) | Reported (strong) |
| P17 | `ShellCommand:Execute` is called with the command name and then the argument text (S2 documents only one parameter) | S4 (all samples), O2 (installed plugin) | Reported; corrects S2 |
| P18 | `Plugin:GetConfiguration` (documented in S2) is absent from the live client; `Shell.Execute` (used by a 2010 sample) is absent | S3 dump (not present), O1 (no such identifier strings) | **Confirmed** absent 2026-10-03 (corrects S1/S2) |
| P19 | Member lists, signatures and enum values in §11–§16 and Appendix A | S3 dump (live names and enum values), S2 (signatures, parameter types) | Reported (names and values come from a live dump) |
| P20 | `Turbine.UI.Lotro.Action` member names share the client's key-binding action namespace | O2 (41 of the 221 action names in the user's own keymap file are `Action` members, e.g. `CaptureScreenshot`, `CameraInstantMouseLook`), S3 | **Confirmed** (partial overlap) 2026-10-03 |
| P21 | API identifiers newer than S2/S3: `GetUsesGlobalScaling`, event `UIScaleChanged`, types `UIScalingGroupType`, `EdgeAttachmentType`, `ComboBox`, event `LuaScriptLog` | O1 (identifier strings only) | Reported (existence only; semantics OPEN) |
| P22 | Plugins cannot issue game commands or automate input | S5 (developer statement), P18 | Reported |
| P23 | `PluginOptions.xml` per account and server holds the plugin manager's autoload and scaling choices | O2 (file and root element), O1 (`PluginOptions`, `AutoloadOptions`, `ScalingOptions`, `UseScaling`, `AllCharacters` strings) | **Confirmed** (file, root, empty `AutoloadOptions`) / Reported (inner structure) |

---

## 3. Runtime and sandbox

<a id="3-1"></a>
### 3.1 Lua version

- Lua **5.1** semantics (P1). The original uses the reference 5.1.4 interpreter: `number` is a
  C `double`, strings are 8-bit clean, `#` is the 5.1 border operator, `table.getn`,
  `table.setn`, `math.mod`, `string.gfind`, `unpack`, `setfenv`/`getfenv` and `module`-era
  behaviour are the 5.1 ones. Real plugins call `table.getn`, `getfenv(0)` and `unpack`
  (O2), so these must exist.
- A vendored PUC-Rio Lua 5.1.x is the compatible choice. LuaJIT is close but differs in
  corner cases (e.g. `#` on tables with holes); not recommended for Milestone 1.

<a id="3-2"></a>
### 3.2 Standard libraries

| Library / global | Original client | Tessera requirement |
|---|---|---|
| base (`print`, `pairs`, `ipairs`, `next`, `type`, `tostring`, `tonumber`, `select`, `unpack`, `pcall`, `xpcall`, `error`, `assert`, `rawget`, `rawset`, `rawequal`, `setmetatable`, `getmetatable`, `getfenv`, `setfenv`, `collectgarbage`, `gcinfo`, `newproxy`, `_G`, `_VERSION`) | opened (P2) | Provide. `print` output: OPEN (route to the script log, §12). |
| `coroutine` | opened as part of base (P2) | Provide. |
| `string`, `table`, `math` | opened (P2) | Provide in full. |
| `debug` | **opened** by the client (P2) | Whether the `debug` table stays visible to plugins is OPEN. Recommended: do not expose it in Milestone 1; `Turbine.Engine.GetCallStack` (§12) covers the common use. |
| `io`, `os`, `package` | **not opened** (P2) | Must be absent (`nil`). No file or process access from Lua. |
| `dofile`, `require`, `module` | removed (P3) | Must be `nil`. |
| `loadfile` | unknown | Must be `nil` (no raw file access; Tessera rule). |
| `loadstring`, `load` | unknown | > **OPEN:** whether plugins can compile source at run time. Recommended: provide `loadstring`/`load` for **text** chunks only (reject chunks starting with the bytecode signature `ESC 'L' 'u' 'a'`), with the calling apartment's globals as the environment. |
| `import` | added by host (§7) | Provide. |

> **OPEN:** whether `string.dump`, `collectgarbage("count")` and `gcinfo` behave normally,
> and whether `debug` is pruned after opening.

<a id="3-3"></a>
### 3.3 Limits

- The client creates states with a custom allocator and reports "out of memory" script errors
  (O1), so each apartment has a memory budget. > **OPEN:** the budget size and whether it is
  per state or global. Recommended: configurable per-apartment cap (default 64 MiB) plus an
  instruction-count hook that aborts a single callback after a configurable budget.
- All plugin code runs on the main (UI) thread, between frames (Tessera architecture §18.1).
- Error handling: every host-to-Lua call is protected; an error in one callback must never
  abort the frame or affect other apartments (§6.6).

<a id="3-4"></a>
### 3.4 Numbers, strings and locale

- Lua numbers are doubles; DIDs (e.g. `0x41007DEA`) and enum values up to `0x7FFFFFFF` are
  represented exactly.
- Text passed to and from the API is a byte string. > **OPEN:** encoding. Recommended: UTF-8
  on both sides (non-ASCII names in chat and item names must survive a round trip).
- The original serializer is reported to write numbers in a locale-dependent way, which
  broke data written by German/French clients (S3 shows integer values written both plainly
  and as `9.000000`). Tessera writes locale-independent numbers (§10.4) and reads both forms.

<a id="3-5"></a>
### 3.5 Anti-automation boundary

Plugins can display information and offer **user-clicked** shortcuts (Quickslot, §14.6) but
cannot themselves: execute chat/slash commands, use skills or items, move the character, or
send keystrokes (P18, P22). Tessera keeps this boundary: no API that converts a script call
into a game action without a user input event on a plugin control.

---

## 4. Files and folders

<a id="4-1"></a>
### 4.1 User folder

The original client's per-user folder is `Documents/The Lord of the Rings Online/` (S1, S5).
Observed contents (O2): `UserPreferences.ini`, `lotro.keymap`, `Plugins/`, `PluginData/`,
`Music/`, `ui/`. The client does not create `Plugins/`; the user does (S5).

Tessera reads an existing original `Plugins/` folder **read-only** (Architecture §18.2) and
writes its own plugin data under its own `data/` directory (§10.7).

<a id="4-2"></a>
### 4.2 `Plugins/` layout

| Path pattern | Meaning |
|---|---|
| `Plugins/<Folder>/<Any>.plugin` | Descriptor (§5). Convention: `<Folder>` is the author's folder (S5); real installs observed exactly at this depth (O2). |
| `Plugins/<Folder>/<Name>/…` | The plugin's Lua files and resources, addressed by dotted package paths. |
| `Plugins/<Folder>/<Any>.plugincompendium` | Third-party manager metadata (id, name, version, download URL, list of descriptor paths, dependency ids). The client ignores it (O1: no such strings). Tessera ignores it. |
| `Plugins/Turbine/…`, `Plugins/TurbinePlugins/…` | The publisher's sample utility package and samples, installed by users when an older plugin needs them (S5). They are ordinary user content, **not** part of the client. Tessera must not ship them. |

<a id="4-3"></a>
### 4.3 Discovery algorithm

1. Enumerate `*.plugin` files under `Plugins/`. Observed descriptors are exactly one folder
   below `Plugins/` (O2, S5). > **OPEN:** whether the original also scans `Plugins/` itself
   and deeper levels. Recommended: scan depth 1 and 2, report others as ignored.
2. Parse each descriptor (§5). Skip, with a logged warning, any descriptor that fails
   validation.
3. The list is built at client start and on `/plugins refresh` or
   `PluginManager.RefreshAvailablePlugins()` (S2, S5). Refresh does not unload plugins whose
   descriptor vanished; they just stop being listed (S2).
4. Name collisions: > **OPEN:** behaviour when two descriptors share a `Name`. Recommended:
   first in case-insensitive path order wins, others are reported.

<a id="4-4"></a>
### 4.4 Path rules

- All plugin paths (packages, images, descriptors' `Image`) are relative to `Plugins/`.
  Packages use `.` as the separator; resource paths use `/` (S2, S4) and must also accept `\`.
- Resolution is **case-insensitive** (P6). On case-sensitive file systems Tessera must match
  each component case-insensitively (pick the first match in byte order if several exist).
- No component may be empty, `.` or `..`; absolute paths, drive letters and NUL bytes are
  rejected. Resolution never leaves `Plugins/` (Architecture §18.2), including through
  symlinks.

---

## 5. The `.plugin` descriptor

<a id="5-1"></a>
### 5.1 Elements

An XML 1.0 document. Observed files carry `<?xml version="1.0"?>` with no encoding
declaration, tab or space indentation, and multi-line text (O2).

| Element path | Occurs | Content | Meaning |
|---|---|---|---|
| `/Plugin` | 1 | element | Root. |
| `/Plugin/Information` | 1 | element | Metadata block. |
| `/Plugin/Information/Name` | 1 | text | Plugin name: the key for `/plugins load`, `PluginManager.LoadPlugin`, `Plugins[...]` and the manager list. May contain spaces (O2). |
| `/Plugin/Information/Author` | 1 | text | Informational; returned by `Plugin:GetAuthor()`. |
| `/Plugin/Information/Version` | 1 | text | Free-form string (e.g. `1.0`, `v1.50`, `2.6.2`); returned by `Plugin:GetVersion()`. |
| `/Plugin/Information/Description` | 0..1 | text | Shown in the plugin manager; may span many lines (O2). |
| `/Plugin/Information/Image` | 0..1 | text | Path (relative to `Plugins/`, `/`-separated) of a JPG or TGA icon. Shown 32×32: larger images are cropped, smaller ones tiled (S3 xsd). |
| `/Plugin/Package` | 1 | text | Dotted package path of the main file (§7). |
| `/Plugin/Configuration` | 0..1 | element | Present only to pick an apartment. Its content is ignored. |
| `/Plugin/Configuration/@Apartment` | 1 if parent present | attribute | Apartment name (§6.1). |

<a id="5-2"></a>
### 5.2 Parsing rules

1. Bounded, non-validating parse: reject files over 64 KiB, DTDs and external entities.
2. Leading and trailing whitespace of each text value is trimmed. Internal newlines in
   `Description` are kept.
3. Comments are allowed and ignored (a sample comments out its `Configuration`, S4).
4. Unknown elements are ignored. > **OPEN:** whether the original rejects them.
5. Encoding: no declaration means UTF-8; accept a UTF-8 BOM. > **OPEN:** whether the original
   treats undeclared bytes as Latin-1.
6. Missing `Name` or `Package`, or an empty value after trimming, makes the descriptor
   invalid.

<a id="5-3"></a>
### 5.3 Invariants

- `Name` is non-empty and contains no control characters.
- `Package` matches: one or more components separated by single dots; each component is
  non-empty and contains no `/`, `\`, `:` or NUL.
- If `Configuration` exists, `@Apartment` is non-empty.

<a id="5-4"></a>
### 5.4 Plugin-manager options file (`PluginOptions.xml`)

The original in-game plugin manager stores its choices per account and server at
`PluginData/<account>/<server>/PluginOptions.xml` (O2). Observed content: XML declaration
with `encoding="UTF-8" standalone="no"`, root `PluginOptions`, one empty `AutoloadOptions`
element. Other names present in the client (P23): `ScalingOptions`, `Plugin`, `Name`,
`UseScaling` (`True`/`False`), `Character`, `AllCharacters`.

> **OPEN:** the inner structure of `AutoloadOptions` and `ScalingOptions` (which of those
> names are elements or attributes). Tessera does not need to read this file; it keeps its
> own autoload list. An importer may be added once a populated sample is observed.

---

## 6. Apartments and plugin lifecycle

<a id="6-1"></a>
### 6.1 Apartments

- An **apartment** (also "script state") is one Lua state (P10). Every plugin runs in
  exactly one apartment.
- A plugin whose descriptor has `Configuration/@Apartment = X` runs in apartment `X`. All
  descriptors naming the same `X` share that state, and therefore share globals, native
  singletons (§9.5) and class-level event handlers.
- A plugin with no `Configuration` runs in the default apartment, reported to be named
  `Default` (P11). > **OPEN:** confirm the name (visible through
  `PluginManager.GetLoadedPlugins()[i].ScriptState`).
- Apartment names compare case-insensitively. > **OPEN:** confirm.
- Unloading is per apartment: every plugin in it goes (S3 xsd, S2).

<a id="6-2"></a>
### 6.2 Loading a plugin

Triggered by `/plugins load <Name>`, `PluginManager.LoadPlugin(name)`, the plugin manager
UI, or autoload at character login.

1. Find the available plugin by `Name` (case-insensitive; OPEN). If none, report and stop.
2. If a plugin with that name is already loaded, report and stop (no reload).
3. Get the apartment; if it does not exist, create it:
   1. create a Lua state with the libraries of §3.2;
   2. install `import` (§7) and the native packages (§7.4);
   3. create an empty global table `Plugins`.
4. Create a `Turbine.Plugin` object holding the descriptor's name, author and version.
   Set `Plugins[<Name>]` to it and set the global `plugin` to it (P9).
5. Run `import(<Package>)` inside the apartment (§7.2). Top-level code of the main file and
   everything it imports runs now.
6. If step 5 raised an error, report it (§6.6). > **OPEN:** whether the failed plugin stays
   in `Plugins`/loaded lists. Recommended: remove it from both and fire no `Load`.
7. Fire the `Load` event on the plugin object (§9) after the main package returned (S2).
8. If the plugin defines `GetOptionsPanel` (§6.5), the manager may call it from now on.

> **OPEN:** the order in which autoloaded plugins load, and whether `plugin` is reset after
> step 7. Recommended: autoload in the order stored in Tessera's autoload list; leave
> `plugin` pointing at the most recently loaded plugin of that apartment.

<a id="6-3"></a>
### 6.3 Unloading an apartment

Triggered by `/plugins unload [<apartment>]`, `PluginManager.UnloadScriptState(name)`, the
manager's "unload" action, logout, or client exit.

1. A state may not unload **itself**: a call to `UnloadScriptState` naming the caller's own
   apartment is refused (S2). Real plugins work around this with a helper plugin in another
   apartment (O2). > **OPEN:** whether refusal is silent or raises an error. Recommended:
   no-op plus a script-log warning.
2. Fire `Unload` on every plugin object of the apartment, in load order (OPEN).
3. Complete any plugin-data saves queued by `Unload` handlers before the state closes
   (real plugins save in `Unload`, O2). This is an invariant (§18).
4. Destroy all windows and controls the apartment created, remove its shell commands (§11)
   and its context menus, cancel its drag operations.
5. Remove its plugins from the loaded list, then close the Lua state.

`/plugins unload` with no argument unloads all apartments (S5).

<a id="6-4"></a>
### 6.4 Chat command `/plugins`

| Sub-command | Effect | Source |
|---|---|---|
| `load <Name>` | §6.2 | S5 |
| `unload [<apartment>]` | §6.3 | S5, S3 xsd |
| `list` | Writes the available and loaded plugins to chat | S5 |
| `refresh` | Rescans `Plugins/` (§4.3) | S5 |
| `manager` | Opens the plugin manager window | S5 |

> **OPEN:** whether `/plugin` (singular) is an alias, and the exact output text (Tessera
> uses its own wording).

<a id="6-5"></a>
### 6.5 Options panel

The manager shows a plugin's options when the plugin object has a field `GetOptionsPanel`
that is a function (S3 stubs, O1, O2). The manager calls it as `GetOptionsPanel(pluginObject)`
and expects a `Turbine.UI.Control`; it re-parents that control into its own options area
and sizes it. `PluginManager.ShowOptions(pluginObject)` opens the manager on that panel
(S2, S3). > **OPEN:** whether the call is made once or every time the tab opens.
Recommended: once per load, cached.

<a id="6-6"></a>
### 6.6 Error reporting

Any Lua error raised while loading, in an event handler, in a shell command or in a
plugin-data callback is caught by the host and written to the chat window (S5) and to the
script log (§12). > **OPEN:** exact message format. Tessera format: plugin name, apartment,
the Lua error message, and a traceback (the latter only in the log).

---

## 7. `import` and package environments

<a id="7-1"></a>
### 7.1 Call form

`import` is a global function taking one string, the dotted package name. Both
`import "A.B"` and `import(expr)` forms occur; the name may be built at run time (O2).
> **OPEN:** return value. Recommended: return nothing.

Errors (functional, from O1): calling with no name, failing to resolve the name, failing to
open or load the file, and finding a non-table where a namespace table must go all raise a
Lua error in the caller.

<a id="7-2"></a>
### 7.2 Resolution algorithm

Given the name `N` = `C1.C2.….Ck`:

1. If `N` is a native package (§7.4), make sure that package (and its parents) is installed
   in the apartment's globals and return.
2. Otherwise let `D` be `Plugins/C1/C2/…/Ck` (case-insensitive, §4.4).
   1. If the file `D.lua` exists, it is the chunk to run, and its **namespace** is
      `C1.….C(k-1)` (for `k = 1`, the global table itself).
   2. Else if `D/__init__.lua` exists, that is the chunk, and its namespace is `C1.….Ck`.
   3. Else raise "cannot resolve package" (P5).
3. Make sure the namespace tables exist: walk from the global table through `C1`, `C2`, …,
   creating an empty table for each missing component. If an existing component is not a
   table, raise an error (O1).
4. Load the chunk as **text** Lua source (reject binary chunks). Its chunk name should be the
   relative path, for readable tracebacks.
5. Set the chunk's environment to the namespace table, and make sure the namespace table
   resolves missing names through the apartment's real global table (P8). Assignments to
   bare names inside the file therefore create fields of the namespace table; code reaches
   real globals through `_G`.
6. Run the chunk with no arguments.

Effects implementers must reproduce (P8):

- File `A/B/X.lua` defining a bare global `X` creates `A.B.X`; another file of package
  `A.B` can use it as a bare name.
- Two files in different packages can each define a bare global with the same name without
  clashing.
- A file sees a bare name defined only in `_G` (e.g. `class`, `Turbine`) through the
  fallback.

> **OPEN:** whether the fallback is a metatable `__index` on each namespace table (making
> `A.B.Turbine` readable) or a separate per-chunk proxy. Recommended: a per-chunk
> environment table with `__index` = namespace-then-globals and `__newindex` = namespace,
> so namespace tables stay clean.

<a id="7-3"></a>
### 7.3 Re-import

> **OPEN:** whether importing an already-imported package runs it again. Sample files import
> the same helper from many files (S4), which works either way. Recommended: run each
> resolved chunk at most once per apartment (cache by resolved path); later imports are
> no-ops.

<a id="7-4"></a>
### 7.4 Native packages

| Package | Contents |
|---|---|
| `Turbine` | Core classes and enums of §11, §12, plus `Object`, `Plugin`, `PluginData`, `PluginManager` |
| `Turbine.Gameplay` | §15 |
| `Turbine.Gameplay.Attributes` | Per-class and per-race attribute classes and stance enums |
| `Turbine.UI` | §13 |
| `Turbine.UI.Lotro` | §14 |

Real plugins sometimes use `Turbine.PluginData` after importing only `Turbine.Gameplay` and
`Turbine.UI` (O2), so the core `Turbine` table must be present without an explicit
`import "Turbine"`. Recommended for Tessera: install all native packages when the
apartment is created; importing a native name is then a no-op.

> **OPEN:** if a user folder `Plugins/Turbine/` (the sample utilities) exists, which wins
> for `import "Turbine"` and `import "Turbine.UI"`. Recommended: native names win; other
> names under `Turbine.` (e.g. `Turbine.Class`, `Turbine.UI.Extensions`) resolve to disk.

---

## 8. Object model

<a id="8-1"></a>
### 8.1 Class tables

- Every API class (e.g. `Turbine.UI.Window`) is a plain Lua **table** in its namespace.
  It holds the class's own methods as functions (the live dump lists only each class's own
  methods, S3) and reaches its base class's methods through its metatable's `__index`
  (`Window` → `Control` → `Object`).
- Instantiable classes have a `Constructor` function and are **callable**: `Class(...)`
  creates a new instance (S2 examples, S4).
- Every class table and every enum table has an `IsA` function (S3).
- Static classes (`Engine`, `Shell`, `PluginData`, `PluginManager`, `Display`, `LotroUI`)
  are called with dot syntax and are never instantiated.
- Static functions must tolerate being called with colon syntax: real plugins call
  `Turbine.Gameplay.LocalPlayer:GetInstance()` (O2), which passes the class table as an extra
  first argument. Rule: a static function ignores a leading argument that is its own class
  table.

<a id="8-2"></a>
### 8.2 Instances

- An instance is a Lua **table** whose lookup chain ends at its class table. Plugins store
  their own fields on it and assign event handlers to it (§9).
- The native object behind an instance is bound to the table by the host, not through any
  field a plugin can see. Native methods find their native object from the table passed as
  `self`, whatever that table's metatable is (needed for §8.3).
- Calling a native method with a `self` that has no bound native object of a compatible
  class raises a Lua error (never a crash).
- Identity: when the API returns the same native object twice (e.g. `GetInstance()`,
  `GetParent()`, an `Item` from two calls), it should return the **same** table, so `==`
  works and handler fields stay attached. > **OPEN:** confirm for gameplay objects.
  Recommended: weak-valued cache from native id to table.
- `obj:IsA(C)` is true when `C` is the object's class or one of its bases, native or Lua.

<a id="8-3"></a>
### 8.3 Subclassing from Lua

Plugins subclass native classes using a small Lua-side helper, conventionally a global
function `class(base)` that each plugin (or the sample package) defines itself (S4, O2).
Tessera does **not** provide `class`. What the host must support so those helpers work:

1. A Lua class table `L` whose metatable `__index` is a native class table `N` sees all of
   `N`'s methods (and `N`'s bases') through ordinary indexing.
2. An instance is a fresh table whose metatable `__index` is `L` (owned by the helper, not by
   the host).
3. The Lua constructor calls `N.Constructor(self, ...)` on that table. This must bind a new
   native object of class `N` to `self` **without** replacing `self`'s metatable.
4. After that, native methods called on `self` work (§8.2), events fire on `self`, and
   methods the subclass defines (including overrides of native method names) take priority
   because lookup reaches `L` before `N`.
5. Nested subclassing (Lua class of a Lua class of a native class) works the same way.

Known helper behaviour (functional summary, S4): calling the class table creates the
instance, sets the metatable, then calls `Constructor` with the call's arguments if one
exists; `IsA` walks the chain of `base` links.

<a id="8-4"></a>
### 8.4 Enumerations

An enumeration is a table of name → number (plus `IsA`). Several names may share a value
(e.g. `BlendMode.None` and `BlendMode.Undefined` are both 8). Values are in Appendix A.

<a id="8-5"></a>
### 8.5 Conventions

- Getter/setter pairs: `GetX()`/`SetX(v)`; booleans use `IsX()`/`SetX(b)`.
- Size and position getters return **two** numbers (`GetSize()` → width, height;
  `GetPosition()` → left, top). `GetMoneyComponents()` returns three numbers.
- Indices into API lists (`ControlList`, `ListBox` items, `Backpack`, `EffectList`, …) are
  **1-based**, matching Lua. > **OPEN:** confirm for every list class; `Equipment.GetItem`
  takes a slot enum value instead (Appendix A).
- Colours are `Turbine.UI.Color` objects (§13.9); blend modes, alignments, fonts and so on
  are enum numbers.

---

## 9. Event model

<a id="9-1"></a>
### 9.1 Handlers are fields

An event `E` of an object `O` is not registered through a function: the plugin assigns a
value to the field `O.E` (S2, S4, O2). Examples of forms seen in real code: assigning a
function to a control's `Click`; assigning to the class-level `Turbine.Plugin.Load`, which
then applies to every plugin object in the apartment; defining `function W:MouseDown(args)`
on a Lua subclass.

<a id="9-2"></a>
### 9.2 Dispatch

When the host fires `E` on `O` with argument table `A`:

1. Read `O.E` with **normal** indexing (honouring `__index`, so class-level and
   subclass-level handlers count).
2. If it is `nil` or `false`: do nothing.
3. If it is a function, or a value with a `__call` metamethod: call it as `handler(O, A)`.
4. If it is a plain table without `__call`: call each element `t[1]`, `t[2]`, … up to the
   border, each as `element(O, A)`, skipping non-callable elements (P16).
5. Each call is protected; an error is reported (§6.6) and dispatch continues with the
   next element.
6. Any return value is ignored, except where an event defines a result field in `A`
   (`Window.Closing`'s `Cancel`, §13.3; `DragDropInfo` success).

So the handler's first parameter is the sender (which is `O`); a handler written as a method
(`function obj:Click(args)`) receives `self = O`. Handlers declared with an extra leading
`self` parameter on a dot-assigned class field (`= function(self, sender, args)`, seen in O2)
simply receive `O` and `A` in their first two parameters; this must keep working.

<a id="9-3"></a>
### 9.3 Argument tables

A fresh table per firing. Fields observed in game (S3 events dump) and documented (S2):

| Event | Fields of `args` |
|---|---|
| `Turbine.Chat.Received` | `Sender` (string), `Message` (string), `ChatType` (number, `Turbine.ChatType`) |
| `Control.MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick` | `Button` (`MouseButton` value), `X`, `Y` (pixels, relative to the control) |
| `Control.MouseMove`, `MouseHover` | `X`, `Y` |
| `Control.MouseWheel` | `Direction` (+1 or −1; OPEN which is "up"), `X`, `Y` |
| `Control.KeyDown`, `KeyUp` | `Action` (a `Turbine.UI.Lotro.Action` value, or another key-binding action id), `Alt`, `Control`, `Shift` (booleans) |
| `Control.DragDrop`, `Quickslot.DragDrop`, `EquipmentSlot.DragDrop` | `DragDropInfo` (object, §13.10), `X`, `Y` |
| `Window.Closing` | `Cancel` (boolean, initially false; a handler sets true to keep the window open) |
| `Backpack.ItemAdded` | `Index`, `Item` |
| `Backpack.ItemRemoved` | `Index` |
| `Backpack.ItemMoved` | `OldIndex`, `NewIndex`, `Item` |
| `Equipment.ItemEquipped`, `ItemUnequipped` | `Index` (slot), `Item` |
| `EffectList.EffectAdded`, `EffectRemoved` | `Index` |
| `SkillList.SkillAdded` | `Index`, `Skill` |
| `SkillList.SkillRemoved` | `Index` |
| `ListBox.ItemAdded`, `ItemRemoved` | `Item` (the control) |
| `Party.AssistTargetAdded` | `Player` |
| all other events | empty table |

> **OPEN:** fields for `Bank.*`, `Wallet.*`, `Party.MemberAdded/Removed`, `Effect.*`,
> `ProfessionInfo.Recipe*` (not captured in S3). Recommended: the same pattern
> (`Index`/item object) and an empty table otherwise.

<a id="9-4"></a>
### 9.4 Per-frame update

`Control:SetWantsUpdates(true)` makes the host fire `Update` on that control once per
rendered frame, with an empty `args` (S2). It stays on until set false or the control is
destroyed. > **OPEN:** whether hidden controls still receive `Update`. Recommended: yes
(plugins use it for timers on hidden controls).

<a id="9-5"></a>
### 9.5 Shared singletons

Objects such as `LocalPlayer.GetInstance()`, `Turbine.Chat` and `Turbine.Plugin` (class
level) are shared by every plugin **in the same apartment**; one plugin replacing a handler
removes another's (S2). Different apartments get different tables. Plugins cope by using
handler arrays (§9.2 step 4) or private apartments; the host must support both.

<a id="9-6"></a>
### 9.6 Re-entrancy and ordering

- Events are delivered on the main thread, never during another Lua call of the same state,
  except events directly caused by a script call (e.g. `SetText` firing `TextChanged`, or
  `Close()` firing `Closing`), which are delivered synchronously before the call returns.
  > **OPEN:** confirm which setters fire their own change events.
- A handler that destroys or reparents the sender must not crash the host; remaining
  handlers of that firing still run.

---

## 10. Plugin data (`Turbine.PluginData`)

<a id="10-1"></a>
### 10.1 Calls

| Call | Behaviour |
|---|---|
| `PluginData.Save(scope, key, data [, onDone])` | Serializes `data` and replaces whatever was stored under (`scope`, `key`). If `onDone` is given it is called later as `onDone(ok, message)` with `ok` a boolean and `message` a string when `ok` is false (S2, S3). |
| `PluginData.Load(scope, key [, onLoaded])` | If the data is available now, returns it (a fresh Lua value) — real plugins rely on this synchronous two-argument form (O2). If nothing is stored, returns `nil`. If the data is not available yet, returns `nil` and later calls `onLoaded(data)` (S2). |

`scope` is a `Turbine.DataScope` value: `Account` = 1, `Server` = 2, `Character` = 3.

> **OPEN:** the original also has a "load handler must be a valid function" error (O1). Since
> real plugins call `Load` with two arguments, the check can only apply when a third argument
> is given and is not a function. Tessera: raise if a third argument is present and not
> callable; otherwise accept.

<a id="10-2"></a>
### 10.2 Validation

- `scope` must be 1, 2 or 3; otherwise raise (O1).
- `key` must be a string of 1–63 bytes (O1). It must not contain characters invalid in a
  file name (O1). Tessera rejects bytes `< 0x20`, `/ \ : * ? " < > |`, and keys that are
  `.` or `..` or end in `.` or space. > **OPEN:** the original's exact set.
- Character scope with no character logged in (e.g. at character selection): > **OPEN:**.
  Tessera: raise.

<a id="10-3"></a>
### 10.3 Storage location (original layout)

Under the user folder (§4.1):

| Scope | Folder | Source |
|---|---|---|
| Account | `PluginData/<account>/AllServers/` | O1 (folder name), S1, S3 |
| Server | `PluginData/<account>/<server>/AllCharacters/` | O1 (folder name), O2 (`PluginData/<account>/<server>/` exists) |
| Character | `PluginData/<account>/<server>/<character>/` | S1 |

File name: `<key>.plugindata`. The folder names and the extension are Confirmed as strings;
the assembly of the paths above is Reported.

<a id="10-4"></a>
### 10.4 File format

UTF-8 (OPEN) text: the word `return`, then a Lua table constructor describing the saved value
(P15). Observed layout: one key–value pair per line, keys always in brackets
(`["Name"] =`, numeric keys `[1] =`), strings in double quotes, tab indentation per nesting
level, a nested table opened on its own line after `=`, entries separated by commas (no
comma after the last), CRLF line ends, and an empty table written as an opening and closing
brace with an empty line between.

Reading rule (Tessera): parse the file as a Lua 5.1 expression of the restricted grammar
below, **without executing code**. Accept LF or CRLF, any whitespace, optional trailing
commas, `;` separators, and numbers written as Lua numerals, including a comma decimal
separator inside a bracketed number (old German/French files, §3.4) — OPEN on how such
files actually look.

| Grammar element | Allowed |
|---|---|
| value | `nil`, `true`, `false`, number, string, table |
| number | Lua 5.1 numeral (decimal, exponent, hex `0x…`), optional leading `-` |
| string | double- or single-quoted with Lua 5.1 escapes; long brackets `[[…]]`, `[=[…]=]` |
| table | `{` fields `}`; field = `[` key `] =` value, or `name =` value, or value (array item) |
| key | number or string (not `nil`, not NaN, not table) |

Limits: file size cap (default 16 MiB), nesting depth cap (default 64), entry count cap.

Writing rule (Tessera): emit the layout above; numbers with integral values as integers,
others with 17 significant digits, always `.` as decimal separator; keys sorted (numbers
ascending first, then strings bytewise) for stable diffs.

<a id="10-5"></a>
### 10.5 What can be saved

Booleans, numbers, strings and tables of those, with string or number keys. Saving a
function, userdata, thread, an API object, or a table that contains itself fails: `Save`
reports failure through `onDone(false, message)` (or raises if no callback; OPEN). Metatables
are not saved. NaN and infinities: > **OPEN:**. Tessera: reject.

<a id="10-6"></a>
### 10.6 Timing

- The original writes asynchronously (a scheduler exists, O1). Tessera may write on a worker
  thread but must preserve per-key order, make a later `Load` of the same key in the same
  session see the last `Save`, and flush everything before an apartment closes (§6.3).
- Writes are atomic: write a temporary file in the same folder, then rename over the target.

<a id="10-7"></a>
### 10.7 Tessera storage

Tessera stores plugin data under its own `data/plugindata/` directory using the same
three-level layout (account, server, character names sanitized for the file system), never
in the original client's folders. A one-time read-only import of an original `PluginData/`
tree is allowed.

---

## 11. Shell and chat (`Turbine.Shell`, `Turbine.ShellCommand`, `Turbine.Chat`)

<a id="11-1"></a>
### 11.1 `Turbine.Shell` (static)

| Function | Parameters → result | Behaviour |
|---|---|---|
| `AddCommand(names, cmd)` | string, ShellCommand → number | `names` is one name or several separated by `;`. Registers `cmd` under each name not already taken (by the client or by any plugin); returns how many were registered (S2). Names are typed in chat as `/name`. |
| `RemoveCommand(cmd)` | ShellCommand → number | Unregisters `cmd` from every name it holds. > **OPEN:** meaning of the return value; Tessera returns the number of names removed. |
| `IsCommand(name)` | string → boolean | True if `name` is any known chat command, built-in or plugin. Plugins use this to detect the client language by probing localized built-in command names (S5), so built-in localized names must be answered too. |
| `GetCommands()` | → table of strings | All command names, aliases included (S2 says "list of strings"; the stub's `string` type is wrong). > **OPEN:** array vs other shape; Tessera returns an array. |
| `WriteLine(text)` | string → — | Writes a line to the chat window, on the standard/system channel (S5). Delivered to `Chat.Received` handlers with `ChatType.Standard` (S3 dump shows such an echo). |

Command matching is case-insensitive (OPEN). Registrations of an apartment are removed when
it unloads (§6.3).

<a id="11-2"></a>
### 11.2 `Turbine.ShellCommand`

A class (instantiate with `Turbine.ShellCommand()` or subclass it). The host calls three
methods that the plugin overrides:

| Method | Called as | Expected result |
|---|---|---|
| `Execute` | `cmd:Execute(commandName, argumentText)` (P17) | ignored |
| `GetHelp` | `cmd:GetHelp()` | string (may contain newlines), shown by help |
| `GetShortHelp` | `cmd:GetShortHelp()` | one-line string, shown in command lists |

`argumentText` is the text after the command word with the separating space removed;
`nil` or empty when there are no arguments (S4 code checks both). > **OPEN:** whether
trailing whitespace is trimmed.

<a id="11-3"></a>
### 11.3 `Turbine.Chat`

Static event source: `Turbine.Chat.Received = handler` receives every chat line the client
shows (player chat, tells, system messages) with the fields of §9.3. Message text may contain
the client's chat markup. > **OPEN:** markup grammar in `Message` (links, colour tags).
`Turbine.ChatType` values are in Appendix A.

---

## 12. Engine (`Turbine.Engine`, static)

| Function | Result |
|---|---|
| `GetDate()` | Table with numeric `Year`, `Month`, `Day`, `Hour`, `Minute`, `Second`, `DayOfWeek`, `DayOfYear` and boolean `IsDST`, local time (S2). > **OPEN:** base of `DayOfWeek` (0 or 1, Sunday or Monday) and `DayOfYear`. |
| `GetLocalTime()` | Seconds since 1970-01-01 (S2). |
| `GetGameTime()` | Monotonic seconds (fractional) for delta timing; only differences matter (S2). |
| `GetLanguage()` | A `Turbine.Language` value (Appendix A). |
| `GetLocale()` | Locale string. > **OPEN:** exact values (expected short codes such as `en`, `de`, `fr`). |
| `GetScriptVersion()` | Two numbers: major, minor API version, independent of the client build (S2). > **OPEN:** current values. |
| `GetCallStack(...)` | Traceback string, equivalent to `debug.traceback(...)` (S2). |
| `ScriptLog(message)` | Appends to a log shared by all scripts (S2). Tessera: the `logs/` directory and a `tracing` target. |

---

## 13. User interface (`Turbine.UI`)

<a id="13-1"></a>
### 13.1 Concepts

- **Coordinates:** pixels; a control's position is relative to its parent's top-left; a
  `Window`'s position is relative to the screen (`Display`) (S2 examples).
- **Tree:** a control is shown only inside a `Window` chain (S2); top-level windows have no
  parent. `SetParent(nil)` detaches. Adding a control to another parent removes it from the
  old one (S2).
- **Visibility:** new controls and windows start hidden; plugins call `SetVisible(true)`
  (S2/S4 usage). `IsDisplayed()` = own visibility and every ancestor's (S2).
- **Transparency:** a bare `Control` draws nothing until it gets a background image or
  colour (S2).
- **Stacking:** higher `ZOrder` draws above lower among siblings (S2); top-level windows
  also come to the front on `Activate()`.
- **Opacity:** 0.0–1.0; applies to windows and their content (S2).
- **Mouse:** `SetMouseVisible(false)` lets mouse input fall through to what is below (S2).
- **Keys:** only controls with `SetWantsKeyEvents(true)` receive `KeyDown`/`KeyUp` (S2).
  Key events carry key-binding **actions**, not raw keys (§9.3).
- **Focus:** `Focus()`, `HasFocus()`, events `FocusGained`/`FocusLost`; text boxes take
  focus on click.
- **Drag and drop:** `SetAllowDrop(true)` enables `DragEnter`/`DragLeave`/`DragDrop`; drags
  of game shortcuts produce `Turbine.UI.Lotro.DragDropInfo` (§14.6).

<a id="13-2"></a>
### 13.2 `Control` (base of all controls)

| Group | Members (all instance methods unless marked) |
|---|---|
| Geometry | `GetLeft/SetLeft`, `GetTop/SetTop`, `GetPosition()`→l,t / `SetPosition(l,t)`, `GetWidth/SetWidth`, `GetHeight/SetHeight`, `GetSize()`→w,h / `SetSize(w,h)`, `PointToClient(x,y)`→x,y, `PointToScreen(x,y)`→x,y, `GetMousePosition()`→x,y (relative to the control) |
| Tree | `GetParent/SetParent(control or nil)`, `GetControls()`→ControlList (live, mutable view of children), `GetZOrder/SetZOrder(n)` |
| State | `IsVisible/SetVisible(b)`, `IsDisplayed()`, `IsEnabled/SetEnabled(b)`, `GetOpacity/SetOpacity(0..1)`, `IsMouseVisible/SetMouseVisible(b)`, `GetWantsKeyEvents/SetWantsKeyEvents(b)`, `GetWantsUpdates/SetWantsUpdates(b)`, `GetAllowDrop/SetAllowDrop(b)`, `Focus()`, `HasFocus()` |
| Look | `GetBackColor/SetBackColor(Color)`, `GetBackColorBlendMode/SetBackColorBlendMode(BlendMode)`, `GetBackground/SetBackground(path or DID)` (§13.8), `GetBlendMode/SetBlendMode(BlendMode)` (applies to the background image), `GetStretchMode/SetStretchMode(n)` (§13.8) |
| Static | `Control.IsAltKeyDown()`, `Control.IsControlKeyDown()`, `Control.IsShiftKeyDown()` |
| Events | `MouseEnter`, `MouseLeave`, `MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick`, `MouseHover`, `MouseMove`, `MouseWheel`, `KeyDown`, `KeyUp`, `FocusGained`, `FocusLost`, `EnabledChanged`, `VisibleChanged`, `PositionChanged`, `SizeChanged`, `DragStart`, `DragEnter`, `DragLeave`, `DragDrop`, `Update` |

<a id="13-3"></a>
### 13.3 `Window` (extends `Control`)

Top-level container with no frame of its own (S2). Members: `GetText/SetText(title)`,
`Activate()` (bring to front), `Close()`, minimum/maximum sizes (`Get/SetMinimumWidth`,
`…Height`, `GetMinimumSize()`→w,h, `SetMinimumSize(w,h)`, same for `Maximum`),
`GetRotation()`/`SetRotation({x=…, y=…, z=…})` (newer; > **OPEN:** units and pivot).
Events: `Activated`, `Deactivated`, `Closing`, `Closed`.

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

**Markup** (when enabled, S2): `<u>…</u>` underline and `<rgb=RRGGBB>…</rgb>` colour (six hex
digits). S2 lists two further tags that were lost from its HTML. > **OPEN:** the other tags
(probably bold/italic) and whether `#` before the hex digits is accepted. Tessera: accept
`<u>`, `<rgb=…>` with or without `#`; render unknown tags literally.

> **OPEN:** selection offsets 0- or 1-based; default font, colour and alignment.

<a id="13-5"></a>
### 13.5 `TextBox` (extends `Label`)

Editable text. Members: `IsReadOnly/SetReadOnly(b)`. Events: `TextChanged`,
`EnabledChanged`. Typing in a focused text box must not trigger game key bindings.

<a id="13-6"></a>
### 13.6 `Button` and `CheckBox` (extend `Label`)

`Button`: events `Click` (> **OPEN:** does `Click` fire on any mouse button or only left;
args), `EnabledChanged`. Unskinned; `Turbine.UI.Lotro.Button` is the skinned variant.
`CheckBox`: `IsChecked/SetChecked(b)`, `GetCheckAlignment/SetCheckAlignment(ContentAlignment)`;
events `CheckedChanged`, `EnabledChanged`.

<a id="13-7"></a>
### 13.7 Scrolling and lists

- `ScrollBar` (extends `Control`): `Get/SetMinimum`, `Get/SetMaximum`, `Get/SetValue`,
  `Get/SetSmallChange`, `Get/SetLargeChange`, `Get/SetOrientation(Orientation)`, and
  replaceable parts `Get/SetDecrementButton`, `Get/SetIncrementButton` (Buttons),
  `Get/SetThumbButton` (Control). Events `ValueChanged`, `OrientationChanged`.
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
  sorts first and must be a consistent ordering (S2 warns inconsistency crashed the
  original; Tessera must not crash, it just yields some order). Events `ItemAdded`,
  `ItemRemoved`, `SelectedIndexChanged`.
- `ControlList` (and `MenuItemList`, `TreeNodeList`): `Add`, `Insert(i,x)`, `Remove(x)`,
  `RemoveAt(i)`, `Clear`, `Get(i)`, `Set(i,x)`, `GetCount`, `IndexOf(x)`, `Contains(x)`.
- `TreeView` / `TreeNode` / `TreeNodeList` (Milestone 2): nodes are controls; `GetNodes()`,
  `GetSelectedNode/SetSelectedNode`, `ExpandAll/CollapseAll`, `Get/SetIndentationWidth`,
  `Get/SetFilter(f)` (`f(node)` → keep?), `Get/SetSortMethod(f)`, `Refresh()`,
  `GetItemAt(x,y)`; node: `GetChildNodes()`, `GetParentNode()`, `Expand`, `Collapse`,
  `ExpandAll`, `CollapseAll`, `IsExpanded/SetExpanded`, `IsSelected`, `EnsureVisible`;
  events `SelectedNodeChanged`, `SelectedChanged`.

<a id="13-8"></a>
### 13.8 Images

- `SetBackground(v)`: `v` is either a **path** string relative to `Plugins/` (TGA or JPG;
  §4.4) or a **number** that is the DID of a client image (render surface `0x41xxxxxx`, P13;
  see `render-surface.md`). `nil` clears.
- `Turbine.UI.Graphic`: `Graphic()` empty, `Graphic(did)` or `Graphic(path)` (S2 overloads);
  accepted wherever a background is.
- Stretch modes (S3 stubs, community-observed, Reported): 0 = no scaling, image cropped or
  tiled to the control and clipped by the parent; 1 = scale the image by the ratio between
  the control's size when the mode was set and its current size; 2 = resize the control to
  the image's size when set, stretch afterwards; 3 = like 0 but not clipped by the parent;
  4 = like 1 but the control ignores the mouse. > **OPEN:** confirm by observation; mode 0
  reportedly also resets alpha.
- Image load failure (missing file, bad DID): the control shows nothing; no error is raised
  (OPEN).

<a id="13-9"></a>
### 13.9 `Color`

`Turbine.UI.Color()` (> **OPEN:** S2 says black, its own example says transparent; Tessera
uses opaque black), `Color(r, g, b)` (alpha 1), `Color(a, r, g, b)`; components 0.0–1.0, read
and written as fields `R`, `G`, `B`, `A`. 141 named colours are fields of the class table
(Appendix A.5), e.g. `Turbine.UI.Color.Red`; they are the standard web/X11 named colours plus
`Transparent` (white with alpha 0).

<a id="13-10"></a>
### 13.10 Other `Turbine.UI` classes

- `Display` (static): `GetWidth()`, `GetHeight()`, `GetSize()`→w,h of the UI area in pixels;
  `GetMouseX()`, `GetMouseY()`, `GetMousePosition()`→x,y in screen pixels.
- `DragDropInfo`: `IsSuccessful()`, `SetSuccessful(b)` (a drop handler marks the drop as
  accepted).
- `ContextMenu` (Milestone 2): `GetItems()`→MenuItemList, `ShowMenu()` (at the mouse,
  kept on screen, closes any other open context menu), `ShowMenuAt(x, y)`, `Close()`.
  `MenuItem(text [, enabled [, checked]])`: `Get/SetText`, `IsEnabled/SetEnabled`,
  `IsChecked/SetChecked`, `GetItems()` (sub-menu), event `Click`.
- Enums `BlendMode`, `ContentAlignment`, `FontStyle`, `MouseButton`, `Orientation`,
  `HorizontalLayout`, `VerticalLayout`: Appendix A.

---

## 14. Game-styled UI (`Turbine.UI.Lotro`)

<a id="14-1"></a>
### 14.1 Fonts

`Turbine.UI.Lotro.Font` values are DIDs of font records (P12). `Label:SetFont` takes one of
them. 48 fonts plus `Undefined = 0` (Appendix A.2).

Font record payload prefix (record type `0x42`, Confirmed 2026-10-03 for all 48 records;
the payload is what follows the 8-byte block header described in the dat container spec):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 LE | The record's own DID (`0x42xxxxxx`) |
| 4 | 4 | u32 LE | Nominal point size; equals the trailing number of the enum name (e.g. 20 for `TrajanPro20`, 36 for `BookAntiqua36`) |
| 8 | … | — | > **OPEN:** glyph metrics and texture references; record sizes observed 2,137–15,645 bytes |

Location: 45 font records in `client_local_English.dat`, 3 (`0x42000027`, `0x42000028`,
`0x420000FD`) in `client_general.dat`. Records were stored uncompressed (directory flags 2).
> **OPEN:** other language archives presumably carry their own copies.

<a id="14-2"></a>
### 14.2 Skinned controls

| Class | Base | Look / extra members |
|---|---|---|
| `Window` | `Turbine.UI.Window` | Standard silver game window frame with title text (`SetText`) and close button; `IsResizable/SetResizable(b)`. |
| `GoldWindow` | `Turbine.UI.Lotro.Window` | Gold variant. |
| `Button` | `Turbine.UI.Button` | Standard game button. |
| `GoldButton` | `Turbine.UI.Lotro.Button` | Gold variant. |
| `CheckBox` | `Turbine.UI.CheckBox` | Game check box art. |
| `TextBox` | `Turbine.UI.TextBox` | Game text-field art. |
| `ScrollBar` | `Turbine.UI.ScrollBar` | Game scroll-bar art; event `OrientationChanged`. |

Art comes from the client's UI skin data (see `ui-layout.md`, OPEN there). Milestone 1 may
draw them with Tessera's own neutral frame art of the same metrics. > **OPEN:** frame
border widths and title-bar height (plugins lay out content assuming them).

<a id="14-3"></a>
### 14.3 Native panel control (`LotroUI`)

`LotroUI.IsEnabled(element)`, `LotroUI.SetEnabled(element, b)`, `LotroUI.Reset()`
(re-enable all). `element` is a `LotroUIElement` value: `PluginsManager`, `Party`,
`Backpack1`–`Backpack6`, `Vitals`, `Target` (Appendix A). Disabling hides the native panel so
a plugin can replace it (S4 sample hides the vitals panel). Tessera maps these to its own
native panels; until they exist, store and report the flag.

<a id="14-4"></a>
### 14.4 Key-binding actions (`Action`)

270 members (Appendix A.3): `Undefined = 0`, three small ids (`CameraInstantMouseLook = 92`,
`CaptureScreenshot = 116`, `Escape = 145`) and the rest in `0x10000002…0x10000246`. Names
are the client's key-binding action names (P20). `KeyDown`/`KeyUp` deliver the action bound
to the pressed key, which may be outside this table (S3 observed `0x100001D8`). Tessera's
input layer maps its own actions onto these ids (Architecture §16).

<a id="14-5"></a>
### 14.5 Game-object controls (Milestone 2)

| Class | Base | Members |
|---|---|---|
| `BaseItemControl` | `Control` | — |
| `ItemControl` | `BaseItemControl` | `GetItem/SetItem(Item)`: shows an item icon with the game's tooltip and context behaviour |
| `ItemInfoControl` | `BaseItemControl` | `GetItemInfo/SetItemInfo(ItemInfo)`, `GetQuantity/SetQuantity(n)` |
| `EquipmentSlot` | `BaseItemControl` | `GetEquipmentSlot/SetEquipmentSlot(slot)`; event `DragDrop` |
| `EntityControl` | `Control` | `GetEntity/SetEntity`, `IsSelectionEnabled/SetSelectionEnabled` (click selects the entity as target), `IsContextMenuEnabled/SetContextMenuEnabled` |
| `EffectDisplay` | `Control` | `GetEffect/SetEffect(Effect)`: icon and timer of a buff/debuff |

<a id="14-6"></a>
### 14.6 Quickslots and shortcuts (Milestone 2)

- `Shortcut()`, `Shortcut(item)`, `Shortcut(type, data)`: `GetType/SetType(ShortcutType)`,
  `GetData/SetData(string)`, `GetItem()`. `ShortcutType`: `Item = 2`, `Alias = 4`,
  `Emote = 5`, `Skill = 6`, `Pet = 7`, `Hobby = 9`. > **OPEN:** the `data` string format per
  type (e.g. alias text, hex skill id).
- `Quickslot` (extends `Control`): `GetShortcut/SetShortcut`, `IsUseOnRightClick/
  SetUseOnRightClick(b)`; events `ShortcutChanged`, `DragDrop`. A **user click** on a
  quickslot activates its shortcut; this is the only way a plugin control causes a game
  action (§3.5).
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
| `Actor` | Morale and power (current, max, base max, temporary, max temporary), `GetLevel()`, `GetTarget()` (an `Entity` or `nil`), `GetEffects()` (`EffectList`); one `…Changed` event per value. |
| `Player` | `GetClass()`, `GetRace()`, `GetAlignment()` (enum numbers), `GetParty()`, `GetPet()`, `GetReadyState()`, link-dead and voice flags; events `PartyChanged`, `RaidChanged`, `PetChanged`, `ReadyStateChanged`, … |
| `LocalPlayer` | `GetAttributes()` (`FreePeopleAttributes` or `MonsterPlayerAttributes`), `GetClassAttributes()` (per-class subclass), `GetRaceAttributes()`, `GetBackpack()`, `GetEquipment()`, `GetVault()`, `GetSharedStorage()`, `GetWallet()`, `GetMount()`, `GetTrainedSkills()`, `GetUntrainedSkills()`, `IsInCombat()`; events `InCombatChanged`, `MountChanged`. |
| `Item` / `ItemInfo` | Names, description, category (`ItemCategory`), quality, durability, wear state, quantity and stack limits, and image DIDs (icon, background, underlay, shadow, quality overlay) that plugins pass to `SetBackground`. |
| `FreePeopleAttributes` | Stats (might, agility, vitality, will, fate and base values), mitigations, resistances, critical/defence ratings, regeneration, healing, `GetMoney()` (copper) and `GetMoneyComponents()` (three numbers, > **OPEN:** order: S2 says gold, silver, copper; a later correction says copper, silver, gold), `GetProfessionInfo(profession)`, `GetVocation()`. |

Enum values used here (`Class`, `Race`, `Alignment`, `Equipment` slots, `ItemCategory`, …)
are in Appendix A. > **OPEN:** whether these numbers equal the game-data enum-mapper values
(see `enum-mapper.md`); the `Class` values (23 = Guardian, 24 = Captain, …) look like them.

Notes from the live dump (S3) that differ from S2:

- `Item` has many getters of its own in the live client (icon ids, quality, quantity, …)
  that S2 lists only on `ItemInfo`.
- `Turbine.Gameplay.Equipment` in the live client is the **slot enumeration** (plus `IsA`);
  the object returned by `LocalPlayer:GetEquipment()` has `GetItem(slot)`, `GetSize()` and
  the two events.
- `Effect.IsDispellable` and Burglar `IsCriticalTier1/2Available` are documented but absent
  from the live dump; `BrawlerAttributes`, `BrawlerStance`, `WardenGambit`, `Raid` are live
  but undocumented; `StoutAxeAttributes` is documented and present as a client string but
  missing from the dump.

---

## 16. API inventory

Columns: namespace; class (flags note differences between S2 and the live dump S3); base
class; kind (`class` = instantiable or returned objects, `static` = function table, `enum`);
own members (inherited ones omitted; "live-only" = in the live dump but not in S2); events
(own); implementation tier (§17).

| Namespace | Class | Base | Kind | Own members | Events | Tier |
|---|---|---|---|---|---|---|
| `Turbine` | **Chat** | Object | class | (none of its own) | Received | M1 |
| `Turbine` | **ChatType** | — | enum | 34 values (Appendix A) | — | M1 |
| `Turbine` | **DataScope** | — | enum | 3 values (Appendix A) | — | M1 |
| `Turbine` | **Engine** | — | static | static: GetCallStack, GetDate, GetGameTime, GetLanguage, GetLocalTime, GetLocale, GetScriptVersion, ScriptLog | — | M1 |
| `Turbine.Gameplay` | **ActiveSkill** | Skill | class | GetBaseCooldown, GetCooldown, GetResetTime, IsUsable | ResetTimeChanged | M3 |
| `Turbine.Gameplay` | **Actor** | Entity | class | GetBaseMaxMorale, GetBaseMaxPower, GetEffects, GetLevel, GetMaxMorale, GetMaxPower, GetMaxTemporaryMorale, GetMaxTemporaryPower, GetMorale, GetPower, GetTarget, GetTemporaryMorale, GetTemporaryPower | BaseMaxMoraleChanged, BaseMaxPowerChanged, LevelChanged, MaxMoraleChanged, MaxPowerChanged, MaxTemporaryMoraleChanged, MaxTemporaryPowerChanged, MoraleChanged, PowerChanged, TargetChanged, TemporaryMoraleChanged, TemporaryPowerChanged | M1 |
| `Turbine.Gameplay` | **Alignment** | — | enum | 3 values (Appendix A) | — | M1 |
| `Turbine.Gameplay` | **Attributes** | PropertyHandler | class | (none of its own) | — | M2 |
| `Turbine.Gameplay.Attributes` | **BeorningAttributes** | ClassAttributes | class | GetWrath, IsInBearForm | FormChanged, WrathChanged | M3 |
| `Turbine.Gameplay.Attributes` | **BlackArrowAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **BrawlerAttributes** (not in U25 docs) | ClassAttributes (inferred) | class | live-only: GetStance | — | M3 |
| `Turbine.Gameplay.Attributes` | **BrawlerStance** (not in U25 docs) | — | enum | 1 value (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **BurglarAttributes** | ClassAttributes | class | GetStance, IsCriticalTier1Available, IsCriticalTier2Available | IsCriticalTier1Changed, IsCriticalTier2Changed, StanceChanged | M3 |
| `Turbine.Gameplay.Attributes` | **BurglarStance** | — | enum | 1 value (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **CaptainAttributes** | ClassAttributes | class | IsInEnemyDefeatResponse, IsInFellowDefeatResponse, IsReadiedTier1Available, IsReadiedTier2Available | IsInEnemyDefeatResponseChanged, IsInFellowDefeatResponseChanged, IsReadiedTier1Changed, IsReadiedTier2Changed | M3 |
| `Turbine.Gameplay.Attributes` | **ChampionAttributes** | ClassAttributes | class | GetFervor, GetStance | FervorChanged, StanceChanged | M3 |
| `Turbine.Gameplay.Attributes` | **ChampionStance** | — | enum | 1 value (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **ChickenAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **DefilerAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **DwarfAttributes** | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **ElfAttributes** | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **FreePeopleAttributes** | Attributes | class | GetAgility, GetArmor, GetBaseAgility, GetBaseArmor, GetBaseFate, GetBaseMight, GetBaseResistance, GetBaseVitality, GetBaseWill, GetDestinyPoints, GetDiseaseResistance, GetFate, GetFearResistance, GetMight, GetMoney, GetMoneyComponents, GetPoisonResistance, GetProfessionInfo, GetVitality, GetVocation, GetWill, GetWoundResistance, GetAcidMitigation, GetBaseCriticalHitAvoidance, GetBaseCriticalHitChance, GetBlock, GetCommonMitigation, GetEvade, GetFinesse, GetFireMitigation, GetFrostMitigation, GetInCombatMoraleRegeneration, GetInCombatPowerRegeneration, GetIncomingHealing, GetLightningMitigation, GetMeleeCriticalHitAvoidance, GetMeleeCriticalHitChance, GetMeleeDamage, GetMeleeDefence, GetOutOfCombatMoraleRegeneration, GetOutOfCombatPowerRegeneration, GetOutgoingHealing, GetParry, GetPhysicalMitigation, GetRangeCriticalHitAvoidance, GetRangeCriticalHitChance, GetRangeDamage, GetRangeDefence, GetShadowMitigation, GetTacticalCriticalHitAvoidance, GetTacticalCriticalHitChance, GetTacticalDamage, GetTacticalDefence, GetTacticalMitigation, CanBlock, CanEvade, CanParry | — | M2 |
| `Turbine.Gameplay.Attributes` | **GuardianAttributes** | ClassAttributes | class | GetStance, IsBlockTier1Available, IsBlockTier2Available, IsBlockTier3Available, IsParryTier1Available, IsParryTier2Available, IsParryTier3Available | IsBlockTier1AvailableChanged, IsBlockTier2AvailableChanged, IsBlockTier3AvailableChanged, IsParryTier1AvailableChanged, IsParryTier2AvailableChanged, IsParryTier3AvailableChanged, StanceChanged | M3 |
| `Turbine.Gameplay.Attributes` | **GuardianStance** | — | enum | 1 value (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **HighElfAttributes** | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **HobbitAttributes** | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **HunterAttributes** | ClassAttributes | class | GetFocus, GetStance | FocusChanged, StanceChanged | M3 |
| `Turbine.Gameplay.Attributes` | **HunterStance** | — | enum | 4 values (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **LoreMasterAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **ManAttributes** | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **MarinerAttributes** | ClassAttributes | class | GetBalance | — | M3 |
| `Turbine.Gameplay.Attributes` | **MinstrelAttributes** | ClassAttributes | class | GetStance, IsSerenadeTier1Available, IsSerenadeTier2Available, IsSerenadeTier3Available | IsSerenadeTier1Changed, IsSerenadeTier2Changed, IsSerenadeTier3Changed, StanceChanged | M3 |
| `Turbine.Gameplay.Attributes` | **MinstrelStance** | — | enum | 3 values (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **MonsterPlayerAttributes** | Attributes | class | GetDestinyPoints, GetMoney, GetMoneyComponents | — | M2 |
| `Turbine.Gameplay.Attributes` | **RangerAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **ReaverAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **RiverHobbitAttributes** | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **RuneKeeperAttributes** | ClassAttributes | class | GetAttunement, IsCharged | AttunementChanged, IsChargedChanged | M3 |
| `Turbine.Gameplay.Attributes` | **StalkerAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **StoutAxeAttributes** (not in live dump) | Attributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **TrollAttributes** | ClassAttributes | class | GetRampage | RampageChanged | M3 |
| `Turbine.Gameplay.Attributes` | **WarLeaderAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay.Attributes` | **WardenAttributes** | ClassAttributes | class | GetGambit, GetGambitCount, GetMaxGambitCount, GetStance, GetTrainedGambits, GetUntrainedGambits | GambitChanged, MaxGambitCountChanged, StanceChanged | M3 |
| `Turbine.Gameplay.Attributes` | **WardenGambit** (not in U25 docs) | — | enum | 4 values (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **WardenStance** | — | enum | 3 values (Appendix A) | — | M3 |
| `Turbine.Gameplay.Attributes` | **WeaverAttributes** | ClassAttributes | class | (none of its own) | — | M3 |
| `Turbine.Gameplay` | **Backpack** | Object | class | GetItem, GetSize, PerformItemDrop, PerformShortcutDrop | ItemAdded, ItemMoved, ItemRemoved, SizeChanged | M2 |
| `Turbine.Gameplay` | **Bank** | Object | class | GetCapacity, GetChestCount, GetChestName, GetCount, GetItem, IsAvailable, PerformShortcutDrop | CapacityChanged, ChestNameChanged, CountChanged, IsAvailableChanged, ItemAdded, ItemMoved, ItemRemoved, ItemsRefreshed | M3 |
| `Turbine.Gameplay` | **BankItem** | Item | class | GetQuantity, GetWearState, GetBoundOwner, GetChest | — | M3 |
| `Turbine.Gameplay` | **BasicMount** | Mount | class | GetMaxMorale, GetMorale | MaxMoraleChanged, MoraleChanged | M3 |
| `Turbine.Gameplay` | **Class** | — | enum | 22 values (Appendix A) | — | M1 |
| `Turbine.Gameplay` | **ClassAttributes** | Attributes | class | (none of its own) | — | M2 |
| `Turbine.Gameplay` | **CombatMount** | Mount | class | GetAgility, GetArmor, GetBaseAgility, GetBaseArmor, GetBaseMaxMorale, GetBaseMaxPower, GetBaseStrength, GetFury, GetMaxFury, GetMaxMorale, GetMaxPower, GetMaxTemporaryMorale, GetMaxTemporaryPower, GetMorale, GetPower, GetStrength, GetTemporaryMorale, GetTemporaryPower, GetEvasion, GetTurnRate | BaseMaxMoraleChanged, BaseMaxPowerChanged, FuryChanged, MaxMoraleChanged, MaxPowerChanged, MaxTemporaryMoraleChanged, MaxTemporaryPowerChanged, MoraleChanged, PowerChanged, TemporaryMoraleChanged, TemporaryPowerChanged | M3 |
| `Turbine.Gameplay` | **CraftTier** | — | enum | 10 values (Appendix A) | — | M3 |
| `Turbine.Gameplay` | **Effect** | Object | class | GetCategory, GetDescription, GetDuration, GetID, GetIcon, GetName, GetStartTime, IsCurable, IsDebuff, IsDispellable | CategoryChanged, DescriptionChanged, DurationChanged, IconChanged, IsCurableChanged, IsDebuffChanged, IsDispellableChanged, NameChanged, StartTimeChanged | M2 |
| `Turbine.Gameplay` | **EffectCategory** | — | enum | 11 values (Appendix A) | — | M2 |
| `Turbine.Gameplay` | **EffectList** | Object | class | Contains, Get, GetCount, IndexOf | EffectAdded, EffectRemoved, EffectsCleared | M2 |
| `Turbine.Gameplay` | **Entity** | PropertyHandler | class | GetName, IsLocalPlayer | NameChanged | M1 |
| `Turbine.Gameplay` | **EntityReference** (not in live dump) | Object | class | (none of its own) | — | M3 |
| `Turbine.Gameplay` | **Equipment** (live table holds the slot enum, Appendix A) | Object | class | GetItem, GetSize | ItemEquipped, ItemUnequipped | M2 |
| `Turbine.Gameplay` | **GambitSkill** | Skill | class | (none of its own) | — | M3 |
| `Turbine.Gameplay` | **GambitSkillInfo** | SkillInfo | class | GetGambit, GetGambitCount | — | M3 |
| `Turbine.Gameplay` | **Item** | Entity | class | GetItemInfo; live-only: GetBackgroundImageID, GetCategory, GetDurability, GetIconImageID, GetMaxStackSize, GetQuality, GetQualityImageID, GetQuantity, GetShadowImageID, GetUnderlayImageID, GetWearState, IsMagic | — | M2 |
| `Turbine.Gameplay` | **ItemCategory** | — | enum | 187 values (Appendix A) | — | M2 |
| `Turbine.Gameplay` | **ItemDurability** | — | enum | 8 values (Appendix A) | — | M2 |
| `Turbine.Gameplay` | **ItemInfo** | Object | class | GetBackgroundImageID, GetCategory, GetDescription, GetDurability, GetIconImageID, GetMaxQuantity, GetMaxStackSize, GetName, GetNameWithQuantity, GetQuality, GetQualityImageID, GetShadowImageID, GetUnderlayImageID, IsMagic, IsUnique | — | M2 |
| `Turbine.Gameplay` | **ItemQuality** | — | enum | 6 values (Appendix A) | — | M2 |
| `Turbine.Gameplay` | **ItemWearState** | — | enum | 5 values (Appendix A) | — | M2 |
| `Turbine.Gameplay` | **LocalPlayer** | Player | class | static: GetInstance; GetAttributes, GetBackpack, GetClassAttributes, GetEquipment, GetMount, GetRaceAttributes, GetReadyState, GetSharedStorage, GetTrainedSkills, GetUntrainedSkills, GetVault, GetWallet, IsInCombat, IsLinkDead, IsVoiceEnabled | InCombatChanged, MountChanged | M1 |
| `Turbine.Gameplay` | **Mount** | Entity | class | GetName | — | M3 |
| `Turbine.Gameplay` | **Party** | Object | class | GetAssistTarget, GetAssistTargetCount, GetLeader, GetMember, GetMemberCount, IsAssistTarget, IsMember | AssistTargetAdded, AssistTargetRemoved, LeaderChanged, MemberAdded, MemberRemoved | M2 |
| `Turbine.Gameplay` | **PartyMember** | Player | class | GetClass, GetLevel, GetName, GetParty, GetRaid, GetReadyState, IsLinkDead, IsVoiceEnabled | — | M2 |
| `Turbine.Gameplay` | **Pet** | Actor | class | (none of its own) | — | M2 |
| `Turbine.Gameplay` | **Player** | Actor | class | GetAlignment, GetClass, GetParty, GetPet, GetRace, GetReadyState, IsLinkDead, IsVoiceActive, IsVoiceEnabled | IsLinkDeadChanged, IsVoiceActiveChanged, IsVoiceEnabledChanged, PartyChanged, PetChanged, RaidChanged, ReadyStateChanged | M1 |
| `Turbine.Gameplay` | **Profession** | — | enum | 11 values (Appendix A) | — | M3 |
| `Turbine.Gameplay` | **ProfessionInfo** | Object | class | GetMasteryExperience, GetMasteryExperienceTarget, GetMasteryLevel, GetMasteryTitle, GetName, GetProficiencyExperience, GetProficiencyExperienceTarget, GetProficiencyLevel, GetProficiencyTitle, GetRecipe, GetRecipeCount | MasteryExperienceChanged, MasteryExperienceTargetChanged, MasteryLevelChanged, MasteryTitleChanged, ProficiencyExperienceChanged, ProficiencyExperienceTargetChanged, ProficiencyLevelChanged, ProficiencyTitleChanged, RecipeAdded, RecipeRemoved | M3 |
| `Turbine.Gameplay` | **PropertyHandler** (not in live dump) | EntityReference | class | (none of its own) | — | M3 |
| `Turbine.Gameplay` | **Race** | — | enum | 9 values (Appendix A) | — | M1 |
| `Turbine.Gameplay` | **Raid** (not in U25 docs) | Object | class | (none of its own) | — | M2 |
| `Turbine.Gameplay` | **ReadyState** | — | enum | 3 values (Appendix A) | — | M2 |
| `Turbine.Gameplay` | **Recipe** | Object | class | GetBaseCriticalSuccessChance, GetCategory, GetCategoryName, GetCooldown, GetCriticalResultItemInfo, GetCriticalResultItemQuantity, GetExperienceReward, GetIngredient, GetIngredientCount, GetIngredientPack, GetName, GetOptionalIngredient, GetOptionalIngredientCount, GetProfession, GetResultItemInfo, GetResultItemQuantity, GetTier, HasCriticalResultItem, IsSingleUse | — | M3 |
| `Turbine.Gameplay` | **RecipeIngredient** | Object | class | GetCriticalChanceBonus, GetItemInfo, GetRequiredQuantity | — | M3 |
| `Turbine.Gameplay` | **SharedStorage** | Bank | class | (none of its own) | — | M3 |
| `Turbine.Gameplay` | **Skill** | Object | class | GetSkillInfo | — | M3 |
| `Turbine.Gameplay` | **SkillInfo** | Object | class | GetDescription, GetIconImageID, GetName, GetType | — | M3 |
| `Turbine.Gameplay` | **SkillList** | Object | class | GetCount, GetItem | SkillAdded, SkillRemoved | M3 |
| `Turbine.Gameplay` | **SkillType** | — | enum | 3 values (Appendix A) | — | M3 |
| `Turbine.Gameplay` | **UntrainedSkill** | Skill | class | GetPrice, GetRequiredLevel, GetRequiredRank | — | M3 |
| `Turbine.Gameplay` | **Vault** | Bank | class | (none of its own) | — | M3 |
| `Turbine.Gameplay` | **Vocation** | — | enum | 8 values (Appendix A) | — | M3 |
| `Turbine.Gameplay` | **Wallet** | Object | class | GetItem, GetSize | ItemAdded, ItemRemoved | M2 |
| `Turbine.Gameplay` | **WalletItem** | Object | class | GetDescription, GetImage, GetMaxQuantity, GetName, GetQuantity, GetSmallImage, IsAccountItem | MaxQuantityChanged, QuantityChanged | M2 |
| `Turbine` | **Language** | — | enum | 5 values (Appendix A) | — | M1 |
| `Turbine` | **LotroPluginManager** (not in live dump) | PluginManager | static | static: ShowOptions | — | M2 |
| `Turbine` | **Object** | — | class | IsA | — | M1 |
| `Turbine` | **Plugin** | Object | class | GetAuthor, GetName, GetVersion | Load, Unload; callback GetOptionsPanel | M1 |
| `Turbine` | **PluginData** | — | static | static: Load, Save | — | M1 |
| `Turbine` | **PluginManager** | — | static | static: GetAvailablePlugins, GetLoadedPlugins, LoadPlugin, RefreshAvailablePlugins, UnloadScriptState, ShowOptions | — | M1 |
| `Turbine` | **Shell** | — | static | static: AddCommand, GetCommands, IsCommand, RemoveCommand, WriteLine | — | M1 |
| `Turbine` | **ShellCommand** | Object | class | Execute, GetHelp, GetShortHelp | — | M1 |
| `Turbine.UI` | **BlendMode** | — | enum | 9 values (Appendix A) | — | M1 |
| `Turbine.UI` | **Button** | Label | class | (none of its own) | Click, EnabledChanged | M1 |
| `Turbine.UI` | **CheckBox** | Label | class | GetCheckAlignment, IsChecked, SetCheckAlignment, SetChecked | CheckedChanged, EnabledChanged | M1 |
| `Turbine.UI` | **Color** | Object | class | (none of its own) | — | M1 |
| `Turbine.UI` | **ContentAlignment** | — | enum | 10 values (Appendix A) | — | M1 |
| `Turbine.UI` | **ContextMenu** | Object | class | Close, GetItems, ShowMenu, ShowMenuAt | — | M2 |
| `Turbine.UI` | **Control** | Object | class | static: IsAltKeyDown, IsControlKeyDown, IsShiftKeyDown; Focus, GetAllowDrop, GetBackColor, GetBackColorBlendMode, GetBackground, GetBlendMode, GetControls, GetHeight, GetLeft, GetMousePosition, GetOpacity, GetParent, GetPosition, GetSize, GetTop, GetWantsKeyEvents, GetWantsUpdates, GetWidth, GetZOrder, HasFocus, IsDisplayed, IsEnabled, IsMouseVisible, IsVisible, PointToClient, PointToScreen, SetAllowDrop, SetBackColor, SetBackColorBlendMode, SetBackground, SetStretchMode, GetStretchMode, SetBlendMode, SetEnabled, SetHeight, SetLeft, SetMouseVisible, SetOpacity, SetParent, SetPosition, SetSize, SetTop, SetVisible, SetWantsKeyEvents, SetWantsUpdates, SetWidth, SetZOrder | DragDrop, DragEnter, DragLeave, DragStart, EnabledChanged, FocusGained, FocusLost, KeyDown, KeyUp, MouseClick, MouseDoubleClick, MouseDown, MouseEnter, MouseHover, MouseLeave, MouseMove, MouseUp, MouseWheel, PositionChanged, SizeChanged, Update, VisibleChanged | M1 |
| `Turbine.UI` | **ControlList** | Object | class | Add, Clear, Contains, Get, GetCount, IndexOf, Insert, Remove, RemoveAt, Set | — | M1 |
| `Turbine.UI` | **Display** | — | static | static: GetHeight, GetMousePosition, GetMouseX, GetMouseY, GetSize, GetWidth | — | M1 |
| `Turbine.UI` | **DragDropInfo** | Object | class | IsSuccessful, SetSuccessful | — | M2 |
| `Turbine.UI` | **FontStyle** | — | enum | 2 values (Appendix A) | — | M1 |
| `Turbine.UI` | **Graphic** | Object | class | (none of its own) | — | M1 |
| `Turbine.UI` | **HorizontalLayout** | — | enum | 2 values (Appendix A) | — | M1 |
| `Turbine.UI` | **Label** | ScrollableControl | class | AppendText, DeselectAll, GetFont, GetFontStyle, GetForeColor, GetOutlineColor, GetSelectedText, GetSelectionLength, GetSelectionStart, GetText, GetTextAlignment, GetTextLength, InsertText, IsMarkupEnabled, IsMultiline, IsSelectable, SelectAll, SetFont, SetFontStyle, SetForeColor, SetMarkupEnabled, SetMultiline, SetOutlineColor, SetSelectable, SetSelectedText, SetSelection, SetSelectionLength, SetSelectionStart, SetText, SetTextAlignment | — | M1 |
| `Turbine.UI` | **ListBox** | ScrollableControl | class | AddItem, ClearItems, ContainsItem, EnsureVisible, GetFlippedLayout, GetIgnoreCellAlignment, GetItem, GetItemAt, GetItemCount, GetMaxColumns, GetMaxItemsPerLine, GetMaxRows, GetOrientation, GetReverseFill, GetSelectedIndex, GetSelectedItem, GetWrapIndentAmount, IndexOfItem, InsertItem, RemoveItem, RemoveItemAt, SetFlippedLayout, SetIgnoreCellAlignment, SetItem, SetMaxColumns, SetMaxItemsPerLine, SetMaxRows, SetOrientation, SetReverseFill, SetSelectedIndex, SetSelectedItem, SetWrapIndentAmount, Sort | ItemAdded, ItemRemoved, SelectedIndexChanged | M1 |
| `Turbine.UI.Lotro` | **Action** | — | enum | 270 values (Appendix A) | — | M1 |
| `Turbine.UI.Lotro` | **BaseItemControl** | Control | class | (none of its own) | — | M2 |
| `Turbine.UI.Lotro` | **Button** | UI.Button | class | (none of its own) | — | M1 |
| `Turbine.UI.Lotro` | **CheckBox** | UI.CheckBox | class | (none of its own) | — | M1 |
| `Turbine.UI.Lotro` | **DragDropInfo** | UI.DragDropInfo | class | GetShortcut | — | M2 |
| `Turbine.UI.Lotro` | **EffectDisplay** | Control | class | GetEffect, SetEffect | — | M2 |
| `Turbine.UI.Lotro` | **EntityControl** | Control | class | live-only: GetEntity, IsContextMenuEnabled, IsSelectionEnabled, SetContextMenuEnabled, SetEntity, SetSelectionEnabled | — | M2 |
| `Turbine.UI.Lotro` | **EquipmentSlot** | BaseItemControl | class | GetEquipmentSlot, SetEquipmentSlot | DragDrop | M2 |
| `Turbine.UI.Lotro` | **Font** | — | enum | 49 values (Appendix A) | — | M1 |
| `Turbine.UI.Lotro` | **GoldButton** | Lotro.Button | class | (none of its own) | — | M1 |
| `Turbine.UI.Lotro` | **GoldWindow** | Lotro.Window | class | (none of its own) | — | M1 |
| `Turbine.UI.Lotro` | **ItemControl** | BaseItemControl | class | GetItem, SetItem | — | M2 |
| `Turbine.UI.Lotro` | **ItemInfoControl** | BaseItemControl | class | GetItemInfo, GetQuantity, SetItemInfo, SetQuantity | EnabledChanged | M2 |
| `Turbine.UI.Lotro` | **LotroUI** | — | static | static: IsEnabled, Reset, SetEnabled | — | M1 |
| `Turbine.UI.Lotro` | **LotroUIElement** | — | enum | 10 values (Appendix A) | — | M1 |
| `Turbine.UI.Lotro` | **Quickslot** | Control | class | GetShortcut, IsUseOnRightClick, SetShortcut, SetUseOnRightClick | DragDrop, ShortcutChanged | M2 |
| `Turbine.UI.Lotro` | **ScrollBar** | UI.ScrollBar | class | (none of its own) | OrientationChanged | M1 |
| `Turbine.UI.Lotro` | **Shortcut** | Object | class | GetData, GetItem, GetType, SetData, SetType | — | M2 |
| `Turbine.UI.Lotro` | **ShortcutType** | — | enum | 8 values (Appendix A) | — | M2 |
| `Turbine.UI.Lotro` | **TextBox** | UI.TextBox | class | (none of its own) | — | M1 |
| `Turbine.UI.Lotro` | **Window** | UI.Window | class | IsResizable, SetResizable, SetText | — | M1 |
| `Turbine.UI` | **MenuItem** | Object | class | GetText, IsChecked, IsEnabled, SetChecked, SetEnabled, SetText, GetItems | Click | M2 |
| `Turbine.UI` | **MenuItemList** | Object | class | Add, Clear, Contains, Get, GetCount, IndexOf, Insert, Remove, RemoveAt, Set | — | M2 |
| `Turbine.UI` | **MouseButton** | — | enum | 3 values (Appendix A) | — | M1 |
| `Turbine.UI` | **Orientation** | — | enum | 2 values (Appendix A) | — | M1 |
| `Turbine.UI` | **ScrollBar** | Control | class | GetDecrementButton, GetIncrementButton, GetLargeChange, GetMaximum, GetMinimum, GetOrientation, GetSmallChange, GetThumbButton, GetValue, SetDecrementButton, SetIncrementButton, SetLargeChange, SetMaximum, SetMinimum, SetOrientation, SetSmallChange, SetThumbButton, SetValue | OrientationChanged, ValueChanged | M1 |
| `Turbine.UI` | **ScrollableControl** | Control | class | GetHorizontalScrollBar, GetVerticalScrollBar, SetHorizontalScrollBar, SetVerticalScrollBar | — | M1 |
| `Turbine.UI` | **TextBox** | Label | class | IsReadOnly, SetReadOnly | EnabledChanged, TextChanged | M1 |
| `Turbine.UI` | **TreeNode** | Control | class | Collapse, CollapseAll, EnsureVisible, Expand, ExpandAll, GetChildNodes, GetParentNode, IsExpanded, IsSelected, SetExpanded | SelectedChanged | M2 |
| `Turbine.UI` | **TreeNodeList** | Object | class | Add, Clear, Contains, Get, GetCount, IndexOf, Remove, RemoveAt | — | M2 |
| `Turbine.UI` | **TreeView** | ScrollableControl | class | CollapseAll, ExpandAll, GetFilter, GetIndentationWidth, GetNodes, GetSelectedNode, GetSortMethod, Refresh, SetFilter, SetIndentationWidth, SetSelectedNode, SetSortMethod, GetItemAt | SelectedNodeChanged | M2 |
| `Turbine.UI` | **VerticalLayout** | — | enum | 2 values (Appendix A) | — | M1 |
| `Turbine.UI` | **Window** | Control | class | Activate, Close, GetMaximumHeight, GetMaximumSize, GetMaximumWidth, GetMinimumHeight, GetMinimumSize, GetMinimumWidth, GetText, SetMaximumHeight, SetMaximumSize, SetMaximumWidth, SetMinimumHeight, SetMinimumSize, SetMinimumWidth, SetText, GetRotation, SetRotation | Activated, Closed, Closing, Deactivated | M1 |

Not listed: the namespace tables themselves (`Turbine`, `Turbine.Gameplay`, `Turbine.UI`,
`Turbine.UI.Lotro`, `Turbine.Gameplay.Attributes`), and the newer identifiers of P21, whose
members are unknown (> **OPEN:** `Plugin:GetUsesGlobalScaling`, event `UIScaleChanged`,
`UIScalingGroupType`, `EdgeAttachmentType`, `ComboBox`, `LuaScriptLog`).

---

## 17. Implementation tiers

Tiers are steps **inside** the plugin-host work (Architecture: `tessera-plugins`, roadmap
M5). "Milestone 1" here means the first plugin-host deliverable, not roadmap M1.

<a id="17-1"></a>
### 17.1 Milestone 1 — "hello window" plugins run

Goal: a typical small plugin (a window with labels, buttons and a text box, a slash command,
saved settings, player name/level/morale display) loads, works and unloads cleanly.

- Runtime: §3 sandbox; §4 discovery; §5 descriptor; §6 apartments and lifecycle including
  `Plugins`/`plugin` globals, `Load`/`Unload`, `/plugins load|unload|list|refresh`; §7
  `import` with namespace environments; §8 object model and subclassing; §9 event dispatch
  including handler arrays and `Update`; §10 plugin data (all scopes, sync `Load`, async
  `Save` with flush on unload).
- `Turbine`: `Object`, `Plugin`, `PluginData`, `PluginManager` (all five functions plus
  `ShowOptions` as no-op), `Shell`, `ShellCommand`, `Engine`, `Chat` (fed by Tessera's chat
  and system messages), enums `ChatType`, `DataScope`, `Language`.
- `Turbine.UI`: `Control`, `Window`, `Label` (markup `<u>`, `<rgb>`), `TextBox`, `Button`,
  `CheckBox`, `ScrollBar`, `ScrollableControl`, `ListBox` (incl. `Sort`), `ControlList`,
  `Color` (with named colours), `Display`, `Graphic`, and all `Turbine.UI` enums.
- `Turbine.UI.Lotro`: `Font` (rendered with Tessera's font pipeline from the `0x42`
  records), `Window`, `GoldWindow`, `Button`, `GoldButton`, `CheckBox`, `TextBox`,
  `ScrollBar`, `LotroUI` (flag store), `LotroUIElement`, `Action`.
- `Turbine.Gameplay`: `LocalPlayer.GetInstance()` with the `Entity`, `Actor` and `Player`
  members (others may return defaults), enums `Class`, `Race`, `Alignment`.
- Every other class of §16 exists as a table (so `import` and `IsA` work) and its methods
  return `nil`/defaults without raising, logged once per method at `debug` level.

<a id="17-2"></a>
### 17.2 Milestone 2 — inventory and group plugins

`ContextMenu`/`MenuItem`/`MenuItemList`, `TreeView`/`TreeNode`/`TreeNodeList`,
`DragDropInfo` (both), the plugin manager window with options panels
(`GetOptionsPanel`, `ShowOptions`), `Backpack`, `Item`, `ItemInfo`, `Equipment`, `Wallet`,
`WalletItem`, `Effect`, `EffectList`, `Party`, `PartyMember`, `Raid`, `Pet`,
`FreePeopleAttributes`, `MonsterPlayerAttributes`, the item/effect/entity controls,
`Quickslot`, `Shortcut`, and their enums.

<a id="17-3"></a>
### 17.3 Milestone 3 — everything else

Per-class attributes and stances, mounts, bank/vault/shared storage, skills and gambits,
professions and recipes, `Window` rotation, stretch modes 1–4, UI scaling identifiers (P21).

---

## 18. Invariants and conformance checks

Each is checkable with a synthetic plugin tree (generated by test code, never shipped
third-party plugins) or against the user install.

1. **Sandbox:** in a fresh apartment, `io`, `os`, `package`, `require`, `module`,
   `dofile`, `loadfile` are `nil`; `string`, `table`, `math`, `coroutine`, `getfenv`,
   `setfenv`, `pcall`, `unpack`, `table.getn` exist; `_VERSION == "Lua 5.1"`.
2. **Descriptor:** every `.plugin` file in the user install's `Plugins/` tree parses; each
   has non-empty `Name` and `Package`; each `Package` resolves to a file under the
   case-insensitive rule (observed 2026-10-03: 4 descriptors, all resolve, one only
   case-insensitively).
3. **Environment isolation:** packages `A.P` and `B.P` each defining a bare global `T`
   produce `A.T` and `B.T`, and `_G.T` stays `nil`.
4. **Namespace sharing:** a bare global defined by `A/X.lua` is visible as a bare name in
   `A/Y.lua` imported later.
5. **Apartment isolation:** a global set by a plugin in apartment `X` is not visible in
   apartment `Y`; `Turbine.Chat` in `X` and `Y` are different tables.
6. **Self-unload refusal:** `UnloadScriptState(own apartment)` leaves the apartment loaded.
7. **Unload flush:** a `Save` issued inside an `Unload` handler is on disk when unload
   returns, and a following `Load` (after reload) returns the saved value.
8. **Plugin data round trip:** for random nested tables of booleans, finite numbers and
   strings (including quotes, backslashes, newlines, NUL, non-ASCII) with string and number
   keys, `Load(Save(v)) == v` (deep equality); files parse with the §10.4 grammar.
9. **Key rule:** keys of length 0 and 64 are rejected; length 1 and 63 accepted.
10. **Handler arrays:** with `obj.Click = {f1, f2}` both run, in order, each with
    `(obj, args)`; an error in `f1` does not stop `f2`.
11. **Class-level handler:** assigning `Turbine.Plugin.Load` before load fires it for the
    plugin with `self`/`sender` = the plugin object.
12. **Subclassing:** a Lua subclass of `Turbine.UI.Window` built with a user-defined `class`
    helper can call `SetSize`, receives `MouseClick`, and `IsA(Turbine.UI.Control)` is true.
13. **Colon-called statics:** `Turbine.Gameplay.LocalPlayer:GetInstance()` returns the same
    table as `Turbine.Gameplay.LocalPlayer.GetInstance()`.
14. **Fonts:** for every non-zero `Font` value `v`, the record `v` exists in the archive set
    and its payload's first u32 equals `v` and second u32 equals the name's trailing number
    (holds for all 48 in the user install, 2026-10-03).
15. **No crash on hostile input:** malformed descriptors, binary or huge Lua files, deep
    recursion, `Sort` with an inconsistent comparator, invalid DIDs and image files, and
    circular tables passed to `Save` all produce errors or empty results, never a panic.

---

## 19. Open questions (summary)

All `> **OPEN:**` callouts above, grouped:

- Sandbox: `loadstring`/`load` availability, `debug` exposure, memory budget (§3).
- Discovery depth and duplicate names; descriptor encoding (§4, §5).
- Default apartment name, case of names, load order, failed-load bookkeeping (§6).
- `import` return value, environment mechanism, re-import, native-vs-disk precedence (§7).
- Object identity caching, list index bases (§8).
- Missing event argument fields, setter-fired events, hidden-control `Update` (§9).
- Plugin-data path assembly, encoding, NaN, failure signalling, locale number form (§10).
- Shell return values and matching rules; chat markup (§11).
- `GetDate` bases, `GetLocale` values, script version numbers (§12).
- Label markup tags, defaults, stretch modes, Color() default, window rotation (§13).
- Font record body, skinned frame metrics, shortcut data strings (§14).
- `GetMoneyComponents` order; enum values vs game enum mappers (§15).
- Newer API identifiers (P21).

---

## Appendix A. Enumeration values

Source: live in-game dump (S3), Reported; Font values Confirmed against data (P12).
`Turbine.Language` in S2 also lists `Russian = 0x10000007`, absent from the live dump.

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

AliceBlue #F0F8FF, AntiqueWhite #FAEBD7, Aqua #00FFFF, Aquamarine #7FFFD4, Azure #F0FFFF, Beige #F5F5DC, Bisque #FFE4C4, Black #000000, BlanchedAlmond #FFEBCD, Blue #0000FF, BlueViolet #8A2BE2, Brown #A52A2A, BurlyWood #DEB887, CadetBlue #5F9EA0, Chartreuse #7FFF00, Chocolate #D2691E, Coral #FF7F50, CornflowerBlue #6495ED, Cornsilk #FFF8DC, Crimson #DC143C, Cyan #00FFFF, DarkBlue #00008B, DarkCyan #008B8B, DarkGoldenrod #B8860B, DarkGray #A9A9A9, DarkGreen #006400, DarkKhaki #BDB76B, DarkMagenta #8B008B, DarkOliveGreen #556B2F, DarkOrange #FF8C00, DarkOrchid #9932CC, DarkRed #8B0000, DarkSalmon #E9967A, DarkSeaGreen #8FBC8B, DarkSlateBlue #483D8B, DarkSlateGray #2F4F4F, DarkTurquoise #00CED1, DarkViolet #9400D3, DeepPink #FF1493, DeepSkyBlue #00BFFF, DimGray #696969, DodgerBlue #1E90FF, Firebrick #B22222, FloralWhite #FFFAF0, ForestGreen #228B22, Fuchsia #FF00FF, Gainsboro #DCDCDC, GhostWhite #F8F8FF, Gold #FFD700, Goldenrod #DAA520, Gray #808080, Green #008000, GreenYellow #ADFF2F, Honeydew #F0FFF0, HotPink #FF69B4, IndianRed #CD5C5C, Indigo #4B0082, Ivory #FFFFF0, Khaki #F0E68C, Lavender #E6E6FA, LavenderBlush #FFF0F5, LawnGreen #7CFC00, LemonChiffon #FFFACD, LightBlue #ADD8E6, LightCoral #F08080, LightCyan #E0FFFF, LightGoldenrodYellow #FAFAD2, LightGray #D3D3D3, LightGreen #90EE90, LightPink #FFB6C1, LightSalmon #FFA07A, LightSeaGreen #20B2AA, LightSkyBlue #87CEFA, LightSlateGray #778899, LightSteelBlue #B0C4DE, LightYellow #FFFFE0, Lime #00FF00, LimeGreen #32CD32, Linen #FAF0E6, Magenta #FF00FF, Maroon #800000, MediumAquamarine #66CDAA, MediumBlue #0000CD, MediumOrchid #BA55D3, MediumPurple #9370DB, MediumSeaGreen #3CB371, MediumSlateBlue #7B68EE, MediumSpringGreen #00FA9A, MediumTurquoise #48D1CC, MediumVioletRed #C71585, MidnightBlue #191970, MintCream #F5FFFA, MistyRose #FFE4E1, Moccasin #FFE4B5, NavajoWhite #FFDEAD, Navy #000080, OldLace #FDF5E6, Olive #808000, OliveDrab #6B8E23, Orange #FFA500, OrangeRed #FF4500, Orchid #DA70D6, PaleGoldenrod #EEE8AA, PaleGreen #98FB98, PaleTurquoise #AFEEEE, PaleVioletRed #DB7093, PapayaWhip #FFEFD5, PeachPuff #FFDAB9, Peru #CD853F, Pink #FFC0CB, Plum #DDA0DD, PowderBlue #B0E0E6, Purple #800080, Red #FF0000, RosyBrown #BC8F8F, RoyalBlue #4169E1, SaddleBrown #8B4513, Salmon #FA8072, SandyBrown #F4A460, SeaGreen #2E8B57, SeaShell #FFF5EE, Sienna #A0522D, Silver #C0C0C0, SkyBlue #87CEEB, SlateBlue #6A5ACD, SlateGray #708090, Snow #FFFAFA, SpringGreen #00FF7F, SteelBlue #4682B4, Tan #D2B48C, Teal #008080, Thistle #D8BFD8, Tomato #FF6347, Transparent #FFFFFF (A = 0), Turquoise #40E0D0, Violet #EE82EE, Wheat #F5DEB3, White #FFFFFF, WhiteSmoke #F5F5F5, Yellow #FFFF00, YellowGreen #9ACD32

---

## Change log

| Date | Change |
|---|---|
| 2026-10-04 | First draft (observations made 2026-10-03). |

---

## Appendix B. Implementation notes from `crates/script` (clean room, 2026-10-04)

Choices the first implementation made where this spec is silent. Each is conservative and
easy to change once the fact is known; spec writers should confirm or correct them.

> **OPEN:** `PluginData.Load(scope, key, handler)` when the data is available immediately.
> Tessera returns the value **and** also calls `handler(value)` on the next pump, so plugins
> that rely only on the callback still work. Confirm whether the original calls the handler
> in this case.

> **OPEN:** `PluginData.Save` with a fourth argument that is neither `nil` nor callable.
> Tessera ignores it (logged at debug level) instead of raising. A `Save` failure with no
> callback raises a Lua error (§10.5 OPEN).

> **OPEN:** fields of the rows returned by `PluginManager.GetAvailablePlugins()` and
> `GetLoadedPlugins()`. Tessera: `Name`, `Author`, `Version`, `Description` (when present),
> plus `ScriptState` (apartment name) on loaded rows only.

> **OPEN:** whether `PluginManager.LoadPlugin`, `UnloadScriptState` and
> `RefreshAvailablePlugins` act immediately when called from a script. Tessera queues them
> and carries them out after the current script call returns (next host pump), which avoids
> re-entering a state from inside itself.

> **OPEN:** `Shell.WriteLine` echo to `Turbine.Chat.Received`: Tessera uses `Sender = ""`
> and delivers the echo on the next pump, never synchronously inside `WriteLine`.

> **OPEN:** default results of `ShellCommand:GetHelp()` / `GetShortHelp()` when a plugin does
> not override them. Tessera returns `nil`.

> **OPEN:** `ShellCommand:Execute(commandName, argumentText)` details. Tessera passes the
> command word as typed (case preserved, no leading `/`), removes exactly one separating
> whitespace character, keeps trailing whitespace, and passes `nil` when nothing follows the
> command word. The name `plugins` is reserved by the host and cannot be registered by
> plugins; `/plugin` (singular) is not an alias.

> **OPEN:** `Engine.GetScriptVersion()` values. Tessera's default host returns `0, 0` until
> the real numbers are known. Default `GetDate()` (host without time-zone data) is UTC with
> `IsDST = false`, `DayOfWeek` 1 = Sunday … 7 = Saturday, `DayOfYear` 1-based.

> **OPEN:** whether the original strips a UTF-8 byte-order mark at the start of a `.lua`
> file. Tessera strips it (editors add it; Lua 5.1 itself would report a syntax error).

> **OPEN:** whether an infinite loop inside a coroutine is caught by the per-call budget in
> the original. Tessera's instruction budget (§3.3, default 500 million VM instructions per
> host-to-Lua call, 64 MiB memory per apartment) also covers coroutines.

> **OPEN:** the vendored interpreter is PUC-Rio Lua 5.1.5 (bug-fix release of 5.1.4, P1).
> Any 5.1.4 vs 5.1.5 behaviour difference that plugins depend on is unknown.
