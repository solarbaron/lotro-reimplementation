<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The offline world walker (`apps/client`, ROADMAP M3–M4)

| | |
|---|---|
| **Status** | §1–§10: M3–M4 design, implemented. §11 (the original's distances and camera): Accepted. §13–§14: M4.5 design (walk mode on collision; world fidelity), implemented; Review (first review 2026-10-07: **changes requested**; re-review 2026-10-07: **changes requested**, not Accepted, see review record) |
| **Last updated** | 2026-10-07 (review records of §13–§14; the body last changed 2026-10-07 by the implementer: §14.4 pointer to `terrain-texturing.md` §4.9, §14.11 interiors note, answered and new OPENs) |
| **Author role** | §1–§10: Clean-room implementer of `tessera-client`. §11: Spec writer (dirty room). §13–§14: clean-room implementer (M4.5). §15: change log |
| **Reviewer** | §11: Claude review session (spec-writer agent). §13–§14: Claude review session (spec-writer agent, M4.5 criterion 9) |
| **Review record** | §11: Claude review session (spec-writer agent), 2026-10-05: changes requested (no-class LOD default contradicts `mesh.md` §8.5 without evidence; *G* step 3 condition incomplete; type 2–4 camera bodies lack offset tables; inline unknowns without OPEN callouts; provenance gaps). Re-review 2026-10-05, Claude review session (spec-writer agent): **passed** (CLEAN_ROOM.md §5.3, every item; remaining OPENs precise, each with a Tessera Decision, non-blocking). Re-review spot checks: all 11 camera-state records re-decoded with the §11.5.3 tables (types 2, 3, 4 to the last byte; body offsets 94 and 95 zero in all four type-1 records); W21 census re-derived with an independent probe over all 2,380,699 Physics SEDs (81,425 / 333,933 / 38,849 / 1,665,106 / 12,993 / 248,393; instance and own set never disagree; 2,802 distinct objects without a class); 607 of 7,625 descriptors find a class via the parent chain; no descriptor holds `Entity_Camera_ForceUnderwater`; the §11.9 `Render_FarViewDistance` buckets (384,355 Physics SEDs); BIN (same build, d22363eb9aff0fb5): the render component's initial values 200, 60, 16, 80, 1000, 1000, 1100, −1 and `Render_LODClass` in its watch list. Non-blocking notes for a later revision: the untraced correspondence of the component's last two initial values to `d6`/`d7` (§11.4 step 4) deserves its own OPEN callout; the change log lacks a row for the repair round (W20–W26). **§13–§14** (implementer design, M4.5): Claude review session (spec-writer agent), first review 2026-10-07 against CLEAN_ROOM.md §5.3 and the specs these sections implement: **changes requested**. Blocking: (1) Unqualified section references that resolve to the wrong part of this file: §13.2 "§10.6", "§11.1", "float height §11.2", "player character (§11.3)" and §13.7 "walkable (§11.3)" mean `collision.md` §10.6 and §11.1–§11.3 (here §10.6 is the avatar, §11.1 provenance, §11.3 the landscape extent); §13.6 "as §2.2" means `m4-5-visual.md` §2.2; §14.3 "§11.1 run backwards" and "default 2, §11.2" mean `environment.md`, "the limits of §10" and "§8.1", "§8.3" mean `lighting.md`; §14.4 "the Tessera Decision there" (`ChunkLayers::layer_indices_for`) is in `terrain.md` §5. Qualify every one. (2) §13.3 attributes "may lie up to 6 landblocks from the cell" and "3,856 of 104,269 cells … 4–6 landblocks away" to `collision.md` §5.8.1, which states neither. The figures are right (reviewer probe over all 104,271 landblock-info cells, Chebyshev distance of the landblock holding the cell origin from the owner: 0: 16,800; 1: 60,963; 2: 17,324; 3: 5,328; 4: 2,311; 5: 1,344; 6: 201; 4–6: 3,856), but they need a source: the Tessera real-data test that measured them, or a provenance row in `collision.md`/`landblock.md` §4.4. Say also that the distance is the cell origin's (a hull can reach further). (3) §13.4 departs from `collision.md` §12.3 step 8 and the §5.8.1 tie-break (lowest cell number among all containing cells): it keeps the current cell, then prefers portal neighbours. Mark it as a Tessera Decision that overrides those, and define "the lowest cell number of the first set" (which set is first; ties across landblocks). (4) Disagreements with Accepted specs: §14.3 applies a cell's or landblock info's `InteriorFog*` values without the `InteriorFogType` condition of `environment.md` §12 (stored values only for Linear); §14.3 takes the sky key "under the eye's landblock" only, without the cell and landblock-info overrides of the `environment.md` §7.1 Decision; §14.3 says water uses the renderer's automatic light selection, but the water permutations read no light slots, only the fake light (`water.md` §8.2, §8.4 step 6). (5) Revisions now in progress upstream contradict parts of §14 (those specs are Revised, not yet re-accepted): `terrain-texturing.md` §4.9 (a chunk without layers is drawn as one full-grid layer of the lowest-`draw_order` entry when the block's open flag is set, else not at all; affects §14.4 last bullet, §14.10a row F1 and the §14.11 OPEN, which is answered there), §4.10 (vertex normals; §14.4 builds them by `terrain.md` §5), §4.8 (ceilings get the downward ambient; the §14.11 cave-ceiling OPEN); `lighting.md` §8.3 row L30 (baked-landscape lights never reach terrain or ceiling chunks, light map or not; §14.3 lets them reach chunks without a light map) and §8.5 (selection radius, box test, key lights first, parent restrictions); `landblock.md` §4.12–§4.14 (indoor and outdoor environment records, `InteriorFogType` Invalid and None switch fog off, interior ambient, no sun indoors, cell render meshes; §14.3 and the §14.11 Y5 and interior-mesh OPENs). Add a pointer or OPEN for each, as the `terrain-texturing.md` review (item 6) also asks. Non-blocking: (6) §14.5 measures fade and draw distance to "a box around the sphere of its uploaded parts", not the world box of `mesh.md` §8.6 (row M23: union of the stored boxes of the level-0 vertex blocks), so objects draw slightly beyond *D*; label it a Decision or use the box. (7) §14.5 and §14.9 give characters *D* = min(*G*, 80 × *M_d*): 80 m is an observation of this install (§11.4.1: read at run time); say where the walker reads it, or that 80 is only a fallback. (8) §13–§14 have no provenance rows; data counts written into them should cite their Tessera test or the spec that now carries them (§14.11: 17,020 / 703 / 11,359 / 42,807, now in `terrain-texturing.md` §4.5 and §4.9; row F2: 74 objects, 22,945 records). §14.8 "at most 136 objects per description" follows from `environment.md` §5.4 (index 1–136, unique); cite it. (9) Tessera choices that answer OPENs are not labelled Decision: §14.4 ceilings cast no shadow; §14.6 one normal layer for records that declare none of 1059–1062, 1064, 1065 (`water.md` §8.2 OPEN); §14.7 a sector's box as its instance origins ± 4 m. (10) §13.8 and §14.10a are dated verification snapshots (the ROADMAP M4.5 status is later: 21 of 32 walks); mark them as such. CLEAN_ROOM §5.3: no code or pseudo-code (the Rust and WGSL names are Tessera's own clean-room identifiers), no copied prose, no game text, keys, hostnames or traffic; property and preference names are data labels used for interoperability as in the source specs; nothing under a closed gate. Spot checks that matched: §13.2 capsule, setup, height and float height against `collision.md` §10.5–§10.6; §13.1 step and cap against decision 0013 and `collision.md` §12.3; turn rate 2.5 rad/s against `keymap.md`; §14.3 day 12,240 at *G* = 0 (`environment.md` §14), frame ambient and hemispheric, *E₀* = *Cd* × *bd*, fog limits, time input (`lighting.md` §4.2 E20, §8.1, §10; `material.md` §5.3.1), shadow distance min(*G*, `d2` × *M_d* × *M_s*) and initial `d2` 16 (`mesh.md` §8.7); §14.5 *D*, *R_d*, smoothed distance for detail only and the 25 m reset (`mesh.md` §8.4–§8.7; reset confirmed in the clean code); §14.7 frill geometry (`scenery.md` §8.5 step 3), rules 2, 4, 5; §14.4 mask cap (twice the largest mask, 512, `terrain-texturing.md` §5.1); §14.6 water quads and *O* (`water.md` §8.1, §8.3, §9); §13.5 against the `water.md` §7.4 OPEN. **§13–§14 re-review** 2026-10-07, Claude review session (spec-writer agent, M4.5 criterion 9; an independent session, not the first reviewer), of the text after the implementer's 2026-10-07 additions (§14.4 pointer to `terrain-texturing.md` §4.9; §14.11: interior-cell OPEN answered, interiors note, F1 OPEN answered, late-landblock note): **changes requested**; §13–§14 stay Review. Items (1)–(4) and (6)–(10) of the first review are unchanged in the text and stand. Item (5) is met only for `terrain-texturing.md` §4.9 (pointer under §14.4) and for `landblock.md` §4.11–§4.14 (the §14.11 interiors note); pointers for `terrain-texturing.md` §4.8 and §4.10 and for `lighting.md` §8.3 (L30) and §8.5 are still missing. New, blocking: (11) §14.3 now contradicts `render.md` §4.3 and §5.2 as revised on 2026-10-07: under `Auto` the renderer leaves `Light_BakedLandscapeLight` lights out of every terrain and ceiling chunk (L30), so "chunks without one use the automatic selection, baked lights included" no longer describes what is drawn; and the `List` the walker builds for chunks with a light map (range sphere against the chunk's sphere, nearest first, no key-light order) is not the `lighting.md` §8.5 rule (ρ, squared distance to the world box, key lights first) that `Auto` now implements. Either hand every chunk to `Auto` or state the departure as a Decision. (12) Design choices inside an OPEN callout: the §14.11 note "interiors in the walker, the Tessera choices" is a list of Decisions (camera cell, portal walk, objects, lights, indoor record, `--interiors`), not a question. Move it into the body (a new subsection after §14.10a; existing numbers kept) with each choice labelled Decision, and revise the body text it supersedes: §14.3 "interior lights only with `--interiors`" (now interior lights from inside), the §14.3 sky-key and interior-fog bullet (now the two environment records of `landblock.md` §4.13, which also settles first-review item (4) for those two points), the last bullet of §14.4 (its pointer says it is to be revised), and §14.10a rows F1 and Y5 (both now render). (13) Departures labelled as silence: the note says its choices are Tessera's "where it is silent", but "characters are always drawn" from inside departs from `landblock.md` §4.12 (moving entities are not switched by the scene flags; the portal query culls them); and in (e) the renderer's cap of an overriding fog end at 10,000 departs from `lighting.md` §10 as much as from `landblock.md` §4.13 (both cap the keyframe's fog end before the override), so "the fog limits are those of `lighting.md` §10" is wrong. Label both as Decisions that depart. (Reviewer probe: all 775 cell and 540 landblock-info `InteriorFogFar` values are at most 10,000, so the cap changes nothing in this install; the 7 values in 0x78 sets were not checked.) (14) Answered OPENs not marked as answered: the cave-ceiling OPEN (`terrain-texturing.md` §4.8, §4.10: downward ambient *A₀* − *H₀*, the ceiling light map and the placed lights that are not baked; `render.md` §5.2 "Stored normal") and the L5 OPEN (`terrain-texturing.md` §9.4, T44: the original shows a faint sheen of about the gloss, mean 0.09 for layer 67, not a broad highlight; so the "fails" of row L5 in §14.10a is the expected look, to be re-judged). (15) Vertex normals: `render.md` §5.2 now shades layers without a normal map with the stored normal of `terrain-texturing.md` §4.10 (average of the triangle normals, not renormalised; ceiling triangles mirrored), but §14.4 builds the vertices by §6.1–§6.4 with `tessera-terrain`'s unit central-difference normals (`terrain.md` §5; ceiling: the terrain rule negated), so the length effect of §4.10 is not reached; state it as an OPEN or Decision (this is the §4.10 part of first-review item (5)). (16) The 2026-10-07 additions have no row in the §15 change log, and the header's "body unchanged since 2026-10-06" was no longer true (corrected by the reviewer in the header only). Non-blocking: (17) the M3-b late-landblock note is a performance observation for `tessera-world` and `tessera-physics`, not a question about this design; it can move to `docs/perf/reference-machine.md` with a pointer here. (18) The interiors note builds on `landblock.md` §4.11–§4.14, which is Revised with changes requested; keep the pointer until it is re-accepted. Spot checks that matched: the interiors census (49,636 landblock infos, 104,271 cells, 18,390 visibility cells, 39,692 portals) against `landblock.md` §4.11 and an independent cell dump of the install; (a) against the `landblock.md` §4.12 camera-cell Decision; (e) the §4.13 lookup order (cell, owner's landblock info, block map; the owner's landblock per the §4.13 implementer OPEN), the 2.5 s re-read and the B, G, R, A colour order; the §14.4 pointer against `terrain-texturing.md` §4.9 rules 1–3 (open flag, smallest `draw_order`, last met on a tie, no mask, the chunk's light map, closed blocks' layerless terrain chunks not drawn); `InteriorFogType` in the data (Linear in 386 cells and 499 landblock infos, None in 389 and 37, Invalid in 5 landblock infos), so the condition missing from §14.3 (first-review item (4)) concerns 431 property sets; the 104,269 placed cells of §13.3 agree with `collision.md` §5.8.1. CLEAN_ROOM §5.3 for the new text: no code or pseudo-code (file and function names are Tessera's own clean-room identifiers), no copied prose, no game text, keys, hostnames or traffic; DIDs, landblock coordinates and property names are functional identifiers or data labels used for interoperability; nothing under a closed gate. |
| **Implements into** | `apps/client/src/walker/` (M4: `walker/characters/` — layer, `figure.rs`, `catalog.rs`, `motion.rs` — and `walker/figures.rs`), `apps/client/src/walker_host.rs`, `apps/client/src/headless.rs`, `apps/client/src/avatar.rs`, `apps/client/src/net.rs`, `crates/net` (sandbox figures, crowd, entity events), `crates/world` (bundle figures), `fuzz/fuzz_targets/client_walker_text.rs`, `client_avatar_spec.rs`, `net_npc_events.rs`, `world_bundle.rs` |
| **Format facts from** | `coordinates.md` (§3, §8–§12), `landblock.md` (§5, §6.3, §8), `entity-descriptor.md` (§5–§9), `setup.md` (§7), `mesh.md` (§8, §11), `material.md` (§9–§12), `terrain.md`, `keymap.md`, `wstate.md` (§11); M4: `appearance.md` (§2–§7), `skeleton-anim.md` (§2, §4, §7, §8), `script-table.md` (§6, §8, §11) |

This file records **Tessera's own design decisions** for the world walker: how it streams,
draws and reports the world, and how it is driven in automated runs. §1–§10 add no format
facts; the questions they could not settle are collected in §9. §11, written in the dirty
room, states what the original client does for draw distances, detail levels, landscape
extent and the login camera, so the walker can match it at default graphics settings.

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
| W20 | An object to which no `Render_LODClass` value is delivered keeps the render component's initial values (`d0` 200, `d1` 60, `d2` 16, `d3` 80, `d4` 1,000, `d5` 1,000; the last two are 1,100 and −1, their correspondence to the record's `d6` and `d7` not traced); the property dictionary's default (class 2) is **not** applied; a class without a table entry leaves the previous values; a class value removed at run time is replaced by the dictionary default (§11.4.3) | BIN-W (g) | BIN |
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

1. **Draw distance** *D* = min(*G*, `d0` × *M_d*). The object is drawn while its smoothed
   distance (`mesh.md` §8.6, nearest point of its world box) is at most *D*; crossing *D*
   starts a fade in or out of 1.5 s (`mesh.md` §8.7).
2. **Detail range** *R_d* = `d1` × *M*; the detail level follows `mesh.md` §8.4 (the last
   level from *R_d* on).
3. **Player's own character:** `d0` … `d5` × 3 (`mesh.md` §8.5).
4. **No class reaches the object** (none of its property sets holds `Render_LODClass`,
   §11.4.3): the render component's initial values stay in force, `d0` = 200 m, `d1` = 60 m
   (`d2` 16, `d3` 80, `d4` 1,000, `d5` 1,000; the last two fields are 1,100 and −1, and
   which of them corresponds to the record's `d6` and which to `d7` was not traced). The property
   dictionary's default (class 2: `d0` 50, `d1` 10) is **not** used (W20, BIN). A class
   value without an entry in the table leaves the values the object already had (for a new
   object, these same initial values).

At the built-in defaults *M* = *M_d* = 1, so *D* = min(*G*, `d0`) and *R_d* = `d1`.

> **OPEN (implementer, 2026-10-06, `tessera-world` draw-distance module):** step 1 says the
> draw test uses the **smoothed** distance of `mesh.md` §8.6, but `mesh.md` §8.6 says the
> smoothed *d_s* serves only the detail factor and §8.7 (row M25, BIN) says the draw, fade
> and shadow tests use the **unsmoothed** *d*. Spec owner to converge the two. Tessera
> follows `mesh.md` (BIN): `DrawFade` is fed the unsmoothed nearest-point distance.

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
## 13. Walk mode on collision (M4.5; clean-room implementer)

Tessera's design (no format facts). Code: `apps/client/src/walker/movement/` (walker,
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
  lower cap 0.35, length 1.10 (the capsule every playable species ends up with, §10.6).
- Movement record: the first 0x79 property set of the game-logic archives (ascending DID)
  that holds `PlayerPhysicsControl_BasePlayerParameters` (`collision.md` §10.3); about 5 s
  in a release build. Without it, the built-in record of `tessera-physics`.
- Conditions (§11.1, float height §11.2): the properties of the avatar's 0x47 entity chain
  (`setup.md` §3.1), the nearest level winning; the avatar is a player character (§11.3).
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
  landblock, which may lie up to 6 landblocks from the cell (`collision.md` §5.8.1; on
  the owner's install 3,856 of 104,269 cells of regions 1–5 and 14 lie 4–6 landblocks
  away). A step therefore also needs the owner of the mover's current cell, and a
  scripted walk requires its own landblock (the owner of its cells) at every step. The
  walk streamer keeps these landblocks resident wherever they lie (one small streamer per
  pinned landblock) and every view of a step holds them.

> **OPEN (implementer, 2026-10-06 review):** the 3 × 3 rule does not cover cells placed
> far from their owner before the mover has entered them: the walker cannot name an
> owner that is not resident, so whether such a cell's floor is in a step's view still
> depends on residency (the walk rings keep owners up to about 3 landblocks away; the
> interactive streamer about 10). Fix: a world-side index of the cell owners whose cell
> bounds reach each landblock, built from landblock info at boot (`tessera-world`), so
> that `needed` can name them.
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
CELL rule): it keeps the current cell while that still contains the point (in a doorway
both cells do), else takes a cell that the current cell's portals lead to and that contains
it (`collision.md` §5.5, §5.8), else the lookup of §5.8.1 over the view (the lowest cell
number of the first set; outdoors when none).

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
- Pass conditions as §2.2, with these readings: penetration is the deepest negative
  separation of the capsule's contacts after each step, settle steps included; so are
  `follow`'s and `pass`'s per-step terrain and object-contact checks ("at every step");
  `follow`'s end condition is horizontal only, as §2.2 gives it; "start"
  cell is the cell after the settle (the last settle step); `follow`'s "never blocked" is: no step with forward
  held, at least 0.5 s into the inputs and more than 1 m from the end point, whose
  horizontal speed is below half the layer's forward speed; `pass`'s "object contact" is
  a static or generated body within 0.01 m. A failure within twice its tolerance is
  reported as a near miss (§2.2: for the spec writer). `blocked` walks also report the
  first stop (where the walker first came to rest with forward held).
- Movement log (CSV, one row per step): `walk, step, t, x, y, z` (feet, frame of the
  walk's landblock), `vx, vy, vz, heading_deg, state` (`collision.md` §10.4), `support`,
  `cell` (`0xXXYY:N` or 0), `water_depth, forward, strafe, jump, walking, stepped_up,
  jumped, snapped, planes, pushed_out, penetration, boundary, terrain` (terrain height
  under the feet), `contacts` (up to 8, `kind@separation`).

<a id="s13-7"></a>
### 13.7 The soak (`m4-5-visual.md` §2.5)

`apps/client/tests/walk_soak_real_data.rs`. Readings (OPEN in §2.5): the draw order is
landblock, x, y, heading, the redraws of step 4 (x, y), then per segment its length, kind
and heading change; a landblock with no valid start within 256 (x, y) draws is passed
over and the next walk planned. Step 4 sweeps the capsule 4 m down from 3 m above the
terrain; the support must be walkable (§11.3) and not a cell, the capsule must not overlap
geometry at the top or on the support. A start with no walkable support is redrawn too.
Step 6's cell floor: a cell physics mesh crossing the capsule's axis more than 0.05 m above
the feet. Walks are planned in order on one thread (one streamer) and run on several,
each with its own streamer (rings 1.5 landblocks) over the shared workers; a digest of the
end positions compares runs.

<a id="s13-8"></a>
### 13.8 Results (2026-10-06, owner's install, release build)

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
## 14. World fidelity in the walker (M4.5; clean-room implementer)

Tessera's design for drawing the M4.5 world systems (no format facts; every rule cited is
the spec's). Code: `apps/client/src/walker/` — `terrain.rs` (texture layers, ceilings,
far band), `farband.rs` (the far band's bake), `objects.rs` (LOD classes, fades, detail
levels, lit surfaces, instancing), `env.rs` (clock, sky, sun, fog, lights), `water.rs`,
`frills.rs`, `resolver.rs` (background lookups), `fxparse.rs` (switches, camera loops,
clock arithmetic), `geometry.rs` (`MaterialPlan::lit`); `walker/mod.rs` wires them.
Every M4.5 system draws into **one lit world pass** (`docs/specs/render.md` §4–§5).

<a id="s14-1"></a>
### 14.1 Frame

Per frame, after the controllers and the streamer (§2): the frill ring follows the eye and
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

- **Clock.** The epoch is OPEN (`environment.md` §6), so the walker starts the game clock
  at day *n* = 12,240 (the day the clock reads at *G* = 0, `environment.md` §14) and time
  of day 0.5 (the checklist's performance time) unless `--day N` and `--time-of-day T` set
  them (*G* from *n* and *t* by §11.1 run backwards); `--time-rate R` (default 1) and
  `--day-type D` (default 2, §11.2). `--day` takes ±20,000,000 at most (days whose *G*
  the game clock holds, ±10¹² s, with any calendar of days up to 50,000 s); a day the
  clock still cannot hold starts the clock on day 12,240 with a warning. Keys: `[` and
  `]` move the time by 1/24 day, `\` stops and restarts the clock (a clock stopped at
  rate 0 restarts at rate 1), `-` and `=` halve and double the rate, never past 1/64 or
  the `--time-rate` maximum of 100,000 and never in the wrong direction; while the clock
  is stopped they change the rate it restarts at.
- **Sky key** under the eye's landblock (`environment.md` §7.1); **interior fog** of the
  cell holding the eye: the cell's own `InteriorFog*` properties, else its landblock
  info's (`environment.md` §7.2, §12; `m4-5-visual.md` Y5).
- **Frame lighting** from the blended values (`lighting.md` §8.1, Tessera Decision there):
  ambient ½ (*X* + *Y*), hemispheric ½ (*X* − *Y*), sun toward −travel with colour *Cd* ×
  *bd* and specular *Cs*, distance fog near/far/colour after the limits of §10, the time
  input = seconds since start (`material.md` §5.3.1). Without a sky library or with `sky`
  off, the defaults of §8.1 and no fog.
- **Sun shadow** (`lighting.md` §9 simple form): one map of 2,048 texels over a sphere of
  radius 90 m centred 54 m ahead of the eye (horizontal view direction), fading out by a
  view depth of 150 m, cast only while the sun's elevation exceeds asin 0.05. Terrain
  chunks within 430 m cast; objects within their shadow distance min(*G*, `d2` × *M_d* ×
  *M_s*) (`mesh.md` §8.7); characters within the render component's initial `d2` (16 m).
- **Clear colour** = the fog colour (the background meets the fog at the horizon); the sky
  objects draw over it (§14.8).
- **Global limit** *G* = `global_limit(E, fog far)` (§11.3, Tessera Decision there: clipped
  whenever fog is on).
- **Light table:** the Light entities within 200 m of the eye, nearest first, at most 255
  (interior lights only with `--interiors`); range and on/off by the day/night rule of
  `lighting.md` §8.3, intensity curves evaluated on the material clock, *σ* = 1. Objects
  and water use the renderer's automatic selection (`render.md` §4.3, the Tessera Decision
  of `lighting.md` §8.5); terrain and ceiling chunks with a light map (one of their
  layers has one) receive only the lights that are not `Light_BakedLandscapeLight` (§8.3
  Decision), nearest first; chunks without one use the automatic selection, baked lights
  included.

<a id="s14-4"></a>
### 14.4 Terrain

- **Layers.** The block job resolves the terrain and ceiling chunks against the region's
  0x19 table (`terrain-texturing.md` §4; the table the scenery tables loaded, else read
  from the region descriptor), checks each mask (A8, §5.1: a rejected mask is dropped and
  the layer drawn as if it had none; masks above 1,024 texels a side, twice the largest
  in the data, are rejected too) and decodes the masks and light maps (§10). Masks wait
  for their upload as their A8 texels and are expanded to (255, 255, 255, *a*) only when
  uploaded (a far block keeps its masks on the CPU until it nears). A mesh
  job builds the chunk's vertices (§6.1–§6.4, the per-block offset of §6.2 with Tessera's
  fixed region coordinates) and every layer's triangles at the block's level and stitched
  edges (`ChunkLayers::layer_indices_for`, the Tessera Decision there); the chunk is
  uploaded when its shared layer textures (diffuse, normal, emissive, glow, through the
  object layer's store) have arrived. A block that draws nothing yet is uploaded at once
  with what is there and rebuilt when the rest arrives. The crack-free promotion of §3 is
  unchanged. Blocks are recorded nearest first (early depth rejection).
- **Ceilings** (`terrain-texturing.md` §4.8): level 0 from the stored triangles, with the
  ceiling light map, no shadow casting.
- **Scrolling layers** (§6.3): the offset curves of the layers that have them are
  evaluated every frame on the material clock and handed to the renderer.
- **Far band (Decision; the original's distance bands are OPEN, `terrain-texturing.md`
  §12).** A block farther than 480 m (`--far-band M`, 0 = off; 40 m hysteresis) is drawn
  with one opaque layer: its layer stack composited on the CPU, once per block, into a
  256 × 256 image with a mip chain (`farband.rs`): each layer's albedo × `diffuse_scale`,
  sampled from the texture's small CPU levels (≤ 64 texels, the level matching the texel
  footprint), blended "over" by its opacity rule after the discard test (§7–§8); missing
  masks and textures follow the renderer's rules. The far layer is lit like the others
  (2 × *C′* × Σ, the light map still added); per-layer normal maps and specular
  highlights are not kept. Reason: one draw per far block instead of one per layer (the
  walker drew 1,800 layer draws in a forest view; the far band leaves about 200), which
  the frame budget of criterion 5 needs; at the band's start a baked texel (0.625 m) is
  about 1.7 pixels at 1080p. Bakes run ahead of need (320 m inside the band) on the
  terrain workers. Visually the same view with and without the band matched on
  2026-10-06 (default spawn, 60 m above the ground). A block in the band keeps only its
  light map on the GPU; its masks are uploaded when it comes within the band's edge (a
  chunk waits only for its own masks and light maps, so the ceiling of a far cave block
  completes while the terrain masks wait). A
  block that enters the band with no full-layer chunk drawn and no bake yet (a fast
  flight) is drawn height-shaded until the bake arrives.
- Without layers (switched off, no table, a chunk that does not resolve, or a terrain
  chunk that resolves to no drawn layer) the M3 height-and-slope mesh is drawn, lit with
  its vertex colours. What the original draws for a terrain chunk without layers is OPEN
  (`terrain-texturing.md` §4.5); a ceiling is drawn with its layers either way.

> **OPEN (implementer, 2026-10-07; pointer):** the last bullet is superseded in the code
> by `terrain-texturing.md` §4.9 (Revised, not yet re-accepted): a terrain or ceiling chunk
> that stores no layers is resolved, when its block is open, as one opaque `whole` layer of
> the table's lowest-`draw_order` entry (last met in stored order on a tie) without a mask,
> with the surface's light map, and drawn through the layer pipeline like any other; when
> the block is not open its terrain chunk is not drawn at all (no height-shaded mesh, no
> shadow) and its ceiling chunk has no layer. The height-shaded mesh remains for layers
> switched off, no table, and chunks whose stored layers all fail to resolve.
> `apps/client/src/walker/terrain.rs` (`block_open`, `layerless_key`, `resolve_surface`).
> This section's text is to be revised with the §13–§14 review.

<a id="s14-5"></a>
### 14.5 Objects (§11.4, `mesh.md` §8.4–§8.7)

- The block job turns the bundle's drawable entities and generated objects into objects
  (one per placement, with its part instances) and gives each its LOD-class values: the
  class of the placement in the order of §11.4.3 (`LandblockBundle::lod_class_of`), the
  table entry or the component's initial values (§11.4 step 4).
- **Draw distance** *D* = min(*G*, `d0` × *M_d*), capped by the object distance cap
  (`--object-distance`, default the landscape extent; it shrinks while the GPU budget is
  exceeded, §4). An object's parts are requested once it is within *D* + 60 m (nearer
  than 200 m at visible priority, else at prefetch priority: parts never outrank the near
  ring's landblocks, which load at immediate priority) and released beyond *D* + 140 m;
  the activation pass of a landblock reruns after 12 m of camera travel, a change of *G*
  of more than 1 m, a change of the distance cap, or of the `lod`, `scenery` or
  `instancing` switch or the distance settings (so a cap that shrinks with the camera
  still releases parts, and F9 and F11 act at once). Generated objects are not drawn
  while `scenery` is off; switching `instancing` on asks for the models of the parts in
  use.
- **Fade** (`mesh.md` §8.7): `DrawFade` on the unsmoothed distance to the object's box (a
  box around the sphere of its uploaded parts); a block not drawn in the previous frame
  resets its objects' fades. A fading surface blends by its output alpha × the opacity
  with depth write (`AlphaMode::TestBlend`, `render.md` §4.3; Decision: the original's
  fade blending is not specified) and casts no shadow.
- **Detail levels** (`mesh.md` §8.4): every kind-1 level of a part is uploaded; the level
  follows the smoothed distance (§8.6) and *R_d* = `d1` × *M*.
- **Surfaces** go through the lit pipeline with the inputs of `lighting.md` §4.1
  (`MaterialPlan::lit`): material colour 2000 as stored, the alpha mode of `material.md`
  §11.3 (lit `ColorAndOpacity` blends with depth write, unlit blend records without,
  additive records additively), normal map (3011), specular colour and power, gloss from
  the base or normal alpha by role (§10.2; an opaque `ColorAndGloss` texture keeps its
  alpha), emissive (3018) and glow mask (3013) with glow colour 2014, fog, shadow casting.
  Characters use the same rules (with their dye in stored values). Texture transforms and
  second colour layers are not drawn (the lit pipeline has none yet, `render.md` §4.1).
- **Instancing** (`render.md` §5.5): fully faded-in objects whose surfaces need no
  emissive, glow or dye draw as instanced batches, one per landblock, part, detail level
  and shadow flag; the part's world models are built once its textures are on the GPU.
  Fading objects and the other surfaces stay single mesh draws.
- **Characters** (§11.4, §11.4.2): drawn to *D* = min(*G*, 80 × *M_d*) (the `d0` of
  `PlayerAvatar`, `Creature` and `Creature2Deg`; × 3 for the player's own character),
  with the 1.5 s `DrawFade` across *D* (blended like a fading object, no shadow while
  fading); `--character-distance` (default 400 m) only caps *D*. Placed figures beyond
  *D* are handed to the object layer (§10.2). Each character's own class is not resolved
  yet (OPEN §14.11). The player's own character is hidden in free-fly mode (the
  checklist's camera, `m4-5-visual.md` §4.0).

<a id="s14-6"></a>
### 14.6 Water (`water.md` §8–§9)

Each resident wet landblock within the landscape extent is one 160 × 160 quad at *h*
(§8.1 Tessera rule); quads are grouped by surface into one draw with the surface's water
parameters (§8.2, resolved on the resolver thread at rest; one normal layer when the
record declares none of 1059–1062, 1064, 1065) and normal map. *O* is the anchor's origin,
so the normal layers use region coordinates (§8.3 Tessera rule). Surfaces with vertex
waves are subdivided 16 × 16 per quad. Distant water is the landblock water of the
resident landblocks out to the extent (§9 Tessera rule: derived from the block maps).
Placed water meshes (§8.5) are ordinary objects.

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
original's groups are (§8.3 rules 2 and 4, §8.5 step 4): a sector whose box (its
instance origins ± 4 m) lies at *D* or beyond draws nothing, and the reduction count is
taken from each sector's own distance, keeping a prefix of that sector's instances; the
walker hands the renderer the reduced list (rebuilt only when a count changes). Trade-off
of the merge: the fade-in runs from the time the cell first appeared, not each sector.
**Terrain colour** (§8.3 rule 7, formula OPEN; Decision): the mean colour of the smallest
level of the top terrain layer's diffuse texture at the sector's centre × its
`diffuse_scale`; while unknown the sector's terrain-colour intensity is 0. Cells out of
view or beyond *D* are not queued.

<a id="s14-8"></a>
### 14.8 Sky (`environment.md` §11.4, §12)

The visible sky objects of the day description in use, each part of its 0x47 object as a
`SkyObject` with the object's angles and keyframe tint (`render.md` §5.4 placement), in
the stored order; objects whose models are not built yet are left out. The parts stay
referenced for the walker's life (the data has at most 136 objects per description).

<a id="s14-9"></a>
### 14.9 Default-settings profile (§11.8)

| Quantity | The walker now |
|---|---|
| Landscape extent | the far ring is *E* of `--landscape-distance` (High: 2,080 m) unless `--view-blocks` sets it; near and mid rings 3 and 6 landblocks |
| Residency | default budget 4,096 MiB: about 600 landblocks are resident, and with their scenery and collision sets they hold 0.85 MiB (region 1, Kingsfell) to 4.6 MiB (region 5) each, 2.7 GiB in all at the densest spot seen |
| Object distances | per class (§14.5), `--object-draw-distance` and `--model-detail` levels (§11.2) |
| Fade, detail levels | §14.5 |
| Other characters | min(*G*, 80 × *M_d*) with the 1.5 s fade (§14.5); `--character-distance` caps it |
| Frills | level 2 |
| Field of view, near plane | 45° vertical, 0.1 m |
| Camera state, login placement, zoom range, look-at point, target fade | not adopted: the walker keeps its own third-person camera (§5); OPEN below |

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
- `apps/client/tests/visual_rows_real_data.rs` checks the expected facts of the visual
  rows (`m4-5-visual.md` §4) on the world model the walker draws from.

<a id="s14-10a"></a>
### 14.10a Checklist rows rendered (criterion 6, Tessera side, 2026-10-06)

Every row of `m4-5-visual.md` §4 was rendered headless at its eye, heading, pitch and time
(1600 × 900, free-fly, no HUD, avatar hidden; private images, not committed) and checked
against the expected facts by `visual_rows_real_data.rs` (all 6 tests pass with
`TESSERA_GAME_DIR` set).

| Rows | Facts (test) | Image | Notes |
|---|---|---|---|
| T1–T3, T6–T11, T13, T14 | pass (every layer stack) | pass | the expected surfaces: cobbles, snow, cliff top, city paving, brick and paved roads, light-mapped tiled floors |
| T4, T12 | pass | target hidden | a placed object (T4) and a generated rock (T12, gone with `--disable scenery`) stand between the eye and the target; the terrain under them draws |
| T5, T15, T16 | pass | too dark to judge | T15 and T16 (cave ceilings) draw their layers, visible with `--disable sky`; under the sky's lighting the downward-facing ceilings are almost black (OPEN below) |
| S2, S3, S4, S7, S9, S11 | pass (every listed object) | pass | |
| S1, S5, S6, S8, S10 | not asserted (§14.11 OPEN) | the objects the walker generates draw | |
| F1 | pass (exact counts) | fails (review, 2026-10-06) | the terrain chunks of the whole 13 × 13-block square around F1 (region 1, 0x4C66) resolve to no layer: they drew nothing, so the grass floated over the clear colour; they are now drawn height-shaded (§14.4; re-rendered 2026-10-06: ground and frills drawn) until the original's behaviour is known (OPEN below) |
| F2–F6 | pass (exact counts) | pass, with a defect | frills draw instanced; in F2 a few groups draw near-white (OPEN below) |
| W1–W9 | pass | pass | sea, lakes, river step, lava, placed water (W8) and distant water to the extent (W9) |
| Y1, Y3, Y4 | pass | pass | noon clouds, evening, night sky with stars and lamps |
| Y2 | pass | not judged | the eye faces a building wall at dawn |
| Y5 | pass (interior fog values) | fails | interior cell meshes are not drawn (§14.11) |
| L1, L3, L4, L6 | L2 checked by the test | pass | sun shadow of the barn, bonfire emissive, glowing crystals, blended cobweb |
| L2, L7 | L2 pass | not conclusive | the night scenes are lit, but pools of point light (L2) and the warm light-map texels (L7) are not distinct |
| L5 | – | fails | no broad specular highlight on layer 67's paving toward the sun (OPEN below) |

<a id="s14-11"></a>
### 14.11 Open questions

> **OPEN (implementer, 2026-10-06):** the original's far landscape (`terrain-texturing.md`
> §12, `DistantImposters`): the far band of §14.4 is Tessera's own; per-layer normal maps
> and specular highlights are not drawn beyond 480 m, and the seam between the bands is
> not blended.

> **OPEN (implementer, 2026-10-06):** how the original blends an object fading across its
> draw distance (`mesh.md` §8.7 gives the 1.5 s opacity ramp, not the blend): Tessera
> blends by alpha with depth write and casts no shadow while fading.

> **Answered (implementer, 2026-10-07, `landblock.md` §4.11–§4.14, Revised):** interior
> cells are drawn; Y5 renders the IN1 cave. The original note: the render meshes of
> interior cells (0x01) are not drawn; interiors show their interior entities
> (`--interiors`) only.

> **OPEN (implementer, 2026-10-07): interiors in the walker, the Tessera choices.** Code:
> `apps/client/src/walker/interiors.rs` (visibility cells, portals, PVS, scene flags,
> portal walk), `objects.rs` (cell meshes as parts, `ObjectInfo::cell`), `terrain.rs`
> (`block_objects`), `env.rs` (indoor record), `walker/mod.rs`. The spec rules followed
> are `landblock.md` §4.11–§4.14; the choices below are Tessera's where it is silent:
> (a) **camera cell**: the §4.12 Tessera Decision (cell lookup at the unclamped eye, else
> the walking character's cell); inside a cell the eye is not kept above the terrain;
> (b) **portal walk**: the §4.11 Decision with screen rectangles in normalised device
> coordinates; a portal polygon is clipped at *w* ≥ 10⁻⁴ before its bounds are taken; an
> eye within 0.5 m of a portal's plane and bounds passes the view on unchanged (an
> edge-on doorway the eye stands in); a portal without a polygon passes it unchanged; at
> most 4,096 portals per frame; (c) **objects**: a cell's render mesh is a part whose DID
> is the cell's 0x01 geometry, placed by the cell's position and rotation (no adjustable
> scale, §4.14 Decision), drawn out to *G* and the distance cap at its finest level (the
> §4.14 OPEN on cell detail levels); an interior entity (`entity-descriptor.md` §4.8) is
> attached to the cell of its landblock's collision set whose volume contains its origin
> (tried at the origin and ±0.05 m), and one that no cell contains is drawn whenever the
> camera is in a cell of its landblock; interior objects are loaded only while the camera
> is in a cell of their landblock; from inside no outdoor object, terrain, ceiling,
> landscape water or frill is drawn, the sky only when sky-visible; characters are always
> drawn; (d) **lights**: from inside only interior Light entities enter the light table,
> from outside only outdoor ones (interior ones too with `--interiors`); (e) **indoor
> record**: the second environment is blended every frame from key B; the interior
> overrides are read at the camera's cell from the cell's set, the owner landblock's info
> set and its block map, re-read when the owner landblock changes or after 2.5 s; colours
> by `lighting.md` §4.4's B, G, R, A rule; the fog limits are those of `lighting.md` §10
> (the renderer's), which equal §4.13's except that §4.13 does not cap an overriding fog
> end at 10,000; (f) `--interiors` now loads and draws every interior cell and entity
> whatever the camera's cell (a debugging view). Checked on the install
> (`apps/client/tests/interiors_real_data.rs`): 49,636 landblock infos, 104,271 cells,
> 18,390 visibility cells, 39,692 portals (the §4.11 census), LI7 and LI8 on every record,
> the IN1 example of §4.11–§4.12 (26 portals, all with polygons), and the Y5 eye in cell 9.

> **OPEN (implementer, 2026-10-06):** the camera rows of §11.8 (camera-state record, login
> placement, zoom range, look-at point, target fade) are not adopted yet.

> **OPEN (implementer, 2026-10-06):** characters' draw distance uses `d0` = 80 m (the
> three character classes agree, §11.4.2) and their shadow distance the render
> component's initial `d2` (16 m), instead of each character entity's own LOD class.

> **OPEN (implementer, 2026-10-06, for the spec writer and the scenery implementer):** the
> walker's generated lists (`tessera-world` scenery) do not hold six of the 22 objects the
> scenery rows of `m4-5-visual.md` §4.2 list: S1 both (0x470035F7 at (127.035, 66.551),
> 0x470035F4 at (128.162, 70.238)), S5 0x4700720F at (60.130, 78.403), S6 0x47005A5F at
> (72.770, 21.815), S8 0x4700720F at (120.468, 106.733), S10 0x47007AA2 at (59.987,
> 43.137) (0.70 m from a generated 0x47007CB0, so possibly a spacing or clearance
> difference). The other 16 objects match to 0.01 m, 0.1° and 0.0001 in scale.
> `apps/client/tests/visual_rows_real_data.rs` reports these rows without failing.

> **OPEN (implementer, 2026-10-06):** cave ceilings (T15, T16) are almost black under the
> sky's frame lighting: a face pointing down receives ambient − hemispheric = the ground
> term *Y* (`lighting.md` §8.1) plus its light map. Whether the original adds more to
> ceilings (interior lights, a different ambient under a ceiling) is not specified.

> **OPEN (implementer, 2026-10-06):** row L5 shows no broad highlight on terrain layer 67
> toward the sun; the layer's specular switch, gloss source and the sun's specular colour
> need checking against the original (owner item for `m4-5-visual.md` L5).

> **Answered (M4.5 exit verification, 2026-10-06):** the near-white groups were the
> sun's specular with gloss 1: instanced surfaces could not take the gloss from a
> normal-and-gloss texture's alpha (`GlossFrom::NormalAlpha`), so `WorldSurface` gained
> `gloss_from_normal_alpha` and F2 now draws its heather pink and its flowers pale, with no
> near-white pixels. The original note follows.
>
> **OPEN (implementer, 2026-10-06; narrowed in review):** in row F2 a few frill groups
> draw near-white (overbright), as bright at *t* = 0.0 (night) as at 0.5, so the shading
> saturates whatever the light. They are frills (gone with `--disable frills`, kept with
> `--disable scenery`), and every frill object of block 3/0x7A69 (74 objects, 22,945
> records) is on lit surfaces, mostly alpha-blended `ColorAndOpacity` with a
> `TSNormalAndGloss` normal map and specular power 0: the unlit flag, specular and the
> terrain-colour intensity are ruled out. What is left is the renderer's instanced lit
> path for normal-mapped frill models (their tangent frame, or the ambient or sun terms).
> Next step: draw F2 with the normal map ignored for frill batches; if the clumps go,
> check the tangents of the frill models and the normal-frame fallbacks of the instanced
> shader (`tessera-render`). Cells merging sectors of different terrain-colour intensity,
> a candidate cause, no longer happen (§14.7).

> **Answered (implementer, 2026-10-07):** by `terrain-texturing.md` §4.9 (Revised); the
> walker follows it (pointer under §14.4). Row F1 renders its 13 × 13 blocks with key 0 of
> 0x19000000 under the frills. The original note follows.
>
> **OPEN (implementer, 2026-10-06, `terrain-texturing.md` §4.5):** what the original
> draws for a terrain chunk that has no layers. In region 1 alone 17,020 landblock-data
> records have none while their heights vary (703 of them with a landblock info), and
> the 13 × 13 blocks around row F1 are among them; region 2 has 11,359 and region 5
> 42,807 (reviewer's probe). The walker draws them height-shaded (§14.4); `m4-5-visual.md`
> row F1 needs the owner's look at the original there.

> **OPEN (implementer, 2026-10-06):** the walker's keys of §14.2–§14.3 are read as raw
> key codes, outside `tessera-input`'s actions and contexts: a keymap binding on one of
> them wins, but they cannot be rebound and a future text-entry context (chat, M5) would
> not make them opaque. Registering them as actions needs the input registry and the
> walker's controls (`controls.rs`).

> **OPEN (implementer, 2026-10-06, for the world implementer):** flight M3-b (3,000 m/s)
> shows one near-ring landblock missing for 13–21 frames near the end of the path in every
> run, also with every M4.5 system off and with the M3 ring size: the bundle (with its
> collision set, generated scenery and water) arrives late. Criterion 5's "no near-ring
> holes" is not met there (`docs/perf/reference-machine.md`).

> **OPEN (implementer, 2026-10-07, for the physics / collision implementer):** the late
> landblock of M3-b is r2 (239, 242) (once also its neighbours under heavy machine load).
> Its bundle job takes 0.53–0.75 s on one worker on a loaded machine and up to about 2 s
> under a load average of 40, almost all of it the collision set: about 160 ms placing the
> static bodies and about 570 ms building the set's hierarchy, for a bundle of about
> 104 MiB (most bundles take under 50 ms). The streamer now loads along the focus's path
> (`tessera-world` streamer "Lookahead", ARCHITECTURE §9.2: 1.5 s ahead by default), which
> removed every other late landblock of the flight in the indicative runs; whether that
> also covers r2 (239, 242) on a quiet reference machine is not yet measured. A cheaper
> collision build for such landblocks (shared per-object hierarchies, or building the
> hierarchy on several workers) would remove the remaining risk; the build order and the
> set's contents stay as `collision.md` §12.1 gives them.

> **OPEN (implementer, 2026-10-07, streaming):** the streamer's lookahead now has an early
> part (`tessera-world` streamer "Lookahead", ARCHITECTURE §9.2): the path out to 4 s is
> requested at `Prefetch` priority, the first 1.5 s (or 1.5 times the slowest recent job)
> at `Visible`. In streamer-only M3-b flights on a loaded machine (indicative,
> `docs/perf/reference-machine.md`) r2 (239, 242) was never missing in 11 runs, against
> 14–26 frames in 6 of 11 runs without the early part. Still open: the walker's own M3-b
> flight on the quiet reference machine, which criterion 5 counts.

---

<a id="s12"></a>
<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-04 | §1–§9: M3 design (clean-room implementer). |
| 2026-10-05 | §8.1, §10: M4 characters (clean-room implementer). |
| 2026-10-05 | Spec writer (dirty room): new §11 (the original's preference levels and built-in defaults, quality presets, landscape extent and the global distance limit, per-class object and character draw distances with fade and detail ranges, camera-state record layout and census, camera-state selection, third-person login placement and behaviour, default-settings profile for the walker, OPENs, corrections to other specs, invariants, worked examples); §9 draw-distance OPEN marked answered; header gains the §11 status and reviewer rows. §11 status: Review. |
| 2026-10-05 | Spec writer (dirty room), answering the review of §11 (changes requested): §11.1 BIN-W gains a second M4 session with questions (g)–(i) and OBS-W the placement-level census; W1 extended (FrillDensity, PlayerCrowdQuality, DialBackGraphics), W5 cites (h), new rows W20–W26. §11.2: case-insensitive reading marked Tessera Decision. §11.3 step 3: the missing flag condition (*S* off, or *F* set and *V* > 0.99), where *F* and *V* come from, OPEN extended. §11.4 step 4 and §11.4.1: the no-class values now rest on BIN (render-component initial values, not the dictionary default); new §11.4.3 (how the class reaches the render component; census over all Physics SEDs; OPEN on SED versus 0x47 precedence with a Tessera Decision). §11.4.2: NPC class statement reworded with the census. §11.5.3: byte-layout tables for the type-2, type-3 and type-4 bodies. §11.7 steps 1–2: look-at height fallback, under-water switch *U* and the yaw rule with flags *R*, *X*, *P*, *Q* (BIN), three new OPEN callouts. §11.8 rows updated. §11.9: OPEN list extended; `Render_FarViewDistance` census corrected (384,845 SEDs, not 8). §11.10: corrections to `mesh.md` §8.5/§8.8 and `scenery.md` §9. §11.11: W-I7–W-I9. §11.12: fog-clip, class-resolution and yaw examples. §11 status stays Review. |
| 2026-10-06 | New §14: world fidelity in the walker (M4.5; clean-room implementer): the lit world pass with texture layers, ceilings and the far band, objects by LOD class with fades, detail levels, lit surfaces and instancing, water, frills, sky, the clock and its controls, the switches, the default-settings profile as adopted, new frame-log columns and options, the checklist rows as rendered (§14.10a; performance in `docs/perf/reference-machine.md`), and the terrain job ranking, upload-budget start and far-band mask deferral found by the M3 flights. |
| 2026-10-06 | New §13: walk mode on collision (M4.5; clean-room implementer): the physics walker on its fixed 1/60 s step, the mover, collision-set residency (the feet's 3 × 3 landblocks; absent landblocks as a solid boundary), cell tracking through portals, water masked inside cells, scripted walks (`--walk-path`, `--walk-log`) and the 1,000-walk soak, with their first results. Supersedes §2's "controllers (30 Hz fixed step)" and §5's terrain-height walk mode when collision is built. |
| 2026-10-06 | Review round of §14 (clean-room implementer): the change log moves to §15 (anchor `s12` kept); §14.2 walker keys yield to keymap bindings; §14.3 pause and rate keys, `--day` range, baked lights on ceilings without a light map; §14.4 chunks without layers drawn height-shaded, masks kept A8 until upload and capped at 1,024, a chunk waits only for its own images (a far cave block's ceiling never completed, so the walker never settled near one); §14.5 part priorities (visible/prefetch), the activation pass reruns on a cap, switch or setting change, characters by class distance with a fade; §14.7 frills: anchor-free sector groups, nearest-first builds, cells merged per frill values, distance rules per sector, the first part's first draw entry; §14.9 characters row; §14.10a F1 no longer passes (OPEN); §14.11 OPENs updated; §13.9 avatar OPEN answered. |
