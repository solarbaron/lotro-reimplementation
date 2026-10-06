<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Reference machine for performance criteria

| | |
|---|---|
| **Status** | **Accepted** (2026-10-05, [decision 0010](../decisions/0010-reference-machine-laptop.md)): the laptop below |
| **Serves** | [ROADMAP](../ROADMAP.md) M3 criterion 2 (frame times), M4 criterion 4 (50 animated characters at 60 fps), M6 criterion 4 (no audio underruns), the [decision 0005](../decisions/0005-no-full-bevy.md) entity-storage benchmark (decided in [0008](../decisions/0008-entity-storage.md); it was measured on this laptop, so its numbers are reference numbers); [§4 open question](../ROADMAP.md#open) |

## Reference: the laptop

| Part | Value (observed 2026-10-05) |
|---|---|
| CPU | Intel Core i9-10885H, 8 cores / 16 threads, 2.4 GHz base, 5.3 GHz boost (the walker starts 14 asset workers) |
| GPU | NVIDIA GeForce RTX 2070 Super with Max-Q Design, 8 GiB; proprietary driver 615.71.09; Vulkan |
| RAM | 32 GB (30 GiB usable) |
| OS | Gentoo Linux, kernel 7.2.5, Hyprland (Wayland) |
| Display | 3840 × 2160 internal panel, 60 Hz, scale 1.333 |
| Power | On AC power. cpufreq governor `powersave` (intel_pstate), EPP `balance_performance`, ACPI platform profile `balanced` |
| Storage | Game install in the local Steam library |
| Toolchain | Pinned rustc 1.99.0 via rustup, release builds |

The tiling compositor sizes windows itself. Windowed runs record the surface size the
compositor gave, not the requested `--size`.

## Rules

1. Performance exit criteria are **met** only when measured on the reference machine with a
   release build. Record next to every number:
   - the machine and driver;
   - the resolution (for windowed runs, the surface size actually given);
   - the vsync setting;
   - the power profile;
   - whether the archives were cached;
   - anything else that was running.
2. Numbers from another machine are **indicative**: record them with that machine's
   description; they can show a regression, but not close a criterion.
3. If the reference machine changes (hardware upgrade, or another machine becomes the main
   one), re-run the M3 frame-time measurement there first, so later results can be
   compared with the M3 numbers, and record the change here with a date.

## M3 baseline on this machine (rule 3, 2026-10-05)

Release build of `a47af81`, rustc 1.99.0, all runs one at a time. Archives in the page cache
(certainly from the second run on, most likely from the first). **Not an idle machine:** an
interactive `tessera-client`, a browser, Steam and btop were running. The client was on the
GPU at about 60 fps and 730 MiB. The longest frames are therefore upper bounds. In the long
headless frames main-thread work was low (1–3 ms), which points to waiting on the GPU or the
scheduler.

Frame logs were re-computed from the CSVs by an independent check. They agree with the
walker's summaries except M3-d: its first log was lost, and the re-run varied as shown.

| Run | Command (all with `--residency-mib`, default 512) | Frames | Longest frame | p50 / p99 | Over 50 ms | Near-ring holes | Peak blocks / bundles / GPU / RSS | Result |
|---|---|---|---|---|---|---|---|---|
| M3-a | `--headless --size 1280x720 --fly-path 1:0:118,1:254:118 --speed 1000` (40.6 km) | 2,429 | 41.8 ms | 16.7 / 16.8 ms | 0 | 0 | 386 / 87.5 MiB / 792 MiB / 876 MiB | pass |
| M3-b | `--headless --size 1280x720 --fly-path 2:0:0,2:254:254 --speed 3000` (57.5 km) | 1,148 | 37.4 ms | 16.7 / 16.7 ms | 0 | 0 | 386 / 120.5 MiB / 669 MiB / 851 MiB | pass |
| M3-c | `--headless --size 1280x720 --fly-path 4:0:128,4:254:128 --speed 2000 --residency-mib 48` | 1,218 | 16.9 ms | 16.7 / 16.7 ms | 0 | 0 | 384 / 11.6 MiB / 33 MiB / 353 MiB | pass |
| M3-d | `--headless --size 1920x1080 --fly-path 5:0:0,5:254:254 --speed 2000` | 1,706 (re-run 1,703) | 33.0 ms (re-run 42.8 ms) | 16.7 / 23.3 ms (re-run 24.8) | 0 | 0 | 386 / 57.9 MiB / 788 MiB / 893 MiB | pass |
| M3-e | windowed, vsync (Fifo, 60 Hz), requested 1920x1080, given 1430x790; `--fly-path 1:0:118,1:254:118 --speed 1000` | 2,440 | 23.1 ms | 16.7 / 18.1 ms | 0 | 0 | 386 / 87.5 MiB / 793 MiB / 919 MiB | pass |

Every bundle requested in these runs was integrated (0 failed).

Compared with the earlier GTX 1070 Ti record (ROADMAP M3 status):
- Residency matches: 386 landblocks, 87–120 MiB bundles, 11.6 MiB under the 48 MiB budget.
- The longest headless frames are higher: 33–42 ms here, 17–22 ms there. They stay well
  inside the 50 ms criterion.
- `walker.md` §8 also has an OPEN about main-thread spikes of up to 40 ms on this machine
  during texture uploads, seen with the M3 build itself. Part of the gap is the machine
  rather than M4.
- Re-run M3-a, M3-b and M3-d with nothing else on the GPU before treating these maxima as a
  regression bar.

## M4 criterion 4 on this machine (2026-10-05)

Both runs drew **51 animated characters in every frame**: the crowd of 50 covering every
playable body, plus the avatar.

| Run | Command | Frames / s | Mean fps | p50 / p95 / p99 / max | Over 16.7 ms | Animation main thread mean (max) | On workers |
|---|---|---|---|---|---|---|---|
| M4-a | `--headless --unpaced --duration 60 --crowd 50 --heading 30 --size 1280x720` | 15,412 / 60.0 | 256.8 | 3.7 / 5.1 / 6.6 / 12.0 ms | 0 | 0.246 ms (1.06 ms) | 82.2 % |
| M4-b | windowed `--no-vsync --duration 60 --crowd 50 --crowd-spacing 2 --crowd-ahead 20 --heading 30`, surface 1430x790 | 21,034 / 60.0 | 350.5 | 2.8 / 3.5 / 4.2 / 9.6 ms | 0 | 0.255 ms (0.95 ms) | 82.8 % |

Both pass the criterion.

## M4.5 criterion 5 on this machine (2026-10-06)

Release build of the M4.5 working tree on `b1587f2` (uncommitted), rustc 1.99.0, every
M4.5 system on (the defaults of `walker.md` §14.9: extent 2,080 m, residency budget
4,096 MiB, far band 480 m, frills level 2), headless, no vsync (nothing is presented),
on AC power with the profile above, archives in the page cache (every run followed
earlier runs of the same spots). **Not an idle machine:** another session's Python job
held one core at 100 % throughout, and btop and a browser were running; load average 4–6
on 16 threads. The walker starts 14 asset workers and, since M4.5, 4 terrain workers.

**Walk spots** (`m4-5-visual.md` §3, P1–P5), `--headless --unpaced --duration 60
--size 1920x1080` plus the spot's camera:

| Spot | Camera | Frames | Mean fps | p50 / p95 / p99 / max | Over 16.7 ms | Peak blocks / bundles / GPU | Result |
|---|---|---|---|---|---|---|---|
| P1 | `--camera-loop "2:119:68:30,30;130,30;130,125;30,125"` | 7,442 | 124.0 | 8.0 / 9.6 / 10.7 / 14.7 ms | 0 | 611 / 553 MiB / 503 MiB | pass |
| P2 | `--camera-loop "1:143:243:30,30;130,30;130,125;30,125"` | 7,201 | 120.0 | 8.2 / 10.8 / 12.2 / 15.4 ms | 0 | 585 / 307 MiB / 652 MiB | pass |
| P3 | `--camera-loop "1:158:127:30,30;130,30;130,125;30,125"` | 11,004 | 183.4 | 5.7 / 7.5 / 9.3 / 14.0 ms | 0 | 611 / 564 MiB / 637 MiB | pass |
| P4 | `--spawn 1,145,165,40.86,51.88,-211.1 --fly --eye-height 1.8 --heading 90 --pitch 0 --turn-rate 6 --interiors` | 7,793 | 129.9 | 7.5 / 9.3 / 11.1 / 15.5 ms | 0 | 584 / 708 MiB / 633 MiB | pass |
| P5 | `--spawn 3,83,215,152,40,431.48 --fly --eye-height 2 --heading 190 --pitch -8 --turn-rate 6` | 11,105 | 185.1 | 5.2 / 7.1 / 8.8 / 11.8 ms | 0 | 583 / 350 MiB / 406 MiB | pass |

Every p99 is under 16.7 ms. Main-thread work had a median of about 2 ms; the rest of a
frame is submission (`end_frame_ms`, 1.2–3.0 ms median) and the wait for the GPU.

**M4 criterion 4 with everything on:** `--headless --unpaced --duration 60 --crowd 50
--heading 30 --size 1280x720`: 4,650 frames, mean 77.5 fps, p50 / p95 / p99 / max 13.2 /
14.3 / 15.2 / 21.4 ms, 13 frames (0.3 %) over 16.7 ms, 51 animated characters drawn in
every frame. It passes (above 60 fps; p99 under 16.7 ms). The M4 record above had 0
frames over 16.7 ms without the M4.5 systems.

**M3 flights** with everything on (paced at 60 fps like the M3 baseline):

| Run | Command | Frames | Longest frame | p50 / p99 | Over 50 ms | Near-ring holes | Peak blocks / bundles / GPU / RSS | Result |
|---|---|---|---|---|---|---|---|---|
| M3-a | `--headless --size 1280x720 --fly-path 1:0:118,1:254:118 --speed 1000` | 2,399 | 35.5 ms | 16.7 / 21.9 ms | 0 | 0 | 622 / 1,111 MiB / 1,638 MiB / 3,240 MiB | pass |
| M3-b | `--headless --size 1280x720 --fly-path 2:0:0,2:254:254 --speed 3000` | 1,127 | 42.8 ms | 16.7 / 23.5 ms | 0 | **20** (one landblock) | 622 / 1,415 MiB / 343 MiB / 3,180 MiB | **fails** (holes) |
| M3-d | `--headless --size 1920x1080 --fly-path 5:0:0,5:254:254 --speed 2000` | 1,630 | 55.3 ms | 16.7 / 39.2 ms | **2** | 0 | 624 / 2,629 MiB / 533 MiB / 4,656 MiB | **fails** (2 frames) |

Every bundle requested was integrated (0 failed, 0 evicted); the residency budget held
(peak 2,629 MiB of 4,096 MiB).

- **M3-b holes:** one near-ring landblock missing for 13–21 frames near the end of the
  path, in every run. The same run with every M4.5 system off (`--disable all`) had 6
  frames, and with the M3 ring size (`--view-blocks 8`) 21: the bundle of that landblock
  arrives late at 3,000 m/s whatever the walker draws. The M3 baseline had none; bundles
  now carry the world systems (collision sets, generated scenery, water) and take longer
  to build.
- **M3-d frames over 50 ms:** 0, 0 and 2 in the last three runs (55.3 and 50.2 ms). In
  those frames the main thread's measured phases add up to about 25 ms (terrain update
  up to 14 ms and object update up to 11 ms while landblocks arrive at 2,000 m/s); the
  rest is time the main thread did not run, with 19 worker threads and the other
  session's job on 16 hardware threads.
- These runs found and fixed a starvation: the walker's terrain fell behind a fast
  camera (no terrain drawn for seconds in M3-a and M3-d) because the upload budget's time
  ran from the start of the frame and the 2 terrain workers worked through a first-in
  first-out queue. `walker.md` §14.1 and §14.4 record the fixes; terrain is now drawn in
  all but 9 (M3-a: the last frames, at the end of the path), 57 (M3-b: four stretches of
  0.03–0.65 s) and 58 (M3-d: 8.9–10.6 s, a stretch where a run with `--disable layers`
  drew no terrain either, and the last frames) frames.
- **M3-b fails criterion 5** on bundle latency, not on load: the holes appear in every
  run, also with `--disable all` and with the M3 ring size, so a quiet machine is not
  expected to remove them. It needs a streaming or bundle-cost fix (`walker.md` §14.11
  OPEN, for the world implementer).
- **M3-d is provisional:** its frames over 50 ms are plausibly background load (the main
  thread's measured phases add up to about half of them). Re-run it with nothing else
  running before treating it as failed. (Re-run in the exit verification below: it passes.)

### Re-run in the M4.5 exit verification (2026-10-06, later the same day)

Release build of the same working tree after the verification's fixes, same settings, with
only a browser and btop running (the other session's job had ended; load average 3.8 before
the runs). Walk spots, `--headless --unpaced --duration 60 --size 1920x1080`, p50 / p99 /
max: P1 7.6 / 10.8 / 15.3 ms, P2 8.3 / 14.4 / 19.8 ms, P3 4.6 / 10.0 / 15.7 ms, P4 8.1 /
14.3 / 21.8 ms, P5 4.2 / 7.8 / 14.0 ms (all pass). Crowd of 50 with everything on: 6,351
frames, 105.8 fps mean, p99 13.7 ms, 1 frame over 16.7 ms, 51 characters drawn every frame
(pass). Flights: M3-a 2,425 frames, longest 28.8 ms, 0 over 50 ms, 0 holes (pass); M3-d
1,698 frames, longest 34.5 ms, 0 over 50 ms, 0 holes, peak 2,684 MiB bundles (pass, so the
earlier 2 frames were background load); M3-b longest 25.8 ms, 0 over 50 ms, but 6, 5 and 8
frames with one near-ring landblock missing in three runs (**fails**, bundle latency as
above).

## Log

- 2026-10-04: the GTX 1070 Ti workstation proposed as the reference. M0–M3 were developed
  on it, and the M3 performance evidence in the ROADMAP was recorded there.
- 2026-10-05: the owner first confirmed the workstation, then, because it is not
  available, accepted this laptop for the runs ([decision 0010](../decisions/0010-reference-machine-laptop.md)).
  The M3 baseline and M4 criterion 4 above were measured the same day.
- 2026-10-06: M4.5 criterion 5 measured on this laptop (walk spots, crowd, M3 flights with
  every M4.5 system on).
- 2026-10-06: M4.5 criterion 5 re-run in the exit verification: everything passes except
  M3-b's near-ring holes.
