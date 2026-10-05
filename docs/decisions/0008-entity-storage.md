<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0008 — Entity storage: a project-owned store, not `bevy_ecs`

**Status:** Accepted, 2026-10-05 (applies the rule of [0005](0005-no-full-bevy.md) to the M4
benchmark below)

## Context
[0005](0005-no-full-bevy.md) left one Bevy question open for M4: should the client's entity
storage (ARCHITECTURE §20.3, main thread) be a standalone `bevy_ecs` or a plain store of our
own? `bevy_ecs` is adopted only if it clearly wins a benchmark of 50 animated characters plus
streamed landblocks on frame time, code clarity, build cost and upgrade churn.

The client needs few component sets: a transform per entity, the network entity ID
(server-driven entities, looked up on every movement event), a render-instance handle, an
animation instance (scheduler ID plus `Animator`), and the landblock membership of streamed
statics (spawned and despawned a landblock at a time). The renderer and the animation
scheduler must learn about additions, moves and removals, and must get removed handles back
to free GPU instances and scheduler slots. Everything runs on the main thread (§21).

Both candidates were built:

- **`tessera-entity`** (`crates/entity`): generational IDs (`Entity`, slot reuse with a
  generation bump, slot cap), sparse-set columns (`Column<T>`), an `EntityStore<R, A>` with the
  five component sets and their indexes (network ID → entity, landblock → statics), fixed
  queries (`animated_mut`, `renderables`, `landblock_renderables`, …), per-frame change lists
  that return removed payloads (`take_changes`), and deferred operations for removal while
  iterating (`Deferred`, `apply`). Neither `Send` nor `Sync`.
- **`bevy_ecs` 0.18.1** standalone (`World`, cached `QueryState`s, one `Schedule` with
  `Changed`/`Added` filters), minimal features (`default-features = false, features =
  ["std"]`), behind the non-default feature `bevy-bench` of `tessera-entity`.

## Measurements
Benchmark: `crates/entity/examples/entity_bench` (both adapters implement one `Backend`
trait). Workload per frame, paced at 60 Hz: (1) a movement event per character looked up by
network ID, five characters leaving and five arriving every 120 frames; (2) the M3 flight,
replayed from a trace built once with `tessera-world`'s `RingTracker` (default rings, at most
32 integrations per frame): integrated landblocks spawn their statics, released ones despawn
them; (3) collect poses, advance 50 animators (idle/walk/run speed blend, 89-bone rig) and
submit one real `tessera-anim` sampling job each to the worker pool; (4) mirror new, moved and
removed render instances into an instance table and copy each character's skinning palette.
Timed: the four phases on the main thread. A full query over every renderable (a cull-style
scan) is timed separately.

Run (2026-10-05, re-run after the review fixes of the store: `f64` transform positions,
payloads handed back on refusal, the own adapter writing an instance once when it is both
added and moved; Intel i9-10885H laptop, which is not the reference machine (see the last
consequence), 8 cores / 16 threads, 14 workers, release build, archives in the page cache):
`TESSERA_GAME_DIR=… cargo run --release -p tessera-entity --example entity_bench --features
bevy-bench -- --real --rounds 3`. Statics come from the install's
landblock bundles along the busiest row of region 1 (row 120): 2,439 frames (40.6 km at
1,000 m/s), 5,355 landblocks integrated and 5,137 released, 205,415 statics (93,703 drawn,
38.4 per landblock), **peak 388 resident landblocks with 50,489 statics** (ROADMAP M3
recorded a peak of 386 landblocks). Both stores ended with identical entity, renderable and
instance-table counts and the same scan checksum.

Per-frame main-thread time, milliseconds, three alternating rounds (own, bevy, own, …):

| Store | mean | p99 | max | stream phase mean | render-sync mean | anim mean | full scan mean |
|---|---|---|---|---|---|---|---|
| `tessera-entity` | 0.214–0.261 | 0.59–0.77 | 1.06–1.50 | 0.016–0.019 | 0.087–0.103 | 0.105–0.133 | 0.051–0.058 |
| `bevy_ecs` 0.18.1 | 0.276–0.300 | 0.90–0.95 | 1.29–1.81 | 0.030–0.032 | 0.123–0.131 | 0.113–0.129 | 0.033–0.034 |

Both stores performed the same 215,653 instance writes and 121,950 palette copies. The
1.50 ms maximum is the own store's first round (columns growing from empty). Unpaced
(throughput, two rounds) runs agree: mean 0.100–0.107 vs 0.127–0.135 ms, stream 0.010–0.011
vs 0.016 ms, scan 0.015 vs 0.009 ms. A synthetic stress run (`--characters 200
--statics-mean 155 --no-pace`, peak 68,440 entities): mean 0.45 vs 0.61–0.63 ms, max
2.2–2.4 vs 2.2–3.3 ms.

Reading: both stores use under 2 % of a 16.7 ms frame. The own store is faster where the
client spends its entity time: spawning and despawning a landblock's statics (about 2×) and
the render hand-off (change lists cost O(changes); `Changed`/`Added` filters visit every
matching entity's change tick each frame). `bevy_ecs` iterates a full query 1.5–1.7 times as
fast (archetype tables); the client's renderer keeps its own per-landblock instance lists,
so it does not scan every entity per frame.

**Code clarity.** Adapter lines (non-blank, non-comment): own 109, bevy 147 (the own
adapter's count includes the write-once bookkeeping for entities both added and moved, and
releasing a render key a full store hands back; the bevy query visits such an entity once by
construction). The bevy adapter
also needs a network-ID map and a landblock → entities map of its own, must read or `take`
the render and animation components before every despawn (removal events carry no payload),
needs a `Schedule`, a `Resource` wrapper moved in and out of the world and `clear_trackers`
for change detection, and a type alias to pass clippy's complexity lint. The own store's
library is 942 non-blank, non-comment, non-test lines; `bevy_ecs` and its own sub-crates are about 70,000 lines with
about 2,000 `unsafe` blocks, functions and impls (the workspace denies `unsafe` in its own
code).

| Task | `tessera-entity` | `bevy_ecs` |
|---|---|---|
| Spawn a character | `spawn_networked`, `set_render`, `set_anim` | `world.spawn((…))` plus the adapter's net-ID map |
| Network ID → entity | `entity_for` (built in) | adapter `HashMap` |
| Landblock statics | `spawn_static`, `despawn_landblock` (built in) | adapter `HashMap<u32, Vec<Entity>>`, despawn one by one |
| Removed render/anim handles | delivered by `take_changes` | read or `take` components before `despawn` |
| Hand-off to renderer | change lists | `Schedule` system with `Or<(Changed<_>, Added<_>)>`, `clear_trackers` |
| Removal while iterating | `Deferred` + `apply` | `Commands` in a system, or collect then despawn |
| New component kinds | a field and accessors in the store, or a caller-side `Column<T>` | any `Component` type |
| Thread rule (§21) | `!Send + !Sync` by type | `World` is `Send + Sync`; components must be too |

**Build cost** (same machine, shared target directory, packages already built for other
workspace members not rebuilt): clean build of `tessera-entity` 0.34 s (dev) / 0.36 s
(release) without the feature; 23.0 s (dev) / 15.9 s (release) with it, compiling 31 units.
Incremental rebuild after touching the benchmark: 0.41 → 0.52 s (dev), 1.32 → 2.09 s
(release); the library itself does not change (it never uses `bevy_ecs`). `cargo tree`
(normal + build edges): 12 crates without the feature, 56 with it: **44 added, 30 of them new
to the workspace**; it adds a second `winnow` and `toml_datetime` version.

**Upgrade churn** (crates.io metadata, 2026-10-05): 64 published versions; breaking 0.x
minor releases 0.13 (2024-02-17), 0.14 (2024-07-04), 0.15 (2024-11-29), 0.16 (2025-04-24),
0.17 (2025-09-30), 0.18 (2026-01-13), 0.19 (2026-06-19), every 105–159 days, and 0.20 is at
release candidate 2 (2026-09-28). MSRV: 0.18.x needs Rust 1.89 (fits the workspace's 1.90);
**0.19 and 0.20 need 1.95**, so staying current would mean raising the workspace MSRV or
staying on an unmaintained line.

**Licence fit**: `cargo deny check licenses bans sources` (cargo-deny 0.20.2, all features)
passes. The 44 added crates are MIT OR Apache-2.0 (36), MIT (6) and Zlib (2), all on the
GPL-3.0-or-later allow list; the only new findings are the two duplicate-version warnings
above.

## Decision
- The client's entity storage is **`tessera-entity`'s own store**. `bevy_ecs` does not
  clearly win: it is slower per frame on this workload (faster only at full scans the client
  does not need), needs more adapter code, adds 44 crates and 16–23 s of clean build, breaks
  its API about every five months, and its current line needs a newer compiler than the
  workspace MSRV. 0005's rule therefore keeps the project-owned store.
- `bevy_ecs` stays only as the optional dependency behind `tessera-entity`'s non-default
  `bevy-bench` feature, used by nothing but the benchmark example, so this record can be
  re-measured. Default builds never compile it. It is pinned to the 0.18 line (MSRV-fit) and
  is not upgraded; it can be deleted (feature, dependency, `bevy.rs`) at any time without
  touching the library.
- `tessera-entity` sits in layer L4 (ARCHITECTURE §3.1). It depends only on `std`,
  `thiserror` and `tracing`; render and animation handles are type parameters, so it depends
  on neither `tessera-render` nor `tessera-anim`.

## Consequences
- The client adopts `EntityStore<R, A>` with its render-instance handle as `R` and the
  animation state (scheduler `InstanceId` plus `Animator`) as `A`, consumes
  `take_changes` once per frame (removals, then additions, then moves) and uses
  `spawn_static` / `despawn_landblock` keyed by the landblock-data DID (`LandblockTag`).
- Entity transforms hold **`f64` positions in the region frame** (ARCHITECTURE §9.1 world
  position, `docs/specs/coordinates.md` §12); rotation and scale stay `f32`. Region
  coordinates reach about 40 km, where `f32` steps are about 4 mm, so streamed statics
  (landblock origin plus block-local placement) and network positions are stored without
  loss, and the renderer subtracts its camera anchor in `f64` before narrowing to `f32`.
- Removed render and animation payloads come back through `take_changes`; a payload an
  operation refuses (`set_render` / `set_anim` on a dead entity, `spawn_static` on a full
  store) comes back in the `Rejected` error. The removal lists hold one entry per removed
  payload (an entity can appear several times); release each entry. An entity can be in
  `render_added` and `moved` in one batch; write its instance once.
- `StoreConfig::max_entities` bounds slots, columns and indexes, not the pending change
  lists, which grow with the operations since the last take: the client takes the changes
  every frame and bounds the network events it applies per frame.
- Caller-side `Column<T>` side tables refuse an insert under an ID older than the slot's
  current value (the value is handed back); events are still checked with `is_alive`
  before their values are inserted.
- New component kinds are added deliberately (a field and accessors in the store, or a
  caller-side `Column<T>` cleaned with the despawn list). If the client later needs many
  ad-hoc component combinations or system scheduling, re-run the benchmark before revisiting
  this record; a later record would supersede it.
- `cargo clippy --all-features` and `cargo deny` (which checks all features) still compile
  and check `bevy_ecs` while the bench feature exists; `Cargo.lock` lists its 30 extra
  packages.
- The measurements above are machine-specific; re-run the example after significant store
  changes and compare against this table.
- The benchmark ran on a laptop, not on the reference machine of
  [`docs/perf/reference-machine.md`](../perf/reference-machine.md) (still to be confirmed by
  the owner). The decision rests on a relative comparison, both stores measured on the same
  machine in alternating rounds, so it stands; under the reference-machine rules the absolute
  numbers are indicative, and the same run on the reference machine is still pending.
  **Update 2026-10-05:** [decision 0010](0010-reference-machine-laptop.md) made this laptop
  the reference machine, so the numbers above are reference numbers.
