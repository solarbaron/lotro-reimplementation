<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Lua plugin API (compatible plugin surface)

| Field | Value |
|---|---|
| Status | **Accepted** (2026-10-09: the P1 revision (§15.1, provenance P81–P83, §18 checks 66–69) with its review fixes passed an independent re-review; implementers may build §15.1 and re-check the `Turbine.Gameplay` `Backpack`, `Equipment`, `Item` and `ItemInfo` views in `crates/plugins`, see the last two change-log rows). Every section other than §15 (enum paragraph and OPEN), §15.1, §18 checks 66–69, §19 and §20 is unchanged from the text Accepted on 2026-10-06 (decision-alignment revision for decisions 0014/0015); section numbers and anchors are unchanged |
| Last updated | 2026-10-09 |
| Author role | Spec writer (dirty room), AI-assisted |
| Reviewer | Claude review session (spec-writer agent), a different session from the writer |
| Review record | **Re-review of the P1 revision and its review fixes (diff against `main`: header, P41, P81–P83, §15 enum paragraph and OPEN, §15.1, §18 lead-in and checks 66–69, §19, §20), 2026-10-09, independent review session that wrote no part of the revision or its fixes and did not do the first review: passed, Accepted.** Checked against CLEAN_ROOM.md §5.3: every new statement is a value, constraint, mapping or behaviour; no code or pseudo-code (the argument lists in braces name event-table fields, as §9.3 does); API, mapper and property labels are interoperability identifiers; no game text (the literal item text is scenario input), assets, keys, hostnames or traffic; no closed gate touched (no M4 method used for this revision, no official-server traffic). Every Tessera rule of §15.1 is under P83 (**Tessera rule**, DEC); the API facts are P81 (Reported) and P82 (Reported + Confirmed). The four blocking items of the first review are answered: (1) §15 enum paragraph, P41, the §15 OPEN and its §19 line point to or follow P82; (2) the §15.1 OPEN is in §19; OPEN markers before §19 counted 56 on `main`, 57 after the P1 revision and 57 now, as the change log says; (3) P83 and the §18 lead-in for 66–69; (4) the two getter tables cover all 13 `Item` and 15 `ItemInfo` getters of §16 with present, absent and literal-item values. Spot-checks (read-only, 2026-10-09): mappers 0x230000EE (base 0x23000101), 0x230000FA, 0x230000FB and 0x23000067 decoded from the install: `ContainerSlot` keys 2–22 match API `Equipment` 1–21 (key 3 ↔ `Chest` 2, as check 68 uses), keys 23–25 are the FX slots, `ItemDurability` 1–7 and `ItemWearState` 1–4 match their keys and names, `ItemQuality` key 4 is `Uncommon` = 4 (check 69); Appendix A `ItemCategory`, `ItemDurability`, `ItemWearState` have `Undefined = 0` (the Absent columns); `scenario-feed.md` on branch `p0` has the `MoveItem` and `DestroyItem` answers of §7.6 and the `ItemQuantity`, `ItemCondition` events of §9.2 that checks 67 and the getter table cite; `character-panels.md` §4.3 rule 1 gives the *N* = *B* + 6 of check 66 and its rule 5 now agrees with the `SizeChanged` row; `icon-layers.md` §3.1 step 1 and `item-tooltips.md` §11 agree with the `GetIconImageID` and `GetWearState` rows. All agree. **Non-blocking notes:** (a) the §20 row on `character-panels.md` §8.1 is stale: that spec (9bced79) already reduced §8 to a pointer to §15.1 and states the `SizeChanged` rule in §4.3 rule 5, so its "Current text" no longer exists; mark it applied or drop it; (b) the new §20 lead-in parenthetical "on O4 (the `character-panels.md` row on Tessera rules, DEC)" reads as if O4 were its basis; reword; (c) P82 "name for name" is loose: the data names differ in form (`Pocket1`, `Weapon_Primary`, `Weapon_Secondary`, `Weapon_Ranged`, `Mounted`) and the API has aliases (`Shield`, `Instrument`); say "slot for slot" or point to `character-panels.md` §8.2; (d) state the order of the two plugin events when one feed `ItemMoved` crosses containers (backpack `ItemRemoved` versus `ItemEquipped`, check 68 asserts none), and whether a move within `Equipment` between keys 2–22 gives `ItemUnequipped` then `ItemEquipped` or nothing; (e) `GetNameWithQuantity(q)` for infinite or very large *q* ("decimal integer part" is undefined for infinity); (f) check 67 continues the scenario of check 66 ("the slot-4 item", size 30); say so. Earlier: first review of the P1 revision, 2026-10-09: changes requested, four blocking items (§15 not reconciled with P82; §15.1 OPEN missing from §19; no Tessera-rule provenance row; incomplete getter coverage, absent-property and literal-item values) and notes (a)–(f), all answered by the review fixes of 2026-10-09. Before that: review of the decision-alignment revision and its two repairs, 2026-10-06: **passed**, Accepted (after revision for decisions 0014/0015). Full record moved to the review log (decision 0018). |
| Implements into | `crates/script` (Lua host, sandbox), `crates/plugins` (discovery, apartments, API surface), `crates/ui` (controls) |
| Related specs | [`plugin-descriptor.md`](plugin-descriptor.md) (disk side: discovery, descriptor, `PluginOptions.xml`; see §20 for corrections to it), dat container spec (block header, DID routing), `records.md` §5 (0x41 render surfaces), §7 (enum mappers), §8 (DID maps), [`keymap.md`](keymap.md) (input actions), [`ui-layout.md`](ui-layout.md) (layout records, inheritance, layout rules, image layers and compositor; the game-styled controls of §14.2 are built from it), [`ui-behaviour.md`](ui-behaviour.md) (UI scale §9.2, resize bars §8.2, moving §8.1), [`skin-definition.md`](skin-definition.md) (skins reach the game-styled controls), [`fonts.md`](fonts.md) (font records, text layout and colour packing), [`plugin-api-usage.md`](plugin-api-usage.md) (how real plugins use this surface), [`plugin-runtime.md`](plugin-runtime.md) (interpreter, limits, collector step). Tessera decisions that place this API in the client: [0014](../decisions/0014-ui-input-bridge.md) (input reaches the UI and plugins; frame-loop placement of all plugin code, D8) and [0015](../decisions/0015-ui-rendering.md) (how surfaces, nested surfaces, opacity, clipping and rotation are drawn) |

> **Naming note (interoperability).** The namespace, class, method, event, field, enum member,
> XML element/attribute and file names in this document are the identifiers that existing
> third-party plugins and their files use. They are reproduced only because a plugin cannot
> run without them (see `docs/ARCHITECTURE.md` §18.2 and the *Google v. Oracle* row of
> `docs/LEGAL.md` §4.1; the written interoperability record is `docs/LEGAL.md` §17). They must appear
> in Tessera only inside the plugin subsystem, never as product names, crate names or UI
> strings. Descriptive text is our own summary. No documentation prose, client message text
> or game text is reproduced: error conditions are described, never quoted, and game-data
> enum entries are referred to by DID and key, not by their internal names. The UI layout
> labels (`luaplugins`, `LotroWindow_Container` and the other element labels of §14.2) and
> the property-dictionary names (`UICore_…`, `UIComponent_…`) used in §13 and §14 are
> labels read from the user's own data files; they are named only so that an implementer can
> find the same records at run time (`ui-layout.md` §3), the same way `ui-layout.md` and
> `skin-definition.md` name them.

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
- the event model: handler forms, dispatch, argument tables, delivery timing and re-entrancy
  (§9);
- plugin-data persistence: calls, timing, validation, location and the exact file format
  (§10);
- the shell/chat, engine, UI and gameplay namespaces (§11–§15), including how the UI routes
  the mouse and keyboard to controls, keyboard focus, z-order and window activation, size
  limits and the edge-attachment layout (§13.11–§13.16), how plugin backgrounds, back
  colours, stretch modes and opacity are drawn, and window rotation and scale
  (§13.17–§13.21), and how the game-styled controls are built from the user's UI layout
  data (§14.2);
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
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, PE file version **4906.0071.1794.4035** (`lotro_live`, built 2026-09-27), and its `x64/lua51.dll`, read in the dirty room on 2026-10-05 with a targeted capstone disassembly of individual functions (no whole-image dump kept). Code read: the Lua host (state creation, allocator, protected-call wrapper, error reporting, per-frame step), `import` and the package resolvers, plugin discovery and descriptor parsing, plugin load and unload, the plugin-data load/save/serialiser/path code, the shell and shell-command bindings, the engine functions, the event dispatcher and the control event-argument builders, the class and enumeration registration code (names, implementations, enum values), the text-markup tag parser, the `PluginOptions.xml` writer and the chat listener entry. A follow-up repair session on the same day read, again function by function: the start-up call that sets the C runtime locale (the only other locale calls switch the numeric category to the C locale for the duration of two value-conversion functions that the plugin code does not call, then restore it), the number formatter used by the plugin-data writer, the plugin-data key validator, the package-name check of the disk resolver, the named-colour registration, and the function bound as `GetScriptVersion` (questions: P5, P14, P35, P43, P45; M1–M3 could not answer them because they are code paths, and S3 shows only resulting values). A second follow-up session on 2026-10-06 (same build, same method: targeted capstone disassembly of single functions, address ranges only, no listing kept) read the client's general UI element framework as the plugin controls use it: the plugin control binding (its event table, notification handler, key and update subscriptions, size, position, size-limit, edge-attachment, z-order, focus, enable and window-activation methods, the scroll bar value setter), the UI element manager (notification posting and queueing, message broadcast and listener registration, pointer hit test, hover tracking, mouse capture, mouse-button and key action routing, keyboard focus, active window), the base element methods those call (resize with limits, move, show/hide, child ordering, edge-attachment layout, activate/deactivate), the plugin-data write step and the script-error listener of the game layer (questions: P47–P61). **Why M1–M3 were not enough for the second pass:** these are run-time behaviours of client code; the data files hold none of them, the owner's settings folder holds no plugins (O2), S2/S3 describe only names and a few captured argument tables, and M3 (running the official client with test plugins) was not available to this session. A third session on 2026-10-06 (same build, same method, address ranges only, no listing kept), answering the review of that day, read: the scroll bar's value, minimum, maximum, large-change and small-change setters and getters, its constructor and the integer conversions of the binding layer's argument and result wrappers; the control-creation step of the plugin layer (the initial element description of every control class) and the UI's element-creation path; the reference-capture step and the edge-attachment layout step again; the hover check, the pointer-movement and mouse-button handlers and the input-action entry of the UI element manager (the toggle-type test and the actions it ignores); and the text box's notification listener (questions: P48–P51, P57, P58, P64). **Why M1–M3 were not enough for the third pass:** the same as for the second; in addition, the clamp order and the creation defaults are code paths that no data file or published source records. **Questions:** everything marked BIN below (sandbox contents, resolution order, environments, re-import, load/unload sequence, dispatch, argument fields, plugin-data timing, key rules, file format, shell semantics, engine values, API surface and enum values). **Why M1–M3 were not enough:** all of these are code, not data (M1 can observe only the data files and the owner's settings folder, which holds no plugins); the published references (S2, S3) list names but not behaviour, and are older than this build; M3 would need the official client running third-party plugins on the owner's account, which is not available to this session and could not expose internal decisions (resolution order, sandbox contents, serialiser rules) exhaustively. A fourth session on 2026-10-06 (same build, same file hash, same method: targeted capstone disassembly of single functions, address ranges only, no listing kept), answering the M5 completeness critique, read: the creation-descriptor functions of the seven game-styled classes and of the scaled plain window; the window classes' element-build, post-build, title, close-button, resize-bar, text, scale, scale-group, origin and registration methods and the scaling service they call; the check box, text box and scroll-bar overrides; the plugin setters and getters of background, blend modes, back colour, stretch mode, opacity, rotation, font, fore colour, text alignment and selection; the UI element's layer setters, surface creation and the surface's rotation setter and transform; the scaler element's scale methods; `Shell.WriteLine`; and the binding layer's instance and `IsA` lookup (questions: P65–P76). **Why M1–M3 were not enough for the fourth pass:** the data (M1, O7) give the layout elements but not which class uses which, nor the code-driven geometry, defaults, drawing operations and scale arithmetic; S2/S3 only describe names; M3 (official client running test plugins) was not available to this session. A repair session on 2026-10-06 (same build, same file hash, same method, address ranges only, no listing kept), answering the review of the third revision, read: the plugin blend-mode setter and its conversion of the eight values, the element's operation getter and setter, the rebuild routine of the plugin controls; the element's root-panel constructor step, the object-mode property handler, the visibility setter and the show/hide step, the surface creation, installation and release steps, the save of the surface state on hide, the window rotation setter and the ancestor-surface lookup (questions: P77, P78, and the corrections of P72). **Why M1–M3 were not enough for the repair:** the operation numbers and the surface lifetime are internal state of client code that no data file or published source records; M3 was not available. Nothing about the network protocol, login, keys, the launcher or anti-tamper was read. Numeric constants are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). |
| **S1** | `research/notes/engine-client-re.md` §5 (dirty-room notes) |
| **S2** | Official "Update 25" Lua API reference, HTML, © 2016 publisher, lotrointerface.com file 1054 (version 2500.0062.0883); mirrored in the `lunarwtr/lotro-api-docs` repository (`U25/`) |
| **S3** | `lunarwtr/vscode-lotro-api` (MIT): `lotro-docs/TurbineDump.json` (a dump of the live in-game `Turbine` table, v1.0.18, 2026-03), `TurbineEventsDump.json` (event argument tables captured in game), `TurbineDumper.lua` (a lightly edited `.plugindata` file), `resources/xsds/lotroplugin.xsd`, EmmyLua stubs generated from S2 |
| **S4** | The publisher's 2010 beta sample plugins (`Turbine/` utility package and `TurbinePlugins/`), archived as `dist/Beta_LuaPlugins.zip` in `Goldbishop/Lotro_LUA`; S2 also mirrored there (U24) |
| **S5** | lotro-wiki.com pages "Lua Plugins", "Lua Scripting", "Plugin manager" (Wayback copies), which quote the 2010–2011 developer forum posts; lotrointerface.com file page 1054 |
| **S6** | Published third-party plugins downloaded for the M5 API census (dirty-room scratch; facts only: call counts and argument shapes, no code copied); the census results are published in [`plugin-api-usage.md`](plugin-api-usage.md) (§5.3 member counts, §9.3 per-plugin needs) |
| **O1** | Observed in user install 2026-10-03, static inspection of files only: `lua51.dll` (32- and 64-bit) version string and export table; `x64/lotroclient64.exe` and `lotroclient.exe` import tables and literal strings |
| **O2** | Observed in the owner's settings folders (read-only). 2026-10-03: a `Plugins/` tree with two third-party plugins (descriptors, file names and call patterns read statically, nothing copied). 2026-10-05: that `Plugins/` tree is no longer present; `PluginData/<account>/<server>/PluginOptions.xml` exists for three worlds (folder names are the world display names, one with a bracketed prefix and a space) |
| **O3** | Observed in user install data files 2026-10-03 (read-only probes): font records `0x42xxxxxx` in `client_local_English.dat` and `client_general.dat`; image records `0x41xxxxxx` in `client_surface.dat` |
| **O4** | Observed in user install data files 2026-10-05 (read-only, M1): the named-enum-mapper DID map 0x28000001 (`records.md` §8) and the enum mappers it names (`records.md` §7), decoded with their base chains: input actions 0x230000AF (290 own keys) with base 0x23000176 (181 keys), 471 keys in all, no key in both; character classes 0x2300000E (no base, 212 keys); species 0x2300000B (no base, 123); languages 0x230000B1 (3 own keys) with base 0x23000186 (2), 5 in all; shortcut types 0x23000137 (no base, 13); item qualities 0x23000067 (no base, 6); item classes 0x23000036 (no base, 279); craft tiers 0x23000063 (no base, 18); chat text types 0x230002A8 (39 own keys) with base 0x230002A9 (9), 48 in all. Key 0 is present in every one of these chains. Counts re-taken 2026-10-05 by the repair session; they are observations of this install, read at run time |
| **O5** | Observed 2026-10-05 (read-only): the owner's `lotro.keymap` in the Proton prefix binds the scroll-up input action to the mouse's Z axis in the positive direction and the scroll-down action to the negative direction. Observed 2026-10-06 in the same file: the left button is bound to the left select action (default activation), the left double-select action (`MouseDblClick` activation) and the left tap action (`Tap`); the right button's select action uses the `Tap` activation; no middle-button action is bound |
| **O6** | Observed 2026-10-06 in user install data (read-only, M1): action-map record 0x51000000, table A (`keymap.md` §12.10), read afresh: toggle type 1 (Momentary) for the six select and double-select actions 0x56, 0x4C, 0xA4, 0x33, 0x2E, 0x93 and for the wheel actions 0x82, 0x10; toggle type 2 (Impulse) for the three tap actions 0x32, 0x73, 0x6E; 364 entries in all (91 Momentary, 250 Impulse, 16 AutoRepeat, 4 Analog, 3 Toggle, as in `keymap.md`). Re-read 2026-10-06 by the second revision: the same counts; action 0xAB is a key of the input-action chain (O4) and has toggle type 2; none of the nine mouse-button actions has toggle type 3, 4 or 5 |
| **O7** | Observed 2026-10-06 in user install data (read-only, M1, own decoder written from `ui-layout.md` §4–§7): the `UILAYOUT` DID map (root key 4) entry with key 0x100000C8 is labelled `luaplugins` and names layout 0x22000826 (design size 1024 × 768, 13 top-level elements: their labels, types, rectangles, edge modes, properties and the trees of the window containers as summarised in §14.2); the `chatwindow` layout (0x22000034) element `ChatLogElement` names the top-level element `ChatLogListboxItemTemplate` in its `UI_ChatLog_TextElement` property, and that template has `UICore_Text_parse_markup_tags` = true; 203 elements of all layouts set that property, all to true. Property dictionary 0x34000000 (read-only): `UICore_Element_ObjectOpacity` has default 1.0, minimum 0, maximum 1; `UICore_Element_ObjectMode`, `UICore_Text_font_color` and the four size-limit properties have no default; `UICore_Text_parse_markup_tags` has default false. Values are observations of this install, read at run time |
| **DEC** | Tessera decision records, not sources of facts about the original: [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge; Accepted by the owner 2026-10-06) and [0015](../decisions/0015-ui-rendering.md) (UI rendering; Accepted by the owner 2026-10-06). Text that cites them states Tessera behaviour, which may differ from the original where the decision records a known difference |

Marks: **Confirmed** = observed in the owner's install (data, settings folder or the client
binary), with date; BIN facts are Confirmed by disassembly on 2026-10-05 (P1–P46) or
2026-10-06 (P47–P61, P64, the third-pass extensions of P48–P51, P57 and P58, the
fourth-pass rows P65–P76 and the repair rows P77–P78) and are marked
"BIN"; facts that rest only on BIN are flagged as such in the row. **Reported** = from
published sources only. Rows that cite only **DEC** state Tessera rules, not facts; they carry the
mark **Tessera rule** and need no observation.

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
| P12 | Font enum values are DIDs of type `0x42`; all 48 non-zero values exist as records; each record's payload starts with its own DID then the **line height in pixels** (the largest glyph height; in this install it equals the number at the end of the enum name). Corrected 2026-10-06: earlier text called the second field a "nominal point size" | O3, S3, BIN (values); `fonts.md` §3.2 and §14 (F4) | **Confirmed** 2026-10-03 (layout), 2026-10-06 (meaning, per `fonts.md`) |
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
| P41 | `Class`, `Race`, `Language`, `ShortcutType`, `ItemQuality`, `ItemCategory`, `CraftTier` and `ChatType` values are keys of the matching data enum mappers; `Equipment` slot values are not (they are `ContainerSlot` keys minus one, P82, added 2026-10-09) | O4, BIN | **Confirmed** 2026-10-05 |
| P42 | `SetBackground` raises a Lua error when an image file cannot be opened or has an unsupported format | BIN | **Confirmed** 2026-10-05 (BIN only) |
| P43 | During start-up the client sets the C runtime locale for all categories to a name chosen by its game language: `English` for `Turbine.Language` 2 and for any value not listed, `english-uk` for 0x10000001, `french` for 0x10000003, `german` for 0x10000004. That locale governs the `isalnum` tests of P5 and P14 and the decimal separator of the plugin-data writer (§3.4, §10.4) | BIN (repair session 2026-10-05: the single call that sets all categories, and its four name choices keyed by the language value; the plugin-data number formatter passes no explicit locale, so it uses this global one). **Why M1–M3 were not enough:** locale use is code, not data; S3 only reports that comma-separated files exist | **Confirmed** 2026-10-05 (BIN only) |
| P44 | Under the standard Windows locale data these four names select code page 1252; `English` and `english-uk` use `.` as decimal separator, `french` and `german` use `,`. In code page 1252 `isalnum` is true for some units 0x80–0xFF (accented letters such as 0xE9) | General platform knowledge (Windows C runtime locale names and code-page 1252 character classes), consistent with S3's reports of comma files | **Reported** |
| P45 | The 141 named colours: each of R, G, B equals the colour's standard web byte value divided by 255 and rounded to 3 decimals, stored as a 32-bit float; A is 1, except `Transparent` (A 0, R/G/B 1) | BIN (repair session 2026-10-05: all 141 constructor calls of the colour registration read, 564 of 564 float constants match the rule); S3 (live dump: all 423 R/G/B values match the rule, 312 of them differ from byte/255) | **Confirmed** 2026-10-05 |
| P46 | UI behaviour from the official reference: controls are transparent until given a background image or colour; `SetZOrder` higher draws on top; `Window:Activate` brings a window to the front; opacity is used by windows to blend with the game; a control that the mouse does not see passes mouse input to what is below; `IsDisplayed` rule; `SetParent` moves a control; `ContextMenu:ShowMenu` opens at the mouse, is moved back on screen when its right or bottom edge would fall off, and first closes any other open context menu; only one context menu shows at a time and input outside it closes it; the `Turbine.UI.Lotro` skinned classes are the game's silver (and gold) styled window, button, check box, text box and scroll bar; `ItemControl`/`ItemInfoControl` show an item the user can use as from any game panel; `EntityControl` gives the game's entity selection and context menu; `EffectDisplay` shows an effect applied to an actor; `EquipmentSlot` lets the user equip items | S2 (class and member pages of `Control`, `Window`, `ContextMenu` and the `Turbine.UI.Lotro` classes), S3 stubs (`EntityControl` flags) | **Reported** |
| P47 | Notification delivery: synchronous when no notification is being delivered, otherwise appended to one FIFO queue that is drained before the outermost posting call returns; a notification reaches the source control's handlers and then its native ancestors; plugin events fire only on the source control (§9.7) | BIN (second pass, 2026-10-06) | **Confirmed** 2026-10-06 (BIN only) |
| P48 | Broadcast messages: `Update` once per UI tick after hover processing, `KeyDown`/`KeyUp` for key actions the focus control or active window did not consume; subscribers called in subscription order with no visibility, focus or activation check (§9.4, §9.7, §13.13). Third pass: the plugin `TextBox` class (and the game-styled text box built on it) has its own notification listener that fires `KeyDown` (key-down notification) or `KeyUp` (key-up notification) on the text box whenever the text box itself is the notification's source, without consulting its wants-key-events flag; action 0xAB is ignored by the UI at its start and at its end (no key routing, no broadcast) (§13.13) | BIN (second and third pass); O6 (0xAB) | **Confirmed** 2026-10-06 (BIN only) |
| P49 | Hit test (visibility gate, topmost child first, children only inside their parent's rectangle, mouse-invisible controls transparent but not their children) and the hover target; `MouseEnter`/`MouseLeave` sent along the non-shared ancestor chains, innermost first, leaves before enters (§13.11, §13.12). Third pass: pointer events skip the hit test and leave no hover target while the input layer holds the pointer locked, and while the manager's pointer-outside flag is set and the pointer lies outside the UI root; that flag is cleared at creation and by any pointer event inside the root, and no code in this build sets it (§13.11) | BIN (second and third pass) | **Confirmed** 2026-10-06 (BIN only; what locks the pointer, OPEN) |
| P50 | Mouse capture and the button events: the nine mouse-button input actions, capture at the first press, `MouseUp` always to the pressing control, `MouseClick`/`MouseDoubleClick` only when released over it, the action-to-`MouseButton` table (§13.12). Third pass: the UI's only toggle-type test is "type 2 or not", looked up in the action map of the input context in force; every other type, including 0 for an action absent from that map, follows the Momentary rules at start and at end (§13.12.1, §13.12.2) | BIN (second and third pass); O4 (action keys); O6 (toggle types of the nine actions: Impulse actions never give button events); O5 (the owner's key map, worked example); S3 (captured `MouseDoubleClick` with `Button` 1 and `MouseClick` with `Button` 2 on plugin controls) | **Confirmed** 2026-10-06 (BIN, O6); Reported (S3 captures) |
| P51 | `MouseHover`: checked once per UI tick, after the pointer rested 0.25 s on the hover target, never while the mouse is captured; re-armed when the hover target changes; it is marked delivered only when the notification is reported handled, which the plugin layer never does (§13.12). Third pass: the check also skips while the manager's pointer-outside flag is set (never set in this build, P49); the comparison is strict (more than the delay); the delay is measured from the last pointer movement or the end of the last capture (§13.12.5) | BIN (second and third pass); numeric constant under decision 0012 | **Confirmed** 2026-10-06 (BIN only; the resulting per-tick repetition is inferred, not observed, OPEN) |
| P52 | `MouseWheel` goes to the captured control, else to the control under the pointer; only the scroll-up and scroll-down actions produce it; it does not activate windows (§13.12) | BIN (second pass), O4, O5 | **Confirmed** 2026-10-06 (BIN only) |
| P53 | Keyboard focus: `Focus()` takes effect at once only in the active window, otherwise it is remembered for the window's next activation; `FocusLost` before `FocusGained`; deactivating a window removes focus (§13.13) | BIN (second pass) | **Confirmed** 2026-10-06 (BIN only) |
| P54 | Window activation: a mouse-button press activates the window under the pointer (or deactivates the active window when the press hits no window); `Activate()` does nothing on an already active window; order Deactivated (old), FocusLost, Activated (new), FocusGained; activation raises the window above siblings of equal `ZOrder` (§13.14) | BIN (second pass) | **Confirmed** 2026-10-06 (BIN only) |
| P55 | Z-order: `SetZOrder(n)` stores −n as the control's depth, siblings are kept sorted by depth, `GetZOrder` returns the value set (default 0) (§13.14) | BIN (second pass) | **Confirmed** 2026-10-06 (BIN only) |
| P56 | Size limits are element properties applied at every later resize, maximum then minimum per axis; setting a limit does not resize; getters return 0 when unset (§13.15) | BIN (second pass) | **Confirmed** 2026-10-06 (BIN only) |
| P57 | Edge attachments: argument and result order left, top, right, bottom; reference rectangles captured on attach, resize, move and re-parent; the per-type layout rules and the constant 0.5 (§13.16). Third pass: before the first capture the control's own reference is its creation rectangle and it has no parent reference (the layout then uses the parent's own reference rectangle); every control created through the client's element-creation path, which includes every plugin control, carries the mark that makes None edges always take the current coordinate (§13.16) | BIN (second and third pass); numeric constant under decision 0012 | **Confirmed** 2026-10-06 (BIN only) |
| P58 | Which setters fire change events and when (§9.6): size, position and visibility only on an actual change and through the notification queue; `EnabledChanged` directly from `SetEnabled`; `ScrollBar:SetValue` clamping and `ValueChanged`. Third pass: the scroll bar's defaults, the clamp order (maximum, then minimum), the range rules of `SetMinimum`/`SetMaximum`, the large/small change caps, and the unsigned results of the scroll bar getters (§9.6) | BIN (second and third pass) | **Confirmed** 2026-10-06 (BIN only) |
| P59 | Script errors and warnings reported to the user are added to the chat window as chat text type 1 (`Turbine.ChatType.Error`) with the message text unchanged and no prefix, and also written to a client log channel (§6.6) | BIN (second pass: the game layer's error listener) | **Confirmed** 2026-10-06 (BIN only) |
| P60 | Plugin-data write step: parent folders created, file opened for text writing, the serialised text written, the write result not checked; `onDone` called only while the issuing apartment still exists; no line end after the final value (§10.4, §10.6) | BIN (second pass) | **Confirmed** 2026-10-06 (BIN only) |
| P61 | The plugin layer passes the command word and argument text to `ShellCommand:Execute` exactly as it receives them from the client's command-line parser (§11.2) | BIN (second pass) | **Confirmed** 2026-10-06 (BIN only) |
| P62 | Captured argument tables: `Party.AssistTargetRemoved` carries an empty table; a `KeyUp` with `Action` 19 (a key below 0x100) occurs | S3 (`TurbineEventsDump.json`) | **Reported** |
| P63 | Usage survey (counts only, no code read for expression): none of the 62 Lua files of the publisher's 2010 sample plugins (S4), the 9 Lua files of one third-party in-game plugin in the research clones (LotroCompanion's) and the 7 Lua files of S3 use `newproxy` or `__gc` | S4, S3, research clones (dirty room, 2026-10-06) | **Reported** (small sample) |
| P64 | Initial geometry of plugin controls: the plugin layer describes every new control as left 0, top 0, width 50, height 50 (all 25 control class tables of this build; seven of them, matching the seven skinned classes of §14.2 in number and in their window and text-box variants, also name a client layout element) (§13.1) | BIN (third pass; fourth pass for the layout element) | **Confirmed** 2026-10-06 (BIN only). Fourth pass: the "layout element" is the **base** of the creation descriptor, whose own 50 × 50 rectangle replaces the base's (P65); the "25 + 1" count question of the earlier review is answered by P65 (the extra function belongs to the scaling service, not to a control class) |
| P65 | Game-styled controls: the seven classes describe their element as a **based** descriptor: no type of its own, base element = a fixed element ID, base layout = the layout that the `UILAYOUT` map gives for entry key 0x100000C8 (labelled `luaplugins`), own rectangle 0, 0, 50 × 50 (P64). Class → element ID: `Button` 0x100022A7, `GoldButton` 0x100022A8, `CheckBox` 0x100025AF, `TextBox` 0x100022A9, `ScrollBar` 0x1000239E (horizontal) or 0x1000239F (vertical, chosen by the orientation at build time), `Window` 0x10002294, or 0x10004D37 when the window is marked as scaled (§13.21), `GoldWindow` 0x10002299, or 0x10004D38 when a second flag set at construction from the scaling service is set (OPEN). A plain `Turbine.UI.Window` marked as scaled is re-described by the scaling service with the type `UIScaler` (0x100000E8); the descriptor also names base element 0x10004CE3, but a descriptor with a type is never resolved through its base (`ui-layout.md` §7.1 step 1; BIN: the resolver tests the type first), so that element's properties are not inherited. The element is rebuilt from a new descriptor when the window is first marked as scaled and when a game-styled scroll bar changes orientation; a rebuild keeps visibility, position, size, z-level and background image, and passes the old base-layer operation number to `SetBlendMode` as if it were a blend-mode value (§14.2.1; numbers and the resulting modes: P77) | BIN (fourth pass: the creation-descriptor functions, the descriptor merge, the element-build path, the rebuild routine); O7 (labels, types and trees) | **Confirmed** 2026-10-06 |
| P66 | Game-styled window behaviour: `SetText` sets the text of descendant 0x10002297 (title text) and measures it; the title text, the title-bar background (0x10002296) and the drag bar (0x10002DFB) are positioned by code from the window width (§14.2.3), after creation, after every `SetText` and after every size change (after `SizeChanged` handlers ran); a `Button_click` UI message (0x35) from descendant 0x10002298 (close button) calls the window's `Close()`; the eight resize bars 0x10002DFC–0x10002E03 are hidden (`UICore_Element_hide` true) unless `SetResizable(true)`; resizable defaults to false; `SetResizable` acts only on a change | BIN (fourth pass); O7 | **Confirmed** 2026-10-06 (BIN only for the behaviour) |
| P67 | Game-styled text targets: `Label`, `TextBox` and `CheckBox` members of the game-styled text box and check box act on the text element 0x100022AA (text box) or 0x100025B1 (check box); the game-styled check box's check state is shown by its descendant 0x100025B0; the game-styled button's own element is the text target | BIN (fourth pass: the per-class "text element" and "check element" overrides) | **Confirmed** 2026-10-06 (BIN only) |
| P68 | Drawing of plugin backgrounds and back colours: `SetBackground` sets the element's base image layer without changing its operation; `SetBlendMode(n)` sets the base layer's compositor operation through the same conversion as layout draw mode *n*; a new element's base operation is Replace; `SetBackColor` sets an overlay colour fill drawn after the base image with the operation of `SetBackColorBlendMode` (default 2); `SetBackColor(nil)` removes the fill; `SetBackColorBlendMode` re-applies the fill with the stored colour even when no colour was set; colour components are packed into bytes by ×255 and truncation, with no clamping; an invalid blend-mode number (above 8) raises an error | BIN (fourth pass: the four setters, the layer setters, the draw-mode converter, the colour-fill draw) | **Confirmed** 2026-10-06 (BIN only) |
| P69 | `SetStretchMode(n)` stores *n* as the element's `UICore_Element_ObjectMode` and marks it as set by the plugin; a non-zero mode gives the element its own drawing surface (0 removes it); `GetStretchMode()` returns the property (0 when neither the plugin nor the layout set it); a Tile surface takes the base image's size and falls back to CanvasResize unless both sides are powers of two | BIN (fourth pass: the stretch setter and getter, the property-change handler, the surface-creation routine); O7 (no dictionary default) | **Confirmed** 2026-10-06 (BIN only); the visible effect of modes 1–4, OPEN in part (§13.18) |
| P70 | `SetOpacity(f)` sets `UICore_Element_ObjectOpacity`, which acts only on an element that has its own surface: every shown window (each is a root panel) and any shown control with a non-zero stretch mode (when a surface exists: P78); `GetOpacity()` returns the property (dictionary default 1.0) | BIN (fourth pass: setter, getter, the window build path); O7 (default); `ui-layout.md` §5.9 (property effect) | **Confirmed** 2026-10-06 |
| P71 | Label defaults and conversions: `GetFont()` is 0 until a `SetFont` with a DID that loads as a font record (others are ignored); the text is drawn in the text element's font (for a plain label the locale default font, `fonts.md` §9.1); `GetTextAlignment()` is 1 (TopLeft) until set; `SetTextAlignment` acts only on a change and maps the nine values to the two justification properties (§13.4); `GetForeColor()` without a colour property on the text element returns opaque black; selection positions are 1-based, clamped to 1 … text length, and counted in the client's UTF-16 units | BIN (fourth pass) | **Confirmed** 2026-10-06 (BIN only) |
| P72 | `Window:SetRotation{x, y, z}`: angles in **degrees**, each reduced modulo 360; the window's surface is rotated about its centre (the three angles build a 3D rotation of the surface quad); a window without a surface first gets object mode Stretch unless the plugin set a non-zero stretch mode; `GetRotation()` returns the last table values | BIN (fourth pass: the setter, the surface's rotation setter and the surface transform with the constants 360, π and 180); S6 and `plugin-api-usage.md` §5.3, §9.3 rows 8 and 12: plugins pass values such as 0, 90, 180, 270 and headings, mostly as `z` (counts and argument shapes only) | **Confirmed** 2026-10-06 (BIN); usage values Reported (S6); direction of positive angles and order of the three rotations, OPEN. Corrected 2026-10-06 (repair): the object-mode change and the surface it creates depend on whether the window is shown (§13.20 steps 2–3, P78) |
| P73 | Window scale: `SetScale`, `SetUIScalingGroup`, `RegisterForGlobalScaling` mark a parentless window as scaled (rebuilding it as a scaler element, P65) and act through the client's scaling service; group 0 and 60 passed to `SetUIScalingGroup` become 72; the effective scale is the window's own scale (or its group's value, default 1) times `UIScale_Global` when the window takes part in the global scale, else the own scale alone; the scale is applied by the panel-scale rule of `ui-behaviour.md` §9.2, and deferred while the window is not shown; `GetScale()` is 1 for a window never marked, else the effective scale; `SetScalingOriginPoint(x, y)` sets the scaler origin properties and is ignored on unmarked windows; `Unregister…` stops taking part in the global scale but keeps the window a scaler | BIN (fourth pass: the window methods, the scaling service, the scaler element's scale methods) | **Confirmed** 2026-10-06 (BIN only) |
| P74 | `UIScaleChanged` fires on a window when the changed scaling group is 60 (`Global`) or equals the window's own group; its argument table carries `Index` (the group) and `Value` | BIN (fourth pass) | **Confirmed** 2026-10-06 (BIN only) |
| P75 | `Shell.WriteLine` text reaches the chat log unchanged (no escaping); the chat log's text elements are built from a template whose `UICore_Text_parse_markup_tags` is true, so §13.4 markup in a `WriteLine` line is rendered | BIN (fourth pass: `WriteLine`); O7 (template and property) | **Confirmed** 2026-10-06 (code path BIN, template data O7); not observed on screen (M3) |
| P76 | Class tables are plain writable tables: the binding layer installs no `__newindex` anywhere, registers each class's own methods only on its own table, and finds methods (and evaluates `IsA`) through the ordinary `__index` chain; so a function stored on a class table is what every Lua lookup through that class finds, for existing and new instances; native client code never calls plugin methods through these tables | BIN (fourth pass: string references and the instance and `IsA` paths; first-pass registration code); P19 | **Confirmed** 2026-10-06 (BIN only); not observed in a running client |
| P77 | Base-layer operation numbers: `SetBlendMode(n)` stores `n` and gives the element the operation number 3 for `n` = 1 (Colorize), 0 for 2 (Replace), 4 for 3 (Multiply), 2 for 4 (Over), 1 for 5 (Mix colour), 6 for 6 (Grayscale), 5 for 7 (Screen), 7 for 8 ("none") and 0 for `n` = 0; a new element's number is 0; the rebuild routine reads the old element's number and passes it to the same setter, so the blend mode after a rebuild follows the table of §14.2.1 step 3; the rebuilt top elements of the plugin layout carry no media items (O7) | BIN (repair of the fourth pass, 2026-10-06: the plugin blend-mode setter and its eight-way conversion, the element's operation getter, the rebuild routine; same build and method as the fourth pass, address ranges only); O7 (media census of the 13 top elements) | **Confirmed** 2026-10-06 (BIN; media census O7) |
| P78 | When an element has its own surface: it **wants** one when its object-mode value is non-zero, or, without a value, when it is a root panel (every plugin window); the surface exists while it wants one **and** its own visibility flag is set or `UICore_Element_KeepObjectWhenHidden` is true; the client re-evaluates this on object-mode, root-flag and visibility changes; hiding saves the surface's rotation and showing restores it; a surface for an element without an object-mode value is CanvasResize; `SetRotation` on a window without a surface clears the saved state, sets object mode 1 unless the plugin set a non-zero mode, and then targets the nearest ancestor surface; no element of the 13 resolved trees of the plugin layout (188 elements, states included) sets `UICore_Element_KeepObjectWhenHidden` | BIN (repair of the fourth pass, 2026-10-06: the root-panel constructor, the object-mode property handler, the element's visibility setter and show/hide step, the surface creation and installation steps, the surface-state save on hide, the window rotation setter); O7 (property census, read-only, 2026-10-06); `ui-behaviour.md` §11.2 (agrees) | **Confirmed** 2026-10-06 (BIN; census O7); whether an unshown top-level window reaches the screen root's surface, OPEN (§13.20) |
| P79 | Frame-loop placement of plugin code in Tessera: input handlers in step 1, the UI tick, `Update`, the plugin host's deferred work (sliced by whole items, 2 ms per frame) and the collector step in step 7; every plugin load (queued `LoadPlugin`, `/plugins load`, manager UI, autoload at login; the last three a Tessera reading of D8) as a step-7 deferred item, none before the first `Plugins/` index (0015 §4.4); asynchronous `PluginData.Load` handlers and writes as deferred items; game-raised plugin events (`Turbine.Chat.Received`, the gameplay views' `…Changed` events) and the logout unload queued as deferred work; the `Received` skip set (interim, pending a revision of 0014, open review item 1); the client-exit unload after the last frame; blocking writes at unload and when the frame clock steps (interim rules, 0014 "Other improvements"); pointer-move coalescing; hit tests against current geometry; the pointer lock as client state; command names and IDs, activation order, timing, repeats and toggle types of the actions the UI receives; typed characters and `opaque_keys`; Escape with a UI attached; the scroll actions over plugin controls (§3.3, §6.2, §6.3, §6.6, §9.4, §9.7, §10.6, §11.1, §11.3, §12, §13.3, §13.11–§13.13, §15) | DEC (0014 D1–D9, "Escape with a UI attached", Known differences 6, 7, 10, 12, 15, 16, 18, 21, open review items 1 and 2, "Other improvements"; 0015 §4.4) | **Tessera rule** (no observation; the original's timing is OPEN where stated) |
| P80 | Drawing of plugin surfaces in Tessera: a nested surface is a separate composition entry with its own opacity only, clipped by its nearest enclosing window and every enclosing window above it, turning with its surfaced ancestors; stand-in surfaces for top-level windows in object mode 0; mode-0 child windows drawn into the nearest surface above; mode 2 repeat addressing and Tile parents; the mode-change refresh; first paints, caps and the image checks of `SetBackground(path)` (§13.2, §13.8, §13.17–§13.21); the clip chain built from windows (interim, pending a revision of 0015, open review item 1) | DEC (0015 Decision 1.1, 1.5–1.8, U4 (c), §2.1, §2.3, §4.3–§4.4, §7, open review item 1) | **Tessera rule** (no observation; the original's composition of nested surfaces is OPEN, §13.18, §13.19) |
| P81 | `Backpack:GetSize()` returns the backpack's size; `Backpack:GetItem(index)` takes 1 … `GetSize()` and returns the item there or nil; `Equipment:GetItem(index)` takes a slot of the `Equipment` enumeration and returns nil for an empty slot; `Equipment:GetSize()` is the number of items that can be equipped; `LocalPlayer:GetBackpack()`, `GetEquipment()` return those objects; `ItemInfo` getters as listed in §16 | S2 (class and member pages of `Backpack`, `Equipment`, `LocalPlayer`, `ItemInfo`) | **Reported** |
| P82 | The `Equipment` enumeration's values 1 … 21 equal the data's `ContainerSlot` keys 2 … 22 minus one, name for name (`Head` … `Class`, `Bridle` ↔ `Mounted`); the keys 23 … 25 (weapon FX slots) have no API value; `ItemDurability` 1–7 and `ItemWearState` 1–4 equal the keys and names of mappers 0x230000FA and 0x230000FB (§15.1) | S2/S3 (Appendix A values); O4-type observation 2026-10-09 of the `ContainerSlot` mapper 0x230000EE (`character-panels.md` OBS) | **Reported** (API values) + **Confirmed** (data keys and names) |
| P83 | Inventory views in Tessera (§15.1): the getter-to-property mapping of `Item` and `ItemInfo`; one `Backpack` and one `Equipment` object per apartment, one `ItemInfo` per record DID per apartment; nil from `GetItem` for a non-integer or out-of-range index; `Equipment:GetSize()` = 21; `Backpack` and `Equipment` events derived from the feed's `ItemAdded`, `ItemMoved`, `ItemRemoved` and `ContainerSize`, moves between containers included; `SizeChanged` whenever *N* changes, also through an arriving item before the first `ContainerSize`; `GetMaxQuantity` = `GetMaxStackSize`; `IsMagic` false; `GetQualityImageID` 0; the `GetNameWithQuantity` form; the values for absent properties and for literal items | DEC ([0024](../decisions/0024-parity-track.md) P1); `character-panels.md` §4.3, §8, §9 | **Tessera rule** (no observation; the original's values are OPEN, §15.1) |
| P90 | `Party.MemberAdded` and `MemberRemoved` are fired through the same argument routine as `AssistTargetAdded` and `AssistTargetRemoved`: it puts the member object under the field `Player` when the member object is available and leaves the table empty otherwise; `Party.LeaderChanged` and `Player.PartyChanged` are fired with an empty table (§9.3, §15.4). Corrects the reading of P31/§9.3 that no field name is wired to the member events | BIN (2026-10-11, same build; question: which argument table each party event gets; the initialisers of the four event names and the routine that fills the table were read by address range; why M1–M3 were not enough: S3 has no capture of the member events), S3 (captures: `AssistTargetAdded` with `Player`; `AssistTargetRemoved`, `LeaderChanged` and `PartyChanged` empty) | **Confirmed** 2026-10-11 (BIN) |
| P91 | `Party` getters: `GetMemberCount` and `GetAssistTargetCount` return the length of the member and assist-target lists; `GetMember(i)` and `GetAssistTarget(i)` take 1 … count and return nil for any other index; `IsMember(p)` and `IsAssistTarget(p)` test whether `p`'s character is in the list; the member list holds the local player as an entry, and `GetMember` and `GetLeader` return the `LocalPlayer` instance for it (§15.4) | BIN (2026-10-11, same build; question: index base, range behaviour and the player's entry; the `Party` registration and the bound getters were read; M1–M3: S2 documents only the signatures) | **Confirmed** 2026-10-11 (BIN only) |
| P92 | The party is the player's fellowship, or, for a monster player, its group (§15.4) | S2 (`Party` class page) | Reported |
| P93 | Party views in Tessera (§15.4): the party model from the feed's `FellowshipChanged` (`social-panels.md` §9 TR2), object identity, getter values without data, the derivation and order of `PartyChanged`, `MemberAdded`, `MemberRemoved` and `LeaderChanged` | DEC (decision 0024 P4; `social-panels.md`) | **Tessera rule** |

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
  **Finalizers (answer of 2026-10-06 to the implementer question of the same day,
  `crates/script`).** Lua 5.1 runs `__gc` metamethods with debug hooks disabled, so a plugin
  finalizer (attached through `newproxy(true)`) would escape the budget, including at state
  close. The implementation's choice is accepted as a stated deviation: `newproxy(true)`
  returns a proxy whose metatable, as seen by `getmetatable`, is a view; writes through it
  reach the real metatable, except that assigning `__gc` installs a host dispatcher that runs
  the plugin's finalizer in a fresh, metered coroutine and re-raises its error as 5.1 does
  for a finalizer error; finalizers run at apartment close share one fresh budget. The
  observable differences from stock 5.1 (`rawget`, `rawset`, `next`/`pairs` on the proxy's
  metatable see the empty view; a `rawset` on it does not affect the proxy) are acceptable:
  no surveyed plugin uses `newproxy` or `__gc` at all (P63). One condition: reading a field
  through the view with normal indexing (including `getmetatable(p).__gc`) must return what
  the plugin stored, not the host dispatcher.
- **Garbage collection.** Once per frame the original runs one incremental collection step
  in every apartment (BIN). Tessera must make the same step, `lua_gc` with `LUA_GCSTEP` and
  20 in every apartment once per tick (`plugin-runtime.md` §9.2, which governs; this replaces
  the earlier "may rely on the default collector"). It runs in frame-loop step 7, after the
  plugin host's deferred work (decision 0014 D8, step 7 item 6).
- All plugin code runs on the main (UI) thread. **Tessera rule (decision 0014 D8):** "between
  frames" (ARCHITECTURE §18.1, `plugin-runtime.md` §10.3) means in frame-loop **step 1**
  (the plugin input handlers that the input layer's offers run: `MouseDown`, `MouseUp`,
  `MouseClick`, `MouseDoubleClick`, `MouseMove`, `MouseEnter`, `MouseLeave`, `MouseWheel`,
  `KeyDown`, `KeyUp`, `FocusGained`, `FocusLost` and the notifications they cause, §9.7) and
  **step 7** (the UI tick below, then the plugin host's deferred work, which includes the
  game-raised plugin events of §11.3 and §15 and the logout unload of §6.3, then the
  collector step), with one exception: the unload of every apartment at client exit, which
  runs after the last frame (§6.3). Plugin code never runs during the session, simulation,
  layout, paint or rendering steps, and never on another thread. Input that arrives between
  two redraws is handled in the window system's callbacks on arrival and counts as step 1 of
  the following frame. Time spent in plugin Lua calls is measured as plugin time, apart from
  the UI's own budget; it counts toward the frame-time rule. Input handlers, the `Update`
  broadcast and the collector step are never deferred or cut by a time slice; each handler
  call has its own instruction budget (above).
  > **OPEN:** how Tessera keeps the notification events (§9.7) that game or session code
  > causes outside steps 1 and 7 inside those steps: for example a native panel shown by a
  > session event with activation on show, which deactivates the active plugin window and
  > takes focus from its text box at once (§13.13, §13.14), or a window-size change that
  > re-lays out root panels and raises `SizeChanged`/`PositionChanged`. Recorded as open
  > review item 2 of decision 0014, to be settled there (either such UI changes are applied
  > in step 7, or the rule above is widened). Until then the rule above is the target, and
  > any such event is still delivered by §9.7.

  > **OPEN (when the frame clock steps, decision 0014 "Other improvements"):**
  > `Turbine.Engine.GetGameTime()` is constant within one frame (§12), and the hover delay
  > of §13.12.5 is measured on the UI's frame clock. Decision 0014 places input handled in
  > window-system callbacks in step 1 of the following frame but does not say when these
  > clocks advance, so a step-1 handler run in a callback could see the previous frame's
  > value while that frame's `Update` sees the next. **Tessera rule until the decision
  > says otherwise:** one frame clock serves `GetGameTime()` and the UI; it advances once
  > per frame, right after the previous frame has been presented and before any step-1
  > work of the next frame, including work done in callbacks. So every plugin call that
  > belongs to one frame (its step-1 handlers wherever they run, and its step 7) sees the
  > same value, and the time of a pointer movement for §13.12.5 is the clock value of the
  > frame whose step 1 delivers it. The client-exit unload (§6.3) sees the value of the
  > last frame.
- **UI tick (P48, P51).** Once per frame the UI element manager runs one tick, in this
  order: (1) if the pointer needs re-evaluating, the hit test and hover target are refreshed
  (§13.11); (2) the hover check, which may deliver `MouseHover` (§13.12); (3) the `Update`
  broadcast to every subscribed control (§9.4); (4) the rest of the UI's per-frame work.
  Input events (mouse movement, mouse-button and key actions) are delivered when the client
  processes input, outside this tick. Deferred work of the plugin host (queued `LoadPlugin`
  requests, asynchronous `PluginData.Load` handlers, plugin-data writes) runs between frames
  too (§6.2, §10.6). > **OPEN:** the order of the plugin host's deferred work relative to
  the UI tick within one frame. Tessera (decision 0014 D8): input (frame-loop step 1), then,
  in step 7, the UI tick in the order above, then the plugin host's deferred work, then the
  collector step; layout, paint and rendering follow in step 8 and later.
- **Tessera's deferred work (decision 0014 D8, Tessera rules).** The queue holds, in the
  order they were queued: `LoadPlugin` requests (§6.2), the other plugin loads (§6.2: loads
  asked for with `/plugins load` or from the manager UI, and the **autoload loads**, one item
  per autoloaded plugin, queued in the order of Tessera's autoload list when the session
  reports the character login; D8 names only `LoadPlugin` requests, so these are a Tessera
  reading of D8, as in `plugin-runtime.md` §8.4 and `plugin-descriptor.md` §8.1),
  asynchronous `PluginData.Load` handlers, plugin-data writes and their `onDone` callbacks
  (§10.1, §10.6), the **game-raised plugin events** (`Turbine.Chat.Received`, §11.3; the
  gameplay views' `…Changed` events, §15) and the **logout unload** (§6.3), one item per
  apartment. The host runs the items in queue order and starts no further item once this
  frame's deferred work has taken 2 ms of plugin time; the rest waits for the next frame's
  step 7. An item is never interrupted, and queue order is never changed, so load order
  (§6.2) and per-key plugin-data order (§10.6) are kept. At least one item runs in each
  frame (D8), with one exception: no load item runs before the first `Plugins/` file index
  of §13.8 is ready (decision 0015 §4.4). While that index is not ready and the item at the
  head of the queue is a load, that frame's deferred work stops at that item (so when it
  is the head at the start of step 7's deferred work, no item runs that frame: D8's
  at-least-one-item rule does not apply), and the items behind it wait too, so queue order
  is kept. Refreshed indexes never hold back a load (the previous index serves until
  the rebuilt one is swapped in). This is the rule of `plugin-runtime.md` §8.4 ("Wait for
  the first file index"); D8 does not state the exception, which follows from 0015 §4.4.
  A plugin's top-level code and its `Load` handlers therefore run only inside such an item,
  in step 7, whatever asked for the load.
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
works through later (between frames; in Tessera, as an item of the plugin host's deferred
work in frame-loop step 7, §3.3). It returns nothing, raises nothing, and does nothing
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

When each load runs in Tessera (Tessera rules; the steps above are unchanged): a queued
`LoadPlugin` request is one step-7 deferred-work item (decision 0014 D8). The other triggers
are queued the same way, one load per item (§3.3): a `/plugins load` or manager-UI load when
it is asked for, and the autoload loads when the session reports the character login (a
Tessera reading of D8, which does not name them; `plugin-runtime.md` §8.4). No load runs
before the first `Plugins/` file index is ready (decision 0015 §4.4, §13.8). So the load
steps 1–8 above never run during the session step or inside the step-1 routing of a chat
line or click; they run in frame-loop step 7.

> **OPEN:** the order in which autoloaded plugins load. Tessera: the order of its own
> autoload list, each load one step-7 deferred-work item queued when the session reports the
> login, as above (decision 0014 D8, Tessera reading; `plugin-runtime.md` §8.4).

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

**When the steps run in Tessera (Tessera rule, decision 0014 D8; the steps themselves are
unchanged).**

- An unload that a plugin or the user asks for (`PluginManager.UnloadScriptState`,
  `/plugins unload`, the manager's unload action) runs where the request is made: inside the
  plugin call, or inside the routing of the chat line or click that asked for it, so in
  frame-loop step 1 or 7 (§3.3).
- The **logout** unload (every apartment) is queued by the session code that reports the
  logout and runs as deferred work in step 7, one item per apartment in queue order, sliced
  like the other items (§3.3). Plugin-data writes queued before it keep their order.
- The **client-exit** unload runs when the client's event loop ends, after the last frame
  has been presented and before the renderer and window are destroyed, on the main thread:
  the `Unload` handlers, then the state close with its synchronous `PluginData.Save`
  (§10.6). Every handler call keeps its instruction budget (§3.3). It is the only plugin
  code outside frame-loop steps 1 and 7.

> **OPEN (blocking writes at unload, decision 0014 "Other improvements", ARCHITECTURE
> §21):** an unload in step 1 or 7 (asked for by a plugin or the user, or the logout unload)
> closes Lua states, and step 5 above with §10.6 makes `PluginData.Save` write synchronously
> while a state closes, and Tessera flushes the apartment's queued writes before the close.
> That is blocking file I/O on the main thread inside a frame, which `docs/ARCHITECTURE.md`
> §21 does not list as an exception; decision 0014 notes the gap and does not settle it (an
> §21 exception, or a flush that does not block the frame). The client-exit unload runs
> after the last frame, so no frame waits for it. Tessera, until it is settled: keep the
> synchronous write and the flush, as `plugin-runtime.md` §8.4 also records.

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
size (§13.2). A protected call that fails with Lua's memory-error status is not reported by
the protected-call step itself (the allocator's own report covers it); only the runtime-error
and syntax-error statuses are (P25).

**What the user sees (P59).** The game layer registers one listener that handles errors and
warnings alike: it converts the reported text to the client's text, adds it to the chat
window as one line of chat text type **1** (the value named `Error` in `Turbine.ChatType`,
Appendix A.1), and writes the same text to a client log channel. The client adds **no
prefix** (no plugin name, no apartment name, no label): the chat line is exactly the Lua
message, e.g. `<chunk name>:12: attempt to call a nil value` for a runtime error in a file.
The colour is whatever the chat window uses for chat type 1 (a user setting of the chat
window, not part of the plugin API). A message containing line breaks is passed on as one
text.

Tessera: add the message to its chat output as an error-type line (chat type 1) with the
same text and no added prefix, so that plugins that parse their own errors from chat keep
working; put the plugin name, apartment and an optional traceback in the log only.

> **OPEN:** whether the error line is also delivered to `Turbine.Chat.Received` handlers
> (with `ChatType` 1). Tessera: deliver it like any other chat line, as a queued
> game-raised event in frame-loop step 7 (§11.3, decision 0014 D8), never re-entrantly
> (§9.6; the skip set of the §11.3 OPEN on re-entrancy).

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

> **OPEN (implementation, M5, `crates/plugins`):** two details of the resolution are not
> stated here and the M5 host chose: (1) the native registry is matched by the exact name
> (`import "turbine.ui"` has no native part); (2) the namespace path is walked with raw
> table access, so a namespace whose metatable falls back to the globals (after an import,
> step 5) does not lend a global table of the same name as a namespace. Also, when the disk
> part of `import "Turbine"` fails at apartment creation (`plugin-runtime.md` §5.1 step 5),
> the M5 host reports the error and keeps the apartment with the native part.

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

> **OPEN:** (clean-room implementer, 2026-10-08) when the original destroys a control that
> plugin code no longer references (§8.2 keeps identity, not lifetime). Tessera rule
> (`tessera-plugins` UI layer): a control stays alive while plugin code references it,
> while it is a child, list item or tree node of a live control, while it is a shown
> top-level window, or while it is subscribed to `Update` or the key broadcast; an open
> context menu stays alive with its items. Otherwise it is destroyed after its table is
> collected, together with the descendants collected with it; a descendant whose table is
> still alive (referenced, or a broadcast subscriber) is detached and lives on without a
> parent. Limitation (revised 2026-10-08 after review): a parent keeps its children's
> tables through a weak-keyed table, and Lua 5.1 has no ephemeron tables, so a hidden
> window that plugin code dropped is kept until unload when a descendant's table refers back
> to it (for example a handler closure that captures the window). Tessera bounds: at most
> 65,536 UI objects and 64 MiB of control text per apartment.

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

<a id="8-6"></a>
### 8.6 Writing to class and enumeration tables (P76)

Plugins add fields to, and replace fields of, the native tables (`plugin-api-usage.md` §6.3).
The original allows all of it and gives it ordinary Lua meaning:

1. Namespace, class and enumeration tables accept writes like any table; no metamethod
   intercepts them.
2. Each class table holds only its **own** native methods (§16); inherited methods are found
   through the `__index` chain at each lookup. Nothing is copied into subclasses or cached.
3. Therefore a function stored as `Turbine.UI.Control.SetBackground` is what
   `x:SetBackground(…)` finds for every instance `x` of `Control` and of every class below it
   (`Label`, `Window`, `Turbine.UI.Lotro.Button`, Lua subclasses), existing or created later,
   unless a class nearer to `x` (or `x` itself) has its own field of that name. A wrapper
   that saved the old function first can call it, because the old value is an ordinary
   function value.
4. Replacing a method changes only Lua lookups. The client's own code never calls plugin
   methods through these tables: for example a control rebuilt by the client (§14.2.1)
   re-applies its blend mode natively, and `Close()` from the game-styled close button
   (§14.2.3) runs the native close even if `Turbine.UI.Window.Close` was replaced.
5. `IsA` follows the same `__index` chain, so replacing `IsA` on a class table changes it
   for that class and the classes below it.
6. Writes stay in the apartment that made them (§6.1). Overwriting an enumeration value
   (for example `MinstrelStance.None`) changes what Lua code reads. Whether any host function
   reads an enumeration table back was not traced; Tessera's host never does, and takes enum
   arguments as plain numbers.

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
| `Control.MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick` | `Button` (`MouseButton` value converted from the input action, §13.12; 0 for an action without a button), `X`, `Y` (pixels, relative to the control) |
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
| `Party.MemberAdded`, `MemberRemoved`, `AssistTargetRemoved` | `Player` (the member) when the member object is available, else no field (P90, BIN 2026-10-11) |
| `Window.UIScaleChanged` | `Index` (the `UIScalingGroupType` that changed), `Value` (the new scale, number) |
| all other events (`Click`, `Load`, `Unload`, `Update`, every `…Changed` without a row above, `EffectsCleared`, `ItemsRefreshed`, …) | empty table |

`Party.AssistTargetRemoved` was captured in game with an empty table (S3, P62), which
agrees with the binary (no field name wired to it).

> **Correction (2026-10-11, P90):** the sentence above and the P31 reading behind it are
> corrected by a BIN reading of the party events: the member and assist-target events share
> one argument routine that sets `Player` when it has the member object, so the empty
> `AssistTargetRemoved` capture is the case without an object, not the absence of a field.
> `LeaderChanged` and `Player.PartyChanged` carry no field.

> **Answered (2026-10-11, P90; was OPEN):** `Party.MemberAdded` and `MemberRemoved` carry
> `Player` (the member) when the member object is available, else an empty table. Tessera
> always provides it (§15.4).

<a id="9-4"></a>
### 9.4 Per-frame update

`Control:SetWantsUpdates(true)` subscribes the control to the UI's per-frame tick; the host
then fires `Update` on it once per frame with an empty `A` (S2, BIN). It stays on until set
false or the control is destroyed (P48):

- `Update` is a **broadcast** (§9.7), not a notification: it is delivered in step 3 of the UI
  tick (§3.3) to every subscribed control, whether or not the control is visible, displayed,
  enabled, parented or inside an open window. Plugins rely on this for timers on hidden
  controls.
- Subscribers are called in subscription order: the order in which the controls turned
  updates on. Calling `SetWantsUpdates` with the value it already has does nothing (no
  re-subscription, no change of order).
- A control that turns updates on while the broadcast is running is appended and receives
  `Update` in the same tick. A control that turns updates off during the broadcast is not
  called again, from that moment on (also later in the same tick).
- `Update` carries no time value; plugins measure time with `Turbine.Engine.GetGameTime()`,
  which is constant within one frame (§12). Plugin timers are `Update` handlers; the API has
  no other timer.
- Tessera (decision 0014 D8): the broadcast runs in frame-loop step 7 as part of the UI
  tick (§3.3) and is never cut short by a per-frame time slice; each handler call has its
  own instruction budget.

Worked example: controls A, B and C call `SetWantsUpdates(true)` in that order; during A's
`Update` handler, C calls `SetWantsUpdates(false)` and D calls `SetWantsUpdates(true)`. In
that tick the handlers run for A, B and D; in the next tick for A, B, D.

> **OPEN:** when the original drops the list entry of a control that turned updates off
> (removal is deferred; until then a control that turns updates back on keeps its old place
> in the order). Tessera: turning updates back on appends the control at the end.

<a id="9-5"></a>
### 9.5 Shared singletons

Objects such as `LocalPlayer.GetInstance()`, `Turbine.Chat` and `Turbine.Plugin` (class level)
are shared by every plugin **in the same apartment**; one plugin replacing a handler removes
another's (S2). Different apartments get different tables. Plugins cope by using handler
tables (§9.2 step 4) or private apartments; the host must support both.

<a id="9-6"></a>
### 9.6 Re-entrancy and ordering

- Events are delivered on the main thread. Events that the plugin layer fires itself
  (`Closing`/`Closed` from `Close()`, `EnabledChanged`, `ScrollBar.ValueChanged`, the
  `Load`/`Unload` events) are delivered synchronously before the call returns. Events that
  come from native notifications (mouse, focus, size, position, visibility, activation and
  the class-specific events listed in §9.7) follow the queueing rule of §9.7: synchronous
  when no notification is being delivered, otherwise after the current one.
- `Turbine.Chat.Received` is not re-entered: a chat line produced while a `Received`
  dispatch is running (for example by `Shell.WriteLine` in a handler) is not delivered to
  that apartment's `Received` handlers (BIN).
- A handler that destroys or re-parents the sender must not crash the host; remaining
  handlers of that firing still run.

**Setters and their change events (P58).** "Fires" means the event is raised on the same
control; "through the queue" means by a notification (§9.7).

| Call | Event | Rule |
|---|---|---|
| `SetSize`, `SetWidth`, `SetHeight` | `SizeChanged` | Through the queue, only when the size after the limits of §13.15 differs from the size before. Not fired for resizes made by the immediate re-layout that follows a plugin resize of a control with edge attachments (§13.16). |
| `SetPosition`, `SetLeft`, `SetTop` | `PositionChanged` | Through the queue, only when the control's position on screen changed. |
| `SetVisible` | `VisibleChanged` | Through the queue, only when the control's own visibility flag changed; showing or hiding a parent does not fire it on the children. |
| `SetEnabled` | `EnabledChanged` | Fired directly, only when the value changed. The plugin layer keeps the flag itself: on a plain `Control` it changes nothing else (mouse and key events keep arriving). |
| `ScrollBar:SetValue(v)` | `ValueChanged` | `v` is converted to 32 bits (scroll-bar numbers, below) and compared with the current value; if equal, nothing happens. Otherwise it is clamped, comparing **as unsigned 32-bit numbers**, in this order: first, a value above the maximum becomes the maximum; then a value below the minimum becomes the minimum. The result is stored and `ValueChanged` fires directly, even when the clamped value equals the old one. So a negative `v` (a large unsigned number) clamps to the maximum. Raises an error while the scroll bar is bound (§13.7). |
| `Window:Activate()` | `Activated` (and `Deactivated` on the previously active window) | Through the queue, only when the window was not already active (§13.14). |
| `Control:Focus()` | `FocusLost`, `FocusGained` | Through the queue, only when focus actually moves (§13.13). |

<a id="9-6-1"></a>
**Scroll-bar numbers (P58).** A scroll bar keeps five 32-bit numbers: minimum, maximum,
value, large change and small change. A new scroll bar has minimum 0, maximum 100, value 0,
large change 10 and small change 1. All comparisons below are **unsigned**.

- **Conversion.** Each of the five setters truncates its Lua number toward zero to a 64-bit
  integer and keeps the low 32 bits: −5 is stored as 4294967291, exactly like 4294967291;
  a number outside the 64-bit range (or NaN) is stored as 0. The five getters
  (`GetMinimum`, `GetMaximum`, `GetValue`, `GetLargeChange`, `GetSmallChange`) return the
  stored bits as an unsigned number, 0 to 4294967295; a stored −5 reads back as 4294967291.
  (Control geometry getters such as `GetLeft` return signed numbers; the unsigned rule is
  specific to these five.)
- **`SetMinimum(m)`.** If `m` equals the current minimum, nothing happens. Otherwise it is
  stored; if it is above the maximum, `SetMaximum(m)` runs with the rules below; then, if
  the value is below the minimum, `SetValue(minimum)` runs (and fires `ValueChanged`).
- **`SetMaximum(n)`.** If `n` equals the current maximum, nothing happens. Otherwise it is
  stored; if it is below the minimum, `SetMinimum(n)` runs; then, if the value is above the
  maximum, `SetValue(maximum)` runs (and fires `ValueChanged`).
- **Consequence.** Through the API the order minimum ≤ value ≤ maximum (unsigned) always
  holds: a minimum above the maximum cannot be reached, because each of the two setters
  drags the other bound along. If such a state existed, `SetValue` would always give the
  minimum, since the minimum clamp comes last. Negative bounds are not usable: they are huge
  unsigned numbers.
- **`SetLargeChange(n)`.** If equal to the current large change, nothing happens. Otherwise
  it is stored; if it is above maximum − minimum (32-bit difference), it becomes
  maximum − minimum; then, if it is below the small change, it becomes the small change.
  **`SetSmallChange(n)`**: if equal, nothing happens; otherwise the smaller of `n` and the
  large change is stored. Neither fires an event, and changing the minimum or maximum does
  not re-check either change.
- **Thumb.** The thumb position is the fraction (value − minimum) / (maximum − minimum),
  set at creation and by every `SetValue` that gets past the equality test (also those run
  by `SetMinimum`/`SetMaximum`); it is 0 when maximum equals minimum. A range change that
  does not move the value leaves the position as it was. The thumb size is the fraction
  large change / (maximum − minimum), set at creation and by every `SetMinimum`/`SetMaximum`
  that changes a bound while maximum ≠ minimum; `SetLargeChange` does not update it. Both
  fractions are 32-bit floats of the unsigned differences.

Worked example (synthetic, a new scroll bar, not bound): `SetMinimum(-10)` stores the
minimum 4294967286, which is above the maximum 100, so the maximum becomes 4294967286 too;
the value 0 is below the minimum, so it becomes 4294967286 and `ValueChanged` fires once.
`GetMinimum()`, `GetMaximum()` and `GetValue()` now return 4294967286. `SetMaximum(10)` then
stores 10, which is below the minimum, so the minimum becomes 10 (inside that call the value
4294967286 is not below 10, so nothing more happens); back in `SetMaximum`, the value is above
10, so it becomes 10 and `ValueChanged` fires again. Result: minimum, maximum and value 10, two
`ValueChanged` in all. A state "minimum −10, maximum 10" is therefore never reached. With
minimum 5 and maximum 10, `SetValue(20)` gives 10 and `SetValue(-1)` (stored as 4294967295)
gives 10 as well, while `SetValue(2)` gives 5.

> **OPEN:** whether `TextBox:SetText` fires `TextChanged` (`TextChanged` comes from the text
> box's native "text changed" notification, which user edits certainly produce), whether
> `CheckBox:SetChecked` fires `CheckedChanged`, and whether `ListBox:SetSelectedIndex` /
> `SetSelectedItem` and `TreeView:SetSelectedNode` fire `SelectedIndexChanged` /
> `SelectedNodeChanged` (they pass a "notify" flag to the native list, whose effect was not
> traced). Tessera: fire each of them, through the queue, when the value actually changes.

<a id="9-7"></a>
### 9.7 Delivery paths, queueing and order (P47, P48)

The UI events of plugin controls reach Lua by two paths.

**Notifications.** Every native control (UI element) reports what happens to it as a
*notification*: pointer entered or left, button pressed, released, clicked or double-clicked,
pointer moved, hovered, wheel turned, focus gained or lost, size or position changed,
visibility changed, window activated or deactivated, drag events, and class-specific ones
(button activated → `Button.Click`, text changed → `TextBox.TextChanged`, value or
selection changes of lists, trees and scroll bars). Posting a notification works like this:

1. The notification gets the next number of a global sequence.
2. If no notification is being delivered at the moment, it is delivered now: first to the
   native handling of its **source** control and the listeners attached to that control
   (the plugin control's own listener among them), then, unless a listener stops it, to the
   source's parent and its listeners, and so on up to the root. A listener that already saw
   that sequence number is skipped. While this runs, the manager is "delivering".
3. If a notification is being delivered already (because the code that posted this one runs
   inside some handler of an earlier notification, for example a plugin `MouseClick` handler
   that calls `SetSize`), the new notification is **appended to a single FIFO queue**. When
   the outermost delivery finishes, the manager delivers the queued notifications one after
   another, in queue order, including any that are appended while it drains the queue, and
   only then returns to the code that posted the outermost notification.

The plugin control's listener turns a notification into a Lua event **only when the control
itself is the source**; notifications of child controls that travel up through a parent
never fire events on the parent's Lua object. The plugin listener never stops a
notification and never reports it as handled.

Consequences implementers must reproduce:

- Outside any event handler (for example in top-level code or an `Update` handler), a call
  such as `SetSize` delivers `SizeChanged` before it returns.
- Inside a handler of a notification-based event (`MouseClick`, `MouseDown`, `SizeChanged`,
  `FocusGained`, …), the events caused by the handler's own calls are delivered after the
  current event has been delivered to all of its handlers, in the order the calls were made,
  but still before the input or tick processing that started the chain continues.
- Broadcast events (`Update`, `KeyDown`, `KeyUp`, below) are not notifications; a call made
  in an `Update` handler therefore delivers its notification synchronously.

Worked example (synthetic): control C has handlers `MouseClick` (calls `C:SetSize(50, 20)`
then `C:SetVisible(false)`), `SizeChanged` and `VisibleChanged`, each appending its name to a
log. One click over C (its size was 40 × 20) gives the log `MouseClick`, `SizeChanged`,
`VisibleChanged`. If `MouseClick` also calls `print(C:GetWidth())`, it prints 50: the size
itself changes at once, only the event waits. Tessera (decision 0014 D8): the next hit test
(§13.11) also reads the new size and position, so the next input event is routed against
the control's current geometry.

**Broadcasts.** A few messages go from the manager to every subscribed control instead:
`Update` (subscription by `SetWantsUpdates`, §9.4) and the key messages behind `KeyDown` and
`KeyUp` (subscription by `SetWantsKeyEvents`, §13.13). A broadcast calls the subscribers in
subscription order and checks nothing about the control (visibility, enabled state, focus,
window activation). Subscriptions added during a broadcast are called in the same
broadcast; a subscription removed during it is skipped from then on.

**Several handlers, several controls.** Within one event on one object, handler order is
§9.2. Between objects, the order is the notification order above, or subscription order
for broadcasts. For the pointer, §13.12 gives the order of `MouseLeave`/`MouseEnter` across
ancestors.

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

> **OPEN (implementation, M5, `crates/plugins`, 2026-10-08):** steps 1 and 2 read the file
> at call time, on the thread that runs plugin code (the main thread), up to the 16 MiB data
> cap; `docs/ARCHITECTURE.md` §21 lists no exception for this read (its exceptions (a)–(c)
> are the image header probe, the writes and flush of a closing state, and `import` source
> reads). Spec owner to decide: add a §21 exception (d) for `PluginData.Load` reads, or allow
> the asynchronous form (step 2) to read on a worker, since its handler runs later anyway.
> Tessera until then: reads at call time as stated here (the latest queued `Save` of the
> file, when one is not yet on disk, is returned without a read), charging the bytes read and
> parsed to the caller's instruction budget (§3.3).

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
6. Nothing follows the value (P60): the file ends with the value's last character (`}` for a
   table), with **no** final line end and no byte-order mark. The bytes of strings are written
   as they are in Lua (UTF-8 from plugins, §3.4).

Byte-exact examples (synthetic): saving the number 5 gives the 15 bytes `return 5.000000`;
saving the string `a"b` gives `return "a\"b"`; saving `true` gives `return true`; saving an
empty table gives `return ` CR LF `{` CR LF CR LF `}` (15 bytes).

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

> **OPEN (implementation, M5):** the comma separator "after `=`" is ambiguous with the
> entry separator (`["a"] = 1,5` could be a keyed field and a positional item). The M5
> reader takes it as a decimal only when the numeral is all digits, a `,` and digits
> follow, and the next token ends the field (`,`, `;`, `}` or `]`), which is how a
> comma-locale writer's output always looks (six decimals, never positional items).

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

> **OPEN (implementation, M5):** two cases are not stated above. A boolean table key
> (written `[true] = …` by the original, which reads back) is skipped by the M5 host,
> whose value model keeps number and string keys only. A NaN or infinite value is written
> as `nil` (the original writes the runtime's text, which reads back as nil or not at all),
> and an entry with an infinite number key is skipped, so the file stays readable.
> Tessera bounds of the M5 host: at most 1,024 queued plugin-data items per apartment and
> 256 MiB of serialized data waiting to be written; beyond them `Save` and `Load` raise an
> error. A later `Save` of a file that is still queued replaces the queued bytes and adds
> its `onDone` to the queued write (per-key order and call order of the callbacks kept).

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
- Tessera (decision 0014 D8): "later, between frames" means as items of the plugin host's
  deferred work in frame-loop step 7, in queue order (§3.3): each queued write with its
  `onDone` callback, and each asynchronous `Load` handler, is one item. Writes made while an
  apartment closes (§6.3 step 5, including the unload at client exit) are written
  synchronously, as above. When that close happens inside a frame (step 1 or 7), the write
  and the flush block the main thread; whether that is allowed is the §6.3 OPEN on blocking
  writes at unload.

**The write step (P60).** Whether run at once (state closing) or later, one write does:

1. Create any missing folders of the path. If that fails, nothing is written and the result
   is failure.
2. Open the file for writing in text mode, truncating an existing file. If that fails, the
   result is failure.
3. Write the serialised text with one formatted write and close the file. The result is
   success; the outcome of the write itself is **not** checked (a write that runs out of
   disk space still reports success).
4. If `onDone` is a Lua function **and the apartment that called `Save` still exists**, call
   `onDone(success, message)` in protected mode: `message` is `""` on success and, on
   failure, a fixed client message saying that an internal error prevented saving (the same
   text for both failure causes; not reproduced here). Otherwise no callback is made.

So a callback is never called into a closed apartment. Tessera: same rule; a callback whose
apartment has closed is dropped.

> **OPEN:** whether a write that was queued (not yet run) when its apartment closed is still
> performed later; step 1–3 do not depend on the apartment. Tessera never has such writes,
> because it flushes an apartment's writes before closing it (above).

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
| `WriteLine(text)` | string → nothing | Adds a line to the chat window with chat text type 8 (a system-log type that has no name in `Turbine.ChatType`). The text is converted to the client's UTF-16 text and posted **unchanged**: nothing is escaped (P75). The chat log draws each line in a text element built from the template that the chat-log element names (`UI_ChatLog_TextElement`; in this install `ChatLogListboxItemTemplate`), and that template has `UICore_Text_parse_markup_tags` = true (O7). So the markup of §13.4 (`<rgb=#RRGGBB>…</rgb>`, `<u>`, `<p>`, `<li>`; `<b>` and `<i>` have no visible effect, `fonts.md` §8.6) **is rendered** in `WriteLine` lines, and link markup is handled as in `fonts.md` §9.1. Tessera: render it the same way; the line's base colour and font are those of chat type 8 (`fonts.md` §9.3). |

Registrations of an apartment are removed when it unloads (§6.3).

> **OPEN:** whether `WriteLine` lines reach `Turbine.Chat.Received` handlers (type 8 is not in
> the `ChatType` enum). Tessera: deliver them with `ChatType = 8`, no `Sender`, as queued
> game-raised events in frame-loop step 7 (§11.3, decision 0014 D8), never inside
> `WriteLine` and never re-entrantly: not to an apartment in the line's skip set (§9.6;
> the interim Tessera rule of the §11.3 OPEN on re-entrancy, pending a revision of decision
> 0014).

> **Note (implementation, M5):** the M5 host cuts a `WriteLine` text longer than 4,096
> bytes at a character boundary (a Tessera bound on chat lines), and keeps at most 256
> chat lines waiting for `Received` delivery; further lines are shown but not delivered to
> plugins (logged).

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
follows (BIN). The plugin layer converts both from the client's text to UTF-8 and passes
them **unchanged**: it neither trims, nor splits, nor changes letter case (P61). Any such
processing happens earlier, in the client's general chat command-line parser (its shell
module contains a tokenizer that treats double quotes specially; whether it is applied to
plugin command lines was not traced). > **OPEN:** the rules of that parser: whether surrounding
whitespace of `arguments` is trimmed, whether runs of spaces are kept, what happens to
double quotes, and whether `command` keeps the user's letter case or is the registered
spelling. Tessera: keep case as typed, remove exactly one separating whitespace character,
keep everything else (including quotes and trailing whitespace).

<a id="11-3"></a>
### 11.3 `Turbine.Chat`

Static event source: `Turbine.Chat.Received = handler` receives the chat lines the client
shows, with the fields of §9.3. `ChatType` is the client's chat text type; the named values
are in Appendix A (they are keys of the chat-text-type enum 0x230002A8, P41), other values
can occur. Message text may contain the client's markup (§13.4).
> **OPEN:** the full grammar of markup in `Message` (item and other links).

**When `Received` is delivered in Tessera (Tessera rule, decision 0014 D8 and its Known
difference 21).** Every chat line the client shows produces one queued `Received` event:
lines from session events, Tessera's own system notices, `WriteLine` lines (§11.1) and
error lines (§6.6). The code that takes the line in never calls Lua. It appends the event,
with its argument values (§9.3) taken at that moment, to the plugin host's deferred-work
queue, and the host delivers it in frame-loop step 7, one line being one item, in queue
order (§3.3), to the `Received` handlers set at delivery; an apartment closed in between
gets nothing. The chat window shows the line at once; only the delivery to plugins waits,
so it may come in a later frame when the deferred work of a frame has used its 2 ms. A line
produced while plugin code runs (in step 1 or 7) is queued the same way, so `Received` is
never entered from inside a handler.

> **OPEN (re-entrancy of queued `Received`, decision 0014 open review item 1):** delivered
> as above "to the handlers set at delivery" (D8 as written), a line that a `Received`
> handler writes would reach its own apartment one item later, against §9.6 (BIN), and an
> echoing plugin would use the whole deferred-work slice in every frame. Decision 0014 lists
> the fix as a must-fix review item but its D8 text does not contain it yet. **Tessera rule
> until decision 0014 is revised** (the same interim rule as `plugin-runtime.md` §8.4; it
> refines D8's "handlers set at delivery" and does not otherwise change it): every queued
> `Received` event carries a **skip set** of apartments, and its delivery calls no handler
> of an apartment in that set. A line produced while no queued `Received` event is being
> delivered gets an empty skip set (in Tessera a `Received` handler runs only inside such a
> delivery). A line produced while a queued `Received` event is being delivered gets that
> event's skip set **plus** the apartment whose handler is running. So a `WriteLine` in a
> `Received` handler never reaches its own apartment, and two or more echoing apartments
> cannot feed each other: a chain of lines that started from one delivery grows its skip
> set by one apartment per step and ends after at most as many steps as there are
> apartments with `Received` handlers. This matches the nested synchronous model of §9.6,
> under which every dispatch of the chain is still running when the next line is written.
> Invariants 61 and 65 test it.

> **OPEN:** when the original delivers `Received`: synchronously when the chat window takes
> the line in, or later in the frame. Decision 0014 (Known difference 21) asks this; no
> surveyed plugin behaviour is known to depend on it. Tessera: the queued delivery above.

---

## 12. Engine (`Turbine.Engine`, static)

| Function | Result |
|---|---|
| `GetDate()` | Table, local time of the current moment (P35): `Year` (four-digit), `Month` 1–12, `Day` 1–31, `Hour` 0–23, `Minute` 0–59, `Second` 0–60, `DayOfWeek` **1 = Sunday … 7 = Saturday**, `DayOfYear` **1-based** (1–366), `IsDST` boolean. |
| `GetLocalTime()` | Seconds since 1970-01-01 00:00 UTC, as a **32-bit signed** integer (wraps after 2038-01-19). |
| `GetGameTime()` | The client's frame clock in seconds (fractional); constant within one frame; only differences matter. When Tessera advances it relative to input handled in window-system callbacks: the §3.3 OPEN on the frame clock. |
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
- **Initial geometry (P64, P65):** every new control, of every class, is created at left 0,
  top 0 with width 50 and height 50 (BIN): `GetSize()` of a fresh `Turbine.UI.Control()`
  returns 50, 50. The seven game-styled classes of §14.2 name a layout element as the
  **base** of their element (P65). Inheritance replaces the base's rectangle by the
  descriptor's own 0, 0, 50 × 50 (`ui-layout.md` §7.2 step 1), so the layout element does
  **not** set the initial size; its inherited children are re-laid out from the base's design
  size to 50 × 50 by the design-time rule (`ui-layout.md` §7.3 step 8, §8.2). The base's
  **properties** are inherited, including size limits: the game-styled windows of this
  install inherit `UICore_Element_min_width` 150 and `UICore_Element_min_height` 100, which
  are the very limits of §13.15, so `GetMinimumWidth()` of a fresh game-styled window
  returns 150 and its first `SetSize` is clamped to at least 150 × 100 (values read from the
  layout at run time). > **OPEN:** whether the creation itself applies those limits (a
  fresh game-styled window 50 × 50 or 150 × 100 before any resize). Tessera: 50 × 50 at
  creation; the limits apply from the first resize, as §13.15 states for every limit.
- **Tree:** a control is shown only inside a `Window` chain (S2, Reported); top-level windows
  have no parent. `SetParent(nil)` detaches; parenting a control to itself is an error (BIN).
  Setting a new parent removes the control from the old one (S2, Reported).
- **Visibility:** new controls and windows start hidden; plugins call `SetVisible(true)`
  (S2/S4, Reported). `IsDisplayed()` = own visibility and every ancestor's (S2, Reported).
- **Transparency:** a bare `Control` draws nothing until it gets a background image
  (`SetBackground`) or colour (`SetBackColor`) (S2; BIN: a new element has no base image
  and no overlay, §13.17).
- **Stacking:** among siblings a higher `ZOrder` draws above a lower one and is hit first by
  the mouse; `Window:Activate()` raises a window above the siblings of equal `ZOrder`
  (S2; BIN, §13.14).
- **Opacity:** 0.0 (fully transparent) to 1.0 (opaque); rules in §13.19 (P70): it is the
  opacity of the control's own drawing surface, so on a window it fades the whole window
  with all its children that are drawn into its surface, and on another control it has a
  visible effect only while the control has a non-zero stretch mode (§13.18). Children with
  a surface of their own: §13.19 OPEN (Tessera: not faded by the window, decision 0015).
- **Mouse:** a control with `SetMouseVisible(false)` does not take mouse input; the controls
  underneath handle it, but its own children still do (S2; BIN, §13.11). Capture, clicks,
  hover and the wheel: §13.12.
- **Keys:** only controls with `SetWantsKeyEvents(true)` receive `KeyDown`/`KeyUp`, and they
  receive them whether or not they have focus (S2; BIN, §13.13). Key events carry
  key-binding **actions**, not raw keys (BIN, §9.3, §14.4).
- **Focus:** `Focus()` asks for keyboard focus; it takes effect at once only when the
  control's window is the active window (S2; BIN, §13.13). `HasFocus()`, events
  `FocusGained`/`FocusLost`. Tessera: a text box takes focus when clicked (a Tessera rule,
  not a sourced fact).
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
| Size limits | `Get/SetMinimumWidth`, `Get/SetMinimumHeight`, `GetMinimumSize()`→w,h / `SetMinimumSize(w,h)`, same for `Maximum` — members of **`Control`** in this build (earlier dumps list them on `Window`; windows inherit them, so existing plugins are unaffected). Rules: §13.15. |
| Edge attachment | `AttachEdges(left, top, right, bottom)` takes four `EdgeAttachmentType` numbers, one per edge, in that order; `GetEdgeAttachments()` returns the four in the same order. Rules: §13.16. |
| Tree | `GetParent/SetParent(control or nil)`, `GetControls()`→ControlList (live view of children), `GetZOrder/SetZOrder(n)` |
| State | `IsVisible/SetVisible(b)`, `IsDisplayed()`, `IsEnabled/SetEnabled(b)`, `GetOpacity/SetOpacity(0..1)`, `IsMouseVisible/SetMouseVisible(b)`, `GetWantsKeyEvents/SetWantsKeyEvents(b)`, `GetWantsUpdates/SetWantsUpdates(b)`, `GetAllowDrop/SetAllowDrop(b)`, `Focus()`, `HasFocus()` |
| Look | `GetBackColor/SetBackColor(Color)`, `GetBackColorBlendMode/SetBackColorBlendMode(BlendMode)`, `GetBackground/SetBackground(path, DID or Graphic)` (§13.8), `GetBlendMode/SetBlendMode(BlendMode)` (applies to the background image), `GetStretchMode/SetStretchMode(n)` (§13.18). A blend-mode number above 8 raises an error. How these are drawn: §13.17. |
| Static | `Control.IsAltKeyDown()`, `Control.IsControlKeyDown()`, `Control.IsShiftKeyDown()` |
| Events | `MouseEnter`, `MouseLeave`, `MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick`, `MouseHover`, `MouseMove`, `MouseWheel`, `KeyDown`, `KeyUp`, `FocusGained`, `FocusLost`, `EnabledChanged`, `VisibleChanged`, `PositionChanged`, `SizeChanged`, `DragStart`, `DragEnter`, `DragLeave`, `DragDrop`, `Update` |

**Size warning.** When a control becomes wider or taller than the graphics device's largest
texture dimension, the original reports a warning (with the place in the Lua source where the
control was created) to the error listeners; it does not clamp the size and raises no error
(BIN). Tessera: same, using the renderer's maximum texture size. Tessera's own surface
limits (decision 0015 §7): a surface is at most the device's largest texture dimension and
at most 16,384 pixels on each axis; beyond that its canvas covers only the part of the
control that lies on the screen, and the case is reported once, like this warning.

<a id="13-3"></a>
### 13.3 `Window` (extends `Control`)

Top-level container with no frame of its own (S2). Members: `GetText/SetText(title)`,
`Activate()` (makes it the active window and brings it to the front, S2; exact rules
§13.14), `Close()`, `GetRotation()`/`SetRotation(t)`, and the
global-scaling members `RegisterForGlobalScaling()`, `UnregisterForGlobalScaling()`,
`SetUIScalingGroup(group)`/`GetUIScalingGroup()` (a `UIScalingGroupType`),
`SetScale(n)`/`GetScale()`, `SetScalingOriginPoint(…)`/`GetScalingOriginPoint()` (BIN).
Events: `Activated`, `Deactivated`, `Closing`, `Closed`, `UIScaleChanged`.

- **Rotation:** `SetRotation` takes a table with number fields `x`, `y`, `z` (lower case);
  `GetRotation` returns the last value set (BIN). Units (degrees), pivot (the window's
  centre) and drawing: §13.20 (P72).
- **Scaling:** `SetScale` with a value that is not greater than 0 is an error. Units (a
  factor, 1 = unscaled), how the window's own scale combines with its group's and with the
  global UI scale, the origin point, `GetScale`, the group members and `UIScaleChanged`:
  §13.21 (P73, P74).

`Close()` algorithm (S2; BIN, fourth pass): only when `IsVisible()` is true (a hidden
window ignores `Close()`, and fires nothing), fire `Closing` with
`Cancel = false`; if a handler set `Cancel` true, stop; otherwise make the window not
visible and fire `Closed`. Closing does not destroy the window. > **OPEN:** whether Escape closes plugin windows automatically. Recommended: no;
plugins handle the `Escape` action themselves. (In Tessera an Escape that no control
consumes, including one typed into a focused text box, goes on to game code as the escape
command and to the plugins' `KeyDown` broadcast, decision 0014 D2 and "Escape with a UI
attached"; what game code then does with an active plugin window belongs to
`game-panels.md`.)

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
`b` and `i` set flags that no text routine of the client tests, so they have no visible
effect (`fonts.md` §8.6); link markup is recognised whether or not markup is enabled
(`fonts.md` §9.1).

**Which element the members act on (P67).** A `Label`'s text, font, colours, alignment and
selection belong to one **text element**: the control's own element for `Label`, `TextBox`,
`Button`, `CheckBox` and the game-styled button, and a descendant for the game-styled text box
and check box (§14.2.2).

**Defaults and conversions (P71, BIN).**

- **Font.** `GetFont()` returns 0 (`Font.Undefined`) until `SetFont` succeeds, even for the
  game-styled controls whose layout gives their text element a font. `SetFont(f)` does
  nothing when `f` equals the stored value, or when `f` is not the DID of a record that loads
  as a font (record type 0x42; any other value, including 0, is ignored without an error);
  otherwise it stores `f` and sets the text element's `UICore_Text_font` to it (normal
  slot, `fonts.md` §9.1 rule 3). The font the text is **drawn** in is the text element's normal-slot font
  (`fonts.md` §9.1): for a plain `Label`, `TextBox`, `Button` or `CheckBox` that has never had
  `SetFont`, the locale default font (`fonts.md` §9.1, "Initial font"; 0x42000028 in the
  English locale record of this install); for the game-styled controls, the font of their
  layout element (read at run time).
- **Fore colour.** `SetForeColor(c)` with nil (or a non-`Color` value) raises an error;
  otherwise it sets the text element's `UICore_Text_font_color`. `GetForeColor()` reads the
  text element's colour properties; when the text element has none, it returns opaque black
  (R 0, G 0, B 0, A 1). The default outline colour is also opaque black (`fonts.md` §8.4).
  > **OPEN:** the colour a text element **draws** with when it has no colour property (the
  > dictionary gives `UICore_Text_font_color` no default, O7; the text widget's built-in
  > value was not traced, `fonts.md` §8.4 "Not covered"). Tessera: draw with the value
  > `GetForeColor()` reports (opaque black) until an owner screenshot of a plain label
  > settles it.
- **Alignment.** `GetTextAlignment()` returns 1 (`TopLeft`) for every new label. The value is
  stored on the plugin control, not read back from the element, so a game-styled control
  whose layout centres its text still reports 1. `SetTextAlignment(a)` does nothing when `a`
  equals the stored value (so the first `SetTextAlignment(TopLeft)` on a new label changes
  nothing, even on a game-styled control drawn centred); otherwise it stores `a` and sets the
  text element's justification properties (`fonts.md` §7.4, values of enum mapper
  0x23000177):

  | `a` | Horizontal justification | Vertical justification |
  |---|---|---|
  | 1 `TopLeft` | 5 (left) | 2 (top) |
  | 2 `TopCenter` | 3 (centre) | 2 |
  | 3 `TopRight` | 4 (right) | 2 |
  | 4 `MiddleLeft` | 5 | 3 (centre) |
  | 5 `MiddleCenter` | 3 | 3 |
  | 6 `MiddleRight` | 4 | 3 |
  | 7 `BottomLeft` | 5 | 1 (bottom) |
  | 8 `BottomCenter` | 3 | 1 |
  | 9 `BottomRight` | 4 | 1 |
  | 0 `Undefined` | 5 | 2 |
  | above 9 (or negative, compared unsigned) | 5 | 2, and the stored value becomes 0 |

  (The table is the client's; the drawn position then follows `fonts.md` §7.4, including its
  rule that reads both axes through one table.)
- **Selection** (all positions **1-based**, counted in the client's UTF-16 code units, the
  same units as `GetTextLength()`; a character outside the Basic Multilingual Plane counts
  as two):
  - `GetSelectionStart()` = the first selected position (the text element's 0-based start
    + 1); with no selection it reports whatever start the text element holds, + 1
    (Probable: the caret position).
  - `GetSelectionLength()` = the number of selected units (0 with no selection).
  - `SetSelection(start, length)`: when `length` ≤ 0, deselect; otherwise the start is
    max(`start`, 1), then min(that, text length), and the selection covers `length` units
    from there.
  - `SetSelectionStart(s)`: with no selection, it only deselects (nothing becomes
    selected); otherwise it moves the selection to start at min(max(`s`, 1), text length) and
    keeps its length.
  - `SetSelectionLength(n)`: `n` ≤ 0 deselects; otherwise the selection keeps its start and
    covers `n` units.
  > **OPEN:** whether the end of a selection that runs past the text is clamped by the text
  > element. Tessera: clamp the end to the text length.
  > **OPEN:** (clean-room implementer, 2026-10-08) `tessera-ui` exposes no caret or
  > selection of a text element, so the plugin layer cannot read "the text element's start"
  > nor show a selection set by `SetSelection*`. Tessera, until the UI offers a selection
  > API: the layer keeps its own selection; after each user edit of a text box it puts the
  > caret (start, length 0) at the end of the edited span, so `GetSelectionStart` and
  > `InsertText` follow typing, but not caret moves made without an edit; `SetSelection*`
  > changes only the layer's selection (no visible caret or highlight).

> **OPEN:** (clean-room implementer, 2026-10-08) the value `IsMultiline()` reports for a
> new label, and whether the label's text element wraps before `SetMultiline` is called.
> Tessera: reports true and writes `UICore_Text_one_line` only when `SetMultiline` is
> called.

<a id="13-5"></a>
### 13.5 `TextBox` (extends `Label`)

Editable text. Members: `IsReadOnly/SetReadOnly(b)`. Events: `TextChanged`,
`EnabledChanged`. Typing in a focused text box must not trigger game key bindings.

<a id="13-6"></a>
### 13.6 `Button` and `CheckBox` (extend `Label`)

`Button`: events `Click` (empty argument table, BIN), `EnabledChanged`. Unskinned;
`Turbine.UI.Lotro.Button` is the skinned variant. `Click` is not derived from `MouseClick`:
the plugin button fires `Click` when its native button reports its own "activated"
notification, and fires `MouseDown`/`MouseUp`/`MouseClick` independently by the rules of
§13.12 (BIN). > **OPEN:** which mouse buttons (and keys) make the native button report
"activated", and whether a disabled plugin button still does. Tessera: the left button, on
release over the button, only when enabled.
> **OPEN:** (clean-room implementer, 2026-10-07) the order of `Click` and `MouseClick` for one
> left click on a plugin button. With the delivery of §9.7 and `ui-behaviour.md` §4.6, the
> button reacts to its own `Element_mouse_release` notification, so its activation is queued
> while the release is delivered and runs before the click notification that the release
> step posts afterwards: `tessera-ui` gives `MouseDown`, `MouseUp`, `Click`, `MouseClick`.
> Whether the native button reacts to the release notification or to the input before it was
> not traced.

`CheckBox`: `IsChecked/SetChecked(b)`, `GetCheckAlignment/SetCheckAlignment(ContentAlignment)`;
events `CheckedChanged`, `EnabledChanged`.

<a id="13-7"></a>
### 13.7 Scrolling and lists

- `ScrollBar` (extends `Control`): `Get/SetMinimum`, `Get/SetMaximum`, `Get/SetValue`,
  `Get/SetSmallChange`, `Get/SetLargeChange`, `Get/SetOrientation(Orientation)`, and
  replaceable parts `Get/SetDecrementButton`, `Get/SetIncrementButton` (Buttons),
  `Get/SetThumbButton` (Control). While a scroll bar is bound to a `ScrollableControl`,
  getting or setting its minimum, maximum, value or large change, and setting its small
  change, raise errors (BIN). Events `ValueChanged`, `OrientationChanged`. Defaults (0, 100,
  0, large 10, small 1), number conversion, `SetValue` clamp order, the range rules of
  `SetMinimum`/`SetMaximum` and `ValueChanged`: §9.6 (scroll-bar numbers).
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

> **OPEN:** (clean-room implementer, 2026-10-08) what `ControlList:Insert(i, c)` and
> `Set(i, c)` do to the child order of the control (where `c` goes among the siblings, and
> how that interacts with `ZOrder`). Tessera: both re-parent `c` as the last child (Set
> first detaches the child at `i`), so `Get(i)` is generally not `c` afterwards;
> `tessera-ui` has no call that inserts a child at a sibling position.

> **OPEN:** (clean-room implementer, 2026-10-08) how the plugin `ListBox` maps its members
> onto the list layout, and its defaults. Tessera rule: the layout of `ui-widgets.md`
> §8.3; `Orientation.Horizontal` is the row-first fill; `SetMaxItemsPerLine(n)` (n > 0) is
> the items per row (horizontal) or per column (vertical), otherwise `SetMaxColumns` /
> `SetMaxRows` give them; `GetOrientation()` of a new list box is `Vertical`; a left press
> on an item (or a control inside it) selects it with `SelectedIndexChanged`; the wheel
> scrolls by 20 pixels, or by the bound scroll bar's small change; `ItemAdded`,
> `ItemRemoved`, `SelectedIndexChanged` and `SelectedNodeChanged` go through the queue of
> §9.7. Trees: one column, nodes indented by `GetIndentationWidth()` (20 for a new tree)
> per level, a press selects and toggles a node (`ui-widgets.md` §9.4), filter and sort
> functions run on `Refresh`, `SetFilter` and `SetSortMethod`. Scroll bars: a new bar's
> `GetOrientation()` is `Horizontal` (0); the layer places the decrement button at the
> start, the increment button at the end and sizes the thumb along the track by large
> change ÷ (maximum − minimum), at least 8 pixels; a press on the track pages by the large
> change, the arrow buttons and the wheel step by the small change.

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
- **Tessera checks for `SetBackground(path)` (decision 0015 §4.3–§4.4, Tessera rules).**
  The call checks existence and format on the main thread without decoding, so that both
  errors are raised at call time: existence against an index of the whole `Plugins/` tree
  (paths matched as §4.4 says; the index is built by a directory walk off the main thread
  when the plugin host starts (no plugin is loaded before that first index is ready) and
  rebuilt when the available plugins are refreshed, §6.7, so
  a file added after the last walk is found only after the rebuilt index is swapped in), and
  format by reading at most the first 64 bytes of the file once (cached per path). Only TGA
  and JPEG are accepted; a `.png` or `.bmp` file raises the unsupported-format error (the
  formats named above; `plugin-api-usage.md` O6 asks whether the original accepts `.png`).
  The full decode runs later, off the main thread; if it fails after the header check
  passed (a corrupt JPEG body), the failure is reported to the apartment's error listeners
  (§6.6) and the layer draws nothing, where the original would have raised the error at the
  call (a stated deviation).
- `Turbine.UI.Graphic`: `Graphic()` empty, `Graphic(did)` or `Graphic(path)` (S2);
  accepted wherever a background is.
- Each call first removes the previous base image; the new image becomes the element's base
  image layer, drawn with the operation set by `SetBlendMode` (Replace until then): §13.17.
- Stretch modes: §13.18 (BIN, P69). The community descriptions that earlier revisions
  carried as Reported (S3 stubs) are kept there for comparison.

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
  > **OPEN:** (clean-room implementer, 2026-10-08) the look and metrics of an open plugin
  > context menu, how a sub-menu opens (hover or click), and whether a disabled item's
  > `Click` can fire. Tessera rule: a popup window of rows 20 pixels high in neutral art,
  > the width estimated from the text, check marks drawn as a prefix; a click opens a
  > sub-menu beside its row; a disabled item does nothing; the popup is activated, so a
  > press anywhere outside the open popups closes them.
- Enums `BlendMode`, `ContentAlignment`, `EdgeAttachmentType`, `FontStyle`, `MouseButton`,
  `Orientation`, `HorizontalLayout`, `VerticalLayout`: Appendix A.

The sections below (§13.11–§13.16) describe the client's general UI framework as plugin
controls experience it. They rest on BIN (P47–P58) unless marked. Plugin windows and
controls live in the same control tree as the client's own panels: the UI root has the
top-level windows (the client's and the plugins') as children, each window has its controls
as children, and so on. Every control's rectangle is relative to its parent's top-left
corner; a rectangle covers the pixels left … left + width − 1 and top … top + height − 1.

<a id="13-11"></a>
### 13.11 Hit testing and the pointer target (P49)

The **control under the pointer** is found by this search, starting at the UI root with the
pointer position:

1. A control that is not visible (its own `IsVisible()` is false) is not hit, and nothing
   inside it is hit: the search reports "no hit" for it.
2. Otherwise its children are tried from the topmost to the bottommost (the order of
   §13.14). A child is tried only if the point lies inside the child's rectangle; trying it
   means repeating these steps for that child, with the point made relative to it. The first
   child that reports a hit ends the search with that hit.
3. If no child reports a hit, the control itself is the hit when it is mouse-visible
   (`IsMouseVisible()`); otherwise it reports "no hit", and its parent goes on with the next
   lower sibling, and then with itself.

Consequences: a part of a control that lies outside its parent's rectangle never receives
the mouse (input is clipped by every ancestor); a mouse-invisible control is transparent to
the mouse, but its children are not, and the control below it (or its parent) receives the
mouse; among overlapping siblings the topmost wins; a hidden control and everything inside it
is transparent.

The search runs on every pointer movement, at the start and end of every mouse-button action
(§13.12.2), and in the UI tick when the pointer needs re-evaluating (§3.3). Its result is the
**pointer target**. The **hover target** is derived from it: when no control has captured
the mouse (§13.12.2), the hover target is the pointer target (which is always
mouse-visible); while a control has captured the mouse, the hover target is that control
when the pointer target is that control, and none otherwise.

Two states suppress the search (P49). When a pointer event (movement, or the start of a
mouse-button or wheel action) arrives, the UI first tests whether the pointer lies inside
the UI root's rectangle; if it does, it clears a manager-level **pointer-outside flag**. The
search is then skipped, and the pointer target is none, when that flag is still set, or when
the input layer reports the pointer **locked** (a state in which the input layer freezes the
reported pointer position and refuses to move the cursor). With no pointer target, the hover
target is none as well, which sends the `MouseLeave` events of §13.12.4. In this build the
pointer-outside flag is cleared when the UI is created and every write to it stores "clear",
so it never suppresses anything; Tessera may omit it. > **OPEN:** which client states lock
the pointer (expected: the camera's mouse-look drag). Tessera (decision 0014 D7): the lock
is the client's state. It turns on when a mouse-look press (the camera-steer command,
`CameraInstantMouseSteer`) is offered to the UI, the UI does not consume it and no item drag
is in progress, and it turns off after the end of the last mouse-look hold that turned it
on; during any item drag a right press never locks the pointer. While it is on, the pointer
position the UI sees stays frozen where the mouse-look press was made, and plugin controls
see no hover, enter or move events, so they get none during a camera drag. For mouse
buttons during the lock see §13.12.2.

**Geometry used by the search (Tessera rule, decision 0014 D8).** The search reads every
control's **current** position, size, visibility and z-order at the moment it runs: a change
made by a plugin call (`SetPosition`, `SetSize`, `SetVisible`, `SetZOrder`), a window move or
resize, or the re-layout of edge attachments (§13.16) applies to the next search at once,
even within one frame. A re-layout that Tessera defers for speed is completed before any
search reads a control it would move. What is drawn may lag behind by a frame or more
(decision 0015 Decision 1.5, "Hit testing during a transition"); hit testing never waits for
drawing.

> **OPEN:** the client's own panels can use two refinements of the point test (a per-pixel
> hit mask and a second geometric check) that the plugin API cannot set; whether a plugin
> control can ever be subject to them. Tessera: plain rectangles.

<a id="13-12"></a>
### 13.12 Mouse events (P50–P52)

<a id="13-12-1"></a>
#### 13.12.1 Mouse-button actions and `MouseButton`

The UI receives mouse buttons as **input actions** (keys of the input-action enum mapper,
§14.4; which physical button events start and end them is set by the key map,
`keymap.md` §12). Exactly nine actions are mouse-button actions for the UI:

| Action keys | Meaning (our own words) | Toggle type in this install (O6) |
|---|---|---|
| 0x56, 0x4C, 0xA4 | select with the left, right, middle button | 1 (Momentary): starts when activated, ends when released |
| 0x33, 0x2E, 0x93 | double-select with the left, right, middle button | 1 (Momentary) |
| 0x32, 0x73, 0x6E | tap with the left, right, middle button | 2 (Impulse) |

An action whose toggle type is 2 (Impulse) never produces `MouseDown`, `MouseUp`,
`MouseClick` or `MouseDoubleClick` on a plugin control: the UI passes it to the control
under the pointer as a separate one-shot notification that the plugin layer ignores, and
its end is ignored. Read the toggle type from the action-map record at run time
(`keymap.md` §12.10); do not hard-code which actions are Impulse.

The UI's only toggle-type test is "type 2 or not" (P50). It looks the type up in the action
map of the input context in force when the action arrives (`keymap.md` §12.10); an action
that this map does not list counts as type 0. Every type other than 2 — 0, 1 (Momentary),
3 (AutoRepeat), 4 (Analog) and 5 (Toggle) — follows the Momentary rules of §13.12.2, both
when the action starts and when it ends; the button events carry no analog amount. In this
install none of the nine actions has type 3, 4 or 5 (O6), so the case arises only with other
action-map data.

The `Button` field of the button events (§9.3) is converted from the action:

| `Button` | Action keys |
|---|---|
| 1 (Left) | 0x32, 0x33, 0x56, 0x66, 0x72 |
| 2 (Right) | 0x03, 0x2E, 0x43, 0x4C, 0x73 |
| 4 (Middle) | 0x67, 0x6E, 0x93, 0xA4 |
| 0 | any other action |

(0x66, 0x72, 0x03, 0x43 and 0x67 are alternate tap and double-select variants that the table
covers although the UI does not route them as mouse-button actions.)

**What Tessera's input layer delivers (Tessera rules, decision 0014 D1, D7, D9).** In
Tessera the actions come from its own key map (`keymap.md`), whose actions carry the
original's command names; the input layer offers them to the UI one event at a time, in
arrival order, each with its command name, its command ID when it resolves (resolved at run
time from the input-action mapper chain, `keymap.md` §12.11), press or release, the value
1.0, the pointer position and the held modifiers.

- An action with **no command name** is never offered: game code gets it and plugins never
  see it. An action with a name but **no ID** (for example in runs without install data) is
  routed by the class of its name (one of the nine mouse-button names, a wheel name, or a
  key action); its `Button` value comes from the name (Left for the left select,
  double-select and tap names, and so on), and it skips the `KeyDown`/`KeyUp` broadcast,
  which needs an ID.
- **Toggle types.** Tessera reads "the action map of the input context in force" as table
  A of the default action map (DID map ACTIONMAP, key 1; `keymap.md` §12.2, §12.10), read
  at run time. An ID not in that table, or an action without an ID, has type 0. Without
  action-map data every type is 0, so a tap action then gives plugin button events, which
  the original would not give (decision 0014 Known difference 12; such runs load no
  plugins).
- **Never offered:** modifier holds, analog presses of button actions and analog or
  relative axis input, so plugins never see their commands, and action 0xAB is never
  produced (decision 0014 Known differences 6 and 10).
- **Repeats.** An operating-system auto-repeat of a held key gives a repeated **start**,
  with no end in between, for each hold whose command has toggle type 3 (AutoRepeat) in that
  table (without action-map data, the built-in repeat flag of the Tessera action decides).
  The UI routes a repeated start like a start (§13.13: offered again and, if not consumed,
  `KeyDown` again). Delay and rate are the operating system's; the original's repeat timing
  is not known (decision 0014 Known difference 7). Mouse buttons get no operating-system
  repeat, so a mouse-button action gets one start at the press and one end at the release.

<a id="13-12-2"></a>
#### 13.12.2 Capture, `MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick`

The UI keeps: at most one **capturing** control, a set of mouse-button actions pressed
while capturing, and for every control the set of actions pressed on it.

**When a Momentary mouse-button action starts** (pointer at screen position P):

1. Refresh the pointer target and hover target at P (§13.11).
2. If no control is capturing:
   1. If there is a hover target H, H starts capturing and the action is recorded as
      pressed while capturing.
   2. Window activation by the press (§13.14) is applied, from the pointer target; its
      events therefore come before this press's `MouseDown`.
3. If a control is capturing (normally H from step 2), and it is not already in that
   control's pressed set: add it, and deliver **`MouseDown`** to the capturing control
   (`Button` from §13.12.1, `X`, `Y` relative to the control). If the action was already in
   its pressed set, the start is treated as a release on that control instead: step 4 of
   the release rules below (`MouseUp`, possibly a click), and no `MouseDown`.
4. With no hover target and no capture, nothing is delivered to plugin controls (the press
   belongs to the game world).

**When a Momentary mouse-button action ends:**

1. The target is the capturing control, or, if none, the hover target.
2. If a control is capturing, remove the action from the capture's pressed set; when that
   set becomes empty, the capture ends now (before step 4).
3. Refresh the pointer target and hover target (with the capture possibly ended).
4. If the target has the action in its own pressed set: remove it and deliver **`MouseUp`**
   to the target, **wherever the pointer is**. Then, only if the target is now the hover
   target (the pointer is over it and it is mouse-visible):
   - for a double-select action (0x33, 0x2E, 0x93), deliver **`MouseDoubleClick`** if the
     control accepts double clicks; if it does not, deliver `MouseClick` with the matching
     select action (0x56, 0x4C, 0xA4) instead;
   - for any other action, deliver **`MouseClick`**.

So `MouseUp` always goes to the control that got the `MouseDown`, even when the button is
released outside it, and `MouseClick` needs the release to happen over that control. All
these events are notifications (§9.7).

**Actions of type 3, 4 or 5.** The UI applies the two rule sets above to whatever starts
and ends the input layer delivers. Consequences: an action that is started again while it is
still in the capturing control's pressed set (as a held AutoRepeat button might be) produces,
at the repeated start, `MouseUp` and possibly `MouseClick` instead of a second `MouseDown`,
so starts alternate between press and release behaviour; an action that starts at one
physical press and ends at a later one (a Toggle latch) behaves as a button held in between,
keeping the capture. > **OPEN:** which start and end sequence the input layer produces for
mouse-button actions of types 3, 4 and 5 (repeat timing, latch, analog threshold);
`keymap.md` does not give it for UI actions. Tessera: the input layer delivers one start at
the press and one end at the release for a mouse-button action of any type other than 2
(this agrees with decision 0014 D7: mouse buttons get no repeat, §13.12.1).

Plugin controls accept double clicks: S3 captured `MouseDoubleClick` (with `Button` 1) on a
plugin control (Reported). > **OPEN:** whether every plugin control class accepts double
clicks. Tessera: all do.

Worked example (derived, owner's key map O5: left select = press and release, left
double-select = a second press near the first and its release, left tap = Impulse): a left
double click over a plugin control C gives, in this order, `MouseDown`(1), `MouseUp`(1),
`MouseClick`(1) for the first click; for the second press, `MouseDown`(1) for the select
action and `MouseDown`(1) for the double-select action; for the second release, `MouseUp`(1)
and `MouseClick`(1) for the select action, then `MouseUp`(1) and `MouseDoubleClick`(1) for the
double-select action (the capture ends with the last of them). > **OPEN:** the order in which
the input layer starts and ends two actions triggered by one physical event (here select and
double-select), and how it turns a `Tap` activation of a Momentary action (the owner's right
select) into a start and an end (expected: both at the release, giving `MouseDown`,
`MouseUp`, `MouseClick` together; S3 captured `MouseClick` with `Button` 2). Tessera
(decision 0014 D3, D4; this replaces the earlier Tessera choice "the order of the bindings
in the effective key map"): at a press, the actions start in this order: bindings that
match the held modifiers exactly before those that match a subset of them; then context by
context in the input layer's resolution order; within one context press-activated bindings
before double-press ones before double-press-nearby ones; then the order in which the
actions were registered. At a release, the ends of the holds that this release ends come
first, in the order those holds began, then the release- and tap-activated actions by the
same keys (release before tap), each as a start immediately followed by its end. A tap
fires at a release that comes at most half the double-click time after the press, so its
start and end are both at the release. Each binding fires at most once per physical event,
so a second start of the same action is never produced by one press (it would turn into a
release at step 3). With Tessera's default bindings (decision 0014 D2 and its worked
example) this gives the order of the worked example above.

**Mouse buttons during the pointer lock (Tessera rule, decision 0014 D7 and its Known
difference 16).** While the pointer is locked (§13.11), the start of a mouse-button action
reaches no control, even while a control is capturing, and is not consumed. The end of an
action that is in the capturing control's pressed set is still applied: the release steps
1, 2 and 4 run, so the capturing control gets `MouseUp` and the capture ends when its set
becomes empty; step 3 finds no hover target while the pointer is locked, so no `MouseClick`
or `MouseDoubleClick` follows. So no capture or pressed set outlives its buttons. Example:
left pressed on control C (C captures), right pressed (mouse-look, the lock turns on), left
released: C gets `MouseUp` and no `MouseClick`, and the capture has ended.

<a id="13-12-3"></a>
#### 13.12.3 `MouseMove`

On every pointer movement the UI notes the time, refreshes the pointer and hover targets
(which may send `MouseLeave`/`MouseEnter`, §13.12.4) and then delivers **`MouseMove`** to the
capturing control if there is one, otherwise to the hover target, with `X`, `Y` relative to
that control. While the mouse is captured, the capturing control receives `MouseMove` even
when the pointer is outside it.

**Coalesced movements (Tessera rule, decision 0014 D7 and its Known difference 15).**
Tessera's input layer does not offer every operating-system pointer movement. It records
the latest position and offers one movement, at that position, before the next key, mouse
button, wheel, typed character, pointer-left or focus-lost event, and once more per frame
in frame-loop step 1. So a plugin gets at most one `MouseMove` per other input event plus
one per frame; a handler that counts movements or traces the path sees fewer, coarser
steps, and a button, key or wheel event always follows a movement to its own position.
`X` and `Y` are relative to the control's **current** position (§13.11, "Geometry used by
the search"): a `MouseMove` handler that drags its window with `SetPosition` gets, for two
movements in one frame, two events relative to where the window is after the first, and no
spurious `MouseLeave` or `MouseEnter`.

<a id="13-12-4"></a>
#### 13.12.4 `MouseEnter`, `MouseLeave`

Whenever the hover target changes from O (old, possibly none) to N (new, possibly none):

1. List O and its ancestors, innermost first, and likewise N and its ancestors; drop the
   ancestors the two lists share.
2. Deliver **`MouseLeave`** to each remaining control of O's list, innermost first.
3. Then deliver **`MouseEnter`** to each remaining control of N's list, innermost first.

Ancestors receive these events whether or not they are mouse-visible. Examples: moving from
a window's background onto a button inside it gives `MouseEnter` on the button only (the
window was entered already); moving from the button out of the window gives `MouseLeave` on
the button, then on the window; moving from outside straight onto the button gives
`MouseEnter` on the button, then on the window. While the mouse is captured the hover
target can only be the capturing control or none, so other controls get no enter/leave
events until the capture ends.

<a id="13-12-5"></a>
#### 13.12.5 `MouseHover` (P51)

In each UI tick (§3.3) the UI checks the hover target H. It delivers **`MouseHover`** to H,
with the pointer position relative to H, when all of these hold:

- H has not been marked as hovered;
- the manager's pointer-outside flag (§13.11) is clear (always so in this build);
- no control is capturing the mouse;
- there is a hover target H;
- **more than 0.25 s** of the UI's frame clock have passed since the last pointer movement
  or the end of the last capture, whichever is later (the client's default hover delay, a
  32-bit float; client panels may override it per control, the plugin API cannot). The
  comparison is strict: exactly 0.25 s is not enough.

Before delivering, the UI hides any tooltip it is showing. When H is already marked as
hovered (or the pointer-outside flag is set), the check only looks after the client's own
tooltip lifetime and delivers nothing.

H is marked as hovered only when the hover notification is reported as handled. The plugin
layer never reports it handled (§9.7), so, unless a native ancestor does, the conditions keep
holding and `MouseHover` is delivered again in every following tick until the pointer moves
(which restarts the 0.25 s) or the hover target changes (which clears the mark).

> **OPEN:** confirm by observation (a test plugin counting `MouseHover` per second while the
> pointer rests on a plain control in a plain window) whether the original repeats it every
> frame. Tessera: repeat every tick, as described.

Tessera (decision 0014 D7, D8 and its Known difference 15): the tick runs in frame-loop step
7 (§3.3). "The last pointer movement" is the time at which the coalesced movement of
§13.12.3 is delivered (at the latest in step 1 of the same frame), read on the frame clock
of the §3.3 OPEN on the frame clock, not the time of the last operating-system movement, so
`MouseHover` may start up to one frame later than in the original.

<a id="13-12-6"></a>
#### 13.12.6 `MouseWheel` (P52)

The scroll-up (0x82) and scroll-down (0x10) actions are not mouse-button actions. When one
starts, the UI refreshes the pointer target and delivers **`MouseWheel`** to the capturing
control if there is one, otherwise to the pointer target, with `X`, `Y` relative to it and
`Direction` +1 for 0x82, −1 for 0x10 (§9.3). Their end is ignored. The wheel never
activates or deactivates windows. When no control is under the pointer, plugin controls get
nothing. A `MouseWheel` notification that no native control consumes travels on to the
ancestors' native handling (§9.7), where client panels may scroll; it never fires
`MouseWheel` on a plugin ancestor.

The scroll actions exist only while an element under the pointer makes them active: in the
client's own panels a scrollable element or scroll bar does so while it is hovered
(`ui-widgets.md` §5.7, §6.9; decision 0014 D5). Tessera rule (decision 0014 D5): every
hovered plugin control makes them active, so every `MouseWheel` handler fires, and the
wheel does not zoom the camera while the pointer is over a plugin control; elsewhere,
including over a plain native panel, it zooms the camera. During the pointer lock (§13.11)
nothing is hovered, so the wheel zooms the camera.
> **OPEN:** which plugin control classes make the scroll actions active in the original
> (the `Label` family derives from `ScrollableControl`, §13.4, but the native element class
> behind each plugin control is not known). Decision 0014 asks the owner whether the wheel
> zooms the camera over a plain plugin window. Tessera: every plugin control, as above.

<a id="13-13"></a>
### 13.13 Keyboard routing and focus (P48, P53)

Every input action that is neither a mouse-button action (§13.12.1) nor a wheel action is a
**key action**, with one exception: the UI ignores action **0xAB** entirely, at its start and
at its end; it is neither offered to a control nor broadcast (P48; in this install 0xAB is a
key of the input-action chain with toggle type 2, O6). Impulse key actions (toggle type 2)
pass through the same path when they start; the end of an Impulse action is ignored. Key
actions of any other type, including 0 for an action that the context's action map does not
list (§13.12.1), also take the path below when they end.

**Key action starts:**

1. If a control has keyboard focus, it is offered the action; otherwise the active window
   (§13.14) is offered it; if there is neither, go to step 3.
2. Being offered means the control's native key handling runs and a key notification goes to
   it and up its ancestors (§9.7). If any native listener consumes it, routing stops.
   The plugin **`TextBox`** class (and the skinned text box of §14.2, which is built on it)
   has a listener of its own: whenever the text box itself is the source of a key-down
   notification it fires `KeyDown` with that action, and for a key-up notification `KeyUp`,
   without consulting `GetWantsKeyEvents()` (P48). A key notification has a text box as its
   source only when the text box holds the focus. Plugin listeners never consume.
3. Not consumed: the **`KeyDown` broadcast** (§9.7) is sent: every control with
   `SetWantsKeyEvents(true)`, in subscription order, gets `KeyDown` with `Action` and the
   modifier flags. Focus, visibility, enabled state and window activation do not matter.

**Key action ends:** the same with `KeyUp` (the focused TextBox fires `KeyUp` from its
notification; the broadcast is `KeyUp`).

A focused text box that has also called `SetWantsKeyEvents(true)` therefore receives
`KeyDown` twice for an action that no native handling consumes: first from its notification
(step 2), then from the broadcast (step 3); the same holds for `KeyUp`.

So a plugin that wants global hot keys subscribes any control, even a hidden one, to key
events; and keys typed into a client text field or a focused plugin text box that consumes
them reach no plugin `KeyDown` broadcast. `SetWantsKeyEvents(true)` on a control that already
wants them changes nothing; turning it off stops both `KeyDown` and `KeyUp` at once.

> **OPEN:** which key actions a focused plugin TextBox (and the client's own text fields)
> consume, and how character input (typed text, as opposed to key-binding actions) reaches a
> text box. Tessera (decision 0014 D2, D6, D7): a focused text box consumes the actions it
> uses for editing (cursor movement, deletion, selection, copy and paste) and lets every
> other action through, Escape and Enter included, which then go on to game code and to
> the `KeyDown` broadcast. Typed characters are not actions: the input layer hands them to
> the UI as character input while an editing context is active, and the focused text box
> inserts them. While a text box has focus, the input layer swallows keyboard keys that no
> UI context binds before they become actions (mouse buttons, the wheel and mouse-look are
> not affected), so the text box's own `KeyDown` (step 2) and the broadcast (step 3) never
> see such keys, for example the function keys or Tab outside a dialog. This narrows step
> 2 for Tessera (decision 0014 Known difference 18). Commands that have no Tessera action
> yet (quick slots and the like) never reach the broadcast either (decision 0014, "Still
> OPEN").

**Keyboard focus.** At most one control has keyboard focus.

- `Control:Focus()`: find the control's top-level window W (the control itself if it is a
  window); if there is none, nothing happens. Remember the control as W's focus control. If
  W is the active window, move the focus to the control now.
- Moving the focus from A to B (B ≠ A) delivers `FocusLost` to A (if any), then
  `FocusGained` to B, both as notifications (§9.7). Moving it to the control that already has
  it does nothing.
- `HasFocus()` is true exactly when the control has keyboard focus.
- When a window becomes active (§13.14), its remembered focus control, if any, receives the
  focus. When a window is deactivated, the control that has focus loses it (`FocusLost`) and
  no control has focus until a window with a remembered focus control is activated; the
  remembered control stays remembered.

Worked example: plugin windows W1 (active, focus on text box T1) and W2 (inactive). `T2:Focus()`
on a text box of W2 only remembers T2 (no events; `T2:HasFocus()` is false). `W2:Activate()`
then gives `Deactivated` on W1, `FocusLost` on T1, `Activated` on W2, `FocusGained` on T2.

<a id="13-14"></a>
### 13.14 Z-order and window activation (P54, P55)

**Z-order.** `SetZOrder(n)` stores n (truncated to an integer); the UI keeps every control's
children sorted so that a higher `ZOrder` is above a lower one: it is drawn later and is
tried first by the hit test (§13.11). Changing the value removes the control from its
parent's child order and inserts it again at its new place. `GetZOrder()` returns the value
set; the default is 0. Top-level plugin windows are children of the UI root, so their
`ZOrder` orders them among all top-level windows, the client's panels included. Among
siblings with the same `ZOrder` the order is by an internal per-control sequence, and
activation (below) moves a window above all its siblings of the same `ZOrder`.

> **OPEN:** the `ZOrder` values of the client's own top-level panels, and which of two new
> siblings with equal `ZOrder` is on top (the internal sequence was not traced). Tessera:
> client panels at 0; among equal `ZOrder` the most recently added or activated is on top.

**Active window.** At most one top-level window is active.

`Window:Activate()` (and activation by a mouse press, below) on window W:

1. If W is already active, nothing happens (in particular W is not raised again).
2. If W cannot be activated, nothing happens. Plugin windows can be activated.
   > **OPEN:** which client panels cannot be activated.
3. W becomes the active window. The previously active window, if any, is deactivated: it
   gets `Deactivated`, and the focus control loses the focus (§13.13).
4. W is moved to the end of the client's activation history and raised above all its
   siblings of the same `ZOrder`.
5. W gets `Activated`.
6. If W has a remembered focus control, it receives the focus (`FocusGained`).

All of these events are notifications (§9.7). `Activate()` on a control that is not a
window activates its top-level window. A guard makes a nested `Activate()` on the same
window, made from a handler that its own activation triggered, do nothing.

**Activation by a mouse press.** When a Momentary mouse-button action starts while no
control is capturing (§13.12.2), the UI looks at the top-level window that contains the
pointer target:

- if it is the active window, nothing changes;
- if there is such a window and it can be activated, it is activated as above (so clicking
  any control of an inactive plugin window activates and raises that window);
- otherwise (the press hit no window, for example the game world, or a window that cannot
  be activated) the active window, if any, is deactivated: `Deactivated`, and the focus is
  lost.

The wheel does not activate. > **OPEN:** whether showing a window (`SetVisible(true)`)
activates it, and whether a plugin window that is hidden while active stays the active
window. Tessera: showing does not activate; hiding the active window deactivates it.

<a id="13-15"></a>
### 13.15 Size limits (P56)

`SetMinimumWidth`, `SetMinimumHeight`, `SetMaximumWidth`, `SetMaximumHeight` (and the
`…Size` pairs, which set width then height) store limits on the control. Values are
truncated to integers. A limit that was never set does not exist; a limit set to any value,
**including 0**, exists from then on. The getters return the stored value, or 0 when the
limit was never set (so `GetMaximumWidth() == 0` cannot tell "no maximum" from "maximum 0").

Setting a limit does **not** resize the control. Every later resize (by `SetSize`,
`SetWidth`, `SetHeight`, or the edge-attachment layout of §13.16) applies the limits to the
requested size, per axis:

1. if a maximum exists and the requested value is greater, use the maximum;
2. then, if a minimum exists and the value is smaller, use the minimum (so a minimum greater
   than the maximum wins).

`SetWidth(w)` resizes to (w, current height) and `SetHeight(h)` to (current width, h), so
both clamp both axes. `SizeChanged` fires only when the clamped size differs from the old
size (§9.6). Negative requests are not rejected; they are clamped like any value. The
oversize warning of §13.2 is checked on the final size.

Worked example: a control 100 × 50 gets `SetMaximumWidth(80)`; it stays 100 × 50, and
`GetWidth()` is still 100. `SetHeight(60)` then makes it 80 × 60 (width clamped by the new
maximum). `SetMinimumWidth(90)` followed by `SetWidth(10)` makes it 90 × 60.

> **OPEN:** what the original does with a negative or zero size (drawing and hit testing of
> an empty rectangle). Tessera: store it; a control with width or height ≤ 0 draws nothing
> and is never hit.

<a id="13-16"></a>
### 13.16 Edge attachments (P57)

`AttachEdges(left, top, right, bottom)` sets one `EdgeAttachmentType` (Appendix A.6) per edge
of the control; values are truncated to integers and stored without checking.
`GetEdgeAttachments()` returns the four in the same order (0 for never set). The attachments
say how the control's edges follow its **parent's** size; they are applied by a layout step.

**Reference rectangles.** A control with attachments keeps two reference rectangles: its own
rectangle R (left `RL`, top `RT`, right `RR` = `RL` + width − 1, bottom `RB`) and its
parent's rectangle Q (width `QW`, height `QH`), both as they were at the last capture. (In a
window whose UI scale is not 1, R's width and height are the displayed ones divided by the
scale.) A capture happens, when the control has a parent, on `AttachEdges`, and after every
plugin call that resizes, moves or re-parents the control (`SetSize`, `SetWidth`,
`SetHeight`, `SetPosition`, `SetLeft`, `SetTop`, `SetParent`); a control without any
attachment call never captures, except a window that the plugin layer has marked for global
UI scaling (§13.3), which captures on the same calls when it has a parent (with four None
attachments that has no visible effect). After `AttachEdges` and after a plugin resize or re-parent
(not after a pure move), the layout step runs once at once against the fresh references.
Resizes made by that immediate layout fire no `SizeChanged` (on the control or on anything
laid out below it during that step); `PositionChanged` is not suppressed. Because the
references were just captured, this immediate layout changes nothing for None, Same,
Opposite and Proportional edges, but it does re-apply Center, Mirror and RatioProportional.

**Initial references (P57).** Until its first capture, a control's own reference R is its
creation rectangle: `RL` 0, `RT` 0, `RR` 49, `RB` 49 (50 × 50, §13.1). It has no parent
reference Q until then; a layout step that finds no Q uses the **parent's own** reference
rectangle R instead (the parent's creation rectangle, or its rectangle at the parent's last
capture), so `QW` and `QH` are the parent's reference width and height.

**When the layout runs.** Whenever a control's size actually changes, the layout step runs
for each of its children, in child order, and so recursively down the tree. Children whose
four attachments are 0 and whose size is not 0 × 0 stay where they are; those of size 0 × 0
go through the layout step, where four None edges keep them where they are. So a control
without attachments is never moved or resized because its parent was.

**The layout step** for a control with references R and Q, whose parent's current size is
`PW` × `PH`:

- Deltas and ratios: `dW` = `PW` − `QW`, `dH` = `PH` − `QH`; `sX` = `PW` / `QW` (0 if `QW` is
  0), `sY` = `PH` / `QH` (0 if `QH` is 0), as 32-bit floats. `RW` = `RR` − `RL` + 1,
  `RH` = `RB` − `RT` + 1. "Half(n)" is n divided by 2, truncated toward zero; "trunc" converts
  a float to an integer by truncation toward zero.
- Each edge gets a new coordinate from its own attachment type:

| Type (value) | left | top | right | bottom |
|---|---|---|---|---|
| None (0) | current left | current top | current right | current bottom |
| Center (1) | Half(`PW`) − Half(`RW`) | Half(`PH`) − Half(`RH`) | Half(`PW`) − 1 + Half(`RW`) | Half(`PH`) − 1 + Half(`RH`) |
| Mirror (2) | `QW` − 1 − `RR` | `QH` − 1 − `RB` | `PW` − 1 − `RL` | `PH` − 1 − `RT` |
| Proportional (3) | trunc(`RL` × `sX`) | trunc(`RT` × `sY`) | trunc(`RR` × `sX`) | trunc(`RB` × `sY`) |
| Opposite (4) | `RL` + `dW` | `RT` + `dH` | `RR` | `RB` |
| Same (5) | `RL` | `RT` | `RR` + `dW` | `RB` + `dH` |
| RatioProportional (6) | trunc(Half(`PW`) − `RW` × `sY` × 0.5) | trunc(Half(`PH`) − `RH` × `sX` × 0.5) | trunc(`RW` × `sY` × 0.5 + Half(`PW`)) − 1 | trunc(`RH` × `sX` × 0.5 + Half(`PH`)) − 1 |
| any other value | `RL` | `RT` | `RR` | `RB` |

  "Same" keeps the edge's distance to the parent's edge on the **same** side, "Opposite" its
  distance to the parent's **opposite** edge. Mirror uses the reference parent size Q for
  left and top but the current size for right and bottom, as stated. None takes the
  control's current coordinate (relative to the parent). (The client's layout step keeps the
  reference coordinate instead for an element of size 0 × 0 that lacks a mark set by the
  client's element-creation step; every plugin control is created through that step and
  carries the mark, which nothing clears, so for plugin controls None always means the
  current coordinate, P57.)
- Second pass, for RatioProportional edges, in this order, each using the values computed so
  far, with `aW` = `RW` / `RH` (1 if `RH` is 0) and `aH` = `RH` / `RW` (1 if `RW` is 0):
  left: trunc(right − (bottom − top + 1) × `aW`); right: trunc(left + (bottom − top + 1) ×
  `aW`) − 1; top: trunc(bottom − (right − left + 1) × `aH`); bottom: trunc(top + (right −
  left + 1) × `aH`) − 1. This keeps the reference aspect ratio.
- The control is then resized to (right − left + 1) × (bottom − top + 1), which applies the
  size limits of §13.15, and moved to (left, top). Its `SizeChanged`/`PositionChanged` fire by
  the rules of §9.6, and its own children are laid out in turn if its size changed.

All arithmetic other than the float steps is 32-bit integer arithmetic; the float steps use
32-bit floats.

Worked examples (synthetic). Reference: parent Q = 200 × 100; control R: `RL` 10, `RT` 20,
`RR` 59, `RB` 39 (50 × 20). The parent is resized to 300 × 150, so `dW` = 100, `dH` = 50,
`sX` = `sY` = 1.5.

| Attachments (left, top, right, bottom) | New left, top, right, bottom | New size |
|---|---|---|
| 0, 0, 0, 0 | 10, 20, 59, 39 (unchanged) | 50 × 20 |
| 0, 0, 5, 0 (right Same) | 10, 20, 159, 39 | 150 × 20 |
| 4, 4, 5, 5 (move with bottom-right) | 110, 70, 159, 89 | 50 × 20 |
| 0, 0, 5, 5 (stretch) | 10, 20, 159, 89 | 150 × 70 |
| 1, 1, 1, 1 (centre) | 125, 65, 174, 84 | 50 × 20 |
| 3, 3, 3, 3 (proportional) | 15, 30, 88, 58 | 74 × 29 |

> **OPEN:** how the layout treats a top-level window (a control whose parent is the UI root)
> when the display size changes, and whether the client re-captures references when the
> parent is moved rather than resized. Tessera: top-level windows are laid out against the
> UI area like any child of the root; moving a parent does not re-capture.

<a id="13-17"></a>
### 13.17 Drawing backgrounds and back colours (P68)

Every plugin control is a client UI element, so it has the two image layers of
`ui-layout.md` §9.6 and is drawn by the same CPU compositor as the native panels
(`ui-layout.md` §9.6.1–§9.6.3). The plugin calls map onto those layers as follows (BIN).

| Plugin call | Layer and effect |
|---|---|
| `SetBackground(v)` | Removes the current base image, then makes the image of §13.8 the element's **base image**. It is drawn at its native pixel size from the control's top-left corner, repeated (tiled) to cover the control's rectangle and clipped to it; it is never scaled by the layer itself (`ui-layout.md` §9.6). The base layer's opacity factor (`UICore_Element_alphablendmod`) has no plugin setter and stays 1.0, so it never weakens a plugin background. `SetBackground` does not change the layer's operation. |
| `SetBlendMode(n)` | Sets the **base layer's operation**. `n` is converted exactly like a layout draw mode `n` (`ui-layout.md` §9.6.1 table; the `BlendMode` values are the `DrawModeType` values): 1 `Color` → Colorize, 2 `Normal` → Replace, 3 `Multiply` → Multiply, 4 `AlphaBlend` → Over, 5 `Overlay` → Mix colour, 6 `Grayscale` → Grayscale (no base-layer routine: the image is not drawn), 7 `Screen` → Screen, 8 `None`/`Undefined` → a "none" operation (no routine was found; Tessera: draw nothing), 0 → Replace. A value above 8 raises an error and changes nothing. `GetBlendMode()` returns the last value set (2 for a new control). The element keeps the converted operation as an **operation number** (0–7), which is not the blend-mode value; the full table is in §14.2.1 step 3, where a rebuild reuses it. |
| *(no call)* | A new control's base operation is **Replace**, the same as `Normal`: an image with transparent pixels **replaces** the pixels below it in the canvas, alpha included, instead of blending over them. Plugins that want transparency call `SetBlendMode(AlphaBlend)`. |
| `SetBackColor(c)` | Makes `c` the element's **overlay** as a colour fill without an image (`ui-layout.md` §9.6.3, "Without an image"), drawn **after** the base image, with the operation of the stored back-colour blend mode. `c` is stored on the control and returned by `GetBackColor()`. |
| `SetBackColor(nil)` | Removes the overlay and resets the stored colour to transparent black (A 0, R 0, G 0, B 0). The stored blend mode is kept. |
| `SetBackColorBlendMode(n)` | Raises an error and changes nothing for `n` above 8. Otherwise stores `n` and then **re-creates the overlay fill** with the stored colour and the operation of draw mode `n` (same conversion as above; 8 gives the "none" operation). It does so even when no back colour was ever set: the fill then uses the stored transparent black, so with mode 2 (Replace) it **clears the whole rectangle to transparent black**, while with mode 4 (Over) it draws nothing. `GetBackColorBlendMode()` returns the stored value (2 for a new control). |

**Colour conversion.** A `Color`'s four components (stored as 32-bit floats, §13.9) are turned
into the overlay's ARGB value when it is drawn: each component is multiplied by 255.0 in
32-bit floating point and converted to an integer by truncation toward zero (so 0.999 gives
254, as for text, `fonts.md` §8.4); the four integers are then combined in 32-bit integer
arithmetic: start with A, shift left by 8 bits and OR in R, shift left by 8 bits and OR in
G, shift left by 8 bits and OR in B. Components are **not clamped**: a
component above 1.0 or below 0.0 gives an integer outside 0–255 whose bits are OR-ed into the
neighbouring channels (a NaN or out-of-range value converts to −2³¹). Tessera reproduces
this packing exactly; it is deterministic.

**Order within one control** (`ui-layout.md` §9.3, §9.6; `fonts.md` §8.4): base image, then
the back-colour overlay, then the control's text (label-derived classes), then its children
in z order. So a back colour drawn with the default Replace operation **hides the background
image completely**, and a back colour with Multiply, Color (Colorize) or Screen tints it.
A plugin control never draws its own content after its children (it has no setter for
`UICore_Element_DrawAfterChildren`; game-styled controls may inherit it from their layout).

**Where it is drawn.** Into the canvas of the nearest element, from the control upwards,
that has its own drawing surface: the control itself when its stretch mode is not 0
(§13.18), otherwise its window (a window has its own surface whenever it is shown, §13.19,
and a hidden window draws nothing). Replace and
the Replace back-colour fill therefore write their alpha into that canvas: a back colour of
alpha 0.5 with the default mode makes that rectangle of the window half transparent, so the
game scene shows through when the window's surface is composed onto the screen
(`ui-layout.md` §9.6.1 OPEN on that composition; Tessera's rule there applies, and its
composition of surfaces is decision 0015 Decision 1.6 and §8: premultiplied source-over,
which at 1 : 1 and opacity 1 equals straight source-over within ±1). For a window that
stored object mode 0 see the Tessera rule of §13.19; a child control with its own surface
is composed by the Tessera rule of §13.18.

**Skins.** A background set by DID is drawn unskinned until the next skin switch, then
re-skinned (`skin-definition.md` §6.8, SK41); file images are never skinned.

Worked example (synthetic). A window's canvas pixel is opaque (A 255, R 100, G 100, B 100). A
child control covers it with a 32-bit image whose pixel is (A 128, R 200, G 0, B 0):

1. Default operation (Replace): the canvas pixel becomes (128, 200, 0, 0); the window is
   half transparent there.
2. After `SetBlendMode(Turbine.UI.BlendMode.AlphaBlend)` (Over, `ui-layout.md` §9.6.2):
   *a* = 128, *k* = 129, the destination is opaque, so each channel is
   mix(*D*, *S*, 129): R = 100 − div256((100 − 200) × 129) = 100 − (−50) = 150,
   G = B = 100 − div256(100 × 129) = 100 − 50 = 50; A stays 255: (255, 150, 50, 50).
3. With additionally `SetBackColor(Turbine.UI.Color(0.5, 0, 0, 1))` (A 0.5, R 0, G 0, B 1,
   §13.9 argument order) and the default back-colour mode: the colour packs to
   (127, 0, 0, 255) and Replace writes (127, 0, 0, 255) over the image's result.

<a id="13-18"></a>
### 13.18 Stretch modes and own surfaces (P69)

`SetStretchMode(n)` stores `n` (an integer, not checked) as the element's
`UICore_Element_ObjectMode` property and records that the plugin set a stretch mode (used by
§13.20). The property is the "object mode" of `ui-layout.md` §5.9 and §8.4: data enum
0x2300017A, 0 None, 1 Stretch, 2 Tile, 3 CanvasResize. Setting it to a non-zero value gives
the element **its own drawing surface**; setting it to 0 removes the surface (BIN: the
property-change handler). If the control already has a surface, the surface is refreshed
for the new mode. The surface exists only while the element is shown; the exact rule for
when an element has a surface is in §13.19 ("Which elements have a surface"). `GetStretchMode()` returns the property: the plugin's value, otherwise
the value from the element's layout (game-styled scaled windows, §14.2.1), otherwise **0**
(the dictionary has no default, O7).

| Mode | Own surface | Canvas | What changes for the control |
|---|---|---|---|
| 0 (default) | no | — | Drawn into its window's (or the nearest surfaced ancestor's) canvas, clipped to the control and its ancestors; images tiled at native size (§13.17). |
| 1 Stretch | yes | the control's size when the surface is created | When the control is later resized, its children are **not** laid out again (the element resize of a Stretch surface keeps the layout box, `ui-behaviour.md` §9.2.1; `ui-layout.md` §8.4 step 1 is not the reason here, as Tessera applies that step only under a root-panel parent, `ui-layout.md` §8.4 scope OPEN) and the canvas keeps its size; the surface is drawn scaled onto the new rectangle (`ui-behaviour.md` §9.2, Probable). A background set at size S therefore appears scaled by (new size ÷ S). |
| 2 Tile | yes | the size of the control's base image (or a size set by client code) when the surface is created | If either side of that size is not a power of two, the client writes a diagnostic and uses mode 3 instead. How the canvas is repeated over the rectangle: `ui-layout.md` §8.4 OPEN. |
| 3 CanvasResize | yes | follows the control's size | Drawn 1:1 as its own layer (Probable). |
| 4 and other values | yes (any non-zero value) | the control's size; the surface is created with the value as given (the creation step special-cases only 2) | Stored and returned; no label in the data enum; any other effect not traced. |

> **OPEN:** (clean-room implementer, 2026-10-07) what `GetSize()` returns after `SetSize` on a
> control in mode 1. By `ui-behaviour.md` §9.2.1 a Stretch element resize keeps the layout
> box and changes the scale, so `tessera-ui` keeps reporting the layout-box size (the size at
> the surface's creation) while the control is drawn and hit-tested on the scaled displayed
> box, trunc((size + 1) × scale − 1) per axis (for example 101 for a 50-pixel control resized
> to 100). Whether the plugin layer reports the requested size instead was not traced.

Community descriptions (S3 stubs; Reported, kept for comparison): 0 = no scaling, cropped or
tiled and clipped by the parent; 1 = scaled by the ratio of the size when the mode was set
to the current size (**agrees** with mode 1 above); 2 = the control takes the image's size
and stretches afterwards (**disagrees**: BIN gives the canvas, not the control, the image's
size); 3 = like 0 but not clipped by the parent (consistent with a separately composed
surface, not traced); 4 = like 1 but the control ignores the mouse (not traced); "mode 0
resets alpha" (explained by the default Replace operation of §13.17, which does not depend
on the stretch mode).

> **OPEN:** (1) how an own surface of a non-root element is composed into its parent's
> canvas or onto the frame: clipping by intermediate parents, filtering when scaled, blending
> (`ui-layout.md` §9.6.1 and §8.4 OPENs; `fonts.md` Q3); (2) the drawing of mode 2 and the
> effects of mode 4; (3) the element flag that `SetStretchMode(1)` also sets on a control
> that has a parent. Tessera (decision 0015, which replaces the earlier "straight
> source-over at the control's rectangle, clipped to its window only"): the rules below;
> treat 4 like 1 and leave mouse visibility unchanged.

**Tessera's drawing of own surfaces (Tessera rules, decision 0015 Decision 1.6–1.8, §2.1,
§2.3, §7).**

- **Separate entry.** A control's own surface is never blended into its window's canvas on
  the CPU. It is composed as a separate quad onto its displayed rectangle, scaled by
  displayed size ÷ canvas size on each axis, with premultiplied source-over. At exactly
  1 : 1 and unrotated the texels are copied unchanged; when scaled or rotated it is filtered
  bilinearly on premultiplied colour (a Tessera setting can choose nearest), until the
  original's filter is known (`fonts.md` Q3).
- **Order.** The window's content and the control's surface keep the drawing order of one
  canvas at every pixel: what the window draws after the control in drawing order (later
  siblings, for example) and inside the control's region is put into a separate layer
  composed after the control. Inside that region an operation that reads the destination
  sees transparent black instead of what lies below, which is a recorded approximation.
- **Clip.** The surface is clipped by its nearest enclosing window, whether that window is
  drawn through its own surface or into the canvas of a surface above it, tested in that
  window's own (rotated) rectangle, and also by every enclosing window further up. A child
  control with a stretch mode between them does not clip (consistent with the community
  description of mode 3, "not clipped by the parent"). So a stretch-mode control inside a
  child window that extends past the child window is cut at the child window's edge, and a
  child window that extends past its parent window is cut, with everything in it, at the
  parent's edge. The clip by the windows above the nearest one is a Tessera extension of
  the earlier rule "clipped to its window only" (decision 0015, spec question 12). Building
  the chain from the windows rather than from their surfaces is an interim Tessera rule,
  pending a revision of decision 0015 (OPEN after this list).
- **Opacity and rotation.** The surface takes its own opacity only (§13.19). It turns with
  every ancestor whose surface holds a rotation, about that ancestor's centre, after its
  own rotation if its surface holds one (§13.20).
- **Mode 2 (Tile).** The canvas has the size of the control's base image when both sides
  are powers of two, otherwise the control's size as for mode 3. It is not fitted to the
  control: it is repeated from the control's top-left corner over the displayed rectangle,
  1 : 1 inside an unscaled window and scaled with the window inside a scaled one. A control
  below a mode-2 surface (or below a native Tile surface) never gets a surface of its own:
  it is painted into the Tile canvas as if its mode were 0 (unscaled, without its own
  opacity or rotation) and repeats with it, with one warning to the apartment's error
  listeners. Its stored mode and `GetStretchMode()` are unchanged.
- **Mode change on an existing surface** ("refreshed for the new mode" above; Tessera rule,
  because only that word was traced). A change from one non-zero mode to another keeps the
  surface, its rotation and its opacity; its canvas takes the size the new mode gives
  (modes 1, 3 and 4: the current size; mode 2: as above). If the canvas size, the displayed
  rectangle or the covered region changes, or the change is to or from 2, the surface is
  repainted in full and the old picture stays on screen until the new one is complete;
  otherwise only its composition changes. A change to 0 releases the surface, and a change
  from 0 creates one, which is drawn after its first paint (§13.19).
- **Caps.** At most 512 surfaces per apartment (windows and controls together), 4,096
  surfaces in all, and 8 levels of nested surfaces below a top-level surface; Tessera's
  memory budgets for canvases and textures act the same way. A control or child window over
  a cap gets no surface and is painted into its parent's canvas as if its mode were 0; a
  top-level window over a cap draws nothing. Each case gives one warning per plugin to the
  apartment's error listeners (§6.6).

> **OPEN (clip chain of mode-0 child windows, decision 0015 open review item 1):** decision
> 0015 as written (Decision 1.6, 1.7 "Clip", §8) builds the clip chain from the surfaces
> above the entry that are a root or belong to a root panel. A plugin child window in
> stretch mode 0 has no surface of its own, so under that text a stretch-mode control inside
> it is not cut at the child window's edge, which contradicts the example in 0015's own
> Decision 1.7 and the earlier rule of this section. 0015 lists the fix as a must-fix review
> item but does not contain it yet. **Tessera rule until decision 0015 is revised** (the
> fix the review proposes): the chain is built from the displayed boxes of the enclosing
> windows, whether each is drawn through its own surface or into the canvas of a surface
> above it; a window without a surface is tested in the space of the surface that draws it.
> Invariant 60 tests it.

<a id="13-19"></a>
### 13.19 Opacity (P70)

`SetOpacity(f)` sets the element's `UICore_Element_ObjectOpacity` to `f` (a 32-bit float;
the setter does not clamp). `GetOpacity()` returns the property; its dictionary default is
1.0 (range 0–1, O7). The property is the opacity of the element's **own surface**
(`ui-layout.md` §5.9): it fades that surface, with everything drawn into it, when the
surface is composed.

<a id="13-19-surfaces"></a>
**Which elements have a surface** (BIN, fourth pass and its repair: the root-panel
constructor, the object-mode property handler, the element's show/hide step and the
surface-creation step; `ui-behaviour.md` §11.2). Two conditions decide it, and the client
re-evaluates them whenever the object mode, the root flag or the element's visibility
changes:

1. **The element wants a surface.** If the element has a `UICore_Element_ObjectMode` value
   (from its layout or from `SetStretchMode`, §13.18), it wants one exactly when that value
   is not 0. If it has no value, it wants one exactly when it is a **root panel**. Every
   window (`Turbine.UI.Window`, `Turbine.UI.Lotro.Window`, `GoldWindow`) is built as a root
   panel (`ui-layout.md` §9.1 step 3; BIN: the window classes pass the root flag to the
   shared control constructor); other controls are not root panels.
2. **The element is shown**: its own visibility flag is set (`IsVisible()` true; the
   ancestors' visibility, `IsDisplayed()`, does not enter), **or** its
   `UICore_Element_KeepObjectWhenHidden` property is true. No element of the plugin layout
   of §14.2.1 sets that property (O7: none of the 188 elements of its 13 resolved top-level
trees, states included, has it in this install; read at run time) and
   plugins have no setter for it, so for every plugin control this second condition is
   simply "`IsVisible()` is true".

The surface exists exactly while both conditions hold. Showing an element that wants a
surface creates one; hiding it releases the surface; changing the object mode to 0 releases
it; changing it to non-zero on a shown element creates (or refreshes) it. A surface created
for an element without an object-mode value uses CanvasResize (§13.18, §20). Because new
controls and windows **start hidden** (§13.1), a new window has **no surface until its first
`SetVisible(true)`**, and loses it again each time it is hidden. A hidden element draws
nothing, so the absence of a surface while hidden is not visible; it matters for
`SetRotation` (§13.20).

Therefore, for opacity:

- **Windows** have a surface whenever they are shown, unless the plugin stored object mode
  0. A window's opacity fades the window **and all its children that are drawn into its
  surface** (every child without a surface of its own), its frame (game-styled windows),
  backgrounds, back colours and text alike; children with a surface of their own are the
  OPEN below. An explicit `SetStretchMode(0)` on a window stores object mode 0 and so
  removes its surface (condition 1), after which its opacity has no effect. Where such a
  window is then drawn was not traced. Tessera rule (decision 0015 Decision 1.1, which
  narrows the earlier "Tessera draws every shown window through a surface"):
  - a **top-level** window in mode 0 (no surface above it) is drawn through a Tessera
    **stand-in surface**, sized and repainted like CanvasResize, composed at opacity 1
    (its stored opacity has no visible effect, as above). The stand-in is not a surface for
    any other rule: the stored mode, `GetStretchMode()`, the resize rule (the window takes
    the new size and its children are laid out again), the "has a surface" test of §13.20
    step 2 and the target search of step 3 still follow conditions 1 and 2. When step 2 of
    §13.20 sets mode 1 on such a shown window, the stand-in becomes its real Stretch
    surface in place, with no new first paint;
  - a **child** window in mode 0 (its parent window is drawn through a surface or a
    stand-in) gets no surface and is drawn into the canvas of the nearest surface above it,
    as §13.17 says for every control without a surface, clipped by its own rectangle and
    its ancestors'. Its opacity has no visible effect and it has no rotation of its own.
- **Other controls** have a surface only while their stretch mode is not 0 (§13.18) and they
  are shown. With mode 0, `SetOpacity` is stored and returned by `GetOpacity()` and has
  **no visible effect**; after `SetStretchMode(1)` (or any non-zero mode) it fades the
  control and its children that are drawn into its surface; setting mode 0 again removes
  the effect.

In every case `GetOpacity()` returns the stored property whether or not a surface exists.

> **OPEN:** whether a child's own surface inside a window is faded by the window's opacity
> as well (it depends on the composition of nested surfaces, §13.18 OPEN (1)), and whether
> values outside 0–1 are clamped when drawing. Tessera (decision 0015 Decision 1.7, accepted
> by the owner; it replaces the earlier "compose nested surfaces into their window first, so
> the window's opacity applies to them"): every surface is composed with **its own**
> opacity only; the opacity of its window and of other ancestors is not multiplied in. So a
> window set to opacity 0 still shows its children that have stretch mode 1, 2, 3 or 4, and
> a window at opacity 0.5 does not fade such a child. This follows the client's auto-fade,
> which writes the fade opacity into the element and into each child with its own surface
> (`ui-behaviour.md` §11.1 step 2), which only makes sense if the child is composed on its
> own. Clamp to 0–1 when drawing and return the stored value from `GetOpacity()`.

**First paint and the logical show (Tessera rule, decision 0015 Decision 1.8).** Showing a
control takes effect at once for every rule the plugin can observe: `IsVisible()`,
`IsDisplayed()`, hit testing (§13.11), activation and focus, and the visibility, activation
and focus events they cause (the setter table of §9.6, delivered as notifications by §9.7).
A newly created surface is drawn only once its first paint is complete and the images it
needs have arrived, but at most 100 ms after the show; after that it is drawn with what has
been painted, and the rest appears as it is painted. The 100 ms bound holds for every
window (a root is a group of its own) and for a nested surface shown while its window's
group has no repaint generation running. **Exception (0015 Decision 1.5 and 1.8, U4 (c)):**
a nested surface (a stretch-mode control inside a window) shown while its group's
generation is running is first drawn at the swap of the next generation, so later than
100 ms when those generations take longer: 0015 bounds it at 8 frames for groups whose
surfaces are up to 1038 × 802 (for example a window resized on every frame) and at 41
frames for a screen-sized plugin window after a screen resize. During any of these waits a
click can reach a control that is not drawn yet. A surface that already exists is repainted
in the background and swapped in whole, so a shown window never appears half-updated.

<a id="13-20"></a>
### 13.20 Window rotation (P72)

`SetRotation(t)` (windows only):

1. Reads `t.x`, `t.y`, `t.z` as numbers (angles in **degrees**).
2. **Object mode.** The test "has its own surface" uses the rule of §13.19 ("Which elements
   have a surface"): a window has one exactly while it is shown, unless the plugin stored
   object mode 0. If the window has **no** surface at the moment of the call (it is hidden,
   for example never shown yet, or its stored object mode is 0), then, unless the plugin
   has set a **non-zero** stretch mode (§13.18), the window's object mode is set to 1
   (Stretch), so `GetStretchMode()` reports 1 from now on. If the window is shown, this
   creates its surface at once (in Stretch mode). If it is hidden, no surface is created
   now; the window gets a Stretch surface when it is next shown. Either way, from then on
   the window's children are not laid out again when it is resized (§13.18, mode 1). If the
   window **has** a surface at the moment of the call, its object mode is left unchanged
   (a plain window keeps reporting `GetStretchMode()` 0; a scaled game-styled window keeps
   its layout value, §14.2.1).
3. **Target surface.** The three angles go to the window's own surface if it has one now,
   otherwise to the surface of the nearest ancestor that has one. Only when a surface was
   found are the angles stored for `GetRotation()` (a table with `x`, `y`, `z`); otherwise
   the call has no further effect (the object-mode change of step 2 stays). The surface
   ignores a change when each angle differs from the previous one by at most 0.0002.
   When the angles went to the window's own surface, the window also remembers the
   resulting rotation: when the window is hidden and shown again, the new surface gets the
   remembered rotation, so a rotation survives hide/show. A call made while the window had
   no surface clears the remembered surface state of the last hide; the next surface then
   gets the rotation the window remembered from its own surface (none, that is 0, 0, 0,
   for a window that was never rotated while shown). So a `SetRotation` on a hidden window
   **never rotates that window** when it is shown later: plugins must call it again after
   `SetVisible(true)`. > **OPEN:** whether a top-level window that has no surface can reach
   any ancestor surface (the screen root's, the window's parent after it is attached to
   the screen, `ui-layout.md` §9.1 step 4), and so whether such a call rotates something
   else or nothing. Tessera: it rotates nothing, `GetRotation()` keeps its previous value,
   and the object-mode change of step 2 still happens. Tessera's stand-in surfaces of mode-0
   top-level windows (§13.19) are never targets, because for the original those windows
   have no surface (decision 0015 §2.3).
4. Drawing: each angle is reduced modulo 360 (C `fmod`, so the sign is kept) and converted
   to radians as angle × π ÷ 180 (32-bit floats). The surface's rectangle is drawn as a
   quad rotated **about its centre** by the three rotations (about the screen's horizontal
   axis for `x`, the vertical axis for `y`, the axis out of the screen for `z`), so `z`
   turns the window in the screen plane and `x`/`y` tilt it. When all three stored angles
   are within 0.0002 of 0, the surface is drawn by the ordinary unrotated path.

**How Tessera draws rotations (Tessera rules, decision 0015 §2.3, Decision 1.6–1.7, §7).**
The rotation is state of the surface that received it: composition reads the angles each
surface holds, never the `GetRotation()` values. A surface turns about its own centre by its
own angles and then with every ancestor whose surface holds a rotation, about that
ancestor's centre, so the controls with their own surfaces inside a rotated window turn
with it and are clipped by the window's rotated rectangle. Example: a hidden child window C
of a shown window P gets `SetRotation{z = 60}`; the target is P's surface (step 3), so P
turns by 60 with everything in it, `C:GetRotation()` returns *z* = 60, and when C is shown it
is drawn unrotated relative to P, turning only with P. Angles that are not finite numbers
(infinity, NaN) are treated as 0, so the unrotated path is used. Hit testing ignores
rotation (OPEN below).

Plugin usage agrees: the values plugins pass are degrees, almost always as `z` (0, 90, 180,
270, headings), with a few `x` tilts (S6 and `plugin-api-usage.md` §5.3, §9.3 rows 8 and
12; usage counts, Reported).

> **OPEN:** the direction in which a positive angle turns on screen, the order in which the
> three rotations are combined, whether `x`/`y` tilts use a perspective projection, and
> whether hit testing follows the rotated quad (the hit test of §13.11 uses unrotated
> rectangles). Tessera (provisional, to be settled by an owner test plugin that rotates an
> arrow image by `z` = 90): positive `z` turns counter-clockwise on screen; rotations apply
> in the order `x`, `y`, `z` with an orthographic projection; hit testing ignores rotation.

<a id="13-21"></a>
### 13.21 Window scale and scaling groups (P73, P74)

The scaling members act through the client's UI-scaling service on a **scaler** element
(the `UIScaler` game types of `ui-behaviour.md` §9.2). Terms used below:

- **marked**: the window has been made scalable. The first `SetScale`,
  `SetUIScalingGroup` or `RegisterForGlobalScaling` on a window **without a parent** marks
  it; nothing unmarks it. On a window with a parent these three calls do nothing.
- **global**: the window takes part in the global UI scale (`UIScale_Global`,
  `ui-behaviour.md` §9.1).

Rules (BIN):

1. **Marking rebuilds the element** (P65): a plain window becomes a bare `UIScaler` element
   (no layout properties: no object mode, so its surface uses CanvasResize, §13.18; no size
   limits; group and origin set by the scaling calls below); a game-styled window is rebuilt
   from `LotroScalerWindow_Container` (§14.2.1), which in this install has object mode
   CanvasResize, scaling group 72, origin 0, 0 and the window size limits (values read from
   the layout at run time). In both cases the window is **resized** when scaled and its
   children follow their own edge modes (plugin children without attachments keep their
   positions, §13.16; the game-styled frame follows its layout edge modes). The title text
   is set again. > **OPEN:** rule 1's "children follow their own edge modes" when a scaled
   window is resized disagrees with the panel-scale rule that rule 5 applies
   (`ui-behaviour.md` §9.2 step 6 resizes scaler types in Stretch mode and then restores
   their object mode, so their children are not laid out again, §9.2.1); which of the two
   holds for a plugin `SetScale` was not settled. Tessera (decision 0015 §2.1, "Plugin
   `SetScale`"): follow `ui-behaviour.md` §9.2 step 6; the drawing handles both outcomes (a
   Stretch resize changes only how the surface is composed, a CanvasResize resize repaints
   it).
2. **Group.** `SetUIScalingGroup(g)` stores `g` as the element's
   `UIComponent_UIScaler_Group`, after replacing it by 72 when it is 0 (both window kinds)
   or 60 (plain windows only; a game-styled window keeps 60). `SetScale` sets the group to
   72 **every time** it is called, on both window kinds, undoing an earlier
   `SetUIScalingGroup`.
   `GetUIScalingGroup()` returns 0 for an unmarked window, else the stored group. The
   group's **value** is the window's own scale when one was set (rule 4), otherwise the
   group's option value (`ui-behaviour.md` §9.1); a group without an entry in
   `UIScaleDefinition_Array` (72 has none in this install) has the value 1.0.
3. **Global.** Each window keeps a "global wanted" flag: false for a new plain window, set
   true when a game-styled window is built as a scaler, set by `RegisterForGlobalScaling()`
   and cleared by `UnregisterForGlobalScaling()` (which does nothing on an unmarked window
   and leaves a marked one marked). `RegisterForGlobalScaling`, `UnregisterForGlobalScaling`
   and `SetUIScalingGroup` apply the flag to the scaler at once; a plain window's `SetScale`
   applies it too; a game-styled window's `SetScale` does not, so there the scaler keeps
   its current state (after a first, marking `SetScale` that is the scaler element's
   initial state, not traced). > **OPEN:** the initial global state of the game-styled
   scaler element (`UIScalerPanel`). Tessera: global.
4. **Effective scale** *s*. With *b* = the group's value (rule 2): a global window has
   *s* = *b* × `UIScale_Global` (when this is recomputed by `RegisterForGlobalScaling` or
   `SetUIScalingGroup` and `UIScale_Global` is 0, *s* keeps its previous value); a
   non-global window has *s* = *b*. `SetScale(f)`: `f` ≤ 0 raises an error (on a parentless
   window); otherwise the window is marked if needed, `f` becomes the window's own scale and
   *s* = `f` × `UIScale_Global` when global, else `f`. So `SetScale` takes a **factor**
   (1 = unscaled) and replaces the group's value for this window.
5. **Applying** *s* uses the panel-scale rule of `ui-behaviour.md` §9.2 steps 2–6 (fit to
   the screen, size = unscaled size × *s* with the size limits scaled, scaling origin kept
   in place, clamp to the screen, cache *s*). While the window is not visible the
   application is postponed and done when it is shown (Probable: a pending flag is set).
6. `GetScale()`: 1.0 for an unmarked window; otherwise *s* (including the global factor).
   Example: global window, `UIScale_Global` 1.25, `SetScale(2)` → `GetScale()` = 2.5.
7. `SetScalingOriginPoint(x, y)`: on a marked window, stores `x`, `y` as
   `UIComponent_UIScaler_Origin_X`/`_Y` (unscaled window pixels; the point that stays in
   place, `ui-behaviour.md` §9.2 step 4); on an unmarked window it does nothing.
   `GetScalingOriginPoint()` was not traced; Tessera returns the two stored values (0, 0 by
   default).
8. **`UIScaleChanged`** (P74) fires on a window when the scale of scaling group 60
   (`Global`) changes, or the scale of the group equal to the window's own group (as rule 2
   reports it). The argument table has `Index` = the group that changed and `Value` = its
   new value (§9.3). > **OPEN:** which windows the client notifies (all windows or only
   marked ones), and whether a `SetScale` on the window itself fires it. Tessera: notify
   every marked window; `SetScale` does not fire it.

Worked example (synthetic). A parentless plain window of unscaled size 300 × 200 at
(100, 100), origin (0, 0), on a 1920 × 1080 screen, `UIScale_Global` 1.0:
`RegisterForGlobalScaling()` marks it (rebuild) and makes it global, *s* = 1.0 (group 72,
value 1.0); `SetScale(1.5)` gives *s* = 1.5 × 1.0 = 1.5; the fit test passes
(1.5 ≤ 1920 ÷ 300 and 1.5 ≤ 1080 ÷ 200); the size becomes 450 × 300; with origin (0, 0) the
position stays (100, 100); `GetScale()` returns 1.5. With `UIScale_Global` 0.8 instead,
*s* = 1.2 and the size 360 × 240. Without the `RegisterForGlobalScaling()` call,
`SetScale(1.5)` alone marks the window non-global and *s* = 1.5 whatever `UIScale_Global`
is.

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
| 4 | 4 | u32 LE | **Line height** in pixels: the height of the line box, equal to the largest glyph height of the record (`fonts.md` §3.2). In this install it also equals the number at the end of the enum name (e.g. 20 for `TrajanPro20`); that is an observation, not a rule — read the field. (Corrected 2026-10-06: earlier revisions called it a "nominal point size".) |
| 8 | … | — | Widest glyph, glyph count, glyph entries and the trailer with the atlas DIDs: `fonts.md` §3.2 (the complete layout) |

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

The rest of this section (§14.2.1–§14.2.6) rests on BIN (fourth pass, P65–P67) and on the
layout data (O7). Element IDs, the entry key and the layout labels are what the client
uses; the layout DID, rectangles, edge modes, properties and art are **observations of this
install** and must be read from the user's data at run time (`ui-layout.md` §3–§7).

<a id="14-2-1"></a>
#### 14.2.1 How the controls are built (P65)

1. **Layout.** All seven classes use top-level elements of one layout: the one that the
   `UILAYOUT` DID map (root map key 4, `ui-layout.md` §3.2) gives for entry key
   **0x100000C8** (labelled `luaplugins`; 0x22000826 in this install, design size 1024 ×
   768).
2. **Element.** When the plugin layer creates the control's element (at construction), it
   writes a **based** element descriptor: no type of its own, base layout = that layout,
   base element = the ID below, rectangle 0, 0, 50 × 50 (P64), and a run-time element ID
   (a counter of the plugin layer). That ID replaces the base's (`ui-layout.md` §7.3
   step 2), so the control's top element carries the run-time ID; its inherited
   descendants keep their layout IDs.
   The client then builds it like any based descriptor: resolve the base
   (`ui-layout.md` §7), merge (`ui-layout.md` §7.2–§7.3: the 50 × 50 rectangle replaces the
   base's, the base's properties, states, media and children are inherited, the inherited
   children are re-laid out from the base's size to 50 × 50 by the design-time rule),
   create the tree (`ui-layout.md` §9.2), and for windows open it as a root panel
   (`ui-layout.md` §9.1 steps 2–5).

   | Class | Base element ID | Label in the layout (this install) | Engine or game type (resolved) |
   |---|---|---|---|
   | `Button` | 0x100022A7 | `LotroButton` | Button |
   | `GoldButton` | 0x100022A8 | `LotroButtonGold` | Button |
   | `CheckBox` | 0x100025AF | `LotroCheckBox` | Button |
   | `TextBox` | 0x100022A9 | `LotroTextBox` | Field |
   | `ScrollBar`, horizontal | 0x1000239E | `LotroScrollBar_Horizontal` | Scrollbar |
   | `ScrollBar`, vertical | 0x1000239F | `LotroScrollBar_Vertical` | Scrollbar |
   | `Window` | 0x10002294 | `LotroWindow_Container` | Field |
   | `Window`, marked as scaled (§13.21) | 0x10004D37 | `LotroScalerWindow_Container` | `UIScalerPanel` (game type 0x10000272) |
   | `GoldWindow` | 0x10002299 | `LotroWindowGold_Container` | Field |
   | `GoldWindow`, second variant | 0x10004D38 | `LotroScalerWindowGold_Container` | `UIScalerPanel` |
   | plain `Turbine.UI.Window` marked as scaled | none in effect: the descriptor has the type `UIScaler` (0x100000E8), which wins over the base reference 0x10004CE3 it also carries (`ui-layout.md` §7.1 step 1) | (`LuaWindow_Container`, not used) | `UIScaler` |

   The scroll bar's element is chosen by its orientation when the element is built; the
   window's by whether it is marked as scaled. A `GoldWindow` chooses its scaled variant by
   a different flag, which the plugin layer sets in the control's constructor from a query
   to the UI-scaling service; its meaning was not traced. > **OPEN:** what that query
   answers (Probable: whether the plugin's manager option "use global scaling",
   `Plugin:GetUsesGlobalScaling()`, §5.4, is on). Tessera: a `GoldWindow` uses its scaled
   variant exactly when a `Window` in the same state would.
3. **Rebuilds.** The element is destroyed and built again from a new descriptor when a window
   is first marked as scaled (§13.21) and when a game-styled scroll bar's orientation changes
   (it then fires `OrientationChanged`). A rebuild keeps visibility, position, size, z-level
   and the background image, but passes the old element's base-layer **operation number**
   (§13.17) to `SetBlendMode` as if it were a blend-mode value, so the operation can change.
   Tessera reproduces this exactly.

   The operation numbers (BIN, repair of the fourth pass: the plugin's blend-mode setter,
   which stores the value and converts it, the element's operation setter and getter, and
   the rebuild routine, which reads the old element's operation number and passes it to
   that setter; numeric constants under decision 0012):

   | Blend mode last set (`n`) | Operation (§13.17) | Operation number | After one rebuild: `GetBlendMode()` | Operation after one rebuild |
   |---|---|---|---|---|
   | never set (new control), 0 or 2 `Normal` | Replace | 0 | 0 | Replace |
   | 5 `Overlay` | Mix colour | 1 | 1 | Colorize |
   | 4 `AlphaBlend` | Over | 2 | 2 | Replace |
   | 1 `Color` | Colorize | 3 | 3 | Multiply |
   | 3 `Multiply` | Multiply | 4 | 4 | Over |
   | 7 `Screen` | Screen | 5 | 5 | Mix colour |
   | 6 `Grayscale` | Grayscale | 6 | 6 | Grayscale |
   | 8 `None`/`Undefined` | "none" | 7 | 7 | Screen |

   Rules:
   - A new element's operation number is 0 (Replace), as §13.17 states; so a control that
     never called `SetBlendMode` reports `GetBlendMode()` 2 before a rebuild and **0**
     after it, with the same Replace drawing.
   - Operation numbers are 0–7, so the call made by the rebuild never raises the
     "above 8" error of §13.17.
   - The number read is the old element's current base-layer operation, whoever set it.
     The top elements that are rebuilt (the four window containers, the two scroll bars
     and the plain window) carry no media items of their own in this install (O7), so the
     number comes from the plugin's `SetBlendMode` or is the initial 0. Tessera reads the
     number from its own element state, not from a cached blend-mode value.
   - The mapping is applied again at **every** rebuild. A window is rebuilt at most once
     (when first marked as scaled), but a game-styled scroll bar is rebuilt at every
     orientation change, so repeated changes walk the table: for example `SetBlendMode(8)`
     followed by three orientation changes gives `GetBlendMode()` 7, then 5, then 1, with
     the operations Screen, Mix colour and Colorize; every chain except the one starting
     at 6 (`Grayscale`, a fixed point) ends at 0 (Replace) after at most seven rebuilds.

   Example: a plugin calls `SetBlendMode(Turbine.UI.BlendMode.AlphaBlend)` (4, Over,
   operation 2) on a window and then `SetScale(1.5)`; the rebuild calls `SetBlendMode(2)`,
   so `GetBlendMode()` reports 2 and the background is drawn with Replace.
4. **Skins** reach these controls like any layout element: their Image items pass the
   base-image setter (`skin-definition.md` §6.8, SK42). A skin's geometry override can never
   change the control's own rectangle: the descriptor's 50 × 50 replaces the base's, and
   the plugin's later sizes replace that. Whether a top-level override keyed by one of these
   base IDs is applied at all was not traced (overrides match the ID path,
   `ui-layout.md` §9.8, and the built top element carries the run-time ID of step 2); if it
   is applied to the base, it changes only the size from which the inherited children are
   re-laid out (`ui-layout.md` §7.3 step 8). Tessera: apply no top-level override to these
   controls; overrides of their descendants by ID path apply as usual.
5. **Fallback.** If the layout, the entry key or an element is missing (a modified install),
   the client cannot build the element. Tessera: build a plain element of the base class
   (as `Turbine.UI.Window`, `Button`, …) with its own neutral art, and log once per class.

<a id="14-2-2"></a>
#### 14.2.2 Text and state of the game-styled controls (P67)

- **Button**, **GoldButton**: the button element itself is the label's text element (§13.4).
  Its text properties, button states and art come from the layout (resolved through its
  base chain); `Click`, `MouseClick` and the other events follow §13.6 and §13.12.
- **TextBox**: the text element is the descendant with ID 0x100022AA (`LotroTextBox_Text`);
  every `Label`/`TextBox` member acts on it. The control's own element only carries the
  frame art (a box frame from its base chain).
- **CheckBox**: the text element is the descendant 0x100025B1 (`LotroCheckBox_Text`); the
  check mark is the descendant 0x100025B0 (`LotroCheckBox_CheckBox`, a two-state button
  with `UICore_Button_boolean_button`), which shows the checked state.
- **ScrollBar**: the layout's scroll-bar widget supplies its own buttons and thumb
  (`UICore_Scrollbar_increment_button`, `_decrement_button`, the engine-level `widget`
  child); the value rules are §9.6 and §13.7.
- **Window**, **GoldWindow**: see §14.2.3.

<a id="14-2-3"></a>
#### 14.2.3 Game-styled windows: parts, title, close button, resizing (P66)

**Parts** (descendants of the window's element; IDs are the client's, the geometry and art
come from the layout). For a window of width *W* and height *H*, the layout of this install
gives the following rectangles (relative to the window's top-left; they follow from the
parts' design rectangles in the 300 × 300 container and their edge modes by
`ui-layout.md` §8, applied through the 50 × 50 creation size):

| Part (ID) | Label | Type | z-level | Rectangle (left, top, width × height) | Notes |
|---|---|---|---|---|---|
| 0x10002295 | `LotroWindow_Background` | Field | 100 | 0, 16, *W* × (*H* − 18) | The framed background (corners, edges and centre from its base chain) |
| 0x10002296 | `LotroWindow_TitleBarBackground` | Field | 90 | by code, below | Title-bar art |
| 0x10002297 | `LotroWindow_TitleBarText` | Text | 80 | by code, below | Caption; its text style comes from its base (a template of the text-style layout, `fonts.md` §9.1) |
| 0x10002DFC–0x10002E03 | `LotroWindow_ResizeNW`, `…N`, `…NE`, `…W`, `…E`, `…SW`, `…S`, `…SE` | ResizeBar | 10 | NW 0, 16, 20 × 20; N 20, 16, (*W* − 40) × 10; NE *W* − 20, 16, 20 × 20; W 0, 36, 10 × (*H* − 56); E *W* − 10, 36, 10 × (*H* − 56); SW 0, *H* − 20, 20 × 20; S 20, *H* − 10, (*W* − 40) × 10; SE *W* − 20, *H* − 20, 20 × 20 | Resize handles (`ui-behaviour.md` §8.2); hidden unless resizable |
| 0x10002298 | `LotroWindow_CloseButton` | Button | 5 | *W* − 21, 21, 16 × 16 | Close button |
| 0x10002DFB | `LotroWindow_Dragbar` | Dragbar | 0 | by code, below | Lets the user move the window (`ui-behaviour.md` §8.1) |

The container itself (the window's element) has no art; its layout properties make it
activatable and give it the size limits of §13.1 (150 × 100 in this install). There is **no
content child**: the plugin's child controls are children of the window's element, placed
relative to the window's top-left corner. Sibling order follows `ui-layout.md` §9.3 (lower
z-level in front): a child with the default `ZOrder` 0 has depth 0 (§13.14), so it is in
front of the background, title bar, title text, resize bars and close button (z-levels 100,
90, 80, 10, 5) and at the drag bar's level (0); a child with `ZOrder` −10 (depth 10) is
behind the close button and level with the resize bars. Plugins therefore keep their
content clear of the frame themselves. > **OPEN:** the read order the client gives to
plugin-created elements, which decides whether a depth-0 plugin child or the drag bar is in
front where they overlap. Tessera: plugin children are in front of every layout child of
equal depth.

**Title layout.** The client positions the title text, the title-bar background and the drag
bar from the window's current width *W* and the measured width *m* of the caption (the width
of the title text element's laid-out caption, in pixels, `fonts.md` §7):

1. *t* = min(*W* − 140, max(*m*, 120)) (32-bit signed integers; *t* can be negative for a
   window narrower than 140).
2. Title text: left = (*W* − *t*) ÷ 2 truncated toward zero, width *t*; its top and height
   are kept (5 and 24 in this install); its four text margins are set to 0.
3. Title-bar background and drag bar: width *t* + 80, left = (*W* − (*t* + 80)) ÷ 2 truncated
   toward zero; tops and heights kept (−8 and 41 in this install, so the title bar rises 8
   pixels above the window's rectangle).

The title layout runs when the element is built (with *m* = 0 for a new window, so
*t* = min(*W* − 140, 120)), after every `SetText` (which first sets the caption and measures
it), and after every size change of the window, **after** its `SizeChanged` handlers ran.
The constants 140, 120 and 80 are the client's (decision 0012); the part heights and tops are
data.

**Close button.** When the close button reports the UI message `Button_click` (0x35,
`UIElementMessage` key), the window runs its native `Close()` (§13.3): `Closing`, then hiding
and `Closed`, unless cancelled. Which mouse gesture makes a button report `Button_click` is
the button widget's rule (§13.6 OPEN; `ui-behaviour.md` §4.6).

**Resizing.** `IsResizable()` is false for a new window. `SetResizable(b)` does nothing when
`b` equals the stored value; otherwise it stores `b` and sets `UICore_Element_hide` on all
eight resize bars to `not b`. Visible resize bars resize the window as `ui-behaviour.md`
§8.2 describes (the target is the window's element, the bars' parent), with the window's
size limits; each resize fires `SizeChanged` (§9.6) and re-runs the title layout.

**Moving.** The drag bar moves the window (`ui-behaviour.md` §8.1).

Worked example (synthetic layout values as above). A window sized 400 × 300 with caption
width *m* = 96: *t* = min(260, max(96, 120)) = 120; title text left (400 − 120) ÷ 2 = 140,
width 120; title bar and drag bar width 200, left 100; close button at 379, 21; background
0, 16, 400 × 282; resize bar E at 390, 36, 10 × 244. After `SetSize(200, 300)`:
*t* = min(60, 120) = 60, title text left 70, width 60; title bar width 140, left 30.

<a id="14-2-4"></a>
#### 14.2.4 What plugins can rely on

- `GetSize()` of a new game-styled control is 50 × 50 (§13.1); plugins set their size.
- The frame metrics a plugin sees are the layout's (§14.2.3): in this install the title bar
  reaches down to y = 32, the framed background starts at y = 16 and fills the rest, and the
  resize handles are 10 pixels deep along the edges. Tessera reads these from the layout and
  never hard-codes them.
- The game-styled controls accept every member of their base classes (§13.2–§13.7),
  including `SetBackground` and `SetBackColor`, which draw on the control's own element
  (§13.17) under its inherited children.

<a id="14-2-5"></a>
#### 14.2.5 Invariants of the layout data (check at run time, report on failure)

- The `UILAYOUT` map has entry key 0x100000C8, and its layout has top-level elements with
  the ten base IDs of §14.2.1 (0x100022A7, 0x100022A8, 0x100025AF, 0x100022A9, 0x1000239E,
  0x1000239F, 0x10002294, 0x10004D37, 0x10002299, 0x10004D38), each of which resolves
  (`ui-layout.md` §7.1).
- In the window containers 0x10002294, 0x10002299, 0x10004D37 and 0x10004D38 the parts of
  §14.2.3 exist as direct children (IDs 0x10002295–0x10002298, 0x10002DFB–0x10002E03).
- The text box element has the descendant 0x100022AA; the check box element has the
  descendants 0x100025B0 and 0x100025B1.

<a id="14-2-6"></a>
#### 14.2.6 Tessera neutral art

Tessera builds these controls from the user's own layout and art (they are skinnable,
`skin-definition.md` §6.8). Only when the data are missing (§14.2.1 step 5) does it draw
its own neutral frame; that frame uses the same part rectangles and z-levels as §14.2.3 so
that plugin layouts stay correct.

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

> **OPEN:** (clean-room implementer, 2026-10-08) which shortcuts `SetShortcut` calls invalid
> (BIN says only that an invalid one raises), what `Shortcut(item)` stores, the default of
> `IsUseOnRightClick()`, and which mouse buttons activate a quickslot. Tessera: only a value
> that is not a `Shortcut` object raises; `Shortcut(item)` stores type `Item` with empty
> data; `IsUseOnRightClick()` is false for a new quickslot; a left click activates, a right
> click only with `SetUseOnRightClick(true)`; `Lotro.DragDropInfo:GetShortcut()` returns
> the drag source's shortcut when the source is a plugin quickslot, otherwise nil.

---

## 15. Gameplay views (`Turbine.Gameplay`)

Read-only views of client-side game state, fed in Tessera by the session layer. Every getter
returns the current value; every `XChanged` event fires after the value changed. The full
member list is in §16; the main entry points are in the table below.

**When `…Changed` events are delivered in Tessera (Tessera rule, decision 0014 D8 and its
Known difference 21).** The session layer and the gameplay views never call Lua. A value
change appends its `…Changed` event, with its argument values taken at that moment, to the
plugin host's deferred-work queue, and the host delivers it in frame-loop step 7, one event
being one item, in queue order (§3.3), to the handlers set at delivery; an apartment closed
in between gets nothing. The rule "fires after the value changed" still holds. Two changes
of one value in one frame give two events in that order, but a getter called in the first
handler already returns the second value, because getters return the value at delivery.
> **OPEN:** when the original delivers these events relative to the change (synchronously,
> or later in the frame). Decision 0014 (Known difference 21) asks this; no surveyed plugin
> behaviour is known to depend on it. Tessera: the queued delivery above.

The main entry points:

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

`Equipment` slot values are not keys of a data enum mapper, but they follow one at a fixed
offset: value = `ContainerSlot` key − 1 for the keys 2 … 22 (P82, §15.1). `ItemDurability`
and `ItemWearState` equal the keys of their data enums (P82, §15.1). `EffectCategory`
matches only partly.

> **OPEN:** whether `Alignment`, `Profession`, `Vocation`, `ReadyState`, `SkillType`,
> `EffectCategory` and the stance enums follow a data enum (narrowed 2026-10-09: `Equipment`,
> `ItemDurability` and `ItemWearState` are answered by P82).

Differences of this build from S2 and from the S3 dump: see §16.

<a id="s15-1"></a>
### 15.1 Inventory views in Tessera (decision 0024 P1)

The views read the session's inventory model (`character-panels.md` §8–§9), which the
scenario feed fills in the sandbox (`scenario-feed.md` §5.5, §9.2) and a server backend
would fill the same way. Every `…Changed`, `ItemAdded` etc. event follows the delivery rule
of the first paragraph of §15 (queued, delivered in frame-loop step 7). (`scenario-feed.md`
is Accepted on branch `p0`; its links resolve once that branch is merged.)

The facts about the original are P81 (S2) and P82 (data keys). Everything else in this
section is a **Tessera rule** (P83): it says how Tessera derives the API values from its own
inventory model, not how the original does.

**Containers.**

| View | Rule |
|---|---|
| `LocalPlayer:GetBackpack()` | One `Backpack` object per apartment (§9.5 singletons). |
| `Backpack:GetSize()` | The backpack size *N* of `character-panels.md` §4.3. |
| `Backpack:GetItem(i)` | The item in backpack slot *i* − 1 for integer *i* in 1 … *N*; nil for an empty slot, a non-integer, or *i* outside 1 … *N*. |
| `Backpack` events | `ItemAdded { Index, Item }` for each item that enters the backpack (feed `ItemAdded` into the backpack, or `ItemMoved` from another container into it); `ItemRemoved { Index, Item }` for each that leaves it (feed `ItemRemoved` of a backpack item, or `ItemMoved` from the backpack to another container); `ItemMoved { OldIndex, NewIndex, Item }` for a move within the backpack (a swap gives two events in the feed's order). Indices are 1-based (slot + 1). |
| `Backpack.SizeChanged` | Whenever *N* takes a new value: on a `ContainerSize` of `Backpack` that changes it, and, before the first `ContainerSize`, when an arriving item raises *N* through the "highest occupied slot + 1" term of `character-panels.md` §4.3; in that case `SizeChanged` is queued before the item's `ItemAdded`. No event when *N* stays the same. |
| `LocalPlayer:GetEquipment()` | One `Equipment` object per apartment. |
| `Equipment:GetItem(slot)` | The item in data slot key `slot` + 1 for integer `slot` in 1 … 21 (P82); nil for an empty slot or any other value. |
| `Equipment:GetSize()` | 21 (the number of API slot values, P82). |
| `Equipment` events | `ItemEquipped { Index = key − 1, Item }` when an item enters the `Equipment` container at a key 2 … 22, `ItemUnequipped { Index, Item }` when it leaves; nothing for keys outside 2 … 22. |

**Items.** "The record" is the item's data record (`items.md`), read through its property
set with inheritance. A **literal item** (`scenario-feed.md` §5.2) has no record. "Absent"
means the record (with its inherited properties) does not carry the property.

`Item` getters (an `Item` is an `Entity`):

| Getter | Value (record present) | Absent | Literal item |
|---|---|---|---|
| `GetName()` (from `Entity`) | rendered `Name` | empty string | the literal text |
| `GetItemInfo()` | the `ItemInfo` of the record: one object per record DID per apartment | — | an `ItemInfo` of its own, one per `Item` object, with the literal-item values of the next table |
| `GetQuantity()` | the feed's quantity (last `ItemQuantity`, else the quantity it arrived with) | — | the same |
| `GetWearState()` | the state of `item-tooltips.md` §11 after an `ItemCondition`; before one, the `Item_WearState` key | 0 (`Undefined`) | 0, or the `ItemCondition` state |
| `GetCategory()`, `GetDurability()`, `GetQuality()`, `GetMaxStackSize()`, `IsMagic()`, `GetQualityImageID()`, `GetBackgroundImageID()`, `GetUnderlayImageID()`, `GetShadowImageID()` | the same value as the getter of that name on `GetItemInfo()` | as there | as there |
| `GetIconImageID()` | `icon-layers.md` §3.1 step 1 for an item instance (the instance's checkpointed override when the item carries one, else `Icon_Layer_ImageDID`) | 0 | 0 |
| `QuantityChanged` | after each feed `ItemQuantity` of the item that changes its quantity | — | the same |

`ItemInfo` getters:

| Getter | Value (record present) | Absent | Literal item |
|---|---|---|---|
| `GetName()` | rendered `Name` | empty string | the literal text |
| `GetDescription()` | rendered `Description` | empty string | empty string |
| `GetNameWithQuantity(q)` | for *q* ≤ 1 (or not a number) the name; otherwise the decimal integer part of *q*, one space, then the rendered `PluralName` | without `PluralName`: the name in its place | the same rule with the literal text as name and no `PluralName` |
| `GetCategory()` | the `Item_Class` key, unchanged (also when it is not a member of `ItemCategory`, P41) | 0 (`Undefined`) | 0 |
| `GetDurability()` | the `Item_DurabilityEnum` key (= `ItemDurability` value, P82) | 0 (`Undefined`) | 0 |
| `GetQuality()` | the `Item_Quality` key (= `ItemQuality` value, P41) | 0 (`Undefined`) | 0 |
| `GetMaxStackSize()` | `Inventory_MaxQuantity` | 1 | 1 |
| `GetMaxQuantity()` | the value of `GetMaxStackSize()` | 1 | 1 |
| `IsUnique()` | `Inventory_Unique` | false | false |
| `IsMagic()` | false | false | false |
| `GetIconImageID()` | `Icon_Layer_ImageDID` (`icon-layers.md` §2.2) | 0 | 0 |
| `GetBackgroundImageID()` | `Icon_Layer_BackgroundDID` | 0 | 0 |
| `GetUnderlayImageID()` | `Icon_Layer_UnderlayDID` | 0 | 0 |
| `GetShadowImageID()` | `Icon_Layer_ShadowDID` | 0 | 0 |
| `GetQualityImageID()` | 0 | 0 | 0 |

An image getter returns 0 also when the property is present with value 0. Plugins pass a
non-zero value to `SetBackground` (§13.8); 0 means "no image".

**Item enums (answers part of the §15 OPEN on enums, 2026-10-09):** `ItemDurability` and
`ItemWearState` follow data enums name for name and value for value: `ItemDurability` 1–7 =
the keys of mapper 0x230000FA (`Item_DurabilityEnum`: `Substantial`, `Brittle`, `Normal`,
`Tough`, `Flimsy`, `Indestructible`, `Weak`), `ItemWearState` 1–4 = the keys of mapper
0x230000FB (`Item_WearState`: `Damaged`, `Pristine`, `Broken`, `Worn`) (P82). The getters
therefore return the data keys unchanged.

> **OPEN:** what the original's `IsMagic()`, `GetMaxQuantity()` (as opposed to
> `GetMaxStackSize()`), `GetQualityImageID()` and `GetNameWithQuantity()` return; what its
> getters return when the item's data lacks the property; and whether `ItemAdded` and
> `ItemRemoved` fire for moves between containers. **Tessera rules** as in the tables above
> (P83).

> **OPEN:** (implementer note, `crates/gamedata/src/inventory`, 2026-10-10; review note (d))
> The order of the plugin events of one feed `ItemMoved` that crosses containers, and a move
> within `Equipment` between keys 2 … 22, are not stated. As implemented: the leaving event
> first (`Backpack.ItemRemoved` or `ItemUnequipped`), then the entering one; a move within
> `Equipment` gives `ItemUnequipped` (old key) then `ItemEquipped` (new key); when the
> arrival raises *N* (`character-panels.md` §4.3 rule 1), `SizeChanged` precedes both.

> **OPEN:** (implementer note, `crates/gamedata/src/inventory`, 2026-10-10) After a
> `ContainerSize` shrinks the backpack to *N*, items left at slots ≥ *N* are shown by no bag
> (`character-panels.md` §4.3) and `GetItem` never returns them, but the `Backpack` events
> row above still derives `ItemMoved`, `ItemAdded` and `ItemRemoved` with their indices
> slot + 1 > `GetSize()` when such an item moves or is removed. The `character-panels.md`
> §4.3 rule 5 promise (indices in 1 … `GetSize()`) covers only growth. Should slots ≥ *N* count
> as outside the backpack for the events (no event, or `ItemAdded`/`ItemRemoved` when an item
> crosses *N*)? As implemented: the row as written (events with indices above `GetSize()`).

> **OPEN:** (implementer note, `apps/client/src/character/plugin.rs`, 2026-10-10, after
> review) The tables do not say what the views do when the player leaves the world (a
> `Disconnected`, or a second `WorldEntered` that clears the inventory model). As
> implemented: every item the views held leaves them as if it were removed, in ascending
> place order (`Backpack.ItemRemoved` with its old index, `Equipment.ItemUnequipped` for keys
> 2 … 22, nothing for keys 23 … 25), then `Backpack.SizeChanged` fires only when *N* changed.

> **OPEN:** (implementer note, `crates/plugins/src/inventory.rs`, 2026-10-10; review note (e))
> Three values the tables above leave open. (1) `GetNameWithQuantity(q)` for a *q* that is
> not finite (infinity, NaN) or is a string such as `"3"`: as implemented, the name (treated
> as "not a number"); a finite *q* > 1 of any size gives all digits of its integer part.
> (2) Whether `GetItem` and the events return **one `Item` object per item per apartment**
> (so `ItemAdded`'s `Item == GetItem(Index)`, and fields or handlers set on it, such as
> `QuantityChanged`, persist): as implemented, yes, keyed by the item ID, as long as the
> item is in a container or a plugin holds the object; an item that left every container
> answers its getters with the last values seen. (3) `Backpack:GetItem` and
> `Equipment:GetItem` with an argument that is not a number (nil, a string): as implemented,
> nil, like a non-integer.


<a id="s15-4"></a>
### 15.4 Party views in Tessera (decision 0024 P4)

The views read the client half's fellowship model of `social-panels.md` §9 TR2, which the
scenario feed fills through `FellowshipChanged` (`scenario-feed.md` §6.4, §9.2) and a server
backend would fill the same way. Every event follows the delivery rule of the first paragraph
of §15 (queued, delivered in frame-loop step 7). The numbers 15.2 and 15.3 belong to the P2
and P3 revisions; this subsection is 15.4 so that the three merge without renumbering.

**API facts** (P90–P92). A `Party` is the player's fellowship (a monster player's group).
`GetMemberCount()` is the length of the member list, which includes the local player;
`GetMember(i)` takes 1 … `GetMemberCount()` and returns nil otherwise; the local player's
entry, and the leader when the player leads, are returned as the `LocalPlayer` instance; the
assist-target getters have the same shape over the assist-target list. `MemberAdded`,
`MemberRemoved`, `AssistTargetAdded` and `AssistTargetRemoved` carry `Player` (the member)
when the member object is available; `LeaderChanged` and `Player.PartyChanged` carry nothing.

Everything else below is a **Tessera rule** (P93).

**Model.** The members are the list of the last `FellowshipChanged` read by
`social-panels.md` §9 TR2 (the player identified by entity, else by name; a list without the
player read with the player appended). A **party episode** runs from a list that becomes
non-empty to the next empty list.

| View | Rule |
|---|---|
| `LocalPlayer:GetParty()` (from `Player`) | nil outside an episode; inside, the episode's `Party` object, one per apartment per episode (a new episode gives a new object) |
| `Party:GetMemberCount()` | the number of members, the player included |
| `Party:GetMember(i)` | for integer *i* in 1 … count, the *i*-th member in list order: the `LocalPlayer` instance for the player, a `PartyMember` otherwise; nil for any other argument (a non-number included) |
| `Party:GetLeader()` | the leader of TR2 (the `LocalPlayer` instance when the player leads); nil when no member has `leader` |
| `Party:IsMember(p)` | true when `p` is the `LocalPlayer` instance, or a `PartyMember` of this party whose name is in the current list; false otherwise (nil and non-objects included) |
| `Party:GetAssistTargetCount()`, `GetAssistTarget(i)`, `IsAssistTarget(p)` | 0, nil, false: the feed carries no assist targets; `AssistTargetAdded` and `AssistTargetRemoved` never fire |
| `PartyMember` objects | one per apartment per member name per episode; a member who leaves and returns in the same episode gets the same object; an object outlives its membership and then answers with the last values seen |
| `PartyMember:GetName()` | the member's name |
| `PartyMember:IsLocalPlayer()` | false |
| `PartyMember:GetParty()` | the episode's `Party` while the name is in the list, else nil; `GetRaid()` and `GetPet()` nil |
| `PartyMember:GetLevel()`, the morale and power getters, `GetTarget()`, `GetEffects()` | as the `Actor` getters of the P2 revision (§15.2) for the member's entity, from the events the feed sent for that entity; a member without an entity answers 0, nil and an empty `EffectList` |
| `PartyMember:GetClass()`, `GetRace()`, `GetAlignment()`, `GetReadyState()` | 0 |
| `PartyMember:IsLinkDead()`, `IsVoiceEnabled()`, `IsVoiceActive()` | false |

**Events** from two consecutive member lists, members compared by name:

1. Empty → non-empty: `LocalPlayer.PartyChanged` (empty table). No member events.
2. Non-empty → empty: `LocalPlayer.PartyChanged`. The old `Party` gets no events.
3. Otherwise, on the episode's `Party`: `MemberRemoved { Player }` for each removed member in
   old-list order; then `MemberAdded { Player }` for each added member in new-list order; then
   `LeaderChanged` (empty table) when the leader's name changed (to or from none included).
   `Player` is the member's `PartyMember` object (Tessera always has it).
4. Identical lists (names, order, leader): no event. A change of order alone gives no event;
   `GetMember` follows the new order at once.

`PartyMember` objects fire no events of their own in P4; `Actor` events of the member's
entity, if any, are those of §15.2.

> **OPEN:** (1) the original's member order in `GetMember` (`social-panels.md` Q3); (2)
> whether the original fires `MemberAdded` for the members present when a party forms, and
> `PartyChanged` on membership changes; (3) whether a `PartyMember` is the same object as an
> `Actor` object of the same entity obtained elsewhere (for example through `GetTarget`); (4)
> what the original's `PartyMember` getters return for a member out of view. **Tessera
> rules** as in the tables above (P93).

> **OPEN:** (implementer note, `crates/plugins/src/party.rs`, 2026-10-11) Cases the tables
> above leave open. (1) A `Party` object whose episode has ended: as implemented it answers
> as an empty party (`GetMemberCount()` 0, `GetMember`, `GetLeader` nil, `IsMember` false
> for every argument, the `LocalPlayer` instance included); a member event of that episode
> still queued when the episode ended (two `FellowshipChanged` in one frame) is not
> delivered (read with rule 2: the old `Party` gets no events). (2) The `PartyMember`
> `Actor` getters: §15.2 is not on this branch; as implemented, the level and the ten
> morale and power getters read a host source by entity (the client's TR3 member data; 0
> for a member without an entity), `GetTarget()` is nil and `GetEffects()` the empty
> `EffectList`, until the P2 revision's rules are merged. When the source has no values
> for a member's entity (while a member, or when its removal is delivered) the object keeps
> the last values seen. These getters, `GetName`, `IsLocalPlayer`, `GetParty`, `GetPet` and
> the `Player` getters are the base classes' methods (`Entity`, `Actor`, `Player`), which
> answer a `PartyMember` object; `PartyMember` itself holds only `GetRaid` (see (5)). (3) Names
> that occur twice in one list (or a member with the player's name besides the player's
> own entry): as implemented the player's entry takes no part in the name comparison; each
> other name gives at most one `MemberAdded`/`MemberRemoved`; `GetMember` returns the one
> `PartyMember` of that name for every entry with it. (4) A list longer than the feed's 64
> entries (a server backend): as implemented only the first 64 are read, the player
> appended after them when not among them. (5) `GetRaid()`: the table above asks for it
> on `PartyMember`, but §16 lists no `GetRaid` on `Player` or `PartyMember` for this build
> (only S2 names it), so the original may have none (calling it would then raise an error).
> As implemented, `PartyMember` holds `GetRaid` as its one own field (nil) until it is known
> whether the build has it, and whether Tessera should keep it as a Tessera rule. (6) A
> `self` without a native object: `Party`'s own methods and `PartyMember.GetRaid` raise a
> Lua error (§8.2); the inherited `Entity`, `Actor` and `Player` getters, shared with the
> P1 views, answer such a `self` with their defaults (0, nil, false), as the §15.1 views
> do; whether §8.2 should apply to them too is open.

Worked example (`social-panels.md` §12.1, player Tam, actors Brin and Cael): the first list
gives `PartyChanged`, `GetMemberCount() == 2`, `GetMember(1) == LocalPlayer.GetInstance()`,
`GetMember(2):GetName() == "Brin"`, `GetLeader() == LocalPlayer.GetInstance()`; adding Cael
gives `MemberAdded` with `Player:GetName() == "Cael"`; the leader change gives one
`LeaderChanged`; dropping Cael gives `MemberRemoved` (Cael); `fellowship none` gives
`PartyChanged`, and `GetParty()` is nil.

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
23. **Fonts:** for every non-zero `Font` value `v`, the record `v` exists in the archive set,
    its payload's first u32 equals `v`, and its second u32 (the line height, §14.1) equals
    the largest glyph height *H* in the record (`fonts.md` F4). In the user install it also
    equals the name's trailing number for all 48 (observation 2026-10-03; not a rule).
24. **Actions:** every non-zero `Action` value is a key of the input-action enum mapper read
    through EMAPPER (holds for all 269, 2026-10-05).
25. **No crash on hostile input:** malformed descriptors, binary or huge Lua files, deep
    recursion, `Sort` with an inconsistent comparator, invalid DIDs and image files, cyclic
    tables passed to `Save`, and malformed `.plugindata` files all produce errors or empty
    results, never a panic.
26. **Notification queueing (§9.7):** with the worked example of §9.7, the log is exactly
    `MouseClick`, `SizeChanged`, `VisibleChanged`, and `GetWidth()` inside the `MouseClick`
    handler returns 50. In top-level code, `c:SetSize(10, 10)` on a control of another size
    has fired `SizeChanged` when it returns; `c:SetSize` with the current size fires nothing.
27. **Update broadcast (§9.4):** a control that was never parented and is not visible, with
    `SetWantsUpdates(true)`, receives `Update` once per tick. The worked example of §9.4
    gives A, B, D in the first tick.
28. **Size limits (§13.15):** the worked example of §13.15 holds step by step;
    `GetMaximumWidth()` of a new control is 0; after `SetMaximumWidth(0)`, `SetWidth(10)`
    gives width 0; after `SetMinimumWidth(30)` and `SetMaximumWidth(20)`, `SetWidth(25)`
    gives 30.
29. **Scroll bar value (§9.6):** with minimum 0, maximum 100 and value 100 (not bound),
    `SetValue(100)` fires nothing; `SetValue(1000)` fires one `ValueChanged` and the value
    stays 100; from value 50, `SetValue(-5)` gives 100. A new scroll bar reports minimum 0,
    maximum 100, value 0, large change 10, small change 1. The worked example of §9.6 gives
    exactly the getter results and the two `ValueChanged` stated there; after `SetMinimum(-5)`
    on a new bar, `GetMinimum()`, `GetMaximum()` and `GetValue()` all return 4294967291. With
    minimum 5 and maximum 10, `SetValue(20)` and `SetValue(-1)` give 10 and `SetValue(2)`
    gives 5. After any sequence of the five setters, minimum ≤ value ≤ maximum and small
    change ≤ large change hold as unsigned numbers.
30. **Edge attachments (§13.16):** for a parent resized from 200 × 100 to 300 × 150 with the
    child references of the worked example, each row of the §13.16 table gives the stated
    rectangle; `GetEdgeAttachments()` returns the four values passed to `AttachEdges` in the
    same order; a control without `AttachEdges` keeps its rectangle when its parent resizes.
31. **Mouse buttons (§13.12.1):** the `Button` value is 1 for actions 0x56 and 0x33, 2 for
    0x4C and 0x2E, 4 for 0xA4 and 0x93, 0 for 0x82; an action whose toggle type is 2 never
    produces `MouseDown`, `MouseUp`, `MouseClick` or `MouseDoubleClick`.
32. **Capture (§13.12.2):** press over control C, move the pointer off C, release: C gets
    `MouseDown`, `MouseUp` and no `MouseClick`; `MouseMove` events between press and release
    all go to C. Press and release over C give `MouseDown`, `MouseUp`, `MouseClick`.
33. **Hit test (§13.11):** with control M (mouse-invisible) above sibling S, both under the
    pointer, a press goes to S; a mouse-visible child of M under the pointer gets it instead;
    the part of a child that lies outside its parent's rectangle never receives
    `MouseEnter`.
34. **Enter/leave order (§13.12.4):** moving the pointer from outside a window W straight
    onto button B inside it gives `MouseEnter` on B, then on W; moving from B to W's
    background gives only `MouseLeave` on B.
35. **Keys (§13.13):** a hidden control without focus, with `SetWantsKeyEvents(true)`,
    receives `KeyDown` and `KeyUp` for an action that no focused control consumes; two such
    controls receive them in the order in which they subscribed.
36. **Focus and activation (§13.13, §13.14):** the worked example of §13.13 produces exactly
    that event order; `Activate()` on the active window produces no event; a press over a
    control of an inactive plugin window produces that window's `Activated` before the
    control's `MouseDown`.
37. **Error to chat (§6.6):** a handler that calls `error("x")` produces exactly one chat
    line of chat type 1 whose text is the Lua error message (chunk name, line, `x`), with
    nothing added before or after it.
38. **Plugin-data reader (§10.4):** the byte-exact original files of §10.4 (`return 5.000000`,
    `return "a\"b"`, `return true`, the 15-byte empty table) load as 5, `a"b`, `true` and an
    empty table.
39. **Initial geometry (§13.1):** a fresh `Turbine.UI.Control()`, `Window()`, `Label()`,
    `TextBox()`, `Button()` and `ScrollBar()` each report position 0, 0 and size 50, 50.
40. **No attachments (§13.16):** a child without `AttachEdges`, sized 0 × 0 and placed at
    (10, 10), is still 0 × 0 at (10, 10) after its parent is resized; so is one of size
    20 × 20. Before any capture the child's references are the 50 × 50 creation rectangle.
41. **Toggle types (§13.12.1):** with action data in which the left select action has
    toggle type 0 (absent), 3, 4 or 5, one start and one end over control C give
    `MouseDown`, `MouseUp`, `MouseClick`, exactly as with type 1; with type 2 they give none
    of the four.
42. **Hover timing (§13.12.5):** with the pointer resting on C since frame-clock time t and
    no capture, no `MouseHover` is delivered in a tick at time t + 0.25 exactly, and one is
    delivered in the first tick after it; ending a capture at time u restarts the delay from
    u.
43. **Text-box keys (§13.13):** a focused plugin text box with `GetWantsKeyEvents()` false
    receives `KeyDown` and `KeyUp` for an action that its native handling does not consume;
    with it true, it receives each of them twice. Action 0xAB produces no `KeyDown` or
    `KeyUp` on any control.
44. **Game-styled build (§14.2.1, synthetic layout):** a test layout registered under the
    `UILAYOUT` entry key 0x100000C8 holds element 0x10002294 (300 × 300, properties
    `UICore_Element_min_width` 150 and `UICore_Element_min_height` 100) with one child at
    (279, 21), 16 × 16, edge modes (4, 5, 5, 4). A new `Turbine.UI.Lotro.Window()` reports
    size 50 × 50 and `GetMinimumWidth()` 150; after `SetSize(400, 300)` the child is at
    (379, 21), 16 × 16; `SetSize(100, 50)` gives 150 × 100. With the entry key missing, the
    window is still created (Tessera fallback) and a diagnostic is logged once.
45. **Title layout (§14.2.3):** with the worked example's widths, a window 400 × 300 and a
    caption measuring 96 pixels has its title text at left 140, width 120, and its title
    bar and drag bar at left 100, width 200; after `SetSize(200, 300)` the title text is at
    70, width 60, and the title bar at 30, width 140. Inside the window's `SizeChanged`
    handler for that resize the title parts still have their old geometry (the title
    layout runs after the handlers).
46. **Close button (§14.2.3, §13.3):** a `Button_click` message from the close button of a
    visible game-styled window fires `Closing`, then `Closed`, and the window is hidden; a
    handler that sets `Cancel` keeps it visible and fires no `Closed`; on a hidden window
    `Close()` fires nothing.
47. **Resizable (§14.2.3):** a new game-styled window has `IsResizable()` false and all eight
    resize bars hidden; `SetResizable(true)` shows them; `SetResizable(true)` again changes
    nothing.
48. **Background and back colour (§13.17):** the three numbered results of the §13.17
    worked example hold byte for byte. A new control reports `GetBlendMode()` 2 and
    `GetBackColorBlendMode()` 2; `SetBlendMode(9)` and `SetBackColorBlendMode(9)` raise
    errors; `SetBackColorBlendMode(2)` on a control that never had a back colour turns its
    rectangle of the window canvas into (0, 0, 0, 0), while `SetBackColorBlendMode(4)`
    leaves it unchanged; `SetBackColor(nil)` removes the fill.
49. **Colour packing (§13.17):** `Color(1, 0.999, 0.5, 0)` (A, R, G, B) packs to A 255,
    R 254, G 127, B 0; `Color(1, 1.5, 0, 0)` packs to 0xFF000000 OR (382 shifted left by 16
    bits) = 0xFF7E0000 (the overflow sets A's lowest bit, already set).
50. **Stretch mode and opacity (§13.18, §13.19):** a new control reports `GetStretchMode()`
    0 and `GetOpacity()` 1; `SetStretchMode(5)` makes `GetStretchMode()` return 5.
    `SetOpacity(0.5)` on a child with stretch mode 0 leaves every pixel of its window
    unchanged; on the window it halves the window surface's contribution when composed;
    on the child after `SetStretchMode(3)` it fades only that child's rectangle.
51. **Label defaults (§13.4):** a new `Label` reports `GetFont()` 0, `GetTextAlignment()` 1
    and `GetForeColor()` with A 1, R 0, G 0, B 0. `SetFont(0x41000001)` (not a font) leaves
    `GetFont()` 0 without an error. `SetTextAlignment(5)` sets horizontal and vertical
    justification 3 and 3; `SetTextAlignment(10)` makes `GetTextAlignment()` return 0 and
    sets 5 and 2. A new game-styled button reports `GetTextAlignment()` 1 although its
    layout centres the text.
52. **Selection (§13.4):** in a label with text `abcdef` (selectable), `SetSelection(2, 3)`
    gives `GetSelectedText()` `bcd`, `GetSelectionStart()` 2 and `GetSelectionLength()` 3;
    `SetSelection(9, 1)` selects `f` (start 6); `SetSelection(0, 2)` selects `ab`;
    `SetSelectionLength(0)` leaves nothing selected; with nothing selected,
    `SetSelectionStart(3)` selects nothing.
53. **Rotation (§13.20, §13.19).** All windows are plain `Turbine.UI.Window`s without a
    parent window.
    (a) *Shown window.* A window whose stretch mode was never set, after
    `SetVisible(true)` and then `SetRotation{x = 0, y = 0, z = 450}`: `GetRotation().z` is
    450, the window is drawn turned by 90 degrees about its centre, and `GetStretchMode()`
    is still **0**. After `SetVisible(false)` and `SetVisible(true)` it is still drawn
    turned by 90 degrees.
    (b) *Never-shown window.* A new window whose stretch mode was never set, after
    `SetRotation{x = 0, y = 0, z = 450}` while still hidden: `GetStretchMode()` is **1**;
    under Tessera's rule for the §13.20 step 3 OPEN, `GetRotation()` returns the same
    values as before the call; after `SetVisible(true)` the window is drawn **unrotated**
    (and is not re-laid out when resized, mode 1). A second `SetRotation{x = 0, y = 0,
    z = 450}` after `SetVisible(true)` turns it by 90 degrees and leaves `GetStretchMode()`
    at 1.
    (c) *Plugin stretch mode.* A hidden window with `SetStretchMode(3)` keeps
    `GetStretchMode()` 3 after `SetRotation`; a shown window with `SetStretchMode(0)`
    reports 1 after `SetRotation` and is drawn turned.
54. **Scale (§13.21):** the worked example of §13.21 holds; `GetScale()` of a window never
    marked is 1; `SetScale(0)` raises an error; on a window with a parent `SetScale(2)`
    changes nothing and `GetScale()` stays 1; `SetUIScalingGroup(0)` then
    `GetUIScalingGroup()` gives 72; on a plain window `SetUIScalingGroup(60)` gives 72, on a
    game-styled window 60; after `SetUIScalingGroup(5)`, `SetScale(1.2)` makes
    `GetUIScalingGroup()` 72.
55. **`WriteLine` markup (§11.1):** `Shell.WriteLine("<rgb=#FF0000>x</rgb>")` shows one
    chat line with the single character `x` in red and no visible tag;
    `Shell.WriteLine("<rgb=FF0000>x</rgb>")` (no `#`) shows the tag text literally.
56. **Class-table writes (§8.6):** after `Turbine.UI.Control.SetBackground = f`, a
    `Turbine.UI.Label()` instance created before or after the write calls `f` from
    `l:SetBackground(…)`; after also `Turbine.UI.Label.SetBackground = g`, labels call `g`
    and plain controls still call `f`; `l:IsA(Turbine.UI.Control)` stays true.
57. **Blend mode across a rebuild (§14.2.1 step 3):** for each row of the operation-number
    table, a window given `SetBlendMode(n)` (no call for the first row) and then
    `SetScale(1.5)` reports the row's "after one rebuild" `GetBlendMode()` value and draws
    its background with the row's operation; a new window without `SetBlendMode` reports 2
    before `SetScale` and 0 after it. A game-styled vertical scroll bar given
    `SetBlendMode(8)` reports 7, 5 and 1 after one, two and three orientation changes.
58. **Surfaces (§13.19):** a new window has no surface; after `SetVisible(true)` it has one
    (CanvasResize, as it has no object mode); after `SetVisible(false)` it has none; a
    child control with `SetStretchMode(1)` has a surface exactly while its own
    `IsVisible()` is true; a shown child with stretch mode 0 has none.

Invariants 59–65 check Tessera rules taken from decisions 0014 and 0015 (P79, P80), not
facts about the original.

59. **Nested opacity (§13.18, §13.19; decision 0015):** a shown window at opacity 0 with a
    child that has `SetStretchMode(1)` and an opaque background: the child's rectangle shows
    the child's background at full strength, and no other pixel of the window is visible.
    With the window at 0.5 and the child at 1, the child's pixels are composed at alpha 1
    times its own canvas alpha.
60. **Clip chain (§13.18; decision 0015):** a stretch-mode control inside a child window,
    extending past the child window, is cut at the child window's edge, not at the outer
    window's; with the child window itself extending past its parent window, both the child
    window and its control are cut at the parent's edge; the same holds when the child
    window has stretch mode 0.
61. **Deferred `Received` (§11.3; decision 0014 D8):** a chat line taken in during a frame
    reaches `Turbine.Chat.Received` in that frame's step 7 (or a later frame's when the
    deferred work's 2 ms are used up), after that frame's `Update` broadcast, and never
    inside the call that produced it. A plugin whose `Received` handler calls
    `Shell.WriteLine` for every line it receives gets no `Received` for its own lines, so it
    produces exactly one `WriteLine` per line from another source; a second apartment
    receives each of those lines once (skip set: the interim rule of the §11.3 OPEN on
    re-entrancy).
62. **Plugin code placement (§3.3; decision 0014 D8):** with a probe that records the
    frame-loop step whenever a host-to-Lua call starts, no plugin call starts in steps 2–6
    or 8 onward during a run that includes a login with autoloaded plugins (their top-level
    code and `Load` handlers run in step 7), a `/plugins load`, chat lines, gameplay value
    changes, input and a logout, as long as no game or session code shows, hides, activates
    or resizes UI elements outside steps 1 and 7 (that case is the §3.3 OPEN on notification events); at
    client exit the `Unload` handlers run after the last frame, and a `PluginData.Save` made
    in them is on disk before the process ends.
63. **Coalesced movement and current geometry (§13.12.3; decision 0014 D7, D8):** a control
    captured by a left press, whose `MouseMove` handler calls `SetPosition` to follow the
    pointer, gets for two coalesced movements in one frame (one delivered before a key
    event, one at the frame's input step) exactly two `MouseMove` events, with `X`/`Y`
    relative to its position after the first, and no `MouseLeave` or `MouseEnter`; a burst
    of 50 pointer movements followed by a button press, all arriving within one frame (no
    redraw between them), delivers one `MouseMove`, at the last position, before the
    `MouseDown`.
64. **Mouse buttons during the lock (§13.12.2; decision 0014 D7):** left pressed on control
    C (C captures), right pressed and not consumed (the lock turns on), left released: C
    gets `MouseUp` and no `MouseClick`, and C no longer captures. A right press during an
    item drag never turns the lock on.
65. **Two echoing apartments (§11.3 OPEN on re-entrancy; decision 0014 open review item 1,
    interim Tessera rule):** apartments A and B each have a `Received` handler that calls
    `Shell.WriteLine` once for every line it receives. One line L0 from the session gives
    exactly four plugin-written lines: L1 by A and L2 by B, each in reply to L0; L1 reaches
    only B and L2 only A; then L3 by B in reply to L1 and L4 by A in reply to L2, which
    reach no apartment. After that no further `Received` event is queued, so the pair does
    not use the deferred work of later frames. (Skip sets: L0 empty; L1 {A}; L2 {B}; L3 and
    L4 {A, B}.)

Invariants 66–69 check §15.1 against a scenario feed (`scenario-feed.md`), not against the
original: the API facts P81 and P82 (1-based backpack indices, `Equipment` value = data key
− 1) and the Tessera rules P83 (event derivation from the feed, `Equipment:GetSize()`, the
`SizeChanged` rule, the values for absent properties and literal items).

66. **Backpack view (§15.1; decision 0024 P1 criterion 4):** a scenario that sets the
    backpack to 30 slots and gives three item records at slots 0, 4 and 29 yields
    `GetSize() == 30`, `GetItem(1)`, `GetItem(5)`, `GetItem(30)` non-nil with the records'
    names, `GetItem(2)`, `GetItem(0)`, `GetItem(31)` and `GetItem(1.5)` nil; a plugin's
    `ItemAdded` handler saw indices 1, 5 and 30 in that order. A scenario without a `bag`
    step that gives one item at slot *B* + 5, where *B* is the install's
    `InventoryControl_BaseBackpackSlots` (`character-panels.md` §4.3, read at run time),
    delivers `SizeChanged` (after which `GetSize() == B + 6`) before that item's `ItemAdded`
    with `Index` *B* + 6.
67. **Backpack events:** with `defaults on` (`scenario-feed.md` §7.6), a `MoveItem` input
    line moving the slot-4 item to backpack slot 10 gives one `ItemMoved` with `OldIndex` 5,
    `NewIndex` 11; a `DestroyItem` input line for it then gives `ItemRemoved` with `Index`
    11; a `bag 40` step gives `SizeChanged` and then `GetSize() == 40`; a second `bag 40`
    step gives no event.
68. **Equipment view:** equipping an item record at data key 3 (`Chest`) gives
    `ItemEquipped` with `Index` 2 (`Turbine.Gameplay.Equipment.Chest`),
    `GetItem(Turbine.Gameplay.Equipment.Chest)` returns it, `GetItem(1)` is nil,
    `GetSize() == 21`; unequipping gives `ItemUnequipped` with `Index` 2 and a backpack
    `ItemAdded`.
69. **Item details:** for an item record with `Item_Quality` 4 and `Inventory_MaxQuantity`
    50, `GetItemInfo():GetQuality() == 4` (`Turbine.Gameplay.ItemQuality.Uncommon`),
    `GetItemInfo():GetMaxStackSize() == 50`, `GetMaxStackSize() == 50` and
    `GetItemInfo():GetMaxQuantity() == 50`; `GetItemInfo()` called twice returns the same object. For an
    item record without `Item_DurabilityEnum`, `Item_WearState` and `Inventory_Unique`,
    `GetDurability()` and `GetWearState()` are 0 and `GetItemInfo():IsUnique()` is false. A
    literal item's `GetName()` and `GetItemInfo():GetName()` are its literal text,
    `GetItemInfo():GetQuality()`, `GetCategory()` and every image getter are 0,
    `GetMaxStackSize()` is 1, and `GetItemInfo():GetNameWithQuantity(3)` is "3", a space and
    the literal text.


Checks 90–96 test §15.4 against a scenario feed: the API facts P90–P91 and the Tessera rules
P93 (numbers 70–89 are left to the P2 and P3 revisions).

90. **Party presence (§15.4):** without a `fellowship` step `GetParty()` is nil. After
    `fellowship "Tam" $a leader="Tam"` (player Tam, actor `$a` named Brin) one `PartyChanged`
    with an empty table was delivered; `GetMemberCount() == 2`, `GetMember(1) ==
    LocalPlayer.GetInstance()`, `GetMember(2):GetName() == "Brin"`; `GetMember(0)`,
    `GetMember(3)`, `GetMember(1.5)` and `GetMember("1")` are nil; `GetLeader() ==
    LocalPlayer.GetInstance()`; `IsMember(LocalPlayer.GetInstance())` is true and
    `IsMember(nil)` false.
91. **Member added:** a following `fellowship "Tam" $a $b leader="Tam"` (`$b` named Cael)
    delivers one `MemberAdded` whose `Player:GetName() == "Cael"` and `Player ==
    GetMember(3)`, and no `PartyChanged`; `GetMemberCount() == 3`.
92. **Leader changed:** `fellowship "Tam" $a $b leader=$a` delivers one `LeaderChanged` whose
    table is empty (`next(args) == nil`); `GetLeader():GetName() == "Brin"`.
93. **Member removed:** `fellowship "Tam" $a leader=$a` delivers one `MemberRemoved` with
    `Player:GetName() == "Cael"`; `IsMember` of that object is false and its `GetParty()` nil;
    `GetMemberCount() == 2`.
94. **Disbanded:** `fellowship none` delivers one `PartyChanged` and no `MemberRemoved`;
    `GetParty()` is nil; a later `fellowship "Tam" $b leader="Tam"` gives a `Party` object
    different from the first.
95. **Player appended:** `fellowship $a $b leader=$a` (the player not listed) gives
    `GetMemberCount() == 3` and `GetMember(3) == LocalPlayer.GetInstance()`.
96. **Assist targets and no-ops:** in any party `GetAssistTargetCount() == 0`,
    `GetAssistTarget(1)` is nil and `IsAssistTarget(GetMember(2))` false; repeating the last
    `fellowship` step unchanged delivers no event.

---

## 19. Open questions (summary)

All `> **OPEN:**` callouts above:

- Sandbox and scheduling: whether any path sets an apartment memory limit (§3.3); order of
  the plugin host's deferred work relative to the UI tick (§3.3); how Tessera keeps
  notification events caused by game- or session-driven UI changes inside frame-loop steps 1
  and 7 (§3.3, decision 0014 open review item 2); when the frame clock behind
  `GetGameTime()` and the hover delay advances (§3.3, decision 0014 "Other improvements").
- Names: C-runtime class of package-name bytes 0x80–0xFF and of key units above 0xFF (§7.2).
- Lifecycle: autoload order (§6.2); blocking `PluginData` writes and flushes when an unload
  closes a state inside a frame (§6.3, ARCHITECTURE §21); localized aliases of `/plugins` (§6.4); whether error
  lines reach `Chat.Received` (§6.6).
- Events: when an `Update` subscriber
  that turned updates off is dropped (§9.4); whether `SetText`, `SetChecked`,
  `SetSelectedIndex`/`SetSelectedItem` and `SetSelectedNode` fire their change events
  (§9.6).
- Plugin data: whether a write queued before its apartment closed is still performed
  (§10.6).
- Shell and chat: `WriteLine` lines and `Chat.Received` (§11.1); the client command-line
  parser's whitespace, quote and case rules for `Execute` arguments (§11.2); markup in chat
  `Message` (§11.3); when the original delivers `Chat.Received` (§11.3); the skip set that
  keeps queued `Received` from re-entering an apartment, pending a revision of decision 0014
  (§11.3, its open review item 1).
- UI: whether creation applies the inherited size limits of game-styled windows (§13.1);
  Escape (§13.3); the drawn colour of a text element without a colour property, and
  clamping of a selection end past the text (§13.4); which buttons and keys make a native
  button report "activated", and disabled buttons (§13.6); `Display.GetUIGroupScale`
  semantics (§13.10);
  special point tests of client panels and which client states lock the pointer (§13.11); double-click acceptance of every control
  class, the order of two actions from one physical event, the start/end of `Tap`
  activations and the start/end sequences of mouse-button actions of toggle types 3–5
  (§13.12.2); per-tick repetition of `MouseHover` (§13.12.5); which plugin controls make
  the scroll actions active (§13.12.6); keys consumed by
  focused text boxes and character input (§13.13); `ZOrder` of client panels and equal-ZOrder
  order (§13.14); non-activatable client panels, activation on show and on hide (§13.14);
  negative or zero sizes (§13.15); layout of top-level windows and re-capture on a parent
  move (§13.16); composition of own surfaces, mode 2 drawing, mode 4 and the extra flag of
  mode 1 (§13.18); the clip chain of controls inside mode-0 child windows, pending a
  revision of decision 0015 (§13.18, its open review item 1); window opacity on nested surfaces and range clamping (§13.19); direction,
  order and projection of rotations, hit testing of rotated windows and whether a rotation of a window that has no surface (hidden) reaches an ancestor's, the screen root's, surface (§13.20); whether
  a scaled window's children follow their edge modes or are not laid out again (§13.21
  rule 1 against `ui-behaviour.md` §9.2 step 6), initial
  global state of the game-styled scaler and which windows receive `UIScaleChanged`
  (§13.21); the scaling-service flag that picks the `GoldWindow` variant (§14.2.1); read
  order of plugin-created elements against the drag bar (§14.2.3); interaction details of
  the game-object controls (§14.5).
- Lotro and gameplay: item shortcut companion value and `SetData` parsing (§14.6); which of
  `Alignment`, `Profession`, `Vocation`, `ReadyState`, `SkillType`, `EffectCategory` and the
  stance enums follow a data enum (§15); when the original delivers the gameplay views'
  `…Changed` events (§15); what the original's `ItemInfo:IsMagic()`, `GetMaxQuantity()`,
  `GetQualityImageID()` and `GetNameWithQuantity()` return, what its item getters return
  when the data lacks the property, and whether `ItemAdded`/`ItemRemoved` fire for moves
  between containers (§15.1); the original's party member order, member events at
  formation and `PartyChanged` on membership changes, `PartyMember` object identity against
  `Actor` objects, and `PartyMember` values for members out of view (§15.4); the implementer
  cases of §15.4 (an ended `Party`, `PartyMember` actor values before §15.2 merges, repeated
  names, lists over 64 entries, `PartyMember:GetRaid()` against §16, gameplay getters
  called with a `self` without a native object).

The 2026-10-06 revision closed: the chat colour and prefix of reported errors (§6.6),
`AssistTargetRemoved` arguments (§9.3), `Update` on hidden controls (§9.4), the general
question of which setters fire change events (§9.6, now narrowed to four calls), callbacks
of writes after an apartment closed (§10.6, narrowed), edge-attachment order and rules
(§13.2, §13.16), and the implementer question on finalizers (§3.3).

The third 2026-10-06 revision closed: the initial size of the skinned controls (§13.1,
narrowed to whether creation applies the inherited limits), the opacity of child and
non-window controls (§13.1, §13.19), rotation units and pivot (§13.3, §13.20; direction and
order remain), scaling units, origin point and combination with groups (§13.3, §13.21),
label selection base and defaults (§13.4; the drawn default colour remains), stretch modes
(§13.8, §13.18; composition and modes 2 and 4 remain), skinned frame metrics and parts
(§14.2), `WriteLine` markup (§11.1, usage O9) and class-table replacement (§8.6, usage O2).

The decision-alignment revision of 2026-10-06 closed no callout. It rewrote the Tessera
parts of the callouts in §3.3, §6.6, §11.1, §13.11, §13.12.2, §13.13, §13.18, §13.19 and
§13.20 to follow decisions 0014 and 0015, and added five: notification events from game- or
session-driven UI changes (§3.3), when the original delivers `Chat.Received` (§11.3) and
the gameplay `…Changed` events (§15), which plugin controls make the scroll actions active
(§13.12.6), and §13.21 rule 1 against `ui-behaviour.md` §9.2 step 6. Its repair after
review added four more, each recording an interim Tessera rule where a decision record is
silent or not yet revised: when the frame clock steps (§3.3), blocking writes at unload
(§6.3), the `Received` skip set (§11.3, moved out of the body text) and the clip chain of
mode-0 child windows (§13.18, likewise).

The clean-room implementation of `tessera-ui` (2026-10-07) added two callouts: the order of
`Click` and `MouseClick` on a plugin button (§13.6) and what `GetSize()` reports for a
control in stretch mode 1 after `SetSize` (§13.18).

The P1 revision of 2026-10-09 (§15.1) and its review fixes of the same day narrowed the §15
callout on enums (`Equipment`, `ItemDurability` and `ItemWearState` answered by P82) and
added one callout (§15.1: the original's values of four `ItemInfo` getters, of getters on
absent properties, and events for moves between containers).

The P4 revision of 2026-10-11 (§15.4, provenance P90–P93, §18 checks 90–96) answered the §9.3
callout on the fields of `Party.MemberAdded`/`MemberRemoved` (P90, which also corrects the
P31 reading of `AssistTargetRemoved`) and added one callout (§15.4); the clean-room
implementation of the same day added a second (§15.4: the cases the tables leave open).

---

## 20. Corrections to other specs

These rest on the disassembly of client build 4906.0071.1794.4035 (BIN) and on O4 (the
`character-panels.md` row on Tessera rules, DEC). They are
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
| `skin-definition.md` §6.8 (note on plugin game-styled controls) | "a top-level panel override whose ID is one of these labels also changes the design rectangle of every plugin control built from it" | The `luaplugins` element is the **base** of the control's descriptor; the control's own 0, 0, 50 × 50 rectangle replaces the base's (`ui-layout.md` §7.2 step 1) and its top element carries a run-time ID, so an override never changes the control's size or position; at most it changes the base size from which the inherited children are re-laid out, and whether it applies at all was not traced (§14.2.1 step 4 here). | BIN (fourth pass) |
| `ui-layout.md` §5.9 (`UICore_Element_ObjectMode`) and §8.4 | "default CanvasResize"; Tile and CanvasResize OPEN | Precision: the property has no dictionary default (O7); when the client creates an element's own surface and the element has no object mode, it uses CanvasResize. A Tile surface takes the size of the element's base image (or a code-set size) and falls back to CanvasResize unless both sides are powers of two. Any non-zero object mode gives the element its own surface; 0 removes it. The plugin API's `SetStretchMode` writes this property (§13.18 here). | BIN (fourth pass) |
| `ui-layout.md` §5.9 (`UICore_Element_ObjectOpacity`) | "Opacity of the element's own drawing surface (root panels and elements that code gives a surface)" | Addition: the plugin API's `SetOpacity` writes this property; every plugin window is a root panel with a surface, other plugin controls have one only while their stretch mode is not 0; dictionary default 1.0, range 0–1 (§13.19 here, O7). | BIN (fourth pass), O7 |
| `fonts.md` §9.4 | "The default font of a new plugin label is an OPEN of `lua-plugin-api.md` §13.4" | Answered: `GetFont()` is 0 until `SetFont`; the text is drawn in the text element's font, which for plain labels is the locale default font of `fonts.md` §9.1 and for game-styled controls the font of their layout element (§13.4 here). | BIN (fourth pass) |
| `ui-behaviour.md` §9.2 OPEN ("the plugin-side combination of `Window:SetScale` with the group scale") | OPEN | Answered in §13.21 here: `SetScale(f)` sets the window's own scale, which replaces its group's value; the effective scale is `f` × `UIScale_Global` for windows that take part in the global scale, else `f`, and it is applied by the §9.2 rule. | BIN (fourth pass) |
| `character-panels.md` §8.1 (Tessera mapping) | `ItemAdded` per `ItemAdded` into `Backpack`; `SizeChanged` per `ContainerSize` of `Backpack` | Align with §15.1 here (P83): `ItemAdded` also per `ItemMoved` from another container into the backpack; `SizeChanged` whenever *N* changes, including growth through an arriving item before the first `ContainerSize` (queued before that item's `ItemAdded`), and not for a `ContainerSize` that leaves *N* unchanged. Added 2026-10-09. | DEC (0024 P1) |
| `plugin-api-usage.md` §6.3 (OPEN, O2) and §6.9 (OPEN, O9) | Whether class-table replacements reach instances; whether `WriteLine` renders markup | Both answered yes: §8.6 and §11.1 here (P76, P75). | BIN (fourth pass), O7 |
| `scenario-feed.md` §6.4 (`fellowship` step) | A `fellowship` list need not name the player | Note, not a correction: the plugin API needs the player as a member (P91), so §15.4 reads a list without the player as if the player were appended, as `social-panels.md` §9 TR2 does; scenario authors should list the player | P91, DEC |

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

Meaning of each value per edge: §13.16.

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
| `LoadPlugin`, `UnloadScriptState`, `RefreshAvailablePlugins` queued until after the current script call | `LoadPlugin` is queued (§6.2). `UnloadScriptState` is immediate but refused for an apartment with running Lua code (§6.3); queuing it was accepted earlier, but decision 0014 D8 now makes Tessera run it where it is asked for, in step 1 or 7 (§6.3), with a self-unload refused. `RefreshAvailablePlugins` reruns discovery (§6.7); immediate or deferred is not observable to plugins except through `GetAvailablePlugins`. |
| `Shell.WriteLine` echo to `Chat.Received` with `Sender = ""`, on the next pump | Original posts the line as chat type 8; delivery to `Received` is OPEN (§11.1). Tessera: deliver with `ChatType = 8` and **no** `Sender` field (system lines have none, §9.3), never re-entrantly. Timing (revision of 2026-10-06, decision 0014 D8): not "on the next pump" but as a queued item of the plugin host's deferred work in frame-loop step 7, skipping the apartments of the line's skip set (the interim Tessera rule of the §11.3 OPEN on re-entrancy, pending a revision of decision 0014; §11.1, §11.3). |
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
| 2026-10-06 | Second OPEN-closing pass (spec writer, dirty room; BIN second pass on the same build, plus O5 addendum and new O6); status Accepted → Review (becomes Revised after re-review). **New sections:** §9.7 (notification delivery and FIFO queueing, broadcasts, order), §13.11 (hit test, pointer and hover targets), §13.12 (mouse-button actions and the `Button` table, capture, `MouseDown`/`MouseUp`/`MouseClick`/`MouseDoubleClick`, `MouseMove`, `MouseEnter`/`MouseLeave` chains, `MouseHover` timing with the 0.25 s delay, `MouseWheel` target), §13.13 (key routing, `KeyDown`/`KeyUp` broadcast independent of focus, keyboard focus), §13.14 (z-order, window activation by `Activate()` and by a mouse press), §13.15 (size limits), §13.16 (edge attachments: argument order, reference capture, per-type layout rules, worked examples). **Changed:** §3.3 (UI tick order; implementer question on finalizers answered: accepted with one condition), §6.6 (errors and warnings go to chat as chat type 1 with no prefix and to a log channel; memory-error status not reported by the protected call), §9.3 (`Button` 0 for other actions; `AssistTargetRemoved` empty per S3), §9.4 (`Update` is a broadcast: hidden and unparented controls receive it; subscription order; worked example), §9.6 (setter/change-event table, `EnabledChanged` direct, `ScrollBar:SetValue` unsigned clamping), §10.4 (no final line end; byte-exact examples), §10.6 (write step; callbacks only into a live apartment), §11.2 (plugin layer passes command and arguments unchanged), §13.1–§13.3, §13.6, §13.7 (pointers to the new sections; `Button.Click` is not `MouseClick`), Appendix A.6. **Provenance:** BIN key extended; rows P47–P63, O6 added. **Invariants** 26–38 added. **OPEN callouts:** 35 before §19 (26 before this pass including the implementer's finalizer question; closed or narrowed: error colour and prefix, `AssistTargetRemoved`, `Update` on hidden controls, the general setter question, post-close callbacks, edge attachments, finalizers; new precise ones on the UI tick order, `Update` entry removal, the client command-line parser, the error line and `Chat.Received`, button activation, double-click acceptance, two-action order and `Tap` start/end, `MouseHover` repetition, text-box key consumption, client-panel `ZOrder` and activation, activation on show/hide, empty sizes, layout of top-level windows, special point tests). **Implementers re-check:** event delivery timing in `crates/plugins`/`crates/ui` (§9.7), `Update` delivery to hidden controls (§9.4), the mouse, key, focus and activation routing (§13.11–§13.14), size limits (§13.15), and the finalizer condition (§3.3). |
| 2026-10-06 | Second revision of the day (spec writer, dirty room; BIN third pass on the same build, O6 re-read), answering the review of 2026-10-06; section numbers unchanged, one anchor added (`9-6-1`). (1) §9.6: `ScrollBar:SetValue` clamps against the maximum first, then the minimum (unsigned); new "scroll-bar numbers" paragraph: defaults 0/100/0/10/1, 64-bit truncation keeping the low 32 bits, unsigned results of the five getters, `SetMinimum`/`SetMaximum` drag the other bound and re-clamp the value (so minimum above maximum cannot be reached; if it were, `SetValue` would give the minimum), large/small change caps, thumb fractions, worked example; §13.7 points to it; invariant 29 extended. (2) §13.1: every plugin control is created at 0, 0 with size 50 × 50 (new P64); §13.16: initial references (R = creation rectangle, no Q, the parent's own reference used instead), captures of windows marked for global scaling, and a correction: plugin controls carry the creation mark that makes None edges always take the current coordinate, so the 0 × 0 exception never applies to them and a control without attachments never moves with its parent. (3) §13.12.5: the hover check also requires the manager's pointer-outside flag to be clear (never set in this build) and a hover target; the delay comparison is strict; the delay restarts at the end of a capture; the tooltip is hidden first. §13.11: the pointer-outside flag and the input layer's pointer lock suppress the hit test. (4) §13.13: the text-box key listener is now sourced (P48 extended): it fires on any key notification whose source is the text box, for `TextBox` and the skinned text box; consequence: double delivery when the text box also subscribes; action 0xAB is ignored by the UI (O6, P48). (5) §13.12.1/§13.12.2: the UI only tests for toggle type 2 in the action map of the context in force; types 0, 1, 3, 4, 5 all follow the Momentary rules; consequences for repeated starts and latches; OPEN on the input layer's sequences. **Provenance:** BIN key extended with the third session; P48–P51, P57, P58, O6 extended; P64 added. **Invariants** 39–43 added. **OPEN callouts:** 38 before §19 (new: initial size of skinned controls §13.1, what locks the pointer §13.11, start/end sequences of type 3–5 mouse-button actions §13.12.2). **Implementers re-check:** scroll-bar arithmetic and getters (§9.6), initial control size (§13.1), the None rule and initial references of the edge layout (§13.16), the hover conditions (§13.12.5), toggle-type handling of mouse-button actions (§13.12.1) and text-box key events and action 0xAB (§13.13). |
| 2026-10-06 | Third revision of the day (spec writer, dirty room; BIN fourth pass on the same build and file hash, new O7 data observations), answering the M5 completeness critique items A4/C2 (game-styled controls), A5/C3 (drawing, opacity, rotation, scale, label defaults, `WriteLine` markup, class-table writes) and B8 (font-record field); status Accepted → **Review** (becomes Revised after re-review). Section numbers 1–20 kept; **new sections and anchors:** §8.6, §13.17 (drawing backgrounds and back colours, with a byte-level worked example), §13.18 (stretch modes and own surfaces), §13.19 (opacity), §13.20 (window rotation), §13.21 (window scale and scaling groups, worked example), §14.2.1–§14.2.6 (how the game-styled controls are built from the `luaplugins` layout; text targets; window parts, title layout with worked example, close button, resize bars; layout invariants; neutral art). **Changed:** naming note (cites `LEGAL.md` §17; layout and dictionary labels are data labels), related specs, purpose; §11.1 `WriteLine` renders markup; §13.1 initial geometry of game-styled controls (layout element is the base; inherited size limits), transparency, opacity; §13.2 look row; §13.3 rotation, scaling, `Close()` only when visible; §13.4 text targets, defaults, alignment table, 1-based selection, `b`/`i` without effect; §13.8 points to §13.17/§13.18 (the community stretch-mode table moved there as Reported, with the disagreements marked); §14.1 offset 4 is the **line height in pixels** (B8; P12, invariant 23 corrected). **Provenance:** BIN key extended with the fourth session; O7 added; P12, P64 corrected; P65–P76 added. **Invariants** 44–56 added. **Corrections to other specs (§20):** `skin-definition.md` §6.8, `ui-layout.md` §5.9 (two rows), `fonts.md` §9.4, `ui-behaviour.md` §9.2, `plugin-api-usage.md` §6.3/§6.9. **OPEN callouts:** 42 before §19 (38 before; closed: initial size of skinned controls (narrowed to one new callout), opacity of children, rotation units and pivot, scaling units and combination, label defaults and selection base (replaced by two narrow callouts), stretch modes, skinned frame metrics; new: own-surface composition and modes 2/4, opacity on nested surfaces, rotation direction and order, rotation of a window without a surface, global state of the game-styled scaler, `UIScaleChanged` recipients, the `GoldWindow` variant flag, read order of plugin elements against the drag bar). **Implementers re-check:** control creation for the `Turbine.UI.Lotro` classes (§14.2.1–§14.2.3), the background/back-colour/blend-mode drawing and defaults (§13.17), stretch mode and opacity (§13.18, §13.19), `Close()` on hidden windows (§13.3), label defaults, alignment and selection (§13.4), rotation and scale (§13.20, §13.21), `WriteLine` markup (§11.1), and any code that read the font record's offset 4 as a point size (§14.1). |
| 2026-10-06 | Repair of the third revision (spec writer, dirty room; BIN repair reading on the same build and file hash, O7 re-checked), answering the review of the third revision; status stays **Review** until re-review; section numbers unchanged, one anchor added (`13-19-surfaces`). (1) §14.2.1 step 3: the full table of base-layer operation numbers (Replace 0, Mix colour 1, Over 2, Colorize 3, Multiply 4, Screen 5, Grayscale 6, "none" 7) with the blend mode and operation that one rebuild produces for every `SetBlendMode` value; a control that never set a blend mode reports 0 after a rebuild; the mapping repeats at every scroll-bar rebuild (chains end at Replace except Grayscale); worked example; §13.17 points to the table. (2) §13.19 new paragraph "Which elements have a surface": an element wants a surface when its object mode is non-zero or, without one, when it is a root panel (every window); the surface exists only while it is wanted and the element's own visibility flag is set (or `UICore_Element_KeepObjectWhenHidden`, which no plugin element has); so a new window has no surface until it is first shown. This replaces "windows always have a surface" in §13.19 and §13.17, and §13.18 points to it. §13.20 steps 2–3 rewritten on that rule: the object-mode change happens only when the window has no surface at the call (hidden, or plugin mode 0), creates the surface at once only when shown, and a rotation set while hidden is never shown (it is not remembered; a rotation set while shown survives hide/show); the OPEN on ancestor surfaces is narrowed to the screen root. Invariant 53 split into (a) shown, (b) never shown and (c) plugin stretch mode; new invariants 57 (blend mode across a rebuild) and 58 (surface lifetime). (3) P72 cited an undefined source key: new source key S6 (the M5 API census of published plugins, results in `plugin-api-usage.md` §5.3 and §9.3). **Provenance:** BIN key extended with the repair reading; P65, P70 and P72 updated; P77 and P78 added. **OPEN callouts:** 42 before §19, unchanged (the §13.20 callout reworded, §19 updated). **Implementers re-check:** the blend mode applied after a rebuild (§14.2.1 step 3), the surface lifetime of windows and stretched controls (§13.19), and `SetRotation` on hidden windows (§13.20). |
| 2026-10-06 | Decision-alignment revision (spec writer, dirty room), applying the owner-Accepted decisions [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge) and [0015](../decisions/0015-ui-rendering.md) (UI rendering) to Tessera's own rules; no new research, **no fact about the original changed** (BIN, O and S statements are untouched; where a body sentence overstated a fact that is OPEN, it was narrowed to the known part). Status Accepted → **Revised, awaiting review**. Section numbers and anchors unchanged. **Changed Tessera rules:** §3.3 collector step "may rely on the default collector" → "must" (`plugin-runtime.md` §9.2, 0014 D8); "between frames" read as frame-loop steps 1 and 7 with the client-exit exception (0014 D8, owners list); the deferred-work list gains the game-raised events and the logout unload, sliced by whole items (2 ms, at least one item, order kept); the Tessera order of the §3.3 OPEN now names steps 1, 7 and 8. §6.2 queued `LoadPlugin` = step-7 deferred work. §6.3 when each unload runs (in the requesting call, logout in step 7, client exit after the last frame). §6.6 and §11.1 OPENs: error and `WriteLine` lines reach `Received` as queued step-7 events, "next pump" retired. §9.4 `Update` is never sliced; plugin timers are `Update` handlers. §9.7 hit tests read the current geometry. §10.6 asynchronous `Load` handlers and writes are step-7 items. §11.3 new paragraph: every chat line is a queued `Received` event delivered in step 7 (0014 Known difference 21), and the queued line records the apartments whose `Received` dispatch produced it so that §9.6's non-re-entrancy holds (the fix of 0014 open review item 1). §13.1 opacity bullet narrowed to children drawn into the window's surface. §13.2 surface size limits (0015 §7). §13.3 Escape note (0014 "Escape with a UI attached"). §13.8 Tessera checks of `SetBackground(path)`: whole-tree index, 64-byte header probe, `.png`/`.bmp` refused, late decode failure reported (0015 §4.3–§4.4). §13.11 the pointer lock is client state (0014 D7) and the search reads current geometry (0014 D8). §13.12.1 what the input layer delivers: command names and IDs, toggle types from table A of ACTIONMAP key 1, actions never offered, repeats per hold (0014 D1, D7, D9; Known differences 6, 7, 10, 12). §13.12.2 the two-action order follows 0014 D4 (replacing "the order of the bindings in the effective key map"); mouse buttons during the lock (0014 D7, Known difference 16); types 3–5 agree with D7. §13.12.3 coalesced movements and current-geometry `X`/`Y` (Known difference 15). §13.12.5 hover clock from the coalesced movement, tick in step 7. §13.12.6 scroll actions active over every hovered plugin control (0014 D5). §13.13 OPEN: text-box consumption, typed characters and the `opaque_keys` narrowing of step 2 (0014 D2, D6, D7, Known difference 18). §13.17 composition cross-reference. §13.18 OPEN Tessera rule replaced ("straight source-over, clipped to its window only" → 0015 Decision 1.6–1.8): separate entry, premultiplied filtering, canvas-above order, clip by the nearest enclosing window and every window above (spec question 12), own opacity, inherited rotation, mode 2 repeat addressing and Tile parents (spec question 11), mode-change refresh (spec question 15), caps. §13.19 OPEN Tessera rule "window opacity applies to nested surfaces" → **own opacity only** (0015 Decision 1.7, owner question 3, spec question 1); body sentences on fading "all its children" narrowed to children drawn into the surface; "Tessera draws every shown window through a surface" → stand-in surface for top-level mode-0 windows only, mode-0 child windows drawn into the nearest surface above (0015 Decision 1.1, spec question 13); first paint and the logical show (0015 Decision 1.8). §13.20 stand-ins are not rotation targets; composition reads surface rotation, non-finite angles are 0 (0015 §2.3, §7). §13.21 rule 1 against `ui-behaviour.md` §9.2 step 6 (spec question 2). Appendix B `WriteLine` row timing. **Provenance:** source key DEC and mark **Tessera rule**; rows P79 (0014) and P80 (0015). **Invariants** 59–64 added (Tessera rules). **OPEN callouts:** 47 before §19 (42 before; 5 new, listed in §19; none closed). **Implementers re-check:** plugin-host scheduling and the deferred-work queue (§3.3, §6.2, §6.3, §10.6), `Received` and `…Changed` delivery (§11.3, §15), input routing to plugin controls (§13.11–§13.13), the composition of plugin surfaces, opacity, clip and rotation (§13.18–§13.20), and `SetBackground(path)` checks (§13.8). **Not applied here (other owners):** ARCHITECTURE §18.1/§20.3/§21, `plugin-runtime.md` §8.3/§10.3, `ui-layout.md` §5.9/§6.6/§8.4, `ui-behaviour.md` §4.9/§7.4/§9.2.1, `keymap.md`, `game-panels.md`, `render.md`. |
| 2026-10-06 | Repair of the decision-alignment revision (spec writer, dirty room), answering its first review (changes requested); no new research, **no fact about the original changed**; status stays **Revised, awaiting review**; section numbers and anchors unchanged. (1) **Autoload placed in the frame:** the §3.3 deferred-work list now holds every plugin load: queued `LoadPlugin` requests, `/plugins load` and manager-UI loads, and the autoload loads, one item per autoloaded plugin, queued in Tessera's autoload order when the session reports the login (a Tessera reading of 0014 D8, as in `plugin-runtime.md` §8.4 and `plugin-descriptor.md` §8.1); no load runs before the first `Plugins/` index (0015 §4.4); §6.2 gains a "when each load runs" paragraph and its OPEN's Tessera text says the same; invariant 62 now includes autoload and `/plugins load`. (2) **First paint (§13.19):** the 100 ms bound is limited to windows and to nested surfaces shown while no generation runs; a nested surface shown during its group's generation is drawn at the next generation's swap (0015 Decision 1.5, 1.8, U4 (c): 8 or 41 frames); the events cited are the §9.6 setter table and §9.7, not "the events of §9.6". (3) **Pending decision revisions framed as such:** the `Received` skip rule moved from §11.3 body text into a new OPEN ("Tessera rule until decision 0014 is revised", as `plugin-runtime.md` §8.4), and it now also covers several echoing apartments: a line raised while a queued `Received` is delivered inherits that event's skip set plus the running apartment (new invariant 65; §6.6, §11.1, invariant 61 and Appendix B point to it); the window-built clip chain of §13.18 moved into a new OPEN ("Tessera rule until decision 0015 is revised", its open review item 1). (4) **P79** now cites 0014 D1–D9, "Escape with a UI attached", Known differences 6, 7, 10, 12, 15, 16, 18, 21, open review items 1 and 2, "Other improvements" and 0015 §4.4, and lists §6.2, §6.6, §9.4, §9.7, §10.6, §12 and §13.3; **P80** adds 0015 Decision 1.5, U4 (c) and open review item 1. (5) **New OPENs with interim Tessera rules:** blocking `PluginData` writes and flushes when an unload closes a state inside a frame (§6.3, cross-referenced from §10.6; ARCHITECTURE §21; same as `plugin-runtime.md` §8.4); when the frame clock behind `GetGameTime()` and the hover delay advances (§3.3: once per frame, right after the previous present and before any step-1 work, so every plugin call of one frame sees one value; §12 and §13.12.5 point to it). (6) **Invariant 63** says the 50-movement burst arrives within one frame. Appendix B `UnloadScriptState` row aligned with §6.3 (runs where asked, 0014 D8). **OPEN callouts:** 51 before §19 (47 before; 4 new, listed in §19; none closed). **Implementers re-check:** load scheduling, autoload included (§3.3, §6.2), the `Received` skip set (§11.3), the frame clock (§3.3), first-paint waits of nested surfaces (§13.19). **Left for other owners:** decision 0014 (skip set into D8 and Known difference 21; the frame clock; the ARCHITECTURE §21 exception) and decision 0015 (clip chain from windows) are locked records; `chat-window.md` §10 item 9 should point to the §11.3 OPEN here. |
| 2026-10-06 | Second repair of the decision-alignment revision (spec writer, dirty room), answering the second review round; no new research, **no fact about the original changed**; status stays **Revised, awaiting review**; section numbers and anchors unchanged. (1) **§3.3 deferred work made consistent:** the at-least-one-item rule of 0014 D8 now carries its one exception, from 0015 §4.4: while the first `Plugins/` file index is not ready and the head of the queue is a load, that frame's deferred work stops at the load, no item runs and the items behind it wait, so queue order is kept; refreshed indexes never hold back a load; this is the rule of `plugin-runtime.md` §8.4 "Wait for the first file index", which §3.3 now cites. (2) **§18:** the lead-in to the Tessera-rule invariants reads 59–65 (invariant 65 is the §11.3 interim rule). (3) **§13.18 mode-1 row** (non-blocking): the reason why the children of a resized Stretch control are not laid out again is now `ui-behaviour.md` §9.2.1 (a Stretch element resize keeps the layout box), not `ui-layout.md` §8.4 step 1, which Tessera applies only under root-panel parents (`ui-layout.md` §8.4 scope OPEN); the behaviour is unchanged. **OPEN callouts:** count unchanged (no new or closed callout). **Implementers re-check:** deferred-work slicing while the first index is pending (§3.3). **Left for other owners:** decision 0014 D8 (locked) does not name the first-index exception to its at-least-one-item rule; it should, when 0014 is next revised, together with the skip set already listed. |
| 2026-10-09 | P1 revision (spec writer, dirty room; status Accepted → **Revised**, awaiting review): new §15.1 (inventory views: `GetBackpack`, `GetEquipment`, `Item`, `ItemInfo` over the session's inventory model and the scenario feed), provenance P81–P82 (API facts S2, the `Equipment` ↔ `ContainerSlot` key relation from the data), §18 checks 66–69 for decision 0024 P1 exit criterion 4. No other text changed; section numbers and anchors unchanged. Implementers re-check: the `Turbine.Gameplay` inventory views in `crates/plugins`. |
| 2026-10-09 | Review fixes for the P1 revision (spec writer, dirty room; no new research; status stays **Revised**, awaiting re-review by a session other than the writer and the first reviewer). Blocking (1): §15 enum paragraph and P41 now point to P82 (`Equipment` value = `ContainerSlot` key − 1; `ItemDurability`, `ItemWearState` follow their data enums); the §15 enum OPEN and its §19 line narrowed to `Alignment`, `Profession`, `Vocation`, `ReadyState`, `SkillType`, `EffectCategory` and the stance enums. (2): the §15.1 OPEN is listed in §19, with a closing note. (3): new provenance row P83 (**Tessera rule**, DEC 0024 P1, `character-panels.md` §4.3, §8, §9) covering every Tessera rule of §15.1; §15.1 says which facts are P81/P82 and which P83; §18 has a lead-in for checks 66–69. (4): §15.1 rewritten with a containers table and two getter tables covering every `Item` and `ItemInfo` getter of §16, each with its value for a present property, an absent property and a literal item; `GetItemInfo()` identity (one object per record DID per apartment) stated; the §15.1 OPEN widened to the original's values for absent properties. Non-blocking: (a) the delivery rule points to §15's first paragraph; (b) note that `scenario-feed.md` links resolve after `p0` is merged; (c) and (e) a `SizeChanged` rule for growth before the first `ContainerSize`, and a §20 row asking `character-panels.md` §8.1 to align; (d) check 67 names the `MoveItem` and `DestroyItem` input lines with `defaults on`; (f) blank lines fixed. Checks 66, 67, 69 extended. OPEN count: 57 callout markers in §1–§18 before and after (one narrowed, one widened; none closed, none added). Section numbers and anchors unchanged. Implementers re-check: `Turbine.Gameplay` `Backpack`, `Equipment`, `Item`, `ItemInfo` in `crates/plugins`. |
| 2026-10-11 | P4 revision (spec writer, dirty room; BIN on the same build, S3 re-read; this part awaits review by a session that wrote no part of it): new §15.4 (`Party` and `PartyMember` views over the fellowship model of `social-panels.md` §9 TR2, event derivation from `FellowshipChanged`, worked example), provenance P90 (party event argument tables; corrects the P31 reading of `AssistTargetRemoved`), P91 (getter semantics), P92 (S2), P93 (Tessera rules); a §9.3 table row, a correction note and the member-event OPEN answered; §18 checks 90–96; §19 summary line and closing paragraph; one §20 row. Numbers chosen to merge with the P2 and P3 revisions (§15.2, §15.3, P84–P89, checks 70–89). |
