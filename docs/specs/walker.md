<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The offline world walker (`apps/client`, ROADMAP M3–M4)

| | |
|---|---|
| **Status** | M3–M4 design, implemented |
| **Last updated** | 2026-10-05 |
| **Author role** | Clean-room implementer of `tessera-client` |
| **Implements into** | `apps/client/src/walker/` (M4: `walker/characters/` — layer, `figure.rs`, `catalog.rs`, `motion.rs` — and `walker/figures.rs`), `apps/client/src/walker_host.rs`, `apps/client/src/headless.rs`, `apps/client/src/avatar.rs`, `apps/client/src/net.rs`, `crates/net` (sandbox figures, crowd, entity events), `crates/world` (bundle figures), `fuzz/fuzz_targets/client_walker_text.rs`, `client_avatar_spec.rs`, `net_npc_events.rs`, `world_bundle.rs` |
| **Format facts from** | `coordinates.md` (§3, §8–§12), `landblock.md` (§5, §6.3, §8), `entity-descriptor.md` (§5–§9), `setup.md` (§7), `mesh.md` (§8, §11), `material.md` (§9–§12), `terrain.md`, `keymap.md`, `wstate.md` (§11); M4: `appearance.md` (§2–§7), `skeleton-anim.md` (§2, §4, §7, §8), `script-table.md` (§6, §8, §11) |

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
| `--avatar SPEC` | The avatar's body and items (§10.6; grammar in `apps/client/src/avatar.rs`): `SPECIES,SEX` or a figure DID, then `;ITEM[@dye=CODE][@dye2=CODE][@stowed\|@wielded]`. Default `human,m`. |
| `--crowd N`, `--crowd-spacing M`, `--crowd-ahead M` | Simulated characters in front of the spawn (§10.7); defaults 0, 3 m, 8 m. |
| `--content-layers LIST` | Content layers treated as active (`entity-descriptor.md` §4.5.1), default `1`. |
| `--anim-distance M`, `--character-distance M` | Sampling and drawing distance of characters (§10.5, §10.2); defaults 200 m and 400 m. |
| `--markers` | Also draw weenie markers (§10.2). |
| `--duration SECS` | Run this long after the spawn settled, with a frame log, then exit (timing runs). |
| `--autorun walk\|run` | Move the avatar forward on its own from the start. |
| `--unpaced`, `--no-vsync` | Headless runs unpaced (not 60 frames per second); windowed runs without vsync. |
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
  characters` (entities with an animation component), `character_draws, characters_ms`
  (main-thread time of the whole character layer), `anim_collect_ms, anim_submit_ms,
  anim_upload_ms` (main-thread animation steps: collecting poses, driving and submitting,
  palette upload), `anim_worker_ms` (worker time of the poses collected), `x, y, z`. The
  summary reports the longest frame, p50/p99, frames over 50 ms, peak residency
  (landblocks, bundle bytes, GPU bytes, RSS) and frames with near-ring holes; with
  characters also the mean fps, p95, the most animated characters, the fewest drawn, the
  main-thread animation time (mean and max), the worker time, the share of the animation
  work done on workers (worker / (worker + main thread)) and the character layer's longest
  frame.

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

<a id="s8-1"></a>
### 8.1 M4: characters (2026-10-05, this install)

Machine: Intel i9-10885H (8 cores, 16 threads; 14 asset workers), NVIDIA GeForce RTX 2070
Super with Max-Q Design (driver 615.71.09), Vulkan, Hyprland (Wayland), release build,
archives in the page cache. This laptop became the reference machine on 2026-10-05
([decision 0010](../decisions/0010-reference-machine-laptop.md)); the runs below were made
before that and are superseded by the ones in `docs/perf/reference-machine.md`. (The M3 rows above name a
GTX 1070 Ti; whether that run was on another machine is not recorded here.)

**Exit criterion 4** (50 animated characters on screen at 60 fps with sampling on
workers): `--crowd 50` at the default spawn (`1,126,140`, heading 30°), every playable body
of the race table in turn, a third standing, a third walking and a third running in
circles, plus the avatar (51 animated characters). Timing starts once the spawn has
settled.

| Run | Frames / s | Mean fps | Frame p50 / p95 / p99 / max (ms) | Characters drawn (every frame) | Animation main thread mean (max) | Worker sampling mean | On workers |
|---|---|---|---|---|---|---|---|
| Windowed, `--no-vsync` (present mode Immediate), `--duration 60`; the tiling compositor sized the window 1430 × 1580 (portrait), so the crowd used `--crowd-spacing 2 --crowd-ahead 20` to stay in view | 22,552 / 60.0 | 375.8 | 2.4 / 4.52 / 5.4 / 12.3 | 51 of 51 | 0.242 ms (1.12 ms) | 1.186 ms | 83.1 % |
| Headless 1280 × 720, `--unpaced --duration 60` (each frame waits for the GPU), default crowd layout | 12,155 / 60.0 | 202.6 | 5.0 / 5.51 / 6.5 / 12.4 | 51 of 51 | 0.239 ms (0.88 ms) | 1.031 ms | 81.2 % |

Both pass (p95 far below 16.7 ms, no frame over 16.7 ms). Main-thread work peaked at
10.1 ms (windowed) and 5.8 ms (headless); the whole character layer (events, figure
uploads, animation) at most 0.96 ms and 0.54 ms per frame. "Animation main thread" is
collecting the finished jobs, driving the animators and submitting jobs, uploading the
palettes and recording the character draws; "worker sampling" is the jobs' own time.
Commands: `tessera-client --no-vsync --duration 60 --crowd 50 --crowd-spacing 2
--crowd-ahead 20 --heading 30 --frame-log …` and `tessera-client --headless --unpaced
--duration 60 --crowd 50 --heading 30 --size 1280x720 --frame-log …`.

For scale, a dense spot: next to N6 (Umbar, `--spawn 5,62,7,11.51,44.5 --content-layers
1,0x10000536`) the sandbox creates 432 placed figures; 423 animated characters, of which at
least 223 were drawn in every frame: headless unpaced 73.7 fps mean, p95 16.0 ms, p99
19.8 ms, main-thread work at most 7.0 ms (GPU-bound: about 7,400 object instances and some
2,000 character submesh draws).

**NPC spots** (`m4-visual.md` §N; `apps/client/tests/npc_spots_real_data.rs`, release,
headless): for every row the sandbox created the spot's entity and the character layer holds
it exactly at the data's position (landblock origin plus the descriptor's block-local
position) and heading (the descriptor rotation applied to the model's +Y,
`coordinates.md` §11.3), and the spot settled with every figure on the GPU:

| Row | Landblock (R, X, Y) | Position (region-global) | Heading (rad) | Figures created around / animated characters |
|---|---|---|---|---|
| N1 | 1, 27, 143 | 4420.58, 22899.27, 494.75 | −2.4192 | 2 / 3 |
| N2 | 1, 103, 138 | 16528.32, 22118.57, 357.86 | −2.3226 | 5 / 6 |
| N3 | 1, 63, 170 | 10230.88, 27324.19, 613.90 | 0.0000 | 5 / 6 |
| N4 | 1, 59, 133 (content layer 0x10000019) | 9487.04, 21346.28, 593.33 | −2.8798 | 3 / 4 |
| N5 | 1, 79, 121 | 12791.07, 19510.72, 402.49 | −0.6248 | 190 / 189 |
| N6 | 5, 62, 7 (content layer 0x10000536) | 9931.51, 1169.93, 156.11 | −2.0375 | 432 / 423 |
| N7 | 2, 53, 246 | 8584.50, 39511.44, 450.85 | −1.5951 | 134 / 1 (distant-battle markers are hidden, §10.2) |

N4 and N6 list only content layers outside the walker's default `[1]`, so a default run
hides them (`entity-descriptor.md` §4.5.1 OPEN); the test activates their layers. Screenshots
were viewed at the default spawn (avatar idle, avatar walking with `--autorun walk`, the
crowd), N1 (two seated figures on logs), N4 (stocky figure with a pick in the right hand),
N5, N6 (a formation of soldiers animated as rig figures, §10.2) and deleted.

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

> **OPEN:** Character colours: figures are drawn with one dye channel (§10.4) and without
> the character's own appearance values (skin tone, hair colour; `appearance.md` §3.4), so
> the avatar's skin shows lighter and greyer than in the explorer's character view.

> **OPEN:** Random idle fidgets (one-shot clips picked by Random nodes,
> `script-table.md` §8.6) and the original's rule for when they repeat are not played; only
> looping idle clips are (§10.5).

> **OPEN:** Speed matching (`script-table.md` §8.5, `skeleton-anim.md` §7.4): the
> locomotion playback rate is scaled by ground speed over natural speed (§10.5); whether the
> original does so is unobserved.

M4 exit criterion 4: resolved 2026-10-05. The i9-10885H / RTX 2070 Super Max-Q laptop of
§8.1 is the reference machine from that date
([decision 0010](../decisions/0010-reference-machine-laptop.md)). The criterion was re-run
there on a 1.99.0 release build and passes. The M3 flights were also re-run there as the new
baseline. Numbers for both are in
[`docs/perf/reference-machine.md`](../perf/reference-machine.md).

> **OPEN:** Main-thread spikes during the M3 flight: on 2026-10-05 the M3 build itself
> (commit 65b1b3f) took frames of up to 40 ms on this machine, inside texture uploads of the
> object layer (`Renderer::upload_texture`); the M4 build behaves the same (character layer
> at most 7.3 ms in one frame of the flight, under 1 ms otherwise). The M3 numbers of §8
> were not reproduced; the cause (driver, machine load) is not known.

> **OPEN:** M3 exit criteria 4 and 5: the walker side of the 20 reference locations
> ([`m3-reference-locations.md`](../checklists/m3-reference-locations.md#s4-3)) passes
> (`apps/client/tests/reference_locations.rs`); the original client's readings (owner item
> [O1](../checklists/owner-observations.md#o1)) are still pending, and with them the
> map-coordinate rounding rule (`coordinates.md` §8.3).

<a id="s10"></a>
## 10. Characters (M4)

Tessera's decisions for bringing characters into the walker and the sandbox backend. Every
format fact is the cited specs'.

<a id="s10-1"></a>
### 10.1 Entity storage

The walker keeps every character in the project's entity store (`tessera-entity`,
decision [0008](../decisions/0008-entity-storage.md)): a networked entity per session
entity ID with an `f64` region-frame transform, the render component (the figure it draws)
and the animation component (an `AnimationScheduler` instance plus an `Animator`).
Caller-side data (session tag, velocity, ground snapping, visibility) is a `Column`. The
store's change lists are taken every frame; removed animation components give their
scheduler instances back. The session's entity events are applied in order each frame:
`EntityCreated` spawns (replacing a live entity of the same ID), `EntityMoved` moves (the
avatar's are ignored: the walk controller drives it), `AppearanceChanged` changes the figure
(for the player it creates the avatar), `EntityDespawned` despawns, `StaticsCleared` despawns
every entity created with a tag, `Disconnected` everything. At most 4,096 events are applied
per frame; the rest wait, in order, for the next frames. The client's queue between the
session and the walker drops moves first when it holds 2^18 events (an entity's next move
replaces them) and never drops creations, appearance changes or removals; those are bounded
by the backend's per-poll spawn and despawn budgets. The queue is filled only while a walker
exists to drain it.

<a id="s10-2"></a>
### 10.2 Figures the landblocks place (closes the weenie question of `entity-descriptor.md` §9 for Tessera)

`tessera-world` marks the **figures** of each landblock bundle and keeps their instance
property sets (the landblock weenie list, `appearance.md` §7.2) within the bundle's
residency charge; the rest of the weenie list is dropped as before:

| Kind | Rule | Shown as |
|---|---|---|
| NPC | A Physics entity that passes the M3 draw rule and whose instance set has `NPC_StartingIdleAnimation`, `Appearance_RandomAppearanceType` or `NPC_Cosmetic_Inventory_Item1` | An animated character dressed from its instance set (`appearance.md` §7.4) |
| Rig | A Physics entity that passes the draw rule, has none of those, and whose setup's skeleton has at least 30 bones (`skeleton-anim.md` §2.4: every such setup is skinned) | An animated character (soldiers in formation, posed figures) |
| Weenie | A Weenie entity (type 6) with an object whose content layers pass | By the `WeenieType` of its game object (`appearance.md` §7.1): `Monster` an animated creature, `RealNPC` an animated NPC, `Door` a static figure (its `PhysObj` drawn rigid), everything else (`Entity` housing hooks, `DistantBattleUnit`, unknown types) a marker, created but not drawn unless `--markers` |

The sandbox serves the figures of the landblocks within 2 landblocks of the player
(Chebyshev; the walker's near ring is 3) through its static streaming (`WorldSource::
static_entities`, same budgets and hysteresis as statics) as `EntityCreated` events. The walker
draws a placed figure either as a character or as a static instance, never both: a figure's
static instance is left out exactly while the character layer draws its entity (the figure
is on the GPU, posed when animated, and within `--character-distance`), and a marker it
hides. Everything else stays a static instance in its bind pose, as in M3: figures beyond
the area, figures whose character is not created yet or still building, figures whose
assembly failed, and figures the sandbox's hysteresis ring (radius + 1) still holds beyond
the character distance. Interior figures follow `--interiors`.

<a id="s10-3"></a>
### 10.3 Event vocabulary (`tessera-net`, ARCHITECTURE §19.1)

Transport-neutral additions:

- `Event::EntityCreated { id, kind, name, class, location, heading, appearance, tag }`: a
  precise region-qualified location, a heading (radians, 0 = east, counter-clockwise), the
  data record of the entity's class (a placed weenie's 0x70 game object), an `Appearance`,
  and for an entity the world data places a `tag` (the world source's opaque value; for the
  walker the landblock-data DID in the high 32 bits and the figure's index in the low 32
  bits), `None` for simulated entities. `StaticsCleared` also removes tagged entities.
- `Event::AppearanceChanged { id, appearance }`: sent for the player right after the world
  entry when the sandbox has an avatar appearance.
- `Appearance { body, scale, equipment, idle }`: `Body::Character { species, sex }` (the data's
  species key and sex value) or `Body::Object(DataId)` (a game object or a visual entity),
  a uniform scale in (0, 100], at most 32 `Equipped { item, dye, secondary_dye, stowed }`
  (dye codes 0 to 1), and the data's starting-idle value. The session drops created entities
  and appearances that fail `Appearance::is_valid`, a non-finite heading or an invalid
  location.
- A placed figure's heading is the heading of its rotation (`Rotation::heading`: the model's
  +Y axis rotated, projected on the ground). Its appearance is `Body::Object` of the placed
  object (the 0x47, or the weenie's 0x70) with the instance set's starting idle and scale 1
  (the instance set's `Render_AdjustableScale` is part of the assembled model scale,
  `appearance.md` §6.1), or the entity's own scale when it has no instance set. The client
  finds the instance set again through the tag.

<a id="s10-4"></a>
### 10.4 Figures: assembly, parts, materials

One figure per distinct appearance (body, items, dyes, wield states, and for a placed
figure with an instance property set its tag) is built once on the asset workers and shared
by every entity that has it; placements of the same object without an instance set share
one figure. Unused figures on the GPU are freed after 30 s; unused figures not on the GPU
(queued, building, built) are dropped at once, so a figure released before it is built is
never built. After a GPU reset (region change) only figures that were on the GPU are
rebuilt. A figure job assembles with `tessera-mesh`
(`assemble_character` for a playable body with its items, `assemble_object` with the
instance set for a placed one), decodes the setup's skeleton, flattens each visible part at
its display level (`mesh.md` §8) skinned to the skeleton where the mesh is skinned, prepares
held items' parts rigid with an attach table of the wearer's holding locations
(`skeleton-anim.md` §8.4), and resolves materials with the part's appearance layers whose
binding key matches the surface (`material.md` §9.3). Materials use the walker's static plan
(diffuse texture, alpha, two-sidedness, tint, lit) plus **one dye channel**: the first
channel with a mask, unless only the second has a colour other than white, its mask the
diffuse alpha inverted (`material.md` §12.2). Textures are the object layer's, shared and
reference-counted. A figure without a skeleton of at most 256 bones, or without a skinned
part, is static (rigid, unanimated). A figure with a skeleton but no playable clip is drawn
in the skeleton's reference pose, with held items at their holding locations' reference
transforms. Bone sliders are not applied (Decision: default
proportions; their application is partly OPEN in `appearance.md` §6.3).

<a id="s10-5"></a>
### 10.5 Clips, speed and root motion

- **Selection** (`script-table.md` §11.3, the `tessera-formats` resolver): idle plays every
  looping candidate of the idle state, the most likely per data layer (at most four),
  layered in the data's layer order with per-track weights where the node asks for them
  (`UseBoneOpacity`): a figure whose base idle is a stance on layer 10 shows the layer-30
  stance above it. Walk and run are the most likely candidates on the `movement` layer
  (else of any layer that is not an idle clip); a run that is the walk clip, or no faster,
  is dropped.
- **Speed**: walk and run form a phase-synchronised speed blend anchored at their natural
  ground speeds (|`LocomotionDisplacement`| / duration × node rate; 2 and 6 m/s for clips
  without displacement, the walk controller's speeds). The locomotion layer fades in over
  0.2 s above 0.3 m/s and out below 0.15 m/s, idle layers above it fade out while moving;
  the playback rate is ground speed / natural speed (clamped 0.3 to 2.5). The avatar's
  ground speed is the walk controller's; simulated characters' the velocity of their last
  move; placed figures stand.
- **Root motion**: locomotion and idle clips are treadmill clips (`skeleton-anim.md` §7.2):
  played in place, movement comes from the controller or the network; extracted motion is
  not applied.
- **Sampling**: each frame first collects the poses sampled for the previous frame, then
  drives every animated character within 200 m (`--anim-distance`) and submits one sampling
  job each to the workers (ARCHITECTURE §20.3 step 6); poses are one frame old, and
  characters beyond the distance keep their last pose. Every animated character gets one
  job for its first pose wherever it is, and is not drawn until that pose arrives (never in
  its bind pose); a placed figure meanwhile stays a static instance (§10.2). Decoded clips are cached per DID and
  baked (every frame sampled once) within a 256 MiB budget. Animators start at a phase
  derived from the entity ID so neighbours do not move in step.

<a id="s10-6"></a>
### 10.6 Avatar

`--avatar` (default `human,m`) is resolved against the race table's species labels
(case-insensitive) or keys; an unknown species falls back to the default body with a
warning. Items are worn or held per `appearance.md` §4–§5. Held items are stowed by category
unless `@wielded` (the avatar is never in combat, and `appearance.md` §5.4 decides that out
of combat held items are stowed; until 2026-10-05 the walker wielded them by default). The sandbox sends the appearance after the world entry; the avatar is drawn at
the walk controller's feet and heading (model +Y forward), on the terrain.

<a id="s10-7"></a>
### 10.7 Crowd and wanderers (sandbox)

`--crowd N` places N simulated characters on a grid in front of the spawn:
`ceil(sqrt(2N))` columns `--crowd-spacing` apart across the spawn heading, rows from
`--crowd-ahead` metres in front outwards; member *i* stands (facing the spawn), walks
(2 m/s) or runs (6 m/s) in a circle around its grid point (radius 0.4 or 0.8 × spacing)
by *i* mod 3, starting at its own phase (golden-angle steps), with the *i*-th playable body
of the race table (cycled). Members follow teleports. The sandbox's wanderers become
characters the same way when it is given wanderer appearances (the walker uses none).
Simulated characters stand on the walker's terrain (their height is the spawn's in the
session).
