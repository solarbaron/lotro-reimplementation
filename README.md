<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera

**A clean-room, from-scratch game client compatible with the data files of
*The Lord of the Rings Online*™.**

Tessera (a working codename) reads the game archives of **your own, legally obtained
installation**, in place and read-only, and aims to present that world as closely to the
original client as it legally can: terrain, objects, characters, animation, UI, plugins,
sound and music. It is written in Rust, renders with wgpu, embeds Lua 5.1 for plugins, and
ships **no game content at all**.

The method follows projects such as OpenMW, ScummVM and ClassicUO: original code, the
user's own data, separate format libraries and tools, and a strict clean-room process
between the people who study the formats and the people who write the code.

> Tessera is an independent, unofficial project. It is not affiliated with, endorsed by, or
> sponsored by Daybreak Game Company LLC, Standing Stone Games, Middle-earth Enterprises,
> LLC, Warner Bros., or the Tolkien Estate. The Lord of the Rings Online is a trademark of
> Middle-earth Enterprises, LLC used under license. Requires a legally obtained installation
> of the game; no game content is distributed.

Tessera **does not connect to official game servers** and must not be used to do so. See
[DISCLAIMER.md](DISCLAIMER.md) for the full disclaimer and [docs/LEGAL.md](docs/LEGAL.md)
for the project's legal position.

---

## Status

**Milestones M0 (base), M1 (asset browser) and M2 (mesh and material viewer) are
complete** (exit criteria verified 2026-10-04 against a real install; see
[ROADMAP M0 status](docs/ROADMAP.md#m0-status), [M1 status](docs/ROADMAP.md#m1-status) and
[M2 status](docs/ROADMAP.md#m2-status)). **M3 (terrain and offline world walker) is done on
Tessera's side** ([M3 status](docs/ROADMAP.md#m3-status)): every criterion passes against
the data; the comparison with the original client at 20 reference locations awaits the
owner's in-game readings. **M4 (characters and animation) is done on Tessera's side**
([M4 status](docs/ROADMAP.md#m4-status)): every skeleton and clip decodes,
characters of all 14 playable bodies assemble with worn and held items and play idle, walk
and run with GPU skinning, and 50 of them animate at over 250 fps on the reference machine.
Still open are the owner's comparisons with the original client. **M4.5 (world fidelity)
is in progress** ([M4.5 status](docs/ROADMAP.md#m4-5-status)): terrain is drawn with its
own texture layers, scenery and grass are generated from the data, the avatar walks on
collision geometry (walls, objects, caves, interiors, wading and swimming), and water, sky,
the game clock, lights, shadows and fog are drawn; decoding, invariants, determinism, golden
images and fuzzing pass. The re-verification of 2026-10-07 still finds failing
generated-object and cave walks (25 of 32 walks pass; the 1,000-walk soak has no
fall-through), visual rows that fail or cannot be judged (45 of 54 pass on the image), a
clippy lint, and design specs with changes requested; the M3-b flight passed an indicative
run and awaits a quiet-machine re-measure. Nothing here is playable yet: the walker flies and walks over the world
offline, with an animated avatar and the NPCs the data places.
**M5 (UI layouts, skins and Lua plugins) is in its exit phase** ([M5 status](docs/ROADMAP.md#m5-status)):
every UI layout and font record decodes, the game's own panels open in the walker with the
expected geometry, user skins change their art, and plugins run in a sandbox that passes its
escape tests; the owner's screenshots and plugin install are still to come, and the UI time
budgets are not met yet.
**M6 (audio and player music) passes on Tessera's side** ([M6 status](docs/ROADMAP.md#m6-status)):
all 44,329 sound payloads decode, area music cross-fades at area borders in the walker, an
ABC conformance suite of original tunes passes (156 / 156 rows), and a 10-minute flight had
no audio underruns (indicative, to be repeated on the idle reference machine); the owner's
listening comparisons are pending.
**M7 (Tessera's own protocol and test server) is done on one machine** ([M7 status](docs/ROADMAP.md#m7-status)):
several clients share a world through `tessera-server` over Tessera's own QUIC protocol,
seeing each other move, animate and chat, with one-hour soaks free of desync and leaks; a
run between two machines on a LAN and the owner's confirmation of the server content rules
are still open.
**P0 (parity track foundations: scenario feed and `tessera-gamedata`) is in its exit phase**
([P0 status](docs/ROADMAP.md#p0-status)): `tessera-client --headless --scenario <file>`
plays scripted gameplay events on a virtual clock and writes the same event log on every
run, scenario selectors resolve against the user's install, the scenario parser is fuzzed,
and CI checks that `tessera-server` links no data-decoding crate; the scenario-feed spec still
waits for an independent re-review before it can be Accepted.
**P1 (characters, inventory and equipment) is in its exit phase**
([P1 status](docs/ROADMAP.md#p1-status)): with `--character-select` the selection and
creation screens offer every race, gender, class and appearance control of the install, and
made characters enter the world, offline or on a test server that passes their look on; the
character panel with its paperdoll and the bag windows show scripted inventory, every one of
151,230 item records gets its examination tooltip or a category, equipping changes the worn
look, and plugins read `GetBackpack()` and `GetEquipment()`. Still open: the creation
viewport's figure is not drawn, and colours and face layers of a look are not applied.

| Area | State |
|---|---|
| Archive reader (`.dat`/`.datx` container, directory, compression, DID routing) | done (M0): all 29 archives, 3.8 M entries validated and inflated with 0 errors |
| Record primitives, string tables, textures, property dictionary | done (M0) |
| Install detection (`TESSERA_GAME_DIR`, Steam libraries, Wine/Proton prefixes) | done (M0) |
| `tessera-dattool` inspection CLI, including `diff` and `snapshot` between installs | done (M0, M1) |
| Window and wgpu renderer with a test scene, headless golden images | done (M0) |
| Sandboxed Lua 5.1 host skeleton | done (M0) |
| Property sets with names and enum labels, string rendering with grammar tags, layered icons, asset worker pool, settings | done (M1): 100 % of texture, string-table and property-set records decode |
| `tessera-explorer` GUI asset browser | done (M1) |
| Mesh records (static and skinned in bind pose), materials (surface → material → textures, feature flags, dye), setup records, mesh pipelines (opaque, alpha-test, alpha-blend, dyeable) | done (M2): 100 % of mesh, material and surface records decode; visual checklist of 74 items in [docs/checklists/m2-visual.md](docs/checklists/m2-visual.md) |
| `tessera-explorer` 3D view: meshes, setups, objects, wireframe, normals, UV checker, material inspector, dye preview | done (M2) |
| Landblock records (heights, block maps, landblock info and static entities, properties and minimap tiles), coordinates and map coordinates, world state (areas, regions) | done (M3): 100 % of 923,545 landblock records decode |
| Terrain meshing with LODs and seamless normals; `tessera-client` offline world walker (streaming, static objects, area name, map coordinates, `/loc`, minimap, keymap, fly and walk) | done (M3): full-region flight with no frame over 50 ms; crack-free seams over all 390,150 blocks; 20 reference locations match the data (owner's in-game readings pending) |
| Havok-format skeletons and clips (`tessera-havok`), sampling, blending, root motion, attach points and skinning palettes on workers (`tessera-anim`) | done (M4): 28,555 / 28,555 skeletons and 25,395 / 25,395 clips decode |
| Character assembly (species, sex, worn and held items, dyes), script-table clip selection, GPU skinning, entity store | done (M4): 14 playable bodies play idle, walk and run (42 / 42 checklist rows, River-hobbit height and body-shape controls included); a 48-item worn/held checklist matches |
| `tessera-explorer` character viewer and animation browser; walker avatar, placed NPCs and `--crowd` | done (M4): 51 animated characters at 257 fps headless (p99 6.6 ms) on the reference machine |
| Terrain texture layers, generated scenery and grass, collision and the walk controller (`tessera-physics`), water, sky and game clock, lit pipeline with shadows and lights, draw distances | in progress (M4.5): every new record kind decodes (3,402 scenes, 16,819 collision meshes, 1,057 cells, …); scenery of 78,114 landblocks deterministic; 25 / 32 scripted walks and a 1,000-walk soak with no fall-through; 45 / 54 visual rows pass on the image (re-verified 2026-10-07) |
| Sound payloads (Ogg Vorbis, WAV), sound infos, script-table sound nodes; script-graph runtime (`tessera-scriptgraph`); audio engine on cpal with our own mixer (decision 0019); world audio in the walker (area music, ambience, emitters); ABC player music (`/play`, `/playstart`, `/playlist`, `/playstop`) with instruments and synchronised start | done on Tessera's side (M6): 44,329 / 44,329 payloads decode; area-music cross-fade at borders; ABC conformance 156 / 156; 0 underruns in a 603 s flight (indicative); owner's listening checks pending |
| Tessera's own protocol (`tessera-protocol`, QUIC, decision 0030), `tessera-server` with an original default world, the client's test-server backend (`tessera-client --server`), `tessera-loadtest` | done on one machine (M7): one-hour soaks with 16 clients, 0 desync, no leak; protocol spec Accepted after an independent review; 10 fuzz targets; LAN run between two machines pending |
| UI layout and font records, user skins, retained UI (`tessera-ui`) with CPU canvases and a UI pass in the renderer; game panels, chat window and plugin manager in the walker | M5 exit phase: 302 / 302 layouts and 48 / 48 fonts decode; 15 / 15 panels at their computed geometry; a user skin changes 13 / 13 skinnable panels; owner screenshots pending; UI budgets U1/U4 not met yet |
| Lua plugin host (`tessera-plugins` over the plugin-free core `tessera-script`): discovery, apartments, import, plugin data, UI classes, class helper, `/plugins` | M5 exit phase: synthetic plugin suite and sandbox escape tests pass; real third-party plugins wait for the owner's install |
| Scenario feed in the sandbox (`tessera_net::scenario`), presentation data layer `tessera-gamedata`, `tessera-client --scenario`, server dependency guard (`tools/check-server-deps.sh`) | P0 exit phase: 6 committed synthetic scenarios replay byte for byte; 383,343 install records indexed with 0 failures; 2 fuzz targets; criteria 2–4 pass, the spec's independent re-review (criterion 1) is pending |
| Character selection and creation screens (`--character-select`), character panel and bag windows, item examination tooltips with comparison and pins, worn look from equipment, plugin inventory views; protocol 1.1 looks on the test server | P1 exit phase: 8 races, 15 bodies, 130 (race, gender, class) choices; 151,230 / 151,230 item records categorised (149,695 tooltips laid out, 0 failures); criteria 2–4 pass, criterion 1 partly (viewport figure, look colours and face layers open) |
| Official-server compatibility | **gated** behind legal review ([LEGAL.md §9](docs/LEGAL.md#9-gates-and-the-official-server-target)); no code exists |

The full plan, with exit criteria per milestone, is in [docs/ROADMAP.md](docs/ROADMAP.md).

---

## Building

You need a Rust toolchain from [rustup](https://rustup.rs/). The repository pins the exact
toolchain version and components in `rust-toolchain.toml` (edition 2024); rustup installs it
on first use. The minimum supported Rust version is 1.90 (`rust-version` in `Cargo.toml`),
except `apps/explorer`, which needs 1.95 for its GUI toolkit.

```sh
cargo build --workspace
cargo test --workspace
```

The client additionally needs a GPU driver that wgpu supports (Vulkan on Linux and
Windows, Metal on macOS, or DX12 on Windows). On Linux, the usual windowing libraries
(Wayland or X11, and libxkbcommon) must be installed.

Setting up another machine or handing work over between machines: see
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) and run `tools/dev-setup.sh`.

## Running

### Point Tessera at your install

Tessera looks for your game directory in this order:

1. a `--game-dir <path>` option, where a binary supports it;
2. the `TESSERA_GAME_DIR` environment variable;
3. the path saved in Tessera's settings (explorer);
4. automatic detection: Steam libraries (native, Flatpak, Proton) and common Wine/Proton
   prefixes.

The directory is the one that contains `client_general.dat`. For a Steam install on Linux
that is typically:

```sh
export TESSERA_GAME_DIR="$HOME/.local/share/Steam/steamapps/common/Lord of the Rings Online"
# or let detection find it:
export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
```

Tests read only `TESSERA_GAME_DIR` (see [Tests against real data](#tests-against-real-data)).

Tessera opens these files read-only and never writes, renames, locks or patches anything
in that directory. Everything it writes (settings, logs, caches, plugin data) goes to its
own per-user directories.

### Inspect the archives

```sh
cargo run -p tessera-dattool -- info "$TESSERA_GAME_DIR"
```

`info` summarises every `client_*.dat`/`.datx` archive: kind, region, block size, version
words and entry count. Other subcommands list entries (`ls`), count record types (`stat`),
hexdump a record by its data ID (`cat`), print a string table (`strings`), write a record
(or a texture as PNG) to a file **outside** the install for personal study (`extract`), find
installs (`detect`), and verify the archives' structure (`validate`, with `--inflate` to
decompress every record). Run
`cargo run -p tessera-dattool -- --help` for the current list.

### Run the client

```sh
cargo run --release -p tessera-client
```

With an install, the client is the **world walker** (M3, with M4 characters): it streams
landblocks with terrain, static objects and the NPCs the data places around you, shows the
area name, map coordinates, `/loc` and a minimap, and lets you walk or fly as an animated
avatar (Ctrl+F toggles free-fly; Shift is faster). Choose where to start with
`--spawn R,X,Y[,x,y[,z]]` (a landblock) or `--place "<area name, map coordinates or a
pasted /loc line>"` (with `--region N`), and `--fly` to start flying; `--avatar` picks the
avatar's body and items and `--crowd N` adds N animated characters. Headless forms, for
scripts and machines without a display:

```sh
cargo run --release -p tessera-client -- --screenshot /tmp/walker.png   # render the spawn, save, exit
cargo run --release -p tessera-client -- --fly-path 100:100,140:100 --frame-log /tmp/frames.csv
cargo run --release -p tessera-client -- --smoke                         # test scene, offscreen
```

Without an install (or with `--no-data`), and with `--showcase`, it renders an original
test scene instead, so you can try it before installing the game. `--help` lists every
option.

### Browse the archives

```sh
cargo run --release -p tessera-explorer
```

The explorer shows the archive tree, DID search, a hex view, textures (mip levels,
channels, alpha), layered icons, string tables with their variables, and property sets
with names and enum labels. Since M2 it also has a 3D view for meshes, setups and objects
(orbit camera, wireframe, normals, UV checker, material inspector, dye preview), and since
M4 a character viewer (any playable body, worn and held items, dyes, sliders) with an
animation browser; `--sheet` renders a grid of play times and camera angles headless. It
remembers the game directory in Tessera's own settings file. Headless:
`xvfb-run -a cargo run --release -p tessera-explorer -- --screenshot /tmp/explorer.png`.

### Logging

Library crates log through `tracing`. Set `RUST_LOG` to control the output, for example:

```sh
RUST_LOG=info,tessera_dat=debug cargo run -p tessera-dattool -- info "$TESSERA_GAME_DIR"
```

### Tests against real data

Most tests use synthetic archives and records that our own code generates. Tests that need
real game data read only `TESSERA_GAME_DIR` (not auto-detection) and **skip**, reporting
"passed", when it is not set:

```sh
TESSERA_GAME_DIR=/path/to/install cargo test --workspace
```

Those tests only open files read-only and only check structure and counts. Nothing derived
from game data is ever written into the repository.

---

## Repository layout

| Path | Contents |
|---|---|
| `crates/dat` | Archive container: header, extents, B-tree directory, zlib, archive set, DID routing |
| `crates/formats` | Record decoders: primitives, string tables, surfaces, property sets, setups, landblocks, entity descriptors, appearance records, items, avatar bodies, script tables and the idle/walk/run clip resolver |
| `crates/install` | Install detection and version identification |
| `crates/render` | wgpu renderer (mesh, material, terrain and skinned pipelines) |
| `crates/script` | Sandboxed Lua 5.1 host |
| `crates/net` | Transport-agnostic session layer with a local sandbox backend and the test-server backend (QUIC, snapshot interpolation) |
| `crates/protocol` | Tessera's own network protocol (`docs/specs/tessera-protocol.md`): messages, framing, encode/decode, session state machines, key pinning, QUIC setup |
| `crates/props` | Property registry, property sets, enum labels, DID maps |
| `crates/text` | String tables, variable substitution, grammar tags, locale selection |
| `crates/assets` | Asset worker pool with priorities, cancellation and a decoded cache |
| `crates/material` | Surface/material/shader records, effective parameters, feature flags, dyes, layered icon composition, mip-chain helpers |
| `crates/mesh` | Mesh records: vertex formats, engine-neutral vertex streams (static and skinned), detail levels, collision data, bounds, a cached mesh library; character and object assembly |
| `crates/havok` | Clean-room reader for the Havok tagfile subset in the archives: skeletons and animation clips |
| `crates/anim` | Skeletons, clip sampling and blending, root motion, attach points, bone sliders, skinning palettes, worker scheduler |
| `crates/entity` | The client's entity store (decision 0008) |
| `crates/wstate` | Script class library (WLib) and object-state (WState) records; area/region/dungeon resolution |
| `crates/world` | Coordinates and map coordinates, landblock IDs, streaming rings, landblock bundles, block-map lookups, area names, minimap tiles |
| `crates/terrain` | Heightmap meshing with LODs and stitched seams, seamless normals, shading, splatting inputs, CPU height queries |
| `crates/input` | Actions, contexts, bindings, Tessera's keymap file, fly and walk controllers |
| `crates/settings` | Tessera's own settings file |
| `apps/dattool` | `tessera-dattool` inspection CLI |
| `apps/client` | `tessera-client`, the game client (currently the offline world walker with an animated avatar and placed NPCs) |
| `apps/explorer` | `tessera-explorer`, the GUI asset browser |
| `apps/server` | `tessera-server`, the headless test server for Tessera's own protocol, with an original default world (private and LAN testing only) |
| `apps/loadtest` | `tessera-loadtest`, load, soak and hostile-input tests against a test server |
| `fuzz/` | Fuzz targets, one per parser |
| `tools/` | Machine setup (`dev-setup.sh`), checks run by CI (`check-content.sh`, `check-spdx.sh`, `check-dco.sh`, `fuzz-smoke.sh`), research-clone fetcher, ImHex patterns and Kaitai schemas written from our specs ([tools/README.md](tools/README.md)) |
| `docs/` | Architecture, roadmap, legal, clean-room process, tooling, specs |
| `research/` | Dirty-room notes. **Implementers must not open this directory.** |

More crates arrive with later milestones (audio, UI layouts, the
plugin host on top of `crates/script`); see
[docs/ARCHITECTURE.md §3](docs/ARCHITECTURE.md#layers).

---

## Documentation

| Document | What it covers |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Crate layers, data flow from install to frame, threading, DID routing, property and scripted-object systems, testing strategy |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones M0–M8: deliverables, specs, research tasks, exit criteria |
| [docs/specs/](docs/specs/) | Clean-room format and protocol specifications, the only input for implementation |
| [docs/LEGAL.md](docs/LEGAL.md) | Legal position, risk tiers, the official-server gate, naming and content rules |
| [docs/CLEAN_ROOM.md](docs/CLEAN_ROOM.md) | Zones, roles, spec lifecycle, AI-assistance rules, commit trailers, attestation |
| [docs/TOOLING.md](docs/TOOLING.md) | Tools for building, testing and research, by zone and milestone |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Machine setup, multi-machine hand-off, running a milestone with Claude Code |
| [docs/USER_GUIDE.md](docs/USER_GUIDE.md) | Testing Tessera on your own install: the walker, explorer and dattool, their options and controls, the keymap file, where files go, reporting problems |
| [docs/decisions/](docs/decisions/) | Decision records (stack, data handling, server gate, Bevy, multi-device workflow) |
| [docs/checklists/](docs/checklists/) | Visual checklists and items only the owner can observe in the original client |
| [DISCLAIMER.md](DISCLAIMER.md) | Non-affiliation statement and warranty disclaimer |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute, required reading, commit rules |
| [CLAUDE.md](CLAUDE.md) | Short form of the project rules, also used by AI agents |

## Built on published research

Tessera stands on years of public community work on these formats: LotroCompanion's data
tools, the lotro_extractor and lotro_mesh_viewer projects, Asheron's Call tools from the
same engine family, Havok-format parsers, the Maestro ABC tools, and the community's
plugin API references. We use them **for facts only**: the facts go into our specs with
a provenance entry, and our code is written fresh from the specs. The list of sources and
how each one is used is in [docs/ROADMAP.md §2.3](docs/ROADMAP.md#method-sources) and
[docs/TOOLING.md §9](docs/TOOLING.md#9-verified-prior-art-and-how-we-use-it).

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) first. Every contributor works either on
specs or on implementation for a given subsystem, never both, and every commit is signed
off under the DCO. Never add game data of any kind to the repository.

## Licence

Tessera is free software, licensed under the **GNU General Public License, version 3 or
(at your option) any later version** (`GPL-3.0-or-later`). It comes with **no warranty**.
Every source file carries an SPDX licence header.
