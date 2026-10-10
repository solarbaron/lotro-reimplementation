<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera user guide

This guide is for testing Tessera on your own install of the game: how to build and start
the three programs, the options worth knowing, the controls, where Tessera keeps its files,
and how to report a problem. Every option named here exists in the programs' own `--help`
output; run that for the complete, current list.

Tessera is not playable yet. The client is mainly an offline **world walker**: you walk or
fly over the world as an animated avatar, among the characters the data places, with no
combat. It can also join Tessera's own test server (not the game's servers) and attach an
early game UI with Lua plugins (§3.12). Scenario files (§3.11) script gameplay events for
testing; they play back what the file says and compute nothing.

---

## 1. Requirements

- **Your own, legally obtained install** of the game. Tessera reads its `.dat`/`.datx`
  archives in place and read-only. It never writes, renames, locks or patches anything in
  the install directory, and every program refuses to save its own output (screenshots,
  extracted records, snapshots, keymaps, settings) inside it.
- **Rust**, through `rustup`. The repository's `rust-toolchain.toml` pins the exact
  toolchain, and `rustup` installs it on first use. Without `rustup`, the minimum versions
  are 1.90 for the workspace and 1.95 for the explorer.
- **A GPU driver that wgpu supports**: Vulkan on Linux and Windows, Metal on macOS, DX12
  on Windows. On Linux, also the usual windowing libraries (Wayland or X11, and
  libxkbcommon).
- **Memory and disk**: by default the walker keeps up to 4 GiB of landblock data resident
  and uses about 1.5 GiB of GPU memory for objects (see `--residency-mib` and `--gpu-mib`
  in §3.8 to lower both). Build in release mode; debug builds are far too slow to walk in.
- Optional: `xvfb-run`, to take explorer screenshots on a machine without a display.

Build everything once:

```sh
cargo build --release --workspace
```

The programs then live in `target/release/` as `tessera-client`, `tessera-explorer` and
`tessera-dattool`. The examples below use `cargo run --release -p <program> -- <options>`,
which builds when needed; calling the binaries directly works the same way.

`tools/dev-setup.sh` checks a machine and reports what is missing
([DEVELOPMENT.md](DEVELOPMENT.md)).

---

## 2. Finding your install

Tessera looks for the install directory (the one holding `client_general.dat`) in this
order:

1. the `--game-dir DIR` option of the program you run;
2. the `TESSERA_GAME_DIR` environment variable;
3. the explorer only: the directory saved in Tessera's settings (the explorer saves it;
   §7);
4. automatic detection: Steam libraries (native, Flatpak, Proton), launchers and common
   Wine prefixes. The explorer's headless `--sheet` mode (§5.3) skips this step, so give
   it `--game-dir` or `TESSERA_GAME_DIR` unless the explorer has saved a directory.

To see what detection finds:

```sh
cargo run --release -p tessera-dattool -- detect
```

`detect` prints every install it finds with its fingerprints (archive versions and a
content fingerprint); `--schema-only` skips reading the housekeeping records that the
content fingerprint needs. To fix the choice for the rest of a shell session:

```sh
export TESSERA_GAME_DIR="$(cargo run -q --release -p tessera-dattool -- detect --path)"
```

`detect --path` prints only the default install's directory and exits with status 1 when
there is none. Tests that use real data read only `TESSERA_GAME_DIR` and skip themselves
when it is unset; the programs also use detection.

---

## 3. The world walker (`tessera-client`)

```sh
cargo run --release -p tessera-client
```

With an install, the client opens a window at the default spawn (landblock `1,126,140`)
in walk mode. Without an install, or with `--no-data`, it shows an original test scene
instead. `--showcase` shows the early texture showcase instead of the walker.

### 3.1 General options

| Option | Meaning |
|---|---|
| `--game-dir DIR` | The install directory (otherwise `TESSERA_GAME_DIR`, then detection). |
| `--no-data` | Do not look for an install; run the test scene only. |
| `--size WIDTHxHEIGHT` | Window or offscreen size, default `1280x720`. |
| `--no-vsync` | Present without vsync. |
| `--no-entry-count` | Skip counting every archive's entries at start-up (faster start-up log). |
| `--data-dir DIR` | Tessera's own data folder (§7). |
| `--keymap FILE` | Keymap file to use instead of the default one (§4). |

### 3.2 Where you start

| Option | Meaning |
|---|---|
| `--spawn R,X,Y` | Start at the centre of landblock (X, Y) of region R. `R,X,Y,x,y` adds a block-local position in metres, `R,X,Y,x,y,z` an exact height. |
| `--place NAME` | Start at a named place: an area, territory or region name, map coordinates such as `12.3S, 45.6W`, or a `/loc` line pasted from the original client. Names match loosely (case, accents and partial words). Cannot be combined with `--spawn`. |
| `--region N` | Region (1 to 15, default 1) for `--place` names and map coordinates, and for `--fly-path` points that do not name one. |
| `--heading DEG` | Facing at the spawn as a compass heading (0 = north, 90 = east). |
| `--pitch DEG` | Camera pitch at the spawn, −89 to 89 (negative looks down; default −14). |
| `--fly` | Start in free-fly mode instead of walking. |
| `--fly-speed M/S` | Base free-fly speed, default 60, at most 2,000. |
| `--eye-height M` | Height of the fly camera above the spawn, default 30 m. |

The HUD shows the area name, map coordinates, the `/loc` reading and the landblock you
are in, so a spot you find can be given back to `--place` or `--spawn` later.

### 3.3 Your avatar

`--avatar SPEC` sets the avatar's body and what it wears and holds. The default is
`human,m`.

```
SPEC = BODY ;ITEM ;ITEM …
BODY = SPECIES,SEX     a playable body: a label or key from the data's race table, then
                       m, male, f, female or a numeric key
     | DID             a placed figure: an entity (0x47…) or game object (0x70… to 0x76…)
ITEM = DID[@dye=CODE][@dye2=CODE][@wielded]     item DIDs 0x70… to 0x76… or 0x79…
```

Held items are stowed unless marked `@wielded` (`@stowed` and `@sheathed` are accepted
and mean the default). Dye codes are numbers from 0 to 1. At most 32 items. Example:

```sh
cargo run --release -p tessera-client -- --avatar "elf,f;0x79006BF8@wielded"
```

Use the explorer's character viewer (§5.2) to find bodies, item DIDs and dye codes before
putting them on the avatar.

### 3.4 Characters around you

| Option | Meaning |
|---|---|
| `--crowd N` | Place N animated characters (every playable body in turn) on a grid in front of the spawn, standing, walking and running. Up to 1,024; default 0. |
| `--crowd-spacing M` | Grid spacing, 0.5 to 100 m, default 3. |
| `--crowd-ahead M` | Distance of the first row in front of the spawn, 0 to 1,000 m, default 8. |
| `--content-layers LIST` | Content layers treated as active, comma-separated keys (decimal or `0x` hex), default `1`. |
| `--anim-distance M` | Characters farther than this keep their last pose, default 200 m. |
| `--character-distance M` | Cap on the characters' draw distance, default 400 m. |
| `--markers` | Also draw weenie markers (housing hooks, distant-battle units) as static figures. |
| `--interiors` | Also draw interior (cell) entities. |
| `--autorun walk` or `--autorun run` | Move the avatar forward on its own from the start. |

### 3.5 Time of day and sky

| Option | Meaning |
|---|---|
| `--time-of-day T` | Time the game clock starts at, 0 to 1 (0.5 is noon, the default). |
| `--time-rate RATE` | Game seconds per real second, default 1; 0 stops the clock; at most 100,000. |
| `--day N` | Game day number the clock starts on, default 12,240. |
| `--day-type D` | Day type, default 2. |

For example `--time-of-day 0.9 --time-rate 0` holds the scene at night, and
`--time-rate 600` runs a whole game day in a few minutes.

### 3.6 Scripted flights and timing runs

These end the run by themselves, so they can run without a window (§3.7).

| Option | Meaning |
|---|---|
| `--fly-path FROM,TO` | Fly a straight line, then exit. Each end is `X:Y`, `R:X:Y`, `X:Y:x:y` or `R:X:Y:x:y` (landblock, optional block-local offset in metres). The camera waits at the start until the area has loaded. |
| `--speed M/S` | `--fly-path` speed, default 1,000. |
| `--fly-height M` | `--fly-path` height above the terrain, default 120. |
| `--camera-loop LOOP` | Fly the camera around a closed loop: `R:X:Y:x,y;x,y[;…]`, block-local waypoints in landblock (X, Y) of region R. Use with `--duration`. Not with `--fly-path`. |
| `--loop-speed`, `--loop-height`, `--loop-pitch` | Loop speed (default 6.5 m/s), height above the higher of terrain and water (default 2 m) and pitch (default −10°). |
| `--turn-rate DEG` | Turn the camera on the spot at this many degrees per second (clockwise) during a timing run. |
| `--duration SECS` | Run this many seconds after the spawn area has loaded, with a frame log, then exit. |
| `--settle-timeout SECS` | Longest wait for the area to load before a screenshot, flight or timing run starts anyway (default 90). |
| `--unpaced` | Headless runs render as fast as possible instead of at 60 frames per second. |
| `--frame-log CSV` | Write per-frame times, residency and counters as CSV when the walker exits; a summary (longest frame, percentiles, peaks) goes to the log. |

```sh
cargo run --release -p tessera-client -- --fly-path 1:100:100,1:140:100 --headless \
    --frame-log ~/tessera-runs/flight.csv
```

### 3.7 Headless runs and screenshots

| Option | Meaning |
|---|---|
| `--screenshot PNG` | Render the spawn offscreen once everything around it has loaded (or `--settle-timeout` passed), save it as PNG and exit. Implies `--headless`. |
| `--headless` | Run without a window: the walker offscreen, or for `--scenario` without `--screenshot`, `--place` or a height-less `--spawn` (always without an install) only the scenario driver, with no walker and no GPU. Needs one of `--screenshot`, `--fly-path`, `--duration`, `--ui-script` (§3.12), `--walk-path` or `--scenario` (§3.11) to end the run. |
| `--no-hud` | Hide the HUD and the minimap (clean screenshots). |
| `--smoke` | Initialise everything, render a few frames of the simple test scene (a texture from your install on a quad; `--surface DID` picks which) offscreen, then exit: a quick check that the GPU path works. `--frames N` (default 8), `--data-timeout SECS` (default 120) and `--smoke-out FILE` (write the last frame as a PPM image) go with it. |
| `--exit-after-frames N` | Windowed: close the window after N frames. |

```sh
cargo run --release -p tessera-client -- --place "12.3S, 45.6W" --heading 90 \
    --time-of-day 0.3 --no-hud --screenshot ~/tessera-shots/morning.png
```

In a windowed run, F12 or Print Screen saves a screenshot to `screenshots/` in Tessera's
data folder (§7).

### 3.8 Distances, quality and budgets

| Option | Meaning |
|---|---|
| `--landscape-distance LEVEL` | `VeryLow`, `Low`, `Medium`, `High` (built-in default) or `VeryHigh`, as in the original client's graphics settings. |
| `--object-draw-distance LEVEL`, `--model-detail LEVEL` | `Low`, `Medium`, `High` (built-in default), `VeryHigh` or `UltraHigh`. |
| `--frill-distance LEVEL` | Grass and small-plant distance, 0 to 5 (default 2). |
| `--view-blocks N` | Terrain view distance in landblocks, 2 to 30 (default: from the landscape distance). |
| `--object-distance M` | Cap on every object's draw distance (default: the landscape extent). |
| `--far-band M` | Terrain farther than this is drawn with its texture layers baked into one image (default 480; 0 draws every layer everywhere). |
| `--texture-cap N` | Largest object texture edge, 16 to 4,096 (default 512). |
| `--residency-mib N` | Memory budget for loaded landblocks, default 4,096 MiB. |
| `--gpu-mib N` | GPU memory for object meshes and textures, default 1,536 MiB. |
| `--disable LIST` | Switch systems off for comparison or profiling, comma-separated: `layers`, `lit`, `shadows`, `lights`, `water`, `sky`, `fog`, `scenery`, `instancing`, `frills`, `lod`, or `all`. |

If the walker stutters or runs out of memory, lower `--landscape-distance` or
`--residency-mib` first. `--disable` is the quickest way to tell which system a visual
problem comes from.

### 3.9 Scripted walks

`--walk-path FILE` runs scripted walks on the collision geometry, headless and without a
GPU, then exits with a non-zero status if any walk failed its expected outcome. `FILE` is
either one walk in the `key = value` form or a Markdown file holding a walk table; the
format, with an example, is in
[checklists/m4-5-visual.md §2.1](checklists/m4-5-visual.md#s2-1), and running
that checklist file itself runs all of its walks. `--walk-log CSV` writes one row per
1/60 s step (position, movement state, cell, contacts). `--settle-timeout` is the
longest wait for a walk's collision data.

```sh
cargo run --release -p tessera-client -- --walk-path docs/checklists/m4-5-visual.md \
    --walk-log ~/tessera-runs/walks.csv
```

### 3.10 Plugins

The client hosts Lua plugins from `Plugins/` in Tessera's data folder (or `--plugins DIR`,
read-only). `--load-plugin NAME` loads one by its descriptor name after discovery
(repeatable). Plugin support is at an early stage.

<a id="scenarios"></a>
### 3.11 Scenario runs

A **scenario file** scripts what a game world would tell the client: characters, actors,
items, vitals, casts, effects, dialogue, quest states, vendor and loot lists, map markers,
chat lines and the like, as timed steps and as scripted answers to your commands ("when the
player targets the signpost, say …"). The client's offline sandbox plays it back as the same
events a server would send, so the gameplay panels of later milestones can be tested without
a server. It is a test tool, not a game: every number and outcome comes from the file, and
nothing is computed from the game's rules.

| Option | Meaning |
|---|---|
| `--scenario FILE` | Play FILE with the sandbox. Cannot be combined with `--server` (§3.12), `--walk-path`, `--fly-path`, `--duration`, `--smoke`, `--showcase` or `--ui-script`. Works with `--no-data`, `--game-dir`, `--spawn`, `--place` and `--heading`; `--place` needs an install (with `--no-data` the run uses the default spawn and logs a chat notice). |
| `--scenario-log FILE` | Write the run's event log to FILE (`-` for standard output). Needs `--scenario` (refused with exit code 2 otherwise). FILE is created or overwritten without asking; the scenario file's own path is refused (exit code 2). A log can grow very large (below). |
| `--scenario-input` | Windowed runs: also play the file's `input` block (headless runs always play it). Each line is sent when the scenario's clock reaches it, the `at pre` lines before the world is entered. |

**Headless runs.** `--headless --scenario FILE` needs nothing else to end the run: the
scenario runs on its own virtual clock, as fast as it can, to its end, then the client
exits. Without an install (`--no-data`, or none found) it needs no window and no GPU. The
same file, the same install (or none) and the same options give the same log, byte for
byte, on every run, so a log is a good regression check.

```sh
cargo run --release -p tessera-client -- --headless --no-data \
    --scenario my-test.scenario --scenario-log -
```

| Exit status | Meaning |
|---|---|
| 0 | The scenario finished |
| 1 | Another client failure |
| 2 | Command-line error (for example a scenario file that does not exist) |
| 3 | The file has an error; the message names `FILE:LINE:COLUMN` |
| 4 | Its selectors could not be resolved (for example `requires install` without an install, or an install you named that cannot be read) |
| 5 | The run failed (a run-time error in the scenario, the log could not be written, or the session ended early) |

**Where the player stands.** Without `--spawn` or `--place`, a headless scenario run puts
the player at a fixed spot (region 0, the origin, facing east) whatever the install, so no
terrain has to load. A scenario that needs real terrain should give an exact position with a
height, `--spawn R,X,Y,x,y,z`. `--place` and `--spawn` without a height wait for the area to
load, and such runs are not guaranteed to give the same log every time. With an install,
`--screenshot PNG` captures the frame after the scenario's end (it is refused with
`--no-data`).

**Windowed runs.** Without `--headless`, the walker plays the scenario in real time. A HUD
line shows its state, and when it ends the world stays open. Logs of windowed runs carry
wall-clock times, so they differ from run to run. The log is written in the background, so a
slow disk or a reader of `--scenario-log -` that stops reading (such as `less`) never freezes
the window; if the writing falls too far behind, the log stops with `error=LogWrite` and the
client exits with status 5. The `input` block is played only with `--scenario-input`; a file
whose `enter` character only an `accept-character` step adds needs it (or `--headless`), and
without it such a run is refused with exit status 2.

**Log size.** A scenario file is at most 1 MiB, and its log at most 256 MiB: a file built
to make a larger one (a trigger that answers many input lines with long chat lines) ends the
run at that size with exit status 5, the log's last line saying `error=LogOverflow`.

**The file at a glance.** A UTF-8 text file, by convention `NAME.scenario`. The first line
that is not blank or a comment is `tessera-scenario 1`; `#` starts a comment, so a file may
begin with its SPDX header comment. Settings (`title`, `tick`, `end-at`, `enter`,
`defaults`, `requires install`) come first, then declarations (`player`, `character`,
`select`, `actor`, `item`, `page`, `option`) and step lines. A step line starts with `at TIME`
(absolute) or `+TIME` (after the previous line). `on PATTERN … end` answers a command you
send, `repeat N every TIME … end` repeats steps, and `input … end` lists the commands a
headless run (or a windowed one with `--scenario-input`) sends for you. This original example plays without an install:

```
tessera-scenario 1
title "Garden test"
tick 100ms
player "Wren"
actor $post kind=object name="Signpost" pos=0,5,0
select $herbs = item kind=general n=3 seed=1

at 0s spawn $post
+0s vital player health 500/500
+0s say $post "Welcome to the garden."
+1s give $herbs
at 2s damage none player 40 left=460

on target $post
+0s say $post "Read me."
end

input
at 1s target $post
end
```

The complete format (every step, trigger and command, the limits and the log format) is in
[`docs/specs/scenario-feed.md`](specs/scenario-feed.md); the scenarios in
`apps/client/tests/scenarios/` show every group of steps.

**Selectors.** A scenario never names game records by number. A `select` line describes
them instead, for example `select $swords = item kind=weapon n=3 seed=7` (three weapons,
picked with seed 7), `select $quests = quest all` or `select $skills = skill class=@Label
n=5 seed=7`. Steps use the whole set (`give $herbs` gives every element) or one element as
a record reference: `quest $quests[0] active`, or, for items, a declaration first
(`item $sword $swords[0]`, then `give $sword`; `give $swords[0]` is an error). The
client resolves selectors against your own install before the run starts; the log's
`resolve` lines say how many records each one found. Without an install every selector
resolves empty and steps that need one of its elements are skipped, while literal strings
(`"Quick jab"`) work everywhere, so a scenario built from literals plays fully with
`--no-data`. A file with `requires install` refuses to run without one (exit status 4).

**Rules for scenario files you share or commit.** Shared scenarios must work for everyone's
install and must not carry game content:

- **Selectors only.** No record IDs: no `select … = did …` lines and no attribute value that
  is a record ID. Enum values (in filters, in the `class=`, `species=` and `sex=` options,
  in `slot=` of `equip` and in `type=` of `damage` and `heal`) are written as labels
  (`@Label`) that an Accepted spec names, as Tessera's own keywords, or as `m` / `f`;
  numeric enum values are for local files only, so `equip $x slot=3` cannot be committed.
  Container slot numbers (`give`, `unequip` and `move-item … slot=`), quickslot and trait
  slot numbers, entry and option numbers are plain numbers and are fine.
- **Original text only.** No names, dialogue, quest, item or place text from the game; every
  literal string is your own. Committed scenarios are licensed GPL-3.0-or-later or CC0-1.0
  and start with the header comment `# SPDX-License-Identifier: GPL-3.0-or-later` (or
  `CC0-1.0`); the committed-scenario test checks it.
- **Keep the rest local.** A scenario that names record IDs you picked (the `did` selector
  family exists for that) or copies text from your install stays on your machine, outside
  the repository or git-ignored. So does every log of a run with an install: it lists record
  IDs from your install and may hold its text. Only `install none` logs of synthetic
  scenarios may be committed, and a test replays them.

<a id="server-ui"></a>
### 3.12 Test server and game UI

These parts are early; `tessera-client --help` lists every option that goes with them.

| Option | Meaning |
|---|---|
| `--server HOST:PORT` | Play on a **Tessera test server** (`tessera-server`, Tessera's own protocol; never the game's servers) instead of the built-in sandbox. Also takes a connect string `tessera://HOST:PORT[?pin=sha256:…]`. Needs `--account NAME`; `--character NAME` picks the character (default: the account's first). The password comes from `TESSERA_PASSWORD` or a prompt, never from the command line. The server places the character, so `--spawn`, `--place` and `--crowd` do not apply. |
| `--server-fingerprint FP`, `--trust-new-server`, `--known-servers FILE` | How the server's key is trusted: a pinned fingerprint, or the known-servers file next to Tessera's settings file; `--trust-new-server` records an unknown key on first use (check the fingerprint the client logs with the server's operator first). |
| `--ui` | Attach the game UI: panels, chat, plugin windows. `--open-panel NAME`, `--skin NAME`, `--ui-script` and `--ui-report FILE` imply it. |
| `--ui-script` | Run the scripted UI measurement (implies `--ui`); with `--headless` it runs offscreen and ends the run. `--ui-report FILE` writes its report as Markdown. |

`--showcase` (§3) shows the early texture showcase instead of the walker.

---

## 4. Controls and the keymap file

### 4.1 Default controls (walker)

| Key | Walk mode | Fly mode |
|---|---|---|
| W / Up, S / Down | forward, backward | forward, backward |
| A / Left, D / Right | turn left, right | turn left, right |
| Q, E | strafe left, right | strafe left, right |
| Left and right mouse buttons together | run forward | forward |
| Right mouse button, held | look around (mouselook) | look around |
| Mouse wheel, Page Up, Page Down | camera distance (1.5 to 60 m) | flying speed (wheel) |
| Space | jump | up |
| C | | down |
| Shift, held | walk instead of run | five times faster |
| Alt, held | | five times slower |
| R, Num Lock | autorun | |
| Home | reset the camera | reset the camera |
| Ctrl+F | switch to fly mode | switch to walk mode |
| F3 | HUD details on or off | HUD details on or off |
| F12, Print Screen | screenshot | screenshot |
| Escape | quit | quit |

A gamepad works too: left stick moves, right stick looks, south button jumps, the left
stick's click toggles autorun, and the shoulder buttons move up and down in fly mode.

### 4.2 The keymap file

Tessera reads its own keymap file in TOML. Its location is `$TESSERA_KEYMAP_FILE` when
set, else `keymap.toml` next to the settings file (on Linux
`~/.config/tessera/keymap.toml`), or the file given with `--keymap FILE`. It holds only
**your changes**; everything it does not mention keeps the built-in default, and a
missing file means all defaults. Nothing is written to it automatically.

The file maps actions to lists of bindings, grouped by context:

```toml
format = "tessera-keymap"
version = 1

[options]
sequence_timeout_ms = 1000

[bindings.movement]
move_forward = ["W", "Up", { input = "S", value = -1.0 }, { axis = "PadLeftY", deadzone = 0.15 }]
autorun = []                  # an empty list unbinds the action

[bindings.global]
screenshot = ["F11"]
```

- **Contexts and actions.** `global` (`menu`, `screenshot`, `toggle_fly`, `toggle_debug`,
  `open_chat`), `camera` (`mouselook`, `look_x`, `look_y`, `zoom`, `reset_camera`),
  `movement` (`move_forward`, `strafe`, `turn`, `jump`, `walk`, `autorun`, `mouselook`),
  `fly` (`fly_vertical`, `fly_boost`, `fly_slow`, `fly_speed`) and `text_entry`.
- **Keys** are named by their position on a US layout (`W` is the key that types W on a US
  keyboard, whatever your layout), with punctuation written as words (`Comma`, `Slash`).
  Mouse buttons are `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`,
  `MouseForward`; wheel clicks `WheelUp`, `WheelDown`; pad buttons `PadSouth`,
  `PadEast`, `PadL1` and so on.
- **Combinations**: modifiers and up to four buttons joined with `+` (`Ctrl+F`,
  `MouseLeft+MouseRight`); sequences of up to four strokes separated by spaces
  (`Ctrl+K Ctrl+W`), each within the sequence timeout of the previous one.
- **Axes**: `{ axis = "PadLeftY", scale = 1.0, deadzone = 0.15, invert = false }` for
  sticks, triggers, mouse motion and the wheel; `{ input = "S", value = -1.0 }` gives a
  key a negative direction on an axis action.

A broken entry is skipped with a warning in the log and the default stays in force, so a
typo never leaves you without controls. The complete reference, with every key name and
limit, is [specs/keymap.md](specs/keymap.md) §2–§8.

---

## 5. The explorer (`tessera-explorer`)

```sh
cargo run --release -p tessera-explorer
```

The explorer is a read-only browser for the archives. On the left are the archive tree
and the list of records (by DID, grouped by type); in the middle, tabs that show the
selected record in every view that applies to it.

| Tab | Shows |
|---|---|
| Info | Entry, archive and routing information. |
| 3D | Meshes, setups, templates, entities, appearances and objects with their materials: orbit camera, wireframe, normals, UV checker, material inspector, dye preview. |
| Landblock | A landblock: terrain map, generated scenery, statics, cells, water and a collision summary. |
| Sky | Sky and day descriptions with the blended timeline over the game day. |
| Character | Character viewer and animation browser (§5.2). |
| Texture | Surfaces and texture lists, with mip levels, channels and alpha. |
| Strings | String tables with their variables. |
| Properties | Property sets with names and enum labels. |
| Icon | Layered icons. |
| Record | Decoded tables (DID maps, enum mappers, the property dictionary, lists, sounds). |
| Hex | Hex dump of the raw record. |

**Navigating.** Ctrl+F jumps to the search box, which takes a DID, a hex prefix or a
name. Up and Down step through the record list; Alt+Left and Alt+Right (or the mouse's
back and forward buttons) go back and forward in the history. In the 3D view, drag to
orbit, drag with the right or middle button (or Shift) to pan, use the wheel to zoom;
F frames the model, R resets the angles, W toggles the wireframe, and a double-click
reframes.

The explorer remembers the game directory, its window and other state in Tessera's
settings file (§7); `--no-persist` turns that off for one run.

### 5.1 Start-up options

| Option | Meaning |
|---|---|
| `--game-dir DIR` | The install directory (otherwise `TESSERA_GAME_DIR`, the saved one, then detection). |
| `--language NAME` | Language of the string tables, as in `client_local_NAME.dat` (default English). |
| `--select DID` | Select this record at start-up (8 hex digits, with or without `0x`). |
| `--view VIEW` | Tab to open for `--select`: `info`, `hex`, `texture`, `strings`, `props`, `icon`, `record`, `mesh`, `character`, `landblock` or `sky`. |
| `--search QUERY` | Run this search at start-up. |
| `--width W --height H` | Initial window size (give both). |
| `--hide-panels` | Hide the archive tree, the record list and the character inspector. |
| `--font FILE` | UI font (`.ttf`/`.otf`); otherwise `TESSERA_EXPLORER_FONT`, then the system's sans-serif font. |
| `--no-persist` | Do not write the settings file. |

3D view options: `--mesh-shading lit|base|normals|uv|wire`, `--mesh-wireframe`,
`--mesh-normals`, `--mesh-vertex-colors`, `--mesh-diffuse-tint`, `--mesh-lod LEVEL`
(0 = full detail), `--mesh-choice N` (which geometry link or appearance entry),
`--dye RRGGBB` (preview a dye colour), `--mesh-select PART,DRAW` and
`--mesh-angle YAW,PITCH`.

### 5.2 Character viewer and animation browser

The Character tab assembles any playable body or placed figure, dresses it, and plays its
animations. Open it on any record; a figure record becomes the character, an animation
clip (`0x05…`) plays on the current character, and an item is previewed. The same setup
can be given on the command line, so a view can be reproduced exactly:

| Option | Meaning |
|---|---|
| `--character FIGURE` | `SPECIES,SEX` (a race-table label or key, then `m`/`f`, e.g. `human,m`) or a figure DID (`0x47…`, `0x70…` to `0x76…`, or `0x04…`). Opens the Character tab. |
| `--wear ITEM` | Wear or hold an item: `DID[@dye=CODE][@dye2=CODE][@sheathed][@wielded][@slot=primary|secondary|ranged|BITS]`. Repeatable, up to 32 times. |
| `--clip CLIP` | `idle`, `walk`, `run`, `none` (reference pose) or a clip DID (`0x05…`). |
| `--time SECS` | Pause at this play time. |
| `--sliders HEX` | Body-slider bytes as hex pairs, one per visible slider. |
| `--bones`, `--trail` | Draw the bone overlay; draw the root-motion trails. |

### 5.3 Screenshots and contact sheets

| Option | Meaning |
|---|---|
| `--screenshot PNG` | Render until the selection is ready, save the window as PNG and exit (implies `--no-persist`). Exit status 2 if it gave up waiting (`--screenshot-timeout SECS`, default 120), 3 if the PNG could not be written, 4 if the install could not be opened. |
| `--browse ROWS` | With `--screenshot`: first step through this many rows of the record list, one per frame, and log frame statistics (a responsiveness check). |
| `--sheet PNG` | Render the character view into a contact sheet without opening a window, and exit: one row per camera angle, one column per play time. Uses `--character`, `--wear`, `--clip`, `--sliders`, `--bones`, `--trail` and `--mesh-wireframe`. |
| `--times T,T,…` | Sheet columns: play times in seconds (default: `--time`, else 0). |
| `--angles Y,P;Y,P…` | Sheet rows: camera yaw and pitch in degrees (default: `--mesh-angle`, else `160,10`). |
| `--focus Z,R` | Frame a sphere at height Z with radius R (metres) instead of the whole figure, e.g. `1.65,0.3` for a head. |
| `--tile WxH` | Tile size in pixels (default `480x640`). |

```sh
# A walk cycle in four frames, from the front and the side
cargo run --release -p tessera-explorer -- --character human,m --clip walk \
    --times 0,0.25,0.5,0.75 --angles "180,10;90,10" --sheet ~/tessera-shots/walk.png

# A screenshot on a machine without a display
xvfb-run -a cargo run --release -p tessera-explorer -- --select 0x41000123 \
    --screenshot ~/tessera-shots/texture.png
```

---

## 6. Command-line inspection (`tessera-dattool`)

```sh
cargo run --release -p tessera-dattool -- <command> [options]
```

An *archive* is a file path or a short name (`general`, `surface_aux_1`,
`client_cell_2.dat`) looked up in the install; a *dir* is an install directory, opened as
one set. DIDs and prefixes are always hexadecimal, with or without `0x`.

| Command | What it does |
|---|---|
| `detect [--path] [--schema-only]` | Find installs and print them with their fingerprints (§2). |
| `info ARCHIVE\|DIR` | Header fields, entry count and record-size statistics. |
| `ls ARCHIVE [--prefix 0xHH] [--from DID] [-n N]` | List directory entries. |
| `stat DIR\|ARCHIVE [--totals]` | Count records by type (DID high byte). |
| `cat DIR\|ARCHIVE DID [--max-bytes N] [--stored] [--raw]` | Hexdump a record. |
| `extract DIR\|ARCHIVE DID -o PATH [--png] [--text] [--stored] [-f]` | Write a record to a file for personal study; `--png` converts a texture, `--text` dumps a string table. Refused inside the install. |
| `strings DIR\|ARCHIVE DID [--token TOKEN]` | Print a string table. |
| `show DIR\|ARCHIVE DID [--max-list N]` | Decode one record of the supported kinds (terrain and scene tables, water, fog, sky, day descriptions, game time, LOD classes, camera states, collision meshes, cells, region descriptors, landblock records) and print its fields. |
| `validate DIR\|ARCHIVE [--inflate] [--no-tiling] [--max-issues N]` | Check every archive's structure; `--inflate` also decompresses every record (slow). |
| `census DIR\|ARCHIVE [--prefix 0xHH]… [--max-failures N]` | Decode every record of the chosen types and count successes and failures. |
| `snapshot DIR\|ARCHIVE -o PATH [--content] [-f]` | Write a compact index of the install (for `diff` after a game patch). |
| `diff A B [--content] [--prefix 0xHH] [--summary] [--max-list N] [--exit-code]` | Compare two installs, archives or snapshots. |

Global options: `--json` (machine-readable output), `--game-dir DIR`, `-v`/`-vv`/`-vvv`
(more log output), `-q` (errors only). Exit status 0 means success, 1 a failed check
(`validate` or `census` found errors, `diff --exit-code` found differences), 2 an error.

Two useful checks after a game patch:

```sh
cargo run --release -p tessera-dattool -- validate "$TESSERA_GAME_DIR"
cargo run --release -p tessera-dattool -- census "$TESSERA_GAME_DIR"
```

---

## 7. Where Tessera keeps its files

Nothing goes into the install directory.

| What | Where (Linux) | Change it with |
|---|---|---|
| Settings (explorer state, saved game directory) | `~/.config/tessera/settings.toml` (`$XDG_CONFIG_HOME/tessera/`) | `TESSERA_SETTINGS_FILE` |
| Keymap | `keymap.toml` next to the settings file | `TESSERA_KEYMAP_FILE`, `--keymap` |
| Tessera's data folder | `~/.local/share/tessera` (`$XDG_DATA_HOME/tessera`; `./tessera-data` if neither is known) | client `--data-dir` |
| Walker screenshots (F12) | `screenshots/walker-<time>.png` in the data folder | client `--data-dir` |
| Plugins and plugin data | `Plugins/` and `plugindata/` in the data folder | client `--plugins` |
| Headless screenshots, sheets, frame and walk logs | wherever you point `--screenshot`, `--sheet`, `--frame-log`, `--walk-log` | |

On other systems the settings file (and the keymap beside it) lives in the platform's
usual per-user configuration directory, under `tessera/`. The data folder follows the same
rule everywhere: `$XDG_DATA_HOME/tessera` when that variable is set, otherwise
`.local/share/tessera` under your home directory (`--data-dir` picks another). A
settings file that cannot be read is kept as a
`.corrupt-<time>.bak` copy beside it and defaults are used.

**Logs** are printed to the terminal, not written to a file: the client prints to
standard output, the explorer and dattool to standard error. Set `RUST_LOG` for more or
less detail, and redirect to keep a copy:

```sh
RUST_LOG=debug cargo run --release -p tessera-client -- --place "12.3S, 45.6W" \
    2>&1 | tee ~/tessera-runs/client.log
```

---

## 8. Reporting a problem

Please open an issue on the project's repository. A good report lets someone else
reproduce the problem on their own install.

**Attach or include:**

- what you did, what you expected and what happened;
- the exact command line, and for the walker the spawn: the `--spawn`/`--place` value or
  the HUD's landblock and `/loc` reading where it happened, the heading, and any time-of-day
  options;
- the version: `tessera-client --version` (or the tool you used) and
  `git rev-parse --short HEAD`, plus whether the working tree had local changes;
- your system: operating system, GPU and driver, and whether you ran on Wayland, X11 or
  headless;
- the output of `tessera-dattool detect` (install path and fingerprints identify the game
  build; remove your user name from the path if you like);
- the log of the run, ideally with `RUST_LOG=debug`, trimmed to the part around the
  problem;
- for performance problems, the `--frame-log` CSV and the summary printed at exit; for
  movement and collision problems, the `--walk-log` CSV or the walk file you ran;
- for a crash, the panic message and backtrace (`RUST_BACKTRACE=1`).

**Never attach game data** of any kind, in an issue, a pull request or anywhere else
public:

- `.dat`/`.datx` archives or parts of them;
- anything `extract` or `cat --raw` writes, hexdumps from `cat`, or `strings`/`show`
  output (records, textures, meshes, sounds, string tables, game text);
- `snapshot` files;
- screenshots or contact sheets that show the game's textures, models or text. Describe
  what you see instead (where, at what time of day, compared with the original client);
  if a picture is really needed, ask the maintainer how to share it privately.

If a log line quotes game text, such as a place name, it is fine to replace it with a
placeholder. Record DIDs, landblock numbers, coordinates and counts are not game content
and are the most useful facts to include.
