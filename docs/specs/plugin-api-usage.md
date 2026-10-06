<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Plugin API usage census (third-party UI plugins)

| Field | Value |
|---|---|
| Status | **Accepted** (2026-10-06: accepted after revision for decisions 0014/0015; the revision added cross-references to the Accepted decisions 0014 and 0015 in §6.8, §6.11, §7.3, §8, §9.3 and §12 and changed no count or other census fact, see the change log and the review record) |
| Last updated | 2026-10-06 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | Claude review session (spec-writer agent) |
| Review record | First version: 2026-10-06, Claude review session (spec-writer agent), against CLEAN_ROOM.md §5.3. **Result: changes required; stays Review.** Its four issues (examined-code scope of §3.3 rule 3 and the consequences for §9.2 #12, §11 Example 1, §3.5 and §7.2; the load-time language-detection wrappers under the §7.4 empty views; the `require` row of §6.4; the dirty-room-only invariant §10.1) are answered by the second version, see the change log. **Re-review of the second version: 2026-10-06, Claude review session (spec-writer agent), against CLEAN_ROOM.md §5.3. Result: changes required; stays Review.** All four earlier issues are resolved. Spot checks passed, all against the published plugin files: 35 distributions and 46 descriptors widened by §3.3 rule 3, and 91 distributions where the closure is every `.lua` file; Compass (#12): 11 `.lua` files, 37 names, computed imports, no class-N name and no `GetLanguage` replacement anywhere in the distribution, plus every §9.3 row-#12 feature; for all 14 primary and 6 reserve distributions, a whole-distribution text search finds no class-N access, only comments and plugin-own identifiers; 8 `Engine.GetLanguage` replacements (1 of them a type stub), groups of 2, 4 and 1, 6 called at load, and 1097 the group-B distribution that never calls its replacement; `require` 2 direct and 4 through `pcall`; 8 `MinstrelStance` overwrites; the §7.2 counts 13 and 56; `SetRotation` 14, `table.getn` 63, `math.mod` 18, `GetUsesGlobalScaling` 8, `RegisterForGlobalScaling` 6, `SetMarkupEnabled` 38 (40 for the whole distribution). Issues: (1) §7.4.1 group A: the text says "the test looks at the first two bytes of the UTF-8 name". That holds for only one of the two (LIX 1199). The other (LIX 901) matches the whole name against a byte set, {0x81, 0x8F, 0x90–0xD1}. Any byte in that set anywhere in the name gives `Language.Russian`, so any accented Latin letter (lead byte 0xC3) also triggers it. Groups A (1199) and B also use the first two bytes as a Lua *pattern* searched inside a Cyrillic string. If those bytes hold a pattern magic character (`^$()%.[]*+-?`), the test can match (for example a leading `.`) or raise a malformed-pattern error at load. So rule 1 and invariant 7 ("first character outside U+0400–U+04FF") do not ensure the native-value path or a clean load. State both tests. Then either constrain the placeholder (for example: at least its first two bytes are ASCII letters, and no byte is ≥ 0x80 when Tessera chooses it), or state, for names read from the install, that the result follows the plugin's own test exactly as in the original. Extend invariant 7 to match. (2) Minor scope label: the §6 lead-in says every §6 count uses the whole-distribution scope, but §6.10's `UnloadScriptState` 28 and `LoadPlugin` 31 are closure-scope counts (29 and 33 for the whole distribution). Label them, or use the whole-distribution values; §7.3 repeats 31/28. No other §5.3 checklist findings: no code, no copied prose, no game text, keys, hostnames or traffic; no closed gate; provenance covers every fact group; OPENs are precise; invariants other than 7 are testable as written. **Third version: 2026-10-06, spec writer (dirty room).** Answers both re-review issues (§7.4.1 tests stated byte by byte with the placeholder constrained, invariant 7 extended; every §6 and §7.3 count labelled with its scope, §6.10 and §7.3 made consistent) and adds the load-limit measurement (§6.12); awaiting re-review. **Third review: 2026-10-06, Claude review session (spec-writer agent), a different session from the author, against CLEAN_ROOM.md §5.3. Result: passed; status Accepted.** Both re-review issues are resolved. Checks: test P of §7.4.2 was re-enumerated with stock Lua 5.1 (`string.match` on the 132-byte string, which starts with 0xD0) over every first byte 0x00–0xFF and every second byte or none. 36,358 prefixes were classified by the outcome table and none disagreed, and no valid-UTF-8 prefix falls outside the table. Test S reduces to exactly 68 bytes, {0x81, 0x8F} ∪ 0x90–0xD1. Groups A1 (1199), A2 (901), B (640, 896, 993, 1097; the same 132-byte string, 66 letters) and C (1113) match their published files. The U1 settings-folder facts were re-checked read-only: no `Plugins/` and no `.plugin` file; three empty `PluginOptions.xml` files. The §6 recounts in both scopes match: `LoadPlugin` 33/31, `UnloadScriptState` 29/28, `Plugins` 88/86, `Plugins[…]` 73/71, `_G` 98/95, `table.getn` 64/63, `unpack` 52/49, the `Turbine.Chat` callback helper 32, `getfenv` 61/56, `setfenv` 11, stretch 1 58/57, stretch 2 56/50, `.jpg` 31/28, `.tga` 115/114, DID literals 73/72, `Server` 41/40, `WriteLine` colour tag 45/44. In §6.5, class calls 121, own definitions 90 (counting `function _G.class`), bundles 11, 31 + 28 → 37 dependents, and no §9.2/§9.4 distribution depends on the sample package. The U12 probe was re-run: #2 peaks at 160.2 MiB (live 125.1 MiB) and is refused during load at 159 MiB; #14 peaks at 75.2 MiB and is refused at 75 MiB; the largest call is 12.8 M instructions. Non-blocking notes for the next revision: (1) §4, §6.12 and §9.3 give 32.5 MB, 10.1 MB and 6.5 MB, which are MiB values (34.1, 10.6 and 6.9 MB decimal), while §6.12 finding 4 gives the same distribution as 34 MB and 131 MB; use one unit. (2) §6.5 `Turbine.Animation` 4 matches neither rule (3 by exact name, 6 counting sub-packages); the other sub-package counts are exact-name counts. (3) §11 Example 5 comes before Example 4. **Review of the decision revision (0014/0015) and its repair: 2026-10-06, Claude review session (spec-writer agent), a different session from the author, against CLEAN_ROOM.md §5.3 and the Accepted decisions 0014 and 0015. Result: passed; status Accepted (accepted after revision for decisions 0014/0015).** Every §5.3 item holds: functional facts and labelled Tessera rules only; no code, pseudo-code or listing structure (the inline forms are interoperability names and stock Lua calls); no copied prose; no game text, keys, hostnames or traffic; every census fact has a provenance row (U1–U12); no byte layout; invariants 1–9 unchanged and testable; 9 OPEN callouts, each in §12 with O5 listed there only; no closed gate and no BIN work. **Decision consistency:** §6.8 (non-zero stretch mode = own surface, own opacity only, clipped by its enclosing windows; mode-0 control drawn into the nearest surface above; stand-in surface only for a top-level window) matches 0015 Decision 1.1 and 1.7; the `.png`/`.bmp` rule and the call-time checks match 0015 §4.3 and §4.4; §6.11 matches §4.4 step 1; §7.3 matches 0015 Decision 1.1, 1.6, 1.7, §2.1–§2.3 and 0014 D8; §8 `Chat.Received` in step 7 matches 0014 D8 and Known difference 21; the §9.3 input bullets match 0014 D1, D2, the D5 OPEN, D7, Known differences 15 and 18, the Consequences entry "Escape with a UI attached" and "Still OPEN"; the census sentence of §9.3 is back among the census facts. **Spot checks** (own probe, read-only, 2026-10-06): U1 (Confirmed) re-checked in the owner's Steam-prefix settings folder: no `Plugins/` folder, no `.plugin` file, three `PluginOptions.xml` files with empty autoload lists. Reported counts recomputed over the 130 LIA archives and the 13 GH repositories (commits match Appendix A), whole distribution, comments removed: `SetStretchMode` literal 1/2/0/3/4 = 58/56/22/13/0, `loadstring(` 34, `SetBackground(nil)` 20, `print(` 19, `math.mod` 18, `setfenv` 11, `collectgarbage(` 13; Lua source of #2, #14 and #1 = 32.49, 10.06 and 6.54 MiB; median 146 KB, 90th percentile about 1 MB: all match. **Non-blocking notes for the next revision:** (1) O9 and the §6.9 OPEN are stale: `lua-plugin-api.md` §11.1 now states that markup **is rendered** in `WriteLine` lines; close O9 and update the §7.3 row. (2) §8 says `Chat.Received` carries only the user's input and Tessera's system messages; LPA §11.1's Tessera rule also delivers `WriteLine` lines (chat type 8); say so. (3) §6.8 summarises mode 2 as repeating the base image; 0015 §2.1 tiles only power-of-two base images and otherwise behaves as mode 3, and a window that never set a mode has a surface (LPA §13.19); either restate it or keep it a bare pointer. (4) The decision-derived Tessera rules have no provenance row (compare LPA P80); add one with source DEC and mark Tessera rule. (5) The three notes of the third review still stand (MB values that are MiB in §4, §6.12 and §9.3; `Turbine.Animation` 4; Example 5 before Example 4). |
| Implements into | `crates/plugins` (which API pieces to build first, which to stub), `crates/net` sandbox backend (which local-player values plugins read), the M5 exit-criterion-4 test plan |
| Related specs | [`lua-plugin-api.md`](lua-plugin-api.md) (the API itself: names, behaviour, tiers; abbreviated **LPA** below), [`plugin-descriptor.md`](plugin-descriptor.md) (discovery, descriptors, apartments), [`install-layout.md`](install-layout.md) §8 (settings folder), [`plugin-runtime.md`](plugin-runtime.md) (limits, frame placement of plugin code, §8.4); decision records [`0014-ui-input-bridge.md`](../decisions/0014-ui-input-bridge.md) (how input reaches plugin handlers) and [`0015-ui-rendering.md`](../decisions/0015-ui-rendering.md) (how plugin controls, stretch modes and images are drawn) |

> **Naming note (interoperability).** The namespace, class, method, event, field and enum
> member names in this document are the identifiers existing third-party plugins call. They
> are reproduced only because a compatible host cannot run those plugins without them. The
> project's interoperability record for these names is `docs/LEGAL.md` §17 (it answers the
> "OPEN (legal)" item of `docs/ARCHITECTURE.md` §18.2); its §17.2 rules bind every use: the
> names appear in Tessera only inside the plugin subsystem, as string data, never as product,
> crate, module or UI names. Plugin names and author names are used only to identify which
> published files were counted and which ones the M5 test should install; they are not
> product names. No plugin code, comment, documentation text, string table or asset is
> reproduced: every statement below is a count or a description in our own words of what the
> code relies on.

---

<a id="1"></a>
## 1. Purpose

[`lua-plugin-api.md`](lua-plugin-api.md) describes the whole plugin API of the current client
(about 150 classes and enumerations, several hundred members). This spec measures which part of
it real, published third-party plugins actually use, so that an implementer can:

- build a **first API surface** (§7) that lets the largest number of plugins load and work
  without any game server;
- know which API pieces need data that only a game server has, and what the sandbox backend
  must supply instead (§8);
- know which behaviours of the host the plugins rely on beyond the plain call surface
  (writable class tables, handler attachment forms, the `Plugins` global, library
  extensions, §6);
- run M5 exit criterion 4 against a fixed, named set of popular UI-only plugins (§9).

It is a census, not a behaviour spec: where a behaviour matters, the rule lives in LPA and is
cited from here.

---

<a id="2"></a>
## 2. Provenance

Source keys:

| Key | Source |
|---|---|
| **OWN** | The owner's settings folder in the Steam Proton prefix (`install-layout.md` §8), read-only, 2026-10-06 (method M1). |
| **LIX** | The public download index of lotrointerface.com, read 2026-10-06: every entry of the stand-alone-plugin category and its sub-categories (action bars, bags, class-specific, crafting, creep play, graphical modifications, maps, role-play and music, raiding, unit frames, other) and of the libraries category; 336 entries with title, uploader, cumulative download count and last-update date (method M2). |
| **LIA** | The 140 most-downloaded entries of LIX, downloaded as published archives on 2026-10-06 into the dirty scratch area and read statically (no plugin was run). File IDs in Appendix A (method M2). |
| **GH** | Thirteen public GitHub repositories of plugins not already covered by LIA, found through the community project list `Dromo/Lotro-Github-projects` (research clone), shallow-cloned 2026-10-06 into the dirty scratch area; repository and commit in Appendix A (method M2). |
| **LPA** | [`lua-plugin-api.md`](lua-plugin-api.md) §16 (registration list of client build 4906.0071.1794.4035, itself BIN-confirmed) and Appendix A (enumeration member names). Used only as the **name set** against which plugin code was matched, and for the tier numbers of §17. |
| **S3** | `lunarwtr/vscode-lotro-api` `TurbineDump.json` (research clone): the named-colour and enumeration member names, used to complete the name set where LPA lists only counts. |
| **LUA** | General knowledge of the Lua 5.1 reference manual and the reference interpreter (MIT licence; not dirty, `docs/CLEAN_ROOM.md` §2 rule 4); used for the pattern-matching rules of §7.4.1 and as the interpreter of the U12 measurement |

Marks: **Confirmed** = observed in the owner's install or settings folder, with date.
**Reported** = taken from published third-party files only. Every census count is
**Reported**: the plugin files are published third-party material, not part of the install. No
BIN (M4) work was done for this spec; nothing of the network protocol, login, launcher or
anti-tamper was looked at.

| # | Fact or group of facts | Sources | Mark |
|---|---|---|---|
| U1 | The owner's settings folder holds no `Plugins/` folder and no `.plugin` file; the three `PluginOptions.xml` files hold empty option lists. The exit-criterion plugins must therefore be installed by the owner (§9.5) | OWN | **Confirmed** 2026-10-06 |
| U2 | Sample definition, sizes, categories, dates, download range (§3.1, §4) | LIX, LIA, GH | Reported 2026-10-06 |
| U3 | Class, member, event and enumeration usage counts (§5) | LIA, GH, LPA, S3 | Reported 2026-10-06 (static count, method §3; recounted 2026-10-06 with the corrected examined-code scope of §3.3 rule 3) |
| U4 | Handler attachment forms, `Plugins` global, writes into native tables, library extensions, standard-library and global use (§6.1–§6.4) | LIA, GH | Reported 2026-10-06 |
| U5 | Dependency on the publisher's 2010 sample utility package and on other distributions (§6.5, §6.11) | LIA, GH | Reported 2026-10-06 |
| U6 | Import call forms, plugin-data call forms, image and stretch-mode use, markup use, descriptor and apartment statistics, code size (§6.6–§6.10, §6.12) | LIA, GH | Reported 2026-10-06 |
| U7 | Names plugins reference that this client build does not register (§5.7) | LIA, GH, LPA | Reported 2026-10-06 |
| U8 | Coverage of candidate surfaces (§7.2) and the data classification (§8) | derived from U3 and LPA §17 | Reported 2026-10-06 |
| U9 | The exit-criterion plugin set, versions, download counts, update dates and per-plugin needs (§9) | LIX, LIA | Reported 2026-10-06 (#12 replaced in the second version) |
| U10 | Replacements of `Engine.GetLanguage` and the gameplay objects they reach while a plugin loads; which distributions call them at load; the exact byte-level name tests of each group (§7.4.1, §7.4.2) | LIA, GH (whole-distribution read of the 8 distributions concerned); LUA (outcomes of the tests checked with stock Lua 5.1.5 in the C locale over every one-byte name and every two-byte prefix, first byte 0x00–0xFF, 2026-10-06) | Reported 2026-10-06 |
| U11 | Scope of every §6 and §7.3 count (whole distribution, closure, or both), recounted 2026-10-06 over the same 143 distributions with comment-stripped token rules | LIA, GH | Reported 2026-10-06 |
| U12 | Peak allocator bytes, live bytes after a full collection and Lua VM instruction counts of loading every §9.2 apartment (§6.12), measured with stock Lua 5.1.5 (64-bit) under a stub host API in the dirty scratch area; no client code was run and nothing of the client was read for it | LIA (the §9.2 archives), LUA (the reference interpreter) | Reported 2026-10-06 (measurement on published third-party files, not on the install) |

---

<a id="3"></a>
## 3. Method

<a id="3-1"></a>
### 3.1 Sample

1. Take every LIX entry in the plugin and library categories (336 entries) and sort by
   cumulative downloads. Download the top 140 (the 140th has 41,870 downloads).
2. Add the GH repositories that are not the same plugin as an LIA entry (13 kept; 10 GitHub
   copies of LIA entries and 2 repositories without a descriptor were dropped).
3. Drop archives without any `.plugin` descriptor (9: skins, UI patches and a bag-size patch)
   and the one archive of the libraries category (it has only a stub descriptor).

Result: **143 distributions** (130 from LIA, 13 from GH) with **203 descriptors**.

<a id="3-2"></a>
### 3.2 Unit of count

The unit is the **distribution**: one LIX entry or one GH repository, i.e. what a user installs
in one step. A distribution may hold several descriptors (28 do, §6.10); its usage is the union
over its descriptors. Every count "N distributions" in this spec is out of 143.

<a id="3-3"></a>
### 3.3 Code examined per descriptor

For each descriptor:

1. Find the `Plugins/` root by locating the folder from which the descriptor's `Package`
   resolves (LPA §7.2 rules: a folder with `__init__.lua`, else a `.lua` file; components
   compared case-insensitively). For 14 descriptors (12 in 11 of the GH repositories, which are
   rooted at the plugin's own folder, and 2 in one LIA archive that has an extra folder level)
   the files had to be placed under the folders that the package path names.
2. Starting at the main package, follow every `import` whose argument is a string literal,
   transitively, resolving each name the same way. Names of the five native packages are not
   followed.
3. If any `import` in the reached files has an argument that is not a single string literal
   (a computed name, or `import` passed as a value, for example to a protected call), the
   examined code of that descriptor is **every `.lua` file of the whole distribution**. This
   applies to 46 descriptors in 35 distributions. A computed name may point at any folder of
   the distribution, including a sibling folder of the same author (for example a shared
   library folder next to the plugin's own folder), so no narrower scope is safe.
4. A closure reached through literal imports alone is not widened, whatever its size: such a
   descriptor can load no other file of the distribution.

The **closure scope** used by counts below is this examined file set (rules 2–4), unioned over
a distribution's descriptors. In 91 distributions it is every `.lua` file of the distribution;
in the other 52 some files are not examined because nothing imports them (old copies kept in
archive folders, tests, editor type-stub files, unused library files).

Files are read as text; a leading UTF-8 byte-order mark is skipped. None of the sampled `.lua`
files is a precompiled chunk.

The **whole-distribution scope** is every `.lua` file in the distribution, with comments
removed. In the 91 distributions where the closure is the whole distribution the two scopes
give the same answer; elsewhere a whole-distribution count can only be larger.

Which scope a count uses:

- every count in §5 is **closure scope**, with the rules of §3.4;
- every count in §6 and §7.3 is **whole-distribution scope** unless it is marked "closure
  scope"; where the closure-scope value differs, it follows in parentheses, for example "73
  (closure 71)"; "both scopes" means the two values are equal;
- a count that §6 or §7 copies from §5 keeps its closure scope and says so.

<a id="3-4"></a>
### 3.4 Name set and counting rules

The name set is LPA §16 (classes, statics and enumerations of the current build, with their own
members and events) plus the enumeration member names of LPA Appendix A and S3. A pseudo-member
`Constructor` stands for calls such as `Window.Constructor(self)` used when subclassing.

Inside the examined code, comments are skipped and string contents are not searched for names
(except for `import` arguments and rule 3), and:

1. **Qualified references.** A dotted chain that starts with `Turbine`, or with a local or
   global name that was assigned such a chain (alias, resolved to a fixed point; the global
   `plugin` is treated as an alias of `Turbine.Plugin`), is resolved to the longest prefix that
   is a namespace, class or enumeration; the next component is classified as a member, an
   event, an enumeration value, or an unregistered name (§5.7).
2. **Method calls.** `x:Name(...)` counts the name `Name` if some class of the name set has a
   member of that name.
3. **Handler definitions.** `x.Name = <function or name or table>`, `function x:Name(...)` and
   `function x.Name(...)` count `Name` as an event if some class has an event (or callback,
   §5.4) of that name. A string literal that equals an event name and is an argument of a call
   whose function name contains "callback", "handler", "event", "hook", "listen", "register" or
   "subscribe", or is a table key in brackets, counts as well.
4. **Class used.** A class counts as used by a distribution when it is named (rule 1),
   subclassed, reached through a getter that returns it (for example `GetBackpack` →
   `Backpack`, `GetEffects` → `EffectList` and `Effect`, `GetItems` on a menu →
   `MenuItemList`, `GetNodes` → `TreeNodeList`, `GetShortcut` → `Shortcut`), or, for
   `Turbine.Gameplay` classes and objects only ever returned by the API, when a member or event
   name that only that class has is used. A base class also counts as used when one of its own
   members or events is used through a subclass.
5. **Member used.** A member counts for a class when it is referenced qualified (rule 1), or
   when its name is called with rule 2 **and** the class or one of its subclasses is used
   (rule 4). Events likewise with rule 3.

<a id="3-5"></a>
### 3.5 Limits of the method

- **What the counts bound.** The closure scope holds every file of the distribution that an
  `import` from the distribution's own code can load (rules 2–4 of §3.3). Within that code the
  counts are upper bounds: code that is never executed (tests, dead branches, other-language
  branches, bundled library functions the plugin does not call, and, under the
  whole-distribution widening, files that are never imported) counts. They are **not** upper
  bounds in three respects: packages imported from **outside** the distribution are not
  examined (the user-installed sample package, §6.5, and another author's unbundled library,
  §6.11); computed member access is not seen (below); and names built at run time from strings
  (for example code compiled with `loadstring`) are not seen.
- A member name shared by several classes (`GetItem`, `GetName`, `SetText`, …) is credited to
  every class that has it and is used by the distribution. The column "used by none" in §5.3 is
  reliable; low non-zero counts on shared names may be inflated.
- Calls through `self` inside a subclass (`self:SetSize(...)`) count, because rule 2 does not
  need the receiver's type.
- Computed member access (`obj[name](...)`) is not seen.
- The sample over-represents long-lived plugins (cumulative downloads favour old entries; 53 of
  the 130 LIA distributions were last updated before 2020) and under-represents plugins
  distributed only by in-game managers.
- Spot check against a plain text search of the same files (2026-10-06): `Quickslot` appears
  in 86 distributions (census: class used by 85), `Received` in 70 (event used by 69),
  `ContextMenu` in 71 (68), `TreeView` in 26 (25), `GetBackpack` in 42 (`Backpack` used by 41),
  `GetEffects` in 30 (`EffectList` 29), `SetStretchMode` in 81 (80), a definition of the
  shell-command `Execute` callback in 122 (123).

> **OPEN:** the static counts say what plugins *reference*, not what they *need at run time*.
> The authoritative incompatibility list is the record that M5 exit criterion 4 produces by
> running the §9 set in Tessera; this census should be re-checked against it.

---

<a id="4"></a>
## 4. Sample profile

| Item | Value |
|---|---|
| Distributions / descriptors | 143 / 203 (130 LIA, 13 GH) |
| LIA cumulative downloads | 41,870 to 1,186,264 per distribution; 19.5 million in total |
| LIA last-update year | 2010–2014: 32; 2015–2019: 21; 2020–2024: 37; 2025–2026: 40 (77 of 130 updated in 2020 or later) |
| LIA categories | other 40, raiding 18, action bars 16, uncategorised stand-alone 12, class-specific 11, maps 10, bags 8, crafting 5, role-play and music 5, unit frames 4, creep play 1 |
| Descriptors without an `Apartment` | 47 of 203 (they load into the default apartment, LPA §6.1) |
| Distributions with several descriptors | 28; 31 descriptors are "reloader"/"unloader" helpers (§6.10) |
| Descriptors with an `Image` | 144 of 203 |
| Lua source per distribution | median 146 KB, 90th percentile about 1 MB; the largest: 32.5 MB, 10.1 MB and 6.5 MB (data tables in Lua, §6.12) |

---

<a id="5"></a>
## 5. Usage ranking

<a id="5-1"></a>
### 5.1 Native package imports

| `import` argument | Distributions |
|---|---|
| `Turbine.UI.Lotro` | 140 |
| `Turbine.UI` | 134 |
| `Turbine.Gameplay` | 132 |
| `Turbine` | 108 |
| `Turbine.Gameplay.Attributes` | 8 |

Imports of names below `Turbine` that are **not** native packages (they come from the
publisher's 2010 sample utility package, LPA §4.2 and §7.4) are counted in §6.5.

<a id="5-2"></a>
### 5.2 Classes, statics and enumerations

Columns: *Tier* = implementation tier in LPA §17 (1, 2, 3; "—" = not registered); *Data* = where
the values come from under the sandbox backend (§8: **H** host only, **H\*** host for alias
shortcuts, game state for other shortcut types, **S** sandbox-supplied local-character values,
**N** game-server state).

| Rank | Name | Kind | Tier (LPA §17) | Data | Distributions (of 143) |
|---|---|---|---|---|---|
| 1 | `Turbine.UI.Control` | class | 1 | H | 141 |
| 2 | `Turbine.Shell` | static | 1 | H | 140 |
| 3 | `Turbine.UI.Label` | class | 1 | H | 138 |
| 4 | `Turbine.UI.Window` | class | 1 | H | 137 |
| 5 | `Turbine.UI.Color` | class | 1 | H | 136 |
| 6 | `Turbine.DataScope` | enum | 1 | H | 135 |
| 7 | `Turbine.PluginData` | static | 1 | H | 135 |
| 8 | `Turbine.UI.ContentAlignment` | enum | 1 | H | 133 |
| 9 | `Turbine.UI.Lotro.Font` | enum | 1 | H | 129 |
| 10 | `Turbine.ShellCommand` | class | 1 | H | 123 |
| 11 | `Turbine.Plugin` | class | 1 | H | 122 |
| 12 | `Turbine.UI.Display` | static | 1 | H | 122 |
| 13 | `Turbine.Gameplay.LocalPlayer` | class | 1 | S | 120 |
| 14 | `Turbine.Engine` | static | 1 | H | 114 |
| 15 | `Turbine.UI.Button` | class | 1 | H | 114 |
| 16 | `Turbine.UI.Lotro.ScrollBar` | class | 1 | H | 107 |
| 17 | `Turbine.UI.ScrollBar` | class | 1 | H | 107 |
| 18 | `Turbine.UI.Orientation` | enum | 1 | H | 106 |
| 19 | `Turbine.Gameplay.Entity` | class | 1 | S | 100 |
| 20 | `Turbine.UI.Lotro.Button` | class | 1 | H | 100 |
| 21 | `Turbine.UI.BlendMode` | enum | 1 | H | 97 |
| 22 | `Turbine.UI.CheckBox` | class | 1 | H | 95 |
| 23 | `Turbine.UI.Lotro.CheckBox` | class | 1 | H | 95 |
| 24 | `Turbine.UI.ListBox` | class | 1 | H | 94 |
| 25 | `Turbine.UI.Lotro.TextBox` | class | 1 | H | 94 |
| 26 | `Turbine.UI.Lotro.Window` | class | 1 | H | 94 |
| 27 | `Turbine.UI.ScrollableControl` | class | 1 | H | 93 |
| 28 | `Turbine.UI.MouseButton` | enum | 1 | H | 92 |
| 29 | `Turbine.UI.Lotro.Shortcut` | class | 2 | H* | 87 |
| 30 | `Turbine.UI.Lotro.Quickslot` | class | 2 | H* | 85 |
| 31 | `Turbine.Gameplay.Player` | class | 1 | S | 82 |
| 32 | `Turbine.UI.FontStyle` | enum | 1 | H | 81 |
| 33 | `Turbine.UI.Lotro.Action` | enum | 1 | H | 72 |
| 34 | `Turbine.Chat` | class | 1 | H | 70 |
| 35 | `Turbine.UI.TextBox` | class | 1 | H | 70 |
| 36 | `Turbine.UI.MenuItem` | class | 2 | H | 69 |
| 37 | `Turbine.UI.ContextMenu` | class | 2 | H | 68 |
| 38 | `Turbine.UI.Lotro.ShortcutType` | enum | 2 | H* | 68 |
| 39 | `Turbine.UI.MenuItemList` | class | 2 | H | 67 |
| 40 | `Turbine.Language` | enum | 1 | H | 61 |
| 41 | `Turbine.ChatType` | enum | 1 | H | 59 |
| 42 | `Turbine.Gameplay.Actor` | class | 1 | S | 59 |
| 43 | `Turbine.UI.Lotro.DragDropInfo` | class | 2 | H* | 51 |
| 44 | `Turbine.Object` | class | 1 | H | 49 |
| 45 | `Turbine.PluginManager` | static | 1 | H | 46 |
| 46 | `Turbine.Gameplay.Item` | class | 2 | N | 43 |
| 47 | `Turbine.Gameplay.Backpack` | class | 2 | N | 41 |
| 48 | `Turbine.Gameplay.Attributes` | class | 2 | N | 35 |
| 49 | `Turbine.UI.ControlList` | class | 1 | H | 32 |
| 50 | `Turbine.Gameplay.Class` | enum | 1 | S | 31 |
| 51 | `Turbine.Gameplay.Effect` | class | 2 | N | 31 |
| 52 | `Turbine.Gameplay.ItemInfo` | class | 2 | N | 30 |
| 53 | `Turbine.Gameplay.EffectList` | class | 2 | N | 29 |
| 54 | `Turbine.UI.Lotro.GoldButton` | class | 1 | H | 27 |
| 55 | `Turbine.UI.Lotro.GoldWindow` | class | 1 | H | 27 |
| 56 | `Turbine.Gameplay.Equipment` | class | 2 | N | 26 |
| 57 | `Turbine.UI.TreeNode` | class | 2 | H | 25 |
| 58 | `Turbine.UI.TreeNodeList` | class | 2 | H | 25 |
| 59 | `Turbine.UI.TreeView` | class | 2 | H | 25 |
| 60 | `Turbine.Gameplay.SkillInfo` | class | 3 | N | 23 |
| 61 | `Turbine.Gameplay.SkillList` | class | 3 | N | 23 |
| 62 | `Turbine.Gameplay.EffectCategory` | enum | 2 | N | 22 |
| 63 | `Turbine.Gameplay.Skill` | class | 3 | N | 22 |
| 64 | `Turbine.Gameplay.Attributes.FreePeopleAttributes` | class | 2 | N | 19 |
| 65 | `Turbine.Gameplay.ClassAttributes` | class | 2 | N | 19 |
| 66 | `Turbine.Gameplay.WalletItem` | class | 2 | N | 18 |
| 67 | `Turbine.Gameplay.Wallet` | class | 2 | N | 17 |
| 68 | `Turbine.UI.Lotro.ItemControl` | class | 2 | N | 17 |
| 69 | `Turbine.Gameplay.Bank` | class | 3 | N | 16 |
| 70 | `Turbine.Gameplay.Party` | class | 2 | N | 15 |
| 71 | `Turbine.Gameplay.PartyMember` | class | 2 | N | 15 |
| 72 | `Turbine.Gameplay.Vault` | class | 3 | N | 15 |
| 73 | `Turbine.Gameplay.SharedStorage` | class | 3 | N | 13 |
| 74 | `Turbine.UI.Graphic` | class | 1 | H | 13 |
| 75 | `Turbine.UI.Lotro.ItemInfoControl` | class | 2 | N | 13 |
| 76 | `Turbine.Gameplay.ActiveSkill` | class | 3 | N | 12 |
| 77 | `Turbine.UI.DragDropInfo` | class | 2 | H | 12 |
| 78 | `Turbine.UI.Lotro.LotroUI` | static | 1 | H | 12 |
| 79 | `Turbine.Gameplay.Attributes.MinstrelStance` | enum | 3 | N | 11 |
| 80 | `Turbine.UI.Lotro.EntityControl` | class | 2 | N | 11 |
| 81 | `Turbine.Gameplay.Race` | enum | 1 | S | 10 |
| 82 | `Turbine.UI.Lotro.EffectDisplay` | class | 2 | N | 10 |
| 83 | `Turbine.Gameplay.Attributes.BeorningAttributes` | class | 3 | N | 8 |
| 84 | `Turbine.Gameplay.Mount` | class | 3 | N | 8 |
| 85 | `Turbine.Gameplay.ProfessionInfo` | class | 3 | N | 8 |
| 86 | `Turbine.UI.Lotro.LotroUIElement` | enum | 1 | H | 8 |
| 87 | `Turbine.Gameplay.Alignment` | enum | 1 | S | 7 |
| 88 | `Turbine.Gameplay.Attributes.ChampionAttributes` | class | 3 | N | 7 |
| 89 | `Turbine.Gameplay.Attributes.HunterAttributes` | class | 3 | N | 7 |
| 90 | `Turbine.Gameplay.ItemCategory` | enum | 2 | N | 7 |
| 91 | `Turbine.Gameplay.Profession` | enum | 3 | N | 7 |
| 92 | `Turbine.Gameplay.Recipe` | class | 3 | N | 7 |
| 93 | `Turbine.Gameplay.Attributes.RuneKeeperAttributes` | class | 3 | N | 6 |
| 94 | `Turbine.Gameplay.Attributes.WardenAttributes` | class | 3 | N | 5 |
| 95 | `Turbine.Gameplay.Attributes.HunterStance` | enum | 3 | N | 4 |
| 96 | `Turbine.Gameplay.Pet` | class | 2 | N | 4 |
| 97 | `Turbine.Gameplay.RecipeIngredient` | class | 3 | N | 4 |
| 98 | `Turbine.UI.EdgeAttachmentType` | enum | 3 | H | 4 |
| 99 | `Turbine.Gameplay.Attributes.MarinerAttributes` | class | 3 | N | 3 |
| 100 | `Turbine.Gameplay.ReadyState` | enum | 2 | N | 3 |
| 101 | `Turbine.Gameplay.Vocation` | enum | 3 | N | 3 |
| 102 | `Turbine.Gameplay.Attributes.ChampionStance` | enum | 3 | N | 2 |
| 103 | `Turbine.Gameplay.Attributes.GuardianAttributes` | class | 3 | N | 2 |
| 104 | `Turbine.Gameplay.Attributes.MinstrelAttributes` | class | 3 | N | 2 |
| 105 | `Turbine.Gameplay.Attributes.WardenStance` | enum | 3 | N | 2 |
| 106 | `Turbine.Gameplay.CombatMount` | class | 3 | N | 2 |
| 107 | `Turbine.Gameplay.CraftTier` | enum | 3 | N | 2 |
| 108 | `Turbine.Gameplay.ItemQuality` | enum | 2 | N | 2 |
| 109 | `Turbine.UI.HorizontalLayout` | enum | 1 | H | 2 |
| 110 | `Turbine.Gameplay.Attributes.BurglarStance` | enum | 3 | N | 1 |
| 111 | `Turbine.Gameplay.Attributes.CaptainAttributes` | class | 3 | N | 1 |
| 112 | `Turbine.Gameplay.Attributes.GuardianStance` | enum | 3 | N | 1 |
| 113 | `Turbine.Gameplay.Attributes.WardenGambit` | enum | 3 | N | 1 |
| 114 | `Turbine.Gameplay.BankItem` | class | 3 | N | 1 |
| 115 | `Turbine.Gameplay.ItemDurability` | enum | 2 | N | 1 |
| 116 | `Turbine.Gameplay.ItemWearState` | enum | 2 | N | 1 |
| 117 | `Turbine.UI.Lotro.EquipmentSlot` | class | 2 | N | 1 |
| 118 | `Turbine.UI.Lotro.UIScalingGroupType` | enum | 3 | H | 1 |

Used by **no** distribution: `Gameplay.Attributes.BlackArrowAttributes`, `Gameplay.Attributes.BrawlerAttributes`, `Gameplay.Attributes.BrawlerStance`, `Gameplay.Attributes.BurglarAttributes`, `Gameplay.Attributes.ChickenAttributes`, `Gameplay.Attributes.DefilerAttributes`, `Gameplay.Attributes.DwarfAttributes`, `Gameplay.Attributes.ElfAttributes`, `Gameplay.Attributes.HighElfAttributes`, `Gameplay.Attributes.HobbitAttributes`, `Gameplay.Attributes.LoreMasterAttributes`, `Gameplay.Attributes.ManAttributes`, `Gameplay.Attributes.MonsterPlayerAttributes`, `Gameplay.Attributes.RangerAttributes`, `Gameplay.Attributes.ReaverAttributes`, `Gameplay.Attributes.RiverHobbitAttributes`, `Gameplay.Attributes.StalkerAttributes`, `Gameplay.Attributes.StoutAxeAttributes`, `Gameplay.Attributes.TrollAttributes`, `Gameplay.Attributes.WarLeaderAttributes`, `Gameplay.Attributes.WeaverAttributes`, `Gameplay.BasicMount`, `Gameplay.EntityReference`, `Gameplay.GambitSkill`, `Gameplay.GambitSkillInfo`, `Gameplay.PropertyHandler`, `Gameplay.Raid`, `Gameplay.SkillType`, `Gameplay.UntrainedSkill`, `UI.ComboBox`, `UI.Lotro.BaseItemControl`, `UI.VerticalLayout`.

<a id="5-3"></a>
### 5.3 Members per class

Own members only (inherited ones are listed under the class that defines them, LPA §16).
`Constructor` is omitted (subclassing is counted in §6.5). Static functions are included.
Classes whose rows show "—" in both columns have no own members in LPA §16, or are counted
only as enumerations. `Equipment`'s object methods `GetItem` and `GetSize` (LPA §16 lists them in
the kind column) are used by 26 distributions each.

| Class | Members used (distributions) | Members used by none |
|---|---|---|
| `UI.Control` | SetParent 140, SetPosition 140, SetSize 140, SetVisible 140, SetBackColor 134, SetBackground 129, GetHeight 128, GetWidth 127, SetZOrder 126, SetMouseVisible 121, SetWantsUpdates 117, GetPosition 111, GetTop 107, SetWantsKeyEvents 107, SetBlendMode 103, GetSize 101, GetLeft 100, IsVisible 98, SetLeft 97, SetTop 96, SetOpacity 95, SetWidth 92, SetHeight 89, SetStretchMode 80, SetEnabled 70, GetParent 59, SetAllowDrop 58, SetBackColorBlendMode 54, GetZOrder 51, GetBackColor 50, PointToScreen 44, Focus 43, IsEnabled 40, GetMousePosition 37, GetControls 31, GetOpacity 23, IsShiftKeyDown 21, IsControlKeyDown 15, SetMinimumSize 15, GetWantsUpdates 13, IsAltKeyDown 13, PointToClient 12, SetMinimumWidth 11, SetMaximumWidth 9, SetMinimumHeight 9, GetBackground 7, IsMouseVisible 6, AttachEdges 4, HasFocus 4, SetMaximumHeight 4, SetMaximumSize 4, GetMinimumHeight 3, GetMinimumWidth 3, GetAllowDrop 1, GetStretchMode 1, GetWantsKeyEvents 1, IsDisplayed 1 | GetBackColorBlendMode, GetBlendMode, GetEdgeAttachments, GetMaximumHeight, GetMaximumSize, GetMaximumWidth, GetMinimumSize |
| `Shell` | WriteLine 139, AddCommand 123, IsCommand 40, RemoveCommand 23, GetCommands 8 | — |
| `UI.Label` | SetText 138, SetTextAlignment 133, SetFont 129, SetForeColor 127, GetText 105, SetFontStyle 104, SetOutlineColor 93, SetMultiline 80, SetSelectable 39, SetMarkupEnabled 38, SetSelection 25, GetFont 20, GetSelectionStart 15, GetSelectedText 14, GetSelectionLength 14, AppendText 13, GetForeColor 13, SelectAll 13, GetTextLength 11, SetSelectionLength 11, SetSelectionStart 11, GetTextAlignment 9, GetFontStyle 8, IsMultiline 8, GetOutlineColor 5, SetSelectedText 5, IsMarkupEnabled 3, InsertText 2, IsSelectable 1 | DeselectAll |
| `UI.Window` | SetText 136, GetText 104, Close 65, Activate 49, SetRotation 14, SetScale 10, UnregisterForGlobalScaling 10, GetScale 7, RegisterForGlobalScaling 6, GetRotation 3, SetUIScalingGroup 1 | GetScalingOriginPoint, GetUIScalingGroup, SetScalingOriginPoint |
| `UI.Color` | construction `Color(…)` with 3 arguments 121, with 4 arguments 119, with none 28 (whole-distribution scope); the fields `A`, `R`, `G`, `B` are read on instances and not counted | — |
| `PluginData` | Save 135, Load 134 | — |
| `ShellCommand` | GetHelp 26, GetShortHelp 8, Execute 7 | — |
| `Plugin` | GetName 92, GetVersion 90, GetOptionsPanel 47, GetAuthor 11, GetUsesGlobalScaling 8 | — |
| `UI.Display` | GetHeight 118, GetWidth 117, GetSize 98, GetMousePosition 46, GetMouseX 35, GetMouseY 34 | GetUIGroupScale |
| `Gameplay.LocalPlayer` | GetInstance 120, IsInCombat 54, GetBackpack 41, GetEquipment 26, GetTrainedSkills 21, GetAttributes 19, GetClassAttributes 19, GetWallet 16, GetVault 15, GetSharedStorage 13, GetMount 7, GetUntrainedSkills 6 | GetRaceAttributes |
| `Engine` | GetGameTime 81, GetLanguage 66, GetDate 39, GetLocalTime 22, GetCallStack 10, GetLocale 7, ScriptLog 4 | GetScriptVersion |
| `UI.Button` | — | — |
| `UI.Lotro.ScrollBar` | — | — |
| `UI.ScrollBar` | SetOrientation 105, GetValue 74, SetValue 69, SetMaximum 58, SetMinimum 57, SetSmallChange 22, SetLargeChange 21, GetMaximum 11, GetOrientation 4, SetDecrementButton 3, SetIncrementButton 3, SetThumbButton 3, GetMinimum 2, GetLargeChange 1 | GetDecrementButton, GetIncrementButton, GetSmallChange, GetThumbButton |
| `Gameplay.Entity` | GetName 100, IsLocalPlayer 4 | — |
| `UI.Lotro.Button` | — | — |
| `UI.CheckBox` | SetChecked 94, IsChecked 92, SetCheckAlignment 14 | GetCheckAlignment |
| `UI.Lotro.CheckBox` | — | — |
| `UI.ListBox` | AddItem 94, SetOrientation 90, GetItem 79, GetItemCount 71, ClearItems 60, GetSelectedIndex 39, RemoveItemAt 37, SetSelectedIndex 31, InsertItem 28, RemoveItem 28, SetMaxItemsPerLine 27, IndexOfItem 23, GetSelectedItem 18, SetItem 16, EnsureVisible 14, Sort 10, SetSelectedItem 7, SetMaxColumns 6, GetItemAt 5, SetReverseFill 5, ContainsItem 4, GetOrientation 4, SetMaxRows 3, SetFlippedLayout 2, GetMaxItemsPerLine 1 | GetFlippedLayout, GetIgnoreCellAlignment, GetMaxColumns, GetMaxRows, GetReverseFill, GetWrapIndentAmount, SetIgnoreCellAlignment, SetWrapIndentAmount |
| `UI.Lotro.TextBox` | — | — |
| `UI.Lotro.Window` | SetResizable 26 | IsResizable |
| `UI.ScrollableControl` | SetVerticalScrollBar 93, SetHorizontalScrollBar 53, GetHorizontalScrollBar 6, GetVerticalScrollBar 4 | — |
| `UI.Lotro.Shortcut` | GetItem 60, GetData 51, GetType 41, SetData 35, SetType 11 | — |
| `UI.Lotro.Quickslot` | SetShortcut 84, GetShortcut 48, SetUseOnRightClick 29 | IsUseOnRightClick |
| `Gameplay.Player` | GetClass 74, GetAlignment 21, GetRace 17, GetParty 14, GetPet 4, IsLinkDead 3, GetReadyState 1, IsVoiceActive 1, IsVoiceEnabled 1 | — |
| `Chat` | — | — |
| `UI.TextBox` | SetReadOnly 29, IsReadOnly 1 | — |
| `UI.MenuItem` | SetText 69, GetItems 67, GetText 62, SetChecked 60, IsChecked 55, SetEnabled 45, IsEnabled 28 | — |
| `UI.ContextMenu` | GetItems 67, ShowMenu 45, Close 43, ShowMenuAt 25 | — |
| `UI.MenuItemList` | Add 67, GetCount 52, Get 47, Clear 30, Remove 10, Insert 8, RemoveAt 7, Set 6, Contains 4, IndexOf 3 | — |
| `Gameplay.Actor` | GetEffects 29, GetLevel 29, GetMaxMorale 22, GetMorale 22, GetTarget 21, GetMaxPower 19, GetPower 16, GetTemporaryMorale 8, GetMaxTemporaryMorale 7, GetBaseMaxMorale 6, GetBaseMaxPower 6, GetMaxTemporaryPower 3, GetTemporaryPower 3 | — |
| `UI.Lotro.DragDropInfo` | GetShortcut 51 | — |
| `Object` | IsA 45 | — |
| `PluginManager` | LoadPlugin 31, UnloadScriptState 28, GetLoadedPlugins 19, ShowOptions 18, GetAvailablePlugins 13, RefreshAvailablePlugins 9 | — |
| `Gameplay.Item` | GetItemInfo 24, GetQuantity 24, GetIconImageID 16, GetCategory 13, GetQuality 12, GetBackgroundImageID 11, GetShadowImageID 9, GetMaxStackSize 8, GetQualityImageID 8, GetUnderlayImageID 8, GetWearState 8, GetDurability 7, IsMagic 1 | — |
| `Gameplay.Backpack` | GetItem 41, GetSize 41, PerformItemDrop 18, PerformShortcutDrop 3 | — |
| `Gameplay.Attributes` | — | — |
| `UI.ControlList` | Get 31, GetCount 30, Clear 27, Add 26, Remove 8, RemoveAt 6, Insert 5, Contains 4, Set 4, IndexOf 3 | — |
| `Gameplay.Effect` | GetName 31, GetDuration 19, GetStartTime 19, GetID 13, GetCategory 12, GetIcon 12, IsCurable 10, IsDebuff 9, GetDescription 6 | — |
| `Gameplay.ItemInfo` | GetName 30, GetQuality 15, GetIconImageID 14, GetBackgroundImageID 12, GetCategory 10, GetDescription 8, GetMaxStackSize 8, GetQualityImageID 8, GetShadowImageID 8, GetUnderlayImageID 8, GetDurability 7, GetMaxQuantity 2, GetNameWithQuantity 1, IsMagic 1, IsUnique 1 | — |
| `Gameplay.EffectList` | Get 29, GetCount 27, Contains 4 | IndexOf |
| `UI.Lotro.GoldButton` | — | — |
| `UI.Lotro.GoldWindow` | — | — |
| `Gameplay.Equipment` | — | — |
| `UI.TreeNode` | GetChildNodes 25, IsExpanded 23, SetExpanded 19, GetParentNode 14, CollapseAll 9, ExpandAll 9, Expand 5, IsSelected 4, Collapse 1 | — |
| `UI.TreeNodeList` | Add 25, Get 25, GetCount 24, Clear 20, Remove 6, RemoveAt 4, Contains 2, IndexOf 1 | — |
| `UI.TreeView` | GetNodes 25, SetIndentationWidth 23, CollapseAll 9, ExpandAll 9, Refresh 7, GetSelectedNode 2, SetFilter 2, GetIndentationWidth 1, GetItemAt 1, SetSelectedNode 1, SetSortMethod 1 | GetFilter, GetSortMethod |
| `Gameplay.SkillInfo` | GetName 23, GetIconImageID 11, GetDescription 10, GetType 8 | — |
| `Gameplay.SkillList` | GetItem 23, GetCount 20 | — |
| `Gameplay.Skill` | GetSkillInfo 22 | — |
| `Gameplay.Attributes.FreePeopleAttributes` | GetAgility 7, GetBaseResistance 7, GetBlock 7, GetCommonMitigation 7, GetEvade 7, GetFate 7, GetFinesse 7, GetIncomingHealing 7, GetMeleeDamage 7, GetMight 7, GetMoney 7, GetParry 7, GetProfessionInfo 7, GetTacticalDamage 7, GetTacticalMitigation 7, GetVitality 7, GetWill 7, GetArmor 6, GetBaseCriticalHitAvoidance 6, GetDestinyPoints 6, GetOutgoingHealing 6, GetPhysicalMitigation 6, GetBaseCriticalHitChance 5, GetRangeDamage 5, GetVocation 5, GetInCombatMoraleRegeneration 3, GetInCombatPowerRegeneration 3, CanBlock 2, CanEvade 2, CanParry 2, GetAcidMitigation 2, GetFireMitigation 2, GetFrostMitigation 2, GetLightningMitigation 2, GetMeleeCriticalHitChance 2, GetMoneyComponents 2, GetOutOfCombatMoraleRegeneration 2, GetOutOfCombatPowerRegeneration 2, GetShadowMitigation 2, GetBaseAgility 1, GetBaseFate 1, GetBaseMight 1, GetBaseVitality 1, GetBaseWill 1, GetMeleeCriticalHitAvoidance 1, GetMeleeDefence 1, GetRangeDefence 1, GetTacticalCriticalHitAvoidance 1, GetTacticalCriticalHitChance 1, GetTacticalDefence 1 | GetBaseArmor, GetDiseaseResistance, GetFearResistance, GetPoisonResistance, GetRangeCriticalHitAvoidance, GetRangeCriticalHitChance, GetWoundResistance |
| `Gameplay.ClassAttributes` | — | — |
| `Gameplay.WalletItem` | GetName 18, GetQuantity 16, GetImage 6, GetDescription 5, GetSmallImage 5, IsAccountItem 5, GetMaxQuantity 3 | — |
| `Gameplay.Wallet` | GetSize 17, GetItem 16 | — |
| `UI.Lotro.ItemControl` | GetItem 15, SetItem 10 | — |
| `Gameplay.Bank` | GetItem 16, GetCapacity 11, GetCount 11, IsAvailable 7, PerformShortcutDrop 3, GetChestCount 1, GetChestName 1 | — |
| `Gameplay.Party` | GetMember 15, GetMemberCount 15, GetLeader 5, GetAssistTarget 4, GetAssistTargetCount 4, IsAssistTarget 2 | IsMember |
| `Gameplay.PartyMember` | — | — |
| `Gameplay.Vault` | — | — |
| `Gameplay.SharedStorage` | — | — |
| `UI.Graphic` | — | — |
| `UI.Lotro.ItemInfoControl` | GetItemInfo 13, SetItemInfo 13, SetQuantity 11, GetQuantity 7 | — |
| `Gameplay.ActiveSkill` | GetResetTime 11, GetCooldown 6, IsUsable 4, GetBaseCooldown 3 | — |
| `UI.DragDropInfo` | SetSuccessful 1 | IsSuccessful |
| `UI.Lotro.LotroUI` | SetEnabled 12, IsEnabled 5, Reset 2 | — |
| `UI.Lotro.EntityControl` | SetEntity 11, GetEntity 3, SetContextMenuEnabled 3, SetSelectionEnabled 3 | IsContextMenuEnabled, IsSelectionEnabled |
| `UI.Lotro.EffectDisplay` | SetEffect 8, GetEffect 2 | — |
| `Gameplay.Attributes.BeorningAttributes` | GetWrath 8 | IsInBearForm |
| `Gameplay.Mount` | GetName 8 | — |
| `Gameplay.ProfessionInfo` | GetName 8, GetProficiencyLevel 6, GetMasteryLevel 5, GetRecipe 5, GetRecipeCount 5, GetMasteryExperience 4, GetMasteryExperienceTarget 4, GetProficiencyExperience 4, GetProficiencyExperienceTarget 4 | GetMasteryTitle, GetProficiencyTitle |
| `Gameplay.Attributes.ChampionAttributes` | GetFervor 7, GetStance 3 | — |
| `Gameplay.Attributes.HunterAttributes` | GetFocus 7, GetStance 3 | — |
| `Gameplay.Recipe` | GetName 7, GetTier 6, GetCategoryName 4, GetIngredient 4, GetIngredientCount 4, GetResultItemInfo 3, GetCategory 2, GetCooldown 2, GetProfession 2, GetBaseCriticalSuccessChance 1, GetCriticalResultItemInfo 1, GetCriticalResultItemQuantity 1, GetExperienceReward 1, GetIngredientPack 1, GetOptionalIngredient 1, GetOptionalIngredientCount 1, GetResultItemQuantity 1, HasCriticalResultItem 1, IsSingleUse 1 | — |
| `Gameplay.Attributes.RuneKeeperAttributes` | GetAttunement 6 | IsCharged |
| `Gameplay.Attributes.WardenAttributes` | GetGambit 3, GetTrainedGambits 3, GetGambitCount 2, GetMaxGambitCount 2, GetStance 2 | GetUntrainedGambits |
| `Gameplay.Pet` | — | — |
| `Gameplay.RecipeIngredient` | GetItemInfo 4, GetRequiredQuantity 4, GetCriticalChanceBonus 1 | — |
| `Gameplay.Attributes.MarinerAttributes` | GetBalance 3 | — |
| `Gameplay.Attributes.GuardianAttributes` | GetStance 2, IsBlockTier3Available 2, IsParryTier3Available 2, IsBlockTier1Available 1, IsBlockTier2Available 1, IsParryTier1Available 1, IsParryTier2Available 1 | — |
| `Gameplay.Attributes.MinstrelAttributes` | IsSerenadeTier1Available 2, IsSerenadeTier2Available 2, IsSerenadeTier3Available 2, GetStance 1 | — |
| `Gameplay.CombatMount` | — | GetAgility, GetArmor, GetBaseAgility, GetBaseArmor, GetBaseMaxMorale, GetBaseMaxPower, GetBaseStrength, GetEvasion, GetFury, GetMaxFury, GetMaxMorale, GetMaxPower, GetMaxTemporaryMorale, GetMaxTemporaryPower, GetMorale, GetPower, GetStrength, GetTemporaryMorale, GetTemporaryPower, GetTurnRate |
| `Gameplay.Attributes.CaptainAttributes` | IsInEnemyDefeatResponse 1, IsInFellowDefeatResponse 1, IsReadiedTier1Available 1, IsReadiedTier2Available 1 | — |
| `Gameplay.BankItem` | GetChest 1, GetQuantity 1, GetWearState 1 | GetBoundOwner |
| `UI.Lotro.EquipmentSlot` | SetEquipmentSlot 1 | GetEquipmentSlot |
| `Gameplay.Attributes.BlackArrowAttributes` | — | — |
| `Gameplay.Attributes.BrawlerAttributes` | — | GetStance |
| `Gameplay.Attributes.BurglarAttributes` | — | GetStance |
| `Gameplay.Attributes.ChickenAttributes` | — | — |
| `Gameplay.Attributes.DefilerAttributes` | — | — |
| `Gameplay.Attributes.DwarfAttributes` | — | — |
| `Gameplay.Attributes.ElfAttributes` | — | — |
| `Gameplay.Attributes.HighElfAttributes` | — | — |
| `Gameplay.Attributes.HobbitAttributes` | — | — |
| `Gameplay.Attributes.LoreMasterAttributes` | — | — |
| `Gameplay.Attributes.ManAttributes` | — | — |
| `Gameplay.Attributes.MonsterPlayerAttributes` | — | GetDestinyPoints, GetMoney, GetMoneyComponents |
| `Gameplay.Attributes.RangerAttributes` | — | — |
| `Gameplay.Attributes.ReaverAttributes` | — | — |
| `Gameplay.Attributes.RiverHobbitAttributes` | — | — |
| `Gameplay.Attributes.StalkerAttributes` | — | — |
| `Gameplay.Attributes.TrollAttributes` | — | GetRampage |
| `Gameplay.Attributes.WarLeaderAttributes` | — | — |
| `Gameplay.Attributes.WeaverAttributes` | — | — |
| `Gameplay.BasicMount` | — | GetMaxMorale, GetMorale |
| `Gameplay.EntityReference` | — | — |
| `Gameplay.GambitSkill` | — | — |
| `Gameplay.GambitSkillInfo` | — | GetGambit, GetGambitCount |
| `Gameplay.PropertyHandler` | — | — |
| `Gameplay.Raid` | — | — |
| `Gameplay.UntrainedSkill` | — | GetPrice, GetRequiredLevel, GetRequiredRank |
| `UI.Lotro.BaseItemControl` | — | — |

<a id="5-4"></a>
### 5.4 Events and callbacks

"Callback" marks a function the plugin defines for the host to call (the shell command's
`Execute`, `GetHelp`, `GetShortHelp`; the plugin's `GetOptionsPanel`) rather than an event
fired by the host. Counts for `Plugin.GetOptionsPanel` differ by form: assigned on a plugin
object in 47 distributions (§5.3 row), defined with a handler form of rule 3 in 8.

| Class | Events and callbacks used (distributions) | Events used by none |
|---|---|---|
| `UI.Control` | MouseDown 124, MouseUp 123, MouseMove 121, MouseClick 120, MouseLeave 119, Update 118, MouseEnter 117, KeyDown 107, PositionChanged 70, SizeChanged 65, DragDrop 49, FocusLost 47, VisibleChanged 41, FocusGained 40, MouseHover 27, MouseDoubleClick 24, MouseWheel 21, DragLeave 10, KeyUp 10, DragEnter 9, EnabledChanged 8, DragStart 2 | — |
| `UI.Window` | Closing 59, Closed 32, Deactivated 17, Activated 11, UIScaleChanged 6 | — |
| `ShellCommand` | Execute (callback) 123, GetHelp (callback) 72, GetShortHelp (callback) 63 | — |
| `Plugin` | Unload 102, Load 60, GetOptionsPanel (callback) 8 | — |
| `Gameplay.LocalPlayer` | InCombatChanged 34, MountChanged 6 | — |
| `UI.Button` | Click 95, EnabledChanged 8 | — |
| `UI.Lotro.ScrollBar` | OrientationChanged 1 | — |
| `UI.ScrollBar` | ValueChanged 66, OrientationChanged 1 | — |
| `Gameplay.Entity` | NameChanged 7 | — |
| `UI.CheckBox` | CheckedChanged 79, EnabledChanged 7 | — |
| `UI.ListBox` | SelectedIndexChanged 35, ItemRemoved 25, ItemAdded 23 | — |
| `UI.Lotro.Quickslot` | DragDrop 44, ShortcutChanged 20 | — |
| `Gameplay.Player` | PartyChanged 6, IsLinkDeadChanged 4, PetChanged 2, IsVoiceActiveChanged 1, IsVoiceEnabledChanged 1, RaidChanged 1, ReadyStateChanged 1 | — |
| `Chat` | Received 69 | — |
| `UI.TextBox` | TextChanged 43, EnabledChanged 8 | — |
| `UI.MenuItem` | Click 67 | — |
| `Gameplay.Actor` | TargetChanged 18, LevelChanged 16, MoraleChanged 13, PowerChanged 11, BaseMaxMoraleChanged 10, MaxMoraleChanged 10, TemporaryMoraleChanged 9, BaseMaxPowerChanged 8, MaxPowerChanged 8, MaxTemporaryMoraleChanged 8, MaxTemporaryPowerChanged 3, TemporaryPowerChanged 3 | — |
| `Gameplay.Item` | QuantityChanged 15 | — |
| `Gameplay.Backpack` | SizeChanged 27, ItemRemoved 21, ItemAdded 19, ItemMoved 10 | — |
| `Gameplay.Effect` | NameChanged 3 | CategoryChanged, DescriptionChanged, DurationChanged, IconChanged, IsCurableChanged, IsDebuffChanged, IsDispellableChanged, StartTimeChanged |
| `Gameplay.EffectList` | EffectAdded 27, EffectRemoved 24, EffectsCleared 10 | — |
| `Gameplay.Equipment` | ItemEquipped 9, ItemUnequipped 8 | — |
| `UI.TreeNode` | SelectedChanged 3 | — |
| `UI.TreeView` | SelectedNodeChanged 12 | — |
| `Gameplay.SkillList` | SkillAdded 5, SkillRemoved 4 | — |
| `Gameplay.WalletItem` | QuantityChanged 11 | MaxQuantityChanged |
| `Gameplay.Wallet` | ItemAdded 12, ItemRemoved 12 | — |
| `Gameplay.Bank` | ItemRemoved 11, ItemAdded 10, CountChanged 8, IsAvailableChanged 4, ItemMoved 4, ItemsRefreshed 4, CapacityChanged 1 | ChestNameChanged |
| `Gameplay.Party` | MemberAdded 6, MemberRemoved 6, LeaderChanged 3, AssistTargetAdded 2, AssistTargetRemoved 2 | — |
| `UI.Lotro.ItemInfoControl` | EnabledChanged 3 | — |
| `Gameplay.ActiveSkill` | ResetTimeChanged 10 | — |
| `Gameplay.Attributes.BeorningAttributes` | WrathChanged 5 | FormChanged |
| `Gameplay.ProfessionInfo` | MasteryExperienceChanged 1, MasteryLevelChanged 1, ProficiencyExperienceChanged 1, ProficiencyLevelChanged 1 | MasteryExperienceTargetChanged, MasteryTitleChanged, ProficiencyExperienceTargetChanged, ProficiencyTitleChanged, RecipeAdded, RecipeRemoved |
| `Gameplay.Attributes.ChampionAttributes` | FervorChanged 5, StanceChanged 4 | — |
| `Gameplay.Attributes.HunterAttributes` | FocusChanged 5, StanceChanged 4 | — |
| `Gameplay.Attributes.RuneKeeperAttributes` | AttunementChanged 4 | IsChargedChanged |
| `Gameplay.Attributes.WardenAttributes` | GambitChanged 3, StanceChanged 2, MaxGambitCountChanged 1 | — |
| `Gameplay.Attributes.MarinerAttributes` | BalanceChanged 3 | — |
| `Gameplay.Attributes.GuardianAttributes` | StanceChanged 2, IsBlockTier3AvailableChanged 1, IsParryTier3AvailableChanged 1 | IsBlockTier1AvailableChanged, IsBlockTier2AvailableChanged, IsParryTier1AvailableChanged, IsParryTier2AvailableChanged |
| `Gameplay.Attributes.MinstrelAttributes` | StanceChanged 2 | IsSerenadeTier1Changed, IsSerenadeTier2Changed, IsSerenadeTier3Changed |
| `Gameplay.CombatMount` | — | BaseMaxMoraleChanged, BaseMaxPowerChanged, FuryChanged, MaxMoraleChanged, MaxPowerChanged, MaxTemporaryMoraleChanged, MaxTemporaryPowerChanged, MoraleChanged, PowerChanged, TemporaryMoraleChanged, TemporaryPowerChanged |
| `Gameplay.Attributes.CaptainAttributes` | — | IsInEnemyDefeatResponseChanged, IsInFellowDefeatResponseChanged, IsReadiedTier1Changed, IsReadiedTier2Changed |
| `Gameplay.BankItem` | ChestChanged 1, QuantityChanged 1 | — |
| `UI.Lotro.EquipmentSlot` | DragDrop 1 | EquipmentSlotChanged |
| `Gameplay.Attributes.BurglarAttributes` | — | IsCriticalTier1Changed, IsCriticalTier2Changed, StanceChanged |
| `Gameplay.Attributes.TrollAttributes` | — | RampageChanged |
| `Gameplay.BasicMount` | — | MaxMoraleChanged, MoraleChanged |

<a id="5-5"></a>
### 5.5 Enumerations

| Enumeration | Values registered | Values referenced | Most referenced (distributions) |
|---|---|---|---|
| `UI.ContentAlignment` | 10 | 9 | MiddleCenter 122, MiddleLeft 103, MiddleRight 66, TopLeft 33, TopCenter 32, BottomCenter 20, BottomLeft 20, TopRight 19 |
| `DataScope` | 3 | 3 | Character 110, Account 57, Server 40 |
| `UI.Orientation` | 2 | 2 | Vertical 97, Horizontal 75 |
| `UI.MouseButton` | 3 | 2 | Left 86, Right 65 |
| `UI.Lotro.Font` | 49 | 48 | Verdana12 81, TrajanPro14 77, Verdana14 71, Verdana16 67, TrajanPro16 53, TrajanPro18 47, TrajanPro15 43, Verdana10 42 |
| `UI.FontStyle` | 2 | 2 | Outline 81, None 32 |
| `UI.BlendMode` | 9 | 9 | Overlay 73, AlphaBlend 57, Color 20, Multiply 15, Screen 15, None 11, Grayscale 10, Normal 10 |
| `UI.Lotro.Action` | 270 | 55 | Escape 67, UI_Toggle 10, ToggleBag1 4, ToggleBag2 4, ToggleBag3 4, ToggleBag4 4, ToggleBag5 4, ToggleBags 4 |
| `Language` | 5 | 4 | German 58, French 56, English 40, EnglishGB 18 |
| `UI.Lotro.ShortcutType` | 8 | 8 | Alias 53, Skill 23, Item 22, Undefined 12, Emote 1, Hobby 1, Pet 1, Undef 1 |
| `UI.Color` | 141 | 91 | Black 51, White 49, Red 33, Yellow 28, Gold 23, Blue 20, Lime 19, Gray 14 |
| `ChatType` | 34 | 34 | Standard 35, Advancement 19, PlayerCombat 18, Quest 14, Death 11, EnemyCombat 10, Say 10, Fellowship 8 |
| `Gameplay.Class` | 22 | 21 | Hunter 23, Minstrel 22, Champion 20, Warden 20, Beorning 19, LoreMaster 19, RuneKeeper 19, Burglar 18 |
| `Gameplay.Attributes.MinstrelStance` | 3 | 3 | Harmony 10, None 10, WarSpeech 2 |
| `Gameplay.Equipment` | 24 | 23 | CraftTool 9, Class 8, Pocket 8, PrimaryWeapon 8, RangedWeapon 8, SecondaryWeapon 8, Back 7, Boots 7 |
| `Gameplay.Race` | 9 | 8 | Beorning 8, Dwarf 8, Elf 8, Hobbit 8, Man 8, HighElf 4, RiverHobbit 2, StoutAxe 2 |
| `Gameplay.EffectCategory` | 11 | 11 | Fear 8, Poison 8, Wound 8, Disease 7, Corruption 5, Cry 4, Dispellable 4, Elemental 4 |
| `UI.Lotro.LotroUIElement` | 10 | 9 | Vitals 6, Party 4, Backpack1 2, Backpack2 2, Backpack3 2, Backpack4 2, Backpack5 2, Backpack6 2 |
| `Gameplay.Alignment` | 3 | 2 | FreePeople 5, MonsterPlayer 4 |
| `UI.EdgeAttachmentType` | 7 | 4 | Same 4, Opposite 3, None 1, Proportional 1 |
| `Gameplay.ItemCategory` | 187 | 187 | Trophy 4, Component 3, CraftingTrophy 3, Ingredient 3, Misc 3, OptionalIngredient 3, Potion 3, Reputation 3 |
| `Gameplay.Attributes.HunterStance` | 4 | 4 | Endurance 3, Precision 3, Strength 3, None 1 |
| `Gameplay.Profession` | 11 | 10 | Cook 3, Farmer 2, Forester 2, Jeweller 2, Metalsmith 2, Prospector 2, Scholar 2, Tailor 2 |
| `Gameplay.Attributes.WardenStance` | 3 | 2 | Assailment 2, Determination 2 |
| `Gameplay.CraftTier` | 10 | 9 | Apprentice 2, Artisan 2, Eastemnet 2, Expert 2, Journeyman 2, Master 2, Supreme 2, Westfold 2 |
| `UI.HorizontalLayout` | 2 | 1 | LeftToRight 2 |
| `UI.Lotro.UIScalingGroupType` | 57 | 2 | OpponentVitals 1, Vitals 1 |
| `Gameplay.ItemWearState` | 5 | 5 | Broken 1, Damaged 1, Pristine 1, Undefined 1, Worn 1 |
| `Gameplay.Attributes.ChampionStance` | 1 | 1 | None 1 |
| `Gameplay.ReadyState` | 3 | 2 | NotReady 1, Ready 1 |
| `Gameplay.ItemQuality` | 6 | 5 | Common 1, Incomparable 1, Legendary 1, Rare 1, Uncommon 1 |

Stretch-mode arguments are plain numbers, not an enumeration: see §6.8.

<a id="5-6"></a>
### 5.6 Event-argument fields

Reads of a field with one of the LPA §9.3 argument names (static, closure scope, any table, so
generic names are upper bounds): `X` 111, `Y` 110, `Action` 107, `Button` 104, `Message` 86,
`ChatType` 63, `Index` 51, `Item` 19, `Value` 19, `DragDropInfo` 12, `Direction` 11,
`OldIndex` 9, `Cancel` 9, `Control` 8, `Sender` 7, `Skill` 7, `NewIndex` 7, `Player` 5, `Shift` 5,
`Alt` 4.

<a id="5-7"></a>
### 5.7 Names this build does not register

Plugins reference names under `Turbine` that the current client does not register (LPA §16,
Appendix A). In the original such a read yields nil. Most of them are **written** by the plugin
first (it defines the missing value or function itself, §6.3).

| Name | Distributions referencing (closure scope) | of which write it |
|---|---|---|
| `Language.Russian` | 26 | 10 |
| `Gameplay.EffectCategory.Tactical` | 15 | 15 |
| `UI.Lotro.Action.ToggleHUD` | 14 | 14 |
| `UI.Lotro.Action.EnterKey` | 13 | 13 |
| `UI.Lotro.Font.TimesRoman12`, `…20`, `…22`, `…24` | 7–8 each | all |
| `UI.ContentAlignment.LeftCenter` | 7 | 0 |
| `UI.Lotro.Action.RotateCharacter` | 8 | 8 |
| `UI.Control.SetToolTip`, `GetToolTip` | 7 | 7 (defined as functions on the class table) |
| `Gameplay.Attributes.MinstrelStance.Melody`, `Resonance`, `Dissonance`, `Warspeech` | 8 | 8 |
| `ChatType.EventBroadcast` | 8 | 8 |
| `UI.Lotro.Font.TrajanPro12` | 5 | 0 |
| `UI.Control.SetCloseEsc`, `GetCloseEsc`, `SetHideF12`, `GetHideF12` | 5 | 5 (functions) |
| `Language.Portuguese` | 5 | 5 |
| `Gameplay.SkillInfo.GetShortcutData` | 4 | 4 |
| `UI.Control.AutoSize` and the same on `Window`, `Lotro.Window`, `Label` | 2–4 | all (functions) |
| `UI.Color.Grey`, `UI.ContentAlignment.Left`, `Gameplay.ReadyState.Unknown` | 3 each | 0, 0, 3 |
| `UI.Lotro.Font.Verdana13` | 3 | 0 |
| `UI.Lotro.Font.BookAntiquaBold16`, `Verdana15` | 2 each | 0 |
| `UI.ComboBox` (name exists, class not registered, LPA §16) | 0 | — |

A name read but never written (for example `ContentAlignment.LeftCenter`, `Font.TrajanPro12`)
reaches the API as nil; LPA defines what each setter does with nil.

---

<a id="6"></a>
## 6. Behaviours plugins rely on

Each item is a count of distributions plus the host behaviour it needs, with the LPA section
that specifies that behaviour. Scope labels follow §3.3: whole-distribution unless marked, the
closure-scope value in parentheses where it differs (recount U11).

<a id="6-1"></a>
### 6.1 Attaching handlers

| Form | Distributions | Host requirement |
|---|---|---|
| Method definition on a Lua subclass (`function MyWindow:MouseClick(...)`) or field assignment on an instance | the usual form; it is included in the §5.4 counts (closure scope: `MouseDown` 124, `Update` 118, `KeyDown` 107, `Closing` 59) | normal-indexing lookup of the handler (LPA §9.2 step 2) |
| Class-level `Turbine.Plugin.Load` / `Unload` assignment with `=` | 13 / 30 (both scopes); a further 1 / 3 define them with `function Turbine.Plugin.Load(…)` | handlers stored on the class table apply to every plugin object of the apartment (LPA §9.1) |
| Instance `plugin.Load` / `Unload` / `GetOptionsPanel` assignment with `=` | 3 / 14 / 43 (both scopes); a further 2 / 1 / 3 use the `function plugin:…` definition form | the `plugin` global, set while the plugin loads (LPA §6.2) |
| `Plugins["<name>"].Load`/`Unload`/`GetOptionsPanel` assignment | 51 (closure 49) in the bracket form, 11 (both scopes) in the dotted form | the `Plugins` table keyed by descriptor `Name` (LPA §6.2) |
| `Turbine.Chat.Received = …` or `function Turbine.Chat.Received(…)` | 16 / 10 (both scopes) | shared singleton, last writer wins (LPA §9.5) |
| A plugin-defined helper that keeps several handlers per event, called with the event name as a string (`AddCallback`-style; counted as a function definition whose name starts with "add" and contains "callback", case-insensitive) | 56 (closure 55) define such a helper, 41 (closure 40) also a matching remove helper; 32 (both scopes) call a function whose name contains "callback" with `Turbine.Chat` as its first argument | handler tables (LPA §9.2 step 4) |

<a id="6-2"></a>
### 6.2 The `Plugins` and `_G` globals

Counted as a global name (not preceded by `.` or `:`): `Plugins[…]` indexing appears in 73
distributions (closure 71) and the name `Plugins` in 88 (closure 86); `_G` in 98 (closure 95),
with `_G[…]` indexing in 3 (both scopes). `getfenv` is used in 61 (closure 56) and `setfenv` in
11 (both scopes). The `plugin` global is set only
while a plugin loads (LPA §6.2), so the `plugin.…` assignments of §6.1 happen at load time.

<a id="6-3"></a>
### 6.3 Writes into native tables

- 83 distributions (whole-distribution scope, as every count of this list) assign to a field
  of a `Turbine…` table or define a function on one (72 assignments, 38 definitions of a
  function at a dotted `Turbine` path). Besides handlers (§6.1) this
  covers: adding values to enumerations that this build lacks or that older builds lacked
  (`Class.Beorning`, `Class.Brawler`, `Class.Mariner`, `CraftTier.*`, `Action.*`, §5.7);
  adding methods to native classes (`Control.SetToolTip`, `AutoSize`, `Center…`); and
  **replacing** native functions with wrappers: `Engine.GetLanguage` (8; one of the 8 is an
  editor type-stub file that the plugin never imports; what the other 7 call, and when, is in
  §7.4), `PluginData.Save` and `Load` (7 each), `Label.SetMultiline` and `IsMultiline` (5),
  `ListBox.SetOrientation` and `SetMaxItemsPerLine` (4), `Control.SetBackground` (2).
- 12 distributions overwrite a registered enumeration value: `MinstrelStance.None` and
  `Harmony` in 8, `Action` values in 2, `Class` values in 1 and `CraftTier` values in 1.
- Host requirement: namespace, class and enumeration tables are ordinary writable tables of the
  apartment (LPA §8.1, §8.4); a function stored on a class table is found by instances of that
  class and its subclasses, before the native method of the same name. Writes stay inside the
  apartment (LPA §6.1).

> **OPEN:** whether the original lets a plugin replace a native function on a class table so
> that **instances** see the replacement (a wrapper on `Control.SetBackground` relies on this).
> LPA §8.1 makes class tables plain tables, which implies yes; not verified by observation.

<a id="6-4"></a>
### 6.4 Standard library and globals

Comments removed; scope labels as in §3.3:

| Use | Distributions | Note |
|---|---|---|
| `loadstring(` | 34 (both scopes) | provided (LPA §3.2) |
| `collectgarbage(` | 13 (both scopes) | provided |
| `print(` | 19 (both scopes) | output goes to the script log, never to chat (LPA §3.2) |
| `table.getn` | 64 (closure 63) | Lua 5.0 compatibility function, present in stock 5.1 |
| `math.mod` | 18 (both scopes) | as above |
| `unpack` | 52 (closure 49) | base function in 5.1 |
| Calls of `require` in any form | 6 (both scopes): 2 call it directly with a string argument and no parentheses (bundled bit-manipulation library files); 4 pass it as a value to a protected call (`pcall`). None uses the parenthesised form with a direct call. | nil in the original (LPA §3.2): a direct call raises when its file runs; a protected call returns false instead of raising. The 2 direct callers are outside the first surface (§7.2). |
| `io.<member>` | 4 (both scopes) | nil in the original |
| `os.<member>` | 2 (both scopes) | nil in the original |
| `debug.<member>` | 2 (both scopes) | nil in the original |
| `coroutine.<member>` | 0 | — |
| `dofile(`, `loadfile(`, `string.dump`, `gcinfo(`, `string.gfind`, `table.setn` | 0 each | — |
| Functions added to the `string` table (`trim`, `split`, …; names the stock 5.1 library does not have) | 12 | the string table is shared by every string's metatable in the apartment |
| Functions added to `math` / `table` (same rule) | 7 / 3 | — |

<a id="6-5"></a>
### 6.5 The class helper and the 2010 sample package

Whole-distribution scope (the counts of this section were taken over every `.lua` file of a
distribution, U11):

- 121 distributions call a global `class(…)` helper to subclass native classes (LPA §8.3).
  90 define it themselves (in their own package or a bundled copy), 11 bundle a copy of the
  publisher's 2010 sample `Turbine` utility folder (they overlap: a distribution may do both).
- **37 distributions depend on the sample package being installed by the user** as
  `Plugins/Turbine/` (LPA §4.2): 31 call `class` without defining or bundling it, and 28 import
  sub-packages of it without bundling it. Distributions importing each sub-package, bundled or
  not: `Turbine.UI.Extensions` 27, `Turbine.Debug` 7, `Turbine.Type` 5, `Turbine.Class` 4,
  `Turbine.Animation` 4, `Turbine.Utils` 3 (test-only imports of `Turbine.Testing` are left
  out). 22 of them are otherwise inside the first surface (§7.2).
- LPA §4.2 says Tessera must not ship that package.

> **OPEN (owner decision):** whether Tessera provides its own clean-room helper package that
> offers the same `class` function and the sub-package names, or leaves the 37 distributions to
> the user's own copy of the sample package. Until decided, exit-criterion-4 candidates must
> not depend on it (§9.1 rule 3).

<a id="6-6"></a>
### 6.6 Import patterns

- Every distribution imports by string literal; 32 (closure 31) also build an import name at
  run time, and 35 (closure 34) when `import` passed as a value is counted too (the trigger of
  §3.3 rule 3).
- Imports placed inside functions and wrapped in a protected call are used for optional
  dependencies (for example an optional import of another author's map package, and an import
  done only when the user asks for an update table). A failing `import` must therefore raise a
  catchable Lua error (LPA §7.1), never abort the apartment.
- 16 distributions (one descriptor each; closure scope, since only reached imports are
  resolved) import at least one package that no file in the distribution provides: in 9 it is a separately published UI helper library by another author
  (§6.11); in 2 the import is optional (protected call, or on demand); in 1 it is test-only; in
  4 the missing package is in the plugin's own or another author's namespace and was not
  classified further.

<a id="6-7"></a>
### 6.7 Plugin-data call forms

At least 65 (closure 64) distributions call `PluginData.Load` with two arguments (synchronous
form) and at least 41 (closure 40) with three (with a completion handler); at least 69 (both
scopes) call `PluginData.Save` with three arguments and at least 81 (closure 80) with four (with
a completion handler). (Counted only where the call is written `PluginData.Load(` /
`PluginData.Save(`; aliased calls are not counted.) Scope values referenced as
`Turbine.DataScope.<name>`: `Character` 110 (both scopes), `Account` 57 (both scopes), `Server`
41 (closure 40); the closure values equal §5.5. Both forms and all three scopes are LPA §10.1.

> **OPEN:** one distribution in the §9 set (Songbook) reads an account-scope key that a
> helper program shipped in the same archive writes outside the game, into the original
> client's `PluginData/` tree. Tessera stores plugin data in its own directory (LPA §10.7).
> Whether Tessera should fall back to reading the original's `.plugindata` files read-only when
> its own store has no value for a key is not decided.

<a id="6-8"></a>
### 6.8 Images and backgrounds

- `SetBackground` with a numeric literal that is an image DID (`0x41xxxxxx`): 73 (closure 72)
  distributions. These need the install's render-surface records (LPA §13.8), not the server.
- Image paths written as string literals: `.tga` in 115 (closure 114) distributions, `.jpg` in
  31 (closure 28), `.png` in 1 (both scopes). Files shipped (a property of the archive, so
  whole distribution by nature): `.tga` in 127 distributions, `.jpg` in 45, `.png` in 6, `.bmp`
  in 4.
- `SetBackground(nil)` (clear): 20 (both scopes).
- `SetStretchMode` with a single numeric literal argument: 1 in 58 (closure 57) distributions,
  2 in 56 (closure 50), 0 in 22 (both scopes), 3 in 13 (both scopes), 4 in none. LPA §17.3
  puts stretch modes 1–4 in tier 3; by use, modes 1 and 2 belong in the first surface (§7.3).
  How Tessera draws a control with a non-zero stretch mode (its own surface, composed as a
  separate entry with its own opacity only and clipped by its enclosing windows; mode 2
  repeating the base image over the control's box) is decision 0015 (Decision 1.1, 1.7,
  §2.1). A control at mode 0 (the value the 22 distributions above set explicitly) gets no
  surface of its own and is drawn into the nearest surface above it; only a top-level window
  at mode 0 is drawn through a stand-in surface (0015 Decision 1.1).

> **OPEN:** whether the original accepts `.png` files in `SetBackground` (LPA §13.8 names JPG
> and TGA). One sampled distribution references a `.png` path. The same is unknown for
> `.bmp` (shipped by 4 distributions). **Tessera rule until
> O6 is answered** (decision 0015 §4.3): `.png` and `.bmp` files are not decoded, and
> `SetBackground` with such a path raises the unsupported-format error of LPA §13.8. The
> call-time checks behind that error (an index of the whole `Plugins/` tree for existence,
> a header read of at most 64 bytes for the format) are decision 0015 §4.4.

<a id="6-9"></a>
### 6.9 Text markup

`Label:SetMarkupEnabled` is used by 40 (closure 38) distributions, but colour-tag strings
(`<rgb=…>`) appear in 91 (closure 89) and underline tags in 13 (both scopes). 45 (closure 44)
distributions put a colour tag in a string literal of the first argument of a `WriteLine` call,
31 (both scopes) of them without ever enabling label markup: plugins expect `Shell.WriteLine` output in chat to be
rendered with the markup of LPA §13.4.

> **OPEN:** LPA §11.1 does not state whether the original renders markup in `WriteLine` text.
> The plugin usage above says it does; confirm in LPA (BIN or M3 observation).

<a id="6-10"></a>
### 6.10 Several descriptors, reloaders and shared apartments

- 28 distributions ship several descriptors. 31 descriptors have names that mark them as
  reloader or unloader helpers, which unload a plugin's apartment and load it again (these two
  counts are about descriptors and do not depend on the code scope).
  `PluginManager.UnloadScriptState` is called in 29 (closure 28) distributions and
  `PluginManager.LoadPlugin` in 33 (closure 31).
- Apartment names shared by **different** distributions: one author's six reference plugins
  declare one apartment; four forks of one toolbar plugin share three apartment names (main,
  reloader, unloader); seven more names occur in 2 distributions each (four times two
  versions or forks of one plugin, once a bundled copy of another plugin, once two plugins of
  one author, once two unrelated plugins). The host must load a second plugin into an existing
  apartment (LPA §6.2) and keep shared singletons shared (§6.1).

<a id="6-11"></a>
### 6.11 Dependencies on other distributions

9 distributions (closure scope: imports reached from the main packages) import a UI helper
library (drag bars and draggable windows) that is published separately by another author and is
not bundled; they fail to load without it. One
distribution optionally uses another author's map package (§6.6). Because plugin paths are
relative to `Plugins/` rather than to the calling plugin's folder (LPA §4.4), Tessera checks
image paths against an index of the whole `Plugins/` tree, not of the calling distribution's
folder (decision 0015 §4.4).

<a id="6-12"></a>
### 6.12 Size

Three of the most downloaded distributions keep large databases as Lua tables: 32.5 MB,
10.1 MB and 6.5 MB of Lua source (whole distribution). The original sets no memory or time
limit on plugins (LPA §3.3); Tessera's limits (M5 exit criterion 5) must still let these load,
which is why they are in the §9 set.

**Measured load cost (U12).** Each apartment of the §9.2 set was loaded as the host would load
it (`import` of the main package as one host call, then the `Load` event, queued plugin-data
callbacks and 1,000 ticks of `Update` delivery each followed by the per-tick collector step of
`plugin-runtime.md` §9.2), with stock Lua 5.1.5 built for 64-bit and a stub host API. Bytes are
what the Lua state requested from its allocator (the quantity the per-apartment cap counts,
`plugin-runtime.md` §9.1); they include about 0.3 MiB for the stub API tables and exclude
allocator overhead and native objects. Instructions are Lua VM instructions of one host call,
counted by a count hook (± 10), including the few the stub API spent.

| Apartment (§9.2 #) | Largest host call (VM instructions) | Peak bytes | Live bytes after a full collection |
|---|---|---|---|
| `Compendium` (#2), its four databases imported by the main package at load | 4.0 M (the plugin load) | **160.2 MiB** | 125.1 MiB |
| `IRV` (#14) | 4.2 M (the plugin load) | **75.2 MiB** | 60.6 MiB |
| `TMAnnot` (#1 helper), with every one of the 1,950 annotation cells requested at once (worst case; the main plugin normally requests a few) | 12.8 M (the plugin load) | 18.9 MiB | 9.1 MiB |
| `TerrainMap` (#1) | 0.7 M (the plugin load) | 12.8 MiB | 7.5 MiB |
| `TMSearch` (#1 helper), searching all 398 map data files, one per `Update` | 0.45 M (one `Update`) | 10.2 MiB | 7.7 MiB |
| `Vinny` (#6 and #7 together) | 0.15 M | 4.3 MiB | 3.2 MiB |
| `Default` (#3, #4, #10, the five descriptors of #11, #12, #13 together) | 0.25 M | 4.1 MiB | 2.9 MiB |
| `LootLogs` (#5) | 0.29 M | 2.1 MiB | 1.6 MiB |
| `RTPlugin` (#9), `StableGuy` and `Waypoint` (#8), `CompendiumReloader` (#2), `TMCust` (#1 helper) | ≤ 0.12 M | ≤ 1.1 MiB each | ≤ 1.0 MiB each |

Findings:

1. The per-host-call instruction budget of LPA §3.3 (500 million) fits every call with a
   margin of at least 39 times.
2. The per-apartment memory default of LPA §3.3 (64 MiB) does **not** fit: #2 is refused
   during its load at 64 MiB (and still at 159 MiB), and #14 at 64 MiB (and still at 75 MiB).
   Stock 5.1 has no emergency collection, so a cap must cover the peak including garbage not yet
   collected, not only the live bytes. `plugin-runtime.md` §9.1 therefore sets the default to
   512 MiB (its §9.4 gives the derivation); both distributions load under it.
3. Compilation is not counted by the instruction budget: loading #2 compiles 33.7 MB of source
   and takes 1.2–1.7 s of wall time on a 2020 laptop CPU while executing only about 4 M VM
   instructions. Compiling needs memory too: the parser held 2.2 to 7.6 times the file size at
   its peak for the 10 largest data files of #2, #1 and #14.
4. Each of those 10 files, compiled and run alone, keeps 1.3 to 6.8 times its source size live
   (the ratio depends on how many distinct strings and nested tables it builds); the whole of
   #2 keeps 131 MB live from 34 MB of Lua source.

---

<a id="7"></a>
## 7. The first API surface

<a id="7-1"></a>
### 7.1 Definition

The **first API surface** is:

1. every class, static and enumeration of LPA tier 1 (§17.1; 49 names), with the behaviour LPA
   gives them; plus
2. these LPA tier-2 and tier-3 names, promoted because host code alone can serve them:
   `Turbine.UI.Lotro.Quickslot`, `Turbine.UI.Lotro.Shortcut`, `Turbine.UI.Lotro.ShortcutType`,
   `Turbine.UI.Lotro.DragDropInfo`, `Turbine.UI.DragDropInfo`, `Turbine.UI.ContextMenu`,
   `Turbine.UI.MenuItem`, `Turbine.UI.MenuItemList`, `Turbine.UI.TreeView`,
   `Turbine.UI.TreeNode`, `Turbine.UI.TreeNodeList`, and the enumeration tables
   `Turbine.UI.EdgeAttachmentType` and `Turbine.UI.Lotro.UIScalingGroupType` (values only);
3. the behaviour items of §7.3.

62 names in all. The first surface is a **build order inside** the tiers of LPA §17, not a
different surface: every promoted name of item 2 is an LPA §16 name of tier 2 or 3, and no name
outside LPA §16 is added. It therefore stays within `docs/LEGAL.md` §17.2 rule 5 ("only what
plugins need", built in the LPA tiers); it only moves 13 names and 8 behaviour items earlier
because the census shows plugins need them early. A shortcut of type `Alias` runs its text as a chat command through Tessera's own
command handling; other shortcut types are accepted, stored, returned and drawn, and executing
them needs game state (§8).

<a id="7-2"></a>
### 7.2 Coverage

A distribution is **inside** a surface when every class it uses (§3.4 rule 4) is in the
surface.

| Surface | Names | Distributions inside | of which need the sample package (§6.5) | Share of LIA downloads inside |
|---|---|---|---|---|
| LPA tier 1 only | 49 | 13 | 6 | 3.5 % |
| First surface (§7.1) | 62 | **56** | 22 | 25.8 % |
| All registered names | ~150 | 143 | 37 | 100 % |

The promotions matter because `Quickslot`/`Shortcut` (85–87 distributions; 53 use the `Alias`
type, which is how a plugin offers a chat command to the user: `Shell` has no function that
runs one, LPA §16) and `ContextMenu`/`MenuItem` (67–69) are
used far more than most tier-1 names.

Classes that keep the other 87 distributions outside, by number of distributions: `Item` 43,
`Backpack` 41, `Attributes` 35, `Effect` 31, `ItemInfo` 29, `EffectList` 29, `Equipment` 26,
`Skill`/`SkillInfo`/`SkillList` 22–23, `EffectCategory` 22, `FreePeopleAttributes` 19,
`ClassAttributes` 19, `WalletItem` 17, `ItemControl` 17, `Wallet` 16, `Bank` 16, `Vault` 15,
`Party`/`PartyMember` 15, `SharedStorage` 13, `ItemInfoControl` 13 — all game-server state
(§8).

<a id="7-3"></a>
### 7.3 Behaviour items to bring into the first surface

LPA §17 places these later; by use they are needed early:

| Item | Use | LPA |
|---|---|---|
| Stretch modes 1, 2 and 3 | 58 (closure 57) / 56 (closure 50) / 13 (both scopes) distributions, §6.8 | §13.8 (tier 3) |
| Writable class and enumeration tables with lookup of plugin-added functions by instances | 83 distributions (whole distribution), §6.3 | §8.1, §8.4; §6.3 here |
| Handler tables and the `Plugins` table | 56 (closure 55) define a handler-table helper; 73 (closure 71) index `Plugins`, §6.1, §6.2 | §9.2, §6.2 |
| Markup in `Shell.WriteLine` output | 45 (closure 44); colour tags anywhere: 91 (closure 89), §6.9 | §11.1, §13.4; §6.9 OPEN |
| `PluginManager.LoadPlugin` and `UnloadScriptState` from plugin code | 33 (closure 31) / 29 (closure 28), §6.10 | §6.2, §6.3 |
| `Window.SetRotation` | 14 (closure scope, §5.3) | §13.3 (tier 3) |
| `Window.SetScale`, `RegisterForGlobalScaling`, `UnregisterForGlobalScaling`, `UIScaleChanged` | 6–10 (closure scope, §5.3, §5.4) | §13.3 (tier 3); a scale of 1 and no events are acceptable at first |
| `Plugin.GetUsesGlobalScaling` | 8 (closure scope, §5.3) | §16, §17.3 (tier 3) |

Where Tessera's behaviour for these items is decided outside LPA: the drawing of stretch
modes, `Window.SetRotation` and `Window.SetScale` (surfaces, nested entries, rotation about the
surface's centre, scaled composition) is decision 0015 (Decision 1.1, 1.6, 1.7, §2.1–§2.3);
the timing of `LoadPlugin` and `UnloadScriptState` work in the frame is decision 0014 D8
(`plugin-runtime.md` §8.4).

Members of first-surface classes used by **no** distribution (§5.3, "used by none") may stay
stubs at first: for example `Control.GetEdgeAttachments`, `GetMaximum*`, `GetMinimumSize`,
`GetBlendMode`, `GetBackColorBlendMode`; `Label.DeselectAll`; `ListBox` flipped-layout, reverse
fill, cell-alignment and wrap-indent getters; `Window` scaling-origin and scaling-group getters;
`Display.GetUIGroupScale`; `Engine.GetScriptVersion`; `TreeView.GetFilter`/`GetSortMethod`;
`Quickslot.IsUseOnRightClick`; `CheckBox.GetCheckAlignment`; `UI.DragDropInfo.IsSuccessful`.

<a id="7-4"></a>
### 7.4 Outside the surface

LPA §17.1 already requires every other registered class to exist as a table whose methods return
nil or defaults. For the 87 distributions outside the surface this is not enough for loading:
their code chains calls on returned objects (`GetBackpack():GetSize()`,
`GetEffects():GetCount()`), so a nil return raises at load time.

Tessera rule (proposal for the plugin host, not an observed original behaviour): under a backend
that has no data for a gameplay view, the getters of §8 class **N** return a valid, **empty**
object of the right class instead of nil: a backpack, vault, shared storage and wallet with
size and count 0, an effect list and the untrained-skill list with count 0, a trained-skill
list with **at least one** skill (§7.4.1), attributes whose numeric getters return 0, an
equipment object whose slots are empty. `GetTarget()` returns nil (LPA §15: an `Entity` or
nil); `GetParty()`, `GetMount()` and `GetPet()` return nil. No events fire on empty views.

> **OPEN:** what the original returns from these getters before the data has arrived (for
> example right after login) — nil or an empty object. Observation with a small owner-written
> plugin in the original client (roadmap M5 RE task) would settle it.

<a id="7-4-1"></a>
#### 7.4.1 Load-time language detection

8 distributions (whole-distribution scope) replace `Turbine.Engine.GetLanguage` with a function
of their own (§6.3). In one of them the replacement is in an editor type-stub file that the
plugin never imports, so it has no run-time effect. In the other 7 the replacement calls the
saved native function and then, to detect a client language that the native function does not
report, reads the name of a game object:

| Group | Distributions | Chain the replacement follows | When it tests | Name test (§7.4.2) |
|---|---|---|---|---|
| A1 | 1 (LIX 1199) | `LocalPlayer.GetInstance` → `GetTrainedSkills` → `GetItem(1)` → `GetSkillInfo` → `GetName` | on every call | **P**: the result is `Language.Russian` (a value the plugin itself adds, §5.7) when test P matches, otherwise the native value |
| A2 | 1 (LIX 901) | the same chain | on every call | **S**: `Language.Russian` when test S matches, otherwise the native value |
| B | 4 | the same chain | only when the native function returns `Language.English` | **P**, as A1 |
| C | 1 | constructs an `Item`-type `Shortcut` from a fixed data string, then `GetItem` → `GetItemInfo` → `GetName`, and maps the name to a language by exact comparison with fixed strings; a name it does not know gives `Language.English` | on every call | exact string equality only |

6 of the 7 call the replacement **while loading** (top-level code of an imported file: the
locale set-up, or a language-dependent data table); one distribution of group B defines it but
none of its own files calls it. All 7 are outside the first surface (they use `SkillList`,
`Skill` and `SkillInfo`, and group C also `Item` and `ItemInfo`). Under an install whose native
language is English, all 6 follow the chain at load; if any step returns nil, the next call
raises and the plugin fails to load. With the empty views above, an empty trained-skill list
would make `GetItem(1)` fail (an out-of-range index) and the 5 load-time callers of groups A
and B would not load.

<a id="7-4-2"></a>
#### 7.4.2 The two name tests

Both tests run in the plugin's own Lua code, so they follow the Lua 5.1 pattern rules in the
C locale (`plugin-runtime.md` §4.3: bytes 0x80–0xFF belong to no character class and are
compared as plain bytes). Let *N* be the name the chain returns.

**Test P (groups A1 and B).** Take the first two bytes of *N* (all of *N* if it is shorter).
Use them, unescaped, as a Lua **pattern**, and search for it anywhere in a fixed string *R*
held by the plugin: the 66 two-byte UTF-8 encodings of the Russian capital and small letters
U+0410–U+044F, U+0401 and U+0451 (132 bytes; every byte of *R* is 0xD0, 0xD1 or in
0x80–0xBF, so *R* has no byte below 0x80; *R* begins with the byte 0xD0, and the order of the
letters changes none of the outcomes below). The test matches when the search finds something,
**including an empty match**.

Outcomes of P by the first two bytes *b1 b2* of *N* (enumerated over every *b1* from 0x00 to
0xFF and every *b2*, 2026-10-06; a 0x00 byte ends a Lua 5.1 pattern, so *b1 b2* with *b2* =
0x00 behaves as *b1* alone, and *b1* = 0x00 as the empty pattern):

| *b1 b2* | Outcome of P |
|---|---|
| *N* empty, or *b1* = 0x00 | the empty pattern matches: **Russian** |
| a two-byte UTF-8 letter of *R* (D0 81, D0 90–BF, D1 80–8F, D1 91) | match: Russian (the intended case) |
| any other non-ASCII UTF-8 character (lead byte 0xC2–0xF4 followed by a continuation byte), for example an accented Latin letter or U+0400 | no match: native value |
| *b1* an ASCII byte other than `$ % ( ) . [ ^`, and *b2* absent or not one of `* - ?` | no match: native value |
| *b1* an ASCII byte other than `% ( ) [ ^` (or a byte of *R*), and *b2* one of `*`, `-`, `?` (zero repetitions match the empty string) | match: **Russian** |
| `$` alone; `.` alone or followed by `$`, `+`, `.` or a byte of *R* (0x80–0xBF, 0xD0, 0xD1); `^` alone or followed by `.` or 0xD0; `%` followed by one of `A C D L P S U W X Z` or by a byte of *R*; `()` | match: **Russian** |
| `%` alone or followed by a digit, `b` or `f`; `(` alone or followed by `$`, `%`, `(`, `.`, `[` or a byte of *R*; `)` or `[` alone or followed by any byte; `.` or `^` followed by `%`, `(`, `)` or `[` | malformed pattern or unfinished capture: the search **raises an error**, so the replacement raises and a load-time caller fails to load |
| `$`, `%`, `(`, `.` or `^` followed by any byte not listed in the rows above (for example `$x`, `%a`, `(x`, `.x`, `^A`) | no match: native value |

Names that are not valid UTF-8 at their start (a continuation byte first, or a lead byte followed by an ASCII byte) are not listed; none can come from a well-formed name.

**Test S (group A2).** Search *N* with a one-character set pattern whose bytes, read as a Lua
set, are {0x81, 0x8F} ∪ 0x90–0xD1 (the plugin wrote it as a range of Cyrillic letters, but
Lua reads the set byte by byte). The test matches when **any byte anywhere** in *N* is in that
set. It never matches a name of ASCII bytes only, and it matches every name that contains a
two-byte UTF-8 character with lead byte 0xC2–0xD1 (U+0080–U+047F: accented Latin letters,
Greek, Cyrillic and others) or any other character whose encoding has a byte in the set. It
cannot raise.

Tessera rule (proposal for the plugin host, not an observed original behaviour):

1. Under a backend without skill data, the sandbox character's trained-skill list holds at
   least one skill. `GetItem(1)` returns a `Skill`; its `GetSkillInfo()` returns a `SkillInfo`;
   that object's `GetName()` returns a **placeholder** chosen by Tessera that satisfies all of:
   - it is not empty;
   - every byte is an ASCII letter (0x41–0x5A, 0x61–0x7A), an ASCII digit (0x30–0x39) or a
     space (0x20), so no byte is 0x80 or above;
   - its first two bytes (its only byte, if it has one) are ASCII letters or digits.

   Under these conditions test P finds no match and raises nothing (checked over every one- and
   two-byte prefix the conditions allow), and test S finds no match. Groups A1, A2 and B
   therefore return the native language and load. The placeholder text is Tessera's own and is
   never a game string.
2. When a backend has a skill name read from the install (once a spec for skill records
   exists), Tessera passes it to the plugin unchanged and does **not** apply rule 1's
   conditions. The plugin's own test then runs on Tessera's C-locale Lua with exactly the same
   result as in the original for the same name: for example a name with an accented Latin
   letter makes group A2 report `Language.Russian`, and a name starting with `(` followed by
   `.` makes groups A1 and B raise. These are behaviours of the plugin, reproduced, not
   corrected.
3. `Shortcut.GetItem()` on an `Item`-type shortcut whose item the backend does not know returns
   an empty `Item` view whose `GetItemInfo()` returns an `ItemInfo` whose `GetName()` returns the
   empty string, instead of nil (group C then falls back to `Language.English`).

> **OPEN:** what the original's `Shortcut.GetItem()` returns for an `Item`-type shortcut whose
> data names an item that the character does not hold (nil, or an `Item` object). Group C
> depends on it at load. An owner-written test plugin in the original client (M3) settles it;
> if the answer is nil, rule 3 is a stated Tessera deviation.

---

<a id="8"></a>
## 8. Data sources under the sandbox backend

The sandbox backend (ARCHITECTURE §19.2) serves the local avatar in a static world. Mapping of
the API to where its values come from:

| Data class | Names | Source under the sandbox |
|---|---|---|
| **H** host only | `Turbine` core (`Object`, `Plugin`, `PluginData`, `PluginManager`, `Shell`, `ShellCommand`, `Engine`, `Chat`, `ChatType`, `DataScope`, `Language`), all of `Turbine.UI`, the `Turbine.UI.Lotro` windows, buttons, check boxes, text boxes, scroll bars, `Font`, `Action`, `LotroUI`, `LotroUIElement`, `UIScalingGroupType` | Tessera alone. Fonts and image DIDs come from the install. `Chat.Received` carries only lines Tessera itself produces (the user's own chat input and Tessera's system messages); Tessera delivers it to plugins in frame-loop step 7, not at the moment the line is shown (decision 0014 D8 and its Known difference 21, `plugin-runtime.md` §8.4). |
| **H\*** | `Quickslot`, `Shortcut`, `ShortcutType`, `Lotro.DragDropInfo` | Host for `Alias` shortcuts; `Item`, `Skill`, `Pet`, `Emote`, `Hobby` shortcuts can be held and drawn, executing them needs game state. |
| **S** sandbox | `LocalPlayer` (instance, `IsInCombat` = false), `Entity` (`GetName`, `IsLocalPlayer`), `Actor` (level, morale, power and their maximum and temporary values), `Player` (`GetClass`, `GetRace`, `GetAlignment`, `IsLinkDead` = false, voice flags false), enums `Class`, `Race`, `Alignment` | The sandbox's local character: a name, level, class, race and alignment chosen by the user or a test fixture, and constant vitals. |
| **N** server state | `Backpack`, `Item`, `ItemInfo`, `Equipment`, `Wallet`, `WalletItem`, `Bank`, `Vault`, `SharedStorage`, `BankItem`, `Effect`, `EffectList`, `EffectCategory`, `Party`, `PartyMember`, `Raid`, `Pet`, `Mount` and its subclasses, `Skill`, `ActiveSkill`, `SkillInfo`, `SkillList`, `UntrainedSkill`, gambit classes, `ProfessionInfo`, `Recipe`, `RecipeIngredient`, `Attributes` and every per-class or per-race attribute class and stance enum, `Actor.GetTarget` and `GetEffects`; the controls `ItemControl`, `ItemInfoControl`, `EffectDisplay`, `EntityControl`, `EquipmentSlot` | None. Empty views (§7.4). |

What the sandbox must provide for the first surface, by use: `LocalPlayer.GetInstance` (120
distributions), `GetName` (100), `GetClass` (74), `IsInCombat` (54), `GetLevel` (29), morale and
power getters (16–22), `GetAlignment` (21), `GetRace` (17); events `InCombatChanged` (34),
`LevelChanged` (16), `MoraleChanged` (13), `PowerChanged` (11) need never fire under the
sandbox.

---

<a id="9"></a>
## 9. Plugin set for M5 exit criterion 4

<a id="9-1"></a>
### 9.1 Selection rules

A distribution qualifies when, by this census:

1. every class it uses is in the first surface (§7.1), so no **N** class is touched;
2. it does not depend on an unbundled third-party library (§6.11);
3. it does not depend on the 2010 sample package (§6.5);
4. it opens at least one window.

Among the qualifying ones, the most downloaded were taken, then adjusted so that the set as a
whole covers: several descriptors in one distribution, a shared apartment, reloader helpers,
options panels, every first-surface promotion of §7.1, chat-fed plugins, game image DIDs, plugin
image files, the three plugin-data scopes, and the three large-database plugins (§6.12).

<a id="9-2"></a>
### 9.2 Primary set (14 distributions)

Identifiers: *Name* = descriptor `Name` (and the LIX title where it differs); *Author* =
descriptor `Author` (LIX uploader in brackets where it differs); *LIX id* = lotrointerface file
ID; downloads and update date from LIX on 2026-10-06.

| # | Name | Author | LIX id | Version | Downloads | Updated | Descriptors (apartment) |
|---|---|---|---|---|---|---|---|
| 1 | TerrainMap, TMAnnot, TMCust, TMSearch ("Terrain Map") | Garan | 846 | 1.41 | 679,632 | 2025-12-27 | 4 (each its own) |
| 2 | Compendium, CompendiumReloader ("LOTRO Compendium") | Lunarwater (lunarwtr) | 526 | 2.7 | 494,103 | 2026-01-06 | 2 (each its own) |
| 3 | Songbook | Chiran | 380 | 0.93 | 311,308 | 2021-04-20 | 1 (default) |
| 4 | Alerter ("LoTRO Alerts") | Garan | 609 | 1.08 | 182,100 | 2016-03-10 | 1 (default) |
| 5 | LootLogs | Souru | 1076 | 3.0.13 | 170,342 | 2026-07-27 | 1 (own) |
| 6 | TravelRef ("Travel locations reference") | David Down (Vinny) | 524 | 3.4 | 166,417 | 2026-10-05 | 1 (shared with #7) |
| 7 | LostLore ("Lost Lore reference") | David Down (Vinny) | 1172 | 3.8 | 102,118 | 2026-10-05 | 1 (shared with #6) |
| 8 | StableGuy, Waypoint ("StableGuy") | Radicus Hood; Lunarwater | 757 | v12.10; 1.5 | 152,662 | 2020-06-25 | 2 (each its own) |
| 9 | RTPlugin | Math (Mathwold) | 909 | 1.7.1 | 117,778 | 2015-08-27 | 1 (own) |
| 10 | Babble | Simbo | 929 | 2.1.5 | 92,418 | 2025-04-13 | 1 (default) |
| 11 | Horsey, Horsey2, Horsey3, Horsey4Other, Horsey4Pets ("Horsey!") | rushl | 376 | 3.2 | 91,660 | 2019-05-25 | 5 (default) |
| 12 | Compass | Garan | 946 | 1.1 | 46,850 | 2016-02-18 | 1 (default) |
| 13 | DelvingDigest ("Delving Digest") | Garan | 1237 | 1.4.1 | 57,939 | 2025-09-05 | 1 (default) |
| 14 | IRV ("IRV (Image Resource Viewer)") | Garan | 449 | 2.0 | 60,512 | 2015-02-26 | 1 (own) |

<a id="9-3"></a>
### 9.3 What each needs

All 14 use `Control`, a window class, `Shell` and `PluginData`. The table lists what goes beyond
LPA tier 1, and what the sandbox lacks.

| # | First-surface promotions used | Sandbox (S) values read | Other features exercised | What the sandbox cannot supply |
|---|---|---|---|---|
| 1 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type), TreeView, TreeNode(List) | Entity, LocalPlayer | 4 apartments; `LoadPlugin`/`UnloadScriptState`; `Update`; key events; label markup; stretch 1, 2; image DIDs; `.tga`, `.jpg` files; 6.5 MB of Lua | It listens to chat lines (types `Standard` and `Tell` referenced) and references the location chat command: the player position comes from chat output, which the sandbox does not produce (§12 O5) |
| 2 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type) | Actor, LocalPlayer | reloader; `Plugin.Load`; label markup; stretch 2; image DIDs; account and character scopes; 32.5 MB of Lua; an optional import in a protected call that fails | `Quest` chat lines; `Item` shortcuts cannot execute |
| 3 | Lotro.DragDropInfo, Quickslot, Shortcut(Type) | none | default apartment; mouse wheel; drag-leave; account and character scopes | Its song list is an account-scope plugin-data value written by an external helper (§6.7 OPEN); alias shortcuts run music commands, which Tessera does not have before M6 |
| 4 | UI and Lotro DragDropInfo, ContextMenu, MenuItem(List), Quickslot, Shortcut(Type), TreeView, TreeNode(List) | Entity, LocalPlayer (`IsInCombat`) | `LoadPlugin`/`UnloadScriptState`; stretch 1, 2, 3; image DIDs; 26 chat types | Alerts are triggered by chat lines of 26 types and by combat state; only the user's own lines exist |
| 5 | Quickslot, Shortcut(Type) | Actor, Class, Entity, LocalPlayer, Player | label markup; stretch 1; account and server scopes | Loot and combat chat lines |
| 6 | ContextMenu, MenuItem(List), Lotro.DragDropInfo, Quickslot, Shortcut(Type) | Actor, Entity, LocalPlayer, Player, Race | shared apartment with #7; options panel; `UIScaleChanged`; stretch 1, 2; image DIDs; character and server scopes | `Item` and `Skill` shortcuts cannot execute |
| 7 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type) | Actor, Entity, LocalPlayer, Player | shared apartment with #6; options panel; chat listener | none beyond chat |
| 8 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type) | none | two plugins by different authors in one archive; `Plugin.Load`; `Window.SetRotation`; stretch 2; image DIDs; its alias shortcuts run the shell command that the bundled second plugin registers, so alias execution must reach plugin-registered commands (LPA §11.2) | chat lines only |
| 9 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type) | Player | key-up events; `Update`; image DIDs; server scope | chat lines |
| 10 | ContextMenu, MenuItem(List) | Entity, LocalPlayer | options panel; label markup; stretch 1; key-up events | Chat-channel lines from other players |
| 11 | ContextMenu, MenuItem(List), Lotro.DragDropInfo, Quickslot, Shortcut | none | 5 descriptors in the default apartment; `GoldWindow` | Shortcuts the user drags in from game panels (the plugin checks their type) need game state to execute |
| 12 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type), TreeView, TreeNode(List) | Entity, LocalPlayer | computed imports (whole-distribution scope, §3.3); `Window.SetRotation` about the x and z axes; an `Alias` quickslot; a plugin-defined handler-table helper; `Plugin.Unload`; `Update`; image DIDs; `.tga` files chosen by client language; character scope | Like #1 it reads the player's heading from `Standard` chat lines produced by the location chat command that its alias runs; the sandbox does not produce them (§12 O5), so the heading never changes |
| 13 | ContextMenu, MenuItem(List), Quickslot, Shortcut(Type), TreeView, TreeNode(List) | Class, Entity, LocalPlayer, Player | all three scopes; label markup; image DIDs; `.tga`, `.jpg` files | `Standard` and `Tell` chat lines |
| 14 | ContextMenu, MenuItem(List), TreeView, TreeNode(List) | none | browses image DIDs of the install; label markup; stretch 1, 2; 10.1 MB of Lua; an import done only on demand | none |

All 14 load, create their windows and respond to input with host and sandbox data only, by this
census; their data-driven features stay empty under the sandbox. None of the 14 replaces
`Engine.GetLanguage` (§7.4.1) or reaches a class-**N** object in its closure-scope code.

**Input under Tessera's input bridge** (decision 0014; Tessera rules, not census facts). What
"respond to input" means in the exit-criterion-4 run follows that record, and the
incompatibility list (§9.5) should name any plugin that depends on these points:

- `MouseMove` (121 distributions, §5.4) is delivered at most once per other raw input event
  plus once per frame, because pointer moves are coalesced (0014 D7, Known difference 15).
- `MouseWheel` (21 distributions, §5.4; #3 uses the wheel) fires over every hovered plugin
  control, because Tessera pushes the wheel's input context for every hovered plugin control;
  the cost is that the wheel never zooms the camera over a plugin window (0014 D5 OPEN).
- `KeyDown` and `KeyUp` (107 and 10 distributions, §5.4; #1, #9 and #10 use key events) are
  broadcast only for commands that have a Tessera action and a command ID (0014 D1); commands
  without a Tessera action (quick-slot keys and the like) do not reach plugins yet (0014
  Consequences, "Still OPEN"). While a `TextBox` has focus, a key that no UI input context
  binds is swallowed and gives no `KeyDown` (0014 Known difference 18).
- Escape and Enter declined by a focused `TextBox` go on to game code as Tessera's `menu`
  and `open_chat` actions, which game code handles as the `EscapeKey` and `AcceptInput`
  commands, and plugins get `KeyDown` with the `EscapeKey` or `AcceptInput` ID
  (0014 D2, Consequences). What game code then does for `EscapeKey` (closing the active
  panel, or opening the game menu when none is active, are the record's examples) is not
  specified by the record; it belongs to `game-panels.md` and the M5 client.

The first version of this set had Friend Alert (Thurallor, LIX 901) as #12. The corrected
examined-code scope (§3.3 rule 3) shows that it loads its author's shared library folder, which
uses `SkillList`, `Skill`, `SkillInfo`, `Backpack`, `Equipment`, `Item`, `EffectCategory` and
`MinstrelStance` and replaces `Engine.GetLanguage` with a group-A wrapper of §7.4.1 that runs at
load. It no longer qualifies (§9.1 rule 1) and was replaced by the first reserve.

<a id="9-4"></a>
### 9.4 Reserves

If a primary distribution cannot be obtained or fails for a reason outside the plugin host, use,
in this order (same rules, §9.1): ChatEdit (Galuhad, LIX 571, 1.4.1), RaidGuy (Radicus Hood,
LIX 857, v2.00), Lyrical (Astleigh Mondegreen, LIX 455, 2.2), Camp Site reference (David Down /
Vinny, LIX 520; joins the shared apartment of #6 and #7), WCFilter (burgus34, LIX 957, 1.6.2;
two reloader helpers), EpicBattlePlugin ("Epic Battle Plugin", Alvirhelm [Stever1388], LIX 874,
1.5; default apartment, chat-fed, `LoadPlugin`; added in the second version when Compass moved
into the primary set).

<a id="9-5"></a>
### 9.5 Acquisition

The owner's settings folder has no `Plugins/` folder (U1). For the test the owner downloads the
listed versions from lotrointerface.com and unpacks them into a `Plugins/` folder that Tessera is
pointed at (LPA §4.1), following each archive's own layout; nothing is committed to the
repository and nothing is written into the install. The test records, per plugin: loads (yes or
no), windows created, input handled, and every error or stub hit, which becomes the
incompatibility list that exit criterion 4 asks for.

> **OPEN:** the owner confirms that these versions are the ones to test, or substitutes the
> versions installed at test time; the census numbers above refer to the versions listed.

---

<a id="10"></a>
## 10. Invariants

1. **Dirty-room invariant (spec writers and reviewers only; not a test for implementers).**
   Re-running the census (§3) on the same 143 distributions gives the same counts. Inputs: the
   LIX file IDs and GH commits of Appendix A, which are third-party plugin sources, and the S3
   research clone; implementers must not obtain or open them.
2. Every distribution of §9.2 and §9.4 uses only first-surface classes (§7.1) by the rules of
   §3.4, and none needs the sample package or an unbundled library.
3. In the first surface, every name of §7.1 items 1 and 2 is a registered name of LPA §16, or
   an enumeration of LPA Appendix A.6/A.7.
4. A Tessera build that implements the first surface, the §7.4 empty views and the §7.4.1
   rules loads each §9.2 distribution without a Lua error raised by an API call (errors raised by plugin code
   because of absent data are recorded, not failures of this invariant).
5. With the §9.2 set installed, the apartment shared by #6 and #7 exists once, and both plugins'
   `Plugins` entries are present in it.
6. "Distributions inside" (§7.2) never shrinks when names are added to a surface; with the
   §3.4 rules the counts are 13 (tier 1), 56 (first surface) and 143 (all names).
7. Under the sandbox backend without skill data, the count of the trained-skill list is at
   least 1, and the chain `LocalPlayer.GetInstance` → `GetTrainedSkills` → `GetItem(1)` →
   `GetSkillInfo` → `GetName` returns a string *N* that (a) is not empty, (b) consists only of
   the bytes 0x30–0x39, 0x41–0x5A, 0x61–0x7A and 0x20, and (c) has ASCII letters or digits as
   its first two bytes (§7.4.2 rule 1). A synthetic test plugin (our own code) checks at load
   that both tests of §7.4.2 return "no match" for *N* without raising: test P, by building the
   132-byte string *R* from the code points U+0410–U+044F, U+0401, U+0451 with `string.char`
   and calling `string.match(R, string.sub(N, 1, 2))` inside `pcall`; test S, by calling
   `string.match` on *N* with a set pattern built from the bytes 0x81, 0x8F and the range
   0x90–0xD1. Both must return nil, and `pcall` must return true.
8. Under the sandbox backend, an `Item`-type `Shortcut` whose data names an item the backend
   does not know returns a non-nil item from `GetItem`, and that item's `GetItemInfo` →
   `GetName` is a string (§7.4.2 rule 3).
9. With the default limits of `plugin-runtime.md` §9.1 (512 MiB per apartment) and §10.1 (500
   million VM instructions per host call), no apartment of the §9.2 set is refused memory and
   no host call exhausts its budget while the set loads and runs 1,000 idle ticks (measured
   margins in §6.12). This is checked by the exit-criterion-4 run, with the plugins the owner
   installed (§9.5); implementers do not obtain them for unit tests.

---

<a id="11"></a>
## 11. Worked examples

**Example 1 — a distribution inside the first surface (§9.2 #12, Compass).** Its main package
imports with computed names, so its examined code is the whole distribution (§3.3 rule 3: 11
`.lua` files). That code uses 37 names. 28 are tier 1: `Turbine.UI.Window`, `Control`, `Label`,
`ListBox`, `Button`, `Display`, `Color`, `Turbine.UI.Lotro.Window`, `Button`, `CheckBox`,
`TextBox`, `ScrollBar`, `Font`, `Action`, `Turbine.Shell`, `ShellCommand`, `PluginData`,
`Engine`, `Chat`, `Plugin`, `Turbine.Gameplay.LocalPlayer` and `Entity`, and the enumerations
`ChatType`, `DataScope`, `Language`, `ContentAlignment`, `BlendMode`, `Orientation`. 9 are
promoted names of §7.1 item 2: `ContextMenu`, `MenuItem`, `MenuItemList`, `TreeView`,
`TreeNode`, `TreeNodeList`, `Quickslot`, `Shortcut`, `ShortcutType`. Its only gameplay access
is `LocalPlayer.GetInstance` and a `GetName` call (which, by §3.4 rule 4, credits `Entity`);
both are class **S**. So it is outside the tier-1 surface (the 9 promoted names) and inside the
first surface. Under the sandbox its window opens; the heading stays at its initial value
because it is read from location chat lines that the sandbox does not produce (§12 O5).

**Example 2 — a distribution outside (synthetic illustration of the rules).** A plugin names
`Turbine.Gameplay.LocalPlayer` and calls `GetEffects`, and on the result `GetCount` and `Get`;
rule 4 (§3.4) adds `EffectList` and `Effect`; it also calls `GetTrainedSkills`, adding
`SkillList`. Those are class **N** (§8),
so it is outside the first surface. With §7.4 empty views it can still load: the effect list
reports 0 effects and the trained-skill list 1 skill (§7.4.1).

**Example 3 — counting a shared member name.** `SetText` is a member of `Label`, `Window` and
`MenuItem`. A distribution that calls `x:SetText(…)` and uses `Label` and `Window` but never
`MenuItem` is counted for `Label.SetText` and `Window.SetText` only (§3.4 rule 5).

**Example 5 — the name tests of §7.4.2 (synthetic names).** Tessera's placeholder `Ab12`:
test P searches the two-byte pattern `Ab` in *R*, whose bytes are all 0x80 or above, and finds
nothing; test S finds no byte of 0x81, 0x8F or 0x90–0xD1. Groups A1, A2 and B return the native
language. A name read from an install, beginning with the UTF-8 letter U+0411 (bytes D0 91):
P finds D0 91 in *R*, S finds 0xD0; both report Russian, as intended. A name beginning with an
accented Latin letter (lead byte 0xC3): P finds no C3 in *R* (native value), but S matches the
lead byte 0xC3, so group A2 reports Russian in the original and in Tessera alike. A synthetic
name `A*x`: P uses the pattern `A*`, which matches the empty string at the start of *R*, so
groups A1 and B report Russian. A synthetic name `(.`: P raises a pattern error, so groups A1
and B fail to load.

**Example 4 — why rule 3 of §3.3 widens to the whole distribution.** A distribution holds a
plugin folder `A/P` and a shared library folder `A/Common` of the same author. The main package
`A.P.Main` is reached through `A/P/__init__.lua`, which reads its own package name (`_.Name`,
LPA §7), removes the last component, and imports six packages under `A.Common` and three under
`A.P` by concatenating names. Following literal imports alone reaches 4 files and no
`Turbine.Gameplay` class. Every `.lua` file of the distribution (18) is the examined code; it
uses `SkillList`, `Skill`, `SkillInfo`, `Backpack`, `Equipment`, `Item`, `EffectCategory` and
`MinstrelStance`, so the distribution is outside the first surface. A narrower widening to the
files under `A/P` (the first version of this rule) would have missed `A/Common`. (This is the
structure of the former §9.2 #12, Friend Alert.)

---

<a id="12"></a>
## 12. Open questions (summary)

| # | Question | Section |
|---|---|---|
| O1 | Static counts versus run-time needs: re-check this census against the exit-criterion-4 record | §3.5 |
| O2 | Can a plugin replace a native function on a class table so that instances see it? | §6.3 |
| O3 | Owner decision: Tessera's own clean-room `class` helper and sample-package names, or rely on the user's copy | §6.5 |
| O4 | Read the original's `.plugindata` files read-only as a fallback for keys written outside the game? | §6.7 |
| O5 | Plugins that read the player position or heading from chat lines (§9.3 #1 and #12, and other map plugins): whether and how the sandbox can produce an equivalent line, given that the original's message text is game text and must not be copied | §9.3 |
| O6 | Does the original accept `.png` (or `.bmp`) in `SetBackground`? Tessera refuses both until answered (decision 0015 §4.3) | §6.8 |
| O7 | What gameplay getters return in the original before data arrives (nil or empty object) | §7.4 |
| O8 | Owner confirms the plugin versions to test | §9.5 |
| O9 | Does the original render markup in `Shell.WriteLine` text? | §6.9 |
| O10 | What the original's `Shortcut.GetItem()` returns for an `Item`-type shortcut naming an item the character does not hold (nil or an `Item` object); one distribution's load-time language detection depends on it | §7.4.2 |

---

<a id="appendix-a"></a>
## Appendix A. Sample identifiers

**LIA** (lotrointerface file IDs, 130): 365 366 370 376 380 406 414 427 440 449 455 487 496 498
520 521 524 526 532 554 556 567 568 571 609 612 626 640 648 661 669 673 684 692 693 694 698 700
711 715 720 728 745 757 758 763 765 767 769 774 780 782 788 793 812 823 826 829 841 846 857 870
874 880 881 896 901 904 909 921 924 929 937 946 954 957 967 982 989 990 992 993 1002 1014 1015
1026 1031 1037 1042 1044 1053 1067 1075 1076 1077 1078 1085 1097 1107 1111 1112 1113 1115 1116
1119 1121 1122 1123 1125 1126 1127 1130 1131 1133 1138 1139 1140 1143 1160 1163 1171 1172 1178
1199 1201 1202 1212 1218 1237 1253.

Downloaded but excluded: 21, 256, 598, 676, 813, 820, 891, 939, 1124 (no descriptor), 1022
(library category).

**GH** (repository, commit):

| Repository | Commit |
|---|---|
| `github.com/Dromo/Pets` | `4143d06dae24` |
| `github.com/Durhamster/Second_Breakfast` | `1d601fcba06f` |
| `github.com/EsyArda/ChatNotif` | `535a3d68016c` |
| `github.com/Pet-etine/ControllerMod` | `af8ba1875b59` |
| `github.com/doe404/lotro-bagoverlay` | `f7baef7f5e46` |
| `github.com/lud0vicb/glomap` | `87703523587a` |
| `github.com/lunarwtr/lffboard-plugin` | `3a7b5763442e` |
| `github.com/souruko/FervourFocus` | `1585eab96dea` |
| `github.com/souruko/Gibberish` | `ae46bad59a51` |
| `github.com/souruko/Potato` | `e2b870597e16` |
| `github.com/souruko/TbdBars` | `ecdd686dd40a` |
| `github.com/souruko/VitalSelf` | `1d39fcf1b6b4` |
| `github.com/wduda/MissionHelper` | `2a5ab3088fe5` |

---

<a id="change-log"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First version (Review). Static census of 143 third-party plugin distributions (130 most-downloaded lotrointerface entries, 13 GitHub repositories) against the LPA §16 name set; rankings of classes, members, events, enumerations and argument fields; behaviours plugins rely on; first API surface (LPA tier 1 plus 13 promoted names and 8 behaviour items, 57 of 143 distributions inside); sandbox data classes; 14-distribution set and 6 reserves for M5 exit criterion 4; 9 OPENs. |
| 2026-10-06 | Second version, answering the review of the same day (status stays Review). **Scope fix:** §3.3 rule 3 now widens the examined code of a descriptor with a computed import to every `.lua` file of the whole distribution (46 descriptors, 35 distributions) instead of the files under the package's top two path components; the single-file trigger is dropped (rule 4), because a literal-only closure loads nothing else; "closure scope" and "whole-distribution scope" are defined. Full recount: §5.2–§5.7 tables regenerated (33 class rows, 96 member counts, 38 event counts and 62 enumeration-value counts changed, all by 1–2), §3.5 spot checks, §5.3 `Equipment` note (26), §5.6, §6.2 (`Plugins[…]` 73 whole / 71 closure), §6.3 (12 enumeration overwrites, precise groups), §6.4, §7.2 (tier 1: 13 distributions, 3.5 %; first surface: 56, 25.8 %; 87 outside, with the blocking-class list), §7.3 (`SetRotation` 14, `Plugins` 73), §8 sandbox needs, invariant 6. §3.5 now states exactly what the counts bound and where they are not upper bounds. **Plugin set:** Friend Alert (LIX 901) no longer qualifies and is replaced as §9.2 #12 by the first reserve, Compass (LIX 946); EpicBattlePlugin (LIX 874) added as sixth reserve; §9.3 row #12 and the closing statement rewritten; §11 Example 1 now uses Compass; new Example 4 shows why the widening must cover the whole distribution. **Load-time language detection:** new §7.4.1 (provenance U10) describes the `Engine.GetLanguage` replacements (8; 7 effective, in three groups; 6 called at load) and adds two Tessera rules: a non-empty trained-skill list with a non-Cyrillic skill name, and a non-nil empty item behind an unknown `Item` shortcut; new OPEN O10; new invariants 7 and 8. **Other:** §6.4 `require` row restated as calls in any form (2 direct calls with a string argument and no parentheses, 4 protected calls; none parenthesised); §10 invariant 1 marked dirty-room only; `collectgarbage` 13, `table.getn` 63, `math.mod` 18. OPEN callouts: 10. |
| 2026-10-06 | Third version, answering the re-review of the same day (status stays Review). **Issue 1 (language detection):** §7.4.1 now has four groups (A1, A2, B, C) and a new §7.4.2 states the two name tests byte by byte: test P (first two bytes used as a Lua pattern searched in a 132-byte string of Cyrillic letters; groups A1 and B) with an outcome table enumerated over every first byte 0x00–0xFF, including the empty-match, quantifier and malformed-pattern cases; test S (a byte set {0x81, 0x8F} ∪ 0x90–0xD1 searched anywhere in the name; group A2). Tessera rule 1 now constrains the placeholder (non-empty; only ASCII letters, digits and space; first two bytes letters or digits) instead of "first character outside U+0400–U+04FF"; new rule 2 says names read from the install are passed unchanged and follow the plugin's own test as in the original; the former rule 2 is rule 3. Invariant 7 rewritten to check both tests and the absence of errors; invariant 8 re-pointed; new Example 5; U10 extended. **Issue 2 (scope labels):** §3.3 states which scope every section uses; every count of §6 and §7.3 is labelled (whole distribution by default, closure value in parentheses where it differs) after a recount in both scopes (new U11). Corrections found by the recount: §6.1 row 1 now cites the §5.4 closure counts 124/118/107/59 (it quoted stale values); §6.1 class-level and instance rows state that they count `=` assignments and give the `function` forms separately; the helper row states its rule; §6.2 `Plugins` 88 (closure 86), `_G` 98 (closure 95), `getfenv` 61 (closure 56), `setfenv` 11 (was 12); §6.4 `table.getn` 64 (63), `unpack` 52 (49); §6.6 computed imports 32 (31) and 35 (34); §6.7 `Account` 57 (was 56, now equal to §5.5) and `Server` 41 (40); §6.8 DID literals 73 (72), `.tga` 115 (114), `.jpg` 31 (28), stretch 1 58 (57), stretch 2 56 (50); §6.9 `SetMarkupEnabled` 40 (38), colour tags 91 (89), `WriteLine` with a colour tag 45 (44); §6.10 and §7.3 `LoadPlugin` 33 (closure 31) and `UnloadScriptState` 29 (closure 28). **Limits (critique A7):** §6.12 adds the measured load cost of every §9.2 apartment (new U12): the 500 M instruction budget fits with a margin of at least 39 times; the 64 MiB memory default does not fit #2 (peak 160.2 MiB) or #14 (peak 75.2 MiB), so `plugin-runtime.md` §9.1 raises the default to 512 MiB; new invariant 9. **Other:** naming note now cites the interoperability record `docs/LEGAL.md` §17 (critique B10); §7.1 explains that the first surface is a build order inside the LPA tiers, so it stays within `docs/LEGAL.md` §17.2 rule 5; new source key LUA. Open questions unchanged: O1–O10 of §12 (9 callouts in the text; O5 is listed in §12 only). |
| 2026-10-06 | Revision (spec writer, dirty room); status Accepted → Revised, awaiting review of this change. Applies the Accepted decisions 0014 (UI input bridge) and 0015 (UI rendering) by cross-reference; no count, sample, invariant or other census fact changed. **§6.8:** the stretch-mode bullet points to 0015 for drawing; the `.png` OPEN extends to `.bmp` and states Tessera's rule until O6 is answered (0015 §4.3: not decoded, unsupported-format error; call-time checks in 0015 §4.4). **§6.11:** image paths are checked against an index of the whole `Plugins/` tree (0015 §4.4). **§7.3:** where the drawing (0015) and the frame timing (0014 D8) of the promoted behaviour items are decided. **§8:** `Chat.Received` reaches plugins in frame-loop step 7 (0014 D8, Known difference 21). **§9.3:** a paragraph on what the exit-criterion-4 input check sees under 0014 (`MouseMove` coalescing, `MouseWheel` over plugin controls, `KeyDown`/`KeyUp` only with command IDs and Known difference 18, Escape and Enter from a focused `TextBox`). **§12:** O6 names `.bmp` and Tessera's interim rule. Related specs and decisions added to the header. |
| 2026-10-06 | Repair of the decision revision after its review (spec writer, dirty room); status stays Revised, awaiting review. **§9.3:** the census sentence "None of the 14 replaces `Engine.GetLanguage` … closure-scope code" is moved back into the "All 14 load…" paragraph, out of the list of Tessera rules where the first pass had left it; the Escape/Enter bullet now says only what decision 0014 states (the declined key reaches game code as the `menu` or `open_chat` action, game code's handling of `EscapeKey` or `AcceptInput`; the game-menu outcome is one example of the record, whose exact behaviour it leaves unspecified). **§6.8:** "a control with a stretch mode" → "a non-zero stretch mode", and a mode-0 control gets no surface of its own (decision 0015 Decision 1.1). No count, sample, invariant or other census fact changed. |
