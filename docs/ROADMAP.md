<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera roadmap

> Status: founding document, written at milestone M0 (2026-10-03). M0, M1 and M2 exit
> criteria were verified and closed on 2026-10-04 ([M0 status](#m0-status),
> [M1 status](#m1-status), [M2 status](#m2-status)). M3 was verified on 2026-10-04
> ([M3 status](#m3-status)): every criterion passes on Tessera's side; criteria 4 and 5
> wait only for the owner's readings in the original client. M4 was verified on 2026-10-05
> ([M4 status](#m4-status)): every criterion passes on Tessera's side (criterion 4 on
> the reference machine, the owner's laptop, [decision 0010](decisions/0010-reference-machine-laptop.md);
> criterion 2 since the River-hobbit fix of the same day); the remaining owner comparisons
> are listed in its status. **M4.5 (world fidelity)** ([M4.5](#m4-5)), inserted by the
> owner on 2026-10-06 to run before M5 (M5–M8 keep their numbers), was verified on
> 2026-10-06 and last re-verified on main `01deafd` on 2026-10-10 ([M4.5 status](#m4-5-status)):
> criteria 1, 2, 3, 4, 7, 8 and 9 pass; 6 (visual rows) is not met (54 of 62 rows pass on
> the image, W10 fails, 7 rows cannot be judged from their cameras); 5 waits for a
> quiet-machine re-measure of flight M3-b (an indicative run passed), so M4.5 stays open. M5 (UI layouts, skins and Lua plugins) was built in
> parallel on branch `m5` and checked on 2026-10-08 ([M5 status](#m5-status)): criteria 1,
> 3, 5 and 6 pass, 2 is done on Tessera's side, 4 waits for the owner's plugin install and
> 7 (UI time budgets) is not met yet. M6 (audio and player music) was built in parallel on
> branch `m6` and checked on 2026-10-07 ([M6 status](#m6-status)): criteria 1 and 3 pass,
> 2 passes on Tessera's side (the owner's listening comparison is pending), and 4 passes as
> an indicative run that is to be repeated on the idle reference machine. M7 (own test
> server and protocol) was built in parallel
> on branch `m7` and verified on 2026-10-07 ([M7 status](#m7-status)): criteria 2–4 pass
> (4 waits for the owner's confirmation before any public test build), criterion 1 passes
> on one machine and waits for a run between two machines on a LAN. The parity track
> (P0–P8, [decision 0024](decisions/0024-parity-track.md)) started on 2026-10-09 with
> [P0](#p0) (scenario feed and `tessera-gamedata`). Milestones are ordered by
> dependency and by legal risk: each one builds on the previous one, and the risk tier
> (T0–T4, see `docs/LEGAL.md` §8) only rises when the earlier, safer tiers are solid.

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
| **M4** | Characters and animation | T1 | `havok`, `anim`, `entity` |
| **M4.5** | World fidelity: terrain texturing, scenery and grass, collision and walking, water, lit materials, sky and time of day | T1 | `physics` (new, L3); grows `formats`, `terrain`, `world`, `material`, `render`, `client` |
| **M5** | UI layouts and skins, Lua plugins | T1 | `ui`, `plugins` |
| **M6** | Audio and player music | T1 | `audio` |
| **M7** | Own test server and own protocol | T2 | `protocol`, `server`, test-server backend |
| **P0–P8** | Parity track, client half: gameplay presentation from the install, driven by a sandbox scenario feed (decision 0024) | T1 | `gamedata` (new, L3, P0); grows `net` (scenario feed, P0), `ui`, `plugins`, `wvm`, `client` |
| **M8** | **Gated**: legal review, then (only if the gate passes) a compatibility layer | T4 (gated) | none before the gate |
| **M9** | Extended scripting: Tessera's own script API beside the standard plugins (planned earlier as M5.5; moved last by the owner on 2026-10-08) | T1 (sandbox) to T2 (own server) | `ext`, `ext-host` |

M1–M6 depend on M0, and mostly on each other in order, but M5 (UI) and M6 (audio) can run
in parallel with M4 once M3's asset and world plumbing exists. M4.5 builds on M3 (streaming,
terrain), M2 (the material chain) and M4 (the walking avatar, setup collision volumes). M5
and M6 do not depend on it; by the owner's decision of 2026-10-06 it runs before them.
M9 (extended scripting) comes last by the owner's decision of 2026-10-08: it starts only
after full parity with the original client and compatibility with the server (M8), or, if
the M8 gate fails, after client parity with Tessera's own server.

**Parity track (P-track), accepted 2026-10-09:** milestones P0–P8 between M7 and M9
([decision 0024](decisions/0024-parity-track.md), [section](#p-track)). The owner chose the
client half only: gameplay presentation from the user's install (creation screen,
inventory, target frames, cast bars, quest log, vendors, social panels, maps, progression
panels, client-run game code), driven by a scenario feed in the sandbox and, if the M8 gate
passes, by M8 Phase 2. Tessera's own server keeps M7's scope; LEGAL.md §11.1 rule 5 is
unchanged. M9 starts only after P8 and M8 (or its fallback).

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

(As written, the primitives and render-surface facts live in `records.md` §1 and §5, and
install detection in `install-layout.md`; see "Specs used" in the status below.)

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
all eight criteria pass. M0 is closed.** Specs used: `dat-container.md`, `records.md` (§1
primitives, §2 type table, §5 render surfaces: the planned `primitives.md` and
`render-surface.md`), `string-table.md`, `property-system.md`, `install-layout.md` (the
planned `install-detection.md`).

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `tessera-dattool info` lists all 29 `client_*` archives (kind, region, block size, engine/data/minor version words, entries, compression, dataset, newest record); total 3,838,957 entries, 42.7 GiB. |
| 2 | ✅ | `validate --inflate` on the install: 29 archives, 3,838,957 entries, 1,740,958 compressed records all inflated to their stated size, 0 errors, 0 warnings, 0 duplicate DIDs, 20.8 s. |
| 3 | ✅ | Real-data test: 3,838,840 non-metadata DIDs located through routing alone, 0 unrouted, 0 answered by another archive, 0 missing; CLI check `ls` → `cat <dir>` on 140 DIDs from 27 archives, all `routed` to the listing archive. |
| 4 | ✅ | 282,152 string tables (806,052 strings) decode with 0 errors and 0 trailing bytes; 1,408,052 of 1,408,052 texture records decode (7 formats); property dictionary: 27,547 names, 27,546 definitions, 0 of unknown type. |
| 5 | ✅ | Windowed client on Wayland/Vulkan (NVIDIA) runs 300/1500 frames and exits 0 with `--no-data` (checkerboard scene) and with `TESSERA_GAME_DIR` (first decodable surface shown); screenshots checked. Headless goldens pass with 0 outliers on NVIDIA Vulkan, Mesa lavapipe (Vulkan, the CI reference) and Mesa llvmpipe (GL). |
| 6 | ✅ | `crates/script/tests/sandbox.rs`: `string`/`table`/`math` usable; `io`, `os.execute`, `require` (C modules), `load`/`loadstring` of bytecode and a bytecode main package refused; runaway loops stopped by the instruction budget; memory limit enforced. |
| 7 | ✅ | `tools/fuzz-smoke.sh -t 60`: all 25 targets of the time (47 after M3) ran 60 s each with no finding (0.3 M–84 M executions per target). Synthetic tests cover truncation, oversized counts (`resource_limits.rs`, `free_table_count_is_capped`) and extent/node cycles (`detects_cycles_and_bad_nodes`). |
| 8 | ✅ | Every CI job reproduced locally and green: `cargo fmt --check` (workspace and `fuzz/`), `clippy --workspace --all-targets [--all-features] -D warnings`, `clippy` on `fuzz/`, `cargo test --workspace --locked` on lavapipe with `TESSERA_REQUIRE_GPU=1` (803 passed, 0 failed, 1 ignored machine-specific), `cargo doc -D warnings`, `cargo deny check bans licenses sources` (both manifests), fuzz smoke. `tools/check-content.sh` and its self-test pass (300 files, 0 errors); `tools/check-spdx.sh` 0 errors. The hosted CI run had not been observed at M0 (no remote). A private GitHub remote exists since 2026-10-04; its Actions results have not been recorded here yet (owner: check the runs, the Actions minutes and the lavapipe job, then update this sentence). |

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

**Specs needed:** `texture-list.md`, `enum-mapper.md`, `did-map.md` (both written as
`records.md` §7–§8),
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

<a id="m2-status"></a>
**Status (verified 2026-10-04 against the owner's install): all four criteria are met. M2 is
closed.** Specs used: `mesh.md`, `material.md`, `setup.md` (part 1), `render.md`. Their
`> **OPEN:**` items are about meanings the viewer does not need (influence list, scalar,
vertex-colour and diffuse-colour use, exact dye blend, level switch distances) or are
recorded with a viewer decision; none blocks a criterion below. One visual-checklist item
fails on such a question (criterion 2).

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `crates/mesh/tests/real_data.rs`: 47,563 of 47,563 mesh records decode (100 %), strict mode included, 0 trailing bytes, lossless re-encoding 47,563 / 47,563, every level flattens to renderer streams; 112,910 submeshes, 89.6 M vertices, 95.4 M triangles, 27 vertex formats. Failure categories are listed in `mesh.md` §15/§18 (none observed; the only invariant finding is MI10 `CollisionIndexOverflow` in 105 records, a data-quality check that does not stop decoding). Materials (`crates/material/tests/material_real_data.rs`): 510 / 510 shader records, 24,094 / 24,094 materials, 13,781 / 13,781 surfaces decode and resolve; 1,192 / 1,192 sampled diffuse textures decode. |
| 2 | ✅ | [`docs/checklists/m2-visual.md`](checklists/m2-visual.md): 74 checks on 72 objects with `tessera-explorer --screenshot`, each image viewed: 15 weapons and shields, 13 armour (worn, bind pose, 2 dye previews), 15 creatures and NPCs (skinned, bind pose), 9 vegetation, 9 buildings, 13 props. **73 pass** (no slivers, UVs correct, alpha-tested foliage/feathers/fringes cut cleanly, dye confined to the mask). **1 fail**: a one-triangle placeholder part of a creature setup draws as a white triangle (decoded correctly; whether the original skips such parts is `setup.md` §8.5 OPEN). |
| 3 | ✅ | `crates/render/tests/mesh_golden.rs` (opaque, alpha test, alpha blend + order independence, two-sided, dye, normal map, debug views): 8 / 8 pass on Mesa lavapipe (Vulkan, Mesa 25.2.6) with `TESSERA_REQUIRE_GPU=1`, and 8 / 8 on Mesa llvmpipe (GL, Mesa 26.1.8); at most 2 (lavapipe) and 7 (llvmpipe) outliers of 40,960 pixels against a budget of 102. `render_test_scene` passes on lavapipe too. |
| 4 | ✅ | `tools/fuzz-smoke.sh -t 60`: `mesh_decode`, `material_records`, `material_resolve`, `formats_surface`, `formats_setup`, `formats_setup_chain`, `formats_appearance`, `render_mesh_data`, `explorer_mesh_scene` each ran 60 s with no finding (0.23 M–8.4 M executions per target). |

Repository checks (final verification, 2026-10-07): `cargo fmt --all --check` and the fuzz
crate's `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings` with
default and with all features; the fuzz crate's `cargo clippy --bins --locked -- -D warnings`
(four lints in the new M6 targets fixed); rustdoc with `-D warnings` on the M6 crates;
`tools/check-content.sh` (758 files) and `tools/check-spdx.sh` (721 files) clean; a link and
anchor check of every Markdown file (87 files, 0 broken). `cargo test --workspace
--no-fail-fast` with `TESSERA_GAME_DIR` set (debug, load average about 50): 2,405 passed, 2
failed, 2 ignored in 169 suites, plus 2 suites whose binaries another process removed during
the run (`tessera-dattool` `cli` and `real_data`, 28 / 28 when re-run). The 2 failures are
load-dependent timeouts outside M6's code and pass when re-run alone:
`tessera-world` `integration_progresses_when_bookkeeping_exceeds_the_frame_budget` (a 60 s
settle limit; 64 / 64 in release) and `tessera-client` `walker_real_data::screenshot_at_spawn`
(the 90 s settle limit; 3 / 3 alone). The M6 real-data tests pass in debug and in release:
`tessera-formats` `sound_real_data` 2 / 2, `tessera-scriptgraph` `real_data` 2 / 2,
`tessera-audio` `decode_real_data` 1 / 1, `abc_conformance` 156 / 156, `abc_examples` 33 / 33,
`instruments_real_data` 1 / 1, `tessera-client` `audio_real_data` 3 / 3 and
`player_music_real_data` 2 / 2. `tools/fuzz-smoke.sh -t 60` (stable libFuzzer) on the eight
new targets, no crash: `formats_sound_info` 1.28 M executions, `audio_decode` 12,929 (and
17,989 after the codebook change), `audio_engine` 23,156, `audio_abc` 244,182,
`audio_instruments` 19,071, `scriptgraph_runtime` 3,609 (117 s), `client_audio` 894,479,
`client_player_music` 3.96 M; rates are low because other builds loaded the laptop.

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

<a id="m3-status"></a>
**Status (verified 2026-10-04 against the owner's install, GTX 1070 Ti / Vulkan, release
builds): criteria 1, 2, 3 and 6 pass; criteria 4 and 5 are data-side ✅, owner observation
pending.** M3 is complete on Tessera's side and closes when the owner records the original
client's readings at the 20 reference locations (item
[O1](checklists/owner-observations.md#o1)). Specs used: `landblock.md`, `coordinates.md`,
`entity-descriptor.md`, `wlib.md`, `wstate.md`, `keymap.md`, `terrain.md`, `walker.md`;
none has a blocking `> **OPEN:**` item (water, sky, splat materials, draw distances and the
map-coordinate rounding rule remain open and are recorded with Tessera's choice).

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `cargo test --release -p tessera-formats --test landblock_real_data` (every record, each with its directory stamp): landblock data 390,150 / 390,150, block maps 390,150 / 390,150, landblock info 49,636 / 49,636 (3,253,862 static entities, 104,271 cells), landblock properties 60,450 / 60,450, kind-5 records 33,159 / 33,159, 5 block-map descriptors and 7 region descriptors decode; 0 failures in any `landblock.md` §9 category, 0 unusual; data and block maps re-encode byte for byte; HM7 0 of 359,949 neighbour pairs differ. |
| 2 | ✅ | `tessera-client --fly-path … --frame-log` on the GTX 1070 Ti, frame logs re-checked independently: windowed 1920 × 1080 (vsync) across all of region 1 (row 118, 40.6 km at 1,000 m/s): 2,439 frames, longest 21.2 ms, p99 18.0 ms, 0 over 50 ms, peak 386 landblocks / 86.8 MiB bundles (budget 512 MiB), 793 MiB GPU, 716 MiB RSS, no near-ring holes; region 2 diagonal at 3,000 m/s: longest 21.0 ms, peak 118.2 MiB; region 4 row 128 at 2,000 m/s with a 48 MiB budget: longest 21.7 ms, peak 11.6 MiB; headless 1920 × 1080 region 5 diagonal at 2,000 m/s: longest 19.4 ms, peak 57.4 MiB. A binding 32 MiB budget held at 34.9 MiB peak (the always-admitted near ring, `landblock.md` §8.1), still 0 frames over 50 ms. `walker_real_data::flight_across_a_region` (release, full region) and `tessera-world` `stream_straight_line_across_region` (5,355 bundles, 0 failed, update max 0.31 ms, peak 87.5 MiB) pass. Archives were in the page cache. |
| 3 | ✅ | `cargo test --release -p tessera-terrain` with the install: all 390,150 blocks meshed; every one of the 129,540 east/north neighbour pairs per region (382,536 seams with relief) compared at pseudo-random levels on each side, 0 mismatches in edge vertices, normals or segments; 390,150 stitched covers valid. Synthetic: property tests (`shared_edges_are_identical`, `meshes_cover_the_block`), `neighbour_seams_are_identical_across_lod_transitions`, the walker's `layer_tracks_bundles` seam-signature check. Seven walker screenshots from low grazing angles (eye 2–40 m, pitch −1° to −6°) along and across landblock boundaries and over mixed-LOD mountains in regions 1, 2, 3 and 5 viewed: no cracks or sky-coloured slivers. |
| 4 | ✅ data-side, owner pending | `cargo test --release -p tessera-client --test reference_locations`: at all 20 locations of [`m3-reference-locations.md`](checklists/m3-reference-locations.md#s4-3) the terrain height (±0.05 m) and the anchor object's SED, scale, yaw, placed bottom corner, bottom and top (±0.1 m) match. One checklist value (M3-10 mesh bottom) was computed over all detail levels; with `mesh.md` §6.1's level-0 bound it is 225.09 m and passes (erratum in the checklist). Two locations viewed in the walker with `--place`. |
| 5 | ✅ data-side, owner pending | Same test: the map-coordinate string equals the expected value at 20 of 20 (always the round-half-away form), the `/loc` tokens equal the expected ones and parse back to the stand point, and the block map's `Area_DID` is the expected area with a resolved name at 20 of 20. Agreement with the original client needs the owner's readings (O1). |
| 6 | ✅ | `crates/terrain/tests/golden.rs` (`terrain_patch`, `terrain_lod_wireframe`, 3 × 3 blocks at levels 0–4 with stitched edges): pass on Mesa lavapipe (llvmpipe Vulkan CPU adapter, Mesa 25.2.6) with `TESSERA_REQUIRE_GPU=1` (0 outliers in both images) and on the NVIDIA adapter (0 outliers). |

Repository checks at the same time: `cargo fmt --check` (workspace and `fuzz/`), `cargo
clippy --workspace --all-targets [--all-features] --locked -D warnings` and on `fuzz/`
(one `manual_is_multiple_of` lint in `fuzz_targets/formats_landblock_data.rs` fixed),
`cargo test --workspace` with `TESSERA_GAME_DIR` set (1,435 passed, 0 failed, 2 ignored:
one machine-specific, one doc example), `cargo doc --workspace --no-deps -D warnings`
(ten links from public walker docs to private items fixed), `cargo deny check bans licenses
sources` (both manifests), `tools/fuzz-smoke.sh -t 60` on the 13 M3 targets
(`formats_block_map`, `formats_landblock_data`, `formats_landblock_info`,
`formats_landblock_properties`, `terrain_mesh`, `world_bundle`, `world_text`,
`wstate_library`, `wstate_object`, `input_keymap`, `input_runtime`, `net_sandbox_world`,
`client_walker_text`; stable libFuzzer, 0.17 M–8.7 M executions each, no finding),
`tools/check-content.sh` and its self-test, `tools/check-spdx.sh` (0 errors) and the
relative links and anchors of the Markdown docs all pass.

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

<a id="m4-status"></a>
**Status (verified 2026-10-05 against the owner's install, release builds, on the owner's
laptop with an i9-10885H and an RTX 2070 Super Max-Q / Vulkan, which is the reference
machine from 2026-10-05, [decision 0010](decisions/0010-reference-machine-laptop.md)):
criteria 1, 3 (data side), 4 and 5 pass. Criterion 2 passes for all 14 bodies since the
River-hobbit fix of 2026-10-05 (below).** Every deliverable exists: `tessera-havok`,
`tessera-anim`, the `tessera-formats` appearance, item, avatar and script-table decoders
with the clip resolver, `tessera-mesh::assembly`, GPU skinning in `tessera-render`, the
explorer's character viewer and animation browser, the walker's animated avatar, placed
NPCs and `--crowd`, and decision [0008](decisions/0008-entity-storage.md) (own entity store,
`bevy_ecs` not adopted). Specs used: `havok-data.md`, `skeleton-anim.md`, `appearance.md`,
`setup.md` (parts 1 and 2), `script-table.md`, plus `mesh.md` §6.2, `render.md` §2.4 and
`walker.md` §10. No `> **OPEN:**` item blocks a criterion; each carries a Tessera
Decision or waits on owner observations O8–O17 in [owner-observations.md](checklists/owner-observations.md). The four new M4 specs
(`havok-data.md`, `skeleton-anim.md`, `appearance.md`, `script-table.md`) are still in
Review: no spec review is recorded for them yet. `script-table.md`'s `BIN` facts come from a
disassembly of the client's script-table node classes, which the owner authorised
retroactively in decision [0009](decisions/0009-script-table-disassembly.md) (an incident
under `docs/CLEAN_ROOM.md` §10, limited to that question and client build).

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `cargo test --release -p tessera-havok --test real_data`: skeleton records (setups 0x04) 28,555 / 28,555 decode (53,370 bones, at most 248); clip records 25,395 / 25,395 decode (100.000 %; 24,800 spline-compressed, 595 interleaved, 31,294 blocks; 5,602 with annotation tracks, 110 leading and 104 embedded reference frames), every clip sampled with a largest quaternion length error of 1.19 × 10⁻⁷; 0 in each of the 27 failure categories of `havok-data.md` §9.3. `tessera-anim` `real_data`: all 28,555 skeletons build; 25,393 clips bind and sample (the other 2 have no skeleton with their track count, `skeleton-anim.md` §3.3), 0 non-finite matrices; reference-pose skinning of all 2,625 skinned setup submeshes (2.5 M vertices) reproduces the stored vertices (largest error 4.4 mm from the stored weight sums, 3.1 × 10⁻⁵ m with weights renormalised; 0 bone-index or influence-list violations). |
| 2 | ✅ data-side, owner observation pending | Rows B1–B42 of [m4-visual.md](checklists/m4-visual.md): all 14 bodies (11 setups) assemble, and their idle, walk and run clips (chosen by the script tables, equal to `script-table.md` §8.3 for all 14, `script_table_real_data`) play through `tessera-anim` on the workers and GPU skinning. The 42 sheets (`tessera-explorer --sheet … --character <body> --clip idle\|walk\|run`, 4 play times × 2 angles each) were re-rendered and viewed in this verification: no stretched, exploding or detached geometry, seams closed, no part left in the bind pose. **42 / 42 rows pass.** The 6 River-hobbit rows (B37–B42) first failed on foot contact (feet about 0.09 m up). The owner then saw the River-hobbit idle in the client pulse taller and shorter in a loop (2026-10-05): the layer-110 `scale_blend` clip was played looping. Fixed the same day by the shared client and explorer figure path (`bind_shape`/`figure_shape`): that clip is held at the fraction `AC_Avatar_Height`, the body-shape deformer clip at `AC_BoneScale_Test` (bind-space deformation), and slider bytes follow `appearance.md` §6.3 (checklist note 1). Verified on the real install (release, `TESSERA_GAME_DIR` set): `figure_shape_real_data` 5 / 5 pass (client path, 33.3 s of idle at 0.05 s steps): River-hobbit M idle changes 0.0032 m on the root chain and 0.0062 m on the held-clip bones (0.0715 m looped as before), F 0.0032 / 0.0071 m (0.0712 m); idle lowest bone 0.011 m (M) and 0.008–0.009 m (F), like the other bodies (0.015–0.03 m, `other_bodies_are_unchanged` passes); standing heights at the ends and middle of each height control match the `appearance.md` §6.5 table exactly (M 1.231–1.300 m, F 1.200–1.245 m, lowest vertex within 0.01 m of the ground); explorer side-view sheets viewed, no pulsing (head-top row changes ≤ 1 px), feet flat, skinning clean. Not blocking: River-hobbit M's lowest skinned vertex reaches −0.055 m while walking (about −0.02 m on the other bodies; `appearance.md` §6.5 OPEN). Owner look, 2026-10-05 (laptop, by eye, no measurements): character positions and heights look fine ([m4-visual.md](checklists/m4-visual.md) note 7). Comparison with the original: O8, O9, O14, O16. |
| 3 | ✅ data-side, owner observation pending | `cargo test --release -p tessera-mesh --test assembly_real_data`: 48 distinct items over 27 worn, 9 layered, 27 held and 25 stowed rows on the checklist's bodies, every expected winner, hidden slot, priority, holding location and bone matches; the 7 static NPC spots and all 775 placed weenie objects assemble. Every W, L and H row was re-rendered and viewed (worn parts replace and hide the expected slots, held items sit at their location's bone and follow it; stowed one-handed weapons at the hip, two-handed weapons, bows and shields on the back). This verification fixed `ItemVisual::equipment_category`, which dropped category bits above 31, so the medium shield (H21) now stows on the back like the other shields. The grip axis of in-hand items (checklist note 3) and the in-world rest positions are open; comparison with the original: O10, O13. |
| 4 | ✅ | On the reference machine ([decision 0010](decisions/0010-reference-machine-laptop.md)), release build of `a47af81` with rustc 1.99.0, 2026-10-05, frame logs re-computed independently: `tessera-client --headless --unpaced --duration 60 --crowd 50 --heading 30 --size 1280x720` gave 51 animated characters (the crowd of all 14 bodies standing, walking and running, plus the avatar), all 51 drawn in every frame, 15,412 frames in 60 s, 256.8 fps mean, p50 3.7 ms, p99 6.6 ms, longest frame 12.0 ms, none over 16.7 ms; animation main thread 0.246 ms per frame, 82.2 % of the animation work on the asset workers. Windowed with vsync off (`--no-vsync --crowd-spacing 2 --crowd-ahead 20`, surface 1430 × 790): 350.5 fps mean, p99 4.2 ms, longest 9.6 ms, 82.8 % on workers. Details and the M3 re-baseline on this machine: [reference-machine.md](perf/reference-machine.md). |
| 5 | ✅ | `docs/specs/havok-data.md` §11 kinds K1–K13 map to the four Havok targets (`havok_tagfile` K1–K5, `havok_skeleton` K6, `havok_clip_record` K7, K8, K10–K13, `havok_spline_block` K9, K10; table in `crates/havok/src/lib.rs`); `crates/havok/tests/categories.rs` produces each of the 27 failure categories from synthetic input. `tools/fuzz-smoke.sh -t 60` (stable libFuzzer) on all 15 new M4 targets plus `world_bundle`, `net_sandbox_world` and `mesh_decode`: no finding; Havok targets 0.09 M (`havok_clip_record`) to 3.9 M (`havok_spline_block`) executions, the others 0.15 M–11.0 M. |

Repository checks at the same time: `cargo fmt --check` (workspace and `fuzz/`), `cargo
clippy --workspace --all-targets --all-features --locked -D warnings` and on `fuzz/`,
`cargo test --workspace` with `TESSERA_GAME_DIR` set (1,752 passed, 0 failed, 2 ignored, in a
debug build), `cargo doc
--workspace --no-deps -D warnings`, `cargo deny check bans licenses sources` (both
manifests), `tools/check-content.sh` (two test identifiers renamed) and its self-test,
`tools/check-spdx.sh` (0 errors) and the relative links and anchors of the Markdown docs all
pass. The checks ran on the distro toolchain (rustc 1.97.1) because rustup is not installed
on this machine; `rust-toolchain.toml` pins 1.99.0, so re-run clippy on the pinned toolchain
before or after committing.

Fixed during this verification: `ItemVisual::equipment_category` widened to the full bit
pattern (H21 stowing); the renderer resolves the `AutoVsync`/`AutoNoVsync` present modes
against the surface instead of falling back to `Fifo` (the client's `--no-vsync` now asks for
`AutoNoVsync`); the NPC-spot test's settle timeout is longer in unoptimised builds (the
432-figure spot did not settle within 60 s in a debug build); `fuzz/README.md` rows for the
M4 targets; ARCHITECTURE §11 brought up to the M4 specs.

Fixed after this verification (2026-10-05, on the owner's report that the River-hobbit idle
pulsed in the client): River-hobbit height and body shape through held clips and bind-space
deformation, one figure path for the client and the explorer (criterion 2, rows B37–B42).

Remaining for M4 to close: owner observations O8–O17 (for criterion 2: O8, O9, O14, O16,
and the River-hobbit M walking toe dip of `appearance.md` §6.5 OPEN).

---

<a id="m4-5"></a>
## M4.5: World fidelity

Inserted by the owner on 2026-10-06 to run after M4 and before M5. M5–M8 keep their numbers.

**Goal:** the offline world looks and behaves like the original's at its default settings.
Terrain is drawn with its own texture layers. Trees, rocks and grass stand where the data's
placement rules put them. The avatar walks on collision geometry instead of terrain heights:
walls, objects and cave ceilings block it, it enters interiors through their cells, and it
wades and swims. Lakes, rivers and seas are drawn. Surfaces go through the lit pipeline,
under a sky that follows the game clock with its sun, ambient light and fog. Everything is
built from the user's own data at run time, deterministically, and stays within the M3/M4
frame budget on the reference machine.

<a id="m4-5-deliverables"></a>
**Deliverables**

- `tessera-formats` (each decoder consumes its record exactly, treats input as hostile, and
  has one fuzz target):
  - per-region tables named by the region descriptor (0x0F, slots 1–8, `environment.md` §3;
    the M3 decoder is extended): terrain type table **0x19** (`terrain-texturing.md` §3),
    terrain-scene table **0x12** and scene-choice table **0x11** (`scenery.md` §4.2–§4.3),
    water table **0x16** and fog-zone table **0x17** (`water.md` §4–§5);
  - scene records **0x02** (`scenery.md` §5);
  - collision: stand-alone collision meshes **0x66**, cell geometry **0x01** (cell volume,
    portals, wall polygons, hull vertices, physics mesh), and the quantised tree reader that
    meshes, terrain, ceilings, 0x66 records and cells share (`collision.md` §3–§5);
  - environment: sky description **0x15** (the single record 0x15000007), day descriptions
    **0x1A**, game time **0x35** (0x35000000), LOD-class table **0x32** (0x32000000)
    (`environment.md` §5, §6, §9.1);
  - landblock data brought up to the revised `landblock.md`: the client's height rule
    (stored × 2,000 / 65,535, §3.4, replacing stored / 32.768), the ceiling diagonal from
    vertex-flag bit 0 (§3.5–§3.6), terrain and ceiling tree blocks (§3.9), light-map
    properties (§6.4), and the water pairs of the kind-5 distant-sector records (§7, only as
    far as the layout is known);
  - optional, may slip to M5 without blocking: camera-state records **0x08**
    (`walker.md` §11.5).
- `tessera-terrain`: layer resolution against the region's 0x19 table (key, order,
  `whole`, unmasked layers, holes, quad and coarse grids, `terrain-texturing.md` §4); alpha
  masks (§5); per-vertex texture-coordinate sets with the per-type scale, rotation and
  offset and the per-block offset (§6.1–§6.3); the tangent frame for normal maps (§6.4);
  the opacity rule and splat inputs per layer (§7); ceiling chunks (§4.8); light-map
  coordinates (§10). This replaces the height and slope shading that ARCH §10 planned until
  splatting was known.
- `tessera-physics` (new crate, below): terrain and ceiling collision triangles with the
  client's numbering (`collision.md` §6.1–§6.4), the terrain radius and plane queries that
  scenery placement uses (§6.6), cell volumes and the point-in-cell lookup (§5.4, §5.8.1),
  per-landblock collision sets of statics, generated objects and cells (§9.3, §9.4, §12.1)
  with Tessera's own bounding-volume hierarchy, capsule sweeps, and the character controller
  (§10–§12: parameter layering, float height, walkable/slide/wall contacts, step-up, fixed
  1/60 s step) including the water states of `water.md` §7.4.
- `tessera-world`:
  - scenery generation per landblock (`scenery.md` §6–§7: vertex scene pick, seeds,
    placement nodes, height and water ranges, clearance and spacing). It is deterministic,
    runs on the asset workers, and its objects enter the bundle like statics, with their
    collision (`collision.md` §9.4);
  - the frill (grass) layer: 10 × 10-unit sectors in their own ring around the camera with
    the engine defaults of `scenery.md` §8.2–§8.4. Frills never collide;
  - landblock water (`water.md` §6.2–§6.3), water volumes from the volume entity and placed
    volumes (§7), and the "under water" query;
  - cells and portals of landblock info in the collision residency; collision sets built
    with the bundle on the asset workers;
  - the game clock, day-description choice per sky key, keyframe blending and sky objects
    (`environment.md` §11). The clock epoch is OPEN, so Tessera's clock has a settable
    start time;
  - light selection per object and Light entities (`lighting.md` §8.3–§8.5);
  - object draw distances by LOD class from 0x32 with fades and detail levels
    (`walker.md` §11.4, `mesh.md` §8).
- `tessera-material`: float-curve evaluation (`material.md` §5.3.1), texture transforms
  (§8.4), feature flags (alpha test, blend, two-sided, §11), the per-surface lighting
  inputs of `lighting.md` §4.1, and the water material chain (`water.md` §8.2). The 0x31,
  0x30 and 0x2B decoders exist since M2.
- `tessera-render`:
  - mip-chain upload (`texture-list.md` §5; closes the aliasing OPEN of `walker.md` §9);
  - the terrain layer pipeline (`terrain-texturing.md` §8–§9: draw order, blend state,
    opacity, normal maps, specular, baked light maps);
  - the lit pipeline (`lighting.md` §5–§7): ambient with the hemispheric term, the sun or
    moon with a shadow map (§9), up to 8 point lights (§8.3), specular, emissive and glow,
    vertex colours, distance fog (§10), and the unlit and additive passes;
  - the water pipeline (`water.md` §8, distant water §9);
  - the sky (`environment.md` §12);
  - instanced scenery and frills (`scenery.md` §8.3, §9);
  - a synthetic golden image for each pipeline.
- `tessera-client` (walker):
  - walk mode on `tessera-physics` (capsule from the avatar's setup, `collision.md` §10.6)
    instead of terrain heights, with jumping, sliding, wading, swimming and interior cells;
  - time-of-day controls (start time and rate);
  - a switch per M4.5 system for profiling;
  - the default-settings profile of `walker.md` §11.8: landscape extent 2,080 m, per-class
    object distances with the 1.5 s fade, detail levels, frills at level 2;
  - a scripted `--walk-path` with a movement log (position, state, cell, contacts) for the
    collision tests;
  - new frame-log columns for the M4.5 systems.
- `tessera-explorer`: views for terrain type tables, scene records and a landblock's
  generated scenery, the water table and landblock water, the sky and day-description
  timeline, and collision meshes and cells as overlays. `tessera-dattool` gets inspection
  commands for the same records.
- Documents:
  - `docs/checklists/m4-5-visual.md`, the visual and collision checklist, written by the
    dirty room as `m3-reference-locations.md` was: coordinates and expected facts only;
  - new owner items in [owner-observations.md](checklists/owner-observations.md) for the
    measurements below;
  - an M4.5 section in `walker.md` (Tessera's design);
  - in ARCHITECTURE: §10 rewritten (splatting, water, sky; the OPEN there is answered by
    the specs), a §3.1 row for `tessera-physics` with the sibling allowance, and §20.3's
    30 Hz simulation tick reconciled with the 1/60 s controller step of `collision.md`
    §12.3.

<a id="m4-5-crates"></a>
**Crate placement (decision for this milestone).** Collision and movement go into a new
L3 crate, **`tessera-physics` (`crates/physics`)**, and not into `tessera-world`. It
depends only on `formats` and `props` (L1–L2) and has no GPU, asset-server or streaming
dependency. `tessera-world` gets an explicit sibling allowance to depend on it: scenery
generation reads the terrain collision mesh and its queries, and bundles carry collision
sets built on the workers. Reasons: the controller can then be unit-tested and fuzzed
without streaming, and the M7 test server can reuse the controller and the collision-set
types for server-side movement (building the sets from install data stays `world`'s work;
[decision 0013](decisions/0013-physics-crate.md) keeps that question OPEN for M7). The quantised-tree reader
and the 0x66 and 0x01 decoders stay in `tessera-formats`, like every record decoder.
Scenery, frills, water state and the game clock are modules of `tessera-world`, as their
specs say. No other new crate.

<a id="m4-5-order"></a>
**Order of work.** (1) Specs to Accepted (table below). (2) Decoders and their real-data
censuses. (3) Terrain layers and the lit pipeline, which every later visual depends on.
(4) Physics: terrain, ceiling, cells, statics, then the controller. (5) Scenery and frills,
which need the terrain collision mesh. (6) Water. (7) Sky, clock and fog. (8) Walker
integration, the default-settings profile and the performance runs.

<a id="m4-5-specs"></a>
**Specs needed** (status as of the M4.5 re-verification on main `01deafd`, 2026-10-10, read
from each spec's header; on 2026-10-07 ten of them were Revised with changes requested or a
pending re-review, and `walker.md` §13–§14 and `render.md` §4–§5 were in review with changes
requested)

| Spec | Covers | Status |
|---|---|---|
| `terrain-texturing.md` | 0x19, layers, masks, texture coordinates, opacity, blending, light maps | **Accepted** (2026-10-07, sixth revision) |
| `scenery.md` | 0x11, 0x12, 0x02, hashes and seeds, generation, frills, LOD classes, wind | **Accepted** (2026-10-08) |
| `collision.md` | Quantised trees, 0x66, 0x01, cells and portals, terrain and ceiling collision, setup volumes, movement data, Tessera's controller | **Accepted** (2026-10-09) |
| `water.md` | 0x16, 0x17, landblock water, water volumes, drawing water | **Accepted** (2026-10-10, fifth revision of the day) |
| `material.md` | 0x31, 0x30, 0x2B, float curves, transforms, feature flags, dyes | **Accepted** (2026-10-10) |
| `lighting.md` | Passes, lighting equation, light sources, sun shadow, fog | **Accepted** (2026-10-09) |
| `environment.md` | 0x0F, 0x15, 0x1A, 0x35, 0x32, game clock, keyframe blending, sky | **Accepted** (2026-10-08 revision) |
| `landblock.md` | Height rule, ceiling grid, vertex flags, tree blocks, light maps, kind 5 | **Accepted** (2026-10-08) |
| `records.md` | Prologues and DB types of every prefix above, VLE, TSize | **Accepted** (2026-10-08) |
| `walker.md` | §11 distances, LOD classes, default-settings profile, camera states; §1–§10 Tessera design; §13–§14 M4.5 design | §11 **Accepted**; §13–§14 **Accepted** (2026-10-08; §13.6 and §14.5 2026-10-09, §14.6 2026-10-10); §1–§10 implemented M3–M4 design (only the M5 UI rules await review, outside M4.5) |
| `terrain.md`, `texture-list.md`, `mesh.md`, `entity-descriptor.md`, `setup.md`, `property-system.md` | Meshing; mip chains; detail levels; light properties and the draw rule; collision volumes and physics bounds; property values | all **Accepted**: `terrain.md` 2026-10-08, `texture-list.md` 2026-10-07, `mesh.md` 2026-10-08, `entity-descriptor.md` 2026-10-07, `setup.md` 2026-10-08, `property-system.md` 2026-10-07 |
| `coordinates.md` | Rotations and axes used by scenery and collision | **Accepted** (2026-10-07) |
| `render.md` | Tessera's renderer design; §4–§5 the M4.5 pipelines | §4–§5 and §7 **Accepted** (2026-10-10, review of the lava and flow-map revision); §6 (M5) Accepted |

Every spec above must be Accepted before M4.5 exits. None has a blocking `> **OPEN:**` item
at drafting time: each open point either has a Tessera Decision or is not needed offline.
On 2026-10-10 the 18 specs held 314 `> **OPEN:**` callouts, none marked blocking.

<a id="m4-5-re"></a>
**RE tasks** (open items from the specs' `> **OPEN:**` callouts. None blocks an exit
criterion, because each has a Tessera Decision; closing them improves fidelity. The
methods are those of §2, with method M4 under the standing authorisation of
[decision 0011](decisions/0011-standing-m4-authorisation.md) and numeric constants under
[decision 0012](decisions/0012-numeric-constants.md).)

- **Movement, by owner observation** (`collision.md` Q9, Q11; `water.md` §7.4): jump height
  and airtime; walk, run, backward, strafe and swim speeds; the highest step that can be
  climbed and the steepest slope that can be walked; the water depth at which wading turns
  into swimming; how a mover is tested against the liquid boxes. Also the integration of
  jump, ramp, control acceleration and damper, and the plane solver (method M4).
- **Collision, from data study and method M4:** the surface property that removes a draw
  from a mesh-level code (Q1); the plane-list terminators, portal lead byte and winding
  (Q2); the neighbour flag byte and whether a mover's cell is re-evaluated every step (Q3);
  load-time trees (Q4); what per-level collision meshes are used for (Q6); the exact
  ethereality test (Q7); bodies for client-only generated objects (Q12); terrain under the
  413,655 hole triangles whose ceiling samples are all 0 (`collision.md` §6.5).
- **Terrain texturing:** the reference landblock of the per-block texture offset (§6.2);
  the landscape-shadow target (§9.5); ceiling normals (§4.8); quad-grid weighting (§4.6);
  distance bands and the near/far seam (§12); the depth comparison of stacked layers (§8,
  also `lighting.md` §6); `slope_orientation`. Frame captures of the original are the
  first method.
- **Scenery and frills:** `kind` and `unknown_f` of terrain entries and the 0x11 choice
  `flag` (§4.2–§4.3); scene `flag_a` (§5.1); what fills the seed-A list (§6.3); the
  `spacing_3d` variant and the static-entity filter mode offline (§7.4); frill fade,
  terrain-colour blend and sway (§8.3); the frill pass's footprint markers (§8.2); the
  meaning of the eight LOD-class values and imposters (§9.1, `walker.md` §11.9:
  `DistantImposters`, prefix 0x27, `Render_FarViewDistance`); wind constants (§10); whether
  the kind-5 sector records feed frills (§15 item 12).
- **Water:** the near-water geometry builder, reflection pass, permutation choice, depth,
  flow and foam, draw order (§8.1, §8.4); the `extra` DIDs and `value` of water entries
  (§4.2); fog-zone modes (§5, moot in this install); the full kind-5 layout (§9,
  `landblock.md` §7).
- **Lighting:** the engine's sun-shadow values (colour, range, cascades, fade, §9); how sun
  colour and brightness form the main light and its specular colour (§8.1); how an
  object's light list is built, the run-time light scale and baked landscape lights
  (§8.3, §8.5); the two-colour-layer combine (§5.2); glow and bloom, cube environment
  maps; projector lights and point-light shadows; the channel order of property colours
  (§4.4).
- **Environment:** how the day type is set (§11.2); the remaining keyframe and sky-object
  fields and how tints and curves blend (§5.3–§5.4, §11.3–§11.4); the game-time fields and
  the clock epoch (§6); sky draw order, depth, scale and fog (§12); region-descriptor field
  4 (§3.1).
- **Landblock:** vertex-flag bits 1–6 (§3.6), the box list (§3.10), block-map key
  0x10000012 (§5.6), and how ceiling vertices at or below the floor are drawn (§3.5).
- **Material** (low priority): render-state byte 6, the use of flag 4007, edge softening,
  and the effect-family parameters for wind and snow (`material.md` §17).

<a id="m4-5-exit"></a>
**Exit criteria** (real-data criteria run with `TESSERA_GAME_DIR` set, release builds;
results recorded as pass/fail and counts)

1. **Decode.** Every record of each new kind decodes to its last byte, with no failures:
   - 0x19: 5 / 5, with 4,688 entries;
   - 0x12: 5 / 5 and 0x11: 5 / 5;
   - 0x02: 3,402 / 3,402;
   - 0x66: 16,819 / 16,819 and 0x01: 1,057 / 1,057;
   - 0x16: 5 / 5 (125 listed entries) and 0x17: 5 / 5;
   - 0x15: 1 / 1 (30,184 bytes), 0x1A: 963 / 963, 0x35: 1 / 1 (180 bytes), and 0x32: 1 / 1
     (32 classes);
   - 0x0F: 7 / 7 with all eight table slots.

   The M2 decoders still decode 0x31 13,781, 0x30 24,094 and 0x2B 510. The quantised-tree
   code of every host walks without an unknown opcode or an out-of-range jump (CO4): all
   0x66 records and cell physics meshes, the 58,546 terrain trees, the ceiling trees and
   the mesh levels. The M3 landblock tests still pass with the new height rule, and so does
   `reference_locations` at its tolerances (or the checklist gets an erratum with the
   recomputed values).
2. **Invariants** hold on the real data, each run as a gated test:
   - `terrain-texturing.md` §3.5 (1–6) and `scenery.md` SC7: every layer key is in its
     region's table.
     All 646,073 masks of drawn layers are A8. The 5,441 light-map properties name 5,422
     DXT1 surfaces;
   - `scenery.md` SC1–SC6, SC9 and SC10. The §12 census is reproduced: 11,921,024
     vertices with a group scene, 11,854,954 with a frill layer, and 7,295 without a scene;
   - `collision.md` CO1–CO11, including 5,310,848 ceiling triangles on the bit-0 diagonal
     and 2,750,134 hole triangles. The §9.3 blocker classification is reproduced (82,139
     undrawn mesh blockers among 97,688 undrawn blocking SEDs). Terrain render triangles
     equal collision triangles in all 390,150 landblocks;
   - `water.md` WA1–WA5: the §6.2 rule finds 89,913 wet landblocks (67,175 fully
     submerged), and all 12,925 distant sectors agree with it;
   - `environment.md` EV1–EV6.
3. **Determinism.** Scenery generation for all 78,114 landblocks with a non-zero coarse grid
   is run twice: with 1 worker and with the default worker count, in different landblock
   orders. The object lists (DID, position, rotation, scale) are identical bit for bit
   (SC8). The only exception is frill thinning at a density below 1, which is random by
   design. The synthetic worked examples are reproduced exactly as tests:
   - `scenery.md` §14 (hash values, orientations, the plane band, the clearance query) and
     SC11;
   - `collision.md` §14;
   - `water.md` §12;
   - `environment.md` §14;
   - `terrain-texturing.md` §14;
   - `lighting.md` §14.

   With the same inputs and fixed step, a controller run gives the same trajectory bit for
   bit. So do the water state and the blended sky values at a given game time.
4. **Walking on collision.** The scripted walks of `m4-5-visual.md` (at least 24, at spots
   the dirty room records as coordinates with expected outcomes) all pass in an automated
   `--walk-path` test, with no capsule penetration deeper than 0.01 m:
   - at least 4 walls of placed buildings block the walker;
   - at least 2 undrawn blockers block it;
   - at least 3 generated trees or rocks block it;
   - it climbs at least 3 steps no higher than the step-up height;
   - it slides on, and does not climb, at least 2 slopes steeper than walkable;
   - it enters at least 3 interiors through their doorways, reporting the expected cell;
   - it walks at least 2 cave floors under a ceiling;
   - in at least 3 water entries it wades, then swims at the depth of `water.md` §7.4;
   - at least 2 areas covered only by frills do not block it.

   A seeded soak of at least 1,000 random walks of 60 s simulated time each, spread over
   the land landblocks of all six regions, ends with 0 fall-throughs (feet more than 0.05 m
   below the terrain, or below the floor of the cell that contains them) and 0 non-finite
   states.
5. **Frame time** on the reference machine
   ([reference-machine.md](perf/reference-machine.md), its rule 1 for recording), with
   every M4.5 system on and the default-settings profile of `walker.md` §11.8:
   - The M3 baseline flights M3-a, M3-b and M3-d have 0 frames over 50 ms and no near-ring
     holes, and the residency budget holds. Scenery generation and collision building run
     on the workers.
   - At 5 walk spots from the checklist (dense forest with frills, a town, a lake shore, an
     interior, a vista at the full landscape extent), 60 s each, headless 1920 × 1080
     unpaced: p99 ≤ 16.7 ms.
   - M4 criterion 4 (50 animated characters at 60 fps) still passes with these systems on.
6. **Visual checklist.** Every row of `docs/checklists/m4-5-visual.md` passes on Tessera's
   renders. The rows cover:
   - terrain layers: at least 12 spots over the six regions, including roads,
     normal-mapped and specular layers, light-mapped blocks and ceiling chunks;
   - scenery: at least 10 spots;
   - frills: at least 5 spots;
   - water: at least 8, including sea level, rivers, lava, placed water and distant water;
   - sky and fog: 4 times of day under at least 3 sky keys, and interior fog;
   - lighting: at least 6, including sun shadows, point lights at night, emissive, glow and
     specular, and blended surfaces.

   As in M3 and M4, the owner then compares the rows with the original client at the same
   coordinates (and game time, where it can be set) and records the result. Screenshots
   stay private.
7. **Golden images.** Each new pipeline has a synthetic golden image that passes on Mesa
   lavapipe and on the reference GPU: a 3 × 3 splatted patch with masks and a normal map, a
   water patch, the sky at two times, a frill sector, and a lit mesh with a point light,
   shadow and fog.
8. **Fuzzing.** There is a fuzz target for each new decoder (0x19, 0x11, 0x12, 0x02, 0x66,
   0x01, 0x16, 0x17, 0x15, 0x1A, 0x35, 0x32, and 0x08 if done), for the tree reader, for
   scenery generation from hostile tables (cycles, depth cap, counts), and for a controller
   step against hostile geometry. `tools/fuzz-smoke.sh -t 60` finds nothing.
9. **Specs and docs.** Every spec in the table above is Accepted with no blocking OPEN.
   ARCHITECTURE §3.1, §10 and §20.3 are updated as listed in the deliverables.

<a id="m4-5-status"></a>
**Status (verified 2026-10-06 against the owner's install, release builds, rustc 1.99.0, on
the reference machine: the owner's laptop, i9-10885H, RTX 2070 Super Max-Q / Vulkan,
[decision 0010](decisions/0010-reference-machine-laptop.md)): criteria 1, 2, 3, 7 and 8
pass. Criteria 4, 5, 6 and 9 were re-verified on 2026-10-07 and again on main `01deafd` on
2026-10-10 (worktree `wt-m45v`, release builds, `TESSERA_GAME_DIR` set, `TESSERA_PERF`
unset; rows below): 4 and 9 now pass; 6 is not met (54 of 62 rows pass on the image, W10
fails, 7 rows cannot be judged from their cameras); 5 has an indicative M3-b pass and waits
for the owner's quiet-machine re-measure. M4.5 stays open.** Every deliverable exists:
the new `tessera-formats` decoders (0x19, 0x12, 0x11, 0x02, 0x66, 0x01 and the quantised
tree reader, 0x16, 0x17, 0x15, 0x1A, 0x35, 0x32, 0x0F with all slots, 0x08) and the landblock
additions; `tessera-terrain` layer resolution, masks, texture coordinates and ceilings; the
new crate `tessera-physics` (collision sets, capsule sweeps, the controller,
[decision 0013](decisions/0013-physics-crate.md)); `tessera-world` scenery, frills, water,
environment, lighting, draw distances and collision in the bundles; the `tessera-material`
curves, transforms, lighting inputs and water chain; the `tessera-render` lit, terrain-layer,
water, sky and instanced pipelines with mip chains and shadows; the walker on physics with
`--walk-path`, the clock controls, the per-system switches and the default-settings profile
(`walker.md` §13–§14); the explorer's world views and `tessera-dattool show` / `census`;
`docs/checklists/m4-5-visual.md`; owner items O18–O27 in
[owner-observations.md](checklists/owner-observations.md); ARCHITECTURE §3.1, §10 and §20.3.

> **Held work in the verified tree (2026-10-07).** The re-verification ran on the working
> tree as it stood, with the then uncommitted diffs of `crates/render/**`, `apps/client/**`
> (interior cells, layer-less blocks: `walker/interiors.rs`, `walker/terrain.rs`),
> `crates/physics/**` and `crates/world/src/scenery/**` / `frills/**`. The incident record
> [0018](decisions/0018-incident-review-notes-in-clean-prompts.md) held all four while it
> was open. The owner lifted the hold on 2026-10-07 (0018, owner decision A: release; the
> work landed in `ecc814b` and `6607d43`), so the results below that depend on those diffs
> (for example F1, Y5, SL1–SL3) count.

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `cargo test --release -p tessera-dattool --test real_data` (`census_decodes_every_world_record`): 0x19 5 / 5 (4,688 entries), 0x12 5 / 5, 0x11 5 / 5, 0x02 3,402 / 3,402, 0x66 16,819 / 16,819, 0x01 1,057 / 1,057, 0x16 5 / 5 (125 listed entries), 0x17 5 / 5, 0x15 1 / 1 (30,184 bytes, `environment_real_data`), 0x1A 963 / 963, 0x35 1 / 1 (180 bytes), 0x32 1 / 1 (32 classes), 0x0F 7 / 7 with all eight slots, 0x08 11 / 11; the M2 decoders still give 0x31 13,781, 0x30 24,094, 0x2B 510. CO4 tree walks with no unknown opcode or jump out of range: 16,819 0x66 trees, 1,010 cell physics meshes, 58,546 terrain and 1,095 ceiling trees, 1,280 embedded and 70,956 0x06 mesh levels (3 and 12 renumbered, as `collision.md` §3.5 lists). The M3 landblock tests and `reference_locations` pass with the new height rule (no erratum; largest change 0.030 m at M3-13). |
| 2 | ✅ | Gated real-data tests, all passing: terrain-texturing §3.5 (1–6) on 5 tables / 4,688 entries; SC7 on 390,150 records (912,038 layers, 88,807,424 quads, all keys); 646,073 drawn-layer masks, all A8; 5,441 light-map properties naming 5,422 DXT1 surfaces. SC1–SC6, SC9, SC10 (3,402 scenes, acyclic, longest path 5; 4,261 objects; 66,191 nodes) and the §12 census exactly (11,921,024 / 11,854,954 / 7,295). CO1–CO11: 5,310,848 ceiling triangles on the bit-0 diagonal, 2,750,134 hole triangles, CO3 393,842 reciprocal links, CO9 one movement record. §9.3: 82,139 undrawn mesh blockers among 97,688 undrawn blocking SEDs, reproduced when the content-layer filter is left out (88,749 with the shared-world filter of §9.3 as written; `collision.md` §9.3 OPEN). Terrain render triangles equal collision triangles in all 390,150 landblocks (and all 6,867 ceilings). WA1–WA5: 89,913 wet, 67,175 fully submerged, 12,925 / 12,925 distant sectors agree. EV1–EV6 hold. |
| 3 | ✅ | `determinism_real_data`: all 78,114 landblocks with a non-zero coarse grid built twice through the bundle store (1 worker ascending, 14 workers descending), water and collision included: 9,496,701 generated objects, 0 lists differ, 0 incomplete; `scenery_real_data`: 10,121,082 objects, 1 worker 49.3 s, 16 workers 6.9 s, 0 differ. Worked examples reproduced as tests: `scenery.md` §14 and SC11, `collision.md` §14.1–§14.8, `water.md` §12, `environment.md` §14, `terrain-texturing.md` §14, `lighting.md` §14 (CPU and GPU). Controller runs repeat bit for bit (`controller`, `controller_real_data`, the soak's digest); water state and blended sky values are pure functions of their inputs (tests in `tessera-world`). |
| 4 | ✅ (re-verified on main `01deafd`, 2026-10-10; rows GN1, GN2, CV2 re-derived by the implementer, confirmed by the spec writer 2026-10-09, `m4-5-visual.md` §2.4), owner observation pending | **Re-verified 2026-10-10 on main `01deafd`** (worktree `wt-m45v`, release, no code changed): `TESSERA_WALKS_STRICT=1 cargo test --release -p tessera-client --test walk_checklist_real_data` 2 of 2 tests pass, **32 of 32 walks**, every walk with 0.0000 m penetration (limit 0.01 m); a second run through the CLI (`tessera-client --walk-path docs/checklists/m4-5-visual.md`) also gives 32 passed, 0 failed. Per category (all at or above the minimum): walls 5 / 5 (first touch 0.001–0.009 m from P), undrawn blockers 3 / 3 (0.002–0.007 m), generated objects 4 / 4 (GN1 0.086, GN2 0.025, GN3 0.003, GN4 0.024 m, d = 0.15), steps 3 / 3 (end 0.036–0.042 m from E, every riser climbed), steep slopes 3 / 3 (slid; end 5.19–5.27 m against rmax 6 / 6.1), interiors 4 / 4 (end 0.002–0.007 m), caves 3 / 3 (CV1, CV2, CV3 end 0.039, 0.011, 0.033 m from E; CV1 and CV2 exceed dz = 0.05 in |feet − terrain| (0.133, 0.082 m) but stay within the slope allowance, as the test applies it), water entries 4 / 4 (held at h − fh with 0.000 m deviation), frill areas 3 / 3 (end 0.011–0.042 m, no contact). Soak (`walk_soak_real_data`): 1,000 walks of 60 s, 3,600,000 steps, 0 skipped, **0 failing** (0 fall-throughs, 0 non-finite states), deepest 0.0004 m below the terrain, regions 1–5 and 14, the same three landblocks with no valid start passed over (walks 69, 661, 859), 81.3 s; **digest 131068b3278ec1bf** (was b34a46aa6aa3754e on 2026-10-09; the change matches the later jump-path and water-state commits `afebc03` and `d3b1f79`). **Re-measured 2026-10-09 after the controller change** (worktree `wt-phys`, `TESSERA_WALKS_STRICT=1 cargo test --release -p tessera-client --test walk_checklist_real_data` with the install): **32 of 32 walks pass**, every walk with 0.0000 m penetration. The controller now follows `collision.md` §11.6 item 1 (target of an input pair), §11.8 (walk mode halves the command values: walking 2.5 m/s instead of about 4.9 m/s; quantised input; walking style), §11.9 (keyboard turn 150°/s) and §11.10 (jump impulse from the avatar's `Loc_Jumping` nodes read at run time: 6.0 m/s on land, 3.0 m/s in water, one step) (implementer OPEN in `collision.md` §12.5). Because walking is now half as fast, the durations of CV1–CV3 were raised from 4 s to 8 s (`m4-5-visual.md` implementer note before §2.4, for the spec writer to confirm); with the old 4 s they end 5.4 m short of E. No other row changed; the walker first touches as before (WL2 0.009 m from P, GN1 0.086 m, GN2 0.025 m, GN3 0.003 m, GN4 0.024 m), and CV1–CV3 end 0.039, 0.011 and 0.033 m from E. Soak (`walk_soak_real_data`, same tree): 1,000 walks of 60 s, 0 skipped, **0 failing** (0 fall-throughs, 0 non-finite states), deepest 0.0004 m below the terrain, the same three landblocks passed over; **digest b34a46aa6aa3754e** (was 7ca9ac480a86c563; it changes because walking segments move at half speed, diagonal and strafing targets follow §11.6 item 1, and jumps use the 6.0 m/s impulse instead of 5.2 m/s). **Re-measured 2026-10-08, second pass** (worktree `wt-walks2`, `TESSERA_WALKS_STRICT=1 cargo test --release -p tessera-client --test walk_checklist_real_data` with the install): **32 of 32 walks pass**, every walk with 0.0000 m penetration; no controller change. The two rows that failed were re-derived with Tessera's own code and the specs (`m4-5-visual.md` §1 TOUCH and CAVE, §8; both implementer OPENs of §2.4 answered, for the spec writer to confirm): GN2's stop point P now follows the first-touch rule of `collision.md` §11.7 (the trunk's steep foot 0.114 m above the feet stops the capsule and the 0.5502 m step-up fails): 40.97,46.19 → 40.97,45.98; GN1's moves the same way, 82.61 → 82.45; GN3, GN4, WL1–WL5 and UB1–UB3 keep theirs (within 0.005 m). CV2 runs 15 m down a walkable passage (at most 35.6°, headroom 4.11 m, no object within 3 m) of the same cave instead of along a 48–61° bank. Walker first touches: WL2 0.009 m from P, GN1 0.086 m, GN2 0.025 m, GN3 0.003 m, GN4 0.024 m (d = 0.15); CV2 ends 0.035 m from E. Every per-category minimum is met: walls 5, undrawn blockers 3, generated trees or rocks 4, steps 3, steep slopes 3, interiors 4, caves 3, water entries 4, frill areas 3. The test's known-open list is empty. Soak (`walk_soak_real_data`, 2026-10-08, same tree): 1,000 walks of 60 s, 0 skipped, **0 failing** (0 fall-throughs, 0 non-finite states), deepest 0.0004 m below the terrain, digest 7ca9ac480a86c563; the same three landblocks passed over. Noted for the `tessera-physics` owner (`m4-5-visual.md` §2.4, CV2 answer): walking CV2's line uphill, the walker slows to about 1.2 m/s at a quad edge (terrain of at most 35.6°, where `collision.md` §11.4 predicts at least 3.3 m/s). Comparison with the original: O18, O19. Earlier: 30 of 32 (2026-10-08 first pass, after §2.2 measured `blocked` at the first touch and allowed the capsule's rest offset on slopes; GN2 and CV2 failed), 25 of 32 (2026-10-07), 21 of 32 (2026-10-06). |
| 5 | ⏳ (M3-b indicative pass; owner's quiet re-measure pending) | **M3-b re-run once on 2026-10-07, indicative only** (other milestone builds ran beside it, load average 7.8–8.0): `--headless --size 1280x720 --fly-path 2:0:0,2:254:254 --speed 3000`, release: 1,140 frames, longest 37.1 ms, p50 / p99 16.7 / 20.4 ms, **0 frames over 50 ms, 0 frames with near-ring holes**, 9,565 bundles requested and integrated (0 failed, 0 evicted), peak 1,122 landblocks / 1,424 MiB bundles (lookahead included), streamer update at most 8.0 ms. The criterion is not claimed until the owner re-measures M3-b on a quiet machine. 2026-10-06: reference machine, release build of that tree, nothing heavy running besides a browser and btop ([reference-machine.md](perf/reference-machine.md)): walk spots P1–P5, `--headless --unpaced --duration 60 --size 1920x1080`, p99 10.8 / 14.4 / 10.0 / 14.3 / 7.8 ms, 0 frames over 50 ms; M4 crowd of 50 with everything on: 105.8 fps mean, p99 13.7 ms, 51 characters drawn every frame, 1 frame over 16.7 ms. M3 flights with every M4.5 system on (paced 60 fps): M3-a 0 over 50 ms (longest 28.8 ms), 0 holes, peak 1,111 MiB bundles; M3-d 0 over 50 ms (longest 34.5 ms), 0 holes, peak 2,684 MiB of the 4,096 MiB budget; **M3-b 0 over 50 ms (longest 25.8 ms) but one near-ring landblock missing for 5–8 frames near the end of the path in each of 3 runs** (bundles with collision sets and scenery arrive late at 3,000 m/s; `walker.md` §14.11 OPEN). Scenery, water and collision are built on the asset workers, frill sectors as worker tasks. |2026-10-06: reference machine, release build of that tree, nothing heavy running besides a browser and btop ([reference-machine.md](perf/reference-machine.md)): walk spots P1–P5, `--headless --unpaced --duration 60 --size 1920x1080`, p99 10.8 / 14.4 / 10.0 / 14.3 / 7.8 ms, 0 frames over 50 ms; M4 crowd of 50 with everything on: 105.8 fps mean, p99 13.7 ms, 51 characters drawn every frame, 1 frame over 16.7 ms. M3 flights with every M4.5 system on (paced 60 fps): M3-a 0 over 50 ms (longest 28.8 ms), 0 holes, peak 1,111 MiB bundles; M3-d 0 over 50 ms (longest 34.5 ms), 0 holes, peak 2,684 MiB of the 4,096 MiB budget; **M3-b 0 over 50 ms (longest 25.8 ms) but one near-ring landblock missing for 5–8 frames near the end of the path in each of 3 runs** (bundles with collision sets and scenery arrive late at 3,000 m/s; `walker.md` §14.11 OPEN). Scenery, water and collision are built on the asset workers, frill sectors as worker tasks. |
| 6 | ❌ Tessera side (re-judged on main `01deafd`, 2026-10-10: 54 of 62 rows pass on the image, W10 fails, T1, S5, S7, W2, W15, W16, Y3 not judgeable from their cameras); owner observation pending | **Re-judged 2026-10-10 on main `01deafd`** (worktree `wt-m45v`, release, no code changed): `cargo test --release -p tessera-client --test visual_rows_real_data -- --test-threads=1` with the install and `TESSERA_VISUAL_SHOTS`: 11 of 11 tests pass (T1–T16 stacks, S1–S11, F1–F6, W1–W8 heights, Y1–Y5, L2; all 24 test cameras settled; water fallback checks W7, W8, W10–W17 clean, W17 brightness ratio 1.04). The other rows were rendered with `tessera-client --screenshot` (1600 × 900, free-fly, clock stopped) from the checklist cameras: 50 row images and 37 comparison images (`--disable` of layers, scenery, frills, water, shadows, lights or fog; top-down views), all settled, private and deleted after judging. Per group (pass / fail / not judgeable): T 15 / 0 / 1, S 9 / 0 / 2, F 6 / 0 / 0, W 13 / 1 / 3, Y 4 / 0 / 1, L 7 / 0 / 0. L5 (corrected row, 0xD7E8) passes: round the mirror point +18 / 255 brighter than from the mirrored camera, a faint white sheen. L7 (redefined row, 0x7873) passes: an orange pool round the fire, gone with `--disable lights` (red 20 → 48 along a line through it, blue about 6). T15, T16, L2 and Y5 as on 2026-10-08. **Fails:** W10, the waterfall sheet has the streaked texture but its outline is hard-edged at both sides and the top (57 → 163 within about 4 px) where the row expects soft edges; `water.md` §8.5.3 gives the soft-edge mask only for 0x310004E6 and 0x310004E7, so the outer sheet 0x310004E8 may be hard-edged in the data (spec writer to check). **Not judgeable:** W15 and W16 check drift speed, which a still cannot show (only the test's still-scroll check covers them; the look of both is right); T1 (a roof between eye and target; from a top-down camera at 12 m the cobblestone road passes), Y3 (walls fill the view; from above the roofs the evening sky, sun and fog match), W2 (reeds at the eye; with frills and scenery off the sea is drawn), S5 (a placed plant at the eye; the data test passes, 151 objects), S7 (a hump hides the two nearest objects; the data test passes, 445 objects). Stale test cameras: `CAMERAS` in `apps/client/tests/visual_rows_real_data.rs` still holds the voided L5 (0x7FBD) and the old L7 (0x9FE8), so the corrected rows were rendered through the command line. Seen outside the rows (owner to compare): L3's ember mesh looks like an upside-down pyramid under the logs; figures stand with arms straight out in L5, T11 and T12; a thin horizontal line crosses the sky in W9; W2's far sea is near black. Before (spec writer 2026-10-09, `m4-5-visual.md` §4.6–§4.7): L5 corrected and T6 moved, because a drawn platform covers their former spots, and L7 redefined. **Re-judged 2026-10-08** (worktree `wt-walks2`): the nine rows that failed or could not be judged on 2026-10-07 got cameras of their own (`m4-5-visual.md` §4.7; table `CAMERAS` in `apps/client/tests/visual_rows_real_data.rs`, rendered by `row_cameras_render` with `TESSERA_VISUAL_SHOTS`, 1600 × 900, free-fly, no HUD, clock stopped; private images, viewed and not committed). **Now pass on the image:** T4 (eye east of the target, looking into a court of placed objects: one tiled paving layer), T5 (from the north: paving and grass layers round the target, which lies under a curb at a plinth's foot; dim by its day description 0x1A00022B, the light map is black), T12 (from the east, past the rock: blended dirt layers), T15 and T16 (the row's cameras: near-black ceilings are what `terrain-texturing.md` §4.8/§4.10 give with these caves' downward ambient; T16's brightness matches *Y* alone, ratio to a `--disable sky` image (0.060, 0.062, 0.076) against (0.052, 0.055, 0.070); stretched images and the `--disable sky` images show the ceiling layers), Y2 (facing south: red dawn sky and fog, north faces sunlit), L2 (top-down over light #460: the point lights add orange to objects only, zero on terrain; with them off a light-map pool remains, red 66 at 1 m → about 20 from 8 m on, blue constant), L5 under its revised row (faint white sheen of about +20 / 255 towards the sun on sunlit layer-67 paving against the mirrored camera). **L5 diagnosis:** row definition, not code: the shader follows `terrain-texturing.md` §9.4; the row expected a broad highlight, which §9.4 (T44) rules out, and from its eye the sun's mirror point lay in a building's shadow (*S* = 0); eye and expectation revised in `m4-5-visual.md` §4.6 for the spec writer to confirm. **Not judgeable:** L7 (row definition: its light map 0x4116792D is black and the block's day description gives the same lighting at *t* 0 and 0.5; OPEN in `m4-5-visual.md` §7). Pass on the image: 53 of 54 rows (2026-10-07: 45). `visual_rows_real_data` with the install: 8 of 8 tests pass (`row_cameras_render` run once with `TESSERA_VISUAL_SHOTS`: 14 images, all settled). Observation outside the rows: near Y2's stand point window and door frames stand in the air without walls (owner to compare). Earlier (2026-10-07): S1, S5, S6, S8, S10, F1, Y5 passed on the image after the scenery re-derivation and interiors. Comparison with the original: O20, O23–O27. |
| 7 | ✅ | `terrain_layers.png` (3 × 3 splatted patch with masks and a normal map), `water.png`, `sky_morning.png` / `sky_evening.png`, `frill_sector.png` and `lit_mesh.png` (point light, shadow, fog), plus `lit_surfaces.png`: all pass on the RTX 2070 Super Max-Q (Vulkan) and on Mesa lavapipe 24.2 (`VK_DRIVER_FILES`, `TESSERA_REQUIRE_GPU=1`: 44 golden tests over the 8 golden suites, old images unchanged). |
| 8 | ✅ | `tools/fuzz-smoke.sh -t 60` (stable libFuzzer path) on the 25 new M4.5 targets and the 5 changed ones (`formats_block_map`, `formats_landblock_data`, `formats_landblock_properties`, `formats_records`, `world_bundle`): no crash; 13 k (`explorer_collision_overlay`) to 25.9 M (`formats_lod_classes`) executions. Every new decoder (0x19, 0x11/0x12, 0x02, 0x66, 0x01, 0x16, 0x17, 0x15, 0x1A, 0x35, 0x32, 0x08, 0x0F), the tree reader, scenery generation from hostile tables (`world_scenery`) and a controller step against hostile geometry (`physics_controller_step`, re-run after the controller fixes: 82 k executions, ok) has a target. |
| 9 | ✅ (2026-10-10) | **Re-verified 2026-10-10 on main `01deafd`:** the header Status of all 18 specs of the [specs table](#m4-5-specs) is Accepted for the M4.5 parts: `terrain-texturing.md`, `scenery.md`, `collision.md`, `water.md`, `material.md`, `lighting.md`, `environment.md`, `landblock.md`, `records.md`, `terrain.md`, `texture-list.md`, `mesh.md`, `entity-descriptor.md`, `setup.md`, `property-system.md`, `coordinates.md` (2026-10-07 to 2026-10-10); `walker.md` §11 and §13–§14 (§13.6, §14.5, §14.6 included; 2026-10-08 to 2026-10-10; only its M5 UI rules await review, outside M4.5); `render.md` §4–§5 and §7 (2026-10-10, review of the lava and flow-map revision) and §6 (M5). The 18 specs hold 314 `> **OPEN:**` callouts, none marked blocking. Before: **re-verified 2026-10-07, ❌.** Review records are now present for the two implementer design sections: `walker.md` §13–§14 (first review and an independent re-review, both 2026-10-07: **changes requested**, 18 items, among them wrong unqualified section references, unsourced cell figures in §13.3, an unlabelled departure from `collision.md` §12.3 / §5.8.1 in §13.4, water's light selection against `water.md` §8.2, the §14.5 fade box against `mesh.md` §8.6, the hard-coded 80 m character distance, no provenance for §13–§14, unlabelled Tessera choices, and dated snapshots in §13.8 and §14.10a) and `render.md` §4–§5 (first review and re-review 2026-10-07: **changes requested**). Neither is Accepted. Since 2026-10-06, ten specs of the table have left Accepted for Revised with changes requested or a pending re-review (`terrain-texturing.md`, `scenery.md`, `collision.md`, `lighting.md`, `landblock.md`, `mesh.md`, `setup.md`, `entity-descriptor.md`, `property-system.md`, `coordinates.md`; [specs table](#m4-5-specs)). ARCHITECTURE §3.1, §10 and §20.3 remain updated. |

Repository checks, re-verification of 2026-10-07 (rustc 1.99.0, the working tree described
in the note above the table): `cargo fmt --check` passes (workspace and `fuzz/`); **`cargo
clippy --workspace --all-targets -D warnings` fails** (with and without `--all-features`) on
one lint, `clippy::neg_cmp_op_on_partial_ord` at `crates/physics/src/controller/step.rs:1332`
in the held physics diff (not changed by the verifier); with that lint allowed the workspace
has no other warning, and `fuzz/` (`--bins`) passes. `tools/check-content.sh` and its
self-test and `tools/check-spdx.sh` pass. `cargo test --workspace --release --no-fail-fast`
with `TESSERA_GAME_DIR` set: **2,613 passed, 1 failed, 5 ignored**; the failure is the
load-sensitive `flight_across_a_region` (1 frame over 50 ms, longest 58.4 ms, load average
about 25 from other milestone builds); run alone right after (load average about 8) it
passes (longest 46.5 ms, 0 over 50 ms). Four libFuzzer `slow-unit-*` files lie untracked in
the repository root and should be removed. `cargo doc` and `cargo deny` were not re-run.

Repository checks on 2026-10-06: `cargo fmt --check` (workspace and `fuzz/`), `cargo clippy
--workspace --all-targets --all-features --locked -D warnings` and on `fuzz/`, `cargo doc
--workspace --no-deps -D warnings`, `cargo deny check bans licenses sources` (both
manifests), `tools/check-content.sh` and its self-test, `tools/check-spdx.sh` and the relative
links and anchors of the Markdown docs pass. `cargo test --workspace --release` with
`TESSERA_GAME_DIR` set: 2,556 passed, 5 ignored, 1 failed (`flight_across_a_region`, 3 frames
over 50 ms while two other walkers ran beside it; it passes alone and its binary now runs
its walkers one at a time); after the fixes, `cargo test --release` of the changed crates
(physics, render, client, world) with the install: 752 passed, 2 ignored, the same flight
test failing once more under load (7 frames over 50 ms with at most 27.5 ms of main-thread
work) and passing in 5 later runs on a quieter machine, so it stays load-sensitive; `cargo
test --workspace` (debug, no install): 2,562 passed, 0 failed, 4 ignored.

Fixed during this verification: the fuzz targets' clippy lints; a trademark identifier in a
physics test; F2's near-white frills (`WorldSurface::gloss_from_normal_alpha`); controller
faults at terrain creases (FR3) and chamfered step edges (ST2), and movers diving below the
float line after sliding into water (`collision.md` §12.3 OPEN (n)–(p)); rustdoc links to
private items in the walker; the `fuzz/README.md` row for `client_world_fx`; the dattool rows
of ARCHITECTURE §24; the owner items O18–O27 completed against the planner's list.

Remaining for M4.5 to close (after the re-verification on main `01deafd`, 2026-10-10):
criterion 6, that is W10 (soft edges of the outer waterfall sheet 0x310004E8; the spec
writer checks `water.md` §8.5.3 and the row) and the seven rows that cannot be judged from
their cameras (T1, S5, S7, W2, Y3 need new cameras or rows; W15 and W16 check drift speed
and need a judgement from motion, not a still); the stale L5 and L7 entries of `CAMERAS` in
`visual_rows_real_data.rs`; the owner's quiet-machine re-measure of M3-b (criterion 5);
then the owner's observations O18–O27. Criteria 4 and 9 pass; the soak digest is now
131068b3278ec1bf. Earlier blockers (held diffs, the physics clippy lint, GN2 and CV2, L5,
the reviews of `walker.md` §13–§14 and `render.md` §4–§5, the ten Revised specs) are
resolved.

<a id="m4-5-particles"></a>
**Particle effects (outside the deliverables above; branch `particles-runtime`,
2026-10-09).** `particles.md` (Accepted) §8 and `particle-runtime.md` (Accepted) are
implemented in `tessera-particles` (simulation and run-time rules), the renderer's particle
pipeline, the explorer's preview and, in the walker, for the Particle nodes of the start-up
scripts of static entities and placed figures (started by `tessera-scriptgraph`; attached
and dropped paths, attachment frames, lingering, housekeeping, draw order, distance fade;
`apps/client/tests/particles_real_data.rs` draws particles at M3-03 and settles). The specs
are **not implemented end to end**: fork effects (0x44) are not drawn (`particles.md` rule
§10.2, Q1); systems of the post-processing pass are not drawn (the walker has none); weather
systems and the global wind (Q13), beams (Q-RT4), the gameplay event handler (Q-RT5), the
lights of lit systems (Q8 (b)), `inherit_opacity`, driven node fields and bounds culling
(Q-RT3) remain; the walker's own choices are recorded as implementer OPENs in
`particle-runtime.md` §3.2, §5.4, §9.2 and §12.

---

<a id="m5"></a>
## M5: UI layouts, skins and Lua plugins

**Goal:** draw the game's own UI panels from layout records, apply user skins, and run
existing third-party UI plugins that the user has installed.

**Deliverables**

- `tessera-formats`: UI layout records (elements, states, per-element data variants).
- `tessera-ui`: retained widget tree, absolute layout with anchors and margins, art tiled at
  native size and text drawn into per-surface CPU canvases (composed by `render` in a UI
  pass, decision 0015), focus and input routing, skin application.
- `tessera-plugins`: `.plugin` discovery, apartments, import resolution, event dispatch,
  plugin data persistence in Tessera's directories, a first API surface (engine, shell
  commands, chat output, plugin data, UI controls and windows, read-only views of the local
  player that the sandbox backend can provide), the clean-room class helper (decision
  0016). The split of `crates/script` leaves `tessera-script` a plugin-free Lua core, with
  no plugin API namespace, `import` or chat state (decision 0023).
- `tessera-client`: native panels for the walker (minimap frame, chat window, a few
  windows), plugin manager window, `/plugins` commands.
- Optional: importer for the original keymap file if its spec exists (read-only).

**Specs needed:** `ui-layout.md`, `skin-definition.md`, `plugin-descriptor.md`,
`lua-plugin-api.md` (names, signatures, enums, events, behaviour), `plugin-runtime.md`,
`plugin-api-usage.md`, `plugin-class-helper.md`, `ui-behaviour.md`, `ui-widgets.md`,
`game-panels.md`, `chat-window.md`, `fonts.md` (if font records are decodable),
`keymap.md` (import section), the `render.md` UI pass; decisions 0014–0016 and 0023.

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
6. `docs/LEGAL.md` records the plugin-namespace interoperability rationale (LEGAL §17,
   which replaces the former ARCH §18.2 OPEN).
7. **UI budgets.** U1–U5 of decision 0015 §6 hold, measured with its scripted run under
   `docs/perf/reference-machine.md`, at 3840 × 2160 fullscreen and at the window size the
   compositor gives. Frames with a scroll step or a new chat line (content-event frames) are
   judged by U4 (b) and (f), not by U1. U2 is a gate only where the adapter has timestamp
   queries. U1's walking-phase mean is accepted only after the painter probe has measured
   Mix colour and the erase (0015 §6, "Probe gap").

<a id="m5-status"></a>
**Status (exit verification 2026-10-08 against the owner's install, release builds, on the
reference laptop of [decision 0010](decisions/0010-reference-machine-laptop.md) while it was
loaded by other agents; branch `m5` at `a292f17` plus the uncommitted M5 changes): criteria
1, 3, 5 and 6 pass; criterion 2 is done on Tessera's side and waits for the owner's
screenshots; criterion 4 waits for the owner's plugin install; criterion 7 is not met
(indicative measurements).** Every deliverable exists: the UI layout (0x22) and font (0x42)
decoders in `tessera-formats`, the UI image decoders in `tessera-material::ui_image`,
`tessera-ui` (element tree, layout, widgets, behaviour, the input bridge's UI side, CPU
canvases and the paint scheduler, text and the bundled fallback font, user skins, game and
chat widgets), the UI pass in `tessera-render` (`render.md` §6), the 0014 input bridge and the
read-only importer of the original keymap file in `tessera-input`, the 0023 split
(`tessera-script` a plugin-free core with no dependency on another Tessera crate;
`tessera-plugins` with the host, the plugin API's UI classes and the clean-room class helper
of decision 0016), and in `tessera-client` the native panels P1–P15, the chat window, the
plugin manager window, `/plugins` commands, the UI host and the U1–U5 scripted run.
Specs used: those listed above, plus decisions 0014–0016 and 0023. **Two specs the
implementation follows are not Accepted:** `plugin-class-helper.md` (status Review; its third
review is outstanding) and `render.md` §6 (status Review; no review recorded). Both need their
review before the code that rests on them is committed.

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `cargo test --release -p tessera-formats --test ui_layout_real_data`: 302 / 302 UI layout records decode (2,610,590 bytes, all in the English local archive). `ui-layout.md` §13 categories: 0 records fail to decode, 0 unknown media kinds, 0 base layouts missing, 3 base elements missing from their base layout's top level (in 2 records), 0 base-chain cycles, 0 missing referenced records, 2 missing string-table tokens; widget types are left to `tessera-ui` (35 fallback game types, `ui_real_data`). The §11 census matches the spec (3,144 top-level elements, 16,421 elements, 12,977 states, 35,615 element and 2,083 state property entries; 27,512 non-zero references, all present); 59,067 ID fields all carry an enum label. `font_real_data`: 48 / 48 font records decode, 28,315 glyphs, no §3.3 or atlas issue. |
| 2 | ✅ data-side, owner observation pending | `cargo test --release -p tessera-client --test panels_real_data --test ui_panels_real_data` (and `tessera-ui` `ui_real_data`): all 15 panels of `ui-layout.md` §17 open from the install by label, every root rectangle equals its §17.3 value at 1024 × 768, 1920 × 1080 and 2560 × 1440 (15 / 15; P15 against the spec, the checklist's P15 rows are stale), 2,904 elements created with 0 resolve failures and 0 refused creations, the per-panel census (elements, text elements, states, art surfaces, fonts, string references) matches §17.2, the object-mode census matches UL22 (92 = 86 Stretch + 6 CanvasResize), Escape opens the main menu, `/plugins manager` opens P12. Tessera's own screenshots (`--ui` HUD set; `--open-panel` P6, P7, P10, P12, P15 at 1920 × 1080; private scratch, deleted) were viewed in this verification and agree with the "Tessera-side self-check" of [m5-visual.md](checklists/m5-visual.md): art at native size, frames closed, labels the game code would fill are empty. The comparison with the original client is owner observation O39. |
| 3 | ✅ (Tessera side) | `cargo test --release -p tessera-client --test ui_skin_real_data`: a generated skin (CC0 one-colour TGA files in a temporary skins root) changes the checked art of 13 / 13 panels that have skinnable art in their default view (P11 and P14 have none); the panel override moves P6 to (40, 30, 240, 290); a missing skin file shows the placeholder; turning the skin off restores the art. `tessera-ui` `skin_real_data`: the skin tables of the install load (5,625 art-asset labels, 4,742 base-table entries, SI1–SI9 hold). Optional cross-check with a published skin: O41. |
| 4 | ⏳ owner install pending | No `Plugins` folder exists on the owner's machine (`compat_real_plugins` reports it and passes as a skip). Tessera's synthetic plugin suite passes: `tessera-plugins` `compat_plugins` 12 / 12 (plugins load, create windows, respond to clicks, hover, typing, keys, timers and shell commands), `ui_classes`, `ui_controls`, `ui_input`, `ui_lifetime`, `class_helper` 37 / 37, and `tessera-client` `plugins_suite` 3 / 3. The current incompatibility table (18 rows) is in the module docs of `crates/plugins/tests/compat_plugins.rs`; the visible ones: game-styled windows use a neutral frame (no call yet to build a plugin control from a based layout element), check boxes draw no mark, quickslots draw no icon, `ControlList:Insert`/`Set` append. The 14 plugins of `plugin-api-usage.md` §9: owner observation O40. |
| 5 | ✅ | `cargo test --release -p tessera-plugins --test escape`: 6 / 6 (path traversal, absolute paths, symlinks, `import` tricks, image paths and plugin-data keys refused; C modules and bytecode refused; memory limits against string, table, `gsub` and format growth; instruction limits against loops, `pcall`/`xpcall` handlers, coroutines, patterns, metamethods and finalizers, per tick and at close); `ui_escape` 7 / 7 (UI-layer paths, refused image formats, hostile numbers, object caps); `sandbox` 6 / 6. |
| 6 | ✅ | [LEGAL.md §17](LEGAL.md#17-plugin-api-names-interoperability-record) records the plugin-namespace interoperability rationale and its binding rules. This verification enforced §17.2 rule 1 in the code: the namespace module of `tessera-plugins` and its bindings got neutral names (`root_ns`, `ns_root`); `tools/content-allowlist.txt` now allows the original names only in `crates/plugins/tests/` (the stale `crates/script/*` entry is gone, so the plugin-free core is checked). |
| 7 | ❌ not met (indicative) | [docs/perf/m5-ui-budgets.md](perf/m5-ui-budgets.md), three runs of the scripted run at 3840 × 2160 and 1920 × 1080 offscreen on the loaded reference laptop. Third run (this verification, 3840 × 2160, load 8–13): U1 walking mean 0.89 ms, p99 8.4 ms (budget 0.3 / 1.0 ms) **fails**; U4 (b) 12.8 ms and U4 (f) 6.1 ms (4 ms) **fail**; U4 (a) 33.5 ms passes here but was 54.0 ms on the second run; U3 (steady mean 72 KiB) and U5 (canvases 45 MiB, textures 90 MiB, images 11 MiB) **pass**; U2 is a proxy only (no timestamp queries in `tessera-render`, `render.md` §6.16 OPEN); U4 (c)–(e) are not measured. The time is in `tessera-ui`'s step 8 (`Ui::paint`). The painter probe (`crates/ui/benches/paint_probe.rs`) has measured Mix colour (1.4 ns/pixel) and the erase. Idle re-measurement and the compositor-window run: O49. |

Repository checks in this verification: `cargo fmt --check` (workspace and `fuzz/`), `cargo
clippy --workspace --all-targets --all-features --locked -D warnings` and `cargo clippy
--bins --locked -D warnings` in `fuzz/`, `cargo doc --workspace --no-deps` with `-D
warnings`, `cargo deny check bans licenses sources` (both manifests), `tools/check-spdx.sh`
(914 files, 0 errors) and the relative links and anchors of the Markdown docs pass. `cargo
test --workspace`: debug without `TESSERA_GAME_DIR` 2,777 passed, 1 failed (a regression of
this verification's rename, fixed and re-run: `class_helper` 37 / 37), 6 ignored; release
with `TESSERA_GAME_DIR` 2,777 passed, 0 failed, 6 ignored (210 test binaries).
`tools/fuzz-smoke.sh -t 60` (stable libFuzzer) on the 22 new or changed M5 targets: no
crash, from 15,772 (`ui_paint`) to 11.4 M (`render_ui_entries`) executions; the seven
targets later touched by lint fixes re-ran 15 s each without a crash. **`tools/check-content.sh`
fails** on one file: `crates/ui/assets/fonts/NotoSans-Regular.ttf` (`[ext]`, `[magic]`), the
bundled fallback font of `fonts.md` §11 (SIL OFL 1.1). Shipping it needs an owner decision
first: an allowlist entry for the font check of `tools/check-content.sh`, a REUSE annotation
with `LICENSES/OFL-1.1.txt`, and an exception to the "third-party material must never be
committed" note of `REUSE.toml` (recorded in `docs/LEGAL.md`); otherwise the font must be
loaded from the system instead. **Settled 2026-10-08** by decision 0022 (owner): the font is
bundled unmodified under OFL-1.1, with `LICENSES/OFL-1.1.txt`, a `REUSE.toml` annotation, an
allowlist entry for exactly that path and the exception in `docs/LEGAL.md` §11;
`tools/check-content.sh` now passes.

Fixed during this verification (mechanical changes only, no behaviour change): neutral
module and binding names in `tessera-plugins` and its client call site (LEGAL §17.2 rule 1),
the content allowlist, two rustdoc links, `is_multiple_of` and two other clippy lints in nine
fuzz targets, rustfmt on six fuzz targets, eight missing `fuzz/README.md` rows, the
`plugins_commands` leak-check exemption in `tools/fuzz-smoke.sh`, owner observations
O39–O49.

Remaining for M5 to close: owner observations O39 (criterion 2) and O40 (criterion 4); the
U1/U4 work in `tessera-ui` step 8, U4 (c)–(e) figures, timestamp queries for U2 and the idle
re-measurement O49 (criterion 7); the reviews of `plugin-class-helper.md` and `render.md` §6;
the P15 rows of `m5-visual.md`. Open review items the implementers
could not fix in their own files: the class helper's `Turbine.Log` clock defaults to UTC
(`crates/plugins/src/host.rs`, client `LogHost`); plugin TextBox read-only flag, caret and
selection API, child insert position and title bars above a window's top edge need
`tessera-ui` calls; `ImageRef::File` needs an origin; a `UIScale_Global` setter; the
`PluginData.Load` main-thread read waits on the `lua-plugin-api.md` §10.1 OPEN (ARCHITECTURE
§21).

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

<a id="m6-status"></a>
**Status (checked 2026-10-07 against the owner's install, release builds, on the reference
laptop while other milestone builds loaded it, [decision 0010](decisions/0010-reference-machine-laptop.md)):
criteria 1 and 3 pass, criterion 2 passes on Tessera's side (owner observation pending),
criterion 4 passes as an indicative measurement (idle re-measurement pending).** Every deliverable exists: `tessera-formats`
sound payloads, sound infos and script-table sound nodes; `tessera-scriptgraph` (the script
graph runtime, `script-runtime.md`); `tessera-audio` with cpal output behind the
`audio-device` feature and a project-owned mixer ([decision 0019](decisions/0019-audio-backend.md),
not kira), streaming music, positional emitters, channels and volumes, the ABC reader
(`abc`) and the instruments with synchronised start (`instruments`); `tessera-client`
world audio, `[audio]` settings, and the player-music commands `/play <name> [<part>]
[sync]`, `/playstart`, `/playlist [<filter>]`, `/playstop` and the debug `/instrument`
(`apps/client/src/audio/player_music.rs`). Specs used: `sound.md`, `script-table.md` (sound
nodes), `script-runtime.md`, `abc-dialect.md`, `instruments.md`. Commands run with
`TESSERA_GAME_DIR` set to the read-only install and `CARGO_BUILD_JOBS=6`.

| # | | Evidence |
|---|---|---|
| 1 | ✅ | `cargo test --release -p tessera-audio --test decode_real_data -- --nocapture` (final verification, 2026-10-07): 44,337 sound records (`client_local_English.dat` 29,193, `client_sound.dat` 12,621, `client_sound_aux_1.datx` 2,523); **44,329 of 44,329 audio payloads decode (100.00 %, full sweep)**: Ogg Vorbis 40,874, WAV 3,455; 0 failures, 0 invariant violations (SN1–SN4, SN3a), 8 DirectMusic and 0 unknown records counted apart, 155.6 h of audio; played ranges as the spec's census (3,811 streams with *A* > 0, 10,613 with *M* < *N*, 15 ending with an empty packet); 1,521 s wall time on 8 threads under a load average of about 45 (indicative). The 2 payloads an earlier hardening check refused (`0x0A016904`, `0x0A0011DC`) carry a placeholder codebook with no used entry as a residue stage book that their packets never read; the setup walk now accepts it and hands the decoder library a header in which that book has a 1-bit codeword, so a hostile packet that reads it cannot panic the library (`crates/audio/src/decode/setup.rs`, `patch_unused_books`; tests `unread_codebook_without_used_entries_is_accepted`, `codebook_without_used_entries_is_read_without_panic_or_rejected`, `patch_inserts_a_codeword_at_each_unused_book`). |
| 2 | ✅ data-side, owner observation pending | Rows A1–A5 of [m6-audio.md](checklists/m6-audio.md) pass: headless walker, null backend, region-1 row 120 from landblock 122 to 132 crosses two area borders; `Area_Music` changes at both, the old tracks ramp out over their own `RampOutTime` (4.0 s and 2.0 s), the new tracks play 16–17 ms after each change, 0 underruns. Automated: `cargo test --release -p tessera-client --test audio_real_data` (3 / 3 pass). Final verification (release, under a load average of about 50): `Area_Music` changes at 4.55 s and 9.88 s, ramp-outs of 4.0 s and 2.0 s, new tracks playing 35 ms and 49 ms after each change. Row O1 (does the cross-fade sound like the original's) waits for the owner: [owner-observations.md](checklists/owner-observations.md#o28) O28.1. |
| 3 | ✅ | `cargo test --release -p tessera-audio --test abc_conformance`: **156 / 156** rows of `abc-dialect.md` §12 pass (accepted, ignored, truncated and refused files as the spec says; note-on times within 1 µs); `--test abc_examples` 33 / 33 (§13 worked examples, §14 invariants). End to end in the client: `cargo test --release -p tessera-client --test player_music_real_data` (2 / 2): start-up builds the instrument table (24 instrument types, 4,760 sound nodes) and finds the message table through the named-string-table map in 0.1 s; an original test tune played with `/play` on the offline backend schedules 10 note-ons at 0, 0.6, 1.2, 1.8, 3.0, 3.6 (×3, a chord), 4.2 and 4.8 s from a start 0.7 s after the command was processed, and renders them with 0 late, 0 missing samples and audible output; `/play … 2 sync` waits until `/playstart` and then starts its 8 note-ons 0.7 s later; `/playstop` silences a running performance; a refused file shows the entry of the user's string table with line and column; the instrument comes from an `--avatar` instrument item (§4.5) or `/instrument`. `--test instruments_real_data`: the table agrees with a live tree walk for 61,440 combinations and a range tune plays through all 23 instruments plus the default set (960 note-ons, 745 distinct samples decoded). |
| 4 | ✅ indicative | [m6-audio.md](perf/m6-audio.md): a 603 s, 36.8 km headless flight with the device backend (cpal on ALSA through PipeWire, 48 kHz) gave **0 device underruns**, 0 mixer overloads and 0 starved stream blocks over 56,586 mixer callbacks (mixer load mean 4.3 %, p99 14.7 %), 18 area-music changes, main-thread audio 0.035 ms per frame. Made while other builds loaded the laptop (load average 3.1–4.3): to be re-measured on an idle reference machine before the criterion is closed (decision 0010). |

Repository checks at the same time: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings` and `cargo test --workspace` (2,423 passed, 0 failed, 2
ignored, debug build, without `TESSERA_GAME_DIR`) pass; the M6 real-data tests in release
with `TESSERA_GAME_DIR` set pass except `decode_real_data` (criterion 1): `tessera-formats`
`sound_real_data` 2 / 2 and `script_table_real_data` 4 / 4, `tessera-scriptgraph`
`real_data` 2 / 2, `tessera-audio` `instruments_real_data` 1 / 1, `tessera-client`
`audio_real_data` 3 / 3 and `player_music_real_data` 2 / 2. `tools/fuzz-smoke.sh -t 60
client_player_music` (stable libFuzzer): 27.0 M executions, no finding.

Remaining for M6 to close: the idle re-measurement of criterion 4 (decision 0010); the
owner observations in [owner-observations.md](checklists/owner-observations.md#o28) O28
(O28.1 is criterion 2's listening comparison, m6-audio.md row O1; the others check
`sound.md` §16, `script-runtime.md` §26, the ABC checks OC1–OC10 of `abc-dialect.md` §17 and
Q1, Q2, Q4–Q6 of `instruments.md` §16), and a listening check of `/play` with device output;
`script-runtime.md` is still in Review status (its OPEN items carry Tessera Decisions). The client has no chat input yet, so the
player-music commands reach it through `--music-command "<line>"` (repeatable, run in order
once the archives are open; replies go to the chat log), for example `tessera-client
--audio device --music-command "/play <name>"` (built with `--features audio-device`) with the song in the settings folder's
`Music` or `<data dir>/Music`. Commands are processed strictly in order (a `/playstop` or
`/playstart` given during a `/play`'s read acts after it; `abc-dialect.md` §11 OPEN
(implementation) (g)); unit tests with a synthetic instrument table cover solo and synced
starts, stop, ordering and the sample budget without game data. Implementation OPENs from this milestone
(client commands, instrument choice, the player music's own engine instance) are in
`abc-dialect.md` §11 and `instruments.md` §12.

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

<a id="m7-status"></a>
**Status (verified 2026-10-07 on the reference laptop of
[decision 0010](decisions/0010-reference-machine-laptop.md), branch `m7`, uncommitted working
tree, release builds, machine loaded by the parallel M4.5–M6 work, so every timing number is
indicative): criteria 2, 3 and 4 pass (4: the rules are written; the owner's confirmation is
still to be recorded before any public test build); criterion 1 passes on one machine
(loopback and a container network), and a run between two machines on a real LAN is
open.** Every deliverable exists: `docs/specs/tessera-protocol.md` (v1.0, Accepted after an
independent review), [decision 0030](decisions/0030-tessera-protocol-transport.md) (QUIC with
quinn and rustls on ring), `tessera-protocol` (`crates/protocol`), `tessera-server`
(`apps/server`, built-in original default world), the `tessera-net` test-server backend
(`remote`, with snapshot interpolation) wired into the client (`--server`), and
`tessera-loadtest` (`apps/loadtest`). Specs: `tessera-protocol.md` (Accepted),
`server-content.md` and `test-server.md` (Draft), LEGAL.md §11.1. No `> **OPEN:**` item blocks
a criterion; the ones to settle before someone writes a second implementation from the spec
are the four round-2 review notes at `tessera-protocol.md` §4.5, §8.3, §8.7 and §13.3.

| # | | Evidence |
|---|---|---|
| 1 | ✅ one machine; LAN between two machines pending | One-hour soaks with `tessera-loadtest` (16 clients, each a `tessera_net::Session` over the test-server backend, walking and running across landblocks, teleporting, changing appearance and stance, chatting and reconnecting; [m7-soak.md](perf/m7-soak.md), third series): **S60d** (in-process, loopback) and **L60d** (server on the host's Docker bridge address, clients in a container with their own network stack) both PASS: 0 desync episodes of every kind over 5,185,495 and 4,272,246 position comparisons (240 observer–entity pairs, 236–240 of them left interest range and came back), 13,649 / 13,649 and 13,434 / 13,434 chat deliveries, 0 unexpected disconnects, 22 reconnections each; no memory growth after warm-up (S60d max 68.6 MiB; L60d server process 47.6 MiB, client process 10.7 MiB), every count back to its baseline. This verification found and fixed the client-side growth that failed the earlier hours (a thread, runtime and 2.9 MiB quinn endpoint per session; now one network thread and one shared endpoint, `tessera-net`), the tool's provisioning against a LAN address, and a drift of the server's tick schedule. Two release clients against a test server on the install, screenshots viewed: each sees the other's character and the default world's wanderers drawn and animated, network HUD (`interp 167 ms`, `4 remote`), message of the day in the chat overlay; `apps/client/tests/net_server.rs` (5 tests): both clients see each other move with smooth interpolation and receive each other's chat. Animation is compared as heading, gait and stance (0 episodes); `test-server.md` §13.2 has no animation rule yet (§15.2 OPEN). |
| 2 | ✅ | `tessera-protocol.md` reviewed in two rounds by a reviewer who did not write the code (its Review section, R.1–R.7): Status Accepted, no blocking finding. The reviewer wrote a second codec from the spec text alone (`crates/protocol/tests/spec_conformance.rs`, 15 tests): it and the crate agree byte for byte on all 42 message vectors of §16.2–§16.5, the 16 tagged values of §16.6 and the handshake vectors of §16.1, both reject the 124 malformed inputs (every §16.7 negative vector) and accept the 23 boundary inputs. Every message type round-trips over 2,048 property-test cases (`proptests.rs`). Non-blocking OPEN notes remain (70 in the spec, most implementer notes); the four round-2 notes named above should be settled before a third party implements it. |
| 3 | ✅ | Ten fuzz targets cover every parser of the protocol, the client backend and the server: `protocol_frame`, `protocol_message`, `protocol_datagram`, `protocol_handshake`, `protocol_fingerprint`, `net_remote_inbound`, `net_remote_world`, `server_connection`, `server_world`, `server_files`. `tools/fuzz-smoke.sh -t 60` (stable libFuzzer): no crash, 0.11 M (`server_world`) to 26.8 M (`protocol_message`) executions each; the two `net_remote` targets re-run after the endpoint change (0.47 M and 0.06 M, no crash). Hostile soaks: H5d (final build, 1,924 attacks of ten kinds, 12,480 raw datagrams) and the earlier H5-16, H15-16, H5b, HR15: 0 panics, honest clients unaffected (0 desync, 0 unexpected disconnects), every count back to baseline, server process memory flat (HR15). Plus `apps/server/tests/hostile.rs`, `crates/net/tests/remote_hostile.rs` and the protocol state-machine tests. |
| 4 | ✅ rules written; owner confirmation pending | [LEGAL.md §11.1](LEGAL.md#111-servers) states the seven server rules (no game data on a server, opaque references relayed but never collected, at most 16 bare positions per content file, an original default world, no gameplay reproduction, private or LAN use only, licensed committed content), with `server-content.md` as the checkable version, and says that the owner's dated confirmation is needed before any public test build (none exists). `tessera-server` links no data-decoding crate (`cargo tree -p tessera-server`), its built-in world has no opaque references, `check-content` enforces the automatic rules, and registration refuses non-private listen addresses. Open for the owner: confirm §11.1, the coordinate rule and the reading of the LEGAL.md banner (`server-content.md` §12). |

Repository checks at the same time: `cargo fmt --check` (workspace and `fuzz/`), `cargo
clippy --workspace --all-targets [--all-features] --locked -D warnings` and on `fuzz/`,
`cargo test --workspace` with `TESSERA_GAME_DIR` set (2,227 passed, 0 failed, 3 ignored: one machine-specific install
test, one doc example, the server's release soak test, which was run separately), `cargo doc --workspace
--no-deps --all-features -D warnings`, `cargo deny check bans licenses sources` (both
manifests; no `aws-lc-sys` or `openssl-sys` in the tree), `tools/check-content.sh` and its
self-test, `tools/check-spdx.sh` (0 errors) and the relative links and anchors of the
Markdown docs all pass. The ignored release soak test of `tessera-server` passes (16 bots,
20 s, 0 overruns).

Fixed during this verification: `tessera-net` runs every session on one network thread per
process with one shared quinn endpoint per address family (the in-process hour soaks failed
the leak check before; regression test `sessions_share_one_network_thread`); the load tool
provisions at most four accounts at once (a server on a LAN address refused its batches of
eight); the server derives each tick deadline from `tick_start(k)` (`tessera-protocol.md`
review R.7 finding 3); `fuzz/README.md` rows for the ten M7 targets; ARCH §19.4.

Remaining for M7 to close: a soak between two machines on a real LAN (criterion 1), the
owner's recorded confirmation of LEGAL.md §11.1 (criterion 4, before any public test build),
re-running S60d and L60d from the merge commit, and the spec writer's decisions on the
review notes (`tessera-protocol.md` §17, `test-server.md` §15). Owner observations O70–O73
([owner-observations.md](checklists/owner-observations.md#m7-items): movement speeds,
interpolation feel, interest distance, say range) tune defaults; no criterion waits on them.

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

<a id="p-track"></a>
## P-track: gameplay parity, client half (P0–P8)

**Plan.** [Decision 0024](decisions/0024-parity-track.md) (accepted by the owner on
2026-10-09) holds the milestones, their specs and exit criteria: P0 foundations (scenario
feed, `tessera-gamedata`), P1 characters, inventory and equipment, P2 targeting, skills and
combat presentation, P3 NPC windows, P4 social panels, P5 map, travel, mounts and
instances, P6 progression and crafting panels, P7 client-run game code, P8 parity audit.
Only the client half is built; the game's own rules are not run, offline included (T3).
Each milestone gets its own section here when it starts.

<a id="p0"></a>
### P0: Foundations

**Goal:** the two foundations every later P milestone stands on: a **scenario feed** that
drives the client half offline with scripted gameplay events, and **`tessera-gamedata`**,
the presentation data layer that reads gameplay records from the user's install
([decision 0024](decisions/0024-parity-track.md) §3–§5). Builds on M5 and M7. Risk tier T1
(client only, offline).

<a id="p0-deliverables"></a>
**Deliverables**

- `docs/specs/scenario-feed.md` (new): the scenario file format, step vocabulary, timing and
  triggers, selector language, resolution contract, the gameplay events and commands,
  determinism and the canonical event log, hostile-input limits, client options and the
  rules for committed scenarios. Tessera's own design; no client facts beyond the Accepted
  specs it cites.
- `tessera-net`: the scenario feed (`tessera_net::scenario`, cargo feature `scenario`):
  parser and static checks, the `SelectorResolver` contract and resolved scenario, the
  run-time feed in the sandbox backend, the headless driver and the canonical log writer;
  the gameplay event and command groups of ARCH §19.1 (Character, Inventory, Combat, Npc,
  Social, World, Progression). It decodes no game data.
- `tessera-gamedata` (`crates/gamedata`, new, L3): the record-family index and typed entry
  points over `props`/`wstate`, and scenario selector resolution against the install
  (canonical order, seeded sampling, notes). Typed stores, tooltip and description
  evaluation grow from P1.
- `tessera-client`: `--scenario`, `--scenario-log` and `--scenario-input`; headless runs on
  the scenario's virtual clock with the exit codes of `scenario-feed.md` §12.2; a resolver
  adapter from the feed to `tessera-gamedata`; committed synthetic scenarios with their
  logs (`apps/client/tests/scenarios/`) and a replay test.
- `fuzz/`: `net_scenario_parse` and `net_scenario_run`.
- `tools/check-server-deps.sh` and the CI job `server-deps`: `tessera-server` links no
  package outside crates.io other than `tessera-protocol` (so no data-decoding crate, no
  `tessera-gamedata` and no `tessera-net`, hence no feed). Every crate that runs the server
  in-process (today `tessera-loadtest`) is checked too, with `tessera-net` also allowed: a
  host refuses data-decoding crates and `tessera-gamedata`, but in a workspace build its
  `tessera-net` carries the feed (feature unification). Whether hosts must stay feed-free is
  OPEN item (3) of `scenario-feed.md` §3.1.
- Documentation: ARCH §3 and §3.1 (`tessera-gamedata`, dependency rules), §19 (the gameplay
  events and commands, the scenario feed of the sandbox), §20.1, §23; `docs/USER_GUIDE.md`
  (scenario runs); at close-out, `CLAUDE.md` Layout (`crates/gamedata`, the `net` scenario
  feed, `tools/check-server-deps.sh`) and Current state (P0).

<a id="p0-specs"></a>
**Specs needed:** [`scenario-feed.md`](specs/scenario-feed.md) (new);
[ARCH §3](ARCHITECTURE.md#layers) (`tessera-gamedata`, [§3.1](ARCHITECTURE.md#crate-table))
and [ARCH §19](ARCHITECTURE.md#net) (the new domain events, grouped by system); the Accepted
gameplay specs that define the selector families (`scenario-feed.md` §8.3).

**RE tasks:** none. The feed is a design, and the selector attributes come from Accepted
specs.

<a id="p0-exit"></a>
**Exit criteria** (decision 0024 §5, quoted)

1. `scenario-feed.md` Accepted after an independent review, no blocking OPEN.
2. `tessera-client --scenario <file>` plays a synthetic scenario headless; the same file
   gives the same event sequence on every run.
3. The scenario parser has a fuzz target; hostile files are refused without a panic.
4. ARCHITECTURE §3 lists `tessera-gamedata`; CI fails when `tessera-server`'s dependency
   graph holds a data-decoding crate, `tessera-gamedata` or the feed.

<a id="p0-status"></a>
**Status:** in progress (started 2026-10-09; exit verification 2026-10-09). Criteria 2, 3
and 4 pass; criterion 1 is not met: `scenario-feed.md` is in Review (revision 3) and waits
for an independent re-review. No criterion needs an owner observation in the original client.
Checks on branch `p0` (uncommitted tree, 2026-10-09): `cargo fmt --check` (workspace and
`fuzz/`), clippy `-D warnings` (workspace `--all-targets`, also `--all-features`; `fuzz/`
`--bins`), rustdoc `-D warnings` on `tessera-net`, `tessera-gamedata`, `tessera-client`;
`cargo test --workspace --release` with the install: 5,103 passed, 0 failed, 10 ignored
(363 test binaries); `tools/check-content.sh`, `tools/check-spdx.sh` and a link and anchor
check of the 122 Markdown files: 0 errors. After the verifier's client fix (`--scenario-log` and
`--scenario-input` refused without `--scenario`, and a log path equal to the scenario file
refused, both exit 2): `cargo test --release -p tessera-client` with the install 500 passed,
0 failed, 1 ignored.

| # | Status | Evidence |
|---|---|---|
| 1 | **not met** | `scenario-feed.md` header: Review, revision 3. Revision 2 was set Accepted by the session that wrote its fixes R1–R31, which `CLEAN_ROOM.md` §3 does not allow; revision 3 fixes the implementation-phase findings S1–S31. A session that wrote none of revisions 1–3 must re-check R1–R31, S1–S31 and every vector before it can be Accepted. The implementers' tests already run Vectors A–F and the 44 hostile-input rows of §14.5 (`crates/net/tests/scenario_vectors.rs`, `scenario_hostile.rs`; all pass). The spec states no blocking OPEN; its 12 OPEN notes are marked non-blocking (implementer interpretations in §3.1, §12.4, §16; later milestones; owner policy on numeric enum keys before P1; a log-size limit and the CC0 wording of §13, added at exit verification). |
| 2 | **pass** | `tessera-client --headless --no-data --scenario F --scenario-log L` on the six committed synthetic scenarios (`apps/client/tests/scenarios/`: combat, inventory, npc-windows, progression, social, world), release build, two runs each: exit 0 every time, both logs byte-identical and equal to the committed `.log` (64, 54, 44, 40, 46, 38 lines). `tests/scenarios.rs` 12 / 12 (replay, content rules, exit codes 2–5); with the install `tests/scenarios_real_data.rs` 4 / 4 in 75 s (each scenario twice with identical logs; ground spawn; screenshot run on the GPU); `scenario_world.rs` 3 / 3; `tessera-gamedata` `selectors` 37 / 37 and `real_data` 2 / 2 (383,343 records indexed, 0 failures, two stores give identical DID lists). |
| 3 | **pass** | Fuzz targets `net_scenario_parse` (parse, then 200 ticks of playback; parse peak allocation bounded) and `net_scenario_run` (synthetic resolver from the input, headless driver, per-poll event and weight bounds). `tools/fuzz-smoke.sh -t 60`: 733,053 and 88,851 executions, no crash. Hostile files given to the release client (random bytes, an unterminated string, a 2 MB file, an over-long integer, unknown keywords): exit 3 with `FILE:LINE:COLUMN` and the error kind, no panic; `scenario_hostile.rs` (the 44 rows of §14.5 and 31 further static errors), `scenario_props.rs` (proptests over random lines and bytes) and the 11 hostile files of `tests/scenarios.rs` pass. |
| 4 | **pass** | ARCHITECTURE §3 (layer diagram, dependency rule) and §3.1 list `tessera-gamedata` (L3, client and tools only). CI job `server-deps` (`.github/workflows/ci.yml`) runs `tools/check-server-deps.sh --self-test` (14 fixture cases incl. feature unification, dev, build and target-specific edges, path and git packages, in-process hosts, `#[path]`/`include!`; all caught) and the real check: `tessera-server` links nothing outside crates.io but `tessera-protocol`; its host `tessera-loadtest` links no data crate or `tessera-gamedata` (exit 0). Also in `tools/ci-local.sh`. |

Remaining issues: criterion 1 (independent re-review of `scenario-feed.md`); open review items
for owners: the windowed scenario log is written on the main thread (ARCH §21 exception (d)),
no test yet checks that event and command `name()` strings are unique (ARCH §19.1), the
windowed input script is not played (§12.4 OPEN), and whether in-process server hosts must
stay feed-free (§3.1 OPEN item 3).

Follow-ups (2026-10-09, after the spec's independent review passed, Accepted): the windowed
log is written by a writer thread behind a bounded channel (ARCH §21, exception (d) gone);
`tessera-net` `session::name_tests` checks that every event and command `name()` is unique
(ARCH §19.1); windowed runs play the input script with `--scenario-input` through the new
public `tessera_net::scenario::InputPlayer` (§12.4 OPEN (1); the refusal of an `enter`
character that only the input script creates now applies only without the option); logs
stop at 256 MiB with `error=LogOverflow`, exit 5 (§11.1 OPEN, the review's recommended
bound); the content test accepts CC0-1.0 headers (§13 OPEN). The review's rulings on §3.1
guard item 1 (refuse `tessera-net` in any form) and module naming match the code as it was.

---

<a id="m9"></a>
## M9: Extended scripting (after full parity)

**Placement (owner, 2026-10-08).** Extended scripting is not a priority. It starts only after
Tessera reaches full parity with the original client and compatibility with the server (M8).
If the M8 gate fails, it starts after client parity with Tessera's own server (M7). It was
planned earlier as M5.5, right after M5.

**Plan.** [Decision 0017](decisions/0017-extended-scripting.md) holds the design: Lua 5.1
"ext" apartments under `Ext`, modules granted by capability, a fail-closed backend gate,
and slices 1a (core), 1b, 2 and 3. Its specs are written and Accepted: `ext-threat-model.md`,
`ext-manifest.md`, `ext-api-1a.md` and `ext-api-1b.md`. Ext scripts never run against an
official server (decision 0017 §4, LEGAL §9), whatever M8 decides.

**What stays in place until then.** M5 already split `tessera-script` (a plugin-free Lua core)
from `tessera-plugins` (decision 0023), as decision 0017 asked. M9 therefore does not redo
that work. The pinned external Lua fork that 0017 needs is created only when M9 starts,
after the owner confirms it.

**Exit criteria:** those of decision 0017's slice 1a section, re-checked when M9 starts.

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

Reference machine for performance criteria: **settled 2026-10-05**
([decision 0010](decisions/0010-reference-machine-laptop.md)). The owner accepted the laptop
(i9-10885H, RTX 2070 Super Max-Q) described in
[`docs/perf/reference-machine.md`](perf/reference-machine.md), because the GTX 1070 Ti
workstation, where M3's evidence was measured, is not available. The M3 flights were re-run
on the laptop as the new baseline. Results from other machines are indicative. The decision
0005 entity-storage benchmark ([0008](decisions/0008-entity-storage.md)) was measured on the
laptop, so its numbers are reference numbers.

> **OPEN:** whether a self-hosted CI runner with the owner's install should run the gated
> real-data tests on every change, or nightly. Results must stay pass/fail and counts.

> **OPEN:** Windows and macOS support targets per milestone. wgpu and winit make both
> plausible from M0; CI coverage for them is undecided.
