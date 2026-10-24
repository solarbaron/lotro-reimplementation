<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Parity checklist (P8 exit criterion 1)

| | |
|---|---|
| **Status** | Accepted (2026-10-23, independent review, `CLEAN_ROOM.md` §5.3; written 2026-10-23) |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | spec-review:parity-checklist:1 (independent session) |
| **Review record** | spec-review:parity-checklist:1 (independent session), 2026-10-23: **passed**, Accepted. Items checked (`CLEAN_ROOM.md` §5.3): every statement is a comparison row, a recording rule or a project fact with a source; no code or pseudo-code (the §1.3 commands are Tessera's own command lines); no copied prose; the `UILAYOUT` labels of §2.14 are DID-map labels of the install's data (tool identifiers, not displayed text; precedent `ui-layout.md` §3.2, §17, `records.md` §8.2), used only to find layouts and read at run time; no game text, assets, keys, hostnames, endpoints, account data or traffic (store and administration layouts are counted, not named); every fact group has a provenance row and a mark (Design rows as in other Accepted specs); no byte layouts (a checklist); the row-state rules and the census count are testable; no unknown is guessed (rows that wait for other specs' OPENs say so); nothing outside decision 0034 §2. Spot checks: the reviewer's own read-only probe of the install (2026-10-23) re-counted 302 layout records (all in the locale archive), root key 4 labelled `UILAYOUT`, 236 map entries of which 232 name distinct, existing layouts; every label quoted in §2.14 exists in the map; the counts 16 (housing), 17 (item advancement), 17 (monster play and PvP), 65 (alert roots) and the out-of-scope 20 (13 + 6 + 1) match; 212 + 20 = 232. Row and state counts (178 system rows, 100 sample rows, 8 deviations, 1 performance row) recount; every cited spec, section, owner row, decision, test file, scenario and `--open-panel` name resolves, except the artefacts decision 0043 has the P8 packages build (`parity-tessera.md`, `parity-tooltips.scenario`, `parity_tooltips_real_data.rs`, `busiest.scenario`, `docs/perf/p8-busiest.md`). KD1–KD8 agree with decisions 0040, 0024, 0016, 0022, 0015 (owner question 5), 0014, 0020 and `keymap.md` §8.3.1. Non-blocking notes: (1) KD7 says the 0014 differences were not put to the owner; the owner did answer that record's question 2 (difference 21) and question 4 asks observations for differences 14 and 20, so the next revision could say "except difference 21" (proposing all of them stays safe, O183); (2) "`particles.md` §10.2" means rule 2 of §10; (3) the header's "Agents set only" could say "the spec writer sets only", as §1.2 and decision 0043 §1 item 2 do |
| **Serves** | [Decision 0024](../decisions/0024-parity-track.md) §5, P8 exit criterion 1 ("every row passes or has an owner-accepted deviation, tooltips of 50 items and 50 skills included") and, through [§5](#s5), criterion 3; the rules of [decision 0043](../decisions/0043-p8-parity-audit.md) §1 and §3 (Proposed, written in parallel) |
| **Who fills it in** | The project owner, in the original client, on their own account and install ([`owner-observations.md`](owner-observations.md#o180) O180–O184). Agents set only `pending`, `deviation (proposed)` and `not built`; Tessera-side evidence per row goes to `docs/checklists/parity-tessera.md` (decision 0043 §1 item 3) |
| **Scope** | **Client half only** (decision 0024 §2, [`server-content.md`](../specs/server-content.md) §7): what the original client shows and computes locally from the install. Server-side rules (combat outcomes, loot, quest logic, prices, creature behaviour) and everything that needs an official server (login, world selection and queue, the store; M8, [decision 0034](../decisions/0034-official-server-compatibility.md)) are out of scope and are listed in [§6](#s6), not as rows |
| **Companion documents** | [`owner-observations.md`](owner-observations.md) (the earlier owner rows the parity rows roll up); [`m3-reference-locations.md`](m3-reference-locations.md), [`m4-visual.md`](m4-visual.md), [`m4-5-visual.md`](m4-5-visual.md), [`m5-visual.md`](m5-visual.md), [`m6-audio.md`](m6-audio.md), [`p2-visual.md`](p2-visual.md), [`ui-scaling-visual.md`](ui-scaling-visual.md) (Tessera-side spots and commands); [`docs/USER_GUIDE.md`](../USER_GUIDE.md) (options) |

This is a checklist of comparisons, not a format spec. Each row names one client system the
owner compares in the original client against Tessera's output, the command or committed
scenario that shows Tessera's side, the specs that rule it, the earlier owner rows it closes,
and its state.

<a id="prov"></a>
## Provenance

| # | Fact or group of facts | Source | Mark |
|---|---|---|---|
| PV1 | The native panels of the original client: the root DID map label `UILAYOUT` names 236 entries, 232 of them UI layout records (prefix 0x22), out of 302 layout records in `client_local_English.dat`; the other 70 are templates reached only through base references and layout-valued properties (`ui-layout.md` §3.2). Every one of the 232 layouts is assigned to a row of [§2](#s2) or to the out-of-scope list of [§6](#s6) (212 to rows, 20 out of scope) | M1: throwaway read-only probe of the owner's install (client build 4906.0071.1794.4035, archives unchanged since 2026-10-04), decoded per `ui-layout.md` §3–§4, all 232 layouts | **Confirmed** (observed in the user's install, 2026-10-23) |
| PV2 | The `UILAYOUT` labels quoted in [§2.14](#s2-14) and [§6](#s6) | Same probe; labels are identifiers read from the user's install, used only to find the layouts (as in `ui-layout.md` §3.2 and §17), and are looked up at run time, never hard-coded | **Confirmed** (2026-10-23) |
| PV3 | What Tessera has built, partly built or not built, per row | `docs/ROADMAP.md` status tables of M1–P6 and the UI-scaling and follow-up notes (2026-10-23), `CLAUDE.md` Current state, `docs/USER_GUIDE.md`; whether a layout is used was checked against the specs that name it | **Reported** (project records) |
| PV4 | Specs, sections and earlier owner rows cited per row | The Accepted specs in `docs/specs/` and `owner-observations.md` (2026-10-23) | **Reported** |
| PV5 | The row states, who sets them, the tooltip sample rule, the performance row | Decision 0043 §1 items 2–6 and §3 (Proposed); decision 0031 (tier-2 local goldens); decision 0010 (reference machine) | **Design** |
| PV6 | The deviations of [§4](#s4) and their states | Decisions 0040, 0024 (amendment of 2026-10-16), 0016, 0022, 0014, 0015, 0020; `keymap.md` §8.2 and §8.3.1 | **Reported** (project records) |
| PV7 | Row IDs, grouping and wording | This checklist | **Design** |

---

<a id="s1"></a>
## 1. Rules for recording

<a id="s1-1"></a>
### 1.1 What may be written here

- **No game text in the repository.** Do not write area, item, skill, effect, NPC, quest,
  place or server names, tooltip lines, dialogue or chat output here. Describe what you see in
  your own words. Name records by DID (for example `0x70001234`) or, for the tooltip sample, by
  its ordinal ([§3](#s3)). Keep anything else in a private note outside the repository.
- **No captures or screenshots in the repository.** Screenshots, videos and frame captures of
  either client stay in a private scratch area; only facts read from them (numbers, states,
  yes or no, a short description) are written here.
- **One dated owner note per state change.** Every change of a row's state gets one line in
  the owner log ([§7](#s7)): date, row ID, old and new state, client build (numbers only, as
  the launcher or the client's about box shows it), the Tessera commit compared, graphics
  settings that matter, whether the original ran under Wine/Proton + DXVK, and a short note.
  The row's State cell is then updated to the new state; the note stays in the log.
- **Run Tessera's side first.** Each row names the command, `--open-panel` name, test or
  committed scenario that shows Tessera's output; `parity-tessera.md` holds the evidence and
  the commit. Look at Tessera first, so you know what to compare.

<a id="s1-2"></a>
### 1.2 Row states

| State | Meaning | Who sets it |
|---|---|---|
| `pending` | Not yet compared (the default) | Spec writer |
| `pass` | The owner compared it in the original client and it matches | **Owner only** |
| `deviation (proposed)` | Tessera differs on purpose or cannot match; the row or [§4](#s4) names the reason and the decision or spec rule | Spec writer (from a decision, a spec's Tessera rule the owner saw differ, or a proposal an implementer records in `parity-tessera.md`) |
| `deviation (accepted by owner, DATE, REASON or DECISION)` | The owner accepted the difference | **Owner only** |
| `not built` | Tessera does not have the system or this part of it yet | Spec writer (from the Tessera-side evidence) |

- A `not built` row cannot pass: it is built and compared, or it becomes a proposed deviation
  that the owner accepts.
- A Tessera rule marked **Design** (or TR) in an Accepted spec is not a deviation by itself. It
  becomes `deviation (proposed)` only when the owner records a difference against it.
- When the owner sees a difference that is a Tessera defect rather than a choice, the row stays
  `pending` with a log note; the defect goes to the roadmap, and the row is compared again once
  fixed.
- **Rolls up.** An earlier owner row cited in a row's "Rolls up" column is closed for P8 when
  that parity row is `pass` or an accepted deviation (decision 0043 §1 item 7); mark it
  **done** in `owner-observations.md`. Rows marked there as answered from the client are cited
  for completeness; the comparison is then a confirmation.

<a id="s1-3"></a>
### 1.3 Commands

Abbreviations used in the rows. Run them from the repository root with
`export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"`.

| Short | Meaning |
|---|---|
| **C** | `cargo run --release -p tessera-client --` (the client; add `--ui` for the game UI) |
| **E** | `cargo run --release -p tessera-explorer --` (the explorer) |
| **D** | `cargo run --release -p tessera-dattool --` (the inspection tool) |
| **S *name*** | `C --ui --scenario-input --scenario apps/client/tests/scenarios/`*name*`.scenario` (a committed synthetic scenario, windowed, its input block played) |
| **T *name*** | `cargo test --release -p tessera-client --test `*name*` -- --nocapture` (a real-data test; its local goldens follow decision 0031) |
| **P *name*** | `C --ui --open-panel `*name* (open a panel at start; names in `C --help`) |

---

<a id="s2"></a>
## 2. System rows

Columns: **What to compare** (in both clients); **Tessera shows it with** ([§1.3](#s1-3));
**Specs**; **Rolls up** (earlier owner rows); **State**.

<a id="s2-1"></a>
### 2.1 Install and pre-world screens

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA1 | The install is found and read in place: archives, language, client build; command-line leniency | `D detect --path`, `D info` | `install-layout.md`, `dat-container.md` | O7 | pending |
| PA2 | Settings taken from the install: the render section of the user's preferences (graphics core, far-view switches), the install's key map when imported | **C** windowed (start-up log), keymap import (`USER_GUIDE.md` §4.2) | `far-view.md` (FD16–FD18), `keymap.md` §8.3, `install-layout.md` | — | pending |
| PA3 | Loading screen: backdrop, meter, label, when it shows | **P** `loading-screen`; **S** `travel` | `travel-windows.md` §5, `ui-scaling.md` §10.1 | — | pending |
| PA4 | Story (theatre) screens with their progress layer | — (no client flow opens them; `ui-scaling-visual.md` §T judges their geometry) | `ui-scaling.md` §10.6 | — | not built |
| PA5 | Intro movies, the movie library and the movie player of the creation screens | — | — | — | not built |
| PA6 | Credits screen | — | — | — | not built |
| PA7 | Disconnected screen and the connection-status indicator | — | — | — | not built |

Character selection and creation are P1 rows ([§2.8](#s2-8)).

<a id="s2-2"></a>
### 2.2 World

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA8 | Terrain heights and object placement at the 20 reference locations | **T** `reference_locations`; **C** `--place` per `m3-reference-locations.md` | `landblock.md`, `terrain.md`, `entity-descriptor.md` §4–§8, `coordinates.md` | O1 | pending |
| PA9 | Position read-outs: `/loc`, map coordinates, area names, compass heading of placed objects | walker HUD at the PA8 spots | `coordinates.md` §4, §8–§11, `wstate.md` §11.2 | O1, O2 | pending |
| PA10 | Terrain texturing, the near and far bands and their seam | **C** at the `m4-5-visual.md` §4.1 spots; F5, `--far-band` | `terrain-texturing.md` | O20, O27 | pending |
| PA11 | Placed objects: which content layers show, placeholder parts | **C** `--content-layers`; **E** Landblock tab | `entity-descriptor.md` §4.5, §9.3, `setup.md` §8.5 | O3, O4 | pending |
| PA12 | Materials, dye and render states of objects | **E** `--select DID --view mesh` (`--dye`); `m2-visual.md` | `material.md`, `mesh.md`, `texture-list.md` | O5, O6 | pending |
| PA13 | Generated scenery | `m4-5-visual.md` §4.2 spots; F2 | `scenery.md` §4–§7 | O20 | pending |
| PA14 | Frills (grass and small plants): radius, fade, density, sway, tint | `m4-5-visual.md` §4.3; F4, `--frill-distance` | `scenery.md` §8 | O25 | pending |
| PA15 | Water: terrain water, placed water, puddles, waterfalls, liquid distortion, lava, the far sea | `m4-5-visual.md` §4.4, §4.8; F7 | `water.md`, `material.md` §10.3, §11.6 | O24, O87, O91 | pending |
| PA16 | Sky, sun, moon and clouds at four times of day | `m4-5-visual.md` §4.5; `--time-of-day`; **E** Sky tab | `environment.md` | O26 | pending |
| PA17 | Game clock: time of day, day length, epoch, day type | `--time-of-day`, `--time-rate`, `--day`, `--day-type`; `[`, `]` | `environment.md` §6, §10.1, §11.1 | O21 | pending |
| PA18 | Fog (outdoor and interior) and default draw distances, object pop-in | `--landscape-distance`, `--object-draw-distance`, `--disable fog` | `environment.md` §11.7, `walker.md` §11.3–§11.4, `mesh.md` §8 | O22, O26 | pending |
| PA19 | Sun shadows, ambient light and point lights | `m4-5-visual.md` §4.6; F6, `--disable lights` | `lighting.md` | O23 | pending |
| PA20 | Day and night lamp lights on objects, figures, particles and decals | **T** `lamp_lights_real_data`; `--time-of-day 0.9 --time-rate 0` | `instance-scripts.md` §6, `lighting.md` | — | pending |
| PA21 | Far view: far terrain, sprites, horizon, far entities, normal map and seam overlay | **C** `--far-view on`; **T** `far_view_real_data` | `far-view.md`, `distant-landscape.md` | O22 | pending |
| PA22 | Placed decals (interiors as non-receivers by `decals.md` §9.1 answer (b)) | walker; **E** 3D decal preview | `decals.md` | — | pending |
| PA23 | Particle effects of placed objects and figures, beams, lit systems, post-pass systems | walker (`--no-particles` to compare); **E** `--particles DID --sheet`; **T** `particles_real_data` | `particles.md`, `particle-runtime.md`, `script-table.md` | — | pending |
| PA24 | Fork effects, weather systems (rain, snow), wind, opacity inherited from the owner | — | `particles.md` §10.2; `particle-runtime.md` §3.2, §5.4, §9.2, §12 (OPEN) | — | not built |
| PA25 | Interiors: cells, their objects and light | **C** `--interiors`; **T** `interiors_real_data` | `landblock.md`, `collision.md` (cells) | — | pending |
| PA26 | Walking on collision: steps, slopes, ceilings, wading and swimming, the scripted walks | **C** `--walk-path docs/checklists/m4-5-visual.md` | `collision.md` §5–§12, `water.md` §7.4 | O18, O19 | pending |
| PA27 | Movement speeds, turn rate and the jump (height, airtime) | walker, `/loc` timing; **T** `jump_script_real_data` | `collision.md` §10–§11 | O18, O70 | pending |
| PA28 | Camera: follow distance, pitch, height, smoothing, field of view, terrain clamp, mouse-look | walker | `walker.md` §11.6–§11.8, §14.11 | — | pending |

<a id="s2-3"></a>
### 2.3 Characters and animation

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA29 | Standing heights of every playable body | **E** `--character SPECIES,SEX --sheet` (`m4-visual.md` §B) | `appearance.md` §6, `skeleton-anim.md` §2.2 | O8 | pending |
| PA30 | Body shape and height controls on figures, feet on the ground | **E** `--character … --sliders HEX`; creation screen | `appearance.md` §6.2–§6.5 | O9 | pending |
| PA31 | Faces, skin, hair and eye tints, face layers, the updated-avatar option | **E** `--character`; **T** `chargen_bodies_real_data` | `appearance.md` §3.3, §3.5 | O11, O15 | pending |
| PA32 | Worn items one at a time and layered | **C** `--avatar "BODY;ITEM;…"`; **E** `--wear DID` (`m4-visual.md` §W, §L) | `appearance.md` §4 | O10 | pending |
| PA33 | Held items: where they rest, drawn and stowed | `--avatar "…;DID@wielded"` (`m4-visual.md` §H) | `appearance.md` §5, `skeleton-anim.md` §8.5 | O13 | pending |
| PA34 | Idle, walk and run clips, fidgets, blending, turning | **E** `--character … --clip idle\|walk\|run`; walker | `skeleton-anim.md`, `script-table.md` §8 | O14, O16 | pending |
| PA35 | Emotes and stances seen on figures | two clients on a test server (`C --server …`) | `tessera-protocol.md` (emotes as opaque keys), `script-table.md` | — | pending |
| PA36 | Static NPCs: presence, position, facing, look | walker at the `m4-visual.md` §N spots; **T** `npc_spots_real_data` | `entity-descriptor.md` §9.4, `appearance.md` §7 | O17 | pending |
| PA37 | Random-appearance and generated figures | walker (`m4-5-visual.md` §4.6 L5 note) | `appearance.md` §7.4, `script-runtime.md` §8 | O12, O92 | pending |
| PA38 | NPC dramas (scripted NPC performances) | **E** Character tab, drama player | `npc-drama.md` | — | pending |
| PA39 | Mount figures: look and rider pose | **E** Character tab, mount viewer; **C** `--mount` | `mounts.md` | — | pending |
| PA40 | Other players' characters: look, movement smoothing | two clients on `tessera-server` (`C --server … --account …`) | `tessera-protocol.md` §8.6, §9 | O71 | pending |

<a id="s2-4"></a>
### 2.4 Audio and player music

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA41 | Area music and its cross-fades at area borders; combat music fade | `cargo run --release -p tessera-client --features audio-device --` (`m6-audio.md`); **T** `audio_real_data` | `sound.md` §10, §16 | O28 | pending |
| PA42 | Ambient beds: day and night, indoor, falloff, gaps | **T** `audio_interiors_real_data`; walker with audio | `sound.md`, `script-runtime.md` §26 | O28 | pending |
| PA43 | Sounds of objects, figures and effects | walker with audio near scripted objects | `sound.md`, `script-table.md` | O28 | pending |
| PA44 | UI sounds (buttons, chat feedback) | game UI with audio | `ui-layout.md` §6.9, `chat-window.md` §5.6 | — | pending |
| PA45 | Player music alone: `/play`, `/playstart`, `/playlist`, `/playstop`, `/instrument`, ABC handling, the instance limit | **C** `--command "/play NAME"`; **T** `player_music_real_data` | `abc-dialect.md`, `instruments.md` | O28 | pending |

Synchronised fellowship music is PA111.

<a id="s2-5"></a>
### 2.5 UI framework

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA46 | Native panels: geometry, art and states of the fifteen M5 panels | **P** `P1` … `P15` (`m5-visual.md` §P); **T** `ui_panels_real_data` | `ui-layout.md` §9, §17 | O39 | pending |
| PA47 | Drawing: nested surfaces, opacity, auto-fade, edge blending | `m5-visual.md` §D; own test plugin | `ui-layout.md` §9, decision 0015 | O46, O47 | pending |
| PA48 | Widgets: buttons, lists, trees, scroll bars, meters, text entry | panels in use; **T** `ui_review_real_data` | `ui-widgets.md`, `ui-behaviour.md` §4–§5 | — | pending |
| PA49 | Fonts and text: choice, outline, truncation, markup | every panel | `fonts.md`, `string-table.md`, `ui-behaviour.md` §10 | — | pending |
| PA50 | UI tooltips: delay, content, size, placement, hiding | hover any element with **C** `--ui` | `ui-behaviour.md` §6 | — | pending |
| PA51 | Dragging items: drag icon, drop targets, cancel | **S** `inventory-panels` | `ui-behaviour.md` §7, decision 0014 | O44 | pending |
| PA52 | Moving and resizing panels, keeping them on screen, saved positions | drag and resize, restart the client | `ui-behaviour.md` §8.1–§8.4, `ui-layout.md` §9.9, `ui-scaling.md` §5.8 | — | pending |
| PA53 | User skins | **C** `--skin NAME`; **T** `ui_skin_real_data` | `skin-definition.md` | O41 | pending |
| PA54 | Cursor: shapes, size, during and after mouse-look | **C** `--ui`, `--ui-scale` | `ui-behaviour.md` §4.8, `ui-scaling.md` §6.7, decision 0015 | O48 | pending |
| PA55 | Key bindings and mouse buttons: defaults, second presses, wheel over plugin windows | keymap file; **T** `routed_keys_real_data` | `keymap.md`, decision 0014 | O42, O43 | pending |
| PA56 | Escape with panels open: what closes, in what order | Escape in **C** `--ui` | `game-panels.md` §16 | O45 | pending |
| PA57 | UI scale, resolution, aspect, the scale slider | `--ui-scale`, `--ui-safe-aspect`, `/uiscale` (`ui-scaling-visual.md`) | `ui-scaling.md` | — | deviation (accepted by owner, 2026-10-16, decision 0040; see KD1) |
| PA58 | Engine dialog boxes: wait, confirmation, message and text-input dialogs, their keys | quest abandon in **S** `quest-log`; vendor quantity in **S** `vendor` | `ui-widgets.md` §18, `ui-behaviour.md` §5.5 | — | pending |
| PA59 | Context menus: items, submenus, check marks, placement, open delay | tracker menu in **S** `quest-log`; map menu in **S** `map` | `ui-widgets.md` §19 | — | pending |
| PA60 | Main menu | **P** `P6`; Escape with nothing selected | `game-panels.md`, `ui-layout.md` §17 | — | pending |
| PA61 | Options window: pages and their settings (only the layout and the Interface window's scale controls exist) | **P** `P7` | `ui-layout.md` §17.5 (OPEN (d)), `ui-scaling.md` §7.2 | — | not built |
| PA62 | Toolbar and extra shortcut bar, combat state of the toolbar | **C** `--ui`; **P** `P3`, `P14` | `game-panels.md`, `combat-ui.md` | — | pending |
| PA63 | Character journal | **P** `P13` | `game-panels.md`, `ui-layout.md` §17 | — | pending |
| PA64 | Portraits in the player and target frames (drawn in tests, not yet by the client's frame loop) | **T** `combat_portraits_real_data` | `ui-widgets.md` §12, `combat-ui.md` §4.1, §5.1 | — | not built |

<a id="s2-6"></a>
### 2.6 Plugins

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA65 | Plugin discovery, loading and unloading, the plugin manager, `/plugins` | `--load-plugin NAME`; **P** `P12`; `cargo test -p tessera-client --test plugins_suite` | `plugin-descriptor.md`, `plugin-runtime.md`, `lua-plugin-api.md` §4–§7 | O40 | pending |
| PA66 | Plugin UI: `Turbine.UI` controls and the game-styled controls | own test plugin; the census plugins | `lua-plugin-api.md` §13–§14 | O47 | pending |
| PA67 | Plugin events, shell commands, chat lines, plugin data | **T** `plugins_suite` | `lua-plugin-api.md` §9–§12 | — | pending |
| PA68 | Inventory views (§15.1): backpack, equipment, items | **S** `inventory` with a test plugin; **T** `plugin_inventory_checks` | `lua-plugin-api.md` §15.1 | — | pending |
| PA69 | Combat views (§15.2): target, vitals, effects, skills, quickslots | **T** `plugin_combat_checks` | `lua-plugin-api.md` §15.2 | — | pending |
| PA70 | Wallet views (§15.3) | **T** `npc_windows_plugins` | `lua-plugin-api.md` §15.3 | O125 | pending |
| PA71 | Party views (§15.4) | **S** `fellowship` with a test plugin | `lua-plugin-api.md` §15.4 | — | pending |
| PA72 | Mount views (§15.5) | **T** `mount_plugin_views` | `lua-plugin-api.md` §15.5 | O154 | pending |
| PA73 | Progression and crafting views (§15.6) | **T** `p6_plugin_views`, `p6_host_plugin_views` | `lua-plugin-api.md` §15.6 | O174 | pending |
| PA74 | Gameplay views outside §15.1–§15.6: vault and shared storage, pets, per-class attributes and stances | — | `lua-plugin-api.md` §15, §16, §17.3 | — | not built |

<a id="s2-7"></a>
### 2.7 Chat and commands

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA75 | Chat window: tabs, filters, colours, timestamps, scrolling, line cap, opacity, auto-fade, at UI scales 1.0, 1.5 and 0.5 | **C** `--ui` | `chat-window.md` §3–§5 | O46 | pending |
| PA76 | Entry line: channel modes, history, line keys, key-repeat guard, `;` substitutions | typing in **C** `--ui` | `chat-window.md` §6–§7 | — | pending |
| PA77 | Built-in commands, `/help`, an unknown command | `--command "/help"`; **T** `chat_commands` | `chat-window.md` §7–§10, decision 0020 | — | pending |

Tells and named channels are P4 rows ([§2.11](#s2-11)).

<a id="s2-8"></a>
### 2.8 P1: character screens, panels, bags and item tooltips

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA78 | Character selection screen: list, figure, scene, entering the world | **C** `--character-select` | `character-screens.md` §2–§3, §6 | O85 | pending |
| PA79 | Race and class screens: order, lists, defaults, locked or hidden choices | **C** `--character-select`, create | `character-screens.md` §4, §9 | O85, O89 | pending |
| PA80 | Appearance and name screen: controls, randomise, colours, the figure's scripts and props | **C** `--character-select`, appearance stage | `character-screens.md` §5, §7.1, `appearance.md` §6 | O9, O86, O90 | pending |
| PA81 | Character panel: opening, paperdoll rotation, header, both pages, stat rows | C key; **P** `character`; **S** `inventory-panels` | `character-panels.md` §2–§3 | O82 | pending |
| PA82 | Bag windows: slots per bag, all-bags command, item-slot overlays | I key; **P** `bags`; **S** `backpack-growth` | `character-panels.md` §4–§5 | O83 | pending |
| PA83 | Item handling: equip, unequip, use, split a stack, destroy | **S** `inventory-panels` | `character-panels.md` §6 | O84 | pending |
| PA84 | Worn look after equipping, which hand a held item goes to | **S** `inventory`; **T** `worn_look_real_data` | `character-panels.md` §7, `appearance.md` §8 | O10, O88 | pending |
| PA85 | Item tooltips: rows, order, colours, worth coins, wrapping (the sample is [§3](#s3)) | hover items; **T** `item_tooltips_real_data`; **E** tooltip preview | `item-tooltips.md` §4–§7, `icon-layers.md` | O80 | pending |
| PA86 | Comparison tooltips and pinned tooltips (H), their cascade and close button | hover with an item equipped; H | `item-tooltips.md` §8–§10 | O81 | pending |

<a id="s2-9"></a>
### 2.9 P2: targeting, quickslots, cast bars, vitals, floating text, effects, defeat

Tessera's side for all rows: **S** `combat-ui` and the commands of `p2-visual.md`.

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA87 | Targeting keys (Tab, Backspace, F9, F10, Delete, Backslash, F), mouse selection, Escape with a selection and a cast | **S** `combat-ui` | `combat-ui.md` §3.3–§3.4, `interaction.md` | O93, O94 | pending |
| PA88 | World selection marker and name block | **S** `combat-ui` | `combat-ui.md` §3.5 | O95 | pending |
| PA89 | Player and target frames: layout, vital texts, bar colours, name colours by level, class resource | **S** `combat-ui` | `combat-ui.md` §4–§5, `game-controls.md` §7.5 | O96, O97, O98, O128 | pending |
| PA90 | Selection's-target window | **S** `combat-ui` | `combat-ui.md` §5.7 | O99 | pending |
| PA91 | Quickslot bar: keys, cooldown sweep and timer, greyed and refused slots | **S** `combat-ui`; keys 1–0, −, = | `combat-ui.md` §9, `keymap.md` | O129 | pending |
| PA92 | Cast, induction and channel bars: delay, interrupt, cancel | **S** `combat-ui` | `combat-ui.md` §7 | O101, O129 | pending |
| PA93 | Effect icons: rows, order, timers, stacks, pulse, overflow | **S** `combat-ui` | `combat-ui.md` §8 | O102 | pending |
| PA94 | Floating combat text: sign, wording, colours, motion, criticals | **S** `combat-ui` | `combat-ui.md` §6 | O100 | pending |
| PA95 | Defeat panel: buttons, ghosting, awaken timer | **S** `combat-ui` | `combat-ui.md` §10 | O103 | pending |
| PA96 | Skill tooltips: damage, effects, cost, stat lines, casting and recovery times, skill-granting items (the sample is [§3](#s3)) | hover a quickslot; **T** `combat_tooltips_real_data` | `skill-tooltips.md` | O104, O106, O107, O109 | pending |
| PA97 | Effect tooltips from the effect bar: lines, countdown, stacks | hover an effect icon | `skill-tooltips.md` §9–§12 | O105, O108 | pending |
| PA98 | Combat visuals: casts, hits, projectiles, combat stance, a weapon change mid-swing | **T** `combat_visuals_real_data`; `p2-visual.md` C4 | `combat-visuals.md` §3–§6 | O126 | pending |
| PA99 | Effect visuals to their end; death pose, body fade, revive | `p2-visual.md` C4 | `combat-visuals.md` §5, §7 | O127 | pending |

<a id="s2-10"></a>
### 2.10 P3: dialogue, quest log and tracker, vendors, loot, wallet

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA100 | Using NPCs and objects: inputs, out-of-range and no-selection lines, use distance | U, double click; **S** `dialogue` | `npc-windows.md` §3.1, `interaction.md` | O110 | pending |
| PA101 | Dialogue windows: conversation, choice list, NPC comment, closing on walking away | **S** `dialogue`; **P** `conversation`, `choice`, `npc-comment` | `npc-windows.md` §5 | O111 | pending |
| PA102 | Quest-offer window: objective, reward rows, choosing a reward | **S** `dialogue`, `npc-windows` | `npc-windows.md` §6 | O112 | pending |
| PA103 | Vendor window: buy modes, quantity, prices, sell, drop to sell, Sell All, buy-back, repair page | **S** `vendor`; **P** `vendor`, `vendor-quantity` | `npc-windows.md` §7.2–§7.6, §7.8 | O113 | pending |
| PA104 | Barter window: cost slots, coin costs, cost text, pages | **P** `barter` | `npc-windows.md` §7.7 | O114 | pending |
| PA105 | Loot window and opened containers: coins, Loot All, closing | **S** `loot`; **P** `loot` | `npc-windows.md` §8 | O115 | pending |
| PA106 | Wallet window: opening (B), order, capped currencies, search | **P** `wallet` | `npc-windows.md` §9.1, §9.4 | O116 | pending |
| PA107 | Coin display: which coins show, zero coins | **P** `wallet`, `vendor` | `npc-windows.md` §9.2–§9.3 | O117 | pending |
| PA108 | Quest log and details pane: order, rows, filters, counters, abandoning | L; **S** `quest-log`; **P** `quest-log` | `quest-journal.md` §4–§5 | O118, O119, O124 | pending |
| PA109 | Quest tracker: auto-tracking, order, long entries, completed quests, its menu | **S** `quest-log`; **P** `tracker` | `quest-journal.md` §6 | O120 | pending |
| PA110 | Quest messages in chat and on screen; quest rings over NPCs; quest markers on the minimap and the map; quest history window | **S** `quest-log`; `--quest-icons on` | `quest-journal.md` §7–§8, `game-panels.md` §8.8, §9.3 | O121, O122, O123 | pending |

<a id="s2-11"></a>
### 2.11 P4: fellowship, trade, friends and ignore lists, tells, channels, synchronised music

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA111 | Synchronised fellowship music: how aligned the start sounds | **S** `music-sync` | `instruments.md` §8.8–§8.9 | O136 | pending |
| PA112 | Fellowship display and member vitals: leader marker, member out of view, removal, disband lines | **S** `fellowship` | `social-panels.md` §4, `chat-window.md` §15.7 | O130 | pending |
| PA113 | Fellowship invitation window: buttons, expiry | **S** `fellowship` | `social-panels.md` §5 | O131 | pending |
| PA114 | Trade window: both sides, coin, ready states, completion | **S** `trade` | `social-panels.md` §6 | O132 | pending |
| PA115 | Social panel: friend and ignore lists, online and offline notices | **S** `friends`; **P** `P8` | `social-panels.md` §3, `chat-window.md` §15.8–§15.9 | O133 | pending |
| PA116 | Tells and the instant-message tab, refusals | two clients on a test server; **S** `social` | `chat-window.md` §15.2–§15.5 | O134 | pending |
| PA117 | Named chat channels: join and leave lines, slots, tabs | test server (`C --server …`) | `chat-window.md` §15.6, `tessera-protocol.md` §10.6 | O135 | pending |
| PA118 | Social panel pages other than friends and ignore: kinship, raid, who and looking-for-fellowship pages (shown empty) | **P** `P8` | `social-panels.md` §3.1, §9 TR8 | — | not built |

<a id="s2-12"></a>
### 2.12 P5: maps, travel, instance finder, mounts

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA119 | World map at first opening: full screen or window, size, position, controls per kind of map | M; **P** `map`; **S** `map` | `map-windows.md` §3.1, §3.4 | O140 | pending |
| PA120 | Map overlays, filters and notes; server-added markers | **S** `map` | `map-windows.md` §6 | O141, O155 | pending |
| PA121 | Map navigation: right click, the map menu, child maps; dungeon maps (stretch, fog, tooltips) | **S** `map`; walk into a dungeon | `map-windows.md` §4 | O142, O143 | pending |
| PA122 | Minimap: frame, notes and blips, rim icons, tooltip, filter menu, marker edges | **C** `--ui`; **P** `P1` | `game-panels.md` §8, `map-windows.md` §6.8 | O48, O144 | pending |
| PA123 | Coordinates under the pointer against `/loc`; place lookup | **S** `map` | `map-windows.md` §3.4, §7.1 | O145 | pending |
| PA124 | Travel window: headers, rows, search, cost display, go button, closing | **S** `travel`; **P** `travel` | `travel-windows.md` §4 | O146, O147 | pending |
| PA125 | Travel presentation: end-travel box, loading screen, durations, chat lines | **S** `travel`; **P** `end-travel` | `travel-windows.md` §5 | O148 | pending |
| PA126 | Instance finder: window, page, headers, featured field, size, tier and level controls | **S** `instances`; **P** `instance-finder` | `travel-windows.md` §6.1–§6.4 | O149 | pending |
| PA127 | Instance queue, invitation window, leaving an instance | **S** `instances`; **P** `queue-alert`, `instance-travel` | `travel-windows.md` §6.5–§6.6, §7.2 | O150 | pending |
| PA128 | Mounted gaits of a normal mount: speeds, strafing, camera height, small-body mount scale | **C** `--mount …`; **S** `mount`; **T** `mount_walk_real_data` | `mounts.md` §5.1, §14, `walker.md` §16.12 | O151 | pending |
| PA129 | Mounted gaits of a war-steed: gait steps, timing, release, braking | **C** `--mount steed:medium` | `mounts.md` §5.3–§5.7, §14.6, `walker.md` §16.12 | O152 | pending |
| PA130 | Mounting and dismounting: summon timing, cancels, water, forbidden areas | Ctrl+M, `/ride`; **S** `mount` | `mounts.md` §14.7, §14.12 | O153 | pending |

<a id="s2-13"></a>
### 2.13 P6: experience, traits, skill training, crafting, gathering

| ID | What to compare | Tessera shows it with | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA131 | Experience bar: place, segments, colours, tooltip, gain animation | **P** `experience`; **S** `progression-panels` | `progression-panels.md`, `game-panels.md` §11 | O160 | pending |
| PA132 | Level-up presentation: effect, sound, chat lines, banner, bar reset | **S** `progression-panels` | `progression-panels.md`, `scenario-feed.md` §6.6 | O161 | pending |
| PA133 | Trait panel: tabs, trees, branches, cells, ranks, dependencies, points, spend and refund | J; **P** `traits`; **S** `traits` | `progression-panels.md`, `combat-data.md` §8.3–§8.5 | O162, O163 | pending |
| PA134 | Trait slots and trait tooltips | **S** `traits` | `progression-panels.md`, `combat-data.md` §8.1–§8.3 | O164 | pending |
| PA135 | Skill-training window: rows, columns, ghosting, confirm | **P** `training`; **S** `progression-panels` | `progression-panels.md`, `npc-windows.md` §5.3 | O165 | pending |
| PA136 | Skills panel (K) | K | `progression-panels.md`, `keymap.md` §8.3 | — | pending |
| PA137 | Crafting panel: profession tabs, extra tabs, tier rows, meters | T; **P** `crafting`; **S** `crafting` | `crafting-panels.md` §4.3, §4.7 | O166 | pending |
| PA138 | Recipe list: expansion, order, counts, green states, search | **S** `crafting` | `crafting-panels.md` §4.4–§4.6 | O167 | pending |
| PA139 | Recipe page and make controls: outputs, tool and facility lines, optional ingredients, critical chance, quantity, Make during a batch | **P** `crafting-recipe`; **S** `crafting` | `crafting-panels.md` §5 | O168, O169 | pending |
| PA140 | Crafting bar and result lines | **S** `crafting`, `progression-panels` | `crafting-panels.md` §6, `combat-ui.md` §7 | O170 | pending |
| PA141 | Recipe-scroll tooltips | hover a recipe scroll in **S** `crafting` | `crafting-panels.md` §7, `item-tooltips.md` §6.20 | O171 | pending |
| PA142 | Crafting trainers: choice rows, window texts, profession selector, vocation confirmation | **P** `vocation-trainer`, `profession-trainer`, `craftguild-trainer` | `crafting-panels.md` §8 | O172 | pending |
| PA143 | Gathering nodes: highlight, cursors, gathering bar, loot, removal, refusals | **S** `gathering` | `crafting-panels.md` §9.1, `interaction.md` §3.3–§3.8 | O173 | pending |
| PA144 | Panel toggle keys of crafting, traits, skills and professions; their close behaviour | T, J, K, `toggle_crafting_1` … `3` | `crafting-panels.md` §12, `keymap.md` §8.3 | O175 | pending |

<a id="s2-14"></a>
### 2.14 Systems of the original client that Tessera has not built or not specified

Found in the install's UI layout census (PV1). Each row lists the `UILAYOUT` labels of its
layouts. Dialog boxes, context menus and frame portraits are rows PA58, PA59 and PA64.

| ID | System (plain words) | Install layouts (`UILAYOUT` labels) | Specs | Rolls up | State |
|---|---|---|---|---|---|
| PA145 | On-screen alerts, error text and broadcast text | `alert` (most of its 65 roots), `gameplay` (the alert field), `errorspewbox`, `raidshoutspewbox`, `globalbroadcastspewbox` | — | — | not built |
| PA146 | Tutorials: hints, highlighters, tutorial list and chooser | `tutorial_list`, `tutorial_display`, `tutorial_highlighter`, `tutorialchooser` | `game-panels.md` §5 (highlighters hidden) | — | not built |
| PA147 | Help and assistance windows | `help`, `assistance` | — | — | not built |
| PA148 | Deeds: the accomplishment panel and tracker | `accomplishment`, `accomplishmentdisplay` | — | — | not built |
| PA149 | Titles and surname | `titleview`, `surnamedisplay` | — | — | not built |
| PA150 | Reputation and factions | `reputationpanel`, `factiondisplay` | — | — | not built |
| PA151 | Lore journal page and discoveries | `lore_journal`, `discovery` | — | — | not built |
| PA152 | Mail | `mail` | — | — | not built |
| PA153 | Auction house | `auction` | — | — | not built |
| PA154 | Housing: neighbourhoods, purchase, permissions, upkeep, decoration, storage, house information, gifts | the 16 housing layouts (`housing…`, `neighborhoodgate`, `playerhouseinfo`, `basehousinginfo`, `neighborgift`) | — | — | not built |
| PA155 | Vault, shared storage and banks | `bankrepository`, `sharedvault`, `characterbank`, `sharedhousingbank`, `standardhousingbank` | — | — | not built |
| PA156 | Wardrobe, outfits, the appearance-change (barber) screen, colour swatches | `wardrobe`, `appearancemod`, `characterappearancepage`, `colorswatch` | `character-panels.md` (page named only) | — | not built |
| PA157 | Item advancement: legendary items, legacies, relics and runes, sockets and gems, reward tracks, trackers | the 17 item-advancement layouts (`itemadvancement…`, `legacy`, `runic`, `rewardtrack`, `advancementtracker`, `itemsocket_usegem`, `itemsocket_control`, `disenchantmentdisplay`, `multiusage`, `examination_ia_imbuedlegacy`) | — | — | not built |
| PA158 | Collections | `collection` | — | — | not built |
| PA159 | Monster play and player-versus-player: lobby, match found, scores, rank and prestige, virtues, feats, battle map, the critter mission controller, destiny points | the 17 such layouts (`pvp…`, `mpvirtue_display`, `pvpinfo_display`, `mpbscoreboard`, `mpmissioncontroller`, `destiny_display`, `destiny_wallet`) | `ui-scaling.md` §10.8(a) (geometry only) | — | not built |
| PA160 | Skirmishes and large battles | `skirmishpanel`, `bigbattles`, `bigbattlesresults` | — | — | not built |
| PA161 | Session play | `sessionplay`, `sessionplayinitiation`, `sessionplayend` | — | — | not built |
| PA162 | Hobbies and the hobby trainer | `hobbypanel`, `hobby_trainer` | — | — | not built |
| PA163 | Pets and escort managers | `pet`; the pet and escort boxes of `gameplayactivatables` | — | — | not built |
| PA164 | Mounted-combat appearance and trait pages | `mountedcombat` | — | — | not built |
| PA165 | Class resource pips, gambits, fellowship manoeuvres | `pip_display` (pips hidden, `combat-ui.md`), `gambit`, `conjunction`, `conjunctioninfo` | `combat-ui.md` | — | not built |
| PA166 | Group loot: item rolls, master loot, remote loot; raid assist window | `itemrolldisplay`, `masterlootdisplay`, `remoteloot`, `raidassist` | — | — | not built |
| PA167 | Threat tracker, property tracker, contextual quickslot bar | `threattracker`, `threatmeter`, `propertytracker`, `contextualquickslots` | — | — | not built |
| PA168 | Readable items: scrolling, paged and editable texts, images, plaques, quest billboards | `textitem_scrolling`, `textitem_paged`, `textitem_bookedit`, `imageitem`, `plaquedisplay`, `billboard` | — | — | not built |
| PA169 | Kinship: the kinship page, calendar, invitation, charter tooltip | `guild`, `guild_calendar`, `guild_invite`, `examination_item_kinshipcharter` | `social-panels.md` §9 TR8 (page empty) | — | not built |
| PA170 | Allegiances | `allegiancepanel` | — | — | not built |
| PA171 | Item utilities: find items, package and random rewards, lockbox keys, amount selector, conversion confirmation, perk vendor, trait manager, notary | `finditems`, `selectpackageitem`, `randomitemreward`, `lockbox_applykey`, `amount_selector`, `conversionconfirmationui`, `perkvendor`, `traitmanager`, `notary` | — | — | not built |
| PA172 | Inspecting another player; stat details and stat update | `inspectdisplay`, `examination_stat`, `statupdate` | — | — | not built |
| PA173 | Start menu of the toolbar | `startmenu` | — | — | not built |
| PA174 | Renaming a character | `renamecharacterdialog` | — | — | not built |
| PA175 | Epilogue screen | `epilogue` | — | — | not built |
| PA176 | Effect filter panel (sound and particle filter buttons) | `filter` | — | — | not built |
| PA177 | Skill group panel | `skillgrouppanel` | — | — | not built |
| PA178 | UI layout files and the options export and import (aliases, chat colours, layouts) | — (files, not layouts) | `ui-behaviour.md` §8.5 (OPEN) | — | not built |

The intro movies, credits, disconnected and story screens of the same census are rows PA4–PA7.

---

<a id="s3"></a>
## 3. Tooltip sample: 50 items and 50 skills

The rule lives in [decision 0043](../decisions/0043-p8-parity-audit.md) §1 item 6; this section
only gives the rows.

- **How the sample is chosen.** The members are not listed here. They are chosen at run time
  from the user's install by the seeded selectors of the committed synthetic scenario
  `apps/client/tests/scenarios/parity-tooltips.scenario` (`scenario-feed.md` §8), which names
  no DID.
- **How Tessera shows them.** The real-data test
  `apps/client/tests/parity_tooltips_real_data.rs`
  (`TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test parity_tooltips_real_data`)
  paints each tooltip with the client's UI host into the local-only golden directory of
  [decision 0031](../decisions/0031-parity-panel-goldens.md)
  (`${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/…/<install version>/`),
  with a local index that gives each ordinal its DID and name so the owner can find the record
  in the original. **Neither the index nor the images are ever committed** (the index holds
  game text).
- **The sample depends on the install version.** A patch can change it; compare the sample of
  the install version the index directory names, and give that version in the log note.
- **What to compare.** Rows and their order, values, colours, icons, wrapping and size of the
  tooltip (`item-tooltips.md` for items, `skill-tooltips.md` for skills). An ordinal the
  original does not let you find stays `pending` with a log note. Record ordinals and DIDs
  only, never names or tooltip text.
- **Rolls up:** O181 (items) and O182 (skills); PA85 and PA96 cover the tooltip systems as a
  whole.

<a id="s3-1"></a>
### 3.1 Items (PT-I1 to PT-I50)

| ID | Ordinal | State |
|---|---|---|
| PT-I1 | 1 | pending |
| PT-I2 | 2 | pending |
| PT-I3 | 3 | pending |
| PT-I4 | 4 | pending |
| PT-I5 | 5 | pending |
| PT-I6 | 6 | pending |
| PT-I7 | 7 | pending |
| PT-I8 | 8 | pending |
| PT-I9 | 9 | pending |
| PT-I10 | 10 | pending |
| PT-I11 | 11 | pending |
| PT-I12 | 12 | pending |
| PT-I13 | 13 | pending |
| PT-I14 | 14 | pending |
| PT-I15 | 15 | pending |
| PT-I16 | 16 | pending |
| PT-I17 | 17 | pending |
| PT-I18 | 18 | pending |
| PT-I19 | 19 | pending |
| PT-I20 | 20 | pending |
| PT-I21 | 21 | pending |
| PT-I22 | 22 | pending |
| PT-I23 | 23 | pending |
| PT-I24 | 24 | pending |
| PT-I25 | 25 | pending |
| PT-I26 | 26 | pending |
| PT-I27 | 27 | pending |
| PT-I28 | 28 | pending |
| PT-I29 | 29 | pending |
| PT-I30 | 30 | pending |
| PT-I31 | 31 | pending |
| PT-I32 | 32 | pending |
| PT-I33 | 33 | pending |
| PT-I34 | 34 | pending |
| PT-I35 | 35 | pending |
| PT-I36 | 36 | pending |
| PT-I37 | 37 | pending |
| PT-I38 | 38 | pending |
| PT-I39 | 39 | pending |
| PT-I40 | 40 | pending |
| PT-I41 | 41 | pending |
| PT-I42 | 42 | pending |
| PT-I43 | 43 | pending |
| PT-I44 | 44 | pending |
| PT-I45 | 45 | pending |
| PT-I46 | 46 | pending |
| PT-I47 | 47 | pending |
| PT-I48 | 48 | pending |
| PT-I49 | 49 | pending |
| PT-I50 | 50 | pending |

<a id="s3-2"></a>
### 3.2 Skills (PT-S1 to PT-S50)

| ID | Ordinal | State |
|---|---|---|
| PT-S1 | 1 | pending |
| PT-S2 | 2 | pending |
| PT-S3 | 3 | pending |
| PT-S4 | 4 | pending |
| PT-S5 | 5 | pending |
| PT-S6 | 6 | pending |
| PT-S7 | 7 | pending |
| PT-S8 | 8 | pending |
| PT-S9 | 9 | pending |
| PT-S10 | 10 | pending |
| PT-S11 | 11 | pending |
| PT-S12 | 12 | pending |
| PT-S13 | 13 | pending |
| PT-S14 | 14 | pending |
| PT-S15 | 15 | pending |
| PT-S16 | 16 | pending |
| PT-S17 | 17 | pending |
| PT-S18 | 18 | pending |
| PT-S19 | 19 | pending |
| PT-S20 | 20 | pending |
| PT-S21 | 21 | pending |
| PT-S22 | 22 | pending |
| PT-S23 | 23 | pending |
| PT-S24 | 24 | pending |
| PT-S25 | 25 | pending |
| PT-S26 | 26 | pending |
| PT-S27 | 27 | pending |
| PT-S28 | 28 | pending |
| PT-S29 | 29 | pending |
| PT-S30 | 30 | pending |
| PT-S31 | 31 | pending |
| PT-S32 | 32 | pending |
| PT-S33 | 33 | pending |
| PT-S34 | 34 | pending |
| PT-S35 | 35 | pending |
| PT-S36 | 36 | pending |
| PT-S37 | 37 | pending |
| PT-S38 | 38 | pending |
| PT-S39 | 39 | pending |
| PT-S40 | 40 | pending |
| PT-S41 | 41 | pending |
| PT-S42 | 42 | pending |
| PT-S43 | 43 | pending |
| PT-S44 | 44 | pending |
| PT-S45 | 45 | pending |
| PT-S46 | 46 | pending |
| PT-S47 | 47 | pending |
| PT-S48 | 48 | pending |
| PT-S49 | 49 | pending |
| PT-S50 | 50 | pending |

---

<a id="s4"></a>
## 4. Known deviations

Every Tessera difference from the original that is already decided. Only the owner moves a
`deviation (proposed)` to accepted (owner row O183, decision 0043 §1 items 4–5).

| ID | Deviation | Decision or rule | Rows it affects | State |
|---|---|---|---|---|
| KD1 | Tessera's own UI scaling instead of the original's: any resolution and aspect ratio (ultrawide and 4K included), a user scale slider, crisp text and art, panels kept on screen; the per-window scale is not built yet | [Decision 0040](../decisions/0040-ui-scaling.md), `ui-scaling.md` | PA57; the geometry of every UI row | deviation (accepted by owner, 2026-10-16, decision 0040; owner answers 2026-10-23) |
| KD2 | No client-run game code: Tessera runs none of the class-library code the original client runs locally (P7 dropped); with M8 the official server runs that code | [Decision 0024](../decisions/0024-parity-track.md) amendment | rows whose original output that code computes | deviation (accepted by owner, 2026-10-16, P7 dropped) |
| KD3 | The world walker's own clock and switch keys moved off the five keys the keymap defaults take (`/`, `,`, `.`, F2, F4 instead) | `keymap.md` §8.2, §8.3.1 | PA17, PA55 | deviation (accepted by owner, 2026-10-16) |
| KD4 | Bundled Noto Sans as the UI fallback font: text that falls back (Tessera's own text, text before an install is found) looks different | [Decision 0022](../decisions/0022-bundled-ui-font.md) | PA49 | deviation (proposed) |
| KD5 | Tessera's own clean-room plugin `class` helper (the user's installed original package wins when present); plugins relying on behaviour outside its spec may differ | [Decision 0016](../decisions/0016-plugin-class-helper.md), `plugin-class-helper.md` | PA65–PA67 | deviation (proposed) |
| KD6 | The recorded UI-drawing differences: banded first paint after 100 ms, nested parts during a resize, clipping by enclosing windows, mode-0 roots at opacity 1, no surfaces below Tile surfaces, up to one pixel per nesting level under a scale, hit tests on the new layout during a resize, parent content over a nested surface not byte-exact, plugin `.png` and `.bmp` images refused, skin images cropped above 4,096 pixels | [Decision 0015](../decisions/0015-ui-rendering.md) owner question 5 | PA47, PA52, PA53, PA66 | deviation (accepted by owner, 2026-10-06, decision 0015 owner question 5) |
| KD7 | The input known differences 1–22 of the UI input bridge (order of consultation, binding policies, key repeat, double-click timing, quick second presses and others) | [Decision 0014](../decisions/0014-ui-input-bridge.md) "Known differences from the original"; the owner accepted the record on 2026-10-06, but the differences were not put to the owner as accepted differences | PA51, PA55, PA67 | deviation (proposed) |
| KD8 | Command routing: channel words the Tessera server cannot carry answer locally; only the listed original command names are registered; unknown commands post an error line and are never sent | [Decision 0020](../decisions/0020-chat-command-routing.md) "Known differences from the original" | PA76, PA77, PA117 | deviation (proposed) |

Tessera rules marked Design (TR) in Accepted specs are not listed: they are deviations only once
the owner sees a difference ([§1.2](#s1-2)).

---

<a id="s5"></a>
## 5. Performance

| ID | What to measure | Tessera shows it with | Rules | Rolls up | State |
|---|---|---|---|---|---|
| PF1 | P8 criterion 3: the frame-time budgets in the busiest scenario, re-measured by the owner on the **idle** reference machine with `TESSERA_PERF=1` (60 Hz p99 at most 16.7 ms and 0 frames over 50 ms after the settle wait; the UI budgets U1–U5 at content scale 1; no audio underruns) | the committed `busiest` scenario and its real-data test, as named in `docs/perf/p8-busiest.md` | Decision 0043 §3, [decision 0010](../decisions/0010-reference-machine-laptop.md), `docs/perf/reference-machine.md`, decision 0015 §6 | O184 (O49 stays with M5) | pending |

Runs by agents on a loaded machine are indicative only and never change this row.

---

<a id="s6"></a>
## 6. Out of scope (no rows)

- **Server-side rules** (decision 0024 §2, `server-content.md` §7): combat outcomes, damage,
  loot tables, quest logic and state, prices, creature behaviour and spawning, experience and
  level rules, crafting outcomes, any server-side use of the data. Tessera shows what a session
  (a scenario or a server) reports and computes none of it.
- **What needs an official server** (M8, decision 0034): the launcher and login flow, world
  selection and the queue, official-server sessions, voice chat, and the in-game store with its
  store currency and purchase dialogs (six layouts of the census, PV1).
- **What a server decides:** the distance at which other players appear (O72), the range of
  local chat (O73).
- **Game-master and developer tools** (13 layouts of the census: administration, property and
  tracing tools, the string-token and browser debuggers) and the **data-patch screen** of the
  patcher (one layout), which is never a research or build target.
- **Owner rows that are not comparisons** stay with their milestones (decision 0043 §1 item 7):
  O137 (a decision), O49 (an M5 measurement).
- **Extended scripting** (M9, decision 0017) comes after P8.

---

<a id="s7"></a>
## 7. Owner log

One line per state change ([§1.1](#s1-1)). Newest last.

| Date | Row | From → to | Client build | Tessera commit | Settings, Wine/DXVK | Note (own words, no game text) |
|---|---|---|---|---|---|---|

> **OPEN:** (P8 exit verifier, clean room, 2026-10-24; for the spec writer's next revision, no
> state changed here) The Tessera-side evidence record `parity-tessera.md` found points where
> this checklist no longer matches the project, besides the three decision 0043 already lists
> (the `differs` state, owner rows O196–O200 instead of O180–O184, PF1's budgets): (1) PA2's
> command points to `USER_GUIDE.md` §4.2, which describes no import of the install's key map,
> and no command imports it; (2) PA23 and PA29 use the explorer's `--sheet` without the PNG
> path it needs; (3) PA64 (portraits) is built and drawn by the frame loop, so `not built` is
> out of date; (4) PA24's still-open list names parts built since (the post-pass systems and
> rule-15 inherited opacity); (5) KD1 says the per-window scale (`ui-scaling.md` §7.4) is not
> built, but the client's UI host now steps the group scale; (6) PA133 (Tessera's own trait
> panel, `progression-panels.md` §1.4) may belong in §4 as a proposed deviation; (7) the
> review's non-blocking notes (KD7 "except difference 21", "`particles.md` §10.2" meaning rule
> 2 of §10, "Agents set only"). Please confirm or correct each in the next revision.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-23 | First version (P8 spec package): rules, 178 system rows, the 100-row tooltip sample, eight known deviations, the performance row, the out-of-scope list; census of the install's 232 UI layouts (PV1). Status Review. |
| 2026-10-23 | Independent review (spec-review:parity-checklist:1): passed, status Accepted; review record in the header. No fact changed. |
