<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera user guide

This guide is for testing Tessera on your own install of the game: how to build and start
the three programs, the options worth knowing, the controls, where Tessera keeps its files,
and how to report a problem. Every option named here exists in the programs' own `--help`
output; run that for the complete, current list.

Tessera is not playable yet. The client is mainly an offline **world walker**: you walk or
fly over the world as an animated avatar, among the characters the data places, with no
combat. It can also join Tessera's own test server (not the game's servers) and attach an
early game UI with Lua plugins (§3.12), with character selection and creation screens, a
character panel with bag windows, and item tooltips (§3.13), and NPC windows, a quest log
and tracker and the wallet (§3.14), and it shows targeting, skills and combat as a session
reports them (§3.15), and social panels, tells and chat channels, and synchronised music
(§3.16), and maps, travel, the instance finder and mounts (§3.17). Scenario files (§3.11) script
gameplay events for testing; they play back what the file says and compute nothing.

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

While the walker runs, these keys work the clock and switch systems off and on:

| Key | Does |
|---|---|
| `[`, `]` | move the clock back or forward by one game hour (1/24 of the day) |
| Slash (`/`) | stop or restart the clock |
| Comma, Period (`,`, `.`) | halve or double the time rate |
| F5, F6, F7, F8 | texture layers, shadows, water, sky on or off |
| F2, F4 | generated scenery, frills on or off |
| F11 | distance detail (LOD) on or off |

A key that the keymap binds (§4.2) belongs to the keymap, and the walker's own action on it
stops. The built-in targeting and quickslot bindings (§3.15) take Backslash, Minus, Equal,
F9 and F10; the walker's clock and switch keys used to sit on those five, so they were moved
(the owner's answer of 2026-10-16): the clock now stops on `/`, the time rate changes on `,`
and `.`, scenery toggles on F2 and frills on F4, none of which the keymap binds by default.
At start-up the walker logs which of its keys a keymap override has taken.

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
| `--scenario FILE` | Play FILE with the sandbox, or with `--server` as an overlay over a test server (below). Cannot be combined with `--walk-path`, `--fly-path`, `--duration`, `--smoke`, `--showcase`, `--ui-script` or `--character-select`. Works with `--no-data`, `--game-dir`, `--spawn`, `--place` and `--heading` (not in overlay runs with `--server`, where the server places the player: `--spawn` and `--place` are refused there, `--heading` does not place it); `--place` needs an install (with `--no-data` the run uses the default spawn and logs a chat notice). |
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

**Plugins in headless runs.** `--headless --scenario FILE --load-plugin NAME` loads the
plugin when the player enters the world and feeds it the scenario's inventory changes on the
scenario's clock, as a windowed run would deliver them frame by frame, so a plugin that
reads the backpack and the equipment (`GetBackpack()`, `GetEquipment()`) can be tested
without a window. What the plugin writes to chat goes to the client's log.

**Over a test server (overlay runs).** With `--server` (and `--account`, §3.12) the scenario
plays as an **overlay**: the test server keeps the login, the character list, world entry,
movement, other players and chat, and the file scripts only your own character's inventory,
level and stats. It may use only the settings `title`, `tick`, `end-at`, `defaults` and
`requires install`; the declarations `select` and `item`; the steps `bag`, `give`,
`remove`, `quantity`, `equip`, `unequip`, `condition`, `wallet` and `item-property`, and
`level` and `stat` for `player`; `repeat`; `on` blocks for `equip`, `unequip`, `move-item`,
`destroy`, `split` and `use-item`; and an `input` block of those commands without `at pre`
lines. Anything else is refused before the client connects (exit status 3, error
`NotAllowedInOverlay`). The scenario's clock starts when the server puts you in the world.

When the scenario changes what you wear, the client sends the new equipment to the server
(at most four times a second, changes in between merged), and the server shows it to every
other player there; your own figure shows what the server sends back. The server learns
nothing else: bags, quantities, coins, condition, levels and stats stay in your client. A
server that does not allow appearance changes leaves the change visible to you only (the
client logs this once). With `--headless` the run ends with exit status 0 when the scenario
ends, 5 when it fails or the server goes away, and 5 after an hour without an end. Logs of
overlay runs depend on the server's timing, so they differ between runs and are never
committed.

```sh
cargo run --release -p tessera-client -- --scenario gear.scenario \
    --server tessera://127.0.0.1:7777 --account wren --ui
```

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
  The same holds for property IDs written as numbers (`pid:N:KIND` in `stat`,
  `item-property` and `look` lines): write the property's label (`@Label`) instead. A
  property value of kind enum, bitfield32 or bitfield64 is a label too, a value of kind
  did is an element, never a number, and so is the `nationality=` of `create-character`
  (a label). Without an install the committed-scenario test cannot check a labelled
  property's value kind, so reviewers check these by hand.
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

<a id="characters"></a>
### 3.13 Characters, inventory and tooltips

These parts are early. Their screens, lists, icons and texts come from your install; what a
character carries, its level and its stats come from a scenario file (§3.11) or are simply
not there, because Tessera runs none of the game's rules. Without a scenario the bags and
equipment slots are empty and moving items does nothing.

**Character selection and creation.** `--character-select` opens the character selection
screen before the world, instead of entering the account's first character (or the one
`--character` names). It needs an install and attaches the game UI (`--ui`). It works
offline and with `--server`; it cannot be combined with `--character`, `--headless`,
`--scenario`, `--no-data`, `--walk-path`, `--fly-path`, `--smoke` or `--showcase`.

```sh
cargo run --release -p tessera-client -- --character-select
cargo run --release -p tessera-client -- --character-select \
    --server tessera://127.0.0.1:7777 --account wren
```

- **Selection.** The list shows the characters the session reports. Select one and enter
  the world with Enter or a double click. Escape on this screen quits the client.
- **New** opens the race and class screen (every race, gender and class your install
  offers), then the appearance and name screen: nationality, the appearance controls of the
  chosen race and gender, randomise, the name field with a random name, and the name checks
  your install defines. Back goes one screen back. The turn and zoom buttons turn the
  viewport. **Create** sends the character to the session, and it appears in the list.
- **Offline** a made character lasts until the client exits; Tessera still keeps its
  creation choices in its settings file (section `[[CreatedCharacters]]`, under the
  sandbox's key). **On a test server** it is
  created there when the server allows creation (otherwise New is greyed out). A server
  that speaks protocol 1.1 stores the character's look, and other players see its heights,
  styles and body sliders; its colours and face layers are not drawn yet on any figure
  (below). With an
  older server (1.0) only the body is stored; Tessera then keeps your choices in its
  settings file (section `[[CreatedCharacters]]`) and applies them to your own view of that
  character only.
- **Delete** is greyed out offline and on test servers; neither supports deletion yet.
- If the screens cannot open (the install's creation data or the creation layout cannot be
  read), the client enters the first character and says why in chat.
- Not done yet: the figure in the screens' viewport is not drawn; the lists, controls and
  buttons work. A look, your own or one received from a server, draws its heights, styles
  and body sliders only: the chosen colours (skin, hair and the like) and face layers are
  not applied to any figure yet, which shows the race's defaults instead.

**Character panel and bags.** With the game UI attached (`--ui`, `--character-select`, or
an option that implies `--ui`), these keys work once you are in the world:

| Key | Action (keymap name) | What it does |
|---|---|---|
| C | `toggle_character` | Character panel: name, level, race and class, the paperdoll with your figure, the 23 equipment slots and the stat rows |
| B | `toggle_bags` | Opens every bag window when one is closed, otherwise closes them all |
| I | `toggle_bag_1` | Bag 1 |
| (none) | `toggle_bag_2` … `toggle_bag_6` | Bags 2 to 6 |
| Ctrl+T | `pin_tooltip` | Pins the item tooltip under the mouse |

All are in the `global` context of the keymap file (§4.2). In fly mode C moves down
(§4.1) and does not open the character panel; unbind `fly_vertical`'s C in the `fly`
context or bind `toggle_character` to another key to have both. `--open-panel character`,
`--open-panel bags` and `--open-panel bag1` … `bag6` open them at start, as soon as you are in
the world (repeatable). A bag window opens only when your backpack has slots for it. The
paperdoll's buttons turn the figure by 15° per step; dragging in it turns it by 0.5° per
pixel. Each bag window's number of columns is kept in Tessera's settings file.

Items in the bags and on the paperdoll:

| You do | The client asks for |
|---|---|
| Drop a bag item on a paperdoll slot it fits | Equip it there (a slot it does not fit refuses the drop) |
| Activate (right-click) a bag item that can be worn | Equip it in its default slot |
| Activate any other bag item | Use it on your current target |
| Drop an equipped item on a bag slot, or on a bag window | Take it off |
| Activate an equipped item | Take it off |
| Drop an item on another bag slot | Move it (swapping with an item already there) |
| Shift and drop a stack of 2 or more on an empty slot | Split off half the stack (it goes to the first free slot) |
| Drag an item out of every window, then again within 10 s | Destroy it (the first time only asks) |

The panels never change anything on their own: each action is a request to the session, and
the slots change when the answer arrives. A scenario's `on` blocks (or its `defaults on`)
give those answers; on a test server only an overlay scenario does. Equipping changes the
worn look of your figure in the world.

**Item tooltips.** Hold the mouse over an item slot and, after the usual tooltip delay, the
item's tooltip opens with the rows, colours, icons and texts your install gives that item.
Next to it, comparison tooltips show what you have equipped in the slots the item fits;
`compare_equipped_items = false` in the `[UI]` section of Tessera's settings file turns them
off. Ctrl+T pins the tooltip under the mouse, so it stays open; a pinned tooltip closes with
its close button.

<a id="npc-windows"></a>
### 3.14 NPC windows, quests and the wallet

These parts are early. The windows' layouts, texts and icons come from your install; which
window opens, what it lists, prices, quantities, quest states and objective counts come from
a scenario file (§3.11), because Tessera runs none of the game's rules: no dialogue tree, no
quest logic, no prices and no loot tables. Without a scenario, using an NPC does nothing.
The windows need the game UI (`--ui`, or an option that implies it, §3.12) and, in a
windowed run, an install; without one they appear only in the tests. The test server sends
none of them.

**Using an NPC.** U uses the nearest entity in range that is not a player (an NPC, a
creature or an object such as a chest): the client checks the distance (the NPC's use range, about 6 m when its record names none) and asks the
session to use it; the scenario's `on use` block answers by opening a window. Out of range,
or with nobody near, a short line in chat says so and nothing is sent. Selecting with the
mouse (a double click uses the entity) is part of targeting (§3.15); U works without a selection. A
scenario's `input` block can also send `use` (headless runs, or `--scenario-input`).

| Key | What it does |
|---|---|
| U | Use the nearest non-player entity in range (NPC, creature or object) |
| L | Quest log (open or close; not with Shift, Ctrl, Alt or Super held) |
| Enter | In the active NPC window: Loot All in the loot window, OK in the quantity dialog |
| Escape | Closes the active NPC window (asks the session, as the close button does) |
| Alt, Shift, Tab, held | Buy modes in the vendor window (below) |

U and L are fixed for now: they are not in the keymap file (§4.2), and neither works while
you type in a text field.

**Dialogue.** An NPC's page opens in one of three windows, as the page needs: a
conversation (text and Continue), a choice (text and a list of options to click) or a short
comment. Option texts are your own literals or install texts named by the scenario (a quest's
objective or role line, a name, or, with `action(@Label)`, the install's own action labels). Continue, the close button and Escape ask the session to close the window;
it closes when the session says so. Walking out of the NPC's use range, or the NPC
disappearing, closes it too.

**Quest offers.** The offer window shows the quest's name, the offer line, the objective and
the rewards (amounts from the scenario's `rewards` step: coins, experience of several kinds,
reputation, wallet items). Accept and Decline answer the offer. A selectable reward can be
highlighted, but the choice is not sent yet.

**Vendor and barter.** The vendor window has a buy page, a sell page and, once the session
sends a buy-back list, a buy-back page. Prices show in coins, in red when you cannot afford
them.

- **Buy:** right-click or double-click an entry, or select it and press Buy. Hold Alt to
  open the quantity dialog (presets, arrows, typing; the cost follows the quantity), Shift
  to buy a full stack, Tab to buy what fills your partial stacks of that item; with no key
  held you buy one.
- **Sell:** the sell page lists the backpack items that can be sold, with the worth the
  item's tooltip shows (the money you receive is whatever the session sends). Right-click,
  double-click or Sell sells one entry; Sell All sells every entry. Dropping a bag item on
  the window does not sell it yet.
- **Buy back:** right-click, double-click or the buy-back button.
- **Barter:** a vendor list whose entries cost items opens the barter window: each entry
  shows what you get and up to three costs, the required quantity in red when you hold
  fewer; Buy, a right click or a double click buys one. Pages hold 20 entries.

Repair, the try-on and find-items buttons are hidden; sorting and filters are greyed out.

**Loot.** The loot window lists what the session offers, with coins in the first row. Loot
All (or Enter) asks for everything and closes the window once all is taken; right-click or
double-click a row, or drag it out of the window, to take one entry.

**The wallet.** Your coins show in gold, silver and copper (split with your install's coin
values) in the vendor window and in the wallet window, which the wallet button of the
character panel (C) opens. The wallet window lists the money first, then the other
currencies by category, sorted by name, with a search field. Plugins see the same wallet
through `GetWallet()` and `GetMoney()` and their change events, also in headless scenario
runs with `--load-plugin` (§3.11).

**Quest log and tracker.** L opens the quest log: quests grouped by category, the
active/completed switch, the filter buttons, expand and collapse, and the details pane with
the objective text, progress lines with their counters (for example 2/4) and the rewards
the quest record names (amounts show only in the offer window). Below the list:

- **Track** adds the selected quest to the quest tracker or takes it off (at most 10).
- **Abandon** shows the confirmation text in chat; click it again for the same quest to
  abandon it (selecting another quest or closing the log cancels). The session decides what
  happens.
- **History** shows the selected quest's details in history order; share and group finder
  are greyed out.
- The tracker check box shows or hides the tracker for this run (not saved yet).

The tracker shows the tracked quests with their current objective and counters; clicking an
entry opens the log at that quest. A right click lists the menu's items in chat (show in
log, remove, remove all), but choosing one needs the popup menu, which is not built yet.

**Quest lines in chat.** A new, completed or failed quest posts a line in the Quest chat
type (shown in the first chat tab), and an objective update posts the install's update text
when the quest's record has one. Lines that the game's servers would send (quest removed,
experience earned and the like) are not made up; a scenario posts them with `system`.

**Quest markers.** A scenario's `marker … quest=` lines put quest markers on the minimap
(within its radius) and on the world map, with the label as tooltip; markers of another
region than yours stay hidden until you enter it. An NPC whose scenario sets
`@Quest_CurrentBestowRingDID` to a quest shows a small sign over its head (available,
continue, in progress), standing in for the ring the original draws. `--quest-icons off`
hides these signs (default on; not a saved setting yet).

**Opening the windows empty.** `--open-panel` (§3.12) also takes `conversation`, `choice`,
`npc-comment`, `vendor`, `vendor-quantity`, `barter`, `loot` and `wallet` (the wallet shows
yours), and `quest-log` and `tracker`. They open as soon as you are in the world, for a look
at the install's layouts.

**The committed scenarios.** Four synthetic scenarios in `apps/client/tests/scenarios/`
show the windows. Each plays without an install (the literal entries) and adds install
records with one:

| File | What it shows |
|---|---|
| `dialogue.scenario` | A choice page with your own and install-labelled options, a conversation page reached by an option, then (with an install) an NPC comment, a quest offer with reward amounts, and a choice page whose text is a quest's own line |
| `vendor.scenario` | A buy list with prices and stock, the sell list from your bags, a buy-back page, a purchase of three, a sale and a buy-back that change the coins, then a barter list that costs a token |
| `loot.scenario` | A creature's loot with coins and stacks: one entry taken, then Loot All; then a chest's loot of which the scenario takes one entry |
| `quest-log.scenario` | Quests offered, accepted, counting up, tracked, completed, failed and abandoned, quest markers added, moved and removed, and an NPC's quest sign |

```sh
# Headless, no install: the events in the log, the same on every run.
cargo run --release -p tessera-client -- --headless --no-data \
    --scenario apps/client/tests/scenarios/vendor.scenario --scenario-log -
# Windowed with your install: the windows on screen, the input block played for you.
cargo run --release -p tessera-client -- --ui --scenario-input \
    --scenario apps/client/tests/scenarios/dialogue.scenario
```

Without `--scenario-input`, walk to the NPC and press U yourself; the scenario's answers
follow its `on` blocks, so the input block's own commands (`choose`, `buy`, `take-loot` and
so on) are then yours to give in the windows.

**Local golden images** ([decision 0031](decisions/0031-parity-panel-goldens.md), Proposed).
Pictures of the windows filled from your install stay on your machine: they are written under
`${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}`, one set per install
version, and the tests refuse a directory inside any git work tree. The first run writes
them and reports `golden: blessed PATH`; later runs compare and report `passed`.

```sh
export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
export TESSERA_GOLDEN_DIR="$HOME/.cache/tessera/my-goldens"   # outside every work tree
cargo test --release -p tessera-client --test npc_windows_real_data -- --nocapture
cargo test --release -p tessera-client --test quest_journal_real_data -- --nocapture
# Look at the PNG files, then run again; a missing picture is now a failure:
TESSERA_GOLDEN_REQUIRE=1 cargo test --release -p tessera-client \
    --test npc_windows_real_data --test quest_journal_real_data -- --nocapture
```

`TESSERA_GOLDEN_BLESS_LOCAL=1` rewrites existing local pictures (it cannot be combined with
`TESSERA_GOLDEN_REQUIRE`), and `TESSERA_SHEET_DIR=DIR` also copies every picture to DIR for
viewing. On a mismatch the new picture and a difference picture are written next to the
golden. `TESSERA_BLESS=1` belongs to the committed synthetic goldens only
(`cargo test -p tessera-ui --test p3_goldens`, images in `crates/ui/tests/golden/`); it never
touches the local ones. Never commit a local picture: it shows your install's text and art.

<a id="combat-ui"></a>
### 3.15 Targeting, skills and combat presentation

With the game UI attached, the client shows combat as the session reports it. Tessera does
not compute combat: every value (health, damage, casts, effects, cooldowns, defeat) comes
from the session, that is a scenario in the sandbox or, later, a server.

**Selecting.** Click a character, creature or object in the world to select it. A click on
the ground or the sky changes nothing. A double click selects the entity and then uses it.
Escape cancels your cast if one is running; otherwise it clears your selection; with
neither it opens the main menu. The selection appears only once the session confirms it,
and it clears by itself when the selected entity leaves your view. The targeting keys
search within 50 m:

| Key | Selects |
|---|---|
| Tab, Shift+Tab | the next or previous creature that is not defeated (nearest first; wraps around) |
| Backspace | the nearest such creature |
| F10, Shift+F10, Ctrl+F10 | the nearest, next or previous creature (defeated ones included) |
| F9 or Ctrl+Tab, Shift+F9, Ctrl+F9 | the nearest, next or previous other player |
| Delete | the nearest object |
| Backslash | yourself |
| F | your selection's own target (assist) |

`target_last`, `target_last_attacker` and `target_clear` have no default key; bind them in
the keymap file (§4.2). The targeting keys and the quickslot keys below keep Backslash,
Minus, Equal, F9 and F10 (the original client's defaults); the walker's own clock and switch
functions that once used those keys moved to other keys (§3.5, the owner's answer of
2026-10-16), so nothing on the walker side competes with them.

**What you see.** Your frame (name, level, health, power and class resource) and the
target frame (name in its colour, level, health, or the name alone for a target without
health). A small window shows your target's own target. The selection gets a marker and a
name block over it in the world. Cast bars appear for your casts and your target's. Effect
icons sit under the frames, beneficial ones in the first row and harmful ones in the
second; an icon pulses in its last 15 seconds. Damage, heals and avoids rise as floating
numbers and words over whoever took them; critical hits start large. The toolbar shows when
you are in combat. When you are defeated, a panel says so; its buttons send nothing (OK
closes it).

**Quickslots.** Keys 1 to 0, Minus and Equal use the slots of the main bar. With Ctrl, Alt
or Shift they use slots 13 to 48. A click on a slot uses it too. A skill slot sends a skill
use, and an item slot an item use, both at your current selection. A slot shows a sweep
while its cooldown runs, a frame while its skill is being cast, and a flash when the session
refuses the use. Hold the mouse over a skill slot or an effect icon to see its tooltip; an
effect's tooltip counts down its remaining time.

**Plugins** see your target, vitals, level, effects, combat state and learned skills
through `Turbine.Gameplay` (`LocalPlayer`, `Actor`, `EffectList`, `Effect`, `SkillList`).
A plugin quickslot that holds a skill or an item uses it like the game's bar.

**On a test server** (`--server`) you can select the server's wanderers and other players.
Their frame shows a name and no level or health, because the server has no combat. Casting,
quickslot changes and other combat commands are refused there. A scenario played over a
server (`--scenario … --server`) may not script combat steps (exit 3).

**Scenarios.** `apps/client/tests/scenarios/combat-ui.scenario` shows every combat display
in turn.
Play it with `tessera-client --scenario apps/client/tests/scenarios/combat-ui.scenario`
(windowed), or headless with `--headless` to check its log.

<a id="social"></a>
### 3.16 Social panels, chat channels and music together

These parts are early. The panels and lines come from your install; who is in your
fellowship, your friends and ignore lists, a trade and other players' music come from a
scenario file (§3.11), because Tessera runs none of the game's rules. Without a scenario the
panels stay empty. On a test server (`--server`) only chat goes through the server: tells and
named chat channels; fellowships, trades, lists and synchronised music are not there (the
server relays chat only).

**Panels.** With the game UI attached (`--ui`):

| Key or option | What it does |
|---|---|
| O (`toggle_social`) | The Social panel; it opens at the page it last showed (first time: the fellowship page) and closes when shown |
| (none) `toggle_social_friends`, `toggle_social_grouping`, `toggle_social_kinship`, `toggle_social_raid`, `toggle_social_raid_lock` | The Social panel at that page; again on the same page closes it |
| (none) `select_fellow_1` … `6`, `assist_fellow_2` … `6` | Target a fellowship member (1 is you, 2 the first row, …) / that member's target |
| `--open-panel social`, `friends`, `fellowship`, `trade` | Open the Social panel (at its friends or fellowship page) at start; the fellowship display and the trade window show themselves while there is a fellowship or a trade |

The friend and member keys are unbound in Tessera's own key map (F3 is its debug key);
bind them in the keymap file (§4.2). Escape closes the Social panel.

- **Friends and ignore lists** show the names in the lists the session gives you, online
  friends in the online colour; level, class and area are unknown (shown as such). Click a
  column header to sort, again to reverse. Add asks you to use `/friend add NAME` (Tessera
  has no input dialog yet); Remove, Invite and Tell act on the selected friend; a double
  click starts a tell.
- **Fellowship display**: one row per member besides you, with morale, power and level where
  the session gives them; the leader marker on the leader's row, or on your own vitals panel
  when you lead. A leader change, a member leaving or being dismissed, and the disbanding
  show at once, with lines in chat. Clicking a row targets the member.
- **Invitation**: accept or decline; nothing happens until the session answers.
- **Trade window**: both offers with the items' icons and tooltips, your coin in the gold,
  silver and copper entries, the trade button to accept, Cancel to end it. Dropping an item
  from your bags offers it.

**Chat commands.** Typed in the chat window (with `--ui`), in the walker's chat line, or with
`--command` (channel and chat commands such as `/tell`, `/join` or `/ch1` are never sent from
a start-up line; the lists and fellowship requests do run). They work under Tessera's names
below and under the names your install's language gives them.

| Command | What it does |
|---|---|
| `/tell NAME TEXT`, `/reply TEXT`, `/retell TEXT` | A private message; reply to the last one you got; tell the last one you told again. Shift or Ctrl with Up and Down in the chat entry steps through recent senders or recipients |
| `/join NAME`, `/part NAME`, `/channels` | Join or leave a named chat channel (8 slots), list your channels |
| `/ch1 TEXT` … `/ch8 TEXT`, `/chsay NAME TEXT` | Speak on a channel by slot or by name |
| `/fellowship TEXT` | Speak to your fellowship |
| `/friend add NAME`, `/friend remove NAME`, `/friend list`, and the same with `/ignore` | Your lists; lines from ignored names are hidden |
| `/invite NAME`, `/dismiss NAME`, `/promote NAME`, `/leave`, `/disband` | Fellowship requests |

With the instant-message option of your install, the chat window has a tab with one
conversation per partner. On a test server, named channels need a server that speaks
protocol 1.2; an older one says user chat is not available.

**Music together.** `/play SONG [PART] sync` gets your part ready without starting it (a ready
line in chat); `/playstart` starts every waiting part of your fellowship at one common time:
offline the session answers with the start; on a test server your own part starts locally.
Members' parts come from a scenario (`perform … sync`), and their songs are read from your own
music folders like yours. A synchronised start shows its line once, also when nothing waits.
A scenario's `perform ENTITY "Tune"` without `sync` plays that tune after its delay. Your own
`/play` still needs an instrument (equipped, or chosen with `/instrument`).

<a id="map-travel"></a>
### 3.17 Maps, travel, the instance finder and mounts

These parts are early and are being built (P5). As for the NPC windows (§3.14), the windows'
layouts, map images, names and icons come from your install, while what they list comes from
a scenario file (§3.11): travel destinations, costs and levels, the instance finder's entries
and queue, and the markers on the maps. Tessera runs none of the game's travel, instance or
mount rules. The windows need the game UI (`--ui`, §3.12). The test server sends none of
this, and mounts are not available with `--server`.

**The world map.** Open it with M (with the game UI, while no text field has the keyboard),
with the map button of the minimap, or at start with `--open-panel map`. It opens full screen or as a window (the install's default; the button in
its corner switches, and Tessera remembers the choice). Click a marked area on a map to open
that map; a right click goes back up to the parent map; the map menu lists the maps by region.
Dungeon maps are drawn stretched into the map's frame. The filter menu turns the kinds of map
notes on and off, with *all* and *none* at the top; the choice is kept per kind of map in
Tessera's settings, not per character. On area and dungeon maps, when the coordinates option
is on (the install's default, kept in Tessera's settings), the coordinates under the pointer are
shown; the world map and the territory maps show none. Every note's tooltip shows its own
coordinates. Tessera hides what needs game state it
does not have: the stable-master button, the quest-guide option and the unexplored-area
shading of dungeon maps.

**Markers and notes.** A scenario's `marker` lines put markers on the world map and the
minimap (`note-type=` picks the icon of a kind of map note; `icon=` gives an image), and a
`markers` line puts one marker on every element of a selection, for example on every map
region. On the minimap, notes outside its circle point at the rim towards their place (for the
kinds that have such an icon); scenario markers show only inside the circle. Markers keep their
place when you change maps and are shown on every map of their region that covers their
position, with their label and coordinates as tooltip.

**Finding a place.** `/findplace TEXT` looks up a place: map coordinates (such as
`12.3S, 45.6W`) or a pasted `/loc` line, a place key, or the name of a map, area, territory,
region or dungeon (matched as loosely as `--place`, §3.2). Up to ten results are posted in
chat, and the map shows the first one with a marker; `/findplace` alone removes that marker.
The original client has no such search: it is Tessera's own command.

**Travel.** Using a stable master (U, §3.14) in a scenario whose `on use` block answers with
`travel-list` opens the travel window: destinations grouped under their territory, a search
field, the selected destination's cost (or free), level and area, and the go button. Rows the
scenario marks as undiscovered or unusable carry the install's marks and cannot be chosen; rows
whose level or requirements are not met show the install's mark but can still be chosen (the
scenario answers). Go (or a double click) asks the session to travel; the window closes when the
scenario answers. A started travel shows the loading screen with its progress bar for a swift
route, or the end-travel box for a slow one; the box's button asks to stop. The client never
moves you: the scenario places you at the destination with `teleport`.

**The instance finder.** A scenario's `instances` step opens the instance finder page: instances
under their kind and category, groups of instances as one row, the size, tier and level
choices, and the join button. Join sends your choice to the session; the scenario's
`instance-queue` lines then show the waiting page with its timer, the confirmation, and the
alert while you wait (a click on the alert, or Ctrl+J, opens the finder); the leave-queue button asks the session to take you out of the queue. Inside an instance the join button
is greyed; `/leaveinstance` asks to leave. The list of other players' groups on the right is shown
as unavailable. `--open-panel` (§3.12) also takes `travel`, `instance-finder`,
`instance-travel`, `end-travel`, `loading-screen` and `queue-alert`, which open these windows
empty at start.

**Mounts in the walker.** In the offline walker your avatar can ride (playable bodies only,
on the collision walker, not in fly mode):

| How | What it does |
|---|---|
| `--mount SPEC` | Start mounted (at once, without summoning). Cannot be combined with `--server`, `--scenario`, `--no-data`, `--fly` or `--showcase`. |
| `/ride` | Mount the last mount (else `--mount`'s, else `auto`) with summoning; while summoning, mounting or mounted, dismount. |
| `/ride SPEC`, `/ride off` | Mount SPEC with summoning; dismount. |
| `/ride list [PAGE]` | List the mounts your body is offered, ten per page, with their index, DID and type. |
| Ctrl+M (`toggle_mount`) | Same as `/ride` (keymap context `movement`, §4.2). |

SPEC is `auto` (the first mount offered to your body), `dir:N` (the N-th of that list, from 0),
`steed:light`, `steed:medium` or `steed:heavy` (your race's war-steed), or a DID of a mount
effect or of a mount skill; options follow with `@`: `@summon` (start with the summoning),
`@instant` (start mounted) and, for war-steeds, `@lock=walk|trot|canter|gallop` (hold the gait
at that gear). Examples: `auto`, `dir:12@summon`, `steed:medium@lock=canter`.

Summoning takes the mount skill's time (about 2 s, 3 s for a war-steed); moving, jumping,
falling or stepping into water interrupts it. Then the mount appears and you ride with the mount's
gaits: a normal mount walks while you hold Shift and runs otherwise; a war-steed shifts up one gait
about every half second while you hold forward, keeps its speed when you let go, and slows down
and backs off when you hold back. Riding into water dismounts you. The HUD shows a mount
line with the state, the mount's DID and name, the gait, the speed and the clips playing; a
refusal or an interruption shows its reason there for a few seconds. `--frame-log` and
`--walk-log` gain the mount's columns, and `--walk-path` runs accept `--mount` (instant only).
In a scenario the scenario decides who rides what (`mount` lines); your own `/ride` requests
are then refused, except a dismount, which asks the session. Plugins see the mount through
`LocalPlayer:GetMount()` and the `MountChanged` event.

**The committed scenarios.** Four synthetic scenarios in `apps/client/tests/scenarios/` show
these parts; each plays without an install (literal entries) and adds install records with
one. Their exact steps are in the files:

| File | What it shows |
|---|---|
| `map.scenario` | Markers on the maps, kinds of map notes, marker sets and their removal |
| `travel.scenario` | A stable master's travel list, a swift and a slow travel with arrival |
| `instances.scenario` | The instance finder, a join, the queue states and leaving an instance |
| `mount.scenario` | Mounting a normal mount and a war-steed, mount values, dismounting |

```sh
# Headless, no install: the events in the log, the same on every run.
cargo run --release -p tessera-client -- --headless --no-data \
    --scenario apps/client/tests/scenarios/travel.scenario --scenario-log -
# Windowed with your install, the input block played for you:
cargo run --release -p tessera-client -- --ui --scenario-input \
    --scenario apps/client/tests/scenarios/instances.scenario
# Ride a war-steed in the offline walker:
cargo run --release -p tessera-client -- --place "12.3S, 45.6W" --mount steed:medium
```

Local golden images of the map, travel and finder windows follow §3.14's rules
([decision 0036](decisions/0036-p5-map-census-and-panel-goldens.md), Proposed).

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
| Ctrl+M | mount or dismount (§3.17) | |
| F3 | HUD details on or off | HUD details on or off |
| F12, Print Screen | screenshot | screenshot |
| Escape | quit | quit |
| Tab, Backspace, F9, F10, Delete, Backslash, F | targeting (§3.15, with the game UI) | targeting |
| 1 … 0, Minus, Equal (also with Ctrl, Alt, Shift) | quickslots (§3.15, with the game UI) | quickslots |

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
  `open_chat`, `toggle_character`, `toggle_bags`, `toggle_bag_1` … `toggle_bag_6`,
  `pin_tooltip`), `camera` (`mouselook`, `look_x`, `look_y`, `zoom`, `reset_camera`),
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
| Settings (explorer state, saved game directory, UI options, the creation choices of characters made on the creation screen) | `~/.config/tessera/settings.toml` (`$XDG_CONFIG_HOME/tessera/`) | `TESSERA_SETTINGS_FILE` |
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
