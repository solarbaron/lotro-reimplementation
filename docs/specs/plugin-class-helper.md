<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Plugin class helper and sample utilities (built-in compatibility package)

| Field | Value |
|---|---|
| Status | **Accepted** (2026-10-08) |
| Last updated | 2026-10-08 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | Claude review session (spec-writer agent, not the author) |
| Review record | Fourth review (re-review of the T1–T2 revision), Claude review session (spec-writer agent, not the author), 2026-10-08: **passed**, status moved to Accepted. T1 (§6.13.1) and T2 (§6.10, §5.3, H15) resolved; O1–O4 and the implementer note O6 stay open, non-blocking. Earlier records in the review log (decision 0018). |
| Implements into | `crates/plugins` (the built-in package, the user-copy test, import resolution of the built-in names), `crates/script` (nothing new beyond `plugin-runtime.md`) |
| Related specs | [`lua-plugin-api.md`](lua-plugin-api.md) (abbreviated **LPA**: §7 `import`, §8 object model, §8.3 subclassing from Lua, §9 events, §13 controls), [`plugin-runtime.md`](plugin-runtime.md) (abbreviated **PRT**: §5.1 apartment creation, §6 environments, §7 packages, §4.4 text forms), [`plugin-descriptor.md`](plugin-descriptor.md) (§7.2 disk resolution and precedence), [`plugin-api-usage.md`](plugin-api-usage.md) (abbreviated **PAU**: the census, §6.5); decision record [`0016-plugin-class-helper.md`](../decisions/0016-plugin-class-helper.md) |

> **Naming note (interoperability).** The global names (`class`, `import`), package names
> (`Turbine.Class`, `Turbine.Type`, `Turbine.Table`, `Turbine.MathUtils`, `Turbine.Log`,
> `Turbine.Debug`, `Turbine.Debug.Table`, `Turbine.Debug.Stack`, `Turbine.Animation`,
> `Turbine.Animation.LerpValue`, `Turbine.UI.Extensions`, `Turbine.UI.Extensions.SimpleWindow`,
> `Turbine.UI.Extensions.Window`, `Turbine.UI.Lotro.Quickslot`), field and method names and the
> folder name `Plugins/Turbine/` are reproduced only because existing third-party plugins, or
> copies of the original helper that plugins carry, cannot work with Tessera without them. Each
> one and its interop path (the last row lists the names this spec mentions but Tessera does not
> reproduce):
>
> | Names | Interop path |
> |---|---|
> | the global `class`, the import names above, `Plugins/Turbine/` | plugins call, import or look for them (§4.1–§4.5, §5.1) |
> | `Constructor`, `GetType`, `IsA` | plugins define and call them (§4.4); every copy of the original helper (50 distributions carry one in a namespace of their own, §4.7) calls `GetType` on each class argument and `IsA` target, and decides from `IsA`'s presence whether to store its own |
> | `ToString`, `GetUniqueID` | `tostring` of a class or instance goes through `ToString` (§6.4), so a plugin's own `ToString` sets the text; and a copy of the original helper stores its own `ToString` and `GetUniqueID` on a subclass only when lookup finds none, so their presence on Tessera classes decides what a copy-made subclass of a Tessera class gets (§6.3) |
> | type-object methods `IsClass`, `IsFinal` | a copy of the original helper asks `IsClass` of the type object of every class argument and `IsA` target, and `IsFinal` of every class base, Tessera classes included; Tessera's `class` and `IsA` ask them of other helpers' type objects (§6.1, §6.7) |
> | type-object method `IsMixin` | a copy's `mixin` function asks it of every argument, and a copy's `IsA` asks it of a target that is not a class; Tessera's `IsA` asks it of other helpers' targets (§6.7) |
> | type-object methods `GetBaseClass`, `GetMixins` | a copy's `IsA` walk asks them of the start type and of each base's type, which are Tessera's type objects when a Tessera class sits below a copy-made root class; Tessera's walk asks them of other helpers' type objects (§6.7) |
> | type-object methods `GetClass`, `GetName` | a copy's default `ToString`, inherited by a Tessera class whose base the copy made, asks the Tessera type object for both (§6.3, §6.8) |
> | `Dump`, `AreEqual`, `round`, the members of `LerpValue`, `SimpleWindow` and `Window`, the instance fields of §7.1 and §7.4 | members of objects that a dependent distribution reaches (§4.4, §4.5); a class a dependent creates or subclasses keeps its whole method set, and the instance fields are names a plugin subclass can read or collide with |
> | `Debug`, `Information` (of `Turbine.Log`) | plugins call them on `Turbine.Log` in colon form without defining the log: `Information` in 3 distributions, `Debug` in 1 (§4.5). The log's other members have no interop path and are not reproduced (§7.7) |
> | **Not provided, named only for that:** the globals `static_class`, `abstract_class`, `final_class`, `mixin`, `enum`, `opairs`, `quicksort`, `string.trim`, `string.startsWith`, `string.endsWith`; the package and import names `Turbine.Compare`, `Turbine.Enum`, `Turbine.Event`, `Turbine.Logger`, `Turbine.LogLevel`, `Turbine.LogManager`, `Turbine.Mixin`, `Turbine.Quicksort`, `Turbine.Service`, `Turbine.ServiceContainer`, `Turbine.String`, `Turbine.Timer`, `Turbine.TimerManager`, `Turbine.Testing`, `Turbine.Testing.Assert`, `Turbine.Utils`; the sample's file names `Debug.lua`, `Debug/` and `UI/Lotro/__init__.lua` | **none, and Tessera never defines them** in any mode (in user-copy mode only the user's own files can). They appear only in §4.2, §4.5, §5.3, §5.6, §8 and the invariants, to give the census counts and to state that Tessera does not provide them; their absence is observable (a read gives nil, an import fails), so the spec must say so. `Turbine.Utils` stays here unless the owner decides O3 otherwise |
>
> The other type-object methods, the type-information table and its field names, the original
> log's name and members other than those of §7.7, and the original's default `ToString` text
> have no interop path and are **not** reproduced: Tessera's behaviour for them is its own
> (§6.2, §6.8, §6.9, §7.7, §8). Per `docs/CLEAN_ROOM.md` §5.3, these
> names appear only inside the plugin subsystem, as string data; never as product, crate,
> module or UI names. The publisher's 2010 sample package was read for **behaviour only**: no
> line of its code, none of its comments, file layout beyond the import names, or message
> texts are reproduced here, and Tessera ships none of it (`CLAUDE.md`). Plugin names
> and file IDs identify counted material only.

---

<a id="1"></a>
## 1. Purpose

About a quarter of published plugins (PAU §6.5: 37 of 143 distributions) only work when the user
has installed the publisher's 2010 sample utility package as `Plugins/Turbine/`. It supplies a
global `class` function used to declare Lua classes and to subclass the native API classes, and a
few utilities (fading windows, a value interpolator, a table dump). Decision 0016 has Tessera
ship its **own clean-room package** with the same import names and the behaviour those plugins
rely on, while a copy the user installed keeps precedence.

This spec lets an implementer build that package:

- which names it must provide, measured on the census corpus (§4);
- when it is installed and how the import resolver chooses between it and a user copy (§5);
- the exact observable behaviour of `class` and the classes it makes (§6);
- the other utilities in the needed set (§7);
- what Tessera's package deliberately leaves out or does differently (§8);
- invariants and worked examples to test against (§9, §10).

Out of scope: the native API (LPA), the extended scripting API of decision 0016 part 2 (a
separate decision), and any third-party library other than the sample package (for example the
separately published drag-bar library of PAU §6.11, which users install themselves).

---

<a id="2"></a>
## 2. Provenance

Source keys:

| Key | Source |
|---|---|
| **SMP** | The publisher's 2010 sample package: folder `Turbine/` (37 Lua files) of `dist/Beta_LuaPlugins.zip` in the research clone `Goldbishop_Lotro_LUA` (LPA source S4). Read in the dirty room on 2026-10-06 for behaviour only (method M2). |
| **TU** | The package as users install it today: lotrointerface.com file 1064, version 1.1.1 (published 2026-02-14), downloaded on 2026-10-06 into the dirty scratch area. Its 37 Lua files are byte-identical to SMP except one corrected line in the logger (the information level, §7.7); it adds a dummy descriptor whose root element is not `Plugin`, and has no `Utils.lua` (method M2). |
| **CEN** | The census corpus of PAU (143 distributions: LIA and GH of PAU §2, Appendix A). Headline counts are PAU §6.5 (its row U5). Detailed counts are a recount in this session, 2026-10-06: whole distribution, comments removed, files that are copies of SMP files excluded (method M2). |
| **RUN** | SMP executed with stock Lua 5.1.5 in the dirty scratch area, 2026-10-06, under a stub host that implements the `import` rules of LPA §7.2 and a few stub native classes. Used to confirm the behaviour read from SMP (method M2; no client code was run or read). A second run the same day, for the revision, checked the class names of the sample's classes, the nature of its type class, the shapes of its debug table and log, the quickslot call order, `Dump` with indent `false`, `IsA` with a mixin target and `IsA` through a base with a foreign type object. A third run the same day, for the second revision, checked `Turbine.Table.AreEqual`, `IsA` and `class` with tables that have a `Constructor` function and no metatable (alone, behind an `__index` link, with a metatable that has no `__index`, and a native class chain whose root has a `Constructor` function), the write order of the fading windows' `SetOpacity` when the game-time read or the base's `GetOpacity` raises, and the log's `Debug` and `Information`. The stub of these three runs restored the **previous** thread global table after a nested import, not the real one that LPA §7.2 step 4 states. A fourth run, 2026-10-08, for the third revision, used a stub that restores the real global table as LPA states, and checked the class names of §6.10 and of the sample's own window classes under that rule (H15). |
| **OWN** | The owner's install and settings folder, read-only, 2026-10-06 (method M1). |
| **LPA**, **PRT**, **PAU** | The Accepted specs named in the header. |
| **DEC** | Decision 0016 (Accepted 2026-10-06). |
| **LUA** | General knowledge of Lua 5.1 (not dirty, `CLEAN_ROOM.md` §2 rule 4). |

Marks: **Confirmed** = observed in the owner's install or settings folder. **Reported** = from
published third-party material (SMP, TU, CEN, RUN). **Tessera rule** = a choice of this project
(DEC or this spec), not a fact about the original. No BIN (M4) work was done for this spec.

| # | Fact or group of facts | Sources | Mark |
|---|---|---|---|
| H1 | The client install ships no `.lua` file and no `Turbine` folder; the owner's settings folder has no `Plugins/` folder. The sample package is user content, so on the owner's machine Tessera's built-in package is the one that applies (§5) | OWN | **Confirmed** 2026-10-06 |
| H2 | TU equals SMP except the logger line and the dummy descriptor | SMP, TU | Reported 2026-10-06 |
| H3 | Census headline: 121 distributions call `class`, 90 define one, 11 bundle a copy of the sample folder, 37 depend on the user's copy; import counts per sub-package (§4.1) | PAU §6.5 (U5) | Reported 2026-10-06 |
| H4 | Detailed usage among the 37 dependent distributions and across the corpus (§4.2–§4.6). The `Turbine.Log` row of §4.5 was recounted for the second revision: 3 distributions, not 1 | CEN | Reported 2026-10-06 |
| H5 | Behaviour of `class`, the class and instance metatables, constructors, `IsA` (including the walk through other helpers' type objects and the mixin-target case), type objects, `ToString`, `GetUniqueID`, class names (§6) | SMP, RUN, LPA §7.2, §8 | Reported 2026-10-06 |
| H6 | What the original runs at apartment creation when the user copy exists, and that the folder `Debug/` shadows the file `Debug.lua` (§5.6) | SMP, RUN, LPA §7.2, §7.4, PRT §5.1 | Reported 2026-10-06 |
| H7 | Behaviour of the fading windows (including the write order of `SetOpacity`) and of the quickslot replacement (§7.1, §7.2) | SMP, RUN | Reported 2026-10-06 |
| H8 | Behaviour of the table dump, the interpolator, `math.round`, `Turbine.Table.AreEqual` and the default log (§7.3–§7.7); the original's type building that makes `AreEqual`, `IsA` and `class` raise on a table with a `Constructor` function and no metatable (K5, K17, K18) | SMP, TU, RUN | Reported 2026-10-06 |
| H9 | `Turbine.Utils` is in neither SMP nor TU; 2 distributions import it without protection; one distribution ships an empty file under that name inside its own copy of the sample folder (§4.5) | SMP, TU, CEN | Reported 2026-10-06 |
| H10 | Plugins rely on instances of one class sharing one metatable, and on reading and reusing it (§6.4) | CEN | Reported 2026-10-06 |
| H11 | The built-in package, its contents, the user-copy test, installation at apartment creation, known differences (§5, §7 "Tessera" items, §8) | DEC, LPA §7, PRT §5, this spec | **Tessera rule** |
| H12 | Copies of the original helper's class file carried by plugins: 53 distributions, 3 in a top-level `Turbine/` folder, 50 in a namespace of their own; one dependent carries a copy it never imports and subclasses `SimpleWindow` with the global `class` (§4.7) | CEN (recount 2026-10-06) | Reported 2026-10-06 |
| H13 | In the original: the sample's class names follow the `_` rule (`Turbine.UI.Extensions.SimpleWindow`, `Turbine.UI.Extensions.Window`, `Turbine.Animation.LerpValue`, `Turbine.Type` in the usual import order); its type class is a final helper class whose instances are the type objects; its debug table is a non-callable helper class; its log is an instance of a helper class (§5.5, §6.8, §7.3, §7.7, §8) | SMP, RUN | Reported 2026-10-06 |
| H14 | Tessera's class registry, type objects, `ToString` text, built-in class names and namespace steps (§5.3, §5.5, §6.2, §6.8, §6.9, §6.13) | this spec | **Tessera rule** |
| H15 | After an import that runs a disk chunk, the importing file's top-level run has the real global table as `getfenv(0)` (LPA §7.2 step 4), so a class it declares afterwards reads `_` of the real global table (§6.10). The original's class function reads `_` of `getfenv(0)`. With the user copy installed and `Turbine.UI.Lotro` not imported before, the sample's `SimpleWindow` (and `Window` when imported alone) is unnamed (§5.5, K14) | LPA §7.2 (its row P8, BIN), SMP, RUN (fourth run) | Reported 2026-10-08 |
| H16 | Lifetime of classes, type objects and registry entries in a runtime without ephemeron tables (§6.8, §6.13) | this spec; LUA (stock Lua 5.1 weak tables mark the values of a weak-keyed table strongly) | **Tessera rule** |

---

<a id="3"></a>
## 3. Terms and relation to other specs

<a id="3-1"></a>
### 3.1 Terms

| Term | Meaning |
|---|---|
| **Sample package** | The publisher's 2010 utility folder that users install as `Plugins/Turbine/` (SMP, TU). |
| **User copy** | Any `Turbine` entry the user has placed directly in `Plugins/` (§5.1). |
| **Built-in package** | Tessera's clean-room package of this spec. It has no files in the user's tree; it stands in for the disk part of the names of §5.3. |
| **Helper class** | Any class table made by a Lua class function rather than by the native API: Tessera's, the sample package's, a copy of it that a plugin carries, or a plugin's own helper. |
| **Tessera class** | A class table made by Tessera's `class` of §6 in built-in mode, including the built-in classes of §5.5. Tessera recognises its own classes only through the private registry of §6.13, never by their fields. |
| **Other helper's class** | A helper class that is not a Tessera class. Tessera knows it only by behaviour: lookup finds a `GetType` function on it, or a `Constructor` function (§6.1). |
| **Type object** | The value a class's `GetType` function returns. Tessera queries any type object, its own or another helper's, only through the methods of §6.8. |
| **Native class** | A class table of the native API (LPA §8.1). |
| **Lookup** | Ordinary Lua indexing, which follows `__index` metatables (LUA). |
| **Dependent distribution** | One of the 37 distributions of PAU §6.5 that fail without the sample package. |

<a id="3-2"></a>
### 3.2 Which spec says what

| Topic | Where |
|---|---|
| `import` call form, resolution, namespaces, run-once record, the `_` field | LPA §7, PRT §6–§7, plugin-descriptor §7.2 |
| Native class tables, instances, binding a native object to a Lua table, native `IsA` | LPA §8.1–§8.3 |
| Event dispatch to handler fields (the `Update` method of §7.1) | LPA §9 |
| Text form of a table address (used by `GetUniqueID`) | PRT §4.4 (R5) |
| The helper's own behaviour, the built-in package, precedence | **this spec** |

---

<a id="4"></a>
## 4. Census: what plugins import and call

<a id="4-1"></a>
### 4.1 Headline (PAU §6.5)

- 121 of 143 distributions call a global `class` function. 90 define one themselves, 11 bundle a
  copy of the sample folder (the two overlap).
- **37 depend on the user's copy**: 31 call `class` without defining or bundling it, 28 import
  sub-packages of the sample package without bundling them.
- Distributions importing each sub-package (bundled or not; test-only imports left out):
  `Turbine.UI.Extensions` 27, `Turbine.Debug` 7, `Turbine.Type` 5, `Turbine.Class` 4,
  `Turbine.Animation` 4, `Turbine.Utils` 3.

<a id="4-2"></a>
### 4.2 Import paths used (interop identifiers)

Every import of a sample name in the corpus is a string literal with this exact spelling and
case (no case variants occur). Counts are distributions whose **own** code (copies of SMP files
excluded) imports the name:

| Import path | Own-code importers | Of them dependent | Notes |
|---|---|---|---|
| `Turbine.UI.Extensions` | 27 | 26 | The 27th bundles the whole sample folder as a top-level `Turbine/` folder of its archive (the user extracts it, so it is a user copy). One more dependent reaches `Turbine.UI.Extensions.SimpleWindow` and `.Window` by their full names from a file of its own. |
| `Turbine.Debug` | 3 | 3 | |
| `Turbine.Animation` | 3 | 2 | The third bundles the sample folder. |
| `Turbine.Utils` | 2 | 2 | Not in SMP or TU (§4.5). |
| `Turbine.Type` | 1 | 1 | In a file that the distribution never imports. |
| `Turbine.Testing`, `Turbine.Testing.Assert` | 3 | 2 | Only in test files that the plugins do not load. |

The other sample names (`Turbine.Class`, `Turbine.Compare`, `Turbine.Enum`, `Turbine.Event`,
`Turbine.Log…`, `Turbine.Logger`, `Turbine.Mixin`, `Turbine.Quicksort`, `Turbine.Service…`,
`Turbine.String`, `Turbine.Table`, `Turbine.Timer…`, `Turbine.MathUtils`,
`Turbine.Debug.Table`, `Turbine.Debug.Stack`, `Turbine.Animation.LerpValue`,
`Turbine.UI.Lotro.Quickslot`) are imported only inside bundled copies of the sample folder.

<a id="4-3"></a>
### 4.3 Calls of `class` in the dependent distributions

- 144 calls in 35 of the 37 dependent distributions (one of them sits in a copy of the helper
  that its distribution never imports). The remaining 2 dependents only import sub-packages.
- Every call passes **zero or one** argument: 7 calls have none, 137 have one. No call passes two.
- Every call result is assigned to exactly one name (one exception in the never-imported copy).
- The single argument is: a native class in 127 calls (`Turbine.UI.Window` 40,
  `Turbine.UI.Control` 39, `Turbine.UI.Lotro.Window` 11, `Turbine.UI.Label` 9,
  `Turbine.ShellCommand` 4, `Turbine.UI.Button`, `ContextMenu`, `TreeNode` 3 each,
  `Turbine.UI.ListBox`, `MenuItem`, `Turbine.UI.Lotro.TextBox`, `Lotro.Button`,
  `Lotro.ItemControl`, `Lotro.Quickslot`, `Turbine.Object` 2 each,
  `Turbine.UI.Lotro.EffectDisplay` 1); a class from `Turbine.UI.Extensions` in 5
  (`SimpleWindow` 4, `Window` 1); a Lua class of the plugin itself in 5 (one of them in the
  never-imported copy).
- Across the whole corpus, 13 distributions (with various helpers) declare a class with base
  `Turbine.Object`, and several call `Turbine.Object.Constructor(self)` in the constructor
  (§11, O2).

<a id="4-4"></a>
### 4.4 Members and side effects the dependents reach

| Name | Dependent distributions | Use |
|---|---|---|
| `Turbine.UI.Extensions.SimpleWindow` | 21 | 17 create instances directly; 4 subclass it |
| `Turbine.UI.Extensions.Window` | 2 | 1 creates an instance, 1 subclasses it |
| `SetFadeSpeed` (of those classes) | 5 | |
| A method named `SetShortcut` | 15 of the 26 dependents that import `Turbine.UI.Extensions` | affected by the quickslot replacement that this import installs (§7.2) |
| `Turbine.Debug.Table.Dump` | 2 | in a debug command |
| `Turbine.Animation.LerpValue` | 1 | creation, `SetValue`, `SetRate`, `Update`, `GetValue`, `GetTargetValue`, `NeedsUpdate` |
| `getmetatable` of a helper-class instance | 2 | change `__index` and `__newindex` of the shared instance metatable inside a constructor (§6.4) |
| Deep-copy routines that copy a table and give the copy the original's metatable | 3 | general-purpose copies of plugin tables; a helper-class instance passed to them keeps working only if its metatable is an ordinary, shared table (§6.4) |
| `GetType`, `IsA` (as written in plugin code) | 11, 2 | on native objects (shortcut types, gameplay entities) or inside the never-imported helper copy; never on the helper's type objects |
| `ToString`, `GetUniqueID`, the helper's type objects | 0 | |

<a id="4-5"></a>
### 4.5 Root names read without importing them

When the user copy is installed, the original runs its root file in every apartment at creation
(§5.6), so its names exist before any plugin code runs. Across all 143 distributions, these are
read by code that neither defines nor bundles them:

| Name | Distributions | Use |
|---|---|---|
| `class` | 31 (PAU §6.5) | declaring classes |
| `math.round` | 1 | colour conversion |
| `Turbine.Table.AreEqual` | 1 | comparing two data tables in an editor window |
| `Turbine.Log` | 3 | `Information` in all 3, in the warning routine of a decompressor each carries (reached on malformed input); `Debug` in 1 of them, on an error path. Both in colon form. No other member is read. One of the 3 carries a copy of the sample folder in a namespace of its own that it never imports, and which could not set `Turbine.Log` anyway |
| `static_class`, `abstract_class`, `final_class`, `mixin`, `enum`, `opairs`, `quicksort`, `string.trim`, `string.startsWith`, `string.endsWith`, the other `Turbine.*` names of SMP | 0 | only in distributions that bundle their own copies or define them |

`Turbine.Utils`: neither SMP nor TU contains it, so in the original `import "Turbine.Utils"`
fails unless the user has some other file of that name. One distribution ships an empty file
of that name inside its own bundled copy of the sample folder. The 2 importers import it at
load without protection, so in the original they load only for users who have such a file (O3).

<a id="4-6"></a>
### 4.6 The needed set

From §4.2–§4.5, the built-in package provides (Tessera rule, DEC):

| Item | Why | Section |
|---|---|---|
| Global `class` | called by 35 dependents (§4.3); read without defining or bundling it by 31 (§4.5) | §6 |
| `Turbine.UI.Extensions` with `SimpleWindow`, `Window` and the quickslot replacement | 26 dependents import the package, 1 more imports its two window files | §7.1, §7.2 |
| `Turbine.Debug` with `Table.Dump` and `Stack` | 3 | §7.3 |
| `Turbine.Animation` with `LerpValue` | 2 | §7.4 |
| `math.round`, `Turbine.Table.AreEqual`, `Turbine.Log` with `Debug` and `Information` | read without import by 1, 1 and 3 distributions | §7.5–§7.7 |
| Import names `Turbine.Class`, `Turbine.Type`, `Turbine.MathUtils`, `Turbine.Table`, `Turbine.Log`, `Turbine.Debug.Table`, `Turbine.Debug.Stack`, `Turbine.Animation.LerpValue`, `Turbine.UI.Extensions.SimpleWindow`, `Turbine.UI.Extensions.Window`, `Turbine.UI.Lotro.Quickslot` succeed | consistency with the names above; a plugin may import a member file directly | §5.3 |

Everything else of the sample package is left out (§8, K1).

<a id="4-7"></a>
### 4.7 Copies of the original helper carried by plugins

53 of the 143 distributions carry a copy of the original helper's class file (recognised by its
type-object queries; H12). 3 have it in a top-level `Turbine/` folder of their archive, which
becomes a user copy once extracted (§4.2). The other **50** keep it in a namespace of their own:
when their copy runs, it replaces the global `class` of the apartment, and its type objects come
from a copy of the type file in the same namespace.

Such a copy meets Tessera classes when a plugin passes a Tessera class (for example
`SimpleWindow`) as a base to the copy's `class`, or passes a copy-made class to Tessera's
`class`, or when a class of one helper is the target of the other's `IsA`. The copy then
queries Tessera's type objects with the method names of §6.8, and Tessera queries the copy's.
In the census this happens in no live code path: the one dependent among the 50 that subclasses
`SimpleWindow` never imports its copy and uses the global `class`. The paths are kept because
the copies' code makes them, and any plugin outside the census that combines a copy with the
built-in classes takes them.

---

<a id="5"></a>
## 5. Installation and precedence

<a id="5-1"></a>
### 5.1 The user-copy test (Tessera rule)

When an apartment is created (PRT §5.1), Tessera decides its **mode** once, for the whole life
of that apartment:

1. Look at the entries directly inside the user's `Plugins/` folder (through the plugin-tree
   index of decision 0015 §4.4). Names compare case-insensitively (LPA §4.4).
2. If there is a **directory** named `Turbine`, or a **file** named `Turbine.lua`, the apartment
   is in **user-copy mode**.
3. Otherwise it is in **built-in mode**.

The test looks only at existence. It does not check the folder's contents, version or
completeness. A refresh of the plugin list does not change the mode of an existing apartment.
A new apartment applies the test again.

<a id="5-2"></a>
### 5.2 User-copy mode

The built-in package takes no part. Every rule is the original's (LPA §7.2, §7.4,
plugin-descriptor §7.2): at creation, after the native `Turbine` part, a
`Plugins/Turbine/__init__.lua` (if present) runs as the merged disk part of `Turbine`; every
`Turbine.*` sub-package resolves from disk; a name missing from the user's tree fails to import.
Tessera never fills a gap with a built-in name. The user's files are read in place, read-only,
under the normal sandbox and limits (`CLAUDE.md`).

<a id="5-3"></a>
### 5.3 Built-in mode: names served

In built-in mode, the import resolver treats each name below as having a **disk part** supplied
by the built-in package. All other steps of LPA §7.2 apply unchanged: the native registry is
asked first; a name that is also native (only `Turbine` here) is merged, native part first; the
namespace is walked from the real global table; the run-once record is keyed by the name as
passed; the namespace's `_` field is set to a table holding the name. Names match
case-insensitively, as disk paths would; a spelling that differs only in case is a different
name for the run-once record (LPA §7.3) and installs the package again under the namespace path
as written.

| Name | Shape (LPA §7.2) | Namespace | Imports first | Installs |
|---|---|---|---|---|
| `Turbine` | folder (merged with native) | `Turbine` | — | the root, §5.4 |
| `Turbine.Class` | file | `Turbine` | `Turbine.Type` (after installing `class`) | global `class` (§6) |
| `Turbine.Type` | file | `Turbine` | `Turbine.Class` | `Turbine.Type` (§6.8) |
| `Turbine.MathUtils` | file | `Turbine` | — | `math.round` (§7.5) |
| `Turbine.Table` | file | `Turbine` | — | `Turbine.Table` (§7.6) |
| `Turbine.Log` | file | `Turbine` | `Turbine.Class` | `Turbine.Log` (§7.7) |
| `Turbine.Debug` | folder | `Turbine.Debug` | `Turbine.Debug.Stack`, `Turbine.Debug.Table` | — |
| `Turbine.Debug.Stack` | file | `Turbine.Debug` | `Turbine` | `Turbine.Debug.Stack` (§7.3) |
| `Turbine.Debug.Table` | file | `Turbine.Debug` | `Turbine` | `Turbine.Debug.Table` (§7.3) |
| `Turbine.Animation` | folder | `Turbine.Animation` | `Turbine.Animation.LerpValue` | — |
| `Turbine.Animation.LerpValue` | file | `Turbine.Animation` | `Turbine` | `Turbine.Animation.LerpValue` (§7.4) |
| `Turbine.UI.Extensions` | folder | `Turbine.UI.Extensions` | `…SimpleWindow`, `…Window`, `Turbine.UI.Lotro.Quickslot` (in this order) | — |
| `Turbine.UI.Extensions.SimpleWindow` | file | `Turbine.UI.Extensions` | `Turbine`, `Turbine.UI`, `Turbine.UI.Lotro` | `SimpleWindow` (§7.1) |
| `Turbine.UI.Extensions.Window` | file | `Turbine.UI.Extensions` | `Turbine`, `Turbine.UI`, `Turbine.UI.Lotro` | `Window` (§7.1) |
| `Turbine.UI.Lotro.Quickslot` | file | `Turbine.UI.Lotro` | `Turbine.UI.Lotro` | the quickslot replacement (§7.2) |

"Imports first" means the package performs those imports, with the normal run-once rule,
before it installs its own names. Importing `Turbine.UI.Extensions` therefore leaves
`Turbine.UI` and `Turbine.UI.Lotro` present, as in the original.

**Namespace steps (Tessera rule, matches the original with the user copy installed).** Installing
the disk part of a name in the table runs steps 1, 2, 4 and 5 of LPA §7.2 "Running a disk chunk"
with that name's namespace table `T`, exactly as a disk chunk would, with the built-in install in
place of loading and calling the file (step 3):

- before the install, if `T` is not the real global table, `T`'s metatable is remembered and
  replaced by a new table whose `__index` is the real global table;
- when the install starts, `T` becomes the thread's global table. It stays so until the install
  performs an import (in its "imports first" step, or in plugin code it reaches) that runs a
  disk chunk or another built-in install; that import ends by restoring the **real** global
  table (LPA §7.2 step 4, whether the chunk succeeded or raised), and from then until this
  install ends the thread's global table is the real one, not `T`. An import that runs neither
  (a name already recorded as run, or a name with only a native part, such as `Turbine.UI` in
  built-in mode) leaves the thread's global table unchanged. Example: the install of
  `Turbine.Class` installs `class` with `T` = `Turbine` as the thread's global table; its import
  of `Turbine.Type`, if not yet run, then leaves the real global table in place for the rest of
  it. `getfenv(0)` in plugin code that the install reaches (for example a metamethod a plugin
  put on a native class table that the install reads) follows this rule; the built-in package
  itself never reads it (built-in class names are fixed, §5.5);
- afterwards the real global table is restored, and the new metatable is kept unless `T` had a
  table metatable before, which is put back.

So, as with the user copy installed, these namespaces fall back to the apartment's globals
through `__index` once the stated import has run: `Turbine` and `Turbine.Debug` (both at
apartment creation, §5.4), `Turbine.Animation` and `Turbine.UI.Extensions` (each on the import
of its folder name or of one of its file names), and `Turbine.UI.Lotro` (only after
`Turbine.UI.Lotro.Quickslot` has been imported; a plain `import "Turbine.UI.Lotro"` has no disk
part in built-in mode). `Turbine.UI` never gets one. For example, after creation `Turbine.print` is the global `print` and
`Turbine.class` is the global `class`. The only built-in behaviour that reads through such a
fallback is `LerpValue.GetRate` (§7.4).

Every other name under `Turbine` that is not native (for example `Turbine.Event`,
`Turbine.Timer`, `Turbine.Testing`, `Turbine.Utils`) has no disk part in built-in mode and fails
to import with the ordinary catchable error of LPA §7.1.

<a id="5-4"></a>
### 5.4 Built-in mode: the root at apartment creation

At PRT §5.1 step 5, `import "Turbine"` installs the native part and then the built-in root,
before any plugin code runs. The root imports, in this order: `Turbine.Class`, `Turbine.Debug`,
`Turbine.Log`, `Turbine.MathUtils`, `Turbine.Table`, and the native `Turbine.UI`. So the first
line of every plugin sees `class`, `math.round`, `Turbine.Type`, `Turbine.Table`, `Turbine.Log`,
`Turbine.Debug` (with `Table` and `Stack`) and `Turbine.UI`, as it would in the original with the
user copy installed (§5.6). All these names are recorded as run, so a later `import` of any of
them does nothing.

The root's names are ordinary writable values of the apartment. A plugin may replace `class`
(PAU §6.5: 90 distributions define their own, several by assigning the global). Later code in
that apartment then sees the replacement, exactly as in the original.

<a id="5-5"></a>
### 5.5 Built-in classes are independent of the current `class`

The classes the built-in package creates (`SimpleWindow`, `Window`, `LerpValue`) are made with
Tessera's own class mechanism of §6 even if a plugin has replaced the global `class` before the
import (Tessera rule; K4). They are Tessera classes (§3.1). Type objects and `Turbine.Type` are
not classes at all (§6.8).

Their full names are fixed (Tessera rule); they do not depend on the `_` field or on
`getfenv(0)` at the time of the import (§6.10 applies only to classes that plugin code declares).
They equal the names the original gives these classes in the usual import order (H13), in
which the plugin has imported `Turbine.UI.Lotro` before `Turbine.UI.Extensions`. (With the user
copy installed and that name not imported before, the sample's `SimpleWindow` file runs the
copy's `UI/Lotro/__init__.lua` through its own import, so it declares its class with the real
global table as `getfenv(0)` and the class is unnamed, §6.10, H15; Tessera keeps the fixed
name, K14.)

| Class | Full name | Base (`GetBaseClass()`) | `GetName()` | `tostring` of the class / of an instance (§6.9) |
|---|---|---|---|---|
| `SimpleWindow` | `Turbine.UI.Extensions.SimpleWindow` | `Turbine.UI.Window` | `SimpleWindow` | `class Turbine.UI.Extensions.SimpleWindow (`ID`)` / `instance of Turbine.UI.Extensions.SimpleWindow (`ID`)` |
| `Window` | `Turbine.UI.Extensions.Window` | `Turbine.UI.Lotro.GoldWindow` | `Window` | `class Turbine.UI.Extensions.Window (`ID`)` / `instance of Turbine.UI.Extensions.Window (`ID`)` |
| `LerpValue` | `Turbine.Animation.LerpValue` | none (root class) | `LerpValue` | `class Turbine.Animation.LerpValue (`ID`)` / `instance of Turbine.Animation.LerpValue (`ID`)` |

ID is the value's `GetUniqueID()` (§6.9). Each install of a name creates its class anew: normally
once per apartment; a case variant of the name installs again (§5.3), as re-running the file
would in the original.

<a id="5-6"></a>
### 5.6 What the original runs when the user copy is installed (for reference)

With SMP or TU installed, the original's apartment creation runs the package's root file, which
imports the package's class, comparison, debug, enumeration, event, logging, math, mixin,
sorting, service, string, table, timer and type files. Observable effects in every apartment
(RUN):

- globals `class`, `static_class`, `abstract_class`, `final_class`, `mixin`, `enum`,
  `quicksort`, `opairs`; `math.round`; `string.startsWith`, `string.endsWith`, `string.trim`;
- in `Turbine`: `Type`, `Compare`, `Debug` (with `Table`, `Stack`), `Event`, `Log`, `Logger`,
  `LogLevel`, `LogManager`, `Service`, `ServiceContainer`, `Table`, `Timer`, `TimerManager`;
- `Turbine.UI` imported, and one hidden `Turbine.UI.Control` created by the timer manager;
- the global `error` function is **not** replaced. The package has both a folder `Debug/` and a
  file `Debug.lua`; `import "Turbine.Debug"` resolves to the folder (LPA §7.2 directory test),
  so the file, which would replace `error`, never runs.

In addition, because the package has a folder `UI/Lotro/` with an `__init__.lua`, every
`import "Turbine.UI.Lotro"` in such an apartment merges that file with the native package, which
installs the quickslot replacement of §7.2 (K2).

---

<a id="6"></a>
## 6. The `class` function

<a id="6-1"></a>
### 6.1 Arguments and the base

`class` takes any number of arguments. Arguments that are nil are ignored, wherever they appear.
Each remaining argument is classified in this order:

1. A **table** on which lookup of `GetType` yields a function: that function is called with no
   arguments, and the result, a type object (Tessera's or another helper's), decides through
   queries (§6.8). Query `IsClass`; if the answer is true (any value other than nil and false),
   query `IsFinal`: a true answer is an error, otherwise the argument is a class base. If
   `IsClass` does not answer true, the argument is an error; this covers mixins made by other
   helpers (K5). Errors raised by the `GetType` call or by a query propagate. (A native class
   with a native `GetType` method, such as `Turbine.UI.Lotro.Shortcut`, ends up here; its native
   method, called without an object, raises the error of LPA §8.2. The original's helper
   behaves the same way.)
2. Otherwise, a **table** on which lookup of `Constructor` yields a function (every
   instantiable native class, and classes of other helpers that have a constructor): a class
   base.
3. Anything else (a number, string, boolean, function, or a table with neither): an error.

At most one class base may be given; a second one is an error. With no base, the class is a
**root class**. Helper-class instances are tables whose lookup finds their class's `GetType`, so
passing an instance makes that instance the base (rule 1).

<a id="6-2"></a>
### 6.2 Results

`class` returns exactly **one** value: the new class table. Every call in the census assigns
the result to one name (§4.3) and none reads a second result, so Tessera exposes no
type-information table (Tessera rule; the original returns more values, K11). The class's
facts (its full name, §6.10, and its base or none) are fixed when `class` returns and are kept
in the registry of §6.13, out of reach of Lua code. Plugins reach the metatables of §6.4 with
`getmetatable`, as the census distributions do (§4.4).

Tessera's type objects answer `IsFinal` and `IsMixin` with false for every Tessera class
(§6.8, K13); there is no way to declare a final Tessera class. (A plugin that replaces those
functions in `Turbine.Type` changes the answers, as any change to a looked-up function would.)

<a id="6-3"></a>
### 6.3 Fields of a new class table

The class table is a new ordinary table. Its metatable is set first (§6.4) and it is entered in
the registry (§6.13). Then:

| Field | When it is stored on the class table | Value |
|---|---|---|
| `GetUniqueID` | only if lookup on the class table (that is, through the base) finds nothing | §6.9 |
| `ToString` | only if lookup finds nothing | §6.9 |
| `IsA` | only if lookup finds nothing | §6.7 |
| `GetType` | always | a function of no arguments that returns this class's type object (§6.8) |

Consequences: every native class has `IsA` (LPA §8.1), so a class with a native base (directly
or through other helper classes) has no own `IsA` and uses the native one (LPA §8.2). A root
class, and a class whose base is another helper's class on which lookup finds no `IsA`, gets
Tessera's `IsA` (§6.7); its subclasses inherit it. A Tessera class whose base is another
helper's class that provides `IsA`, `ToString` or `GetUniqueID` uses that helper's function
(for a copy of the original helper, its `ToString` then queries the Tessera type object with
`GetClass` and `GetName`, §6.8). `ToString` and `GetUniqueID` are stored on the first Tessera
class of a chain whose base does not provide them. Conversely, a copy of the original helper
declaring a subclass of a Tessera class finds these three by lookup and stores none of its own.
A plugin may overwrite any of these fields afterwards; lookup then finds the plugin's function.
Nothing else is stored on the class table.

<a id="6-4"></a>
### 6.4 Metatables

**Class metatable** (one per class, an ordinary table, no `__metatable` field):

- `__index` = the base table when a base was given; absent for a root class. Lookup on the class
  table therefore reaches the base's fields, and the base's own `__index` chain (LPA §8.1 for
  native classes).
- `__call` = the constructor of §6.5. Calling the class table creates an instance.
- `__tostring` = calls the class table's `ToString` method (found by lookup) and returns its
  result.

**Instance metatable** (one per class, **shared by every instance** of that class, created when
the class is declared, an ordinary table, no `__metatable` field):

- `__index` = the class table;
- `__tostring` = calls the instance's `ToString` method (lookup) and returns its result.

Plugins rely on the sharing (H10): reading `getmetatable` of an instance returns that shared
table; a write to it (for example a new `__index` function, or a `__newindex`, as 2 dependents
do in a constructor) changes lookup and assignment for every instance of the class, existing and
later ones. Copy routines that build a plain table and give it the same metatable produce a table
that behaves as an instance of the class for Lua lookup (a native object is not bound to the copy,
so native methods on it raise, LPA §8.2).

<a id="6-5"></a>
### 6.5 Creating an instance: constructor order

Calling a class table `C` with arguments `a1 … an`:

1. Create a new empty table `i` and set its metatable to `C`'s instance metatable.
2. Look up `Constructor` on `i` (so: on `C`, then through `C`'s base chain, or through whatever
   `__index` the shared instance metatable has at that moment).
3. If the result is a function, call it with `i` as the first argument followed by `a1 … an`,
   all passed through unchanged (trailing nils included). Its return values are ignored. If it
   is not a function (absent, or a table), no constructor runs.
4. Return `i` (exactly one value). An error raised inside the constructor propagates to the
   caller of `C(...)`; nothing is returned.

So **exactly one** constructor runs: the nearest one found by lookup. Base constructors run only
when that constructor calls them explicitly (for example `Base.Constructor(self, …)` or
`Turbine.UI.Window.Constructor(self)`), in the order the plugin code calls them. When a subclass
of a native class defines no `Constructor`, the nearest one is the native class's, which runs on
`i` and binds a native object to it (LPA §8.3 step 3). When a subclass defines a constructor that
does not call the native one, no native object is bound and native methods called on the
instance raise (LPA §8.2).

<a id="6-6"></a>
### 6.6 Inheritance and method lookup

Lookup on an instance goes: the instance's own fields, then (through the shared instance
metatable) the class table, then the class's base, and so on to the end of the chain. For a
subclass of a native class the chain continues into the native class tables (LPA §8.1, §8.3).
Methods and fields are never copied into subclasses: a function stored later on any class of
the chain (including a native class table, LPA §8.6) is found by every instance below it that
has no nearer field of the same name. Subclassing a helper class works the same way at any depth
(native class, helper class, helper class, …).

<a id="6-7"></a>
### 6.7 Tessera's `IsA`

For a class whose `IsA` is Tessera's (§6.3), `x:IsA(target)`, where `x` is normally a class
table or an instance found below that class (including classes and instances made by another
helper that inherit Tessera's `IsA`). The walk uses only type objects and their queries (§6.8),
so it treats Tessera's and other helpers' classes alike, as the original does:

1. **Start type.** Look up `GetType` on `x` and call it with no arguments; the result is the
   start type `S`. If lookup does not give a function, raise an error.
2. **Target type.**
   - If `target` is a table on which lookup finds a `GetType` function, call it with no
     arguments; the result is the target type `T`. Go to step 3.
   - Otherwise, if `target` is a table on which lookup finds a `Constructor` function (a native
     class, or a class of a helper without type objects): answer **false**, without reading
     any metatable. (The original first builds a type object of its own for such a target by
     following metatable `__index` links, and raises when a table on that path has no
     metatable, K17. Otherwise its walk can never meet that type; it then answers false, or
     raises if the walk reaches a base without a `GetType` function, K6.)
   - Anything else (nil, number, string, boolean, function, a table with neither) raises an
     error.
3. **Class target.** Query `T` with `IsClass`. If the answer is true, walk, starting with the
   current type `S`:
   1. If the current type is `T` itself (the same value; no `__eq` is involved), answer **true**.
   2. Query the current type with `GetBaseClass`. If the answer is nil, answer **false**.
   3. If the answer is a table on which lookup finds a `GetType` function, call it with no
      arguments; the result becomes the current type. If that value was already the current
      type earlier in this walk, answer **false** (Tessera rule, a cycle guard; K16).
      Otherwise continue with step 3.1.
   4. Otherwise (the base is a native class, or a class of a helper without type objects, or
      not a table) answer **false** (Tessera rule; the original raises an error here, K6).
4. **Mixin target.** Otherwise query `T` with `IsMixin`. If the answer is true: query `S` (not
   its bases) with `GetMixins`. If the answer is a table, go through its values in `pairs`
   order: answer **true** at the first value that equals `target` (stock `==`), or on which a
   method call `IsA(target)` (by lookup) returns a true value. Otherwise answer **false**. For a
   Tessera class `S` answers with an empty table, so the answer is false; for a class made by a
   copy of the original helper with mixins, the copy's mixin list decides. This reproduces the
   original.
5. Otherwise (`T` reports neither a class nor a mixin) raise an error.

Because `GetType` returns a class's existing type object while anything outside the registry
reaches it (§6.8, §6.13), and the walk itself holds `T` and every type it has met, the
comparison in step 3.1 is by identity, and an instance target resolves to its class. A walk
that passes through another helper's class continues as long as that helper's type objects
answer `GetBaseClass` and the bases provide `GetType`, so it can answer true for a target made
by that helper. Native-derived classes use the native `IsA` instead (§6.3, LPA §8.2).

<a id="6-8"></a>
### 6.8 Type objects and `Turbine.Type`

`C.GetType()` (dot or colon form; arguments are ignored) on a Tessera class returns `C`'s
**current** type object: the one an earlier call made, as long as it has not been collected;
otherwise a new one, which becomes the current one. A type object is collected only when
nothing outside the registry reaches it (§6.13), so every type object a plugin holds for `C`
is the same value as the result of any later call, and a held type object answers every method
below for as long as it is held, even when nothing else refers to `C` any more. Lookup makes
`i.GetType()` on an instance return its class's current type object.

**A query** of a type object `t` with a method name means: look up that name on `t` and call the
result with `t` as its only argument (a method call). If `t` is not a table, or lookup does not
give a function, the query raises an error; errors inside the method propagate. Tessera uses
queries for every type object, its own included, and needs only the answers below.

**Tessera's type objects.** Each is a new empty table. All type objects of an apartment share
one metatable, an ordinary table with only an `__index` field, whose value is `Turbine.Type`.
They answer exactly these seven methods, which are the only members of `Turbine.Type` (each
reads the registry entry of the type object's class; arguments after the first are ignored):

| Method | Answer for a Tessera class `C` | Interop path (naming note) |
|---|---|---|
| `IsClass()` | true | asked of bases and targets |
| `IsFinal()` | false | asked of a class base after `IsClass` |
| `IsMixin()` | false | asked by a copy's `mixin` function and of `IsA` targets that are not classes |
| `GetBaseClass()` | `C`'s base as given to `class` (a class table or an instance), or nil for a root class | `IsA` walks |
| `GetMixins()` | an empty table, the same one for every call on the same type object | `IsA` with a mixin target |
| `GetClass()` | `C` | a copy's default `ToString` |
| `GetName()` | the text after the **last** dot of `C`'s full name (§6.10), when the name contains a dot; otherwise nil | a copy's default `ToString` |

Called on a table that is not one of Tessera's type objects (for example
`Turbine.Type.GetName({})`), each of these raises an error.

**`Turbine.Type`** (Tessera rule) is a plain table holding the seven functions above and
nothing else. It is not a helper class and has no metatable: it is not callable (calling it
raises the stock Lua error), it has no `GetType`, `IsA`, `ToString`, `GetUniqueID` or
`Constructor`, and `tostring` gives the stock table text. Type objects likewise have no
`GetType`, `IsA`, `ToString` or `GetUniqueID`, and `tostring` of a type object gives the stock
table text (`table: ` followed by the address, PRT §4.4). Consequences:

- `class(Turbine.Type)` and `class(t)` for a type object `t` raise (§6.1 rule 3: no `GetType`, no
  `Constructor`). The original raises too, for another reason (its type class is final).
- `x:IsA(Turbine.Type)` and `x:IsA(t)` raise (§6.7 step 2). The original answers them (K12).
- A plugin may store new fields in `Turbine.Type`; lookup on every type object then finds them.

Other helpers' type objects are whatever their helper makes; Tessera only queries them.

<a id="6-9"></a>
### 6.9 `ToString`, `tostring`, `GetUniqueID`

- `GetUniqueID()` on a class table or instance returns the text that the stock `tostring` gives
  for that table **without** its metatable, minus the leading `table: ` prefix. In the original
  this is 16 upper-case hexadecimal digits (PRT §4.4, its row R5). The value is computed once per
  table and then cached (a weak-keyed cache, so it does not keep the table alive). The table's
  metatable is not changed in the process (Tessera rule; the original removes and restores it
  briefly).
- Tessera's default `ToString()` text is **Tessera's own** (Tessera rule). No plugin in the
  census reads or parses the text (§4.4: 0 uses of `ToString`; no code matches on it), so the
  original's wording is not reproduced (K7). For a value `v` (with ID = the text Tessera's
  `GetUniqueID` gives for `v`, computed directly rather than through lookup, and "name" = the
  class's full name of §6.10; if `v` is not a table, `ToString` and `GetUniqueID` raise an
  error):
  - `v` is a Tessera class (registry, §6.13): `class`, a space, the name and a space when the
    name is not empty, then `(`, the ID and `)`. Example: `class P.Q.R (`ID`)`; unnamed:
    `class (`ID`)`.
  - Otherwise, if lookup of `GetType` on `v` gives the `GetType` function of a Tessera class
    `C` (registry): `instance of`, a space, `C`'s name and a space when the name is not empty,
    then `(`, the ID and `)`. Unnamed: `instance (`ID`)`.
  - Otherwise (a class or instance of another helper that inherits Tessera's `ToString`, or a
    call on an unrelated table): `object (`ID`)`.

  A subclass table that inherits `ToString` reports its own name, because every Tessera class
  has its own `GetType` function.
- `tostring` of a class table or instance calls `ToString` through the metatables of §6.4, so a
  plugin's own `ToString` replaces the default text. If that function returns a non-string, the
  stock `tostring` raises (LUA).

<a id="6-10"></a>
### 6.10 The class name

When `class` runs, it reads the field `_` of the thread's current global table (`getfenv(0)`,
PRT §6.2). If that field is a table whose `Name` field is a string, that string is the class's
full name; otherwise the full name is empty. Consequences of the `import` rules (LPA §7.2,
"Run once" step 2 and "Running a disk chunk" steps 2 and 4; H15):

- A class declared by the top-level code of a file imported as `A.B.File`, **before** that code
  performs any import that runs a disk chunk, is named `A.B.File`, so `GetName()` gives `File`.
  **Every** class declared in that part of the file's top-level run gets the file's package
  name, whatever variable it is assigned to.
- An import that runs a disk chunk (in built-in mode also a built-in install, §5.3) ends by
  restoring the **real** global table as the thread's global table, whether its chunk succeeded
  or raised, and whatever its namespace. For the rest of the importing file's top-level run,
  `getfenv(0)` is the real global table, so a class declared there reads `_` of the real global
  table (below). This holds also when the imported package is in the same namespace as the
  file: the class does **not** take the other package's name. (The file's own functions keep
  the namespace table as their environment, LPA §7.2; only the thread's global table changes.)
- An import that runs no disk chunk (a name already recorded as run, a name with only a native
  part, or an import that fails before any chunk runs) leaves `getfenv(0)` unchanged. A class
  declared after it still reads `_` of the file's namespace table, which holds the file's own
  name unless that import set it (LPA §7.2 "Run once" step 2 sets `_` of the imported package's
  namespace, which can be the file's namespace).
- A class declared outside every file's top-level run (in an event handler, a command, or any
  function called after loading has ended) reads `_` of the real global table. A class declared
  by a function of one file that runs during another file's top-level run reads `_` of whatever
  the thread's global table is at that moment, which the rules above give for that other file.
- The real global table normally has no `_` field: the full name is then empty, `GetName()` is
  nil and `ToString` omits the name. It has one after an import of a package whose namespace is
  the top-level namespace (a one-component file package, LPA §7.2 "Namespace"), or after plugin
  code assigned it; a class declared with the real global table as `getfenv(0)` then takes that
  name.

The classes of the built-in package do not follow this rule; their names are fixed (§5.5).

<a id="6-11"></a>
### 6.11 Errors

`class` raises a Lua error, catchable with `pcall`, when: an argument is not acceptable
(§6.1 rule 3); two class bases are given; a base's type object does not answer `IsClass` with
true (this includes mixins, K5) or answers `IsFinal` with true; a query fails (§6.8). `IsA`
raises when `x` has no `GetType` function (§6.7 step 1), when the target is not acceptable
(step 2), when the target's type reports neither a class nor a mixin (step 5), and when a query
fails. The methods of §6.8 raise when called on a table that is not one of Tessera's type
objects; `ToString` and `GetUniqueID` raise on a value that is not a table. Message texts are
Tessera's own (`CLAUDE.md`); plugins observe only that an error is raised. Tessera's
helper never catches errors raised by plugin code or by other helpers (constructors, `GetType`
functions, type-object methods, `ToString`, `IsA` of mixins).

<a id="6-12"></a>
### 6.12 Subclassing a native class: summary

For `MyWindow = class(Turbine.UI.Window)` followed by a `MyWindow:Constructor()` that calls
`Turbine.UI.Window.Constructor(self)`:

- `getmetatable(MyWindow).__index` is `Turbine.UI.Window`, which meets LPA §8.3 item 1;
- `MyWindow()` makes a fresh table whose shared metatable's `__index` is `MyWindow` (LPA §8.3
  item 2), then runs `MyWindow.Constructor`, which binds the native window (item 3) without
  touching the metatable;
- native methods, events and overrides then behave as LPA §8.3 items 4 and 5 describe;
- `MyWindow` has its own `GetType`, `ToString` and `GetUniqueID` (a native `Window` has neither
  of the last two) and no own `IsA`;
- `tostring` of an instance gives the `ToString` text of §6.9.

<a id="6-13"></a>
### 6.13 The class registry: how Tessera recognises its own classes

Each apartment in built-in mode has one private **registry** (Tessera rule). Lua code cannot
reach it: it is not a field of any table, a metatable or an upvalue a plugin can read.

- Every class table that Tessera's `class` creates (§6.3), and every built-in class (§5.5), is
  entered with: its full name (§6.10 or §5.5), its base (the value given to `class`, or none),
  its `GetType` function and its current type object, if it has one (§6.8).
- Every Tessera type object (§6.8) is entered with the class it belongs to and its mixin list
  (the table its `GetMixins` returns).

Tessera uses the registry only to: answer the methods of §6.8 on its own type objects; choose the
form of its default `ToString` text (§6.9); and return a class's current type object from its
`GetType` calls. Everything else (base classification in §6.1, the `IsA` walk in §6.7) works
through lookup and type-object queries, the same way for Tessera's classes and other helpers'
classes. A table that only looks like a Tessera class (same fields, same metatable shape) is not
one.

<a id="6-13-1"></a>
#### 6.13.1 Lifetime (Tessera rule, H16)

Tessera runs stock Lua 5.1, whose weak tables have no ephemeron behaviour: an entry of a
weak-keyed table keeps its value alive, and so keeps its key alive whenever the value reaches
the key. The registry therefore cannot both keep nothing alive and let a type object answer for
a class nothing else holds. The rules are:

1. **A type object keeps its class alive.** While a Tessera type object is reachable from
   outside the registry, its class, that class's base (the value given to `class`, even after a
   plugin changed or removed the class's metatable) and its mixin list stay alive, and every
   method of §6.8 keeps answering on it as for a live class.
2. **A class keeps its base alive, not its type object.** A class that is reachable keeps its
   base and its `GetType` function alive, so a type object made later can still answer
   `GetBaseClass`. It does not keep its type object alive: a type object that nothing outside
   the registry reaches is collected like any other table, and the next `GetType` call on the
   class makes a new one (a new table with a new, empty mixin list). The registry keeps its
   entries for the class itself while the class is reachable.
3. **Otherwise the registry keeps nothing alive.** A class, its `GetType` function and its
   type object are collected, with their registry entries, once nothing outside the registry
   reaches any of them, whatever the class's fields, its base, its instances, its type object's
   fields or its mixin list refer to, with the one exception of rule 4. For example a base that
   holds its subclass (`D.child = C`), a type object field that holds its class
   (`C.GetType().owner = C`) or a mixin list that holds its class do not keep a class alive.
4. **Exception: a type object reachable from its own class.** When the type object is
   reachable from its class, from that class's base or from its mixin list (for example
   `C.T = C.GetType()`, a base field or an instance stored on the class that holds the type
   object, or the type object placed in its own mixin list), rules 1 and 2 together hold the
   class, its type object and everything they reach. Tessera does **not** guarantee that such a
   class is collected before its apartment closes; at apartment close everything of the
   apartment is released. Tests must not depend on whether it is collected earlier. No plugin of
   the census reads a type object at all (§4.4).

Observable consequences: two type objects a plugin holds at the same time for the same class are
always the same value (rules 1 and 2), and `IsA` walks compare by identity (§6.7). A type object
that was collected while nothing held it cannot be compared with its successor; only the text
`tostring` gives for the type object or its mixin list, and the contents of a weak table that
had it as a key, can show that a new one was made.

---

<a id="7"></a>
## 7. The other utilities

<a id="7-1"></a>
### 7.1 Fading windows: `Turbine.UI.Extensions.SimpleWindow` and `.Window`

Two Tessera classes (§6; names and text forms in §5.5). `SimpleWindow` has base
`Turbine.UI.Window`; `Window` has base `Turbine.UI.Lotro.GoldWindow`. They behave identically except for the base. Below, **B** is the
base class, and "B's method" means the function found by lookup on the base class table **at the
time of the call** (so a plugin's replacement on that native table, LPA §8.6, is the one called).
"The instance's method" means lookup on the instance (so subclass overrides are called). "The
game time" means the result of calling, with no arguments, the function found by lookup of
`Turbine.Engine.GetGameTime` in the apartment's global table at the time of the call.

Instance fields (interop: plugins can read them and can collide with them):

| Field | Meaning | Units |
|---|---|---|
| `fadeSpeed` | opacity change per second of game time | 1/s |
| `realOpacity` | the opacity the plugin last asked for | 0–1 |
| `targetOpacity` | where the current fade ends | 0–1 |
| `currentOpacity` | the opacity last applied by a fade step | 0–1 |
| `currentTime` | game time of the last fade step | seconds (`Turbine.Engine.GetGameTime`) |
| `hideOnClose` | a fade-out is in progress that ends by hiding the window | boolean |

Behaviour:

1. **Constructor** (the class's own `Constructor`): call B's `Constructor` on the instance, then
   the instance's `SetFadeSpeed(0.2)`, then the instance's `SetOpacity(1)`.
2. **`SetFadeSpeed(s)`**: set `fadeSpeed` to 1 divided by `s`, so a full fade from 0 to 1 takes `s`
   seconds. No check: `s` = 0 gives an infinite speed (stock division).
3. **`SetOpacity(v)`**, in this order:
   1. set `realOpacity` to `v`;
   2. set `currentTime` to the game time;
   3. set `currentOpacity` to the result of B's `GetOpacity` called with the instance;
   4. set `targetOpacity` to `v`;
   5. if `targetOpacity` differs from `currentOpacity`, call the instance's
      `SetWantsUpdates(true)`.

   An error in step 2 or 3 propagates and ends the call there, so the later fields keep their
   old values. For example, on an instance with no native object bound (LPA §8.2) B's
   `GetOpacity` raises: `realOpacity` and `currentTime` already hold the new values, while
   `currentOpacity` and `targetOpacity` keep the old ones. This reproduces the original (H7).
   The native opacity is **not** changed now; the fade does it (step 5).
4. **`SetVisible(v)`**:
   - If `v` is exactly `true`: call B's `SetOpacity(0)`, then B's `SetVisible(true)`, then the
     instance's `SetOpacity(realOpacity)`. The window appears at opacity 0 and fades in to the
     requested opacity. Calling it again while visible restarts the fade from 0.
   - For **any other value** (false, nil, 1, a string): set `hideOnClose` to true, remember
     `realOpacity`, call the instance's `SetOpacity(0)`, then put the remembered value back into
     `realOpacity`. The window stays shown and fades out; it is hidden at the end of the fade.
5. **`Update`** (a method of the class, so it is the instance's `Update` event handler, LPA §9.1,
   §9.4, called while wants-updates is on): let now be the game time, let the step be
   (now − `currentTime`) × `fadeSpeed`, and set `currentTime` to now.
   - If `currentOpacity` < `targetOpacity`: the new opacity is `currentOpacity` + step; if it
     exceeds the target, call the instance's `SetWantsUpdates(false)` and use the target.
   - Otherwise (`currentOpacity` ≥ `targetOpacity`): the new opacity is `currentOpacity` − step;
     if it falls below the target, call the instance's `SetWantsUpdates(false)`, use the target,
     and if `hideOnClose` is true, call B's `SetVisible(false)` and set `hideOnClose` to false.
   - Then set `currentOpacity` to the new opacity and call B's `SetOpacity` with it.
6. `GetOpacity`, `IsVisible` and every other method are B's, so `GetOpacity` reports the
   current (animated) native opacity, not the target, and `IsVisible` stays true during a
   fade-out.

Edge behaviour that follows (and that Tessera reproduces): a fade that lands exactly on the
target keeps updates on for one more tick, which then stops them (and, for a fade-out, hides the
window). `SetVisible(false)` while the native opacity already equals 0 does not switch updates
on, so the window is never hidden and `hideOnClose` stays true until a later fade-out ends. A
subclass or instance that defines its own `Update` replaces the fade.

<a id="7-2"></a>
### 7.2 The quickslot replacement (`Turbine.UI.Lotro.Quickslot`)

Importing `Turbine.UI.Lotro.Quickslot` (which `Turbine.UI.Extensions` does) replaces the field
`SetShortcut` of the native class table `Turbine.UI.Lotro.Quickslot` with a new function, once
per apartment (run-once record):

1. The value of `Turbine.UI.Lotro.Quickslot.SetShortcut` at import time (the native method, or
   an earlier replacement) is kept.
2. The new function, called as `q:SetShortcut(s)`, does three things in this order:
   1. calls `q`'s `IsVisible` method (lookup on `q`, so overrides apply) with `q` alone, and
      ignores its result. This call is observable: an override runs, and on a table with no
      native object bound to it the native `IsVisible` raises (LPA §8.2), which ends the call
      before the shortcut is set;
   2. calls the kept function with `q` and `s`;
   3. calls `q`'s `SetVisible` method (lookup on `q`) with the **same value `s`**.

   It returns nothing. Errors in any of the three calls propagate. This reproduces the original
   (H7).

All quickslots of the apartment are affected, including those of plugins that never imported
the package (LPA §8.6 item 3). How the native `SetVisible` treats a shortcut object or nil is
O1; in the original, the replacement therefore makes a quickslot visible or hidden according to
that rule each time a shortcut is set. In built-in mode, a plain `import "Turbine.UI.Lotro"`
does **not** install it (K2, O4).

<a id="7-3"></a>
### 7.3 `Turbine.Debug`

- `Turbine.Debug.Stack` is a new empty table with no metatable. Nothing reads it. (The same in
  the original.)
- `Turbine.Debug.Table` is a **plain table** with no metatable and one field, the function
  **`Dump(t, indent)`** (Tessera rule). It is not a class: not callable, no `GetType`, `IsA`,
  `ToString` or `GetUniqueID`, and `tostring` gives the stock table text. (In the original it
  is a non-callable helper class with those four members and a `__tostring`; K15.) `Dump`
  writes lines to chat with `Turbine.Shell.WriteLine`:
  1. `indent` defaults to two spaces when it is nil or false (as in the original).
  2. If `t` is nil, write one line: `indent` followed by `(nil)`; stop.
  3. Otherwise (a table; any other value raises, as the stock `pairs` does) mark `t` as seen,
     collect its keys, and sort them by the text `tostring` gives for each key (byte order;
     the order of keys with equal text is unspecified).
  4. For the k-th key in that order (k from 1), write one line: `indent`, the number k, `: `,
     and the key's text. The **value is not written**.
  5. If the key's value is a table not yet seen in this dump, dump it with `indent` plus two
     spaces before moving on to the next key.

  The output therefore lists each table's keys in sorted order with ordinal numbers, nested by
  indentation. A table reached twice is listed once. Cycles end.

<a id="7-4"></a>
### 7.4 `Turbine.Animation.LerpValue`

A root Tessera class (§6; name and text forms in §5.5) that moves a number toward a target at a
fixed rate. Instance fields
(interop): `value`, `target`, `rate`.

| Method | Behaviour |
|---|---|
| constructor | `value` = 0, `target` = 0, `rate` = 1 |
| `GetValue()` | `value` |
| `GetTargetValue()` | `target` |
| `SetValue(x)` | `target` = x; `value` is unchanged |
| `SetValueImmediate(x)` | `value` = `target` = x |
| `SetRate(r)` | `rate` = r |
| `GetRate()` | **not** the rate: returns the value that lookup finds under the name `rate` in the `Turbine.Animation` namespace table, which falls back to the apartment's global table through the metatable of §5.3; normally nil (the original's behaviour) |
| `NeedsUpdate()` | true when `value` ≠ `target` |
| `Update(d)` | if `value` < `target`: add `rate` × d, and if the result passes `target`, set it to `target`; if `value` > `target`: subtract `rate` × d, clamped the same way; if equal, nothing. No check of `d` or `rate`. |

<a id="7-5"></a>
### 7.5 `math.round`

`math.round(x)` returns `math.floor(x + 0.5)`, with stock arithmetic (so numeric strings are
converted and other values raise). Halves round up: 2.5 gives 3, −2.5 gives −2.

<a id="7-6"></a>
### 7.6 `Turbine.Table.AreEqual`

`Turbine.Table.AreEqual(a, b)` returns:

- **true** when both `a` and `b` are **plain tables**, whatever their contents: tables on which
  lookup finds neither a `GetType` function nor a `Constructor` function. Two plain tables with
  different contents also give true. This is the original's observable result: its element
  comparison never compares anything. One distribution compares data tables with it, and Tessera
  keeps the same result so that plugin sees what it saw;
- **false** in every other case: either value is not a table (so two equal numbers give false),
  or either is a helper-class instance or a class table, or one is a plain table and the other is
  not;
- for a table whose lookup finds a native `GetType` method (a shortcut object), the call of
  that method without an object raises (LPA §8.2), as in the original.

Tessera reads no metatable here and raises in no other case. The original raises in more cases,
before it compares anything: when either argument is a table with a `Constructor` function, no
`GetType` function and no metatable, and in the other cases of K17 and K18.

No other member of the original's `Turbine.Table` is provided (K1).

<a id="7-7"></a>
### 7.7 `Turbine.Log`

`Turbine.Log` is a **plain table** (Tessera rule) with no metatable and exactly two fields,
both functions, called in colon form (so the table itself is the first argument):

| Method | Behaviour |
|---|---|
| `Debug(m)` | write one chat line with `Turbine.Shell.WriteLine`, level label `debug` |
| `Information(m)` | write one chat line with `Turbine.Shell.WriteLine`, level label `info` |

Both always write. The original's log starts at its lowest level, at which every message is
written, and only a plugin call of its level setter changes that; no plugin in the census makes
that call, and Tessera's log has no level and no level setter. Arguments after `m` are ignored.

It is not a class instance: no `GetType`, `IsA`, `ToString` or `GetUniqueID`, and `tostring`
gives the stock table text. It has no name, no level and no other members. (In the original it
is an instance of a helper class, with a name, a format string, a level with methods to read and
set it, a method for each of two further message levels, and further members; K1, K15. Of all
these, the census reads only `Information`, in 3 distributions, and `Debug`, in 1, §4.5.)

The line text is Tessera's own (Tessera rule, K7): the local time of day as `hh:mm:ss` on a
24-hour clock (the computer's local clock), a space, the level label, a colon and a space, and
`tostring(m)`. Tessera may colour it with the markup of LPA §13.4. No plugin parses these lines
(§4.5). `Information` writes its line as `Debug` does. This matches TU; the 2010 archive raised
an error there (H2), so with SMP installed the 3 distributions' warning routine raises instead.

---

<a id="8"></a>
## 8. Known differences from the original with the user copy installed

All are Tessera rules (H11). In user-copy mode none applies, because the user's package runs
itself.

| # | Difference | Census basis |
|---|---|---|
| K1 | Not provided: `static_class`, `abstract_class`, `final_class`, `mixin`, `enum`, `quicksort`, `opairs`, `string.startsWith`, `string.endsWith`, `string.trim`; `Turbine.Compare`, `Event`, `Logger`, `LogLevel`, `LogManager`, `Service`, `ServiceContainer`, `Timer`, `TimerManager`, `Testing` and their import names; the import names `Turbine.Enum`, `Turbine.Mixin`, `Turbine.Quicksort`, `Turbine.String`; members of `Turbine.Table` other than `AreEqual`; members of `Turbine.Type` other than the seven of §6.8; members of `Turbine.Log` other than `Debug` and `Information` (§7.7: its level, the level getter and setter, the two further message levels and the rest) | §4.5: 0 distributions read them without bundling or defining them |
| K2 | A plain `import "Turbine.UI.Lotro"` does not install the quickslot replacement; only `Turbine.UI.Extensions` and `Turbine.UI.Lotro.Quickslot` do | O4 |
| K3 | No hidden timer control is created at apartment creation; no test command is registered | follows from K1 |
| K4 | Built-in classes are built by Tessera's mechanism even after a plugin replaced `class` (§5.5) | none observed |
| K5 | `class` rejects mixin arguments; it accepts a table with a `Constructor` function and no metatable (the original raises for the latter, while building its type, as in K17) | §4.3: no dependent passes either |
| K6 | Tessera's `IsA` answers false where the original raises: when its walk reaches a base on which lookup finds no `GetType` function (a native class, a class of a helper without type objects, or a value that is not a table), and when the target is a class without a type object whose type the original can build (K17), since its walk can then end the same way (§6.7 steps 2 and 3.4). Where the base's `GetType` gives a type object that answers `GetBaseClass`, both keep walking and give the same answer | none observed (§4.7) |
| K7 | Error messages, the `Turbine.Log` line text and the default `ToString` text (§6.9) are Tessera's own; a `%` in a log message is written as is (the original's substitution can raise) | §4.4: 0 uses of `ToString`; no plugin parses either text |
| K8 | `Turbine.UI.Lotro.Quickslot` imports the native `Turbine.UI.Lotro` itself; the original raises if that package was not imported before | — |
| K9 | `GetUniqueID` does not touch the metatable, so it also works on a table whose metatable is protected | — |
| K10 | `Turbine.Utils` is not provided. Its import fails, as it does with SMP or TU installed | O3 |
| K11 | `class` returns only the class table. The original also returns the class metatable, a type-information table and the instance metatable | §4.3: every call assigns one result |
| K12 | `Turbine.Type` is a plain table of seven functions, not a class (§6.8): it is not callable, and type objects have only those seven methods, no `GetType`, `IsA`, `ToString` or `GetUniqueID`, and the stock `tostring` text. `x:IsA(Turbine.Type)` and `x:IsA(typeObject)` raise. In the original `Turbine.Type` is a final helper class whose instances are the type objects, with further query methods and statics | §4.4: 0 uses of type objects; §4.7 |
| K13 | A Tessera class never reports final or mixin, and there is no way to declare one final (the original's type-information table can be changed, and it has `final_class`) | §4.3, §4.5 |
| K14 | Built-in class names are fixed (§5.5); in the original they follow the `_` rule and so can change with import order: with the user copy installed, `SimpleWindow` is unnamed when `Turbine.UI.Lotro` was not imported in that apartment before (and `Window` too when its file is imported alone), because the file's own import of that name runs the copy's `UI/Lotro/__init__.lua` first (§6.10, H15) | none observed |
| K15 | `Turbine.Debug.Table` and `Turbine.Log` are plain tables (§7.3, §7.7). In the original the first is a non-callable helper class and the second an instance of a helper class; both have `GetType`, `IsA`, `ToString`, `GetUniqueID` and a `__tostring`, and the log has a name, a format and further members | §4.4, §4.5: `Dump` used by 2, the log's `Information` by 3 and `Debug` by 1, nothing else |
| K16 | Tessera's `IsA` walk ends with false when a type comes round a second time; the original walks until the plugin's time limit (LPA §3.3) | none observed |
| K17 | `IsA` with a target that is a table on which lookup finds a `Constructor` function and no `GetType` function answers false (§6.7 step 2). The original first builds a type for such a target by following `__index` links: it reads the metatable's `__index` of the target and of each next table on which lookup finds a `Constructor` function and no `GetType` function, and it **raises** when one of these tables has no metatable. Examples: a table holding only a `Constructor` function; a table whose metatable's `__index` is such a table. It does the same, and raises, for a target table without a metatable on which lookup finds a field under a name the original reserves for native objects (not reproduced, naming note). The build stops without an error of its own at a link that is not a table, at a table without a `Constructor` function, or at a metatable without `__index`; at a table on which lookup finds a `GetType` function it calls that function instead (whose own errors propagate, as in Tessera). For a native class target the result depends on the root native class: the original raises when that class has a `Constructor` function and no metatable (O2) | §4.4: plugin code calls `IsA` only on native objects or inside the never-imported copy; none observed |
| K18 | `Turbine.Table.AreEqual` answers by the rules of §7.6 where the original raises: the original builds a type for **both** arguments, the first and then the second, before it compares anything, in the way K17 describes, so it raises when either argument is such a table, whatever the other argument is (also when the other is not a table). Examples: `AreEqual(G, {})`, `AreEqual({}, G)` and `AreEqual(1, G)` for a table `G` holding only a `Constructor` function | §4.5: the one caller compares two data tables of its own, which hold no `Constructor` function |

---

<a id="9"></a>
## 9. Invariants

Each can be checked with a synthetic plugin tree and Tessera's stub or real native classes.

1. **Mode.** With no `Turbine` entry in `Plugins/`, a new apartment's first plugin line sees
   `type(class) == "function"`, `math.round`, `Turbine.Table.AreEqual`, `Turbine.Log`,
   `Turbine.Debug.Table.Dump` and `Turbine.UI`. With a directory `Plugins/Turbine/` holding
   only an empty `__init__.lua`, it sees `class == nil`, and `import "Turbine.UI.Extensions"`
   fails.
2. **Run once.** In built-in mode, `import "Turbine.Class"` after creation returns nothing,
   and the global `class` is the same function value as before the import.
3. **Failing names.** In built-in mode, `pcall(import, "Turbine.Event")` and
   `pcall(import, "Turbine.Utils")` both return false.
4. **Results.** `select("#", class())` is 1; the value is a table `C`, and
   `getmetatable(C()) == getmetatable(C())` (one shared instance metatable, a different table from
   `getmetatable(C)`).
5. **Fields.** For a root class `C`, `rawget(C, k)` is a function for `k` in `GetType`, `IsA`,
   `ToString`, `GetUniqueID`. For `W = class(Turbine.UI.Window)`, `rawget(W, "IsA")` is nil and
   `W.IsA == Turbine.UI.Window.IsA`.
6. **Constructor.** With `B` defining a constructor that records its arguments and `C = class(B)`
   defining none, `C(5, 6)` runs B's constructor once with 5 and 6. When `C` defines its own
   constructor, B's does not run unless C's calls it.
7. **Native binding.** `class(Turbine.UI.Window)()` gives an object on which native methods
   work; a subclass whose constructor does not call the native one gives an object on which
   they raise.
8. **Base errors.** `class(3)`, `class("x")`, `class({})`, `class(print)`, `class(A, B)` (two
   classes), `class(Turbine.Type)`, `class(A.GetType())`, and `class(F)` where `F` is a table
   with a `Constructor` function and a `GetType` function returning a table whose `IsClass`
   answers true and `IsFinal` answers true, each raise. With `IsFinal` answering false, `class(F)`
   succeeds and `getmetatable(class(F)).__index == F`. With `IsClass` answering false, it raises.
   `class(nil)` and `class(nil, B)` succeed (no base, base B).
9. **IsA.** With `A = class()`, `B = class(A)`, `C = class()`: `B():IsA(A)` and `B:IsA(B)` are
   true, `B():IsA(C)` is false, `B():IsA(B())` is true, `A():IsA(B)` is false,
   `B():IsA(Turbine.UI.Control)` is false, and `B():IsA(nil)`, `B():IsA("x")`, `B():IsA({})`,
   `B():IsA(Turbine.Type)` and `B():IsA(A.GetType())` raise.
   **Other helpers.** Let `F` be a table with a `Constructor` function and a `GetType` function
   returning a fixed table `tf` whose `IsClass` answers true, `IsFinal` and `IsMixin` answer
   false and `GetBaseClass` answers nil; let `K = class(F)`. Then `rawget(K, "IsA")` is a
   function (F has none), `K():IsA(F)` is true (the walk meets `tf` through the base), and
   `K():IsA(A)` is false. For `G` a table with only a `Constructor` function and `L = class(G)`,
   `L():IsA(A)`, `L():IsA(G)` and `B():IsA(G)` are false and do not raise (K17). For a target `X` whose `GetType`
   returns a table whose `IsClass` answers false and `IsMixin` true, `B():IsA(X)` is false; if
   `IsMixin` also answers false, it raises.
10. **Names.** A file `P/Q/R.lua` that declares two classes at top level, imported as `P.Q.R`:
    both have `GetType():GetName() == "R"`. A class declared later inside a function has
    `GetName() == nil`. **After a nested import.** In built-in mode, with `P/Q/S.lua` (any
    content) next to it and `P/Q/R.lua` declaring `A1 = class()`, then `import "Turbine.UI"`,
    then `A2 = class()`, then `import "P.Q.S"`, then `A3 = class()`, imported as `P.Q.R` with no
    one-component package imported before: `A1` and `A2` have `GetName() == "R"` (the import of
    `Turbine.UI` runs no chunk), `A3` has `GetName() == nil`, and `getfenv(0) == _G` holds in
    `P/Q/R.lua` after the import of `P.Q.S`.
11. **Text.** For an instance `r` of that class, `tostring(r)` equals `instance of P.Q.R (` +
    `r:GetUniqueID()` + `)`, and `tostring(R)` equals `class P.Q.R (` + `R:GetUniqueID()` + `)`.
    For an instance `u` of an unnamed class, `tostring(u)` equals `instance (` +
    `u:GetUniqueID()` + `)`. After `function R:ToString() return "x" end`, `tostring(r)` is `x`.
12. **Shared metatable.** Setting `getmetatable(C()).__index` to a function that returns 42
    for every key makes `C().anything == 42` for a new and an earlier instance alike.
13. **Fade.** With game time controlled by the test: a new `SimpleWindow` keeps native opacity
    1 (the native default, LPA §13.19) and wants-updates off; after `SetVisible(true)` at time t, native opacity is 0, the window
    is visible and wants updates. With fade speed 5 per second, `Update` at t + 0.05 sets native
    opacity 0.25. After `SetVisible(false)` and enough updates, native visibility becomes false
    at the first tick whose step would pass below 0, and `realOpacity` keeps its earlier value.
    **Write order.** For a `SimpleWindow` `w` with B's `GetOpacity` replaced by a function that
    raises, and the four fields set to known values, `w:SetOpacity(0.3)` raises; afterwards `realOpacity` is 0.3,
    `currentTime` is the game time, and `currentOpacity` and `targetOpacity` are unchanged. With
    `Turbine.Engine.GetGameTime` replaced by a function that raises instead, `realOpacity` is the
    new value and the other three fields are unchanged.
14. **Quickslot.** After `import "Turbine.UI.Extensions"`, `q:SetShortcut(s)` calls
    `q:IsVisible()`, then the earlier `SetShortcut` with `s`, then `q:SetVisible(s)`, in that
    order; when `q.IsVisible` is set to a function that raises, the earlier `SetShortcut` is not
    called and the error reaches the caller. After only
    `import "Turbine.UI.Lotro"` in built-in mode, `SetShortcut` is still the native function.
15. **Dump.** `Dump({b = 1, a = {c = true}, [3] = "x"})` writes exactly 4 lines: indent 2
    spaces + `1: 3`, `2: a`, indent 4 spaces + `1: c`, indent 2 spaces + `3: b`.
    `Dump({z = 1}, false)` writes indent 2 spaces + `1: z`.
16. **Utilities.** `math.round(2.5) == 3`, `math.round(-2.5) == -2`;
    `Turbine.Table.AreEqual({1}, {2})` is true, `AreEqual({}, 1)` and `AreEqual(1, 1)` are false;
    for `G` a table holding only a `Constructor` function, `AreEqual(G, {})`, `AreEqual({}, G)`
    and `AreEqual(1, G)` are false and do not raise (K18);
    a `LerpValue` after `SetValue(10)`, `SetRate(4)`, `Update(1)` has value 4 and needs an
    update; after another `Update(2)` it has value 10 and needs none; `GetRate()` is nil, and
    after `Turbine.Animation.rate = 7` it is 7.
17. **Type objects.** For a Tessera class `C` with base `B`: `C.GetType() == C():GetType()`;
    the type object `t` has `t:IsClass() == true`, `t:IsFinal() == false`,
    `t:IsMixin() == false`, `t:GetClass() == C`, `t:GetBaseClass() == B`,
    `type(t:GetMixins()) == "table"` with `next(t:GetMixins()) == nil`, and
    `t:GetMixins() == t:GetMixins()`. `getmetatable(t).__index == Turbine.Type`,
    `tostring(t)` starts with `table: `, `next(t) == nil`, and `t.GetType == nil`.
    `Turbine.Type` has exactly the seven keys of §6.8, `pcall(Turbine.Type)` returns false,
    `getmetatable(Turbine.Type)` is nil, and `Turbine.Type.GetName({})` raises.
18. **Built-in names.** After `import "Turbine.UI.Extensions"` and `import "Turbine.Animation"`,
    with `S = Turbine.UI.Extensions.SimpleWindow`: `S.GetType():GetName() == "SimpleWindow"`,
    `S.GetType():GetBaseClass() == Turbine.UI.Window`, `tostring(S)` equals
    `class Turbine.UI.Extensions.SimpleWindow (` + `S:GetUniqueID()` + `)`, and `tostring(S())`
    equals `instance of Turbine.UI.Extensions.SimpleWindow (` + the instance's ID + `)`; likewise
    for `Window` and `LerpValue` with the values of §5.5. These hold even when a plugin replaced
    `class` before the import, or imported the package from a function called after load.
19. **Namespace fallback.** In built-in mode, at a plugin's first line, `Turbine.print == print`
    and `getmetatable(Turbine).__index == _G`. After `import "Turbine.UI.Extensions"`,
    `Turbine.UI.Extensions.print == print`; after `import "Turbine.UI.Lotro"` alone,
    `rawequal(getmetatable(Turbine.UI.Lotro), nil)` (no fallback). `getmetatable(Turbine.UI)` is
    nil in both cases.
20. **Plain tables.** `getmetatable(Turbine.Debug.Table)` and `getmetatable(Turbine.Log)` are nil;
    `tostring` of each starts with `table: `; `Turbine.Debug.Table.GetType` and
    `Turbine.Log.GetType` are nil. `Turbine.Log` has exactly the two keys `Debug` and
    `Information`; `Turbine.Log:Debug("m")` writes one line ending in `debug: m`, and
    `Turbine.Log:Information("m")` one line ending in `info: m`.

21. **Collection** (§6.13.1). Each check runs the named steps inside a function, returns, and
    then calls `collectgarbage("collect")` several times; "gone" means a weak-keyed probe table
    (`__mode = "k"`) that held the table as a key no longer has it.
    1. For `D = class()`, `C = class(D)` and an instance of each, with no further link, and
       separately with each of the links `D.child = C`, `C.GetType().owner = C`,
       `C.GetType():GetMixins()[1] = C`, and `x = D()`, `E = class(x)`, `x.sub = E`: `D`, `C`
       (and `x`, `E`) are gone once nothing outside the function holds them.
    2. For `t` the type object of `C = class(B)` with `B = class()`, returned from the function
       while `B` and `C` are not: `t:IsClass() == true`, `rawequal(t:GetClass().GetType(), t)`,
       `t:GetBaseClass().GetType():IsClass() == true`, `t:GetMixins() == t:GetMixins()`, and
       `t:GetClass()():IsA(t:GetBaseClass())` is true. When the function also ran
       `setmetatable(C, nil)` before returning, the same answers hold except the last (`C` is
       then not callable); `GetBaseClass` still gives the base class. After
       `t = nil` and the collections, `C` and `B` are gone.
    3. With `C` held and a weak-keyed probe holding `C.GetType()` only as a key: after the
       collections the probe is empty, and `C.GetType():GetClass() == C` still holds. With
       `t = C.GetType()` held instead: after the collections `rawequal(C.GetType(), t)`.

---

<a id="10"></a>
## 10. Worked examples (behaviour)

1. **A plugin that only needs `class`.** Its main file declares a subclass of
   `Turbine.UI.Lotro.Window` at top level and creates one. The user has no `Plugins/Turbine/`.
   → The apartment is in built-in mode. `class` exists from creation. The subclass's
   metatable `__index` is the native class, the instance is bound by the native constructor its
   own constructor calls, and `tostring(window)` gives `instance of Author.Plugin.Main (…)` when
   the file was imported as `Author.Plugin.Main` and declares the subclass before any of its
   imports runs a disk chunk (imports of native-only names such as `Turbine.UI.Lotro` do not
   count in built-in mode). If the file first imports a file of its own plugin, the subclass is
   unnamed and the text is `instance (…)` (§6.10).
2. **A plugin using the fading window.** It imports `Turbine.UI.Extensions`, creates
   `Turbine.UI.Extensions.SimpleWindow()`, sets size and position, and calls `SetVisible(true)`.
   → `Turbine.UI` and `Turbine.UI.Lotro` are present after the import. The window appears at
   opacity 0 and reaches opacity 1 after about 0.2 s of game time, after which updates stop.
   `SetVisible(false)` fades it out over about 0.2 s (scaled by the opacity it had) and then
   hides it. Every quickslot in the apartment now also gets `SetVisible(shortcut)` after each
   `SetShortcut`.
3. **The same plugin for a user who installed the package.** `Plugins/Turbine/` exists.
   → User-copy mode: the user's files run exactly as in the original (§5.2), including their
   timer control and the quickslot replacement on every `import "Turbine.UI.Lotro"`. The
   built-in package does nothing in this apartment.
4. **A partial user copy.** `Plugins/Turbine/` holds only `UI/Extensions/` (no `__init__.lua`).
   → User-copy mode. `class` is nil at the first plugin line (no root runs), as in the original;
   a plugin that calls `class` fails to load. Tessera does not mix in built-in names.
5. **A plugin that brings its own helper.** It assigns its own function to the global `class`
   at load and later subclasses `Turbine.UI.Extensions.SimpleWindow` with it. → Its function
   is used from then on for every later declaration in that apartment, including other plugins'
   (as in the original). `SimpleWindow` itself was made by Tessera's mechanism (§5.5). The
   plugin's helper sees a table with a `Constructor` function and a `GetType` function whose
   type object answers `IsClass()` true and `IsFinal()` false. If that helper is a copy of the
   original (§4.7), its subclass finds `IsA` (the native one, through `Turbine.UI.Window`),
   `ToString` and `GetUniqueID` by lookup and stores none of its own, so `tostring` of an
   instance gives Tessera's `object (…)` form (§6.9). The copy's own `IsA` (on its root classes)
   asks `IsClass` of a Tessera target's type object, and walks Tessera type objects with
   `GetBaseClass` (and, for a mixin target, `GetMixins`) when a Tessera class lies below one of
   its root classes (§4.7, §6.8).
6. **A plugin that imports `Turbine.Utils`.** → The import raises the catchable error of
   LPA §7.1. The plugin is not protected, so its load fails and the failure is reported
   (PRT §11.5), as it would be with SMP or TU installed.
7. **Equality helper.** A plugin compares two different item-stat tables with
   `Turbine.Table.AreEqual`. → true, so the plugin keeps the value it would otherwise clear, as
   it did in the original.

---

<a id="11"></a>
## 11. Open questions

> **OPEN (O1, for LPA):** how the native `SetVisible` treats an argument that is not a boolean
> (a `Shortcut` object, or nil), and whether the native `SetShortcut` itself changes a
> quickslot's visibility in the current client. The replacement of §7.2 passes the shortcut
> value to `SetVisible`. 15 dependent distributions call `SetShortcut` after importing
> `Turbine.UI.Extensions`, so the visible state of their quickslots after each call depends on
> this rule.

> **OPEN (O2, for LPA):** LPA §16 lists only `IsA` as a member of `Turbine.Object`, but 13
> distributions declare a class with base `Turbine.Object`, and several call
> `Turbine.Object.Constructor(self)`. Tessera's `class` accepts that base only if lookup finds a
> `Constructor` function on it (§6.1 rule 2). LPA should state whether `Turbine.Object` has a
> constructor and is callable. (The original's own helper also needs `Turbine.Object` to have a
> metatable for this base to work.)

> **OPEN (O3, owner):** whether the built-in package should also answer `import "Turbine.Utils"`
> with an empty package. Neither SMP nor TU has it, so 2 distributions fail in the original
> unless the user has some stray file of that name. Until decided: not provided (K10).

> **OPEN (O4, owner):** whether built-in mode should install the quickslot replacement on every
> `import "Turbine.UI.Lotro"`, as the original does when the user copy is installed (§5.6).
> That would change quickslot behaviour for plugins that never asked for the package. Until
> decided: only on the imports of §7.2 (K2).

> **OPEN (O6, implementation M5, clean-room implementer 2026-10-08):** the spec does not say
> what happens when a native class the built-in package builds on is missing when its name is
> installed (`Turbine.UI.Window` for `SimpleWindow`, `Turbine.UI.Lotro.GoldWindow` for
> `Window`, `Turbine.UI.Lotro.Quickslot` for §7.2), for example while the UI layer is
> incomplete. The implementation raises a catchable error from that install, so the import
> fails (for `Turbine.UI.Extensions`, the whole folder import) and the run-once record is
> undone, as for a disk file that indexes a missing table.

---

<a id="12"></a>
## 12. Notes for other specs

| Spec | Note |
|---|---|
| LPA §8.3 | "Tessera does **not** provide `class`" is superseded by decision 0016: in built-in mode Tessera provides it (§5.4 here); the host requirements of §8.3 stay as stated. |
| LPA §4.2, §7.4 | Unchanged: Tessera still ships none of the publisher's files. Add a pointer to this spec for the built-in stand-in and the user-copy test (§5.1). |
| LPA §16 | O2 (`Turbine.Object` constructor). O1 (non-boolean `SetVisible`). |
| PRT §5.1 step 5, §5.2 | In built-in mode, step 5 also installs the built-in root (§5.4), so the first-line globals include `class`, and `math.round` is present. In user-copy mode, the user's root decides, as now. |
| plugin-descriptor §7.2 | The built-in names stand in for the disk part only in built-in mode (§5.3). The precedence rule there is unchanged. TU's dummy descriptor (root element not `Plugin`) is rejected by its rule D4, which is the intended outcome. |
| PAU §6.5 | Its OPEN (owner decision) is answered by decision 0016 and this spec. Its §9.1 rule 3 (exit-criterion candidates must not depend on the package) may be relaxed once this spec is Accepted and implemented. |

---

<a id="change-log"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First version (Review), for decision 0016. Census of the sample-package names and import paths used by the 143 distributions (§4), the user-copy test and built-in installation (§5), the behaviour of `class` (§6) and of the fading windows, quickslot replacement, table dump, interpolator, `math.round`, `Turbine.Table.AreEqual` and `Turbine.Log` (§7), known differences (§8), 16 invariants, 7 worked examples, 4 OPEN items. |
| 2026-10-06 | Revision for review items R1–R5 (status stays Review). **R1:** naming note now lists each kept identifier with its interop path; the type-information table and its field names, the type-object methods without a path, the original log name and the original `ToString` wording are dropped: `class` returns one value (§6.2, K11), type objects answer seven methods (§6.8), Tessera's own `ToString` text (§6.9, K7) and log line (§7.7). New §4.7 counts the copies of the original helper that plugins carry (H12). **R2:** fixed full names, base, `GetName` and text of the built-in classes (§5.5, K14, H13); built-in installs run the namespace steps of LPA §7.2, so `Turbine`, `Turbine.Debug`, `Turbine.Animation`, `Turbine.UI.Extensions` and (after the quickslot import) `Turbine.UI.Lotro` fall back to the globals (§5.3). **R3:** `Turbine.Type` is a plain, non-callable table of the seven methods; type objects have stock `tostring`; consequences for `class` and `IsA` (§6.8, K12). **R4:** the quickslot replacement queries `IsVisible` first (§7.2); `Turbine.Debug.Table` and `Turbine.Log` are plain tables (§7.3, §7.7, K15); `IsA` with a mixin target reproduces the original (§6.7 step 4); `Dump` treats indent `false` as absent (§7.3). **R5:** §6.7 rewritten as a walk over type-object queries that continues through other helpers' classes, K6 reworded, cycle guard K16; §3.1 terms split into helper class, Tessera class, other helper's class and type object; new §6.13 class registry (H14). Invariants 4, 8–11, 14–16 revised, 17–20 added; worked examples 1 and 5 revised; H5 and the RUN source widened. |
| 2026-10-06 | Second revision, for review items S1–S4 (status stays Review). **S1:** a recount found `Turbine.Log` read by 3 distributions, not 1: `Information` in all 3 (a decompressor's warning routine) and `Debug` in 1 (§4.5, §4.6, H4). `Turbine.Log` now has exactly `Debug` and `Information`, which always write; its level, level getter and setter and the two other message levels are dropped (§7.7, K1, K15, invariant 20, naming note). **S2:** new naming-note row for the original's names that Tessera does not provide, which the spec names only to give census counts and to state their absence; `Turbine.Utils` moved there from the list of reproduced names. **S3:** new K17 (`IsA`) and K18 (`AreEqual`) describe where the original raises while building a type for a table with a `Constructor` function and no metatable; Tessera keeps answering false, without reading metatables (§6.7 step 2, §7.6, K5, K6, invariants 9 and 16). **S4:** §7.1 step 3 fixes the order of the writes of `SetOpacity` and the state left when the game-time read or B's `GetOpacity` raises; "the game time" defined; invariant 13 extended. H7, H8 and the RUN source widened. |
| 2026-10-08 | Third revision, for review items T1–T2 (status stays Review). **T1** (implementer OPEN of §6.13): ruling for Tessera's Lua 5.1 runtime, which has no ephemeron tables. New §6.13.1: a type object keeps its class, the class's base and its mixin list alive; a class keeps its base but not its type object, so an unheld type object is collected and the next `GetType` makes a new one; otherwise the registry keeps nothing alive; a class whose type object is reachable from the class, its base or its mixin list may stay until the apartment closes. §6.8 now says "current type object" instead of "the same type object on every call", §6.7 states why identity still holds, §6.13 entries restated and its OPEN removed. New invariant 21 (collection); provenance H16. **T2** (O5): LPA §7.2 step 4 (BIN, its row P8) restores the real global table after every import that runs a disk chunk. §6.10 rewritten: a class declared after such an import in a file's top-level run reads `_` of the real global table and is normally unnamed (it does not take the other package's name); imports that run no chunk leave `getfenv(0)` alone; the real global table has `_` only after a one-component file package or a plugin assignment. §5.3 says when the thread's global table is `T` during a built-in install. §5.5 and K14 add the consequence for the original's `SimpleWindow` and `Window` names; invariant 10 and worked example 1 extended; O5 removed; RUN source extended by a fourth run whose stub follows LPA §7.2 step 4, provenance H15. O6 unchanged. |
