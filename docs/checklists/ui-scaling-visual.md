<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI scaling visual checklist (decision 0040, `ui-scaling.md` §9)

A by-eye record for the UI-scaling work of [decision 0040](../decisions/0040-ui-scaling.md),
built from the design spec [`ui-scaling.md`](../specs/ui-scaling.md). It is the human half of
the test plan of §9: the committed synthetic goldens (§9.3, tier 1) and the local real-data
goldens (§9.4, tier 2) are the machine check; this checklist is what the owner looks at on the
real screens to judge the design choices the spec leaves open (§13: the default safe aspect,
the auto-scale formula, the magnification bound *ρ*, derived glyphs above *k* = 2, whether
full-window screens follow the slider).

Text only: screenshots stay in a scratch directory and are deleted after the check; no game
names, art or strings are recorded. Descriptions are our own words.

## Method

- **Install:** the owner's install, read in place (`TESSERA_GAME_DIR`). Record the data date
  when the checks are run.
- **Tools:** the client (`apps/client`) for the HUD, the loading screen, the character and
  mission-controller screens, the map and the Interface window; the explorer's scale preview
  (`apps/explorer`, `scale_preview`) for a fit container at an arbitrary window and safe aspect
  without booting the world.
- **How to reach each configuration:** start the client at the window size of the row with
  `--size WxH`, set the scale with `--ui-scale` (a number or `auto`) and the safe area with
  `--ui-safe-aspect` (`4:3`, `16:10`, `16:9`, `21:9`, `full`, or a number), or change them live
  with the `/uiscale` command and the Interface window (§7.2, §7.3). The named configurations
  are those of §9.2:

  | # | Window | *S* | `ui_safe_aspect` | Aspect | *Se* | *F* |
  |---|---|---|---|---|---|---|
  | C1 | 1280 × 720 | 1.0 | 21:9 | 16:9 | 0.9375 | 1365 × 768 |
  | C2 | 1920 × 1080 | 1.0 | 21:9 | 16:9 | 1.0 | 1920 × 1080 |
  | C3 | 1920 × 1080 | 1.25 | 21:9 | 16:9 | 1.25 | 1536 × 864 |
  | C4 | 2560 × 1440 | 1.0 | 21:9 | 16:9 | 1.0 | 2560 × 1440 |
  | C5 | 3840 × 2160 | 1.5 | 21:9 | 16:9 | 1.5 | 2560 × 1440 |
  | C6 | 3840 × 2160 | 2.0 | 21:9 | 16:9 | 2.0 | 1920 × 1080 |
  | C7 | 3440 × 1440 | 1.0 | 21:9 | 21:9 | 1.0 | 3440 × 1440 |
  | C8 | 5120 × 1440 | 1.0 | 21:9 | 32:9 | 1.0 | 5120 × 1440, *A* 3456 at 832 |
  | C9 | 5120 × 1440 | 1.0 | full | 32:9 | 1.0 | 5120 × 1440 |
  | C10 | 1600 × 1200 | 1.0 | 21:9 | 4:3 | 1.0 | 1600 × 1200 |
  | C11 | 7680 × 4320 | 3.0 | 21:9 | 16:9 | 3.0 | 2560 × 1440 (headless) |
  | C12 | 3840 × 2160 | 1.0 | 21:9 | 16:9 | 1.0 | 3840 × 2160 |

- **What "pass" means** for each row is in its "Looked for" column; mark Pass / Fail / n-a and
  add a short note. A machine golden backs most rows (the §9.4 subject is named); the eye check
  confirms the design choice, not the pixels.

## Rows

### G — general, every configuration (C1–C12)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| G1 | No window or panel is cut off the screen, nothing overflows the window edges | all | | |
| G2 | No degenerate, squashed or inverted box; nothing is drawn at zero size that should be visible | all | | |
| G3 | Text is crisp — laid out and rasterised at the shown size, not a magnified bitmap; no blur or fringing at the glyph edges | all | | |
| G4 | Art (frames, icons, backdrops) is sharp; no soft upscaling blur on 4K (C5, C12) or the stretched look of a magnified panel | C5, C12 | | |
| G5 | The pointer lands exactly on what it points at (buttons, tooltips, drag handles), with no offset, at every scale | all | | |
| G6 | A live scale change (slider or `/uiscale`) re-lays the UI in the same frame with no blank panel and no flicker; the old picture is shown until the new one is ready (§4.5, I9) | C2→C3, C4→C5 | | |

### L — loading screen (§10.1; §9.4 subject, C1, C2, C5, C7, C8, C10, C12)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| L1 | The backdrop covers the whole screen (no tiling, no stale picture behind it) | all | | |
| L2 | The meter and its label keep their place inside the backdrop at every size (no drift, no degenerate meter) | all | | |
| L3 | On 32:9 (C8) the backdrop is centred with equal black side bars (the magnification bound, ~284-unit bars, §10.1); no content is stretched across the full width | C8 | | |
| L4 | The label text height follows the scale (exact `Verdana16` at C7 1.0, distance-derived at 4K) and stays readable | C5, C7, C12 | | |

### T — theatre layout, story and progress layers together (§10.6; §9.4 subject, C2, C7, C8)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| T1 | The two layers (story backdrop, progress bar) share one placement — the progress bar sits where the story art puts it, in line, at every size | all | | |
| T2 | Showing or hiding one layer does not move the other (I16) | all | | |
| T3 | On wide screens the letterbox bars beyond the shared box are opaque black, drawn once, with nothing showing through | C7, C8 | | |

### H — HUD (§10.3; §9.4 subject, C1, C2, C4, C5, C6, C8, C9)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| H1 | On 21:9 and 32:9, the HUD (toolbar, vitals, minimap) is anchored to the safe area, not stretched across the full width; the toolbar is centred in the safe area (C8: at screen x≈2048) | C7, C8 | | |
| H2 | With `ui_safe_aspect = full` (C9), the HUD uses the full width; a panel that would run off the right edge (e.g. the minimap) is kept on screen (§10.3) | C9 | | |
| H3 | At 4K the whole HUD grows with the window (C5 at 1.5, C12 at 1.0) and stays crisp; the auto default feels right on the owner's screen (§13 OPEN) | C5, C12 | | |
| H4 | A panel the user dragged stays where it was put across a scale change, anchored to the right edge it was near (§10.4, I8) | C2→C3 | | |

### A — character appearance screen (§10.2; §9.4 subject, C1, C2, C5, C7, C8, C10)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| A1 | The left and right side panels sit at the safe area's edges; the 3D scene and the full-cover controls span the screen (§10.2, §5.3) | C7, C8 | | |
| A2 | On 32:9 the side panels do not stretch to the screen edges; they hug the centred safe area | C8 | | |
| A3 | `CharGen_Appearance_Panel` stays inside the window at 1280 × 720 (the pre-decision overflow of CS §2.2 is gone) | C1 | | |
| A4 | Panel text and the figure scale together; the figure is not clipped by the panels | all | | |

### M — mission-controller screen with the naming window (§10.8(a); §9.4 subject, C1, C2, C8)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| M1 | The screen is a safe-area panel: on 32:9 it is centred in the safe area with **no** letterbox bars of its own (§10.8(a); the C8 golden shows none) | C8 | | |
| M2 | The naming window keeps its place inside the scaled picture, in line with the other scale-parent content (nested fit, §10.8(a), I18) | all | | |

### P — world map (§10.3; §9.4 subject, C1, C5)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| P1 | At C1 the world map is larger than *F* and is fit to screen (shrunk to fit, *q* ≈ 0.9576), fully visible, not cut off (§5.6) | C1 | | |
| P2 | The map decoration and background share one placement (the map fit group, SC13) | C1, C5 | | |
| P3 | Dragging the map moves it under the pointer by the full offset even while it is fit to screen (§3.5 moves) | C1 | | |

### I — Interface window and the scale slider (§7.2; §9.4 subject, C1, C5)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| I1 | The scale slider shows the percentage and, when the window is too small for the chosen scale, the "limited" note (C1 at *S* 1.0 is limited to 94 %, §4.3, §7.2) | C1 | | |
| I2 | While the slider is dragged, the Interface window stays under the pointer — it does not slide away as the scale changes (§4.5) | C5 | | |
| I3 | The "Auto (…%)" label shows the automatic default for the current window; choosing Auto applies it live | C1, C5 | | |

### S — magnification bound and sparse content (§10.7, §13 OPEN on *ρ*)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| S1 | Sparse full-screen content (e.g. the tutorial chooser) does not grow without end with the window's width; above 16:9 it stops at the bound and gets black side bars (C7, C8: ~184 and ~1024-unit bars, §10.7) | C7, C8 | | |
| S2 | Up to 16:9 the same content covers the screen with no bars (C2, C4, C12) | C2, C4, C12 | | |
| S3 | Owner judgement: is *ρ* = 4⁄3 (the magnification bound of §5.4; it caps how far full-screen art is enlarged, and so sets the width of the black side bars on wider-than-16:9 screens — ~1024-unit bars for the tutorial chooser, ~284-unit for the loading screen at C8) the right trade-off, or should it be larger/smaller (§13 OPEN)? | C8 | | |

### X — extreme sizes and derived text (§13 OPENs)

| # | Looked for | Config | Pass? | Note |
|---|---|---|---|---|
| X1 | At 7680 × 4320 (C11, headless capture) the UI is at scale 3.0, the floor is respected and nothing is degenerate | C11 | | |
| X2 | Derived text above *k* = 2 (small fonts at 4K/8K) is judged acceptable by eye, or should switch to the bundled font (§13 OPEN) | C5, C11 | | |

## Result

Record the data date, the client/explorer commit, and the pass/fail tally here when the checks
are run. Owner rows feed [`owner-observations.md`](owner-observations.md) (the UI-scaling items
are O-numbers assigned when the owner runs this checklist). The `tessera-ui` live
layout/paint integration of §5 and §6 is merged (6015d0e, 2026-10-17), so the fit, safe-area
and content-scale rows above can now be run in the client. Not built yet: the §6.4
derived-glyph path (scaled text is soft until it lands, which affects X2 and the text rows),
multi-member fit groups, the §5.3 per-child safe boxes and the §5.4 letterbox-bar fills.
