<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M5 UI budgets U1–U5 (ROADMAP M5 criterion 7)

| | |
|---|---|
| **Status** | **INDICATIVE.** Measured on the reference machine of [decision 0010](../decisions/0010-reference-machine-laptop.md) while it was **not idle** (see Conditions). To be re-measured on the idle reference machine before criterion 7 is judged (rule 1 of [`reference-machine.md`](reference-machine.md)). |
| **Budgets** | [Decision 0015](../decisions/0015-ui-rendering.md) §6: U1 UI main-thread time, U2 GPU time of the UI pass at 3840 × 2160, U3 UI upload bytes, U4 event frames, U5 memory |
| **Run** | The scripted run of 0015 §6, `--ui-script` (`apps/client/src/ui_host/script.rs`), headless, release build, paced at 60 frames per second |

## How to run it

```sh
export TESSERA_GAME_DIR=...   # the install, read-only
cargo build --release -p tessera-client
# 3840 x 2160 offscreen
target/release/tessera-client --headless --ui-script --walker-hud off \
    --size 3840x2160 --ui-report /tmp/u-2160.md
# the window size the compositor gives: windowed, same script
target/release/tessera-client --ui-script --walker-hud off --ui-report /tmp/u-window.md
```

The run writes its own synthetic plugin (three windows: a stretch-mode child with
`MouseMove` handlers, a window rotated by `SetRotation` *z* = 90 holding a stretch-mode
child, and a screen-sized window with a full-size background) into
`<data dir>/ui-script/Plugins` (never into the install), opens the HUD set, P7 and P9, waits
until the world around the spawn has settled and the plugin is loaded, then plays the phases
listed in the module documentation of `script.rs`: walk 20 s with key turns, mouse-look and
the run chord; pointer sweeps at the frame rate and at 1000 Hz (17 cursor events per frame);
wheel notches over P7; 10 chat lines per second; every panel opened and closed; drags of P10
and P15; `SetSize` on a plugin window every frame with a second stretch child shown; panel
scale 1.0 → 1.5 → 1.0; a screen resize with a stretch child shown in the screen-sized window.
It records one sample per frame (`apps/client/src/ui_host/stats.rs`) and prints the report;
`--ui-report` saves it.

How the figures are taken:

- **U1** is the client host's own time in frame-loop steps 1, 7 and 8 minus the plugin
  host's Lua time measured over the same frame (`PluginManager::take_plugin_time`). Frames
  inside an **event window** and content-event frames are left out of the steady-state
  figure, as §6 defines. An event window is the trigger frame of an event (panel opened or
  hidden, screen resize, scale change, drags, plugin resize) plus every following frame
  until the UI is quiet again (no band painted, nothing uploaded, no image or font job in
  flight), at most 41 frames (the U4 (e) bound). Content-event frames are a consumed wheel
  step (`ScrollUp` / `ScrollDown`), a chat line or a submitted chat line. *(Since the review
  fixes of 2026-10-08. Before them only the trigger frame counted as an event frame, and a
  consumed wheel step was never recognised because the check looked for "Wheel" in the
  command name; the 2026-10-08 results of the first run, kept below for comparison,
  therefore counted wheel repaints and the frames that continue an event's jobs as steady
  state.)*
- **U2**: the adapter's timestamp queries are not used by `tessera-render` yet
  (`render.md` §6.16 OPEN, `UiStats::gpu_ns` is `None`). The report gives a **proxy**: the
  median time to submit a frame and wait for the GPU, with and without the UI pass,
  interleaved over 120 frames. It is not a gate (§6: "otherwise not a gate").
- **U3** is the scheduler's upload bytes per frame (`PaintFrameStats::upload_bytes`).
- **U4** (a) is the longest frame time without pacing (steps 1–10 and the GPU wait) inside
  an event window, (b) the longest step-8 time inside an event window, (f) the UI time of
  content-event frames. **U4 (c)–(e)** (first draw within 100 ms of being shown; each job
  of a surface up to 1038 × 802 within 4 frames; the screen-sized window's first job within
  40 frames) are **not measured** yet: the recorder has no show-to-first-draw or job
  latency figures. The report lists them as "not measured", and criterion 7 stays open on
  them. The report also gives the longest frame of the whole run against the M3 50 ms rule.
- **U5** are the largest canvas and texture bytes the scheduler reports and the decoded-image
  bytes of the client's image cache (pinned = still referenced by a canvas).

### Approximations of the scripted run

1. The run is headless (offscreen) unless started without `--headless`; see Conditions.
2. U2 is a proxy (above).
3. Plugin Lua time is measured per frame, not per call.
4. **The screen phase.** Headless, the "window resize" changes the UI's screen, the offscreen
   render target and the walker's viewport together (since 2026-10-08; before that only the
   UI's screen changed and the UI was laid out for 3/4 of the size on a full-size frame).
   Windowed, the run asks the window for the new size; the resize event then changes the
   renderer, the viewport and the UI. A compositor that refuses the size (tiling) leaves
   the UI at the asked size inside the unchanged window.
5. The phases start from the UI's screen size when the script starts (in a windowed run,
   the size the compositor gave, not `--size`).

### Open items found by the review of 2026-10-08

- `[UI] ui_scale_global` reaches every native panel through the panels' scalers, also
  panels opened after the HUD (fixed 2026-10-08). `tessera-ui` has no setter for its own
  `UIScale_Global`, so a plugin window's `SetScale` does not include it; that needs a
  setter from the `tessera-ui` owner (decision 0015 §6 implementation OPEN).
- `[UI] ui_scale_filter = nearest` now samples every UI entry nearest (the client
  overrides the composition list's filter); a per-entry choice inside `tessera-ui` would
  need its owner.
- Plugin image files and install Image-item files share `tessera-ui`'s `ImageRef::File`
  with no origin. The client now decodes each root with its own origin (plugin files as
  plugin images, install files as install images), but a file under `Plugins/` still
  shadows an Image-item file of the same relative path. Needs an origin in `ImageRef::File`
  from the `tessera-ui` owner.
- The double-click time and rectangle come from `[Input] double_click_ms` and
  `double_click_px` (or 500 ms and 4 × 4 logical pixels), scaled by the window's scale
  factor; the client reads no platform values yet (`keymap.md` §7.7 names them; winit
  offers none and a plain Wayland compositor publishes none).

## Conditions (2026-10-08)

- Machine: the reference laptop (i9-10885H, RTX 2070 Super Max-Q, driver 615.71.09, Vulkan),
  on AC, governor `powersave`, platform profile `balanced`.
- **Not idle:** other agents were compiling and running test suites at the same time (load
  average about 20 on 16 hardware threads; `rustc`, a real-data decoder test at about 700 %
  CPU, btop). The figures are therefore upper bounds with large noise, in particular the
  p99 and maximum values and U2.
- Build: release, rustc 1.99.0, worktree of branch `m5` at `a292f17` plus the uncommitted
  M5 changes.
- Archives in the page cache (earlier runs had read them). Offscreen, no vsync (paced at
  60 frames per second by sleeping), `--walker-hud off`, no user skin, spawn at the default
  block of region 1.
- **Window size:** the windowed run at the compositor's size was not made in this session
  (it needs the owner's desktop); the 1920 × 1080 offscreen run below stands in for it until
  then.

## Results after the review fixes (second run, 2026-10-08)

Same machine, build and command lines as below, with the review fixes of 2026-10-08 in the
tree (event windows, wheel steps as content events, U4 (c)–(e) rows, the overall frame
maximum, the screen phase resizing the render target and the viewport, the string tables
preloaded by the start-up thread, the O(1) image-cache figures). **Still not idle** (load
average 22–48 on 16 hardware threads during the runs). A first 3840 × 2160 attempt of this
session is discarded: it contained one 7.5 s frame in the `panels` phase while the load
average was 48 (a machine stall, not UI work); the run below is its repeat.

#### 3840x2160 offscreen, NVIDIA GeForce RTX 2070 Super with Max-Q Design

| Measure | Phase | Value | Budget |
|---|---|---|---|
| U1 UI main-thread time (steady) | walk | mean 1.067 ms, p99 9.033 ms, max 15.359 ms (1200 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep | mean 1.548 ms, p99 18.860 ms, max 29.584 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep1000 | mean 0.859 ms, p99 2.657 ms, max 3.158 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | all | mean 1.191 ms, p99 8.437 ms (2817 frames) | — |
| U3 UI upload bytes | steady mean / max | 73.4 KiB / 8192.0 KiB | mean ≤ 256 KiB, cap 8 MiB |
| U4 (a) longest frame in an event window | event windows | 54.0 ms | ≤ 50 ms |
| U4 (b) longest UI paint in an event window | event windows | 28.56 ms | ≤ 4 ms |
| U4 (c) first draw within 100 ms of being shown | — | not measured | ≤ 100 ms |
| U4 (d) each job of a surface up to 1038 × 802 within 4 frames | — | not measured | ≤ 4 frames |
| U4 (e) first job of the screen-sized window within 40 frames | — | not measured | ≤ 40 frames |
| U4 (f) longest UI time in a content-event frame | content frames | 7.64 ms | ≤ 4 ms |
| U5 memory (peak) | all | canvases 45.2 MiB, textures 89.3 MiB, images 11.0 MiB (pinned 8.7 MiB) | 128 / 256 / 64 LRU / 512 MiB |
| Longest frame overall (M3 rule) | all | 54.0 ms (3630 frames) | ≤ 50 ms |
| Plugin Lua time (reported apart) | all | mean 0.159 ms, p99 0.664 ms, max 9.231 ms | (U4 frame rule) |
| Pointer offers per frame (coalescing) | sweep1000 | mean 1.00, max 1 | ≤ 1 per other event + 1 |
| U2 proxy: median frame submit+GPU-wait time with / without the UI pass | | 6.336 ms / 5.607 ms (difference 0.729 ms; no timestamp queries) | |

| Phase | Frames | U1 mean (all frames) | step 1 / step 7 / step 8 mean (host, plugins incl.) | Plugin mean | Frame mean | Frame max |
|---|---|---|---|---|---|---|
| startup | 28 | 8.795 ms | 0.001 / 0.389 / 6.650 ms | 0.350 ms | 18.47 ms | 53.98 ms |
| prepare | 2 | 3.322 ms | 0.000 / 0.174 / 3.142 ms | 0.113 ms | 12.29 ms | 15.11 ms |
| settle | 180 | 1.128 ms | 0.001 / 0.149 / 0.982 ms | 0.119 ms | 7.10 ms | 19.26 ms |
| walk | 1200 | 1.067 ms | 0.001 / 0.162 / 1.001 ms | 0.129 ms | 7.34 ms | 23.90 ms |
| sweep | 180 | 1.548 ms | 0.022 / 0.157 / 1.322 ms | 0.128 ms | 8.27 ms | 42.22 ms |
| sweep1000 | 180 | 0.859 ms | 0.024 / 0.174 / 0.800 ms | 0.141 ms | 7.68 ms | 17.61 ms |
| wheel | 180 | 0.983 ms | 0.004 / 0.222 / 0.938 ms | 0.182 ms | 8.98 ms | 24.96 ms |
| chat | 300 | 2.502 ms | 0.001 / 0.411 / 2.422 ms | 0.340 ms | 14.68 ms | 34.55 ms |
| panels | 300 | 3.379 ms | 0.001 / 0.253 / 2.972 ms | 0.182 ms | 12.62 ms | 35.48 ms |
| drag10 | 200 | 1.566 ms | 0.020 / 0.176 / 1.470 ms | 0.140 ms | 10.51 ms | 24.83 ms |
| drag15 | 200 | 1.284 ms | 0.043 / 0.174 / 1.215 ms | 0.152 ms | 11.16 ms | 17.58 ms |
| pluginresize | 200 | 2.619 ms | 0.001 / 0.192 / 1.945 ms | 0.151 ms | 10.44 ms | 18.67 ms |
| scale | 240 | 1.173 ms | 0.001 / 0.164 / 1.134 ms | 0.127 ms | 10.59 ms | 18.98 ms |
| screen | 240 | 1.265 ms | 0.001 / 0.187 / 1.222 ms | 0.146 ms | 11.10 ms | 22.47 ms |

#### 1920x1080 offscreen, NVIDIA GeForce RTX 2070 Super with Max-Q Design

| Measure | Phase | Value | Budget |
|---|---|---|---|
| U1 UI main-thread time (steady) | walk | mean 0.868 ms, p99 2.355 ms, max 13.934 ms (1200 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep | mean 0.986 ms, p99 9.595 ms, max 10.425 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep1000 | mean 0.672 ms, p99 1.519 ms, max 1.775 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | all | mean 0.819 ms, p99 2.260 ms (2870 frames) | — |
| U3 UI upload bytes | steady mean / max | 29.3 KiB / 8192.0 KiB | mean ≤ 256 KiB, cap 8 MiB |
| U4 (a) longest frame in an event window | event windows | 53.8 ms | ≤ 50 ms |
| U4 (b) longest UI paint in an event window | event windows | 21.19 ms | ≤ 4 ms |
| U4 (c) first draw within 100 ms of being shown | — | not measured | ≤ 100 ms |
| U4 (d) each job of a surface up to 1038 × 802 within 4 frames | — | not measured | ≤ 4 frames |
| U4 (e) first job of the screen-sized window within 40 frames | — | not measured | ≤ 40 frames |
| U4 (f) longest UI time in a content-event frame | content frames | 4.93 ms | ≤ 4 ms |
| U5 memory (peak) | all | canvases 21.4 MiB, textures 24.9 MiB, images 11.0 MiB (pinned 9.0 MiB) | 128 / 256 / 64 LRU / 512 MiB |
| Longest frame overall (M3 rule) | all | 53.8 ms (3629 frames) | ≤ 50 ms |
| Plugin Lua time (reported apart) | all | mean 0.117 ms, p99 0.326 ms, max 8.248 ms | (U4 frame rule) |
| Pointer offers per frame (coalescing) | sweep1000 | mean 1.00, max 1 | ≤ 1 per other event + 1 |
| U2 proxy: median frame submit+GPU-wait time with / without the UI pass | | 5.256 ms / 4.729 ms (difference 0.527 ms; no timestamp queries) | |

| Phase | Frames | U1 mean (all frames) | step 1 / step 7 / step 8 mean (host, plugins incl.) | Plugin mean | Frame mean | Frame max |
|---|---|---|---|---|---|---|
| startup | 27 | 4.434 ms | 0.001 / 0.475 / 3.560 ms | 0.431 ms | 12.85 ms | 53.81 ms |
| prepare | 2 | 8.210 ms | 0.001 / 0.248 / 7.155 ms | 0.176 ms | 15.11 ms | 17.57 ms |
| settle | 180 | 0.780 ms | 0.001 / 0.128 / 0.723 ms | 0.102 ms | 4.28 ms | 14.20 ms |
| walk | 1200 | 0.868 ms | 0.001 / 0.141 / 0.820 ms | 0.113 ms | 7.41 ms | 26.66 ms |
| sweep | 180 | 0.986 ms | 0.023 / 0.146 / 0.891 ms | 0.119 ms | 4.46 ms | 15.01 ms |
| sweep1000 | 180 | 0.672 ms | 0.023 / 0.143 / 0.622 ms | 0.117 ms | 4.40 ms | 8.11 ms |
| wheel | 180 | 0.662 ms | 0.003 / 0.148 / 0.630 ms | 0.120 ms | 6.94 ms | 18.53 ms |
| chat | 300 | 0.866 ms | 0.001 / 0.166 / 0.817 ms | 0.122 ms | 5.87 ms | 13.26 ms |
| panels | 300 | 1.861 ms | 0.001 / 0.156 / 1.658 ms | 0.117 ms | 9.16 ms | 28.20 ms |
| drag10 | 200 | 1.133 ms | 0.015 / 0.133 / 1.068 ms | 0.106 ms | 9.12 ms | 21.59 ms |
| drag15 | 200 | 0.995 ms | 0.035 / 0.131 / 0.944 ms | 0.118 ms | 7.73 ms | 10.56 ms |
| pluginresize | 200 | 2.717 ms | 0.001 / 0.178 / 1.935 ms | 0.139 ms | 9.65 ms | 42.71 ms |
| scale | 240 | 0.941 ms | 0.001 / 0.141 / 0.908 ms | 0.109 ms | 7.76 ms | 12.12 ms |
| screen | 240 | 0.886 ms | 0.001 / 0.136 / 0.853 ms | 0.106 ms | 7.25 ms | 10.51 ms |

## Exit-verification run (third run, 2026-10-08)

The M5 exit verification repeated the 3840 × 2160 offscreen run once on the same machine and
build (plus the verification's mechanical fixes: neutral module names in `tessera-plugins`,
doc links, fuzz lints), with the same command line. **Still not idle:** load average 13 at
the start and 8 at the end (other agents' builds). Indicative only.

| Measure | Value | Budget | Verdict (indicative) |
|---|---|---|---|
| U1 walk (steady) | mean 0.887 ms, p99 8.399 ms, max 21.5 ms (1200 frames) | mean ≤ 0.3, p99 ≤ 1.0 ms | fails |
| U1 sweep / sweep1000 | mean 1.186 / 0.604 ms, p99 13.3 / 1.48 ms | same | fails |
| U2 proxy | 3.652 / 3.049 ms with / without the UI pass (difference 0.603 ms) | 0.3 ms where timestamp queries exist | not a gate (no timestamp queries) |
| U3 | steady mean 72.1 KiB, max 8 MiB | mean ≤ 256 KiB, cap 8 MiB | passes |
| U4 (a) | 33.5 ms | ≤ 50 ms | passes on this run (54.0 ms on the second run under higher load) |
| U4 (b) | 12.78 ms | ≤ 4 ms | fails |
| U4 (c)–(e) | not measured | | not judged |
| U4 (f) | 6.08 ms | ≤ 4 ms | fails |
| U5 | canvases 45.2 MiB, textures 90.0 MiB, images 11.0 MiB (pinned 9.0) | 128 / 256 / 64 / 512 MiB | passes |
| Longest frame overall | 33.5 ms (3628 frames) | ≤ 50 ms | passes |
| Pointer offers per frame (sweep1000) | mean 1.00, max 1 | | coalesced |

The pattern of the second run holds: U1, U4 (b) and U4 (f) fail inside `tessera-ui`'s step 8;
U3 and U5 pass; U4 (a) depends on machine load. Criterion 7 stays **not met** until the idle
re-measurement (owner observation O49) and U4 (c)–(e) figures exist.

## First run (2026-10-08, before the review fixes)

Kept for comparison. Wheel repaints and the frames that continue an event's jobs were
counted as steady state here (see "How the figures are taken"), and U4 (a)/(b) were judged on
trigger frames only.

#### 3840x2160 offscreen, NVIDIA GeForce RTX 2070 Super with Max-Q Design

| Measure | Phase | Value | Budget |
|---|---|---|---|
| U1 UI main-thread time (steady) | walk | mean 1.922 ms, p99 9.060 ms, max 26.127 ms (1200 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep | mean 3.455 ms, p99 28.554 ms, max 29.851 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep1000 | mean 1.982 ms, p99 10.497 ms, max 12.011 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | all | mean 2.410 ms, p99 19.515 ms (2960 frames) | — |
| U3 UI upload bytes | steady mean / max | 199.7 KiB / 8192.0 KiB | mean ≤ 256 KiB, cap 8 MiB |
| U4 (a) longest event frame | event frames | 43.6 ms | ≤ 50 ms |
| U4 (b) longest UI paint in an event frame | event frames | 11.48 ms | ≤ 4 ms |
| U4 (f) longest UI time in a content-event frame | content frames | 21.02 ms | ≤ 4 ms |
| U5 memory (peak) | all | canvases 45.2 MiB, textures 85.6 MiB, images 11.0 MiB (pinned 9.4 MiB) | 128 / 256 / 64 LRU / 512 MiB |
| Plugin Lua time (reported apart) | all | mean 0.176 ms, p99 0.634 ms, max 28.342 ms | (U4 frame rule) |
| Pointer offers per frame (coalescing) | sweep1000 | mean 1.00, max 1 | ≤ 1 per other event + 1 |
| U2 proxy: median frame submit+GPU-wait time with / without the UI pass | | 5.832 ms / 4.994 ms (difference 0.838 ms; no timestamp queries) | |

| Phase | Frames | U1 mean (all frames) | step 1 / step 7 / step 8 mean (host, plugins incl.) | Plugin mean | Frame mean | Frame max |
|---|---|---|---|---|---|---|
| startup | 45 | 7.964 ms | 0.000 / 0.814 / 6.420 ms | 0.779 ms | 22.17 ms | 44.81 ms |
| prepare | 2 | 9.498 ms | 0.000 / 0.198 / 9.213 ms | 0.094 ms | 24.32 ms | 27.27 ms |
| settle | 180 | 2.649 ms | 0.001 / 0.203 / 2.281 ms | 0.170 ms | 13.63 ms | 62.65 ms |
| walk | 1200 | 1.922 ms | 0.001 / 0.211 / 1.788 ms | 0.171 ms | 16.76 ms | 46.49 ms |
| sweep | 180 | 3.455 ms | 0.040 / 0.194 / 3.063 ms | 0.159 ms | 17.55 ms | 47.04 ms |
| sweep1000 | 180 | 1.982 ms | 0.026 / 0.205 / 1.919 ms | 0.172 ms | 15.97 ms | 42.40 ms |
| wheel | 180 | 2.597 ms | 0.004 / 0.216 / 2.552 ms | 0.176 ms | 20.81 ms | 88.67 ms |
| chat | 300 | 3.343 ms | 0.001 / 0.263 / 3.259 ms | 0.191 ms | 20.29 ms | 52.55 ms |
| panels | 300 | 3.460 ms | 0.001 / 0.214 / 3.013 ms | 0.168 ms | 13.38 ms | 57.69 ms |
| drag10 | 200 | 2.486 ms | 0.018 / 0.216 / 2.386 ms | 0.168 ms | 13.77 ms | 32.18 ms |
| drag15 | 200 | 1.652 ms | 0.049 / 0.171 / 1.584 ms | 0.156 ms | 10.42 ms | 19.96 ms |
| pluginresize | 200 | 4.022 ms | 0.000 / 0.231 / 3.162 ms | 0.188 ms | 13.32 ms | 26.52 ms |
| scale | 240 | 2.430 ms | 0.000 / 0.187 / 2.393 ms | 0.152 ms | 13.88 ms | 41.37 ms |
| screen | 240 | 1.552 ms | 0.000 / 0.175 / 1.516 ms | 0.142 ms | 10.56 ms | 19.31 ms |

#### 1920x1080 offscreen, NVIDIA GeForce RTX 2070 Super with Max-Q Design

| Measure | Phase | Value | Budget |
|---|---|---|---|
| U1 UI main-thread time (steady) | walk | mean 1.604 ms, p99 7.233 ms, max 23.882 ms (1200 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep | mean 1.933 ms, p99 21.107 ms, max 21.182 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | sweep1000 | mean 1.124 ms, p99 3.924 ms, max 4.250 ms (180 frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |
| U1 UI main-thread time (steady) | all | mean 2.047 ms, p99 10.215 ms (2962 frames) | — |
| U3 UI upload bytes | steady mean / max | 84.1 KiB / 8192.0 KiB | mean ≤ 256 KiB, cap 8 MiB |
| U4 (a) longest event frame | event frames | 40.7 ms | ≤ 50 ms |
| U4 (b) longest UI paint in an event frame | event frames | 18.41 ms | ≤ 4 ms |
| U4 (f) longest UI time in a content-event frame | content frames | 7.96 ms | ≤ 4 ms |
| U5 memory (peak) | all | canvases 21.4 MiB, textures 28.4 MiB, images 11.0 MiB (pinned 9.0 MiB) | 128 / 256 / 64 LRU / 512 MiB |
| Plugin Lua time (reported apart) | all | mean 0.167 ms, p99 1.126 ms, max 18.856 ms | (U4 frame rule) |
| Pointer offers per frame (coalescing) | sweep1000 | mean 1.00, max 1 | ≤ 1 per other event + 1 |
| U2 proxy: median frame submit+GPU-wait time with / without the UI pass | | 7.876 ms / 6.257 ms (difference 1.619 ms; no timestamp queries) | |

| Phase | Frames | U1 mean (all frames) | step 1 / step 7 / step 8 mean (host, plugins incl.) | Plugin mean | Frame mean | Frame max |
|---|---|---|---|---|---|---|
| startup | 47 | 3.878 ms | 0.000 / 0.583 / 3.105 ms | 0.544 ms | 17.39 ms | 62.28 ms |
| prepare | 2 | 7.183 ms | 0.001 / 0.333 / 6.995 ms | 0.223 ms | 21.99 ms | 28.64 ms |
| settle | 180 | 2.246 ms | 0.000 / 0.187 / 2.062 ms | 0.146 ms | 12.82 ms | 40.09 ms |
| walk | 1200 | 1.604 ms | 0.001 / 0.200 / 1.527 ms | 0.160 ms | 12.64 ms | 42.48 ms |
| sweep | 180 | 1.933 ms | 0.025 / 0.184 / 1.705 ms | 0.146 ms | 13.65 ms | 37.73 ms |
| sweep1000 | 180 | 1.124 ms | 0.025 / 0.197 / 1.066 ms | 0.166 ms | 12.18 ms | 21.90 ms |
| wheel | 180 | 1.326 ms | 0.003 / 0.213 / 1.282 ms | 0.174 ms | 14.00 ms | 27.97 ms |
| chat | 300 | 2.100 ms | 0.001 / 0.250 / 2.026 ms | 0.193 ms | 15.16 ms | 31.28 ms |
| panels | 300 | 3.728 ms | 0.000 / 0.198 / 3.480 ms | 0.146 ms | 16.06 ms | 42.91 ms |
| drag10 | 200 | 3.657 ms | 0.018 / 0.202 / 3.577 ms | 0.168 ms | 16.51 ms | 37.58 ms |
| drag15 | 200 | 3.330 ms | 0.041 / 0.198 / 3.252 ms | 0.163 ms | 14.69 ms | 23.62 ms |
| pluginresize | 200 | 6.169 ms | 0.001 / 0.220 / 5.037 ms | 0.175 ms | 18.14 ms | 37.54 ms |
| scale | 240 | 3.042 ms | 0.000 / 0.193 / 3.005 ms | 0.158 ms | 14.48 ms | 26.90 ms |
| screen | 240 | 2.951 ms | 0.001 / 0.193 / 2.913 ms | 0.157 ms | 14.61 ms | 24.79 ms |

## Reading

Second run (after the review fixes); indicative, the machine was loaded.

| Budget | 3840 × 2160 | 1920 × 1080 | Verdict (indicative) |
|---|---|---|---|
| U1 walking phase | mean 1.07 ms, p99 9.03 ms | mean 0.87 ms, p99 2.36 ms | **fails** (budget mean 0.3 ms, p99 1.0 ms) |
| U1 1000 Hz sweep | mean 0.86 ms, p99 2.66 ms | mean 0.67 ms, p99 1.52 ms | **fails** |
| U2 | proxy difference 0.73 ms | proxy difference 0.53 ms | not a gate without timestamp queries; the proxy is dominated by noise on a loaded GPU |
| U3 | steady mean 73 KiB, max 8 MiB (the cap) | steady mean 29 KiB, max 8 MiB | **passes** |
| U4 (a) | 54.0 ms (in the start-up event window: HUD, P7 and P9 opening) | 53.8 ms (same window) | **fails** narrowly on a loaded machine (≤ 50 ms); re-measure idle. The first run's "passes" (43.6 ms) judged trigger frames only and is withdrawn |
| U4 (b) | 28.6 ms | 21.2 ms | **fails** (slice 4 ms) |
| U4 (c)–(e) | not measured | not measured | **not judged**: the recorder has no show-to-first-draw or job-latency figures yet; criterion 7 stays open on them |
| U4 (f) | 7.64 ms | 4.93 ms | **fails** (4 ms) |
| U5 | canvases 45 MiB, textures 89 MiB, images 11 MiB | canvases 21 MiB, textures 25 MiB, images 11 MiB | **passes** |
| Longest frame overall (M3 50 ms rule) | 54.0 ms | 53.8 ms | over by 4 ms, in the start-up event window |

Where the time goes (the per-phase tables; the figures below are from the first run, the
second run shows the same pattern at lower absolute values): step 1 (offers, hit tests, plugin input
handlers) stays below 0.05 ms per frame even in the 1000 Hz sweep, because the mapper
coalesces pointer moves to one offer per frame (mean 1.00, maximum 1 in `sweep1000`), and
step 7 is about 0.2 ms with the plugin work included. **Step 8 (`Ui::paint`: layout,
damage, paint, composition) takes 1–3 ms in steady frames and up to 5 ms in the plugin-resize
phase**, so U1 and U4 (b)/(f) are decided inside `tessera-ui`'s step 8. In the walking phase
the minimap scrolls on every frame, as §6 predicted ("U1's mean of 0.3 ms is therefore at
risk in the walking phase"); the steady phases without walking (settle, sweep) are not
cheaper, which points at per-frame work in step 8 that does not depend on damage (for
example the composition list being rebuilt every frame). U4 (b) exceeding the 4 ms slice
means a single band or the non-sliced part of step 8 runs longer than the slice.

Next steps, before the owner's idle re-measurement:

1. Profile `Ui::paint` on the idle machine (the painter micro-benchmark
   `crates/ui/benches/paint_probe.rs` and a sampling profiler over this script); report
   to the `tessera-ui` owner whether the steady cost is the composition rebuild, the
   minimap repaint or layout.
2. If U4 (d)/(e) fail as well on the idle machine, move bands to the workers (§6 "Workers if
   needed").
3. Make the windowed run at the compositor's window size and add `render.md` §6.16 timestamp
   queries so that U2 can be judged.
