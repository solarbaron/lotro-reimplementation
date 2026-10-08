<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M6 audio: underruns, mixer load and frame-time impact (exit criterion 4)

[ROADMAP M6](../ROADMAP.md#m6) criterion 4: "No audio underruns during a 10-minute world
flight on the reference machine."

> **INDICATIVE.** These runs were made on the reference laptop
> ([decision 0010](../decisions/0010-reference-machine-laptop.md)) while other milestone
> builds loaded its CPU (load average 3.1–4.3 on 16 threads during the runs). Under
> decision 0010 such runs are indicative: **to be re-measured on an idle reference
> machine** before the criterion is closed.

## Setup

- Machine: the reference laptop (i9-10885H, RTX 2070 Super Max-Q, Gentoo, Hyprland),
  2026-10-07, release build of the M6 worktree, `CARGO_BUILD_JOBS=6` builds of other
  milestones running at the same time.
- Audio: the **device backend** (cpal on ALSA, through the PipeWire PulseAudio/ALSA layer;
  "Default Audio Device", 48 kHz, stereo, f32). A scratch settings file
  (`TESSERA_SETTINGS_FILE`) set `[audio] master_volume = 0.05` so the run was quiet; the
  mixer does the same work at any volume above the silence gate (`sound.md` §6.8 item 8).
- World: the walker's music graph on the avatar, ambience and positional emitters
  (static entities whose start-up scripts make sound) within two landblocks of the viewer.

```sh
cargo build --release -p tessera-client --features audio-device
TESSERA_SETTINGS_FILE=<scratch>/settings.toml \
tessera-client --headless --audio device --audio-settings user \
  --fly-path 1:10:120:80:80,1:240:120:80:80 --speed 61 --size 1280x720 \
  --frame-log perf-device.csv
```

The device backend needs the `audio-device` feature (opt-in, decision 0019; it was on by
default when these runs were made). Headless runs ignore the user's settings file unless
`--audio-settings user` (or a file) is given; when these runs were made they always read it,
so `--audio-settings user` repeats them exactly.

Region 1, landblock row 120 from landblock 10 to 240 (36.8 km) at 61 m/s, paced at 60
frames per second, rendered offscreen at 1280 × 720.

## Result: 10-minute flight

| Measure | Value |
|---|---|
| Flight | 603.3 s, 36,145 frames |
| **Device underruns (xruns)** | **0** |
| Mixer overloads (a callback slower than the audio it produced) | 0 |
| Streamed music starved blocks | 0 |
| Mixer callbacks | 56,586 |
| Mixer load (render time / audio time of the last callback, sampled every frame) | mean 4.3 %, p99 14.7 %, max 56.2 % (one frame) |
| Area-music changes | 18 |
| Voices audible | mean 2.1, max 5 |
| Payload loads (worker threads) | 53; 0 sound infos and 0 script records read on the main thread |
| Main-thread audio update | mean 0.035 ms/frame, p99 0.060 ms, max 0.99 ms (in the flight); 19 ms once at start-up (the music graph's first run, before the flight) |
| Share of main-thread frame work | 3.3 % (0.035 of 1.07 ms) |
| Frame time | p50 16.7 ms, p99 16.7 ms, max 35.7 ms; 0 frames over 50 ms |

## Frame-time impact: two-minute A/B

The first two minutes of the same row (landblock 10 to 56, 61 m/s), once with `--audio off`
and once with `--audio device`, one after the other:

| | `--audio off` | `--audio device` |
|---|---|---|
| Frames | 7,231 | 7,232 |
| Main-thread work per frame | mean 0.751 ms, p99 1.53 ms, max 5.70 ms | mean 0.813 ms, p99 1.75 ms, max 4.53 ms |
| Audio update | — | mean 0.035 ms, max 0.33 ms |
| Frame time | p50 16.68, p99 16.72, max 16.85 ms | p50 16.68, p99 16.72, max 16.85 ms |
| Underruns | — | 0 |

Audio adds about 0.06 ms of main-thread work per frame (the engine update, the graph
advance and the property checks; the mixer runs on the device's own thread) and does not
change the paced frame times.

## Notes

1. The single 56 % mixer-load callback is one period; the p99 is 14.7 %. Whether it
   coincided with a load spike from the parallel builds is not known; re-measure idle.
2. The null-backend verification of criterion 2 is in
   [`m6-audio.md`](../checklists/m6-audio.md).
3. The frame log's audio columns (`audio_ms, audio_voices, audio_underruns, mixer_load`) and
   the summary's audio part are written by every walker run with audio on.
4. After these runs the walker was limited to spawning 6 emitters per 0.25 s check (nearest
   first), which lowers the largest main-thread audio update during fast flights (region-1
   row 118 at 1,000 m/s: audio max 1.9–4.3 ms per frame afterwards, 5.8 ms before). It does
   not change the mixer's work.
