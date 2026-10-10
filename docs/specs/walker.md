<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The offline world walker (`apps/client`, ROADMAP M3–M4)

| | |
|---|---|
| **Status** | §1–§10: M3–M4 design, implemented; the M5 rules for a game UI attached (the items marked M5 in §5 and §6, from the accepted decisions 0014 and 0015; added 2026-10-06, awaiting review of this change) are not implemented yet. §11 (the original's distances and camera): **Accepted** (2026-10-07 revision re-reviewed and passed; implementers re-check the §11.4 step 4 OPEN). §13–§14: M4.5 design (walk mode on collision; world fidelity, interiors), implemented; §14.5 and §14.6 revised 2026-10-09 (spec writer: three OPENs for water features), review of 2026-10-09 passed: **Accepted**; §14.6 **Accepted** 2026-10-10 (review of the lava revision passed, see the review record; before: **Revised** 2026-10-10, spec writer: both OPENs name the lava record); otherwise **Accepted** (fifth revision of 2026-10-08: body brought in line with the current code, implementer OPENs folded in, implementer choices labelled as Decisions; re-review passed 2026-10-08); §13.6 revised 2026-10-09 (spec writer: the implementer OPEN of 2026-10-08 answered by restating the `blocked` and slope-allowance readings); review of 2026-10-09 passed: **Accepted**. §7 and §10.8: Tessera rules added 2026-10-09 by the spec writer (settle wait, audio columns, overlay on the dispatcher); review of 2026-10-09 passed |
| **Last updated** | 2026-10-10 (spec writer, dirty room: lava in the §14.6 OPENs). Before: 2026-10-09 (spec writer, dirty room: water-feature OPENs in §14.5 and §14.6; before that, spec writer: implementer OPENs of §7, §10.8 and §13.6 answered; before that, the same day, merge of main into remediation-2: main's M5 rules of §5 and §6 and implementer OPENs in §7, §10 and §13.6 combined with the fifth revision; earlier, 2026-10-08, spec writer, dirty room: fifth revision of §13.3, §14.3–§14.5, §14.7, §14.8, §14.11–§14.13; see §15) |
| **Author role** | §1–§10: Clean-room implementer of `tessera-client`. §11: Spec writer (dirty room). §13–§14: clean-room implementer (M4.5), revised 2026-10-07 by a spec writer (dirty room), who also wrote §14.12–§14.13. §15: change log |
| **Reviewer** | §11: Claude review session (spec-writer agent). §13–§14: Claude review sessions (spec-writer agent, M4.5 criterion 9), independent of the writer |
| **Review record** | §11: 2026-10-05 changes requested, re-review **passed**; 2026-10-07 re-review of the revision: **passed**. §13–§14: 2026-10-07 first review and re-review: changes requested; 2026-10-07 re-review of the spec writer's revision: **changes requested**; 2026-10-07 re-review of the second revision (Claude review session, spec-writer agent): **changes requested** (one unlabelled departure in §14.3); 2026-10-07 re-review of the third revision (Claude review session, spec-writer agent): **passed, Accepted**; non-blocking notes applied in a fourth revision (Revised); 2026-10-07 re-review of the fourth revision (Claude review session, spec-writer agent): **changes requested**; 2026-10-08 re-review of the fifth revision (Claude review session, spec-writer agent): **passed, Accepted**. §11 unchanged: Accepted. 2026-10-09 review of the spec writer's change of §7, §10.8 and §13.6 (Claude review session, spec-reviewer agent, independent of the writer; CLEAN_ROOM.md §5.3): **passed**, §13.6 Accepted again. Spot-checked read-only: the §13.6 first-touch and slope-allowance wording agrees with `m4-5-visual.md` §2.2 (sampling at the axis and on rings of *r* and *r*/2, 16 directions; 0.05 m tolerances); decision 0020 and `chat-window.md` §10 items 2, 11–13 exist as cited; the settle-wait rule and the four audio columns match what the implementation does. No client fact added, no code, no game text. Non-blocking: the §7 frame cap could say that it counts start-up frames too. Details in the dirty-room review log (decision 0018). Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: review of the water-feature revision on branch `water-fix` (`git diff main...water-fix`, commits 88edc8f and 4be89e7) against CLEAN_ROOM.md §5.3 (OPENs at the end of §14.5 and §14.6, change log): **passed**, §14.5 and §14.6 Accepted (every item). The three OPENs are implementer tasks that point to `water.md` §8.5–§8.7, `material.md` §8.4, §10.3, §11.6 and `render.md` R-D12–R-D14; the statements about the current walker (parameters evaluated once at clock 0, generic path for placed water) describe Tessera's own code; no client fact, code, copied prose or game text. Checklist rows cited (W10–W14, W15–W17 of `m4-5-visual.md` §4.8) match. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: independent re-review of the whole `water-fix` branch (`git diff main...water-fix` up to commit 8c578bc) against CLEAN_ROOM.md §5.3. The water-feature text of this spec did not change after its acceptance of the same day: **passed**, stays Accepted. Non-blocking: the §14.6 OPEN on R-D12 names only the two flow-map records; landblock lava (0x2B000810) is drawn by `water.md` §8.6.5 (recorded in `water.md` §13); add it at the next revision. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: re-review of the whole `water-fix` branch up to commit 113462a (`git diff main...water-fix`) against CLEAN_ROOM.md §5.3. §14.5–§14.6 did not change after their acceptance: **passed**, stay Accepted; the lava note above stands (the §14.6 curve OPEN's census list also omits 1186 and 1187 of 0x31006FAE, now in `water.md` W37). Claude review session (spec reviewer, fresh session; not the writer), 2026-10-10: review of the revision of commit 9e9a29a (§14.6 both OPENs, §15 row) against CLEAN_ROOM.md §5.3: **passed**, §14.6 Accepted again (§14.5 unchanged, stays Accepted). The OPENs point to `water.md` §8.6.4 (per-record colour), §8.6.5 (lava) and `material.md` §11.6 (lava recognition) and list 1186 and 1187 of 0x31006FAE among the curve-valued parameters, as `water.md` W37 does; no new client fact, no code; nothing under a closed gate. The lava note of the previous rounds is resolved. |
| **Implements into** | `apps/client/src/walker/` (M4.5: `interiors.rs`, `env.rs`, `terrain.rs`, `objects.rs`, `movement/`; M4: `walker/characters/` — layer, `figure.rs`, `catalog.rs`, `motion.rs` — and `walker/figures.rs`), `apps/client/src/walker_host.rs`, `apps/client/src/headless.rs`, `apps/client/src/avatar.rs`, `apps/client/src/net.rs`, `crates/net` (sandbox figures, crowd, entity events), `crates/world` (bundle figures), `fuzz/fuzz_targets/client_walker_text.rs`, `client_avatar_spec.rs`, `net_npc_events.rs`, `world_bundle.rs` |
| **Format facts from** | `coordinates.md` (§3, §8–§12), `landblock.md` (§5, §6.3, §8), `entity-descriptor.md` (§5–§9), `setup.md` (§7), `mesh.md` (§8, §11), `material.md` (§9–§12), `terrain.md`, `keymap.md`, `wstate.md` (§11); M4: `appearance.md` (§2–§7), `skeleton-anim.md` (§2, §4, §7, §8), `script-table.md` (§6, §8, §11); M4.5: `landblock.md` (§4.4, §4.11–§4.14), `lighting.md` (§8.1, §8.3, §8.5, §9, §10), `terrain-texturing.md` (§4.8–§4.10, §10), `collision.md` (§5.8.1, §10–§12), `environment.md`, `water.md`, `scenery.md` (§8); the client facts §13–§14 rely on are restated with their provenance in §14.13 |

This file records **Tessera's own design decisions** for the world walker: how it streams,
draws and reports the world, and how it is driven in automated runs. §1–§10 add no format
facts; the questions they could not settle are collected in §9. §11, written in the dirty
room, states what the original client does for draw distances, detail levels, landscape
extent and the login camera, so the walker can match it at default graphics settings. §13–§14
are the M4.5 design; every client fact they rely on is restated in §14.13 with its
provenance, and every place where the walker departs from the client is marked there and in
the text as a **Decision** or an `OPEN` for the implementer.

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
| `--walk-path FILE`, `--walk-log CSV` | Scripted walks on collision, headless without a GPU (§13.6): a walk file or the checklist's walk table; the movement log per 1/60 s step. |
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

> **Superseded in part (M4.5):** the worker split is now that of §14.1 (terrain workers a
> quarter of the hardware threads, 2 to 6, plus a resolver thread), and the 30 Hz controller
> step that of §13.1 when collision is built. The anchor, the controller origin and the
> upload budget above still apply.

<a id="s3"></a>
## 3. Terrain

Per resident bundle a worker builds the `TerrainBlock` (forcing its level error bounds), a
33 × 33 height-shaded overview tile (§6) and the object instances (§4). Levels are chosen per
frame with `LodSelector` for the actual viewport; a block is re-meshed when its level, its
neighbours' levels or the set of neighbours its normals saw changes, the old mesh drawn until
the new one is uploaded. Shading is `terrain.md` §6 (white lit material, vertex colours).
The far ring (`--view-blocks`, default 10 landblocks) bounds the terrain draw distance; near
and mid rings are 3 and 6 landblocks, hysteresis one landblock.

> **Superseded in part (M4.5):** shading is now §14.4 (texture layers through the lit
> pipeline, ceilings, chunks without layers, far band); the "white lit material" above is the
> height-shaded fallback of §14.4 only. Level choice, re-meshing and the rings still apply.

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

> **Superseded in part (M4.5):** the object range, detail level and materials are now §14.5
> (draw distance by LOD class with the distance cap `--object-distance`, default the
> landscape extent; every detail level; lit surfaces with normal maps, specular and dye) and
> interior entities follow §14.12. Still in force: the texture cap, the GPU-memory budget
> and the shrinking cap above, and the culling (including the 1.5 px projected-radius test,
> a Tessera choice).

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
  F3 toggles HUD details, F12 saves a screenshot to `<data dir>/screenshots/`, Escape quits
  (without a game UI attached; see the next item).
  Bindings come from Tessera's keymap file over the built-in defaults (`keymap.md` §6).
- **Escape with a game UI attached** (M5; decision 0014, Consequences "Escape with a UI
  attached", and its owners list). "Escape quits" holds only while no game UI is attached
  (the walker of M3 and M4). With a UI attached, Escape gives `menu` (command `EscapeKey`),
  which is offered to the UI first; when the UI does not consume it, the client's reaction
  to its press is game code's handling of `EscapeKey` (`game-panels.md` §16: hide the active
  on-demand panel, otherwise show or hide the main menu), never a quit. How the client is
  quit while a UI is attached is outside this rule. The HUD help line (`help_text` in
  `apps/client/src/walker/controls.rs`) labels `menu` "menu" instead of "quit" while a UI is
  attached, and keeps "quit" without one (0014, Cost).

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
- **The walker's overlays with a game UI attached** (M5; decision 0015 Decision 1.6, "Walker
  overlays with the game UI attached", and its owner question 6, answered "yes"). The HUD text
  and the walker minimap draw **under** the game UI: after the world passes and before the UI
  pass (`render.md` §6.9), so they never cover a native panel or a plugin window.
  - The walker minimap is **off** by default while a game UI is attached, because the game's
    own minimap (P1 of `ui-layout.md` §17.2) takes its place. The Tessera setting
    `walker_minimap = auto | on | off` controls it; `auto` (the default) means off with a game
    UI and on without one. When it is on, it still draws under the UI.
  - The HUD is switchable with the Tessera setting `walker_hud = on | off` (default `on`, for
    development). HUD text under semi-transparent panel art would show through it, so the M5
    visual checklist run (`docs/checklists/m5-visual.md`) and the owner's comparison
    screenshots use `walker_hud = off`; the scripted performance run of decision 0015 §6 reads
    `FrameStats` instead of the HUD.
  - Without a game UI attached nothing changes: there is no UI pass, and both overlays draw on
    top of the world as described above.
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
- **Settle wait** (Tessera rule, 2026-10-09). The settled rule above is the same for every run;
  what differs is when a run gives up waiting for it. The command line uses a **wall-clock**
  limit (`--settle-timeout`). Library callers (tests through `tessera_client::headless`) may
  choose a **progress** limit instead: the run gives up only after a given number of
  consecutive frames in which no loading work finished or arrived, or after a frame cap, with no
  wall-clock limit. "Loading work" is read from the completion and outstanding-work counters of
  the streamer, terrain, objects, characters (sampled poses included), water, frills, sky and
  the resolver. Real-data tests that assert the settled state use the progress limit, so a
  heavily loaded machine that is still making progress does not fail them; both numbers are
  the caller's parameters. The command line gains no option for it.
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
- Audio columns (M6, appended last, after the M4.5 columns of §14 that end in
  `frills_draw_ms`, so that readers of the earlier columns keep working): `audio_ms`
  (main-thread world-audio update), `audio_voices` (audible voices), `audio_underruns` (device
  underruns so far, cumulative) and `mixer_load` (render time over audio time of the last
  mixer call). With audio on, the summary also reports the mean and longest audio update, the
  most audible voices, the underruns during the log and the highest mixer load.

> **Answered (2026-10-09, spec writer; was OPEN, M6 implementation, 2026-10-07):** the four
> audio columns and the summary lines are folded into the list above.

> **Answered (2026-10-09, spec writer; was OPEN, clean-room implementer, 2026-10-09):** the
> progress-based settle wait is accepted as a Tessera rule ("Settle wait" above) for library
> callers and real-data tests; the command line keeps `--settle-timeout` only.

> **OPEN:** (clean-room implementer, 2026-10-09; engineering choice, no client fact) the
> other waits of library callers follow the same rule. `WorldStreamer::settle_on_progress`
> settles like `settle` but gives up only after a number of consecutive updates in which the
> streamer's cumulative work counters (requests, completed jobs, integrations, evictions,
> releases, cancellations, failures, retries) and its loading, pending and retry counts did
> not change, or after an update cap. The scripted-walk and soak runners (§13.6, §13.7) take
> a `LoadWait`: the command line's `--settle-timeout` as before, or a progress wait that
> gives up after consecutive polls in which neither the walk world's loading counters
> (main and pin streamers, cell-owner index) nor the missing landblocks changed, or after a
> poll cap. Player-music waits count polls in which start-up, a job or a queued command
> finished. The real-data tests use these progress waits; no client behaviour changes.

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

**Answered 2026-10-05 (was OPEN): the original's draw distances, camera placement on login
and object LOD switch distances.** §11 gives them: the landscape radius per
`LandscapeDrawDistance` level and the global distance limit derived from it (§11.3), the
per-class object draw and detail distances with their multipliers and fades (§11.4), the
built-in default settings (§11.2) and the camera-state records with the login placement
(§11.5–§11.7). The values in §3–§5 remain Tessera's own until the walker adopts the
default-settings profile of §11.8.

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

> **OPEN** (implementer, 2026-10-10, for the spec writer; generated rigs): the Rig row names
> "a Physics entity". `scenery.md` §7.6 says the client turns every generated object into
> a static-entity creation request of the same form as the landblock-info statics, so
> Tessera now reads the Rig row as covering generated objects too: a drawn generated object
> whose setup's skeleton has at least 30 bones is a figure (no instance set, no IID, its
> idle from the object's own script table). Before, such objects stayed static instances in
> the skeleton's bind pose (arms straight out), for example the 54 generated figures of
> landblock 3, 0xD7E7 seen from row L5 of `m4-5-visual.md` §4.6; as figures they lie on the
> ground in the pose their data's idle gives. Please confirm the reading (and whether the
> original animates generated rigs at all), or state the rule for generated objects.
>
> **OPEN** (implementer, 2026-10-10, for the spec writer; bind pose beyond the area): the
> exit check of 2026-10-10 also saw figures with arms straight out in rows T11 and T12.
> On this branch the row cameras of T11 and T12 and their surroundings show every served
> figure posed; the figures there without any idle candidate (`§10.4`, reference pose) are
> skinned banners. The remaining source of bind-pose figures is this section's rule that
> figures beyond the served area (more than 2 landblocks from the player) stay static
> instances in their bind pose. Is that the look the original gives at those distances, or
> should such figures be left out or drawn in a pose (for example the first sample of their
> idle)?

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

<a id="s10-8"></a>
### 10.8 Test-server sessions (M7, implementer notes)

What the client does with `--server` (`tessera-protocol.md`, Appendix A); these notes
describe the implementation and list what it leaves open.

- The session uses `tessera_net::remote::RemoteBackend` instead of the sandbox; the walker's
  `WorldSource` still serves statics and placed figures (the protocol does not replicate
  them, `tessera-protocol.md` §9.1). The password comes from `TESSERA_PASSWORD` or a
  prompt without echo, never from the command line, and is never logged.
- Networked characters are placed every frame at their interpolated state
  (`tessera-protocol.md` §8.6, render time = estimated server time − *D*, *D* raised to
  the server's recommended delay), instead of the sandbox path's extrapolation of the last
  move (§10.5). They stand on the walker's terrain and are animated by ground speed as in
  §10.5; a sample whose motion byte says Standing animates as standing.
- A minimal chat overlay (last 8 lines, plain text) and a one-line input: Enter opens it,
  Enter submits the line to the client's command dispatcher (decision 0020, `chat-window.md`
  §10 items 2, 11–13), which says plain text on Say through the session, runs `/play` and the
  other client commands locally and posts an unknown command as a local error line without
  sending anything; Escape cancels; the HUD shows backend, session state, round-trip
  time, clock offset and the interpolation delay. M5 replaces this with the real chat
  window.

> **Answered (2026-10-09, spec writer; was OPEN, implementation, 2026-10-08):** the bullet
> above now says that the overlay feeds the dispatcher.

> **OPEN:** the motion byte's Backward flag and the Airborne gait, and the `STANCE` and
> `EMOTE` properties (`tessera-protocol.md` §7.5), do not change a networked character's
> animation yet: §10.5 has no backward playback, airborne clips, stance idle families or
> emote clips. The walker plays locomotion by ground speed only.

> **OPEN:** `--avatar` does not reach a test server: the avatar's appearance is the one the
> server stores for the character (no `CharacterCreate` or `AppearanceSet` command in
> `tessera-net`'s neutral `Command` set, `tessera-protocol.md` Appendix A OPEN).

> **OPEN:** trust on first use (`tessera-protocol.md` §4.5) is not an in-connection
> prompt: an unknown key fails as `KEY_UNKNOWN`, the client logs and shows the presented
> fingerprint, and the user reconnects with `--server-fingerprint FP` (pin) or
> `--trust-new-server` (accept and record it in `known_servers` next to Tessera's settings
> file). Spec writer: confirm this counts as the user's acceptance, or specify a prompt.

> **OPEN:** the chat input opens on the Enter key directly, not through the keymap's
> `open_chat` action and `text_entry` context (`keymap.md`): the walker's controls do not
> use contexts for text yet.

---

<a id="s11"></a>
## 11. The original client's distances, detail levels and camera (spec writer, dirty room)

This section adds format and behaviour facts. It lets the walker reproduce how far the
original draws the landscape, objects and characters, when objects change detail level and
fade, and where the third-person camera stands when the player enters the world, at the
original's built-in default graphics settings. Every value that comes from the data is
read at run time; the numbers quoted from this install are observations. Numeric constants
marked BIN come from the client's code and may be used under
[decision 0012](../decisions/0012-numeric-constants.md).

<a id="s11-1"></a>
### 11.1 Provenance

Legend: **Confirmed** = observed in the user's install on the date given; **Reported** =
published sources only; **BIN** = read from the client's code only (method M4); **Inferred**
= the spec writer's reading of BIN or data, not checked against the running client.

| Tag | Source |
|---|---|
| BIN-W | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** `x64/lotroclient64.exe` of the user's install, file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5, disassembled with objdump in the dirty scratch area on 2026-10-05, by address range. **Questions:** (a) which levels the preferences `ModelDetail`, `ObjectDrawDistance`, `LandscapeDrawDistance`, `DistantImposters` and `FrillDistance` have, their built-in initial values, and the quality presets the client compares them with; (b) how `LandscapeDrawDistance` becomes a landscape radius; (c) the source of the global object distance limit that `mesh.md` §8.7 calls *G*; (d) the field order and field types of camera-state records (prefix 0x08); (e) how the client chooses the camera state of the player's third-person camera; (f) how that camera sets its yaw, pitch, distance and look-at height when it becomes active, and its pitch limits and target fade. **Why M1–M3 were not enough:** the preference levels and the mapping from a level to a distance exist only in code; the camera records carry values without any field markers, and two of their fields cannot be named from the data; no frame capture or running client was available to this session (M3); the published tools report only their own modified values (FC). Only offline client code was read (preference registration, scene and landscape setup, camera controllers). No network, login, launcher or anti-tamper code was examined. **Second session (same build, same SHA-256 prefix, 2026-10-05, answering the review of §11):** (g) which LOD-class values an object gets when no `Render_LODClass` value reaches it: how the render component receives property values (its watch list, the change notifications of the entity's layered property collection, and the collection's lookup); (h) the complete condition of §11.3 step 3 and where its two fog values come from; (i) the look-at height used when `Entity_Camera_Lookat_Height` is absent, the switch that selects the under-water height factor, and the conditions under which the target's heading is added to the yaw at activation. **Why M1–M3 were not enough:** the data shows only which records set a value (census in OBS-W); whether the client then falls back to the property dictionary's default or to the render component's own initial values, and which flags gate the fog clip and the yaw, exist only in code; no running client or frame capture was available. Only offline code was read (property collections and their change notifications, the render component, the frame's fog state, the third-person camera controller). No network, login, launcher or anti-tamper code was examined. |
| OBS-W | **M1**, user install, 2026-10-05: all **11** camera-state records (prefix 0x08, all in `client_general.dat`) decoded to the last byte; the DID maps 0x28000001 (EMAPPER) and 0x28000002 (UNIQUEDB, `property-system.md` §8); the enum mappers 0x230000d4 (`CameraController`), 0x230000d8 (`EngineLOD`), 0x2300008d (`RenderLODClass`) and its base 0x23000165; the LOD-class table 0x32000000 (32 entries); `Render_LODClass` and `Entity_Camera_*` over **all 40,643** entity descriptors (0x47). Second session (2026-10-05): `Render_LODClass` along the 0x47 parent chain (`setup.md` §3.1 step 2) for all 40,643 descriptors; `Render_LODClass` in the own property set and in the instance property set (`entity-descriptor.md` §4.5, §4.5.2) of **all 2,380,699** Physics SEDs of all landblock-info records, compared with the class of the 0x47 chain they name; `Entity_Camera_ForceUnderwater` over all 40,643 descriptors; the type-2, type-3 and type-4 bodies of all 7 such records decoded with the tables of §11.5.3. Repository decoders and throwaway probes were used as tools. |
| PREF | The owner's own `UserPreferences.ini` (`install-layout.md` §8.3), read-only, 2026-10-05. |
| FOV | M2: the public "lotro FOV mod" cheat-table README (mklinik): the third-person value is 45 and is combined with the screen aspect ratio. Facts only. |
| FC | M2: the public "lotro freecam toolkit" script comments: landscape bands at 14 and 24 landblocks and a resident-landblock ceiling of 16–20 landblocks. These are the tool's own overrides, not the client's defaults. Facts only. |
| MESH, SCN, LGT, ENV, REC | `mesh.md` §8.4–§8.8 (detail-level rule, draw distance, fade); `scenery.md` §8.3 (frill radius); `lighting.md` §10 (fog limits); `environment.md` §9.1 (LOD-class table layout); `records.md` §2.1, §7, §8 (DB types, enum mappers, DID maps). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| W1 | Level names and stored values of the five distance preferences; their built-in initial values; the built-in initial values of `FrillDensity` (1.0), `PlayerCrowdQuality` (1.0; the client's own registration of the setting states the range 0.0 to 2.0) and `DialBackGraphics` (false) (§11.2) | BIN-W (a); PREF (names as stored in the file) | BIN; names Confirmed 2026-10-05 |
| W2 | Six quality presets the client compares the settings with (§11.2.2) | BIN-W (a) | BIN |
| W3 | `LandscapeDrawDistance` → far radius 2, 4, 6, 9, 12 landblocks; far group count ⌊R/3⌋ + 1 (§11.3) | BIN-W (b) | BIN |
| W4 | Near landscape radius 1 landblock (§11.3) | BIN-W (b) (the only value the client writes was found) | BIN; completeness OPEN |
| W5 | Landscape extent E = 160 m × (3 g + n); global limit *G* = E, clipped by fog far under a condition, at least 60 m (§11.3) | BIN-W (c), (h) (the complete condition of step 3); LGT | BIN |
| W6 | Object draw distance min(*G*, `d0` × *M_d*) with a 1.5 s fade; detail range `d1` × *M* (§11.4) | MESH; BIN-W (c) for *G* | BIN |
| W7 | LOD-class census: 33,018 of 40,643 entity descriptors set `Render_LODClass`; the classes used and their `d0`, `d1` (§11.4.1) | OBS-W | Confirmed 2026-10-05 |
| W8 | The 14 playable bodies of `appearance.md` §2.2 all carry class `PlayerAvatar` (`d0` 80, `d1` 26) and a look-at height of 1.07 to 1.8 m (§11.4.2, §11.7) | OBS-W | Confirmed 2026-10-05 |
| W9 | Camera-state record layout: header, four flags, controller type, controller body by type, 44-byte target block (§11.5) | OBS-W (11 of 11 parse with zero leftover bytes); BIN-W (d) (field order and types) | Confirmed (layout); BIN (field meanings) |
| W10 | Controller types 1 ThirdPerson, 2 FirstPerson, 3 Cinematic, 4 Flight (§11.5.2) | OBS-W (mapper 0x230000d4); BIN-W (d) (the third-person object stores type 1) | Confirmed |
| W11 | Two unnamed f32 fields between the third-person smoothing times and the target block (§11.5.3) | OBS-W (sizes); BIN-W (d) (not among the named fields) | Confirmed (position); meaning OPEN |
| W12 | Camera state per controller type through UNIQUEDB keys 0x0A, 0x2E, 0x8D; the game layer replaces the third-person key by 0x10000041 (§11.6) | BIN-W (e); OBS-W (keys resolve) | BIN; keys Confirmed |
| W13 | Activation of the third-person camera: yaw, pitch, clamped distance, target opacity 1 (§11.7) | BIN-W (f) | BIN |
| W14 | Look-at height = height factor × look-at height × the entity's z scale, from property `Entity_Camera_Lookat_Height` (§11.7) | BIN-W (f); OBS-W (26 descriptors carry the property) | BIN; census Confirmed |
| W15 | Pitch clamp to [low, high] when the constrain flag is set; angles kept in degrees (§11.7) | BIN-W (f) | BIN |
| W16 | Target fade: within the fade distance the target is drawn at the fade opacity (§11.7) | BIN-W (f) | Inferred from BIN |
| W17 | Third-person field of view 45 in the in-world camera | OBS-W (record 0x08000002); FOV | Confirmed (value); axis OPEN |
| W18 | Built-in defaults reproduce no preset exactly: they equal preset 3 except `ObjectDrawDistance` (§11.2.2) | BIN-W (a) | BIN |
| W19 | Landscape bands at 14 and 24 landblocks | FC | Reported; disagrees with W3–W5 (§11.10) |
| W20 | An object to which no `Render_LODClass` value is delivered keeps the render component's initial values (`d0` 200, `d1` 60, `d2` 16, `d3` 80, `d4` 1,000, `d5` 1,000; in record order `d6` −1 and `d7` 1,100, as `mesh.md` row M35 traced on 2026-10-07; this row first gave them as "1,100 and −1, correspondence not traced"); the property dictionary's default (class 2) is **not** applied; a class without a table entry leaves the previous values; a class value removed at run time is replaced by the dictionary default (§11.4.3) | BIN-W (g); `mesh.md` row M35 (the order of `d6` and `d7`) | BIN |
| W21 | Class resolution census: of the 7,625 descriptors without an own class, 607 find one along the 0x47 parent chain; of the 2,380,699 Physics SEDs, 454,207 set a class in their own set (the instance set agrees wherever it has one), 333,933 of them differ from the class of the 0x47 chain they name, and 248,393 have no class anywhere (§11.4.3) | OBS-W | Confirmed 2026-10-05 |
| W22 | Precedence of the SED's class over the 0x47 chain's class, and whether the client follows the 0x47 parent chain for this property (§11.4.3) | BIN-W (g) (layered lookup, first layer holding a value wins); which layer holds which set not traced | OPEN; Tessera Decision |
| W23 | Fog clip of *G* (§11.3 step 3): switch *S* off, or (the fog state's second flag set and its second value > 0.99); in the keyframe-blending path the flag is also set when the blended value exceeds 0.001 | BIN-W (h) | BIN; meaning of *S* and of the value OPEN |
| W24 | Look-at height fallback, under-water selector and yaw rule at activation (§11.7 steps 1–2) | BIN-W (i); OBS-W (no descriptor carries `Entity_Camera_ForceUnderwater`; all four type-1 records store 0 in both mouse-look fields) | BIN; parts OPEN |
| W25 | Byte layouts of the type-2 (65 bytes), type-3 (12 bytes) and type-4 (70 bytes) controller bodies (§11.5.3) | OBS-W (7 of 7 records decode with the tables to the last byte); BIN-W (d) (field order and types) | Confirmed (layout); BIN (field meanings) |
| W26 | Reading the stored level name case-insensitively and treating an unknown or missing value as the built-in initial value (§11.2) | Tessera Decision (the client's parsing of the stored name was not traced) | Decision |

<a id="s11-2"></a>
### 11.2 Graphics preferences that set distances

The client keeps these settings in `UserPreferences.ini` section `[Render]`
(`install-layout.md` §8.3) under the key names below, and writes the **level name**, not the
number (PREF: `ModelDetail=UltraHigh`, `LandscapeDrawDistance=VeryHigh`). **Tessera
Decision** (W26; how the client parses the stored name was not traced): read the name
case-insensitively; an unknown or missing value means the built-in initial value.

| Key | Stored value → level name | Built-in initial value | Multiplier or radius per level |
|---|---|---|---|
| `ModelDetail` | 0 Low, 1 Medium, 2 High, 3 VeryHigh, 4 UltraHigh | 2 (High) | *M* = 0.4, 0.7, 1.0, 1.4, 2.0 (`mesh.md` §8.5) |
| `ObjectDrawDistance` | 0 Low, 1 Medium, 2 High, 3 VeryHigh, 4 UltraHigh | 2 (High) | *M_d* = 0.6, 0.8, 1.0, 1.4, 2.0 (`mesh.md` §8.7) |
| `LandscapeDrawDistance` | 0 VeryLow, 1 Low, 2 Medium, 3 High, 4 VeryHigh | 3 (High) | far radius *R* = 2, 4, 6, 9, 12 landblocks (§11.3) |
| `DistantImposters` | boolean | true | distant landscape imposters (OPEN, §11.9) |
| `FrillDistance` | 0 Disabled, 1 Low, 2 Medium, 3 High, 4 VeryHigh, 5 UltraHigh | 2 (Medium) | frill radius per level (`scenery.md` §8.3) |
| `FrillDensity` | float | 1.0 | `scenery.md` §8 |
| `PlayerCrowdQuality` | float, 0.0 to 2.0 (the range the client's own registration of the setting states; whether values outside it are clamped was not traced) | 1.0 | use not traced (OPEN) |

Note the different scale of `LandscapeDrawDistance`: its five levels run from VeryLow to
VeryHigh, so "High" is stored as 3 there and as 2 for the other two distance settings.

`DialBackGraphics` (boolean, built-in false) lowers the `ModelDetail` level by two before
the multiplier is taken (`mesh.md` §8.5). Tessera may ignore it.

The owner's file (PREF) holds `ObjectDrawDistance=UltraHigh`, `ModelDetail=UltraHigh`,
`LandscapeDrawDistance=VeryHigh`, `FrillDistance=UltraHigh`: the owner's choice, not the
default. Comparisons with the owner's screenshots must use these values, not §11.8.

<a id="s11-2-2"></a>
#### 11.2.2 Quality presets (BIN)

The client compares the current settings with six presets (numbered here 0 to 5, from the
lowest; their display names were not traced). For the settings of this section:

| Preset | `ModelDetail` | `ObjectDrawDistance` | `LandscapeDrawDistance` | `DistantImposters` | `FrillDistance` |
|---|---|---|---|---|---|
| 0 | 0 Low | 1 Medium | 0 VeryLow | false | not read (compared with a run-time value) |
| 1 | 0 Low | not compared | 1 Low | false | 1 Low |
| 2 | 1 Medium | 2 High | 2 Medium | true | 2 Medium |
| 3 | 2 High | 3 VeryHigh | 3 High | true | 2 Medium |
| 4 | 2 High | 3 VeryHigh | 4 VeryHigh | true | 3 High |
| 5 | 4 UltraHigh | 4 UltraHigh | 4 VeryHigh | true | 5 UltraHigh |

The built-in initial values equal preset 3 except `ObjectDrawDistance` (built-in 2, preset 3).

> **OPEN:** Which preset, if any, the client applies on first start (it records
> `MaxHardwareClass` and `MeasuredFillRate`, which suggests a hardware test), and the
> presets' display names. Until an owner observation of a fresh settings file exists,
> "default graphics settings" in this spec means the **built-in initial values**.

> **OPEN (implementer, 2026-10-06, `tessera-world` draw-distance module):** the table above
> has no `StencilShadows` row, so the built-in initial value of `StencilShadows` (and so
> *M_s* of `mesh.md` §8.7 at default settings) is not stated anywhere; `mesh.md` §8.7 uses 3
> only in a synthetic example. Tessera (Decision): 3 (*M_s* = 1.0) until the spec owner
> adds the row.

<a id="s11-3"></a>
### 11.3 Landscape extent and the global distance limit *G* (BIN)

**Landscape radius.**

1. The far radius *R*, in landblocks, follows from `LandscapeDrawDistance`: VeryLow 2, Low 4,
   Medium 6, High 9, VeryHigh 12. (A debugging switch of the client can replace *R* by a
   fixed value; Tessera ignores it.)
2. The far group count is *g* = ⌊*R* / 3⌋ + 1: the far landscape is handled in groups of
   three landblocks.
3. The near radius *n* is 1 landblock.
4. The landscape extent is *E* = 160 m × (3 *g* + *n*).

| Level | *R* | *g* | *E* |
|---|---|---|---|
| VeryLow | 2 | 1 | 640 m |
| Low | 4 | 2 | 1,120 m |
| Medium | 6 | 3 | 1,600 m |
| High (built-in) | 9 | 4 | 2,080 m |
| VeryHigh | 12 | 5 | 2,560 m |

**Global limit *G*.** Each frame, before objects are drawn:

1. *G* starts unbounded.
2. When a landscape is present in the scene, *G* = *E*.
3. *G* = fog far when **all** of these hold (W23):
   - distance fog is on in the frame's fog state;
   - *G* is greater than the frame's fog far distance (the blended fog far of `lighting.md`
     §10, after its cap at 10,000);
   - **either** a switch *S* of the scene being drawn is off, **or** both of the following
     hold: a second flag *F* of the frame's fog state is set, and a second value *V* of the
     frame's fog state is greater than 0.99.
4. If *G* < 60 m, *G* = 60 m.

*G* is the global limit of `mesh.md` §8.7: no entity is drawn beyond it (§11.4).

Where *F* and *V* come from (BIN): the frame's fog state holds, besides the on switch, the
colour and the near and far distances, a flag *F* and a value *V*. When the fog state is
taken from one source, *F* is that source's flag and *V* its value (0 when its flag is clear).
When a whole fog state is copied from another, *F* and *V* are copied unchanged. When it is blended from several keyframes, *V* is blended exactly like the near and far
distances, and afterwards *F* is set when either the source's flag was set or the blended *V*
exceeds 0.001. In the blending path *F* is therefore always set when *V* > 0.99, and the
condition reduces to "*S* off, or *V* > 0.99". The same *F*-and-*V* > 0.99 test elsewhere
in the frame skips a drawing step (Inferred: the scene is treated as fully fogged).

> **OPEN:** The meaning of *S* (a flag of the scene object being drawn; the same flag also
> gates a step of the frame that runs only while the fog is not fully opaque; Inferred: the
> scene has a sky or background, so that objects past the fog far distance can still show
> against it) and which keyframe or record field *V* is (`environment.md` §5.3 lists
> `fog_extra` with observed values 0 to 0.8, which would never pass the 0.99 test; whether
> *V* is that field was not confirmed). With *S* off, objects end at the fog far distance
> whenever fog is on; with *S* on they end there only in fully fogged frames. **Tessera
> Decision** until known: apply step 3 as if *S* were off (clip at fog far whenever fog is
> on), because objects beyond fog far are invisible anyway.

> **OPEN:** Whether anything other than the reset path sets the near radius *n* (BIN found
> only the value 1), and how the landscape inside *E* is meshed: the near ring at full
> resolution and the far groups coarser (the client has an engine LOD enum with labels
> `Near`, `Minimal`, `Far`, mapper 0x230000d8), or all at full resolution. Also which
> landblocks' static objects are loaded (the near ring only, or all of *E*). **Inferred:**
> terrain is drawn out to *E*; the walker may draw its terrain to *E* with its own level
> selector (§3).

<a id="s11-4"></a>
### 11.4 Objects and characters: draw distance, detail switch, fade

For an object drawn through an entity descriptor with LOD-class entry `d0` … `d7`
(`mesh.md` §8.5; layout `environment.md` §9.1), with *M* and *M_d* from §11.2 and *G* from
§11.3:

1. **Draw distance** *D* = min(*G*, `d0` × *M_d*). The object is drawn while its
   **unsmoothed** distance (`mesh.md` §8.6, nearest point of its world box) is at most *D*;
   crossing *D* starts a fade in or out of 1.5 s (`mesh.md` §8.7, row M25). The smoothed
   copy of the distance feeds only the detail factor of step 2. (Corrected 2026-10-07 after
   `mesh.md` §19; this step first said "smoothed".)
2. **Detail range** *R_d* = `d1` × *M*; the detail level follows `mesh.md` §8.4 (the last
   level from *R_d* on).
3. **Player's own character:** `d0` … `d5` × 3 (`mesh.md` §8.5).
4. **No class reaches the object** (none of its property sets holds `Render_LODClass`,
   §11.4.3): the render component's initial values stay in force, `d0` = 200 m, `d1` = 60 m
   (`d2` 16, `d3` 80, `d4` 1,000, `d5` 1,000, `d6` −1, `d7` 1,100 in record order; the last
   two were traced by `mesh.md` row M35, 2026-10-07). The property
   dictionary's default (class 2: `d0` 50, `d1` 10) is **not** used (W20, BIN). A class
   value without an entry in the table leaves the values the object already had (for a new
   object, these same initial values).

At the built-in defaults *M* = *M_d* = 1, so *D* = min(*G*, `d0`) and *R_d* = `d1`.

> **Answered (spec writer, 2026-10-07):** step 1 now says unsmoothed, as `mesh.md` §8.7
> (row M25, BIN) and its §19 correction give. The original question (implementer,
> 2026-10-06, `tessera-world` draw-distance module): step 1 said the draw test uses the
> smoothed distance, but `mesh.md` uses the unsmoothed *d*; Tessera already followed
> `mesh.md`.

> **OPEN (spec writer, 2026-10-07, for the `tessera-world` implementer):** the built-in
> initial values of the draw-distance module list the last two fields as 1,100 then −1;
> in record order they are `d6` −1 and `d7` 1,100 (step 4, `mesh.md` row M35). No walker
> code reads `d6` or `d7` today, so nothing drawn changes; correct the order before
> anything uses them (`mesh.md` §8.7: `d5`/`d7` switch per-model attachments).

<a id="s11-4-1"></a>
#### 11.4.1 LOD classes in use (observation; read at run time)

Census over all 40,643 entity descriptors (OBS-W): 33,018 set `Render_LODClass` in their
own property set, 7,625 do not. Of those 7,625, 607 find a class along the 0x47 parent
chain and 7,018 find none (§11.4.3 for what a placement finally gets). `d0` and `d1` of the
table 0x32000000 in this install, for the classes that are used:

| Class (enum label) | Descriptors | `d0` (draw, m) | `d1` (detail range, m) |
|---|---|---|---|
| LargeStructure | 8,843 | 300 | 100 |
| SmallStructure | 5,175 | 180 | 70 |
| LargeDecor | 4,906 | 80 | 40 |
| Landmark | 3,899 | 1,000 | 200 |
| Equipment | 2,651 | 80 | 26 |
| MediumDecor | 2,353 | 45 | 30 |
| SmallDecor | 1,735 | 25 | 14 |
| Tree | 1,350 | 140 | 60 |
| Creature | 1,171 | 80 | 26 |
| Creature2Deg | 207 | 80 | 30 |
| UmbarLandmark | 171 | 1,000 | 200 |
| Bush | 113 | 80 | 40 |
| PlayerAvatar | 108 | 80 | 26 |
| UmbarLarge | 104 | 300 | 100 |
| Water | 51 | 1,000 | 200 |
| UmbarFarLandmark | 42 | 2,000 | 200 |
| DefaultLight | 36 | 140 | 20 |
| UmbarWarWall, UmbarTiny | 25 each | 50, 45 | 30, 30 |
| FarView500 | 21 | 500 | 100 |
| Unspecified | 16 | 50 | 10 |
| FarView750 | 6 | 750 | 150 |
| UmbarSmall, UmbarMedium, SpecterDegrade20m | 4, 3, 3 | 80, 180, 100 | 40, 70, 20 |
| (no own class) | 7,625 | 200 if no class reaches the object (component initial values, §11.4 step 4) | 60 |

Seven further classes of the table are not used by any descriptor's own set. In every entry
`d1` ≤ `d0`.

<a id="s11-4-2"></a>
#### 11.4.2 Characters

The 14 playable bodies (`appearance.md` §2.2) carry class `PlayerAvatar` (`d0` 80 m, `d1`
26 m), so at default settings
other characters fade out beyond 80 m, and the player's own character beyond 240 m with a
detail range of 78 m. Besides the 14 playable bodies, 94 further descriptors carry
`PlayerAvatar`; creature bodies carry `Creature` (1,171 descriptors, 80 m, 26 m) or
`Creature2Deg` (207, 80 m, 30 m) (W7). Which class each NPC uses was not counted per NPC
(an NPC dressed as a playable species may use a `PlayerAvatar` descriptor; Inferred). All
three classes have `d0` 80 m, so the draw distance of other characters is 80 m whichever
applies. Held and worn items use the wearer's range (`mesh.md` §8.5: every part of an
object uses the object's range). The walker's 400 m character distance (§10.2) is therefore five times the
original's default; the original's animation-update distance was not traced.

<a id="s11-4-3"></a>
#### 11.4.3 Where an object's class comes from (BIN; census Confirmed)

**How the value reaches the render component (BIN, W20).**

1. Each object keeps its properties in a layered collection: an ordered list of five
   layer slots (the object's own run-time values first; then layers that resolve a
   database record the object names and expose that record's properties; last a layer
   shared by all objects, which holds world values such as the time of day). A lookup asks
   the layers in order and returns the value of the first layer that holds the property; if
   none holds it, the lookup reports "absent". It never substitutes the property
   dictionary's default.
2. The render component registers a fixed list of properties it watches, among them
   `Render_LODClass`. It learns their values only from change notifications of the
   collection: "added" (a layer gained a value that no earlier layer masks), "changed" (the
   visible value changed) and "removed" (the visible value went away). Notifications are
   produced only for properties that some layer holds or held.
3. On "added" or "changed" with a class value, the component looks the class up in the
   LOD-class table (§11.4) and copies the entry's eight values; then, if the object is the
   player's own character, multiplies `d0` … `d5` by 3. A class without a table entry
   leaves the current values unchanged (the ×3 step still runs on them).
4. On "removed", the new value handed to the component is the property dictionary's
   default (class 2 here), which is then applied as in step 3. This happens only when a
   value disappears at run time.
5. Consequently, an object none of whose layers ever holds `Render_LODClass` keeps the
   component's initial values (§11.4 step 4: `d0` 200 m, `d1` 60 m), and the ×3 for the
   player's character is not applied to them.

**What the data provides (census, all Physics SEDs of all landblock-info records, W21).**

| Where a class is found | Physics SEDs |
|---|---|
| SED's own set (and its instance set, which agrees wherever it has one) and the 0x47 chain, same class | 81,425 |
| SED's own set and the 0x47 chain, **different** classes | 333,933 |
| SED's own set only (the 0x47 chain has none) | 38,849 |
| Only the 0x47 record the SED names (its own set) | 1,665,106 |
| Only a parent 0x47 of that record (`setup.md` §3.1 step 2) | 12,993 |
| Nowhere | 248,393 |
| Total | 2,380,699 |

The 248,393 placements without a class (10.4 %; they name 2,802 distinct 0x47 records) are
drawn to 200 m with a detail range of 60 m at the built-in settings, not 50 m and 10 m. If
the client does not follow the 0x47 parent chain for this property, a further 12,993 join
them.

> **OPEN:** Which layer holds the SED's own and instance property sets and which holds the
> 0x47 record's set (and whether that layer includes the record's parent 0x47), so which
> class wins in the 333,933 placements where they differ. **Tessera Decision** until
> traced (W22): the class is the first one found in the order of `entity-descriptor.md`
> §9.2 rule E22: the SED's instance property set, the SED's own property set, then the 0x47
> chain from the named record towards its parents; if none is found, §11.4 step 4 applies.
> The dictionary default is never used for an object's initial class.

<a id="s11-5"></a>
### 11.5 Camera-state records (prefix 0x08, DB type 0x0D)

Eleven records in `client_general.dat`. Little-endian; f32 values; angles in degrees;
distances in metres; times in seconds. There is **no** own-DID field: the first field is the
camera entity's DID.

<a id="s11-5-1"></a>
#### 11.5.1 Header (24 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Camera entity (a 0x47 entity descriptor; a client-only mobile whose ethereality type is `Camera`, `collision.md` §9.1) |
| 4 | 4 | f32 | Collision pull-in smoothing time (s). Inferred from the client's label |
| 8 | 4 | f32 | Near clip distance (m) |
| 12 | 4 | f32 | Far clip distance (m) |
| 16 | 1 | u8 bool | Camera uses physics (collides) |
| 17 | 1 | u8 bool | Camera uses the water surface physics |
| 18 | 1 | u8 bool | Camera follows the player under water |
| 19 | 1 | u8 bool | Automatic transitions disabled (Inferred: between third and first person when zooming) |
| 20 | 4 | u32 | Controller type (mapper 0x230000d4): 1 ThirdPerson, 2 FirstPerson, 3 Cinematic, 4 Flight |
| 24 | var | | Controller body (§11.5.3) |

<a id="s11-5-2"></a>
#### 11.5.2 Target block (44 bytes, inside bodies of types 1, 2 and 4)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 8 | bytes | A field the client stores but whose kind was not resolved; zero in every record |
| 8 | 4 | u32 | Holding location of the target to aim at (an enum value, 0 = none) |
| 12 | 4 | u32 | Bone of the target to aim at (0 = none) |
| 16 | 12 | 3 × f32 | Offset from the target point (x, y, z, m) |
| 28 | 4 | f32 | Yaw offset (degrees) |
| 32 | 4 | f32 | Pitch offset (degrees) |
| 36 | 4 | f32 | Height factor *f* (§11.7 step 1) |
| 40 | 4 | f32 | Height factor under water |

<a id="s11-5-3"></a>
#### 11.5.3 Controller bodies

**Type 1, third person (137 bytes; record size 161):**

| Offset in body | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | f32 | Angle smoothing time (s) |
| 4 | 4 | f32 | Zoom smoothing time (s) |
| 8 | 4 | f32 | Target-position smoothing time (s) |
| 12 | 8 | 2 × f32 | Two values without a name in the client's field list (0.07 and 0 in every record) |
| 20 | 44 | | Target block (§11.5.2) |
| 64 | 1 | u8 bool | Constrain pitch |
| 65 | 4 | f32 | Pitch limit, high (degrees) |
| 69 | 4 | f32 | Pitch limit, low (degrees) |
| 73 | 1 | u8 bool | Constrain yaw |
| 74 | 12 | 3 × f32 | Offset range (zero in every record; use OPEN) |
| 86 | 4 | f32 | Yaw limit, high (degrees) |
| 90 | 4 | f32 | Yaw limit, low (degrees) |
| 94 | 1 | u8 bool | Mouse-look rotation relative to the target |
| 95 | 1 | u8 bool | Detached mouse |
| 96 | 4 | f32 | Minimum distance (m) |
| 100 | 4 | f32 | Maximum distance (m) |
| 104 | 4 | f32 | Field of view (degrees) |
| 108 | 4 | f32 | Initial pitch (degrees) |
| 112 | 4 | f32 | Initial yaw (degrees) |
| 116 | 4 | f32 | Initial distance (m) |
| 120 | 1 | u8 bool | Supports aligning to the slope |
| 121 | 4 | f32 | Minimum target height (m; use OPEN) |
| 125 | 4 | f32 | Target fade distance (m) |
| 129 | 4 | f32 | Target fade opacity (0 to 1) |
| 133 | 4 | f32 | Look-at offset smoothing time (s) |

Offsets below are from the start of the body (record offset 24). Multi-byte fields are not
aligned; read them byte-wise.

> **OPEN:** Whether the client rejects a "u8 bool" byte other than 0 or 1 in the header and
> bodies (not stated by BIN-W). The `tessera-formats` decoder (M4.5) rejects such bytes, like
> every other flag byte of the crate; all 11 records of this install hold 0 or 1. Note also
> that the record has no own DID (`records.md` §1.6), so its decoder cannot check the DID
> it was looked up by.

**Type 2, first person (65 bytes; record size 89; W25):**

| Offset in body | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | f32 | Field of view (degrees) |
| 4 | 1 | u8 bool | Constrain pitch |
| 5 | 4 | f32 | Pitch limit, high (degrees) |
| 9 | 4 | f32 | Pitch limit, low (degrees) |
| 13 | 44 | | Target block (§11.5.2) |
| 57 | 4 | f32 | Initial yaw (degrees) |
| 61 | 4 | f32 | Look-from offset smoothing time (s) |

**Type 3, cinematic (12 bytes; record size 36; W25):**

| Offset in body | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | f32 | Field of view (degrees) |
| 4 | 4 | f32 | Target smoothing time (s) |
| 8 | 4 | f32 | Angle smoothing time (s) |

**Type 4, flight (70 bytes; record size 94; W25):**

| Offset in body | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | f32 | Field of view (degrees) |
| 4 | 4 | f32 | Initial yaw (degrees) |
| 8 | 1 | u8 bool | Constrain pitch |
| 9 | 4 | f32 | Pitch limit, high (degrees) |
| 13 | 4 | f32 | Pitch limit, low (degrees) |
| 17 | 1 | u8 bool | Constrain roll |
| 18 | 4 | f32 | Roll limit, left (degrees) |
| 22 | 4 | f32 | Roll limit, right (degrees) |
| 26 | 44 | | Target block (§11.5.2) |

Values in this install (OBS-W): the type-2 record has FOV 60, constrain pitch 1, pitch +90 /
−90, initial yaw 0, smoothing 0.5; the type-4 record FOV 50, initial yaw 0, constrain pitch
1, pitch +90 / −90, constrain roll 1, roll −89 / +89; both have an under-water height
factor of 0. The type-3 records have FOV 35 to 90 and smoothing times 0.04 to 0.22 s.

> **OPEN:** The two unnamed f32 of the type-1 body (offsets 12 and 16), the 8-byte field at
> the start of the target block, the offset range and the minimum target height. The client's
> game layer registers two angle smoothing times for its own third-person camera ("while
> travelling" and not), which may be the two unnamed values; not confirmed.

<a id="s11-5-4"></a>
#### 11.5.4 Census (this install)

| DID | Type | Entity | Near / far | Main values |
|---|---|---|---|---|
| 0x08000000 | 1 | 0x47000002 | 0.1 / 2,000 | distance 6 (1–16), pitch −15 (−80 to 45), FOV 45, height factor 1.0 |
| 0x08000001 | 2 | 0x47000002 | 0.1 / 2,000 | FOV 60, pitch ±90, height factor 0.925 |
| 0x08000002 | 1 | 0x47000002 | 0.1 / 2,000 | distance 6 (1–20), pitch −15 (−80 to 60), FOV 45, height factor 0.925, target fade 0.7 m / 0.1, zoom smoothing 0.4, water physics on |
| 0x08000003 | 4 | 0x47000002 | 0.1 / 2,000 | FOV 50, pitch ±90, roll ±89, offset (0, −1, 0), height factor 0.9 |
| 0x08000004 | 1 | 0x47001A7C | 0.1 / 2,000 | distance 4.2 (1–30), pitch −10, FOV 65, height factor 0.9 |
| 0x08000005 | 3 | 0x47001A7C | 0.1 / 2,000 | FOV 65 |
| 0x08000009 | 1 | 0x47000002 | 0.1 / 2,000 | distance 6 (1–20), pitch −15 (−80 to 60), FOV 45, height factor 0.925, offset (0, 0.2, 0.75), zoom smoothing 0.1, no target fade, water physics on, automatic transitions off |
| 0x08000016, 0x0800001A, 0x0800001B, 0x08000020 | 3 | 0x47001A7C | 0.1 / 2,000 | FOV 60, 45, 90, 35 |

In all type-1 records: angle and target smoothing 0.08, minimum distance 1, initial yaw 0,
slope alignment supported, look-at offset smoothing 0.5.

<a id="s11-6"></a>
### 11.6 Which camera state the player's camera uses (BIN)

The client finds the default camera state of a controller type through the `UNIQUEDB`
sub-map of the root DID map (`records.md` §8.2, `property-system.md` §8.1), by key:

| Controller type | Key (label in the map) | This install |
|---|---|---|
| ThirdPerson | 0x10000041 (`gmThirdPersonCamera`): the shipped game layer replaces the engine's key 0x0A (`DefaultThirdPersonCameraState`) by this one | 0x08000002 (both keys) |
| FirstPerson | 0x2E (`DefaultFirstPersonCameraState`) | 0x08000001 |
| Flight | 0x8D (`DefaultFlightCameraState`) | 0x08000003 |
| Cinematic | none | — |

Other labelled entries of the same map: 0x60 `DefaultCameraState` → 0x08000000;
0x10000021 `HorsebackCameraState` → 0x08000009; 0xA2 `CameraEntity` → 0x47000002. Their use
was not traced (Inferred: the horseback state replaces the third-person state while mounted).
Look the keys up at run time; do not hard-code the DIDs. **Tessera Decision:** when key
0x10000041 is missing, use 0x0A.

> **OPEN:** Per-entity camera overrides. The property dictionary has `Entity_Camera_Array`
> entries with a controller type and override values (initial distance, maximum distance,
> initial pitch and others); one entity descriptor (0x4700234B) carries one (third person:
> initial distance 12, maximum 30, pitch −15). Inferred: when the player's entity carries an
> entry for the active controller type, its values replace the state's. Not traced.

<a id="s11-7"></a>
### 11.7 The third-person camera: login placement and behaviour (BIN)

When the third-person camera becomes active (entering the world after login, and every
other activation):

1. **Look-at point.** Start at the target entity's position (the player), or at its holding
   location or bone when the target block names one; add the target block's offset (Inferred:
   in the entity's frame). Then raise it by *f* × *H* × *s_z*, where:
   - *f* is the target block's height factor, or its under-water height factor while the
     controller's under-water switch *U* is on (rule for *U* below);
   - *H* is the entity's `Entity_Camera_Lookat_Height` (property 0x771) when the entity's
     property lookup finds one, else a height value the client reads from a second object
     the entity exposes (see the OPEN below);
   - *s_z* is the entity's scale along z.

   The raise is skipped when *H* × *s_z* < 0.0002, and also, whatever the property holds,
   when the entity exposes no such second object (W24).

   **Switch *U* (BIN, W24).** A per-frame routine of the camera system recomputes *U* and
   hands it to the active controller. *U* is on only when three run-time flags of the
   camera system are set, a further run-time lookup of that routine succeeds, and the
   player's own entity holds the boolean property `Entity_Camera_ForceUnderwater` with the
   value true; in every other case *U* is off. No entity descriptor of this install
   carries that property (OBS-W), so *U* can only come from values set at run time; the
   walker uses *f*, never the under-water factor.
2. **Yaw** (BIN, W24), in degrees. The controller keeps four booleans:
   - *R* = "mouse-look rotation relative to the target" and *X* = "detached mouse": the
     record's fields at body offsets 94 and 95 (§11.5.3). Before a record is applied the
     controller holds *R* true and *X* false; all four type-1 records of this install store
     0 in both.
   - *P*: a run-time flag, false in a new controller, set by a controller entry point that
     also re-aims the camera (it sets *P* even while the controller is inactive); its
     callers were not traced.
   - *Q*: a run-time flag set from outside the controller through a setter; its callers
     were not traced.

   The base yaw is the activation request's yaw when the request carries one (a cinematic
   or script request); otherwise the state's initial yaw when *P* is set and *R* is clear;
   otherwise 0. The target's heading is added when *X* is set, or when *P* is set and
   (*Q* is set or *R* is clear). The heading is taken from the target's forward direction
   (the angle of the rotated forward axis, converted to degrees).

   With the shipped records (*R* and *X* clear), an activation after *P* has been set gives
   yaw = initial yaw (0) + heading; the first activation of a new controller (*P* clear)
   gives yaw 0 with no heading added.
3. **Pitch** = the state's initial pitch (degrees).
4. **Distance** = the initial distance clamped to [minimum distance, maximum distance]. A
   script request may override the initial distance (still clamped).
5. The target's opacity is reset to 1.

> **OPEN:** The look-at height *H* when `Entity_Camera_Lookat_Height` is absent. The client
> then reads one float of a second object the entity exposes through an interface query:
> the third component of the fourth of six three-float fields of that object (they look
> like three pairs of box corners; Inferred: the upper corner of a box of the entity, so
> *H* would be its top). What fills that field was not traced. **Tessera Decision:** when
> the property is absent, use the top of the entity's bounding box in its own frame if the
> walker has one, else skip the raise. All 14 playable bodies carry the property (W8), so
> the decision does not affect the player's camera.

> **OPEN:** The three camera-system flags and the run-time lookup that gate switch *U*
> (step 1), and who sets `Entity_Camera_ForceUnderwater` on the player's entity (no
> descriptor does). **Tessera Decision:** *U* is always off.

> **OPEN:** Who sets the run-time flags *P* and *Q* of step 2 and when, in particular
> whether *P* is already set when the camera is first activated after login. If it is
> not, the login camera faces yaw 0 in the world frame instead of following the
> character's facing. **Tessera Decision:** at login treat *P* as set, so yaw = initial
> yaw + heading, which with initial yaw 0 puts the camera **behind** the character
> (Inferred from the sign conventions). An owner screenshot right after login, with the
> character facing a known direction other than the world's yaw 0, would settle it.

Every frame afterwards:

- Pitch is kept in degrees; when the constrain-pitch flag is set it is clamped to
  [pitch low, pitch high].
- **Target fade:** when the camera is closer to the look-at point than the target fade
  distance, the target is drawn at the target fade opacity, otherwise at 1; the opacity
  moves towards its goal smoothly.
- Distance, angles and the look-at point move towards their goals with the smoothing times
  of §11.5.3.

Look-at heights in this install (OBS-W): 26 descriptors carry `Entity_Camera_Lookat_Height`:
15 of class `PlayerAvatar` (among them the 14 playable bodies, 1.07 to 1.8 m), 9 creature
bodies (`Creature`, `Creature2Deg`, 1.07 to 1.8 m) and two structures (2.3 and 4 m).

**Values of the in-world state (0x08000002 here):** field of view 45°, near 0.1 m, far
2,000 m, initial distance 6 m (1 to 20 m), initial pitch −15°, pitch limits −80° and +60°,
height factor 0.925 (a 1.75 m body looks at 1.62 m above its feet), target fade below
0.7 m to opacity 0.1, zoom smoothing 0.4 s.

> **OPEN:** The sign of pitch. **Inferred:** negative pitch looks down (the camera is above
> the character): the limits allow 80° down but only 60° up, and the first-person limits
> are symmetric. An owner screenshot at login (camera untouched) would settle it.

> **OPEN:** The axis of the field of view. FOV reports that the value is combined with the
> aspect ratio, which fits a **vertical** field of view; **Tessera Decision:** vertical.

> **OPEN:** Whether the projection's far plane is the state's far clip (2,000 m) while the
> landscape reaches 2,080 m at the built-in High (§11.3), and how the client draws beyond
> it. Tessera may use a far plane of at least *E*.

> **OPEN:** The built-in defaults of the preferences `Camera.AlignToSlope` and
> `Camera.ResetYawWhenPlayerMoves` (`[Camera]` in the preferences file; both False in PREF)
> and what they change.

<a id="s11-8"></a>
### 11.8 Default-settings profile for the walker

To match the original at its built-in default settings (§11.2), the walker uses:

| Quantity | Original (built-in defaults) | Walker until now |
|---|---|---|
| Landscape (terrain) extent | *E* = 2,080 m (High: *R* 9, *g* 4, *n* 1) | far ring 10 landblocks (§3) |
| Object draw distance | per class, min(*G*, `d0`), *G* = 2,080 m or the fog far distance (§11.3), at least 60 m; examples: SmallDecor 25 m, LargeStructure 300 m, Landmark 1,000 m, no class anywhere 200 m (§11.4 step 4); the class of a placement in the order of §11.4.3 | 900 m for every object (§4) |
| Object fade | 1.5 s in or out when crossing the draw distance | none |
| Detail levels | `mesh.md` §8.4 with *R_d* = `d1` | first detail level only (§4) |
| Other characters | 80 m (`PlayerAvatar`, `Creature`) | 400 m (§10.2) |
| Own character | 240 m | always |
| Frills | level 2 (`scenery.md` §8.3) | not drawn |
| Camera state | UNIQUEDB key 0x10000041 (Tessera: else 0x0A) | Tessera's own |
| Camera at login | behind the character (Tessera Decision of §11.7 on flag *P*), distance 6 m, pitch −15°, FOV 45° (vertical), near 0.1 m | 7 m behind the head, 0.8 m above the terrain (§5) |
| Zoom range | 1 to 20 m, smoothing 0.4 s | 1.5 to 60 m |
| Look-at point | feet + 0.925 × look-at height × z scale | the head |
| Target fade | below 0.7 m to opacity 0.1 | none |

Command-line options (§1) may keep overriding these values; the defaults should be the
original's. When the user's `UserPreferences.ini` is available and the walker wants to match
the user's own client, read the five settings of §11.2 from it instead.

<a id="s11-9"></a>
### 11.9 Open questions

> **OPEN:** First-start preset (§11.2.2), switch *S* and the fog value *V* (§11.3), near
> radius and far-ring meshing (§11.3), which property layer wins for an object's LOD class
> (§11.4.3), unnamed camera fields (§11.5.3), per-entity camera overrides (§11.6), the
> look-at height fallback, switch *U* and the yaw flags *P* and *Q*, pitch sign, FOV axis,
> far plane and camera preferences (§11.7): see the callouts in those sections.

> **OPEN:** `DistantImposters`: what the distant landscape imposters are and how far they
> are drawn. Records of prefix 0x27 are imposter tables (`records.md` §2.1, layout OPEN).

> **OPEN:** `Render_FarViewDistance` (a per-instance float; `entity-descriptor.md` §4.5
> counts 384,845 SEDs with it; of the 384,355 Physics SEDs among them (OBS-W), 267,822 hold
> 1,000 or more, 49,366 hold 250 to 1,000, 3,362 hold 160 to 250, 61,354 hold 25 to 160 and
> 2,451 hold 0) and the far-view entity list 0x0E100001 (`records.md` §2.1): how they extend
> the draw distance of single objects beyond §11.4. The client combines the property with a
> value of the object's LOD-class entry and compares the result with 250 and 160 m (seen
> while tracing §11.4.3; not traced further).

> **OPEN:** Interior cells: their geometry uses *R_d* = 200 m × *M* (`mesh.md` §8.5); their
> draw distance was not traced.

> **OPEN:** `PlayerCrowdQuality` (§11.2) and the animation-update distance of characters.

<a id="s11-10"></a>
### 11.10 Corrections and additions to other specs

These are recorded here instead of editing the other specs.

- **`mesh.md` §8.5** (OPEN "which class an entity **without** `Render_LODClass` gets" and
  its decision "use the dictionary default (class 2)"): answered by BIN (§11.4.3, W20). The
  client never hands the dictionary default to the render component for a property that no
  layer of the object holds; the component keeps its own initial values (`d0` 200, `d1` 60,
  `d2` 16, `d3` 80), which are the values step 3 of `mesh.md` §8.5 already lists. The
  dictionary default reaches the component only when a value is removed at run time. The
  decision should become: no class anywhere → 200 m and 60 m (× the multipliers). This
  affects 248,393 of the 2,380,699 Physics SEDs (W21). The same mechanism governs every
  property the render component watches, so the question raised for `Render_LODCrossFade`
  in `mesh.md` §8.8 has the same answer (absent everywhere → the component's own initial
  value, not the dictionary default); that initial value was not read in this session.
  Which of the SED's and the 0x47 record's classes wins is OPEN (§11.4.3, Tessera
  Decision there).
- **`scenery.md` §9** (an entity without `Render_LODClass` "has class 2, the property's
  registry default"; `class` keys 1, 2, 3 used by entities without the property): same
  correction; such an entity keeps the render component's initial values (`d0` 200,
  `d1` 60), not class 2.
- **`mesh.md` §8.7** (`d0`): the global limit *G* "whose source, the environment's
  visibility distance, was not traced further" is the landscape extent *E*, clipped by the
  fog far distance under the condition of §11.3 step 3, and raised to 60 m.
- **`records.md` §2.1** (prefix 0x08, "layout OPEN") and **`environment.md` §9** (0x08 row:
  "the first u32 is a 0x47-like DID, then floats (0.15, 0.1, 2000.0)"): the layout is §11.5;
  the first u32 is the camera entity's DID, the floats are the collision smoothing time, the
  near clip and the far clip.
- **`terrain.md` §9** (OPEN on the original's draw distance) and **`terrain-texturing.md`
  §12** (Reported bands at about 14 and 24 landblocks): the client's own landscape extent is
  640 to 2,560 m (4 to 16 landblocks) by `LandscapeDrawDistance` level (§11.3). The 14- and
  24-landblock figures come from a third-party tool's overrides (FC), not from the client's
  defaults.
- **`install-layout.md` §8.3**: the `[Render]` distance keys store level names; the level
  names, numbering and built-in values are those of §11.2 (note the VeryLow–VeryHigh scale of
  `LandscapeDrawDistance` and the Disabled level of `FrillDistance`).
- **`render.md`** (OPEN on default third-person camera parameters, "must never be read out of
  the client binary"): the parameters are data, in the camera-state records (§11.5), found
  through the DID map (§11.6); only the selection rule and the activation steps are BIN, as
  allowed by [decision 0012](../decisions/0012-numeric-constants.md).

<a id="s11-11"></a>
### 11.11 Invariants

- **W-I1** (Confirmed, 11 of 11): every 0x08 record parses with §11.5 to its last byte.
- **W-I2** (Confirmed): the controller type is 1 to 4 and the record size is 161, 89, 36 or
  94 bytes for types 1, 2, 3 and 4.
- **W-I3** (Confirmed): the UNIQUEDB keys 0x0A, 0x2E, 0x60, 0x8D, 0x10000021 and
  0x10000041 resolve to 0x08 records, key 0xA2 to a 0x47 descriptor.
- **W-I4** (Confirmed): every LOD-class entry has `d1` ≤ `d0`; every `d0` > 0.
- **W-I5** (BIN): *E* increases with the `LandscapeDrawDistance` level; *G* ≥ 60 m; *D* ≤ *G*.
- **W-I6** (Confirmed): each of the 14 playable bodies of `appearance.md` §2.2 has class
  `PlayerAvatar` and a look-at height; 26 descriptors carry a look-at height (15
  `PlayerAvatar`, 7 `Creature`, 2 `Creature2Deg`, 2 `SmallStructure`).
- **W-I7** (Confirmed): the type-2 body is 65 bytes, the type-3 body 12 bytes and the
  type-4 body 70 bytes; each of the 7 records of these types decodes with the tables of
  §11.5.3 to its last byte, every bool byte is 0 or 1, and the target block of the type-2
  and type-4 records has zero in its first 8 bytes.
- **W-I8** (Confirmed): over all Physics SEDs, wherever both the SED's own property set
  and its instance property set hold `Render_LODClass`, the two values are equal; the six
  rows of the census table in §11.4.3 add up to the number of Physics SEDs.
- **W-I9** (Confirmed): no 0x47 descriptor holds `Entity_Camera_ForceUnderwater`.

<a id="s11-12"></a>
### 11.12 Worked examples (synthetic)

**Camera record.** A synthetic type-3 record of 36 bytes: DID 0x47000010; f32 0.05, 0.2,
1,500.0; bytes 0, 0, 0, 0; u32 3; f32 50.0, 0.1, 0.2. Decoded: camera entity 0x47000010,
collision smoothing 0.05 s, near 0.2 m, far 1,500 m, no physics, cinematic, FOV 50°, target
smoothing 0.1 s, angle smoothing 0.2 s, zero bytes left.

**Landscape and draw distance.** `LandscapeDrawDistance` = Low: *R* = 4, *g* = ⌊4/3⌋ + 1 =
2, *E* = 160 × (6 + 1) = 1,120 m. Fog is on with fog far 900 m and step 3 applies: *G* =
900 m. A class with `d0` = 300 at `ObjectDrawDistance` VeryHigh (*M_d* 1.4): *D* = min(900,
420) = 420 m. A class with `d0` = 1,000: *D* = min(900, 1,400) = 900 m. With fog far 40 m,
*G* would be raised to 60 m.

**Look-at point and login distance.** A body with look-at height 1.75 m, z scale 1.0, height
factor 0.925: the look-at point is 1.61875 m above the entity's position. A state with
initial distance 25 m and limits 1 to 20 m starts at 20 m; with initial distance 0.5 m it
starts at 1 m.

**Fog clip.** Fog on, fog far 900 m, *G* = 2,080 m. With *S* off, *G* = 900 m. With *S* on,
*F* set and *V* = 0.995, *G* = 900 m. With *S* on and *V* = 0.6 (blended, so *F* is set),
*G* stays 2,080 m. With *S* on, *F* clear and *V* = 0 (a single source whose flag is clear), *G* stays 2,080 m.

**Class of a placement (Tessera order of §11.4.3).** Table: class A with `d0` 300,
`d1` 100; class B with `d0` 45, `d1` 30. (1) A SED whose own set holds A, naming a 0x47
record whose set holds B: class A, *D* = min(*G*, 300) at *M_d* 1. (2) A SED without the
property, naming a 0x47 record without it whose parent 0x47 holds B: class B, 45 m.
(3) Neither the SED nor any 0x47 of the chain holds it: no class, `d0` 200 m and `d1`
60 m; at `ObjectDrawDistance` Low (*M_d* 0.6) *D* = min(*G*, 120) m. (4) The SED holds a
class value C that has no table entry: the object keeps 200 m and 60 m.

**Yaw at activation.** Initial yaw 10°, target heading 90°, no request. *R* clear, *X*
clear: with *P* clear, yaw = 0; with *P* set, yaw = 10 + 90 = 100°. *R* set, *X* clear,
*P* set, *Q* clear: base 0 (the initial yaw is used only when *R* is clear), heading not
added, yaw = 0. *X* set: heading always added; with *P* clear the yaw is 0 + 90 = 90°.

---

<a id="s13"></a>
## 13. Walk mode on collision (M4.5; clean-room implementer, revised by a spec writer)

Tessera's design; the client facts it relies on are those of `collision.md` and
`landblock.md`, listed in §14.13 (rows W-C1–W-C3). Code: `apps/client/src/walker/movement/` (walker,
mover, scripted walks, soak), `apps/client/src/walk_path.rs` (walk format),
`apps/client/src/walker/controls.rs` (walk mode). It supersedes, for walk mode with
collision, the "controllers (30 Hz fixed step)" of §2 and the terrain-height walk mode of
§5 ([decision 0013](../decisions/0013-physics-crate.md)).

<a id="s13-1"></a>
### 13.1 Controller and step

- When the world builds collision sets (`BundleOptions::build_collision`) and the mover has
  loaded (§13.2, on its own thread at start-up), walk mode runs the `tessera-physics`
  character controller (`collision.md` §12) on its own accumulator of exactly
  `STEP_SECONDS` = 1/60 s, at most `MAX_STEPS_PER_FRAME` = 5 steps per frame; time beyond
  the cap is dropped (ARCHITECTURE §20.3). The jump press is latched until a step takes it;
  keyboard turning is applied per step at the walk controller's turn rate (2.5 rad/s), mouse
  look once per frame. The avatar and the follow camera use the pose interpolated between
  the last two controller states.
- Otherwise (collision off, or before the mover is ready) walk mode is the `tessera-input`
  terrain-height controller on the 30 Hz step, as in M3–M4. The free-fly controller is
  unchanged.
- Positions are f64 region-frame positions; each step runs against the streamer's
  collision view around the feet (its landblock and eight neighbours, plus owners of cells
  reaching there), whose frame origin is the feet's landblock corner.
- **Placement (Decision, 2026-10-06 review).** Whenever the walk-mode controller is placed
  (entering walk mode from fly mode, a placement or a correction by the session, the mover
  becoming ready), the first step that can run (§13.3: its sets installed) first checks
  the feet: outside every interior cell (lookup at the feet + 0.05 m) and more than
  0.05 m below the terrain height, they are put on the highest walkable support that the
  capsule, swept 4 m down from 3 m above the terrain, finds (as the soak's start search,
  §13.7); on the terrain + 0.01 m when the sweep finds none, finds a ceiling, or starts
  in an overlap. Reason: the controller does not recover a capsule that starts 1 m or
  more below the terrain (it falls without end), and fly mode or the follow camera can
  be below the terrain. Scripted walks and soak walks are placed by their own rules and
  are not checked. Mouse look turns both ends of the interpolated pose, so a turn shows
  in the frame it is made.
- The HUD shows the movement state (`collision.md` §10.4), the cell and the water depth.

<a id="s13-2"></a>
### 13.2 The mover

- Capsule: the single capsule of the avatar's setup (`--avatar`, default `human,m`) times
  its model scale (`collision.md` §10.6; `appearance.md` §6.1). A body that does not
  assemble, or a setup without exactly one usable capsule, falls back on radius 0.35,
  lower cap 0.35, length 1.10 (the capsule every playable species ends up with,
  `collision.md` §10.6).
- Movement record: the first 0x79 property set of the game-logic archives (ascending DID)
  that holds `PlayerPhysicsControl_BasePlayerParameters` (`collision.md` §10.3); about 5 s
  in a release build. Without it, the built-in record of `tessera-physics`.
- Conditions (`collision.md` §11.1, float height `collision.md` §11.2): the properties of
  the avatar's 0x47 entity chain (`setup.md` §3.1), the nearest level winning; the avatar is
  a player character (`collision.md` §11.3).
  On the owner's install: setup 0x04000553, scale 1.0, radius 0.35, height 1.81, float
  height 1.42.

<a id="s13-3"></a>
### 13.3 Collision-set residency (Decisions)

- **Needed landblocks.** A step runs only when every landblock that the controller's query
  box overlaps (decision 0013) **and** the feet's landblock with its eight neighbours have
  installed sets. Reason: placed and generated objects of a landblock reach across its
  edge; with only the query box required, a neighbour's set installed while the mover
  stands next to it put geometry around the capsule, and the push-out then moved the
  feet 0.18–0.40 m below the terrain (two soak fall-throughs before this rule). The rule
  also makes a step's view independent of how far the streamer has got.
- **Cell owners (2026-10-06 review).** A cell's physics mesh belongs to its owner
  landblock (the landblock info that lists it, `landblock.md` §4.4), and a cell's origin
  may lie up to 6 landblocks from that owner (§14.13 row W-C1: Chebyshev distance of the
  landblock holding the cell's origin from the owner, over the 104,269 placed cells of
  this install: 0 in 16,798, 1 in 60,963, 2 in 17,324, 3 in 5,328, 4 in 2,311, 5 in 1,344,
  6 in 201; so 3,856 lie 4–6 landblocks away). The distance is the origin's: a cell's hull
  can reach further. These counts are observations of this install; the walker reads cell
  positions at run time. A step therefore also needs the owner of the mover's current cell, and a
  scripted walk requires its own landblock (the owner of its cells) at every step.
- **Pinned landblocks (Decision; residency is Tessera's own, the client streams by other
  rules).** These landblocks (a scripted walk's own landblock, the current cell's owner)
  are **pinned**: kept resident wherever they lie by one small streamer per pinned
  landblock, in scripted walks and, while in walk mode on collision, in the interactive
  walker. Every view of a step, and the view of the placement check of §13.1, holds the
  installed sets of the pinned landblocks in addition to the 3 × 3 square, also when they
  lie outside it.

> **OPEN (implementer, 2026-10-06 review):** the 3 × 3 rule does not cover cells placed
> far from their owner before the mover has entered them: the walker cannot name an
> owner that is not resident, so whether such a cell's floor is in a step's view still
> depends on residency (the walk rings keep owners up to about 3 landblocks away; the
> interactive streamer about 10). Fix: a world-side index of the cell owners whose cell
> bounds reach each landblock, built from landblock info at boot (`tessera-world`), so
> that `needed` can name them. (Still open on 2026-10-08: no such index exists.)

> **OPEN (implementer, 2026-10-08, status; for the spec writer):** a cell-owner index now
> exists, in the walker's movement module (`apps/client/src/walker/movement/owners.rs`),
> not in `tessera-world`, and it is built around the mover rather than at boot: reading
> every landblock-info record of the owner's install took about 20 s single-threaded (about
> 1 GB of records, own measurement 2026-10-08). Engineering choices to confirm: the
> landblock-info records within 8 landblocks (chessboard) of the feet's landblock are read
> on a background thread and kept while within 16; a cell reaches the landblock holding its
> origin and that landblock's eight neighbours (its hull is not read; the exact bounds of
> the resident set then decide what the view holds); a step needs, and every view of it
> holds (pinned), the owners of the cells reaching the feet's 3 × 3 square other than the
> square's own landblocks, nearest first, at most 8 (the pin limit is now 17); the step
> waits while the index does not cover the feet; an owner more than 6 landblocks from its
> cell's origin is not found (none in this install, row W-C1). Scripted walks and the soak
> use the same index. The OPEN above can be closed.

- **Waiting.** While a needed set is missing the walker does not step: the frame's
  accumulated time is dropped, the latched input kept, the state unchanged.
- **Absent landblocks.** A landblock the streamer resolved as failed (no data, or a data
  error), a bundle without collision, and positions off the grid count as installed and
  empty, and are a solid boundary: a step that moves the capsule's footprint (feet ± radius)
  into such a landblock, when it was not already in one, is undone with the velocity
  zeroed.

<a id="s13-4"></a>
### 13.4 Cells

The walker tracks its cell after every step at the feet raised by 0.05 m (the checklist's
CELL rule). **Tessera Decision, departing from `collision.md` §12.3 step 8 and the
tie-break of `collision.md` §5.8.1** (both: run the containment lookup after every step and
take the lowest cell number among all cells that contain the point):

1. keep the current cell (owner landblock and number) while its volume still contains the
   point (in a doorway both cells do, and the current one is kept);
2. else take the first cell, in the order of the current cell's portals (`collision.md`
   §5.5, §5.8), that a portal of the current cell leads to and whose volume contains the
   point;
3. else run the containment lookup of `collision.md` §5.8.1 on each collision set of the
   step's view in the view's order and take the first set that finds a cell; within one
   set the lowest cell number wins (the §5.8.1 Decision). The view's order is: the
   landblocks of the square of the view around the feet's landblock row by row from its
   south-west corner (west to east, then south to north), then the landblocks further away
   whose cells reach into that square, by region, then landblock x, then y. So a tie
   between two landblocks goes to the first in that order, not to the lower cell number;
4. outdoors when none contains the point.

Effect of steps 1–2: in a doorway, where both cells contain the point, the walker keeps the
cell it came from. Whether the client re-evaluates a moving character's cell every step is
OPEN (`collision.md` §5.8.1).

<a id="s13-5"></a>
### 13.5 Water

The controller's water state (`water.md` §7.4) reads the water planes and liquid boxes of
the view's sets, which `tessera-world` fills when it builds them. Inside an interior cell
(the cell lookup at the feet + 0.05 m) there is no landscape water (Decision; OPEN in
`water.md` §7.4): without the mask the dungeon of walk IN1, 660 m under a lake's plane,
was inside the lake's volume boxes and the walker swam up out of its cell.

<a id="s13-6"></a>
### 13.6 Scripted walks (`--walk-path FILE`, `--walk-log CSV`)

- `FILE` is one walk in the file form of `m4-5-visual.md` §2.1, or a Markdown text holding
  the walk table (the checklist itself runs its walks). The run is headless and needs no
  GPU: it opens the install, the world context with scenery, water and collision (no sky,
  figures or area names), and a streamer with rings of 2–3 landblocks. The exit status is
  non-zero when a walk fails or cannot run.
- Per walk: the feet are put at `start` 0.01 m up (a start rounded to the centimetre never
  begins inside its support), facing `heading`; a walk whose `cell` is not 0 starts in
  that cell of its landblock (the tracker of §13.4 keeps it while it contains the feet);
  the walker waits until the walk's own landblock (the owner of its cells; required by
  every step and pinned, §13.3) and the 3 × 3 landblocks around the feet are installed;
  30 steps without input (0.5 s settle), then `duration` in steps, each with the command
  of the walk script. Inputs are given per step; worker timing changes only when a step
  runs. A cell after the settle that differs from the row's `cell` is reported as a note.
- `inputs` with waypoints and a `stop` or `turn` token are refused (the waypoints steer
  forward and heading every step, so those tokens would have no effect; OPEN in
  `m4-5-visual.md` §2.1). The walk file is read through an 8 MiB limit (a pipe or device
  reports no length); a parsed walk whose `inputs` or `expect`, written back in the file
  form, would exceed 4,096 bytes is refused, so every accepted walk's file form parses
  back.
- Waypoint steering decides the release every step (OPEN in `m4-5-visual.md` §2.1): a
  walker that coasts past the last waypoint after the release (about 0.4 m at walking
  speed) is steered back.
- Pass conditions as `m4-5-visual.md` §2.2, with these readings: penetration is the deepest negative
  separation of the capsule's contacts after each step, settle steps included; so are
  `follow`'s and `pass`'s per-step terrain and object-contact checks ("at every step");
  `follow`'s end condition is horizontal only, as `m4-5-visual.md` §2.2 gives it; "start"
  cell is the cell after the settle (the last settle step); `follow`'s "never blocked" is: no step with forward
  held, at least 0.5 s into the inputs and more than 1 m from the end point, whose
  horizontal speed is below half the layer's forward speed; `pass`'s "object contact" is
  a static or generated body within 0.01 m. A failure within twice its tolerance is
  reported as a near miss (`m4-5-visual.md` §2.2: for the spec writer).
- `blocked` (revised 2026-10-09 to follow `m4-5-visual.md` §2.2 as revised 2026-10-08 and
  confirmed 2026-10-09): the walk is judged at the **first touch**, the first step with forward
  held after which the capsule lies within 0.01 m of a static, generated or other body that is
  not terrain, ceiling or cell, searched among all contacts of that step (not only the 8 that
  the movement log keeps). "Never past P + `d`" (the feet's distance along the start heading) is
  checked at every step up to the first touch; at the first touch the feet must be within `d` of
  P horizontally and within 0.05 m below to 0.05 m plus the slope allowance above P. After it the
  walker may slide along or round the blocker; the end point and the first stop (where the
  walker first came to rest with forward held) are reported for information only.
- **Slope allowance** (`m4-5-visual.md` §2.2, `collision.md` §11.7): the upper vertical
  tolerance of `follow` and `pass` (feet above the terrain under them) and of `blocked` (feet
  above P at the first touch) is raised by *r* · (sec *θ* − 1), where *r* is the capsule radius
  and *θ* the steepest terrain triangle sampled within *r* of the capsule's axis (at the axis and
  on rings of radius *r* and *r*/2, 16 directions each); the lower tolerance is unchanged.
- Movement log (CSV, one row per step): `walk, step, t, x, y, z` (feet, frame of the
  walk's landblock), `vx, vy, vz, heading_deg, state` (`collision.md` §10.4), `support`,
  `cell` (`0xXXYY:N` or 0), `water_depth, forward, strafe, jump, walking, stepped_up,
  jumped, snapped, planes, pushed_out, penetration, boundary, terrain` (terrain height
  under the feet), `contacts` (up to 8, `kind@separation`).

> **Answered (2026-10-09, spec writer; was OPEN, implementer, 2026-10-08):** the first-touch
> reading of `blocked` and the slope allowance are restated above; the earlier "`blocked` walks
> also report the first stop" is kept as information only.

<a id="s13-7"></a>
### 13.7 The soak (`m4-5-visual.md` §2.5)

`apps/client/tests/walk_soak_real_data.rs`. Readings (OPEN in `m4-5-visual.md` §2.5): the draw order is
landblock, x, y, heading, the redraws of step 4 (x, y), then per segment its length, kind
and heading change; a landblock with no valid start within 256 (x, y) draws is passed
over and the next walk planned. Step 4 sweeps the capsule 4 m down from 3 m above the
terrain; the support must be walkable (`collision.md` §11.3) and not a cell, the capsule must not overlap
geometry at the top or on the support. A start with no walkable support is redrawn too.
Step 6's cell floor: a cell physics mesh crossing the capsule's axis more than 0.05 m above
the feet. Walks are planned in order on one thread (one streamer) and run on several,
each with its own streamer (rings 1.5 landblocks) over the shared workers; a digest of the
end positions compares runs.

<a id="s13-8"></a>
### 13.8 Results (2026-10-06, owner's install, release build)

*Dated verification snapshot, not a requirement: later counts are in `docs/ROADMAP.md`
(M4.5 status), which is the current record.*

- Soak: 1,000 walks of 60 s (3,600,000 steps) over 15,881 / 17,694 / 14,489 / 19,548 /
  8,435 / 1,981 land landblocks of regions 1, 2, 3, 4, 5, 14: 0 fall-throughs, 0 non-finite
  states; deepest below the terrain 0.0005 m; 1,357 start redraws, 3 landblocks passed
  over; the same digest in two runs; 23 s with 8 runners.
- Checklist walks: 19 of 32 pass, every one with 0.0000 m penetration (all walls,
  undrawn blockers, water entries, IN1–IN4, CV3, ST1, ST3, FR2). The 13 others are listed
  with their causes in the OPEN of `m4-5-visual.md` §2.4. After the controller fixes of the
  M4.5 exit verification (`collision.md` §12.3 OPEN (n)–(p)) ST2 and FR3 pass too: 21 of 32;
  the soak still has 0 fall-throughs and 0 non-finite states.

<a id="s13-9"></a>
### 13.9 Open questions

> **Answered (implementer, 2026-10-06, review round):** the host (`walker_host.rs`) now
> passes `--avatar` to `WalkerConfig::avatar`, so the mover takes the float height of the
> avatar shown (the capsule is the same for every playable species).

---

<a id="s14"></a>
## 14. World fidelity in the walker (M4.5; clean-room implementer, revised by a spec writer)

Tessera's design for drawing the M4.5 world systems. The client facts it relies on are
those of the cited specs, restated with their provenance in §14.13 (spec writer, dirty
room, 2026-10-07); the rest is Tessera's design. Code: `apps/client/src/walker/` — `terrain.rs` (texture layers, ceilings,
far band), `farband.rs` (the far band's bake), `objects.rs` (LOD classes, fades, detail
levels, lit surfaces, instancing), `env.rs` (clock, sky, sun, fog, lights), `water.rs`,
`frills.rs`, `resolver.rs` (background lookups), `fxparse.rs` (switches, camera loops,
clock arithmetic), `geometry.rs` (`MaterialPlan::lit`), `interiors.rs` (§14.12);
`walker/mod.rs` wires them. Every M4.5 system draws into **one lit world pass**
(`docs/specs/render.md` §4–§5). In §13–§14 a bare "§*n*" means a section of this file;
a section of another spec is always given with its file name.

<a id="s14-1"></a>
### 14.1 Frame

Per frame, after the controllers, the camera's interior cell and scene flags (§14.12) and
the streamer (§2): the frill ring follows the eye and
the player (`scenery.md` §8.2); the resolver's answers are collected; the environment
advances (§14.3) and the light table is built; terrain, objects, characters, water, sky
and frills upload within the frame's budget and record into the pass. The main thread
still reads no archive: 0x47 objects (sky objects, frills) and water surfaces are
resolved on a `tessera-resolver` thread, terrain blocks, layer resolution, masks, light
maps and far-band bakes on the terrain workers (a quarter of the hardware threads, 2 to
6), parts and textures (with their mip chains, `texture-list.md` §5.5) on the asset
workers. Queued terrain jobs are re-ranked every frame, nearest block first, and the jobs
of blocks that left are dropped (a fast camera otherwise leaves the workers on blocks
long passed). The upload budget's time (§4) runs from the start of the upload phase, not
of the frame.

<a id="s14-2"></a>
### 14.2 Switches (profiling)

`--disable LIST` switches systems off: `layers` (texture layers; the M3 height-shaded
meshes instead), `lit` (the M2/M3 studio lighting; it also turns off every system that
needs a lit pass: layers, shadows, lights, water, sky, instancing, frills), `shadows`,
`lights` (point lights), `water`, `sky` (sky objects and the environment's lighting; the
defaults of `lighting.md` §8.1 instead), `fog`, `scenery` (generated objects), `instancing`
(every object as single mesh draws), `frills`, `lod` (per-class distances, fades and
detail levels; every object to the distance cap at its finest level); `all` gives the
M3/M4 walker. At run time F5–F11 toggle layers, shadows, water, sky, scenery, frills and
LOD. The HUD names the systems that are off. The walker reads these keys and those of
§14.3 as raw key codes; a key that a keymap binding uses (any context, alone or in a
chord) is left to the keymap, and the HUD's key line lists them (OPEN §14.11).

<a id="s14-3"></a>
### 14.3 Environment, clock and lighting (Decisions)

- **Clock (Decision; Tessera's own controls, no client counterpart).** The epoch is OPEN
  (`environment.md` §6), so the walker starts the game clock at day *n* = 12,240 (the day
  the clock reads at *G* = 0, `environment.md` §14) and time of day 0.5 (the checklist's
  performance time) unless `--day N` and `--time-of-day T` set them (*G* from *n* and *t* by
  `environment.md` §11.1 run backwards); `--time-rate R` (default 1) and `--day-type D`
  (default 2, `environment.md` §11.2). `--day` takes ±20,000,000 at most (days whose *G* the
  game clock holds, ±10¹² s, with any calendar of days up to 50,000 s); a day the clock still
  cannot hold starts the clock on day 12,240 with a warning. Keys: `[` and `]` move *G* by
  1/24 of the calendar's day length, across midnight into the next or previous day; `\`
  stops and restarts the clock; `-` and `=` halve and double the rate, never past 1/64 or
  the `--time-rate` maximum of 100,000 and never in the wrong direction; while the clock is
  stopped they change the rate it restarts at. A clock restarts at the rate it had before
  it was stopped, or at rate 1 when that was 0: a clock started with `--time-rate 0` counts
  as stopped from the start, so `\` starts it at rate 1 on the first press and `-` and `=`
  change the rate it will start at. The keys act on both environment records below at once.
- **Two environment records** (`landblock.md` §4.13, `environment.md` §7.1.1, §7.2, §11.6;
  rows W-E1, W-E2, W-E6, W-E7 of §14.13). The walker keeps two environments on the same
  clock and blends both every frame, each on its own: the **outdoor record** from sky key A
  and the **indoor record** from sky key B with the interior overrides. It uses the indoor
  record while the scene flag outdoor-visible is clear (§14.12 (c)), the outdoor record
  otherwise; it changes between them at once (no blend; this departs from the client's
  sun-change fade, see "Sun presence" below).
- **Where the records read their properties** (`environment.md` §7.1.1 position lookup,
  `landblock.md` §4.13; rows W-E1, W-V5). With the camera in an interior cell (§14.12 (a)):
  that cell's property set, then the landblock-info property set of the cell's **owner**
  landblock, then the owner's block map (the owner's, Inferred, `landblock.md` L38).
  Otherwise, also while outdoor-visible is clear without a camera cell (possible only with a
  `cell_flags` bit-0 cell, none in this install, LI8): the landblock-info property set of the
  eye's landblock, then its block map; off the grid nothing. The first of these that holds
  the property gives its value. The character's own properties, which the client asks first
  for both keys, never apply offline.
- **Key A** (`environment.md` §7.1.1; rows W-E1, W-E3, W-E8): `Environment_LocalSky` by that
  lookup, else the value 2. The walker takes key A in every frame, also while the indoor
  record is in use.
- **Key B and the interior overrides** (`landblock.md` §4.13, `environment.md` §7.2; rows
  W-E1, W-E2): by the same lookup. Key B is `Environment_IndoorAmbiance`, else the value 1.
  The overrides, applied to the indoor record only:
  - if any of `InteriorUpperAmbientLight`, `InteriorLowerAmbientLight`,
    `InteriorAmbientLight` is present, the up-facing ambient *X* = upper + ambient and the
    down-facing *Y* = lower + ambient replace the keyframe's (an absent colour counts as
    black), every channel limited to at most 1;
  - `InteriorFogType` present: fog is on exactly when the walker's fog switch (its
    counterpart of the client's distance-fog setting, `environment.md` §11.7) is on and the
    value is 1 (Linear); 0, 2 or any other value switch fog off. Absent: fog follows the
    switch, as the keyframe's fog does;
  - `InteriorFogColor`, `InteriorFogNear`, `InteriorFogFar` replace the fog colour, start
    and end.

  The walker re-reads the overrides when the lookup's landblock changes and otherwise every
  2.5 s (the client: when the position's region, landblock or instance changes, else at most
  every 2.5 s). A colour property is read as its four stored bytes B, G, R, A
  (`lighting.md` §4.4) divided by 255 (Decision: the client's conversion to floats was not
  traced, `landblock.md` §4.13).
- **Stored types** (rows W-E2, W-E8): as in the client, key A, key B and `InteriorFogType`
  count only with property type 8 (enum value) or 10 (property ID), the colours only with
  type 16, near and far only with type 9; a value of another type, or a float that is not
  finite, counts as absent. In this install every stored value of the three keys is an enum
  value (row W-E9).
- **Frame lighting** from the blended values of the record in use (`lighting.md` §8.1,
  Tessera Decision there): ambient ½ (*X* + *Y*), hemispheric ½ (*X* − *Y*); a main light
  (the sun or moon, toward −travel, colour *Cd* × *bd*, specular *Cs*) only on the outdoor
  record and only while the colour and brightness tests of the next bullet find a sun; in
  the indoor record **no sun or moon** (row W-E7); distance fog with the limits below; the
  time input = seconds since start (`material.md` §5.3.1).
  **Decision (no client counterpart: the client always has its sky):** without a sky library
  or with `sky` off, the frame takes the defaults of `lighting.md` §8.1 with no fog, no
  interior override is applied, and the indoor record still has no main light.
- **Sun presence and the sun-change fade (departs in part from `environment.md` §11.6 and
  §11.7 step 9; rows W-E10–W-E13; OPEN below).** The client gives each environment state a
  "sun present" flag. It is true only on the outdoor record, and there only while all three
  of these hold (row W-E10): the blended keyframe `effect` is the null DID (`environment.md`
  §11.3 step 7); the sun colour is not opaque black (some channel differs from (0, 0, 0, 1)
  by more than 0.0002); the sun brightness *bd* is at least 0.0002. While the flag is false
  the renderer has no main light: no sun term and no sun shadow, one submission path adds
  the default directional light of `lighting.md` §8.5 (its slot a Tessera Decision there),
  and the water's fake light takes its default (`lighting.md` E18; `render.md` §5.3
  fake-light Decision for a pass without a main light). In addition, whenever the flag of
  the state in use differs from the previous update's, the client fades the state it shows
  over 5 s of the timer's raw seconds, from the state shown at that moment (held fixed) to
  the state in use, with the weights and the blend rules of the cross-fade of
  `environment.md` §11.6 step 4 (a side without a sun counts as black); a further change
  during the fade restarts it from the state then shown, over the time since the previous
  change; an immediate update (Inferred: entering the world or a teleport) skips the fade
  (row W-E11).
  The walker, as built: the outdoor record has no main light (no sun term, no sun shadow,
  the light-0 slot left to the renderer's default directional light) when each of the sun
  colour's R, G and B lies within 0.0002 of 0 (inclusive) or *bd* is below 0.0002 (a
  non-finite *bd* counts as below); the indoor record never has one. So the black-sun and
  dim-sun keyframes (154 without an `effect`, row W-E12) are drawn as in the client.
  What remains different:
  - **Weather keyframes (visible).** The walker makes no test on the blended `effect`. 503
    of the 7,590 keyframes of this install carry a non-null `effect` (`environment.md` §5.3
    table). While such a keyframe's `effect` is the blended one and its sun passes the
    colour and brightness tests, the walker draws sunlight, sun shadows and a sun-directed
    water highlight where the client draws none, and when the `effect` starts or ends the
    client fades the sun out or in over 5 s.
  - **Alpha of the sun colour (no effect at this install's keyframes).** `tessera-world`'s
    blended values keep no alpha of the sun colour, so the walker's colour test reads R, G
    and B only; the client compares all four channels with (0, 0, 0, 1), so a sun colour
    with black R, G, B and an alpha other than 1 has a sun in the client and none in the
    walker. All 6 keyframe sun colours with black R, G, B have alpha 255 (row W-E13), so
    the tests agree at every keyframe; between keyframes they can differ only while the
    blended R, G, B are within 0.0002 of black.
  - **Going indoors or out.** Between a sunlit outdoor state and the indoor state (which
    never has a sun) the flag changes, so the client fades ambient, fog and sun over 5 s
    where the walker switches at once. When the outdoor state has no sun at that moment
    (a weather `effect` running, or a black or dim sun), the flag does not change and the
    client switches at once too. `landblock.md` §4.13 and `lighting.md` §8.1 (row L37) call
    the change of record immediate: that is the choice of record, which the fade then
    smooths (BIN re-check of 2026-10-07, row W-E11).

> **OPEN (spec writer, 2026-10-07, narrowed 2026-10-08, for the walker and `tessera-world`
> implementers):** expose the sun-present flag of `environment.md` §11.7 step 9 with each
> environment record (false on the indoor record), including the test on the blended
> `effect` and the alpha of the sun colour, and give the renderer no main light while it
> is false; replace the immediate change of record by the 5 s sun-change fade of
> `environment.md` §11.6 (row E34 there), skipped at start-up and on a teleport. The flag
> rests on the blended `effect`, whose choice in `tessera-world` differs from the client
> today: the keyframe blend takes *kₐ*'s `effect` whenever it is not null, without the
> |*f* − 1| > 0.0002 test of `environment.md` §11.3 step 7, and a cross-fade switches from
> the old to the new `effect` at weight ½, where §11.6 step 4 keeps the old state's unless
> it is null. Both have to follow `environment.md` before the flag is used.

> **OPEN (implementer, 2026-10-08, remediation pass; status and two conflicts):** the walker
> now (a) carries "sun present" with each record (`SkyValues::sun_present`, the
> `environment.md` §11.7 step 9 test including the blended `effect`, false on the indoor
> track), (b) blends the `effect` by `environment.md` §11.3 step 7 and the sky-key
> cross-fade by §11.6 steps 1–6 (key changes only, 8 s default, vector light direction,
> `effect` kept from the old state unless null), and (c) composes the record in use and
> passes it through the 5 s sun-change fade of §11.6 (E34 with the corrected duration rule,
> E37), forced on the first update and on every placement by the sandbox (the Tessera
> Decision of `landblock.md` §4.13). Two points of this section disagree with Accepted
> specs, and the code follows the other spec: (1) "Where the records read their
> properties" reads the camera cell's own property set first, but `landblock.md` §4.13
> (BIN4 (l), L32 corrected 2026-10-08) says the cell's set is **not** part of the lookup;
> the walker reads the owner's landblock-info set and block map only. (2) "Alpha of the
> sun colour" assumes the blended sun colour keeps the keyframes' alpha, but
> `environment.md` §11.3 step 4 gives every blended sun colour alpha 1, so the R, G, B
> test is the client's test; no alpha is kept. Please restate both bullets (and the
> departures table) or say why they stand.

> **OPEN (spec writer, 2026-10-07, for the writers of `landblock.md` §4.13 and
> `lighting.md` §8.1):** both call the change between the outdoor and the indoor record
> immediate without mentioning the sun-change fade of `environment.md` §11.6, which applies
> to it whenever the outdoor state has a sun (row W-E11). Say that the immediate part is the
> choice of record.

- **Fog limits** (`lighting.md` §10, `landblock.md` §4.13; rows W-E5, W-E6). In the client's
  order: the walker caps the keyframe's fog end at 10,000 **before** the interior override,
  an overriding `InteriorFogFar` replaces it **without** that cap, then the end is raised to
  at least 20 and the start kept within −10,000 … end − 20. A non-finite keyframe end counts
  as 10,000 and a non-finite start as −10,000 (Decision for malformed values; an override
  that is not finite counts as absent, above). The renderer applies only the lower limits to
  the fog it is given (`render.md` §4.2), which leaves these values unchanged, so an
  overriding end above 10,000 is drawn as stored. (No `InteriorFogFar` of this install
  exceeds 7,000, row W-E5.)
- **Sun shadow** (`lighting.md` §9; the renderer's form uses the variants that the
  simple-form Decision there lists, `render.md` §4.4): one map of 2,048 texels over a sphere
  of radius 90 m centred 54 m ahead of the eye (horizontal view direction), fading out by a
  view depth of 150 m, cast only while the record in use has a main light (above) and the
  sun's elevation exceeds asin 0.05 (Decisions: map, sphere, fade and elevation are Tessera's
  values). Casters:
  - terrain chunks whose bounding sphere comes within 430 m of the eye (3 × the shadow
    radius + 160 m; Decision); ceiling chunks never (§14.4);
  - objects while their fade opacity is **above 0.8** and their unsmoothed distance *d* is
    **strictly below** their shadow distance min(*G*, `d2` × *M_d* × *M_s*), at any distance
    while `lod` is off, as the client (`mesh.md` §8.7–§8.8; row W-O1). The opacity the walker
    tests is the fade alone: the render component's two factors, an inherited opacity and the
    tint alpha of `mesh.md` §8.8 are taken as 1 (Decision; the walker sets none of them). A
    fading object is a single mesh draw and casts through the alpha-tested shadow path;
    instanced batches hold only fully faded-in objects (§14.5);
  - characters only while their opacity is 1 and *d* ≤ 16 m: **departs** from `mesh.md`
    §8.5, §8.7–§8.8 (row W-O2; §14.5 "Character shadow distance").
- **Clear colour** = the fog colour (the background meets the fog at the horizon); the sky
  objects draw over it (§14.8).
- **Global limit** *G* = `global_limit(E, fog far)` (§11.3, Tessera Decision there: clipped
  whenever fog is on).
- **Light table (Decisions).** The Light entities of resident landblocks within 200 m of
  the eye, nearest first, at most 255 (the client's own walk of the scene's lights is OPEN,
  `lighting.md` §8.5); from inside an interior cell only interior Light entities (flag
  0x400), from outdoors only outdoor ones, plus interior ones with `--interiors` (§14.12);
  range and on/off by the day/night rule of `lighting.md` §8.3 (Tessera Decision there),
  intensity curves evaluated on the material clock, *σ* = 1.
- **Light lists** (`lighting.md` §8.5; rows W-L1–W-L4, W-L7). The walker hands the renderer
  a reach entry for every table light (`render.md` §4.3: ρ = Range × (1 − 0.15 × min(Falloff,
  1)) from the Range in force, Range for `Light_GlobalLight`; the key-light switch; the
  baked-landscape switch), and every object, character, terrain chunk and ceiling chunk uses
  the renderer's automatic selection. That selection follows the client: the squared
  distance from the light to the draw's world box below ρ² (strict), key lights first, then
  the others (each group nearest first, the Tessera Decision of `lighting.md` §8.5),
  `Light_BakedLandscapeLight` lights never on a terrain or ceiling chunk, at most 8 lights
  with light 0; the generated objects drawn instanced get one selection **per instance**
  against the instance's own box (`render.md` §4.3, §5.5), as the client builds one list per
  object. What remains different:
  - **Parent switches (departs from `lighting.md` §8.5 step 3, row W-L2; no effect on this
    install; OPEN below).** `tessera-world`'s light parameters carry neither
    `Light_OnlyInfluenceParent` nor `Light_NeverInfluenceParent`, and the walker gives no
    light a parent, so a light with `Light_OnlyInfluenceParent` set reaches every object in
    the walker, where in the client a light with that switch and no parent reaches no
    object. No Light SED of this install's landblock infos sets the switch true (136 store
    false, row W-L2).
  - **Height-shaded fallback meshes (Decision).** The fallback meshes of §14.4 are mesh
    draws, not terrain chunks, so baked-landscape lights reach them; they appear only in the
    fallback cases listed there.
  - Water receives no table light: its permutations read only the fake light (`water.md`
    §8.2, §8.4 step 6; `render.md` §5.3).

> **OPEN (spec writer, 2026-10-07, narrowed 2026-10-08, for the `tessera-world` and walker
> implementers):** expose `Light_OnlyInfluenceParent` and `Light_NeverInfluenceParent` in
> `tessera-world`'s light parameters and pass them in the reach entries (`render.md` §4.3),
> so that a parentless light with `Light_OnlyInfluenceParent` set reaches no object (the
> renderer already applies the rule of row W-L2 to the entries it is given).

- **Baked-landscape lights on the ground** (`lighting.md` §8.6, Tessera Decision there):
  the walker shows them on the terrain only through the stored light maps
  (`terrain-texturing.md` §10). The client contains a light-map bake that draws such lights
  into a chunk's baked-lighting texture with pass code 0x41 (row W-L5); it runs only in a
  special client mode and, Inferred, not in normal play (`lighting.md` L36), so the stored
  maps are what normal play shows.

<a id="s14-4"></a>
### 14.4 Terrain

- **Layers.** The block job resolves the terrain and ceiling chunks against the region's
  0x19 table (`terrain-texturing.md` §4; the table the scenery tables loaded, else read
  from the region descriptor), checks each mask (A8, `terrain-texturing.md` §5.1: a
  rejected mask is dropped and the layer drawn as if it had none; masks above 1,024 texels
  a side, twice the largest in the data, are rejected too) and decodes the masks and light
  maps (`terrain-texturing.md` §10). Masks wait for their upload as their A8 texels and are
  expanded to (255, 255, 255, *a*) only when uploaded (a far block keeps its masks on the
  CPU until it nears). A mesh job builds the chunk's vertices (`terrain-texturing.md`
  §6.1–§6.4, the per-block offset of §6.2 with Tessera's fixed region coordinates) and
  every layer's triangles at the block's level and stitched edges
  (`ChunkLayers::layer_indices_for`, the Tessera Decision of `terrain.md` §5); the chunk is
  uploaded when its shared layer textures (diffuse, normal, emissive, glow, through the
  object layer's store) have arrived. A block that draws nothing yet is uploaded at once
  with what is there and rebuilt when the rest arrives. The crack-free promotion of §3 is
  unchanged. Blocks are recorded nearest first (early depth rejection).
- **Vertex normals (Decision, departing from `terrain-texturing.md` §4.10).** The chunk
  vertices carry `tessera-terrain`'s unit central-difference normals (`terrain.md` §5;
  ceiling: the terrain rule negated). The client stores per sample the plain average of
  the unit normals of the grid triangles around it, not renormalised (so shorter than 1 at
  creases), with mirrored triangles for ceilings (row W-T2; `terrain-texturing.md` §4.10,
  `landblock.md` §3.14). The renderer
  shades layers without a normal map with the normal as given (`render.md` §5.2 "Stored
  normal"), so with the walker's unit normals the darkening of creases that §4.10 gives is
  not reproduced. Under flat terrain and flat ceilings the two agree.

> **OPEN (spec writer, 2026-10-07, for the walker and `tessera-terrain` implementers):**
> build the chunk normals by `terrain-texturing.md` §4.10 (terrain and ceiling rules,
> neighbour blocks across the edges, triangles left out where a neighbour is missing).

- **Ceilings** (`terrain-texturing.md` §4.8): level 0 from the stored triangles, with the
  ceiling light map; they receive the downward ambient *A₀* − *H₀* because their normals
  point down. No shadow casting (Decision: whether the client draws ceiling chunks into the
  sun's shadow map was not traced).
- **Chunks without layers** (`terrain-texturing.md` §4.9; row W-T1). The block's open flag
  is set when the block has no ceiling, or when at some grid sample (*i*, *j*) with
  0 ≤ *i*, *j* ≤ 31 the stored terrain height is strictly below the stored ceiling height.
  A terrain or ceiling chunk that stores no layers is resolved, when the flag is set, as one
  opaque `whole` layer of the table's entry with the smallest `draw_order` (the last met in
  stored order on a tie), without a mask, with the surface's light map, and drawn through
  the layer pipeline like any other first layer. When the flag is clear (rule 3), the chunk
  is left out of the walker's scene entirely, as the client does not add it to its scene:
  no layered chunk, no height-shaded mesh (one built while layers were off is freed when
  they are switched back on), no light selection, no shadow, no far-band bake, and it is
  not counted as a drawn chunk or ceiling.
- **Height-shaded fallback (Decision).** The M3 height-and-slope mesh, lit with its vertex
  colours, is drawn for a terrain chunk in these cases:
  - layers or lighting switched off (`--disable layers` or `lit`; debugging views, no
    client counterpart). In these views a chunk that rule 3 hides is drawn height-shaded too
    (the M3 view);
  - the region has no 0x19 table, or its table has no entry (so no smallest `draw_order`
    for a layerless chunk). Neither occurs in this install: every region's descriptor names
    one of five tables, each with 316 to 1,421 entries (`terrain-texturing.md` §3.1). The
    client's behaviour with an empty table was not traced;
  - the chunk stores layers but none of them resolves. This does not occur in this install
    (no layer key is missing from its table, `terrain-texturing.md` T5); the client's
    per-chunk routine stops at the first layer that fails and draws the layers before it
    (`terrain-texturing.md` §8, T38), and the Tessera Decision there skips only the failing
    layer. A terrain chunk whose layers fail does not affect the block's ceiling, which keeps
    its own layers and its own rule-3 flag.

  A ceiling gets the same treatment: its height-shaded mesh (the ceiling heights at level 0,
  the downward normals of `terrain-texturing.md` §4.8, triangles facing down, no shadow) is
  drawn wherever its layered chunk is not drawn for one of the reasons above (with layers or
  lighting switched off also a ceiling that rule 3 hides). In the normal view a ceiling that
  rule 3 hides draws nothing.
- **Scrolling layers** (`terrain-texturing.md` §6.3): the offset curves of the layers that
  have them are evaluated every frame on the material clock and handed to the renderer.
- **Far band (Decision; the original's distance bands are OPEN, `terrain-texturing.md`
  §12).** A block farther than 480 m (`--far-band M`, 0 = off) is drawn with one opaque
  layer; it leaves the band at min(40 m, *M* / 2) nearer than *M* (hysteresis; a band of
  40 m or less still has a positive exit distance). The layer is its layer stack composited
  on the CPU, once per block, into a 256 × 256 image with a mip chain (`farband.rs`): each
  layer's albedo × `diffuse_scale` *M*, sampled from the texture's small CPU levels (≤ 64
  texels, the level matching the texel footprint), blended "over" by its opacity rule after
  the discard test (`terrain-texturing.md` §7–§8); missing masks and textures follow the
  renderer's rules; the result is stored in 8 bits per channel (*C* × *M* never exceeds 1:
  *M* is a colour of four bytes, `terrain-texturing.md` §3.3). The far layer is lit like a
  first layer with *M* = 1 (2 × *C′* × Σ, the block's light map still added); per-layer
  normal maps and specular highlights are not kept. The bake runs only for a terrain chunk
  drawn with its layers (not for one that rule 3 hides or whose layers do not resolve).
  Reason: one draw per far block instead of one per layer (the walker drew 1,800 layer
  draws in a forest view; the far band leaves about 200), which the frame budget of
  criterion 5 needs; at the band's start a baked texel (0.625 m) is about 1.7 pixels at
  1080p. Bakes run ahead of need (320 m inside the band) on the terrain workers. Visually
  the same view with and without the band matched on 2026-10-06 (default spawn, 60 m above
  the ground). A block that enters the band with no full-layer chunk drawn and no bake yet
  (a fast flight) is drawn height-shaded until the bake arrives.
  - **Light map in the far band (departs from `terrain-texturing.md` §9.1).** In the full
    stack the ambient, sun and point-light terms carry *M* but the light-map term does not
    (2 × *C* × *B*ₗ). The bake folds *M* into the albedo, so in the far band the light-map
    term is scaled by *M* too. The far layer is exact only where *M* = 1 or the block has
    no light map. One baked image cannot hold both weightings (Σ *w* *C* and Σ *w* *C* *M*);
    removing the difference needs a second baked image or an unscaled-albedo input on the
    renderer's far layer (`render.md` §5.2).
  - **Masks in the band (Decision; engineering, no visual effect).** A block in the band
    keeps only its light map on the GPU; its masks are uploaded when it comes within the
    band's edge (a chunk waits only for its own masks and light maps, so the ceiling of a far
    cave block completes while the terrain masks wait). Masks uploaded while a block was
    near are released once the block is back in the band, its bake is drawn and no
    full-layer chunk is drawn, staged or waiting; they are queued again when it nears. To
    upload them again without a new read, the walker keeps each uploaded terrain mask's A8
    texels on the CPU.
- **Light lists** of chunks: §14.3.
- **From inside an interior cell** no terrain or ceiling chunk is drawn (§14.12).

> **OPEN (spec writer, 2026-10-08, from the implementer's note of 2026-10-07, for the walker
> and `tessera-render` implementers):** the far band's light-map departure above; it needs a second baked image or an
> unscaled-albedo input on the renderer's far layer.

> **OPEN (implementer, 2026-10-08, status; for the spec writer):** the far band's light-map
> term now follows `terrain-texturing.md` §9.1 without a renderer change. When a block has
> a light map and one of its layers has a `diffuse_scale` other than white, the bake also
> composites the stack **without** `diffuse_scale` and multiplies it texel by texel by the
> light map (sampled as the layer pipeline samples it: clamp, bilinear, at x / 160, y /
> 160), a second 256 × 256 image with its own mip chain; the far layer binds that image as
> its emissive texture with *G* = 2 and binds no light map, so it draws 2 × *C′* × Σ
> (without the light map) + 2 × Σ *w* *C* × *B*ₗ, which is the full stack's light-map
> term (`render.md` §5.2: emissive is added as *Em* × *G* before fog). Without a light map,
> or with every `diffuse_scale` white, no second image is made and the far layer binds the
> light map as before (exact in those cases). The light map is sampled at the bake's
> resolution, not at the light map's own per pixel, and both terms are stored in 8 bits.
> The "Light map in the far band" departure above and the §14.13 row can be closed.

<a id="s14-5"></a>
### 14.5 Objects (§11.4, `mesh.md` §8.4–§8.7)

- The block job turns the bundle's drawable entities and generated objects into objects
  (one per placement, with its part instances) and gives each its LOD-class values: the
  class of the placement in the order of §11.4.3 (`LandblockBundle::lod_class_of`), the
  table entry or the component's initial values (§11.4 step 4). Interior entities and the
  render meshes of interior cells become objects too, each naming the cell it is drawn
  through (§14.12).
- **Draw distance** *D* = min(*G*, `d0` × *M_d*), capped by the object distance cap
  (`--object-distance`, default the landscape extent; it shrinks while the GPU budget is
  exceeded, §4). An object's parts are requested once it is within *D* + 60 m (nearer
  than 200 m at visible priority, else at prefetch priority: parts never outrank the near
  ring's landblocks, which load at immediate priority) and released beyond *D* + 140 m;
  the activation pass of a landblock reruns after 12 m of camera travel, a change of *G*
  of more than 1 m, a change of the distance cap, or of the `lod`, `scenery` or
  `instancing` switch or the distance settings, or a change of the camera's interior
  landblock or of `--interiors` (so a cap that shrinks with the camera still releases
  parts, and F9 and F11 act at once). Generated objects are not drawn while `scenery` is
  off; switching `instancing` on asks for the models of the parts in use. A landblock whose
  outdoor objects are all out of reach is skipped whole, except the landblock whose
  interior the camera is in.
- **Distance box (Decision, departing from `mesh.md` §8.6, row M23 there).** Fade and draw
  distance are measured to a box around the bounding sphere of the object's uploaded
  parts, not to the world box that `mesh.md` §8.6 gives (the union of the stored boxes of
  the level-0 vertex blocks). The sphere's box is at least as large, so objects draw and
  fade slightly beyond *D*.
- **Fade** (`mesh.md` §8.7; row W-O3): `DrawFade` on the unsmoothed distance *d* to that
  box, a linear opacity ramp of 1.5 s whenever *d* crosses *D*. The fade restarts (the
  object appears or disappears at once) in the first frame after the object becomes active,
  after a camera move of more than 25 m in one frame, and whenever the object's fade was not
  updated in the previous frame. The walker updates an object's fade only in the frames in
  which its draw loop reaches it, so a fade is not updated, and restarts in the next frame,
  when the object's block was out of view, when the draw cap stopped the frame before it,
  or when the object was not in the walker's scene (an outdoor object while the camera is in
  a cell, a generated object with `scenery` off, an object out of reach).
  **This departs from the client** (row W-O3, BIN (o)): the client runs the fade step of
  every object of its scene in every frame, in the scene's cull pass and before the frustum
  and occlusion culls; its fade opacity is the time since the fade began divided by 1.5 s,
  so a frame without the step would neither pause nor restart it; and it restarts a fade
  only in the first frame of the object's per-view state, after a camera jump of more than
  25 m, or on a render-state reset (`mesh.md` §8.6–§8.7). Effect: an object whose fade is
  running when its block leaves the view, and that comes back into view within the ramp,
  shows its end state at once in the walker and the rest of the ramp in the client.
- **Fade blending (Decision: the original's blending of a fading object is not specified,
  OPEN §14.11).** A fading surface blends by its output alpha × the opacity with depth write
  (`AlphaMode::TestBlend`, `render.md` §4.3); opaque surfaces become `BLEND_DEPTH_WRITE`,
  alpha-tested ones `TestBlend` with their threshold, blended and additive ones keep their
  mode.

> **OPEN (spec writer, 2026-10-08, for the walker implementer):** advance the fade of
> every object of the walker's scene in every frame, as the smoothed distance below already
> is (also for objects of blocks out of view and after the draw cap), and restart a fade
> only in the object's first frame and after a camera move of more than 25 m (row W-O3).

- **Smoothed distance (`mesh.md` §8.6 step 3; row W-O3).** Updated every frame for every
  object of the walker's scene (active, not a generated object with `scenery` off, in a
  cell the camera sees), drawn or not: before the fade test and the pixel and frustum culls,
  for the objects of a block whose box is out of view, and, when the draw cap stops the
  frame, for the objects after the one that reached the cap and for every later block. This
  is the client's rule: its smoothing runs in the same per-object step as the fade, for every
  object of the scene, before the culls (BIN (o)). **Decision:** the walker starts *d_s*
  again at *d* when an object becomes active again (it is released beyond *D* + 140 m),
  where the client keeps an object's state while the object stays in its scene; the two
  differ only for at most a second after the object returns within *D* + 60 m, before it
  can be drawn at *D*.
- **Shadow casting** (`mesh.md` §8.7–§8.8, rows M27, M40; W-O1). An object casts into the
  sun's map while its opacity is above 0.8 and its unsmoothed distance *d* is strictly below
  its shadow distance min(*G*, `d2` × *M_d* × *M_s*), at any distance while `lod` is off, as
  the client does. The opacity is the fade alone (Decision, §14.3 "Sun shadow"). A fading
  object casts through the alpha-tested shadow path of its single mesh draw.
- **Detail levels** (`mesh.md` §8.4): every kind-1 level of a part is uploaded; the level
  follows the smoothed distance (`mesh.md` §8.6) and *R_d* = `d1` × *M*.
- **Surfaces** go through the lit pipeline with the inputs of `lighting.md` §4.1
  (`MaterialPlan::lit`): material colour 2000 as stored, the alpha mode of `material.md`
  §11.3 (lit `ColorAndOpacity` blends with depth write, unlit blend records without,
  additive records additively), normal map (3011), specular colour and power, gloss from
  the base or normal alpha by role (`material.md` §10.2; an opaque `ColorAndGloss` texture
  keeps its alpha), emissive (3018) and glow mask (3013) with glow colour 2014, fog,
  shadow casting. Characters use the same rules (with their dye in stored values).
  Texture transforms and second colour layers are not drawn (the lit pipeline has none
  yet, `render.md` §4.1). Two choices depart from `material.md` §11.3 and `lighting.md` §7:
  - **Unlit blend pairs (departs from the `material.md` §11.3 rule for unlit records).**
    The walker draws an unlit record whose most common blend pair is not one of
    SRCALPHA / INVSRCALPHA, SRCALPHA / ONE or ONE / ZERO (DESTCOLOR / ONE, ZERO / SRCCOLOR,
    ONE / ONE and others: 21 records, 57 surface records in `material.md` §11.3's census) as
    a source-alpha blend, and every additive record without depth write, where the
    `material.md` rule takes the record's own pair and depth write as a fixed-function blend
    (the renderer offers it as `AlphaMode::Fixed`, `render.md` §4.3);
  - **Fog of unlit surfaces (departs from `lighting.md` §7 code 0x00).** Every unlit surface
    is fogged (`LitSurface::fog` set; world models uploaded with the default surface options,
    whose fog switch is on, `render.md` §5.5), where the client fogs an unlit pass only when
    the record's program reads the fog inputs.

> **OPEN (spec writer, 2026-10-08, for the walker and `tessera-material` implementers):**
> draw unlit records with the fixed-function blend and depth write of the `material.md`
> §11.3 rule (`AlphaMode::Fixed`), and pass each unlit surface's fog switch (whether its
> record's unlit program reads the fog inputs, `lighting.md` §7) to `LitSurface::fog` and,
> for world models, through `Renderer::upload_world_model_with` (`render.md` §5.5).

- **Instancing** (`render.md` §5.5): fully faded-in objects whose surfaces need no
  emissive, glow or dye draw as instanced batches, one per landblock, part, detail level
  and shadow flag (each instance with its own light list, §14.3); the part's world models
  are built once its textures are on the GPU. Fading objects and the other surfaces stay
  single mesh draws.
- **Characters** (§11.4, §11.4.2): drawn to *D* = min(*G*, 80 × *M_d*), and the player's
  own character to *D* = min(*G*, 3 × 80 × *M_d*) (the ×3 applies to the class value before
  the cap at *G*, as `mesh.md` §8.5 step 4), with the 1.5 s `DrawFade` across *D* (blended
  like a fading object; it restarts when the character was not in view in the previous
  frame, the departure of "Fade" above); `--character-distance` (default 400 m) only caps
  *D*. A character casts a shadow only while its opacity is 1 and its distance *d* ≤ 16 m
  (next bullet). Placed figures beyond *D* are handed to the object layer (§10.2). The
  player's own character is hidden in free-fly mode (the checklist's camera,
  `m4-5-visual.md` §4.0). **Decision (departing from §11.4.1):** 80 m is the `d0` that the
  classes `PlayerAvatar`, `Creature` and `Creature2Deg` have in this install (§11.4.2), and
  the walker uses it as a constant for every character; §11.4.1 says such values are
  observations to be read at run time, and each character's own class is not resolved
  (OPEN below).
- **Character shadow distance (departs from `mesh.md` §8.5 steps 3–4, §8.7–§8.8; rows
  W-O1, W-O2, W-K1; OPEN below).** Every character, the player's own included, casts within
  the fixed 16 m (the render component's initial `d2`), with no *M_d* × *M_s* factor, no
  cap at *G* and no ×3 for the player's own character, and only while its fade is complete
  (opacity 1) and *d* ≤ 16 m. The client gives a character the shadow distance of its own
  class like any object: min(*G*, `d2` × *M_d* × *M_s*), where for the player's own
  character `d2` (one of `d0` … `d5`) has been multiplied by 3, and casts while the opacity
  is above 0.8 and *d* is strictly below that distance. In this install `PlayerAvatar`,
  `Creature` and `Creature2Deg` all have `d2` 26 (row W-K1), so at *M_d* = *M_s* = 1 other
  characters cast to 26 m and the player's own to 78 m (both below *G*), where the walker
  stops at 16 m. A character entity none of whose property layers holds `Render_LODClass`
  keeps `d2` 16 in the client too (`mesh.md` row M33), but its shadow distance is still
  scaled by *M_d* × *M_s* and capped at *G*.

> **OPEN (spec writer, 2026-10-07, narrowed 2026-10-08, for the walker implementer):** read
> each character's LOD-class values from the table at run time (the class of each character
> entity, §11.4.3, with the player's ×3 of `mesh.md` §8.5 step 4; 80 m only as the fallback
> for `d0` when the table is missing) and derive from them both the draw distance
> *D* = min(*G*, `d0` × *M_d*) and the shadow distance min(*G*, `d2` × *M_d* × *M_s*); let
> characters cast, as objects already do, while their opacity is above 0.8 and *d* is
> strictly below the shadow distance (rows W-O1, W-O2).

> **OPEN (spec writer, 2026-10-09, for the walker and `tessera-material` implementers):**
> placed water is not "ordinary objects" for shading. Route each placed surface by
> `water.md` §8.5 (recognition by `material.md` §11.6): water records to the water pipeline
> at the placed mesh's triangles, the mirror record (puddles) and liquid-distortion records to
> their drawings, and draw every other surface with both colour layers and its animated
> texture matrices (`material.md` §8.4, §10.3; `render.md` R-D13, R-D14). Today fountain and
> pool water shows the water records' near-white slot-3000 placeholder, puddles their white
> shape texture, and waterfall sheets one static layer with hard edges (owner report of
> 2026-10-09; `m4-5-visual.md` rows W10–W14).

<a id="s14-6"></a>
### 14.6 Water (`water.md` §8–§9)

Each resident wet landblock within the landscape extent is one 160 × 160 quad at *h*
(`water.md` §8.1 Tessera rule); quads are grouped by surface into one draw with the
surface's water parameters (`water.md` §8.2, resolved on the resolver thread at rest) and
normal map. **Decision** (`water.md` §8.2 OPEN): a record that declares none of the
parameters 1059–1062, 1064, 1065 is drawn with one normal layer. *O* is the anchor's
origin, so the normal layers use region coordinates (`water.md` §8.3 Tessera rule).
Surfaces with vertex waves are subdivided 16 × 16 per quad. Distant water is the landblock
water of the resident landblocks out to the extent (`water.md` §9 Tessera rule: derived
from the block maps). Placed water meshes (`water.md` §8.5) are ordinary objects. Water
receives no table lights (§14.3). Landscape water is not drawn from inside an interior cell
(§14.12).

> **OPEN (spec writer, 2026-10-09, for the walker implementer; `render.md` R-D12; lava added
> 2026-10-10):** the one-layer Decision above and the use of §8.3 for every surface do not hold
> for the flow-map records 0x2B0007FD and 0x2B0005EC, which draw 95 % of the wet landblocks:
> draw them by `water.md` §8.6 (1057 and 1058 are cycle rates of a flow map, not scroll speeds;
> the current drawing drifts 6–23 m/s, the original 0.15–0.9 m/s or not at all, §8.7), each
> record with its own colour list (`water.md` §8.6.4 Tessera rule, revised 2026-10-10). Nor do
> they hold for the lava record 0x2B000810 (656 wet landblocks): draw it by `water.md` §8.6.5
> (recognised by the lava rule of `material.md` §11.6), never by §8.3 or the generic path. Keep
> passing the time input in seconds (E20). Checklist rows W15–W17 of `m4-5-visual.md` §4.8 and
> W7 of §4.4.

> **OPEN (spec writer, 2026-10-09, second revision, for the walker implementer):** water,
> mirror and liquid parameters are taken from the surface once, at clock 0. Evaluate every
> curve-valued one at the clock each frame (`water.md` §8.7.1; census there: 1045 on
> 0x31002A44–0x31002A46 and three liquid surfaces, 1059 and 1061 on 0x3100338F, 1063–1065 on
> 0x3100328B and 0x3100824C, 1105 on the puddles, 1182, 1183, 1186, 1187 on the flow-map
> surfaces, 1186 and 1187 on the lava surface 0x31006FAE; lava added 2026-10-10), and for the two surfaces with curve-valued scales measure *B* from the camera's
> landblock (Tessera Decision of `water.md` §8.7.1). The fast plain-water surfaces of
> `water.md` §8.7.2 are right as drawn; do not clamp them, and never divide by a layer scale
> (0x31000698 has a scale of 0).

<a id="s14-7"></a>
### 14.7 Frills (`scenery.md` §8)

The streamer's frill ring (level 2 by default, `--frill-distance 0–5`) is drawn instanced.
Each ready sector's instances are grouped by object, in the sector's landblock frame (so
a moving anchor never rebuilds them); new sectors are built nearest first, at most 96 per
frame. The geometry is the first part of the object's part list and, of its detail level
0, only the first draw entry (`scenery.md` §8.5 step 3; the object layer builds that
model for a level with more than one entry). The sectors of a 4 × 4-sector cell (40 m)
are merged into one batch per object part and frill values (Decision: one draw per 16
sectors), translated into the anchor's render frame and merged again when a member
changes or the anchor moves; sectors whose terrain-colour intensity differs (one without
its colour yet) stay in separate batches. The distance rules stay per sector, as the
original's groups are (`scenery.md` §8.3 rules 2 and 4, §8.5 step 4): a sector whose box
lies at *D* or beyond draws nothing, and the reduction count is taken from each sector's
own distance, keeping a prefix of that sector's instances; the walker hands the renderer
the reduced list (rebuilt only when a count changes). **Decision:** a sector's box is the
box around its instance origins widened by 4 m on every side (the original's group box
is not specified). Trade-off of the merge: the fade-in runs from the time the cell first
appeared, not each sector. **Terrain colour** (`scenery.md` §8.3 rule 7, formula OPEN;
Decision): the mean colour of the smallest level of the top terrain layer's diffuse texture
at the sector's centre × its `diffuse_scale`; while unknown the sector's terrain-colour
intensity is 0; **Decision (engineering, no client counterpart):** the walker checks at most
once a second whether an uncoloured sector's colour can now be found, and rebuilds the
sector as soon as it is, however long that takes. Cells out of view or beyond *D* are not
queued. Frills are neither streamed nor drawn from inside an interior cell (§14.12).

<a id="s14-8"></a>
### 14.8 Sky (`environment.md` §11.4, §12)

The visible sky objects of the day description in use, each part of its 0x47 object as a
`SkyObject` with the object's angles and keyframe tint, placed by the client's rotation
(`render.md` §5.4; `environment.md` §11.4 step 5), in the stored order; objects whose
models are not built yet are left out. The parts stay referenced for the walker's life (a
day description holds at most 136 objects, indices 1–136, `environment.md` §5.4). The day
description in use is that of the record in use (§14.3): from inside an interior cell the
indoor record's, and only while sky-visible is set (§14.12). As the client, the walker
leaves the sky objects out while fog is on (the keyframe's, or the indoor record's after
`InteriorFogType`, §14.3) and the blended `fog_extra` exceeds 0.99 (`environment.md` §11.7
step 10, §12; row W-Y1).

<a id="s14-9"></a>
### 14.9 Default-settings profile (§11.8)

| Quantity | The walker now |
|---|---|
| Landscape extent | the far ring is *E* of `--landscape-distance` (High: 2,080 m) unless `--view-blocks` sets it; near and mid rings 3 and 6 landblocks |
| Residency | default budget 4,096 MiB: about 600 landblocks are resident, and with their scenery and collision sets they hold 0.85 MiB (region 1, Kingsfell) to 4.6 MiB (region 5) each, 2.7 GiB in all at the densest spot seen (2026-10-06 measurement) |
| Object distances | per class (§14.5), `--object-draw-distance` and `--model-detail` levels (§11.2) |
| Fade, detail levels | §14.5 |
| Other characters | min(*G*, 80 × *M_d*) with the 1.5 s fade (§14.5, Decision and OPEN there); `--character-distance` caps it |
| Frills | level 2 |
| Field of view, near plane | 45° vertical, 0.1 m |
| Camera state, login placement, zoom range, look-at point, target fade | not adopted: the walker keeps its own third-person camera (§5); OPEN §14.11 |

<a id="s14-10"></a>
### 14.10 Automated runs and logs

- `--camera-loop R:X:Y:x,y;x,y[;…]` (with `--loop-speed`, `--loop-height`,
  `--loop-pitch`; defaults 6.5 m/s, 2 m above the higher of terrain and water, −10°)
  flies the checklist's closed loops (`m4-5-visual.md` §3); `--turn-rate DEG` turns a
  free-fly camera on the spot. Both wait at their start until a `--duration` run starts.
- `--no-hud` hides the HUD and minimap (clean screenshots).
- New frame-log columns (after `x, y, z`): `env_ms, frills_ms` (main thread), `time_of_day,
  g, fog_far`, `terrain_layered, ceilings, objects_drawn, fading, instanced, batches,
  water_quads, frill_groups, frill_instances, lights`, the renderer's counters of the
  frame (`gpu_draw_calls, gpu_terrain_layers, gpu_instances, gpu_water_quads,
  gpu_sky_objects, gpu_shadow_draws, gpu_lit_draws`), in headless runs
  `end_frame_ms, gpu_wait_ms` (submission and the wait for the GPU), and the main
  thread's phases `terrain_update_ms, objects_update_ms, minimap_update_ms,
  terrain_draw_ms, objects_draw_ms, water_draw_ms, sky_draw_ms, frills_draw_ms`. A
  screenshot that times out names what was still loading.
- The HUD shows, inside an interior cell, the cell, its owner landblock, the number of
  visibility cells drawn and whether sky and landscape are visible.
- `apps/client/tests/visual_rows_real_data.rs` checks the expected facts of the visual
  rows (`m4-5-visual.md` §4) on the world model the walker draws from;
  `apps/client/tests/interiors_real_data.rs` checks the interior census and the IN1 and Y5
  examples (§14.12).

<a id="s14-10a"></a>
### 14.10a Checklist rows rendered (criterion 6, Tessera side, 2026-10-06)

*Dated verification snapshot, not a requirement: it records what the walker showed on
2026-10-06, with notes added on 2026-10-07 where the walker or the specs changed since.
`docs/ROADMAP.md` (M4.5 status) is the current record.*

Every row of `m4-5-visual.md` §4 was rendered headless at its eye, heading, pitch and time
(1600 × 900, free-fly, no HUD, avatar hidden; private images, not committed) and checked
against the expected facts by `visual_rows_real_data.rs` (all 6 tests pass with
`TESSERA_GAME_DIR` set).

| Rows | Facts (test) | Image | Notes |
|---|---|---|---|
| T1–T3, T6–T11, T13, T14 | pass (every layer stack) | pass | the expected surfaces: cobbles, snow, cliff top, city paving, brick and paved roads, light-mapped tiled floors |
| T4, T12 | pass | target hidden | a placed object (T4) and a generated rock (T12, gone with `--disable scenery`) stand between the eye and the target; the terrain under them draws |
| T5, T15, T16 | pass | too dark to judge | T15 and T16 (cave ceilings) draw their layers, visible with `--disable sky`; under the sky's lighting the downward-facing ceilings are almost black. 2026-10-07: that is what `terrain-texturing.md` §4.8 and §4.10 give (the downward ambient *A₀* − *H₀*, the ceiling light map and the placed lights that are not baked); to be re-judged |
| S2, S3, S4, S7, S9, S11 | pass (every listed object) | pass | |
| S1, S5, S6, S8, S10 | not asserted (§14.11 OPEN) | the objects the walker generates draw | |
| F1 | pass (exact counts) | failed on 2026-10-06 | the 13 × 13 blocks around F1 (region 1, 0x4C66) store no layers. 2026-10-07: drawn by `terrain-texturing.md` §4.9 with key 0 of 0x19000000 under the frills (§14.4); to be re-rendered and re-judged |
| F2–F6 | pass (exact counts) | pass | frills draw instanced; F2's near-white groups were fixed in the M4.5 exit verification (§14.11) |
| W1–W9 | pass | pass | sea, lakes, river step, lava, placed water (W8) and distant water to the extent (W9) |
| Y1, Y3, Y4 | pass | pass | noon clouds, evening, night sky with stars and lamps |
| Y2 | pass | not judged | the eye faces a building wall at dawn |
| Y5 | pass (interior fog values) | failed on 2026-10-06 | interior cell meshes were not drawn. 2026-10-07: interiors are drawn (§14.12) and Y5 renders the IN1 cave; to be re-judged |
| L1, L3, L4, L6 | L2 checked by the test | pass | sun shadow of the barn, bonfire emissive, glowing crystals, blended cobweb |
| L2, L7 | L2 pass | not conclusive | the night scenes are lit, but pools of point light (L2) and the warm light-map texels (L7) are not distinct (see the baked-light note of §14.3). 2026-10-08: the light lists now follow `lighting.md` §8.5 (§14.3); to be re-rendered and re-judged |
| L5 | – | judged "fails" on 2026-10-06 | no broad specular highlight on layer 67's paving toward the sun. 2026-10-07: `terrain-texturing.md` §9.4 (T44) gives a faint sheen of about the gloss (mean 0.09 for layer 67), not a broad highlight, so this is the expected look; to be re-judged |

<a id="s14-11"></a>
### 14.11 Open questions

> **OPEN (implementer, 2026-10-06):** the original's far landscape (`terrain-texturing.md`
> §12, `DistantImposters`): the far band of §14.4 is Tessera's own; per-layer normal maps
> and specular highlights are not drawn beyond 480 m, and the seam between the bands is
> not blended.

> **OPEN (implementer, 2026-10-06):** how the original blends an object fading across its
> draw distance (`mesh.md` §8.7 gives the 1.5 s opacity ramp, not the blend): Tessera
> blends by alpha with depth write. (Spec writer, 2026-10-08: only the blend is
> unspecified; the shadow rule is `mesh.md` §8.8, opacity above 0.8, which objects now
> follow; characters still depart, §14.5 with its own OPEN.)

> **Answered (implementer, 2026-10-07, `landblock.md` §4.11–§4.14, Revised):** interior
> cells are drawn; Y5 renders the IN1 cave. The original note: the render meshes of
> interior cells (0x01) are not drawn; interiors show their interior entities
> (`--interiors`) only.

> **Moved (spec writer, 2026-10-07):** the implementer's note "interiors in the walker,
> the Tessera choices" was a list of Decisions, not a question; it is now the body of
> §14.12, with each choice labelled and the departures from `landblock.md` §4.12–§4.14
> marked.

> **OPEN (implementer, 2026-10-06):** the camera rows of §11.8 (camera-state record, login
> placement, zoom range, look-at point, target fade) are not adopted yet.

> **OPEN (implementer, 2026-10-06):** characters' draw distance uses `d0` = 80 m (the
> three character classes agree, §11.4.2) and their shadow distance the render
> component's initial `d2` (16 m), instead of each character entity's own LOD class.
> (2026-10-07: see the Decision and OPEN of §14.5.)

> **OPEN (implementer, 2026-10-06, for the spec writer and the scenery implementer):** the
> walker's generated lists (`tessera-world` scenery) do not hold six of the 22 objects the
> scenery rows of `m4-5-visual.md` §4.2 list: S1 both (0x470035F7 at (127.035, 66.551),
> 0x470035F4 at (128.162, 70.238)), S5 0x4700720F at (60.130, 78.403), S6 0x47005A5F at
> (72.770, 21.815), S8 0x4700720F at (120.468, 106.733), S10 0x47007AA2 at (59.987,
> 43.137) (0.70 m from a generated 0x47007CB0, so possibly a spacing or clearance
> difference). The other 16 objects match to 0.01 m, 0.1° and 0.0001 in scale.
> `apps/client/tests/visual_rows_real_data.rs` reports these rows without failing.

> **Answered (spec writer, 2026-10-07):** cave ceilings (T15, T16). `terrain-texturing.md`
> §4.8 and §4.10: the ceiling normals point down, so a ceiling receives the downward
> ambient *A₀* − *H₀*, its ceiling light map and the placed lights that reach it and are not
> baked-landscape lights; the client adds nothing else to ceilings. The walker's remaining
> difference is the normals of §14.4 (OPEN there); its light lists follow the client since
> 2026-10-08 (§14.3). The
> original note: cave ceilings are almost black under the sky's frame lighting; a face
> pointing down receives ambient − hemispheric = the ground term *Y* (`lighting.md` §8.1)
> plus its light map; whether the original adds more to ceilings is not specified.

> **Answered (spec writer, 2026-10-07):** row L5. `terrain-texturing.md` §9.4 (T44): the
> terrain highlight is the gloss × sat(*n* · *H*)^*p* × the shadow factor × the sun's
> specular × *Ms*, not multiplied by *N* · *L*, the albedo or 2; layer 67's gloss (from its
> diffuse alpha) averages 0.09, so the original shows a faint sheen, not a broad highlight.
> `render.md` §5.2 draws it so. The original note: row L5 shows no broad highlight on
> terrain layer 67 toward the sun; the layer's specular switch, gloss source and the sun's
> specular colour need checking against the original.

> **Answered (M4.5 exit verification, 2026-10-06):** the near-white groups were the
> sun's specular with gloss 1: instanced surfaces could not take the gloss from a
> normal-and-gloss texture's alpha (`GlossFrom::NormalAlpha`), so `WorldSurface` gained
> `gloss_from_normal_alpha` and F2 now draws its heather pink and its flowers pale, with no
> near-white pixels. The original note: in row F2 a few frill groups drew near-white
> (overbright), as bright at night as at noon; they were frills on lit, alpha-blended
> `ColorAndOpacity` surfaces with a `TSNormalAndGloss` normal map and specular power 0
> (block 3/0x7A69: 74 frill objects, 22,945 records, figures from that review's probe).

> **Answered (implementer, 2026-10-07):** by `terrain-texturing.md` §4.9; the walker
> follows it (§14.4). Row F1 renders its 13 × 13 blocks with key 0 of 0x19000000 under
> the frills. The original note: what the original draws for a terrain chunk that has no
> layers; in region 1 alone 17,020 landblock-data records have none while their heights
> vary (703 of them with a landblock info), region 2 has 11,359 and region 5 42,807 (that
> review's probe; `terrain-texturing.md` §4.9 now gives the census of all 390,150 records).

> **OPEN (implementer, 2026-10-06):** the walker's keys of §14.2–§14.3 are read as raw
> key codes, outside `tessera-input`'s actions and contexts: a keymap binding on one of
> them wins, but they cannot be rebound and a future text-entry context (chat, M5) would
> not make them opaque. Registering them as actions needs the input registry and the
> walker's controls (`controls.rs`).

> **Note (performance, not a question about this design; for the world and physics
> implementers):** flight M3-b (3,000 m/s) showed one near-ring landblock missing for
> 13–21 frames near the end of the path in every run, also with every M4.5 system off and
> with the M3 ring size: the bundle (with its collision set, generated scenery and water)
> arrived late. Criterion 5's "no near-ring holes" is not met there
> (`docs/perf/reference-machine.md`, which is the record for these measurements). The
> late landblock is r2 (239, 242): its bundle job takes 0.53–0.75 s on one worker on a
> loaded machine and up to about 2 s under a load average of 40, almost all of it the
> collision set (about 160 ms placing the static bodies, about 570 ms building the set's
> hierarchy, for a bundle of about 104 MiB). The streamer's lookahead (`tessera-world`
> streamer "Lookahead", ARCHITECTURE §9.2: an urgent part of 1.5 s, or 1.5 times the
> slowest recent job, at `Visible` priority, and an early part out to 4 s at `Prefetch`)
> removed the late landblocks in streamer-only indicative runs (r2 (239, 242) never
> missing in 11 runs, against 14–26 frames in 6 of 11 without the early part). Still to
> measure: the walker's own M3-b flight on the quiet reference machine, which criterion 5
> counts. A cheaper collision build (shared per-object hierarchies, or building the
> hierarchy on several workers) would remove the remaining risk; the build order and the
> set's contents stay as `collision.md` §12.1 gives them.

<a id="s14-12"></a>
### 14.12 Interiors (Decisions; spec writer, 2026-10-07, from the implementer's note)

The walker draws interior cells through the portal system of `landblock.md` §4.11–§4.14
(rows W-V1–W-V5, W-M1 of §14.13). Code: `apps/client/src/walker/interiors.rs`,
`objects.rs` (cell meshes as parts, `ObjectInfo::cell`), `terrain.rs` (`block_objects`),
`env.rs` (indoor record), `walker/mod.rs`. What follows is the rule set as built; every
choice the specs leave open is labelled **Decision**, and every place where the walker does
something the client does not is labelled **departs**.

**(a) The camera's cell (`landblock.md` §4.12 Decision).** Each frame, before the streamer
update, the walker looks the eye up (the eye before it is kept above the terrain): every
resident landblock whose cells' bounds contain the eye is asked with the containment lookup
of `collision.md` §5.8.1, the eye's own landblock first, then by Chebyshev distance from it,
then by region, x and y; the first that finds a cell names it, and within one landblock the
lowest cell number wins (Decision). When none does and the walker is in walk mode on
collision, the mover's cell (§13.4) is taken. The result is the cell number and its owner
landblock. **Decision:** only the walk-mode third-person camera is kept 0.8 m above the
terrain (§5), and only outside a cell; free-fly, `--fly-path` and `--camera-loop` cameras are
never raised.

**(b) Visibility of a landblock's cells (`landblock.md` §4.11–§4.12).** Built from the owner
landblock's resident bundle when the camera first enters one of its cells, and kept while
that bundle stays resident:

1. visibility cells: each cell, in stored order, shares the visibility cell of the first
   cell of its group list (`landblock.md` §4.11; the cell itself skipped) that already has one, else gets
   a new one;
2. portals: for each cell and each neighbour entry, unless the neighbour is in the cell's
   group list or the entry's 32-bit back-reference is negative (none is, row W-V2), a
   one-way portal from the cell's visibility cell into the neighbour's, shaped as the portal
   that the entry's `portal` field names in the cell's 0x01 geometry, placed with the cell's
   frame (taken from the landblock's collision set; `collision.md` §5.5, §5.8), as the
   client does (`landblock.md` §4.11, L39). The back-reference is the i32 that the
   decoder's two u16 halves form (equal to the `landblock.md` §4.4 field while `extra_cells`
   is empty, which it is in every entry of this install);
3. the potentially visible set of each cell (`landblock.md` §4.12): the cell, its list A, and with
   `cell_flags` bit 5 the members of the groups its list B names (a list-B key 0xFFFF names
   no group). **Decision** (malformed lists): the cell's own number, 0xFFFF and numbers that
   are no cell of the record are skipped and add nothing.

When the owner landblock is not resident, the camera counts as inside with both scene
flags clear and no interior drawn (Decision).

**(c) Scene flags (`landblock.md` §4.12 rule 3).** A visited cell with `cell_flags` bit 0
clears both flags; then, with bit 4, it sets sky-visible. Inside: both flags start clear and
the camera's cell is visited with its potentially visible set, in the order cell, list A,
list-B group members, each cell at most once. Outdoors: both start set and every bit-0 cell
of the resident landblocks is visited with its potentially visible set. **Decision:** the
landblocks are taken in ascending key order (the client's order was not traced; it matters
only when bit 0 occurs, and no cell of this install has it, LI8).

**(d) Portal walk (Decision of `landblock.md` §4.11, with these choices).** From the camera
cell's visibility cell, recursively: every visibility cell reached is drawn; each portal of
a reached visibility cell is crossed when its screen rectangle overlaps the current one,
and the walk continues into the portal's target with the overlap. Screen rectangles are in
normalised device coordinates, starting from the whole screen. A portal polygon is clipped
against clip-space *w* ≥ 10⁻⁴ before its bounds are taken; a polygon wholly behind the eye
is not crossed. An eye within 0.5 m of the portal's plane and inside its bounding box
widened by 0.5 m passes the view on unchanged (an edge-on doorway the eye stands in has no
screen area); so does a portal with fewer than three corners or with a non-finite corner.
Portals are two-sided. At most 32 portals deep and 4,096 portal crossings per frame (only
portals actually crossed count, not portals tested and rejected); a visibility cell may be
reached again through another portal. No occlusion culling.

**(e) What is drawn from inside.** The objects of the owner landblock attached to a cell
whose visibility cell the walk reached (cell meshes and interior entities), and the
unattached interior entities of that landblock ((h) below). No outdoor object, no terrain or
ceiling chunk, no landscape water (`landblock.md` §4.12). The sky only while sky-visible is
set, from the indoor record (§14.3, §14.8; `landblock.md` §4.12 gives this as Inferred).
- **Frills:** not streamed and not drawn while outdoor-visible is clear; the client sets up
  no landscape pass then, frills included (`landblock.md` §4.12, L41).
- **Characters (departs from `landblock.md` §4.11–§4.12; OPEN for the implementer in
  `landblock.md` §4.15):** the avatar and the figures of the character layer are always
  drawn, inside or out. The client does not switch moving entities by the scene flags but
  culls them with the portal query; a moving entity belongs to the visibility cell of the
  cell it stands in (Inferred there), so the client draws a character only in a visibility
  cell the camera's walk reaches, and from outdoors only characters standing outdoors. The
  walker's avatar placeholder (drawn before the avatar's figure is ready) follows that rule:
  it is drawn only when the mover's cell, or outdoors, is visible from the camera.

**(f) From outdoors** no interior object is drawn (`landblock.md` §4.12: the outdoor
visibility cell has no portal into any cell). **Decision (resources):** interior objects are
loaded only while the camera is in a cell of their landblock, and a landblock whose outdoor
objects are all out of reach is still visited when the camera is in one of its cells (cells
may lie landblocks away from their owner, §13.3).

**(g) Cell meshes (`landblock.md` §4.14; row W-M1).** One object per placed cell of the
landblock info: its part is the cell's 0x01 geometry record, whose embedded render mesh
(`collision.md` §5.7 part 11) is decoded and drawn; placed with the cell's position and
rotation, without `Render_AdjustableScale` (the `landblock.md` §4.14 Decision). Cells numbered 0 or 0xFFFF
are left out. A cell whose position has no x, y, z offset (two cells in this install) is
drawn at the landblock origin, and one without an orientation with the identity, as
`landblock.md` §4.4 gives (L42). **Departs (collision side):** `tessera-physics` still
refuses to place those two cells in the collision set, so the camera's cell lookup and the
mover never find them and the portals out of them have no polygon (they pass the view on
unchanged, (d)); OPEN for the physics implementer in `landblock.md` §4.15.
**Decision** (`landblock.md` §4.14 OPEN on cell detail levels and distances): drawn out to
*G* and the object distance cap, always at the finest level.
- **Appearance (departs from the `landblock.md` §4.14 Decision):** the cell's
  `Appearance_InstanceList` is not applied to its mesh (80,825 cells carry it,
  `landblock.md` §4.14).

> **OPEN (spec writer, 2026-10-07, for the walker implementer; also in `landblock.md`
> §4.15):** apply each cell's `Appearance_InstanceList` to its render mesh as `appearance.md`
> §3.2 applies an instance list to an entity (the Tessera Decision of `landblock.md` §4.14).

**(h) Interior entities (`entity-descriptor.md` §4.8; Decision on attachment, the stored
rule being OPEN there).** An entity with the interior flag (0x400) is attached to the cell
of its landblock's collision set whose cell volume contains its origin, tried at the origin,
0.05 m above and 0.05 m below it (the lookup of `collision.md` §5.8.1 within that landblock);
an entity that no cell contains, and every interior entity of a landblock whose bundle carries no
collision set (§13.3), is
**unattached** and is drawn whenever the camera is in a cell of its landblock.

**(i) Lights (Decision).** From inside, only the interior Light entities (flag 0x400) within
200 m enter the light table; from outdoors only the outdoor ones (and the interior ones too
with `--interiors`). `landblock.md` §4.14 names the interior Light entities as the point
lights of cells; which lights the client's scene offers outdoor objects, and whether it
restricts interior lights to the visibility cells the camera reaches, was not traced. In this
install 40,727 Light SEDs are interior and 32,984 outdoor (row W-L6).

**(j) Environment:** the indoor record of §14.3.

**(k) `--interiors` (Tessera's own debugging view, no client equivalent):** every interior
cell and interior entity is loaded and drawn whatever the camera's cell, and interior lights
join the light table from outdoors. Outdoor objects are still not drawn while the camera is
inside a cell.

**Checked on the install** (`apps/client/tests/interiors_real_data.rs`, 2026-10-07; on every
record in debug builds too): 49,636 landblock infos, 104,271 cells, 18,390 visibility cells,
39,692 portals (the census of `landblock.md` §4.11), LI7 and LI8 on every record, the IN1
example of `landblock.md` §4.11–§4.12 (26 portals, all with polygons) and the Y5 eye in cell 9,
found through the walker's own camera-cell lookup (the IN1 start + 1.8 m), with its scene
flags. `visual_rows_real_data.rs` checks rows T and F1 through the walker's
`terrain-texturing.md` §4.9 path, row Y5 through the walker's indoor-record lookup (key B 1,
*X* = *Y*, the fog of the `landblock.md` §4.13 worked example) and key A of rows Y1–Y4
through the walker's lookup of §14.3. The interiors fuzz target feeds neighbour entries whose
`portal` field differs from their position and cells with hostile `index_echo` values.



<a id="s14-13"></a>
### 14.13 Client facts §13–§14 rely on, and their provenance (spec writer, 2026-10-07)

Every fact about the original client or its data that the code of §13–§14 (with
`tessera-render` and the `tessera-world` streamer under it) relies on, restated here with its
source. Marks: **BIN** = read from the client (decision 0011); **Confirmed** = observed in
the user's install; **Spec** = taken from the cited spec, whose own provenance applies.
"Re-checked" means this spec writer verified it again on 2026-10-07 in an own copy of the
Ghidra project of the installed client build (the build of §11.1) or with an own probe over
every record of the kind. Counts are observations of this install and are read at run time,
never hard-coded.

**BIN source of the re-checks (decision 0011).** `x64/lotroclient64.exe` of the user's
install, SHA-256 prefix d22363eb9aff0fb5 (the build of §11.1, unchanged), read with the
Ghidra headless decompiler in a private copy of the project on 2026-10-07 by this spec
writer. **Questions:** (j) which property sets, in which order, and which defaults the
outdoor and indoor environment records read, and when the interior overrides are re-read;
(k) where the fog limits stand relative to the interior override, and whether the indoor
record has a main light; (l) how the light-list builder applies the parent switches to a
light without a parent, which objects it considers, and how it treats baked-landscape lights;
(m) whether baked-landscape lights are also drawn into a chunk's baked-lighting texture, with
which pass code and target size. **Why M1–M3 were not enough:** these are run-time rules
with no trace in the data (the data shows only which records carry the properties, rows
W-E3–W-E5, W-L6); no running client or frame capture was available; and the specs under
revision disagreed on (k) and (l). Only offline environment and rendering code was read; no
network, login, launcher or anti-tamper code. **Data probe:** a throwaway reader in the dirty
scratch area over all 49,636 landblock-info records of the install, read-only. **Second
revision (2026-10-07, after the re-review):** no new decompilation; the listing of the
interior-override reader kept from question (j) was re-read for the stored types each
override accepts (row W-E8), the probe was extended to record the stored type of
`Environment_IndoorAmbiance` (row W-E9), and the LOD-class record 0x32000000 was read with
the repository's `tessera-dattool show` and its class labels with `tessera-props` as tools
(row W-K1). **Third revision (2026-10-07, after the second re-review):** question (n) which
state's sun-present flag the client tests, where, and whether a change between the outdoor
and the indoor record starts the sun-change fade. The environment-state builder of question
(j) was re-read for the flag (row W-E10), and the per-update routine that picks the record
in use and the routine that produces the shown state were decompiled (row W-E11). **Why
M1–M3 were not enough for (n):** a run-time rule with no trace in the data; no running
client or frame capture was available; and two Accepted specs read as disagreeing
(`environment.md` §11.6 names going indoors as a case of the fade, `landblock.md` §4.13 calls
the change of record immediate). Again only offline environment code was read. The
keyframe inputs of the flag were counted with the repository's `tessera-dattool show` over
every day description (row W-E12). **Fifth revision (2026-10-08, after the re-review of the
fourth revision):** question (o) whether the client advances an object's draw-distance fade
and its smoothed distance for objects that are not drawn in a frame, and whether a frame
without that step restarts the fade. Read in a private copy of the Ghidra project (same
build; copy deleted afterwards): the scene's per-frame cull pass, the per-object, per-view
step it calls for each object of the scene, and the fade step within it (row W-O3). **Why
M1–M3 were not enough for (o):** a run-time rule with no trace in the data; no running client
or frame capture was available; `mesh.md` §8.6–§8.7 state when the fade restarts but not
which objects the step visits. Only offline scene and rendering code was read. A throwaway
probe with the repository's `tessera-dattool show` over every day description counted the
alpha of the keyframe sun colours (row W-E13).

| Row | Fact | Used in | Source | Mark |
|---|---|---|---|---|
| W-C1 | Chebyshev distance, in landblocks, of the landblock holding a cell's origin from the cell's owner, over the 104,269 placed cells (2 more have no x, y, z): 0: 16,798; 1: 60,963; 2: 17,324; 3: 5,328; 4: 2,311; 5: 1,344; 6: 201 | §13.3 | own probe over all 49,636 landblock infos | Confirmed 2026-10-07 |
| W-C2 | Cell containment lookup: bound of 7,593,750 on \|*p*\|², phantoms overlapping *p* ± 0.05, cell volume test at −0.0002; ties by broad-phase order (Tessera: lowest number) | §13.4, §14.12 (a), (h) | `collision.md` §5.8.1 | Spec (BIN) |
| W-C3 | Capsule, movement record, conditions and float height of the mover | §13.1–§13.2 | `collision.md` §10.3–§10.6, §11.1–§11.3, §12 | Spec (BIN) |
| W-V1 | Visibility cells, portals and their skip rules, the portal shape from the cell's geometry | §14.12 (b) | `landblock.md` §4.11 | Spec (BIN2); census Confirmed by `landblock.md` and by the walker's real-data test |
| W-V2 | The neighbour entry's back-reference is a signed 32-bit value (the 16-bit value read first is its low half); none is negative in this install | §14.12 (b) | `landblock.md` §4.4 neighbour-entry table | Spec (BIN2, Confirmed on all 393,842 entries) |
| W-V3 | Potentially visible set; scene flags outdoor-visible and sky-visible and what they switch (terrain, ceiling chunks, landscape water; sky); no portal leads outside | §14.12 (c), (e), (f) | `landblock.md` §4.12 | Spec (BIN2) |
| W-V4 | `cell_flags` bit 0 (outdoor-walk entry) never set; bit 4 sky visible in 9,493 cells; bit 5 tail format | §14.12 (c) | `landblock.md` §4.4 | Spec (Confirmed) |
| W-V5 | Visibility never leaves a landblock; a portal's polygon is the one the entry's `portal` field names (equal to the entry's position in every entry); with outdoor-visible clear no landscape pass is set up (terrain, ceilings, landscape water, frills); the two cells without x, y, z lie at the landblock origin with the identity orientation; a position in a cell names the cell's owner landblock (Inferred) | §14.12 (b), (e), (g), §14.3 | `landblock.md` §4.4, §4.11–§4.13, rows L38, L39, L41, L42 | Spec (BIN3; L38 Inferred for the lookup) |
| W-M1 | A cell draws the render mesh embedded in its 0x01 record, placed with the cell frame, adjustable scale not applied | §14.12 (g) | `landblock.md` §4.14, `collision.md` §5.7–§5.8 | Spec (BIN2, L35) |
| W-E1 | Two environment records: outdoor from key A (character's housing hook, local-sky override, local sky; else `Environment_LocalSky` at the position; else 2), indoor from key B (character's indoor-ambiance override, indoor ambiance; else `Environment_IndoorAmbiance` at the position; else 1); position lookup cell, landblock info, block map; the indoor record used while outdoor-visible is clear; each blends on its own | §14.3 | `landblock.md` §4.13, `environment.md` §7.1.1, §11.6 | BIN; the key precedence and the defaults 2 and 1 re-checked 2026-10-07; the position lookup's internals, the choice of record by the flag and the separate blending as those specs report |
| W-E2 | Interior overrides: upper and lower ambient (absent: opaque black) replace *X* and *Y* when any of the three ambient properties is present, the plain ambient added to both, every channel limited to at most 1; `InteriorFogType` present: fog on iff the fog switch is on and the value is 1, absent: the keyframe's; fog colour, near and far replace (absent: keyframe values); a colour counts only with type 16, near and far only with type 9, `InteriorFogType` only with type 8 or 10 (row W-E8); re-read when the position's region, landblock or instance changes, else at most every 2.5 s | §14.3 | `landblock.md` §4.13, `environment.md` §7.2 | BIN, re-checked 2026-10-07 (the absent near and far are kept as the keyframe's) |
| W-E3 | `Environment_LocalSky` is stored in 193 landblock-info property sets and 299 cell property sets; `Environment_IndoorAmbiance` in 40 landblock-info sets and 777 cells | §14.3 (key A, key B) | own probe; agrees with `environment.md` §7.1–§7.2 | Confirmed 2026-10-07 |
| W-E4 | `InteriorFogType` in landblock-info sets: 0 in 5, 1 in 499, 2 in 37; in cells: 1 in 386, 2 in 389 | §14.3 | own probe | Confirmed 2026-10-07 |
| W-E5 | `InteriorFogFar` (775 cells, 540 landblock-info sets) lies in 0–7,000; `InteriorFogNear` at most 5,000 | §14.3 (fog limits) | own probe | Confirmed 2026-10-07 |
| W-E6 | Fog limits: the keyframe's fog end is capped at 10,000 first; the interior override then replaces it without that cap; then end ≥ 20, start ≤ end − 20 and ≥ −10,000 | §14.3 | `landblock.md` §4.13, `lighting.md` §10 | BIN, re-checked 2026-10-07 |
| W-E7 | The indoor record has no main light (sun and moon give no light indoors) | §14.3 | `landblock.md` §4.13 | BIN, re-checked 2026-10-07 |
| W-E8 | Key A and key B count as present only when the stored value has property type 8 (enum value) or 10 (property ID); `InteriorFogType` is tested the same way (type 8 or 10); a value of another type is reported and treated as absent, so a wrongly typed `InteriorFogType` leaves the keyframe's fog on or off | §14.3 (stored types) | `environment.md` §7.1.1, row E21 (keys); BIN (j) listing of the interior-override reader (`InteriorFogType`) | Spec (BIN-E, Accepted) for the keys; BIN, re-read 2026-10-07, for `InteriorFogType` |
| W-E9 | Every stored value of `Environment_LocalSky` (299 cell and 193 landblock-info property sets), `Environment_IndoorAmbiance` (777 and 40) and `InteriorFogType` (775 and 541) is an enum value (type 8); a block map's palette value is encoded by its property's registered type (`landblock.md` §5.2), an enum for `Environment_LocalSky`. A property set carries no type tag per value: every value is decoded by the type of its property in the dictionary (`property-system.md` §5.2, §5.3 step 2), as `tessera-formats` does. So the probe confirms the dictionary's type of the three properties (enum), and the stored-type departure of §14.3 can show only with a dictionary that gives one of them another type (another build) | §14.3 (stored types) | own probe over all 49,636 landblock infos and their 104,271 cells; `landblock.md` §5.2; `property-system.md` §5.2–§5.3 | Confirmed 2026-10-07 |
| W-E10 | Sun present: true only on the outdoor record, while the blended keyframe `effect` is the null DID, the sun colour differs from opaque black (0, 0, 0, 1) by more than 0.0002 in some channel, and the sun brightness is at least 0.0002; always false on the indoor record; where false the renderer has no main light (no sun term, no sun shadow; one submission path adds the default directional light of `lighting.md` §8.5; the water's default fake light, `lighting.md` E18). 503 of the install's 7,590 keyframes carry a non-null `effect` | §14.3 (sun-presence departure) | `environment.md` §11.7 step 9, §11.3 step 7, rows E24, E28, §5.3 table (census); `lighting.md` §8.5, E18 | Spec (BIN-E, Accepted; census Confirmed there); the test re-checked 2026-10-07 (BIN (n)) |
| W-E11 | Sun-change fade: each update the client takes the outdoor record's state while the landscape is visible and not forced indoor, else the indoor record's; when that state's sun-present flag differs from the one of the previous update and the update is not immediate, the shown state fades from the state shown at that moment (held fixed) to the state in use over 5 s of the raw timer, or, when a fade is already running, over the time since the previous change (at least 0.0002 s); weight ½ + ½ sin(π (*s* − ½)) and the blend of `environment.md` §11.6 step 4; an immediate update ends the fade. So a change between a sunlit outdoor state and the indoor state is faded; the choice of record itself is immediate | §14.3 (sun-presence departure, record change) | `environment.md` §11.6 (row E34); `landblock.md` §4.13 | Spec (BIN-E, Accepted); the record choice, the trigger on the state in use and the fade re-checked 2026-10-07 (BIN (n)) |
| W-E12 | Sun-present inputs per keyframe over all 963 day descriptions (7,590 keyframes): `effect` not null in 503; sun brightness below 0.0002 in 558 (557 exactly 0, one 0.0001: a keyframe without `effect` whose sun colour and specular colour are not black, the one exception among the 154); sun colour opaque black in 6; at least one of the three in 657; 154 without an `effect`, of which 153 also have an opaque black specular colour. Blended values between keyframes are not counted | §14.3 (consequences of the sun-presence departure) | own probe with the repository's `tessera-dattool show` over every 0x1A record, re-run for the exact values below 0.0002; agrees with the `environment.md` §5.3 census (503, 558; its 558 count includes the one 0.0001) | Confirmed 2026-10-07 |
| W-E13 | Sun colours of all 7,590 keyframes (963 day descriptions): 6 have R, G and B all 0, and all 6 have alpha 255; the alpha of the others varies (0 in 222, 255 in 5,390, other values in the rest) | §14.3 (sun-colour alpha departure) | own probe with the repository's `tessera-dattool show` over every 0x1A record | Confirmed 2026-10-08 |
| W-L1 | Selection radius ρ = Range × (1 − 0.15 × min(Falloff, 1)), Range for `Light_GlobalLight` | §14.3 | `lighting.md` §8.5 step 1 | Spec (BIN) |
| W-L2 | A light reaches an object when the squared distance from the light to the object's world box is below ρ² (strict); key lights are inserted at the front, others appended; for the light's parent the never-parent switch is tested, for every other object the only-parent switch, whether or not the light has a parent, so a light with `Light_OnlyInfluenceParent` and no parent reaches no object (one special light record's parent is looked up separately, not traced). No Light SED of landblock info sets `Light_OnlyInfluenceParent` true (136 store false) | §14.3 | `lighting.md` §8.5 steps 2–4, row L34; census own probe | BIN, re-checked 2026-10-07; census Confirmed 2026-10-07 |
| W-L3 | Only objects drawn in the current frame are candidates, and objects carrying any of three scene flags are skipped | none (the walker hands the renderer only drawn objects) | `lighting.md` §8.5 step 2, row L34 | BIN, re-checked 2026-10-07 |
| W-L4 | A light with `Light_BakedLandscapeLight` is skipped for every terrain and ceiling chunk (the landscape-chunk object kind), light map or not (L30) | §14.3 | `lighting.md` §8.3, §8.5 step 3 | BIN, re-checked 2026-10-07 |
| W-L5 | The client's light-map bake draws each baked-landscape light that reaches a chunk with pass code 0x41 into an off-screen target of at most 1,024 × 1,024 and sums the results into the chunk's baked-lighting texture; it runs only after a re-bake flag that a special client mode raises (Inferred: not in normal play) | §14.3 | `lighting.md` §8.6, rows L35, L36 | BIN; the 0x41 draw and the target size re-checked 2026-10-07, the trigger as `lighting.md` reports |
| W-L7 | The client builds a light list **for each object** drawn in the frame, against that object's own world box (key lights first, then the others in processing order); no list is shared between objects | §14.3, §14.5 (per-instance light lists) | `lighting.md` §8.5, row L32 | Spec (BIN) |
| W-L6 | Light SEDs of landblock info: 40,727 with the interior flag, 32,984 without; `Light_BakedLandscapeLight` true on 23,103 outdoor and 221 interior ones | §14.12 (i) | own probe | Confirmed 2026-10-07 |
| W-T1 | Chunks without layers: the open flag and the one-layer rule | §14.4 | `terrain-texturing.md` §4.9, `landblock.md` §3.15 | Spec (BIN (r), census Confirmed) |
| W-T2 | Grid vertex normals: average of the unit triangle normals, not renormalised; ceiling rule mirrored | §14.4 (Decision) | `terrain-texturing.md` §4.10, `landblock.md` §3.14 | Spec (BIN (s), BIN2 (g)) |
| W-T3 | Ceiling chunks: ceiling heights and normals, `Landblock_CeilingLightMap`, layers as the terrain's | §14.4 | `terrain-texturing.md` §4.8 | Spec (BIN (d)) |
| W-K1 | Character classes `PlayerAvatar`, `Creature`, `Creature2Deg` have `d0` 80 in this install; the render component's initial values `d0` … `d7` = 200, 60, 16, 80, 1000, 1000, −1, 1100 in record order (`mesh.md` row M35). The three classes (keys 0x10000014, 0x10000008, 0x10000004 of mapper 0x2300008D here) all have `d2` 26 | §14.3, §14.5 | §11.4, §11.4.2 of this file; `d2` from the record 0x32000000 (all 32 entries) | Spec (BIN; census Confirmed); `d2` Confirmed 2026-10-07 |
| W-O1 | An object casts shadows only while its opacity (the fade opacity × the render component's two factors × the inherited opacity where `Render_InheritOpacity` × the tint alpha, which is 1) is **above 0.8** and its unsmoothed distance is **strictly below** min(*G*, `d2` × *M_d* × *M_s*); no distance limit while distance-based levels are off | §14.3, §14.5 (shadow casting; characters' departure) | `mesh.md` §8.7 `d2` row, §8.8, rows M27, M40, M46 | Spec (BIN, Accepted) |
| W-O2 | Characters are entities like any other: their shadow distance comes from their own class, min(*G*, `d2` × *M_d* × *M_s*); for the player's own character `d0` … `d5` (so `d2` too) are 3 × the entry; an entity whose property layers never hold `Render_LODClass` keeps `d2` 16 | §14.3, §14.5 (character-shadow departure) | `mesh.md` §8.5 steps 3–4, §8.7, rows M33, M45 | Spec (BIN, Accepted) |
| W-O3 | The client's scene cull pass runs, each frame and for every object of the scene, before the frustum and occlusion culls, one per-object, per-view step that smooths the distance (`mesh.md` §8.6) and advances the draw-distance fade; the fade's opacity is (time since the fade began) / 1.5 s, so it does not depend on the step running in every frame; the fade restarts (appear or disappear at once) only in the first frame of the object's per-view state, when the camera moved more than 25 m, or on a render-state reset. One culling mode, not traced, can skip the step for objects that fail an earlier visibility query | §14.5 (fade departure; smoothed distance) | BIN (o); `mesh.md` §8.6–§8.7 (rows M24, M36) | BIN 2026-10-08 (the culling mode not traced) |
| W-S1 | Frill groups, distances, reduction and fade-in; the first part's first draw entry | §14.7 | `scenery.md` §8.2–§8.5 | Spec (BIN) |
| W-W1 | Water parameters, shading and the fake light; water reads no table light | §14.3, §14.6 | `water.md` §8–§9 | Spec (BIN) |
| W-Y1 | Sky objects: rotation (first about +Z by −*B*, then about +Y by −*A*), tint as `Render_Color` and `Render_Opacity`, not drawn while fog is on and `fog_extra` > 0.99; at most 136 per day description | §14.8 | `environment.md` §5.4, §11.4, §11.7 step 10, §12 | Spec (BIN-E, Accepted) |

**Departures, in one place** (each is labelled where it occurs; state of the code on
2026-10-08):

| Where | The walker | The client | Resolution |
|---|---|---|---|
| §13.4 | keeps the current cell, then portal neighbours, then the first set in view order | lookup after every step, lowest number among all containing cells | Decision |
| §14.3 sun presence | no test on the blended `effect`; the sun-colour test reads R, G, B only | no main light while the blended `effect` is not null, or the sun colour (all four channels) is opaque black, or *bd* < 0.0002 (W-E10) | OPEN for the walker and `tessera-world` implementers (§14.3); the `effect` part is visible during the 503 weather keyframes; the alpha part has no effect at this install's keyframes (W-E13) |
| §14.3 record change | outdoor ↔ indoor at once | 5 s sun-change fade whenever the sun-present flag of the state in use changes, so on every change between a sunlit outdoor state and the indoor state (W-E11) | OPEN for the walker and `tessera-world` implementers (§14.3) |
| §14.3 parent switches | `Light_OnlyInfluenceParent` and `Light_NeverInfluenceParent` not passed; no light has a parent | a parentless light with `Light_OnlyInfluenceParent` reaches no object (W-L2) | OPEN for the `tessera-world` and walker implementers (§14.3); no effect on this install (W-L2) |
| §14.3, §14.5 character shadows | a fixed 16 m for every character, the player's own included (no *M_d* × *M_s*, no cap at *G*, no ×3); only at opacity 1 and *d* ≤ 16 m | min(*G*, `d2` × *M_d* × *M_s*) of the character's own class, `d2` × 3 for the player's own; while opacity > 0.8 and *d* strictly below (W-O1, W-O2; `d2` 26 in the three character classes, W-K1) | OPEN for the implementer (§14.5) |
| §14.4 normals | unit central-difference normals | `terrain-texturing.md` §4.10 averages | OPEN for the implementer |
| §14.4 far band | light-map term × `diffuse_scale` in the baked far layer | light-map term not × `diffuse_scale` (`terrain-texturing.md` §9.1); the far band itself is Tessera's (the original's bands OPEN) | OPEN for the walker and render implementers (§14.4) |
| §14.5 distance box | box around the parts' bounding sphere | union of the level-0 vertex blocks' stored boxes | Decision |
| §14.5 fade restart | a fade restarts after any frame in which the object's fade was not updated (block out of view, draw cap, object not in the scene); characters after a frame out of view | the fade step runs for every object of the scene each frame; restart only in the state's first frame, after a camera jump > 25 m or a render-state reset (W-O3) | OPEN for the implementer (§14.5) |
| §14.5 character draw distance | `d0` 80 m as a constant | each character's class | OPEN for the implementer (§14.5) |
| §14.5 unlit blend pairs | other pairs drawn as source-alpha blend; additive without depth write | the record's own pair and depth write (`material.md` §11.3 rule, a Tessera Decision there; 21 records, 57 surface records) | OPEN for the walker and material implementers (§14.5) |
| §14.5 unlit fog | every unlit surface fogged | fog only when the record's unlit program reads the fog inputs (`lighting.md` §7 code 0x00) | OPEN for the walker and material implementers (§14.5) |
| §14.12 (e) characters | always drawn | culled by the portal query | OPEN for the implementer (`landblock.md` §4.15) |
| §14.12 (g) cells without x, y, z | drawn at the landblock origin, but refused by the collision cell placement (camera lookup, mover, portal polygons) | at the landblock origin, identity orientation, everywhere (L42) | OPEN for the physics implementer (`landblock.md` §4.15) |
| §14.12 (g) cell appearance | `Appearance_InstanceList` not applied | applied (`landblock.md` §4.14: private copy with the changes) | OPEN for the implementer (here and `landblock.md` §4.15) |
| §14.12 (i) lights | interior lights only from inside, outdoor only from outside | not traced | Decision |

Withdrawn on 2026-10-08 because the code now follows the client (rows kept in the change log
only): key A lookup, stored types of the keys, black or dim sun, fog limits, light lists
without reach entries, one light list per instanced batch, object shadows while fading and
the ≤ test, portal polygon by entry position, simplified scene flags, sky objects whatever
the fog, sky placement.

---

<a id="s12"></a>
<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-04 | §1–§9: M3 design (clean-room implementer). |
| 2026-10-05 | §8.1, §10: M4 characters (clean-room implementer). |
| 2026-10-07 | §10.8: test-server sessions, implementer notes and OPENs (clean-room implementer, M7). |
| 2026-10-05 | Spec writer (dirty room): new §11 (the original's preference levels and built-in defaults, quality presets, landscape extent and the global distance limit, per-class object and character draw distances with fade and detail ranges, camera-state record layout and census, camera-state selection, third-person login placement and behaviour, default-settings profile for the walker, OPENs, corrections to other specs, invariants, worked examples); §9 draw-distance OPEN marked answered; header gains the §11 status and reviewer rows. §11 status: Review. |
| 2026-10-05 | Spec writer (dirty room), answering the review of §11 (changes requested): §11.1 BIN-W gains a second M4 session with questions (g)–(i) and OBS-W the placement-level census; W1 extended (FrillDensity, PlayerCrowdQuality, DialBackGraphics), W5 cites (h), new rows W20–W26. §11.2: case-insensitive reading marked Tessera Decision. §11.3 step 3: the missing flag condition (*S* off, or *F* set and *V* > 0.99), where *F* and *V* come from, OPEN extended. §11.4 step 4 and §11.4.1: the no-class values now rest on BIN (render-component initial values, not the dictionary default); new §11.4.3 (how the class reaches the render component; census over all Physics SEDs; OPEN on SED versus 0x47 precedence with a Tessera Decision). §11.4.2: NPC class statement reworded with the census. §11.5.3: byte-layout tables for the type-2, type-3 and type-4 bodies. §11.7 steps 1–2: look-at height fallback, under-water switch *U* and the yaw rule with flags *R*, *X*, *P*, *Q* (BIN), three new OPEN callouts. §11.8 rows updated. §11.9: OPEN list extended; `Render_FarViewDistance` census corrected (384,845 SEDs, not 8). §11.10: corrections to `mesh.md` §8.5/§8.8 and `scenery.md` §9. §11.11: W-I7–W-I9. §11.12: fog-clip, class-resolution and yaw examples. §11 status stays Review. |
| 2026-10-06 | New §14: world fidelity in the walker (M4.5; clean-room implementer): the lit world pass with texture layers, ceilings and the far band, objects by LOD class with fades, detail levels, lit surfaces and instancing, water, frills, sky, the clock and its controls, the switches, the default-settings profile as adopted, new frame-log columns and options, the checklist rows as rendered (§14.10a; performance in `docs/perf/reference-machine.md`), and the terrain job ranking, upload-budget start and far-band mask deferral found by the M3 flights. |
| 2026-10-06 | New §13: walk mode on collision (M4.5; clean-room implementer): the physics walker on its fixed 1/60 s step, the mover, collision-set residency (the feet's 3 × 3 landblocks; absent landblocks as a solid boundary), cell tracking through portals, water masked inside cells, scripted walks (`--walk-path`, `--walk-log`) and the 1,000-walk soak, with their first results. Supersedes §2's "controllers (30 Hz fixed step)" and §5's terrain-height walk mode when collision is built. |
| 2026-10-06 | Review round of §14 (clean-room implementer): the change log moves to §15 (anchor `s12` kept); §14.2 walker keys yield to keymap bindings; §14.3 pause and rate keys, `--day` range, baked lights on ceilings without a light map; §14.4 chunks without layers drawn height-shaded, masks kept A8 until upload and capped at 1,024, a chunk waits only for its own images (a far cave block's ceiling never completed, so the walker never settled near one); §14.5 part priorities (visible/prefetch), the activation pass reruns on a cap, switch or setting change, characters by class distance with a fade; §14.7 frills: anchor-free sector groups, nearest-first builds, cells merged per frill values, distance rules per sector, the first part's first draw entry; §14.9 characters row; §14.10a F1 no longer passes (OPEN); §14.11 OPENs updated; §13.9 avatar OPEN answered. |
| 2026-10-06 | Spec writer, applying the accepted decisions 0014 (owners list, "`walker.md` §5"; Consequences "Escape with a UI attached"; Cost) and 0015 (Decision 1.6; owner question 6): §5 gains the Escape rule with a game UI attached (`menu` becomes game code's `EscapeKey` handling, `game-panels.md` §16; the help label reads "menu"); §6 gains the overlay rule with a game UI attached (under the UI; `walker_minimap` off by default; `walker_hud` switchable and off in checklist runs). Tessera rules only; no format fact changed. |
| 2026-10-07 | Clean-room implementer (M4.5 close-out, released under incident 0018): §14.4 pointer to `terrain-texturing.md` §4.9 (chunks without layers); §14.11: the interior-cell OPEN answered by `landblock.md` §4.11–§4.14 and a note on the walker's interior choices, the F1 OPEN answered, two notes on the M3-b late landblock and the streamer's lookahead. (Row added by the spec writer; the additions themselves carried no row.) |
| 2026-10-07 | Spec writer (dirty room), answering the first review and the re-review of §13–§14 and incident 0018 (decision 0018, owner decision A): header review record shortened (details moved to the dirty-room review log); §13.2, §13.6, §13.7 references qualified with their spec; §13.3 cell-owner figures sourced (row W-C1, own probe) and the distance stated as the cell origin's; §13.4 rewritten as a Decision that departs from `collision.md` §12.3 step 8 and the §5.8.1 tie-break, with the view's set order defined; §13.8 marked a dated snapshot; §14.3 rewritten: two environment records (`landblock.md` §4.13), key A Decision and OPEN, key B and the interior overrides with the `InteriorFogType` condition, no sun indoors, fog-limit Decision, light table sides, the light lists as built against `lighting.md` §8.5 (OPEN), water without table lights, baked lights on the ground (Decision); §14.4: vertex-normal Decision and OPEN (`terrain-texturing.md` §4.10), ceilings, chunks without layers by §4.9 (replaces the height-shaded rule and the pointer OPEN), height-shaded fallback as a Decision; §14.5: distance-box Decision (`mesh.md` §8.6), character-distance Decision and OPEN, interior objects; §14.6 one-normal-layer Decision; §14.7 sector-box Decision; §14.8 136 objects cited (`environment.md` §5.4), sky from inside, the `fog_extra` departure and OPEN; §14.10 HUD and interior test; §14.10a marked a dated snapshot, rows T15/T16, F1, Y5, L5 annotated; §14.11: the interiors note moved to §14.12, cave-ceiling and L5 OPENs answered, the late-landblock notes merged into one performance note; new §14.12 (interiors, Decisions and departures) and §14.13 (client facts with provenance; BIN rows re-checked in an own copy of the Ghidra project, census rows from an own probe; table of departures). Status of §13–§14: Review. |
| 2026-10-07 | Spec writer (dirty room), §11 (non-blocking notes of the §11 re-review, and `mesh.md` §19): §11.4 step 1 says the draw test uses the unsmoothed distance (`mesh.md` row M25; the implementer OPEN there answered); §11.4 step 4 and row W20 give `d6` −1 and `d7` 1,100 in record order (`mesh.md` row M35); new OPEN for the `tessera-world` initial values. §11 status: Accepted → Revised. |
| 2026-10-07 | Spec writer (dirty room), second revision of §13–§14 answering the re-review of the same day (changes requested): §14.3 the stored-type rule of key A, key B and `InteriorFogType` labelled a departure with no effect on this install, with an OPEN (rows W-E8, W-E9; the "wrong type counts as absent" sentence now covers colours and floats only); §14.3 sun-shadow bullet: object and character shadows labelled as departing from `mesh.md` §8.7–§8.8; §14.3 light lists: instanced batches share one selection per batch, labelled as departing from `lighting.md` §8.5 (L32), with its effect on the night rows and the OPEN extended; §14.5: shadow casting while fading and the ≤ test split out of the fade-blend Decision as a departure, character shadow distance (fixed 16 m) labelled as a departure, the character OPEN widened to the shadow distance, the ×3 and the opacity rule; §14.11 fade-blend OPEN annotated; §14.12 (h): an interior entity is also unattached when its landblock's bundle has no collision set; §14.13: second-revision note under the BIN source, W-E2 states each override's accepted type, new rows W-E8, W-E9, W-L7, W-O1, W-O2, W-K1 gains `d2` 26 of the three character classes (Confirmed from 0x32000000), five new rows in the departures table. Row W20 (§11) cites `mesh.md` row M35 as a second source (no fact changed). Status of §13–§14: Review. |
| 2026-10-07 | Spec writer (dirty room), third revision of §13–§14 answering the re-review of the second revision (changes requested: one blocking item, three non-blocking): §14.3 new bullet "Sun presence and the sun-change fade": the walker's main light in every outdoor frame labelled a departure from the sun-present test of `environment.md` §11.7 step 9 (blended `effect` null, sun not opaque black, *bd* ≥ 0.0002), with its consequences (503 weather keyframes visible; in 154 further keyframes with a black or dim sun only the shadow pass and the water's fake light differ), and the walker's immediate change of record labelled a departure from the 5 s sun-change fade of `environment.md` §11.6; the "Two environment records", "Frame lighting" and "Sun shadow" bullets point to it; two OPENs (walker and `tessera-world` implementers, including the `effect` choice of `tessera-world` against `environment.md` §11.3 step 7 and §11.6 step 4; writers of `landblock.md` §4.13 and `lighting.md` §8.1 on the word "immediate"); "Sun shadow" now says the renderer's form uses the variants the `lighting.md` §9 Decision lists. §14.5: the player's character distance written min(*G*, 3 × 80 × *M_d*). §2, §3, §4: notes on what §13.1, §14.1, §14.4, §14.5 and §14.12 supersede and what stays in force (the 1.5 px cull, texture cap, GPU budget). §14.13: BIN question (n) with its reason, new rows W-E10 (sun-present test), W-E11 (sun-change fade; record choice re-checked in the client) and W-E12 (keyframe census of the flag's inputs, own probe), W-E9 states that a value's type is its property's dictionary type, two new rows in the departures table. No section number or anchor changed. Status of §13–§14: Review. |
| 2026-10-07 | Spec writer (dirty room), fourth, wording-only revision of §13–§14 after the acceptance, applying the non-blocking note of that review on row W-E12; no fact about the walker's behaviour changed: §14.3 "Black or dim sun" says a sun brightness below 0.0002 (exactly 0 in all but one keyframe, which holds 0.0001), and that in that one keyframe the walker's sun diffuse term is near black but not exactly; row W-E12 gives 557 keyframes at exactly 0 and one at 0.0001 (own probe re-run over all 963 day descriptions) and notes that the `environment.md` §5.3 count of 558 includes it. No section number or anchor changed. Status of §13–§14: Accepted → Revised. |
| 2026-10-08 | Spec writer (dirty room), fifth revision of §13–§14 answering the re-review of the fourth revision (changes requested: B1 stale body, B2 fog contradiction with `render.md`, B3 choices only in OPENs; N1 sun-colour alpha), checked against the code of the remediation worktree: §13.3 the pinned landblocks in every view as a Decision (implementer status OPEN folded); §14.3 rewritten to the current code: key A by the position lookup with fallback 2, key B and the overrides by the same lookup (also with outdoor-visible clear and no camera cell), stored types as the client, the black/dim-sun test now made (remaining departures: no `effect` test, sun-colour alpha, no 5 s fade; new row W-E13, own probe), fog limits in the client's order with the renderer's lower limits only (consistent with `render.md` §4.2; the fog departure is withdrawn), object shadows as the client (opacity above 0.8, strictly below the shadow distance; opacity factors taken as 1, Decision), reach entries for every light and the automatic selection for every draw with per-instance lists (remaining departure: parent switches, no effect here), the clock keys, the no-sky case and the 430 m terrain casters labelled Decisions; §14.4: ceiling-shadow, rule-3, fallback (empty or missing table, unresolved layers: not in this install), far-band hysteresis, far-band bake and mask parking as Decisions, the far-band light-map term labelled a departure (implementer OPENs folded); §14.5: the fade restart labelled a departure from the client (new BIN question (o), row W-O3: the client advances fade and smoothed distance for every object of the scene each frame, the fade is time-based), the smoothed distance confirmed as the client's rule, object shadows as the client, character shadows still departing (OPEN narrowed), new departures for unlit blend pairs and unlit fog (OPEN); §14.7 the colour retry as a Decision; §14.8 the sky-fog rule and the client's sky rotation (departures withdrawn, OPENs removed); §14.10a rows L2/L7 annotated; §14.11 annotations updated; §14.12 (a) camera raise, (b) portal by `portal` field, PVS skip rules, (c) full scene-flag pass with the landblock-order Decision, (d) crossing count, (e) avatar placeholder, (g) cells without x, y, z drawn at the origin (collision side still departs), test notes (implementer OPENs folded); §14.13 BIN source (o), rows W-E13 and W-O3, "Used in" columns updated, departures table rewritten (eleven rows withdrawn, four added). No section number or anchor changed. Status of §13–§14: Revised (Accepted text changed). |
| 2026-10-09 | Merge of main into remediation-2: main's 2026-10-06 M5 rules (§5, §6) and its implementer OPENs (§7 frame-log columns, §10 chat overlay, §13.6 first touch) kept beside the fifth revision; header status combined. No fact changed. |
| 2026-10-09 | Spec writer (dirty room; Tessera rules only, no new client fact): §7 gains the settle-wait rule (wall-clock limit on the command line, progress limit for library callers and real-data tests) and the four audio frame-log columns; both §7 implementer OPENs marked Answered. §10.8: the overlay bullet says that lines go to the command dispatcher (decision 0020); its OPEN marked Answered. §13.6: `blocked` judged at the first touch and the slope allowance restated from `m4-5-visual.md` §2.2; the implementer OPEN of 2026-10-08 marked Answered. Status of §13.6 Accepted → Revised. |
| 2026-10-09 | Spec writer (dirty room), water features (owner report of 2026-10-09): OPENs at the end of §14.5 (route placed water surfaces by `water.md` §8.5; draw second colour layers and animated texture matrices) and §14.6 (flow-map records by `water.md` §8.6). §14.5 and §14.6 Revised; review pending. |
| 2026-10-09 | Spec writer (dirty room), second water-feature revision: OPEN at the end of §14.6 (evaluate curve-valued water parameters each frame; *B* for curve-valued scales; fast rates and scale 0, `water.md` §8.7.1–§8.7.2). §14.6 stays Revised; review pending. |
| 2026-10-10 | Spec writer (dirty room), note (3) of the `water.md` re-review of 2026-10-09: both §14.6 OPENs name the lava record 0x2B000810 (draw by `water.md` §8.6.5; its curve-valued 1186 and 1187 evaluated each frame), and the flow-map OPEN points to the per-record colour of `water.md` §8.6.4 (revised the same day). §14.6 Accepted → Revised, awaiting review. |
