<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The offline world walker (`apps/client`, ROADMAP M3)

| | |
|---|---|
| **Status** | M3 design, implemented |
| **Last updated** | 2026-10-04 |
| **Author role** | Clean-room implementer of `tessera-client` |
| **Implements into** | `apps/client/src/walker/`, `apps/client/src/walker_host.rs`, `apps/client/src/headless.rs`, `fuzz/fuzz_targets/client_walker_text.rs` |
| **Format facts from** | `coordinates.md` (§3, §8–§12), `landblock.md` (§5, §6.3, §8), `entity-descriptor.md` (§5–§9), `setup.md` (§7), `mesh.md` (§8, §11), `material.md` (§9–§12), `terrain.md`, `keymap.md`, `wstate.md` (§11) |

This file records **Tessera's own design decisions** for the world walker: how it streams,
draws and reports the world, and how it is driven in automated runs. It adds no format facts;
the questions it could not settle are collected in §9.

---

<a id="s1"></a>
## 1. Running it

| Option | Meaning |
|---|---|
| `--spawn R,X,Y[,x,y[,z]]` | Landblock spawn (the sandbox's spawn grammar): block centre, block-local, or exact height. Default `1,126,140` (the `coordinates.md` §8.4 B example block). |
| `--place NAME` | An area, territory or region name, map coordinates (`12.3S, 45.6W`, in `--region`), or a pasted `/loc` line. |
| `--fly`, `--fly-speed`, `--heading DEG`, `--pitch DEG`, `--eye-height M` | Start mode and framing (compass heading, 0 = north). |
| `--fly-path FROM,TO`, `--speed`, `--fly-height` | Scripted straight flight (`X:Y`, `R:X:Y`, `X:Y:x:y` or `R:X:Y:x:y` per end), then exit. |
| `--frame-log CSV` | Per-frame log (§7), written when the walker exits. |
| `--screenshot PNG` | Render the spawn offscreen once settled (§7), save, exit. |
| `--headless` | Offscreen without a window (implied by `--screenshot`). |
| `--residency-mib`, `--view-blocks`, `--object-distance`, `--gpu-mib`, `--texture-cap`, `--interiors`, `--keymap` | Budgets and options (§3–§5). |
| `--showcase` | The M0 texture showcase instead of the walker. |

Without an install the client shows the M0 test scene (`--no-data`), as before.

<a id="s2"></a>
## 2. Frames and threads

- Positions are f64 region-global (`coordinates.md` §2). Rendering is relative to an
  **anchor landblock** containing the eye (`coordinates.md` §12): a landblock's meshes are
  drawn translated by `160 · (X − X_anchor, Y − Y_anchor, 0)`.
- Movement controllers (`keymap.md` §9) keep f32 positions relative to a **controller
  origin** moved only after 8 km, so motion is not disturbed at every landblock crossing and
  f32 precision stays near 1 mm.
- Main thread per frame: input → controllers (30 Hz fixed step) → `WorldStreamer::update`
  → new/released bundles to the three layers → GPU uploads within a per-frame budget (24 MiB
  and 4 ms; the first upload of a frame always proceeds) → culling and draw recording. It
  reads no archive and decodes nothing; a `/proc` RSS sample every 0.5 s is its only file
  access.
- Workers: landblock bundles, part meshes, materials, textures and minimap tiles run on the
  `tessera-assets` pool; terrain blocks and meshes on two dedicated threads; the world
  context, area names and the place index each on a start-up thread.

<a id="s3"></a>
## 3. Terrain

Per resident bundle a worker builds the `TerrainBlock` (forcing its level error bounds), a
33 × 33 height-shaded overview tile (§6) and the object instances (§4). Levels are chosen per
frame with `LodSelector` for the actual viewport; a block is re-meshed when its level, its
neighbours' levels or the set of neighbours its normals saw changes, the old mesh drawn until
the new one is uploaded. Shading is `terrain.md` §6 (white lit material, vertex colours).
The far ring (`--view-blocks`, default 10 landblocks) bounds the terrain draw distance; near
and mid rings are 3 and 6 landblocks, hysteresis one landblock.

<a id="s4"></a>
## 4. Static objects

- Instances: `LandblockBundle::drawable()` (the `entity-descriptor.md` §9 rule), minus
  interior entities (flag 0x400) unless `--interiors`; one instance per part of the resolved
  setup, transform `T(position) · R(q) · S(scale)` (`entity-descriptor.md` §5, `setup.md` §7).
- Parts are loaded when their landblock comes within the object range (default 900 m, to the
  block square; one landblock of hysteresis) and reference-counted per landblock. Level: the
  first detail level (`mesh.md` §8). Materials: diffuse texture with the opacity of the alpha
  test (else of the blend), alpha mode, two-sidedness, diffuse tint, lit flag
  (`material.md` §10–§11); normal maps, specular and dye are not used.
- Textures are composed (alpha merged) and capped at `--texture-cap` (512) on a worker. A
  submesh waits for its texture; one whose texture failed is drawn untextured only when
  opaque.
- GPU memory: unreferenced parts and textures are freed after 30 s idle or least recently
  used first while over `--gpu-mib`. Referenced resources cannot be freed, so while they alone
  exceed the budget the object range shrinks by 10 % per second (to 160 m) and grows back by
  5 % per second below 80 % of the budget. The budget is therefore soft: it is exceeded for a
  few seconds when a dense area streams in (measured: 286 MiB peak against 128 MiB).
- Culling: landblock box, then per instance the frustum, the range and a projected radius of
  at least 1.5 px; at most 20,000 draws per frame, nearest landblocks first.

<a id="s5"></a>
## 5. Avatar, spawn and controls

- The sandbox backend owns the avatar. The walker hands it a `WorldSource` that resolves map
  coordinates, `/loc` lines and place names (§6), answers ground heights from resident
  terrain (pending until the spawn block is built; the walker streams towards every location
  the sandbox asks about), describes locations (area, map coordinates, `/loc` tokens) and
  serves no static entities (the walker draws them from its own bundles). The walker places
  the viewer on the `Entered` and `Teleported` location reports and reports its movement at
  10 Hz.
- Walk mode: the `tessera-input` walk controller on terrain heights, a third-person camera
  7 m behind the head (wheel zooms 1.5–60 m), kept 0.8 m above the terrain and then aimed at
  the head. Fly mode: the free-fly controller (wheel scales its speed). `Ctrl+F` toggles,
  F3 toggles HUD details, F12 saves a screenshot to `<data dir>/screenshots/`, Escape quits.
  Bindings come from Tessera's keymap file over the built-in defaults (`keymap.md` §6).

<a id="s6"></a>
## 6. HUD, minimap and places

- HUD lines: frame rate, mean and longest frame of the last second; area name (area –
  territory, region) from `tessera-world`'s area names; map coordinates (`coordinates.md`
  §8.3); `/loc` tokens with compass heading (§9, §11.4); region, landblock, record DID and
  block-local position; resident landblocks and bundle bytes against the budget, loading and
  near-ring holes; GPU bytes by kind and process RSS; details (terrain, objects, draws,
  streamer time, uploads, worker queue). Text is rasterised with the public-domain 8 × 8
  glyphs of the `font8x8` crate (Basic Latin and Latin-1), sanitised and capped.
- Minimap: north-up, ±320 m around the viewer, from the landblock minimap tiles of resident
  blocks (`landblock.md` §6.3, pixel frame `coordinates.md` §10, active content layer 1)
  drawn as clipped quads; blocks without a tile, or until it loads, show the overview tile.
- Place index (`--place`): every block map of every region with a block-map descriptor is
  read for `Area_DID`; each area, territory and region name maps to the landblocks it covers;
  the target is the centre of the covered landblock nearest to their centroid. Matching is on
  names lower-cased, Latin-1 folded and reduced to alphanumeric words: exact, then prefix at a
  word boundary, then whole-word substring; ties prefer `--region`, the more specific kind and
  the larger place. Built on first use (0.55 s for 390,150 landblocks here).

<a id="s7"></a>
## 7. Automated runs

- `--screenshot`: the spawn is settled when the viewer is placed, the streamer has nothing
  loading, deferred or missing in the near ring, every terrain block is meshed and every part
  and texture in range is on the GPU or failed, for 20 consecutive frames (or after
  `--settle-timeout`).
- `--fly-path`: the camera waits at the start (`--fly-height` above the terrain, facing the
  end, pitched 8.6° down) until settled, then moves at `--speed`, following the terrain
  upwards at once and downwards slowly. Headless runs are paced at 60 frames per second and
  wait for the GPU after every frame, so frame times include GPU work.
- Frame log columns: `frame, t_s, frame_ms` (wall time between frame starts), `cpu_ms`
  (main-thread work of the frame), `stream_ms`, `uploaded_bytes, resident_blocks,
  resident_bytes, loading, near_missing, gpu_bytes, rss_bytes, object_draws, terrain_draws,
  x, y, z`. The summary reports the longest frame, p50/p99, frames over 50 ms, peak residency
  (landblocks, bundle bytes, GPU bytes, RSS) and frames with near-ring holes.

<a id="s8"></a>
## 8. Verification (2026-10-04, this install, GTX 1070 Ti, Vulkan, release build)

- Screenshots viewed at Kingsfell (default spawn, walk), Bree (walk and fly), Minas Tirith
  (region 3), Caras Galadhon (region 2), the Rivendell valley and a region-2 edge block
  without minimap tiles (overview fallback); all settled.
- Full straight flight across region 1 (landblocks 0 → 254 on row 118, 40.6 km at
  1,000 m/s, 1280 × 720 offscreen): 2,438 frames, longest frame 16.8 ms (paced), longest
  main-thread work 4.7–7.8 ms, streamer update at most 0.29 ms, 0 frames over 50 ms, no
  near-ring holes; peak 386 landblocks / 86.5 MiB bundles (budget 512 MiB), about 790 MiB GPU,
  705–730 MiB RSS; 5,355 bundles requested and integrated, none failed. Diagonal of region 2
  at 3,000 m/s: longest main-thread work 7.7 ms, 0 stalls, peak 118 MiB bundles. Windowed
  (vsync, 100 Hz) 16 km at 1,000 m/s: longest frame 11.8 ms.
- These runs had the archives in the page cache; a cold-cache run was not possible without
  privileges to drop caches.

<a id="s9"></a>
## 9. Open questions

> **OPEN:** Water and sky are not drawn (`environment.md` §8, `terrain.md` §9); the sky is a
> fixed colour and lakes show their bed.

> **OPEN:** The renderer uploads one texture level (no mip chain), so distant object
> textures alias; the walker caps textures at 512 texels to limit it. A mip chain upload
> (`texture-list.md` §5) belongs to `tessera-render`.

> **OPEN:** Some opaque submeshes whose texture does not resolve draw as their (often
> white) tint, visible as bright patches in the Rivendell valley. Which materials these are,
> and what the original shows for them, is not established (`material.md` §18–§19).

> **OPEN:** Area names are not unique places: "Helm's Deep" resolves to an area record that
> covers 524 landblocks at the region's northern edge, apparently an instanced copy rather
> than the place at the fortress. Which area records are the "main" world placement (versus
> instance copies) is unknown (`wstate.md`); the index prefers exact names, then size.

> **OPEN:** The original's draw distances, camera placement on login and object LOD switch
> distances are unknown (`terrain.md` §9); the walker's values in §3–§5 are Tessera's own.

> **OPEN:** M3 exit criteria 4 and 5: the walker side of the 20 reference locations
> ([`m3-reference-locations.md`](../checklists/m3-reference-locations.md#s4-3)) passes
> (`apps/client/tests/reference_locations.rs`); the original client's readings (owner item
> [O1](../checklists/owner-observations.md#o1)) are still pending, and with them the
> map-coordinate rounding rule (`coordinates.md` §8.3).
