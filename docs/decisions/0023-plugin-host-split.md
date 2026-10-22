<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0023 — Plugin host split: a plugin-free `tessera-script` core, `tessera-plugins` for the compatible surface, and the `plugins` → `ui` allowance

**Status:** Accepted, 2026-10-06 (M5 plan). Part 3 records the owner's answer "yes" to
owner question 1 of [0015](0015-ui-rendering.md) (the `plugins` → `ui` sibling allowance).
The owner may revise that allowance in a later record; Consequences says what would change.

**How it was written.** Clean-room architect work from `docs/` only: ARCHITECTURE §3, §3.1
and §18; ROADMAP M5; decisions 0014, 0015 and 0016; `plugin-runtime.md`,
`lua-plugin-api.md` (LPA); the module list of `crates/script`; and the proposed decision on
extended scripting (`docs/decisions/proposed-extended-scripting.md`, M5.5, proposed and not
accepted), its §1.1 and its owner question 2. No dirty-room material was read.

## Context

- **One crate holds two things today.** `crates/script` (about 5,400 lines) holds the
  sandboxed Lua 5.1 runtime and the whole compatible plugin surface: `Turbine` and its
  packages, `import`, the `NativeApi` object model, `.plugin` descriptors, the
  `PluginManager`, the command table, the chat-line queue and `.plugindata` files. ROADMAP M5
  and ARCHITECTURE §3.1 already list `tessera-plugins` (L4, M5) for the plugin surface, and
  `crates/plugins` exists as an empty package. M5 has to split the code.
- **The proposed extended-scripting record asks for a particular split.** It plans a second
  kind of Lua apartment (M5.5) on the same runtime. Its §1.1 sizes the split: about 2,000
  lines stay in a core (the sandbox install, the instruction budget, the pattern matcher, the
  finalizer proxy and `pcall` guard, text-only compilation, error description, the reader
  part of the plugin-data module), and about 3,400 lines move (`turbine`, `manager`,
  `descriptor`, `paths`, `api`, and the plugin parts of `runtime` and `host`). Its owner
  question 2 and one of its Open items ask that M5 leave a core with no `Turbine`, `import`
  or chat state, so that M5.5 does not redo the split.
- **Plugin windows are `tessera-ui` elements.** 0015 (Consequences, "Crate boundaries")
  notes that the `Turbine.UI` bindings in `tessera-plugins` create and drive `tessera-ui`
  elements (LPA §13.17), and that ARCHITECTURE §3 has no `plugins` → `ui` allowance. It
  proposed one, named a trait alternative, and left the choice to the accepting session
  (its owner question 1). The owner answered that question "yes".
- **`input` and `ui` stay independent** (0014, Decision intro): the client wires them through
  the mapper's synchronous sink. Nothing here changes that.

## Decision

### 1. `tessera-script` is a plugin-free Lua core

`tessera-script` keeps only what any sandboxed Lua 5.1 apartment needs, whatever API it
exposes:

| Part | What it is |
|---|---|
| Apartment profile and state creation | A profile value says which base functions exist, the limits, how chunks are named and which random generator `math.random` uses. The core creates a state from a profile with the allow-listed libraries (ARCHITECTURE §18.1). The core builder takes **no** plugins root and no shared plugin state, and runs neither `import` nor any API install |
| `Limits` | Memory cap, per-call instruction budget, depth limits (`plugin-runtime.md` §9–§10) |
| Protected calls | Every host-to-Lua call protected; error description and tracebacks (`plugin-runtime.md` §11) |
| The `pcall` guard | Budget errors cannot be swallowed by protected calls in Lua |
| The instruction `Budget` and its hook | Per-call instruction counting (`plugin-runtime.md` §10.1) |
| Text-only compilation | Compiles host-supplied source text; binary chunks are refused |
| The bounded pattern matcher | Lua pattern matching with bounded work |
| The metered finalizer proxy | The core's metered `newproxy` view (`plugin-runtime.md` §9.3) |
| Collector stepping | The per-tick `LUA_GCSTEP` call (`plugin-runtime.md` §9.2); the caller decides when |
| The neutral Lua-table data reader | The no-execution reader of a Lua table constructor of plain values, with its value model and bounds (`ParseLimits`) |
| A small core host trait | Script log and error reports only |

**What the core does not hold.** No `Turbine` or any other plugin API name, no `import`, no
plugins root and no virtual file system of the plugin tree, no shared plugin state (the
apartment table, the random generator that all plugin apartments share), no command table, no
chat-line or request queue, and no `.plugindata` file naming, key grammar (LPA §10.2), comma
decimal rule (LPA §3.4) or data scopes. Its public API names no plugin concept. Its only
dependencies are the Lua binding, `thiserror` and `tracing`; it depends on no Tessera crate.

**`HostApi` splits in two.** A small core trait (script log, error reports) stays in
`tessera-script`. The plugin `HostApi` (chat output, plugin-data directories, engine date and
time, language and locale, built-in command names, native installs) moves to
`tessera-plugins` and extends the core trait. The names are the implementer's choice.

### 2. `tessera-plugins` takes the compatible plugin surface

`tessera-plugins` (L4) is built on the core and holds everything plugin-specific:

- `Turbine` and its packages, `import`, `NativeApi` and the LPA §8 object model;
- `.plugin` descriptors (`plugin-descriptor.md`) and the `PluginManager` (discovery,
  apartments, load and unload, the `/plugins` commands' logic);
- the plugin profile (including the generator of `plugin-runtime.md` §4.5 that every plugin
  apartment shares) and the plugin `HostApi`;
- the command table, the request queue, the chat-line queue and the rest of the step-7
  deferred work (0014 D8);
- the `.plugindata` wrapper over the core's neutral reader: file naming, the LPA §10.2 key
  grammar, the LPA §3.4 comma decimal separator and the three scopes;
- the virtual file system index of the whole `Plugins/` tree and the header probe of
  `SetBackground(path)` (0015 §4.4);
- the clean-room class helper ([0016](0016-plugin-class-helper.md),
  `plugin-class-helper.md`);
- the `Turbine.UI` bindings, which create and drive `tessera-ui` elements (part 3).

It does no settings I/O of its own: the client hands it the plugins root and the data
directories, and feeds it the session's events (chat lines, gameplay views), so it needs no
allowance to `settings`, `net` or `input`.

**What moves with the code.** The plugin behaviour tests and the sandbox escape tests (M5
exit criterion 5) run plugins through `Turbine`, so they move to `tessera-plugins` and must
pass unchanged. The core gets its own tests: a state built from a bare profile has no
`import`, `Turbine` or `plugin` global, and the budget, guard, pattern and reader tests run
without any plugin code. The client's plugin glue depends on `tessera-plugins`. The fuzz
targets follow their code: the descriptor and `.plugindata` targets drive `tessera-plugins`,
the pattern target and a reader target drive `tessera-script`. The XML parser dependency
(descriptors) moves to `tessera-plugins`.

### 3. The `plugins` → `ui` allowance (0015 owner question 1)

- **Allowance:** `plugins` → `ui`, a sibling allowance at L4, **one-way**. `ui` never depends
  on `plugins` or `script`, and its API has no Lua types.
- **Reason:** the plugin API's UI classes are thin bindings over `tessera-ui` elements
  (LPA §13.17). Painting, damage, paint jobs and the composition list stay in `ui` (0015).
  Plugin handlers are reached through an event-listener interface that `tessera-ui` defines
  and `tessera-plugins` implements, so `ui` routes input to plugin controls without naming
  plugins.
- **Rejected alternative:** `tessera-ui` defines a trait for every element operation the
  bindings need, and the client wires `plugins` to `ui` through it, as it wires `input` to
  `ui` (0014). Rejected because the trait would mirror the element API of every `Turbine.UI`
  class one for one, so the two would always change together. `tessera-plugins` is the only
  user of those operations, and the client glue would add a layer to each call without
  isolating anything. `input` and `ui` are different: neither uses the other's API; they
  meet only at the sink.
- **No other allowance.** This record adds no `plugins` → `input`, `net` or `settings`
  allowance and no `ui` → anything allowance.

### 4. Relation to the extended-scripting proposal

M5's split meets the "plugin-free core" condition that the proposal asks for (its owner
question 2, second half, and its Open item on the split): the core holds no `Turbine`,
`import`, shared plugin state, plugin-shaped `HostApi` or chat state, and its builder takes no
plugins root. If the owner accepts that proposal, its slice 1a does not have to finish the
split first.

This record does **not** accept the proposal or any part of it. It adds no `ext` crate, no
`ext` → `script` allowance, no patched Lua build, no wall-clock deadline or charge API and no
per-apartment generator for other profiles. It only builds what M5 needs anyway, in a shape
that keeps those later additions additive: the profile is a value the core reads, and the
budget hook is one place where further checks could be added.

## Consequences

- **ARCHITECTURE:** §3 gains the allowance (diagram arrow `plugins*(→script, ui)`) and the
  rule that `input` and `ui` stay independent; §3.1's rows for `tessera-ui`, `tessera-script`
  and `tessera-plugins` follow this split; §18.1 and §18.2 describe the core and the plugin
  host.
- **ROADMAP M5:** the `tessera-plugins` deliverable notes that the split leaves a plugin-free
  core.
- **CLAUDE.md** Layout: lines for `crates/ui` and `crates/plugins`; the `crates/script` line
  becomes the core.
- **Checks for the implementation.** `cargo tree -p tessera-script` lists no Tessera crate;
  `cargo tree -p tessera-ui` lists neither `tessera-script` nor `tessera-plugins`; the core's
  bare-profile test passes; the moved escape and behaviour tests pass unchanged.
- **If the owner revises the allowance** to the trait alternative, only `tessera-plugins`'
  dependency list, a trait in `tessera-ui` and the client glue change. The core split of
  parts 1 and 2 is independent of that choice.
- **Specs:** none changes. The plugin specs describe behaviour, not crates; where one names
  `crates/script` as the home of plugin-surface code, read `crates/plugins`.

## Implementation notes (M5, 2026-10-07)

Names chosen by the implementation (part 1 left them open):

- **Neutral module names (exit verification, 2026-10-08):** the module that builds the
  API's root namespace is `tessera_plugins::root_ns` (it was named after the namespace);
  its handle field and accessor are `Handles::ns_root` and `NativeApi::ns_root()`, and the
  user-copy test is `PluginFileIndex::has_user_api_copy`. Project naming rules (`CLAUDE.md`)
  keep module, type and binding names neutral outside the plugin subsystem's tests; the
  original names stay as string data. `tools/content-allowlist.txt` allows them only in
  `crates/plugins/tests/`.

- **Core (`tessera-script`):** `Profile` (state name, `Limits`, `BaseFunctions`, chunk
  prefix, `RandomSource`, `Conversions`) and `ScriptState::new(profile, host)`; the core host
  trait `CoreHost` (`script_log`, `report_error` with an `ErrorReport` whose `source` the
  embedding fills, for plugins the plugin name); `native_fn` (stock-library error style) and
  `api_fn` (no position prefix, `plugin-runtime.md` §11.2); `call_protected` and
  `ScriptState::call` (the host-call trampoline of §11.1); `compile_source`;
  `ScriptState::gc_step`; the reader `data::{parse, parse_with, NumeralHook, DataValue,
  DataKey, ParseLimits}`; `CrtRandom`/`SharedRandom` (the generator type; the shared
  instance lives in the plugin host). `cargo tree -p tessera-script` lists no Tessera
  crate.
- **Plugin host (`tessera-plugins`):** `HostApi: CoreHost`; `PluginManager` with
  `ManagerConfig`; the `.plugindata` wrapper `plugindata` (Tessera key rule, writer,
  `CommaDecimals` hook over the core reader); `vfs` (whole-tree index, header probe);
  `packages::BuiltinPackages` (the hook the class helper fills; `ManagerConfig::builtins`);
  `gameplay::LocalPlayerSource` (the local-player views the client feeds from the
  sandbox). The behaviour tests and the escape suite moved with the code
  (`crates/plugins/tests/`); the core has its own tests (`crates/script/tests/core.rs`).
- Fuzz targets: `script_pattern` and `script_reader` drive the core; `plugins_descriptor`,
  `plugins_plugindata` and `plugins_commands` drive the plugin host.
- **"The moved escape and behaviour tests pass unchanged" (checks above), as met
  (2026-10-08).** Besides import paths and harness set-up (`ManagerConfig::new`, index and
  writes on the calling thread, one pump running the whole queue), the moved tests in
  `crates/plugins/tests/{sandbox,plugins,patterns}.rs` changed these expectations, each
  because an Accepted spec revision made after the tests were written requires it; no
  escape check was weakened:
  - `load(bytecode string)` is checked through `pcall`: Lua 5.1's `load` takes a reader
    function only and raises on a string (stock 5.1 functions, `lua-plugin-api.md` §3.1,
    `plugin-runtime.md` §6.2); `loadstring(bytecode)` and `load(bytecode reader)` are still
    refused as before.
  - A failed load keeps the shell commands, handlers and apartment it created and only loses
    its `Plugins[name]` entry (test renamed `failed_load_keeps_its_registrations_but_not_its_
    plugin_entry`): no rollback, the apartment stays (`plugin-runtime.md` §11.5).
  - API errors carry no `chunk:line:` prefix (`plugin-runtime.md` §11.2).
  - Error values that are not strings or numbers are reported as the empty message
    (`plugin-runtime.md` §11.1).
  - `LoadPlugin` of a plugin that is not available queues nothing and reports no failure;
    loads run as step-7 items (`lua-plugin-api.md` §6.2, `plugin-runtime.md` §8.2, §8.4).
  - The chat-echo count is 9, not 5: lines carry the `Received` skip sets of
    `plugin-runtime.md` §8.4 (`lua-plugin-api.md` §11.3 OPEN, invariant 65).
  - Plugin-data file text (`return \r\n` header, no final line break) and `onDone`
    arguments (`true, ""`) follow `lua-plugin-api.md` §10.4 and §10.1.
  - The arguments of a command typed without any are the empty string
    (`lua-plugin-api.md` §11.2).
  - Not spec-driven, Tessera's own wording (`lua-plugin-api.md` §6.4): the "Loaded plugin"
    line names the plugin as its descriptor writes it, not lower-cased.
