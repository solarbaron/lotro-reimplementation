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

**Milestone M0 (base) is in progress.** Nothing here is playable yet.

| Area | State |
|---|---|
| Archive reader (`.dat`/`.datx` container, directory, compression, DID routing) | in progress (M0) |
| Record primitives, string tables, textures, property dictionary | in progress (M0) |
| Install detection (`TESSERA_GAME_DIR`, Steam libraries, Wine/Proton prefixes) | in progress (M0) |
| `tessera-dattool` inspection CLI | in progress (M0) |
| Window and wgpu renderer with a test scene | in progress (M0) |
| Sandboxed Lua 5.1 host skeleton | in progress (M0) |
| Asset browser, mesh viewer, offline world walker, characters, UI and plugins, audio, own test server | planned (M1–M7) |
| Official-server compatibility | **gated** behind legal review ([LEGAL.md §9](docs/LEGAL.md#9-gates-and-the-official-server-target)); no code exists |

The full plan, with exit criteria per milestone, is in [docs/ROADMAP.md](docs/ROADMAP.md).

---

## Building

You need a Rust toolchain from [rustup](https://rustup.rs/). The repository pins the
channel and components in `rust-toolchain.toml` (stable, edition 2024, MSRV 1.90).

```sh
cargo build --workspace
cargo test --workspace
```

The client additionally needs a GPU driver that wgpu supports (Vulkan on Linux and
Windows, Metal on macOS, or DX12 on Windows). On Linux, the usual windowing libraries
(Wayland or X11, and libxkbcommon) must be installed.

## Running

### Point Tessera at your install

Tessera looks for your game directory in this order:

1. the `TESSERA_GAME_DIR` environment variable;
2. a `--game-dir <path>` option, where a binary supports it;
3. the path saved in Tessera's settings;
4. automatic detection: Steam libraries (native, Flatpak, Proton) and common Wine/Proton
   prefixes.

The directory is the one that contains `client_general.dat`. For a Steam install on Linux
that is typically:

```sh
export TESSERA_GAME_DIR="$HOME/.local/share/Steam/steamapps/common/Lord of the Rings Online"
```

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
cargo run -p tessera-client
```

At M0 the client opens a window, reports the install it found, and renders a test scene.
It also runs without any game data, so you can try it before installing the game.

### Logging

Library crates log through `tracing`. Set `RUST_LOG` to control the output, for example:

```sh
RUST_LOG=info,tessera_dat=debug cargo run -p tessera-dattool -- info "$TESSERA_GAME_DIR"
```

### Tests against real data

Most tests use synthetic archives and records that our own code generates. Tests that need
real game data read `TESSERA_GAME_DIR` and **skip** when it is not set:

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
| `crates/formats` | Record decoders: primitives, string tables, textures, property dictionary |
| `crates/install` | Install detection and version identification |
| `crates/render` | wgpu renderer |
| `crates/script` | Sandboxed Lua 5.1 host |
| `crates/net` | Transport-agnostic session layer with a local sandbox backend |
| `apps/dattool` | `tessera-dattool` inspection CLI |
| `apps/client` | `tessera-client`, the game client |
| `fuzz/` | Fuzz targets, one per parser |
| `tools/` | ImHex patterns and Kaitai schemas written from our specs |
| `docs/` | Architecture, roadmap, legal, clean-room process, tooling, specs |
| `research/` | Dirty-room notes. **Implementers must not open this directory.** |

More crates arrive with later milestones (assets, world streaming, terrain, meshes,
animation, materials, audio, UI, input, settings, plugin host); see
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
