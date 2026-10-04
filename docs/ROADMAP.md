# Tessera roadmap

> Status: founding document, written at milestone M0 (2026-10-03). M0 and M1 exit criteria
> were verified and closed on 2026-10-04 ([M0 status](#m0-status), [M1 status](#m1-status));
> M2 is next. Milestones are ordered by dependency and by legal risk: each one builds on the
> previous one, and the risk tier (T0–T4, see `docs/LEGAL.md` §8) only rises when the
> earlier, safer tiers are solid.

<a id="purpose"></a>
## 0. Purpose

This document lists what Tessera builds, in what order, and how we know a milestone is
done. Each milestone lists:

- **Deliverables**: what exists at the end, by crate or app.
- **Specs needed**: the `docs/specs/*.md` files implementers work from. A milestone cannot
  start implementing a format until its spec exists, and cannot exit while its spec has
  blocking `> **OPEN:**` items.
- **RE tasks**: dirty-room work that produces those specs. Methods are limited to what
  `docs/LEGAL.md` allows (§2 below).
- **Exit criteria**: checkable conditions. "Real data" criteria are run locally against
  the owner's install via `TESSERA_GAME_DIR`; their results are recorded as pass/fail and
  counts, never as game content.

The architecture these milestones fill in is in `docs/ARCHITECTURE.md`. Section numbers
like "ARCH §8" refer to it.

<a id="provenance"></a>
### 0.1 Provenance

| Fact this roadmap relies on | Source(s) | Status |
|---|---|---|
| Archive set, header magic, per-archive block sizes, kind and region words | Observed in user install 2026-10-03 (see ARCH §4.2) | Confirmed |
| DID type → archive routing; property, string, enum, WState, landblock, mesh, material, texture, sound, UI layout record families | research/notes/data-semantics.md, research/notes/dat-format.md | Reported |
| Skeletons and animation stored as Havok-era binary data; most clips spline-compressed | data-semantics.md §8 | Reported |
| Plugin system uses Lua 5.1 with apartments and `.plugin` XML descriptors | engine-client-re.md §5 | Reported |
| Player music uses an ABC dialect with documented limits | engine-client-re.md §7 (Maestro test suite, MIT) | Reported |
| No public, working server emulator exists; the official protocol descends from the AC lineage | research/notes/network.md §0 | Reported |
| Risk tiers T0 format library, T1 offline engine, T2 own-server networked, T3 emulated gameplay, T4 official-server connectivity; the T4 gate G1–G5 | `docs/LEGAL.md` §8–§9 (from research/notes/legal.md §7) | Reported |
| Reimplementation practice: per-format CLIs first, gated real-data tests, golden images, fuzzing | research/notes/analogues.md §4 | Reported |

---

<a id="tracks"></a>
## 1. Overview

| Milestone | Theme | Risk tier | Main new crates/apps |
|---|---|---|---|
| **M0** | Base: archives, primitives, install detection, dattool, window + renderer, Lua host skeleton | T0/T1 | `dat`, `formats`, `install`, `render`, `script`, `net` (sandbox stub), `dattool`, `client` |
| **M1** | Asset browser: textures, strings, icons, properties | T0 | `props`, `text`, `assets`, `settings`, `explorer` |
| **M2** | Mesh and material viewer | T0/T1 | `mesh`, `material` |
| **M3** | Terrain and offline landblock world walker | T1 | `world`, `terrain`, `input`, `wstate` |
| **M4** | Characters and animation | T1 | `havok`, `anim` |
| **M5** | UI layouts and skins, Lua plugins | T1 | `ui`, `plugins` |
| **M6** | Audio and player music | T1 | `audio` |
| **M7** | Own test server and own protocol | T2 | `protocol`, `server`, test-server backend |
| **M8** | **Gated**: legal review, then (only if the gate passes) a compatibility layer | T4 (gated) | none before the gate |

M1–M6 depend on M0, and mostly on each other in order, but M5 (UI) and M6 (audio) can run
in parallel with M4 once M3's asset and world plumbing exists.

Cross-cutting work that never stops: specs and their provenance, fuzz targets for every
new parser, CI hygiene, `docs/LEGAL.md` upkeep, and performance budgets (frame time,
memory, upload bandwidth).

---

<a id="method"></a>
## 2. How RE work is done (applies to every milestone)

The clean-room rule in `CLAUDE.md` and the process in `docs/CLEAN_ROOM.md` are binding:
implementers read only `docs/`, never `research/` or third-party LOTRO/AC source. Spec
writers follow the research methods of `docs/CLEAN_ROOM.md` §4, in order of preference.
(Those methods are also numbered M1–M4 there; they are unrelated to the milestone numbers
here.)

<a id="method-allowed"></a>
### 2.1 Methods used by this roadmap

| Method (CLEAN_ROOM.md §4) | Used for | Typical output |
|---|---|---|
| Study of data files in the owner's install, read-only, in place | Every format milestone | Byte layouts, invariants, value ranges, counts |
| Reading published RE work for **facts** (§2.3) | Every format milestone | Provenance rows in specs; no code copied, no transliterated pseudo-code |
| Black-box observation of the unmodified official client: its own UI and settings screens, in-game `/loc`, files it writes, small plugins the owner writes using the official plugin API, frame captures (RenderDoc/apitrace) | M2–M6 behaviour and rendering questions | Behavioural specs (units, ranges, timings, pass structure, blend states) in prose. Captures stay private to the dirty room. |
| Differential checks of `tessera-dattool` against published tools run locally | M0–M4 | "Agrees on N of M records" notes; disagreements become spec questions |

<a id="method-not-default"></a>
### 2.2 Not part of this roadmap

- Disassembly or decompilation of the client binaries. Not authorised by default; any
  exception needs a decision record (CLEAN_ROOM.md §4, method 4).
- Invasive dynamic tools (injection, debuggers, memory reading) against the client; gated
  in `docs/TOOLING.md` §6.
- Observing, capturing or describing the official login and session traffic. That is behind
  the T4 gate (`docs/LEGAL.md` §9.3). M0–M7 never need it, because Tessera's network
  protocol is its own (M7).
- Using any proprietary Turbine/Daybreak library (for example the circulating dat-export
  DLL), even as an oracle.

<a id="method-sources"></a>
### 2.3 Published work we build on (facts only)

The full verified list with licences is in `docs/TOOLING.md` §9. The sources that feed
these milestones most directly:

| Source | Licence (Reported) | What we take from it |
|---|---|---|
| LotroCompanion dat utilities and tools (Damien Morcellet) | Apache-2.0 jar distribution; GitHub repos have no licence file | Container layout, DID routing, record grammars for properties, strings, enums, DID maps, WLib/WState, landblocks, UI layouts, images, sounds |
| Mirarkitty `lotro_extractor` | Apache-2.0 | Property type codes, WLib notes, an independent dat reader as a cross-check |
| Mirarkitty `lotro_mesh_viewer` | MIT (one GPL-3 Havok file) | Mesh vertex strides, skin tails, material/surface/texture chain, appearance grammar, dye rule |
| jtauber/lotro | No licence (all rights reserved) | Cross-checks only: header, B-tree, heightmap, texture header |
| Chorizite DatReaderWriter, ACE, ACViewer, WorldBuilder (Asheron's Call lineage) | MIT / AGPL / GPL / none | Turbine-lineage concepts: dat header, B-tree, landblock/cell model, testing approach |
| exyorha/hkxparse | MIT | Havok tagfile/packfile structure facts |
| digero/Maestro | MIT | Player-music ABC dialect behaviour |
| lunarwtr API docs / VS Code extension dump | MIT (extension) | Plugin API names, signatures, enums, events |
| JuneStepp/OneLauncher | GPL-3.0 | Install locations on Linux/Wine, settings-folder layout |
| Official public material (plugin Lua docs, skinning docs, ABC usage) | SSG | Behaviour to implement; prose is never copied |

No code from any of these is copied, whatever its licence (CLAUDE.md).

<a id="method-tools"></a>
### 2.4 Tools per milestone

`docs/TOOLING.md` is the authoritative tool list (purpose, zone, licence, install hint,
priority). In short:

| Milestone | Clean room (implementation) | Dirty room (specs) |
|---|---|---|
| M0 | Rust stable via rustup, clippy, rustfmt, cargo-nextest, cargo-fuzz (nightly, fuzzing only), cargo-deny, reuse | hexdump/xxd, Python probes, ImHex, Kaitai Struct |
| M1 | egui for the explorer, insta for metadata snapshots | `tessera-dattool` as the main probe |
| M2 | Mesa lavapipe/llvmpipe for headless golden images, RenderDoc on Tessera's own output | Wine/Proton + DXVK to run the original client; RenderDoc/apitrace frame captures |
| M3–M6 | Tracy (profiling), audio test harness | Same as M2, plus the owner's own small plugins for API observation |
| M7 | Network load/soak tools for our own protocol, Wireshark on our own traffic | None beyond behavioural observation |

---

<a id="m0"></a>
## M0: Base

**Goal:** a workspace that can open every archive in a real install read-only, find any
record by DID, decode the first record families, show a window with a rendered test scene,
and run a sandboxed Lua script.

<a id="m0-deliverables"></a>
**Deliverables**

| Crate/app | Content |
|---|---|
| `tessera-dat` | Header, modern and legacy extents, B-tree lookup and iteration, zlib inflate with caps, `ArchiveSet` with classification by header, DID routing table, `Stale` detection. Test-only archive builder (`testkit`). |
| `tessera-formats` | Bounded `Reader` with primitives; string-table, render-surface (texture header + payload) and property-dictionary decoders. |
| `tessera-install` | Detection order (env, CLI, Steam libraries, Wine/Proton prefixes), validation, version identification. |
| `tessera-render` | wgpu device/surface setup, a test scene (procedural mesh and texture), headless offscreen mode. |
| `tessera-script` | Lua 5.1 state with the sandbox allow-list, memory and instruction limits, a minimal binding example. |
| `tessera-net` | `Backend`/`Session` traits, event and command enums (first variants), sandbox backend stub. |
| `tessera-dattool` | `info`, `ls`, `stat`, `cat`, `strings`, `extract` (outside the install only), `validate`. |
| `tessera-client` | Window, frame loop skeleton, install detection log, test scene; runs with or without game data. |
| Docs | `README.md`, `docs/ARCHITECTURE.md`, this roadmap, `docs/LEGAL.md`, first specs. |
| CI | fmt, clippy `-D warnings`, tests, SPDX check, content guard. |

<a id="m0-specs"></a>
**Specs needed**

- `docs/specs/dat-container.md`: header, extents (modern and legacy), B-tree node and
  entry, compression, archive set and kinds, routing table.
- `docs/specs/primitives.md`: integer encodings, sized lists, string encodings, name hash.
- `docs/specs/string-table.md`: string-table records, variable slots (grammar tags can be
  a later section).
- `docs/specs/render-surface.md`: texture/icon record header, pixel formats, JPEG payload.
- `docs/specs/property-system.md`: property dictionary (names, types, definitions); the
  property-set section can follow in M1.
- `docs/specs/install-detection.md`: search order, Steam library parsing, prefixes,
  validation, version identification.

<a id="m0-re"></a>
**RE tasks**

- Confirm the header fields against all archives in the install: kind word, subset word,
  version words, the root-extent-size word (observed to equal 2460 for some archives and
  the block size for others), free-list fields.
- Walk every B-tree with a probe and record invariants: key ordering, entry counts per
  node, child-pointer rules, flags values seen (2 = stored, 3 = compressed observed in
  root nodes), the allocated-size vs stored-size relationship.
- Determine whether the legacy chained extent layout occurs anywhere in current data.
- Confirm the routing table: for each DID type, which archives actually contain it.
- Record the largest legitimate record size per archive, so decompression caps are
  justified.

<a id="m0-exit"></a>
**Exit criteria**

1. `cargo run -p tessera-dattool -- info "$TESSERA_GAME_DIR"` lists all `client_*` archives
   of the install with kind, region, block size, version words and entry count.
2. `tessera-dattool validate --inflate` walks every directory tree of every archive without an
   invariant violation and inflates every compressed entry to its stated size.
3. Every DID listed by `ls` can be fetched by `cat` **through routing** (not by naming the
   archive), and the routing answers from the same archive the listing found it in.
4. Every string-table record in the local archive decodes with no error and no trailing
   bytes; every texture record header decodes; the property dictionary decodes.
5. `cargo run -p tessera-client` opens a window and renders the test scene on Linux
   (Vulkan), with and without `TESSERA_GAME_DIR` set. The headless renderer produces a
   golden image of the test scene in CI.
6. A Lua script can use `string`/`table`/`math` and is refused `io`, `os.execute`,
   `require` of C modules and `load` of bytecode. A runaway loop is stopped by the
   instruction limit.
7. Every parser has a fuzz target that runs for 60 s without a finding. Synthetic tests
   cover truncation, oversized counts and extent cycles.
8. CI is green; the repo contains no game data (content guard passes).

<a id="m0-status"></a>
**Status (verified 2026-10-04 against the owner's install, 29 archives, 3,838,957 entries):
all eight criteria pass. M0 is closed.**

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `tessera-dattool info` lists all 29 `client_*` archives (kind, region, block size, engine/data/minor version words, entries, compression, dataset, newest record); total 3,838,957 entries, 42.7 GiB. |
| 2 | ✅ | `validate --inflate` on the install: 29 archives, 3,838,957 entries, 1,740,958 compressed records all inflated to their stated size, 0 errors, 0 warnings, 0 duplicate DIDs, 20.8 s. |
| 3 | ✅ | Real-data test: 3,838,840 non-metadata DIDs located through routing alone, 0 unrouted, 0 answered by another archive, 0 missing; CLI check `ls` → `cat <dir>` on 140 DIDs from 27 archives, all `routed` to the listing archive. |
| 4 | ✅ | 282,152 string tables (806,052 strings) decode with 0 errors and 0 trailing bytes; 1,408,052 of 1,408,052 texture records decode (7 formats); property dictionary: 27,547 names, 27,546 definitions, 0 of unknown type. |
| 5 | ✅ | Windowed client on Wayland/Vulkan (NVIDIA) runs 300/1500 frames and exits 0 with `--no-data` (checkerboard scene) and with `TESSERA_GAME_DIR` (first decodable surface shown); screenshots checked. Headless goldens pass with 0 outliers on NVIDIA Vulkan, Mesa lavapipe (Vulkan, the CI reference) and Mesa llvmpipe (GL). |
| 6 | ✅ | `crates/script/tests/sandbox.rs`: `string`/`table`/`math` usable; `io`, `os.execute`, `require` (C modules), `load`/`loadstring` of bytecode and a bytecode main package refused; runaway loops stopped by the instruction budget; memory limit enforced. |
| 7 | ✅ | `tools/fuzz-smoke.sh -t 60`: all 25 targets ran 60 s each with no finding (0.3 M–84 M executions per target). Synthetic tests cover truncation, oversized counts (`resource_limits.rs`, `free_table_count_is_capped`) and extent/node cycles (`detects_cycles_and_bad_nodes`). |
| 8 | ✅ | Every CI job reproduced locally and green: `cargo fmt --check` (workspace and `fuzz/`), `clippy --workspace --all-targets [--all-features] -D warnings`, `clippy` on `fuzz/`, `cargo test --workspace --locked` on lavapipe with `TESSERA_REQUIRE_GPU=1` (803 passed, 0 failed, 1 ignored machine-specific), `cargo doc -D warnings`, `cargo deny check bans licenses sources` (both manifests), fuzz smoke. `tools/check-content.sh` and its self-test pass (300 files, 0 errors); `tools/check-spdx.sh` 0 errors. The hosted CI run itself has not been observed yet (no remote). |

---

<a id="m1"></a>
## M1: Asset browser (textures, strings, icons)

**Goal:** a GUI explorer that makes the data understandable: browse any archive, see
textures and icons, read string tables, inspect property sets with real names and enum
labels. This is also the main RE workbench for later milestones.

**Deliverables**

- `tessera-props`: registry, `Value`, `PropertySet` (disk encoding), enum mappers, DID
  maps, the "+0x09000000" property-set rule.
- `tessera-text`: string tables, StringInfo resolution, `${VAR}` substitution, grammar
  tags, locale selection.
- `tessera-assets`: worker pool with priorities and cancellation, decoded cache,
  placeholders, completion channel.
- `tessera-formats`: texture lists (mip chains), enum mapper, DID map, property set.
- `tessera-material::icon`: layered icon composition.
- `tessera-settings`: Tessera settings file, game-dir persistence.
- `tessera-explorer` (egui + wgpu): archive tree, DID search, hex view, texture viewer (mip
  levels, channels, alpha), icon view, string-table view with variable rendering,
  property-set view with names and enum labels, DID-map navigation.
- `tessera-dattool diff` between two installs or snapshots.

**Specs needed:** `texture-list.md`, `enum-mapper.md`, `did-map.md`,
`property-system.md` (property sets, StringInfo), `string-table.md` (grammar tags),
`icon-layers.md`.

**RE tasks**

- Validate the texture header and every pixel format against all texture records; list
  any format code not yet covered.
- Validate property-set decoding across all property-set records: count records with
  trailing bytes, identify the property types responsible.
- Map the root DID map's labels to their sub-maps; document which singleton records the
  client needs by label.
- Confirm the local archive's subset word as a language ID (needs a second locale
  installed, or the plugin API's language report).
- Document grammar-tag selection with examples taken from **our own** synthetic strings
  (no game text in specs).

**Exit criteria**

1. 100% of texture records decode or are reported as a listed, specified unsupported
   format; the explorer displays every supported format correctly (checked visually on a
   sample of each format, recorded as pass/fail).
2. 100% of string-table records render without error in the installed locale.
3. ≥ 99.5% of property-set records decode with no trailing bytes; the remainder are
   listed with the responsible property type in an OPEN callout.
4. The explorer stays responsive (no frame over 50 ms) while browsing a texture archive
   with tens of thousands of entries; decoding runs on workers.
5. Fuzz targets for every new decoder.

<a id="m1-status"></a>
**Status (verified 2026-10-04 against the owner's install): all five criteria pass. M1 is
closed.** Specs used: `texture-list.md`, `property-system.md`, `string-table.md`,
`icon-layers.md`; enum mappers and DID maps are specified in `records.md` §7-§8 rather than
separate files. Their `> **OPEN:**` items are questions about unused fields and meanings,
none of which blocks a criterion below.

| # | | Evidence |
|---|---|---|
| 1 | ✅ | 1,408,052 of 1,408,052 render surfaces decode to RGBA8; unsupported format codes: none (A8 659,208, DXT1 346,604, DXT3 206,910, JPEG 65,706, DXT5 60,676, A8R8G8B8 49,243, R8G8B8 19,705). 81,136 texture lists decode, 0 errors. Visual check in the explorer, one sample per format: A8, A8R8G8B8, DXT1, DXT3, DXT5, JPEG, R8G8B8 all pass (7/7), plus a composed item icon. |
| 2 | ✅ | 282,152 of 282,152 string tables in the English locale; 806,052 of 806,052 entries render (plain and explorer mode), 0 not clean, 0 leftover selectors. |
| 3 | ✅ | 384,950 of 384,950 property-set records (100.000 %) decode with 0 trailing bytes and 0 failures (47,680,867 values); no OPEN list needed. Registry: 27,546 definitions, all named; sampled objects resolve 100 % of names, enum labels and bit labels. |
| 4 | ✅ | Explorer `--browse` with frame statistics: 3,000 rows of `surface` (443,694 entries) → 3,022 frames, 0 over 50 ms, worst 18.4 ms; 8,000 rows of `surface_aux_1` (545,622 entries) → 8,022 frames, 0 over 50 ms, worst 15.0 ms. Decoding runs on the 22-worker asset pool. |
| 5 | ✅ | Fuzz targets for every M1 decoder: `formats_texture_list`, `formats_property_set`, `formats_string_table`, `formats_records` (enum mapper, DID map), `props_set`, `text_render`, `material_icon`, `settings_file`, `dattool_snapshot`; each ran 60 s with no finding. |

---

<a id="m2"></a>
## M2: Mesh and material viewer

**Goal:** view any static or skinned mesh with its materials, rendered by Tessera's own
shaders.

**Deliverables**

- `tessera-mesh`: mesh record decoding into engine vertex streams (static and skinned in
  bind pose), bounding volumes, submeshes.
- `tessera-material`: surface → material → texture list → texture chain, feature flags,
  dye rule.
- `tessera-render`: mesh and material pipelines (opaque, alpha-test, alpha-blend,
  dyeable), shared vertex/index buffers, orbit camera.
- `tessera-explorer`: mesh viewer (wireframe, normals, UVs, per-submesh material),
  material inspector, dye preview.
- `tessera-formats`: setup records (part assemblies) for static objects, as far as the
  spec allows.

**Specs needed:** `mesh.md`, `material.md` (surface, material, shader-record
classification), `setup.md` (first part).

**RE tasks**

- Establish a deterministic vertex-stride rule (per submesh) from mesh headers and data,
  validated on all mesh records. Prefer a rule from header fields over heuristics.
- Decode material slot lists: which slot is diffuse, normal, specular, and what
  parameters follow.
- Decide how materials get their feature flags without interpreting shader bytecode
  (ARCH §12 OPEN).
- Observe the original's rendering of a few reference objects (frame captures, prose
  notes on blend modes and alpha test thresholds).

**Exit criteria**

1. ≥ 99% of mesh records decode without error; failures are categorised in the spec.
2. A visual checklist of at least 50 meshes across categories (buildings, props, armour,
   weapons, vegetation) passes: no slivers, correct UVs, correct alpha-tested edges,
   correct dye regions. Recorded as pass/fail per item, not as images.
3. Synthetic golden images for each material pipeline pass on the software adapter.
4. Fuzz targets for mesh and material decoders.

---

<a id="m3"></a>
## M3: Terrain and landblock world walker (offline)

**Goal:** fly or walk across a whole region of the user's world offline: terrain, static
objects in place, area names and minimap, streamed without stalls. The sandbox backend
provides the avatar.

**Deliverables**

- `tessera-world`: coordinates, landblock IDs, streaming rings with hysteresis, landblock
  bundles, block maps (area, music, ambient keys), landblock properties (minimap tiles).
- `tessera-terrain`: heightmap meshing with LODs and seamless normals; splatting once the
  terrain-type data is decoded, height/slope shading before that.
- `tessera-formats`: landblock data, landblock info (cells, static entity descriptors,
  links), block-map descriptors, entity descriptors, positions.
- `tessera-wstate`: class library and object state, enough to resolve areas and region
  records by name.
- `tessera-input`: actions, contexts, Tessera keymap file, free-fly and walk controllers.
- `tessera-net` sandbox backend: spawn at a position (CLI `--spawn region,bx,by` or a
  named place), report position, serve static world entities.
- `tessera-client`: the world walker; on-screen area name, coordinates in the game's
  map-coordinate convention, minimap.

**Specs needed:** `landblock.md` (data, info, block map, properties), `coordinates.md`,
`entity-descriptor.md`, `wlib.md`, `wstate.md`, `keymap.md` (Tessera's own format first).

**RE tasks**

- Height scale: find the mapping from stored height values to world units, validated
  against in-game `/loc` heights at known spots (observation, written up as a table of
  test points in the spec).
- Decode the bytes after the height grid (terrain types, blend data); document what
  remains unknown.
- Validate static entity placement: position + rotation conventions, scale properties.
- Determine how outdoor static objects reference meshes vs setups.
- First look at sky/environment records and water (feeds an `environment.md` with OPEN
  items; not required for exit).

**Exit criteria**

1. Every landblock record (data, info, block map, properties) in every region archive
   decodes, or failures are categorised in the spec.
2. Flying a straight line across a full region at high speed never stalls a frame for more
   than 50 ms on the reference machine, and resident memory stays under a configured
   budget.
3. Terrain is crack-free at landblock boundaries and between LODs.
4. At 20 reference locations chosen by the dirty room (recorded as coordinates and
   expected facts like "a building corner at height H"), terrain height and object
   placement match within tolerance.
5. Area name and map coordinates shown by the walker agree with the original client at
   the same 20 locations.
6. Synthetic golden image of a generated terrain patch passes.

---

<a id="m4"></a>
## M4: Characters and animation

**Goal:** assemble and animate characters and creatures from the data: species/body
types, worn and held items, idle/walk/run cycles, and the animation events that script
tables drive.

**Deliverables**

- `tessera-havok`: clean-room reader for the Havok-format subset in the archives:
  skeletons; spline-compressed clips (including 40-bit quaternion tracks);
  uncompressed interleaved clips; annotation tracks.
- `tessera-anim`: sampling, blending, root motion, attach points, skinning palettes
  (sampling on workers).
- `tessera-formats`: appearance (wardrobe) records, held-item templates, entity records
  for avatars; first part of script tables (animation nodes).
- `tessera-render`: skinned pipeline with GPU skinning.
- `tessera-explorer`: character viewer (species, body type, worn items, dyes), animation
  browser.
- Sandbox backend: an animated avatar in the world walker; static NPCs placed where the
  data places them.
- Decision record on entity storage (ARCH §20.3 OPEN).

**Specs needed:** `havok-data.md`, `skeleton-anim.md` (bone mapping, attach points, root
motion), `appearance.md`, `setup.md` (complete), `script-table.md` (animation nodes).

**RE tasks**

- Describe the tagfile/packfile structures actually present (only those) and the
  spline-compressed track encoding, from public format documentation and data
  observation.
- Track-to-bone mapping rule and annotation semantics.
- Appearance assembly: how species, sex, appearance keys and part tags select meshes and
  materials; how bone sliders and height scale apply.
- How the original chooses idle/walk/run clips for an avatar (script tables and
  properties).

**Exit criteria**

1. 100% of skeleton records and ≥ 99% of clip records decode; the rest are categorised.
2. For every playable species and body type, an assembled character plays idle, walk and
   run without visible skinning errors (visual checklist, pass/fail).
3. Worn items replace body parts correctly and held items attach to the right bones on a
   checklist of at least 30 items.
4. 50 animated characters on screen at 60 fps on the reference machine, with sampling on
   workers.
5. Fuzz targets for the Havok reader cover each structure kind.

---

<a id="m5"></a>
## M5: UI layouts, skins and Lua plugins

**Goal:** draw the game's own UI panels from layout records, apply user skins, and run
existing third-party UI plugins that the user has installed.

**Deliverables**

- `tessera-formats`: UI layout records (elements, states, per-element data variants).
- `tessera-ui`: retained widget tree, absolute layout with anchors and margins, nine-slice
  art, text rendering, focus and input routing, skin application.
- `tessera-plugins`: `.plugin` discovery, apartments, import resolution, event dispatch,
  plugin data persistence in Tessera's directories, a first API surface (engine, shell
  commands, chat output, plugin data, UI controls and windows, read-only views of the local
  player that the sandbox backend can provide).
- `tessera-client`: native panels for the walker (minimap frame, chat window, a few
  windows), plugin manager window, `/plugins` commands.
- Optional: importer for the original keymap file if its spec exists (read-only).

**Specs needed:** `ui-layout.md`, `skin-definition.md`, `plugin-descriptor.md`,
`plugin-api.md` (names, signatures, enums, events, behaviour), `fonts.md` (if font records
are decodable), `keymap.md` (import section).

**RE tasks**

- UI layout grammar end to end, including the per-element data variants and state
  handling; map element IDs to their enum labels via the enum mapper.
- Font records: find the record type and decode it, or document that fallback fonts are
  required.
- Plugin API behaviour: event ordering, sizing rules, mouse/keyboard semantics, data
  persistence format, observed with small plugins written by the owner running in the
  original client.
- Keymap file syntax by observation of the file the original writes.

**Exit criteria**

1. Every UI layout record decodes, or failures are categorised.
2. At least 10 native panels render with the right art and geometry (checklist compared
   with screenshots taken by the owner; screenshots stay private).
3. A user skin folder changes the art of the panels it maps.
4. At least 10 popular third-party UI-only plugins from the user's plugin folder load,
   create their windows and respond to input. Their incompatibilities are recorded.
5. Sandbox escape tests: plugins cannot read or write outside their tree or data
   directory, cannot load C modules, and cannot exceed memory and instruction limits.
6. `docs/LEGAL.md` records the plugin-namespace interoperability rationale (ARCH §18 OPEN).

---

<a id="m6"></a>
## M6: Audio and player music

**Goal:** the world sounds right: area music and ambience by location, positional sound
effects from animation and script events, and the player-music system playing ABC files.

**Deliverables**

- `tessera-formats`: sound info and sound payload records; script-table sound nodes.
- `tessera-audio`: output backend (decision record: kira on cpal or alternative), streaming music,
  positional emitters, buses and volumes, music director with crossfades by area.
- `tessera-audio::abc`: ABC parser implementing the game's dialect and limits, sequencer,
  instrument sample mapping, fellowship-style synchronised start between local players
  (sandbox) and later between networked players (M7).
- `tessera-client`: audio settings, `/play`-style commands.

**Specs needed:** `sound.md`, `script-table.md` (sound nodes), `abc-dialect.md`,
`instruments.md`.

**RE tasks**

- Sound record layouts and how music/ambient enums map to sound records.
- Instrument samples: which records, pitch mapping, sustain behaviour per instrument.
- ABC dialect: accepted and rejected constructs, timing rule, chord limit, note range,
  described from the game's observable behaviour and from Maestro's MIT test suite
  behaviour (no copied code).

**Exit criteria**

1. 100% of sound payloads decode (WAV and Vorbis).
2. Area music changes with crossfade when walking across area borders in the walker.
3. An ABC conformance suite written by us (original tunes) passes: accepted/rejected
   files match the spec, note timing within tolerance.
4. No audio underruns during a 10-minute world flight on the reference machine.

---

<a id="m7"></a>
## M7: Own test server and own protocol

**Goal:** several Tessera clients share a world through a server and a protocol that this
project designs and documents itself.

**Deliverables**

- `docs/specs/tessera-protocol.md`: Tessera's protocol, versioned. Transport (to be decided by
  a decision record, for example QUIC), handshake with project-generated keys, message schema,
  property replication using the domain property model, movement and interest management,
  chat, time sync.
- `tessera-protocol`: schema, framing, encode/decode, fuzz targets.
- `tessera-server` (`apps/server`): headless server with accounts local to the server,
  world instances, interest management by landblock, chat, original test content
  (CC0 or project-made) as the default world.
- `tessera-net`: test-server backend implementing the same `Backend` trait as the
  sandbox.
- Load and soak test tools.

**Specs needed:** `tessera-protocol.md`, `server-content.md` (what content a server may
use, from `docs/LEGAL.md`).

**RE tasks:** none on the original's protocol. This milestone is design work, not RE.
Dirty-room input is limited to behavioural facts that make gameplay feel right (movement
speeds, interpolation feel), observed in the original client.

**Exit criteria**

1. Two or more clients on a LAN see each other move, animate and chat, with
   interpolation, for a one-hour soak without desync or leak.
2. The protocol spec is complete enough that a second implementation could be written
   from it (reviewed by someone who did not write the code).
3. Every protocol parser is fuzzed; malformed or hostile packets cannot crash client or
   server.
4. `docs/LEGAL.md` confirms the server's content rules before any public test build.

---

<a id="m8"></a>
## M8: GATED. Legal gate first, then a compatibility layer only if the gate passes

**Context.** The project owner's stated long-term goal is compatibility with the official
servers. `docs/LEGAL.md` §8 rates official-server connectivity as tier **T4**, the
highest-risk tier: it touches the EULA's protocol-emulation and unauthorised-connection
clauses, the ToS ban on deciphering transmissions, DMCA §1201 theories that a recent case
against an emulator of another game by the same rights holder accepted, and every user's
account. `docs/LEGAL.md` §9.3 therefore gates it.

M8 is split into a gate and a conditional build phase.

<a id="m8-gate"></a>
**Phase 1: the gate (allowed now)**

Work through conditions G1–G5 of `docs/LEGAL.md` §9.3 and record each in
`docs/decisions/`:

- G1: a written legal review by a qualified IP lawyer of the specific design, covering at
  least US and EU/UK law.
- G2: a permission request to Daybreak Game Company LLC, and separately to Middle-earth
  Enterprises for the marks, modelled on published precedents of written arrangements
  with emulator projects (non-commercial, users bring their own client data, no assets
  distributed, disclaimer).
- G3: a maintainer decision record citing G1 and G2 and listing residual risks, accepted
  by the project owner.
- G4: the design of a user-facing warning about account risk, shown before any official
  connection could be made.
- G5: confirmation that the design obeys the permanently-forbidden list of
  `docs/LEGAL.md` §8.

Before the gate passes, no spec about the official login, its encryption or key handling
is written, and no code for it lands (`docs/LEGAL.md` §9.3).

**Phase 2: build (only if G1–G5 are met)**

- Scope, methods and distribution exactly as the gate's decision record defines.
- A compatibility backend implementing the `Backend` trait (ARCH §19.3), outside the
  default workspace members, behind an explicit opt-in, not built by CI.
- No hostnames, keys or tables from the official client in the repository, ever. Anything
  of that kind is loaded at run time from the user's install, if the decision allows it
  at all.

**If the gate fails** (G1 advises against it, or a rights holder objects), the project
stays at T2 permanently, as `docs/LEGAL.md` §9.4 says. M1–M7 are useful on their own: an
offline world explorer, a data-file toolkit, and a client for the project's own server.

**Specs needed (Phase 2 only):** defined by the gate's decision record.

**Exit criteria (Phase 1):** G1–G5 each recorded in `docs/decisions/` with an outcome, and
a go/no-go decision accepted by the project owner.

---

<a id="not-planned"></a>
## 3. Explicitly out of scope

- Shipping, mirroring or linking any client binaries, archives, patches or extracted
  content.
- Modifying, patching or redirecting the official client or launcher.
- Any monetisation.
- A server that reproduces the live game's content and gameplay (tier T3) without written
  permission from Daybreak and Middle-earth Enterprises.
- Bulk-export features marketed for ripping content. `dattool extract` stays a
  personal-study tool.

---

<a id="open"></a>
## 4. Open roadmap questions

> **OPEN:** reference machine for performance criteria (CPU, GPU, RAM). Proposal: the
> owner's Linux workstation, recorded in `docs/perf/reference-machine.md` at M1.

> **OPEN:** whether a self-hosted CI runner with the owner's install should run the gated
> real-data tests on every change, or nightly. Results must stay pass/fail and counts.

> **OPEN:** Windows and macOS support targets per milestone. wgpu and winit make both
> plausible from M0; CI coverage for them is undecided.
