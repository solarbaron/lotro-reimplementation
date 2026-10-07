<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M4.5 visual and collision checklist (exit criteria 4, 5 and 6)

| | |
|---|---|
| **Status** | Expected values complete (2026-10-06); scenery rows S1–S11 and walks GN1–GN4 re-derived 2026-10-07 ([§8](#s8)); walker side and owner observations pending |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Review record** | Claude review session (spec-reviewer agent), 2026-10-07, of the re-derivation of 2026-10-07 (§1 GEN row, §2.3–§2.4 GN1–GN4, §4.2 rows S1–S11 and callout, §4.3 intro, §8): **passed.** The rule the rows now follow (`scenery.md` §7.4 step 14b, S36) was re-checked in a separate copy of the Ghidra project. The totals of all 11 landblocks and the list indices of GN1–GN4 (16, 33, 176, 138) agree with the clean-room implementation's generated lists in the writer's scratch output, as do GN1's new tree and position. No game text, keys or code. The companion `scenery.md` revision has changes requested on §7.4 step 15 only (failure causes and frill-layer containment), which no row of this checklist depends on. Second review, Claude review session (spec-reviewer agent, a different session), 2026-10-07: **passed again.** S36 re-checked in a fresh copy of the Ghidra project. Independently of the writer's output, a fresh read-only build of the repository's crates generated the 11 landblocks of §4.2 on the install: totals 25, 315, 165, 309, 151, 111, 445, 166, 221, 80 and 186 as listed; GN1–GN4 at 0-based list indices 16, 33, 176 and 138 with the listed DIDs, positions, headings and scales; the changed nearest objects of S1, S2, S5, S6, S8 and S10 at the listed positions (S8's second at x 126.421, within rounding). The in-view counts and the frill rows were not re-measured. The open issues of `scenery.md` §7.4 step 15 still do not affect any row. |
| **Serves** | [ROADMAP M4.5](../ROADMAP.md#m4-5-exit) exit criterion 4 (scripted walks on collision and the 1,000-walk soak, [§2](#s2)), criterion 5 (five performance walk spots, [§3](#s3)) and criterion 6 (visual rows, [§4](#s4)); the M3 reference-location check asked for by criterion 1 ([§5](#s5)) |
| **Companion specs** | [`collision.md`](../specs/collision.md) §5–§6, §9–§12; [`scenery.md`](../specs/scenery.md) §4–§8; [`water.md`](../specs/water.md) §6–§7; [`terrain-texturing.md`](../specs/terrain-texturing.md) §3–§4, §8, §10; [`environment.md`](../specs/environment.md) §5–§7, §11; [`lighting.md`](../specs/lighting.md) §8–§9; [`landblock.md`](../specs/landblock.md) §3–§6; [`coordinates.md`](../specs/coordinates.md) §5, §7, §9, §11 (normative) |
| **Owner list** | Items [O18–O27](owner-observations.md#o18) of the owner list compare these rows with the original client |

Places in all six regions with landblock data (1, 2, 3, 4, 5 and 14) where the data
predicts exact facts: where a walker is stopped, what it climbs, which cell it is in, where
it starts to wade and to swim, which terrain layers, generated objects, frills, water, sky
values and lights a renderer must show. Tessera's walker is checked against the
"expected" columns; the owner compares the original client with the same rows.

Text only. No game text: areas, objects and terrain types are named by DID and described
in our own words. No screenshots, captures or extracted data are committed; screenshots of
either client stay private in a scratch directory (same rules as
[`m3-reference-locations.md`](m3-reference-locations.md#s4)).

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | The owner's install, data current on 2026-10-06, read in place and read-only by throwaway dirty-room probes. Records decoded for this file: landblock data, block maps, landblock info and landblock properties of regions 1–5 and 14 (all records); 0x66 (all 16,819); 0x01 (all 1,057); 0x02, 0x11, 0x12, 0x19 (all); 0x16; 0x15000007, 0x1A (all 963), 0x35000000; the block-map descriptors 0x18; 0x31/0x30/0x2B for the lighting rows. |
| GEN | A dirty-room re-implementation of `scenery.md` §6–§7 (static pass) and §8.2 with the Tessera Decision of §8.6 (frill pass), written from the spec text only. It reproduces the synthetic worked example of `scenery.md` §14 exactly (vertex hash 0x7237E00C, the frequency, displacement, heading and scale draws, both quaternions). It reads the landblock's statics and their clearance volumes from landblock info (`scenery.md` §7.4 step 14a). **Corrected 2026-10-07:** until then its footprint test (`scenery.md` §7.4 step 14b) was inverted: it tested only the open markers and skipped the closed ones. The corrected generator follows step 14b, which a BIN re-check confirmed unchanged. Its static-pass lists equal those of the clean-room implementation object for object in all 11 landblocks of [§4.2](#s4-2). Its frill pass keeps the static pass's markers (`scenery.md` §8.2 item 5). The frill records of the six landblocks of [§4.3](#s4-3) are identical under the old and the corrected generator. Rows re-derived: [§8](#s8). |
| SWEEP | A geometric sweep of the Human male capsule (`collision.md` §10.6: radius 0.35 m, cap centres 0.35 m and 1.45 m above the feet) along a straight horizontal path, with the feet on the terrain (`landblock.md` §3.11) or on the highest walkable surface no more than the step-up height (0.5 m, BasePlayer, `collision.md` §10.3) above the current feet. A contact counts as blocking only when it lies above the step-up height. The collision set is `collision.md` §12.1: all terrain triangles, the 0x66 meshes of every blocking SED (§9.3) and every blocking generated object (§9.4) of the landblock and its eight neighbours, and, for interiors, the cell physics meshes (§5.3) and interior SEDs. Not Tessera's controller: it gives the geometric fact the controller must reproduce. |
| CELL | The point-in-cell rule of `collision.md` §5.4 and §5.8.1 (tolerance −0.0002), evaluated at the feet raised by 0.05 m. Where two cells contain the point, both are listed. |
| WAT | `water.md` §6.2 (landblock water), §7.1 (volume entity) and the Tessera rule of §7.4. |
| ENV | `environment.md` §11.1–§11.3 with the day type *D* = 2 (the Tessera initial value, §11.2) and game day *n* = 12,240 (the day at clock *G* = 0, `environment.md` §14). |
| MAT | `material.md` §9.2 (effective parameters), §11.3 (blend family), §11.5 (emissive, glow, specular flags). |

| # | Fact used by this checklist | Sources | Mark |
|---|---|---|---|
| F1 | Heights: stored × 2,000 / 65,535 in single precision; terrain under (x, y) by the triangulation of `landblock.md` §3.11 | `landblock.md` §3.4, §3.11 | Confirmed (spec) |
| F2 | Mover: Human male capsule radius 0.35 m, height 1.80 m; step-up 0.5 m; walkable when the support normal's z exceeds cos 45°, sliding up to 85° | `collision.md` §10.3, §10.6, §11.3 | Confirmed (spec) |
| F3 | Speeds of the default avatar: run (BasePlayer forward) 6.5 m/s, walk (Walking layer) 5 m/s, in water (Swimming layer) 3.2 m/s | `collision.md` §10.3, §11.1 | Confirmed (data) |
| F4 | Float height of the default avatar (`human,m`): 1.42 m; wading while the water surface is at most that far above the feet, swimming beyond | `collision.md` §10.5, §11.2; `water.md` §7.4 | Confirmed (data); the switching rule is a Tessera Decision |
| F5 | A generated object blocks when its effective `Physics_NoPhysics` is not true, its setup has a 0x66 mesh or volumes and it is not ethereal to Player | `collision.md` §9.4 | Tessera Decision (spec) |
| F6 | Undrawn blocking SEDs: effective `Render_Draw`, `Render_DrawMeshes` or `Render_NeverAddToScene` hide the object; collision ignores the draw flags | `collision.md` §9.3; `entity-descriptor.md` §9 | Confirmed (spec) |
| F7 | Cells are entered by containment; portals link cells to cells | `collision.md` §5.8, §5.8.1 | BIN (spec) |
| F8 | Sky key per landblock = block-map key 0x1000000F through the region's 0x18 descriptor palette | `environment.md` §7.1; `landblock.md` §5.4 | Confirmed |

<a id="s1-1"></a>
### 1.1 Census facts found while choosing the spots (OBS, 2026-10-06)

These facts shaped the rows and are new; they are recorded here because no spec states them
yet.

1. **No interior cell can be walked into from the landscape.** Of the cells whose hull
   overlaps the square of their landblock, only cells of 0x8021FAF9 and 0x8021FAFA have a
   vertical face whose outside point (1.5 m out, at the face centre) lies within 0.6 m of
   the terrain at the cell's floor height (61 faces). Both landblocks are flat at height 0
   and have no terrain layers (nothing drawn): modular rooms laid out on an undrawn floor.
   Within 4 m of the floor the count is 233 faces in 19 landblocks, every one of them a
   block whose X or Y is 0xF3 or more (1,433 landblocks have cells). Inferred: interior
   cells are reached by teleport, not by walking in from outdoors. The interior walks of §2
   start inside a cell and cross a portal (doorway) into the next one. Neighbouring cell
   volumes can overlap at their shared portal, so a point in the doorway can be inside both
   (IN1, IN4; `collision.md` §5.8.1: Tessera takes the lower cell number there).
2. **Stair collision is mostly ramps.** The 0x66 meshes of placed stairs replace the treads
   by sloped faces (33 of the first 39 distinct stair meshes have more sloped than level
   faces). A walk that climbs real risers needs an object with level treads; the step
   walks use a platform-with-steps object (0x47000AE1) whose risers are 0.13–0.32 m.
3. **Day type 2 selects only the local default table.** No day group of 0x15000007 has day
   type 2 (the four groups have 0x10000001, 0x10000008, 0x10000004, 0x1000000B), so with the
   Tessera initial *D* = 2 the sky entry comes from the local default table. 76 of the 477
   sky keys used by the block-map palettes are in a day group but not in the default table,
   and 20 are in neither; with *D* = 2 those landblocks get no day description. The sky rows
   of §4.5 use keys that are in the default table. Owner item [O26](owner-observations.md#o26)
   asks for the day type.
4. **Sun direction is almost always constant within a day description.** In 947 of the
   963 day descriptions every keyframe has the same `sun_heading` and `sun_pitch` (612 of
   the 628 with more than one keyframe). In 0x1A000000, 0x1A000359, 0x1A000213 and
   0x1A000377 (rows Y1, Y3, Y4, L1–L3, L5) they are 15° and 150° (toward-sun vector
   (−0.224, −0.837, 0.5): elevation 30°, from bearing 195°). The time of day changes
   colours, brightness and fog, not the lighting direction; the moving sun and moon discs
   are sky objects (`environment.md` §11.4).

---

<a id="s2"></a>
## 2. Scripted walks (exit criterion 4)

<a id="s2-1"></a>
### 2.1 Walk format

One walk is one row of the table in [§2.3](#s2-3), or one file with the same fields given
to `tessera-client --walk-path FILE`. Both forms carry the same twelve fields.

**Table form.** The walk table is the first Markdown pipe table after the anchor `s2-3`
whose header row is exactly
`| id | category | region | landblock | cell | start | loc | heading | mode | inputs | duration | expect |`.
A parser reads every following row until the first line that does not start with `|`.
Cells are trimmed; a cell wrapped in backticks has the backticks removed.

**File form.** UTF-8 text, one `key = value` per line, keys as the column names, `#`
starts a comment, blank lines ignored. Unknown keys are an error. Example:

```
id = WL1
category = wall
region = 1
landblock = 0xA197
cell = 0
start = 127.81,89.59,389.26
loc = r1 lx1294 ly1212 ox127.81 oy89.59 oz389.26
heading = 0.0
mode = run
inputs = 0:fwd
duration = 3
expect = blocked P=127.81,94.77,389.26 d=0.10
```

**Fields.**

| Field | Grammar | Meaning |
|---|---|---|
| `id` | letters and digits | Unique walk name |
| `category` | `wall`, `undrawn`, `generated`, `step`, `steep`, `interior`, `cave`, `water`, `frill` | The criterion-4 category the walk counts for |
| `region` | decimal | Region *R* (`coordinates.md` §4) |
| `landblock` | `0xXXYY` | The landblock (X, Y) whose frame every coordinate of the row uses |
| `cell` | decimal | Interior cell of the start (`coordinates.md` §7); 0 = outdoors |
| `start` | `x,y,z` | Feet position at the start, block-local metres (`coordinates.md` §5.1, §6). z is the support height there (terrain, object or cell floor). The walker places the capsule's foot point at `start` within 0.02 m and lets it settle for 0.5 s of simulated time before the inputs begin. Interior coordinates may lie outside 0 … 160. |
| `loc` | `/loc` position text | The same start as the original's `/loc` would print it (`coordinates.md` §9), heading included; the owner types `/loc` there to compare. Redundant with `region`, `landblock`, `cell`, `start` and `heading`. |
| `heading` | degrees | Compass heading of the avatar at the start, 0 = north, 90 = east (`coordinates.md` §11.4) |
| `mode` | `walk`, `run`, `jump`, `swim` | `walk`: the Walking layer (`Movement_MovementStyle` 1); `run`: the default (BasePlayer) layer; `jump`: `run` with a jump input at every `jump` token of `inputs`; `swim`: reserved for walks that start in deep water (none in this file). Layers per `collision.md` §11.1. |
| `inputs` | `;`-separated tokens | `T:fwd`: hold forward from time T (s) on; `T:stop`: release forward at T; `T:jump`: press jump at T; `T:turn=H`: set the heading to H at T; `wp=x,y[;x,y…]`: steer: every step set the heading toward the current waypoint, hold forward, switch to the next waypoint within 0.25 m, release forward within 0.05 m of the last one. Times count from the end of the settle period. |
| `duration` | seconds | Simulated time after the settle period, in fixed steps of 1/60 s (`collision.md` §12.3) |
| `expect` | kind and `key=value` list, space-separated | The pass condition ([§2.2](#s2-2)) |

All positions in `expect` are feet positions in the frame of `landblock`. Numbers use a
point as the decimal separator; coordinates are separated by commas without spaces.

> **OPEN (implementer, 2026-10-06, `--walk-path`):** `wp=` does not say whether "release
> forward within 0.05 m of the last one" is decided once or every step. Released once, a
> walker coasts on under the controller's deceleration (about 0.4 m at walking speed, 20 m/s²
> from 4.9 m/s), which no `reach` tolerance covers. Tessera (`walker.md` §13.6) decides it
> every step: forward is held, toward the waypoint, whenever the feet are more than 0.05 m
> from it, so a walker that coasted past is steered back. Please confirm or state the
> intended rule.

> **OPEN (implementer, 2026-10-06 review, `--walk-path`):** §2.1 does not say what `T:stop`
> and `T:turn=H` do in a walk that also has `wp=`: the steering sets the heading and holds
> forward every step, so they would have no effect. Tessera refuses that combination (no
> row of §2.3 uses it); `T:fwd` and `T:jump` are accepted with waypoints (`jump` presses
> jump). Please confirm or state the intended precedence.

<a id="s2-2"></a>
### 2.2 Expected outcomes and pass conditions

Every walk also passes only if, at every step: the capsule penetrates no collision geometry
by more than 0.01 m (criterion 4); position and velocity are finite.

| Kind | Parameters | Passes when |
|---|---|---|
| `blocked` | `P` (stop point), `d` | At the end the feet are within `d` of P horizontally and within 0.05 m vertically, and at no step was the feet's distance along the start heading greater than that of P plus `d`. P is where the SWEEP capsule first touches the blocker above the step-up height. |
| `reach` | `E`, `d`, `risers` | The feet end within `d` of E horizontally and within 0.05 m vertically; `risers` lists the vertical steps (m) of the support profile on the way, in order, each below the step-up height, which the walker climbs without a jump. |
| `noclimb` | `B` (foot of the steep band on the path), `zmax`, `rmax` | The feet never rise above `zmax` (B.z + 0.60 m) and end no farther than `rmax` (distance of B + 0.60 m) from the start, measured horizontally along the path; the state is `Earth_UncontrolledSurface` (6) at least once. The band beyond B is steeper than 50° for at least 3 m (all its triangles between 50° and 85°). |
| `cells` | `A>B`, `E`, `d` | The cell lookup (`collision.md` §5.8.1) reports A at the start and B at the end; the feet end within `d` of E (horizontal) and 0.05 m (vertical). In the doorway both cells contain the feet; a report of either is allowed there. |
| `follow` | `E`, `d`, `dz`, `headroom` | At every step the feet are within `dz` of the terrain height under them (`landblock.md` §3.11) and the walker is never blocked; it ends within `d` of E. `headroom` is the smallest ceiling-minus-floor distance on the path (`landblock.md` §3.5, bit-0 diagonal), for information. |
| `wade_swim` | `in`, `swim`, `h`, `fh`, `d` | The state first becomes a water state (1, 2 or 3) within `d` of `in` (where the terrain under the path first goes below h) and first becomes `Water_NoSurface` (2) within `d` of `swim` (where it first goes below h − `fh`). Once 3 s have passed after `swim`, the feet stay within 0.25 m of h − `fh`. |
| `pass` | `E`, `d` | The walker ends within `d` of E; no object contact at any step; feet within 0.05 m of the terrain at every step. |

Tolerances cover the SWEEP's simplifications (a capsule kept upright, contacts below the
step-up height ignored, 2 cm path sampling). Walks that fail by less than twice their
tolerance are reported to the spec writer, who re-checks the row before the implementation
is changed.

<a id="s2-3"></a>
### 2.3 The walks

32 walks: 5 walls, 3 undrawn blockers, 4 generated objects, 3 steps, 3 steep slopes,
4 interiors, 3 caves, 4 water entries, 3 frill areas. Every region with landblock data has
at least three walks.

| id | category | region | landblock | cell | start | loc | heading | mode | inputs | duration | expect |
|---|---|---|---|---|---|---|---|---|---|---|---|
| WL1 | wall | 1 | 0xA197 | 0 | 127.81,89.59,389.26 | `r1 lx1294 ly1212 ox127.81 oy89.59 oz389.26` | 0.0 | run | `0:fwd` | 3 | `blocked P=127.81,94.77,389.26 d=0.10` |
| WL2 | wall | 2 | 0x6EA5 | 0 | 120.73,60.31,517.46 | `r2 lx886 ly1323 ox120.73 oy60.31 oz517.46 h52.7` | 52.7 | run | `0:fwd` | 3 | `blocked P=124.86,63.45,517.46 d=0.10` |
| WL3 | wall | 3 | 0xA73F | 0 | 130.69,32.49,311.99 | `r3 lx1342 ly505 ox130.69 oy32.49 oz311.99 h270.0` | 270.0 | run | `0:fwd` | 3 | `blocked P=125.98,32.49,311.99 d=0.10` |
| WL4 | wall | 4 | 0xC755 | 0 | 30.26,122.36,155.40 | `r4 lx1593 ly686 ox30.26 oy122.36 oz155.40 h315.0` | 315.0 | run | `0:fwd` | 3 | `blocked P=26.82,125.79,155.40 d=0.10` |
| WL5 | wall | 5 | 0x757A | 0 | 66.39,69.64,228.76 | `r5 lx939 ly979 ox66.39 oy69.64 oz228.76 h180.0` | 180.0 | run | `0:fwd` | 3 | `blocked P=66.39,64.71,228.76 d=0.10` |
| UB1 | undrawn | 2 | 0x3B91 | 0 | 59.64,64.39,578.19 | `r2 lx474 ly1163 ox59.64 oy64.39 oz578.19` | 0.0 | run | `0:fwd` | 3 | `blocked P=59.64,69.04,578.19 d=0.10` |
| UB2 | undrawn | 4 | 0x7CBC | 0 | 78.03,89.18,165.80 | `r4 lx995 ly1508 ox78.03 oy89.18 oz165.80 h225.0` | 225.0 | run | `0:fwd` | 3 | `blocked P=74.74,85.89,165.80 d=0.10` |
| UB3 | undrawn | 14 | 0x1506 | 0 | 36.06,16.63,204.10 | `r14 lx169 ly48 ox36.06 oy16.63 oz204.10 h225.0` | 225.0 | run | `0:fwd` | 3 | `blocked P=32.78,13.35,204.10 d=0.10` |
| GN1 | generated | 1 | 0x669A | 0 | 130.61,77.52,558.62 | `r1 lx822 ly1235 ox130.61 oy77.52 oz558.62` | 0.0 | run | `0:fwd` | 3 | `blocked P=130.61,82.61,559.19 d=0.15` |
| GN2 | generated | 4 | 0x1A23 | 0 | 40.97,40.97,192.11 | `r4 lx210 ly282 ox40.97 oy40.97 oz192.11` | 0.0 | run | `0:fwd` | 3 | `blocked P=40.97,46.19,192.12 d=0.15` |
| GN3 | generated | 5 | 0x655A | 0 | 117.96,37.66,441.31 | `r5 lx813 ly721 ox117.96 oy37.66 oz441.31 h90.0` | 90.0 | run | `0:fwd` | 3 | `blocked P=120.52,37.66,441.69 d=0.15` |
| GN4 | generated | 14 | 0x1FE2 | 0 | 117.03,136.97,435.00 | `r14 lx253 ly1814 ox117.03 oy136.97 oz435.00 h180.0` | 180.0 | run | `0:fwd` | 3 | `blocked P=117.03,131.97,435.76 d=0.15` |
| ST1 | step | 1 | 0x5B6D | 0 | 47.23,59.12,374.46 | `r1 lx730 ly874 ox47.23 oy59.12 oz374.46 h180.0` | 180.0 | walk | `wp=47.23,49.67` | 6 | `reach E=47.23,49.67,375.64 d=0.15 risers=0.26/0.13` |
| ST2 | step | 1 | 0x7C6E | 0 | 98.09,76.44,356.27 | `r1 lx996 ly883 ox98.09 oy76.44 oz356.27 h180.0` | 180.0 | walk | `wp=98.09,66.99` | 6 | `reach E=98.09,66.99,357.50 d=0.15 risers=0.32/0.13` |
| ST3 | step | 1 | 0x0725 | 0 | 138.81,81.08,349.98 | `r1 lx62 ly300 ox138.81 oy81.08 oz349.98 h90.0` | 90.0 | walk | `wp=148.26,81.08` | 6 | `reach E=148.26,81.08,351.19 d=0.15 risers=0.17/0.26` |
| SL1 | steep | 2 | 0xB876 | 0 | 69.88,75.06,552.84 | `r2 lx1475 ly947 ox69.88 oy75.06 oz552.84 h160.9` | 160.9 | run | `0:fwd` | 6 | `noclimb B=71.65,69.96,556.72 zmax=557.32 rmax=6.00` |
| SL2 | steep | 14 | 0x0A04 | 0 | 74.02,54.65,243.65 | `r14 lx83 ly34 ox74.02 oy54.65 oz243.65 h349.1` | 349.1 | run | `0:fwd` | 6 | `noclimb B=72.98,60.05,244.97 zmax=245.57 rmax=6.10` |
| SL3 | steep | 3 | 0x696C | 0 | 34.50,32.40,529.09 | `r3 lx841 ly865 ox34.50 oy32.40 oz529.09 h89.3` | 89.3 | run | `0:fwd` | 6 | `noclimb B=40.00,32.47,532.31 zmax=532.91 rmax=6.10` |
| IN1 | interior | 1 | 0x91A6 | 9 | 40.86,-108.12,-212.90 | `r1 lx1160 ly1328 cInside ox40.86 oy-108.12 oz-212.90 h90.0` | 90.0 | walk | `wp=46.86,-108.12` | 3 | `cells 9>8 E=46.86,-108.12,-212.90 d=0.15` |
| IN2 | interior | 3 | 0x98D6 | 49 | -1.56,-1.01,-221.09 | `r3 lx1216 ly1712 cInside ox-1.56 oy-1.01 oz-221.09 h90.0` | 90.0 | walk | `wp=4.44,-1.01` | 3 | `cells 49>48 E=4.44,-1.01,-221.09 d=0.15` |
| IN3 | interior | 4 | 0xFC85 | 31 | 69.01,-5.38,-93.83 | `r4 lx2016 ly1064 cInside ox69.01 oy-5.38 oz-93.83 h270.0` | 270.0 | walk | `wp=63.01,-5.38` | 3 | `cells 31>35 E=63.01,-5.38,-93.83 d=0.15` |
| IN4 | interior | 2 | 0xF3EE | 32 | 0.00,-7.00,-91.26 | `r2 lx1944 ly1904 cInside ox0.00 oy-7.00 oz-91.26 h180.0` | 180.0 | walk | `wp=0.00,-13.00` | 3 | `cells 32>39 E=0.00,-13.00,-91.26 d=0.15` |
| CV1 | cave | 1 | 0xE13A | 0 | 107.61,127.21,209.02 | `r1 lx1805 ly470 ox107.61 oy127.21 oz209.02 h30.0` | 30.0 | walk | `wp=115.11,140.20` | 4 | `follow E=115.11,140.20,208.03 d=0.15 dz=0.05 headroom=3.37` |
| CV2 | cave | 3 | 0x5B60 | 0 | 18.75,101.57,369.92 | `r3 lx728 ly773 ox18.75 oy101.57 oz369.92 h120.0` | 120.0 | walk | `wp=31.74,94.07` | 4 | `follow E=31.74,94.07,370.97 d=0.15 dz=0.05 headroom=6.41` |
| CV3 | cave | 4 | 0xC9AE | 0 | 19.11,63.44,697.15 | `r4 lx1608 ly1395 ox19.11 oy63.44 oz697.15 h195.0` | 195.0 | walk | `wp=15.23,48.95` | 4 | `follow E=15.23,48.95,697.90 d=0.15 dz=0.05 headroom=4.04` |
| WA1 | water | 1 | 0x9E7F | 0 | 73.22,128.04,337.68 | `r1 lx1267 ly1022 ox73.22 oy128.04 oz337.68 h224.9` | 224.9 | run | `wp=44.99,99.71` | 16 | `wade_swim in=72.58,127.40 swim=70.50,125.31 h=337.255 fh=1.42 d=0.50` |
| WA2 | water | 2 | 0x5BC7 | 0 | 100.87,125.06,942.79 | `r2 lx733 ly1598 ox100.87 oy125.06 oz942.79 h85.1` | 85.1 | run | `wp=140.72,128.48` | 16 | `wade_swim in=105.00,125.41 swim=114.72,126.25 h=941.177 fh=1.42 d=0.50` |
| WA3 | water | 3 | 0x7E2C | 0 | 128.11,108.17,149.90 | `r3 lx1014 ly357 ox128.11 oy108.17 oz149.90 h172.1` | 172.1 | run | `wp=133.61,68.55` | 16 | `wade_swim in=128.69,103.96 swim=129.31,99.55 h=149.020 fh=1.42 d=0.50` |
| WA4 | water | 5 | 0xEFB1 | 0 | 37.10,135.01,150.60 | `r5 lx1913 ly1422 ox37.10 oy135.01 oz150.60 h186.0` | 186.0 | run | `wp=32.92,95.23` | 16 | `wade_swim in=36.50,129.29 swim=36.07,125.16 h=149.020 fh=1.42 d=0.50` |
| FR1 | frill | 1 | 0x4C66 | 0 | 85.96,27.56,343.87 | `r1 lx612 ly817 ox85.96 oy27.56 oz343.87 h90.0` | 90.0 | run | `wp=115.96,27.56` | 6 | `pass E=115.96,27.56,335.92 d=0.10` |
| FR2 | frill | 3 | 0x7A69 | 0 | 66.00,106.57,189.51 | `r3 lx979 ly845 ox66.00 oy106.57 oz189.51 h90.0` | 90.0 | run | `wp=96.00,106.57` | 6 | `pass E=96.00,106.57,190.39 d=0.10` |
| FR3 | frill | 5 | 0x6FD9 | 0 | 100.17,88.49,173.75 | `r5 lx893 ly1740 ox100.17 oy88.49 oz173.75` | 0.0 | run | `wp=100.17,118.49` | 6 | `pass E=100.17,118.49,174.40 d=0.10` |

<a id="s2-4"></a>
### 2.4 What each walk meets

**Walls and undrawn blockers** (blocking SEDs, `collision.md` §9.3; every one Physics,
`Physics_NoPhysics` false, not ethereal to Player, no content-layer list, 0x66 mesh). The
wall plane is the face hit by a horizontal ray 1.0 m above the start ground along the
heading; its distance minus the capsule radius is the stop distance within 0.04 m.

| Walk | Landblock info, SED index | Object → collision mesh | Kind (our words) | Drawn | Wall plane: distance from start along the heading, normal (x, y) |
|---|---|---|---|---|---|
| WL1 | 0x8021A197 #23 | 0x470003D3 → 0x660000BC | house (M3-02 anchor), south wall | yes | 5.53 m, (0, −1) |
| WL2 | 0x80226EA5 #20 | 0x470093DF → 0x66003840 | long barn (M3-06 anchor), long side | yes | 5.54 m, (−0.83, −0.56) |
| WL3 | 0x8023A73F #523 | 0x47007788 → 0x66002B41 | large building (M3-11 anchor), east wall | yes | 5.07 m, (1, 0) |
| WL4 | 0x8024C755 #40 | 0x470085DA → 0x66003426 | long building (M3-14 anchor) | yes | 5.21 m, (0.71, −0.70) |
| WL5 | 0x8025757A #6 | 0x4700E31C → 0x66004BF4 | two-storey building (M3-18 anchor), north wall | yes | 5.28 m, (0, 1) |
| UB1 | 0x80223B91 #102 | 0x47000C82 → 0x66000379 | invisible wall panel; mesh 10 × 2 × 10 m, SED scale 1.95 | **no** | 5.00 m, (0, −1) |
| UB2 | 0x80247CBC #0 | 0x47000DD1 → 0x66000446 | invisible wall block; mesh 30 × 10 × 33.5 m, SED scale 0.75 | **no** | 5.00 m, (0.71, 0.71) |
| UB3 | 0x802E1506 #385 | 0x47000C82 → 0x66000379 | invisible wall panel; mesh 10 × 2 × 10 m, scale 1 | **no** | 4.99 m, (0.71, 0.71) |

**Generated objects** (GEN; `scenery.md` §7). Each object is the record at the given index
of the landblock's generated object list (static pass, vertices in the order of §7.2). The
path names the vertex scene, the index of the group child, the index of the node child and,
for nested scenes, the scene reference and its children. Heading is the compass bearing of
the object's yaw (`coordinates.md` §11.3–§11.4). A = X + Y + R + 5 (`scenery.md` §6.3).

| Walk | Landblock (A) | Object list index | Vertex (i, j) | Path | Object → setup → collision mesh | Position x, y, z | Rotation (w, x, y, z); heading | Scale | Kind (our words); collision extent (m) |
|---|---|---|---|---|---|---|---|---|---|
| GN1 | 1, 0x669A (262); no landblock info, so no statics | 16 | (13, 8) | 0x02000152 > 2 > 0 | 0x470035F4 → 0x04000671 → 0x66003EF4 | 130.612, 83.519, 559.263 | (0.98286, 0, 0, 0.18435); 338.75° | 1.1701 | tall tree; trunk 2.6 × 1.9, 36.0 high |
| GN2 | 4, 0x1A23 (70); 24 statics with clearance volumes | 33 | (4, 5) | 0x020009D1 > 2 > 0 | 0x4700988D → 0x04006806 → 0x6600395A | 40.970, 46.965, 192.134 | (0.99017, 0, 0, −0.13984); 16.08° | 1.0176 | tree; 2.5 × 3.7, 20.1 high |
| GN3 | 5, 0x655A (201); 4 statics with clearance volumes | 176 | (13, 4) | 0x02000E92 > 0 > 0 > 0x02000EF9 > 17 > 0 | 0x4700D780 → 0x04007CA0 → 0x66004813 | 123.961, 37.660, 440.486 | (0.99149, 0.04956, −0.11506, 0.03534), tilted to the slope; 356.54° | 1.0304 | rock outcrop with a steep face; 5.9 × 4.8, 3.65 high |
| GN4 | 14, 0x1FE2 (276); 25 statics with clearance volumes | 138 | (12, 13) | 0x0200030F > 3 > 0 | 0x47003B84 → 0x04002B43 → 0x660015B8 | 117.026, 130.965, 435.875 | (0.14215, 0, 0, 0.98985); 196.34° | 0.9556 | tree; 2.9 × 2.7, 12.0 high |

On every row the first contact on the path is the listed object, and no object with
collision volumes (boxes, spheres, capsules) lies within 3 m of the path.
If GN1–GN4 fail because the object is absent or elsewhere, compare the landblock's whole
generated list with the explorer's scenery view first: the spot then reports a
generation difference, not a collision one.

**Steps.** All three climb the same kind of object, a platform with steps and a ramp
(0x47000AE1, collision mesh of its setup), placed at three spots; its risers differ with
the terrain under it. Ramp faces on the way are at most 33.7°.

| Walk | SED | Risers (m), in order | Rise start → end (m) |
|---|---|---|---|
| ST1 | 0x80215B6D #354 | 0.26, 0.13 | 374.46 → 375.64 |
| ST2 | 0x80217C6E #272 | 0.32, 0.13 | 356.27 → 357.50 |
| ST3 | 0x80210725 #67 | 0.17, 0.26 | 349.98 → 351.19 |

**Steep slopes.** Terrain only; no landblock info, and no generated object within 20 m.
SL1: triangles of 53.5–58.6° rise 4.1 m within 3 m of B; SL2: 60.0–61.4°, 5.5 m; SL3:
56.3–62.0°, 4.7 m.

**Interiors** (CELL). Dungeon cells reached by portal ([§1.1](#s1-1) item 1). Each start is
inside the first cell, 3 m before the portal's centre, on the floor of its physics mesh;
the end is 3 m past the portal. Portal sizes are width × height of the portal polygon.

| Walk | Landblock info | Start cell (geometry) | Portal | End cell (geometry) | Doorway: both cells |
|---|---|---|---|---|---|
| IN1 | 0x802191A6 | 9 (0x01000063) | #0, 30 × 15 m | 8 (0x01000038) | 3.00–3.02 m from the start |
| IN2 | 0x802398D6 | 49 (0x010002FF) | #0, 20 × 15 m | 48 (0x010002FF) | at 3.00 m |
| IN3 | 0x8024FC85 | 31 (0x010002EF) | #0, 20 × 15 m | 35 (0x010002FA) | at 3.00 m |
| IN4 | 0x8022F3EE | 32 (0x010002DB) | #0, 20 × 15 m | 39 (0x01000307) | 3.00–3.02 m |

**Caves** (`landblock.md` §3.5, `collision.md` §6.4). Every quad on the path has drawn
terrain and drawn ceiling triangles and four non-zero ceiling samples; `Landblock_UseCeiling`
(block-map key 9) has the value 1. CV1 floor 206.61–209.02, ceiling 212.38–215.59; CV2 floor
369.20–370.97, ceiling 377.38–379.95; CV3 floor 696.81–697.90, ceiling 701.94–703.00.

**Water** (WAT). No landblock info in any of the four blocks.

| Walk | Water type, level *L* → height *h* | Surface | Volume entity | Water body |
|---|---|---|---|---|
| WA1 | 37, 86 → 337.255 | 0x31004359 | 0x47000754 | lake (84 % of the samples below *h*) |
| WA2 | 11, 240 → 941.177 | 0x31004359 | 0x47000754 | mountain lake |
| WA3 | 15, 38 → 149.020 | 0x31006653 | 0x47000754 | sea (region 3 sea level) |
| WA4 | 23, 38 → 149.020 | 0x31008DB5 | 0x47000754 | sea (region 5 sea level) |

**Frill areas.** No blocking SED or generated object within 3 m of the path; frill records
(GEN frill pass) within 1 m of the path line: FR1 30 (mostly 0x4700077C, 0x47006200; vertex
scene 0x0200000A, a frill-only scene: the block has no group objects at all), FR2 51
(0x47006206, 0x47006239, 0x47005A50, 0x4700077A), FR3 35 (0x4700605D, 0x47006201,
0x47006083). Frills never collide (`scenery.md` §8.1), so the walk passes with or without
them; the row checks that none of them was turned into an entity.

> **OPEN (implementer, 2026-10-06, first run of every walk; `walker.md` §13.8):** 19 of
> 32 walks pass, all with 0.0000 m penetration. For the spec writer (§2.2: re-check before
> the implementation changes), the 13 others:
> - **WL2** (near miss): the first stop is 0.009 m from P, but forward stays held for 3 s
>   against a wall 34° off the heading and the walker creeps 0.104 m along it (d 0.10).
> - **GN2, GN3, GN4**: the first stop is 0.196, 0.036 and 0.109 m from P (the objects
>   block where the row says), then the walker slides around the trunk or rock with
>   forward held and ends 0.29, 0.39 and 1.23 m from P. `blocked` measures the end; the
>   SWEEP keeps the capsule upright and stops at the first contact. Should `blocked`
>   measure the first stop for round obstacles?
> - **GN1**: nothing blocks the path; the walker passes P and runs on (no generated object
>   body within reach). A generation difference (the block has no landblock info) or a
>   world-side gap; to be compared with the explorer's generated list as §2.4 says.
>   **Answered 2026-10-07 (spec writer, [§8](#s8)):** a generation difference on the
>   checklist's side. The former tree (0x470035F3 at 126.899, 87.394) came from a
>   generator with an inverted footprint test, and the client does not place it
>   (`scenery.md` §7.4 step 14b). GN1 now walks into another tree of the same landblock,
>   which the walker's generated list holds at index 16. GN2–GN4 keep their objects and
>   stop points; only the list indices changed (33, 176, 138).
> - **CV2**: the start lies on terrain steeper than walkable for the 0.35 m capsule
>   (support classified steep from the first step); the walker slides 11 m down before
>   the input begins.
> - **CV1**: the end point lies on a steep stretch: at (114.3, 140.8) the walker's
>   support turns steep (state 6) and it slides back; it ends 2.3 m short and 1.96 m below
>   E.
> - **FR1** (near miss): on the slope near the start the capsule rests 0.131 m above the
>   terrain under its axis (a sphere on a ~43° slope touches it beside the axis), against
>   `pass`'s 0.05 m.
> - **SL1–SL3**: the walker never climbs (highest feet below B.z) but never slides either:
>   the controller meets a steeper-than-walkable band from walkable ground as a wall
>   (`collision.md` §12.3 OPEN (e)), so state 6 never occurs. Either the condition "state
>   6 at least once" or reading (e) needs a decision.
> - **ST2**: stopped at the first riser (0.32 m, below the 0.5 m step-up height) with no
>   step-up; **FR3**: stopped on near-flat terrain at (100.17, 114.77) with two terrain
>   contacts and full forward speed. Both are controller faults reported to the
>   `tessera-physics` owner (a step-up landing on a riser/tread edge is classified by the
>   riser's face normal), not checklist issues. **Fixed in the M4.5 exit verification
>   (2026-10-06, `collision.md` §12.3 OPEN (n), (o)): both pass, so 21 of 32 walks pass.**

<a id="s2-5"></a>
### 2.5 The 1,000-walk soak: how its landblocks are chosen

A rule, applied to the user's install at run time:

1. **Regions:** the regions whose descriptor exists and that have landblock data (in this
   install 1, 2, 3, 4, 5 and 14), in ascending order.
2. **Land landblocks of a region:** every landblock-data record 0x800RXXYY with
   1 ≤ X, Y ≤ 253 (so all eight neighbours can exist) such that
   - its terrain chunk has at least one layer (drawn terrain, `landblock.md` §3.8);
   - its 1,089 height samples are not all equal (excludes flat filler blocks such as the
     undrawn height-0 blocks of [§1.1](#s1-1));
   - it is not fully submerged: it has no landblock water (`water.md` §6.2), or at least a
     quarter of its samples lie above the water height.
   Sorted by DID.
3. **Seeded choice.** A SplitMix64 generator with seed 0x4D34352D534F414B (the bytes of
   "M45-SOAK"), one draw u = (next() ≫ 11) / 2⁵³ per random number. Walk *k* (0 ≤ *k* < *N*,
   *N* ≥ 1,000) uses region number *k* mod (number of regions) of step 1, then a landblock
   ⌊u · n⌋ of that region's list (n = list length), a start x and y each 16 + 128 u, and a
   heading 360 u.
4. **Start:** the feet are put on the highest walkable support found by a downward sweep
   of the capsule from 3 m above the terrain at (x, y); if the capsule overlaps geometry
   there, or the support is a cell floor, draw a new (x, y).
5. **Inputs:** 60 s of simulated time in segments of 2 + 4 u seconds. Per segment, by one
   draw: run forward (u < 0.50), walk forward (< 0.65), backward (< 0.75), strafe left
   (< 0.80), strafe right (< 0.85), run forward with a jump at the segment start (< 0.95),
   stand still (otherwise); at each segment start the heading changes by 180 u − 90 degrees.
6. **Failure:** feet more than 0.05 m below the terrain height (`landblock.md` §3.11) of the
   landblock that contains them while outdoors, or more than 0.05 m below the floor of the
   cell that contains them (the highest cell physics-mesh triangle below the feet), or any
   non-finite position or velocity. The criterion is 0 failures.

The landblock lists and the seed are logged with the result, so a failing walk can be
replayed alone.

> **OPEN (implementer, 2026-10-06, soak; `walker.md` §13.7):** (a) the order of the draws
> is not given; Tessera draws landblock, x, y, heading, the (x, y) redraws of step 4, then
> per segment its length, kind and heading change. (b) Step 4 does not bound the redraws,
> and some land landblocks have no valid start: 3 of the first 1,003 planned walks (region
> 4 (23, 78), region 2 (203, 114) and (94, 207)) found none in 256 draws; Tessera passes
> such a landblock over and plans the next walk. (c) A start whose sweep finds no support,
> or a support that is not walkable, is redrawn too. (d) "Backward" and "strafe" segments
> use the run layer. Please confirm or state the rule.

---

<a id="s3"></a>
## 3. Performance walk spots (exit criterion 5)

Headless 1920 × 1080, unpaced, default-settings profile (`walker.md` §11.8), every M4.5
system on, 60 s per spot, p99 frame time ≤ 16.7 ms on the reference machine. A spot's path
is a camera path, not a collision test: the camera flies at 6.5 m/s through the waypoints,
2 m above the higher of the terrain and the landblock water surface, looking along the
path with pitch −10°, and loops until 60 s have passed. Stationary spots turn the camera
at 6° per second (one full turn in 60 s). Game time: day 12,240, time of day 0.5
([§4.0](#s4-0)).

| Spot | Kind | Region, landblock | Start (x, y) and ground z | Heading | Path (block-local waypoints, closed loop) | Why this spot |
|---|---|---|---|---|---|---|
| P1 | dense forest with frills | 2, 0x7744 | (30.00, 30.00), 155.31 | 90 | (30, 30) → (130, 30) → (130, 125) → (30, 125) | 364 generated objects and 23,172 frill records in the block (GEN) |
| P2 | town | 1, 0x8FF3 | (30.00, 30.00), 615.00 | 90 | the same rectangle | 468 drawn Physics SEDs with 0x66 meshes and 35 Light SEDs inside the block square, the most outdoor Light SEDs of any region-1 landblock; light map 0x41221577 |
| P3 | lake shore | 1, 0x9E7F | (30.00, 30.00), 330.14 (under water *h* 337.255; the camera flies at 339.26) | 90 | the same rectangle | lake type 37, *h* 337.255, 84 % of the samples under water; shore and slopes at the north-east corner |
| P4 | interior | 1, 0x91A6, cell 9 | (40.86, −108.12), −212.90 | 90 | stationary, turning | the IN1 dungeon: 30 × 15 m portals to neighbouring cells, interior fog (row Y5) |
| P5 | vista at the full landscape extent | 3, 0x53D7 | (152.00, 40.00), 431.48 | 190 | stationary, turning | a summit; along heading 190 the sea (*h* 149.02) is in direct view from 960 m to 2,060 m (W9), so terrain and water are drawn to the 2,080 m extent |

---

<a id="s4"></a>
## 4. Visual rows (exit criterion 6)

<a id="s4-0"></a>
### 4.0 Conventions

- **Camera.** "Eye" is the camera position in the frame of the row's landblock; heading is
  a compass bearing and pitch is negative downward. Field of view 45° vertical
  (`walker.md` §11.8). Free-fly camera; the avatar is hidden.
- **Game time.** Day number *n* and time of day *t* (`environment.md` §11.1). The original's
  epoch is OPEN, so Tessera's clock is set to the row's values; day type *D* = 2 (the
  Tessera initial value, [§1.1](#s1-1) item 3). *n* = 12,240 unless stated.
- **Colours.** Keyframe colours are given as R, G, B in 0–255 after blending
  (`environment.md` §5.6, §11.3). Property colours are given as stored (bytes B, G, R, A,
  `property-system.md` §4.7) and as R, G, B.
- **Facts.** "Expected" lists what Tessera must use at that spot (record DIDs and values);
  the walker side is checked by a real-data test or by the explorer's inspector, the look
  by the owner (items O20–O27).

<a id="s4-1"></a>
### 4.1 Terrain layers

Terrain-type keys of the region's 0x19 table (`terrain-texturing.md` §3); layers listed in
draw order, bottom first, as stored in the chunk (§4.2), keeping only the layers whose
triangles cover the triangle under the target point. Features of the type entry
(`terrain-texturing.md` §3.3): N = normal texture; S = both specular booleans set; Sn =
gloss from the normal texture; O = `use_opacity_mask` (the opacity switch of §8.1 for a
layer that is not the first drawn). With S, the layer's specular switch of §8.1 is on
unless O takes the diffuse alpha and Sn is absent; no row has that case. "Quad" is the quad-grid key there (§4.6).
The camera of every row: eye 4.28 m from the target against the heading, 3 m above the
target's ground, pitch −35° (the view ray meets the ground at the target on flat ground).
Time *t* = 0.5.

| Row | Region, landblock (table) | Target x, y, z | Heading | Layer stack: key (diffuse / normal / mask; features) | Quad | Also expected |
|---|---|---|---|---|---|---|
| T1 | 1, 0x7574 (0x19000000) | 128.90, 98.63, 387.15 | 0 | 2 (0x40015178 / 0x40015179 / 0x4100E6BD; N) · 6 (0x40000E0F / – / 0x4100E6C4; rotation 45°) · 7 (0x40000E0F / – / 0x4100E6C5) · 675 (0x40018390 / 0x4001838E / 0x41134A3F; N) · **10** (0x40001160 / 0x40001161 / 0x4100E6C6; N S; rotation 300°, scale 0.15) | 10 | road: a cobblestone road type is the top layer and the quad's key |
| T2 | 1, 0x54A0 | 37.34, 111.30, 501.09 | 0 | **391** (0x40002FD8 / 0x40003051 / no mask; N S; scale 0.055) | 391 | single snow layer with specular, no mask |
| T3 | 2, 0x65B1 (0x19000001) | 139.41, 106.00, 990.33 | 90 | **850** (0x4000112B / 0x4000243A / 0x411D9C74; N S) | 850 | cliff-top layer, normal-mapped and specular |
| T4 | 2, 0x4F32 | 139.42, 64.30, 410.83 | 90 | **195** (0x40016521 / 0x40016522 / 0x41110016; N; scale 0.35) | 195 | stone road, the only layer under the target |
| T5 | 3, 0x9FE8 (0x19000002) | 44.36, 27.70, 188.97 | 180 | 0 (0x400010FC / – / no mask) · 297 (0x40017270 / 0x40017271 / 0x41160C8A; N) · 306 (0x40001138 / – / 0x41160913) · **57** (0x4001A9DD / 0x4001A9DE / 0x41160D9C; N S) | 297 | **light-mapped block**: `Landblock_LightMap` 0x4116792D, added to the lighting (`terrain-texturing.md` §10) |
| T6 | 3, 0x7FBD | 52.91, 49.90, 192.02 | 195 | **67** (0x4001AE5F / 0x400156E1 / 0x4116F0C6; N S; specular colour white, power 8.0) | 67 | paved city ground, specular from the diffuse alpha (see L5) |
| T7 | 3, 0x49E7 | 24.31, 121.73, 171.53 | 0 | 290 (0x40003F04 / 0x40003F05 / 0x4116367F; N) · **274** (0x400035B1 / 0x400035B2 / 0x41163485; N) | 274 | cobbled road over rocky dirt |
| T8 | 4, 0x25BC (0x19000007) | 34.19, 44.50, 68.53 | 0 | 385 (0x4001D061 / 0x4001D03D / no mask; N) · 257 (0x4001D0BF / 0x4001D027 / 0x41223EBD; N) · **232** (0x4001D02A / 0x4001D021 / 0x41223EBE; N) | 232 | brick road |
| T9 | 4, 0xAC6E | 67.77, 93.62, 226.95 | 0 | **609** (0x40017840 / 0x40018055 / 0x4120E819; N S; scale 0.05) · 613 (0x4001E0A0 / 0x4001E0A1 / 0x4120E81A; N; scale 0.375) | 609 | **light-mapped block** (0x411AD5A6) |
| T10 | 5, 0x5C6C (0x19000007) | 97.57, 49.58, 331.29 | 0 | 1089 (0x40016A54 / 0x40016A55 / 0x41255730; N) · **1256** (0x40022F9F / 0x40022F9D / 0x41255731; N S Sn O; rotation 37°) | 1256 | gloss from the normal texture; the diffuse alpha is the opacity |
| T11 | 5, 0x3B85 | 89.66, 104.89, 174.87 | 0 | 691 (0x40016A56 / 0x4001534A / 0x412363B2; N S) · 884 (0x40020D37 / 0x40020BE8 / 0x41245C92; N S Sn O) · **936** (0x40020BE7 / 0x40020BE8 / 0x41245FD8; N S Sn O) | 936 | paved road, three road layers |
| T12 | 5, 0x4F62 | 124.98, 105.99, 319.65 | 0 | 1103 · 1245 (O; rotation 60°) · 1256 · 1257 · 1258 · 1259 (all four N S Sn O, rotations 37°, 73°, 141°, 169°) · 1268 · 1220 (O) · 1222 (O) — nine layers | 1222 | **light-mapped block** (0x4125EE9F); a deep stack |
| T13 | 14, 0x07CA (0x19000001) | 74.78, 120.78, 575.94 | 0 | **33** (0x400039F3 / 0x400039F2 / 0x41205A82; N S) | 33 | tiled road in a light-mapped block (0x412056D3) |
| T14 | 14, 0x0DCA | 133.79, 82.41, 600.47 | 0 | 29 (0x40003796 / 0x40003795 / 0x41205288; N S) · **78** (0x40003B5A / 0x40003BA4 / 0x41205404; N S) | 29 | **light-mapped block** (0x412054EE) |
| T15 | 1, 0xE13A | 111.36, 133.71, 206.67 (ceiling 215.55) | 30 | floor: 774, 775, 776 (all 0x400175A7 / 0x400175A8; N; projections 2, 1, 3, scales 0.01, 0.01, 0.03) · 644 (0x40016C5D / 0x40016C5E / 0x41112F0A; N O) | 776 | **ceiling chunk** above (row CV1's cave): ceiling layers 639, 640, 641, 774, 775, 776 (0x400175A7 / 0x400175A8, masks 0x41112F00, 0x41112F01, 0x41112F02, 0x411E8B5F, 0x411E8B60, 0x411E8B61); ceiling quad 641; `Landblock_CeilingLightMap` 0x4111307C and `Landblock_LightMap` 0x4111307D. Camera for this row: eye at the target + 1.8 m, heading 30, pitch +30° (looking up at the ceiling). |
| T16 | 4, 0xC9AE | 17.17, 56.20, 696.87 (ceiling 702.93) | 195 | **426** (0x4001D041 / – / 0x411AF0FF; no normal) | 426 | **ceiling chunk**: one ceiling layer, key 426 (mask 0x411AF102); light map 0x411AF104. Eye at target + 1.8 m, pitch +30°. |

Projections are 3 (XY) unless stated; scales are `u_scale` (= `v_scale` in every row).

<a id="s4-2"></a>
### 4.2 Scenery

Generated objects (GEN static pass) seen from the eye: the count of objects whose position
lies 3–30 m from the eye inside ±45° of the heading, the four most common DIDs among them,
and the two nearest with their exact placement (heading = compass bearing of the yaw).
Eye 1.8 m above the ground, pitch −10°, *t* = 0.5. A row passes when the walker's
generated list for the landblock contains every listed object at the listed position
(within 0.01 m), heading (0.1°) and scale (0.0001), and the owner sees the same objects in
the original.

| Row | Region, landblock | Eye x, y (ground z) | Heading | In view (total in block) | Most common DIDs | Two nearest: DID, position, heading, scale, vertex, path |
|---|---|---|---|---|---|---|
| S1 | 1, 0x669A | 123.88, 62.62 (559.55) | 30 | 6 (25) | 0x470035F4 ×2, 0x470035F6, 0x470035F3, 0x47003675 | 0x470035F6 (134.757, 76.422, 555.839) 222.29° 1.2196 (13, 8) 0x02000152 > 4 > 0; 0x470035F4 (130.612, 83.519, 559.263) 338.75° 1.1701 (13, 8) 0x02000152 > 2 > 0 (GN1) |
| S2 | 1, 0xA197 | 96.27, 42.08 (408.96) | 90 | 35 (315) | 0x470092DC ×9, 0x470021A0 ×8, 0x470092DF ×6, 0x470008C5 ×4 | 0x470092DC (101.636, 43.146, 404.978) 170.70° 0.7830 (10, 4) 0x02000CC1 > 14 > 0; 0x470008C5 (102.067, 41.334, 405.268) 200.52° 1.1077 (10, 4) 0x02000CC1 > 0 > 0 |
| S3 | 2, 0xAEBC | 24.43, 43.55 (672.29) | 120 | 11 (165) | 0x47006181 ×6, 0x47006180 ×5 | 0x47006181 (33.282, 36.418, 673.242) 84.89° 0.7148 (3, 4) 0x0200090A > 0 > 0; 0x47006181 (35.800, 43.432, 669.749) 156.23° 1.1352 (4, 4) 0x0200090A > 0 > 0 |
| S4 | 2, 0x7744 | 57.18, 124.28 (158.89) | 270 | 16 (309) | 0x4700617F ×7, 0x47005A5C ×3, 0x47005FB6 ×3, 0x47005FEC | 0x47005A62 (54.338, 122.030, 158.063) 64.84° 1.0634 (5, 12) 0x02000340 > 2 > 0; 0x47005A5C (50.475, 128.619, 159.243) 11.21° 0.9836 (5, 13) 0x02000340 > 0 > 0 |
| S5 | 3, 0x79AD | 66.49, 77.24 (311.99) | 240 | 9 (151) | 0x47006A36 ×2, 0x4700720E, 0x47007212, 0x470007CC | 0x47007213 (61.890, 74.593, 311.986) 18.84° 0.8633 (6, 7) 0x020005BA > 16 > 0; 0x47005FEC (55.744, 77.442, 311.986) 344.47° 0.6526 (6, 8) 0x020005B6 > 30 > 0 |
| S6 | 3, 0x7A69 | 76.48, 26.84 (190.22) | 180 | 6 (111) | 0x47006180 ×2, 0x47006BEB, 0x47000796, 0x47006BEA | 0x47006BEA (83.824, 18.210, 189.609) 100.84° 1.0760 (8, 2) 0x02000425 > 22 > 0; 0x47006BEB (68.317, 13.195, 189.385) 158.61° 1.2235 (7, 1) 0x02000425 > 15 > 0 |
| S7 | 4, 0x1A23 | 85.68, 93.43 (188.55) | 150 | 47 (445) | 0x47008202 ×6, 0x47008207 ×6, 0x47008203 ×6, 0x47008206 ×6 | 0x47008203 (85.939, 86.901, 187.329) 225.05° 0.9025 (9, 9) 0x020009CA > 4 > 0; 0x47007F5A (92.493, 91.001, 187.369) 311.70° 1.0031 (9, 9) 0x020009CA > 2 > 0 |
| S8 | 4, 0x7ADD | 119.52, 103.74 (252.65) | 60 | 14 (166) | 0x47006A36 ×3, 0x470007CA ×3, 0x4700079E, 0x47000620 | 0x47006A36 (122.779, 107.565, 252.148) 122.83° 1.1231 (12, 11) 0x020005BA > 25 > 0; 0x47000620 (126.422, 101.922, 254.398) 163.79° 1.1500 (13, 10) 0x020005B6 > 5 > 0 |
| S9 | 5, 0x655A | 123.83, 31.49 (442.10) | 30 | 18 (221) | 0x47000CAA ×4, 0x47008481, 0x4700C56C, 0x4700C85D | 0x4700D780 (123.961, 37.660, 440.486) 356.54° 1.0304 (13, 4) 0x02000E92 > 0 > 0 > 0x02000EF9 > 17 > 0 (tilted, GN3); 0x47008481 (130.043, 34.487, 444.051) 81.28° 0.4816 (13, 3) 0x02000E92 > 3 > 0 > 0x02000EFC > 1 > 0 |
| S10 | 5, 0x6FD9 | 59.39, 48.53 (174.25) | 150 | 22 (80) | 0x47007AA5 ×6, 0x47007AA8 ×5, 0x47007AA0 ×4, 0x47007AA1 ×4 | 0x47007CB0 (59.346, 42.853, 174.406) 128.72° 1.2184 (6, 4) 0x0200071A > 15 > 0; 0x47007AA0 (58.741, 41.622, 174.458) 17.71° 1.0178 (6, 4) 0x0200071A > 1 > 0 |
| S11 | 14, 0x1FE2 | 113.69, 66.56 (450.63) | 300 | 22 (186) | 0x470009A6 ×4, 0x47005B1E ×3, 0x470009A9 ×3, 0x470009A8 ×2 | 0x470009A6 (109.528, 66.155, 450.852) 291.40° 0.5993 (11, 7) 0x02000310 > 18 > 0; 0x47005B1E (106.341, 65.737, 450.953) 88.19° 0.7630 (11, 7) 0x02000310 > 38 > 0 |

> **Answered 2026-10-07 (spec writer):** the verification of 2026-10-06 found that
> Tessera's generator, which follows `scenery.md` §7.4 step 14b, missed 6 of the 22 listed
> objects and gave lower totals than this table. The table was wrong. It had been made with
> a generator whose footprint test was inverted ([§1](#s1), GEN). A BIN re-check
> confirmed step 14b as written (`scenery.md` S36). With the corrected generator, the
> rows above match the clean-room implementation's generated lists object for object in
> all 11 landblocks. Every row's total changed (S9: 221 instead of 362). The in-view
> counts and most common DIDs are unchanged in S3, S7 and S11 and changed in the other
> rows. The six objects that Tessera missed (both of S1, one each of S5, S6, S8 and S10)
> are not placed by the client and have left the rows. The next nearest objects take their
> place ([§8](#s8)).

<a id="s4-3"></a>
### 4.3 Frills

Frill records of the GEN frill pass (`scenery.md` §8.2, with item 5 as revised on
2026-10-07, and the Tessera Decision of §8.6; density 1.0, so the counts are exact). The
records of all six landblocks are identical under the earlier Tessera Decision of §8.2 ([§8](#s8)). The eye is 1.8 m above the target, pitch
−20°, *t* = 0.5. Expected drawing at the default level 2 (`scenery.md` §8.3–§8.4): frills
out to 97 units, fading from 67 units, distance reduction from 40 units; frills tinted
toward the terrain colour (intensity 1.0 unless the object says otherwise).

| Row | Region, landblock | Target x, y (z) | Heading | Frill records in block | within 10 m / 30 m | Most common within 10 m | Vertex scene at the target; its frill layer's scene references |
|---|---|---|---|---|---|---|---|
| F1 | 1, 0x4C66 | 85.96, 27.56 (343.87) | 90 | 4,075 | 22 / 453 | 0x47006200 ×11, 0x47006069 ×2, 0x4700077C ×2, 0x47000780, 0x47000784 | (9, 3) 0x0200000A; 0x020000CF, 0x020003A2 (no group objects in the whole block: frills only) |
| F2 | 3, 0x7A69 | 66.00, 106.57 (189.51) | 90 | 22,945 | 291 / 2,268 | 0x47006239 ×57, 0x4700077A ×15, 0x47006204 ×14, 0x470060EA ×12, 0x470060EC ×11 | (7, 11) 0x02000423; 0x02000450, 0x02000489, 0x02000475 |
| F3 | 5, 0x6FD9 | 100.17, 88.49 (173.75) | 0 | 15,584 | 314 / 1,940 | 0x4700605D ×188, 0x47006083 ×61, 0x47006201 ×51, 0x4700607C ×8 | (10, 9) 0x020005FA; 0x020005F0, 0x020004D9 |
| F4 | 2, 0x7744 | 93.14, 63.84 (153.74) | 90 | 23,172 | 284 / 2,699 | 0x47006062 ×165, 0x47006206 ×44, 0x4700606A ×36, 0x4700607C ×17 | (9, 6) 0x02000340; 0x0200034A, 0x020003A1 |
| F5 | 4, 0x7ADD | 64.18, 53.06 (278.03) | 90 | 15,066 | 135 / 818 | 0x4700605D ×80, 0x47006083 ×26, 0x47006200 ×18, 0x470071D9 ×4 | (6, 5) 0x020005BE; 0x020005AF, 0x020004DA |
| F6 | 14, 0x1FE2 | 80.00, 80.00 (448.98) | 0 | 8,347 | 132 / 1,119 | 0x4700078C ×42, 0x4700078D ×34, 0x4700078B ×31, 0x47006204 ×10 | (8, 8) 0x02000310; 0x0200030E (referenced twice) |

<a id="s4-4"></a>
### 4.4 Water

Water heights by `water.md` §6.2; *t* = 0.5 unless stated.

| Row | Region, landblock | Eye x, y, z | Heading, pitch | Expected |
|---|---|---|---|---|
| W1 | 5, 0xEFB1 | 37.10, 135.01, 152.40 | 186, −10 | **sea level**: type 23, level 38 → *h* 149.020; surface 0x31008DB5; volume entity 0x47000754 at (0, 0, 149.020); the shore of walk WA4 |
| W2 | 3, 0x7E2C | 128.11, 108.17, 151.70 | 172, −10 | **sea level** of region 3: type 15, level 38 → 149.020; surface 0x31006653 (a different surface from W1 at the same height) |
| W3 | 2, 0x5BC7 | 100.87, 125.06, 944.60 | 85, −10 | mountain lake: type 11, level 240 → 941.177; surface 0x31004359 |
| W4 | 1, 0x9E7F | 73.22, 128.04, 339.50 | 225, −15 | lake: type 37, level 86 → 337.255; surface 0x31004359; 84 % of the block's samples under water, deepest sample 314.24 at (0, 130) |
| W5 | 2, 0x81F8 | 84.90, 53.70, 449.22 (10 m above *h*) | 90, −45 | **river**: type 11, level 112 → 439.216; a west–east band of 10 % of the samples, deepest 436.29 at (150, 50); the neighbours to the west (0x80F8) carry the same level, the others are dry |
| W6 | 14, 0x3DEB | 150.00, 51.70, 390.39 | 90, −30 | **river with a step**: this block level 97 → 380.392, its east neighbour 0x3EEB level 94 → 368.627; the two planes meet at the shared edge x = 160 with an 11.765 m drop; surface 0x31004359 in both |
| W7 | 4, 0x38A8 | 95.00, 113.30, 152.02 | 0, −30 | **lava**: type 21, level 38 → 149.020; surface 0x31006FAE; volume entity 0x47008188 (lava monitors 0x78005CDF / CE0 / CE1, `water.md` §7.1–§7.2); 40 % of the samples below *h* |
| W8 | 3, 0x4DE9 | 40.20, 95.63, 167.60 | 0, −20 | **placed water**: SED 0x80234DE9 #67, object 0x47006F68 at (40.20, 101.63, 165.74), scale 0.70, a water-plane mesh with water-volume monitors (`water.md` §7.3); its surface 0x31006748 is unlit with source-alpha / inverse-source-alpha blending (`material.md` §11.3). The landblock has no landblock water. |
| W9 | 3, 0x53D7 | 152.00, 40.00, 433.48 | 190, −8 | **distant water** (perf spot P5): the sea at *h* 149.020 (type 11, level 38, sea floor at height 0) is in direct view from 960 m (block 0x52D1) to 2,060 m (0x51CA); the distant-landscape sectors 0x805351CF, 0x805351CC and 0x805351C9 cover those blocks (`water.md` §9). Expected: unbroken water out to the 2,080 m landscape extent. The block's sky key 2 gives 0x1A000000, fog far 2,500 at *t* 0.5, above the extent, so *G* = 2,080 m (`walker.md` §11.3). |

<a id="s4-5"></a>
### 4.5 Sky and fog

ENV: *D* = 2 (local default table), *n* = 12,240. *a*, *b* and *f* are the keyframe indices
and factor of `environment.md` §11.3. Fog near and far in world units. Eyes: Y1–Y3 1.8 m
above the M3 stand point of the landblock (`m3-reference-locations.md` §2.2), Y4 1.8 m
above the target of row T1; heading 0, pitch +10° (some sky in view).

| Row | Region, landblock | Sky key → day description | *t* | *a*, *b*, *f* | Fog near / far, colour | Sun brightness, colour; toward-sun vector | Ambient brightness, colour 1 / colour 2 |
|---|---|---|---|---|---|---|---|
| Y1 | 1, 0x5378 (M3-01) | 0x1000023B → 0x1A000359 | 0.5 | 4, 5, 0.5588 | 300.0 / 2,058.8, (88, 126, 146) | 0.55, (255, 243, 189); (−0.224, −0.837, 0.500) | 0.20, (226, 230, 240) / (226, 230, 240) |
| Y2 | 4, 0xC755 (M3-14) | 0x10000106 → 0x1A000269 | 0.25 | 1, 2, 0.4545 | 363.6 / 2,500.0, (15, 5.3, 0) | 0.50, (123.3, 48.0, 28.4); heading 7, pitch 50: (0.078, 0.638, 0.766) | 0.40, (89, 95, 120) / (31, 33, 42) |
| Y3 | 3, 0x693E (M3-10) | 0x10000084 → 0x1A000213 | 0.78 | 6, 7, 0.75 | 200.0 / 1,725.0, (140.3, 77.3, 67.8) | 0.55, (245.3, 174.0, 86.3); (−0.224, −0.837, 0.500) | 0.20, (226, 230, 240) / (226, 230, 240) |
| Y4 | 1, 0x7574 | 0x10000070 → 0x1A000000 | 0.0 | 0, 1, 0 | 1,000.0 / 2,500.0, (1, 2, 6) | 0.55, (74, 81, 158) (night); (−0.224, −0.837, 0.500) | 0.20, (125, 121, 147) / (125, 121, 147) |
| Y5 | 1, 0x91A6, cell 9 (IN1 start) | interior | any | – | **interior fog** from the landblock-info property set 0x802191A6: `InteriorFogType` 1 (Linear), `InteriorFogNear` 0.059, `InteriorFogFar` 90, `InteriorFogColor` stored (2, 14, 11, 255) = R 11, G 14, B 2; `InteriorAmbientLight` stored (8, 25, 8, 255) | – | The cell itself carries no fog properties; the landblock info does (`environment.md` §7.2, §12 last item). Eye at the IN1 start + 1.8 m, heading 90, pitch 0. |

Y1 and Y3 use the same sun direction (fact 4 of [§1.1](#s1-1)); the time of day shows in the
colours and fog. Y2's description has a different sun (7°, 50°) and ambient colour 2
darker than colour 1 (hemispheric term, `lighting.md` §8.1). Y4 is night: the sun colour
is a dim blue moonlight. For Y1 the global object limit *G* of `walker.md` §11.3 is the fog
far distance 2,058.8 m, just below the 2,080 m landscape extent (Tessera Decision of that
section: clip at fog far whenever fog is on).

<a id="s4-6"></a>
### 4.6 Lighting

| Row | Region, landblock | Eye x, y, z; heading, pitch | *t* (day description) | Expected |
|---|---|---|---|---|
| L1 | 1, 0x5378 | 60.00, 40.00, 465.05; 333, −15 | 0.5 (0x1A000359) | **sun shadow**: the barn of M3-01 (SED 0x80215378 #0, object 0x47000E92, top 467.36, 12.3 m above the ground 455.05) casts its shadow toward bearing 15° (away from the sun at bearing 195°, elevation 30°); the shadow of its top falls 21.3 m (12.3 × cot 30°) from the wall toward 15°, on flat ground. Shadow parameters per the Tessera Decision of `lighting.md` §9. |
| L2 | 1, 0x8FF3 | 100.00, 25.00, 617.00; 0, −5 | 0.0 (0x1A000377: fog 0 / 300, ambient 0.20 (190, 228, 195)) | **point lights at night**: Light SEDs #460 at (90.30, 42.41, 621.09), #462 (125.62, 48.18, 621.09), #483 (74.68, 34.44, 621.09), #464 (119.10, 80.34, 621.09), #474 (71.64, 72.15, 621.09), 6.1 m above the ground (615.00); each `LightRange` 10, `LightFalloff` 0.5 (attenuation *k* = 0.05, half strength at 4.5 m, `lighting.md` §8.3), `LightDiffuse` stored (3, 97, 251, 255) = R 251, G 97, B 3 (orange), `Light_BakedLandscapeLight` true. On objects within 10 m they add orange light; on the terrain they do not (Tessera Decision of `lighting.md` §8.3: the block has light map 0x41221577, which carries their light). Fog far 300: no object beyond 300 m is drawn (*G* = 300, `walker.md` §11.3). |
| L3 | 1, 0x4883 | 92.78, 41.64, 392.73; 0, −10 | 0.0 (0x1A000359: fog 200 / 700, ambient 0.40 (94, 91, 114)) | **emissive**: a camp bonfire (SED 0x80214883 #133, object 0x47005179, setup 0x04003A50) 6 m ahead; its surface 0x310045AE (lit, `ColorAndGloss`) has emissive texture 0x400166F3 (slot 3018) and glow colour (1, 1, 1, 1): it stays bright where the ambient is dark (`lighting.md` §5.9). |
| L4 | 2, 0x0340 | 128.00, 104.00, 244.53; 30, −15 | 0.5 (0x1A0001F3: one keyframe, sun brightness 0, fog 5 / 900, ambient 0.20 (32, 28, 21)) | **glow and emissive** in a dark place: a crystal cluster (SED 0x80220340 #18, object 0x470069D6, at (132.16, 110.19, 243.69), tilted) with surface 0x3100631F: emissive 0x40018902, glow mask (slot 3013) 0x40018902, glow colour (1, 1, 1, 1), drawn into the glow target (pass 0x08). Also: skull light poles 0x47005D98 near L3 (#297 at (73.22, 37.49, 389.71)) with glow surface 0x310051B1 (glow mask 0x40017652). |
| L5 | 3, 0x7FBD | 54.98, 57.63, 195.02; 195, −20 | 0.5 (0x1A000000) | **specular**: row T6's terrain layer 67 (specular switch on, gloss from the diffuse alpha, specular colour white, power 8.0) between the eye and the sun (bearing 195°, elevation 30°): a broad highlight on the paving ahead. |
| L6 | 1, 0x3F7C | 47.66, 98.54, 452.12; 0, 0 | 0.5 | **blended (translucent) surface**: a cobweb (SED 0x80213F7C #9, object 0x470004F3 at (47.66, 104.54, 450.35)), surface 0x31000493: lit, `ColorAndOpacity` with alpha test off, so it blends by its texture alpha (`material.md` §11.3) and is drawn in the transparent pass. See W8 for an unlit alpha-blended surface. |
| L7 | 3, 0x9FE8 | 44.36, 23.42, 191.97; 0, −35 | 0.0 (night) | **baked light map** on terrain: row T5's block with `Landblock_LightMap` 0x4116792D added to the layer lighting (`terrain-texturing.md` §10); at night the dim warm texels of the map are the only light other than the ambient. |

---

<a id="s5"></a>
## 5. M3 reference locations under the revised height rule

All 20 rows of [`m3-reference-locations.md`](m3-reference-locations.md#s2-2) were recomputed
with `landblock.md` §3.4 (stored × 2,000 / 65,535, single precision) and §3.11 (triangulated
height): every terrain height at the stand point and at the corner stays within the 0.05 m
tolerance of its listed value. The largest change is +0.030 m (M3-13 stand point: 217.690
against 217.66; the bilinear value under the new rule is 217.662); every other row moves by
−0.002 to +0.014 m. No erratum is needed; that file carries a one-line note.

---

<a id="s6"></a>
## 6. Procedure

<a id="s6-1"></a>
### 6.1 Walker side (implementer)

1. Criterion 4: a real-data test runs every walk of [§2.3](#s2-3) through `--walk-path`
   (table form) and checks [§2.2](#s2-2); then the soak of [§2.5](#s2-5). Record pass/fail
   per walk and the soak counts in the ROADMAP status table.
2. Criterion 5: run [§3](#s3) on the reference machine (`docs/perf/reference-machine.md`).
3. Criterion 6: check the "expected" facts of [§4](#s4) with real-data tests (layer
   stacks, generated lists, frill counts, water heights, blended sky values, lights and
   material flags); view each row once in the walker and keep the screenshots private.

<a id="s6-2"></a>
### 6.2 Original-client side (owner)

Items [O18–O27](owner-observations.md#o18): measurements of movement, the outcome of the
scripted walks in the original, and a side-by-side look at the visual rows. No
screenshots in the repository.

<a id="s7"></a>
## 7. Open points

> **OPEN:** Whether the original's collision answers match SWEEP where its controller is
> unknown (`collision.md` Q9): stop distances at walls, how far a steep slope is climbed
> before sliding, the exact wade-to-swim switch. Owner items O18 and O19 measure them.

> **OPEN:** The day type of the original on a given day (`environment.md` §11.2). Rows
> Y1–Y4 assume *D* = 2; owner item O26 records what the original shows.

> **OPEN:** The interior doorway rows cross portals between cells only, because no cell of
> this install adjoins the landscape ([§1.1](#s1-1) item 1). If a later install adds
> walk-in interiors, add a row that enters one from outdoors (`collision.md` §5.8.1).

<a id="s8"></a>
## 8. Change log

| Date | Change |
|---|---|
| 2026-10-06 | First version: walks, performance spots, visual rows, M3 reference locations. |
| 2026-10-07 | **Generated objects re-derived** (spec writer; `scenery.md` footprint revision). The GEN generator of [§1](#s1) had the footprint test of `scenery.md` §7.4 step 14b inverted: it tested only open markers and skipped closed ones. A BIN re-check confirmed step 14b as written. With the corrected generator, the static-pass lists of the 11 landblocks of [§4.2](#s4-2) equal the clean-room implementation's object for object. **§4.2:** every row re-derived with the same eye, heading and cone. Totals: S1 36 → 25, S2 344 → 315, S3 173 → 165, S4 364 → 309, S5 252 → 151, S6 176 → 111, S7 471 → 445, S8 361 → 166, S9 362 → 221, S10 102 → 80, S11 201 → 186. In view: S1 11 → 6, S2 41 → 35, S4 19 → 16, S5 29 → 9, S6 17 → 6, S8 34 → 14, S9 35 → 18, S10 29 → 22 (S3, S7, S11 unchanged). New nearest objects: S1 (both; the second is GN1's tree), S5 (second), S6 (first becomes the former second), S8 (first becomes the former second), S10 (first becomes the former second); S2–S4, S7, S9 and S11 keep both. The OPEN under §4.2 is answered. **§2.3–§2.4:** GN1's former tree (0x470035F3, list index 27) is not placed by the client. GN1 now runs north into tree 0x470035F4 of the same landblock (list index 16), from (130.61, 77.52) to the stop point (130.61, 82.61, 559.19). The terrain on the way slopes 30–34° across the path, below the 45° walkable limit. No object with collision volumes lies within 3 m of the path, and the first contact is the tree. GN2–GN4 keep their object, start and stop point; their list indices change: 40 → 33, 283 → 176, 149 → 138. The GN1 item of the implementer's OPEN in §2.4 is answered. **Unchanged** (checked with the corrected generator): the first contact of every other outdoor walk (WL, UB, ST, SL, CV, WA, FR) and the count of generated objects within 3 m of their paths; the frill records of the six landblocks of [§4.3](#s4-3), identical record for record under the frill rule of `scenery.md` §8.2 item 5 as revised; hence rows F1–F6 and walks FR1–FR3. |
