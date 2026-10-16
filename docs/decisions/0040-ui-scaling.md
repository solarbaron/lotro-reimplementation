<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0040 — Tessera's own UI scaling, better than the original's

**Status:** Accepted (owner, 2026-10-16). The design is in `docs/specs/ui-scaling.md` (to be
written, then reviewed by a session that did not write it, before it is built).

The number 0040 is out of sequence on purpose: P6 runs in parallel on its own branch and
takes the numbers from 0037.

## Context
- `ui-layout.md` §8.4 step 6 left open how a layout made for one size (for example the
  loading screen at 2560 × 1080 or the creation screen's side panels at 1600 × 900) is shown
  in a window of another size. Both readings it offered reproduce the original client. At
  1280 × 720 the creation side panels overflow the window, and the loading screen's meter and
  label needed a stop-gap rule to be drawn at all.
- The owner, 2026-10-16: "the official client has bad scaling so i would like to implement it
  better, so support for ultrawide 4k and scale slider etc."

## Decision
1. **Tessera does not copy the original's UI scaling.** It defines its own, as a deliberate,
   owner-accepted deviation (to be listed as such in the P8 parity checklist). Layout data,
   element trees and art still come from the user's install unchanged; only how they are
   sized and placed on the screen is Tessera's own.
2. **Requirements the design must meet:**
   1. **Any window size and aspect ratio:** from 1280 × 720 up to at least 7680 × 4320, and
      from 4:3 to 32:9 (ultrawide and super-ultrawide). No element ends up with a degenerate
      or negative box, and no window or panel is placed beyond the visible screen.
   2. **A user scale setting (a slider):** a UI scale chosen by the user, stored in Tessera's
      settings file, applied live without a restart. Its default is derived from the
      display (window height and the operating system's scale factor), so a 4K screen gets a
      readable interface out of the box.
   3. **Ultrawide placement:** the interface keeps a sensible layout on wide screens (for
      example HUD elements anchored to a configurable safe area rather than stretched across
      the full width); the user can choose how wide that area is.
   4. **Crisp at every scale:** text is laid out and rasterised at the scaled size (not drawn
      small and magnified), and art is sampled so it stays sharp; the UI must look right on
      4K and high-DPI displays.
   5. **Consistent coordinates:** input hit-testing, tooltips, drag and drop, window
      positions saved in the settings file and the plugin API's screen and element
      coordinates all use one defined coordinate space, so plugins and saved layouts keep
      working when the scale or the window size changes.
   6. **Testable:** golden images at several window sizes, aspect ratios and scales.
3. The spec may add further options (per-window scale, text-only scale, and so on) where the
   owner's "etc." calls for them; each one is recorded in the spec.

## Consequences
- New spec `docs/specs/ui-scaling.md` (Tessera design; no client facts needed beyond the
  Accepted layout specs), reviewed independently before it is built.
- `ui-layout.md` §8.4 step 6's OPEN is answered by this record; the stop-gap rescue rule
  stays until the new design is built.
- `tessera-ui`, the client's UI host, `tessera-settings` and the plugin API's display
  views change when it is built.
