<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# P8 busiest scenario: frame-time and UI budgets (ROADMAP P8 criterion 3)

| | |
|---|---|
| **Status** | **INDICATIVE.** Measured on the reference machine of [decision 0010](../decisions/0010-reference-machine-laptop.md) while it was **not idle**: other agents' builds and tests ran throughout (load averages with every run below). Rule 2 of [`reference-machine.md`](reference-machine.md) applies: these figures can show a regression but cannot close the criterion. The owner re-measures on the idle reference machine (owner row [O200](../checklists/owner-observations.md#o200)). |
| **Budgets** | [Decision 0043](../decisions/0043-p8-parity-audit.md) §3 item 3: (a) paced run, 0 frames over 50 ms after the settle wait; (b) unpaced run, frame-time p99 ≤ 16.7 ms; (c) the UI budgets U1–U5 of [decision 0015](../decisions/0015-ui-rendering.md) §6 from the paced run at *Se* = 1, split as 0043 §3 item 3 (c) says; (d) no audio underruns when audio plays |
| **Scenario** | [`apps/client/tests/scenarios/busiest.scenario`](../../apps/client/tests/scenarios/busiest.scenario) (committed, synthetic, selectors only; its `install none` log `busiest.log` beside it) |
| **Run** | The windowed scenario run (`scenario-feed.md` §12.4) with the client's timing option `--scenario-timing 60` (`scenario-feed.md` §16 implementer OPEN of 2026-10-23), release build |
| **Test** | [`apps/client/tests/p8_busiest_real_data.rs`](../../apps/client/tests/p8_busiest_real_data.rs): determinism and completion with the install always; the two windowed runs and the budgets only with `TESSERA_PERF=1` |

## The scenario

Three minutes of virtual time (`end-at 180s`, tick 100 ms), everything at once
(decision 0043 §3 item 1):

| Minimum (0043 §3 item 1) | In the scenario |
|---|---|
| A crowd of at least 50 feed actors with figures around the player | 54 actors in three rings (9 m, 15 m, 22 m) with bodies from `creature kind=npc has-appearance=yes` (36) and `creature kind=mob has-appearance=yes` (18), four of them walking out and back every 10 s; plus the combat target, a town crier and two fellowship members (58 actors). With the install every crowd selector resolves (36 and 19 bodies) and the frame log shows up to 57 animated characters |
| Combat: selection and target frame, casts and inductions, effects with icons and particles, floating text, a defeat | A loop every 6 s from 2 s to 176 s: the player selects the target and the target the player, combat states on, three casts (one an induction), a parry, damage and heal texts, a cooldown, literal and record effects (`effect role=vital-over-time`, `role=property-modification`), the target's own cast; the target is defeated at 30 s and 90 s, the player at 45 s and 120 s (the defeat panel) |
| A full fellowship (the player and five members, `social-panels.md` SP12) with changing vitals | `fellowship player $brin $cael "Dara" "Edric" "Fenn"`; two members with entities (`vital`), three by name (`fellow-vital`); ten vital changes every 1.5 s |
| Chat at a steady 10 lines per second | Three lines every 300 ms (yell, fellowship, say), 1,800 lines in all; every whole second of the run has 10 |
| The main panels open: bags, character, quest tracker, minimap | `--open-panel bags --open-panel character --open-panel tracker` on the command line; the minimap is the HUD's. The bags hold 24 record items (`item has-icon=yes`) and 8 literal ones; a literal quest is tracked, with objective changes at 36 s, 96 s and 125 s |
| Synchronised music, if the feed allows it | Every 20 s from 5 s (nine times, a `repeat` block): three waiting parts (the player and two members), one `perform-start` 1 s later (starts at 6, 26, …, 166 s), and the player and Brin stop 12 s after that; so any 60 s window that starts in the first 120 s holds at least two synchronised starts (the test checks every such window). The tune is looked up in the user's own music folder |
| At least 60 s of this after the settle wait | 180 s. The settle wait starts once the player is placed, and the scenario's clock at world entry; with `--settle-timeout 60` the 60 s window ends at most 120 s after the player is placed, so the scenario still plays at its end when placement takes under 60 s (the report records `scenario at the end: running`, which the test asserts). A loot window opens at 32 s and 92 s and the defeat panel at 45 s and 120 s (both a panel open, an event frame of 0015 §6 U4 (a), (b)) |

Headless without data the scenario writes the committed log (4,551 lines, 361,291 bytes);
`tessera-net`'s committed-log test and the client's replay test check it byte for byte, and
the real-data test plays it twice with the install at the reference location with
byte-identical logs (4,810 lines; selectors: `$folk` 36, `$beasts` 19, `$strikes` 2,
`$wards` 2, `$boons` 2, `$wares` 24, `$errands` 2).

## Where and how

**Reference location:** M3-03 of
[`m3-reference-locations.md`](../checklists/m3-reference-locations.md): region 1, landblock
(109, 141) = 0x6D8D, block-local (62.96, 15.07), height 413.60, given as the exact spawn
`--spawn 1,109,141,62.96,15.07,413.60` (no world lookup, so the headless deterministic run is
driver-only, `scenario-feed.md` §12.2). The M4.5 systems run at their built-in defaults; a
windowed run also takes the user's `[Render]` preferences of the original client
(`walker.md` §14.10, `far-view.md` FD21), as any windowed run does.

**Commands** (release build, install read-only; settings and data in a scratch folder so
the user's own Tessera settings take no part):

```sh
export TESSERA_GAME_DIR=...            # the install, read-only
export TESSERA_SETTINGS_FILE=/tmp/p8/settings.toml XDG_CONFIG_HOME=/tmp/p8/config XDG_DATA_HOME=/tmp/p8/data
# the audio device backend, so that (d) is measured (without it audio mixes on the null
# backend and the report says "(d) not measured")
cargo build --release -p tessera-client --features audio-device
COMMON="--game-dir $TESSERA_GAME_DIR --scenario apps/client/tests/scenarios/busiest.scenario \
  --spawn 1,109,141,62.96,15.07,413.60 --size 1920x1080 --ui-scale 1 \
  --open-panel bags --open-panel character --open-panel tracker \
  --settle-timeout 60 --scenario-timing 60 --data-dir /tmp/p8/tessera-data"
# (a), (c), (d): paced (vsync, Fifo)
target/release/tessera-client $COMMON --ui-report /tmp/p8/paced.md --frame-log /tmp/p8/paced.csv
# (b), (d): unpaced
target/release/tessera-client $COMMON --no-vsync --ui-report /tmp/p8/unpaced.md --frame-log /tmp/p8/unpaced.csv
# the same through the test (asserts the budgets; needs a display and an idle machine)
TESSERA_PERF=1 cargo test --release -p tessera-client --features audio-device --test p8_busiest_real_data -- --nocapture
```

`--scenario-timing 60` waits until the player is placed and the area has settled (or
`--settle-timeout` has passed), then logs every frame for 60 s and closes the window. The
timing report (Markdown, ending with a `tessera-timing-report 1` block of figures) goes to
the log and to `--ui-report`; `--frame-log` holds the 60 s of frames (`walker.md` §7
columns). The UI recorder labels the frames before the window `settle` and those in it
`timed`; U1–U4 are taken over the `timed` frames, U5 over every recorded frame of the run
(decision 0043 §3 item 3 (c): "U5 over the whole run"). The report also records the UI's
effective scale *Se* (and whether the surface limited it), whether the window started and
ran its whole duration (`window=not-started|running|done`), and the frames the frame log or
the UI recorder could not keep because they were full; `budget_failures` (the test) fails a
window that never started or ended early, a full log or recorder, reports (c) as **not
judged** unless *Se* = 1 (a surface of at least 1024 × 768 with `--ui-scale 1`,
`ui-scaling.md` §4.3), and (d) as **not measured** unless audio ran on the device backend. A
run that has not ended after the settle timeout + the timed seconds + 180 s, or whose report
cannot be written, exits non-zero, windowed or headless; a headless run whose scenario
finishes before the window starts writes a report that says so, writes no frame log and
exits non-zero. Adding `--headless` makes the same run
offscreen (the UI run of `scenario-feed.md` §12.2, exactly 1920 × 1080, each frame waiting
for the GPU), which decision 0043 does not judge; it is listed below for comparison.

## Results (2026-10-23, indicative)

Reference laptop (i9-10885H, RTX 2070 Super with Max-Q Design, Vulkan), release build of the
`p8` worktree with other agents' uncommitted changes of that time, the tiling compositor
sizing the window (it gave 941 × 1041 for the requested 1920 × 1080 while other windows were
tiled beside it; `reference-machine.md`: windowed runs record the surface given). **At
941 × 1041 the UI ran at *Se* = 941 ÷ 1024 ≈ 0.919, not 1** (`ui-scaling.md` §4.3: *Se* =
min(*S*, *Wp* ÷ 1024, *Hp* ÷ 768)), so the two windowed runs of the table do not meet
decision 0043's *Se* = 1 condition for (c): their U1–U5 figures were painted on the content
scale ≠ 1 path (§6) and are indicative of that path only. Only the fifth run below
(1895 × 2101) and the headless runs (1920 × 1080) were at *Se* = 1. The build
has no `audio-device` feature, so audio mixed on the null backend: budget (d) was **not
measured** (0 underruns is what the null backend always reports). The area had settled when
every window started (6.3–8.4 s after world entry), and the scenario was still playing when
every window ended. These runs used `--settle-timeout 90` and the scenario's earlier music
(synchronised starts at 6 s and 96 s only, so at most the first fell inside a window), and
they counted only the NPC windows' opens as event frames (the 45 s defeat panel open was
counted as a steady frame); the event-frame counts below are from that version.

| Run | Windowed, paced | Windowed, unpaced | Headless, paced | Headless, unpaced |
|---|---|---|---|---|
| Surface | 941 × 1041 (*Se* ≈ 0.919) | 941 × 1041 (*Se* ≈ 0.919) | 1920 × 1080 (*Se* = 1) | 1920 × 1080 (*Se* = 1) |
| Load average (start → end) | 10.6 → 8.0 | 8.0 → 6.5 | 5.7 → 12.0 | 12.0 → 13.6 |
| Frames in 60 s | 3,573 | 6,451 | 2,904 | 3,758 |
| Frame p50 / p95 / p99 / max (ms) | 16.66 / 19.36 / 22.10 / 42.0 | 8.66 / 12.78 / **19.09** / 38.2 | 16.71 / 40.58 / 51.09 / 75.5 | 14.25 / 36.75 / 56.18 / 92.1 |
| Frames over 16.7 ms | 1,551 | 161 | 1,814 | 719 |
| Frames over 50 ms | **0** | 0 | 34 | 59 |
| Audio underruns | not measured (null backend) | not measured | not measured | not measured |
| U1 steady mean / p99 / max (ms) | **3.740 / 6.158** / 13.17 (2,938 frames) | 3.224 / 5.501 / 11.61 | 3.974 / 11.79 / 22.72 | 3.246 / 12.22 / 23.40 |
| U2 UI pass GPU time | 0.0000 ms in 2,938 frames (see Open items) | 0.0000 ms | 0.0000 ms | 0.0000 ms |
| U3 steady mean / max | **261.7 KiB** / 1,011 KiB | 231.7 / 868 KiB | 235.1 / 933 KiB | 228.1 / 886 KiB |
| U4 (a) / (b), 41 event frames (ms) | 24.2 / **6.65** | 11.5 / 4.36 | 46.4 / 12.53 | 18.5 / 3.71 |
| U4 (c)–(e) | not applicable (0043 §3 item 3 (c)) | | | |
| U4 (f), 600 content-event frames (ms) | **9.12** | 9.52 | 18.38 | 27.90 |
| U5 peak: canvases / textures / images unpinned / pinned (MiB) | 15.6 / 12.6 / 7.9 / 1.5 | 14.5 / 11.7 / 7.5 / 1.5 | 17.0 / 13.5 / 7.4 / 1.6 | 18.8 / 13.3 / 7.5 / 1.7 |

A fifth run (windowed, paced, load average 27.7 → 16.4, after the report gained the U1 split;
the compositor gave 1895 × 2101 this time) measured: 0 frames over 50 ms (max 46.2 ms),
p99 22.43 ms; U1 steady mean 3.671 / p99 5.385 ms, of which **step 8 (the paint) 3.376 ms**,
step 7 0.077 ms and step 1 0.001 ms; U3 steady mean 406.2 KiB (max 1,609 KiB); U4 (a) 20.0 ms,
(b) 3.91 ms over 41 event frames; U4 (f) 10.23 ms; U5 canvases 31.2 MiB, textures 14.9 MiB.

Other figures of the windowed paced run (frame log summary): main-thread work max 40.3 ms,
streamer max 0.68 ms, 583 landblocks resident (519.5 MiB bundles, 737 MiB GPU, 2.6 GiB RSS),
no near-ring holes, up to 57 animated characters (at least 18 drawn), animation 0.36 ms per
frame on the main thread (59.8 % of the animation work on workers), character layer max
4.0 ms.

The `TESSERA_PERF=1` test was run once as well (load average 10.3 → 8.3, the deterministic
runs serialised before it): it failed, as expected on a loaded machine, with paced (a) 16
frames over 50 ms, U1 mean 3.958 / p99 9.793 ms, U4 (a) 24.5 / (b) 5.43 ms, U4 (f) 19.36 ms,
and unpaced (b) p99 19.52 ms. The path itself (two windowed runs, report written, figures
read back, budgets judged) works.

### Exit verification run (2026-10-24, indicative)

The `TESSERA_PERF=1` test once more, after the review fixes, on a quieter machine (load
average 1.3 → 2.4; other worktrees were idle, but this is not the owner's verified idle run of
`reference-machine.md`), release build without `audio-device`. The compositor gave
1895 × 1041 for both windows, so *Se* = 1.0000 and (c) was judged.

| Run | Windowed, paced | Windowed, unpaced |
|---|---|---|
| Frames in 60 s | 3,602 | 7,018 |
| Frame p50 / p95 / p99 / max (ms) | 16.62 / 21.39 / 23.50 / 38.3 | 8.37 / 10.16 / **11.47** / 38.0 |
| Frames over 16.7 ms / over 50 ms | 1,409 / **0** | 1 / 0 |
| U1 steady mean / p99 / max (ms) | **2.920 / 4.774** / 5.285 (2,926 frames) | 2.590 / 3.422 / 4.587 |
| U1 by step (steady means, ms) | step 1 0.001, step 7 0.069, **step 8 2.684** | step 8 2.389 |
| U2 | 0.0000 ms (fed) | 0.0000 ms |
| U3 steady mean / max | **262.1 KiB** / 903 KiB | 228.2 / 1,020 KiB |
| U4 (a) / (b), 82 event frames | 23.3 ms / **4.30 ms** | 12.5 / 4.19 ms |
| U4 (f), 610 content-event frames | **5.28 ms** | 5.04 ms |
| U5 peak canvases / textures / images unpinned / pinned (MiB) | 19.9 / 15.3 / 7.9 / 5.2 | 18.7 / 13.5 / 7.5 / 5.5 |
| Audio | not measured (null backend) | not measured |

Budgets: (a) met (0 frames over 50 ms); (b) met (p99 11.47 ms ≤ 16.7 ms); (c) missed: U1
(about 10 × its budget), U3 (just over 256 KiB), U4 (b) and (f) (over the 4 ms slice); (d) not
measured. The test failed on (c) and (d), as the path should.

**Where U1 goes** (two headless 30 s timing runs at 1920 × 1080, *Se* = 1, same machine): with
the bags, character panel and tracker open, step 8 averaged 2.60 ms (U1 2.85 ms, U3 266 KiB);
without them 1.49 ms (U1 1.75 ms, U3 271 KiB). So the three static panels add about 1.1 ms of
paint to every steady frame **without uploading anything**, which points at a per-frame cost
of the paint pass that grows with the open tree rather than at repainted boxes; the other
1.5 ms and the 260-odd KiB per frame come from the HUD of this scenario (combat, fellowship,
floating text, timers). Both are for the owner of `tessera-ui` to profile before the owner's
re-measure (O200).

### Reading (indicative)

- **(a)** The windowed paced run had no frame over 50 ms (max 42.0 ms); the second paced run
  of the test had 16. On the idle machine this decides itself.
- **(b)** The unpaced p99 of 19.1–19.5 ms misses 16.7 ms on the loaded machine; p95 is
  12.8 ms. Not judged here.
- **(c) U1 is far over its budget in this scenario**, by a factor of about 10 (mean 3.2–4.0 ms
  against 0.3 ms) in every run, windowed and headless (the windowed table runs at *Se* ≈ 0.92,
  outside the decision's condition; the fifth run and the headless runs at *Se* = 1 show the
  same), at loads where the M5 scripted run
  measured U1 means of 0.08–0.18 ms after the step-8 optimisation (`m5-ui-budgets.md`).
  The load alone does not explain a gap of that size: something in this scenario's UI costs
  milliseconds in steady frames (candidates: the floating texts, the cast bars, the effect and
  cooldown timers, the fellowship rows, the quest tracker, the target frame; the bags and
  character panel are static). U3's steady mean near 256 KiB (230–262 KiB) points the same
  way: steady frames upload as much as a repaint of a mid-sized box every frame. The fifth
  run's split puts almost all of it in **step 8**, the `tessera-ui` paint (3.38 of 3.67 ms):
  some element changes in every steady frame and its box is repainted and uploaded. A profile
  of `Ui::paint` in this scenario (which elements are dirty in a steady frame) should come
  before the owner's re-measure. U4 (b) 4.4–6.7 ms and U4 (f) 9.1–9.5 ms windowed are over the 4 ms slice too.
- **U5** stays far below every cap.
- **(d)** Needs a build with `--features audio-device` (or the audio device backend) to mean
  anything; the idle re-measure should use one.

### Open items

- **U2 reads 0.0000 ms in every frame.** The adapter has timestamp queries and the runs fed
  the renderer's figures (`u2_fed=1`), and every steady frame carried a figure, but all of
  them round to 0.0000 ms (below 50 ns). `UiStats::gpu_ns` documents that a zero figure is
  never reported, so either the UI pass's timestamps are written without the GPU work between
  them or the conversion loses it; a question for the owner of `crates/render` (`render.md`
  §6.16). Until it is answered U2 is not a gate (decision 0043 §3 item 3 (c)).
- **U1 in the busiest scenario** (above): step 8 repaints in every steady frame; profile it
  before the owner's re-measure, or the re-measure will fail (c) whatever the machine's load.
- **The window size.** The compositor gave 941 × 1041 (and once 1895 × 2101) instead of 1920 × 1080 while other
  windows were tiled; at 941 × 1041 *Se* ≈ 0.919, so (c) was not judged at *Se* = 1. The idle
  re-measure must run with the window alone on its workspace (or a floating rule) so that the
  surface is at least 1024 × 768 and close to the requested size; the test now asserts a
  surface of at least 1024 × 768 and the report records *Se*.
- **Decision 0043 wording.** Its Consequences say "no client timing mode … is needed"; the
  run uses the client's `--scenario-timing` (the windowed run otherwise never ends by itself:
  the world stays after the scenario finishes, §12.4). Whether to name the option in
  `scenario-feed.md` §12.1 is the §16 OPEN of 2026-10-23.

## Log

| Date | Change |
|---|---|
| 2026-10-23 | First indicative measurement (this file): the scenario, the timing option and the test are new. |
| 2026-10-23 | Review fixes: the report records *Se*, the window state and full-store counts; U5 over the whole run; the defeat panel open is an event frame; (c) not judged unless *Se* = 1 and (d) not measured without the device backend; music every 20 s; `--settle-timeout 60`; windowed runs have a deadline and fail when the report cannot be written. The table above is from before these fixes and was not re-measured. |
| 2026-10-24 | Exit verification: one `TESSERA_PERF=1` run after the review fixes at load 1.3–2.4 (above): (a) and (b) met, (c) missed (U1 2.92 ms, step 8), (d) not measured; two headless runs split U1 into the static panels (about 1.1 ms, no uploads) and the HUD (about 1.5 ms). |
