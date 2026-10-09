<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Particle runtime: starting, attaching, running, stopping and drawing particle effects

| | |
|---|---|
| **Status** | Accepted (2026-10-08) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted, not the author), 2026-10-08 |
| **Review record** | Review 2026-10-08: **passed**, Accepted. Details and non-blocking notes in the review log (decision 0018). |
| **Implements into** | `apps/client` (Particle-node instances, effect attachment to entities, lingering and removal, the per-frame order), `crates/render` (particle draw calls, draw order inside a system), `crates/formats` (no new decoding; the Particle node's extra byte of `script-table.md` §4.1 gets its meaning here), a shared random source (§3.3) |
| **Companion specs** | [`particles.md`](particles.md) (record layouts §4, §6; the simulation of one 0x39 system §8; start gating §7.1.2; node record choice §7.1.1; fork effects §6.5). [`script-table.md`](script-table.md) §4.1 (the extra byte after a Particle node's base block), §5.3 (Particle node fields). [`script-runtime.md`](script-runtime.md) §2.1 (contexts: intensity, flags), §3 (node start, completion, stop). [`setup.md`](setup.md) §5.2 (render scale, `Model_ParticleScale`). [`skeleton-anim.md`](skeleton-anim.md) (holding locations, bone types). [`material.md`](material.md) §5.3.1 (clock, curve evaluation), §4 and §7 (surface records and their blend states). [`water.md`](water.md) (water surfaces). |

---

<a id="s0"></a>
## 0. Purpose

`particles.md` describes the two record families and how **one** running 0x39 system
simulates and draws its particles. This spec covers the run time around that: everything an
implementer needs to make effects appear, follow their owners and disappear as in the
original client. It lets an implementer:

- run the client's clock and random source for particles (§3);
- start an effect from a Particle node: the start gates, the stored "never-ending" byte, the
  attached and the dropped paths, the water probe and the redirect property (§5);
- attach a running effect to an entity each frame: bone, holding location, offset, removal of
  the bone's rotation, and the owner's two scale vectors (§6; this answers `particles.md` Q4);
- stop effects, let them linger without emitting, and remove them (§7);
- know which simulation facts live in `particles.md` (§8) and draw systems with the right
  calls, material and particle order (§9);
- handle fork effects and engine-started systems at the run-time level (§10, §11);
- check an implementation against a complete synthetic emitter over 2 s (§14).

Out of scope: the record layouts (`particles.md` §4, §6); the internal format and simulation
of the third-party fork effect image (`particles.md` Q1); the beam renderer (§11, OPEN); the
weather schedule (`particles.md` Q13).

Names. Node fields are written with the `ParticleScript_` labels of the master property
dictionary 0x34000000 (`script-table.md` §5.3), needed for interoperability. Other names are
this spec's own. No game text appears here; records are referred to by DID.

All counts are observations of one install, read at run time, never hard-coded. Numbers that
define how the client computes (the 2 s and 6 s timers, the 0.0002 tolerance, the probe
heights, the random generator's constants) come from the client executable and may be used as
constants ([decision 0012](../decisions/0012-numeric-constants.md)).

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed (BIN)** = read from the client executable (row BIN); **Confirmed (BIN +
OBS)** = read from the executable and agreeing with the data over all records of the kind;
**Confirmed** = observed in the install on 2026-10-08 over all records of the kind;
**Inferred** = follows from names or consistent data, not traced in code; **Decision** = a
Tessera rule (§12); **Reported** = published sources only (none used).

| Tag | Source |
|---|---|
| BIN | **M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (re-checked 2026-10-08), analysed with Ghidra 12.1.3 in headless mode on a temporary copy of the analysis project (deleted afterwards), 2026-10-08, in the dirty room. **Questions:** (a) what the client does with each Particle-node field at run time, the start gates, the meaning of the extra byte after the node's base block, the dropped-effect path, the water probe and the redirect property; (b) how an attached effect's frame is computed each frame and which owner values feed the two scale vectors of `particles.md` §8.6.3; (c) how an effect is stopped, when it lingers and when it is removed; (d) the random generator particle code draws from and how it is seeded; (e) the draw calls of one system instance, the material they use and the order of particles inside a system; (f) every code path that creates a particle or fork system; (g) how fork draw batches select their shader. **Why M1–M3 were not enough:** all of these are code paths; the data holds parameters only; no published source describes them (M2); no running client and no frame capture are available to this session (M3). Only offline-client code for effects, scripts, scene placement and drawing was read; nothing of the network, login, launcher or anti-tamper code (decision 0004). Facts that rest on BIN alone are marked so. |
| OBS | User install (Steam), read in place and read-only on 2026-10-08: all 7,526 script tables (7,524 decodable, as in `script-table.md`) and 28,992 scriptlets of `client_gamelogic.dat`, every Particle node (41,354); all 11,155 records with prefix 0x39 and all 254 with prefix 0x44 of `client_general.dat`; the master property dictionary 0x34000000. |
| PS | [`particles.md`](particles.md) (Accepted 2026-10-08): layouts, censuses, the simulation of one 0x39 system (§8), start gating (§7.1.2), fork effects (§6.5). |
| SR | [`script-runtime.md`](script-runtime.md) §2.1, §3 (contexts, node start, completion, stop). |
| SE | [`setup.md`](setup.md) §5.2 (render scale; `Model_ParticleScale`, PID 0x5A0). |
| MA | [`material.md`](material.md) §5.3.1 (global clock; curve evaluation). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| RT1 | Particle and fork systems are created only by: the Particle node (attached path, dropped path, redirect restart), the weather code (10 UNIQUEDB systems), the beam renderer, and one gameplay event handler (§5.1) | BIN (the complete caller lists of the three creation routines) | Confirmed (BIN) |
| RT2 | The global clock is the double-precision client clock of `material.md` §5.3.1; particle code reads no other time source (§3.1) | BIN; MA | Confirmed (BIN) |
| RT3 | Particle randomness comes from the client's single global generator: L'Ecuyer's combined generator with a 32-entry shuffle table (constants in §3.3), seeded once at start-up from the wall-clock time in seconds; range draws as in §3.4 | BIN (generator, seeding routine and its two callers, the range routines) | Confirmed (BIN) |
| RT4 | The extra byte after a Particle node's base block is the node's stored never-ending classification: 0 = runs out by itself, 1 = never-ending; the client uses the stored byte and computes the classification only when writing a node or when the byte is missing (§5.3) | BIN (the node's serialisation routine); OBS (41,354 of 41,354 stored bytes equal the classification of §5.3: 0x39 nodes 26,196 × 0 and 13,634 × 1; 0x44 nodes 934 × 0 and 590 × 1) | Confirmed (BIN + OBS) |
| RT5 | The classification rule of a 0x39 effect takes the per-emitter maximum starting from 0, so only `physics_duration` > 0 or an emitter (active or not) with `emission_limit` = 0 and `time_limit` = 0 makes it never-ending; a 0x44 effect is never-ending when its `flag` = 1 (§5.3) | BIN; OBS (as RT4; record 0x39002566, whose per-emitter values are all negative, is stored as 0) | Confirmed (BIN + OBS) |
| RT6 | Node start: no effect DID fails the node; a finite effect is not started when the context intensity ≤ 0.0002 or context flag 0x40 is set; `ParticleScript_DropEffect` selects the dropped path (§5.4) | BIN | Confirmed (BIN) |
| RT7 | Attached path: target resolution through `ParticleScript_RedirectProperty`, the holding-location/bone pre-check, the effects interface, the start gating of `particles.md` §7.1.2, an asynchronous request; the node keeps an instance only for never-ending effects (§5.5) | BIN | Confirmed (BIN) |
| RT8 | Dropped path: the effect is placed once in the world at the attachment frame, with no start gating, no fade settings and no instance; the offset enters twice (§5.6) | BIN | Confirmed (BIN) |
| RT9 | `ParticleScript_OnWater` acts only on the dropped path: a vertical probe against water surfaces sets the effect's height to the highest water hit, or nothing is placed (§5.7) | BIN; OBS (130 nodes set it, 15 of them with `ParticleScript_DropEffect` = 1) | Confirmed (BIN + OBS) |
| RT10 | Attachment frame: holding location or bone or none, `ParticleScript_BoneOffset` in the bone's space, multiplied by the bone's current model-space matrix, rotation replaced by identity when `ParticleScript_RemoveRotation` = 1, then the owner's frame (§6.1–§6.3) | BIN | Confirmed (BIN) |
| RT11 | The two owner scale vectors of `particles.md` §8.6.3: *A* = the owner's render scale (`setup.md` §5.2), *B* = (*p*, *p*, *p*) with *p* = the owner's `Model_ParticleScale` (1 when absent or not a float) (§6.4) | BIN; SE | Confirmed (BIN) |
| RT12 | Stopping a never-ending effect: the node instance's stop hook stops it, with `ParticleScript_ImmediateDisposal` deciding whether it may linger (§7.1) | BIN | Confirmed (BIN) |
| RT13 | Release rule: a system whose last user lets go lingers (emission stopped, particles live on) when it is loaded, not disposed immediately, not of the shareable kind, and was drawn last at opacity ≥ 0.0002 or is world-space (§7.2) | BIN | Confirmed (BIN) |
| RT14 | Lingering systems are removed once their end test passes or 6 s after they were stopped, checked every frame; attached systems are checked every 2 s (§7.3, §7.4) | BIN (both constants and both loops) | Confirmed (BIN) |
| RT15 | Particles of an emitter are stored in creation order; a dying particle is replaced by the last one; they are drawn in storage order with no depth sort (§9.2) | BIN (spawn, update and vertex-writer routines) | Confirmed (BIN) |
| RT16 | A system instance is drawn with at most two draw calls (indexed quads, then triangles) using the material of its loaded surface record, tinted (1, 1, 1, instance opacity), at the global clock (§9.1) | BIN | Confirmed (BIN) |
| RT17 | Fork draw batches select one of the seven UNIQUEDB fork shaders by a blend-mode index 0–6 (§10.2) | BIN; PS §6.4 (the keys) | Confirmed (BIN) |
| RT18 | Census of Particle-node fields (§5.2) | OBS | Confirmed |
| RT19 | Tessera rules (§12) | This spec | Decision |
| RT20 | Whether an owner that is not drawn places its effects; the exact per-frame order; context flag 0x40; bounds culling; beams; the 0xC000 draw flag (Q-RT1–Q-RT6) | BIN (partial) | OPEN |

---

<a id="s2"></a>
## 2. Where each run-time topic is specified

| Topic | Where |
|---|---|
| Clock, frame order, random source | §3 here |
| Emitter start (system start, `fast_forward`, restart) | `particles.md` §8.17 |
| Emitter activity window and end-of-effect test | `particles.md` §8.2 |
| Spawn rate and bursts, caps, catch-up | `particles.md` §8.3; distance emission §8.4 |
| Initial position and velocity distributions (shapes, direction, spread, speed) | `particles.md` §8.5 (random source: §3.3 here) |
| Forces: gravity, wind, mass; drag (the 0.95 damping of controller 4 only) | `particles.md` §8.7, §8.8 |
| Lifetime | `particles.md` §8.6 |
| Size, colour and alpha over life (keyframe channels), emitter fade envelope | `particles.md` §8.6.1, §8.6.2, §8.9 |
| Rotation | `particles.md` §8.12 (modes 3) and §8.7 (growth with age) |
| Texture-sheet animation | `particles.md` §8.10 |
| Billboarding and orientation modes | `particles.md` §8.12 |
| Blend modes | §9.1 here (material of the surface record, `material.md`); fork effects §10.2 |
| Sorting between systems and passes | `particles.md` §8.15; inside a system §9.2 here |
| Forks (child effects) | `particles.md` §6.3–§6.5; run-time level §10 here |
| Culling, LOD, caps | `particles.md` §7.1.2 (options), §8.11 (32,768 vertices), §8.13 (distance fade, not-drawn state); §9.3 here |
| How scripts start and attach effects | §5, §6 here |
| Stop, linger, removal | §7 here |
| Sharing of running systems | `particles.md` §8.16; release §7.2 here |

---

<a id="s3"></a>
## 3. Clock, frame order and random source

<a id="s3-1"></a>
### 3.1 Clock (BIN)

All particle times are values of the client's **global clock**: seconds since the client's
timer started, held in double precision (`material.md` §5.3.1). "Now" below is that clock.
The particle code reads no other time source; frame length enters only as the difference
between two readings. `particles.md` §8.1 adds one rule: a system's update uses the later of
its creation time and the clock.

<a id="s3-2"></a>
### 3.2 Per-frame work (BIN; order partly OPEN)

Each frame the client does the following particle work:

1. **Lingering sweep** (§7.3): once per frame, from the world renderer's update, every
   lingering system is tested for removal.
2. **Placement.** Each entity's effects component places each of its attached systems for the
   frame with the attachment frame of §6 (and an attached system's frame is recomputed whenever
   the owner's frame or pose changes). Lingering and dropped systems are placed by the scene
   they were registered with. The first placement of a system in a frame runs its update
   (`particles.md` §8.1, §8.16).
3. **Housekeeping** (§7.4): each effects component, at most every 2 s, removes its attached
   systems that have finished.
4. **Draw** (`particles.md` §8.15 for the order of systems; §9 here for one system).

> **OPEN (Q-RT1):** The order of steps 1–3 within a frame, and whether an entity that is
> culled (not drawn) still places its attached systems, were not traced. **Decision
> (Tessera):** run 1, 2, 3, 4 in that order each frame; an entity places its effects only when
> it is drawn, so the effects of an undrawn entity freeze like the not-drawn state of
> `particles.md` §8.13.

> **OPEN:** (implementer, 2026-10-09) What "drawn" means in the walker
> (`apps/client/src/walker/effects`), engineering choices until Q-RT1 is answered: the
> walker scans the landblocks within 2 of the viewer for static entities and placed figures
> whose start-up scripts reach Particle nodes (content layers as the world's; interior
> entities only with `--interiors`), spawns such an owner in its own script runtime when the
> viewer is within the owner's *reach* + 20 m and despawns it beyond *reach* + 60 m or when
> its landblock leaves (checked every 0.25 s, at most 16 spawns per check). *Reach* is the
> largest `far_fade_end` of the 0x39 records its nodes name (fork nodes: their
> `FarFadeMaxDistance`) plus the node's `BoneOffset` length, at most 500 m, 60 m when none is
> known. A static owner counts as drawn while it is spawned (no frustum or per-object draw
> distance test); a placed figure while the character layer draws it, and it is spawned only
> once its figure is built (its attachment points and pose come from there). Despawning
> stops the owner's never-ending nodes (§7.1) and releases its other effects (§7.2).

<a id="s3-3"></a>
### 3.3 The random generator (BIN)

Every random number the particle code uses (shape points, spread angles, random start frames,
the point indices of controllers 1 and 3, the edge choice of the `Quad` shape) and every
evaluation of a mode-3 curve (`material.md` §5.3.1) is drawn from **one global generator**
that the whole client shares. It is the combined multiplicative congruential generator of
L'Ecuyer with a Bays–Durham shuffle, the algorithm published in *Numerical Recipes* as
"ran2" (general knowledge), with these constants (BIN):

| Constant | Value |
|---|---|
| Modulus 1, multiplier 1 | 2,147,483,563 and 40,014 (with Schrage's decomposition 53,668 / 12,211) |
| Modulus 2, multiplier 2 | 2,147,483,399 and 40,692 (52,774 / 3,791) |
| Shuffle table | 32 entries; index = previous output ÷ 67,108,862 (integer division) |
| Output scale | output × (1 / 2,147,483,563), capped at 1 − 1.2 × 10⁻⁷ |

The generator keeps two 32-bit states *s*₁ and *s*₂, the table and its previous integer
output *y*.

- **One draw.** Each state is multiplied by its multiplier and reduced modulo its modulus
  (computed without overflow; a negative intermediate result is made positive by adding the
  modulus). The table index *j* is *y* integer-divided by 67,108,862. The new *y* is the table
  entry at *j* minus the new *s*₂, and the table entry at *j* is replaced by the new *s*₁; when
  the new *y* is below 1, 2,147,483,562 is added to it. The draw's value is *y* divided by
  2,147,483,563 in double precision, capped as in the table, so it lies in (0, 1).
- **Seeding with *n*.** Both states are set to *n* (to 1 when *n* is 0). Then *s*₁ is advanced
  forty times by the first rule; counting the steps *k* = 39 down to 0, the value after step
  *k* is stored in table entry *k* whenever *k* < 32. Finally *y* is set to table entry 0.
- **When.** The client seeds the generator once at start-up with the current wall-clock time
  in whole seconds. One other routine reseeds it with a caller-supplied value (its caller was
  not traced). Every system of the client draws from the same stream, so the sequence a given
  effect sees depends on everything else the client did before.

<a id="s3-4"></a>
### 3.4 Range draws (BIN)

- **Real range [*a*, *b*]**: if *a* = *b*, the result is *a* and **no** number is drawn;
  otherwise, with lo = min(*a*, *b*) and hi = max(*a*, *b*), the result is lo + *u* × (hi − lo)
  for one draw *u*, computed in double precision and rounded to single precision.
- **Integer range [*a*, *b*]**: if *a* = *b*, *a* without a draw; otherwise lo + the integer
  part of *u* × (hi − lo + 1).

Every "random in [*x*, *y*]" of `particles.md` §8.5–§8.10 is one real-range draw; every "index
drawn uniformly" is one integer-range draw.

---

<a id="s4"></a>
## 4. Run-time objects (BIN)

| Object | Created by | Holds | Ends |
|---|---|---|---|
| **System** | a request that finds no shareable running copy (`particles.md` §8.16) or the weather and beam code | the loaded record, emitters, particles, the draw instances of the current frame (`particles.md` §8.16), a *stopped* flag with its time, an *immediate-disposal* flag, a reference count | when its last reference is released (§7.2) |
| **Attachment** | the owner's effects component when a requested system has loaded (§5.5) | the request handle, the bone index (or none), the attachment matrix (§6.1), the `RemoveRotation` byte, the system | when stopped (§7.1) or finished (§7.4) |
| **Pending request** | the attached path (§5.5) | a handle (an increasing counter), the record DID, the requester | when the record has loaded (served) or the request is stopped (§7.5) |
| **Lingering entry** | the release rule (§7.2) or the dropped path (§5.6) | the system, its fixed placement (frame, the two scale vectors, opacity) | §7.3 |
| **Node instance** | a Particle node whose effect is never-ending (§5.3) | the effect's handle, its owner's ID, the `ImmediateDisposal` byte, a listener for the redirect property | when the node is stopped (`script-runtime.md` §3.5) |
| **Shareable list** | every system of the shareable kind while it runs | running systems that may serve later requests (`particles.md` §8.16) | entries leave when the system is released |

Each entity with effects has one **effects component** holding its pending handles, its
attachments and a 2-s housekeeping timer (§7.4).

---

<a id="s5"></a>
## 5. Starting effects

<a id="s5-1"></a>
### 5.1 Entry points (BIN)

| Entry point | Path | Attached to an owner | Gated by `particles.md` §7.1.2 |
|---|---|---|---|
| Particle node, `ParticleScript_DropEffect` = 0 | §5.5 | yes | yes |
| Particle node, `ParticleScript_DropEffect` = 1 | §5.6 | no (placed once in the world) | **no** |
| Change of the redirect property of a running never-ending node | §5.8 | yes | yes |
| Weather code (the ten UNIQUEDB systems, `particles.md` §7.1.3, §8.8.1) | created synchronously at weather start-up | no | no |
| Beam renderer (Beam node's `BeamScript_BeamParticleSystem` and `BeamScript_HeadParticleSystem`) | created synchronously by the beam renderer (§11) | by the beam | not traced |
| One gameplay event handler | places an effect named by a property of a game object at the local player's position | no | no |

No other code creates a particle or fork system: setups, animations and items start effects
only through scripts.

<a id="s5-2"></a>
### 5.2 The Particle node's fields at run time

| Field (`ParticleScript_…`) | Use | Default of a new node (BIN) | Census over 41,354 nodes (OBS) |
|---|---|---|---|
| `ParticleID`, `ForkParticleID` | the effect DID (`particles.md` §7.1.1) | none | `particles.md` §2.2 |
| `ShouldFade`, `FarFadeMinDistance`, `FarFadeMaxDistance`, `CloseFadeMinDistance`, `CloseFadeMaxDistance` | distance fade of **fork** effects only, attached path only (`particles.md` §6.5) | 1, 40, 60, 0, 0 | `ShouldFade` 0 in 57 nodes (all 0x39, so without effect); fade pairs other than (40, 60, 0, 0) in 78 nodes, all 0x39 (without effect) |
| `HoldingLocation`, `BoneType`, `BoneOffset` | attachment point (§6.1) | 0, 0, (0, 0, 0) | holding location only 9,650; bone only 6,756; both 88; offset ≠ 0 in 15,879 |
| `DropEffect` | selects the dropped path (§5.6) | 0 | 1 in 2,856 nodes (2,644 for 0x39, 212 for 0x44), every one of them finite (§5.3) |
| `RemoveRotation` | drop the bone's rotation (§6.2) | 0 | 1 in 7,334 nodes (3,200 of them with a holding location or bone) |
| `OnWater` | water probe on the dropped path (§5.7) | 0 | 1 in 130 nodes (15 dropped, 115 attached and so without effect) |
| `ImmediateDisposal` | no lingering after a stop (§7.1) | 0 | 1 in 3,584 nodes (3,020 never-ending; 564 finite and so without effect) |
| `RedirectProperty` | target redirection (§5.8) | a value the client reads once at run time from a game-data record (not traced) | one and the same PID, 0x10006F25 (an integer property, not an object reference), in 41,344 nodes; 0 in 10 |
| extra byte after the base block | stored never-ending classification (§5.3) | computed | 0 in 27,130, 1 in 14,224 |

The node's in-memory defaults matter only for nodes created by a writer; every stored node
carries all fields (`script-table.md` §5.3).

<a id="s5-3"></a>
### 5.3 The never-ending classification and its stored byte (BIN + OBS)

A Particle node classifies its effect as **finite** (it runs out by itself) or
**never-ending** (it runs until stopped).

- **Stored.** The byte after the node's base block (`script-table.md` §4.1, "`particle_flag`")
  holds the classification: 0 finite, 1 never-ending. When reading a node the client takes the
  byte as it is and does not recompute it. When writing a node, or when the in-memory value is
  "not computed" (value 255), the client computes it from the effect record as below.
- **Rule for a 0x39 effect.** Never-ending when `physics_duration` > 0, or when any emitter —
  active or not — has `emission_limit` = 0 and `time_limit` = 0. Otherwise compute for each
  emitter *v* = max(`time_limit`, (`emission_limit` integer-divided by `burst_count`) ×
  `emit_interval`) + *L*′ + `start_time` − `fast_forward`, with *L*′ = `w[0]` of `lifespan`,
  plus |`w[2]`| when `lifespan` is not mode 1; take the maximum of 0 and all *v*. The effect is
  never-ending only when that maximum is negative, which cannot happen because the maximum
  starts at 0: in practice only the first two conditions make an effect never-ending. (This
  makes precise the last sentence of `particles.md` §8.17.)
- **Rule for a 0x44 effect.** Never-ending when the record's `flag` = 1 (`particles.md` §6.1).
- **Data.** All 41,354 stored bytes equal this rule's result for the node's current effect
  record (OBS, RT4). Example: record 0x39002566 has `fast_forward` 10 and three emitters
  whose *v* are −2.1, −5.1 and −7.2; its nodes store 0.

<a id="s5-4"></a>
### 5.4 Starting a Particle node (BIN)

When the script runner reaches a Particle node (`script-runtime.md` §3.1 step 6):

1. **Effect DID.** Chosen by `particles.md` §7.1.1. If there is none, the start fails (no
   instance; the node completes at once, `script-runtime.md` §3.1).
2. **Classification** (§5.3).
3. **Intensity gate.** If the effect is **finite** and either the context's intensity
   (`script-runtime.md` §4; the same context value its intensity propagation writes, BIN) is
   ≤ 0.0002 or the context's flags word has bit 0x40 set, nothing
   is started and the node completes at once. A never-ending effect is not gated here.
4. If `ParticleScript_DropEffect` = 1: the dropped path (§5.6); the node completes at once.
5. Otherwise the attached path (§5.5).

The intensity is used only by this gate: the effect request carries no intensity, and the node
instance's intensity-change hook is the generic one, so a later intensity change does not reach
the effect either. A finite effect started at intensity 0.5 looks the same as one started at 1.

> **OPEN (Q-RT2):** Which client paths set bit 0x40 of a script context's flags word
> (`script-runtime.md` §2.1 lists bits 1 and 3 only). **Decision (Tessera):** treat it as clear.

> **OPEN:** (implementer, 2026-10-09) Choices of the walker where this spec gives no fact for
> it (`tessera-scriptgraph` runs the node, `apps/client/src/walker/effects` the paths):
> (a) the redirect of §5.5 step 1 and §5.8 counts as naming "an existing object" only an
> owner spawned in the walker's effects runtime (the offline walker has no other object
> IDs); (b) the start gating of `particles.md` §7.1.2 runs with the client's defaults
> (`DynamicParticles` 0, `StaticEnvironmentalObjects` on), with which the whitelist decides
> nothing, so it is not read; (c) a Particle node's driven fields (its base block's driven
> map naming `ParticleScript_` fields, `script-runtime.md` §5.3) are not applied: are any
> of these fields driven in practice, and how do driven values reach a node that has already
> started?; (d) the instance opacity of attached and dropped placements is 1 (§8.15's
> object opacity and `inherit_opacity` are not applied: the walker keeps no parent-object
> opacity), and which owner value gives an attached instance its own opacity is not stated
> here; (e) the water probe of §5.7 tests the landblock water planes only (`water.md` §8.1),
> not cell water or liquid volumes; (f) vector *A* is `Render_AdjustableScale` ×
> `Model_Scale` for static entities (factors 3 and 4 of `setup.md` §5.2 need attachment
> parents, which the walker does not have) and the drawn model scale for placed figures; *p*
> is the product of `value_b` along the object's visual chain (`setup.md` §5.2).
> (g) Observation: at reference location M3-03, 40 spawned owners asked for 2 effects; the
> other owners' Particle nodes wait behind Switch nodes on properties the walker does not
> set (it feeds no world state into these scripts).

<a id="s5-5"></a>
### 5.5 Attached path (BIN)

1. **Target.** The target is the script's entity, unless `ParticleScript_RedirectProperty` ≠ 0:
   then the client reads that property from the script's entity; if the entity has it and its
   value names an existing object, that object is the target. If not, the script's entity is.
2. **Attachment pre-check.** If `HoldingLocation` or `BoneType` is non-zero and the target has a
   drawn model with a skeleton: look up the bone type (when `BoneType` ≠ 0, which wins) or the
   holding location on it. If the lookup fails, nothing starts (go to step 5).
3. **Effects interface.** If the target has no effects component, nothing starts (go to
   step 5).
4. **Gating and request.** The start gating of `particles.md` §7.1.2 (owner kind, whitelist,
   `DynamicParticles`, `StaticEnvironmentalObjects`). If it passes, the client creates a
   **request** with a new handle and asks for the record. A shareable running copy may serve
   it at once (`particles.md` §8.16); otherwise the record is loaded asynchronously, and all
   requests for the same DID that arrive while it loads are served when it is ready. The
   handle is added to the component's pending list.
5. **Node result.** It depends only on the classification, not on whether anything started.
   A **finite** effect: the node completes at once; a started effect plays on by itself. A
   **never-ending** effect: the node leaves an **instance** holding the handle (an invalid
   handle when nothing started in steps 2–4), the target's ID and the `ImmediateDisposal` byte;
   when `RedirectProperty` ≠ 0 the instance also listens for changes of that property on the
   script's entity (§5.8). Such an instance runs until it is stopped, like every never-ending
   node, even when its effect could not start. The one exception: a script without an entity
   fails the start (no instance).
6. **When the record has loaded**, the component takes the handle off its pending list (if
   the handle is no longer pending, the effect was stopped meanwhile: the new system is
   discarded, §7.5). For a fork record it also copies the node's fade settings
   (`particles.md` §6.5). It then resolves the attachment again (§6.1). If the holding location
   or bone is not found on the owner's model this time, the client logs a warning and discards
   the system (the effect never appears). Otherwise it stores an **attachment** (§4), gives the
   system the owner's frame and render scale, and registers it with the owner's scene.

<a id="s5-6"></a>
### 5.6 Dropped path (BIN)

A dropped effect is placed **once** at the attachment point and then stays in the world; it
does not follow the owner and the node keeps no instance.

1. The target is always the script's entity (no redirect).
2. **Attachment frame.** The client computes the attachment point's current world frame with
   the rules of §6.1–§6.3 (holding location or bone, `BoneOffset`, `RemoveRotation`), using the
   entity's current pose. If the attachment cannot be resolved, nothing is placed.
3. **Water** (only when `ParticleScript_OnWater` = 1): §5.7. If the probe finds no water,
   nothing is placed. On water the offset of step 4 is zero.
4. **Second offset.** The effect's frame is the attachment frame moved by `BoneOffset` along the
   attachment frame's own axes (the offset rotated by that frame's rotation). Because step 2
   already applied `BoneOffset` in the bone's space, a dropped effect without water is offset
   twice: for an effect with no holding location and no bone, about 2 × `BoneOffset` from the
   owner's origin in the owner's axes (BIN; 1,421 of the 2,856 dropped nodes have a non-zero
   offset, OBS). This is the original's arithmetic.
5. **Placement.** The system is created for the effect DID **without** the start gating of
   `particles.md` §7.1.2 and without the node's fade settings, with one fixed draw instance:
   the frame of step 4, scale vector *A* = the entity's render scale, *B* = (*p*, *p*, *p*)
   with *p* = the entity's `Model_ParticleScale` (§6.4), opacity 1. When loaded it becomes a
   **lingering entry that was never stopped** (§7.3): it is removed when its end test passes.

<a id="s5-7"></a>
### 5.7 Water probe (`OnWater`, BIN)

From the attachment point's position the client casts a vertical segment against water
surfaces only (`water.md`):

- if the point is under a water surface of the cell it is in (the client's under-water test,
  not traced further): from 40 m above the point to 1 m below it;
- otherwise: from the point to 10 m below it.

Of all hits it takes the one with the highest Z. The effect's position keeps its X and Y and
takes that hit's height (the hit plane's height at the hit). Without a hit nothing is placed.

<a id="s5-8"></a>
### 5.8 Redirect property (BIN)

`ParticleScript_RedirectProperty` names a property of the script's entity whose value is an
object ID (§5.5 step 1). For a never-ending effect the node instance listens for changes of
that property; on each change it stops the running effect as in §7.1 (with the node's
`ImmediateDisposal`) and starts it again on the newly named object by §5.5 steps 1–4,
replacing its stored handle and owner ID.

In this install the field holds the PID 0x10006F25 in 41,344 nodes, an integer property rather
than an object reference, and 0 in 10 (OBS). Unless an entity carries that property with the
ID of an existing object as its value, the effect stays on the script's entity.

---

<a id="s6"></a>
## 6. Attaching a running effect to its owner

<a id="s6-1"></a>
### 6.1 The attachment matrix (BIN)

When an attached effect is created (§5.5 step 6), the client builds a 4 × 4 **attachment
matrix** *M*ₐ and a bone index:

| Node fields | Bone index | *M*ₐ |
|---|---|---|
| `BoneType` ≠ 0 | the bone of that bone type on the owner's skeleton | identity rotation, translation = `BoneOffset` |
| `BoneType` = 0, `HoldingLocation` ≠ 0 | the holding location's bone (`skeleton-anim.md`) | the holding location's frame as a matrix, with `BoneOffset` added to its translation (not rotated by the holding location's rotation) |
| both 0 | none (the owner's model frame) | identity rotation, translation = `BoneOffset` |

`BoneOffset` is therefore in the space of the bone (or of the owner's model when there is no
bone), in metres, before the owner's render scale.

<a id="s6-2"></a>
### 6.2 Each frame (BIN)

Each time the owner's effects component updates its attachments (it is called from five
places in the owner's frame and pose updates, each passing the owner's render scale; which
events trigger them was not traced), for each attachment:

1. Local matrix *L* = *M*ₐ when there is no bone index; otherwise *M*ₐ multiplied by the
   bone's current model-space matrix of the pose (row vectors: *M*ₐ first).
2. If the attachment's `RemoveRotation` byte is set, the rotation part of *L* is replaced by
   the identity; its translation is kept. The effect then follows the bone's position but not
   its rotation; it still turns with the owner (step 3).
3. The system's frame is *L* followed by the owner's world frame. Its scale vectors are set by
   §6.4.

<a id="s6-3"></a>
### 6.3 What follows the owner

A world-space system (`world_space` = 1) is moved by these frames only at spawn: its particles
keep their world positions (`particles.md` §8.6.3). A local-space system draws its whole
particle set through the current frame, so it follows the owner completely
(`particles.md` §8.16). Emitters with their own `holding_location` or `bone`
(`particles.md` §8.5.5) attach their particles to the owner a second time, inside the system.

<a id="s6-4"></a>
### 6.4 The owner's two scale vectors (BIN; answers `particles.md` Q4)

Every draw instance carries the two vectors *A* and *B* that `scale_mode` combines
(`particles.md` §8.6.3):

- ***A*** = the owner's **render scale**, the per-axis scale the client draws the owner's
  model with (`setup.md` §5.2);
- ***B*** = (*p*, *p*, *p*), *p* = the owner's `Model_ParticleScale` (PID 0x5A0, a float; the
  model component serves the template's `value_b` under that label, `setup.md` §5.2). When the
  property is absent or not a float, *p* = 1.

So `Total` scales effects by the render scale times the particle scale, `VDescOnly` by the
particle scale alone, `None` not at all. Attached effects receive both vectors each time their
frame is updated; dropped effects receive them once (§5.6); lingering effects keep the last
ones (§7.2).

---

<a id="s7"></a>
## 7. Stopping, lingering and removal

<a id="s7-1"></a>
### 7.1 Stopping an attached effect (BIN)

A never-ending effect is stopped when its node instance is stopped (`script-runtime.md` §3.5:
the end of its script, a case change, an entity removal, and so on). Finite effects have no
instance and are never stopped by their script.

To stop it, the owner's effects component removes the handle from its pending list and, if the
effect is attached, removes the attachment. If `ParticleScript_ImmediateDisposal` = 1 it first
marks the system for **immediate disposal**. It then releases its reference (§7.2).

<a id="s7-2"></a>
### 7.2 The release rule (BIN)

When the last reference to a system is released (a stop, a finished attachment, an entity's
component going away; every path ends here):

1. The system leaves the shareable list.
2. It **lingers** when all of these hold:
   - its record is loaded;
   - it had no draw instance in the current frame, or its first instance's opacity is at least
     0.0002;
   - it is not marked for immediate disposal;
   - it is not of the shareable kind (`shareable` = 1, `world_space` = 0, `use_lighting` = 0,
     `particles.md` §8.16): a shared system never lingers;
   - it is registered with a scene that is still active (one further test of the scene was
     not traced).
3. If it lingers:
   - a local-space system that had at least one draw instance this frame keeps the first
     instance's transform, scale vectors and opacity as its fixed placement;
   - a world-space system keeps its own last frame;
   - a local-space system with no instance this frame does **not** linger after all.

   A lingering system is **stopped at now** (it records the stop time and from then on runs no
   emission, `particles.md` §8.1), registered with the scene and added to the lingering list
   (§7.3). Its particles go on moving, fading and dying.
4. Otherwise it is destroyed at once: its particles disappear.

<a id="s7-3"></a>
### 7.3 The lingering list (BIN)

Once per frame (§3.2 step 1) every lingering entry is removed (and its system destroyed) when
either:

- it has been stopped and more than **6 s** have passed since the stop time; or
- its end-of-effect test passes (`particles.md` §8.2 for 0x39, §6.5 for 0x44).

So a stopped never-ending effect disappears 6 s after its stop, even when its particles live
longer (1,044 emitters of never-ending 0x39 records have a constant `lifespan` above 6 s, up to
240 s, OBS); its particles that die earlier are simply gone. A dropped effect is never stopped,
so only its end test removes it; every dropped node of this install names a finite effect
(§5.2). Tearing a scene down also removes the lingering entries registered with it (BIN; that
the caller is the scene's teardown is Inferred).

<a id="s7-4"></a>
### 7.4 Housekeeping of attached effects (BIN)

Each effects component keeps the time of its last check. When more than **2 s** have passed
since then, it records now as the new check time and removes every attachment whose system has
**finished**: stopped more than 6 s ago, or its end test passes. The removal releases the
system (§7.2); since a finished system's end test already passed, a lingering entry made from it
goes at the next sweep (§7.3). A finite attached effect therefore disappears at the first check
after its end test passes, up to 2 s later; its particles are all dead by then (the end test
includes the lifetime).

<a id="s7-5"></a>
### 7.5 Effects stopped before their record loads (BIN)

A request whose handle is stopped while its record is still loading is removed from the pending
list; when the record arrives, the system made for it is discarded and nothing appears.

---

<a id="s8"></a>
## 8. Simulation

The simulation of a running 0x39 system is `particles.md` §8 without change: update cadence
and interpolation (§8.1), window and end test (§8.2), timed and distance emission (§8.3–§8.4),
spawning (§8.5), channels (§8.6), controllers (§8.7), forces (§8.8), fade envelope (§8.9),
atlas frames (§8.10), primitives (§8.11), billboards (§8.12), distance fade (§8.13), lighting
(§8.14), order and passes (§8.15), sharing (§8.16), start and restart (§8.17). This spec adds:

- the clock and the random source (§3);
- that a **stopped** system (§7.2) runs particle updates but no emission;
- the storage order of particles (§9.2), which decides the draw order inside an emitter;
- when a channel value is drawn, the age is now − the particle's spawn time at the draw (the
  clock of the draw, not of the last particle update), while the key that is current is the one
  reached at the last particle update (BIN).

---

<a id="s9"></a>
## 9. Drawing one system

<a id="s9-1"></a>
### 9.1 Draw calls and material (BIN)

For each draw instance of a 0x39 system that passes the distance test (`particles.md` §8.13)
the client writes the vertices of all emitters into the shared draw buffer (`particles.md`
§8.11: `Quad` emitters first, then `Triangle` emitters) and issues at most two draw calls:

1. the `Quad` vertices as an indexed triangle list, two triangles per quad (0, 1, 2) and
   (0, 2, 3) through a shared quad index buffer;
2. the remaining (`Triangle`) vertices as a non-indexed triangle list.

Both use the system's **loaded surface record** (`particles.md` §4.1) and its material and
shader as `material.md` describes them, with the instance's transform (`particles.md` §8.16;
the frames of local-space systems are `particles.md` Q8), a colour multiplier (1, 1, 1, instance
opacity) and the global clock as the material's time. The draw call receives no blend, depth or
cull state from the particle code (BIN: its arguments are the buffers, counts, the material and
shader of the surface record, the transform, the colour multiplier and the clock), so **these
states are those of the surface record's material** (`material.md` §7). Vertex colours carry
the particle colour of `particles.md` §8.6.2. The second call passes one more flag value
(0xC000) whose meaning was not traced (Q-RT6).

<a id="s9-2"></a>
### 9.2 Order of particles inside a system (BIN)

- Each emitter stores its live particles in one array, in creation order: a new particle is
  appended; when a particle dies the **last** particle of the array moves into its slot, and the
  next particle checked is the one now in that slot.
- Vertices are written emitter by emitter (in stored order, `Quad` emitters before `Triangle`
  emitters) and, inside an emitter, in array order. There is **no depth sort** of particles.

So after deaths the draw order inside an emitter is no longer creation order (example in
§14.3). Between systems the order is `particles.md` §8.15.

> **OPEN:** (implementer, 2026-10-08) Frames of local-space systems are `particles.md` Q8.
> `crates/particles` (`sim.rs`, `draw.rs`) uses this Tessera choice until it is answered: a
> local-space system simulates in its own frame (the identity at the origin: spawn, direction
> and gravity are not turned by the owner), and each draw instance moves the whole particle
> set by its transform, scaled per axis by *S*, with the camera axes turned into the
> instance's frame for the billboards. The two anchors of distance emission (`particles.md`
> §8.4) follow the world position and orientation of the frame's first instance, also for
> a local-space system, and each offset from a stepped anchor to the current one (and the
> `Line` vector between the stepped anchors) is turned back into the system's own frame by
> that instance's rotation (not divided by *S*, so the drawn trail scales with *S* like the
> particles); a moving owner therefore leaves a trail. (Corrected 2026-10-09 after
> verification: the first implementation kept the anchors in the identity frame, so a
> local-space system never emitted by distance, although this note said otherwise.) A
> world-space system simulates in world space with the frame of the first instance of
> the frame and is drawn once, for the first instance that passes the distance test (§4.1 of
> `particles.md`: "drawn once"). `use_lighting` multiplies by an ambient colour only (white
> unless the caller supplies one); the point lights wait for Q8 (b). Which of these the
> client does is the question.

<a id="s9-3"></a>
### 9.3 Culling, LOD and caps

- **Distance:** per instance, `particles.md` §8.13 (far and near fade, not drawn beyond the far
  end; a system with no drawn instance freezes).
- **Client options:** `particles.md` §7.1.2 (`DynamicParticles`, `StaticEnvironmentalObjects`)
  and §8.8.1 (`PrecipitationEffects`). No other particle quality or density option was found:
  the request, update and draw paths read none (BIN).
- **Caps:** per emitter `max_particles` and `emission_limit` (`particles.md` §8.3); per system
  draw 32,768 vertices (`particles.md` §8.11). No global particle count is kept or limited (BIN:
  the creation, update and draw routines read none).

> **OPEN (Q-RT3):** Whether the record's bounds (`has_bounds`, `bounds_min`, `bounds_max`,
> `particles.md` §4.1) are used for view-frustum culling. The system exposes them to the scene
> code, but the scene's culling of particle systems was not traced. **Decision (Tessera):** the
> simulation never depends on the view frustum; the renderer may skip drawing an instance when
> `has_bounds` = 1 and the bounds, transformed by the instance and expanded by the largest drawn
> particle size, lie outside the frustum. With `has_bounds` = 0 an instance is never frustum-culled.

---

<a id="s10"></a>
## 10. Fork effects at run time

<a id="s10-1"></a>
### 10.1 Same life cycle (BIN)

A fork effect (prefix 0x44) goes through the same Particle-node start (§5), attachment (§6) and
stop, linger and removal (§7) as a 0x39 effect, with these differences: it is never-ending when
its `flag` = 1 (§5.3); its fade comes from the node on the attached path (`particles.md` §6.5);
it is always local-space, never shared, and its end test is the `tail_value` rule of
`particles.md` §6.5. Its simulation is the third-party library's (`particles.md` Q1). Nested
effects (§6.3 of `particles.md`) start and stop with their parent effect (`particles.md` §6.5).

<a id="s10-2"></a>
### 10.2 Shader by blend-mode index (BIN)

When the client creates a fork system it fetches the seven shader records of `particles.md`
§6.4 from UNIQUEDB. Each draw batch the library produces carries a **blend-mode index**; the
client draws the batch with:

| Index | UNIQUEDB key (label) |
|---|---|
| 0 | 0x15C (`…_Normal`) |
| 1 | 0x15E (`…_Emissive`) |
| 2 | 0x15F (`…_Subtractive`) |
| 3 | 0x15D (`…_DarkMap`) |
| 4 | 0x159 (`…_LightMap`) |
| 5 | 0x15A (`…_Emissive2`) |
| 6 | 0x15B (`…_Emissive3`) |
| any other | nothing is drawn |

For indices other than 2 the client binds textures of the effect, looked up by name among the
render surfaces of the record's `resources` (`particles.md` §6.4), to the shader's texture
parameters once, the first time the batch is drawn; for index 2 it skips that step. Where the library takes the index from, and the batch primitives, are
part of `particles.md` Q1.

> **OPEN:** (implementer, 2026-10-08) Without the third-party library (rule §10.2 of
> `particles.md`), `crates/particles` (`fork.rs`, `runtime.rs`) counts a fork effect as
> active until the `tail_value` rule of `particles.md` §6.5 expires it (the "library reports
> no live particles" exit is never taken), and, since a fork effect is local-space and Tessera
> gives it no draw instance, the release rule of §7.2 step 3 destroys it at once instead of
> letting it linger. Both follow from drawing nothing; confirm or replace when Q1 is answered.

---

<a id="s11"></a>
## 11. Engine-started systems

- **Weather:** the ten UNIQUEDB systems are created synchronously (loaded at once, not through
  a pending request) when the weather code starts, kept for its life and driven by it
  (`particles.md` §7.1.3, §8.8.1; schedule OPEN there, Q13).
- **Beams:** a Beam node copies its fields into a beam object of the client's beam renderer,
  which creates its particle systems synchronously.

> **OPEN (Q-RT4):** How the beam renderer places and drives `BeamScript_BeamParticleSystem`
> (with `BeamScript_BeamSegmentParticlesPerSecond`) and `BeamScript_HeadParticleSystem` (with
> `BeamScript_HeadParticlesPerSecond`), and whether the start gating applies. **Decision
> (Tessera):** until specified, beams draw no particle systems.

> **OPEN (Q-RT5):** The gameplay event handler of §5.1 (which event, which property names the
> effect). **Decision (Tessera):** not implemented; it belongs to a gameplay spec.

---

<a id="s12"></a>
## 12. Tessera rules (Decision)

1. **Random source.** Implement the generator of §3.3 as one shared stream for particle code and
   mode-3 curves, seeded from the wall-clock seconds at start-up. Exact sequences cannot match
   the original (the stream is shared with unrelated code, `particles.md` §10.3), so tests use a
   fixed seed and check distributions, not values. A test or tool may give a system its own
   seeded stream.
2. **Stored classification.** Use the node's stored byte (§5.3); compute the rule only for
   nodes without one (synthetic data) or when writing. Do not "repair" stored bytes.
3. **Finite effects started at low intensity** are skipped (§5.4 step 3); intensity does not
   scale effects.
4. **Dropped effects** follow §5.6 exactly, including the doubled offset and the absence of
   start gating.
5. **Lingering:** follow §7.2–§7.4, including the 6-s cut and the 2-s housekeeping; the time
   points are taken from the global clock.
6. **Hostile classification input:** a 0x39 record with `burst_count` = 0 in an emitter that
   has `emission_limit` ≠ 0 would make the client divide by zero in §5.3; Tessera treats that
   emitter's term as `time_limit` alone (consistent with `particles.md` §10.6).
7. **Hostile dropped effects:** a never-ending effect on the dropped path (never in the data,
   §5.2) would never be removed by the client except with its scene; Tessera removes such an
   entry 6 s after it is placed, as if it had been stopped at once.

> **OPEN:** (implementer, 2026-10-09) Rule 7 has two readings. `crates/particles` takes the
> removal time only: the effect is placed without being stopped, emits normally and is removed
> at the first sweep more than 6 s after its placement (so it shows its first 6 s). Read
> literally ("stopped at once"), the effect would be stopped at placement and, since a stopped
> system emits nothing, would show nothing at all. Which reading is meant?
8. Q-RT1 to Q-RT6: the Decisions given in their callouts.

> **OPEN:** (implementer, 2026-10-08) Engineering limits the spec does not set, chosen
> conservatively in `crates/particles` (`SimConfig`) for hostile records; the data never
> reaches them (the real-data run of all 11,155 records over 5 s peaks at 1,320 live
> particles in one system): at most 262,144 live particles per system (all emitters
> together; spawning then stops as if every emitter were full), at most 64 controller-4 steps
> per particle and update (a positive `time_step` far below 1/120 s would otherwise make the
> 0.25 s catch-up loop unbounded; rule §10.5 of `particles.md` covers only non-positive and
> non-finite values), and at most 1,024 legs of a controller-1 walk per particle and update
> (points that repeat make the squared-distance walk of `particles.md` §8.7 land on zero-length
> legs without consuming the budget). Mode-3 curves draw their *r* as one real-range draw over
> [−1, 1] (§3.4). Should these limits be recorded as Tessera rules?

---

<a id="s13"></a>
## 13. Invariants

Against the install (all hold in this install, OBS) or synthetic fixtures:

- **RT-I1:** for every Particle node, the stored extra byte equals the classification of §5.3
  computed from the record its effect DID names (41,354 of 41,354).
- **RT-I2:** every node with `ParticleScript_DropEffect` = 1 has stored byte 0.
- **RT-I3:** the generator of §3.3 seeded with *n* > 0 produces the same outputs as the
  published ran2 initialised with −*n* (same algorithm and constants); checkable against an
  independent implementation of the published algorithm over the first 10,000 draws.
- **RT-I4:** a real-range draw with *a* = *b* consumes no number of the stream (§3.4).
- **RT-I5:** a stopped lingering system with no end condition is removed at the first sweep
  more than 6 s after its stop time; with `ImmediateDisposal` = 1 it is never in the lingering
  list.
- **RT-I6:** a system of the shareable kind is never in the lingering list.
- **RT-I7:** with `RemoveRotation` = 1 the rotation part of the attachment's local matrix is the
  identity for every pose.
- **RT-I8:** inside one emitter, the vertex order of a frame equals the array order of §9.2;
  after the death of particle *k* of *n*, the former last particle takes position *k*.
- **RT-I9:** an attached finite effect is removed at most 2 s (plus one frame) after its end
  test first passes.

---

<a id="s14"></a>
## 14. Worked synthetic example: one emitter over 2 s

All values are invented for testing; they are not game data.

<a id="s14-1"></a>
### 14.1 Set-up

- **System:** `time_step` 0.1, `fast_forward` 0, `physics_duration` 0, `world_space` 1,
  `use_lighting` 0, far fade 40–60 m, near fade 0, `scale_mode` 1 (`Total`), `sorting_priority`
  5, `shareable` 0, a loaded surface.
- **Emitter:** `shape` 4 (`Point`), `primitive` 2 (`Quad`), `emit_interval` 0.25, `speed`
  constant 2.0, `direction` (0, 0, 1), spread 0, `emission_limit` 4, `max_particles` 10,
  `burst_count` 2, `lifespan` constant 1.0, `particle_scale` constant 1, rotation 0, `streak` 0,
  `start_time` 0.25, `time_limit` 0, `emission_distance` 0, `active` 1, `fade_in` 0.5, `fade_out`
  −1, 1 × 1 frames, no holding location or bone.
- **Keys:** K0 = (time 0, flags 0x770, scale 0.2 × 0.2, colour 0xFFFFFFFF, mass 1.0, controller
  2, physics flags 0x1). K1 = (time 1.0, flags 0x6, scale 0.6 × 0.6, colour 0x00FF0000, mass 1.0,
  controller 2, physics flags 0).
- **Owner:** at the origin with identity orientation, render scale (1, 1, 1),
  `Model_ParticleScale` absent (*p* = 1), drawn every frame at opacity 1, camera 10 m away.
- **Node:** `ParticleScript_ParticleID` = this record, `DropEffect` 0, no holding location,
  bone or offset, `ImmediateDisposal` 0, `RedirectProperty` 0, context intensity 1.
- **Frames:** the client clock reads 10.00 at the first placement and advances 0.04 s per frame
  (10.00, 10.04, …), so no event falls exactly on a frame.

<a id="s14-2"></a>
### 14.2 Start

- §5.3: no emitter has both limits 0; *v* = max(0, (4 div 2) × 0.25) + 1.0 + 0.25 − 0 = 1.75 ≥ 0,
  so the effect is **finite** and the node's stored byte is 0. §5.4: intensity 1 > 0.0002, so it
  starts; §5.5: the node completes at once (no instance).
- First placement at 10.00: system start *S* = 10.00; last particle-update, last-burst and
  last distance-emission times = *S* + 0.25 = 10.25 (`particles.md` §8.17).
- End test (`particles.md` §8.2): the system has ended once now > 10.00 + 0.25 + 4 × 0.25 / 2 +
  1.0 = 11.75.

<a id="s14-3"></a>
### 14.3 Timeline

| Frame (clock) | What happens |
|---|---|
| 10.00–10.24 | Emitter time < 0: outside the window; nothing. |
| 10.28 | Timed emission: the last burst (10.25) is before now, so 2 particles P1, P2 are spawned **at 10.25**; total 2; last burst → 10.50. Each new particle is advanced to the last particle-update time, 10.25 (age 0, position (0, 0, 0)). No particle update (0.03 s since 10.25 < 0.1). |
| 10.36 | Particle update (0.11 ≥ 0.1): P1, P2 at age 0.11; last update → 10.36. |
| 10.48 | Particle update: ages 0.23; last update → 10.48. |
| 10.52 | Spawn P3, P4 **at 10.50**, advanced to 10.48: age −0.02, so controller 2 places them at *z* = 2 × (−0.02) − 4.9 × 0.0004 = −0.04196 m. Total 4 = `emission_limit`; last burst → 10.75. |
| 10.56 | Drawn (see §14.4). |
| 10.60, 10.72, … | Particle updates every 0.12 s (every third frame). |
| 10.76 and later | Last burst 10.75 is before now, but cap = min(`emission_limit` 4, 10) = 4 and 4 particles live: room 0, so the last-burst time is set to now. After deaths room > 0, but the burst is min(2, 4 − 4) = 0 < 1: no emission ever again. |
| 11.32 | Update: P1, P2 at age 1.07 > 1.0 die. Array [P1, P2, P3, P4] → P4 moves into slot 0, then P2's slot takes P3: [P4, P3]. Draw order is now P4, P3 (§9.2). |
| 11.56 | Update: P3, P4 at age 1.06 die. |
| 11.76 | First frame with now > 11.75: the end test passes. |
| next 2-s check | Housekeeping (§7.4) removes the attachment; the release (§7.2) makes a lingering entry that the next sweep removes (§7.3) because its end test passes. |

<a id="s14-4"></a>
### 14.4 Drawn values at clock 10.56

Interpolation factor *f* = (10.56 − 10.48) / 0.1 = 0.8 (`particles.md` §8.1). Emitter time
0.31, so the fade envelope is 0.31 / 0.5 = 0.62 (`particles.md` §8.9). *m* = 1 (*A* = *B* =
(1, 1, 1)). Motion: controller 2, *z*(*τ*) = 2*τ* − 4.9*τ*² (gravity 9.8 × mass 1).

| Particle | Age at the draw | Previous / current *z* (at updates) | Drawn *z* | Width = height | Colour (R, G, B, A) | Drawn alpha |
|---|---|---|---|---|---|---|
| P1 (and P2) | 0.31 | 0.16071 (age 0.11) / 0.20079 (age 0.23) | 0.16071 + 0.8 × 0.04008 = 0.19277 m | 0.2 + 0.31 × 0.4 = 0.324 m | (1, 0.69, 0.69, 0.69) | 0.69 × 0.62 = 0.4278 |
| P3 (and P4) | 0.06 | −0.04196 / −0.04196 (no update since creation) | −0.04196 m | 0.224 m | (1, 0.94, 0.94, 0.94) | 0.94 × 0.62 = 0.5828 |

Colour and scale interpolate from K0 towards K1 (K1 has bits 0x2 and 0x4); mass stays 1.0 (K1
lacks 0x8). The quads face the camera (`particles.md` §8.12, default corners).

<a id="s14-5"></a>
### 14.5 Variants

- **Never-ending:** `emission_limit` 0 → stored byte 1; the node keeps an instance. If the
  script stops at clock 11.00 with `ImmediateDisposal` 0, the system lingers (world-space, §7.2
  step 3): no emission after 11.00, live particles continue, and the entry is removed at the
  first sweep after 17.00 (6 s). With `ImmediateDisposal` 1 every particle disappears at 11.00.
- **Intensity 0:** the finite original is not started at all (§5.4 step 3); the never-ending
  variant is.
- **Dropped:** `DropEffect` 1 with `BoneOffset` (0, 0, 1): the effect is placed once at about
  (0, 0, 2) (offset twice, §5.6) and stays there if the owner moves; removed as in §14.3 after
  11.75.
- **Owner scale:** render scale (2, 2, 2) and `Model_ParticleScale` 0.5: `Total` gives *S* =
  (1, 1, 1) and *m* = 1; `VDescOnly` gives *m* = 0.5 (width 0.162 m at clock 10.56 for P1);
  `None` gives *m* = 1.

---

<a id="s15"></a>
## 15. Open questions

> **OPEN (Q-RT1):** Per-frame order and culled owners (§3.2). Decision there.

> **OPEN (Q-RT2):** Source of context flag 0x40 (§5.4). Decision there.

> **OPEN (Q-RT3):** Frustum culling by the record's bounds (§9.3). Decision there.

> **OPEN (Q-RT4):** Beam-driven particle systems (§11). Decision there.

> **OPEN (Q-RT5):** The gameplay event handler that places an effect at the local player
> (§5.1, §11). Decision there.

> **OPEN (Q-RT6):** The meaning of the flag value 0xC000 that the `Triangle` draw call of §9.1
> passes and the `Quad` call does not. **Decision (Tessera):** draw both lists with the same
> pipeline state.

Still open in `particles.md`: Q1 (fork effect image and simulation; the blend-mode index source
of §10.2), Q2, Q8 (local-space frames, light attenuation, streak in `world_rotation` mode), Q13
(weather schedule). `particles.md` Q4 is answered by §6.4 here.

---

<a id="s16"></a>
## 16. Notes for other specs

- `particles.md` §13 Q4: answered by §6.4 (*A* = render scale, *B* = `Model_ParticleScale`).
- `particles.md` §8.17, last bullet: the per-emitter maximum starts at 0, so the "largest is
  negative" case cannot occur (§5.3); reviewer note (c) of its review log is answered there too
  (inactive emitters count).
- `script-table.md` §4.1: the Particle node's extra byte (`particle_flag`) is the stored
  never-ending classification (§5.3).
- `script-runtime.md` §3.1 / kind table: the Particle node's start, instance and stop hook are
  §5.4, §5.5 and §7.1 here; context flag 0x40 is read by it (Q-RT2).
- `setup.md` §5.2: "how an effect combines the two is for the particle specs" is §6.4 here.
- `material.md` §5.3.1: the "client's global random generator" of mode 3 is §3.3 here.

These are suggestions for the owners of those specs; this spec edits none of them.

---

<a id="s17"></a>
## 17. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-09 | §3.2, §5.4, §9.2, §12 | Implementer OPEN notes only (clean room): the walker's owners and "drawn" (§3.2), its choices for redirect, gating, driven fields, opacity, water and scales (§5.4), the corrected distance-emission note of §9.2, the two readings of rule §12.7. No fact changed. |
| 2026-10-08 | all | First version, status Review. Run-time behaviour from a third M4 pass (row BIN) and a census of all 41,354 Particle nodes (OBS): entry points, clock and random generator, the stored never-ending byte, node start gates, attached and dropped paths, water probe, redirect, attachment frames, the owner's scale vectors, stop / linger / removal, particle storage order, draw calls, fork blend-mode table; worked example; Q-RT1–Q-RT6. |
