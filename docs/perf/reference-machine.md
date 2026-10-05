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

## Log

- 2026-10-04: the GTX 1070 Ti workstation proposed as the reference. M0–M3 were developed
  on it, and the M3 performance evidence in the ROADMAP was recorded there.
- 2026-10-05: the owner first confirmed the workstation, then, because it is not
  available, accepted this laptop for the runs ([decision 0010](../decisions/0010-reference-machine-laptop.md)).
  The M3 baseline and M4 criterion 4 above were measured the same day.
