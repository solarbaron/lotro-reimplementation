<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0013 — Collision and movement in a new `tessera-physics` crate; controller on a fixed 1/60 s step

**Status:** Accepted, 2026-10-06 (M4.5 crate placement, `docs/ROADMAP.md` M4.5)

## Context
M4.5 (world fidelity) replaces walking on terrain heights with walking on collision
geometry: terrain and ceiling collision triangles, cells and portals, the collision shapes of
placed and generated objects, capsule sweeps and a character controller with walking,
sliding, step-up, jumping, wading and swimming (`collision.md` §5–§12, `water.md` §7.4).

That code could live in `tessera-world`, which already holds landblock streaming. But `world`
depends on the asset server, animation (`havok`), text and the scripted-object graph, and is
tied to streaming. A controller inside it could not be unit-tested or fuzzed alone, and the
project-owned test server of M7, which needs server-side movement, would have to pull in all
of `world` even to use the controller. (To move exactly like the client on install data it
also needs the same collision sets; see "Reuse at M7".)

On the other side, two parts of `world` need collision. Scenery generation reads the terrain
collision mesh and its radius and plane queries (`scenery.md` §7.4, `collision.md` §6.6), and
landblock bundles carry collision sets built on the asset workers (`collision.md` §12.1).

There was also a conflict of rates. ARCHITECTURE §20.3 named one simulation tick of 30 Hz,
interpolated for rendering, for "local movement and collision". The controller of
`collision.md` §12.3 is specified with a fixed step of 1/60 s and at most 5 steps per frame.

## Decision
- **New crate.** Collision and movement go into a new L3 crate, **`tessera-physics`
  (`crates/physics`)**, not into `tessera-world`. It holds terrain and ceiling collision
  triangles with the client's numbering, the terrain radius and plane queries, cell volumes
  and the point-in-cell lookup, per-landblock collision sets of statics, generated objects,
  cells and water with Tessera's own bounding-volume hierarchy, capsule sweeps, and the
  character controller with its water states.
- **Dependencies.** `tessera-physics` depends only on `tessera-formats` and `tessera-props`
  (L1–L2). It has no GPU, asset-server or streaming dependency, and no clock of its own.
- **Positions.** `physics` defines no landblock key or coordinate type of its own. A
  collision set is keyed by `tessera_formats::LandblockId` and holds its geometry in that
  landblock's frame (f32, `coordinates.md` §4–§5); the mover's position is an f64
  region-frame position (x = bx · 160 + local x, the world position of ARCHITECTURE §9.1)
  and `physics` converts between the two with that rule. `tessera-world` converts its
  `LandblockKey` and `WorldPos` to and from these at the boundary.
- **Water.** A collision set also carries its landblock's water plane (level and type of
  `water.md` §6.2, or none) and its liquid boxes (the monitor boxes of the landblock volume
  entity and of the placed volume entities, `water.md` §7.1–§7.3, placed as oriented boxes
  in the landblock frame with their §7.2 flags). `world` fills them when it builds the set on
  the workers. The controller's water step (`collision.md` §12.3 step 7) reads only the
  resident sets; no water query goes back to `world` (ARCHITECTURE §10.3).
- **Sibling allowance.** `tessera-world` may depend on `tessera-physics` (ARCHITECTURE §3).
  The allowance is one-way; `physics` never depends on `world`, `terrain` or any other L3 or
  L4 crate.
- **Decoders stay in `tessera-formats`.** The quantised-tree reader and the 0x66 and 0x01
  decoders are record decoders and live in `formats` like every other one. `physics` builds
  on their output.
- **Other M4.5 systems stay where their specs put them.** Scenery, frills, water state and
  the game clock are modules of `tessera-world`; terrain layers are in `tessera-terrain`;
  pipelines are in `tessera-render`. M4.5 adds **no other new crate**.
- **Reuse at M7.** The test server (`apps/server`) uses `tessera-physics` (the controller
  and the collision-set types) for server-side movement, with the same step as the client.
  `physics` alone does not make its movement identical to the client's: on install data the
  client's sets include generated scenery that blocks (`collision.md` §9.4, §12.1), the
  statics of landblock info and the water of `water.md` §6–§7, and `world` builds all of
  them. **OPEN (M7):** either `apps/server` depends on `tessera-world` for set building, or
  the asset-free part of it (sets from decoded records, scenery placement, the `water.md`
  §6.2 rule) moves into a module the server can use without `world`'s asset, animation and
  text dependencies. Decide with `server-content.md`. On the server's own content (sets it
  builds itself) `physics` is enough.
- **Tick reconciliation.** The character controller runs on its own fixed-step accumulator
  inside the frame, on the main thread: steps of exactly 1/60 s, at most 5 per frame
  (`collision.md` §12.3). `tessera-physics` owns these numbers as public constants
  (`STEPS_PER_SECOND` = 60, `STEP_SECONDS` = 1/60, `MAX_STEPS_PER_FRAME` = 5, citing
  `collision.md` §12.3), and its step function takes no step length, so no caller can step
  another length. The client owns the accumulator (`tessera-input`'s `FixedTimestep` built
  from `STEPS_PER_SECOND` and `MAX_STEPS_PER_FRAME`), which only decides how many steps run.
  Time beyond the cap is dropped, not carried over. Input is latched per step, and the
  rendered avatar and follow camera are interpolated between the last two controller states.
  The 30 Hz tick of ARCHITECTURE §20.3 (`keymap.md` §9.1) remains for the other fixed-rate
  local simulation (free-fly controller, camera smoothing) and no longer drives movement
  against collision.
- **Collision-set residency.** Sets are installed and removed when the frame drains asset
  completions (ARCHITECTURE §20.3 step 5), after the frame's controller steps and never
  during a step; a set is not removed while the mover needs it. A step needs the sets of
  every landblock that the capsule's bounding box, grown by the largest per-step
  displacement, overlaps. While one of them is not installed (not yet built, deferred,
  after a teleport) the controller does not step: accumulated time is dropped, latched input
  is kept, the mover's state does not change (the "pending until built" rule of `walker.md`
  §5 carries over). A landblock the streamer has resolved as absent from the archives counts
  as an empty set and is a solid boundary. Queries over several sets visit them in
  ascending landblock order, and ties between contacts at the same time of impact go to the
  smallest (landblock, primitive index), so worker completion order never changes a result.
- **Input validation.** Movement parameters come from archive property sets (`collision.md`
  §10.3, §11.1) and are hostile input. `physics` validates at its API boundary and never
  panics in a step: a non-finite or out-of-range parameter is replaced by the layer below it
  in the §11.1 overlay and finally by a documented built-in base-layer default; clamp bounds
  are ordered first and no panicking `clamp` is used; speeds are bounded so that one step
  never moves the mover more than a documented displacement bound (far below 160 m); capsule
  dimensions must be finite and positive; input axes are sanitised as in `keymap.md` §9.1.
  A fuzz target drives the controller with arbitrary parameters, inputs and synthetic sets
  and checks that its state stays finite.
- **The M3–M4 walk controller.** `tessera-input::WalkController` (terrain heights,
  `keymap.md` §9.3) is not retired at M4.5. It stays as the walk mode used when the
  collision system is switched off (the per-system switches of ROADMAP M4.5); the
  physics controller is the walk mode otherwise. Retiring it later needs no new record.

## Consequences
- The controller is deterministic and independent of the frame rate: with the same start
  state, per-step inputs and collision sets (water included) it produces the same positions
  at 30, 60 or 144 frames per second. Because it waits rather than steps without its sets,
  worker timing changes only when a step runs, not what it computes. The scripted
  `--walk-path` runs and the soak walks give inputs per step, not per wall-clock time, and
  start once the first step's sets are installed; the collision tests rely on this.
- `tessera-physics` can be unit-tested and fuzzed without streaming, assets or a GPU.
  Collision sets are built from decoded records on the asset workers and handed to it.
- ARCHITECTURE §3 (rules and diagram), §3.1 (crate rows and the M7 OPEN), §10.3 (water
  interface) and §20.3 (simulation steps, residency, input validation) record this decision;
  CLAUDE.md lists the crate in its Layout section.
- Two Tessera design specs describe the M3–M4 walk mode and are superseded for walk mode
  with collision: `walker.md` §2 ("controllers (30 Hz fixed step)") and §5 ("the
  `tessera-input` walk controller on terrain heights"), and `keymap.md` §9.3 as the walk
  mode of the client. The walker's M4.5 design section (ROADMAP M4.5, an implementer
  document) states the new walk mode and updates §2 and §5. The free-fly controller and the
  terrain-height walk controller stay on `keymap.md` §9.1's 30 Hz step, which is unchanged.
- `collision.md` §12 (a Tessera Decision section) should point at the residency and
  input-validation rules above; that is the spec owner's change, not this record's.
- The 1/60 s step is Tessera's design (`collision.md` §12 is a Decision section). When the
  original's integration is known (`collision.md` §12.3 and §16 Q9 keep it OPEN), the
  controller is revised in `collision.md`; if the step length changes, a record that
  supersedes this one says so.
- A later need for collision outside `world` and the client (for example the explorer's
  collision overlays) uses `tessera-physics` directly; tools may depend on any library crate.
