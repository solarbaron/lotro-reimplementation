<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera architecture

> Status: founding document, written at milestone M0 (2026-10-03). Update this file in the
> same change as anything that alters a crate boundary, a thread boundary or a data flow
> described here.

<a id="purpose"></a>
## 0. Purpose and how to read this document

Tessera is a clean-room, from-scratch game client that reads the data files of a user's
own, legally obtained installation of *The Lord of the Rings Online*™ **in place and
read-only**. This document describes how the client is structured:

- the layers and crates,
- how bytes in the user's archives become pixels, sound and UI on screen,
- which thread does what,
- how the game's data-ID (DID) routing, property system and scripted-object (WState)
  system map onto Rust modules,
- which crates come after the M0 base,
- how the whole thing is tested without committing any game data.

It is an **architecture** document. Byte-exact layouts belong in `docs/specs/*.md`, and
this document points to them instead of repeating them. Where a section mentions a format
fact, the fact is here only because it shapes the architecture.

This file is in the clean room: implementers may read it. It contains no code from any
third-party source and no decompiler output.

### 0.1 Fact labels

| Label | Meaning |
|---|---|
| **Confirmed** | Observed directly in the user's install on 2026-10-03 (header and directory probes, file listings). Black-box observation of data files only. |
| **Reported** | Taken from published sources and research notes, not yet checked against real data. |
| **Decision** | A project design choice. It can be changed by a decision record ("ADR") in `docs/decisions/`. |
| **> OPEN:** | Unknown or undecided. Must be resolved by a spec or ADR before the dependent milestone exits. |

<a id="provenance"></a>
### 0.2 Provenance

| Fact used in this document | Source(s) | Status |
|---|---|---|
| Archive set: `client_{general,gamelogic,anim,mesh,sound,surface,highres,local_English}.dat`, aux `client_{mesh_aux_1,sound_aux_1,surface_aux_1..2,highres_aux_1..3}.datx`, `client_cell_{1..5,14,15}.dat`, `client_map_{1..5,14,15}.dat` | Observed in user install 2026-10-03; LotroCompanion dat-utils naming (research/notes/dat-format.md §1) | Confirmed |
| Every archive has magic `0x5442` at file offset 0x140, and the header file-size field equals the real file length | Observed in user install 2026-10-03; LotroCompanion, jtauber/lotro, Chorizite DatReaderWriter (dat-format.md §2) | Confirmed |
| No archive reaches 4 GiB (largest is 4,287,627,264 bytes); big content types are split into `.datx` aux archives | Observed in user install 2026-10-03; dat-format.md §1 | Confirmed (size limit); Reported (reason: 32-bit offsets) |
| Block size differs per archive (256, 1024, 4096, 17408, 24000, 40960 bytes) | Observed in user install 2026-10-03 | Confirmed |
| The header word at 0x14C identifies the archive kind, and the word at 0x150 holds the region number for cell/map archives | Observed in user install 2026-10-03; AC "DataSet/SubSet" naming (analogues.md §1.1) | Confirmed (values); Reported (meaning) |
| The root directory node of each archive holds DIDs whose high byte matches the routing table (0x25 in local, 0x70/0x79 in gamelogic, 0x06 in mesh and mesh_aux_1, 0x41 in surface and highres, 0x0A in sound, 0x05 in anim, 0x80 in cell/map) | Observed in user install 2026-10-03; LotroCompanion `DATFilesDirectory` (data-semantics.md §1) | Confirmed |
| DID = u32 whose high byte is the record type; routing by type to an ordered list of archives | data-semantics.md §1, dat-format.md §6 | Reported, partly Confirmed (row above) |
| Landblock DIDs of the form `0x80TRXXYY` (T = record kind, R = region, XX/YY = block coordinates) | data-semantics.md §1; root DID `0x8001B000` in cell_1 observed 2026-10-03 | Reported, shape Confirmed |
| Property dictionary at DID 0x34000000; 22 property type codes; property sets at `DID + 0x09000000` | data-semantics.md §4–5 (LotroCompanion, lotro_extractor) | Reported |
| Scripted-object model: class library (WLib) at 0x56000000, object state (WState) at 0x70000000–0x77FFFFFF | data-semantics.md §5, engine-client-re.md §9 | Reported |
| Landblock 160 × 160 units; region grid up to 256 × 256 blocks; 33 × 33 height samples per block | data-semantics.md §6, engine-client-re.md §4 | Reported |
| Meshes Z-up; skinned vertex strides vary per submesh; Havok tagfile skeletons and animation clips | data-semantics.md §7–8 | Reported |
| Texture formats are D3DFORMAT values (R8G8B8, A8R8G8B8, A8, DXT1/3/5) plus embedded JPEG | data-semantics.md §7.4, dat-format.md §7 | Reported |
| Audio payloads are RIFF/WAVE or Ogg Vorbis | data-semantics.md §9 | Reported |
| The original 64-bit client ships `lua51.dll`, `OpenAL32.dll`/`soft_oal.dll`, `vorbis*.dll`, `xerces-c_2_6.dll`, `umbra.dll`, `binkw64.dll` and D3D10/D3D11/OpenGL graphics-core DLLs | File listing of `x64/` in user install 2026-10-03; engine-client-re.md §1 | Confirmed (file names only; binaries were not opened) |
| Plugin system: Lua 5.1, "apartments" as separate Lua states, `.plugin` XML descriptors, per-scope persisted plugin data | engine-client-re.md §5 (official public Lua docs, lunarwtr API dump) | Reported |
| UI layouts are data records (0x22), user skins are `SkinDefinition.xml` + TGA | engine-client-re.md §6, data-semantics.md §11 | Reported |
| Separation of format libraries, per-format CLI tools, gated real-data tests, golden images, fuzzing | analogues.md §4 (OpenMW, OpenRW, OpenSAGE, DatReaderWriter, ClassicUO) | Reported (practice) |
| Risk tiers T0–T4 and the official-server gate | `docs/LEGAL.md` §8–§9 (from research/notes/legal.md §7) | Reported |

---

<a id="principles"></a>
## 1. Design principles

1. **Read in place, read-only.** The client never writes, renames, locks, patches, copies
   or repacks anything in the install directory. Everything the client writes (settings,
   caches, logs, plugin data) goes to Tessera's own directories (§17).
2. **Format libraries are separate from the engine.** `tessera-dat` and `tessera-formats`
   know nothing about windows, GPUs or threads. Tools and the client share them. This is
   the pattern of OpenMW `components/`, OpenRW `rwcore` and ClassicUO `Assets` (Reported,
   analogues.md §4.1).
3. **Hostile input everywhere.** Every byte from an archive, a network peer, a skin file or
   a plugin is untrusted. Parsers return `Result`, never panic, and bound every allocation
   by the bytes actually available (a count field can never cause an allocation larger than
   the remaining input could justify).
4. **Data-driven, not hard-coded.** Property IDs, enum values, archive routing and class
   layouts come from the user's data at run time wherever the data carries them. The code
   holds names (for example a property name) and resolves them through the dictionary
   (§7), so patches that renumber things do not break us.
5. **Pure decoders, impure edges.** A decoder is a pure function from bytes to a value.
   I/O, caching, threading and GPU upload live in other layers. That makes every decoder
   unit-testable with synthetic bytes and fuzzable.
6. **Each layer runs headless.** Everything below the client app runs without a window,
   GPU or audio device, so CI can test it.
7. **Works without game data.** The client starts, opens a window and renders an original
   test scene when no install is configured. This keeps substantial non-infringing use and
   keeps CI honest.

<a id="legal-shape"></a>
## 2. Legal constraints that shape the architecture

`docs/LEGAL.md` is authoritative and `docs/CLEAN_ROOM.md` defines the process. These are the
constraints that show up as structure:

| Constraint | Architectural consequence |
|---|---|
| No game data in the repo | Fixtures are generated by our own code (§23). Real-data tests read `TESSERA_GAME_DIR` and skip when it is unset. |
| Never modify the install | `tessera-dat` opens files read-only and exposes no write API against a user install. A writer, if one is ever built, is a test-only feature that targets temp files. |
| No hostnames, keys or bulk data tables extracted from the client binary (numeric algorithm constants are allowed: decision 0012) | `tessera-net` has no built-in endpoints for any official service. Anything of that kind would have to be loaded at run time from the user's install, and only in the gated compatibility tier. |
| Official-server connectivity is the highest-risk tier (T4, LEGAL.md §8) and is gated (LEGAL.md §9.3) | The network layer is a trait with interchangeable backends (§19). No compatibility backend exists before the gate passes; if one is ever built it is not a default workspace member and not built by CI. |
| No marks in product names | Crates, binaries, Rust identifiers and UI strings use "Tessera" and neutral words. The plain-text phrase "LOTRO data files" is fine in docs. Where plugin compatibility needs the original Lua namespace names, those names appear only as interop identifiers inside the Lua API surface (§18). |

---

<a id="layers"></a>
## 3. Layered crate map

```
                         ┌────────────────────────────────────────────────┐
 apps                    │ client      dattool      explorer*    server*  │
                         └──────┬────────────┬───────────┬───────────┬────┘
                                │            │           │           │
 L5 application glue     ┌──────▼──────┐     │           │           │
                         │ client app  │     │           │           │
                         └──┬───┬───┬──┘     │           │           │
 L4 presentation & host     │   │   │        │           │           │
     render  audio*  ui*  input*(→settings)  settings*  plugins*(→script)  net(backends)  entity*
        │      │      │     │                   │               │
 L3 world & assets   world*(→physics)  physics*  terrain*       │
                     anim*(→mesh)  mesh*  material*             │
        └──────┴──────┴──────┬─────────────────────────────────┘
 L2 asset system          assets*   props*/wstate*/text*/wvm*
                             │
 L1 record decoders       formats  (+ havok*)
                             │
 L0 container & install   dat      install
```
`*` = planned after M0.

Rules:

- A crate may depend only on crates in lower layers, plus siblings explicitly allowed
  below. No cycles. These rules govern `[dependencies]`; test-only `[dev-dependencies]`
  (for example `terrain`'s golden-image test through `render`, or `entity`'s tests through
  `world` and `anim`) are exempt.
- Explicit sibling allowances (the arrows in the diagram; no others exist, and a new one is
  added here with its reason in the same change):
  - `plugins` → `script` (L4);
  - `input` → `settings` (L4, M3): saving the keymap file goes through the settings
    crate's guard that refuses paths inside a game install (§16, §17);
  - `anim` → `mesh` (L3, M4): the figure's shape inputs come from the setup/part assembly
    (`tessera-mesh::assembly`, §11);
  - `world` → `physics` (L3, from M4.5, [0013](decisions/0013-physics-crate.md)): scenery
    generation reads the terrain collision mesh and its queries, and landblock bundles carry
    collision sets built on the asset workers. The allowance is one-way: `physics` depends
    only on `formats` and `props` (L1–L2) and never on `world`, `terrain`, `assets`, `render`
    or any other L3/L4 crate, and it has no GPU, asset-server or streaming dependency.
- L0–L2 crates have no GPU, window or audio dependencies.
- `render`, `audio` and `ui` do not depend on each other. The client app wires them
  together.
- `net` depends only on L0–L2 types it needs for its event vocabulary (property values,
  positions). It never depends on `render`, `ui` or `audio`.
- Tools (`dattool`, `explorer`) may use any library crate. The client must never depend
  on a tool.

<a id="crate-table"></a>
### 3.1 Crate inventory

| Crate (path) | Layer | Exists at | Responsibility |
|---|---|---|---|
| `tessera-dat` (`crates/dat`) | L0 | M0 | Archive header, extent/block layer, B-tree directory, zlib inflate, archive set, DID routing. Read-only. |
| `tessera-install` (`crates/install`) | L0 | M0 | Find installs (env var, CLI, Steam libraries, Wine/Proton prefixes), validate them and identify the data version. |
| `tessera-formats` (`crates/formats`) | L1 | M0 | Record decoders on top of `dat`: primitives (variable-length integers, sized lists, strings), string tables, render surfaces (textures), property dictionary. Grows by module per record type. |
| `tessera-havok` (`crates/havok`) | L1 | M4 | Clean-room reader for Havok binary tagfile (version 3) data as stored in the archives: skeletons and animation clips (spline-compressed and uncompressed). |
| `tessera-props` (`crates/props`) | L2 | M1 | Property registry, typed values, property sets, enum mappers, DID maps (§7). |
| `tessera-text` (`crates/text`) | L2 | M1 | String-table lookup, StringInfo resolution, `${VAR}` substitution and grammar-tag selection. |
| `tessera-wstate` (`crates/wstate`) | L2 | M3 | Class library (WLib) schema and the generic scripted-object graph (WState) (§7.4). From census 2 also typed views of the player-character records (`player-character.md`) and of the game-control singletons (`game-controls.md`). |
| `tessera-wvm` (`crates/wvm`) | L2 | census 2 | Script machine of the class library: decodes and validates the compiled code (chunk 1 of 0x56000000) and its message list (chunk 2), resolves function IDs and slots against the class library, disassembles, counts and cross-references function bodies (`wlib-bytecode.md`). Decoder level only; no interpreter yet. Depends on `formats` and `wstate` (the L2 scripted-object group, like `wstate` on `props` and `text`). |
| `tessera-assets` (`crates/assets`) | L2 | M1 | Asset server: typed handles, decode jobs on worker threads, caches, priorities, cancellation, GPU-upload hand-off (§8). |
| `tessera-world` (`crates/world`) | L3 | M3 | Coordinates, landblock and cell IDs, streaming rings, landblock info (cells, static entities, links), block maps (§9). From M4.5 also: scenery generation and the frill (grass) ring, landblock water and water volumes, the game clock and sky state (§10), light selection (`lighting.md` §8.3–§8.5), per-class draw distances (`walker.md` §11.4). Builds each landblock's collision set, water included, on the asset workers and hands it to `physics` (§10.3, §20.3). May depend on `physics` (sibling allowance, §3). |
| `tessera-physics` (`crates/physics`) | L3 | M4.5 | Collision and movement: terrain and ceiling collision triangles with the client's numbering, the terrain radius and plane queries that scenery placement uses, cell volumes and the point-in-cell lookup, per-landblock collision sets (statics, generated scenery, cells) with Tessera's own bounding-volume hierarchy, capsule sweeps, and the character controller with its water states (`collision.md` §5.4, §5.8.1, §6, §9–§12; `water.md` §7.4). A collision set also carries its landblock's water plane and liquid boxes, so the controller reads water only from its own sets (§10.3). Owns the step constants (1/60 s, at most 5 per frame) and validates every input at its API boundary: non-finite or out-of-range movement parameters, bounded speeds and per-step displacement, no panicking clamps ([0013](decisions/0013-physics-crate.md)). Sets are keyed by `formats::LandblockId` in landblock frames; the mover is in the f64 region frame (§9.1). Depends only on `formats` and `props`; no GPU, asset-server or streaming dependency. Record decoders (quantised trees, 0x66, 0x01) stay in `formats`. The M7 test server reuses the controller and the collision-set types; building the sets from install data is `world`'s work (§10.3, §10.5). |
| `tessera-terrain` (`crates/terrain`) | L3 | M3 | Heightmap meshing, terrain LOD bands, far-terrain imposters; from M4.5 the texture layers of the region's terrain type table, alpha masks, per-layer texture coordinates and tangent frames, ceiling chunks and light-map coordinates (§10). |
| `tessera-mesh` (`crates/mesh`) | L3 | M2 | Static and skinned mesh (GfxObj) decoding into engine vertex streams; setup/part assemblies (§11). |
| `tessera-anim` (`crates/anim`) | L3 | M4 | Skeleton hierarchy, pose sampling, blending, skinning palettes, attach points (§11). |
| `tessera-material` (`crates/material`) | L3 | M2 | Surface → material → texture-list → texture chains, shader-feature classification, dye rules (§12). |
| `tessera-entity` (`crates/entity`) | L4 | M4 | Main-thread entity store: generational entity IDs, sparse-set component columns, the client's component sets (transform, network ID, render and animation handles as type parameters, landblock membership of streamed statics), per-frame change and despawn lists, deferred removal ([0008](decisions/0008-entity-storage.md)). Depends on no other Tessera crate; L4 because it holds the client's game state, which lower layers never see. |
| `tessera-render` (`crates/render`) | L4 | M0 | wgpu renderer: device and surface, frame graph, pipelines and WGSL shaders, texture/mesh uploads, headless mode (§13). |
| `tessera-audio` (`crates/audio`) | L4 | M6 | Mixer, 3D sound, music regions, Vorbis/WAV decoding, player-music (ABC) synthesis (§14). |
| `tessera-ui` (`crates/ui`) | L4 | M5 | Retained widget tree built from UI layout records, user skins and plugin controls; text layout; 2D batching (§15). |
| `tessera-input` (`crates/input`) | L4 | M3 | Raw input → actions via a keymap; input contexts; gamepad (§16). |
| `tessera-settings` (`crates/settings`) | L4 | M1 | Tessera's own settings file (INI-shaped), with a read-only importer for the original client's preferences file (§17). |
| `tessera-script` (`crates/script`) | L4 | M0 | Sandboxed Lua 5.1 host: state creation, library allow-list, limits, binding helpers (§18). |
| `tessera-plugins` (`crates/plugins`) | L4 | M5 | Plugin discovery (`.plugin` XML), apartments, lifecycle, the compatible Lua API surface, plugin-data persistence (§18). |
| `tessera-net` (`crates/net`) | L4 | M0 | Transport-agnostic session layer: `Backend` trait, domain events, sandbox backend; later the test-server backend (§19). |
| `tessera-protocol` (`crates/protocol`) | L1 | M7 | Tessera's own wire protocol: message schema, framing, versioning. Shared by the client backend and the test server. |
| `tessera-client` (`apps/client`) | app | M0 | The game client binary: window, frame loop, wiring (§20). |
| `tessera-dattool` (`apps/dattool`) | app | M0 | CLI: `detect`, `info`, `ls`, `stat`, `cat`, `extract`, `strings`, `validate`, later `diff` (§24). |
| `tessera-explorer` (`apps/explorer`) | app | M1 | GUI asset browser: archives, strings, textures, icons, later meshes and maps (§24). |
| `tessera-server` (`apps/server`) | app | M7 | Project-owned test server speaking the Tessera protocol with original test content (§19). Runs server-side movement on `tessera-physics` with the same fixed step as the client (§20.3). It moves exactly like the client only over the same collision sets: on install data these include generated scenery, statics and water, which `world` builds (§10.3, §10.5). |

> **OPEN (M7, [0013](decisions/0013-physics-crate.md)):** whether `apps/server` depends on
> `tessera-world` for collision-set building on install data, or the asset-free part of that
> building (sets from decoded records, scenery placement, the `water.md` §6.2 rule) moves
> into a module the server can use without `world`'s asset, animation and text
> dependencies. Decide with `server-content.md`; until then `physics` alone does not give
> movement identical to the client's.

> **OPEN:** whether `props`, `text` and `wstate` stay separate crates or become modules of
> one `tessera-gamedata` crate. Decide in M1 when `props` is written; the module boundaries
> in §7 hold either way.

---

<a id="install"></a>
## 4. The data source: the user's install

<a id="install-detect"></a>
### 4.1 Locating an install (`tessera-install`)

Search order (Decision). The first candidate that validates wins:

1. `TESSERA_GAME_DIR` environment variable.
2. A `--game-dir <path>` command-line option of the binary in use.
3. The path saved in Tessera's own settings (§17).
4. Steam libraries: parse the Steam library-folder list in each known Steam root (native,
   Flatpak, and Proton/Wine Steam), look for the game's app directory under
   `steamapps/common/`.
5. Common Wine and Proton prefixes and launcher-managed prefixes (OneLauncher, Lutris,
   Bottles) on Linux; the default Windows install locations and registry uninstall entries
   on Windows.

Validation: the directory contains `client_general.dat`, and that file's header carries the
`0x5442` magic at 0x140 (Confirmed). Identification records the header's engine-pack and
game-pack version words and the set of archives present, so tests and caches can key on
the data version.

The installer for the original game also leaves unrelated `.dat` files in the root
(`icudtl.dat`, `unins000.dat`, Confirmed). The archive scanner therefore matches only
`client_*.dat` and `client_*.datx`, and validates the magic of each.

<a id="install-archives"></a>
### 4.2 The archive set (Confirmed, user install 2026-10-03)

| Archive | Kind word @0x14C | Region/subset @0x150 | Block size | Root-node DID types seen |
|---|---|---|---|---|
| `client_gamelogic.dat` | 0x01010000 | 0 | 256 | 0x70, 0x79 |
| `client_mesh.dat`, `client_mesh_aux_1.datx` | 0x01020000 | 0 | 17408 | 0x06 |
| `client_general.dat` | 0x01030000 | 0 | 1024 | 0x1F, 0x2A, 0x31, 0x40 |
| `client_anim.dat` | 0x01040000 | 0 | 24000 | 0x05 |
| `client_sound.dat`, `client_sound_aux_1.datx` | 0x01070000 | 0 | 17408 | 0x0A |
| `client_highres.dat`, `client_highres_aux_1..3.datx` | 0x01080000 | 0 | 40960 | 0x41 |
| `client_surface.dat`, `client_surface_aux_1..2.datx` | 0x01090000 | 0 | 4096 | 0x41 |
| `client_cell_{1..5,14,15}.dat` | 0x02030000 | region (1–5, 14, 15) | 256 | 0x80 (region 15: 0xFF) |
| `client_map_{1..5,14,15}.dat` | 0x020A0000 | region (1–5, 14, 15) | 256 | 0x80 (region 15: 0xFF) |
| `client_local_English.dat` | 0x03030000 | 2 | 256 | 0x25 |

Observations that matter for the design:

- The archive kind can be read from the header, so the archive set does not have to trust
  file names alone. `tessera-dat` should classify archives by header and warn when the file
  name disagrees.
- Aux archives carry the same kind word as their primary. Lookups for a type search the
  primary and all its aux archives (§5.2).
- Region 15 archives exist but are nearly empty (1 MiB, root node at 0x400) and hold a
  0xFFFF0001 record rather than landblocks. Routing must tolerate regions with no
  landblocks.
- The local archive's subset word is 2. The plugin API's language enum reports English = 2
  (Reported, engine-client-re.md §5), so this is plausibly a language ID.

> **OPEN:** exact meaning of the kind word's two significant bytes and of the subset word
> for local archives. Owner: `docs/specs/dat-container.md`.
> **OPEN:** locale handling when several `client_local_*.dat` files are installed. Reported
> behaviour of other tools: pick the newest; Tessera should pick by settings, then by
> language ID, then newest.

<a id="install-readonly"></a>
### 4.3 Read-only guarantees

- Files are opened read-only with default sharing on Windows (read, write and delete
  sharing allowed), so the official launcher can still patch while Tessera runs. No
  advisory locks are taken.
- **Decision:** `tessera-dat` reads with positioned reads (`read_at`-style), not memory
  maps. Reasons: the workspace denies `unsafe_code`, memory mapping needs it, and a mapped
  file that the official patcher truncates or rewrites would crash the process. A
  memory-mapped backend may be added later behind a feature with a reviewed, documented
  `unsafe` exception.
- Each archive records its header file-size field and file metadata at open. If either
  changes while open (the official patcher ran), the archive reports `Stale`, and the
  asset server drops caches for that archive and reopens it.
- Nothing in the workspace, including tests, constructs a writable handle to a path under
  the install directory. The dattool `extract` command writes only to a user-supplied
  output directory and refuses targets inside the install.

---

<a id="did-routing"></a>
## 5. Container layer and DID routing (`tessera-dat`)

Byte layouts: `docs/specs/dat-container.md`. This section covers structure only.

<a id="dat-modules"></a>
### 5.1 Modules

| Module | Responsibility |
|---|---|
| `header` | Parse and validate the superblock (magic, block size, file size, kind, subset, root offset, version words). |
| `extent` | Assemble a logical byte range from the 8-byte block header and fragment list (modern layout) or the legacy chained layout. Cycle and bounds checks; step limit. |
| `btree` | Decode directory nodes; lookup by DID; in-order iteration; node cache. |
| `entry` | Directory entry (DID, flags, offset, stored size, timestamp, iteration, allocated size). |
| `inflate` | Compressed-entry handling: length prefix, zlib stream, size check against the prefix, hard cap. |
| `archive` | One open file: header + reader + node cache. `Send + Sync`; reads need only `&self`. |
| `set` | `ArchiveSet`: every archive in an install, classified by kind and region. |
| `route` | DID → ordered list of archive keys (§5.2). |
| `did` | `Did` newtype with helpers: type byte, landblock decomposition. |

<a id="did-route"></a>
### 5.2 Routing model

A DID is a `u32`. Its high byte is the record type (Reported, partly Confirmed in §4.2).
Routing turns a DID into an ordered list of **archive keys** to search:

1. Landblock-space DIDs (`0x80TRXXYY`, Reported) route by the kind nibble T and region
   nibble R: heightmap, landblock-info and landblock-properties records go to the cell
   archive of region R; block maps go to the map archive of region R.
2. All other DIDs route by type byte through a **table**, not a `match` scattered over the
   code. The table lists, for each type range, the archive keys in search order (for
   example "textures: highres, then surface, then local"; "property sets: gamelogic, then
   local").
3. An archive key expands to the primary archive followed by its aux archives in numeric
   order.
4. Lookup searches each archive in order and returns the first hit, along with which
   archive answered (tools show this; patch-diff tooling needs it).

The table is data in `route` and comes from `docs/specs/dat-container.md#8-did--archive-routing`. Unknown
type bytes fall back to searching every archive, and log once at `debug`, so new record
types appearing after a patch are still reachable.

> **OPEN:** whether the archives carry their own routing information (a master map or the
> kind word) that could replace the static table. The kind words in §4.2 suggest the
> archive kind is self-describing; whether type→archive is self-describing is unknown.

<a id="dat-api"></a>
### 5.3 API shape

- `ArchiveSet::open(install) -> Result<ArchiveSet>` opens and classifies everything.
- `ArchiveSet::get(did) -> Result<Option<Record>>`: the record's raw bytes (inflated),
  plus entry metadata (archive key, iteration, timestamp).
- `ArchiveSet::entry(did)`: metadata only, no payload read.
- `Archive::iter()`: in-order directory walk, for tools and indexing.
- Buffers come from the caller or a pool. Decompressed size is capped by a configurable
  limit (default sized to the largest legitimate record, which the dat spec must state).

Indexing (Decision): lookups descend the B-tree on demand with an LRU cache of decoded
nodes. A full DID → (archive, entry) index built by walking every tree is optional. Tools
(`dattool ls`, explorer) build it on a background thread; the client does not need it.

---

<a id="formats"></a>
## 6. Record decoders (`tessera-formats`)

<a id="formats-conventions"></a>
### 6.1 Conventions

- One module per record family, named for what it is (for example `string_table`,
  `render_surface`, `texture_list`, `property_dictionary`, `enum_mapper`, `did_map`,
  `mesh`, `landblock`, `ui_layout`, `sound`).
- Each module exposes a `decode(bytes, ctx) -> Result<T, FormatError>` entry point. The
  context carries only what the format really needs (for example the property registry
  for property-set decoding) and never an `ArchiveSet`. Following references to other
  DIDs is the asset layer's job.
- A shared `Reader` cursor provides bounded little-endian reads and the primitive
  encodings: the variable-length integer, sized lists (one hint byte then a variable-length
  count), Latin-1 Pascal strings, length-prefixed UTF-16 strings, booleans that must be 0
  or 1. Spec: `docs/specs/records.md` §1 (primitive encodings).
- Every decoder checks that a record's leading DID matches the requested DID when the
  format carries one (Reported: most decoded records start with their own DID).
- Decoders report trailing unread bytes as a warning value, not an error, so tools can
  flag partially understood formats without the client failing.
- Every decoder has a fuzz target (`fuzz/fuzz_targets/<module>.rs`) and a synthetic
  round-trip test (§23).

<a id="formats-map"></a>
### 6.2 Module map by record type

| DID type | Record | Module | Milestone | Spec |
|---|---|---|---|---|
| — | primitives | `reader` | M0 | `records.md` §1 |
| 0x25–0x26 | string tables | `string_table` | M0 | `string-table.md` |
| 0x41 | render surface (texture, icon) | `render_surface` | M0 | `records.md` §5 |
| 0x34000000 | property dictionary | `property_dictionary` | M0 | `property-system.md` |
| 0x23 | enum mapper | `enum_mapper` | M1 | `records.md` §7 |
| 0x28 | DID map / directory | `did_map` | M1 | `records.md` §8 |
| 0x78–0x7F | property sets | `property_set` | M1 | `property-system.md` |
| 0x40 | texture list (mip chain) | `texture_list` | M1 | `texture-list.md` |
| 0x06 | mesh (GfxObj) | `mesh` | M2 | `mesh.md` |
| 0x30, 0x31 | material, surface | `material` | M2 | `material.md` |
| 0x20 | appearance (wardrobe) | `appearance` | M4 | `appearance.md` |
| 0x04, 0x1F, 0x47 | setup (with an embedded Havok skeleton tagfile), visual template, entity descriptor | `setup` | M2/M4 | `setup.md` |
| 0x01, 0x66 | cell geometry, stand-alone collision mesh, and the quantised collision tree they share with meshes and landblock data | `collision` | M4.5 | `collision.md` §3–§5 |
| 0x0F | region descriptor (names the per-region tables) | `landblock` (`region`) | M3/M4.5 | `environment.md` §3 |
| 0x19 | terrain type table | `terrain_type` | M4.5 | `terrain-texturing.md` §3 |
| 0x12, 0x11, 0x02 | terrain-scene table, scene-choice table, scene | `scenery` | M4.5 | `scenery.md` §4–§5 |
| 0x16, 0x17 | water table, fog-zone table | `water` | M4.5 | `water.md` §4–§5 |
| 0x15, 0x1A, 0x35, 0x32, 0x08 | sky description, day description, game time, LOD-class table, camera state | `environment` | M4.5 | `environment.md` §5, §6, §9.1; `walker.md` §11.5 |
| 0x800R/0x802R/0x804R, 0x801R | landblock data, info, properties; block map | `landblock` | M3 | `landblock.md` |
| 0x805R | distant-landscape sector (3 × 3 landblocks: terrain and ceiling meshes, water, billboards, far-visible entities) | `landblock` (`sector`) | census 2 | `distant-landscape.md` |
| 0x79 (property sets) | instanced and group content views: encounter templates and controls, skirmish, world join, battles, session play, fellowship maker, phasing | `instance` | census 2 | `instances.md` |
| 0x18 | block-map descriptors | `block_map` | M3 | `landblock.md` |
| — (inside 0x04), 0x05 | skeleton (the Havok tagfile slice of a 0x04 setup, handed over by `setup`), animation clip | `tessera-havok` | M4 | `havok-data.md` |
| 0x56000000 | class library (WLib); its compiled code and message chunks in `tessera-wvm` | `wlib` | M3 (code: census 2) | `wlib.md`, `wlib-bytecode.md` |
| 0x70–0x77 | object state (WState) | `wstate` | M3 | `wstate.md` |
| 0x22 | UI layout | `ui_layout` | M5 | `ui-layout.md` |
| 0x0A | sound info and sound payload | `sound` | M6 | `sound.md` |
| 0x07 | script tables (VFX, sound, animation events) | `script_table` | M4/M6 | `script-table.md` |

> **OPEN:** Kaitai `.ksy` schemas in `tools/kaitai/` as the single machine-readable source
> for formats, with generated documentation and differential tests. The Rust decoders stay
> hand-written (Kaitai's Rust target is immature, Reported analogues.md §4.3). Decide at
> M1 whether `.ksy` files are normative or illustrative.

---

<a id="property-system"></a>
## 7. Property, enum, text and scripted-object systems

The game's data model is property-driven (Reported, data-semantics.md §4, §12). Items,
quests, skills, areas and NPCs are scripted objects plus a property bag, not fixed C-style
structs. Tessera mirrors that with generic runtime types instead of one Rust struct per
game concept.

<a id="props-registry"></a>
### 7.1 Property registry (`tessera-props::registry`)

- Loaded once per `ArchiveSet` from the property dictionary record (DID 0x34000000,
  Reported) by `formats::property_dictionary`, then frozen behind an `Arc`.
- Maps `PropertyId` (u32 newtype) ↔ name, type code (22 types, Reported) and type data
  (the enum or bitfield DID that labels values, default/min/max, struct children).
- **Name resolution, not constants.** Code that needs a property asks for it by name once
  ("Name", "Area_Name", "UI_Map_MapImage", ...) and keeps the returned `PropertyId` in a
  small cache struct. Missing names are a `tracing::warn!` and a `None`, never a panic. No
  numeric property IDs are hard-coded in library code.

<a id="props-values"></a>
### 7.2 Values and property sets (`tessera-props::value`, `::set`)

- `Value` is an enum with one variant per property type (string, string token, waveform,
  timestamp, tri-state, vector, instance ID, enum value, float, property ID, struct, array,
  StringInfo, 64-bit bitfield, int, color, position, 32-bit bitfield, long, data-file DID,
  boolean, variable-width bitfield).
- `PropertySet` is an ordered map `PropertyId → Value`, plus lookups by name through the
  registry.
- Two decoders share the `Value` model: the **disk** encoding (property sets inside
  records) and the **wire** encoding (packed IDs, variable-length ints). Only the disk
  decoder exists until a network backend needs the wire one.
- The rule "the property set of scripted object D is at D + 0x09000000" (Reported) is a
  function on `Did`, used by the asset layer to fetch an object's properties.

<a id="props-enums"></a>
### 7.3 Enums, DID maps and text

| Concept | Module | Behaviour |
|---|---|---|
| Enum mapper (0x23) | `props::enums` | Key → internal label and key → localized StringInfo, with inheritance from a base mapper. Used to label enum and bitfield property values. |
| DID map (0x28) | `props::did_map` | Label → DID directories reachable from the root map 0x28000000 (Reported). This is how the client finds singleton control records by name rather than by hard-coded DID. |
| String table (0x25/0x26) | `text::table` | Token → text pieces and variable slots. |
| StringInfo | `text::info` | Literal UTF-16 or (token, table DID), plus substitution arguments. `text::render` resolves it to a `String` in the active locale. |
| Grammar tags | `text::grammar` | Selection among variant spellings by tags (gender, plural, race, class, vowel); spec in `string-table.md`. |
| Name hash | `text::hash` | The 28-bit name hash used for variable names and class attribute names; spec in `records.md` §1.5. |

<a id="wstate"></a>
### 7.4 Scripted objects (`tessera-wstate`)

- `ClassLibrary` (from WLib 0x56000000): classes by index, parent links, per-class
  attribute lists (name hash, slot index, type code). Attributes are not flattened by the
  data (Reported), so the library computes the flattened view once.
- `ObjectState` (from 0x70–0x77): an object graph of references, each either an instance
  ID, a DID, or an embedded object. Embedded objects are either library-defined (slots
  read by attribute type) or **native containers** (hash maps, arrays, sets, positions,
  property sets, strings, StringInfo). Native containers are chosen by class index through
  a table in the spec, and unknown native classes decode as opaque byte ranges with a
  warning rather than failing the whole object.
- `GameObject` = DID + class index + `ObjectState` + `PropertySet` (from DID + 0x09000000).
  The class index at byte offset 4 of the state record (Reported) tells tools what kind of
  object it is (item, quest, skill, effect, NPC, ...).
- Attribute names are only known as hashes. Tessera does not ship third-party name lists;
  names are recovered at run time where the data contains them, or shown as hashes.
- Typed views on top of these (census 2): `wstate::character` for the player-character
  records (`player-character.md`) and `wstate::game_controls` for the game-control
  singletons (`game-controls.md`). They read decoded property sets and object states and
  need no byte decoder of their own. The compiled code of the class library is decoded by
  `tessera-wvm` (`wlib-bytecode.md`).

The client runtime's entity model (§20) stores a `PropertySet` per live entity. Gameplay
features (tooltips, vitals, quest log) read properties by name, so the same code works for
data from the sandbox backend, the test server or a future backend.

---

<a id="assets"></a>
## 8. Asset system (`tessera-assets`)

<a id="assets-model"></a>
### 8.1 Model

- `AssetServer` owns the `ArchiveSet`, the property registry, a worker pool and caches.
- `Handle<T>`: a cheap, clonable, typed reference to an asset keyed by `Did` (plus a
  variant key where the same DID decodes to different products, for example a texture
  decoded at a capped mip level).
- Asset states: `Queued → Loading → Ready(Arc<T>) | Failed(error)`. Failure is a value
  that renders as a placeholder (checkerboard texture, unit cube, silence) and is logged
  once.
- Dependencies are explicit. An asset loader returns its product plus the DIDs it depends
  on (a mesh lists its materials, a material its texture lists). The server schedules
  those with the parent's priority.
- Products are CPU-side (`DecodedTexture`, `MeshData`, `Skeleton`, ...). GPU objects are
  created by `render` from CPU products on the main thread (§21).

<a id="assets-cache"></a>
### 8.2 Caching

| Tier | Contents | Policy |
|---|---|---|
| Directory node cache | Decoded B-tree nodes per archive | LRU by count; in `dat`. |
| Raw record cache | Inflated record bytes | Small LRU by bytes; avoids re-inflating during dependency fan-out. |
| Decoded cache | `Arc<T>` products | Weak references plus an LRU "keep-alive" budget in bytes. |
| GPU residency | Textures, buffers | Owned by `render`, budgeted in bytes, evicted by last-used frame. |
| Disk cache (optional) | Derived data that is expensive to compute (for example transcoded textures for GPUs without BC support) | In Tessera's cache directory, keyed by data version + DID + iteration. Never inside the install. Cleared on data-version change. |

> **OPEN:** whether the disk cache is worth it at all. Decide at M3 from measured
> decode times. The legal posture is that it is a local, per-user cache of the user's own
> data, never shared or uploaded; `docs/LEGAL.md` must confirm before it ships.

<a id="assets-priority"></a>
### 8.3 Priorities and cancellation

- Each request carries a priority: `Ui` (visible now) > `Near` (in the view frustum,
  near ring) > `Mid` > `Far` > `Prefetch`.
- The streamer (§9) re-prioritises requests every tick as the camera moves.
- Requests are cancellable. A cancelled job checks its token between stages (read →
  inflate → decode) and drops its work.
- Per-frame **upload budget** (bytes and milliseconds) on the main thread keeps frame
  time stable while streaming.

---

<a id="world"></a>
## 9. World model and streaming (`tessera-world`)

<a id="world-coords"></a>
### 9.1 Coordinates

| Quantity | Representation | Source |
|---|---|---|
| Region | u8 (observed regions 1–5, 14, 15) | Confirmed (archive subsets) |
| Landblock | (region, bx: u8, by: u8); 160 × 160 units | Reported |
| Landcell | 8 × 8 per landblock (20-unit cells) for outdoor addressing | Reported |
| Interior cell | u16 index within a landblock's cell list | Reported |
| Instance | u16 | Reported |
| Position | landblock + local f32 (x, y, z) + orientation quaternion; Z is up | Reported |
| World position | f64 (bx·160 + x, by·160 + y, z) for global math | Decision |
| Render position | f32 relative to a camera-anchored origin, rebased when the camera crosses a landblock | Decision |

The user-visible map coordinates are an affine function of world position (Reported); the
constants live in `docs/specs/coordinates.md`, not in code comments.

<a id="world-streaming"></a>
### 9.2 Landblock streaming

- The streamer runs once per frame on the main thread (§20.3 step 4). It is cheap: it
  computes the **desired set** of landblocks in concentric rings around the camera and
  diffs it against the resident set.
- Rings (Decision, defaults from Reported observations of the original's draw-distance
  behaviour): **near** ring with full terrain, static objects, grass and collision; **mid**
  ring with terrain and large objects only; **far** band with distant terrain/imposters.
  The radii are settings, not constants.
- For each newly desired landblock the streamer asks the asset server for the landblock
  bundle: heightmap record, landblock-info record (cells, static entity descriptors,
  links), block-map entry (area, music, ambient sound), landblock properties (minimap
  tile). The bundle loader then requests the meshes and materials those reference.
- Leaving landblocks are released with hysteresis (one ring of slack) to avoid thrash at
  boundaries.
- **Lookahead (Decision, M4.5).** A bundle with a large collision set can take longer to
  build than a fast camera needs to cross from the far to the near radius. The streamer
  therefore also loads along the camera's path: from its velocity (given by the caller or
  estimated from the last quarter second of positions) it takes the segment the camera
  covers in the next 4 s (at most 12,800 units) and makes the landblocks within the near
  radius of that segment desired, those beyond the far radius ahead of their ring. The
  first 1.5 s of the segment (longer, 1.5 times the slowest recent bundle job, after slow
  jobs) is *urgent*: its landblocks are requested with `Visible` priority; the rest with
  their ring's priority (`Prefetch` ahead of the rings), raised when the urgent part
  reaches them, so very slow bundles start seconds early whenever a worker is free
  without delaying the urgent work (only a job that has started gains: a queued request
  that is raised is ordered like a new one). Ahead landblocks are admitted and evicted by
  distance like far-ring landblocks; they lie beyond the far radius, so the residency
  budget cuts them before the near and mid rings and the far ring inside its radius,
  interleaved with far-ring members in the hysteresis band. They are released when they
  leave the path, and their bundles are not listed as resident (so nothing draws or
  populates them) until they enter the rings. The velocity estimate restarts on a step
  faster than 10,000 units per second (a teleport or a focus moved elsewhere), not on a
  long frame. Lookahead changes only when bundles load, never what they hold.
- Interior cells (dungeons, buildings) are loaded with their landblock. Visibility between
  cells (portals) is a renderer concern at M3+.
- **World systems in the bundle (M4.5, Decision).** The bundle job builds, on the same
  asset worker and in the order of `collision.md` §9.4: the terrain collision mesh and the
  landblock's water decision (`water.md` §6.2); the static list with its clearance volumes
  and the procedural scenery (`scenery.md` §7, its water ranges reading that water plane);
  the landblock water with the liquid boxes of statics and generated objects (`water.md`
  §7); the collision set of terrain, ceiling, blocking statics, blocking generated objects,
  cells and water (`collision.md` §12.1), with the cells and portals of landblock info
  (§5.8); the Light entities (`lighting.md` §8.3) and the LOD class of every placed and
  generated entity (`walker.md` §11.4.3). Generated objects enter the bundle as placed
  entities beside the statics and are drawn like them. The per-region tables named by the
  region descriptor's eight slots (`environment.md` §3), the scene library, the object
  caches, the sky library and the LOD-class table are loaded once into the world context
  and shared by every worker.
- **Residency (Decision).** A bundle's residency charge includes its collision set,
  generated scenery, clearance volumes, water and lights, so the one residency budget
  bounds them all. Collision is built for every resident landblock, not only the near ring
  (simpler than a second residency; measured mean about 0.75 – 1.3 MiB per bundle on the
  owner's install), and the default budget is 1 GiB.
- **Frills (Decision).** The frill ring of `scenery.md` §8.2 is separate from the landblock
  rings: it follows the blend of camera and player, and its 10 × 10-unit sectors are
  generated on the asset workers from the scenery inputs a resident bundle keeps, nearest
  first, a bounded number per frame and within their own byte budget. Frills never collide.
- **Lookups.** Over what is resident the streamer answers area, music, ambient sound, sky
  key and sky values, terrain height, minimap tiles, water ("under water", the water
  surface over liquid boxes), nearby lights, LOD values and draw distances, a collision
  view around a position (decision 0013 frames) and the interior cell containing a point
  (`collision.md` §5.8.1).

> **OPEN:** interior cell geometry, portals and the role of the original's PVS data are
> unknown (Reported as unexplored, data-semantics.md §13). M3 exit criteria do not depend
> on interiors. *Answered in part (M4.5):* cell geometry, cell volumes, portals and the
> point-in-cell lookup are specified in [`collision.md`](specs/collision.md) §5; the role
> of the PVS data is still unknown.

---

<a id="terrain"></a>
## 10. Terrain, terrain texturing, water and sky

This section says which crate does what for the ground, water and sky. The data layouts and
algorithms are in the specs it cites; it repeats only what shapes crate boundaries and data
flow. Grass and procedural scenery are in §10.5 because they hang off the terrain data.

<a id="terrain-mesh"></a>
### 10.1 Terrain meshing (`tessera-terrain`, M3)

- Input per landblock: a 33 × 33 height grid (five-unit spacing). Heights use the client's
  rule, stored × 2,000 / 65,535 (`landblock.md` §3.4; `terrain-texturing.md` §13); the
  ceiling grid, vertex flags and collision tree blocks that follow the heights are in
  `landblock.md` §3.5–§3.9.
- Output: a CPU mesh per landblock per LOD (shared index buffers per LOD and stitch
  pattern), normals computed across landblock edges (needs neighbour heights; the bundle
  loader requests the 8 neighbours at low priority) (`terrain.md` §3–§5). Landblocks with a
  ceiling also get a ceiling mesh (`terrain-texturing.md` §4.8).
- Far terrain: coarser LOD meshes from the same heights. How far the original draws full
  layers and what replaces them beyond that is OPEN (`terrain-texturing.md` §12); Tessera
  draws full layers at every distance it meshes until it is known.

<a id="terrain-splat"></a>
### 10.2 Texture splatting (`terrain-texturing.md`, M4.5)

Terrain is drawn as stacked texture layers, as the original draws it. The chain
(`terrain-texturing.md` §2): each terrain or ceiling chunk of a landblock's data lists layers
in draw order; a layer names a key of the region's **terrain type table** (0x19, found through
slot 8 of the region descriptor 0x0F), an A8 alpha mask (0x41) and a triangle range. The
table entry names the texture lists, the texture transform, the opacity switches and a
physics-material property set.

| Step | Crate | Spec |
|---|---|---|
| Decode the 0x19 table and the extended landblock data (tree blocks, ceiling diagonal, light-map properties) | `tessera-formats` | `terrain-texturing.md` §3; `landblock.md` §3, §6.4 |
| Resolve layers against the table by key: stored order, `whole` and unmasked layers, holes, quad and coarse grids, ceiling chunks | `tessera-terrain` | `terrain-texturing.md` §4 |
| Mask placement, per-vertex texture-coordinate sets (per-type scale, rotation and offset, per-block offset), tangent frame for normal maps, opacity inputs, light-map coordinates | `tessera-terrain` | §5, §6.1–§6.4, §7, §10 |
| Terrain layer pipeline: the features and parameters of each layer (shader-record choice), draw the layers in stored order on the same triangles, first drawn layer opaque, later ones blended "over" after the opacity test; per-layer colour with normal maps, specular, emissive and glow; the baked light map **added** to the lighting sum. Texture lists resolve through `tessera-material` as for any surface | `tessera-render` | §7–§10; `lighting.md` |

- Layer products are CPU data built on the asset workers with the landblock bundle and handed
  to the renderer like any mesh product (§8, §20.2).
- Whether layers are drawn one pass per layer or folded into one splatting shader per block
  is the renderer's choice; the result is the same "over" chain (`terrain-texturing.md` §8).
  The depth comparison for stacked layers is a Tessera Decision there ("less or equal").
- Failure handling follows `terrain-texturing.md` §8: a layer whose setup fails is skipped
  and logged, the rest of the chunk is drawn; an entry without a diffuse texture is drawn
  with the shader record's default texture, never skipped.
- Roads, paths and river beds are ordinary layers; there is no road record
  (`terrain-texturing.md` §11).
- The height-and-slope palette that this section planned before the layers were known
  survives only as Tessera's fallback and debug view (`terrain.md` §6).

<a id="terrain-water"></a>
### 10.3 Water (`water.md`, M4.5)

Water is not a terrain layer (`terrain-texturing.md` §11). It has three sources
(`water.md` §2):

- **Landblock water** (seas, large lakes and rivers): `tessera-world` decides each
  landblock's water type and flat level from the block map and the region's **water table**
  (0x16) (`water.md` §4, §6.2) and answers the "under water" query (§6.3). The renderer draws
  one 160 × 160 quad per wet landblock at that level with the water type's surface record
  (Tessera rule, §8.1); terrain above the plane hides it through the depth test.
  `tessera-material` resolves the water material chain (§8.2) and `tessera-render` has a
  water pipeline (§8.3–§8.4).
- **Placed water** (ponds, pools, fountains, water in interiors): static entities drawn like
  any other with the water shaders (§8.5).
- **Distant water:** derived from the block maps with the same rule as landblock water
  instead of decoding the distant-landscape sector records (Tessera rule, `water.md` §9).

Water volumes come from the landblock volume entity and from placed volumes (`water.md`
§7). `tessera-world` derives them, and the landblock water plane, from the data; the
controller in `tessera-physics` uses them for its wading and swimming states (`water.md`
§7.4; `collision.md` §12.3 step 7).

**How water reaches the controller** (Decision, [0013](decisions/0013-physics-crate.md)).
When `world` builds a landblock's collision set on the asset workers, it also stores in that
set, as plain physics data in the landblock frame:

- the landblock water plane: the level *h* and water type of `water.md` §6.2, or none for a
  dry landblock;
- the liquid boxes: each monitor box of the landblock volume entity (§7.1) and of every
  placed volume entity of the landblock (§7.3), transformed by its entity's placement
  (position, rotation, and scale when `Monitor_ScaleWithEntity` is true) into an oriented
  box, with the flags of its property set (§7.2: liquid, lava, surface, wading, deep).

The controller reads water only from the resident sets, like the triangles, so the
determinism of §20.3 covers water too, and `physics` needs nothing from `world` at step
time. `world`'s "under water" query (§6.3) reads the same plane and boxes.

The fog-zone table (0x17) is not water; it is decoded with the other per-region tables
(`water.md` §5).

<a id="terrain-sky"></a>
### 10.4 Sky, time of day and fog (`environment.md`, M4.5)

- `tessera-formats` decodes the sky description (0x15), the day descriptions (0x1A), game
  time (0x35) and the LOD-class table (0x32) (`environment.md` §5, §6, §9.1).
- `tessera-world` owns the **game clock** (`environment.md` §11.1). The original's epoch is
  OPEN, so Tessera's clock has a settable start time and rate. It chooses the day description
  per sky key and game day (§11.2; the per-day pick is also the weather variant), blends the
  keyframes (§11.3) and animates the sky objects (§11.4). The state is plain data handed to
  the renderer each frame.
- `tessera-render` draws the sky objects around the camera before the world without writing
  depth (§12), and takes the blended sun, ambient light and fog as inputs of the lit pipeline
  (`lighting.md` §8.1, §10). Interior cells with linear fog use their own fog values
  (`environment.md` §7.2, §12).

<a id="terrain-scenery"></a>
### 10.5 Scenery and grass (`scenery.md`, M4.5)

- Procedural scenery (trees, rocks) is generated per landblock in `tessera-world` on the
  asset workers, deterministically, from the per-vertex scenery words, the region's scene
  tables (0x12, 0x11) and the scene records (0x02) (`scenery.md` §6–§7). Placement needs the
  terrain collision mesh and its radius and plane queries, which is why `world` may depend on
  `physics` (§3). Generated objects enter the bundle like statics, with their collision
  (`collision.md` §9.4).
- Frills (grass and ground cover) are a separate layer of 10 × 10-unit sectors in their own
  ring around the camera with their own frame budget (`scenery.md` §8). They never collide.
- The renderer draws scenery and frills instanced (`scenery.md` §8.3, §9).

> **Answered (M4.5):** the former OPEN of this section, "terrain texture blend data, roads,
> water and the sky/weather/time-of-day system", is answered by
> [`terrain-texturing.md`](specs/terrain-texturing.md) (layers, masks, blending, roads,
> light maps), [`water.md`](specs/water.md) (water tables, landblock water, volumes, drawing)
> and [`environment.md`](specs/environment.md) (region tables, sky, day descriptions, game
> clock, fog). Their remaining `> **OPEN:**` items are listed in those specs; none blocks
> M4.5, because each has a Tessera Decision (`docs/ROADMAP.md` M4.5).

---

<a id="mesh-anim"></a>
## 11. Meshes, skeletons and animation

<a id="mesh"></a>
### 11.1 Meshes (`tessera-mesh`)

- Decodes mesh records (0x06) into engine vertex streams: position, normal, UV, tangent
  frame, and for skinned meshes bone indices and weights. Source vertices are
  stride-variable per submesh (Reported); the spec must give a deterministic stride rule.
  Heuristic stride detection is acceptable only if the spec defines it exactly and tests
  it on synthetic data.
- Converts Z-up source data into the engine convention at load (Decision: the engine uses
  Z-up world space too, and converts only in the camera/projection, so world data never
  needs re-axing).
- Assemblies: setup records (0x04; 0x01 records are cell geometry, `collision.md` §5;
  `setup.md` §2.2) and appearance records (0x20) combine parts into a character or
  object. M4 implements this as `tessera-mesh::assembly` (`appearance.md` §8): body parts
  by priority, worn and held items, dye layers, and the inputs of the clip resolver.

<a id="anim"></a>
### 11.2 Skeletons and animation (`tessera-havok`, `tessera-anim`)

- Skeletons (a tagfile inside each 0x04 setup) and clips (0x05) are stored as Havok binary
  tagfiles, version 3 (Confirmed, `havok-data.md` §2): skeleton bones with parent indices
  and reference pose; clips spline-compressed with 40-bit quaternions (24,800) or
  uncompressed interleaved (595). M4 decodes 100 % of both.
- `tessera-havok` is a clean-room reader for exactly the subset found in the archives,
  written from `docs/specs/havok-data.md`. It never links against or calls Havok code,
  and no GPL-only implementation is ported.
- `tessera-anim`: pose sampling (decompression on worker threads, cached per clip),
  blending, root motion, attach points (weapon bones), skinning palette output for the
  renderer. Tracks map to bones by position: track *j* drives bone *j*, and a clip fits a
  skeleton only when its track count equals the bone count (Confirmed,
  `skeleton-anim.md` §3). Vertex bone indices address the setup's skeleton directly, with
  no palette remap (`skeleton-anim.md` §4). Which clips play for idle, walk and run is
  decided by script tables (`script-table.md`), resolved in `tessera-formats`.

---

<a id="materials"></a>
## 12. Materials and textures (`tessera-material`)

Chain (Reported): surface (0x31) → material (0x30) → texture list (0x40) → render
surfaces (0x41), with a reference to a compiled shader record (0x2B).

- Texture products: `DecodedTexture { format, width, height, mips }`. Format mapping:
  DXT1/3/5 → BC1/2/3; A8R8G8B8 and R8G8B8 → RGBA8 with channel swizzle; A8 → R8;
  embedded JPEG → decoded to RGBA8 on a worker. When the adapter lacks BC texture
  support, BC data is decompressed on workers.
- **Shaders are our own.** The original's compiled shader records are not translated or
  executed. A material is classified into **feature flags** (alpha-test, dyeable,
  specular, metallic, ...) from functional facts in the material spec, and Tessera's WGSL
  pipelines are selected by those flags.
- Dye: per-material dye colour applied where the diffuse alpha marks a dyeable area
  (Reported rule; spec in `material.md`).
- Icons: layered composition (underlay, background, image, overlay, shadow) is a
  `material::icon` helper used by the UI and the explorer.

> **OPEN:** how to classify shader records without reading their bytecode semantics.
> The approach reported by others (counting known constant names in the blob) is a
> black-box heuristic; the material spec must decide whether that is acceptable or
> whether classification comes from material/surface fields alone.

---

<a id="render"></a>
## 13. Renderer (`tessera-render`)

- **API:** wgpu 30 (Vulkan, Metal, DX12, GL) via winit windows. WGSL shaders written for
  this project.
- **Frame structure:** a small, explicit frame graph: depth pre-pass → opaque (terrain,
  static, skinned) → alpha-tested → sky → transparent → UI → present. Shadows, bloom and
  other post effects are added as passes later.
- **Resources:** texture and buffer uploads from CPU products, with per-frame byte
  budgets. Static meshes share large vertex/index buffers (sub-allocation) to keep draw
  calls cheap. Instancing for repeated static objects.
- **Camera-relative rendering:** all transforms are computed relative to the render
  origin (§9.1) in f64 on the CPU and uploaded as f32.
- **Headless mode:** the renderer can target an offscreen texture without a window. This
  is what golden-image tests use (§23).
- **Diagnostics:** debug labels on every pass and resource so RenderDoc captures of
  Tessera itself are readable; an optional egui overlay for frame stats.
- **Settings-driven quality:** draw distances, texture caps, shadow quality come from
  `tessera-settings`, with names chosen by us.

---

<a id="audio"></a>
## 14. Audio and music (`tessera-audio`)

- **Output:** kira on cpal (Decision, to be confirmed by an ADR at M6). Requirements:
  streamed playback for music, many short positional sounds, per-bus volumes, smooth
  crossfades between music regions.
- **Decoding:** Ogg Vorbis and WAV payloads (Reported) decoded by pure-Rust decoders on
  worker threads; long tracks streamed.
- **Spatial:** listener follows the camera; emitters attached to entities; distance
  attenuation and simple panning first, HRTF optional.
- **World hooks:** block-map properties give area music and ambient sound per landblock
  (Reported); the streamer feeds them to a music director that crossfades on area change.
- **Player music:** an ABC parser and sequencer (`audio::abc`) implementing the game's
  documented player-music dialect (note range, chord limits, minimum and maximum note
  lengths, tempo rule; Reported in engine-client-re.md §7). Instrument samples come from
  the user's sound archives.

> **OPEN:** which sound records hold the instrument samples and how per-note pitch is
> derived. RE task in `docs/ROADMAP.md` M6.

---

<a id="ui"></a>
## 15. User interface (`tessera-ui`)

Two separate UI stacks:

1. **Tool and debug UI:** egui (with egui-wgpu and egui-winit) for the explorer and
   in-client developer overlays. Never used for game UI.
2. **Game UI:** a retained widget tree owned by `tessera-ui`.

Game UI design:

- **Sources merged into one tree:** UI layout records (0x22: element IDs, rectangles,
  colours, per-state images, child elements; Reported), user skins
  (`SkinDefinition.xml` mapping art asset IDs to TGA files and overriding element
  geometry; Reported), and controls created by plugins (§18). Native panels and plugin
  windows are the same kind of node, so z-order, focus and input routing are uniform.
- **Layout:** absolute rectangles with margins and anchors, as the data describes.
  Plugins position controls absolutely too (Reported API semantics). A flexbox engine is
  not needed for compatibility. Taffy may be used for Tessera's own panels.
- **Rendering:** the tree produces a 2D draw list (textured quads, nine-slice frames,
  text runs, clip rects) that `render` draws in the UI pass.
- **Text:** font rasterisation and shaping with a pure-Rust stack. The fonts the
  original UI uses are referenced by DID in the data (Reported); whether they can be
  decoded is OPEN. Tessera bundles only freely licensed fonts as fallback.
- **XML:** skins and plugin descriptors are parsed with a strict, bounded pure-Rust XML
  parser (no DTDs, no external entities).

> **OPEN:** the UI layout record grammar beyond the element header, the meaning of the
> per-element UIData variants, and the font record type. RE tasks in ROADMAP M5.

---

<a id="input"></a>
## 16. Input and key mapping (`tessera-input`)

- Pipeline: winit events (and gilrs gamepad events) → raw input state → **contexts**
  (text entry, UI, camera, movement, quick-slots) → **actions**.
- Actions are named by Tessera. A table maps them to the action IDs that the plugin API
  exposes (Reported: about 270 key-binding actions), so plugins that listen for actions
  behave as expected.
- Bindings live in Tessera's own keymap file (TOML, in Tessera's config directory).
  Importing the original client's keymap file read-only is planned.

> **OPEN:** the original keymap file syntax (Reported as undocumented,
> engine-client-re.md §8). The importer waits for a spec.

---

<a id="settings"></a>
## 17. Settings and user directories (`tessera-settings`)

- Tessera's directories come from the platform conventions (XDG on Linux, Known Folders on
  Windows, Application Support on macOS): `config/` (settings, keymap), `data/` (plugin
  data, UI layouts saved by the user), `cache/` (optional derived-data cache), `logs/`.
- The settings file is INI-shaped with sections such as `[Display]`, `[Render]`, `[Audio]`,
  `[Game]` so it is familiar, but the keys are Tessera's. Typed accessors with defaults,
  validation and range clamping; unknown keys are kept and written back unchanged.
- **Importer:** on first run, if the original client's preferences file is found in the
  user's documents folder, a read-only importer maps the known display, audio and
  draw-distance keys to Tessera's. The original file is never written.
- The game directory chosen by detection (§4.1) is saved here.

---

<a id="plugins"></a>
## 18. Scripting and plugin host (`tessera-script`, `tessera-plugins`)

<a id="script"></a>
### 18.1 Lua host (`tessera-script`)

- Lua 5.1 semantics (Reported: the original plugin system embeds Lua 5.1), via a safe
  Rust binding with a vendored Lua 5.1 build (Decision: mlua with the `lua51` and
  `vendored` features; LuaJIT is an option later if performance requires it).
- **Sandbox:** only the base library (minus `dofile`, `loadfile`, `load` of binary
  chunks), `string`, `table`, `math`. No `io`, `os` (beyond time queries), `debug`,
  `package` loaders or C modules. Memory limit and an instruction-count hook per state.
- **Threading:** Lua states are not `Send`; all plugin code runs on the main thread in a
  per-frame time slice.

<a id="plugins-host"></a>
### 18.2 Plugin host (`tessera-plugins`)

- Discovers plugins from a user-configured plugins directory (the original client's
  plugins folder can be pointed at read-only), parsing `.plugin` XML descriptors.
- **Apartments:** each apartment is a separate Lua state; plugins sharing an apartment
  share a state; unloading unloads the apartment (Reported semantics).
- **Import:** the dotted `import` resolves to files in the plugin tree through a virtual
  file system rooted at the plugins directory; no path escapes.
- **API surface:** the compatible namespaces (engine, chat, shell commands, plugin data,
  UI controls and windows, gameplay read-only views) are implemented from functional
  documentation of names, signatures, enums and event semantics. No original Lua files or
  documentation prose are copied or shipped.
- **Persistence:** plugin data (account, server, character scopes) is stored under
  Tessera's `data/` directory, not in the original client's folders.

> **OPEN (legal):** the plugin API's global namespace names are the original vendor's
> name. They are needed verbatim for interoperability. `docs/LEGAL.md` must record the
> interoperability rationale before M5 ships.

---

<a id="net"></a>
## 19. Networking (`tessera-net`, `tessera-protocol`)

<a id="net-backend"></a>
### 19.1 Backend trait

The client never talks to sockets directly. It talks to a `Session` that wraps a
`Backend`:

- `Session::poll_into(&mut self, now: Instant, out: &mut Vec<Event>)` (non-blocking,
  called once per tick on the main thread; appends to a buffer the caller reuses, so
  handling an event can call `send`). `Session::poll(&mut self, now) -> Vec<Event>` is a
  convenience wrapper.
- `Session::send(&mut self, now: Instant, Command) -> Result<(), NetError>`.
- `now` is supplied by the caller and is monotonic; the session clamps a stale value.
- `Event` is **domain-level**, not wire-level: entity created (ID, class, property
  set, position), entity removed, property changed, position/motion update, chat line,
  time sync, world-state change, disconnect with reason.
- `Command`: movement intent, target selection, use/interact, chat, emote, request
  object data, and so on, added as milestones need them.

<a id="net-backends"></a>
### 19.2 Backends

| Backend | Status | Description |
|---|---|---|
| **Sandbox** | M0 skeleton, grows each milestone | In-process, no sockets. Spawns the local avatar at a chosen position in the user's data and serves a static world: landblock-placed static entities only. This is the offline world walker of M3. |
| **Test server** | M7 | `tessera-server` + `tessera-protocol`: a protocol designed and documented by this project (spec `docs/specs/tessera-protocol.md`), with its own handshake, its own encryption (standard, documented algorithms) and its own content: original test content, or the server operator's own data limited to what LEGAL.md allows. |
| **Compatibility** | M8, **gated** | See §19.3. |

<a id="net-compat"></a>
### 19.3 The gated compatibility tier

The owner's long-term goal is compatibility with the official service. `docs/LEGAL.md` §8
rates official-server connectivity as tier **T4**, the highest-risk tier, and §9.3 gates it:
no code, spec or test for it may be merged until conditions G1–G5 (written legal review,
permission sought from the rights holders, a recorded maintainer decision, a user-facing
warning, and the permanently-forbidden list still obeyed) are met and recorded in
`docs/decisions/`.

Architecturally this means:

- Before the gate, `tessera-net` contains the `Backend` trait, the domain-event vocabulary,
  the sandbox backend and (from M7) the test-server backend. Nothing else.
- The trait and event vocabulary are designed so that another backend could be added
  later without changes above `tessera-net`. That is the only preparation done before the
  gate.
- If the gate ever passes, a compatibility backend would live outside the default
  workspace members, behind an explicit opt-in, and would not be built by CI or default
  release artefacts.
- No official hostnames, ports, keys, ciphers, login flows or tables extracted from the
  client binary appear anywhere in the repository at any milestone.

<a id="net-threading"></a>
### 19.4 Network threading

Socket I/O runs on a dedicated network thread (a single-threaded async runtime inside
`tessera-net`), which decodes messages into `Event`s and posts them over a bounded
channel. The main thread drains the channel in `Session::poll_into`. The sandbox backend runs
inline on the main thread.

---

<a id="frame"></a>
## 20. Client application and the path from install to frame

<a id="frame-startup"></a>
### 20.1 Start-up

1. Initialise `tracing` (env filter from `RUST_LOG`), parse CLI options.
2. Load Tessera settings (§17).
3. Locate and validate the install (§4.1). If none is found, run in **no-data mode**:
   original test scene, explorer-style message pointing at `TESSERA_GAME_DIR`.
4. `ArchiveSet::open` (§5), log the archive table at `info`.
5. Start the asset server and worker pool (§8).
6. Load bootstrap records on workers: property registry, root DID map, string tables
   for the UI locale. The client shows a loading screen meanwhile.
7. Create the window and renderer (§13); create the audio device (§14).
8. Connect a backend (§19): sandbox by default.
9. Load plugins (§18) after the UI is ready.

<a id="frame-dataflow"></a>
### 20.2 Data flow for one landblock (illustrative)

```
avatar position (backend) ─► streamer: desired set ─► asset requests (priority)
     ─► worker: ArchiveSet.get(DID) ─► extent read ─► zlib inflate ─► formats::decode
     ─► loader: emit dependencies (meshes → materials → texture lists → textures)
     ─► CPU product (Arc) ─► completion channel
main thread: drain completions within upload budget ─► render: create GPU buffers/textures
     ─► scene: insert instances into the landblock's render list
frame: cull ─► build draw lists ─► encode passes ─► submit ─► present
```

<a id="frame-loop"></a>
### 20.3 Frame loop (main thread)

1. Pump window and input events; update input state and actions.
2. `Session::poll`: apply server events to the entity store (entity ID → transform,
   property set, render/audio components).
3. Simulation at fixed rates, interpolated for rendering (Decision, see "Simulation steps"
   below): the character controller (`tessera-physics`) on its own 1/60 s step; the other
   local simulation (free-fly controller, camera) on the 30 Hz tick.
4. Streamer: update desired landblocks and priorities.
5. Drain asset completions within the upload budget; create GPU resources; install and
   remove collision sets ("Collision-set residency" below).
6. Animation: collect pose results computed on workers for the previous frame; submit new
   sampling jobs.
7. Plugins: run Lua `Update` events and timers within their time slice.
8. UI: process input, layout, build the 2D draw list.
9. Audio: update listener and emitters; send commands to the audio thread.
10. Render: cull, sort, encode, submit, present.

**Simulation steps** (Decision, M4.5, [0013](decisions/0013-physics-crate.md)). There are two
fixed-step accumulators inside the frame, both on the main thread, and neither depends on
the frame rate:

- **Character controller: 1/60 s.** Walk mode (walking, jumping, sliding, wading, swimming,
  cells) advances `tessera-physics` in fixed steps of 1/60 s, at most 5 steps per frame
  (`collision.md` §12.3). `tessera-physics` owns these numbers as constants
  (`STEPS_PER_SECOND` = 60, `STEP_SECONDS` = 1/60, `MAX_STEPS_PER_FRAME` = 5) and its step
  function takes no step length: every caller (client, tests, the M7 server) steps exactly
  `STEP_SECONDS`. `physics` has no clock. The client owns the accumulator, a
  `tessera-input` `FixedTimestep` built from `STEPS_PER_SECOND` and `MAX_STEPS_PER_FRAME`;
  it only decides how many steps run. Time beyond the cap is dropped, not carried
  (`FixedTimestep::dropped_seconds`), so a stall never causes a burst of catch-up steps.
  Input is latched per step. The rendered avatar and the follow camera use the pose
  interpolated between the last two controller states by the accumulator's leftover
  fraction.
- **Other local simulation: 30 Hz.** The free-fly controller, camera smoothing and any other
  fixed-rate local work keep the 30 Hz tick that this section has always named (the
  `tessera-input` default, `keymap.md` §9.1). The 30 Hz tick does not drive the controller;
  it is not a network rate either (the network has its own message timing, §19).

**Collision-set residency** (Decision, [0013](decisions/0013-physics-crate.md)):

- **Installing and removing.** Sets built on the workers arrive with the asset completions
  and are installed when completions are drained (step 5), that is after the frame's
  controller steps (step 3) and never during a step. Sets are removed at the same point. A
  set is not removed while the mover needs it (below); its removal waits until the mover
  has left it.
- **Which sets a step needs.** Every landblock that the mover's query box overlaps: the
  capsule's bounding box grown on every side by the controller's largest per-step
  displacement (`physics` bounds it, see "Input validation"). The bound is far below 160 m,
  so a step needs at most 2 × 2 landblocks.
- **Missing sets.** While any needed landblock has no installed set (not yet built, deferred
  by the residency budget, or just teleported to), the controller does not step: the
  frame's accumulated time is dropped, the latched input (a pending jump too) is kept, and
  the mover's state and step count do not change. This carries over the walker's "pending
  until the spawn block is built" (`walker.md` §5). A landblock that the streamer has
  resolved as absent from the archives counts as installed with an empty set and is a
  solid boundary for the mover; the walker's M4.5 section specifies that contact.
- **Order independence.** A query over several sets visits them in ascending landblock
  order (region, x, y). Among contacts at the same time of impact the one with the smallest
  (landblock, primitive index) wins, so the result never depends on the order in which
  workers finished.

**Input validation** (Decision, [0013](decisions/0013-physics-crate.md)). The movement
parameters come from property sets in the archives (`collision.md` §10.3, §11.1) and so are
hostile input. `tessera-physics` validates everything at its API boundary and never panics
in a step: a non-finite or out-of-range parameter is replaced by the value of the layer
below it in the §11.1 overlay, and finally by the built-in base-layer default that
`physics` documents; clamp bounds are ordered first and no panicking `clamp` is used; speeds
are bounded so that the per-step displacement never exceeds the documented bound; capsule
dimensions must be finite and positive; input axes are sanitised as in `keymap.md` §9.1
(non-finite → 0, then −1..=1). A fuzz target drives the controller with arbitrary
parameters, inputs and synthetic sets and checks that the state stays finite.

**Determinism.** Given the same start state, the same per-step inputs and the same collision
sets (water planes and liquid boxes included, §10.3), the controller produces the same
positions whatever the frame rate, because it only ever sees whole `STEP_SECONDS` steps and
waits instead of stepping without its sets. Scripted `--walk-path` runs and the soak walks
give their inputs per step, not per wall-clock time, and start only once the sets the first
step needs are installed; worker timing then changes when a step runs, never what it
computes. The collision tests rely on this.

The M7 test server runs the same controller with the same step. It moves exactly like the
client only over the same collision sets; on install data they include generated scenery,
statics and water, which `world` builds (see the OPEN under §3.1).

Entity storage (Decision, [0008](decisions/0008-entity-storage.md)): the project-owned
`tessera-entity` store (generational IDs, sparse-set components, per-frame change lists),
main thread only. A standalone `bevy_ecs` lost the M4 benchmark that
[0005](decisions/0005-no-full-bevy.md) required and is not used by the client. Entity
transforms hold `f64` region-frame positions (the §9.1 world position); the renderer
subtracts its camera anchor in `f64` and narrows to `f32` (the §9.1 render position).

---

<a id="threading"></a>
## 21. Threading model

| Thread | Owns | Talks to others via |
|---|---|---|
| **Main** (winit event loop thread) | Window, input, entity store, streamer, UI tree, Lua states, wgpu queue submission and resource creation | Request queue to workers; completion channel from workers; command channel to audio; event channel from network |
| **Asset workers** (N = available cores − 2, at least 1) | Nothing long-lived; run jobs: read, inflate, decode, transcode, mesh/terrain generation, animation decompression and sampling | Priority job queue in; completion channel out; cancellation tokens |
| **Audio** (owned by the audio backend) | Mixer state, voices, streams | Lock-free command queue from main |
| **Network** (when a socket backend is active) | Sockets, protocol state machine, crypto | Bounded channels to and from main |
| **Index builder** (tools only) | Full DID index build | Result handed to the UI thread |

Rules:

- `ArchiveSet` and `Archive` are `Send + Sync`; reads take `&self` and use positioned reads,
  so any worker can read any archive without a lock. Node caches use internal sharded
  locks.
- Decoded products are immutable `Arc<T>`. There is no shared mutable game state outside
  the main thread.
- GPU resource creation happens on the main thread in M0–M3. wgpu allows creation from
  other threads; moving uploads to a dedicated thread is an optimisation for later, guarded
  by profiling.
- No blocking I/O on the main thread after start-up. A missing asset is a placeholder for
  a frame, never a stall.
- Worker pool: a dedicated pool with a priority queue (Decision), not a general
  work-stealing pool, because priority and cancellation matter more than throughput.

---

<a id="diagnostics"></a>
## 22. Errors, logging and diagnostics

- One `thiserror` error enum per library crate. Errors carry the DID and archive key when
  known. Apps use `anyhow` at the top level.
- Library crates log with `tracing` only. Hot paths log at `trace`. "Unknown but
  tolerated" data logs once per kind at `debug`.
- The client installs a panic hook that writes a crash report (backtrace, versions, data
  version, last log lines) into `logs/`. It never includes game data.
- Profiling: `tracing` spans on frame phases and asset jobs; an optional Tracy
  integration behind a feature.

---

<a id="testing"></a>
## 23. Testing strategy

No test, fixture, snapshot or golden file in the repository contains game data. Four
tiers:

<a id="testing-synthetic"></a>
### 23.1 Synthetic fixtures (always run, CI)

- `tessera-dat` has a test-only archive builder (feature `testkit`) that writes a valid
  archive into a temp directory from a list of (DID, bytes, compress?) entries: header,
  directory B-tree with several levels, modern and legacy extents, fragmented entries.
  Its output is our own code's output, never a copy of real data.
- `tessera-formats` has record builders per format that produce byte buffers from
  structured input. Tests round-trip: build → decode → compare.
- Property tests (proptest) generate random entry sets and random records within the
  spec's constraints.
- Negative tests: truncated input, oversized counts, cycles in extent chains, out-of-range
  offsets, wrong magic. Each must yield an error, never a panic or a huge allocation.

<a id="testing-real"></a>
### 23.2 Gated real-data tests (local and self-hosted only)

- Read `TESSERA_GAME_DIR` (or the auto-detected install). If unset or invalid, the test
  prints a one-line skip notice and returns successfully.
- They never write to the install directory and open everything read-only.
- They assert **structural invariants and non-expressive metadata only**: header fields
  self-consistent, every B-tree sorted and acyclic, every entry inside its file, every
  compressed entry inflates to its stated size, every record of a supported type decodes
  without error or trailing bytes, counts per type within expected ranges.
- Snapshots (insta) may contain counts, DID ranges, hashes and structural summaries. They
  must never contain decoded text, pixels, vertices or audio.

<a id="testing-golden"></a>
### 23.3 Golden images

- Headless renderer on a software adapter (lavapipe/llvmpipe or another software
  Vulkan/GL implementation) renders synthetic scenes: procedurally generated meshes,
  CC0 or self-generated textures, test terrain heightfields.
- Images are compared with a per-pixel tolerance and a maximum differing-pixel ratio.
  Goldens in the repo come from synthetic scenes only.
- Real-data visual regression (a landblock, a character) is a **local-only** tier: the
  goldens are written to the user's own cache directory and never committed.

<a id="testing-fuzz"></a>
### 23.4 Fuzzing

- `fuzz/` holds a cargo-fuzz target per parser: header, extent assembly, B-tree node,
  inflate wrapper, each `formats` decoder, Havok reader, XML skin and plugin descriptors,
  protocol framing.
- Seed corpora come from the synthetic builders. Corpora derived from real data stay in
  ignored local directories (`fuzz/corpus`, `fuzz/artifacts` are git-ignored).
- CI runs a short fuzz smoke per target; longer runs happen locally.

<a id="testing-ci"></a>
### 23.5 CI gates

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, fuzz smoke, licence and advisory checks on dependencies
(GPL-3.0-compatible licences only), SPDX header check, and a content guard that rejects
`.dat`/`.datx` files, large binary blobs and known media containers in the tree.

---

<a id="tooling"></a>
## 24. Tooling inside the workspace

| Tool | Purpose | Milestone |
|---|---|---|
| `tessera-dattool info <dir>` | Install summary: archives, kinds, regions, block sizes, versions, entry counts | M0 |
| `tessera-dattool ls <archive>` | List entries (DID, size, flags, iteration, timestamp), filter by type | M0 |
| `tessera-dattool stat <dir>` | Histogram of record types (DID high bytes) per archive and in total | M0 |
| `tessera-dattool strings <did>` | Print a decoded string table | M0 |
| `tessera-dattool detect` | Find installs and print their fingerprints | M0 |
| `tessera-dattool cat <did>` | Dump a record (raw or decoded) to stdout | M0 |
| `tessera-dattool extract` | Write a record (raw, as PNG for textures, or as text for string tables) to a user-chosen path outside the install, for personal study | M0 |
| `tessera-dattool validate` | Run all structural invariants over an install | M0/M1 |
| `tessera-dattool diff` | Compare two installs or two snapshots (DID added/removed/changed iteration) to see what a patch touched | M1 |
| `tessera-dattool show <did>` | Decode one record and print its fields (text or JSON): world tables, scenes, collision meshes, cells, environment records, region descriptors, landblocks with their trees | M4.5 |
| `tessera-dattool census` | Decode every record of chosen prefixes and walk their collision trees; exits 1 on any failure (`docs/TOOLING.md` §4.4) | M4.5 |
| `tessera-explorer` | GUI browser: archives, string tables, textures, icons, then meshes, landblocks, UI layouts | M1+ |
| `tools/imhex/`, `tools/kaitai/` | Pattern and schema files written from our specs, for interactive inspection | ongoing |
| `fuzz/` | Fuzz targets | ongoing |

---

<a id="open"></a>
## 25. Consolidated open questions

| # | Question | Blocks | Owner doc |
|---|---|---|---|
| 1 | Meaning of header kind/subset words; locale selection | M1 | `specs/dat-container.md` |
| 2 | Self-describing routing vs static table | M1 | `specs/dat-container.md` |
| 3 | One gamedata crate vs `props`/`text`/`wstate` | M1 | ADR |
| 4 | Normative status of Kaitai schemas | M1 | ADR |
| 5 | Disk cache of derived data (value and legal sign-off) | M3 | ADR + `LEGAL.md` |
| 6 | Height scale and trailing landblock data (terrain types): **answered** (`landblock.md` §3.4–§3.9, `terrain-texturing.md`) | M3 | `specs/landblock.md` |
| 7 | Interiors, portals, PVS | after M3 | `specs/landblock.md` |
| 8 | Entity storage / ECS choice: **decided**, project-owned store ([0008](decisions/0008-entity-storage.md)) | M4 | ADR 0008 |
| 9 | Shader-record classification method | M2 | `specs/material.md` |
| 10 | UI layout grammar, UIData variants, font records | M5 | `specs/ui-layout.md` |
| 11 | Plugin namespace naming rationale | M5 | `LEGAL.md` |
| 12 | Original keymap file syntax | M5 | `specs/keymap.md` |
| 13 | Instrument samples and pitch mapping for player music | M6 | `specs/sound.md` |
| 14 | Sky, weather, time of day, water: **answered** (§10; remaining OPENs non-blocking, in the specs) | M4.5 | `specs/environment.md`, `specs/water.md`, `specs/terrain-texturing.md` |
| 15 | Anything about the compatibility tier | M8 gate | `LEGAL.md` §9.3, `docs/decisions/` |
