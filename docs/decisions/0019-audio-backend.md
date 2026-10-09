<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0019 — Audio backend: cpal for output, a project-owned mixer

**Status:** Accepted, 2026-10-07

## Context
`docs/ARCHITECTURE.md` §14 named "kira on cpal" as the planned audio output and asked for a
decision record at M6. The M6 specs fix the playback semantics exactly:

- `sound.md` §6.1–§6.8 sets the gain chain min(*v*, *v* × *a* × master × channel), the
  distance law (computed by the client with the library's own distance model off), the pitch
  multiplier, seamless looping over the whole payload, and a ramp-out **linear in amplitude**
  over the ramp-out time.
- It also defines the voice limit with per-frame scores and virtualisation (pause and keep),
  the gain-0 rule (a one-shot stops, a loop is virtualised) and twelve ordered request gates,
  including the 250-request bound and exclusive-channel queueing.
- §7 has nine channels with volumes and flags. §8 has reverb environments chosen by
  priority.
- `instruments.md` §12 requires note-ons and note-offs scheduled on the audio clock **with
  sample accuracy**, and a linear release.
- ARCH §21 requires a lock-free command queue from the main thread to the audio thread.
- CI and `--headless` runs must build and run without an audio device and without system
  audio libraries. On Linux, cpal needs the ALSA headers (`alsa-sys`).
- M6 exit criterion 4 asks for no underruns during a 10-minute flight, so the backend must
  report underruns (xruns).

Candidates, checked on 2026-10-07 against the crates in the local registry:

| Candidate | Licence | MSRV | Fit |
|---|---|---|---|
| **kira** 0.12.5 on cpal 0.18 | MIT OR Apache-2.0 | no `rust-version`; edition 2024 (≥ 1.85) | Has a mixer with tracks, a spatial listener, streaming, clocks and a mock backend. Volumes are decibels and fades are tweens over decibels, so the linear-amplitude ramp of §6.5 and the per-frame gain of §6.1 must be emulated with a tween per voice per frame. Its own spatial attenuation and panning must be bypassed (the spec computes attenuation itself). Voice ranking, virtualisation by score, the gain-0 rule and the request gates are not kira concepts and would sit on top anyway. Starts are scheduled on kira clocks, which tick at a rate the user chooses, not on an absolute frame counter. It adds glam, mint, triple_buffer, atomic-arena and pastey. |
| **cpal** 0.18.2 plus a project-owned mixer | Apache-2.0 (cpal); rtrb 0.4 is MIT OR Apache-2.0 | 1.85 (cpal), 1.38 (rtrb) | Output only: one callback per period, plus an error callback that reports xruns (`ErrorKind::Xrun`) and device loss. Everything the spec defines is our code: gain chain, distance law, ramp, voice limit, gates, reverb, per-frame scheduling on an absolute frame counter. The mixer is a few hundred lines. |
| **rodio** 0.22.2 on cpal | MIT OR Apache-2.0 | 1.87 | A source-iterator player with no frame-accurate scheduling, no voice limit and no command-queue model. It would need the same work as our own mixer, plus working around rodio's design. |
| **oddio** 0.7.4 | MIT OR Apache-2.0 | none declared (edition 2018) | A lock-free spatial mixer with its own distance and Doppler model. It has had no release since 2023, and the same mismatch with the spec's distance law applies. |

All of these are allowed by `deny.toml` without change and are within the workspace MSRV
(1.90).

## Decision
1. **Output:** cpal 0.18, behind the cargo feature `device` of `tessera-audio` (off by
   default). It runs ALSA on Linux, WASAPI on Windows and CoreAudio on macOS. Only the
   client (once it wires audio) and the `tone` example enable the feature, so a plain
   workspace build has no system audio libraries. CI jobs that build with all features
   install the ALSA headers (`libasound2-dev`).
2. **Mixer:** project-owned, in `tessera_audio::engine`. It implements `sound.md` §6–§8
   directly (no external mixer, no kira).
   - **Threading:** the main thread computes gains, panning, the voice limit and the gates
     once per frame (`AudioEngine::update`). It sends commands through a wait-free
     single-producer, single-consumer ring (rtrb). The audio thread sends finished voices
     back through a second ring, so they are dropped on the main thread.
   - **Real-time rules:** the mixer never allocates, frees, locks or does I/O. A test
     checks the allocation part with a counting allocator.
   - **Mixing:** the mixer resamples linearly by pitch × source rate / output rate. It
     loops, starts and stops voices at absolute audio-clock frames, and evaluates the
     linear ramp per sample: finer than the per-block evaluation of the `instruments.md`
     §12 Decision, and equal to the original's 10 ms steps at every step boundary.
   - **Streaming:** streamed music is decoded into per-voice ring buffers, never on the
     audio thread: by a feeder thread for the device and null backends, inline between
     blocks for the offline backend. The first 4,096 frames of a new stream are decoded on
     the main thread when its voice is created, so that it starts without a gap.
   - **Bounded queues:** when the command ring is full (a stalled device callback), voice
     starts wait in order (at most one per slot), stops wait per voice, gain updates are
     coalesced per voice (only the latest is sent) and a reverb change waits as one value.
     Memory stays bounded by the slot count, and the mixer never replays stale gains.
   - **Source rates:** sources outside 8–384 kHz are refused (`InvalidSource`); the data
     holds 8–192 kHz. Stream buffers are sized from the accepted rate, so a hostile header
     cannot size an allocation.
3. **Backends:** four, chosen by `EngineConfig::backend`:
   - `Device` (cpal);
   - `Null` (mixes against the wall clock and discards the output; for `--headless`);
   - `Offline` (renders into a caller buffer, deterministically; for tests and tools);
   - `Disabled` (every request is refused, `sound.md` §6.8 item 1).

   `Auto` tries the device and falls back to `Null`.
4. **Diagnostics:** the engine reports device xruns (cpal's error callback), mixer
   overloads (render time greater than the audio time produced), stream starvation, mixer
   load (last and maximum), and active and virtualised voices (`AudioEngine::stats`).
5. **Revisit kira** only if we need features it has and we lack (effects chains, HRTF), and
   only if it can be driven without changing the semantics above.

## Consequences
- ARCH §14 changes from "kira on cpal" to cpal with Tessera's own mixer.
- `tessera-audio` depends on `rtrb` always, and on `cpal` only with `device`.
  `cargo test --workspace` needs no audio device and no ALSA.
- The reverb presets are Tessera's own placeholder values. `sound.md` §8 names the 26
  standard environment presets but not their parameters, so the reverb is a generic
  Schroeder–Moorer network tuned by ear. Matching the original's reverb needs a spec
  addition.
- Underrun counts on any machine other than the reference laptop are indicative (decision
  0010). A device run while other builds load the CPU is indicative even on the laptop, and
  must be re-measured on an idle reference machine.
- The client chooses the backend: `Auto` for windowed runs, `Null` for `--headless`.
- **Device loss** (cpal reports the device gone or the stream invalidated): every live
  request completes, new requests are refused (`NoBackend`) and `AudioEngine::is_failed()`
  and `EngineStats::failed` turn true. The engine does not retry by itself, so a flapping
  device cannot make the main thread reopen it every frame; the client calls
  `AudioEngine::reopen()` (for example after a back-off), which keeps settings, channel
  flags, environments and handle numbering. With `Auto`, a reopen that cannot open a device
  gives the null backend.
- The client converts a sound-info record (0x2A) and a Sound node into a `SoundRequest`.
- Payloads arrive decoded from the client's asset workers through `payload_channel`. The
  engine never reads archives.
