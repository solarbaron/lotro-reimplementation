<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M6 audio checklist (exit criterion 2)

Pass/fail record for [ROADMAP M6](../ROADMAP.md#m6) criterion 2: "Area music changes with
crossfade when walking across area borders in the walker". The expected behaviour is
[`sound.md`](../specs/sound.md) §10.5: when `Area_Music` changes, the persistent Switch
stops the running case and starts the new one at once, so the old track fades out linearly
over its own `RampOutTime` (§6.5) and the new case's first step starts at once at full gain.
`Area_Music` is set by the offline Decision of §10.3.1 (block map `Area_DID` → the area's
`Ambient_MusicType`) on every landblock change.

This file records numbers and DIDs only: no area, track or label names (the HUD shows the
`AmbientMusicType` label of the current value; it is not copied here).

## Method

- **Install and date.** The owner's Steam install, read in place (`TESSERA_GAME_DIR`),
  2026-10-07. If the data changes, re-run the search below before judging.
- **Choosing the border.** `cargo test --release -p tessera-client --test audio_real_data
  area_music_for_landblocks -- --nocapture` computes `Area_Music` for every second region-1
  landblock and prints neighbouring landblocks with different values. Row 120 has
  `0x10000076` at landblock 124, `0x10000077` at 126 and `0x100000B1` at 130, so a flight
  from landblock (122, 120) to (132, 120) crosses two area borders.
- **Run.** Headless walker, null audio backend (the engine mixes against the wall clock and
  discards the output, so the voice timing is the device's):

  ```sh
  tessera-client --headless --audio null \
    --fly-path 1:122:120:80:80,1:132:120:80:80 --speed 40 --size 640x360 --frame-log fly.csv
  ```

  The walker logs every music event (`audio: music`) with the walker time `t` (seconds since
  the audio started) and the audio clock `audio_t`, and an `audio summary` at the end.
  The automated form is `music_changes_across_area_borders` in
  `apps/client/tests/audio_real_data.rs` (speed 150 m/s; it asserts both changes, a
  ramp-out with a positive time at each, a new track playing within 2 s, and no underruns).

## Result (2026-10-07, release build, laptop shared with other builds)

Run at 40 m/s (log excerpt, times in seconds; sound infos by DID):

| t | audio_t | Event |
|---|---|---|
| 0.206 | 0.173 | `Area_Music` none → `0x10000076` (avatar starts its start-up scripts) |
| 0.206 | 0.173 | track requested: sound info `0x2A00119E` |
| 0.320 | 0.287 | `0x2A00119E` playing |
| 10.697 | 10.653 | **border 1:** `Area_Music` `0x10000076` → `0x10000077` |
| 10.697 | 10.653 | `0x2A00119E` ramp-out starts, `RampOutTime` 4.0 s |
| 10.697 | 10.653 | new track requested: `0x2A0011AA` |
| 10.714 | 10.670 | `0x2A0011AA` playing (17 ms later: payload decode on a worker) |
| 14.687 | 14.641 | `0x2A00119E` ended (stopped after its ramp: 3.99 s) |
| 30.700 | 30.644 | **border 2:** `Area_Music` `0x10000077` → `0x100000B1` |
| 30.700 | 30.644 | `0x2A0011AA` ramp-out starts, `RampOutTime` 2.0 s |
| 30.700 | 30.644 | new track requested: `0x2A00329F` |
| 30.717 | 30.661 | `0x2A00329F` playing |
| 32.686 | 32.628 | `0x2A0011AA` ended (1.99 s after the ramp began) |

Summary line: 2 area changes, 0 underruns, 0 overloads, 0 starved stream blocks, no sound
info or script record read on the main thread, 10 emitters alive at the end, music and
ambient voices 1 each.

The automated test at 150 m/s gave the same pattern (changes at 3.358 s and 8.705 s, ramp-outs
of 4.0 s and 2.0 s ending 3.994 s and 2.003 s later, new tracks playing 17 ms and 16 ms after
each change).

| Row | Check | Result |
|---|---|---|
| A1 | `Area_Music` changes at each area border of the path | **pass** (2 of 2) |
| A2 | The old track fades out over its own `RampOutTime` (§6.5, §10.5) | **pass** (4.0 s and 2.0 s, ended within one frame of the ramp end) |
| A3 | The new area's track starts at once, at full gain (no fade-in, §6.5) | **pass** (requested in the same frame, playing 16–17 ms later) |
| A4 | Music plays from the start of the walk (avatar start-up scripts, §12.4) | **pass** |
| A5 | No underruns or stream starvation during the run | **pass** (null backend; device runs in [`m6-audio.md`](../perf/m6-audio.md)) |
| O1 | Owner: the cross-fade sounds like the original's at the same border | pending (owner observation) |

## Notes

1. The original's area-music precedence and granularity are OPEN (`sound.md` §10.3.1); the
   walker uses the Decision at landblock granularity. The walker has no dungeons, landmarks
   or cell test, so only the area rule is exercised in the world (the others by unit tests
   in `apps/client/src/audio/props.rs`).
2. The first avatar update can take about 25 ms on the main thread (the music graph is
   compiled when the start-up scripts first run); later audio updates stay near 1–3 ms.
3. Row O1 needs a windowed build with device output, which is opt-in (decision 0019):
   `cargo run --release -p tessera-client --features audio-device -- --spawn 1,122,120,80,80`.
   Without the feature the walker mixes into the null backend and the HUD reads
   `Audio null`. Headless runs use the built-in volume defaults (`--audio-settings default`)
   unless `--audio-settings user` or a file is given, so the rows above do not depend on the
   user's settings file.
