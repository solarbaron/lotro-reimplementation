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
| Risk tiers T0–T4 and the official-server conditions (decision 0034) | `docs/LEGAL.md` §8–§9 (from research/notes/legal.md §7) | Reported |

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
| No hostnames, keys or bulk data tables extracted from the client binary (numeric algorithm constants are allowed: decision 0012) | `tessera-net` has no built-in endpoints for any official service. Anything of that kind is loaded at run time from the user's install, and only by the compatibility backend (M8, [decision 0034](decisions/0034-official-server-compatibility.md)). |
| Official-server connectivity is the highest-risk tier (T4, LEGAL.md §8); in scope by the owner's decision without a legal review, G4 and G5 binding (LEGAL.md §9, decision 0034) | The network layer is a trait with interchangeable backends (§19). The compatibility backend (M8) is its own crate, so it can be withdrawn without restructuring (LEGAL.md §9.4); CI never connects to an official server; no connection is possible before the user accepts the G4 warning. |
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
     render  audio*  ui*  input*(→settings)  settings*  plugins*(→script, ui)  net(backends)  entity*
        │      │      │     │                   │                   │
 L3 world & assets   world*(→physics)  physics*  terrain*           │
                     anim*(→mesh)  mesh*  material*                 │
                     scriptgraph*  gamedata*                        │
        └──────┴──────┴──────┬─────────────────────────────────────┘
 L2 asset system          assets*   props*  text*  wstate*(→props,text)  wvm*(→wstate)
                             │
 L1 record decoders       formats  (+ havok*)
                             │
 L0 container & install   dat      install
```
`*` = planned after M0. `gamedata` (P0) serves the client and the tools only; `tessera-server`
never links it (rules below, §3.1).

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
    or any other L3/L4 crate, and it has no GPU, asset-server or streaming dependency;
  - `wstate` → `props`, `text` (L2, M3): a game object carries its property set, read
    through the property registry, and the area views render StringInfo names through the
    string tables (§7.4, `wstate.md` §11);
  - `wvm` → `wstate` (L2, census 2): the script machine resolves function IDs and slots of
    the compiled code against the class library schema that `wstate` decodes
    (`wlib-bytecode.md`). One-way: `wstate`, `props` and `text` never depend on `wvm`, and
    no crate below the apps depends on `wvm` except `gamedata` (L3, from P0,
    [0024](decisions/0024-parity-track.md) §3). The reason for that one exception: the
    presentation data layer evaluates tooltip and description text, which needs the decoded
    and validated compiled code (running the client's own class-library code was planned for
    P7, which the owner dropped on 2026-10-16). `gamedata` serves only the client and the tools, so `wvm` still never
    reaches the server (rule on `gamedata` below). Today `dattool` uses `wvm` directly.
- L0–L2 crates have no GPU, window or audio dependencies.
- `render`, `audio` and `ui` do not depend on each other. The client app wires them
  together.
- `input` and `ui` do not depend on each other either. The client wires them: the mapper
  offers events to the UI through a synchronous sink that the client implements
  ([0014](decisions/0014-ui-input-bridge.md)).
- Sibling allowances at L4, each one-way ([0023](decisions/0023-plugin-host-split.md)):
  - `plugins` → `script`: the plugin host runs on the plugin-free Lua core (§18).
  - `plugins` → `ui`: the plugin API's UI classes are thin bindings over `tessera-ui`
    elements; painting and composition stay in `ui`. `ui` never depends on `plugins` or
    `script`. The alternative, a trait in `ui` that the client wires to `plugins`, was
    rejected (0023); the owner may revise this.
- `net` depends only on L0–L2 types it needs for its event vocabulary (property values,
  positions). It never depends on `render`, `ui` or `audio`. Its scenario feed (cargo
  feature `scenario`, P0, `scenario-feed.md` §3.1) decodes no game data and depends on no
  data-decoding crate and not on `gamedata`: the client resolves a scenario's selectors
  through `gamedata` and hands the sandbox the resolved scenario (§19.2).
- `gamedata` (L3, P0, [0024](decisions/0024-parity-track.md) §3) depends only on `formats`,
  `props`, `text`, `wstate` and `wvm` (L1–L2): no L3 sibling, no asset server, no GPU. It is
  for the client and the tools only. `tessera-server` never links `gamedata`, any crate that
  decodes or reads game data, or `tessera-net` with the scenario feed (LEGAL.md §11.1 rule
  5, decision 0024 §2.3); `tools/check-server-deps.sh` fails CI when its dependency graph
  holds one (§23.5). It refuses `tessera-net` in the server in any form. Crates that run
  the server in-process (today `tessera-loadtest`) are refused data-decoding crates and
  `gamedata` too, but may link `tessera-net`, which in a workspace build carries the feed
  (feature unification); whether hosts must stay feed-free is OPEN (`scenario-feed.md`
  §3.1, item 3).
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
| `tessera-wstate` (`crates/wstate`) | L2 | M3 | Class library (WLib) schema and the generic scripted-object graph (WState) (§7.4). From census 2 also typed views of the player-character records (`player-character.md`) and of the game-control singletons (`game-controls.md`). May depend on `props` and `text` (sibling allowance, §3). |
| `tessera-wvm` (`crates/wvm`) | L2 | census 2 | Script machine of the class library: decodes and validates the compiled code (chunk 1 of 0x56000000) and its message list (chunk 2), resolves function IDs and slots against the class library, disassembles, counts and cross-references function bodies (`wlib-bytecode.md`). Decoder level only; no interpreter yet. Depends on `formats` and `wstate` (sibling allowance, §3); nothing below the apps depends on it except `tessera-gamedata` (§3). |
| `tessera-assets` (`crates/assets`) | L2 | M1 | Asset server: typed handles, decode jobs on worker threads, caches, priorities, cancellation, GPU-upload hand-off (§8). |
| `tessera-world` (`crates/world`) | L3 | M3 | Coordinates, landblock and cell IDs, streaming rings, landblock info (cells, static entities, links), block maps (§9). From M4.5 also: scenery generation and the frill (grass) ring, landblock water and water volumes, the game clock and sky state (§10), light selection (`lighting.md` §8.3–§8.5), per-class draw distances (`walker.md` §11.4). From census 3 also: placed decal volumes (parameters, projection volume and mesh builder, colour, light, depth-bias table, draw range; `decals.md`, module `decals`), the phasing manager of quest-state phasing (`world-logic.md` §4.5, module `phasing`) and the day and night lights of placed lamps (`instance-scripts.md` §6, `lighting::glow`). Builds each landblock's collision set, water included, on the asset workers and hands it to `physics` (§10.3, §20.3). May depend on `physics` (sibling allowance, §3). |
| `tessera-physics` (`crates/physics`) | L3 | M4.5 | Collision and movement: terrain and ceiling collision triangles with the client's numbering, the terrain radius and plane queries that scenery placement uses, cell volumes and the point-in-cell lookup, per-landblock collision sets (statics, generated scenery, cells) with Tessera's own bounding-volume hierarchy, capsule sweeps, and the character controller with its water states (`collision.md` §5.4, §5.8.1, §6, §9–§12; `water.md` §7.4), and the war-steed (ramp-up) controller of mounted movement (`mounts.md` §5.3–§5.7, `controller::warsteed`). A collision set also carries its landblock's water plane and liquid boxes, so the controller reads water only from its own sets (§10.3). Owns the step constants (1/60 s, at most 5 per frame) and validates every input at its API boundary: non-finite or out-of-range movement parameters, bounded speeds and per-step displacement, no panicking clamps ([0013](decisions/0013-physics-crate.md)). Sets are keyed by `formats::LandblockId` in landblock frames; the mover is in the f64 region frame (§9.1). Depends only on `formats` and `props`; no GPU, asset-server or streaming dependency. Record decoders (quantised trees, 0x66, 0x01) stay in `formats`. Planned in M4.5: the M7 test server would reuse the controller and the collision-set types, with building the sets from install data left to `world` (§10.3, §10.5). Not built: the M7 server as built links no physics, and the P0 rules (§3, §23.5) refuse it (see the OPEN below this table). |
| `tessera-terrain` (`crates/terrain`) | L3 | M3 | Heightmap meshing, terrain LOD bands, far-terrain imposters; from M4.5 the texture layers of the region's terrain type table, alpha masks, per-layer texture coordinates and tangent frames, ceiling chunks and light-map coordinates (§10). |
| `tessera-mesh` (`crates/mesh`) | L3 | M2 | Static and skinned mesh (GfxObj) decoding into engine vertex streams; setup/part assemblies (§11). |
| `tessera-anim` (`crates/anim`) | L3 | M4 | Skeleton hierarchy, pose sampling, blending, skinning palettes, attach points (§11). From P2, for clips that combat scripts start on figures (`skeleton-anim.md` §6.6): `Animator::insert_layer` adds a layer between existing ones, so a clip sits by its data-layer key between the idle and locomotion layers and the held proportion layer, and `Layer::blend_bone_opacity` moves a playing clip's track-opacity mixing factor over 0.2 s when its `UseBoneOpacity` flag changes, so an upper-body attack plays over the legs' locomotion. |
| `tessera-material` (`crates/material`) | L3 | M2 | Surface → material → texture-list → texture chains, shader-feature classification, dye rules (§12). |
| `tessera-particles` (`crates/particles`) | L3 | 2026-10-08 | Particle runtime (`particles.md` §8, `particle-runtime.md`): the shared random generator (§3.3) and its range draws, deterministic CPU simulation of 0x39 systems (start, restart, update cadence, timed and distance emission with the clamped caps, spawn shapes and spread, keyframe channels, motion controllers, forces, fade envelope, atlas frames, distance fade and the not-drawn state), billboard vertex generation in storage order with the 32,768-vertex draw limit, fork-effect lifetimes (not simulated, rule §10.2), and the run-time rules: never-ending classification, node start gates, start gating, attachment frames, owner scale vectors, sharing, stop, lingering and housekeeping. Depends on `formats` (records) and `material` (curve evaluation); no GPU. `render` draws its vertices; the explorer previews records; the walker (`apps/client/src/walker/effects`) starts effects from the Particle nodes of the start-up scripts of static entities and placed figures (run by `tessera-scriptgraph`), attaches, places, lingers and draws them. Not end to end: fork effects are not drawn, nor systems of the post-processing pass; weather, beams, lit systems' lights, `inherit_opacity` and bounds culling wait for their specs (`particle-runtime.md` OPENs). From P2: beams (`particle-runtime.md` §11.1, rule 14; module `beam`): envelope, the two ends with their offsets and detach times, head and tail, the head's arrival (which fires the Beam node's port), ribbon segment layout and the beam and head particle spawns (`SystemSim::spawn_at`); the walker's combat layer starts the Particle nodes of combat scripts (behaviours, hit and projectile scripts, effect, induction, channel, combat-state and death scripts) on the figures of session entities, attached and looping as above (`combat-visuals.md` §4.5, §5, §9). The walker draws a beam's ribbon (`beam::ribbon_mesh`, camera-facing quads with the surfaces of its three materials through the particle pipeline, before the systems) and its head entity (the 0x47 entity's parts through the object layer's instanced models, at the head point in `beam::beam_rotation`); the readings the spec leaves open are the implementer OPEN of `particle-runtime.md` §11.1. |
| `tessera-gamedata` (`crates/gamedata`) | L3 | P0 | The presentation data layer of the parity track ([0024](decisions/0024-parity-track.md) §3), the "future gameplay-data crate" the gameplay specs name: typed stores over `props`/`wstate` for items, skills, effects, creatures, quests, economy, map and progression records; tooltip and description evaluation; scenario selector resolution (`scenario-feed.md` §8, with query types of its own that mirror the feed's, since it cannot depend on `tessera-net`), from P1 (decision 0025) also property labels and kinds, enum labels through a property's mapper and nationality labels for the feed's `stat`, `item-property` and `look` values, and from P5 the family `map-region` (elements with row and DID, test-point locations, names, attributes `kind` and `region`) and the `effect` attributes `mount-type` and `war-steed` (`scenario-feed.md` §8.3, "P5 travel"; the client's resolver adapts them). From P1 three models, each a module: **`tooltip`**, the item examination tooltip model (`item-tooltips.md` §3–§8, §11–§14: the tooltip layout an item names and its row templates, row content, Tessera's wording for requirement, effect, set, slot and duration rows, comparison with equipped items, attached tooltips one level deep, and the per-record census categories of §12; `TooltipEngine` over `InstallTooltipData`, bounded caches); **`chargen`**, the data of the screens before world entry (`character-screens.md` §3–§5, §9; `player-character.md` §5, §8: race, gender, class and nationality lists in screen order, the creation check and request, appearance controls per race and gender, random names and name checks; one unreadable record is skipped and reported, never a failed load); **`inventory`**, the local player's inventory model (`character-panels.md` §3.3, §4, §6, §9; `lua-plugin-api.md` §15.1: backpack and equipment containers fed by events that the client adapts from `tessera-net`'s, the change list of each event, the backpack size and the slots each bag window shows, the paperdoll slot table, slot keys and bits and slot compatibility, the command of each item interaction, plugin indices and events). From P3 two more models: **`quest`**, the quest journal's presentation model (`quest-journal.md` §3–§9: the journal kept from the session's quest reports (`QuestJournal`, at most 65,536 quests and 262,144 objective reports), the log's rows with grouping, order, filters and header counts, the details pane, objective progress texts with their counters and player variables, tracker entries and the tracker's height budget, the quest chat lines, the NPC ring key chosen from a quest and its reported state, quest marker images, and the per-record census categories of exit criterion 1); **`npc`**, the NPC windows' presentation model (`npc-windows.md` §5–§11: dialogue page texts and the choice of conversation, choice or comment window, quest-offer content with the objective block and the reward rows of the feed's `QuestRewards`, vendor, barter, buy-back and loot entries with item names, icon layers and prices as coins, splitting an amount into coins and which coins show, the wallet catalog and the wallet window's categories, and the census categories of what the windows show). Both present only what the session reports: they never compute a quest state, an objective count, a price, a loot result or a reward amount (decision 0024 §2 principle 4). From P5 four more models, each a module: **`map`**, the map windows' data (`map-windows.md` §5, §7.3: the map-region list of R-nodes in tree order and R-dungeons by DID with each region's test point and census category C1–C6, and the place lookup's name normalisation, matching and ranking over coordinates, place keys and the names of nodes, areas, territories, regions and dungeons). The map tree has one decoder, `tessera-ui`'s `game::worldmap::MapTree`: the client hands `map` a neutral list of the decoded nodes and `map` adds the dungeon maps, so the census order and the `map-region` rows follow the tree the UI shows. The place index stays the walker's `--place` index in the client (built from block maps with `tessera-dat` and `tessera-world`), which passes its areas, territories and regions to `map` as candidates, so `findplace` and `--place` read one index and share `map`'s normalisation; **`travel`**, the travel window's rows (`travel-windows.md` §4.10, §8.1: one row per feed entry with the destination's or route's name, swift flag and level and the area's and territory's names, and the census of destinations and station rows); **`instance`**, the instance finder's entries (`travel-windows.md` §6.8, §8.2: kind and category headers from the fellowship-maker index, group rows and the template a size resolves to, size and tier menus, the level range, and the census of private-encounter templates); **`mount`**, the mount catalogue (`mounts.md` §2.3, §2.8, §14.10–§14.11: mount effects and the skills that toggle them, the mounts offered to each playable body, mount types and the war-steed rule, names, and the census of mount effects; the clips stay with the walker and `formats`' clip resolver). The P5 models likewise present only what the session reports and the install holds: no travel cost, level requirement, usability, eligibility, queue state or mount state is computed (decision 0024 §2 principle 4). None of these models parses bytes or knows `tessera-net` or `tessera-ui`. Depends on `formats`, `props`, `text` and `wstate` (`wvm` is allowed, §3, and not yet used; `mesh` only as a dev-dependency of the creation real-data test). Client and tools only: `tessera-server` never links it (LEGAL.md §11.1 rule 5; `tools/check-server-deps.sh`, §23.5). From P2: **`tooltip`** also models skill and effect examination tooltips (`skill-tooltips.md` §3–§13: `TooltipEngine::build_combat` for a skill or effect record or a live effect, effect lines of effects, skills and items, duration and live timer texts, numbers as message values, examination and skill icons) and puts every skill and effect record into one tooltip and one icon category (`classify_combat`, §14); the item model gains effect lines, skill-item rows and the `SkillGrantingItem` comparison (`item-tooltips.md` §6.16, §6.20, §8.3). A fourth module, **`combat`**, holds the typed skill and effect records (`SkillRecord`, `EffectRecord`; `combat-data.md` §6, §7) and `CombatData`, the install's combat data read on demand with bounded caches: the panels' data (consideration and name colours, health-meter states, vital display overrides, target-frame decoration and level display, effect icons and rows, cast-bar labels and styles, defeat records, floating-text settings and avoidance words; `combat-ui.md`, `game-controls.md` §5.8, §7.1, §7.5, `interaction.md` §3.8, §8) and the script references of combat visuals (skill behaviours, attack hooks, inductions, channels, effect, interval and combat-state scripts, the death and revive labels, the stance of a figure's worn items; `combat-visuals.md` §2–§7, §11). Neutral types: nothing in it decides a gameplay outcome (decision 0024 §2.4). |
| `tessera-entity` (`crates/entity`) | L4 | M4 | Main-thread entity store: generational entity IDs, sparse-set component columns, the client's component sets (transform, network ID, render and animation handles as type parameters, landblock membership of streamed statics), per-frame change and despawn lists, deferred removal ([0008](decisions/0008-entity-storage.md)). Depends on no other Tessera crate; L4 because it holds the client's game state, which lower layers never see. |
| `tessera-render` (`crates/render`) | L4 | M0 | wgpu renderer: device and surface, frame graph, pipelines and WGSL shaders, texture/mesh uploads, headless mode (§13). |
| `tessera-audio` (`crates/audio`) | L4 | M6 | Output backend (cpal behind the `device` feature; null and offline backends otherwise) and the project-owned mixer: channels and volumes, positional voices, gain chain, distance law, voice limit, request gates, reverb environments, sample-accurate scheduling; Vorbis/WAV decoding; player music (ABC dialect, instruments) (§14, [0019](decisions/0019-audio-backend.md)). |
| `tessera-scriptgraph` (`crates/scriptgraph`) | L3 | M6 | Continuous script-graph runtime: keeps script-table and scriptlet node instances alive over time (Switches, Sequences, Timers, Randoms, driven properties, intensities) and hands sound and sound-environment effects to sinks that the client wires to `tessera-audio`, and Particle-node effects (`particle-runtime.md` §5.4–§5.8: start gates, never-ending instances, stop hook, redirect listener) to sinks that the walker wires to `tessera-particles`. Depends on `formats` only, not on `audio` or `particles`. From P2 combat runs (`combat-visuals.md`, `script-table.md` §9.1): `Runtime::start_script_with` starts a script with a run-parameter set (`StorySystem_StoryID`, `Combat_AnimationSpeedMultiplier`, the strike parameters) and returns a `RunId`; a StoryHook node hands its type and its run's story to the sink, an Execute node asks the sink (`GetUpcomingTargets`, `GetUpcomingHitResult`) and fires the ports that match the answer, a Beam node asks the sink to start a beam and fires its ports on the arrival, and animation nodes reach the sink with their driven speed and `UseBoneOpacity`. Still no dependency on `anim` or `particles`. |
| `tessera-ui` (`crates/ui`) | L4 | M5 | Retained element tree built from UI layout records, user skins and plugin controls; layout, behaviour and focus; text; art tiled at native size into sparse 32-bit CPU canvases (one per surface-owning element), damage and paint jobs, the composition list that `render` draws (§15). From P1 the widgets of the character, inventory and examination panels (`game::items`, `game::examine`, `game::avatar`, `game::creation`, vocabulary `game::charvocab`): item slots with composed icons, quantities and overlays, bag item lists, paperdoll slots and item drag and drop (`character-panels.md` §3–§6, `ui-behaviour.md` §7); the examination tooltip panel with row assembly, text wrap, placement, comparison tooltips and pins (`item-tooltips.md` §4–§10); viewports as hosts of external content that the client renders offscreen (`render.md` §6.10) and the paperdoll's turning; the creation screens' fallback controls. It knows neither items nor commands: the client fills neutral slot and row descriptions and turns the slots' neutral interactions into commands. From P3 the NPC windows and the quest journal (`game::npc`, `game::quest`, vocabulary `game::npcvocab`): the conversation, choice and comment windows, the quest offer with its objective and reward rows, the vendor window with its buy, sell and buy-back pages, buy modes and quantity dialog, the barter window, the loot window with Loot All, the money display with its coin split and the wallet window (`npc-windows.md` §5–§9); the quest log with its grouped list, filters, counts, buttons and details pane, the quest tracker with its height budget and entry button, and quest markers drawn on the minimap and the world map (`quest-journal.md` §4, §6, §8.3), each with a Tessera fallback window built from code-made elements, with committed synthetic goldens of the windows (decision 0031 tier 1, `tests/golden/p3_*.png`; the vendor's buy-back page, quest markers and the history view have none yet). They take plain view structs (texts rendered, icons as image references, prices as coin counts), plus a text hook (`NpcTexts`, `QuestTexts`) through which the client supplies the install's fixed labels and number formats (its defaults are Tessera's own), and return neutral actions (`NpcAction`, `QuestAction`) that the client turns into session commands (§15). No GPU and no Lua dependency; depends on neither `script` nor `plugins`. From P2 the combat widgets (`game::combat`, `combat-ui.md`, §15.1): player and target frames, the selection's target, cast bars, effect rows, the floating-text layer, the selection indicator and name block, the combat-state display and the defeat panel, each in its install look or its fallback look; `game::quickslot` gains skill displays, the cooldown sweep, slot states and activation (`combat-ui.md` §9), and `game::examine` shows skill and effect tooltips with live timer rows (`skill-tooltips.md` §10.2, §12). The widgets know neither the session nor the world: the client feeds neutral data, screen anchors and the clock, and turns their neutral actions into commands. |
| `tessera-input` (`crates/input`) | L4 | M3 | Raw input → actions via a keymap; input contexts; gamepad (§16). From P1 the Tessera actions and default keys of the P1 panel commands (`toggle_character`, `toggle_bags`, `toggle_bag_1` … `6`, `pin_tooltip`; `keymap.md` §12.14.2 OPEN). From P2 (`keymap.md` §8.2, K47–K49; module `combat`): the 15 targeting actions (`target_nearest_enemy`, `target_next_enemy`, …, `target_self`, `target_assist`, `target_last`, `target_last_attacker`, `target_clear`) and `quickslot_1` … `quickslot_72`, buttons of the `global` context with exact modifiers and no repeat, with their original command names and the original's default keys (quickslots 1–48 on `1` … `0`, `Minus`, `Equal`, plain and with Ctrl, Alt, Shift); what they do is the client's (`combat-ui.md` §3.3, §9.2). The defaults take Backslash, Minus, Equal, F9 and F10 from the walker's own keys (`keymap.md` §8.2 OPEN). |
| `tessera-settings` (`crates/settings`) | L4 | M1 | Tessera's own settings file (INI-shaped), with a read-only importer for the original client's preferences file (§17). From P1 the creation choices of characters made on Tessera's creation screen where the session cannot carry their look (`CreatedCharacters`, section `[[CreatedCharacters]]`, keyed by server and name, bounded; `character-screens.md` §7.1 Tessera rule, decision 0025 D2). |
| `tessera-script` (`crates/script`) | L4 | M0 | Plugin-free sandboxed Lua 5.1 core: state creation from an apartment profile, library allow-list, limits and instruction budget, protected calls, text-only compilation, bounded pattern matcher, collector stepping, neutral Lua-table data reader. No plugin API names or plugin state ([0023](decisions/0023-plugin-host-split.md), §18.1). |
| `tessera-plugins` (`crates/plugins`) | L4 | M5 | Plugin host on the `script` core: discovery (`.plugin` XML), apartments, lifecycle, `import` and the `Plugins/` file index, the compatible Lua API surface (its UI classes drive `tessera-ui` elements), command table, request and chat queues, `.plugindata` persistence, the class helper ([0023](decisions/0023-plugin-host-split.md), §18.2). From P1 the `Turbine.Gameplay` inventory views `Backpack`, `Equipment`, `Item` and `ItemInfo` (`lua-plugin-api.md` §15.1, module `inventory`) over an `InventorySource` the client implements, with their events queued for step 7; the host never reads the install for them. From P2 the `Turbine.Gameplay` combat views (`lua-plugin-api.md` §15.2, module `combat`): actors' morale, power, level and target, `EffectList` and `Effect`, `LocalPlayer:IsInCombat()` and the trained `SkillList` with its skills, over a `CombatSource` the client implements (`CombatModel` is a ready-made source that derives the §15.2 events from session-level inputs), with `CombatViewEvent`s queued for step 7 and delivered only to view objects that existed when they were raised; a plugin `Quickslot` holding a skill or an item hands its use to the client as a game action (`PluginUi::take_game_actions`). From P3 the wallet and money views `LocalPlayer:GetWallet()`, `Wallet`, `WalletItem`, `GetMoney()`, `GetMoneyComponents()` and their events `ItemAdded`, `ItemRemoved`, `QuantityChanged` and `MoneyChanged` (`lua-plugin-api.md` §15.3, module `wallet`) over a `WalletSource` the client implements (`LocalPlayerSource::wallet`); the client precomputes each `WalletItem`'s values from the install, and the host applies the API's rules (index range, money split, one object per entry per apartment, which apartments receive an event). |
| `tessera-net` (`crates/net`) | L4 | M0 | Transport-agnostic session layer: `Backend` trait, domain events, sandbox backend, test-server backend (`remote`, decision 0030; M7), snapshot interpolation (§19). From P0 the scenario feed (feature `scenario`, default on; `scenario-feed.md`): scenario parser, resolution contract, the sandbox's run-time feed, the headless driver and the canonical event log. From P1 ([0025](decisions/0025-p1-character-look-on-test-server.md)): character looks (`Look`, `LookEntry`, `PropertyValue`), held-item slots (`Equipped::slot`), creation in the plain sandbox, the `remote` backend at protocol 1.1, and the overlay backend that runs the feed's Character and Inventory groups over the `remote` backend (§19.2). Decodes no game data. From P2 ([0032](decisions/0032-p2-targeting-on-test-server.md), `scenario-feed.md` revision 6): selection is local on the `remote` backend: a `SelectTarget` of a known target is answered with `TargetChanged` for the player, first in the next poll, and nothing goes on the wire; overlay runs route it there too. No protocol minor; overlay files may not script the Combat group (§19.2). |
| `tessera-protocol` (`crates/protocol`) | L1 | M7 | Tessera's own wire protocol (`docs/specs/tessera-protocol.md`): message schema, framing, versioning, the sans-IO client and server connection state machines, property values, movement and time-sync helpers, key fingerprints and connect strings. Version 1.1 from P1 (minor 1, [0025](decisions/0025-p1-character-look-on-test-server.md)): looks appended to the character and entity messages, `Equipped.slot`. No I/O and no async code; feature `transport` adds the server key files and the rustls/quinn configurations of decision 0030. Shared by the `tessera-net` test-server backend, the test server and the load tools. |
| `tessera-client` (`apps/client`) | app | M0 | The game client binary: window, frame loop, wiring (§20). From P1: the character selection and creation screens (`screens`, run before world entry by `pre_world`, `--character-select`), the local player's state, the character panel and bag windows, item tooltips and the plugin inventory views (`character`), received looks and held-item slots on the walker's figures, overlay runs (`--scenario` with `--server`, `scenario::overlay`) and plugins in headless scenario runs (`scenario::plugins`). From P2 (`combat`): the local player's combat state from the session (`CombatState`: selection, kept vitals and levels, combat state, effects, casts, quickslots, cooldowns, learned skills; local clearing of a despawned selection, decision 0032 D2), selection by mouse pick and targeting keys, quickslot use, the combat widgets filled from `tessera-gamedata` (`CombatHud` over `CombatInstall`), skill and effect tooltips, the plugin combat views' source (`PluginCombat`) and the headless combat screens of the P2 exit criteria (`combat::headless`); in the walker, `walker::combat`: the combat-visual state machine of `combat-visuals.md` §9 (stories, strikes and hooks, inductions and channels, effects, stance, defeat and revive) behind the neutral per-entity API `CombatLayer` (casts, strikes, effects, combat state, defeat, revive, vitals and the selection, by network ID, or a whole `CombatEvent`), figure picking and screen anchors (`walker::combat::pick`), the clips scripts start on figures (`walker::characters::combat_anim`) and session figures as script owners with beams (`walker::effects::session`). From P3 the NPC windows (`npc`: using an NPC through one entry point with the use range of `interaction.md` §3.4.1, the input action "use nearest NPC in range" on U, the windows' session state mirrored from the feed's events, the windows on the game UI from the install's layouts or as fallback widgets, the wallet window, and the session's wallet model behind the plugin wallet views, also in headless scenario runs) and the quest journal (`quests`: the reported quest state and quest markers, the quest log with its details pane on L, the quest tracker, quest chat lines of chat type 21, quest markers on the minimap and the world map, and world-anchored indicators over NPCs with a quest ring, `--quest-icons`); `--open-panel` takes the windows' names (§15). |
| `tessera-dattool` (`apps/dattool`) | app | M0 | CLI: `detect`, `info`, `ls`, `stat`, `cat`, `extract`, `strings`, `validate`, later `diff` (§24). |
| `tessera-explorer` (`apps/explorer`) | app | M1 | GUI asset browser: archives, strings, textures, icons, later meshes and maps (§24). From census 3 also: the landblock logic card with its interactive parts (day and night lights for a chosen day fraction, a simulated journal for the phasing boxes, per-instance script views with the table set and switches), decal previews in the 3D view (box, axes, preview mesh with the decal's material; `decal_preview`), the mount viewer (rider and mount at `Mount_Connection`, gait selector; `mount_view`) and the drama player (Decisions T1/T2/T5; `drama_play`) in the character view, and the node display of 0x07/0x0C records (`script_view`). From P1 the item card previews the item's examination tooltip with `tessera-gamedata`'s tooltip model. |
| `tessera-server` (`apps/server`) | app | M7 | Project-owned headless test server speaking the Tessera protocol over QUIC (decision 0030), with original test content only (`docs/specs/server-content.md`, `test-server.md`): accounts with Argon2id, characters, world instances, the 30 Hz simulation, interest management by landblock, chat relay (§19); from P1 protocol 1.1: it stores each character's look and held-item slots as its client sent them (character files at most 1 MiB) and relays them, never judging them (`server-content.md` §4.1). A library plus a binary, so tests and the load tools run it in-process. Movement design planned in M4.5 (not built): server-side movement on `tessera-physics` with the same fixed step as the client (§20.3), identical to the client's only over the same collision sets (on install data generated scenery, statics and water, which `world` builds, §10.3, §10.5). The server as built links no physics and validates movement by speed and extent only; `tessera-physics` decodes game data, so the P0 rules (§3, §23.5) refuse it in the server's graph (see the OPEN below this table). |
| `tessera-loadtest` (`apps/loadtest`) | app | M7 | Headless bot clients built on the `tessera-net` test-server backend: connect storms, walking, chat, churn, cheating and soak scenarios against an in-process or remote `tessera-server`, with desync, leak and latency reports (`test-server.md` §13). |

> **OPEN (M7, [0013](decisions/0013-physics-crate.md)):** whether `apps/server` depends on
> `tessera-world` for collision-set building on install data, or the asset-free part of that
> building (sets from decoded records, scenery placement, the `water.md` §6.2 rule) moves
> into a module the server can use without `world`'s asset, animation and text
> dependencies. Decide with `server-content.md`; until then `physics` alone does not give
> movement identical to the client's. The M7 server as built has neither: `server-content.md`
> §3.2 and §10 E1 keep v1 servers free of game data (no terrain heights, no collision; no
> dependency on `tessera-dat`, `tessera-formats` or any crate that decodes game data, which
> `tessera-physics` does today), and movement is validated by speed and extent only. The
> owner OPEN in `server-content.md` §3.2 governs any change.

> **Settled (P0, [decision 0024](decisions/0024-parity-track.md) §3):** `props`, `text` and
> `wstate` stay separate crates. The name `tessera-gamedata` goes to the presentation data
> layer above them (L3, row above). The module boundaries of §7 are unchanged. (This was the
> M1 OPEN on merging the three crates into one of that name; §25 row 3.)

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
| 0x79 (property sets), landblock instance sets | mount effects, mount types, rider fields, template mount stats, landblock riders | `gameplay::mounts` | census 3 | `mounts.md` |
| landblock instance sets, 0x79 | NPC drama lists, actions and facets, scripted fights, starting idles, Tessera's playback rules (T1–T6) | `gameplay::drama` | census 3 | `npc-drama.md` |
| landblock instance sets, 0x79 | quest-state phasing (check lists, box test, script path), logic boxes, world-event wiring and conditions, travel hooks, property responses, version objects | `gameplay::world_logic` | census 3 | `world-logic.md` |
| landblock instance sets (`Entity_InstanceScriptTable`) | instance script tables as property trees, an entity's table set and key lookup, link targets, member-override names | `script_table::instance` | census 3 | `instance-scripts.md` |
| 0x18 | block-map descriptors | `block_map` | M3 | `landblock.md` |
| — (inside 0x04), 0x05 | skeleton (the Havok tagfile slice of a 0x04 setup, handed over by `setup`), animation clip | `tessera-havok` | M4 | `havok-data.md` |
| 0x56000000 | class library (WLib); its compiled code and message chunks in `tessera-wvm` | `wlib` | M3 (code: census 2) | `wlib.md`, `wlib-bytecode.md` |
| 0x70–0x77 | object state (WState) | `wstate` | M3 | `wstate.md` |
| 0x22 | UI layout | `ui_layout` | M5 | `ui-layout.md` |
| 0x0A | sound info and sound payload | `sound` | M6 | `sound.md` |
| 0x07 | script tables (VFX, sound, animation events); typed Light (13) and MeshFX (41) nodes, and the MeshFX nodes a run starts (`resolve`), from census 3 | `script_table` | M4/M6 | `script-table.md` |

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

> **OPEN (implementer, 2026-10-07):** `tessera-world` makes engineering choices that this
> section does not state: the retry and failure rules, the velocity-estimate details, the
> decay of "the slowest recent bundle job", corridor hysteresis, and the frill budget
> projection. The default residency budget is also out of date in `landblock.md` §8.1. These
> are listed for confirmation in the implementer OPENs of
> [`landblock.md` §8.1](specs/landblock.md#s8-1) (2026-10-07).

> **OPEN (implementer, 2026-10-07, meshes and materials):** the third bullet of this
> section says "the bundle loader then requests the meshes and materials those
> reference". `tessera-world`'s bundle job requests none: it resolves each object chain to
> its setup and part DIDs and stops there. The walker requests parts and textures itself,
> by draw distance and within its GPU budget (`walker.md` §4, §14.5), which a request at
> bundle time would bypass (every resident landblock's parts would load, whatever their
> distance). The two specs disagree; which one holds? If `walker.md` does, the bullet
> should say that the bundle resolves objects to setups and parts and the consumer loads
> them.

> **OPEN (implementer, 2026-10-07, streamer details):** further `tessera-world` choices
> that this section does not state (default byte budgets and per-frame limits, the
> one-update overshoot of the residency budget at integration, the re-admission margin
> of a seam closure, the velocity estimate's sample rules, what a data error in a
> world-system input does, and the water lookups with `build_water` off) are listed for
> confirmation in these implementer OPENs of [`landblock.md` §8.1](specs/landblock.md#s8-1),
> all dated 2026-10-07: "streamer defaults", "transient overshoot", "re-admission margin of
> a seam closure", "lookahead (a) continued", "data errors in world-system inputs" and
> "`build_water` and the water lookups".

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

> **OPEN (implementer, 2026-10-07):** the bundle loader requests no neighbours.
> `tessera-world`'s residency admission guarantees the east, north and north-east
> neighbours of near- and mid-ring landblocks (`landblock.md` §8.1). The normals of
> `landblock.md` §3.14 read all eight. See the neighbours OPEN in `landblock.md` §8.1.

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

> **OPEN (implementer, 2026-10-07):** the bullet above does not match the code. The landblock
> bundle (`tessera-world`) does not build layer products. The walker builds layer
> resolution, masks, light maps and far-band bakes from the bundle's landblock data on its
> own terrain workers (`walker.md` §14.1). Should this bullet describe that split?

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

> **OPEN (implementer, 2026-10-07):** `tessera-world` builds the landblock water with every
> collision set, whatever its `build_water` option says. It loads the water tables only
> when water or scenery is on, so with both off the set is dry (`landblock.md` §8.1, water
> OPEN).

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
  static, skinned) → alpha-tested → sky → transparent → walker overlays when shown (HUD and
  walker minimap, `walker.md` §6; under the game UI) → UI (one quad per UI surface,
  `render.md` §6) → developer overlay (egui frame stats, planned) → software cursor (only
  when used) → present ([0015](decisions/0015-ui-rendering.md) Decision 1.6). Shadows,
  bloom and other post effects are added as passes later.
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

- **Output:** cpal behind the `tessera-audio` feature `device`, with a project-owned mixer
  (Decision, [0019](decisions/0019-audio-backend.md); kira was considered and not used).
  Without the feature, the crate builds with no system audio libraries and offers a null
  backend (wall-clock mixing, output discarded; `--headless`) and an offline backend
  (renders into a buffer; tests and tools). Requirements: streamed playback for music, many
  short positional sounds, per-channel volumes, smooth crossfades between music regions.
- **Engine (`tessera_audio::engine`, Decision 0019):**
  - **Main thread:** `AudioEngine` takes `SoundRequest`s and applies the request gates of
    `sound.md` §6.8. Once per frame it computes each voice's gain (gain chain §6.1, distance
    law §6.2), panning relative to the camera (§6.7), the gain-0 rule and the voice limit
    (§6.6).
  - **Audio thread:** the mixer receives the results through a wait-free command ring
    (§21). It resamples by pitch, loops, starts and stops at audio-clock frames with sample
    accuracy (`instruments.md` §12), fades out linearly (§6.5) and feeds the reverb of the
    active environment (§8). It never allocates, frees or locks.
  - **Back to the main thread:** finished voices return through an event ring, and tracked
    requests produce completion events for their script nodes.
  - **Payloads:** decoded payloads come from the asset workers through a one-shot payload
    channel. Streamed sources are decoded by a feeder thread (device and null backends;
    inline between blocks for the offline backend), never by the audio thread; the first
    4,096 frames of a new stream are decoded on the main thread when its voice starts.
  - **Back-end loss:** a lost device completes every live request and refuses new ones
    until the client calls `AudioEngine::reopen()` (decision 0019).
  - **Stats:** active and virtualised voices, device xruns, mixer overloads and mixer load
    (M6 exit criterion 4).
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
- **Rendering** (Decision, [0015](decisions/0015-ui-rendering.md)):
  - **CPU canvases.** Art is tiled at native size (no nine-slice scaling) and text is drawn
    into sparse 32-bit CPU canvases, one per element that owns a surface, byte-exact to the
    compositor arithmetic of `ui-layout.md` §9.6 and `fonts.md` §8.4. Only damaged
    rectangles are repainted. `tessera-ui` has no GPU dependency; it hands the client a
    composition list and the changed rectangles.
  - **GPU composition.** `render` composes one textured quad per surface in the UI pass
    (`render.md` §6), with the surface's opacity, scale and rotation.
  - **Nested surfaces** (plugin controls with a stretch mode, child windows, the minimap
    marker, portraits) are separate quads. They take their own opacity only and are clipped
    by their enclosing windows.
  - **Paint jobs** span frames: a large paint is split into bands within a per-frame paint
    slice and is never restarted. A window and its nested parts swap to the new picture
    together, once per generation. A newly shown surface is drawn within 100 ms.
  - **Cursor:** the OS cursor. It is hidden and grabbed only when the UI declines a
    mouse-look press (0014 D7).
  - **Budgets:** U1–U5 of 0015 §6 (main-thread time, GPU time, upload bytes, event frames,
    memory) are M5 exit criteria. The UI's upload budget is separate from the asset upload
    budget (§20.3 step 5).
- **Images** (0015 §4):
  - All image decoding runs on the asset workers; every decoder has a fuzz target.
  - Every image becomes one canonical 32-bit ARGB form, plus a source-format tag
    (*ARGB32*, *alpha-only*, *no alpha*, *other*). The tag decides whether an overlay image
    draws (`ui-layout.md` §9.6.3).
  - Plugin `.png` and `.bmp` files are refused with the unsupported-format error.
  - `SetBackground(path)` fails at call time without decoding: existence is checked
    against a file index of the whole `Plugins/` tree, which is built by a directory walk on
    an asset worker; the format is checked by a header probe of at most 64 bytes, cached per
    path (the stated exception (a) of §21).
  - Images that canvases draw from are pinned. Others stay in an LRU cache within the
    decoded-image budget. Past the hard cap, images get the placeholder.
  - An image with more than 4,096 pixels on an axis is cropped at decode to its top-left
    4,096 pixels on that axis.
- **Text:** font records and glyph atlases are decoded and drawn on the CPU into the same
  canvases (`fonts.md`). Tessera bundles only freely licensed fonts as fallback.
- **XML:** skins and plugin descriptors are parsed with a strict, bounded pure-Rust XML
  parser (no DTDs, no external entities).

The layout record grammar and the per-element UIData variants are in `specs/ui-layout.md`;
the font record type is in `specs/fonts.md`. Behaviour, widgets, game panels and the chat
window are in `ui-behaviour.md`, `ui-widgets.md`, `game-panels.md` and `chat-window.md`.

**P3 controllers: NPC windows and the quest journal** ([0024](decisions/0024-parity-track.md)
P3; [`npc-windows.md`](specs/npc-windows.md), [`quest-journal.md`](specs/quest-journal.md)):

- **Split.** `tessera-ui` (`game::npc`, `game::quest`) holds the windows' controllers and
  links neither `tessera-net` nor `tessera-gamedata`. The client (`apps/client/src/npc`,
  `apps/client/src/quests`) keeps the session state, builds plain view structs from it with
  `tessera-gamedata`'s `npc` and `quest` models (install texts rendered, icons as image
  references, prices as coin counts) or, without an install, from the feed's literal refs
  alone, and hands them to each window's `show` together with a text hook (`npc::NpcTexts`,
  `quest::QuestTexts`: fixed labels, coin-count and amount formats, coin tooltips) through
  which the client supplies the install's own; the hook's defaults are Tessera's own. Every UI event goes to the controllers'
  `handle_event`, which return neutral actions (`NpcAction`, `QuestAction`); the client
  maps them to the session commands of `npc-windows.md` §10.1 and `quest-journal.md` §3.
- **Two trees, one vocabulary.** Each controller works on the install's layout, opened by
  `UILAYOUT` label with its root by `UIElementID` label (`npc-windows.md` §4,
  `quest-journal.md` §4.1, §6.1), or on Tessera's fallback window built from code-made
  elements with the same element IDs. The IDs, widget types, states and properties are
  looked up by label at run time (`game::npcvocab::NpcVocab`); their defaults are
  placeholders in ranges no data uses, so the fallback windows and synthetic trees use them
  directly and an unresolved label never matches real data. A window whose layout or root
  does not resolve falls back and is counted as `LayoutMissing`.
- **State mirrors events.** The NPC windows' state (`npc::state`) and the journal's
  (`quests::session`) are fed by `PlayerState::apply`, which every world session reaches
  (windowed walker, headless driver, tests). A feed window opens on `DialogueShown`,
  `QuestOffered`, `VendorShown` or `LootShown` and is hidden only on `WindowClosed`; a close
  button or Escape only sends `CloseWindow`. Each feed window has its own native-window
  instance. The client sends one `CloseWindow` for a window whose entity despawned or moved
  out of use range (`npc-windows.md` §5.7 Tessera rules).
- **Frame order** (windowed client, after the UI's own update): the NPC hub
  (`npc::NpcHub::frame`) takes the frame's UI events from the UI host's event collector,
  syncs the windows with the player state, sends their commands and the use action's
  `UseEntity`, and posts system lines; then the quest hub (`quests::QuestHub::frame`) reads
  the same events, turns quest notes into chat lines (chat type 21) and marker changes,
  refreshes the open journal windows incrementally and sends `TrackQuest` and
  `AbandonQuest`. Escape and Enter go to an active NPC window before the walker sees them.
  The install's names (`NpcRefs`) and facts (`NpcData`) load on two background threads, off
  the frame, once the install is open; no window opens before the names arrive, and facts
  that arrive later refill the open windows. The windowed client therefore shows the P3
  windows only with an install; the fallback windows serve the tests and golden images.
- **Markers and indicators.** Quest markers (`MarkerAdded` with a quest) are drawn as
  children of the radar's `MinimapViewport` and of the world map's `MapBackground`, above
  the map and below the player marker, through hooks the radar and the world map expose for
  notes that game code places. The quest ring over an NPC is, until the walker starts
  script keys on entities, a world-anchored glyph (the walker HUD's bitmap font) projected
  with the walker's camera and drawn after the scene and before the game UI
  (`quests::indicators`, `quest-journal.md` §8.2 OPEN).
- **Stand-ins.** `tessera-ui` has no dialog box or popup menu widget yet: the abandon
  confirmation is a second click with a notice line, and the tracker's menu posts its items
  as a notice line (`quest-journal.md` §4.8, §6.6 OPENs). The plugin wallet views read the
  client's `npc::SessionWallet` (§18.2).

<a id="ui-combat"></a>
### 15.1 Combat widgets (P2, `game::combat`)

[`combat-ui.md`](specs/combat-ui.md) is normative; this section says where the parts sit.
The combat widgets are controllers over the same element tree as the panels, built like the
P1 widgets: no GPU, no Lua, no session and no world.

- **Elements.** `CombatUi` holds the player frame (E5) and target frame (E6) with their vital
  fields, class-resource pips and frame decoration, the selection's target (E7), the player
  and target cast bars (E11, E12, for casts, inductions, channels and crafts), the player and
  target effect rows (E8, E9, with timed and pulsing icons E10), the floating-text layer (E13:
  damage, critical, avoidance and heal texts; the gain texts E14 and stack counts are not
  shown yet), the selection indicator and name block (E1, E3), the combat-state display (E17)
  and the defeat panel (E18). The quickslot bar's skill displays, cooldown sweep, slot states
  and activation (E15, E16, `combat-ui.md` §9) live in `game::quickslot`; skill and effect
  tooltips go through `game::examine` (the same panel as item tooltips), whose live timer
  rows count down a skill's recovery and an effect's remaining time without rebuilding the
  tooltip (`skill-tooltips.md` §10.2, §12).
- **Two looks.** With an install, each element is created from the opened panels (P3 `toolbar`,
  P4 `AvatarPanel`, P5 `OpponentPanel`, the `gameplayactivatables` panels) or from templates of
  the `floatytext`, `effects` and `Dialog` layouts, found by label at UI start
  (`apps/client/src/combat/boot.rs`). An element whose layout, root or parts are missing (always
  with `--no-data`) takes its fallback look of `combat-ui.md` §13: Tessera's own boxes, meters
  and text in the bundled font on a 1024 × 768 reference, scaled to the screen (TR41). Both
  looks paint through the canvases of §15 like any other surface.
- **Input.** The client fills neutral data (`game::combat::data`: frame, cast, effect and
  floating-text views by entity number) from its combat state and `tessera-gamedata`. The
  widgets turn clicks into neutral `CombatAction`s, which the client maps to commands, so no
  display update sends a command (CUI8). Screen anchors of entities come from the client
  through `AnchorSource` (the walker's figures, or the synthetic camera of `combat-ui.md` §13.6 without a
  world), and elapsed times from the `now` the client passes: the scenario's virtual clock in
  headless runs, so the same scenario gives the same element states (CUI9).
- **Order.** Floating elements are screen-space layers drawn after the 3D scene and before
  the panels: name blocks, floating texts (oldest first), then the selection indicator
  (TR40). Panels keep the UI's own order.
- **Lifetime.** `CombatUi::new` attaches the widgets after the panels open; `CombatUi::detach`
  removes their layers, the fallback root, the code-made effect rows and the handler
  registrations before a rebuild (world entry again, reconnect, skin switch). When the
  install's combat data arrives after the widgets attached, the client rebuilds them from its
  state.
- **Tests.** Element states are read from the UI tree, not from pixels (`combat-ui.md` §16):
  `crates/ui/tests/game_combat.rs`, and in the client the synthetic goldens of
  `apps/client/tests/combat_ui_golden.rs` (committed, `--no-data` looks) and the local
  real-data goldens of `combat_ui_real_data.rs` (§23.3).

**P5 game types: map, travel and instance-finder windows** ([0024](decisions/0024-parity-track.md)
P5; [`map-windows.md`](specs/map-windows.md), [`travel-windows.md`](specs/travel-windows.md),
[decision 0036](decisions/0036-p5-map-census-and-panel-goldens.md)):

- **Split.** As for P3: `tessera-ui` holds the controllers and links neither `tessera-net` nor
  `tessera-gamedata`; the client keeps the session state, builds plain view structs with
  `tessera-gamedata`'s `map`, `travel` and `instance` models (install texts rendered, map
  images and icons as image references, costs as coin counts) or, without an install, from
  the feed's literal refs, and maps the controllers' neutral actions to session commands
  (`ChooseTravel`, `JoinInstance`, `LeaveQueue`, `LeaveInstance`, `Dismount`, `CloseWindow`).
- **Map window** (`game::worldmap`, `game::radar`, `game::map`, grown from M5): the full-screen
  and windowed presentations with the presentation switch, navigation (child-map buttons,
  going up, the map menu, the player's map), the dungeon-map view stretched into its frame,
  table notes with their filters, run-time notes, the player marker and feed markers on the
  world map, notes, blips and feed markers on the radar, and the note-type filter menu
  (`map-windows.md` §3–§6). Markers are drawn through the hooks the radar and the world map
  expose for notes that game code places (P3), with the drawing order of §6.2 and §6.8.
  Hidden by Tessera rule: the stable-master button, the quest-guide option and the fog of war
  (§8). The filter values and the presentation switch are Tessera settings
  (`tessera-settings`, initialised from the install's option defaults at run time).
  `game::worldmap::MapTree` is the only decoder of the map tree: the client hands its nodes
  to `tessera-gamedata`'s `map` model for the region list and the census. A Tessera fallback
  map window for runs without an install is not specified yet (`map-windows.md` §8 OPEN).
- **Travel and instance windows** (module names chosen by the implementer, additive to the
  `game` index): the travel window with territory headers, marks, search, selection display
  and the coin go button (paper-item and buy-now buttons hidden); the end-travel box and the
  loading-screen presentation of a started travel; the fellowship maker's instance-finder page
  with kind and category headers, group rows, size, tier and level controls, the join button,
  the confirm page and the queue alert; the invitation window; the instance-travel window
  (`travel-windows.md` §3–§7). The listings board is shown unavailable. Each has a Tessera
  fallback window built from code-made elements with the same element IDs, which the tests
  and the committed goldens use.
- **State mirrors events.** The windows open and refresh on `TravelShown`, `TravelStarted`,
  `TravelEnded`, `InstanceListShown`, `InstanceChanged` and `InstanceQueueChanged`
  (§19.1), reached through `PlayerState::apply` as P3's are. A go or join click only sends the
  command; the window closes when the feed answers (`travel-windows.md` §4.10 rule 6). The
  client never moves the player for a travel: arrival is the feed's `teleport`.
- **Client built-ins** of the one dispatcher (decision 0020): `findplace TEXT` (place lookup,
  `map-windows.md` §7.3: results as chat lines, the first shown with a lookup marker) and
  `leaveinstance` (`travel-windows.md` §6.6). Mounting (`/ride`, `walker.md` §16) is walker
  presentation, not a UI widget; the plugin mount views live in `tessera-plugins`
  ([`lua-plugin-api.md`](specs/lua-plugin-api.md) §15.5).
- **Goldens.** Committed synthetic goldens of the world map with markers, the travel window and
  the instance finder (decision 0031 tier 1, as decision 0036 §2 lists them; the world-map one
  waits for the fallback map window above); local real-data goldens of the same panels
  (tier 2).

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
- **UI band and sink** (Decision, [0014](decisions/0014-ui-input-bridge.md)): the context
  stack gains a UI band of owned, prioritised entries above the game contexts (the UI's
  input maps, `keymap.md` §3.2). UI commands are ordinary actions that carry their command
  name and ID, with activation filters (press, release, tap, double) computed in the mapper.
  The mapper offers each resolved event to the UI in order through a synchronous sink; the
  UI's verdict decides who owns the hold, and a declined command goes on to game code
  (`keymap.md` §7.8). The sibling rules do not change: `input` and `ui` stay independent,
  and the client implements the sink (§3).

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
- **Plugin-free core** (Decision, [0023](decisions/0023-plugin-host-split.md)): apartment
  profile and state creation, `Limits`, protected calls and error description, the `pcall`
  guard, the instruction budget and its hook, text-only compilation, the bounded pattern
  matcher, the metered finalizer proxy, collector stepping and the neutral Lua-table data
  reader. It holds no plugin API namespace, `import`, plugins root, shared plugin state,
  command table, chat-line queue or `.plugindata` file naming. Its host trait covers only
  the script log and error reports; the plugin `HostApi` lives in `tessera-plugins`
  (§18.2).
- **Threading** (Decision, [0014](decisions/0014-ui-input-bridge.md) D8): Lua states are
  not `Send`.
  - All plugin code runs on the main thread in frame-loop steps 1 and 7 (§20.3), except the
    unload of every apartment at client exit, which runs after the last frame. This is
    what `lua-plugin-api.md` §3.3 and `plugin-runtime.md` §10.3 call "between frames".
  - Plugin events raised by game or session state (the chat-line event of
    `lua-plugin-api.md` §11.3, the gameplay views' change events) are queued and delivered
    in step 7. UI changes driven by game or session code (show and hide, activation,
    focus, screen resize, `HideWhenField`) are applied in step 7 too, so the plugin
    notifications they cause run there.
  - Input handlers (step 1), the `Update` broadcast and the collector step (step 7) are
    bounded by the per-call instruction budget and are not sliced.
  - The step-7 deferred work (queued loads, plugin-data callbacks, game-raised events, the
    logout unload) is sliced by whole items: 2 ms of plugin time per frame, at least one
    item (except while a load waits for the first `Plugins/` file index,
    `lua-plugin-api.md` §3.3).
  - Plugin time is measured separately from the UI's own time (0015 §6, U1).

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
- **Crate** ([0023](decisions/0023-plugin-host-split.md)): `tessera-plugins` holds the
  plugin API's namespaces, `import`, `NativeApi` and the object model, descriptors, the
  `PluginManager`, the command table, the request and chat-line queues, the `.plugindata`
  wrapper over the core's reader, the plugin `HostApi`, the clean-room class helper
  ([0016](decisions/0016-plugin-class-helper.md)) and the bindings of the plugin UI
  classes, which create and drive `tessera-ui` elements (`plugins` → `ui`, §3).
- **`Plugins/` file index:** a virtual file system index of the whole `Plugins/` tree,
  built by a directory walk on an asset worker when the host starts and again when the
  plugin manager refreshes its list (`plugin-descriptor.md` §4.2, `lua-plugin-api.md`
  §4.3, §6.7), and swapped in whole on the main thread. Image paths are checked against it
  (§15, 0015 §4.4).
- **Names:** the plugin API's global namespace names are the original vendor's, used
  verbatim for interoperability only, under the rationale and binding rules of
  `docs/LEGAL.md` §17 (M5 exit criterion 6).

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

<a id="net-gameplay"></a>
**Gameplay events and commands (P0, grown in P1 and P3).** [`scenario-feed.md` §9](specs/scenario-feed.md#s9) is
normative for these variants, their fields and their session rules; this list only names them.
They extend `Event` and `Command` by **group**, each group an enum of its own and
`#[non_exhaustive]` like `Event` and `Command`, so later milestones add variants without a
breaking change: `Event::Character(CharacterEvent)`, `Event::Inventory(…)`,
`Event::Combat(…)`, `Event::Npc(…)`, `Event::Social(…)`, `Event::World(…)`,
`Event::Progression(…)`, and the same groups for `Command`. Variant names must be unique
among all events and among all commands, groups included, because `Event::name`,
`Command::name` and the canonical log use the inner name only. The `tessera-net` unit test
`session::name_tests` enforces it: it reads every `name()` of `Event`, `Command` and their
groups from the source and fails on a repeated name or a name that differs from its
variant. Record references travel
as `DataId` or as literal labels (`Ref`), texts as literals or as references the client half
resolves (`Text`); `tessera-net` interprets neither. Numbers are whatever the driving backend
sends: the scenario feed's are scripted, never derived (decision 0024 §2.4).

| Group | Variant | Kind | Meaning |
|---|---|---|---|
| Character | `CharacterInfo` | event | One entry of the character list: name, body, class, level and (P1) look |
| | `CharacterRefused` | event | A character creation was refused, with a reason |
| | `CharacterRemoved` | event | (P1) A character left the list |
| | `LevelChanged` | event | An entity's level |
| | `PropertyChanged` | event | (P1) A scripted property value of an entity (stat rows): property ID and value, or none to clear it |
| | `CreateCharacter` | command | Create a character: name, optional appearance (with the look, P1), class and (P1) nationality |
| | `DeleteCharacter` | command | Delete a character by name |
| Inventory | `ContainerSize` | event | A container's number of slots |
| | `ItemAdded` | event | A new item instance: record, place, quantity |
| | `ItemMoved` | event | An item now sits in another container slot |
| | `ItemQuantity` | event | An item's stack quantity |
| | `ItemRemoved` | event | An item is gone |
| | `ItemCondition` | event | An item's current and maximum condition |
| | `WalletChanged` | event | The amount held of a currency (coin or an item currency) |
| | `ItemProperty` | event | (P1) A scripted property value of an item instance: property ID and value, or none to clear it |
| | `MoveItem` | command | Move an item to a container slot |
| | `EquipItem` / `UnequipItem` | command | Equip an item (optionally into a slot) / take it off |
| | `SplitStack` | command | Split a quantity off a stack |
| | `DestroyItem` | command | Destroy an item |
| | `UseItem` | command | Use an item, optionally on a target |
| Combat | `TargetChanged` | event | An entity's target (or none) |
| | `VitalChanged` | event | An entity's vital (health, power or a class resource): current and maximum |
| | `CombatState` | event | An entity enters or leaves combat |
| | `QuickslotChanged` | event | A quickslot's content: a skill, an item or empty |
| | `CastStarted` | event | A cast, induction, channel or craft bar starts: what, target, duration |
| | `CastDelayed` | event | The running cast is pushed back |
| | `CastEnded` | event | The running cast ends: completed, interrupted or cancelled |
| | `CooldownStarted` / `CooldownEnded` | event | A skill or item cooldown starts / ends |
| | `EffectApplied` / `EffectRemoved` | event | An effect instance on an entity: what, duration, stacks, source / its removal |
| | `VitalDelta` | event | A damage or heal number with its hit result (floating text), source, target, skill and damage type |
| | `Defeated` / `Revived` | event | An entity is defeated (optionally by whom) / revived |
| | `SelectTarget` | command | Select a target or clear it |
| | `UseSkill` | command | Use a skill, optionally on a target (mounting is a mount skill) |
| | `CancelCast` | command | Cancel the player's cast |
| | `SetQuickslot` | command | Put a skill or item into a quickslot, or clear it |
| Npc | `DialogueShown` | event | A dialogue page in a window: speaker, text, numbered options |
| | `QuestOffered` | event | A quest offer window |
| | `QuestRewards` | event | The reward amounts of the open quest offer (P3) |
| | `QuestState` | event | A quest is active, completed, failed or removed |
| | `QuestObjective` | event | An objective's counter (count, total, done) |
| | `QuestTracked` | event | A quest is tracked or untracked |
| | `VendorShown` | event | A vendor window: entries with price and stock |
| | `BuybackShown` | event | The vendor window's buy-back list (P3) |
| | `LootShown` / `LootTaken` | event | A loot window: entries and coin / one entry was taken |
| | `WindowClosed` | event | A window of any kind closed |
| | `UseEntity` | command | Use (interact with) an entity |
| | `ChooseOption` | command | Choose a dialogue option |
| | `CloseWindow` | command | Close a window |
| | `AnswerQuest` | command | Accept or decline the offered quest |
| | `AbandonQuest` / `TrackQuest` | command | Abandon a quest / track or untrack it |
| | `Buy` / `Sell` | command | Buy a vendor entry / sell an item, with a quantity |
| | `TakeLoot` | command | Take one loot entry, or all |
| | `BuyBack` | command | Buy back a buy-back entry (P3) |
| Social | `ChannelJoined` / `ChannelLeft` | event | The player joined / left a named chat channel |
| | `FellowshipChanged` | event | The full fellowship member list with the leader (empty: none) |
| | `FellowshipInvite` | event | An invitation to a fellowship |
| | `TradeShown` | event | A trade window with a peer |
| | `TradeOffer` | event | One side's full trade offer: entries and coin |
| | `TradeReady` | event | One side's ready flag |
| | `FriendList` / `IgnoreList` | event | The friends list with online flags / the ignore list |
| | `PerformanceStarted` / `PerformanceStopped` | event | An entity starts (tune, delay) / stops a music performance |
| | `PerformanceReady` | event | (P4) A fellowship member's part waits for a synchronised start (member, entity, tune) |
| | `PerformanceSyncStart` | event | (P4) One synchronised start of every waiting part: who, the start time on the backend's base, the lead |
| | `PerformanceWithdrawn` | event | (P4) A member's waiting part was withdrawn |
| | `FellowVitalChanged` | event | (P4) A fellowship member's vital as the group reports it, with or without an entity |
| | `JoinChannel` / `LeaveChannel` | command | Join / leave a named chat channel |
| | `InviteToFellowship` / `KickFromFellowship` / `SetFellowshipLeader` | command | Fellowship management by name |
| | `AnswerInvite` / `LeaveFellowship` | command | Accept or decline an invitation / leave the fellowship |
| | `DisbandFellowship` | command | (P4) Disband the fellowship |
| | `OpenTrade` / `OfferTrade` / `SetTradeReady` | command | Open a trade with an entity / offer an item or coin / set ready |
| | `AddFriend` / `RemoveFriend` / `Ignore` / `Unignore` | command | Friends and ignore lists by name |
| | `ReadyPerformance` / `StartPerformance` | command | (P4) The player's accepted `/play … sync` / `/playstart` |
| World | `MarkerAdded` / `MarkerRemoved` | event | A map marker: location, label, icon, quest / its removal |
| | `TravelShown` / `TravelStarted` | event | A travel window: destinations and costs / a journey starts |
| | `MountChanged` | event | An entity mounts (which mount) or dismounts |
| | `InstanceListShown` / `InstanceChanged` | event | An instance-finder window / the player's current instance |
| | `ChooseTravel` / `JoinInstance` | command | Choose a travel or instance entry |
| | `LeaveInstance` / `Dismount` | command | Leave the instance / dismount |
| | `TravelEnded` | event | (P5) The travel presentation ends |
| | `InstanceQueueChanged` | event | (P5) The player's instance queue state: state, instance, seconds |
| | `MountVitalChanged` | event | (P5) A mount vital of an entity (durability, morale, power or fury): current and maximum |
| | `LeaveQueue` | command | (P5) Leave the instance queue |
| Progression | `ExperienceChanged` | event | Experience: current and needed |
| | `SkillLearned` / `SkillForgotten` | event | A skill was learned / forgotten |
| | `TrainerShown` | event | A trainer window: entries with cost and level |
| | `TraitPoints` / `TraitSlotted` | event | Available and total trait points / a trait slot's content and rank |
| | `ProfessionChanged` | event | A profession's tier, proficiency and mastery |
| | `RecipeLearned` / `CraftResult` | event | A recipe was learned / a craft succeeded, was critical or failed |
| | `ReputationChanged` | event | A faction's points and tier |
| | `TitleGranted` | event | A title was granted |
| | `TrainSkill` | command | Train a trainer entry |
| | `SlotTrait` | command | Put a trait into a slot, or clear it |
| | `Craft` | command | Craft a recipe a number of times |

Existing variants stay as they are and are reused (`scenario-feed.md` §9.3): `Connected`
carries the character list, `EntityCreated` / `EntitySpawned` / `EntityMoved` /
`EntityDespawned` drive actors, `AppearanceChanged` follows equipment changes, and the
existing `Chat` command and `ChatMessage` event carry two new channels, `Fellowship` and
`Named`. Session rules (`scenario-feed.md` §9.5): character events are surfaced from `Connected` on and
character commands are allowed in `Connected`; every other new event and command belongs to
`InWorld`, under the existing entity rules (`PropertyChanged` is an entity event). The
test-server backend refuses the new commands and the two new chat channels with
`NetError::InvalidCommand` (its server has no gameplay, LEGAL.md §11.1 rule 5), except
`CreateCharacter`, which it maps to the protocol's creation (P1, below and §19.2); the
sandbox without a scenario accepts and ignores them, except `CreateCharacter`, so client
panels open offline.

**Looks and slots (P1, [decision 0025](decisions/0025-p1-character-look-on-test-server.md)).**
`Appearance` gains an optional **look**: 1–48 `LookEntry` values (a data property ID and a
`PropertyValue`: bool, 32- and 64-bit integers, finite float, enum key, 32- and 64-bit
bitfields, data ID, RGBA colour or a short printable-ASCII string), strictly ascending by
property ID, at most 768 bytes encoded. The creation screen fills it with the values its
controls write; `CharacterInfo` carries it, so the selection screen can show a character's
figure. `Equipped` gains an optional `slot`, the equipment slot key (1–64) that holds the
item, so an item that fits either hand says which hand holds it; the feed fills it from its
equipment list. `tessera-net` interprets neither. A client applies a received look entry only
when a creation control of that body's player avatar in its own install writes that property
with that value kind (the receiver rule, `tessera-protocol.md` §7.4), never shows look
strings as text, and logs ignored entries at a bounded rate; without a slot the walker falls
back to the hand rule of `character-panels.md` §7. The client keeps at most 1,024 scripted
properties per entity and per item.

**Selection and the Combat group (P2, [decision 0032](decisions/0032-p2-targeting-on-test-server.md),
`scenario-feed.md` revision 6).** The Combat group's variants are unchanged; P2 gives them
their consumers in the client (§20.3). One rule changes for the test-server backend and
amends the refusal rule above: `SelectTarget` is no longer refused there but answered
locally (§19.2). On every backend the client half shows a new selection only when the session
reports it as `TargetChanged` for the player, so selection has one path whichever backend
answers it (`scenario-feed.md` §0 item 3), and it clears a selection locally, without sending
a command, when the selected entity is removed (`EntityDespawned`, or the despawns of an
`InterestReset`; decision 0032 D2). `UseSkill`, `CancelCast` and `SetQuickslot` stay refused
on the test-server backend, so the quickslot bar cannot be used in a `--server` run (owner
question 1 of decision 0032). Revision 6 adds no step, trigger, input form or event field to
the feed; a scenario that wants a strike at a given hook orders its `damage` and `heal` lines
(`combat-visuals.md` §15).

<a id="net-social"></a>
**The Social group in the client (P4, decision 0024 §5 P4, [decision 0035](decisions/0035-p4-social-on-test-server.md)).**
`tessera-client` hands every polled event, in arrival order, to the world session's
`social::SocialState` (`apps/client/src/social/`): the pure models of `tessera-ui`
(`game::chat::social` for the chat lines of `chat-window.md` §15.10, `game::social` for the
fellowship, friends, ignore list, invitation and trade of `social-panels.md` §9) take the
Social events, the member data of the Combat and Character groups (vitals, levels, defeat,
effects, targets) and the spawns and despawns. `ChatMessage` lines on `Tell`, `Fellowship`
and `Named` leave the state in the original's line forms with their chat-type masks
(rendered from the install's string tables, session values inserted literally, CT12), after
the ignore filter (CT8); the client derives the fellowship notices of TR4, the slot lines of
CT5 and the friend notices of CT9 the same way. The social panels (`social::SocialUi` on the
UI host) draw the models; their actions become the commands of `social-panels.md` §8.1. The
command dispatcher's P4 built-ins (`tell`, `reply`, `retell`, `fellowship`, `join`, `part`,
`channels`, `chsay`, `ch1` … `ch8`, `friend`, `ignore`, `invite`, `leave`, `dismiss`,
`promote`, `disband`, under their Tessera names and the install's `Commands` names) act on
the same state and send `Chat`, `JoinChannel`, `LeaveChannel` and the list and fellowship
commands. The plugins' `Party` views get each member list (`set_fellowship`) and the members'
values. The performance events go to the player music, which sends `ReadyPerformance` for an
accepted `/play … sync` and `StartPerformance` for `/playstart` and starts the waiting parts
only on the backend's `PerformanceSyncStart`; a backend that refuses `StartPerformance` (the
test server) gets the local start of `instruments.md` §12 P4 Decision 3. A `PerformanceStarted`
plays its tune solo-like at the processing time plus its delay (Decision 5). World entry and
disconnection end the session's fellowship for the music (the members' waiting parts are
withdrawn), and member characters that are neither listed, waiting nor performing are freed.
Every command the player sends passes `World::send`, which notes `KickFromFellowship` and
`LeaveFellowship` for TR4 (panels, dispatcher and the windowed input script alike); a panel
command the session refuses posts a type 1 line (TR13). Without a game UI, session lines
reach the plugins' `Received` with their mask's chat type (CT10), as the chat window passes
them with one.

**Map, travel, mounts and instances (P5, `scenario-feed.md` revision "P5 travel", §18.P5).**
Besides the four variants marked P5 in the table, existing variants gain optional fields:
`MarkerAdded.note_type` (a map-note type 1–64 whose icon the client draws when the marker
has none), `TravelShown`'s entries (`min_level`, `flags`: not discovered, requirements not met, not usable; `route`: swift
or slow; `area`), `TravelStarted.route`, `MountChanged.combat`, `InstanceListShown.source` and
`.featured` with entry fields (`level`, `size`, `tier`, `locked`), and `JoinInstance.size`,
`.tier`, `.level`. All values are scripted; `tessera-net` holds no data key and interprets
none of them. The canonical log writes an added field only when it is not at its default, so
logs written before the revision stay byte for byte the same. The new events follow the entity
rule and `LeaveQueue` belongs to `InWorld`; the test-server backend refuses `LeaveQueue` like the
other new commands and never produces the P5 events, so no protocol minor is needed
(LEGAL.md §11.1 rule 5).

<a id="net-backends"></a>
### 19.2 Backends

| Backend | Status | Description |
|---|---|---|
| **Sandbox** | M0 skeleton, grows each milestone; scenario feed from P0 | In-process, no sockets. Spawns the local avatar at a chosen position in the user's data and serves a static world: landblock-placed static entities only. This is the offline world walker of M3. From P0 it can also play a **scenario feed** ([`scenario-feed.md`](specs/scenario-feed.md), feature `scenario`, [0024](decisions/0024-parity-track.md) §4): a scenario file scripts actors, items, vitals, casts, effects, windows, quest states, markers, chat and the like as timed steps and as answers to client commands, and the sandbox emits them as the gameplay events of §19.1, the same events a server backend would send. The client parses the file and resolves its selectors against its own install through `tessera-gamedata` before the session starts, then hands the sandbox the resolved scenario (`SandboxConfig::scenario`) the way it hands over its `WorldSource`; `tessera-net` decodes no game data. It receives record IDs and table rows, optional locations and anchors, enum values resolved from labels, scripted values, literal text, and the display names of actors declared with `name(…)` (install text held in memory only, at most 64 bytes, sanitised), and it treats everything the resolver returns as untrusted (`scenario-feed.md` §3.3, §8.6). Values are scripted, never derived: the feed has no formula, table or behaviour taken from the game. A headless driver (`tessera_net::scenario::HeadlessDriver`) runs it on a virtual clock and writes a canonical event log that is the same on every run (§23). From P1 (decision 0025 D5) the plain sandbox (no scenario) also creates characters: a `CreateCharacter` with a valid, new name (fewer than 64 characters in the list) adds it and answers `CharacterInfo` with the command's body, class and look, and `EnterWorld` with that name uses the command's appearance as the player's; a taken name or a full list gives `CharacterRefused`. In memory only, for the sandbox's lifetime; `DeleteCharacter` stays accepted and ignored. Revision 4 of the feed adds `look` declarations (`look=` on `player`, `character` and `accept-character`), `remove-character`, `stat` and `item-property`, with values that are literal and bounded (`scenario-feed.md` §5.8, §9.5). Revision 5 (P3, for the NPC windows) adds the text function `action(…)` (`Text::Action`), the steps `rewards` and `buyback` with the events `QuestRewards` and `BuybackShown`, and the command `BuyBack` with its `buy-back` pattern and input form (`scenario-feed.md` §18.P3). Revision 7 (P4 social) adds waiting parts and one synchronised start (`perform … sync`, `perform-start`, `perform-withdraw`), fellow vitals (`fellow-vital`) and the commands `ReadyPerformance`, `StartPerformance` and `DisbandFellowship` (`scenario-feed.md` §6.7); the plain sandbox answers `StartPerformance` with one `PerformanceSyncStart` by the player, 700 ms ahead (§9.8). |
| **Overlay** | P1 (`tessera_net::scenario::OverlayBackend`, [0025](decisions/0025-p1-character-look-on-test-server.md) D1, `scenario-feed.md` §12.5) | Wraps a test-server backend (`OverlayServer`: `RemoteBackend`, or a test double) and runs the scenario feed's Character and Inventory groups for the local player beside it; `tessera-client --scenario FILE --server CONNECT`. The file is checked against the overlay subset before anything connects. Inventory-group commands go to the feed, every other command to the server backend; each poll returns the server's events first, then the feed's. The feed's clock starts at the poll that delivers the server's `WorldEntered`. The player's worn look stays the server's: when the feed's equipment list changes and the server advertised `APPEARANCE_CHANGE`, the overlay sends `AppearanceSet` with the server's last appearance and the feed's equipment (at most one in flight, at least 250 ms apart, changes in between coalesced, 10 s answer timeout), and the server's `EntityAppearance` answer reaches the player and every observer as `AppearanceChanged`; without the capability it emits `AppearanceChanged` locally only. Nothing else of the feed leaves the client: bags, quantities, wallet, levels and stats are never sent. `Backend::as_any` exposes the overlay; `Session::remote_backend` finds the wrapped `RemoteBackend` either way. Lives in the client's process: `tessera-server` links neither it nor the feed (§23.5 unchanged). |
| **Test server** | M7 (`tessera_net::remote::RemoteBackend`, feature `remote`, on by default) | `tessera-server` + `tessera-protocol`: a protocol designed and documented by this project (spec `docs/specs/tessera-protocol.md`, transport per [0030](decisions/0030-tessera-protocol-transport.md)), with its own handshake (QUIC v1 + TLS 1.3, the server's own Ed25519 key pinned by SHA-256 fingerprint or recorded in the client's `known_servers` file), its own encryption (standard, documented algorithms) and its own content: original test content, or the server operator's own data limited to what LEGAL.md allows. See below. |
| **Compatibility** | M8 (in scope, [decision 0034](decisions/0034-official-server-compatibility.md)); not built yet | See §19.3. |

The test-server backend implements the same `Backend` trait, so `Session` and everything
above it work unchanged:

- **Configuration** (`RemoteConfig`): a connect string `tessera://host:port[?pin=sha256:…]`,
  the trust policy (known-servers file in Tessera's own settings directory, never the game
  install, nor below one; trust on first use only when explicitly enabled, and never when
  the file exists but cannot be read completely), the account name and a password
  supplied by the caller (never logged, sent only after the server key was verified), the
  `Hello` parameters, statics streaming and interpolation parameters.
- **Events** follow the protocol spec's Appendix A: `LoginOk` → `Connected`; `WorldEnter` →
  `WorldEntered`, `AppearanceChanged`, `LocationReport`; entities → `EntityCreated` (with
  appearance), `EntityMoved` (with velocity), `AppearanceChanged`, `EntityDespawned`; chat,
  notices and refusals → `ChatMessage`; every way a connection ends → exactly one
  `Disconnected` with a reason (`RemoteBackend::failure` gives the detail: key mismatch,
  login code, server code, violation). Everything from the network is decoded strictly and
  checked by the protocol's client state machine first (`tessera-protocol.md` §13).
- **Protocol 1.1 and characters** (P1, decision 0025 D2–D4): the backend negotiates minor 1
  (the minimum stays 0). At minor 1 looks travel with `CharacterCreate`, `LoginOk`,
  `CharacterList`, `WorldEnter`, `EntityCreate` and `EntityAppearance`, and `Equipped`
  carries its slot; at minor 0 neither is sent and received appearances carry none. After
  `Connected` the backend emits one `CharacterInfo { name, body, look }` per `LoginOk`
  entry. `CreateCharacter` becomes `CharacterCreate` (class and nationality dropped) when
  the server advertised `CHARACTER_CREATION` and no creation is outstanding
  (`character-screens.md` §7.1.1); the answering `CharacterList` becomes `CharacterInfo`
  for the new name (matched by its NFC form, as the server stores names) and
  `CharacterCreateFailed` becomes `CharacterRefused` with Tessera's own text per code. A
  creation the session cannot send, or one still unanswered when the connection ends, is
  answered locally with `CharacterRefused` (server error), so the screen's outstanding
  creation clears. `DeleteCharacter` and every other new command stay refused.
- **Protocol 1.2 and named chat channels** (P4, [decision 0035](decisions/0035-p4-social-on-test-server.md)
  D1, `scenario-feed.md` §9.8): the backend negotiates minor 2. With the server's
  `CHAT_CHANNELS` (`RemoteBackend::has_chat_channels`), `JoinChannel` sends `ChannelJoin`,
  `LeaveChannel` sends `ChannelLeave` and `Chat` on `Named` sends `ChatSend` with channel 6;
  `ChannelJoined` and `ChannelLeft` from the server surface with the server's spelling of the
  name, a `ChatMessage` on channel 6 as `ChatChannel::Named`, and `ChannelRefused` as a System
  line in Tessera's words. Below minor 2 or without the capability the three are refused with
  `NetError::InvalidCommand`, and the client's `join` says user chat is unavailable (CT11).
  Tells are unchanged. Fellowship chat, the fellowship, trade, list and performance commands
  stay refused at every minor (decision 0035 D2): the server learns nothing beyond who joined
  which channel name and the chat it relays (LEGAL.md §11.1 rule 5).
- **Statics are not replicated.** As in the sandbox, the client may hand the backend its own
  `WorldSource`; the backend streams the statics of the player's neighbourhood from the
  user's install with the sandbox's streaming code (`StaticSpawned`, `StaticsCleared`).
- **Movement**: `Command::Move` updates a single "newest move" slot; the network thread sends
  `MoveInput` by the client send rules (15 Hz moving, 1 Hz heartbeat, immediate on gait
  changes, teleports and corrections) and never sends a move the client made before it saw
  a teleport or correction.
- **Remote entities** are interpolated by `tessera_net::interp` (snapshot buffers on server
  timestamps, render time = estimated server time − *D*, bounded extrapolation, snap across
  teleports; *D* and the kept history grow together to the server's snapshot rate). `RemoteBackend::sample` gives an entity's state at the current render time;
  `RemoteBackend::stats` gives RTT, clock offset, traffic and drop counters for the HUD and
  the load tools. Replicated properties (stance, emote, speeds) are read with
  `RemoteBackend::entity_state`; commands beyond the neutral `Command` set (emote, stance,
  appearance) go through `RemoteBackend::handle`.
- A network-free `InboundHarness` runs the same driver and mapper from a script of server
  bytes; it backs the `net_remote_*` fuzz targets.

**P2 on the test-server and overlay backends** ([decision 0032](decisions/0032-p2-targeting-on-test-server.md),
`scenario-feed.md` §9.7, §12.5, revision 6):

- **Local selection (test server).** `submit` answers `SelectTarget` in `InWorld` itself; it
  becomes no network work (`Routed::Local`), so the server receives nothing and keeps nothing
  (LEGAL.md §11.1 rule 5). A target is known when it is none, the player, a remote entity
  created and not removed, or a static the backend streamed and has not cleared, judged by
  the events already handed to the session. For a known target the mapper keeps one pending
  `TargetChanged { entity: player, target }`, which a later selection before the next poll
  replaces, and `poll_into` delivers it first, before the events mapped in that poll; an
  unknown target gives `Ok`, no event and a rate-limited debug line. The backend keeps no
  selection and never emits `TargetChanged` on its own. What a test actor's target frame shows
  follows from the events the server sends: the name and kind from `EntityCreated`, the figure
  from its appearance, and no level, vitals, effects or casts, because none ever arrive
  (decision 0032 D2).
- **Overlay.** `SelectTarget` is not an Inventory-group command, so the overlay routes it to
  the wrapped test-server backend, which answers it as above; the feed never sees it. The
  overlay subset admits no Combat-group step, trigger or input form (`target`, `vital`,
  `combat`, `cast`, `effect`, `damage`, `defeat`, … ; `scenario-feed.md` §12.5.1): such a file
  is refused with `NotAllowedInOverlay` (exit 3) before anything connects.
- **Unchanged:** `tessera-protocol` stays at 1.1 (no new message), `tessera-server` and the
  server dependency check (§23.5) do not change, and the sandbox's feed answers
  `SelectTarget` as before (`scenario-feed.md` §7.6).

<a id="net-compat"></a>
### 19.3 The compatibility tier (M8)

The owner's goal is compatibility with the official service. `docs/LEGAL.md` §8 rates
official-server connectivity as tier **T4**, the highest-risk tier. Until 2026-10-11, §9.3
gated it behind a written legal review and permission from the rights holders. On
2026-10-11 the owner put it in scope without either
([decision 0034](decisions/0034-official-server-compatibility.md)). Conditions G4 (a
ban-risk warning before any official connection) and G5 (the permanently forbidden list of
LEGAL §8) stay binding.

Architecturally this means:

- Today `tessera-net` contains the `Backend` trait, the domain-event vocabulary, the sandbox
  backend and (from M7) the test-server backend. The trait and the event vocabulary are
  designed so that another backend can be added without changes above `tessera-net`.
- The compatibility backend (M8) implements `Backend` in **its own crate**, so it can be
  withdrawn without restructuring the client (LEGAL §9.4). The M8 plan decides where it sits
  in the workspace and how it is built.
- **CI.** CI never connects to an official server. The backend's tests use synthetic
  fixtures.
- **G4.** The client makes no official connection until the user has accepted the G4
  warning in the client's own interface.
- **No secrets in the repository.** No official hostnames, ports, endpoints, keys,
  certificates or account data appear anywhere in the repository at any milestone. The
  backend loads endpoints and key material at runtime from the user's install, as its specs
  describe. Numeric constants of the protocol follow decision 0012 (as amended by 0034).
- **Extended scripts (M9).** On an official-server session, extended scripts get a backend
  class of their own. They run there only after the user's explicit opt-in. Automation
  stays offline only (LEGAL §9.5).
- **Decision 0017 §6.3.** Its build-level exclusion of `tessera-ext` and a compatibility
  crate is withdrawn by decision 0034 §4.

<a id="net-threading"></a>
### 19.4 Network threading

Socket I/O runs on a dedicated network thread inside `tessera-net`
([decision 0030](decisions/0030-tessera-protocol-transport.md)): one thread per process,
started by the first `Command::Connect` and kept for the life of the process, runs a tokio
`current_thread` runtime and one quinn client endpoint per address family. Each
test-server backend is a task on it with its own connection, the protocol's client state
machine and a 30 Hz tick (timeouts, time sync, ping, the movement schedule). (A thread,
runtime and endpoint per session, as first built, left memory behind in the allocator at
every reconnection; see `docs/perf/m7-soak.md`.) The main thread and the network thread share only:

| Direction | Channel | Bound | When full |
|---|---|---|---|
| Main → network | commands (enter world, teleport, chat, emote, ...) | 256 | `submit` returns `NetError::SendQueueFull`; never blocks |
| Main → network | newest player move (a `watch` slot) | 1 | Replaced; moves can never fill a queue |
| Main → network | shutdown flag (`Disconnect`, or the backend dropped) | 1 | — |
| Network → main | decoded, validated items | 1,024 | The network thread stops reading the world, chat and control streams (QUIC flow control then holds the server back) and drops movement; reliable state is never lost. Reading stops once 256 items wait, but one read (8 KiB) that started below that is decoded in full, so at the peak about 1,620 items (256 + 8,192 / 6, the smallest world frame being 6 bytes) wait on the network thread |
| Network → main | statistics snapshot (mutex, `try_lock` on the main thread) | 1 | The main thread keeps its previous copy |

`Session::poll_into` drains the channel without waiting and maps items to `Event`s on the
main thread, where the entity table, interpolation buffers and statics streaming live (the
`WorldSource` is the client's and stays on the main thread). Dropping the backend asks the
session task to say `Disconnect` and end; nothing waits for it. The sandbox backend runs
inline on the main thread, as before.

---

<a id="frame"></a>
## 20. Client application and the path from install to frame

<a id="frame-startup"></a>
### 20.1 Start-up

1. Initialise `tracing` (env filter from `RUST_LOG`), parse CLI options.
2. Load Tessera settings (§17).
3. Locate and validate the install (§4.1). If none is found, run in **no-data mode**:
   original test scene, explorer-style message pointing at `TESSERA_GAME_DIR`.
   With `--scenario FILE` the client now reads, parses and resolves the scenario
   (`scenario-feed.md` §3.2, §12), synchronously on the main thread, before the walker's
   own start-up (steps 4–7) and before a window exists, so no window freezes and there is
   no loading screen for it. Selectors resolve through `tessera-gamedata` (an empty resolver in no-data
   mode); a file without selectors resolves at once, broad selectors against an install
   can take seconds (the record-family index is built lazily on first use). Parse and
   resolve errors end the run here with the codes of `scenario-feed.md` §12.2, windowed
   or headless. If resolution ever moves after window creation, it runs on a worker behind
   the loading screen. With `--server` as well (an overlay run, P1, `scenario-feed.md`
   §12.5) the parsed file is also checked against the overlay subset here
   (`NotAllowedInOverlay`, exit 3), before anything connects.
4. `ArchiveSet::open` (§5), log the archive table at `info`.
5. Start the asset server and worker pool (§8).
6. Load bootstrap records on workers: property registry, root DID map, string tables
   for the UI locale. The client shows a loading screen meanwhile. From P1, when the
   walker runs, the character facts (`character::CharacterHub`: bag layout, paperdoll
   table, slot rules, the tooltip engine, per-record item facts and, with
   `--character-select`, the screens' data from `tessera-gamedata::chargen`) load on a
   thread of their own once the archive set is open; the frame loop takes them when they
   are ready (step 2 of §20.3), and items that arrived before them are re-read then.
7. Create the window and renderer (§13); create the audio device (§14).
8. Connect a backend (§19): sandbox by default. With `--scenario FILE` the sandbox gets
   the scenario resolved in step 3 (`SandboxConfig::scenario`). `--headless --scenario
   FILE` runs the scenario's headless driver on its virtual clock to the scenario's end
   (without `--screenshot` and without a spawn that needs a lookup, always without an
   install: the driver only, no walker, window or GPU; `scenario-feed.md` §12.4 OPEN (2)) and exits with the codes of `scenario-feed.md` §12.2.
   With `--load-plugin` as well, the driver's run carries the plugin host
   (`scenario::plugins`): the plugins load at world entry, and after each driver poll the
   events go to the player's state, the plugin inventory views and the host's step-7 work,
   as one frame would deliver them. With `--server` the backend is the overlay over the
   test-server backend (`World::overlay`, §19.2); a headless overlay run paces one scenario
   tick per frame on the wall clock and ends when the feed ends (exit 0), fails or loses the
   server (exit 5), or after an hour (exit 5).
   With `--character-select` (P1, `pre_world`) the world session is created with manual
   entry and external polling: it does not enter the first character on `Connected`, and
   nothing polls it until the character screens open. They open as soon as the game UI, the
   world session and the screens' data exist; if one of them can never exist (the facts or
   the creation layout cannot be read), the session goes back to entering the first
   character and a system line says why.
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

1. Pump window and input events; the mapper resolves them and offers them to the UI in
   order (UI input routing and plugin input handlers run here, against the elements'
   current geometry; events that arrive between redraws are handled in winit's callbacks
   on arrival and belong to this step of the next frame); offer pending Ends; publish the
   action snapshot ([0014](decisions/0014-ui-input-bridge.md) D8).
2. `Session::poll`: apply server events to the entity store (entity ID → transform,
   property set, render/audio components). A windowed scenario run with `--scenario-log`
   also formats the polled events for the log here and hands them to the log's writer
   thread (§21); with `--scenario-input` it sends the input lines due before the poll.
   Every event also reaches the local player's state (P1, `character::PlayerState`:
   inventory model, scripted properties, header, vitals, wallet, worn look); its changes of
   the frame go to the plugin inventory views as queued gameplay events (delivered in step
   7) and are kept for the character panels. While the character screens are open the
   world session does not poll here: the screens poll it after step 7 (below) and hand the
   events back to it, so the entity table, chat and player state stay current.
   From P4 every event also reaches the social state (§19.1, "The Social group in the
   client"): the chat lines leave it in their P4 forms, the panels' models change, and the
   performance events and the fellowship's member list are handed to the player music and
   the plugins' party views; the player music's requests (`ReadyPerformance`,
   `StartPerformance`) go to the session here.
3. Simulation at fixed rates, interpolated for rendering (Decision, see "Simulation steps"
   below): the character controller (`tessera-physics`) on its own 1/60 s step; the other
   local simulation (free-fly controller, camera) on the 30 Hz tick.
4. Streamer: update desired landblocks and priorities.
5. Drain asset completions within the upload budget; create GPU resources; install and
   remove collision sets ("Collision-set residency" below).
6. Animation: collect pose results computed on workers for the previous frame; submit new
   sampling jobs.
7. UI tick (`lua-plugin-api.md` §3.3): hover refresh, `MouseHover`, the plugins' `Update`
   broadcast (plugin timers are `Update` handlers), the rest of the UI's per-frame work;
   then the plugin host's deferred work, sliced by whole items (2 ms per frame; it
   includes the plugin events raised by game and session state and the logout unload);
   then the collector step in every apartment (`plugin-runtime.md` §9.2); then apply the
   UI's band changes to the mapper. UI changes driven by game or session code (show and
   hide, activation, focus, screen resize, `HideWhenField`) are applied in this step, so
   the plugin notifications they cause run here (0014 D8).
   Then, from P1, the character panels' frame: the panel commands the UI declined in step 1
   (`ToggleJournalPanel`, `QUICKSLOT_BACKPACK…`, `TOOLTIP_DETACH`), panels asked for with
   `--open-panel` once the player is in the world, this frame's UI events (examination
   hovers answered with tooltips built by `tessera-gamedata`, drops, activations, the
   paperdoll's turning) and the player-state changes of step 2. Item interactions become
   session commands sent here; the slots change only when the answering events arrive in a
   later step 2. Last, with `--character-select`, the character screens' frame
   (`PreWorld::frame`): the UI's events become the screens' actions, the screens poll the
   session and send their commands, the world session takes the polled events, and the
   chargen layouts follow the screens' model. On `WorldEntered` the screens close and step
   2 polls again; their quit (Escape on the selection screen) ends the client. A creation
   whose look the session cannot carry is saved to Tessera's settings on a worker thread
   (a locked read-modify-write), never on the frame.
   From P4, after the character panels, the social panels' frame (`social::SocialUi`): the
   social input commands the UI declined in step 1 (`ToggleSocialPanel`, the five page
   commands, `SelectFellowOne` … `Six`, `AssistFellowTwo` … `Six`), panels asked for with
   `--open-panel social|friends|fellowship|trade`, and this frame's UI events (rows, buttons,
   the invitation's answers, the trade window's button, coin entries and drops, item
   tooltips of the trade lists) against the social state; the resulting session commands
   are sent here, and every panel is refreshed when the state changed. The chat window takes
   the tell targets (CT3), the instant-message lines of this frame's tells (CT6, its tab
   created once the window is open when `GOP_FullIRCMode` is set) and the panels' lines.
   Lines typed in the chat window run through the one command dispatcher (decision 0020) with
   the world session, so player music, `--server` chat, tells and channel commands work there
   as in the walker's overlay and `--command`; the chat window keeps no registry of its
   own.
8. UI: layout, paint damaged rectangles within the UI slice, build the composition list.
   Layout here changes no box that a hit test reads (0014 D8; paint and composition:
   [0015](decisions/0015-ui-rendering.md)).
9. Audio: update listener and emitters; send commands to the audio thread.
10. Render: cull, sort, encode the world passes, then the screen-space passes (walker
    overlays, the UI pass that draws the composition list, developer overlay, software
    cursor; §13), submit, present. From P1, before the frame, the paperdoll's portrait
    (the player's figure at the paperdoll's turn) is rendered into an offscreen target of
    its viewport's size, which the UI pass composes as external content
    (`render.md` §6.10). The character screens' creation viewport is drawn the same way
    with the creation figure (P1 re-check; the UI scene's objects and lights are not
    drawn, `character-screens.md` §2.1 OPEN).

<a id="frame-combat"></a>
**Combat in the frame** (P2, decision 0024; client half only). The combat state lives on the
main thread in the world session (`net::World`); only data loads run elsewhere. Every value
shown comes from session events; nothing is computed from game rules (decision 0024 §2.4).

- **Start-up.** When the P1 character facts arrive (§20.1 step 6), `Runtime::tick` builds the
  combat panels' data over them (`combat::CombatHub`, `CombatInstall`: `tessera-gamedata`'s
  `CombatData` and the tooltip engine, which read records on demand). The UI boot resolves the
  combat layouts and element labels with the other UI data (`combat::boot`). The walker's
  combat layer opens the combat-visual data on an asset worker at walker start-up; until it is
  ready its session events wait in a bounded queue (4,096, oldest dropped), and without an
  install nothing plays.
- **Step 2.** `World::update` polls the session. Each event goes first to the combat state
  (`CombatState::apply`, at the combat clock: seconds since the session was created), then to
  the P1 player state. The combat state's changes queue for the HUD (bounded; on overflow the
  HUD rebuilds from the state). The events that matter to the plugin combat views queue too;
  right after `World::update` the runtime hands them to `CombatHub::after_events` at the
  plugins' game time, which queues `CombatViewEvent`s for step 7. While the walker captures
  entity events, Combat-group events also queue for the walker's combat visuals (at most
  65,536; further ones are dropped and counted).
- **Steps 3–6 (walker update).** After the characters (figures, animation) and the particle
  effects, the walker drains at most 1,024 combat events per frame into its combat layer
  (`walker::combat::CombatLayer::handle`) and runs `CombatLayer::update`: clips the combat
  scripts started or stopped go to the character layer, session figures on the GPU become
  script owners in the effects runtime (their lasting combat state, stance and the selection
  effect then shown on them), queued events are handled in order (`combat-visuals.md` §9.6),
  then the story hooks the script runtime reported, then stories that ended or timed out.
- **Step 7.** After the character panels' frame, the combat frame
  (`ui_host::client::combat_frame`): widgets are rebuilt when the install's combat data has
  just arrived; the step-2 changes reach the widgets; world clicks the UI declined in step 1
  (a select tap, a right click, a double click) are picked through the walker's figure boxes
  (`Walker::pick_entity`, `combat-ui.md` TR1, TR3); the targeting and quickslot actions the UI
  declined, the widgets' actions and the picks become session commands (`SelectTarget`,
  `UseEntity`, `UseSkill`, `UseItem`, `CancelCast`, `SetQuickslot`) sent here, and nothing on screen changes
  until their answering events arrive in a later step 2 (CUI8). A refused quickslot use flashes
  its slot. A selection the client cleared locally (decision 0032 D2) also stops the walker's
  selection effect. Floating texts, the name block and the selection indicator are anchored
  above the walker's figures as drawn this frame (`Walker::entity_anchor`, `combat-ui.md` §6.4). Before the
  plugin host's step-7 work, a `menu` press the UI declined goes to the combat widgets first:
  when no on-demand panel is open (`game-panels.md` §16 step 1), it cancels a
  cancellable cast or clears the selection, and only otherwise opens the main menu
  (`combat-ui.md` TR5). The plugin combat view events of step 2 are delivered in the host's step-7
  work.
- **Steps 8 and 10.** The widgets paint and compose like any panel (§15.1). The target
  frame's portraits stay empty (`combat-ui.md` §16.1 OPEN).
- **Headless runs.** A driver-only `--headless --scenario` run has no walker and no UI; with
  `--load-plugin` each driver poll's events also feed the plugin combat views
  (`scenario::plugins`). The headless combat screens of the P2 tests
  (`combat::headless::CombatScreens`) drive a UI host without plugins on the scenario's
  virtual clock, with the synthetic camera of `combat-ui.md` §13.6. A headless walker run
  with `--scenario … --screenshot` hands the walker the entity events and the combat events
  of each driver poll (`scenario::screenshot::DriverView`, bounded queue), so combat visuals
  play there as in a windowed run; `apps/client/tests/combat_visuals_real_data.rs` uses
  that path.

At client exit, after the last frame, the plugin host unloads every apartment
(`lua-plugin-api.md` §6.3); this is the only plugin code outside steps 1 and 7. The frame
clock behind the plugin API's game-time query (`lua-plugin-api.md` §12) and the UI's hover
clock advance once per frame, right after present, so step-1 handlers run in winit
callbacks and the step-7 `Update` broadcast of the same frame see the same value. The UI's
upload budget (0015 §6, U3) is separate from step 5's asset upload budget.

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
| **Audio** (owned by the audio backend) | Mixer state, voices, streams | Lock-free command queue from main; event ring back to main (finished voices are dropped there) |
| **Audio feeder** (`tessera-audio`, device and null backends) | Stream decoders of playing streamed voices | Job channel from main; wait-free sample rings to the audio thread |
| **Network** (when a socket backend is active) | Sockets, protocol state machine, crypto | Bounded channels to and from main |
| **Index builder** (tools only) | Full DID index build | Result handed to the UI thread |
| **Character facts loader** (client, P1; once per run, when the walker runs) | Builds the character facts and, with `--character-select`, the screens' data from the archive set (`character::CharacterHub`) | Join handle polled by the main thread each frame (§20.1 step 6) |
| **Creation-choices writer** (client, P1; one per save) | A locked read-modify-write of Tessera's settings file with the creation choices the session cannot carry (`screens`) | Join handle; `CharacterScreens::wait_saved` waits for it |

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
  a frame, never a stall. Stated exceptions: (a)–(c) are needed because the plugin API's
  calls are synchronous and must complete or fail before they return:
  - **(a)** the header probe of `SetBackground(path)`: one synchronous read of at most
    64 bytes, cached per path ([0015](decisions/0015-ui-rendering.md) §4.4). The
    `Plugins/` index walk runs on a worker and is not an exception;
  - **(b)** the synchronous `PluginData.Save` writes and the flush when a Lua state closes
    (`lua-plugin-api.md` §6.3, §10.6): an unload asked for by the user or a plugin in step 1
    or 7, the logout unload in step 7, and the unload at client exit after the last frame;
  - **(c)** the reads of plugin source files from the `Plugins/` tree by `import`
    (`lua-plugin-api.md` §7.2), including the imports of a plugin's load item in step 7.
    This is the precedent that 0015 §4.4 cites for (a) ("of the same kind that `import`
    already needs"); that wording stands, and its reference for it is this exception, not
    §18.2;
  - the `--scenario-log` of a windowed scenario run is not an exception: frame-loop step 2
    only formats each polled event and accepted command into memory, and a writer thread
    (`apps/client/src/scenario/live_log.rs`) does the I/O behind a bounded channel (256
    chunks; a chunk the full channel refuses stays with the frame and goes with the next
    one). The frame never waits: when 16 MiB are formatted but not yet written (a
    standard-output reader that stopped reading), the log drops the lines from there on and
    ends with `reason=error error=LogWrite` (exit 5). The headless driver draws no frames,
    so its writes delay no frame.

  > **OPEN (P1, client):** the bag windows' column settings (`character-panels.md` §4.1)
  > are saved on the main thread, in the character panels' frame after step 7, when a
  > resize changed them (`character::settings::save_columns`: a synchronous
  > read-modify-write of Tessera's settings file). It is rare, user-triggered I/O, but it is
  > neither listed as an exception above nor moved to a worker like the creation choices;
  > either move it to a worker or list it as exception (d).
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
- **Deterministic scenario tests** (P0, [`scenario-feed.md`](specs/scenario-feed.md) §10,
  §14): the headless driver (`tessera_net::scenario::HeadlessDriver`) plays a scenario on a
  virtual clock over the plain sandbox, without a window, GPU or install, and writes a
  canonical event log that is the same, byte for byte, on every run. The net crate's tests run
  the spec's conformance vectors and hostile inputs this way. The committed scenarios
  (original, synthetic, selectors only) and their `install none` logs live in
  `apps/client/tests/scenarios/`; a client test plays each one with
  `tessera-client --headless --scenario FILE --no-data`, compares the log with the committed
  one, and rejects record IDs and numeric enum values in them (`scenario-feed.md` §13).
  Later milestones use the same driver for their panels' golden images and the explorer's
  sheet mode.
- **P1 synthetic tests** (no install, no GPU): `tessera-gamedata` `tooltip_model` (the
  tooltip model over an in-memory `TooltipData` with invented layouts, records and texts)
  and `chargen_load` (a synthetic install with a duplicate, a missing and a rejected race
  table); `tessera-ui` `game_examine` and `game_items` (tooltip rows, placement,
  comparison, pins; item slots, drag and drop, viewports, the paperdoll's turning, the
  creation controls, on synthetic trees with the vocabulary's placeholder IDs);
  `tessera-plugins` `inventory_views` (the `Backpack`, `Equipment`, `Item` and `ItemInfo`
  views against `lua-plugin-api.md` §15.1 and its checks 66–69, with a synthetic source);
  `tessera-protocol`'s 1.1 vectors and negative vectors (`tessera-protocol.md` §16.8);
  `tessera-net` `scenario_p1` (the overlay cases of `scenario-feed.md` §14.9 against a test
  double of the server backend, the plain-sandbox creation cases of §14.10, property values
  and looks) and `remote_creation` (creation under `character-screens.md` §7.1.1, character
  lists with looks, minor-0 fall-back, against a scripted peer); `tessera-client`
  `character_screens` (the screen model over real sessions: plain sandbox, the feed's
  triggers, refusal, removal and time-out, the per-backend button rules),
  `character_screens_ui` (the chargen layouts' states and clicks, synthetic layouts) and
  `plugin_inventory_checks` (decision 0024 P1 criterion 4: checks 66–69 through the
  client's plugin host on headless scenario runs); the committed scenarios `chargen`,
  `inventory-panels` and `backpack-growth` with their logs; `tessera-loadtest`
  `v11_looks` (an in-process `tessera-server`: looks stored and relayed, held-item slots,
  minor-0 observers, the hostile 1.1 vectors, and an overlay run whose equipment another
  client sees; skipped only where loopback UDP is unavailable).
- **P4 synthetic tests** (no install, no GPU): `tessera-client` `social_chat_server`
  (criterion 2: an in-process `tessera-server` at protocol minor 2, two clients whose lines
  go in as the chat window's entry hands them to the command dispatcher: a tell, its reply,
  a tell in the `Tell` mode, two joins of one named channel, a line on it, a part, and a line
  the parted client no longer hears; skipped only where loopback UDP is unavailable) and
  `social_music_sync` (criterion 3: the committed `music-sync` scenario on its virtual clock
  with the player music on the offline engine and a synthetic instrument table; every
  performance of both synchronised starts has its time-0, measured from the instruments'
  schedule log, within 0.0001 s of *S*, `instruments.md` §8.9; and the player's
  `/play … sync` and `/playstart` through the dispatcher and the plain sandbox, one start);
  the client's unit tests of the social state, the dispatcher's P4 built-ins, the panels'
  command and name rules and the party bridge; the committed scenarios `fellowship`,
  `trade`, `friends` and `music-sync` with their logs.

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
- Scenario runs with an install resolve selectors against it through `tessera-gamedata`.
  Their logs list record IDs chosen from the install and may hold install text, so they are
  compared within the run and never committed (`scenario-feed.md` §10.5).
- **P1** (decision 0024 §5 P1), counts and identifiers only: `tessera-gamedata`
  `tooltip_real_data` (the hover tooltip model of every item record, each sorted into one
  category of `item-tooltips.md` §12; IV1–IV5), `chargen_real_data` (`character-screens.md`
  §9 steps 1–3, CSI2–CSI5; each race and gender's controls and default figure, with
  `tessera-mesh` as a dev-dependency) and `inventory_real_data` (CPI1–CPI3); `tessera-ui`
  `game_p1_real_data` (the tooltip elements, item-slot template, paperdoll and bag layouts
  by label); `tessera-client` `item_tooltips_real_data` (criterion 2: every item record's
  tooltip modelled, converted and laid out headless in the UI's tooltip panel, ending in
  exactly one category), `character_screens_real_data` (criterion 1: every race, gender
  and class offered and accepted, each (race, gender)'s controls and default figure
  through the walker's figure path, random names, a headless creation and world entry per
  (race, gender), and on an in-process test server one creation per race whose look a
  second client sees), `pre_world_real_data` (`--character-select` start-up without a
  window), `character_panels_real_data` (default keys to panels, the 23 paperdoll slots,
  bag 1 and its column setting, tooltips with comparison and pins, a drop that sends
  `EquipItem`; the panels drawn offscreen with a GPU, pictures only to a private
  directory) and `worn_look_real_data` (criterion 3: the walker's figure changes after a
  scenario equips a chest piece and a weapon; an in-process server relays an overlay
  client's equipment to an observer with the slot). Selectors in these tests use labels
  only (decision 0025 open item 1).
- **P4** (decision 0024 §5 P4): `tessera-ui` `game_social_real_data` (the social
  vocabulary by label, SPI1–SPI4); `tessera-client` `social_panels_real_data` (criterion 1:
  the committed `fellowship`, `trade` and `friends` scenarios played headless against the
  install with the client's social state, player state and game-UI host; at checkpoints the
  rows, names, leader markers, the trade sides with the install's icons and their ready
  states, the friends and ignore rows, and with a GPU the covered share of each panel's
  rectangle: the invitation accepted with a click, the leader change, the removal and the
  disbanding shown, the completed trade closed; pictures only to a private directory).

<a id="testing-golden"></a>
### 23.3 Golden images

- Headless renderer on a software adapter (lavapipe/llvmpipe or another software
  Vulkan/GL implementation) renders synthetic scenes: procedurally generated meshes,
  CC0 or self-generated textures, test terrain heightfields.
- Images are compared with a per-pixel tolerance and a maximum differing-pixel ratio.
  Goldens in the repo come from synthetic scenes only.
- Real-data visual regression (a landblock, a character) is a **local-only** tier: the
  goldens are written to the user's own cache directory and never committed.

**Panels with install content** (parity track, P2, P3 and P6): see
[decision 0031](decisions/0031-parity-panel-goldens.md) (Proposed). Panels should also have
committed goldens from synthetic input only, painted in-process; a criterion that asks for
golden images requires them (decision 0031 §3 step 1). Panels with install layouts, text and
icons can also have local-only goldens under
`${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/<package>/<test binary>/<function>/<install version>/`,
which are refused inside any git work tree and written atomically. A missing golden is
written and reported as "blessed", not "passed"; local goldens are re-blessed with
`TESSERA_GOLDEN_BLESS_LOCAL=1`, never with `TESSERA_BLESS`. Capture is deterministic (virtual
clock, fixed fonts, no running animation, never after a settle timeout). For an exit
verifier, "golden images pass" means: bless once into a fresh golden directory, judge the
blessed images against the spec, then run again on the same directory with
`TESSERA_GOLDEN_REQUIRE=1` and match.

**P5 panels** ([decision 0036](decisions/0036-p5-map-census-and-panel-goldens.md), Proposed):
the same two tiers cover the world map with markers, the travel window and the instance
finder. Tier 1 paints Tessera's fallback windows from literal-ref scenarios with a generated
map image (the map window's fallback waits for its rule, `map-windows.md` §8 OPEN); tier 2
captures the install's layouts, a small rule-chosen sample of map regions (the root without a
marker, being C5; a territory, an area and a dungeon map with theirs), and install destinations
and templates. The map-region census of P5 criterion 1 checks every region by numbers (image
drawn except C1–C4, marker within 1 map pixel except C1–C5), not by goldens; the mounted walks of criterion 2 are checked from the walk and frame logs.

<a id="testing-fuzz"></a>
### 23.4 Fuzzing

- `fuzz/` holds a cargo-fuzz target per parser: header, extent assembly, B-tree node,
  inflate wrapper, each `formats` decoder, Havok reader, XML skin and plugin descriptors,
  protocol framing, and (P0) the scenario feed: `net_scenario_parse` (arbitrary bytes as a
  scenario file through the parser and static checks) and `net_scenario_run` (a scenario plus
  bytes that drive a synthetic selector resolver, then a bounded headless run with every
  limit checked) (`scenario-feed.md` §11.4). P1 adds `net_scenario_overlay` (an
  overlay-checked scenario over a hostile stand-in for the test-server backend; asserts at
  most one `AppearanceSet` in flight, never two within 250 ms, at most 32 items each) and
  `settings_created_characters` (any settings document through the `[[CreatedCharacters]]`
  reader: bounds kept, stored entries read back unchanged); the protocol, scenario and
  server targets cover the 1.1 forms and the new scenario lines.
- Seed corpora come from the synthetic builders. Corpora derived from real data stay in
  ignored local directories (`fuzz/corpus`, `fuzz/artifacts` are git-ignored).
- CI runs a short fuzz smoke per target; longer runs happen locally.

<a id="testing-ci"></a>
### 23.5 CI gates

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, fuzz smoke, licence and advisory checks on dependencies
(GPL-3.0-compatible licences only), SPDX header check, and a content guard that rejects
`.dat`/`.datx` files, large binary blobs and known media containers in the tree.

**Server dependency check** (P0, `tools/check-server-deps.sh`, the `server-deps` job of CI and of
`tools/ci-local.sh`): LEGAL.md §11.1 rules 1 and 5 and decision 0024 §2.3 as a
gate. It reads `tessera-server`'s dependency graph with `cargo tree` (no build) and fails when
it holds `tessera-gamedata`, any crate that decodes or reads game data, or the scenario feed.
It works from an allow list, so a crate added later cannot slip in unnoticed: in
`tessera-server`'s graph only `tessera-server`, `tessera-protocol` and crates.io packages may
appear (`tessera-net` is refused in any form). The crates that run the server in-process
(today `tessera-loadtest`) are checked too, with the allow list `tessera-server`,
`tessera-protocol` and `tessera-net`: they are refused every data-decoding crate and
`tessera-gamedata`, but their `tessera-net` carries the scenario feed in a workspace build
(feature unification), so the guard does not keep hosts feed-free. Whether it should is item
(3) of the OPEN in `scenario-feed.md` §3.1. The script's header lists the edges, features
and targets it checks, the hosts, and its source scan; `--self-test` plants a violation of
each kind in throwaway workspaces and runs first in CI. Where it is stricter than or goes
beyond `scenario-feed.md` §3.1, that section records an OPEN.

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
| `tools/check-server-deps.sh` | Server dependency guard: `tessera-server` links nothing outside crates.io but `tessera-protocol` (no data-decoding crate, `tessera-gamedata` or `tessera-net`, so no scenario feed); its in-process hosts link no data-decoding crate or `tessera-gamedata` but may link `tessera-net`, feed included in a workspace build (§23.5) | P0 |

---

<a id="open"></a>
## 25. Consolidated open questions

| # | Question | Blocks | Owner doc |
|---|---|---|---|
| 1 | Meaning of header kind/subset words; locale selection | M1 | `specs/dat-container.md` |
| 2 | Self-describing routing vs static table | M1 | `specs/dat-container.md` |
| 3 | One gamedata crate vs `props`/`text`/`wstate`: **answered**, they stay separate; the name goes to the presentation data layer `tessera-gamedata` (§3.1, [0024](decisions/0024-parity-track.md) §3) | M1 | ADR 0024 |
| 4 | Normative status of Kaitai schemas | M1 | ADR |
| 5 | Disk cache of derived data (value and legal sign-off) | M3 | ADR + `LEGAL.md` |
| 6 | Height scale and trailing landblock data (terrain types): **answered** (`landblock.md` §3.4–§3.9, `terrain-texturing.md`) | M3 | `specs/landblock.md` |
| 7 | Interiors, portals, PVS | after M3 | `specs/landblock.md` |
| 8 | Entity storage / ECS choice: **decided**, project-owned store ([0008](decisions/0008-entity-storage.md)) | M4 | ADR 0008 |
| 9 | Shader-record classification method | M2 | `specs/material.md` |
| 10 | UI layout grammar, UIData variants, font records: **specified** (`ui-layout.md`, `fonts.md`, Accepted) | M5 | `specs/ui-layout.md`, `specs/fonts.md` |
| 11 | Plugin namespace naming rationale: **recorded** (`LEGAL.md` §17) | M5 | `LEGAL.md` §17 |
| 12 | Original keymap file syntax | M5 | `specs/keymap.md` |
| 13 | Instrument samples and pitch mapping for player music | M6 | `specs/sound.md` |
| 14 | Sky, weather, time of day, water: **answered** (§10; remaining OPENs non-blocking, in the specs) | M4.5 | `specs/environment.md`, `specs/water.md`, `specs/terrain-texturing.md` |
| 15 | Anything about the compatibility tier: **in scope** by [decision 0034](decisions/0034-official-server-compatibility.md) (no gate; G4, G5 binding) | M8 | `LEGAL.md` §9, M8 specs |
