<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Parity checklist: Tessera-side evidence (P8 exit criterion 1)

| | |
|---|---|
| **Status** | Evidence record (decision 0043 §1 item 3). It decides nothing: only the owner sets `pass`, `differs` or an accepted deviation in [`parity.md`](parity.md); the spec writer sets `pending`, `deviation (proposed)` and `not built` there |
| **Recorded** | 2026-10-24, clean-room implementer (Clean-Room-Role: implementation), from `docs/specs/`, the decisions, the ROADMAP and the clean code only |
| **Tree** | Branch `p8` at `c8b764b` plus the uncommitted P8 work of 2026-10-23/24 in the same worktree (other packages' files were edited while these runs went on). **Not yet reproducible:** decision 0043 §1 item 3 asks for the commit that holds the evidence. When the P8 work is committed, the orchestrating session replaces this line with that commit, after re-running R1–R6 on it (or checking that none of the files they test changed after the runs) |
| **Install** | The owner's install, read-only (`TESSERA_GAME_DIR`), content fingerprint `3905db5e60168527` (`tessera-dattool detect`) |
| **Machine** | The reference laptop of decision 0010, **loaded** (other worktrees building and testing at the same time; load average 12–25). Nothing here is a timing result |
| **Checklist** | [`parity.md`](parity.md), Accepted 2026-10-23 (independent review spec-review:parity-checklist:1); one row below per row ID there, in the same order |

## How to read this

Columns: **ID** (the `parity.md` row); **Tessera side** (the command, scenario, test or
explorer view that shows the system, abbreviations as in `parity.md` §1.3); **Run here**
(what was run on 2026-10-24 and its summary line, or why it was not run); **State** and
**Notes**.

Every State cell of a system row starts with what is built (decision 0043 §1 item 3):
**built**, **partly built** or **not built**. Then come this file's own states (not
`parity.md` states):

- **data-side ready**: the system is built and reachable, and the tests that show it pass
  here. The comparison in the original client is still the owner's (`parity.md` row stays
  `pending`).
- **feed only**: the row's scenario plays on the install (`scenarios_real_data`), but no
  test run here shows the row's presentation (panel, bar, cursor or tooltip). This is not a
  gap finding: the presentation was not exercised.
- **gap**: not built, or built but not reachable or not following its Accepted spec; the
  note names the ROADMAP, `CLAUDE.md` or spec OPEN that records it, or says that nothing
  records it yet (those are listed again in [Gaps](#gaps)).
- **command wrong in parity.md**: the row's command, option or reference does not work as
  written; the note says what works.

A row can carry two states (for example "data-side ready; command wrong in parity.md").

### Runs

All runs from the worktree root with
`TESSERA_GAME_DIR="/…/Lord of the Rings Online"` (the install, read-only), release builds,
one shared build of the worktree's own target directory.

| Run | Command | Result |
|---|---|---|
| R1 | `cargo test --release -p tessera-client --no-fail-fast --test …` for every file of `apps/client/tests/` except `walk_soak_real_data` (96 targets) | 359 passed, **1 failed** (`parity_tooltips_real_data`), 0 ignored; exit 101; 30 min |
| R2 | `cargo test --release -p tessera-dattool -p tessera-explorer --no-fail-fast` | 19 targets, 312 passed, 0 failed; exit 0 |
| R3 | `cargo test --release -p tessera-client --lib` | 859 passed, 0 failed; exit 0 |
| R4 | `cargo test --release -p tessera-input -p tessera-plugins -p tessera-ui --no-fail-fast` | 1,524 passed, 0 failed, 2 ignored; exit 0 |
| R5 | `tessera-dattool detect --path`; `tessera-dattool detect`; `tessera-dattool info <install>` | exit 0 each: the install found (Steam library), 29 archives (29 ok), complete; `info` lists every archive |
| R6 | `cargo test --release -p tessera-client --test parity_tooltips_real_data -- --nocapture` (R1's failure, run again alone) | same result as in R1: 5 passed, 1 failed; exit 101 ([§3](#s3)) |
| R7 | R6 again with `TESSERA_GOLDEN_DIR` set to a fresh, empty scratch directory (2026-10-24, after a review finding) | 5 passed, 1 failed; exit 101: the same 94 records fail the panel audit (125 messages), the 6 painted tooltips are all blessed new (item-37 … item-41, item-47); item-47 passes the audit ([§3](#s3)) |
| R8 | `cargo test --release -p tessera-client --test parity_tooltips_real_data -- --nocapture` with `TESSERA_GOLDEN_DIR` set to a fresh scratch directory, after the P8 exit verification's fixes of 2026-10-24 (the examination panel fills the icon field `Icon_Base`; the worth row's coins get the coin-state images of `item-tooltips.md` §6.11 and the currencies are sorted largest first; the audit judges only elements that draw; a warm-up tooltip before the pictures) | **6 passed, 0 failed**; exit 0: 55 + 55 records resolved, 100 tooltips modelled, **100 painted, 0 failed the panel audit**, 0 cut by the screen. The default local directory was then re-blessed with `TESSERA_GOLDEN_BLESS_LOCAL=1` (decision 0031), so it now holds the 100 pictures of this sample |
| — | `tessera-client --help` (the release binary) | every option `parity.md` names exists (`--place`, `--far-band`, `--content-layers`, `--frill-distance`, `--landscape-distance`, `--object-draw-distance`, `--disable`, `--day`, `--time-of-day`, `--time-rate`, `--day-type`, `--far-view`, `--no-particles`, `--interiors`, `--walk-path`, `--avatar`, `--mount`, `--command`, `--skin`, `--ui-scale`, `--ui-safe-aspect`, `--load-plugin`, `--character-select`, `--quest-icons`, `--server`, `--account`, `--scenario`, `--scenario-input`, `--open-panel`); the feature `audio-device` exists in `apps/client/Cargo.toml` |

**Not run here** (and why): windowed runs (**S** with `--scenario-input`, **P**, **C**
windowed, the explorer's GUI): they need a person at the screen; their headless or
in-process counterparts are named per row. `walk_soak_real_data` (the 1,000-walk soak; long,
left out to keep heavy runs few). A `--features audio-device` build. `TESSERA_PERF=1`
budgets (indicative only on this machine; [§5](#s5)). Sessions of two clients on a test
server by hand (`net_server` and `social_chat_server` run two clients in process).

**Committed scenarios** (rows that name **S** *name*): `scenarios_real_data` (R1, 4 passed)
plays every committed scenario of `apps/client/tests/scenarios/` (28 files) headless twice
with the install and checks that both logs are byte-identical, that each selector resolves
on the install and that each run ends `complete`; `scenarios` (R1, 13 passed) replays their
committed synthetic logs byte for byte. Both run `--headless --scenario` **without `--ui`**,
so only the scenario driver runs (`apps/client/src/scenario/mod.rs`: no walker, no UI, no
panels). They show that the feed is deterministic and its selectors resolve on the install,
**not** the presentation. For each **S** row the presentation evidence is the in-process or
headless `--ui` test named in that row (only `scenario_ui_windows_real_data` drives
windows from a headless `--ui` scenario run, and it covers the P3 and P5 windows). A row
whose only evidence is `scenarios_real_data` is **feed only**. The windowed
`--scenario-input` run is the owner's.

**`--open-panel` names** (rows that name **P** *name*): every name `parity.md` uses is
accepted by the client (`P1`–`P15`, `character`, `bags`, the NPC, quest, P5 and P6 names).
In-process tests open the same windows by name: `npc_windows` (R1, 14 passed: every NPC
window name), `travel_windows` (R1, 2 passed), `p6_windows` (R1, 9 passed:
`open_panel_names_open_their_windows_empty`), `panels_real_data` and `ui_panels_real_data`
(R1: P1–P15 on the install), `character_panels_real_data` (R1: character panel and bags),
`scenario_ui_windows_real_data` (R1, 1 passed: a headless `--ui` scenario run drives the P3
and P5 windows). Notes: `C --help` lists the names of P1–P5 but not the P6 names
(`experience`, `traits`, `training`, `crafting`, `crafting-recipe`, `vocation-trainer`,
`profession-trainer`, `craftguild-trainer`), which the client accepts all the same
(`progression::commands`); `--open-panel map` opens the map only in windowed runs.

---

<a id="s2"></a>
## 2. System rows

<a id="s2-1"></a>
### 2.1 Install and pre-world screens

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA1 | `D detect --path`, `D info` | R5: exit 0, 29 archives; R2 dattool `real_data` 4 passed, `cli` 25 passed | built; data-side ready | |
| PA2 | **C** windowed (render preferences); keymap import | R3 (the client's `[Render]` reader, `walker_host::apply_render_preferences`, unit tests) passed; R4 `import_real_data` 7 passed. Windowed start-up not run | partly built; data-side ready (render section); **gap** (keymap import); **command wrong in parity.md** | The importer exists only as a library (`tessera_input::import::import_key_map`, tested on the install); no client or tool command imports the install's key map into a Tessera keymap (`--keymap` reads only Tessera's TOML), and `USER_GUIDE.md` §4.2 describes no import. Not recorded in the ROADMAP or a spec |
| PA3 | **P** `loading-screen`; **S** `travel` | R1 `travel_windows_real_data` 1 passed, `travel_windows` 2 passed, `scenarios_real_data` 4 passed | built; data-side ready | |
| PA4 | — | — | not built; gap | No client flow opens the story screens (`parity.md` PA4; `ui-scaling.md` §10.6 judges only their geometry) |
| PA5 | — | — | not built; gap | No spec; `parity.md` §2.14 census |
| PA6 | — | — | not built; gap | No spec; `parity.md` §2.14 census |
| PA7 | — | — | not built; gap | No spec. The walker HUD's network line (backend and session state) is a debug read-out, not the original's indicator |

<a id="s2-2"></a>
### 2.2 World

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA8 | **T** `reference_locations`; **C** `--place` | R1 `reference_locations` 1 passed, `walker_real_data` 3 passed | built; data-side ready | |
| PA9 | walker HUD at the PA8 spots | R1 `reference_locations` 1 passed, `map_lookup_real_data` 2 passed, `walker_real_data` (`place_by_map_coordinates`) passed | built; data-side ready | The HUD line gives area, map coordinates and `/loc` tokens (`walker/source.rs`); windowed spots not run |
| PA10 | **C** at the `m4-5-visual.md` §4.1 spots; F5, `--far-band` | R1 `visual_rows_real_data` 11 passed (terrain rows, row cameras render), `far_view_real_data` 1 passed | built; data-side ready | The visual-row screenshots are written only with `TESSERA_VISUAL_SHOTS` (not set here) |
| PA11 | **C** `--content-layers`; **E** Landblock tab | R2 explorer `world_real_data` 2 passed | built; data-side ready | |
| PA12 | **E** `--select DID --view mesh` (`--dye`) | R2 explorer `mesh_real_data` 1 passed; R1 `material_fallback_real_data` 2 passed, `texture_wrap_real_data` 1 passed | built; data-side ready | `--dye` takes `RRGGBB` |
| PA13 | `m4-5-visual.md` §4.2 spots; F2 | R1 `visual_rows_real_data` (`scenery_rows_place_their_objects`) passed | built; data-side ready | |
| PA14 | `m4-5-visual.md` §4.3; F4, `--frill-distance` | R1 `visual_rows_real_data` (`frill_rows_count_their_records`) passed | built; data-side ready | |
| PA15 | `m4-5-visual.md` §4.4, §4.8; F7 | R1 `visual_rows_real_data` (water and puddle rows) passed, `water_materials_real_data` 1 passed | built; data-side ready | |
| PA16 | `m4-5-visual.md` §4.5; `--time-of-day`; **E** Sky tab | R1 `visual_rows_real_data` (`sky_rows_blend_their_keyframes`) passed; R2 explorer `world_real_data` (landblocks, scenes and sky) passed | built; data-side ready | |
| PA17 | `--time-of-day`, `--time-rate`, `--day`, `--day-type`; `[`, `]` | options present (`--help`); R3 client unit tests passed; R1 `walker_cli` 2 passed | built; data-side ready | Keys as `walker.md` §14.3 (KD3) |
| PA18 | `--landscape-distance`, `--object-draw-distance`, `--disable fog` | options present; R1 `visual_rows_real_data` passed | built; data-side ready | |
| PA19 | `m4-5-visual.md` §4.6; F6, `--disable lights` | R1 `visual_rows_real_data` (`light_row_places_its_lights`) passed | built; data-side ready | |
| PA20 | **T** `lamp_lights_real_data`; `--time-of-day 0.9 --time-rate 0` | R1 `lamp_lights_real_data` 1 passed | built; data-side ready | |
| PA21 | **C** `--far-view on`; **T** `far_view_real_data` | R1 `far_view_real_data` 1 passed, `far_view_golden` 4 passed | built; data-side ready | |
| PA22 | walker; **E** 3D decal preview | R1 `particles_real_data` (`placed_decals_are_drawn_near_a_reference_location`) passed; R2 explorer `logic_views_real_data` (`decal_previews`: preview scenes of every decal choice of every 50th landblock with decals; parts, materials, collision boxes) passed | built; data-side ready | Explorer GUI not run |
| PA23 | walker; **E** `--particles DID --sheet`; **T** `particles_real_data` | R1 `particles_real_data` 3 passed, `particle_post_stage` 1 passed, `beam_visuals_real_data` 1 passed; R2 explorer `particles_real_data` 1 passed | built; data-side ready; **command wrong in parity.md** | The explorer's `--sheet` takes a PNG path (`--sheet FILE.png`); `--sheet` alone is refused |
| PA24 | — | R1 `particle_post_stage` 1 passed (post-pass systems); R3 client unit tests passed (among them `walker::objects` `placed_object_opacity_is_found_by_the_entity`, rule 15) | partly built; gap | Partly built since `parity.md` was written: post-pass systems are drawn, the weather systems are placed (`walker/effects/weather.rs`), and opacity inherited from the owner is applied (`particle-runtime.md` rule 15: `inherit_opacity` systems of a figure take the character layer's fade, those of a placed object the object layer's draw-distance fade or cross-fade, `ObjectLayer::placed_object_opacity`; both one frame late; module docs of `walker/effects/mod.rs`). Still not built: rain streaks and snow flakes and the other drop-fed spawns (`particle-runtime.md` §11.2.3 step 4, Q-RT13), the figure effects of §11.2.5, fork effects (`particles.md` §10 rule 2). Recorded: `CLAUDE.md` particle bullet (which still lists `inherit_opacity` as waiting), the module docs of `weather.rs` |
| PA25 | **C** `--interiors`; **T** `interiors_real_data` | R1 `interiors_real_data` 2 passed, `cell_looks_real_data` 1 passed | built; data-side ready | |
| PA26 | **C** `--walk-path docs/checklists/m4-5-visual.md` | R1 `walk_checklist_real_data` 2 passed (every walk of the checklist), `walk_synthetic` 26 passed, `walk_path_format` 9 passed | built; data-side ready | The soak (`walk_soak_real_data`) was not run here |
| PA27 | walker, `/loc` timing; **T** `jump_script_real_data` | R1 `jump_script_real_data` 1 passed | built; data-side ready | |
| PA28 | walker | R3 client unit tests passed (the walk camera, `walker/controls.rs`: smoothing as half-lives, `walker.md` §14.11) | built; data-side ready | Windowed camera not run |

<a id="s2-3"></a>
### 2.3 Characters and animation

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA29 | **E** `--character SPECIES,SEX --sheet` | R2 explorer `character_real_data` 3 passed; R1 `figure_shape_real_data` 5 passed, `chargen_bodies_real_data` 2 passed | built; data-side ready; **command wrong in parity.md** | `--sheet` takes a PNG path, as in PA23 |
| PA30 | **E** `--character … --sliders HEX`; creation screen | R1 `figure_shape_real_data` 5 passed, `chargen_viewport_real_data` 1 passed | built; data-side ready | |
| PA31 | **E** `--character`; **T** `chargen_bodies_real_data` | R1 `chargen_bodies_real_data` 2 passed | built; data-side ready | |
| PA32 | **C** `--avatar "BODY;ITEM;…"`; **E** `--wear DID` | R1 `worn_look_real_data` 2 passed; R2 explorer `character_real_data` passed | built; data-side ready | |
| PA33 | `--avatar "…;DID@wielded"` | option syntax present (`--help`); R2 explorer `character_real_data` passed | built; data-side ready | No dedicated held-item test in these runs; windowed spots not run |
| PA34 | **E** `--character … --clip idle\|walk\|run`; walker | R2 explorer `character_real_data` passed (clip names `idle`, `walk`, `run` accepted) | built; data-side ready | |
| PA35 | two clients on a test server | R1 `net_server` 6 passed (in-process server, two clients: movement and chat; no test there names emotes or stances) | partly built (`tessera-net` only); gap | `tessera-net` carries the stance and emote of remote figures (`EntityState { stance, emote }` in `Event::EntityStateChanged`), but `apps/client/src` never handles `EntityStateChanged`, so no remote figure shows a server stance or emote; the client never sends `Command::Emote` either (`/emote` only posts a chat line). Emote coverage exists only in the `tessera-net`, `tessera-protocol` and `tessera-server` tests, which R1–R4 did not run. **Not recorded** in the ROADMAP, `CLAUDE.md` or a spec OPEN. Two windowed clients not run |
| PA36 | walker at the `m4-visual.md` §N spots; **T** `npc_spots_real_data` | R1 `npc_spots_real_data` 1 passed | built; data-side ready | |
| PA37 | walker (`m4-5-visual.md` §4.6 L5 note) | no dedicated test; R1 `npc_spots_real_data` passed | built; data-side ready | Windowed spot not run |
| PA38 | **E** Character tab, drama player | R2 explorer `logic_views_real_data` 4 passed, `gameplay_real_data` 2 passed | built; data-side ready | GUI not run |
| PA39 | **E** mount viewer; **C** `--mount` | R1 `mount_walk_real_data` 7 passed, `mount_frame_log_real_data` 3 passed; R2 explorer `logic_views_real_data` passed | built; data-side ready | |
| PA40 | two clients on `tessera-server` | R1 `net_server` 6 passed | built; data-side ready | |

<a id="s2-4"></a>
### 2.4 Audio and player music

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA41 | `--features audio-device`; **T** `audio_real_data` | R1 `audio_real_data` 3 passed (area music, borders, avatar scripts) | built; data-side ready | The device build was not made here |
| PA42 | **T** `audio_interiors_real_data`; walker with audio | R1 `audio_interiors_real_data` 1 passed | built; data-side ready | |
| PA43 | walker with audio near scripted objects | R1 `audio_real_data` passed | built; data-side ready | No dedicated object-sound test in these runs |
| PA44 | game UI with audio | — | partly built (`tessera-ui` only); gap | `tessera-ui` raises its UI sound event (`UiEvent::Sound`, `ui-layout.md` §6.9), but the client consumes no such event, so no UI sound plays; the chat guard's sound waits for `chat-window.md` §6.6 (Q13). Not recorded in the ROADMAP or `CLAUDE.md` |
| PA45 | **C** `--command "/play NAME"`; **T** `player_music_real_data` | R1 `player_music_real_data` 2 passed, `chat_commands` 1 passed | built; data-side ready | |

<a id="s2-5"></a>
### 2.5 UI framework

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA46 | **P** `P1` … `P15`; **T** `ui_panels_real_data` | R1 `ui_panels_real_data` 1 passed, `panels_real_data` 3 passed, `panels_synthetic` 5 passed, `panels_review` 9 passed | built; data-side ready | |
| PA47 | `m5-visual.md` §D; own test plugin | R1 `ui_render` 1 passed, `plugins_suite` 3 passed; R4 `tessera-ui` paint tests (`paint_compose` 14, `paint_scheduler` 27, `paint_real_data` 1, `ui_surfaces` 6) passed | built; data-side ready | KD6 |
| PA48 | panels in use; **T** `ui_review_real_data` | R1 `ui_review_real_data` 1 passed, `ui_review` 7 passed; R4 `widget_examples` 16, `behaviour_examples` 22 passed | built; data-side ready | |
| PA49 | every panel | R4 `text_real_data` 1, `text_fallback` 8, `text_examples` 20, `text_review` 8 passed | built; data-side ready | KD4 |
| PA50 | hover with **C** `--ui` | R4 `behaviour_examples` 22 passed | built; data-side ready | Windowed hover not run |
| PA51 | **S** `inventory-panels` | R4 `game_items` 16 passed (`drag_between_slots_onto_the_paperdoll_and_out`, `drop_on_the_bag_window_outside_the_list_is_the_bag_area`); R1 `character_panels_real_data` 1 passed (a backpack item dragged onto the chest slot sends `EquipItem`), `scenarios_real_data` passed (feed only) | built; data-side ready | KD7. `ui_host_drag_placements` (cited before) drags a panel, not an item, so it is evidence for PA52 only |
| PA52 | drag and resize, restart | R1 `ui_host_placements` 4 passed, `ui_host_drag_placements` 2 passed; R4 `ui_scaling_live` 42 passed | built; data-side ready | |
| PA53 | **C** `--skin NAME`; **T** `ui_skin_real_data` | R1 `ui_skin_real_data` 1 passed; R4 `skin_real_data` 2, `skin_art` 16 passed | built; data-side ready | |
| PA54 | **C** `--ui`, `--ui-scale` | R1 `ui_host_wiring` 10 passed (cursor size, `ui-scaling.md` §6.7) | built; data-side ready | |
| PA55 | keymap file; **T** `routed_keys_real_data` | R1 `routed_keys_real_data` 1 passed; R4 `ui_bridge` 39 passed | built; data-side ready | KD7 |
| PA56 | Escape in **C** `--ui` | R1 `panels_real_data` (`escape_chat_and_minimap_on_the_install`) passed; R4 `escape` 6, `ui_escape` 7 passed | built; data-side ready | |
| PA57 | `--ui-scale`, `--ui-safe-aspect`, `/uiscale` | R1 `ui_scale_real_data` 1 passed, `ui_interface_window` 2 passed; R4 `ui_scaling_goldens` 24, `ui_scaling_invariants` 25 passed | built; data-side ready | Accepted deviation KD1 |
| PA58 | quest abandon in **S** `quest-log`; vendor quantity in **S** `vendor` | R1 `ui_host_dialogs` 6 passed (modal dialogs in the client UI host: Enter answers, keys kept from game code); R4 `dialogs_menus` 34, `dialogs_menus_real_data` 2 passed; R1 `scenarios_real_data` passed (feed only) | built; data-side ready | The presentation evidence is the in-process dialog tests; the quest-abandon and vendor-quantity dialogs of the two scenarios were not driven through `--ui` here |
| PA59 | tracker menu in **S** `quest-log`; map menu in **S** `map` | R4 `dialogs_menus` 34 passed (placement, submenus, open delay, fallback menu); R1 `scenarios_real_data` passed (feed only) | built; data-side ready | The presentation evidence is `dialogs_menus`; the tracker and map menus of the two scenarios were not driven through `--ui` here |
| PA60 | **P** `P6`; Escape | R1 `panels_real_data` 3 passed | built; data-side ready | |
| PA61 | **P** `P7` | R1 `ui_review_real_data` (opens P7) passed | partly built; gap | Only the layout and the Interface window's scale controls (`ui-layout.md` §17.5 OPEN (d), `ui-scaling.md` §7.2) |
| PA62 | **C** `--ui`; **P** `P3`, `P14` | R1 `panels_real_data` passed, `combat_ui_real_data` 1 passed | built; data-side ready | |
| PA63 | **P** `P13` | R1 `panels_real_data` passed | built; data-side ready | |
| PA64 | **T** `combat_portraits_real_data` | R1 `combat_portraits_real_data` 1 passed, `combat_portraits` 3 passed | built; data-side ready | `parity.md` says `not built`, which is out of date: the client's windowed and headless frame loops now draw the portraits (`ClientUi::render_combat_portraits`, `apps/client/src/portrait/`). Still open there: the per-entity viewport settings of `ui-widgets.md` §12.1 step 3 (module docs of `portrait/mod.rs`). For the spec writer: move the row to `pending` |

<a id="s2-6"></a>
### 2.6 Plugins

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA65 | `--load-plugin NAME`; **P** `P12`; `plugins_suite` | R1 `plugins_suite` 3 passed; R4 `tessera-plugins` `plugins` 22, `host` 25 (2 ignored), `class_helper` 44 passed | built; data-side ready | KD5 |
| PA66 | own test plugin; the census plugins | R4 `ui_classes` 23, `ui_controls` 28, `styled_real_data` 1, `compat_plugins` 12 passed | built; data-side ready | KD6 |
| PA67 | **T** `plugins_suite` | R1 `plugins_suite` 3 passed; R4 `ui_input` 21, `ui_lifetime` 14 passed | built; data-side ready | |
| PA68 | **S** `inventory` with a test plugin; **T** `plugin_inventory_checks` | R1 `plugin_inventory_checks` 9 passed; R4 `inventory_views` 15 passed | built; data-side ready | |
| PA69 | **T** `plugin_combat_checks` | R1 `plugin_combat_checks` 12 passed; R4 `combat_views` 24 passed | built; data-side ready | |
| PA70 | **T** `npc_windows_plugins` | R1 `npc_windows_plugins` 5 passed; R4 `wallet_views` 13 passed | built; data-side ready | |
| PA71 | **S** `fellowship` with a test plugin | R4 `party_views` 29 passed; R1 `scenarios_real_data` passed | built; data-side ready | The windowed run with a user test plugin was not made |
| PA72 | **T** `mount_plugin_views` | R1 `mount_plugin_views` 1 passed; R4 `mount_views` 8 passed | built; data-side ready | |
| PA73 | **T** `p6_plugin_views`, `p6_host_plugin_views` | R1 both 1 passed; R4 `progression_views` 20 passed | built; data-side ready | |
| PA74 | — | — | not built; gap | `lua-plugin-api.md` §15, §16, §17.3 (views not specified) |

<a id="s2-7"></a>
### 2.7 Chat and commands

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA75 | **C** `--ui` | R4 `game_chat` 21 passed | built; data-side ready | Windowed checks at scales 1.0, 1.5, 0.5 not run |
| PA76 | typing in **C** `--ui` | R4 `game_chat` 21 passed; R1 `chat_commands` 1 passed | built; data-side ready | KD8 |
| PA77 | `--command "/help"`; **T** `chat_commands` | R1 `chat_commands` 1 passed (`/help` is a registered built-in) | built; data-side ready | KD8 |

<a id="s2-8"></a>
### 2.8 P1: character screens, panels, bags and item tooltips

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA78 | **C** `--character-select` | R1 `character_screens_real_data` 6 passed, `pre_world_real_data` 1 passed, `character_screens` 14 passed, `character_screens_server` 1 passed | built; data-side ready | |
| PA79 | **C** `--character-select`, create | R1 `character_screens_real_data` passed, `chargen_bodies_real_data` 2 passed, `character_screens_ui` 3 passed | built; data-side ready | |
| PA80 | **C** `--character-select`, appearance stage | R1 `chargen_viewport_real_data` 1 passed, `character_screens_real_data` passed | built; data-side ready | Open parts: `character-screens.md` §2.1.4 OPEN |
| PA81 | C key; **P** `character`; **S** `inventory-panels` | R1 `character_panels_real_data` 1 passed, `scenarios_real_data` passed | built; data-side ready | Open: empty stat rows on this install (`character-panels.md` §3.4) |
| PA82 | I key; **P** `bags`; **S** `backpack-growth` | R1 `character_panels_real_data` passed, `scenarios_real_data` passed | built; data-side ready | |
| PA83 | **S** `inventory-panels` | R4 `game_items` 16 passed (drag onto the paperdoll and out, `right_click_activates`); R1 `character_panels_real_data` 1 passed (drag equips: `EquipItem`), `scenarios_real_data` passed (feed only) | built; data-side ready (equip, unequip, use); feed only (split, destroy) | No test run here names splitting a stack or destroying an item; those two are exercised only as feed events |
| PA84 | **S** `inventory`; **T** `worn_look_real_data` | R1 `worn_look_real_data` 2 passed | built; data-side ready | |
| PA85 | hover items; **T** `item_tooltips_real_data`; **E** tooltip preview | R1 `item_tooltips_real_data` 1 passed; R4 `game_examine` 21, `game_p1_real_data` 1 passed; R2 explorer `gameplay_real_data` 2 passed | built; data-side ready | The 50-item sample is [§3.1](#s3-1) |
| PA86 | hover with an item equipped; H | R4 `game_examine` 21 passed | built; data-side ready | Windowed hover not run |

<a id="s2-9"></a>
### 2.9 P2: targeting, quickslots, cast bars, vitals, floating text, effects, defeat

Shared evidence for PA87–PA95: R1 `combat_ui_real_data` 1 passed, `combat_ui_golden` 2
passed, `combat_server` 2 passed, `scenarios_real_data` (scenario `combat-ui`) passed; R4
`game_combat` 43 passed. The windowed `p2-visual.md` commands were not run.

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA87 | **S** `combat-ui` | shared evidence; R1 `npc_use` 6 passed (selection use) | built; data-side ready | |
| PA88 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA89 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA90 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA91 | **S** `combat-ui`; keys 1–0, −, = | shared evidence; R1 `plugin_combat_checks` 12 passed | built; data-side ready | |
| PA92 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA93 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA94 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA95 | **S** `combat-ui` | shared evidence | built; data-side ready | |
| PA96 | hover a quickslot; **T** `combat_tooltips_real_data` | R1 `combat_tooltips_real_data` 1 passed; R4 `game_p2_real_data` 1 passed | built; data-side ready | The 50-skill sample is [§3.2](#s3-2) |
| PA97 | hover an effect icon | R1 `combat_tooltips_real_data` passed (effect tooltips) | built; data-side ready | |
| PA98 | **T** `combat_visuals_real_data`; `p2-visual.md` C4 | R1 `combat_visuals_real_data` 2 passed, `beam_visuals_real_data` 1 passed | built; data-side ready | |
| PA99 | `p2-visual.md` C4 | R1 `combat_visuals_real_data` passed | built; data-side ready | C4 windowed not run |

<a id="s2-10"></a>
### 2.10 P3: dialogue, quest log and tracker, vendors, loot, wallet

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA100 | U, double click; **S** `dialogue` | R1 `npc_use` 6 passed, `npc_windows_real_data` (`use_cylinders_on_the_install`) passed | built; data-side ready | |
| PA101 | **S** `dialogue`; **P** `conversation`, `choice`, `npc-comment` | R1 `npc_windows_real_data` 6 passed, `npc_windows` 14 passed; R4 `p3_npc_windows` 33 passed | built; data-side ready | |
| PA102 | **S** `dialogue`, `npc-windows` | R1 `npc_windows` passed, `scenarios_real_data` passed | built; data-side ready | |
| PA103 | **S** `vendor`; **P** `vendor`, `vendor-quantity` | R1 `npc_windows_real_data` (`vendor_on_the_install`) passed | built; data-side ready | |
| PA104 | **P** `barter` | R1 `npc_windows` passed; R4 `p3_goldens` 13 passed | built; data-side ready | |
| PA105 | **S** `loot`; **P** `loot` | R1 `npc_windows_real_data` (`loot_on_the_install`) passed | built; data-side ready | |
| PA106 | **P** `wallet` | R1 `npc_windows_real_data` (`wallet_on_the_install`) passed, `routed_keys_real_data` passed | built; data-side ready | |
| PA107 | **P** `wallet`, `vendor` | R1 `npc_windows_real_data` passed | built; data-side ready | |
| PA108 | L; **S** `quest-log`; **P** `quest-log` | R1 `quest_journal_real_data` 2 passed, `quest_journal` 3 passed, `quest_hub` 4 passed; R4 `p3_quest_journal` 19 passed | built; data-side ready | |
| PA109 | **S** `quest-log`; **P** `tracker` | R1 `quest_journal_real_data` passed, `quest_ui_rules` 10 passed | built; data-side ready | |
| PA110 | **S** `quest-log`; `--quest-icons on` | R1 `quest_rules_real_data` 2 passed, `quest_ui_rules` passed | built; data-side ready | |

<a id="s2-11"></a>
### 2.11 P4: fellowship, trade, friends and ignore lists, tells, channels, synchronised music

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA111 | **S** `music-sync` | R1 `social_music_sync` 7 passed | built; data-side ready | |
| PA112 | **S** `fellowship` | R1 `social_panels_real_data` 2 passed; R4 `game_social` 30, `p4_goldens` 5, `game_social_real_data` 1 passed | built; data-side ready | |
| PA113 | **S** `fellowship` | as PA112 | built; data-side ready | |
| PA114 | **S** `trade` | as PA112 | built; data-side ready | |
| PA115 | **S** `friends`; **P** `P8` | as PA112; R1 `panels_real_data` passed | built; data-side ready | |
| PA116 | two clients on a test server; **S** `social` | R1 `social_chat_server` 1 passed; R4 `game_chat_social` 20 passed | built; data-side ready | |
| PA117 | test server (`C --server …`) | R1 `social_chat_server` passed | built; data-side ready | KD8 |
| PA118 | **P** `P8` | — | partly built (friends and ignore pages); gap | `social-panels.md` §3.1, §9 TR8 (pages shown empty) |

<a id="s2-12"></a>
### 2.12 P5: maps, travel, instance finder, mounts

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA119 | M; **P** `map`; **S** `map` | R1 `map_census_real_data` 1 passed, `map_windows` 7 passed; R4 `p5_map_windows` 27 passed | built; data-side ready | `--open-panel map` acts only in windowed runs |
| PA120 | **S** `map` | R1 `map_windows` passed, `scenario_ui_windows_real_data` 1 passed | built; data-side ready | |
| PA121 | **S** `map`; walk into a dungeon | R1 `map_census_real_data` passed, `panels_real_data` (`interior_cells_have_dungeon_maps`) passed | built; data-side ready | |
| PA122 | **C** `--ui`; **P** `P1` | R1 `panels_real_data` (`escape_chat_and_minimap_on_the_install`) passed | built; data-side ready | |
| PA123 | **S** `map` | R1 `map_lookup_real_data` 2 passed | built; data-side ready | |
| PA124 | **S** `travel`; **P** `travel` | R1 `travel_windows_real_data` 1 passed, `travel_windows` 2 passed; R4 `p5_travel_windows` 17 passed | built; data-side ready | |
| PA125 | **S** `travel`; **P** `end-travel` | as PA124 | built; data-side ready | |
| PA126 | **S** `instances`; **P** `instance-finder` | R1 `instance_finder_real_data` 1 passed, `instance_finder_windows` 3 passed | built; data-side ready | |
| PA127 | **S** `instances`; **P** `queue-alert`, `instance-travel` | as PA126 | built; data-side ready | |
| PA128 | **C** `--mount …`; **S** `mount`; **T** `mount_walk_real_data` | R1 `mount_walk_real_data` 7 passed | built; data-side ready | |
| PA129 | **C** `--mount steed:medium` | R1 `mount_walk_real_data` (`a_war_steed_shifts_through_every_gait`) passed, `mount_frame_log_real_data` 3 passed | built; data-side ready | |
| PA130 | Ctrl+M, `/ride`; **S** `mount` | R1 `mount_walk_real_data` (`summoning_and_mounting_follow_the_install_timeline`) passed | built; data-side ready | |

<a id="s2-13"></a>
### 2.13 P6: experience, traits, skill training, crafting, gathering

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA131 | **P** `experience`; **S** `progression-panels` | R1 `p6_windows` 9 passed; R4 `p6_progression` 8, `p6_real_data` 3 passed | built; data-side ready | |
| PA132 | **S** `progression-panels` | R4 `p6_progression` 8 passed (`experience_bar_fill_and_texts`, `levelup_banner_on_a_virtual_clock`); R1 `p6_panels` 4 passed (`the_experience_bar_shows_the_raised_level`), `p6_windows` 9 passed (experience bar beside the toolbar meter), `scenarios_real_data` passed (feed only) | built; data-side ready (banner, bar reset); feed only (effect, sound, chat lines) | No test run here shows the level-up effect, sound or chat lines |
| PA133 | J; **P** `traits`; **S** `traits` | R1 `p6_windows` passed, `p6_panels` 4 passed; R4 `p6_goldens` 7 passed | built; data-side ready | Proposed deviation, not yet in `parity.md` §4: Tessera's point-based trait panel instead of the original's usual specialisation view (`progression-panels.md` §1.4, answer PP19–PP21: "a deviation for the P8 parity checklist") |
| PA134 | **S** `traits` | R4 `p6_progression` 8 passed (`trait_panel_cells_and_requests`, `trait_grid_holds_every_cell_position`), `p6_goldens` 7 passed; R1 `scenarios_real_data` passed (feed only) | built; data-side ready (trait slots); feed only (trait tooltips) | The in-process trait-panel tests give their cells an empty tooltip; the client's trait tooltip text (`progression/present.rs` `trait_tooltip_text`, from `tessera_gamedata` `trait_tooltip`) is not shown by any test run here |
| PA135 | **P** `training`; **S** `progression-panels` | R1 `p6_windows` passed | built; data-side ready | |
| PA136 | K | R1 `routed_keys_real_data` passed (the key routes) | not built (skill panel); gap | The skill panel is neither specified nor built. `progression-panels.md` §1.4 (Accepted; answer PP19) sets the Tessera rule "`toggle_skills` opens nothing" until a revision describes the panel; the client still toggles the skill-training window on K (`progression/ui.rs` `toggle_training`), so the code does not follow the Accepted rule (`progression/hub.rs` routes `RoutedCommand::Skills` to `toggle_training`). The passing test `p6_windows::the_trait_and_skill_toggles_open_and_close_their_windows` (R1) asserts the retired behaviour ("K opens the skill-training window", "K again closes it") and must change together with `hub.rs`. Not recorded in the ROADMAP; `CLAUDE.md` (P6 bullet) still describes K as the skill-training toggle |
| PA137 | T; **P** `crafting`; **S** `crafting` | R4 `p6_crafting` 23 passed; R1 `p6_windows` passed, `scenarios_real_data` passed | built; data-side ready | |
| PA138 | **S** `crafting` | as PA137 | built; data-side ready | |
| PA139 | **P** `crafting-recipe`; **S** `crafting` | as PA137 | built; data-side ready | |
| PA140 | **S** `crafting`, `progression-panels` | as PA137 | built; data-side ready | |
| PA141 | hover a recipe scroll in **S** `crafting` | R1 `item_tooltips_real_data` passed (every item record of the install; it does not name recipe scrolls separately); R4 `p6_real_data` 3 passed, `p6_crafting` 23 passed | built; data-side ready | Windowed hover not run |
| PA142 | **P** `vocation-trainer`, `profession-trainer`, `craftguild-trainer` | R1 `p6_windows` passed (opens `vocation-trainer`) | built; data-side ready | |
| PA143 | **S** `gathering` | R1 `scenarios_real_data` passed (feed only) | partly built (feed actors on the P3 use path; the rest not checked here); feed only | Gathering nodes are feed actors used through the P3 use path (`progression/mod.rs` module docs, `crafting-panels.md` §9.1 Tessera rule); `npc_use` (R1) covers that use path in general, but no test run here shows a gathering node's highlight, cursors, gathering bar, loot or removal |
| PA144 | T, J, K, `toggle_crafting_1` … `3` | R1 `routed_keys_real_data` 1 passed, `p6_windows` 9 passed (`the_crafting_toggle_opens_and_closes_the_panel`, `the_trait_and_skill_toggles_open_and_close_their_windows`); R4 `ui_bridge` 39 passed | built; data-side ready (T, J, `toggle_crafting_1` … `3`); **gap** (K, see PA136) | The K half of `p6_windows::the_trait_and_skill_toggles_open_and_close_their_windows` asserts the retired K behaviour (PA136) |

<a id="s2-14"></a>
### 2.14 Systems of the original client that Tessera has not built or not specified

Every row of `parity.md` §2.14 is a **gap** on Tessera's side: nothing in the clean code
builds these panels, and no Accepted spec describes them (except the partial references in
the notes). They are recorded by `parity.md` §2.14 itself (the install's layout census) and
by no ROADMAP milestone.

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PA145 | — | — | not built; gap | No spec |
| PA146 | — | — | not built; gap | `game-panels.md` §5 (highlighters hidden) |
| PA147 | — | — | not built; gap | No spec |
| PA148 | — | — | not built; gap | No spec |
| PA149 | — | — | not built; gap | No spec |
| PA150 | — | — | not built; gap | No spec |
| PA151 | — | — | not built; gap | No spec |
| PA152 | — | — | not built; gap | No spec |
| PA153 | — | — | not built; gap | No spec |
| PA154 | — | — | not built; gap | No spec |
| PA155 | — | — | not built; gap | No spec (also PA74 for the plugin views) |
| PA156 | — | — | not built; gap | `character-panels.md` names the page only |
| PA157 | — | — | not built; gap | No spec |
| PA158 | — | — | not built; gap | No spec |
| PA159 | — | — | not built; gap | `ui-scaling.md` §10.8(a) (geometry only) |
| PA160 | — | — | not built; gap | No spec |
| PA161 | — | — | not built; gap | No spec |
| PA162 | — | — | not built; gap | No spec |
| PA163 | — | — | not built; gap | No spec |
| PA164 | — | — | not built; gap | No spec |
| PA165 | — | — | not built; gap | `combat-ui.md` (pips hidden) |
| PA166 | — | — | not built; gap | No spec |
| PA167 | — | — | not built; gap | No spec |
| PA168 | — | — | not built; gap | No spec |
| PA169 | — | — | not built; gap | `social-panels.md` §9 TR8 (page empty) |
| PA170 | — | — | not built; gap | No spec |
| PA171 | — | — | not built; gap | No spec |
| PA172 | — | — | not built; gap | No spec |
| PA173 | — | — | not built; gap | No spec |
| PA174 | — | — | not built; gap | No spec |
| PA175 | — | — | not built; gap | No spec |
| PA176 | — | — | not built; gap | No spec (`particle-runtime.md` notes the effect-filter page is in no milestone) |
| PA177 | — | — | not built; gap | No spec |
| PA178 | — | — | not built; gap | `ui-behaviour.md` §8.5 (OPEN) |

---

<a id="s3"></a>
## 3. Tooltip sample: 50 items and 50 skills

**Test:** `apps/client/tests/parity_tooltips_real_data.rs` with the committed scenario
`apps/client/tests/scenarios/parity-tooltips.scenario` (selectors only). R1, R6 and R7 (before
the exit verification's fixes) **failed**: 94 tooltips failed the panel audit. **R8 passes**
(all six tests). Every ordinal's tooltip model is **built** (0 model failures) and every
painted panel passes the audit; the comparison with the original client is the owner's
(O197, O198).

| Step | Result (R8) |
|---|---|
| Resolve | 55 distinct item records, 55 distinct skill records (the first 50 of each are the sample) |
| Play through the feed | 55 `ItemAdded`, 50 `QuickslotChanged` (50 distinct), reason `complete`, 0 skipped |
| Model | 100 tooltips (44 item and 50 skill inputs with an icon or image row), 0 categorised without a tooltip, 0 failures; every record's category is `Rendered` (3 skills with lookup misses, noted in the index) |
| Panel audit and paint | **100 painted, 0 failed the panel audit**, 0 cut by the screen |

What R1–R7 found and how it was fixed (exit verification, 2026-10-24):

- **No examination icon in any tooltip.** The model gives the icon field `Icon_Base`
  (`item-tooltips.md` §4.2, a field holding the `ExaminationIcon`) as a list of the field's
  filled children; the panel only filled lists from entry templates, so the field was
  measured empty and removed. `tessera-ui` now fills a field's own children
  (`ExamTooltips` `fill_held`; regression test `an_icon_field_entry_fills_the_icon_it_holds`).
- **Worth coins without an image.** The client's conversion gave no coin image; it now gives
  each coin the state image of `MoneyDisplay_Icon` in the examination coin element
  (`item-tooltips.md` §6.11 answer; `tessera_gamedata::tooltip::layout::coin_icon_images`),
  and the model sorts the currency directory's coins largest first as the §6.11 order caveat
  says (regression test `worth_coins_sort_a_property_set_currency_list_largest_first`).
- **Elements outside the frame.** One was an empty field of a pair row's template that draws
  nothing (the audit now judges only elements that draw an image or a text); the other came
  from the first tooltip being built while its fonts still loaded (the panel then measures
  texts with the elements' design sizes). The test warms up with one tooltip; the
  first-hover layout is a client defect recorded in the P8 status.

Visible in the pictures, for the owner's comparison (spot checks, not a full review): a skill
tooltip shows a stray `..` row (skill-01), an item tooltip shows an enum label instead of a
text on a bonus line (item-10), and skill tooltips leave an empty band under the name row.

**Local output** (decision 0031 tier 2, never committed; the index holds game text):
`${XDG_CACHE_HOME:-~/.cache}/tessera/goldens/tessera-client/parity_tooltips_real_data/fifty_item_and_fifty_skill_tooltips_are_painted/3905db5e60168527/`
with `index.txt` (ordinal, family, DID, category, name) and `item-NN.png` / `skill-NN.png`.
Re-blessed after R8 (`TESSERA_GOLDEN_BLESS_LOCAL=1`): it holds the 100 pictures of this sample
for the owner's comparison (O197/O198); the stale pictures of the pre-revision sample are
gone.

<a id="s3-1"></a>
### 3.1 Items (PT-I1 to PT-I50)

| ID | Ordinal | Run here (R8) | State |
|---|---|---|---|
| PT-I1 | 1 | painted, panel audit passed (R8) | data-side ready |
| PT-I2 | 2 | painted, panel audit passed (R8) | data-side ready |
| PT-I3 | 3 | painted, panel audit passed (R8) | data-side ready |
| PT-I4 | 4 | painted, panel audit passed (R8) | data-side ready |
| PT-I5 | 5 | painted, panel audit passed (R8) | data-side ready |
| PT-I6 | 6 | painted, panel audit passed (R8) | data-side ready |
| PT-I7 | 7 | painted, panel audit passed (R8) | data-side ready |
| PT-I8 | 8 | painted, panel audit passed (R8) | data-side ready |
| PT-I9 | 9 | painted, panel audit passed (R8) | data-side ready |
| PT-I10 | 10 | painted, panel audit passed (R8) | data-side ready |
| PT-I11 | 11 | painted, panel audit passed (R8) | data-side ready |
| PT-I12 | 12 | painted, panel audit passed (R8) | data-side ready |
| PT-I13 | 13 | painted, panel audit passed (R8) | data-side ready |
| PT-I14 | 14 | painted, panel audit passed (R8) | data-side ready |
| PT-I15 | 15 | painted, panel audit passed (R8) | data-side ready |
| PT-I16 | 16 | painted, panel audit passed (R8) | data-side ready |
| PT-I17 | 17 | painted, panel audit passed (R8) | data-side ready |
| PT-I18 | 18 | painted, panel audit passed (R8) | data-side ready |
| PT-I19 | 19 | painted, panel audit passed (R8) | data-side ready |
| PT-I20 | 20 | painted, panel audit passed (R8) | data-side ready |
| PT-I21 | 21 | painted, panel audit passed (R8) | data-side ready |
| PT-I22 | 22 | painted, panel audit passed (R8) | data-side ready |
| PT-I23 | 23 | painted, panel audit passed (R8) | data-side ready |
| PT-I24 | 24 | painted, panel audit passed (R8) | data-side ready |
| PT-I25 | 25 | painted, panel audit passed (R8) | data-side ready |
| PT-I26 | 26 | painted, panel audit passed (R8) | data-side ready |
| PT-I27 | 27 | painted, panel audit passed (R8) | data-side ready |
| PT-I28 | 28 | painted, panel audit passed (R8) | data-side ready |
| PT-I29 | 29 | painted, panel audit passed (R8) | data-side ready |
| PT-I30 | 30 | painted, panel audit passed (R8) | data-side ready |
| PT-I31 | 31 | painted, panel audit passed (R8) | data-side ready |
| PT-I32 | 32 | painted, panel audit passed (R8) | data-side ready |
| PT-I33 | 33 | painted, panel audit passed (R8) | data-side ready |
| PT-I34 | 34 | painted, panel audit passed (R8) | data-side ready |
| PT-I35 | 35 | painted, panel audit passed (R8) | data-side ready |
| PT-I36 | 36 | painted, panel audit passed (R8) | data-side ready |
| PT-I37 | 37 | painted, panel audit passed (R8) | data-side ready |
| PT-I38 | 38 | painted, panel audit passed (R8) | data-side ready |
| PT-I39 | 39 | painted, panel audit passed (R8) | data-side ready |
| PT-I40 | 40 | painted, panel audit passed (R8) | data-side ready |
| PT-I41 | 41 | painted, panel audit passed (R8) | data-side ready |
| PT-I42 | 42 | painted, panel audit passed (R8) | data-side ready |
| PT-I43 | 43 | painted, panel audit passed (R8) | data-side ready |
| PT-I44 | 44 | painted, panel audit passed (R8) | data-side ready |
| PT-I45 | 45 | painted, panel audit passed (R8) | data-side ready |
| PT-I46 | 46 | painted, panel audit passed (R8) | data-side ready |
| PT-I47 | 47 | painted, panel audit passed (R8) | data-side ready |
| PT-I48 | 48 | painted, panel audit passed (R8) | data-side ready |
| PT-I49 | 49 | painted, panel audit passed (R8) | data-side ready |
| PT-I50 | 50 | painted, panel audit passed (R8) | data-side ready |

<a id="s3-2"></a>
### 3.2 Skills (PT-S1 to PT-S50)

| ID | Ordinal | Run here (R8) | State |
|---|---|---|---|
| PT-S1 | 1 | painted, panel audit passed (R8) | data-side ready |
| PT-S2 | 2 | painted, panel audit passed (R8) | data-side ready |
| PT-S3 | 3 | painted, panel audit passed (R8) | data-side ready |
| PT-S4 | 4 | painted, panel audit passed (R8) | data-side ready |
| PT-S5 | 5 | painted, panel audit passed (R8) | data-side ready |
| PT-S6 | 6 | painted, panel audit passed (R8) | data-side ready |
| PT-S7 | 7 | painted, panel audit passed (R8) | data-side ready |
| PT-S8 | 8 | painted, panel audit passed (R8) | data-side ready |
| PT-S9 | 9 | painted, panel audit passed (R8) | data-side ready |
| PT-S10 | 10 | painted, panel audit passed (R8) | data-side ready |
| PT-S11 | 11 | painted, panel audit passed (R8) | data-side ready |
| PT-S12 | 12 | painted, panel audit passed (R8) | data-side ready |
| PT-S13 | 13 | painted, panel audit passed (R8) | data-side ready |
| PT-S14 | 14 | painted, panel audit passed (R8) | data-side ready |
| PT-S15 | 15 | painted, panel audit passed (R8) | data-side ready |
| PT-S16 | 16 | painted, panel audit passed (R8) | data-side ready |
| PT-S17 | 17 | painted, panel audit passed (R8) | data-side ready |
| PT-S18 | 18 | painted, panel audit passed (R8) | data-side ready |
| PT-S19 | 19 | painted, panel audit passed (R8) | data-side ready |
| PT-S20 | 20 | painted, panel audit passed (R8) | data-side ready |
| PT-S21 | 21 | painted, panel audit passed (R8) | data-side ready |
| PT-S22 | 22 | painted, panel audit passed (R8) | data-side ready |
| PT-S23 | 23 | painted, panel audit passed (R8) | data-side ready |
| PT-S24 | 24 | painted, panel audit passed (R8) | data-side ready |
| PT-S25 | 25 | painted, panel audit passed (R8) | data-side ready |
| PT-S26 | 26 | painted, panel audit passed (R8) | data-side ready |
| PT-S27 | 27 | painted, panel audit passed (R8) | data-side ready |
| PT-S28 | 28 | painted, panel audit passed (R8) | data-side ready |
| PT-S29 | 29 | painted, panel audit passed (R8) | data-side ready |
| PT-S30 | 30 | painted, panel audit passed (R8) | data-side ready |
| PT-S31 | 31 | painted, panel audit passed (R8) | data-side ready |
| PT-S32 | 32 | painted, panel audit passed (R8) | data-side ready |
| PT-S33 | 33 | painted, panel audit passed (R8) | data-side ready |
| PT-S34 | 34 | painted, panel audit passed (R8) | data-side ready |
| PT-S35 | 35 | painted, panel audit passed (R8) | data-side ready |
| PT-S36 | 36 | painted, panel audit passed (R8) | data-side ready |
| PT-S37 | 37 | painted, panel audit passed (R8) | data-side ready |
| PT-S38 | 38 | painted, panel audit passed (R8) | data-side ready |
| PT-S39 | 39 | painted, panel audit passed (R8) | data-side ready |
| PT-S40 | 40 | painted, panel audit passed (R8) | data-side ready |
| PT-S41 | 41 | painted, panel audit passed (R8) | data-side ready |
| PT-S42 | 42 | painted, panel audit passed (R8) | data-side ready |
| PT-S43 | 43 | painted, panel audit passed (R8) | data-side ready |
| PT-S44 | 44 | painted, panel audit passed (R8) | data-side ready |
| PT-S45 | 45 | painted, panel audit passed (R8) | data-side ready |
| PT-S46 | 46 | painted, panel audit passed (R8) | data-side ready |
| PT-S47 | 47 | painted, panel audit passed (R8) | data-side ready |
| PT-S48 | 48 | painted, panel audit passed (R8) | data-side ready |
| PT-S49 | 49 | painted, panel audit passed (R8) | data-side ready |
| PT-S50 | 50 | painted, panel audit passed (R8) | data-side ready |

---

<a id="s4"></a>
## 4. Known deviations

| ID | Tessera side | State | Notes |
|---|---|---|---|
| KD1 | `ui-scaling.md` built (rows PA57; R1 `ui_scale_real_data`, `ui_interface_window`; R4 `ui_scaling_*`) | built; data-side ready | Accepted by the owner; the per-window scale (`ui-scaling.md` §7.4) is now built in the client's UI host: `ui_window_scale_up`/`_down` step the group scale (`ui_host/mod.rs` `step_window_scale`, unit test `per_window_scale_follows_the_group`, R3). `parity.md` KD1's "the per-window scale is not built yet" is out of date ([Gaps](#gaps), corrections) |
| KD2 | No client-run game code | not built (by decision); data-side ready | Accepted by the owner (P7 dropped) |
| KD3 | Walker keys `[`, `]`, `/`, `,`, `.`, F2, F4 (`walker.md` §14.2–§14.3) | built; data-side ready | Accepted by the owner; `keymap.md` §8.3.1 invariant tested in `tessera-input` (R4) |
| KD4 | Bundled fallback font (R4 `text_fallback` 8 passed) | built; data-side ready | Proposed; owner row O199 (`parity.md` cites O183) |
| KD5 | Clean-room class helper (R4 `class_helper` 44 passed) | built; data-side ready | Proposed; O199 |
| KD6 | UI-drawing differences of decision 0015 | built; data-side ready | Accepted by the owner |
| KD7 | Input differences of decision 0014 (R4 `ui_bridge` 39 passed) | built; data-side ready | Proposed; O199 |
| KD8 | Command routing of decision 0020 (R1 `chat_commands` passed) | built; data-side ready | Proposed; O199 |

**Proposed by this record** (for the spec writer to list in `parity.md` §4 if it agrees):

- **PA133:** the trait panel is Tessera's point-based presentation of the class-3486 trees,
  where the original's usual trait window is the specialisation view. Source:
  `progression-panels.md` §1.4, answer PP19–PP21 (Accepted), which calls it "a deviation for
  the P8 parity checklist".

---

<a id="s5"></a>
## 5. Performance

| ID | Tessera side | Run here | State | Notes |
|---|---|---|---|---|
| PF1 | `apps/client/tests/scenarios/busiest.scenario`, its real-data test `apps/client/tests/p8_busiest_real_data.rs`, the commands and the indicative numbers in [`docs/perf/p8-busiest.md`](../perf/p8-busiest.md) | R1 `p8_busiest_real_data` 4 passed: the scenario has the decision-0043 minimums, its timing options combine, it plays deterministically with the install; `busiest_scenario_frame_budgets` asserts nothing without `TESSERA_PERF=1` (not set here). The windowed paced and unpaced runs were not repeated here | built; data-side ready | Every number on this loaded machine is **indicative** (decision 0010, `reference-machine.md` rule 2); `p8-busiest.md` records the perf package's runs. The criterion is the owner's re-measure on the idle reference machine (O200; `parity.md` cites O184) |

---

<a id="gaps"></a>
## Gaps

Rows whose Tessera side is a gap, with what records them. "Not recorded" means that no
ROADMAP entry, `CLAUDE.md` note or spec OPEN names the gap yet; those are new findings of this
record and are reported to the orchestrating session.

| Row | Gap | Recorded by |
|---|---|---|
| PA2 | No command imports the install's key map (the importer is a library only); `USER_GUIDE.md` §4.2 describes no import (command wrong in `parity.md`) | **Not recorded** |
| PA4–PA7 | Story screens, intro movies and movie player, credits, disconnected screen and connection indicator | `parity.md` PA4–PA7; `ui-scaling.md` §10.6 (geometry only) |
| PA24 | Rain and snow drawing and the drop-fed spawns, the §11.2.5 figure effects, fork effects (post-pass systems, weather placement and rule-15 inherited opacity are built) | `particle-runtime.md` §11.2.3 (Q-RT13), §11.2.5; `particles.md` §10 rule 2; `CLAUDE.md` particle bullet |
| PA35 | Stances and emotes of remote figures: the client ignores `Event::EntityStateChanged` and never sends `Command::Emote` (`/emote` is chat only) | **Not recorded** |
| PA44 | UI sounds: `tessera-ui` raises them, the client plays none | **Not recorded** (`chat-window.md` §6.6 Q13 covers only the chat guard's sound) |
| PA61 | Options window pages | `ui-layout.md` §17.5 OPEN (d) |
| PA74 | Plugin views beyond §15.1–§15.6 | `lua-plugin-api.md` §15, §16, §17.3 |
| PA118 | Social panel pages other than friends and ignore | `social-panels.md` §3.1, §9 TR8 |
| PA136, PA144 (K) | The player's skill panel is neither specified nor built; K still toggles the skill-training window against the Accepted Tessera rule ("opens nothing"); `p6_windows::the_trait_and_skill_toggles_open_and_close_their_windows` asserts the retired behaviour and must change together with `progression/hub.rs` | `progression-panels.md` §1.4 (answer PP19) records the panel as unspecified; **the code's non-conformance is not recorded** (`CLAUDE.md` P6 bullet still describes K as the training toggle) |
| PA145–PA178 | The 34 systems of the layout census that Tessera has not built or specified | `parity.md` §2.14 |

Row notes that are not gaps but correct `parity.md`:

- PA64 is built (portraits drawn by the frame loop), so its `not built` state is out of date.
- PA24: built since `parity.md` was written are the post-pass systems (`particle_post_stage`
  passes) and the opacity inherited from the owner (`particle-runtime.md` rule 15: a figure's
  fade and a placed object's draw-distance fade, one frame late; `walker/effects/mod.rs`).
- KD1: the per-window scale (`ui-scaling.md` §7.4, `ui_window_scale_up`/`_down`) is now
  built in the client UI host; the clause "the per-window scale is not built yet" is out of
  date.
- PA23 and PA29: the explorer's `--sheet` needs a PNG path.
- `parity.md` cites owner rows O180–O184; `owner-observations.md` numbers them O196–O200
  (decision 0043 already lists this for the next revision).

Not part of `parity.md`, found on the way: `tessera-client --help` does not list the P6
`--open-panel` names.

## Change log

| Date | Change |
|---|---|
| 2026-10-24 | First version (clean-room implementer, P8): evidence for the 178 system rows, the 100 sample rows, the 8 known deviations and PF1, from runs R1–R6 on branch `p8` (`c8b764b` plus uncommitted P8 work) with the install `3905db5e60168527` |
| 2026-10-24 | Review fixes (clean-room implementer, P8): PA35 is a gap (the client ignores remote stances and emotes; `net_server` tests none); `scenarios_real_data` is feed evidence only, new state **feed only** (PA83 split/destroy, PA132 effect/sound/chat, PA134 tooltips, PA143), item rows PA51/PA83 cite `game_items`; PT-I47 passes the audit (R7) and its local golden is from the pre-revision sample, so 94 gap rows; PA24 inherited opacity (rule 15) is built; PA22 cites `decal_previews`; PA144 K is a gap and `p6_windows` asserts the retired K behaviour; KD1 correction listed; every State cell names built, partly built or not built; the Tree line says the evidence commit is still owed |
| 2026-10-24 | P8 exit verification: run R8 after the fixes of the examination panel (icon fields), the worth-row coin images and currency order, and the audit; the tooltip sample passes (100 painted, 0 audit failures), the 100 PT rows are data-side ready, and the default local golden directory is re-blessed |
