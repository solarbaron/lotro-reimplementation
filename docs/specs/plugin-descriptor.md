<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Plugin discovery and the `.plugin` descriptor

| Field | Value |
|---|---|
| Status | **Revised** (2026-10-06: cross-references to the Accepted decisions 0014 and 0015 in §4.2, §6.2, §8.1 and §8.2, see the change log; repaired the same day after its first review, last change-log row; awaiting review of this change. No fact about the original changed; every other section is unchanged from the Accepted revision) |
| Last updated | 2026-10-06 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | Claude review session (spec-writer agent) |
| Review record | Claude review session (spec-writer agent), 2026-10-05: changes requested (descriptor field rules D6–D8 and §5 contradict the client; D14 key-character rule wrong; stale against `lua-plugin-api.md` §20; broken internal section references; quoted third-party/client prose; D4 mis-marked Confirmed). Revised the same day; every item is addressed (see the change log). Re-review, Claude review session (spec-writer agent), 2026-10-05: changes requested (D14/§11.1 and D21/§7.1 state ASCII-only as the client's rule, but the client applies the C runtime `isalnum` in the game-language locale it sets at start-up; contradicts `lua-plugin-api.md` P5, P14, P43, P44 and its §20 D14 row, and §12.2 row 7; D14 "Confirmed (folders)" overclaims; naming note's LEGAL §10 citation; §4.2 ordering claim unsourced). Revised 2026-10-06; every item is addressed (see the change log). Third review, Claude review session (spec-writer agent), 2026-10-06: **passed**, status Accepted. Every `CLEAN_ROOM.md` §5.3 item holds, and the spec can be implemented without dirty material. Every item from the 2026-10-05 re-review is resolved: the client rule is now stated as `isalnum` in the start-up locale, and ASCII-only is a stated Tessera deviation; D14's Confirmed mark is narrowed; the naming-note citation is fixed; the ordering claim is sourced (D33/D34); §12.2 row 7 agrees with `lua-plugin-api.md` P5, P14, P43, P44 and its §20 D14 row. Spot checks on the owner's install (2026-10-06): D1 (no `Plugins/` and no `.plugin` file in the settings folder), D13 (three `PluginOptions.xml` files, each with a UTF-8 declaration with `standalone="no"`, CRLF line ends, an empty `AutoloadOptions` and, in one file, an empty `ScalingOptions`), D14 (only the `PluginData/<account>/<world>/` level exists and there is no `.plugindata` file), and D15 (three world folders, one with a bracketed prefix and a space). BIN spot checks on build SHA-256 prefix `d22363eb9aff0fb5`: D30 (one all-categories `setlocale` call choosing among the four names, with `English` as the default; the other calls set only the numeric category); D14 and D21 (a key unit is zero-extended and a package byte is sign-extended before `isalnum`); D33 (narrow `_findfirst64i32`/`_findnext64i32` from the UCRT); D12 (no `compendium` string). The CRT reading of D32 also matches the published UCRT and Wine sources for single-byte locales. Non-blocking suggestion for the next revision: in the WINE source row, describe the directory-cache sort in words instead of naming the Wine function. **Review of the naming-note revision: 2026-10-06, Claude review session (spec-writer agent), a different session from the author, against `CLEAN_ROOM.md` §5.3. Result: passed; status Accepted.** The diff against the committed version touches only the status row, the naming note and a change-log row. The note correctly cites `docs/LEGAL.md` §17 and agrees with its §17.2 rules 1, 2 and 4. Spot checks: on the owner's settings folder (read-only), D1 (no `Plugins/`, no `.plugin` or `.plugindata` file), D13 (three `PluginOptions.xml` files, each with `encoding="UTF-8"`, `standalone="no"`, CRLF and an empty `AutoloadOptions`, one with `ScalingOptions`), D14 (only the `PluginData/<account>/<world>/` level) and D15 (three world folders, one bracketed with a space). On the client (SHA-256 prefix `d22363eb9aff0fb5`), D12: no `compendium` string in ASCII or UTF-16. Non-blocking: the earlier suggestion about the WINE source row still applies. |
| Implements into | `crates/plugins` (discovery, descriptor parse, apartments, package resolution), with `crates/install` (settings-folder location) and `crates/script` (Lua host) |
| Related specs | [`lua-plugin-api.md`](lua-plugin-api.md) (the Lua runtime, apartment states, `import` internals and the API surface; this spec is the disk-and-discovery companion to it and agrees with its §4, §5, §6.1, §7.2 and §10.2), [`install-layout.md`](install-layout.md) §8 (settings folder and its contents), [`plugin-runtime.md`](plugin-runtime.md) §8.4 (where loads and unloads run in Tessera's frame); decision records [`0014-ui-input-bridge.md`](../decisions/0014-ui-input-bridge.md) (D8) and [`0015-ui-rendering.md`](../decisions/0015-ui-rendering.md) (§4.4, the plugin file index) |

> **Naming note (interoperability).** The XML element and attribute names (`Plugin`,
> `Information`, `Name`, `Author`, `Version`, `Description`, `Image`, `Package`,
> `Configuration`, `Apartment`), the apartment name `Default`, the folder and extension names
> (`Plugins`, `.plugin`, `.plugindata`, `__init__.lua`, `AllServers`, `AllCharacters`), the
> `PluginOptions.xml` element and attribute names and values (`PluginOptions`,
> `AutoloadOptions`, `ScalingOptions`, `Plugin`, `Name`, `AllCharacters`, `Character`,
> `UseScaling`, `true`, `True`) and the native package name `Turbine` are the identifiers the
> original client and existing third-party plugins use. They are reproduced only because a
> compatible implementation cannot read the same files without them. The project's
> interoperability record for these names is `docs/LEGAL.md` §17 (it answers the "OPEN
> (legal)" item of `docs/ARCHITECTURE.md` §18.2; `docs/LEGAL.md` §10 governs naming in
> general). Its §17.2 rules bind every use: the names appear in Tessera only inside the plugin
> subsystem, as string data, never as product, crate, module or UI names; Tessera reads the
> user's descriptors and `PluginOptions.xml` in place and read-only and writes its own settings
> elsewhere (§17.2 rule 4). All descriptive text
> here is our own; no third-party prose, documentation wording or client message text is
> reproduced.

---

## 1. Purpose

This spec lets an implementer build the on-disk side of the plugin host without reading any
dirty material:

- where plugins live and how the client discovers them (§4);
- the `.plugin` XML descriptor: its structure, how the client parses it, and exactly what it
  accepts or rejects, with worked examples (§5);
- the apartment (script-state) model as it is expressed on disk (§6);
- how a dotted package name resolves to a Lua file (§7);
- load order and lifecycle at the discovery level (§8);
- the companion/compendium files the client does **not** read (§9);
- the per-account/per-world `PluginOptions.xml` the in-game manager writes (§10);
- the plugin-data facts that concern files and folders (§11).

The Lua sandbox, the `Turbine` API surface, the object and event model, and plugin-data
serialisation are owned by [`lua-plugin-api.md`](lua-plugin-api.md). This spec states only the
facts that govern files and folders. Where both specs state a fact they agree; the corrections
each made to the other are listed in §12.

---

## 2. Provenance

Source keys:

| Key | Source |
|---|---|
| **OBS** | The owner's install, read-only, 2026-10-05 (re-checked the same day for this revision): the settings folder of the Steam/Proton prefix (`…/compatdata/212500/pfx/drive_c/users/steamuser/Documents/The Lord of the Rings Online/`). It holds `UserPreferences.ini`, `lotro.keymap`, `CEF/`, and `PluginData/<account>/<world>/PluginOptions.xml` for three worlds. It has **no** `Plugins/` folder and no `.plugin` file anywhere (this owner has installed no plugins). The in-prefix settings folder is the only one present. |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix `d22363eb9aff0fb5`, re-checked for this revision), with its imports from `xerces-c_2_6.dll` (the XML parser) and `lua51.dll`; read with capstone on individual functions in the dirty room on 2026-10-05, first draft and again for this revision. **Questions:** (a) what the descriptor parser reads, in what structure, and what it rejects; (b) the per-field length limits and trimming; (c) how a package name is checked and resolves to a file, the precedence of the folder form over the file form, and the native/disk merge; (d) how discovery lists and filters directory entries, how deep it looks, how it treats duplicate names, and whether it reads companion files; (e) the structure and value rules of the per-account/world options file; (f) the plugin-data key rules; (g) how apartment names compare; (h) how a disk chunk is loaded. **Why M1–M3 were not enough:** the acceptance and rejection rules, resolution precedence, depth and duplicate handling are code, not data, and no public source describes them at the needed precision (the community schema even disagrees with the client on one point, D18); this owner's install has no `Plugins/` tree and only empty options files, so OBS can confirm the folder layout and the empty file form but no parser branch; no running official client with third-party plugins is available to this session for M3. **Re-read 2026-10-06** (same build, SHA-256 prefix re-checked) for the second revision, function by function: the plugin-data key validator and the package-name check of the disk resolver (how each passes its characters to `isalnum`), the start-up `setlocale` call, the directory-listing routine that discovery calls, and the discovery loop. Questions: (f) and (c) again, plus (i) which listing API discovery uses and whether it reorders entries. Only offline plugin-host, XML and file code was read: nothing about network, login, keys, the launcher or anti-tamper. Numeric constants below are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). |
| **XSD** | `lunarwtr/vscode-lotro-api` (MIT), `resources/xsds/lotroplugin.xsd` (also vendored in OneLauncher), `plugincompendium.xsd`, `skincompendium.xsd`, `musiccompendium.xsd`, `compendium-basetypes.xsd` (community schemas; facts only). |
| **DOC** | Official "Update 25" Lua API reference HTML (© 2016 publisher), mirrored in `lunarwtr/lotro-api-docs` (`U25/`) and `Goldbishop/Lotro_LUA` (`U24/`): the `PluginManager` and `Plugin` pages, including `RefreshAvailablePlugins`. Facts only; no wording reproduced. |
| **SMP** | Sample `.plugin` files read in the dirty room: the publisher's 2010 beta samples (`Goldbishop/Lotro_LUA`, `dist/Beta_LuaPlugins.zip`) and two modern descriptors (`LotroCompanion/lotro-companion-plugin`, `lunarwtr` TurbineDumper). Nothing from these files is reproduced; only structure and shapes. |
| **OL** | `JuneStepp/OneLauncher` (GPL-3), `src/onelauncher/addon_manager_window.py`: a third-party plugin manager's discovery and install logic (facts only). |
| **XML** | The XML 1.0 recommendation (general knowledge, not dirty): §4.3.3 (encoding detection; bytes illegal in the detected encoding are a fatal error) and the DOM Level 3 `textContent` rule. |
| **LPA** | [`lua-plugin-api.md`](lua-plugin-api.md), whose BIN rows were made from the same client build. Cited where this spec relies on it rather than re-reading the code. |
| **CRT** | Published C runtime sources, read 2026-10-06 for facts only: the Microsoft Universal C Runtime as vendored in ReactOS (`sdk/lib/ucrt/convert/_ctype.cpp` for character classification; `sdk/lib/ucrt/filesystem/findfile.cpp` and `sdk/lib/ucrt/inc/corecrt_internal_win32_buffer.h` for the narrow find functions and their code page) and Wine's `dlls/msvcrt/ctype.c`. The two runtimes state the same rule for `isalnum` (D32); Wine's narrow find functions were not checked (D35). Not observed on a running client. |
| **WINE** | Wine's `dlls/ntdll/unix/file.c` (directory cache: entries other than `.` and `..` are sorted by `name_compare`) and its case-insensitive wide compare (upper-case folding with Wine's Unicode table), read 2026-10-06 for facts only. Relevant because the owner runs the client under Steam Proton. |
| **NTFS** | General platform knowledge (not dirty): an NTFS directory index is kept sorted by file name compared after upper-casing each UTF-16 unit with the volume's upper-case table, and directory enumeration returns entries in index order. |

Marks: **Confirmed** = observed in the owner's install, with date; **BIN** = rests on the
disassembly (for BIN facts, no contradicting data was found); **Reported** = from published
sources only, not observed here.

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| D1 | Plugins live under a `Plugins/` folder inside the per-user settings folder; the client does not create it (the user or a manager does). The owner's settings folder has no `Plugins/`, so discovery yields nothing here | OBS (no `Plugins/`; `PluginData/` present); DOC; OL; LPA §4.1 | **Confirmed** 2026-10-05 (folder absent) / Reported (creation rule) |
| D2 | Discovery keeps files whose name matches the wildcard `*.plugin`; the extension is held as the bare token `plugin` and the pattern is formed from it. The wildcard matcher supports `*` and `?` and compares characters case-insensitively. No `plugincompendium`, `skincompendium`, `musiccompendium` or `compendium` string exists in the client | BIN (d) | BIN |
| D3 | Discovery reads exactly one folder level: it lists `Plugins/` and keeps only its subdirectories (skipping `.` and `..`), then lists the files directly inside each subdirectory. Files directly in `Plugins/` and anything deeper than one subdirectory are never seen. Neither listing recurses | BIN (d) (the listing routine recurses only when a flag is set; discovery passes it clear in both calls); LPA P27 | BIN |
| D4 | The descriptor root element must be named exactly `Plugin` (case-sensitive); any other root rejects the descriptor | BIN (a); XSD; SMP | BIN / Reported (XSD, SMP) |
| D5 | Below the root, the document must contain exactly one element named `Information` and exactly one named `Package`, and at most one named `Configuration`. Each count covers descendants of the root at **any depth**: the client obtains them with the DOM by-name descendant query on the root element. Counts outside these ranges reject | BIN (a) | BIN |
| D6 | Only the **direct** child elements of `Information` are examined. A child named exactly `Name`, `Version`, `Author`, `Description` or `Image` sets that field; any other child node is ignored. A second child of the same field name rejects the descriptor | BIN (a); LPA P28 | BIN |
| D7 | Field limits, in UTF-16 code units after trimming: `Name` and `Version` must be 1–120 (longer rejects); `Author` and `Image` are cut to their first 120; `Description` is cut to its first 2048 (0x800); any of the five that is present but empty after trimming rejects; `Name` is required (absent rejects) | BIN (b); LPA P28 | BIN |
| D8 | `Package` text, trimmed, must be 1–120 UTF-16 code units; empty or longer rejects | BIN (a, b) | BIN |
| D9 | `Configuration` may carry an attribute `Apartment`. When there is no `Configuration`, or a `Configuration` without `Apartment`, the plugin's apartment is `Default` (pre-set before the parse) | BIN (a); LPA P11 | BIN |
| D10 | A dotted package `C1.….Ck` resolves against `Plugins/C1/…/Ck`: if that path is an existing directory, the chunk is its `__init__.lua`; otherwise the chunk is `Ck.lua` beside it. A directory without `__init__.lua` does not fall back to the `.lua` file | BIN (c); LPA P5 | BIN |
| D11 | In plugin apartments a native package name is **also** looked up on disk; when both a native package and a disk chunk exist, the package is their merge (native part installed, then the disk chunk run in the same namespace) | BIN (c) (the native hit does not end the lookup when the apartment's plugin flag is set); LPA §7.2, §7.4, P30 | BIN |
| D12 | The client does not read `.plugincompendium`, `.skincompendium` or `.musiccompendium` files; these are third-party manager metadata only | BIN (d) (no such strings in the client); OL; XSD | BIN |
| D13 | The in-game manager stores its choices per account and world at `PluginData/<account>/<world>/PluginOptions.xml`: root `PluginOptions` with `AutoloadOptions` and (in newer files) `ScalingOptions`. Populated structure in §10 | OBS (three files: XML declaration with `encoding="UTF-8" standalone="no"`, CRLF line ends, root, empty `AutoloadOptions`; one file also has an empty `ScalingOptions`); BIN (e) (writer and reader); LPA P23 | **Confirmed** 2026-10-05 (empty form) / BIN (populated structure) |
| D14 | A plugin-data key must be 1 to 64 UTF-16 units. Each unit must be `_` (0x5F) or pass the C runtime's `isalnum` test, to which the client passes the unit's value unchanged (zero-extended), in the locale set at start-up (D30). ASCII letters, ASCII digits and `_` therefore always pass; which other units pass is D31/D32. Data is stored as `<key>.plugindata` under `PluginData/<account>/AllServers/`, `PluginData/<account>/<world>/AllCharacters/` or `PluginData/<account>/<world>/<character>/` | BIN (f) (re-read 2026-10-06: length check, then one `isalnum` call per unit, `_` accepted); OBS (only `PluginData/<account>/<world>/` exists in the owner's settings folder, and no `.plugindata` file); LPA P14, §10.2–§10.3 | BIN (key rule; `AllServers`, `AllCharacters`, `<character>` folders and the `.plugindata` name, also LPA) / **Confirmed** 2026-10-05, re-checked 2026-10-06 (`PluginData/<account>/<world>/` level only) |
| D15 | World folder names under `PluginData/<account>/` are the display names of the worlds and may contain spaces and other punctuation (one observed name has a bracketed prefix and a space) | OBS (three world folders, one bracketed) | **Confirmed** 2026-10-05 |
| D16 | The `Image` file is a JPG or TGA icon shown at 32×32 pixels; a larger image is cropped to that size and a smaller one is repeated (tiled) to fill it | XSD | Reported |
| D17 | Refresh rebuilds the available list from disk; it does not unload a loaded plugin whose descriptor has been removed, which then simply stops being listed | DOC; LPA §4.3 step 5 | Reported |
| D18 | The `Apartment` attribute is optional (the community schema marks it required; the client does not). When present, its value is used exactly as written: not trimmed, no length check, and an empty value is accepted | BIN (a, g); XSD (disagrees on "required", resolved by BIN) | BIN |
| D19 | Namespace of a resolved package: `C1.….Ck` for the folder form and for a native package; `C1.….C(k-1)` for the file form (for `k = 1`, the apartment's global table) | BIN (c); LPA §7.2 | BIN |
| D20 | A disk chunk is loaded with the stock Lua 5.1.4 file loader of `lua51.dll`, which accepts both source text and precompiled chunks and skips a first line that starts with `#`; it does not strip a UTF-8 byte-order mark | BIN (h) (the resolver calls the library's file loader); LPA §7.2, Appendix B; Lua 5.1 reference behaviour | BIN |
| D21 | A package name is checked before any lookup: components separated by single dots, with no leading, trailing or doubled dot; every other byte of the (UTF-8) name must be `_` (0x5F) or pass the C runtime's `isalnum` test in the locale of D30, the byte being passed **sign-extended** (bytes 0x80–0xFF arrive as −128 to −1). ASCII letters, ASCII digits and `_` always pass; bytes 0x80–0xFF are D32. A name that fails the check does not resolve from disk | BIN (c) (re-read 2026-10-06: one `isalnum` call per byte with sign extension, `.` as separator, `_` accepted); LPA P5, §7.2 | BIN |
| D22 | The descriptor is parsed by a general DOM parser (Xerces-C 2.6) with an error handler that counts errors; the parse succeeds only when that count is zero. Encoding is taken from a byte-order mark or the encoding declaration, otherwise UTF-8; bytes illegal in that encoding are a fatal XML error and therefore reject | BIN (a) (parser library, error-count test); XML §4.3.3 | BIN / Reported (encoding-error rule, not exercised on this install) |
| D23 | A field's value is the element's text content: the concatenated text of all its descendant text and CDATA nodes, in document order, excluding comments and processing instructions | BIN (a) (the DOM text-content call); XML (DOM Level 3 rule) | BIN |
| D24 | Trimming removes leading and trailing characters from the set space (0x20), line feed (0x0A), carriage return (0x0D), tab (0x09) and form feed (0x0C). A value made only of these characters trims to empty | BIN (b) (the trim routine and its character-set constant); LPA §5.2 | BIN |
| D25 | Discovery does not check that a matched entry is a regular file; a directory whose name matches `*.plugin` fails to parse and is dropped like any invalid descriptor | BIN (d) | BIN |
| D26 | The available list is a map keyed by the descriptor's `Name`, compared case-insensitively; when a name is already present, the later descriptor is dropped (first found wins). A rejected descriptor never occupies a name | BIN (d) (case-folding hash plus case-insensitive wide compare; find-then-insert); LPA P27 | BIN |
| D27 | Apartment names compare case-insensitively | BIN (g) (the apartment map hashes lower-cased characters and compares case-insensitively); LPA P11 | BIN |
| D28 | `PluginOptions.xml` reading rules: `AllCharacters` is true only when its value is exactly `true`, `UseScaling` only when exactly `True` (case-sensitive; anything else is false). Autoload entries are kept in a map keyed by plugin `Name` with a case-sensitive hash; a later entry with a name already present is dropped. The file order of entries is therefore not kept | BIN (e) | BIN |
| D29 | While a plugin's main package runs, the plugin's apartment carries a "loading a plugin" mark, which is cleared afterwards (this changes `PluginData.Load`, LPA §10.1). No per-plugin "loading" log line was found in the load path | BIN; LPA §6.2 steps 4 and 7 | BIN |
| D30 | At start-up the client sets the C runtime locale for all categories to a name chosen by its game language: `English` (also for any language not listed), `english-uk`, `french` or `german`. That global locale governs the `isalnum` tests of D14 and D21 | BIN (re-read 2026-10-06: one `setlocale` call for all categories with one of these four names; the client's other `setlocale` calls touch only the numeric category); LPA P43 | BIN |
| D31 | Under the standard Windows locale data these four names select code page 1252. A value 0x00–0xFF passed to `isalnum` is classified by that locale's character-type table, which is indexed by the value as a code-page-1252 byte. For a plugin-data key unit 0xA0–0xFF this coincides with the Latin-1 character of the same value (for example U+00E9 passes); a unit 0x80–0x9F (a C1 control in UTF-16) is classified as the code-page-1252 character at that byte position, not as a control | LPA P44; CRT (the table is indexed by the value) | Reported |
| D32 | In a single-byte locale such as code page 1252, `isalnum` returns false for every argument below −1 or above 0xFF, and the table entry for −1 is "no class". Consequences: no package-name byte 0x80–0xFF ever passes D21 (so a package name with any non-ASCII byte never resolves from disk), and no plugin-data key unit above 0xFF passes D14 | CRT (Microsoft UCRT source and Wine agree); not observed on a running client | Reported |
| D33 | Discovery lists folders with a narrow (byte-string) C runtime find-first/find-next pair over the pattern `*.*`. Each entry other than `.` and `..` is tested against the wanted name pattern and appended to the end of the result list; subdirectories are appended to a second list. The client never sorts either list, so "first found" is the order the operating system returns. Paths and names stay narrow byte strings through discovery and the descriptor open | BIN (i) (listing routine and discovery loop re-read 2026-10-06) | BIN |
| D34 | Listing order returned to the client: on NTFS, ascending order of names after upper-casing each UTF-16 unit; under Wine/Proton (the owner's setup), Wine sorts each directory's entries (after `.` and `..`) by a case-insensitive comparison that upper-cases each UTF-16 unit, then by a case-sensitive comparison as tie-break. Other file systems (FAT, exFAT, network shares) may return other orders | NTFS; WINE | Reported |
| D35 | Because the listing is narrow, the operating system converts each name to the process's ANSI code page (not the C runtime locale of D30). A folder or file name with a character outside that code page reaches the client altered (best-fit or `?`), so such a descriptor or plugin folder may fail to open | CRT (the UCRT narrow find functions call the wide find API and convert each name to the ANSI code page, or the OEM code page when file APIs are set to OEM, or UTF-8 when the C runtime locale's code page is UTF-8, which D30's locales are not; no replacement character is specified, so the system default applies); BIN (D33) | Reported |

---

## 3. Scope and terms

<a id="3-1"></a>
### 3.1 Terms

| Term | Meaning |
|---|---|
| **Settings folder** | The per-user folder that holds `UserPreferences.ini`, `Plugins/`, `PluginData/` and so on. Its location is `install-layout.md` §8; for a Steam/Proton install it is inside the prefix under `…/users/steamuser/Documents/The Lord of the Rings Online/`. |
| **Plugins folder** | `Plugins/` inside the settings folder. The root of plugin discovery and of package resolution. |
| **Descriptor** | A `.plugin` file: the XML document that declares one plugin (§5). |
| **Package** | A dotted name (`Author.Name.File`) that names a Lua chunk, resolved relative to the Plugins folder (§7). |
| **Apartment** | A named Lua script state. Plugins that name the same apartment share one state; unloading unloads a whole apartment (§6). |
| **Compendium / companion file** | A `.plugincompendium` / `.skincompendium` / `.musiccompendium` file written by third-party managers. The client ignores these (§9). |
| **Character count** | Every length in this spec counts UTF-16 code units of the value after decoding (the client holds these strings as UTF-16). A character outside the Basic Multilingual Plane counts as 2. |

<a id="3-2"></a>
### 3.2 Case and path rules

The game is a Windows program; its file system is case-insensitive. Name comparisons that
touch the file system (folder names, the `plugin` extension, package components used as paths)
are **ASCII case-insensitive**, following `install-layout.md` §2 and `lua-plugin-api.md` §4.4.
On a case-sensitive host, resolve each path component by listing its parent and matching
case-insensitively; if several entries match, take the first in byte order (the same rule as
`lua-plugin-api.md` §4.4) and log a warning. No resolved path may leave the Plugins folder:
reject absolute paths, drive letters, empty components, `.`, `..` and NUL, and do not follow
symlinks out of the tree (Tessera policy, `docs/ARCHITECTURE.md` §18.2).

XML element and attribute names, by contrast, compare **case-sensitively** and by their
qualified name as written (D4, D6): `plugin`, `NAME` or `x:Name` do not match.

---

## 4. Folder layout and discovery

<a id="4-1"></a>
### 4.1 Layout

| Path pattern | Meaning |
|---|---|
| `Plugins/` | Root of discovery, inside the settings folder. Not created by the client; created by the user or a manager (D1). |
| `Plugins/<Folder>/<Any>.plugin` | A descriptor (§5). The **only** place the client looks for descriptors (D3). Convention: `<Folder>` is the author's folder (SMP, OL). |
| `Plugins/<Folder>/<Name>/…` | The plugin's Lua files and resources, addressed by dotted packages (§7). |
| `Plugins/<Folder>/<Any>.plugincompendium` and `.skincompendium`, `.musiccompendium` | Third-party manager metadata. The client ignores them (§9). |
| `Plugins/Turbine/…`, `Plugins/TurbinePlugins/…` | The publisher's 2010 sample utility package and example plugins, installed by users when an older plugin needs them (SMP). Ordinary user content; **Tessera must not ship them**. A `Plugins/Turbine/` folder is merged into the native `Turbine` package (§7.2, D11). |

<a id="4-2"></a>
### 4.2 Discovery algorithm

The client scans for descriptors at client start and on an explicit refresh (the `/plugins
refresh` chat command and `PluginManager.RefreshAvailablePlugins()`, `lua-plugin-api.md`
§6.4, §6.7). Each scan rebuilds the list of **available** plugins (D17):

1. Resolve the Plugins folder (§3.1). If it does not exist, the available list is empty and
   discovery stops without error (the owner's install is in this state, OBS).
2. List the entries directly inside the Plugins folder. Skip `.` and `..`. Keep only the
   entries that are directories; files at this level are ignored (D3).
3. For each kept directory, in the order the listing returned them, list the entries directly
   inside it whose name matches the wildcard `*.plugin` (D2). The match is over the whole
   name, case-insensitive, and `*` may match nothing, so `x.plugin`, `X.PLUGIN`, `a.b.plugin`
   and `.plugin` all match, while `x.plugin.bak` and `x.plugins` do not. Do not descend
   further: subdirectories of the author folder are never searched (D3).
4. Parse each matched entry as a descriptor (§5). The entry is not first checked to be a
   regular file; a directory with a matching name simply fails to parse (D25). A descriptor
   that fails is dropped; it does not abort discovery. The original drops it silently;
   Tessera logs a warning naming the file and the rule that failed.
5. Add each accepted descriptor to the available map under its `Name`, compared
   case-insensitively. If the map already holds that name, the new descriptor is dropped:
   the **first one found wins** (D26). Tessera logs each dropped duplicate.
6. Refresh does not unload anything. A loaded plugin whose descriptor has disappeared stays
   loaded and is no longer listed (D17).

**Tessera, the plugin file index** (decision 0015 §4.4; not a fact about the original).
Besides the available list, Tessera keeps an index of every file in the whole `Plugins/`
tree, under the path rules of §3.2, which `SetBackground(path)` uses to check at call time
that an image exists. That index is built by a directory walk on an asset worker, never on
the main thread: once when the plugin host starts (the host loads no plugin before the
first index is ready, and the main thread does not wait for it) and again on every refresh
of this section. A rebuilt index replaces the previous one as a whole, on the main thread,
when it is ready; until then lookups use the previous one. Whether the descriptor scan of
this section itself runs at once or deferred is not observable to plugins except through
`GetAvailablePlugins` (`lua-plugin-api.md` Appendix B, row on `RefreshAvailablePlugins`).

**Order.** The original never sorts: both listings keep the order in which the operating
system returns entries (D33). That order is the file system's, not the client's (D34): on
NTFS it is ascending name order after upper-casing each UTF-16 unit; under Wine/Proton, Wine
sorts each directory's entries the same way (upper-case folding) and breaks ties between
names that are equal after folding by comparing them case-sensitively; FAT, exFAT and network
shares may return other orders. So "first found" depends on folder and file names and, on
some file systems, on history.

Tessera does not depend on the host's order. It sorts both listings itself by this exact
rule, its definition of the "case-insensitive byte order" of `lua-plugin-api.md` §4.3:

1. Take each name as its UTF-8 bytes.
2. Form a key by replacing every byte 0x61–0x7A (`a`–`z`) with that byte minus 0x20 (`A`–`Z`).
   No other byte changes.
3. Compare two keys byte by byte as unsigned values; the first differing byte decides. If one
   key is a prefix of the other, the shorter sorts first.
4. If the keys are equal (possible only on a case-sensitive host), compare the unmodified
   UTF-8 bytes the same way.

For names made only of ASCII characters this gives the same order as NTFS and Wine (both fold
to upper case, so `_`, 0x5F, sorts **after** the letters). For names with non-ASCII
characters it can differ, because Tessera folds ASCII letters only; this is accepted.

**Names on disk.** The original receives names as narrow strings in the process's ANSI code
page (D33, D35), so a folder or file name with a character outside that code page may reach
it altered and fail to open. Tessera reads names in the host's native form and does not
reproduce this loss; a plugin that the original could not find because of its name may be
found by Tessera (a deliberate, widening deviation).

**Case folding of `Name`.** The original folds case with the C runtime's wide-character
case-insensitive comparison. Tessera folds ASCII letters only; names that differ only in the
case of non-ASCII letters are treated as distinct.

> **OPEN:** whether the original treats names that differ only in the case of non-ASCII letters
> (for example accented Latin letters) as the same name. This depends on the C runtime locale
> in effect at discovery time and was not determined. Low impact: no known plugin pair relies
> on it.

<a id="4-3"></a>
### 4.3 Invariants

- With no `Plugins/` folder, the available list is empty and no error is raised.
- A descriptor file directly in `Plugins/` (for example `Plugins/a.plugin`) is never listed.
- A descriptor two levels down (for example `Plugins/A/B/c.plugin`) is never listed.
- `Plugins/A/x.plugin` and `Plugins/A/Y.PLUGIN` are both found; `Plugins/A/x.plugin.bak` is
  not.
- Two valid descriptors with names `Alpha` and `ALPHA` yield one available entry: the one
  found first (in Tessera, the first in case-insensitive byte order of folder name, then file
  name).
- An invalid descriptor whose `Name` text would duplicate a valid one does not block the valid
  one, whatever the order.
- Order rule (Tessera): `Ab` sorts before `A_b` (keys `AB` and `A_B`; 0x42 is below 0x5F);
  `abc` sorts before `ABCD` (prefix); `B` and `b` have equal keys, and `B` sorts first by the
  tie-break (0x42 is below 0x62). So with folders `A_b/` and `Ab/` each holding a valid
  descriptor named `Same`, the one in `Ab/` is kept.

---

## 5. The `.plugin` descriptor

<a id="5-1"></a>
### 5.1 Document shape

An XML 1.0 document, parsed by a general DOM parser (D22). Observed files carry
`<?xml version="1.0"?>` (some add `encoding="utf-8"`), with tab or space indentation and
multi-line text (SMP).

"Occurs" counts are those the client enforces (D5–D9). Paths with a leading `/` are absolute;
the three paths marked "any depth" are counted among all descendants of the root, wherever
they sit.

| Element path | Occurs | Content | Meaning |
|---|---|---|---|
| `/Plugin` | exactly 1 (root) | element | Root. Its name must be exactly `Plugin` (D4). |
| `Information` (any depth) | exactly 1 | element | Metadata block (D5). Conventionally a direct child of the root. |
| `Information/Name` (direct child) | exactly 1 | text | Plugin name: the key for `/plugins load`, `PluginManager.LoadPlugin`, the manager list and the global `Plugins` table. 1–120 characters after trimming; longer rejects (D7). May contain spaces (SMP). |
| `Information/Version` (direct child) | 0 or 1 | text | Free-form string (for example `1.0`, `v0.0.1`, `2.6.2`); returned by `Plugin:GetVersion()`. If present: 1–120 characters after trimming; longer rejects (D7). |
| `Information/Author` (direct child) | 0 or 1 | text | Returned by `Plugin:GetAuthor()`. If present: non-empty after trimming; cut to its first 120 characters (D7). |
| `Information/Description` (direct child) | 0 or 1 | text | Shown in the manager; may span many lines (SMP, XSD). If present: non-empty after trimming; cut to its first 2048 characters (D7). Internal line breaks are kept. |
| `Information/Image` (direct child) | 0 or 1 | text | Path of the plugin's icon, relative to `Plugins/` (`lua-plugin-api.md` §4.4): a JPG or TGA image shown at 32×32 (D16). If present: non-empty after trimming; cut to its first 120 characters (D7). |
| `Package` (any depth) | exactly 1 | text | Dotted package path of the plugin's main Lua chunk (§7). 1–120 characters after trimming; longer rejects (D8). Conventionally a direct child of the root. |
| `Configuration` (any depth) | 0 or 1 | element | Present only to choose an apartment (§6). Its children and text are ignored. |
| `Configuration/@Apartment` | optional | attribute | Apartment name, used exactly as written (D18). |

<a id="5-2"></a>
### 5.2 Parse rules (BIN D4–D9, D18, D22–D24)

The descriptor is **accepted** only if every rule below holds. Rule order matters only in that
each rule is checked once; any single failure rejects the whole descriptor, and nothing of a
rejected descriptor is kept.

1. **Well-formed XML.** The file parses with no error reported by the XML parser (D22). The
   encoding is taken from a byte-order mark or the encoding declaration; with neither it is
   UTF-8. There is no fallback to Latin-1 or a system code page: bytes that are not legal in
   the detected encoding are a fatal XML error and reject the descriptor (§5.3).
2. **Root.** The document element is named exactly `Plugin`.
3. **One `Information`.** The number of elements named `Information` among all descendants of
   the root, at any depth, is exactly 1. The search is the DOM by-name descendant query on the
   root element, so an `Information` nested inside another element still counts (D5).
4. **Fields.** The direct children of that `Information` element are visited in document
   order. Children that are not elements (text, comments, CDATA, processing instructions) and
   elements with any other name are skipped. For each child element named `Name`, `Version`,
   `Author`, `Description` or `Image`:
   1. If the same field has already been set by an earlier child, the descriptor is rejected
      (a repeated field element is an error; D6).
   2. The raw value is the element's text content (D23): all descendant text and CDATA,
      concatenated, without comments. Predefined and character entity references are
      expanded by the parser.
   3. The value is trimmed of leading and trailing space, line feed, carriage return, tab and
      form feed (D24). A whitespace-only value becomes empty.
   4. Length rules on the trimmed value, in characters (§3.1):
      - `Name`, `Version`: longer than 120 rejects.
      - `Author`, `Image`: longer than 120 is cut to the first 120.
      - `Description`: longer than 2048 is cut to the first 2048.
      The cut is not followed by a second trim, so a cut value may end in whitespace.
   5. If the resulting value is empty, the descriptor is rejected.
5. **Name present.** After the walk, `Name` must have been set. A descriptor with no `Name`
   child is rejected.
6. **One `Package`.** Exactly one element named `Package` among all descendants of the root,
   at any depth. Its text content, trimmed as in step 4.3, must be 1–120 characters; empty or
   longer rejects (no cut).
7. **At most one `Configuration`.** Zero or one element named `Configuration` among all
   descendants of the root, at any depth; two or more reject.
8. **Apartment.** The apartment starts as `Default`. If there is a `Configuration` and it has
   an `Apartment` attribute, the apartment becomes that attribute's value exactly as written
   (after normal XML attribute-value normalisation by the parser, but not trimmed, not
   length-checked, and possibly empty; D18). A `Configuration` without `Apartment` keeps
   `Default`.

Fields that were not present (`Version`, `Author`, `Description`, `Image`) read as empty
strings.

Consequences worth stating plainly:

- Field elements outside `Information`, or nested one level deeper inside a child of
  `Information`, are ignored as fields; but a `Package` or `Configuration` anywhere in the
  document counts towards rule 6 or 7, and an extra `Information` anywhere counts towards
  rule 3.
- Element order inside `Information`, and the order of `Information`, `Package` and
  `Configuration` under the root, do not matter.

<a id="5-3"></a>
### 5.3 Robust-parsing requirements (Tessera)

Treat the file as hostile input:

- Bounded, non-validating parse. Reject files over a cap (64 KiB is ample), bound element
  counts and nesting depth, and do not resolve external entities. Tessera also rejects
  documents with a DTD. This is a deliberate deviation: the original's parser accepts an
  internal DTD subset, but no known descriptor uses one (SMP).
- Encoding: honour a byte-order mark (UTF-8, UTF-16 LE/BE) and the encoding declaration; with
  neither, decode as UTF-8. Treat invalid byte sequences as a parse error, which rejects the
  descriptor (D22, rule 1 of §5.2). Encodings named in a declaration that Tessera's decoder
  does not support also reject; log the name.
- Apply exactly the trim set, counts, length limits and cut rules of §5.2, counting UTF-16
  code units, so that the same files are accepted and the stored values match. When a cut
  would split a surrogate pair, Tessera drops the dangling high surrogate (the original keeps
  it; the difference cannot be seen through the plugin API except as an unpaired code unit).
- Comments are allowed anywhere and ignored (a 2010 sample comments out its `Configuration`,
  SMP; a commented-out element is not an element and is not counted).

<a id="5-4"></a>
### 5.4 Invariants

- A document whose root element is not named exactly `Plugin` is rejected.
- A document with zero or two-plus `Information` elements anywhere below the root, zero or
  two-plus `Package` elements anywhere below the root, or two-plus `Configuration` elements
  anywhere below the root is rejected.
- A document with two `Name` children (or two `Version`, `Author`, `Description` or `Image`
  children) of `Information` is rejected.
- A document with no `Name` child of `Information`, or whose `Name` is empty or whitespace
  only after trimming, is rejected.
- An accepted descriptor has a `Name` of 1–120 characters with no leading or trailing
  whitespace from the trim set, a `Package` of 1–120 characters, a `Version` of 0–120, an
  `Author` and `Image` of 0–120, and a `Description` of 0–2048.
- A `Name` or `Version` of 121 characters after trimming rejects; an `Author` of 121 reads back
  as its first 120; a `Description` of 3000 reads back as its first 2048.
- A descriptor with no `Configuration`, or with a `Configuration` lacking `Apartment`, yields
  apartment `Default`; one with `Apartment` value `X` yields exactly `X`, including leading or
  trailing spaces and the empty string.

<a id="5-5"></a>
### 5.5 Worked synthetic examples

Each row describes a synthetic descriptor by its element tree (all names and values are
invented for this spec). Unless the row says otherwise, the document is well-formed UTF-8 with
root `Plugin`, one `Information` child of the root, one `Package` child of the root whose text
is `Example.Alpha.Main`, and no `Configuration`. "_n_ × c" means a run of _n_ copies of the
character c.

| # | Variation | Outcome | Resulting values |
|---|---|---|---|
| E1 | `Information` has one child `Name` with text `Alpha` | accepted | Name `Alpha`; Version, Author, Description, Image empty; Package `Example.Alpha.Main`; apartment `Default` |
| E2 | `Name` text is a line feed, two spaces, `Alpha`, a tab, a line feed | accepted | Name `Alpha` |
| E3 | `Name` text is three spaces and a line feed | rejected (empty after trimming) | — |
| E4 | `Information` has no `Name` child, only `Author` with text `Someone` | rejected (no `Name`) | — |
| E5 | `Information` has two `Name` children, `Alpha` and `Beta` | rejected (repeated field) | — |
| E6 | `Name` is 120 × `a` | accepted | Name of 120 characters |
| E7 | `Name` is 121 × `a` | rejected (too long) | — |
| E8 | E1 plus `Author` = 130 × `b` | accepted | Author = 120 × `b` |
| E9 | E1 plus `Description` = 3000 × `c` | accepted | Description = 2048 × `c` |
| E10 | E1 plus an empty `Image` element | rejected (present but empty) | — |
| E11 | E1 plus a child `Notes` with any content | accepted (`Notes` ignored) | as E1 |
| E12 | E1, but `Name` holds text `Al`, a child element `Extra` with text `pha`, and a comment | accepted | Name `Alpha` (descendant text joined, comment excluded) |
| E13 | E1, but `Name` sits inside a child `Group` of `Information` instead of directly under it | rejected (no direct `Name`) | — |
| E14 | E1 plus a second `Package` element nested inside `Information` | rejected (two `Package` elements in the document) | — |
| E15 | E1, `Package` text is 121 × `p` | rejected (too long; no cut for `Package`) | — |
| E16 | E1, `Package` text is whitespace only | rejected | — |
| E17 | E1 plus `Configuration` with no attributes | accepted | apartment `Default` |
| E18 | E1 plus `Configuration` with `Apartment` = `Shared` | accepted | apartment `Shared` |
| E19 | E1 plus `Configuration` with `Apartment` = a space, `Shared`, a space | accepted | apartment is the 8-character value with both spaces (distinct from `Shared`) |
| E20 | E1 plus two `Configuration` elements | rejected | — |
| E21 | E1, root element named `plugin` (lower case) | rejected | — |
| E22 | E1, no encoding declaration and no byte-order mark, `Name` holds the single byte 0xE9 | rejected (illegal UTF-8) | — |
| E23 | E1, declaration names `ISO-8859-1`, `Name` holds byte 0xE9 | accepted | Name is the single character U+00E9 |
| E24 | E1 with the `Information` element wrapped inside a `Wrapper` element under the root | accepted (`Information` found at any depth) | as E1 |

---

## 6. Apartments

<a id="6-1"></a>
### 6.1 On-disk expression

An apartment is a named Lua script state (the runtime model is `lua-plugin-api.md` §6). On
disk, an apartment is named only in a descriptor's `Configuration/@Apartment` attribute (§5).
Every plugin belongs to exactly one apartment:

- with `Configuration/@Apartment = X`, the plugin runs in apartment `X`;
- with no `Configuration`, or a `Configuration` without `Apartment`, the plugin runs in the
  apartment named `Default` (D9).

All plugins whose descriptors name the same apartment run in one shared Lua state, so they
can use each other's globals and call each other's code. Plugins in different apartments
cannot see each other's globals or native singletons.

<a id="6-2"></a>
### 6.2 Load-by-name, unload-by-apartment

The client loads a plugin by its `Name` but unloads by apartment: unloading an apartment
unloads **every** plugin in it, and an apartment cannot be unloaded while its own Lua code is
running, which includes a request from inside itself (DOC; `lua-plugin-api.md` §6.2–§6.3).
When an unload runs in Tessera's frame loop (an unload asked for by a plugin or the user in
the call or input handling that asks; the logout unload as deferred work of frame-loop step
7; the client-exit unload after the last frame) is decision 0014 D8, restated in
`plugin-runtime.md` §8.4.
Apartment names compare **case-insensitively** (D27): `Shared` and `SHARED` name the same
apartment. Tessera folds ASCII letters only (as for `Name`, §4.2).

An empty `Apartment` value is accepted by the parse (D18, example E19 shows that spaces are
also kept).

> **OPEN:** what the original does when it loads a plugin whose apartment name is the empty
> string (whether it creates an apartment with an empty name, and how that apartment is shown
> and unloaded). The parse accepts it, but the load path was not followed for this case.
> Tessera: treat the empty string as an ordinary apartment name, distinct from `Default`, and
> log a warning when a descriptor uses it.

---

## 7. Package names and `import` resolution

<a id="7-1"></a>
### 7.1 Package grammar

A package name is one or more components separated by single dots: `C1.C2.….Ck`. There is no
leading or trailing dot and no empty component (D21).

- **Original rule.** Every byte of the UTF-8 name other than a separating dot must be `_` or
  pass the C runtime's `isalnum` test in the game-language locale (D30); the byte is passed
  sign-extended (D21). ASCII letters (`A`–`Z`, `a`–`z`), ASCII digits (`0`–`9`) and `_` always
  pass. Under the code-page-1252 locales of all four game languages, the published runtime
  sources say that bytes 0x80–0xFF, arriving as −128 to −1, never pass (D32), so no name with
  a non-ASCII byte resolves from disk.
- **Tessera rule.** Each component is one or more ASCII letters, ASCII digits or `_`. Any
  other byte fails, in every locale. This is stated as a Tessera deviation because it is
  locale-independent by design; by D32 it gives the same result as the original under its
  four locales.

> **OPEN:** D32 rests on published C runtime sources, not on a running client: whether the
> runtime that the original actually loads (Microsoft's on Windows, Wine's built-in one under
> Proton) rejects package-name bytes 0x80–0xFF, and plugin-data key units above 0xFF (§11.1),
> was not observed. An owner test with a plugin folder and package name containing `é`, and a
> plugin-data key containing U+0100, would settle it. Same question as the OPEN in
> `lua-plugin-api.md` §7.2. Tessera rejects both in any case.

The descriptor's `/Plugin/Package` text is the package of the plugin's main chunk; plugin code
reaches further chunks with `import`, whose argument may be a literal or computed at run time
(`lua-plugin-api.md` §7.1).

A name that fails the grammar does not resolve at all: `import` fails, and a descriptor whose
`Package` fails it is accepted by the parse (§5.2 checks only the length) but cannot load.

<a id="7-2"></a>
### 7.2 Resolution (BIN D10, D11, D19–D21)

The full mechanics (per-apartment cache, run-once record, namespace environments, the `_`
field) are owned by `lua-plugin-api.md` §7.2–§7.4; this section states the disk-facing part,
which agrees with it. To resolve a package `N` = `C1.C2.….Ck` in a plugin apartment:

1. Check `N` against the grammar of §7.1 (Tessera applies its rule there). If it fails, resolution fails.
2. Ask the native registry for `N` (the names `Turbine`, `Turbine.Gameplay`,
   `Turbine.Gameplay.Attributes`, `Turbine.UI`, `Turbine.UI.Lotro`; `lua-plugin-api.md` §7.4).
3. Look on disk as well. This happens in plugin apartments **even when step 2 found a native
   package** (D11):
   1. Form the path `D = Plugins/C1/C2/…/Ck` (case-insensitive, §3.2).
   2. If `D` **exists and is a directory**, the chunk is `D/__init__.lua` and the package is a
      *folder package*. A directory without `__init__.lua` does **not** fall back to a sibling
      `Ck.lua`; the disk part is then absent.
   3. Otherwise the chunk is the file `D.lua` (that is, `Plugins/C1/…/C(k-1)/Ck.lua`) and the
      package is a *file package*.
   4. The chunk must exist and be readable; otherwise the disk part is absent.
4. Combine: if both a native part and a disk part exist, the package is their **merge**: the
   native part is installed, then the disk chunk runs in the same namespace. If only one
   exists, that one is the package. If neither exists, resolution fails.
5. **Namespace** (D19): `C1.….Ck` for a folder package and for a native package;
   `C1.….C(k-1)` for a file package (for `k = 1`, the apartment's global table itself).
6. **Loading the chunk** (D20): the original uses the stock Lua 5.1.4 file loader, which also
   accepts precompiled (binary) chunks and skips a first line beginning with `#`, and does not
   strip a UTF-8 byte-order mark. Tessera loads **source text only** and rejects precompiled
   chunks, as a deliberate safety deviation (precompiled chunks can crash the interpreter); it
   may strip a leading UTF-8 byte-order mark (`lua-plugin-api.md` Appendix B).

The key consequence is the **precedence**: when both a folder `Plugins/A/B/` and a file
`Plugins/A/B.lua` exist, resolving `A.B` uses the folder's `__init__.lua`, because the
directory test comes first. A second consequence is the merge: a user folder
`Plugins/Turbine/` with an `__init__.lua` runs in the `Turbine` namespace whenever an
apartment imports `Turbine`, which every new apartment does (`lua-plugin-api.md` §7.4).

<a id="7-3"></a>
### 7.3 Invariants

- `import "A.B"` where `Plugins/A/B/__init__.lua` exists runs that file with namespace `A.B`.
- `import "A.B.Main"` where `Plugins/A/B/Main.lua` exists (and `Plugins/A/B/Main/` is not a
  directory) runs that file with namespace `A.B`.
- When both `Plugins/A/B/` (a directory) and `Plugins/A/B.lua` exist, `import "A.B"` runs
  `Plugins/A/B/__init__.lua`, not `A/B.lua`.
- When `Plugins/A/B/` is a directory with no `__init__.lua` and `Plugins/A/B.lua` exists,
  `import "A.B"` fails.
- A native name with no matching disk folder or file (for example `Turbine.UI` when
  `Plugins/Turbine/UI/` and `Plugins/Turbine/UI.lua` do not exist) resolves to the native
  package alone.
- When `Plugins/Turbine/__init__.lua` exists, `import "Turbine"` installs the native `Turbine`
  table and then runs that file with `Turbine` as its namespace.
- Names such as `A..B`, `.A`, `A.`, `A-B.C` and `A.B/C` never resolve.

<a id="7-4"></a>
### 7.4 Worked synthetic example

Synthetic tree under `Plugins/`: a folder `Example/`, holding `Alpha.plugin` (Package
`Example.Alpha.Main`), a folder `Alpha/` with `Main.lua` and `Util/__init__.lua`, and a file
`Alpha/Util.lua`.

| Request | Path tested | Result | Namespace |
|---|---|---|---|
| main package `Example.Alpha.Main` | `Example/Alpha/Main` is not a directory | runs `Example/Alpha/Main.lua` | `Example.Alpha` |
| `import "Example.Alpha.Util"` | `Example/Alpha/Util` is a directory | runs `Example/Alpha/Util/__init__.lua` (the sibling `Util.lua` is never used) | `Example.Alpha.Util` |
| `import "Example.Alpha.Missing"` | not a directory; `Missing.lua` absent | fails | — |
| `import "Example.Alpha-2"` | none (fails the grammar) | fails | — |

---

## 8. Load order and lifecycle (disk level)

<a id="8-1"></a>
### 8.1 Loading

- The **available** list is built by discovery (§4). **Loading** is triggered by
  `/plugins load <Name>`, `PluginManager.LoadPlugin(name)`, the manager UI, or autoload at
  character login (the autoload set is stored in `PluginOptions.xml`, §10). Loading creates
  the plugin's apartment if it does not yet exist, then runs `import "<Package>"` (§7); the
  full load and unload sequences and the `Load`/`Unload` events are `lua-plugin-api.md`
  §6.2–§6.3.
- While the main package runs, the apartment carries a "loading a plugin" mark, cleared
  afterwards (D29). It changes how `PluginData.Load` behaves (`lua-plugin-api.md` §10.1).
  This spec defines no logging behaviour for the load path; Tessera's own diagnostics are
  free-form.
- **Tessera, when loads run** (`plugin-runtime.md` §8.4, which states the rules and their
  sources). Queued `LoadPlugin` requests run as deferred work in frame-loop step 7, one load
  per item, in queue order (decision 0014 D8). The autoload loads are not named in D8;
  Tessera queues them the same way, one item per autoloaded plugin, when the session reports
  the login (a Tessera reading of D8, `plugin-runtime.md` §8.4). Loads asked for with
  `/plugins load` or from the manager UI are queued the same way too (a Tessera choice,
  `plugin-runtime.md` §8.4). The deferred work is sliced by whole items at 2 ms per frame
  (D8), so a long list of loads can take several frames, but their order is kept. No load
  item runs before the first plugin file index of §4.2 is ready (decision 0015 §4.4).

<a id="8-2"></a>
### 8.2 Autoload order

The options reader keeps autoload entries in a map keyed by plugin `Name`, so the order of
`Plugin` elements in `PluginOptions.xml` is not kept in memory (D28).

> **OPEN:** the order in which autoloaded plugins load at character login. It is decided by
> the code that walks the autoload map at login, which was not read; the map is hashed, so the
> order is probably unrelated to the file order or to alphabetical order. Recommended for
> Tessera: load in the order of its own autoload list, each load one step-7 deferred-work
> item (a Tessera reading of decision 0014 D8, which does not name autoload;
> `plugin-runtime.md` §8.4), none before the first plugin file index of §4.2 is ready
> (decision 0015 §4.4); within an apartment, later loads add to the shared state.
> (Same open question as `lua-plugin-api.md` §6.2.)

---

## 9. Companion and compendium files

<a id="9-1"></a>
### 9.1 What they are, and that the client ignores them

Third-party managers (LotroPluginCompendium, OneLauncher, and similar) keep a sidecar file
next to a descriptor:

| File | Written by | Content (XSD) |
|---|---|---|
| `<Any>.plugincompendium` | plugin managers | `PluginConfig`: `Id` (the plugin's numeric id on the community download site), `Name`, `Version`, `Author`, `InfoUrl`, `DownloadUrl`, `Descriptors` (one or more `descriptor` paths, back-slash separated), `Dependencies` (zero or more `dependency` ids), optional `StartupScript` |
| `<Any>.skincompendium`, `<Any>.musiccompendium` | skin / music managers | `SkinConfig` / `MusicConfig`, same base fields |

**The client does not read any of these** (D12: the client contains none of the strings
`plugincompendium`, `skincompendium`, `musiccompendium` or `compendium`). They exist only so
that a manager can track installs, versions, download URLs and dependencies. A compatible
client likewise ignores them for discovery and load. Tessera may optionally read a
`.plugincompendium` for a convenience feature (for example showing an update URL), but
discovery and load must not depend on it, and it must never be written into the install.

> **Note.** A `.plugincompendium` lists the descriptor paths a manager installed; a manager
> that owns a descriptor through its compendium treats the plain descriptor as "managed". This
> is manager bookkeeping only and has no effect on what the client loads. OneLauncher also
> searches author folders recursively for descriptors (OL); the client does not (D3).

---

## 10. `PluginOptions.xml`

<a id="10-1"></a>
### 10.1 Location and observed form

The in-game plugin manager stores its choices **per account and per world** at
`PluginData/<account>/<world>/PluginOptions.xml` (D13). Observed form (OBS, three files): an
XML declaration with `encoding="UTF-8" standalone="no"`, CRLF line ends, a root
`PluginOptions` element, and an empty, self-closing `AutoloadOptions` element; one of the three
files also carries an empty, self-closing `ScalingOptions` element. The three files belong to
three worlds under one account, and world folder names are the worlds' display names verbatim,
including spaces and a bracketed form (D15). The observed files are empty because this owner
has autoloaded nothing.

<a id="10-2"></a>
### 10.2 Populated structure (BIN D13, D28)

The client's writer produces, and its reader expects, the following (this table matches
`lua-plugin-api.md` §5.4):

| Path | Kind | Value and meaning |
|---|---|---|
| `/PluginOptions` | root element | |
| `/PluginOptions/AutoloadOptions` | element | Autoload choices |
| `…/AutoloadOptions/Plugin` | element, repeated | One per plugin that autoloads for some character |
| `…/AutoloadOptions/Plugin/@Name` | attribute | The plugin's `Name` |
| `…/AutoloadOptions/Plugin/@AllCharacters` | attribute | Written as `true` or `false` (lower case). Read as true only when exactly `true`; any other value is false. True: autoload for every character of the account on that world |
| `…/AutoloadOptions/Plugin/Character` | element, repeated | One per character for which the plugin autoloads |
| `…/AutoloadOptions/Plugin/Character/@Name` | attribute | Character name |
| `/PluginOptions/ScalingOptions` | element | UI-scaling choices |
| `…/ScalingOptions/Plugin` | element, repeated | One per plugin |
| `…/ScalingOptions/Plugin/@Name` | attribute | The plugin's `Name` |
| `…/ScalingOptions/Plugin/@UseScaling` | attribute | Written as `True` or `False` (capitalised). Read as true only when exactly `True`. The value returned by that plugin's `Plugin:GetUsesGlobalScaling()` |

Reading rules: autoload entries are keyed by plugin `Name` with a case-sensitive hash; when two
`AutoloadOptions/Plugin` elements carry the same `Name`, the later one is dropped (D28). The
file is written pretty-printed with CRLF line ends.

Tessera keeps its **own** autoload and scaling state and does not need to read this file. An
optional importer may read it, read-only, using the rules above; it must never write it.

<a id="10-3"></a>
### 10.3 Invariants

- An importer reading a file whose `AllCharacters` value is `True` (capitalised) treats it as
  false, matching the original.
- An importer reading two `AutoloadOptions/Plugin` elements with the same `Name` keeps the
  first.
- An empty `AutoloadOptions` (the observed form) yields no autoload entries.

---

## 11. Plugin-data files (cross-reference)

<a id="11-1"></a>
### 11.1 Facts that concern files

Plugin data (the `Turbine.PluginData` API) is specified in `lua-plugin-api.md` §10. The facts
that concern files and folders are:

- A plugin-data **key** must have **1 to 64** UTF-16 units. An empty key or one of 65 or
  more units raises an error in the caller (D14).
- **Original character rule.** Each unit must be `_` or pass the C runtime's `isalnum` test,
  called with the unit's value, in the game-language locale (D14, D30). ASCII letters, ASCII
  digits and `_` pass in every locale. Under code page 1252 (all four game languages, D31),
  units 0xA0–0xFF that are Latin-1 letters or digits pass (for example U+00E9), and units
  0x80–0x9F pass when the code-page-1252 character at that byte position is a letter (D31).
  Units above 0xFF never pass (D32; see the OPEN in §7.1).
- **Tessera rule (stated deviation).** Each unit must be an ASCII letter, an ASCII digit or
  `_`; anything else raises the same error. Keys are file names, and a fixed ASCII rule keeps
  them portable across file systems and independent of locale. A plugin that uses a key such
  as `é` works in the original and gets an error in Tessera (`lua-plugin-api.md` §10.2).
- Data is stored as `<key>.plugindata` under `PluginData/<account>/`: the account scope in a
  folder named `AllServers`, the world scope under `<world>/AllCharacters/`, and the character
  scope under `<world>/<character>/` (`lua-plugin-api.md` §10.3, BIN). Only the
  `PluginData/<account>/<world>/` level is observed in the owner's install, which holds no
  `.plugindata` file (OBS).
- **Path safety.** Under either rule a key can never form a path separator (`/`, `\`), a
  drive colon, a dot, a control character below 0x20 or a Windows reserved file-name character
  (`< > : " | ? *`): all of these are ASCII non-alphanumerics, which `isalnum` rejects in every
  locale and which the Tessera rule rejects. The extra units the original accepts under code
  page 1252 (0x80–0xFF alphanumerics) are all legal in Windows file names. A key can still
  spell a Windows reserved device name such as `CON`, `NUL` or `COM1`.

> **OPEN:** what the original does with a plugin-data key that spells a Windows reserved
> device name (for example `CON`, which gives `CON.plugindata`); its save and load paths were
> not followed for this case. Tessera: write plugin data only through its own path layer,
> which on a Windows host must refuse to open a device name and report the same error as an
> invalid key.

---

## 12. Corrections to other specs

<a id="12-1"></a>
### 12.1 Corrections from the first draft (all already applied)

The first draft of this spec recorded four corrections to `lua-plugin-api.md`. That spec's
2026-10-05 revision already contains all four, so no action remains:

| Target | Correction | Status |
|---|---|---|
| `lua-plugin-api.md` §7.2 (resolution order) | Folder form first; a directory does not fall back to the sibling `.lua` | Applied there (§7.2, P5) |
| `lua-plugin-api.md` §10.2, P14 (key length) | Keys of 1 to 64 characters are accepted | Applied there (§10.2, P14) |
| `lua-plugin-api.md` §4.2–§4.3 (companion files) | No compendium string of any kind exists in the client | Applied there (§4.2) |
| `lua-plugin-api.md` §6.1, P11 (default apartment) | `Default` confirmed from the binary | Applied there (P11) |

<a id="12-2"></a>
### 12.2 Corrections received from `lua-plugin-api.md` §20 (all applied here)

`lua-plugin-api.md` §20 listed seven corrections to the first draft of this spec. This
revision applies all seven, and re-verified each against the client:

| Correction | Where applied |
|---|---|
| Discovery reads only `Plugins/<folder>/*.plugin`, one level, non-recursive | D3, §4.2, §4.3 |
| Duplicate `Name`, compared case-insensitively: first found wins | D26, §4.2 step 5 |
| Repeated field rejects; `Name` required; `Name`/`Version` over 120 reject; `Author`/`Image` cut to 120; `Description` cut to 2048; present-but-empty rejects; `Package` 1–120 | D6–D8, §5.1, §5.2, §5.4, §5.5 |
| In plugin apartments native names are also looked up on disk and merged | D11, §7.2, §7.3 |
| Apartment names compare case-insensitively | D27, §6.2 |
| Populated structure of `PluginOptions.xml` | D13, D28, §10.2 |
| Plugin-data keys: the original rule is `_` or the C runtime's `isalnum` in the game-language locale (code page 1252, so units such as U+00E9 pass); Tessera accepts only ASCII letters, digits and `_`, a stated deviation (the current `lua-plugin-api.md` §20 D14 row, P14, P43, P44). An earlier version of that row said "ASCII only"; this revision follows the current row | D14, D30–D32, §11.1 (package names likewise: D21, §7.1) |

<a id="12-3"></a>
### 12.3 Additions suggested for `lua-plugin-api.md` (not contradictions)

These facts from this revision are not stated in `lua-plugin-api.md`. Nothing there is wrong;
the owner of that file may add them. They are recorded here, not applied, because this session
owns only this file.

| Target | Addition | Basis |
|---|---|---|
| `lua-plugin-api.md` §5.2 step 6 | An `Apartment` attribute value is not length-checked and may be empty; the parse accepts it (open question on load behaviour in §6.2 here) | BIN (D18) |
| `lua-plugin-api.md` §5.2, encoding paragraph | Bytes illegal in the detected encoding are a fatal XML error, so the descriptor is rejected; there is no Latin-1 fallback | BIN (D22); XML §4.3.3 |
| `lua-plugin-api.md` §5.4 | `AllCharacters` reads as true only for exactly `true`, `UseScaling` only for exactly `True`; a repeated autoload `Name` keeps the first | BIN (D28) |
| `lua-plugin-api.md` §6.2 OPEN (autoload order) | The reader keeps autoload entries in a hashed map, so file order is not preserved | BIN (D28) |
| `lua-plugin-api.md` §4.3 step 2 | The `*.plugin` match is a whole-name, case-insensitive wildcard match; matched entries are not checked to be files | BIN (D2, D25) |
| `lua-plugin-api.md` §4.3 (order) | The client does not sort the listings (narrow find functions, entries appended in returned order); NTFS and Wine both return names in upper-case-folded order; Tessera's "case-insensitive byte order" is defined exactly in §4.2 here (fold `a`–`z` to `A`–`Z`, unsigned byte compare, unmodified bytes as tie-break) | BIN (D33); NTFS, WINE (D34) |
| `lua-plugin-api.md` §7.2 OPEN (bytes 0x80–0xFF, units above 0xFF) | The published Microsoft UCRT and Wine sources both make `isalnum` false for arguments below −1 or above 0xFF in a single-byte locale, and the entry for −1 is empty; the OPEN can be narrowed to "not observed on a running client" | CRT (D32) |
| `lua-plugin-api.md` §4.3 / §4.4 (non-ASCII names) | Discovery names reach the client in the ANSI code page, so names outside it may fail to open in the original | BIN (D33); CRT (D35) |

---

## 13. Consolidated open questions

<a id="13-1"></a>
### 13.1 Open

- Case folding of non-ASCII letters when comparing plugin names (§4.2).
- Run-time confirmation of D32: whether the runtime the original loads rejects package-name
  bytes 0x80–0xFF and plugin-data key units above 0xFF (§7.1; same as `lua-plugin-api.md`
  §7.2).
- Plugin-data keys that spell a Windows reserved device name (§11.1).
- Load behaviour of an empty apartment name (§6.2).
- Autoload ordering at character login (§8.2).

<a id="13-2"></a>
### 13.2 Closed in the 2026-10-05 revision

| Former open question | Answer | Where |
|---|---|---|
| Discovery recursion depth (former §4.2) | Exactly one folder level below `Plugins/` | D3, §4.2 |
| Duplicate `Name` behaviour (former §4.2) | Case-insensitive; first found wins | D26, §4.2 |
| Encoding fallback for undeclared non-UTF-8 bytes (former §5.3) | None; illegal bytes reject | D22, §5.2 rule 1, §5.3 |
| Case folding of apartment names (former §6.2) | Case-insensitive | D27, §6.2 |
| Populated inner structure of `PluginOptions.xml` (former §10) | Elements and attributes as in §10.2 | D13, D28, §10.2 |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First draft. Discovery, descriptor structure and validation, apartments, package resolution, companion files and `PluginOptions.xml`, from the owner's install (OBS), public schemas and docs (XSD, DOC, SMP, OL), and disassembly of client build 4906.0071.1794.4035 (BIN). Records four corrections to `lua-plugin-api.md` (§12). |
| 2026-10-05 | Revision answering the review (changes requested) and reconciling with `lua-plugin-api.md` §20; all facts re-verified by BIN on the same build. **Corrected:** D3 and §4.2 (one folder level, no recursion; whole-name case-insensitive wildcard; entries not checked to be files); D6–D8, §5.1, §5.2, §5.4 (repeated field rejects; `Name` required; `Name`/`Version` over 120 reject instead of being cut; `Author`/`Image` cut to 120; `Description` cut to 2048; present-but-empty rejects; `Package` 1–120; lookups of `Information`, `Package`, `Configuration` reach any depth; fields are direct children only; trim set stated and whitespace-only values are empty); D9, D18, §5.1 (`Apartment` optional, used as written, may be empty; the schema disagrees); D11, §7.2, §7.3 (native names also looked up on disk and merged); D14, §11 (keys are ASCII letters, digits and underscore); §7.1 grammar (ASCII letters, digits and underscore, matching `lua-plugin-api.md` P5); §7.2 step 6 (the original's loader accepts precompiled chunks; rejecting them is a Tessera deviation); §3.2 multiple-match rule aligned with `lua-plugin-api.md` §4.4. **Removed:** the "loading plugin" log-line claim (it was a misreading of the per-apartment loading mark, D29); the quoted documentation sentence in §6.1 and the quoted client message wording in §12. **Provenance:** D4 re-marked BIN / Reported (no `.plugin` file exists in the owner's install); new rows D16–D29 (icon size, refresh, apartment value, namespace rule, chunk loader, name grammar, parser and encoding, text content, trim set, non-file entries, duplicate names, apartment case, options-file reading rules, loading mark); new source keys XML and LPA. **Closed OPENs:** recursion depth, duplicate names, encoding fallback, apartment case, `PluginOptions.xml` structure (§13.2). **New OPENs:** non-ASCII case folding of names (§4.2), empty apartment name at load (§6.2); autoload order narrowed (§8.2). **Structure:** fixed every internal section reference; added `<a id>` anchors to §8–§13 (§8.1, §8.2, §9.1, §10.1–§10.3, §11.1, §12.1–§12.3, §13.1–§13.2); added §4.3 invariants, §5.5 and §7.4 worked synthetic examples, §10.3 invariants; §12 now lists the first draft's corrections as applied, the seven received corrections as applied here, and new suggested additions to `lua-plugin-api.md`. Reviewer field reset for re-review. |
| 2026-10-06 | Revision answering the re-review (changes requested). **Accuracy:** D14, D21, §7.1, §11.1 and §12.2 row 7 no longer state ASCII-only as the client's rule; they state the original rule (`_` or C runtime `isalnum` in the game-language locale; key units passed as is, package bytes sign-extended) and make ASCII-only a stated Tessera deviation, aligned with `lua-plugin-api.md` P5, P14, P43, P44 and its §20 D14 row. New rows D30 (start-up locale, BIN re-read), D31 (code-page-1252 classification, Reported), D32 (runtime rejects arguments outside −1…0xFF, Reported from the UCRT and Wine sources). New OPEN in §7.1 (run-time confirmation of D32) and §11.1 (reserved device names); §11.1 path-safety argument restated for code-page-1252 alphanumerics. **Marks:** D14 is Confirmed only for `PluginData/<account>/<world>/`; `AllServers`, `AllCharacters`, `<character>` and `.plugindata` are BIN/LPA. **Order:** §4.2 rewritten: the client does not sort (D33, BIN re-read of the listing routine and discovery loop); NTFS and Wine orders stated with sources (D34; new source keys NTFS, WINE, CRT); Tessera's order defined exactly (ASCII upper-case fold, unsigned byte compare, unmodified bytes as tie-break) with a new §4.3 invariant; narrow names and the ANSI code page (D35) with a stated widening deviation. **Naming note:** cites `docs/ARCHITECTURE.md` §18.2 and the *Google v. Oracle* row of `docs/LEGAL.md` §4.1, with the interoperability record pending; `LEGAL.md` §10 is cited for naming only. §12.3 gains four suggested additions to `lua-plugin-api.md`; §13.1 lists the two new OPENs. |
| 2026-10-06 | Revision (spec writer, dirty room); status Accepted → Review for the re-review of this change only. The naming note now cites the interoperability record `docs/LEGAL.md` §17 (which answers the ARCHITECTURE §18.2 "OPEN (legal)" item it previously called pending) and its §17.2 rules (critique B10). No functional fact, section number, invariant or OPEN changed; implementers may keep building from §1–§13 as Accepted. |
| 2026-10-06 | Revision (spec writer, dirty room); status Accepted → Revised, awaiting review of this change. Applies the Accepted decisions 0014 (UI input bridge) and 0015 (UI rendering) by cross-reference only; no fact about the original, provenance row, invariant or OPEN changed. **§4.2:** a Tessera paragraph on the whole-`Plugins/` file index of decision 0015 §4.4 (built on an asset worker at host start and on every refresh, swapped in whole; no plugin loads before the first index), and the note that an immediate or deferred descriptor scan is acceptable. **§6.2:** when unloads run in Tessera's frame loop (0014 D8, `plugin-runtime.md` §8.4). **§8.1:** queued loads run as step-7 deferred items, sliced by whole items, order kept. **§8.2:** the autoload recommendation names the step-7 item. Related specs and decisions added to the header. |
| 2026-10-06 | Repair of the decision revision after its review (spec writer, dirty room); status stays Revised, awaiting review. **§8.1:** the autoload loads as step-7 items are labelled a Tessera reading of decision 0014 D8 (D8 names only queued `LoadPlugin` requests), as `plugin-runtime.md` §8.4 already says; `/plugins load` and manager-UI loads are queued the same way (Tessera choice, `plugin-runtime.md` §8.4); no load item runs before the first plugin file index (decision 0015 §4.4). **§8.2:** the autoload recommendation carries the same label and the same index gate. No fact about the original, provenance row, invariant or OPEN changed. |
