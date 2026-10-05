<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0010 — Reference machine: the owner's laptop

**Status:** Accepted, 2026-10-05

## Context
Performance exit criteria are met only on a reference machine (`docs/perf/reference-machine.md`).
The proposal was the GTX 1070 Ti workstation that M0–M3 were developed on. The owner confirmed
it on 2026-10-05 but cannot reach it at the moment. M4 criterion 4 (50 animated characters at
60 fps) had passed only on the laptop, so it was held open.

## Decision
- The owner accepts the laptop for the performance runs. It is an Intel i9-10885H with an
  RTX 2070 Super Max-Q, 32 GB RAM, running Gentoo with Hyprland. From 2026-10-05 it is the
  reference machine.
- Following rule 3 of the reference-machine document, the M3 frame-time flights were re-run
  on it the same day. The results are the new comparison baseline, in
  `docs/perf/reference-machine.md`.
- The rules themselves are unchanged. Runs on any other machine, including the workstation,
  are indicative.

## Consequences
- M4 criterion 4 closes on the laptop's runs: 51 characters drawn in every frame, 257 fps
  headless and 351 fps windowed, with 82–83 % of the animation work on the workers.
- Decision 0008's entity-storage benchmark was measured on this laptop, so its numbers are
  now reference numbers.
- The baseline runs shared the GPU with an interactive client. Their longest frames (up to
  42 ms headless, against 17–22 ms on the workstation) are upper bounds. Re-run them on an
  idle machine before using them as a regression bar.
- If the main machine changes again, record a new decision and re-run the baseline first.
