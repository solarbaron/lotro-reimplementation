<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Proposed — Game UI rendering: exact CPU compositing per panel surface, GPU composition of the surfaces

**Status:** Proposed (awaiting the owner), 2026-10-06. It gets a number when it is accepted.
It is an architecture proposal for M5 (`docs/ROADMAP.md` M5). It does not change any
accepted record.

**Revision 1 (2026-10-06)** answered the review of the first draft. **Revision 2
(2026-10-06)** answered the second review and aligned this record with its companion
`proposed-ui-input-bridge.md` (D7, D8), `game-panels.md` (Accepted), `ui-widgets.md` and the
updated `ui-behaviour.md` and `lua-plugin-api.md`. **Revision 3 (2026-10-06)** answers the
third review: one rule for damage from nested surfaces (an angle-independent coverage region),
Tile surfaces exempt from the box-to-canvas scale, the cursor at the end of the pointer lock
taken from IB D7 as written, a derived budget for scrolling and chat, one transition for a
whole window with its nested parts, coverage regions under a scaled parent, auto-fade against
the withheld first paint, and the Tessera overlays above the UI pass (changed in revision
4). **Revision 4 (2026-10-06)** answers the fourth review: the frame loop follows IB D8 (UI
tick in step 7,
layout and paint in step 8), the pointer lock follows IB D7 as revised (on only after the UI
declines the `mouselook` Start; no cursor-position events during the lock), a generation's
jobs never write a front texture before the generation swaps, the walker's own overlays draw
under the game UI, mode-0 elements take a resize into their layout box, and "root" is
defined. **Revision 5 (2026-10-06)** answers the fifth review: overlay source-format tags
that follow UL §9.6.3 (only A8R8G8B8 sources are *ARGB32*), nested entries clipped by their
nearest enclosing window and the windows above it (LPA §13.18 OPEN (1)), rotation as state of
the surface that received it (LPA §13.20 step 3), a first-paint bound that holds while a
generation runs, and the smaller items (content-job back textures, texture usages, external
content format, the software cursor, `SetStretchMode` refreshes, mode-0 child windows, Tile
parents, the `plugins` → `ui` allowance). It also applies IB's "Coordination" items 7–9. The
last five sections map each item of the five reviews to its answer, and the section
"Coordination with IB" lists what the companion record must change.

Abbreviations for the documents cited most: UL = `ui-layout.md`, UB = `ui-behaviour.md`, UW =
`ui-widgets.md`, FO = `fonts.md`, LPA = `lua-plugin-api.md`, SK = `skin-definition.md`, PU =
`plugin-api-usage.md`, GP = `game-panels.md`, KM = `keymap.md`, IB =
`docs/decisions/proposed-ui-input-bridge.md`.

## Owner summary

**The decision in five lines.**
1. Game UI art and text are composed on the CPU, byte-exact to the specs (UL §9.6, FO §8.4),
   into one sparse 32-bit canvas per element that owns a surface; only damaged rectangles
   are repainted.
2. The GPU only composes the finished surfaces: one textured quad per surface, in a UI pass
   after the world and the walker's own overlays, with the surface's opacity, scale and
   rotation. The GPU draw list and the "nine-slice art" of ARCHITECTURE §15 and ROADMAP M5
   are dropped.
3. Nested surfaces (plugin controls with a stretch mode, child windows, the minimap marker,
   portraits) are separate quads with their own opacity only, clipped by their nearest
   enclosing window. Parent content drawn over them goes into a "canvas above".
4. Large paints run as jobs over several frames and are never restarted. Each window keeps
   its last complete picture, nested parts included, and swaps to the new one in a single
   frame. A newly shown panel waits at most 100 ms. A part shown inside a window whose
   resize is still painting also waits for that paint (Decision 1.5, U4 (c)).
5. The budgets U1–U5 (§6) become M5 exit criteria. The UI runs in frame-loop steps 1, 7 and
   8 as IB D8 says. The cursor is an OS cursor; it is hidden and grabbed only when a
   mouse-look press is declined by the UI and is not consumed by the client during an item
   drag (IB D7).

**Questions the owner must answer.**
1. Accept option A, together with IB, and the rewording of ARCHITECTURE §3 (a `plugins` →
   `ui` sibling allowance), §13, §15, §20.3 and §21 and of the ROADMAP M5 deliverable
   "nine-slice art" (Consequences)?
2. Accept U1–U5 (§6) as M5 exit criteria, including the split that judges frames with a
   scroll step or a new chat line by the 4 ms paint slice (U4 (b), (f)) instead of the
   steady-state U1? Note that U1's mean is at risk in the walking phase, where the moving
   minimap alone may take most of it (§6). The walking-phase mean is accepted as a
   criterion only after the probe has measured Mix colour and the erase (§6).
3. Nested surfaces take their own opacity only (Decision 1.7). This departs from the Tessera
   rule of LPA §13.19 and from the wording of UL §5.9 and §6.6, and asks their spec writers
   to align them. Its visible consequences: a Fade item or `SetOpacity` on a window does not
   fade its nested surfaces, so the P1 marker stays opaque under a Fade on P1's root, and
   the stretch-mode icons of a plugin window set to opacity 0 stay visible. The alternative
   ("own × ancestors") fades children twice (*o*²) under auto-fade. Agree with "own only"?
4. Accept the stated exception to ARCHITECTURE §21: a bounded synchronous header read (at
   most 64 bytes) when a plugin calls `SetBackground(path)` (§4.4)? The `Plugins/` index is
   rebuilt on a worker, so it needs no exception.
5. Accept the recorded differences:
   - a first paint may appear in bands after 100 ms, and it can shorten an auto-fade-in
     (Decision 1.8);
   - a nested part shown while its window's resize is painting appears only with the
     resize: up to about 8 frames during a resize-bar drag of P10 or P15, and up to about
     41 frames (about 0.7 s at 60 Hz) in a screen-sized plugin window after a screen
     resize (Decision 1.5, U4 (c));
   - a nested entry is clipped by its nearest enclosing window **and** by the windows above
     it, an extension of LPA §13.18's "clipped to its window only" (Decision 1.7);
   - a window or native root panel in object mode 0 is drawn at opacity 1, because it has
     no surface for its opacity (LPA §13.19); a child window in mode 0 is drawn into its
     parent's canvas (Decision 1.1);
   - elements below a Tile surface get no surface of their own (Decision 1.7);
   - under a UI scale, drawn positions and hit-test boxes of elements inside a surface can
     differ by up to one pixel per nesting level (§2.4);
   - during a resize, hit tests use the new layout while the old picture is shown (Decision
     1.5);
   - parent content drawn over a nested surface is not byte-exact there (Decision 1.7);
   - plugin `.png` and `.bmp` images are refused until PU O6 is answered (§4.3);
   - skin images over 4,096 pixels on an axis are cropped at decode, where SK §6.10 and
     §6.11 show them whole up to 16,384 (§4.6).
6. With the game UI attached, the walker's HUD and minimap draw **under** the game UI, the
   walker minimap is off by default, and checklist runs set `walker_hud = off` (Decision
   1.6). Agree?
7. Will you make the seven in-game observations listed under Consequences? None of them
   blocks M5.


## Open review items (2026-10-06)

After five revise-and-review rounds an independent clean-room review still lists the items
below. They are recorded here for the owner and for the M5 implementer; none changes the
chosen option.

**Must fix before acceptance:**

1. Clip chain leaves out mode-0 child windows (Decision 1.1, Decision 1.6 clip chain, Decision 1.7 'Clip', §8 'Clip'). Decision 1.1 draws a plugin child Window after SetStretchMode(0) into its parent's canvas, with no surface and no stand-in. The clip chain is defined as the 'surfaces strictly above its element that are a root or belong to a root panel (stand-in surfaces included)'. So a stretch-mode control inside such a child window C has only the outer window P in its chain, and it is not cut at C's edge. This contradicts LPA §13.18's Tessera rule ('clipped to its window only': for that control, its window is C). It also contradicts the record's own example in Decision 1.7 ('a stretch-mode control inside a child window that extends past the child window is cut at the child window's edge'). Fix: build the chain from the displayed boxes of the enclosing root panels (windows), whether each is drawn through a surface or into an ancestor's canvas. A window without a surface is tested in the space of the surface that draws it. Add this case to the §8 clip-chain test.
2. The coverage region of an entry with its own rotation ignores where its subtree turns to (Decision 1.7 steps 1–2, Decision 1.4, §2.3). Step 1 makes the footprint of an own-rotated entry N an angle-independent square. Step 2 joins it with the UNROTATED footprints of the nested entries below N, yet those entries turn about N's centre (Decision 1.6 transform chain). When N is not a window, its nested entries are not clipped by N (the clip chain holds windows only). LPA §13.20 step 3 makes this case reachable: a stretch-mode control receives the angles of a hidden child window. A stretch-mode child M of N that extends past N is then drawn, at N's angle, outside both N's square and M's unrotated footprint. Parent content drawn after N at those pixels goes to the canvas below and appears under M. That breaks the record's claim that the split 'gives the drawing order of a single canvas at every pixel, for any number of ... deeply nested surfaces', and its claim that turning is never damage. Fix: for an entry with its own rotation, first join its subtree's footprints in its own frame, then take the square around the circle centred on N's centre that passes through the farthest corner of that join. Alternatively, state that the descendants of a rotated non-window surface are clipped by it.

**Other improvements:**

- Group visibility while the root's first paint is withheld is not defined. Two cases: (1) a window is hidden and shown again, while its children with stretch modes keep their surfaces (Decision 1.1, 'Surfaces under a hidden ancestor'); (2) a new window is shown together with nested surfaces whose first paints finish before the root's. In both cases the nested entries can be drawn, standing alone, for up to 100 ms before the window appears. Add a rule: no entry of a group is drawn before its root has been drawn, and nested entries join at the root's first draw.
- Switching between the stand-in surface and a real surface on a shown top-level window: Decision 1.1 converts the stand-in in place only on the LPA §13.20 step-2 path. Decision 1.8 makes a direct SetStretchMode(0→n) a release plus a first paint, so the window vanishes for up to 100 ms. The reverse change (n→0, real surface to stand-in) is not specified at all. Make both directions in place for any mode change of a shown root panel that has no surface above it.
- Shared per-frame resources have no scheduling policy. U4 (b) gives one 4 ms paint slice and U3 one 8 MiB upload cap, shared by every running job. The derivations of U4 (d) and (e), the 8- and 41-frame bounds of U4 (c), and the 'at most 4 frames' wait before a back-texture swap all assume that one surface has the whole slice or cap. A screen resize starts jobs in every open panel at once (UL §8.5), and the P10 drag runs alongside the minimap and chat repaints. State the order in which jobs get the slice (for example, first paints and generations before content jobs, and FIFO among them), and either derive the bounds with that sharing included or weaken them.
- U2 (GPU ≤ 0.3 ms at 3840 × 2160) is stated without a derivation. The scripted run includes a screen-sized plugin window with a full-size background. That window alone is an 8.3 Mpx blended quad: one texel read plus a framebuffer read and write per pixel, about 100 MB per frame. On the RTX 2070 Super Max-Q that is close to 0.3 ms before any other entry. Derive U2, or set it with that window in mind.
- The upload bound 'a job's rectangles are at most one canvas' (Decision 1.5) ignores canvases above. A parent job writes into the canvas below and into its canvases above, and pixels that leave a canvas above must be cleared or re-uploaded there. Count the canvases above in the U3 and U5 arithmetic.
- A back texture that does not fit U5 makes the surface's job wait, and no new job starts on that surface (§7, Decision 1.5 step 4). Nothing bounds this wait, so if U5 stays full the surface or its generation can stall indefinitely. Define a way out, for example uploading straight into the front texture over several frames, or giving back textures a reserve inside U5.
- The cropped-canvas case of §7 does not fit the composition-scale formula. §7 says a canvas over the size limits covers only the on-screen part, while Decision 1.6 computes the scale as displayed box ÷ canvas size. For a cropped canvas, use the size of the layout box and treat the crop as an offset, as Decision 1.2 says for the tile box. Use one word for the full canvas and another for the stored part.
- Alpha-threshold order: §8's 'UI pipeline' row applies the threshold after multiplying by opacity. UW §4.5's Tessera rule compares the image alpha. Apply the threshold to the unfilled texel alpha before opacity. No alpha-test meter is in the M5 panels.
- During a banded first paint (Decision 1.5), each band can grow the tile box. Each growth re-creates the texture with a GPU copy (Decision 1.2), so a 4K first paint could re-create a 31.6 MiB texture up to 33 times. Allocate the texture at the full canvas size, or at the job's damage box, when a job starts.
- Wording: Decision 1.6 tests clip boxes 'in that surface's own unrotated space', while Decision 1.7 says 'in that surface's own rotated space'. Both mean the window's local rectangle; use one phrase.
- §4.5 pins every image 'referenced by an element that has a canvas'. Read literally, that leaves out the images of descendants drawn into a parent's canvas, and those are exactly the images that partition invariance needs. Say 'every displayed element drawn into a canvas'.
- §4.4 covers SetBackground(path) only. LPA §13.8 also accepts Turbine.UI.Graphic(path) wherever a background is taken. Say whether Graphic(path) uses the same index lookup and header probe, and when its errors are raised.
- U1 does not say how tooltip and drag-icon shows count. They are root first paints that happen during the toolbar pointer sweep. Say whether they are U4 'panel open' event frames or part of the U1 steady state.
- U4 (c) states 41 frames as a hard bound, but Decision 1.5 derives it as 40 + 'normally one frame'. The next generation is bounded only by U4 (d) and (e). Either give the bound that is actually guaranteed, or define the coverage-only generation as at most one frame.
- The software cursor is drawn last of all, so with ui_cursor = software it would appear in the walker's screenshot action and in the owner's M5 comparison captures. Say whether screenshots leave it out.
- Owner question 3 and spec question 1 cite LPA §13.19's OPEN and UL §5.9 and §6.6. LPA §13.19's body also says that a window's opacity 'fades the window and all its children' and that a control in a non-zero mode fades 'the control and its children'. Cite those sentences too, so the spec writer aligns them.
- §4.4 justifies the 64-byte header read as 'of the same kind that import already needs'. ARCHITECTURE §21 has no exception for import either. Either record import's synchronous reads in the same §21 exception, or do not cite import as a precedent.
- §4.4 says the index is rebuilt 'when the plugin manager refreshes its list (ARCHITECTURE §18.2)', but §18.2 does not mention a refresh. Cite LPA or the plugin-manager spec instead.

## Context

**What the specs establish.**
- **CPU composition.** UL §9.6.1 (BIN, U43) says that the original UI is composed on the
  CPU. A root panel owns a drawing surface (Probable: 32-bit ARGB). Element image layers are
  blended into that surface pixel by pixel with integer arithmetic. There are seven
  compositor operations, given byte-exact in UL §9.6.2–§9.6.3. They include:
  - the Colorize HSV recombination;
  - a base-layer Grayscale that draws nothing;
  - an overlay Grayscale that turns 255 into black;
  - an Over rule with a special case for a transparent destination.
- **Source formats.** Sources without alpha are read with alpha 255 (UL §9.6.2, BIN for
  Over). An overlay with an image draws nothing unless the image is 32-bit ARGB or
  alpha-only, and alpha-only images have routines of their own that were not read (UL
  §9.6.3 OPEN).
- **Update regions.** The surface is repainted by update regions (UL §9.3, U34). Each
  element is clipped to its own box and to its ancestors' boxes. Each layer is tiled at the
  image's native size and never stretched (UL §9.6).
- **Text.** Text is composited on the CPU into the same canvas, with its own exact rule (FO
  §8.4), at the font's own pixel size. A UI scale acts afterwards on the painted result (FO
  §6.3).
- **Plugin controls** are client elements. They are drawn by the same CPU compositor:
  `SetBackground` is the base layer, `SetBackColor` the overlay, drawn in that order (LPA
  §13.17, BIN).
- **Which elements own a surface** (LPA §13.19, BIN). An element wants a surface when its
  `UICore_Element_ObjectMode` value (from its layout or from `SetStretchMode`) is not 0. An
  element with no value wants one when it is a root panel. The surface exists only while the
  element's **own** visibility flag is set, or while `UICore_Element_KeepObjectWhenHidden`
  is true (UB §11.2); the ancestors' visibility does not enter. Alpha-test meters also get
  a surface at post-creation (UW §4.5, §16 row 3). The tooltip and the drag icon have their
  own surfaces (UB §6.4, §7.2). Game code gives the minimap's player marker object mode 1
  (GP §8.2 step 3), so it owns a surface too.
- **Object modes in the data.** 446 elements and 3 states carry an object mode: Stretch 301,
  CanvasResize 119, None 24, Tile 2 (UL §5.9, §8.4). Tile needs power-of-two surface sizes;
  otherwise the client logs it and switches the element to CanvasResize (UL §8.4).
- **Scale.** A scaled root panel keeps its layout box at the unscaled size and gets a scale
  vector and a scaled displayed box (UB §9.2.1, BIN). For the game's scaler types
  (`UIScaler`, `UIScalerPanel`, `ChatWindow`, the bags) the scaler switches the panel to
  Stretch for the resize and then **restores** its object mode (UB §9.2 step 6), so most
  scaled M5 panels end in CanvasResize with a scale other than 1. The surface is drawn scaled
  onto the displayed box (UB §9.2.1 OPEN, Probable).
- **Opacity.** Fades, auto-fades and the plugin `SetOpacity` all set
  `UICore_Element_ObjectOpacity`, the opacity of the element's own surface (UL §5.9, §6.6;
  UB §11.1; LPA §13.19). Auto-fade applies the opacity "to the element and to each child
  with its own surface" (UB §11.1 step 2).
- **Rotation.** A plugin window's rotation is three angles in degrees, about the surface's
  centre (LPA §13.20); plugins pass mostly `z` headings. The angles go to the surface that
  exists at the call: the window's own, otherwise the nearest ancestor's. A window
  remembers only a rotation that reached its own surface, so a `SetRotation` on a hidden
  window never rotates that window when it is shown (LPA §13.20 step 3, BIN). The minimap's player marker is
  drawn rotated about its centre by the compass heading (GP §8.6.3).
- **The minimap (P1).** The map element is moved inside `MinimapViewport` on every position
  update (GP §8.6.2), so it moves on every frame while the player walks. The map is round
  only because the panel surface's alpha is used when the surface is composed (GP §8.7.1).
  Tessera draws the rim `Minimap_Frame` unclipped by the viewport, clipped only by the radar
  root (GP §8.7.2 Tessera rule).
- **Pointer lock.** While the input layer's pointer lock is on (mouse-look), the UI pointer
  is frozen at the position where the lock began and the UI hit-tests nothing (UB §4.9). In
  the walker, mouse-look is `MouseRight` (`mouselook`), which also fires with the
  `MouseLeft+MouseRight` run chord (KM §8). IB D7 makes the lock client state: it turns on
  only after the UI declined a `mouselook` Start and no item drag is in progress, and off
  after the End of the last `mouselook` hold that turned it on.
- **Input timing.** UI input routing and the plugin input handlers run in frame-loop step 1,
  against the previous frame's layout (IB D8, from LPA §3.3 and §9.7). Step 7 runs the UI
  tick of LPA §3.3 (hover refresh, `MouseHover`, the plugins' `Update` broadcast, the rest of
  the UI's per-frame work) and then the plugin host's deferred work (sliced by whole items)
  and the collector step; step 8 lays out, paints
  and builds the composition list (IB D8).

**What is OPEN.** The following facts are not known:
- how a surface reaches the frame (blend state, premultiplication, filtering) and what an
  erased area holds (UL §9.6.1; Probable: transparent black);
- the filter for scaled surfaces, and whether the whole surface or each element is
  resampled (FO Q3, UB §9.2.1 OPEN, UB §17 item 18);
- object modes None, Tile and CanvasResize beyond the layout rule (UL §8.4, §16 item 7);
  plugin mode 2 drawing and mode 4 (LPA §13.18 OPEN (2));
- how a nested surface (an element with its own surface inside another surface) is
  composed: its clip, its order against later siblings, and whether its ancestors' opacity
  applies (LPA §13.18 OPEN (1), §13.19 OPEN);
- which other non-root elements code gives a surface (UL §6.6, §16 item 11);
- the canvas format for text (FO Q6);
- the direction, order and projection of rotations, and hit testing of rotated windows
  (LPA §13.20 OPEN); the resampling of the rotated minimap marker and its screen direction
  (GP §8.6.3 OPEN);
- whether the minimap rim is clipped by the viewport (GP §8.7.2 OPEN);
- the effect of `UICore_Element_ShouldEraseBackground` (UL §5.9, named only);
- the compositor routines for sources other than 32-bit ARGB and 24-bit RGB (UL §9.6.2
  OPEN, §9.6.3 OPEN; SK OPEN-4);
- how a Viewport is drawn and composed into its panel (UW §12 OPEN, §17 item 6);
- what the visible cursor does while the pointer is locked (UB §4.9 states only the UI
  pointer).

**What the code and the architecture have.** `render.md` has no UI pass.
`docs/ARCHITECTURE.md` §15 planned a "2D draw list (textured quads, nine-slice frames, text
runs, clip rects)" drawn by the GPU, and ROADMAP M5 lists "nine-slice art". The data does not
support either: the art is tiled at native size and blended with operations that read the
destination. The renderer today has:
- straight-alpha `ALPHA_BLENDING` quads in `PassSpace::Screen`;
- RGBA8-only uploads (`TextureUpload`);
- whole-texture `update_texture` and no sub-rectangle update;
- `register_target_texture` for offscreen targets;
- an sRGB swap-chain format (`renderer.rs` picks the first sRGB format).

The client does not grab or hide the cursor today (no grab code in `apps/client`).

ARCHITECTURE §3 says that `render` and `ui` do not depend on each other; the client wires
them. ARCHITECTURE §21 puts the UI tree and the Lua states on the main thread, and forbids
blocking I/O there after start-up.

**Scale of the work.**
- The 15 M5 panels have 18 to 881 element instances each, and up to 572 distinct art
  surfaces (UL §17.2). Their sizes go up to 1038 × 802 (P10). P2, P9, P10 and P15 have
  resize bars (UB §8.2, UL §17.4).
- The reference machine (`docs/perf/reference-machine.md`) drives a 3840 × 2160 panel. The
  original lays out against the render size (UL §8.5, UB §9.4), so at UI scale 1.0 a panel
  covers the same number of physical pixels as at 1024 × 768.
- Plugins use own surfaces a lot. `SetStretchMode` is used by 80 distributions (PU §5.3; 81
  by a whole-distribution text search, PU §3.5). Counted by literal argument, mode 1 appears
  in 58 distributions, mode 2 in 56 and mode 3 in 13 (PU §6.8); these sets overlap and do not
  add up. `Display.GetSize` appears in 98, so screen-sized plugin windows occur (PU §5.3).

**Indicative compositing cost** on the reference machine, 2026-10-06. This was a scratch
probe, not Tessera code, built in release mode with the pinned rustc 1.99.0, single-threaded
and not vectorised. The machine was not idle. The numbers close no criterion (rule 1 of the
reference-machine document needs a Tessera release build with its conditions recorded), so
they are indicative. Over a 1024 × 768 32-bit destination, tiling a 64 × 64 source:

| Operation | Cost per destination pixel |
|---|---|
| Replace (tiling copy) | 0.4–0.5 ns |
| Over (UL §9.6.2), opaque runs of source alpha | 1.0–1.4 ns |
| Over, random mixed source and destination alpha (worst case for branches) | 3.3–4.0 ns |
| Over, element opacity < 1 | 4.4–5.5 ns |

So one full layer over a 1024 × 768 panel costs about 0.3–4 ms. A first paint with an overdraw
of 3–4 costs about 1–15 ms. At 3840 × 2160 (8.3 Mpx, about 10× that area) a first paint of a
screen-sized surface costs about 4–130 ms, which is more than one frame's slice: paints that
span several frames are therefore part of the design (Decision 1.5). Repainting only small
damaged rectangles costs tens of microseconds. A whole scrollable box (a list scrolled by
one line, a chat log with a new line) is not small; §6 treats it as a content event.

## Options considered

**A. Exact CPU compositing per surface; the GPU only composes the finished surfaces
(proposed).**

`tessera-ui` keeps one 32-bit canvas per surface. It paints that canvas with the integer
rules of UL §9.6.2–§9.6.3 and FO §8.4, only inside damaged rectangles. It hands the changed
rectangles to the client, which uploads them as sub-rectangle texture writes. The renderer
draws each surface as one textured quad in a UI pass, with the surface's opacity, scale and
rotation.

- **For:**
  - Byte-exact surface content, which is what the specs give and what the M5 visual
    checklist compares (`docs/checklists/m5-visual.md`, for example row D3, the Grayscale
    overlay).
  - The paint is a pure function of the display list and the decoded images, so tests check
    exact bytes without a GPU, against the spec examples (UL §14.7, FO §13.8, LPA §13.17).
  - It mirrors the original's update-region model, so damage tracking is natural.
  - Fades and opacity changes of any surface, and the rotation, Stretch scaling and moves
    of a root surface, cost nothing on the CPU. So does the turning minimap marker, which
    has its own surface and a coverage region that does not depend on its angle (Decision
    1.7, §2.3).
  - `tessera-ui` stays free of GPU code (ARCHITECTURE §3).
- **Against:**
  - The cost on the main thread grows with damaged area × layers. A first paint, a resize
    or a UI-scale change at 4K can take tens of milliseconds, so large paints span several
    frames, and their transition must be defined (Decision 1.5).
  - Uploads must be bounded. `walker.md` §9 already records an OPEN about main-thread spikes
    of up to 40 ms inside texture uploads on this machine.
  - Nested surfaces and content drawn by the GPU (Viewport portraits) do not fit into one
    canvas per root. They need the split of Decision 1.7.
  - The step from surface to frame is OPEN, so that last blend is exact only for alpha 0 and
    255.

**B. GPU draw list (the ARCHITECTURE §15 plan): textured quads per layer, glyph-atlas quads,
fixed-function blending, scissor clips.**

- **For:** Little CPU work, simple batching, and no per-pixel cost on the main thread.
- **Against:**
  - Only Replace and the opaque cases of Over map to fixed-function blending. Multiply,
    Screen, Colorize, Mix colour and both Grayscale variants read the destination through
    the client's integer formulas, including wrap-around quirks. wgpu has no portable
    framebuffer fetch, so they need a ping-pong copy per element or approximations.
  - The transparent-destination rules of Over and of text compositing, and the plugin
    default Replace (LPA §13.17), write the source's or the coverage's raw alpha into the
    surface. That only means something when the surface is separate, which B does not have.
    The round minimap (GP §8.7.1) depends on exactly this.
  - Results would differ from the original by one or more units in many pixels. They would
    need a GPU for every test, and every difference would have to be recorded as a known
    difference against FO §8.4's own Tessera Decision.
  - Fades of panels with children need offscreen layers anyway.

**C. Exact compositing on the GPU: a compute shader that runs the integer formulas per tile
over a per-surface display list, into a storage texture.**

- **For:** It is exact and takes the per-pixel cost off the CPU.
- **Against:**
  - It means a second implementation of the traversal, the clipping, the tiling, the media
    layers and the text placement, in WGSL.
  - Tests need a GPU, and CI's headless adapter may differ.
  - Write-only `Rgba8Unorm` storage textures are core in wgpu. `Bgra8Unorm` storage needs an
    optional feature. So C would write `Rgba8Unorm` and swap the bytes, or need that
    feature.
  - Debugging is harder for hostile-input problems.
  - It is useful only if A misses its budget after damage tracking, banded paints and worker
    bands. The CPU painter of A would stay as its test oracle anyway. C can be added later;
    it changes the renderer and the client, not `tessera-ui`'s painter interface.

**D. A single screen-sized UI canvas on the CPU (all panels composed together), uploaded as
one texture.**

- **For:** One texture and one quad, and the whole UI is exact even between panels.
- **Against:**
  - It composes panels on top of each other with a rule that is itself OPEN.
  - Every fade, move or opacity change of a panel becomes a repaint of everything under it.
  - At 3840 × 2160 the canvas is 33 MB, and an upload of a large damaged area costs a lot.
  - Panel scale and plugin window rotation would have to be done on the CPU too.

**E. egui (already in the explorer) for game UI.** This was excluded by ARCHITECTURE §15
("never used for game UI"). It cannot express the data's blend modes, its layout or its fonts.

## Decision (proposed)

### 1. Model: option A

1. **Surfaces and their owners.** A *surface* is a CPU canvas of 32-bit pixels, 0xAARRGGBB,
   owned by one element. In memory the pixels are little-endian, so the bytes are B, G, R, A
   and upload to a `Bgra8Unorm` texture with no conversion.
   - **Owners.** `tessera-ui` uses the rule of LPA §13.19 for every element, native or
     plugin. An element has a surface while both conditions hold:
     1. it **wants** one: its `UICore_Element_ObjectMode` value is not 0, or it has no value
        and is a root panel;
     2. it is **shown** (its own visibility flag), or `UICore_Element_KeepObjectWhenHidden`
        is true.

     In addition, these elements own a surface:
     - alpha-test meters (UW §4.5);
     - the tooltip panel and the drag icon (UB §6.4, §7.2).

     The minimap marker needs no special case: game code sets its object mode to 1 (GP §8.2
     step 3), so condition 1 holds.
   - **Root, root panel and group (terms of this record).** A **root** is an element that
     has a surface and no ancestor with a surface. It is the top of a composition **group**
     (Decision 1.5). A **root panel** is the layout and API notion of UL §9 and LPA §13.19:
     every plugin `Window` is built as one, even when it has a parent window. The two differ.
     A child `Window` whose parent window has a surface is a root panel but not a root: it
     is a **nested entry** of its parent window's surface (Decision 1.7). It is clipped by
     the windows that enclose it (Decision 1.7, "Clip"), and it takes the transforms of its
     surfaced ancestors. A rotation held by its **own surface** (§2.3: set by a
     `SetRotation` made while that surface existed, or restored from the window's
     remembered rotation when the surface is created) is its **own rotation**, so its
     coverage region is the angle-independent square of Decision 1.7 step 1, and its
     turning is not damage after the first rotation (Decision 1.4). A `SetRotation` made
     while it has no surface rotates the nearest surfaced ancestor instead, never it
     (§2.3). "Root surface", "root entry" and "the root's …" in this record mean the root
     in this sense. Condition 1 above uses "root panel".
   - **Modal dialogs.** The dialog itself owns the surface: UL §9.1 gives it CanvasResize
     when it has no mode, so it wants one. The screen-sized container of UL §9.1 gets **no**
     canvas. It is a Field created by code with no images and no text, so a canvas would hold
     only transparent black. It stays in the tree for layout and hit testing. Whether the
     original gives the container a surface is not stated; the difference cannot be seen.
   - **Tessera rule for root panels in mode 0.** A shown root panel whose stored object
     mode is 0 and that has **no surface above it** (a top-level plugin window after
     `SetStretchMode(0)`, a native top-level panel with a layout value of 0) is drawn
     through a Tessera **stand-in surface**. For plugin windows this is the Tessera rule of
     LPA §13.19 ("for drawing only"); this record extends it to native root panels with a
     layout value of 0, where the original's drawing was not traced either (UL §8.4 OPEN).
     A stand-in surface is sized and composed like CanvasResize (§2.1), with two
     differences:
     - it is composed at **opacity 1**. Its `ObjectOpacity` is stored and returned only,
       because the element has no surface for it: LPA §13.19 says that after
       `SetStretchMode(0)` a window's "opacity has no effect", and UL §5.9 makes
       `ObjectOpacity` the opacity of the element's own surface. So a Fade item or an
       auto-fade on such a panel is not seen (recorded, owner question 5);
     - it is **not a surface** for any rule other than drawing: the stored mode,
       `GetStretchMode()`, the resize rule of UB §9.2.1 and both the "has a surface" test
       of LPA §13.20 step 2 and the target search of step 3 (§2.3) follow conditions 1 and
       2. For the resize this means the rule for an element without a surface (UB §9.2.1
       step 3): the layout box takes the new size and the children are laid out again. The
       stand-in surface's canvas follows the layout box and is repainted in full, as for
       CanvasResize. When step 2 of LPA §13.20 sets mode 1 on such a shown window, the
       stand-in surface becomes its real Stretch surface in place (same canvas, texture and
       content; no first paint).

     A root panel in mode 0 **below** a surface (a plugin child `Window` after
     `SetStretchMode(0)` whose parent window is drawn through a surface or a stand-in
     surface) gets **no** surface and no entry. It is drawn into the canvas of the nearest
     surface or stand-in surface above it, as LPA §13.17 ("Where it is drawn") says for every
     control without a surface, clipped by its own box and its ancestors' (UL §9.3). Its
     opacity has no visible effect (LPA §13.19), and it has no rotation of its own. This
     narrows LPA §13.19's Tessera rule ("Tessera draws every shown window through a
     surface") to windows that would otherwise be drawn nowhere; the content is byte-exact
     in its parent's canvas, and spec question 13 asks the spec writer to align the
     wording.
   - **Surfaces under a hidden ancestor.** Condition 2 tests only the element's own flag, so
     a child with its own surface keeps it while its window or another ancestor is hidden.
     Such a surface gets **no composition entry** while any ancestor is hidden (it is not
     displayed, LPA §13.19, `IsDisplayed()`), and its canvas and texture are kept, so it
     costs no repaint when the ancestor is shown again. Its memory still counts toward U5
     and the caps of §7.
   - **Initial content.** A new surface and an erased area hold transparent black
     (0, 0, 0, 0). This is the Probable value of UL §9.6.1. It is one constant to change if a
     capture shows otherwise.
   - **Empty surfaces.** A surface whose width or height is ≤ 0 has no canvas, no texture
     and no composition entry. This follows the Tessera rule of LPA §13.15: such a control
     draws nothing and is never hit.
2. **Canvas storage and the GPU copy.**
   - **CPU.** A canvas is stored as **sparse tiles** of 256 × 256 pixels, cut to the canvas
     at its right and bottom edges, so a canvas smaller than 256 pixels on an axis has tiles
     of its own size on that axis (a 16 × 16 control costs 1 KiB, not 256 KiB). A tile is
     allocated on the first write that is not transparent black, and a missing tile reads as
     transparent black. An erase that covers a whole tile frees it. So a mostly empty canvas,
     such as a screen-sized plugin window with a small frame, costs memory in proportion to
     its painted area.
   - **GPU.** Each surface has one texture, sized to the **bounding box of its allocated
     tiles** (tile-aligned) and placed at that box's offset inside the canvas. The canvas
     origin offset of §7 is the same mechanism. The shader treats every texel outside the
     texture's box as transparent black, so the quad still covers the whole displayed box.
     When the bounding box grows, a new texture is created and the old content is copied
     into it on the GPU; when it shrinks by at least half on an axis, the texture is
     re-created smaller in the same way. A surface with no allocated tile has no texture and
     no entry. One texture per surface is kept (no tiled GPU copy), because the shader's own
     bilinear filter (§2.2) would otherwise need gutters or neighbour lookups at tile edges.
3. **Painting.** `tessera-ui` paints a damaged rectangle as UL §9.3 describes:
   1. Erase the rectangle.
   2. Walk the visible tree in drawing order, intersecting each element's clip with the
      rectangle.
   3. Draw a background object attached by code first (UL §9.3 step 3), then both image
      layers with the operations of UL §9.6.1–§9.6.3, then text with FO §7–§8. The overlay
      with an image follows the source-format rule of §4.2.
   4. Handle `UICore_Element_DrawAfterChildren`.

   Tiling is anchored at the element's top-left, not at the rectangle, and every operation
   is per pixel. Repainting any partition of a region therefore gives the same bytes as one
   full repaint, whatever the tile layout. This is a required test property.

   > **OPEN (this record):** the effect of `UICore_Element_ShouldEraseBackground` (UL §5.9,
   > named only). It may make the erase of step 1 conditional per element. Tessera rule
   > until it is traced: always erase, and ignore the property.
4. **Damage tracking.** Every change that can alter pixels adds the affected rectangles to
   its surface's damage list. A rectangle is the old box and the new box in surface
   coordinates, clipped by the ancestors. These changes are:
   - a state switch that changes media or properties (this includes the rollover states
     that hover selects);
   - a media item that sets or clears a layer, or an animation frame;
   - a text, colour or font change;
   - a move or resize of an element without its own surface (for example the minimap's map
     element, which moves on every frame while the player walks, GP §8.6.2: the damage is
     its old and new box clipped by `MinimapViewport`, so 170 × 170 pixels);
   - show or hide of an element without its own surface;
   - a skin refresh (SK §6.6) or an image arriving;
   - a scroll offset change of a list or tree (UW §5.3–§5.4): the scrollable's box. One
     wheel notch is one line step of the bar (UB §4.7, UW §5.6–§5.7, §6.8), so each notch
     repaints the box once;
   - a meter position change (UW §4.6), except for alpha-test meters (below): the meter's
     box. A smooth movement damages it on every frame while it runs. A frame meter damages
     it only when the frame index changes (UW §4.4 step 3);
   - **a nested entry that is shown or hidden, or whose coverage region changes** (Decision
     1.7: a move or resize of the entry, a change of its displayed box under a new scale, or
     the first rotation its surface holds, §2.3): its old and new coverage regions in the **parent
     surface's canvas** are damaged, because the parent's content there moves between the
     canvas below and the canvas above. The nested canvas itself is repainted only if its
     own canvas size changed. Turning an entry that has already been rotated does not change
     its coverage region (Decision 1.7), so it is **not** damage.

   A setter that does not change the effective value adds no damage. Plugins that call
   `SetText` with the same text in every `Update` therefore cost nothing. Hover adds damage
   only through the state switches it causes. The hover push of
   `ScrollableControls_Mouse` at priority 0 (UW §5.7, §6.9) changes input maps, not pixels;
   it belongs to IB (D5, D7).

   These changes are **not** surface damage:
   - a change of the cursor image when the captured or hovered element changes (UB §4.8). It
     sets the OS cursor, or changes the software cursor (§2.5);
   - a position change of an **alpha-test meter** (UW §4.5). The update only passes
     1 − *p* as the surface's alpha threshold, which is a composition-entry change
     (Decision 1.6). None of these meters is in the M5 panels (UW §4.5);
   - a change of the opacity of any surface; a change of the rotation of a root surface, or
     of a nested entry that has already been rotated; a change of the scale (Stretch) or the
     position of a **root** surface. Each changes only composition entries. For a nested
     entry, a move or a change of its displayed size changes its coverage region and is
     damage, as above.

   Once per frame, the damage of a surface is merged: overlapping or touching rectangles
   are joined. Above 16 rectangles, or when they cover more than half the surface, the
   bounding box is used. A change of the canvas size repaints the canvas in full.
5. **Paint jobs, uploads and the transition rule.**
   - **Paint jobs.** A surface is painted by one **job** at a time. A job takes an immutable
     paint snapshot (the display list plus `Arc`ed images) and the canvas size at its start,
     and paints the merged damage of that moment in horizontal bands, within the U4 slice
     per frame (§6). Damage that arrives while a job runs is kept for the next job. A
     running job is **never restarted or cancelled** because of new damage or a new size: it
     finishes at its own size first. So every job ends after a bounded number of frames,
     and a surface whose damage keeps coming (a continuous drag on a resize bar, a screen
     resize, a smooth meter) still updates, at the rate at which jobs finish.
   - **Front and back.** The GPU texture that is shown (the *front*) holds the content of
     the last job of that surface that has been **swapped in**. The CPU canvas belongs to the
     running job. A job is either a **content job** or a job of a **generation** (the
     transition rule below). The two upload differently:
     - **Content job.** When it completes, its rectangles are uploaded. If they fit what is
       left of the frame's upload budget and the job changed neither the canvas size nor the
       tile box, they go straight into the front texture in that frame, so the new content
       and its upload appear together. Otherwise the surface gets a **back texture**:
       1. Copy the front texture into it on the GPU (at the new size and tile box, if those
          changed).
       2. Upload the damage into it over as many frames as the per-frame cap of U3 needs.
          Bands that the job has finished may be uploaded before the job ends.
       3. Swap it in when the job is complete and fully uploaded.
       4. Until that swap, **no new job starts on the surface**, whether the job is still
          painting or only uploading. New damage waits in its damage list. Uploads read
          the CPU canvas, so this keeps the bytes that the remaining uploads read equal to
          the finished job's result, and no later content is mixed into a back texture
          that is still being filled. The wait is bounded by the upload cap: a job's
          rectangles are at most one canvas, so at most 4 frames at the 8 MiB cap for a
          canvas of 3840 × 2160 (31.6 MiB).
     - **Generation job.** Its rectangles **never reach a front texture before the frame in
       which its generation swaps.** Each surface of the generation either uploads them
       into a back texture (steps 1 and 2 above, bands possibly before the job ends) or, if
       its canvas size and tile box are unchanged, **holds** them in its CPU canvas. A
       surface whose canvas size or tile box changes always uses a back texture, because its
       front cannot hold the new size. A surface that holds rectangles starts no other job
       until the swap, so its canvas keeps them. The generation **swaps** in the first frame
       in which every job of it is complete and all the rectangles still held fit what is
       left of that frame's upload budget. In that frame the held rectangles are written
       into the front textures, the back textures are swapped in, and every entry of the
       group takes the new generation's geometry. When the held rectangles of a generation
       are more than one frame's cap, the client moves them into back textures over the
       following frames, so every generation swaps after a bounded number of frames.

     Only empty tiles are skipped: each merged rectangle is cut into its intersections with
     allocated tiles before upload, so a merged bounding box never uploads runs of zeros. A
     tile freed by the job is cleared on the GPU with a scissored clear draw, not by an
     upload of zeros; for a generation job the clear goes into its back texture, or is held
     with its rectangles until the swap.
   - **Budget.** Uploads count against a per-frame UI byte budget (U3 in §6). A surface whose
     rectangles do not fit in what is left of the budget keeps its previous texture and
     carries its upload to the next frame.
   - **Transition rule, per root group.** The entries derived from one root (Decision 1.1)
     form its **group**: the root's entry, its nested entries at every depth, their
     external content and every canvas above (Decision 1.7). A **layout change** of the
     group is any change to the root's displayed size, or to the displayed box, canvas size,
     coverage region or visibility of an entry in the group. Examples are a resize-bar step,
     a plugin `SetSize`, a scale change, and a child surface that is moved, shown or hidden.
     The jobs that one layout change starts form one **generation**. They are the jobs of
     every surface of the group that the change touches: each surface whose canvas size
     changes, and each parent surface whose direct nested entries change their coverage
     regions (the damage of Decision 1.4; for a resized root, its resize and those regions).
     While a generation is running, uploading or holding, the **whole group** is composed
     from the **last swapped generation**:
     - every entry keeps that generation's size, canvas-to-box scale, position relative to
       the root and coverage split, and is drawn from its front texture (which a job of the
       new generation has not touched, by the upload rule above). An entry that the new
       generation adds is not drawn yet, and one that it removes is still drawn;
     - the root's position is the current displayed position when the current displayed
       size equals the size of the last swapped generation (a pure move moves the whole
       group at once). Otherwise it is the position of the last swapped generation, so a
       panel resized from its left or top edge does not jump while its new paint runs;
     - opacity and rotation always take their current values.

     When the generation swaps, the whole group takes the new generation in the same frame.
     So the screen never shows a texture on a box of another size, a child surface never
     stands out of line with its lagging window, parent content is never split around a
     child that is drawn at another place, and no surface that has been shown is ever shown
     half-updated. Example: a root job that moves the parent's content between the canvas
     below and the canvas above for a moved child finishes first, while the child's first
     paint is still withheld. Its rectangles stay held (or in the back texture) until the
     child's job counts as complete, and both appear in the same frame.

     **Content jobs and generations.** A content job (no layout change) is not tied to a
     generation; it swaps on its own surface, in the box the group shows at that moment.
     While a surface has a job in a generation that has not swapped, no content job is
     started on that surface: its new damage waits in its damage list for the job that
     starts after the swap. A content job that was already running when the layout change
     arrived finishes and swaps as a content job, because its content belongs to the
     generation that is shown; the surface's generation job starts after it. Surfaces of
     the group that are not part of the generation run content jobs as usual: their canvas
     size and their coverage split are unchanged, so their content does not depend on what
     the generation changes.

     A layout change that arrives while a generation runs is kept for the next generation,
     as damage is kept for the next job. The next generation starts in the frame after the
     running one swaps. A small generation (a child surface moved by a plugin, a hover
     state that shows a child) normally completes, uploads and swaps in the frame in which
     it starts, so in steady state it shows no lag.

     **A nested surface shown while a generation runs.** Showing a nested surface is a
     layout change of its group (its entry appears, and its parent's split changes), so it
     waits for the next generation like any other. Only its parent's split waits, though:
     the shown surface's **first-paint job starts at once**, in the frame of the show,
     because its canvas is new and belongs to no running job. When the next generation
     starts, that job is part of it, already running or complete, and the generation adds
     only the parent's repaint of the coverage regions (and whatever other layout changes
     were kept). The surface is therefore first drawn at the swap of that next
     generation, and the bound is the later of:
     - 100 ms after the show (Decision 1.8, as for any first paint); and
     - the rest of the running generation, plus the next generation, each bounded by U4
       (d) and (e).

     Examples (U4 (c)): a child shown in P10 during a continuous resize-bar drag waits for
     the running generation (at most 4 frames) and for the next, which also carries the
     next drag step (at most 4 frames): at most 8 frames, about 133 ms at 60 Hz. A plugin
     control in stretch mode shown in the screen-sized plugin window during a screen-resize
     generation waits for that generation (at most 40 frames by U4 (e)) and then for the
     repaint of its coverage region in the window's canvas, normally one frame: at most
     about 41 frames, about 0.7 s at 60 Hz. A **root** (a newly shown panel or plugin
     window) is a group of its own, so no other group's generation delays it, and it keeps
     the plain 100 ms bound. The longer wait is a recorded difference (owner question 5).
     Joining the running generation was considered and not chosen: that generation's
     parent job has already taken its snapshot with the old split, and a surface runs one
     job at a time, so the parent's new split cannot be added to it.
   - **The one exception: a first paint.** A surface that has never had a job swapped in
     (newly created, or shown again after its surface was released) has nothing to keep.
     It is withheld until the condition of Decision 1.8 holds (its first job complete
     **and** the images its current states request arrived), for at most 100 ms (U4 (c);
     for a nested surface shown during a running generation, see the bound above).
     After that, its finished bands go straight into its texture as they complete, under
     the upload budget; the parts not yet painted show the surface's initial content,
     transparent black. This is a recorded difference: the original is assumed to paint in
     one go. A nested surface whose first paint belongs to a generation counts as complete
     for that generation when the Decision 1.8 condition holds or its 100 ms run out. It is
     then released from the generation and goes on as just described; before the swap
     nothing draws it, because the entry is new to the generation. When that generation
     is one that waits for a running generation, the first draw comes later than 100 ms,
     as bounded above.
   - **Hit testing during a transition** uses the current layout (UB §4.1–§4.2; IB D8: the
     step-1 hit tests and the step-7 hover refresh use the layout of the last completed step
     8), not the drawn box. While a generation runs, a click near a moving edge or on a
     moved child can therefore hit a box that is not drawn yet. This is a recorded known
     difference, bounded by the job time that U4 bounds.
6. **Composition (GPU).** The client gives the renderer an ordered composition list. Each
   entry holds:
   - the surface's texture, with the offset of its tile box inside the canvas (Decision
     1.2), and the canvas size;
   - its destination box: the element's **displayed box** of UB §9.2.1 in screen pixels,
     under the transition rule of Decision 1.5. For every surface **except Tile** surfaces,
     the **composition scale** is that box's size divided by the canvas size, on each axis,
     whatever the object mode. It is 1 : 1 only where the two sizes are equal (§2.1). A
     **Tile** surface (native Tile with power-of-two sides, plugin mode 2) is not fitted to
     its box: its canvas is repeated over the box (repeat addressing, below), so its scale
     is the **display-node scale** of UB §9.2.1, that is (*ex*, *ey*) of the nearest
     display node at or above the element. That scale is (1, 1) unless a panel above is
     scaled;
   - its **transform** (§2.3): the rotation held by its own surface, about its own centre,
     if any, followed by the rotation held by every surfaced ancestor's surface about that
     ancestor's centre, up to its root. Rotations are read from the surfaces, never from
     the `GetRotation()` values (§2.3). An unrotated surface contributes nothing. So every
     entry derived from a rotated root (its nested entries, their canvases above, Decision
     1.7) turns with the root;
   - its **clip chain**: the displayed boxes of its **clip surfaces**, each tested in that
     surface's own unrotated space (§8: the fragment's position is taken back through that
     surface's transform and discarded outside its rectangle). The clip surfaces of an
     entry are the surfaces **strictly above** its element that are a root or belong to a
     root panel (a plugin `Window`, a native root panel), stand-in surfaces of Decision 1.1
     included. The nearest of them is the box that LPA §13.18's Tessera rule names
     ("clipped to its window only"); the ones further up cut only where a child window
     itself extends past its own enclosing window (Decision 1.7, "Clip"). Surfaces that
     belong to other elements (a plugin control with a stretch mode, the minimap marker)
     do not clip. A root entry has an empty chain. The chain has at most 8 boxes (an
     entry has at most 8 surfaced ancestors under the depth cap of §7); the builder intersects consecutive boxes that share one unrotated
     space into one, so an unrotated chain is a single screen scissor, which the renderer
     may use as a fast path;
   - its opacity: the element's current `UICore_Element_ObjectOpacity`, clamped to 0–1 when
     drawn (LPA §13.19 Tessera rule). Auto-fade (UB §11.1), Fade items (UL §6.6) and the
     plugin `SetOpacity` (LPA §13.19) all write that one property, so nothing else is
     multiplied in. Canvases above (Decision 1.7) are root content and take the opacity of
     the surface they belong to. A stand-in surface of a mode-0 root panel (Decision 1.1)
     is composed at opacity 1;
   - a **source flag** for external content (Decision 1.7): its texels are already
     premultiplied, so the shader skips its premultiply step for them;
   - its filter (§2.2);
   - an alpha threshold, used only by alpha-test meters (UW §4.5 Tessera rule: drop the
     pixels whose alpha ÷ 255 is below 1 − *p*);
   - repeat addressing, used only by Tile surfaces (§2.1). The canvas position of a screen
     position *p* inside the box (in the root's unrotated space) is
     ((*p* − box origin) ÷ scale) modulo the canvas size, on each axis, with the
     display-node scale above. With scale 1 this is an exact copy at 1 : 1 that wraps at the
     canvas edges, with integer texel positions. Inside a scaled panel the pattern is scaled
     with the panel, like the rest of the panel's art, and filtered as §2.2 says, with the
     filter's neighbour texels also wrapped. The wrap comes first; the tile-box lookup of
     Decision 1.2 is applied to the wrapped position. A Tile surface has no nested entries
     and so no canvases above (Decision 1.7, "Tile parents"): everything below it is in its
     canvas and repeats with it. This is a Tessera rule (UL §8.4 OPEN, LPA §13.18 OPEN (2)).

   The order is the screen root's drawing order (UL §9.3), with nested entries placed as
   Decision 1.7 says. The software cursor is not part of the list (below).

   **Order of the screen-space passes.** After the world passes the renderer draws:
   1. the walker's own overlays, when shown: the HUD text and the walker minimap
      (`walker.md` §6);
   2. the **UI pass**: this composition list;
   3. Tessera's developer overlay: the egui frame-stats overlay (ARCHITECTURE §13,
      "Diagnostics"), which is **planned** (`apps/client` has no egui dependency today) and
      off by default;
   4. the software cursor (§2.5), when it is used.

   **Walker overlays with the game UI attached** (Tessera rule). They are drawn **under**
   the game UI, as the world is, so they never cover a native panel or a plugin window.
   Drawn above, they would hide what M5 must compare. At 3840 × 2160 the walker minimap is
   drawn at x = w − size − 14, y = 14 with size = clamp(0.3 h, 120, 320) = 320
   (`apps/client/src/walker/mod.rs`, the overlay in the frame recording), so it covers
   3506–3826 × 14–334. P1 lays out at (3638, 12, 224, 280) there (UL §8.4, §17.3: 202
   pixels from the right edge at every listed size), almost entirely under it. The HUD text
   starts at (10, 10), inside P4 at (2, 2, 280, 300). P1 and P4 are M5 checklist panels
   (ROADMAP M5 exit criterion 2, `m5-visual.md` P1-A). In addition:
   - the walker minimap is **off** by default while the game UI is attached, because P1 is
     the game's minimap. The Tessera setting `walker_minimap = auto | on | off` (`auto`: off
     with a game UI, on without) brings it back, still under the UI;
   - the walker HUD stays on by default, for development, with the Tessera setting
     `walker_hud = on | off`. HUD text under semi-transparent panel art would show through
     it, so the M5 visual checklist run and the owner's comparison screenshots use
     `walker_hud = off`; the scripted run of §6 reads `FrameStats` instead.

   Without a game UI attached (the walker of M3 and M4) nothing changes: there is no UI
   pass, and both overlays draw on top as today. The developer overlay stays above the game
   UI, because it is a tool that is off by default and off in checklist runs. The software
   cursor is drawn last of all, so the pointer is never under the HUD or a tool window.
   ARCHITECTURE §13's frame structure names none of these passes yet; the Consequences add
   them, and `walker.md` §6 gets the rule.
7. **Nested surfaces and external content.** A surface can sit inside another surface.
   Examples are a plugin control with a stretch mode other than 0 inside its window, a
   plugin child `Window` inside its parent window (Decision 1.1), an alpha-test meter
   inside a panel (none in the M5 panels, UW §4.5), and the minimap's player marker (GP §8.2
   step 3).
   Viewport and `TargetViewport` elements (and later Browser and Movie) have content drawn by
   the GPU.

   Each of them is composed as a **separate entry**, never blended into its parent's canvas
   on the CPU. Definitions:
   - The **parent surface** of a nested element is the surface of its nearest ancestor that
     owns one. A nested element is a **direct nested entry** of that surface. Nesting is
     recursive: a nested surface splits its own content by its own direct nested entries in
     the same way.
   - The **coverage region** of a nested entry *N* is a rectangle in its parent surface
     *P*'s **canvas** coordinates. Those are the unscaled pixels of *P*'s layout box, less
     *P*'s canvas origin offset (§7). It is computed in three steps:
     1. **Footprint, in the root's unrotated space.** Take *N*'s displayed box (UB §9.2.1),
        made absolute by adding the displayed positions of its ancestors. If *N* has its
        **own rotation** (a rotation held by its own surface, §2.3), replace the box by the
        square around the circle through its corners: same centre, half side = half the
        box's diagonal. This applies to any non-root entry whose surface game code or a
        plugin rotates: in M5 the minimap marker, and for plugins a child window whose own
        surface received a `SetRotation`, or a nested surface that received the angles of a
        `SetRotation` made on a hidden window below it (§2.3). The square holds
        the rotated quad at every angle, `x` and `y` tilts included,
        because the projection is orthographic (§2.3). So turning never changes the region.
        The 28 × 28 marker gets a square of side 28√2 ≈ 39.6, which is 40 × 40 after
        rounding. An entry has its own rotation from the first time its surface holds an
        angle that is not within 0.0002 of 0 (a surface created with a remembered rotation
        holds it from its creation) until the surface is released, even while the angle is
        back at 0. That first rotation is one coverage change (Decision 1.4).
        Ancestors' rotations are not part of the footprint: they turn the whole group after
        the split (Decision 1.6).
     2. **Join** the footprints of *N* and of every nested entry below *N*, at any depth.
     3. **Convert to *P*'s canvas.** Subtract *P*'s displayed origin (in the same space),
        divide by *P*'s composition scale on each axis (Decision 1.6; *P* is never a Tile
        surface, see "Tile parents" below), and subtract *P*'s canvas origin offset. Round **outward**:
        left and top down, right and bottom up. Then clip to *P*'s canvas.

     Step 3 matters because displayed boxes are scaled (UB §9.2.1) while canvases are not.
     A nested surface in a scaled panel of a scaler type (the normal case after UB §9.2
     step 6) has a displayed box of about *s* times its layout box, and step 3 brings it
     back to canvas pixels. A change of the root's scale is divided out again, so it
     normally leaves every coverage region unchanged. Where rounding moves a region by a
     pixel, that is a coverage change like any other.

   The parent surface's content is split into canvases, using the parent's drawing order
   (UL §9.3), in which each direct nested entry *N*<sub>1</sub>, …, *N*<sub>k</sub> has its
   place:
   - Every pixel that the parent draws is assigned to the **last** direct nested entry
     before it in drawing order whose coverage region contains that pixel. It goes to that
     entry's **canvas above**, *A*<sub>i</sub>. A pixel with no such entry goes to the
     parent's **canvas below**, its ordinary canvas.
   - The composition order for the parent is: the canvas below; then, for each direct nested
     entry in drawing order, that entry (recursively expanded the same way) followed by its
     canvas above.

   This gives the drawing order of a single canvas at every pixel, for any number of
   sibling, overlapping or deeply nested surfaces: at a pixel covered by *N*<sub>1</sub>
   and *N*<sub>3</sub> the order is below, *N*<sub>1</sub>, *A*<sub>1</sub>,
   *N*<sub>3</sub>, *A*<sub>3</sub>, and *A*<sub>2</sub> holds nothing there. Every pixel
   outside all coverage regions keeps the exact single-canvas result. Inside a coverage
   region, the content above a nested entry is composed with the GPU blend instead of the
   compositor operations, and an operation that reads the destination there sees the
   transparent black of the canvas above instead of what lies below. That is a recorded
   approximation until LPA §13.18 OPEN (1) is settled.

   Each canvas above has its parent surface's canvas size and is sparse (Decision 1.2), so
   it costs memory only where something is drawn over a nested entry. It is composed with
   exactly its parent's destination box, transform, clip, opacity and filter, so it needs
   no placement of its own. The split exists only while a nested entry is displayed.

   The Tessera rules for nested entries:
   - **Opacity:** a nested entry has its own `ObjectOpacity` only. The ancestors' opacity is
     **not** multiplied in. This follows the BIN fact of UB §11.1 step 2: auto-fade writes
     the fade opacity into the element **and** into each child with its own surface. That
     only makes sense if the child is composed on its own; if the child were blended into
     its parent, the fade would apply twice (*o*²). This differs from the Tessera rule of
     LPA §13.19 OPEN, which composes nested surfaces into their window so that the window's
     opacity applies to them. That rule is an unsourced default, and this record proposes
     that the spec writer align it (Consequences).

     UB §11.1 step 2 covers auto-fade only. Fade items (UL §6.6) and the plugin
     `SetOpacity` (LPA §13.19) write the element's own `ObjectOpacity` and nothing else, so
     under this rule they leave nested surfaces **unfaded**. Two visible consequences:
     a Fade item on P1's root fades the radar but not the player marker (its own surface,
     GP §8.2 step 3); a plugin window set to opacity 0 still shows its children in stretch
     mode 1, 2 or 3. This contradicts the wording of UL §5.9 (`ObjectOpacity` "fades the
     element with all its children") and UL §6.6 (the element "fades together with its
     children"), not only LPA §13.19's Tessera rule. Owner question 3 and spec question 1
     carry this. The choice is one switch in the composition-list builder: "own" or
     "own × ancestors". The second fades nested surfaces with Fade items and `SetOpacity`,
     but applies auto-fade twice to them (*o*²).
   - **Clip:** the clip chain of Decision 1.6. A nested entry is clipped by its **nearest
     enclosing window**: the nearest surface strictly above it that is a root or belongs to
     a root panel (a plugin `Window`, a native root panel, or a stand-in surface of Decision
     1.1), tested in that surface's own rotated space. This is the Tessera rule of LPA
     §13.18 OPEN (1), "clipped to its window only". Surfaces of other elements between them
     (a plugin control with a stretch mode) do not clip it, which agrees with the community
     description of plugin mode 3, "not clipped by the parent" (LPA §13.18). Example: a
     stretch-mode control inside a child window that extends past the child window is cut
     at the child window's edge, not at the outer window's.

     The record **adds** the windows further up (Tessera extension, spec question 12): the
     entry is also clipped by each enclosing window above its nearest one, up to the
     root. This changes nothing while a child window lies inside its own parent window,
     which is the normal case. It matters only when a child window extends past its parent
     window: the child window, a nested entry, is cut at the parent's edge, and so is the
     content of its own nested entries, so no part of a child window is drawn where the
     child window itself is cut. LPA §13.18's Tessera rule does not cover this case: for an
     entry inside a child window, "its window" is the child window.

     For the M5 native panels the result is the same as clipping by the root, because their
     only named nested entry, the P1 marker, has the radar root as its nearest root panel.
   - **Transform:** a nested entry inherits the rotation of every surfaced ancestor about that
     ancestor's centre (Decision 1.6). So in a window rotated with `SetRotation` (LPA §13.20,
     mostly `z` headings) a child arrow with `SetStretchMode(1)` turns with the window and
     is clipped by the window's rotated rectangle, not by an axis-aligned screen box. A
     child window whose own surface holds a rotation (§2.3) turns about its own centre
     first and then with its ancestors; that rotation is state of its surface, read into
     the entry, not root-only data.
   - **Fades** of nested surfaces therefore cost no repaint, as for roots.
   - **Tile parents** (Tessera rule). A Tile surface (native Tile with power-of-two sides,
     plugin mode 2) has **no nested entries**. Repeat addressing (Decision 1.6) draws the
     canvas at every period, while a nested entry would be drawn once, and a coverage
     region in one canvas period would not match the entry's place in the other periods.
     So an element below a Tile surface whose mode would give it a surface is painted into
     the Tile surface's canvas as if its mode were 0 (unscaled, without its own opacity or
     rotation), as the cap fallback of §7 does, and repeats with the canvas. External
     content below a Tile surface draws nothing. Each case gives one warning (per plugin to
     its error listeners, or one `tracing` warning for native panels) and a `FrameStats`
     counter. The element's stored mode, `GetStretchMode()` and its resize rule (UB §9.2.1)
     are unchanged. The data has 2 native Tile elements (UL §8.4); whether anything below
     them owns a surface is not stated, and the census of spec question 7 covers it. A
     plugin reaches this rule only with a mode-2 control that holds a stretch-mode child
     or a child window. This replaces the earlier unstated behaviour (the entry drawn once
     at its place while the canvas repeated).
   - **External content in M5:** the portraits of P4 and P5 are optional (UW §12 Tessera
     rule, UL §17.4). The `UICore_Viewport_RenderToTexture…` properties (UW §12) are
     recorded but unused until UW §12's OPEN is answered. Browser and Movie items stay
     empty in M5. The offscreen target (Tessera rule, UW §12 OPEN):
     - **Size:** the element's layout box, so it is composed onto the displayed box with
       the composition scale of any surface (Decision 1.6).
     - **Format and colour space:** `Rgba8Unorm` (the renderer's `OffscreenFormat::
       Rgba8Unorm`), never sRGB. The lit passes compute stored values (`render.md` §4.1),
       so the target holds stored values, and `textureLoad` returns them unconverted, as
       the UI pass's non-sRGB view expects (§8). An sRGB target would return decoded values
       and darken the portrait in the non-sRGB UI view, so the client refuses to register
       one as UI content (a categorised failure; the portrait draws nothing).
     - **Alpha:** **premultiplied**. The target is cleared to transparent black
       (0, 0, 0, 0) each frame it is drawn. Opaque draws write alpha 1; translucent draws
       blend alpha with `One`/`OneMinusSrcAlpha`, so where nothing is drawn the entry is
       transparent and the panel art below shows, and blended colour over the cleared target
       is premultiplied. The entry carries the source flag of Decision 1.6, and the shader
       skips its premultiply step for it; filtering, opacity and the blend are those of any
       entry. These are requirements on the portrait's passes, which the render implementer
       states in the `render.md` UI section (§8).

   **How many native elements this affects.** Under Decision 1.1 every non-root element with
   a non-zero object mode in its layout gets a surface, so the region where surface contents
   are not byte-exact (content drawn after a nested surface, inside its coverage region) is
   bounded by the number of such elements in each panel. The specs do not give that number
   per M5 panel: UL §5.9 and §8.4 count object modes over all 302 layouts (Stretch 301,
   CanvasResize 119, None 24, Tile 2) without saying how many are on root panels, and UL
   §17.2 does not split them by panel. The only non-root surfaces the specs name in the M5 panels are the
   P1 marker (GP §8.2) and the P4 and P5 portraits (external content). The alpha-test
   meters do not concern M5: UW §4.5 says that none of the 4 (UW §4.8) is in the UL §17
   panels. This record cannot count the rest, because the architect role reads no game
   data. It therefore (a) asks the
   spec writers for the census (Consequences, question 7), and (b) has `tessera-ui` log, for
   each panel it opens, the number of nested surfaces and the area of their coverage
   regions, which the M5 checklist run records next to the U1–U5 results.
8. **Showing and hiding.**
   - **Hiding** an element that has a surface releases its canvas, its canvases above and its
     texture, unless `UICore_Element_KeepObjectWhenHidden` is true (UB §11.2, LPA §13.19; 35
     elements in the data). Showing it again creates a new surface and paints it in full,
     through a first paint (Decision 1.5). A rotation survives this as LPA §13.20 step 3
     says: the new surface gets the rotation the window remembered from its own last
     surface, not the `GetRotation()` angles (§2.3). Hiding a window does not release the
     surfaces of its children that are still shown themselves (Decision 1.1).
   - **A mode change on an existing surface** (`SetStretchMode` from one non-zero mode to
     another, for example 1 → 3 or 3 → 2; LPA §13.18 "the surface is refreshed for the new
     mode"; Tessera rule, because only that word was traced). The surface is **kept**, with
     its texture, its rotation and its opacity, so it is **not a first paint**. Its canvas
     takes the size that the new mode gives (§2.1: the current layout box for modes 1, 3
     and 4, as at creation; for mode 2 the base image's size when both sides are powers of
     two, otherwise the layout box). Then:
     - if the canvas size, the entry's displayed box or its coverage region changes, or the
       change is to or from Tile (which removes or allows the surfaces below it, Decision
       1.7), it is a **layout change of its group**: a generation (Decision 1.5) whose job
       repaints the canvas in full into a back texture, together with the parent's split
       and the surfaces below that are released or created. The old picture stays until
       the swap;
     - otherwise only the entry's composition changes (its scale, or repeat addressing),
       which takes effect in the next composition list, with no repaint.

     A change to mode 0 releases the surface and a change from 0 to another mode creates
     one, through a first paint (LPA §13.18–§13.19).
   - **The logical show is immediate.** Visibility for hit tests, `activate_on_show` and
     `focus_on_show` (UB §11.2, §5.1) take effect in the frame in which code shows the
     element, exactly as in the original. Plugins and game code never see a delayed state.
   - **The visual show** of a newly created surface waits until two things are done: its
     first job, and the arrival of the images that its current states request when it is
     opened. It waits **at most 100 ms** (U4 (c)), with one exception: a nested surface
     shown while its group's generation runs also waits for that generation and the next
     (Decision 1.5, "A nested surface shown while a generation runs"). This is the one
     definition of "first paint complete" that Decision 1.5 and U4 (c) use. When the 100 ms run out first, the surface
     is drawn with what has been painted and what has arrived: the completed bands of its
     first job (Decision 1.5)
     and, in them, the images that have arrived. Later images and bands are painted in as
     they arrive, under the no-stall rule of ARCHITECTURE §21. A pending image is not a
     failed one: it draws nothing until it arrives, and only a failed load gets the
     placeholder (SK §6.5).
   - **Withheld window.** During that wait of up to 100 ms, a click can reach a panel that
     is not drawn yet. This is a recorded difference. It is bounded by U4 and affects only
     panels that open under a pointer.
   - **Auto-fade and the withheld first paint** (Tessera rule). The fade-in of UB §11.1
     starts at the **logical** show: its start time, its `Element_auto_fade_start` and
     `Element_auto_fade_stop` messages, its opacity and its hit-test exclusion all follow
     the original's clock. The drawn opacity is the current `ObjectOpacity` (Decision 1.6),
     so a surface whose first paint is withheld for *d* ms first appears part-way through
     its fade. With the common 0.1 s fade (101 of the 105 elements, UB §11.1), it appears
     at opacity *d* ÷ 100 and the fade is cut short. When *d* ≥ 100 the fade is not seen
     and the surface appears at full opacity. This is a recorded difference. It keeps the
     rule that plugins and game code never see a delayed state, and it touches only
     surfaces that need a first paint: a surface kept with `KeepObjectWhenHidden` shows at
     once. Starting a separate drawn-opacity clock at the visual show would keep the fade
     visible, but it would add a second opacity besides `ObjectOpacity`. It is the change
     to make if the owner prefers it.

### 2. Object modes, scale, rotation, hit testing and the cursor

1. **Object modes.** For every surface, the canvas has the size of the element's **layout
   box** (UB §9.2.1), with the one exception of plugin mode 2 below. The composition
   destination is the **displayed box**. The composition scale is displayed box ÷ canvas
   size, except for Tile surfaces, which repeat their canvas over the box at the
   display-node scale (Decision 1.6). Apart from Tile, the object mode decides only what an
   element resize does to the layout box (UB §9.2.1), and so whether the canvas size changes
   and a repaint is needed:

   | Mode | Surface | Canvas | Element resize | Composition |
   |---|---|---|---|---|
   | 0 None | none (Decision 1.1), except a shown root panel with no surface above it, which is drawn through a stand-in surface (Tessera rule, Decision 1.1); a mode-0 child window is drawn into the nearest surface above it | the layout box | the element owns no surface (condition 1 of Decision 1.1), so the no-surface rule of UB §9.2.1 step 3 applies: the layout box takes the size and the children are laid out again. A root panel drawn through a stand-in surface then resizes its canvas and repaints it in full, as for CanvasResize. This covers a plugin window after `SetStretchMode(0)` (`SetSize`, resize bars, screen re-layout; LPA §13.15, §13.19). UB §9.2.1's branch for a surface-owning element in None (the layout box is not changed) is never reached, because by LPA §13.19 condition 1 a mode-0 element owns no surface; the two specs state different things, and spec question 14 asks which holds | as CanvasResize; a stand-in surface at opacity 1 (Decision 1.1) |
   | 1 Stretch | yes | the layout box, which a Stretch resize does not change; so it keeps its size, as LPA §13.18 says for plugin mode 1 | changes the scale vector, not the layout box: no repaint | scaled onto the displayed box; filter §2.2 |
   | 2 Tile, native | yes | the layout box, if both sides are powers of two; otherwise the client's fallback: switch to CanvasResize, one log entry per element (UL §8.4) | the layout box is not changed (UB §9.2.1) | power-of-two case (Tessera rule, UL §8.4 OPEN): **not** fitted to the box; repeat addressing from the top-left over the displayed box at the display-node scale (Decision 1.6). Unscaled, this is an exact 1 : 1 copy; inside a scaled panel the canvas is scaled with the panel and its displayed box, so it still covers that box about once. 2 elements in the data |
   | 3 CanvasResize (the default for a surface without a value) | yes | the layout box; a resize changes it and repaints in full | changes the layout box and lays the children out again | onto the displayed box: 1 : 1 without a scale; **scaled** when a display node at or above has a scale, which is the normal case for the scaler-type panels after UB §9.2 step 6 restored their mode |

   **Plugin stretch modes** (LPA §13.18). Any mode other than 0 gives the control its own
   surface, composed by Decision 1.7. The Tessera rules of LPA §13.18 OPEN apply:

   | Mode | Canvas | Composition |
   |---|---|---|
   | 1 Stretch, and 4 | the control's layout box, which keeps the size it had when the surface was created or last refreshed for a new mode (Decision 1.8) | onto the control's current displayed box (scaled when they differ); filter §2.2. Mode 4 leaves mouse visibility unchanged |
   | 2 Tile | the size of the control's base image, if both sides are powers of two; otherwise as mode 3 | **not** fitted to the box: repeated from the control's top-left corner over its displayed box, by repeat addressing at the display-node scale (Decision 1.6). Example: a 32 × 32 canvas on a 200 × 100 control is drawn 1 : 1 and repeats 6.25 times across and 3.125 times down; inside a panel scaled by 1.5 each repeat covers 48 × 48 screen pixels |
   | 3 CanvasResize | the control's layout box | onto its displayed box, 1 : 1 unless a scale applies |

   Earlier drafts of this record proposed a CPU nearest-neighbour blit for the stretch
   modes. That is withdrawn: LPA §13.18 (BIN) shows that the modes act on the control's
   surface, not on the image layer. The community note that "mode 0 resets alpha"
   (LPA §13.18) needs no special case. It is the default Replace operation of LPA §13.17,
   which writes the image's alpha into the canvas in every mode.

   **Plugin `SetScale`** goes through the native panel-scale rule (LPA §13.21 rule 5, UB
   §9.2 steps 2–6). The renderer handles both possible results: a Stretch resize changes
   only the composition entry, and a CanvasResize resize repaints. Which result applies is a
   question for the specs. UB §9.2 step 6 resizes scaler types in Stretch, so the children
   are not laid out again (UB §9.2.1). LPA §13.21 rule 1 says that the children of a scaled
   window "follow their own edge modes". The Consequences ask the spec writers to reconcile
   the two. Until then Tessera follows UB §9.2 step 6, the rule that LPA §13.21 rule 5
   cites.
2. **Scale filter.**
   - **At exactly 1 : 1** (composition scale 1 on both axes, which for a Tile surface is
     its display-node scale, and no rotation in the entry's transform), the shader reads
     texels with integer positions, so the screen gets the canvas texels unchanged.
   - **When scaled or rotated**, the filter is **bilinear** with clamp-to-edge.
   - **Premultiplied filtering.** Canvases are straight alpha, and erased areas are
     transparent black. A hardware bilinear filter would therefore pull black into the
     edges of scaled surfaces. So the shader filters itself: it reads the four texels,
     premultiplies each, weights them, and outputs premultiplied colour (§8, UI pipeline).
   - **Status.** This is a Tessera default, with the setting `ui_scale_filter = linear |
     nearest` in Tessera's own settings. It stays until the owner screenshot that FO Q3 asks
     for settles the original's filter.
   - **Whole surface, not each element.** Each surface is resampled as a whole, and the
     elements in it are not resampled one by one. This is a **Tessera default**, not a
     fact: it is OPEN in FO Q3 and UB §17 item 18. It matches the data model, in which an
     element without its own display node takes its scale from the nearest node at or above
     it (FO §6.3 item 1, UB §4.1 step 3, §9.2.1).
   - **No mip chain.** Scales go down to 0.4 (UB §9.1), and without mips a reduced surface
     aliases. Nothing in the specs says that the original filters UI surfaces with mips, and
     adding them could make Tessera differ in the other direction. Settle it with the owner
     screenshot at a scale below 1 (Consequences). A premultiplied mip chain per reduced
     surface is the change if the screenshot shows smoothing.
   - **Text** of game fonts is never re-rasterised at the scaled size (FO §6.3 Tessera
     Decision).
3. **Rotation.**
   - **Rotation is state of a surface** (LPA §13.20 step 3, BIN). Every surface holds three
     angles; a new surface holds (0, 0, 0) unless a remembered rotation is restored (below).
     A `SetRotation` on a window *W* runs as LPA §13.20 says:
     1. Step 2 first: it may set *W*'s object mode to 1, which creates *W*'s surface at once
        if *W* is shown (a stand-in surface of Decision 1.1 becomes that surface in place).
     2. Step 3, the **target**: *W*'s own surface if it has one now, otherwise the surface
        of the nearest ancestor that has one. Stand-in surfaces of Decision 1.1 are not
        targets, because for the original those elements have no surface. With no target
        the call rotates nothing and `GetRotation()` keeps its value (LPA §13.20 OPEN,
        Tessera rule).
     3. The target surface takes the angles, unless each differs from its current one by
        at most 0.0002. The angles are stored for `GetRotation()` only when a target was
        found.
     4. When the target is *W*'s own surface, *W* **remembers** the resulting rotation.
        When *W*'s surface is released (hide) and created again (show), the new surface
        gets that remembered rotation. A call made while *W* has no surface does not change
        what *W* remembers.

     **Composition reads the surfaces, never `GetRotation()`.** The rotation of an entry
     (Decision 1.6) is the rotation held by its surface; a canvas above takes its
     surface's. Example: a hidden child window *C* of a shown window *P* gets
     `SetRotation{z = 60}`. The target is *P*'s surface, so *P* turns by 60 with all its
     nested entries, and `C:GetRotation()` returns *z* = 60. When *C* is shown, its new
     surface gets *C*'s remembered rotation (0, 0, 0 if *C* was never rotated while shown),
     so *C* is drawn unrotated relative to *P*, turning only with it. If *P* is a root, its
     turn is a root rotation and no damage; if *P* is itself a nested entry, this is a
     rotation of its own surface, so its coverage becomes the angle-independent square
     (Decision 1.7 step 1), a coverage change the first time. Any surface can therefore
     hold a rotation: a window's, an ancestor control's with a stretch mode that received a
     window's call, and the minimap marker's, set by game code (below).
   - **Drawing** (LPA §13.20 step 4). The renderer reduces each angle modulo 360 and
     converts it to radians as LPA §13.20 step 4 says. It rotates the quad about its centre
     with the Tessera rules of LPA §13.20 OPEN: positive *z* turns counter-clockwise on
     screen, the order is *x*, *y*, *z*, and the projection is orthographic. When all three
     angles are within 0.0002 of 0, the unrotated path is used. Apart from a step-2 mode
     change, a rotation changes only composition entries. A nested entry whose surface
     holds a rotation has it as its **own rotation**: its coverage region is the
     angle-independent square of Decision 1.7, and its ancestors' rotations follow its own
     (Decision 1.6).
   - **Entries derived from a rotated root** (its nested entries and canvases above) inherit
     the root's rotation about the root's centre (Decision 1.6, 1.7), and each clip box of
     their clip chain is tested in its own window's rotated space.
   - **The minimap marker** (GP §8.6.3; Tessera rule answering that section's resampling
     OPEN). The marker owns a surface (GP §8.2 step 3), so it is a nested entry, and the
     radar's rotation is a `z` rotation of that entry about its centre. The angle is set so
     that the screen result is the one GP §8.6.3 gives: the east-pointing arrow turns
     **clockwise on screen** by (*h* − 90°), with *h* the compass heading (CO §11.4). This
     is fixed by the screen result, not by the plugin sign convention: if the owner test of
     LPA §13.20 reverses the plugin direction, the marker does not change. The resampling is
     the premultiplied bilinear filter of §2.2 (or nearest, by the same setting). The marker
     has its own rotation, so its coverage region is the angle-independent square of
     Decision 1.7: 40 × 40 around its centre, the same at every heading. Turning (including
     during mouse-look) therefore changes only its entry: no damage in P1, no repaint and
     no upload. The rim content drawn after the marker inside that square lands in its
     canvas above, also at every heading.
4. **Hit testing.**
   - The hit test works in displayed boxes (UB §4.1–§4.2, §9.2.1), and composition draws
     every surface on its displayed box (Decision 1.6), so drawing and hit testing need no
     separate inverse transform. They agree except in three recorded cases:
     1. rotation, which hit testing ignores (LPA §13.20 OPEN, Tessera rule);
     2. a resize whose job is still running (Decision 1.5, transition rule);
     3. **rounding under a scale.** An element without its own surface is drawn where the
        whole surface puts it: its canvas position times the surface's composition scale
        (§2.2, whole-surface resampling). Its hit-test box is its displayed box, which UB
        §9.2.1 derives level by level with truncation (left = trunc(*l* × *px*) relative to
        its parent, width = trunc((*w* + 1) × *ex* − 1)). The two can differ by less than
        one screen pixel per nesting level of the element below its surface, so by a pixel
        or two at an element's edge. Under scale 1 they are equal. Composition and the
        coverage regions use the surfaces' displayed boxes, which are the same in both, so
        only clicks on an element's edge pixels are affected. This is not within any
        tolerance of the filter (§2.2 decides colours, not positions); it is a recorded
        difference.
   - Points inside a scaled surface are converted back to unscaled element pixels where the
     specs say so: the alpha threshold divides when any scale component differs from 1 by
     more than 0.0002 (UB §4.1 step 3); text hit tests always divide (FO §6.3 item 3). Both
     use the scale of the nearest display node at or above the element (UB §9.2.1, UB §17
     item 18 for the open parts).
   - Hit tests in frame-loop step 1, and the hover refresh of the step-7 tick, use the
     layout of the last completed step 8 (IB D8, and IB "Coordination" item 5).
5. **Cursor.**
   - **Image.** The cursor image comes from Cursor media items of the captured or hovered
     element, then from a code override, then the default (UB §4.8). Its size is
     trunc(32 × `UIScale_Cursor`) pixels square, with the hot spot scaled with it (UB §9.3,
     Probable for the hot spot). The image is resampled once, with the same filter setting
     as surfaces. A change of image or size is not surface damage (Decision 1.4); OS cursor
     images are cached per (image, size).
   - **OS cursor, provisional size rule.** Tessera shows the cursor as an **OS cursor** set
     through the windowing layer, so it has no frame of latency. The image is built at the
     physical size of UB §9.3 and handed over as it is. winit cannot report whether the
     compositor rescales custom cursors, so this rule is **provisional**. Earlier drafts
     divided the size by the platform factor (1.333 on the reference machine); that is
     withdrawn, because 32 ÷ 1.333 is not an integer and the image would be resampled
     twice. If the owner sees a larger cursor than 32 physical pixels on the reference
     machine (owner observation 6), the setting `ui_cursor = os | software` switches to the
     software cursor, which has the exact physical size at one frame of latency.
   - **Software cursor.** Where custom cursors are not available, or by that setting, the
     cursor is drawn by the renderer at the pointer position, as the **last draw of the
     frame**, after the UI pass and the developer overlay (Decision 1.6). It is not an entry
     of the composition list. While the software cursor is in use (by the setting, or
     because custom cursors are not available), the client keeps the **OS cursor hidden**
     over the window (`set_cursor_visible(false)` through winit) for as long as that mode
     lasts, so only one cursor is seen. When the pointer lock of IB D7 "shows the cursor"
     at its end, in this mode that means the software cursor is drawn again; the OS cursor
     stays hidden. Switching the setting back to `os` shows the OS cursor again. Which of
     the two kinds the original uses is not stated; the visible result is the same apart
     from latency.
   - **Cursor during the pointer lock** (Tessera rule).

     > **OPEN:** UB §4.9 states that the UI pointer is frozen and the UI hit-tests nothing
     > while the pointer lock is on, but not what the visible cursor does. Tessera rule,
     > aligned with IB D7, until an owner observation (owner observation 5) settles it.

     The lock is client state (IB D7). The client routes a `mouselook` Start to the UI
     first, and turns the lock on **only if the UI did not consume it**. It turns the lock
     off after it routes the End of the **last** `mouselook` hold that turned it on; the End
     of a hold whose Start was consumed changes nothing. `mouselook` is `MouseRight`, and it
     also fires with the `MouseLeft+MouseRight` run chord (KM §8), so the lock is on while
     the avatar runs with both buttons. A **consumed** Start turns nothing on. During every
     item drag (while a drag icon exists) the `mouselook` Start is consumed: by the UI when
     the source's container created the icon and there is a keyboard target (UB §5.3 step
     4), and otherwise by the client (IB D7, Tessera rule; UB §7.4 OPEN). The cursor then
     stays **visible, ungrabbed and hit-tested**, and the drop at the left release reaches
     its target.

     **Grab, hiding and the end of the lock follow IB D7 ("Pointer lock" and "How the
     position is frozen", items 1 and 2) as written there.** In short: when it turns the
     lock on, the client hides the cursor and asks for `Locked`, falling back to
     `Confined`. While the lock is on, the client feeds the mapper **no cursor-position and
     no cursor-left events**, so the mapper's position stays at the frozen position *F*,
     the position of the `mouselook` Start (the press position). When it turns the lock
     off, it moves the cursor back to *F* where the platform allows (on Wayland while the
     grab is still `Locked`), then releases the grab and shows the cursor. **No cursor
     position is fed when the lock ends**: the next cursor-position event that the OS
     reports gives the real position, which is *F* when the move worked or the grab was
     `Locked`. If the last pointer report withheld during the lock was a cursor-left, the
     client feeds that cursor-left right after the call that turned the lock off (IB D7
     item 1). Camera turning reads raw
     mouse motion (KM §7.1), which both grab modes deliver. If IB D7 changes, this section
     follows it.

     This record adds only what concerns drawing:
     1. "Hidden" covers both cursor kinds: the OS cursor is made invisible, and the
        software cursor is **not drawn** while the lock is on. It is not drawn at *F*.
     2. When the lock ends, the software cursor is drawn again from the next frame, at the
        mapper's position. That position is *F*, because no cursor-position or cursor-left
        event reached the mapper during the lock. The `select_right` pulse of
        the same release is hit-tested at *F* for the same reason: every event of the call
        whose End turns the lock off carries *F* (IB D7 item 1). So the cursor reappears
        where the pulse is resolved. When a withheld cursor-left is fed after the lock
        ends, the pointer is outside the window: the software cursor is not drawn and
        nothing is hovered until a position inside the window arrives (UB §4.9 step 2). The
        `select_right` pulse of the turning-off call is still hit-tested at *F*, because the
        cursor-left is fed after that call.
     3. **Refused move** (recorded difference). A platform may refuse to move the cursor:
        IB D7 moves it only "where the platform allows". The UI pointer, the mapper's
        position and the software cursor then stay at *F*, because the mapper has seen no
        cursor-position or cursor-left event since the lock began. Only the **OS cursor** is shown where
        the platform left it, until the next real cursor-position event reports that
        position and moves the UI pointer there. With `ui_cursor = software` the cursor is
        drawn at *F* instead and jumps to the real position at that event.
     4. Focus loss ends `mouselook` (the mapper's focus-loss release, IB D7), so it also ends
        the lock by the same sequence.
6. **Screen size.** Layout uses the render target's physical size (UL §8.5). The OS scale
   factor is **not** applied automatically, because the original has no such factor. Users
   who want larger panels on a 4K display set `UIScale_Global` (UB §9.1), with the same
   results as in the original. On a screen resize, a root panel whose size does not change
   is only moved (its children are not laid out again, UL §8.4 step 6), which changes only
   its entry. Only panels whose size changes, and plugin windows that resize themselves,
   start paint jobs.

### 3. Text

- One path for all text: glyph coverage (A8) composited into the canvas with the exact rule
  of FO §8.4. The outline pass comes first, then the fill pass with underlines (FO
  §8.1–§8.5). Line layout follows FO §7, and truncation follows FO §7.6 and UB §10.
- The glyph atlases of the 0x42 fonts are decoded once on the asset workers into CPU-side A8
  buffers, which `tessera-ui` holds as `Arc`s. They are never uploaded to the GPU.
- Tessera's own fallback fonts (FO §11 items 2–3) are rasterised on the CPU by a pure-Rust
  rasteriser (ARCHITECTURE §15) into the same A8 coverage form. They then take the same
  compositing path, so plugin text in a Tessera font blends exactly like game text. They are
  rasterised at 1 : 1 like game fonts, although FO §11 item 4 would allow the scaled size.
  This keeps one path.
- Editable text (caret, selection highlight) is drawn as rectangle fills into the canvas.
  The blink damages only the caret's rectangle.

### 4. Images

1. **Sources.**
   - render surfaces 0x41 (`records.md` §5) named by layout media, properties and plugins;
   - user-skin files (SK §7): TGA at depths 32, 24, 16, 15 and 8; DDS (DXT1–DXT5 and the
     uncompressed formats of SK §7.4); JPEG; IFF is not decoded in M5 (SK D15);
   - plugin files (LPA §13.8): TGA and JPEG, through the plugin VFS.

   All decoding runs on the asset workers. Every decoder has a fuzz target (§7).
2. **One canonical pixel form, plus a source-format tag.** Every image becomes 32-bit ARGB
   pixels before the painter sees it, so the painter has one pixel format. Next to the
   pixels it keeps a **source-format tag**: *ARGB32*, *alpha-only*, *no alpha* or *other*.
   The tag records the **source format**, not what the pixels look like after conversion:
   UL §9.6.3 says that an overlay with an image draws only when the image is 32-bit ARGB or
   alpha-only, and draws nothing for any other format. *ARGB32* is given to the A8R8G8B8
   format alone (Tessera reading of "32-bit ARGB": whether the client converts other
   32-bit layouts to A8R8G8B8 at load is not stated, spec question 6). Every source:

   | Source | Canonical pixels | Tag |
   |---|---|---|
   | 0x41 render surface, format 21 A8R8G8B8 (`records.md` §5.2) | as stored | *ARGB32* |
   | 32-bit TGA (SK §7.2, depth 32 → A8R8G8B8) | as stored | *ARGB32* |
   | DDS A8R8G8B8 (SK §7.4) | as stored | *ARGB32* |
   | 0x41 format 28 A8; DDS A8; 8-bit TGA (SK D17) | black, the 8-bit value as alpha | *alpha-only* |
   | 0x41 format 20 R8G8B8 and 500 (embedded JPEG); DDS R8G8B8, X8R8G8B8, R5G6B5, X1R5G5B5, X4R4G4B4, X8B8G8R8 and the engine-specific B8G8R8 (SK §7.4); 24- and 15-bit TGA; skin and plugin JPEG | alpha 255 | *no alpha* |
   | DXT1–DXT5, from 0x41 surfaces (DXT1, DXT3, DXT5) or DDS (all five) | decompressed on the workers; DXT1's one-bit alpha as 0 or 255; DXT2 and DXT4 (premultiplied) decoded like DXT3 and DXT5 (Tessera rule, recorded approximation) | *other* |
   | DDS A8B8G8R8 and the engine-specific R8G8B8A8 (SK §7.4): 8-bit alpha in another byte order | reordered to 0xAARRGGBB | *other* |
   | DDS A4R4G4B4, A1R5G5B5, A2B10G10R10, A2R10G10B10; 16-bit TGA (SK §7.2, A1R5G5B5) | channels and alpha widened (below) | *other* |

   - *No alpha*: alpha 255 is the "read with *S*.A = 255" rule of UL §9.6.2, which is BIN
     for Over and Probable for the other operations.
   - Channels of 1, 2, 4, 5 and 6 bits are widened to 8 bits by bit replication, and 10-bit
     channels keep their top 8 bits. The original's routines for these sources are OPEN
     (UL §9.6.2); both conversions are Tessera rules.
   - *Alpha-only* follows SK D17, pending SK OPEN-4 and the UL §9.6.2 and §9.6.3 OPENs on
     alpha-only routines.

   **Use of the tag.** For an **overlay with an image** (UL §9.6.3), the painter draws only
   when the tag is *ARGB32* or *alpha-only*. For *no alpha* and *other* it draws nothing,
   as UL §9.6.3 says the client does for any other format, so an overlay image from a DXTn
   surface, an A4R4G4B4 file or an A8B8G8R8 file draws nothing. For *alpha-only* it uses
   the 8-bit value as *I*.A, which is a Tessera rule until the client's alpha-only routines
   are read (UL §9.6.3 OPEN). The base layer uses the canonical pixels for every tag. A
   failed load gets the placeholder of SK §6.5 (SK D16). In practice overlay images come
   only from 0x41 surfaces named by data: all 16 in this install are 32-bit ARGB (UL
   §9.6.3), skins never substitute overlays (SK §6.3), and plugins have no overlay image
   (LPA §13.17: `SetBackColor` is a fill without an image).
3. **Plugin image formats.** `.png` and `.bmp` files (shipped by 6 and 4 distributions; one
   distribution references a `.png` path, PU §6.8) are **not** decoded. `SetBackground` with
   such a path raises the unsupported-format error of LPA §13.8. Whether the original
   accepts `.png` is PU O6. Tessera follows the formats LPA §13.8 names and changes this
   only if O6 is answered yes.
4. **`SetBackground(path)` fails synchronously.** LPA §13.8 raises the error at call time
   when the file cannot be opened or has an unsupported format. Tessera checks this on the
   main thread without decoding:
   1. **Existence** is checked against the plugin VFS index of the **whole `Plugins/`
      tree**, not of the calling plugin's own folder. Every plugin image path is relative to
      `Plugins/` and matched case-insensitively component by component (LPA §4.4), and
      distributions use files of other distributions (PU §6.11). The index applies the
      containment and component rules of LPA §4.4, and a lookup needs no I/O. It is built
      by a directory walk **on an asset worker**, never on the main thread: once when the
      plugin host starts (the host loads no plugin before the first index is ready; the
      main thread does not wait for it), and again when the plugin manager refreshes its
      list (ARCHITECTURE §18.2). A rebuilt index is swapped in whole on the main thread
      when it is ready; until then lookups use the previous one, so a file added after the
      last walk is found only after the swap. The walk therefore needs no exception to
      ARCHITECTURE §21.
   2. **Format** is checked by reading the file's header once: at most 64 bytes, enough for
      the TGA header (SK §7.2) and the JPEG start marker. The result is cached per path.

   The header read is a bounded synchronous read of the plugin tree, of the same kind that
   `import` already needs, because Lua's semantics are synchronous (ARCHITECTURE §18.2). It
   is recorded as a stated exception to ARCHITECTURE §21's no-blocking-I/O rule, limited to
   this header read in plugin calls that must fail at call time. The index walk of step 1
   is not part of the exception.

   The full decode runs on the workers. If it fails after the header check passed (a
   corrupt JPEG body), the failure is reported to the plugin error listeners, and the layer
   draws nothing. This is a stated deviation: the original would have raised the error at
   the call.
5. **Cache and pinning.**
   - **Pinned images.** Every image referenced by an element that has a canvas is pinned.
     A pinned image is never evicted, so a repaint never lacks pixels and the
     partition-invariance property holds.
   - **LRU.** Unpinned images stay in an LRU cache, keyed by DID or by path, within the
     decoded-image budget (U5).
   - **Hard cap.** When the pinned images alone exceed the hard cap of U5, further images
     get the placeholder and a categorised failure.
6. **Very large images.** Skin images may be up to 16,384 × 16,384 (SK D4), which is 1 GiB
   decoded. An image with more than 4,096 pixels on an axis is **cropped at decode** to its
   top-left 4,096 pixels on that axis. Layers draw from the top-left at native size (UL §9.6,
   SK §6.10), so the crop changes nothing for an element up to 4,096 pixels on that axis.
   Larger elements would tile with the wrong period, which is a recorded difference. It
   also changes what SK §6.10 and §6.11 give for skin images between 4,097 and 16,384
   pixels on an axis, which are shown whole there (owner question 5, spec question 6). The
   first review suggested the placeholder for images over the budget; the crop is chosen
   because it keeps every normal case exact (see the response table).

### 5. Plugin controls share the path

- A plugin `Window` is a root panel with a surface (LPA §13.19). Its controls are elements of
  its tree, painted by the same painter.
- `SetBackground`, `SetBlendMode`, `SetBackColor` and `SetBackColorBlendMode` map onto the
  two layers exactly as LPA §13.17 (BIN) gives them:
  - the order is base image, then back-colour overlay, then text, then children;
  - a new control's operation is Replace, so it writes alpha into the canvas;
  - `SetBackColorBlendMode` re-creates the overlay;
  - the colour packing is exact.

  An earlier draft of this record drew the colour behind the image and proposed a new LPA
  OPEN for the order. Both are withdrawn: LPA §13.17 settles the order.
- **Where a control is drawn.** It is drawn into the canvas of the nearest surface at or
  above it (LPA §13.17), split by Decision 1.7 where that surface has nested entries.
  Replace and Replace fills write their alpha there. A back colour with alpha 0.5 therefore
  makes that part of the window half transparent over the game scene. The composition blend
  of Decision 1.6 and §8 decides how that looks. That blend is OPEN (UL §9.6.1).
- **Opacity.** `SetOpacity` is `ObjectOpacity` (LPA §13.19). On a window it fades the
  window's surface. On another control it has a visible effect only while the control has
  its own surface, that is, a stretch mode other than 0. Earlier drafts said opacity on
  other controls stays stored only; that is corrected.
- **Rotation and scale:** see §2.1 and §2.3. Children with their own surfaces turn with
  their window.
- **Many windows and controls.** Many small plugin surfaces mean many small textures and one
  quad each. The caps of §7 bound their number. Packing small surfaces into a shared atlas
  is an optimisation for later, only if the UI pass misses its budget.

### 6. Performance budget (reference machine, rules of `docs/perf/reference-machine.md`)

These are proposed M5 performance criteria. They are measured with a scripted walker run
that:
- opens the HUD set (P1, P2, P3, P4, P11, P14), two windows (P7, P9) and three plugin windows:
  one with a child control in stretch mode 1, one rotated with `SetRotation` (*z* = 90) that
  holds such a child, and one screen-sized (`Display.GetSize`) with a full-size background;
- **walks** for 20 s, including turns by key and by mouse-look (`MouseRight` held and
  dragged) and a stretch with the `MouseLeft+MouseRight` run chord, so the minimap scrolls on
  every frame (GP §8.6.2) and its marker turns (GP §8.6.3);
- sweeps the pointer across the toolbar and over a plugin window that has `MouseMove`
  handlers, once at the system's normal event rate and once with a **1000 Hz** pointer
  sweep (injected raw `CursorMoved` events at 1 ms intervals; IB D7 coalesces them, IB D8
  asks for this);
- scrolls the wheel over the P7 options tree (one line step per notch, UW §5.6–§5.7);
- adds 10 chat lines per second;
- opens and closes every UL §17 panel;
- **drags the resize bar** of P10 (1038 × 802) continuously for 3 s, and then that of P15
  (UB §8.2);
- resizes the plugin window that holds the stretch-mode child with `SetSize` on every frame
  for 3 s, and shows a second stretch-mode child in it during that resize (U4 (c));
- changes `UIScale_Global` from 1.0 to 1.5 and back;
- resizes the window, and shows a stretch-mode child in the screen-sized plugin window while
  that window's resize generation runs (U4 (c)).

The runs are made at 3840 × 2160 fullscreen and at the window size that the compositor
gives.
- **Content-event frames** are the frames in which a scroll offset changes (wheel notches,
  scroll-bar steps, a list or tree scrolled by code) or a new chat line arrives. Each of
  these repaints a whole scrollable box (Decision 1.4).
- *Steady state* means every frame outside the event windows of U4 and outside the
  content-event frames. So it **includes** walking, turning, the minimap scrolling and the
  pointer sweeps.

| # | Measure | Budget |
|---|---|---|
| U1 | UI main-thread time per frame in steady state: Tessera's own work inside the step-1 offers (hit tests, routing, native element handling; IB D7, D8), plus Tessera's own work in the step-7 tick (hover refresh, hover check, the rest of the UI's per-frame work; IB D8), plus step 8 (layout, paint, damage merge, upload encoding). Plugin Lua time inside the offers, in the step-7 `Update` broadcast, in the plugin host's deferred work and in the collector step is **excluded**; it is measured and reported separately as plugin time (IB D8) and counts only toward the frame rule of U4 | mean ≤ 0.3 ms, p99 ≤ 1.0 ms, in each of the walking phase and the 1000 Hz sweep |
| U2 | GPU time of the UI pass at 3840 × 2160 (timestamp queries where the adapter has them; otherwise not a gate) | ≤ 0.3 ms |
| U3 | UI upload bytes per frame | steady-state mean ≤ 256 KiB, including the walking phase (content-event frames are bounded by the cap and by U4 (f)); hard cap 8 MiB per frame. The rest is deferred, and an upload larger than the cap is spread over frames into a back texture (Decision 1.5) |
| U4 | Event frames (panel open, screen resize, scale change, resize-bar drag) | (a) no frame over 50 ms (the M3 criterion), plugin time included; (b) UI paint ≤ 4 ms per frame on the main thread (the *slice*), plugin handler time excluded (IB D8); (c) a newly shown surface is drawn within 100 ms of being shown: complete if the condition of Decision 1.8 holds by then (its first job complete and its requested images arrived), otherwise with its completed bands (Decision 1.5). This holds for every root (a panel, a plugin window) and for a nested surface shown while its group has no generation running. A nested surface shown while its group's generation runs is drawn at the swap of the next generation, which (d) and (e) bound (Decision 1.5): within 8 frames when the group's surfaces are up to 1038 × 802 (for example a child shown in P10 or P15 during a resize-bar drag, or in a plugin window resized on every frame), and within 41 frames in the screen-sized plugin window after a screen resize. The scripted run shows a stretch-mode child in the two plugin-window cases and records the delay; (d) every paint job of a surface up to 1038 × 802 completes and is uploaded within 4 frames, so during the P10 and P15 resize-bar drags the panel's drawn size changes at least every 4 frames; (e) the first job of the screen-sized plugin window at 3840 × 2160 completes and is uploaded within 40 frames; (f) **content-event frames**: UI main-thread time ≤ the 4 ms slice of (b), and a scroll step of the P7 options tree or a new line of P2 at its design size is painted and uploaded in the frame of the event. Several notches in one frame merge into one repaint (Decision 1.4) |
| U5 | Memory | CPU canvases, including canvases above (sparse tiles) ≤ 128 MiB; GPU surface textures (front and back, sized to their tile boxes) ≤ 256 MiB; decoded images: unpinned LRU ≤ 64 MiB, pinned hard cap 512 MiB. All are Tessera settings |

Where the numbers of U4 come from (indicative probe, Context): a full repaint of P10 is
0.83 Mpx × an overdraw of 3–4 × 1–4 ns ≈ 2.5–13 ms, so 1–4 slices; its upload of 3.3 MB fits
one frame's cap. The screen-sized window's first job is 8.3 Mpx × 1–4 layers × 0.5–4 ns ≈
4–130 ms, so up to 33 slices, and 33 MB (31.6 MiB) of upload is 4 frames at the cap, partly overlapped
with painting (Decision 1.5). If (d) or (e) fails, the bands move to the workers (below).

**Why scrolling and chat are not steady state** (indicative probe, Context). Canvases have
the unscaled layout size (§2.1), so these costs do not grow with the UI scale.
- **Scroll step.** One wheel notch over the P7 options tree repaints the tree's box. The
  whole P7 panel is 577 × 534 ≈ 0.31 Mpx (UL §17.2). A tree box of about 0.15 Mpx (an
  estimate; the specs do not give the tree's box) with an overdraw of 3 at 1–4 ns per pixel
  costs about 0.45–1.8 ms before text. Repainting all of P7 would cost 0.9–3.7 ms. The
  upload is about 0.6 MB.
- **Chat line.** A new line scrolls the log, which repaints the log's box. At P2's design
  size, 313 × 199 ≈ 62,000 pixels (UL §17.2), that is about 0.19–0.75 ms. A chat window
  enlarged to its maximum of 1024 × 1024 (`chat-window.md` §3.1) is about 1 Mpx, or 3–13 ms.
  That is a multi-frame content job (Decision 1.5), which shows the previous content until
  it swaps.
- **Conclusion.** "Repainting only small damaged rectangles costs tens of microseconds" does
  not hold for these boxes. A fast wheel sweep repaints the box on every frame (the notches
  of one frame merge into one repaint), and at 10 chat lines per second about one frame in 6
  (at 60 frames per second) to one in 14 (at 144) has a chat repaint. Under U1's p99 of
  1.0 ms both would fail by design. So they are content events: their frames are taken out of U1
  and U3's mean, and bounded by U4 (b) and (f), whose 4 ms slice covers the tree and the
  design-size chat log with room for text. They are still counted and reported
  (`FrameStats`), so a regression stays visible. A later optimisation that scrolls by
  copying canvas rows is possible only where the background does not scroll with the
  content. It is not part of M5.

**The minimap in steady state** (from GP §8.6.2 and the probe numbers). While the player
walks, the map element moves, which damages the 170 × 170 viewport (Decision 1.4), about
29,000 pixels. Per layer, with the indicative probe figures of the Context:

| Layer over the viewport | Operation | ns per pixel | Cost for 29,000 px |
|---|---|---|---|
| erase of the damaged rectangle | fill with transparent black | ≈ 0.4–0.5 (taken as Replace) | 12–15 µs |
| the disc (GP §8.7.1) | Replace | 0.4–0.5 | 12–15 µs |
| the map tiles | Mix colour: **no probe figure**; taken as Over | 1–4 | 29–116 µs |
| rim parts inside the viewport (GP §8.7.2), at most the whole area | Over | 1–4 | ≤ 29–116 µs |
| **Total, before the coordinate label** | | | **≈ 0.08–0.26 ms** |

The upload is 29,000 × 4 bytes ≈ 113 KiB per frame with damage. The coordinate label
repaints when its text changes. The map's position is in whole pixels (GP §8.6.2: one map
pixel is one screen pixel), and a setter that does not change the value adds no damage
(Decision 1.4), so this cost falls only in frames where the player's map pixel changes. The
specs do not give the map's metres per pixel, so that fraction is not computed here; at a
run it can be every frame. **U1's mean of 0.3 ms is therefore at risk in the walking
phase**: the minimap alone can take most of it. The scripted run measures it. If the walking
phase misses the mean, the answer is to paint the minimap's band on a worker (below, "Workers
if needed"); relaxing U1 for the walking phase is the owner's alternative (owner question 2).

**Probe gap before U1 becomes a criterion.** Two rows of the table have no measured figure:
Mix colour, the map tiles' operation, is taken as Over, and the erase is taken as Replace.
Before the walking-phase mean of U1 is accepted as an M5 exit criterion, the probe is
repeated as a micro-benchmark of the `tessera-ui` painter, under rule 1 of
`docs/perf/reference-machine.md` (a Tessera release build, conditions recorded), with Mix
colour (UL §9.6.2) and the erase (a fill with transparent black, including the freeing of
whole tiles, Decision 1.2) added. The estimate above is then recomputed, and owner question
2 is answered with that figure.
The marker costs nothing on the CPU while the player turns. It is a nested entry, and its
coverage region is the same 40 × 40 square at every heading (Decision 1.7, §2.3), so a turn
causes no damage in P1. U3's mean of 256 KiB holds with the 113 KiB.

The UI upload budget is **separate** from the asset upload budget of ARCHITECTURE §20.3
step 5, which is 24 MiB and 4 ms per frame (`walker.md` §2). `FrameStats` reports the two
separately. The 50 ms rule of U4 covers the whole frame, so both budgets together must stay
inside it.

How the budget is kept:
- **Spreading large paints.** A paint is a pure function of an immutable paint snapshot (the
  display list plus `Arc`ed images). A large job (first paint, show after a release, resize,
  scale change) is split into horizontal bands. In M5 the bands run on the main thread, up
  to the U4 slice per frame, under the job and transition rules of Decision 1.5.
- **Workers if needed.** If U4 (d) or (e) fails, the bands can go to the asset workers
  (ARCHITECTURE §21 sizes the pool as N = cores − 2, which is 14 on the reference machine
  by `docs/perf/reference-machine.md`). `tessera-ui`'s painter interface stays the same,
  because it is already a pure function of a snapshot. The client changes, though:
  - the snapshot and the band's target buffer must be sent to a worker, and the painted band
    returned and copied into the canvas;
  - the worker queue is shared with streaming and animation (ARCHITECTURE §8.3), so UI bands
    need a priority class of their own.

  Earlier drafts said this needs no interface change at all; that is corrected.
- **Input.** Pointer moves are coalesced by the mapper (IB D7), so a 1000 Hz mouse costs at
  most one hit test per other event plus one per frame inside U1.

### 7. Hostile input

Layout rectangles, plugin sizes, rotation angles, image sizes and the number of plugin
controls come from data or from plugins, so everything that allocates is bounded:
- **Surface size:** each axis is at most the device's `max_texture_dimension_2d` and at most
  16,384. When a surface would exceed either limit on an axis, its canvas covers only the
  part of the element's box that lies on the screen on that axis. It carries a **canvas
  origin offset**, which is the position of that part inside the element. When the element
  moves, the offset changes and the newly exposed part is damaged and painted. A surface
  **within** the limits always keeps its full canvas with offset 0, also when it lies partly
  off the screen or at negative coordinates: it is not cropped, so moving it back on screen
  needs no paint. Its sparse tiles (Decision 1.2) are allocated where it is painted, on or
  off the screen. Such a surface is reported once as a categorised
  failure, like LPA §13.2's size warning, which the original also gives without clamping.
- **Number of surfaces and entries** (Tessera settings; U5 bounds only bytes, and thousands
  of tiny `SetStretchMode(1)` controls fit inside it):
  - at most **512 surfaces per plugin apartment** (windows and controls together) and
    **4,096 surfaces in total** (native and plugin, including surfaces kept under hidden
    ancestors and with `KeepObjectWhenHidden`);
  - at most **8,192 composition entries** per frame, counting nested entries, canvases above
    and external content (the software cursor is not an entry); a canvas above exists only
    for a nested entry, so it is bounded by that count;
  - at most **8 levels** of nested surfaces below a root.

  A non-root element whose surface would exceed a cap, or that lies deeper than the depth
  cap, gets no surface: it is painted into its parent surface's canvas as if its mode were 0
  (unscaled, without its own opacity or rotation). A window over a cap draws nothing. Each
  case gives one warning per plugin to the error listeners (or one `tracing` warning for
  native panels) and a `FrameStats` counter, so it is not silent.
- **Memory:** a surface whose texture or canvas would push past U5 is handled the same way:
  a non-root element falls back to its parent's canvas, a root draws nothing, with the same
  warning and counter. A back texture that does not fit U5 is not created; the surface keeps
  its front texture and its job waits, so no shown surface disappears for lack of a back
  texture.
- **Images:** the limits of SK D4 and D18, the crop of §4.6 and the hard cap of §4.5.
- **Rotation angles:** values that are not finite are treated as 0, so the unrotated path is
  used.

**Fuzz targets** (CLAUDE.md: every parser has one; names for the implementer to fix):
- the painter: arbitrary display lists, images with every source-format tag, damage
  rectangles, nested surfaces and canvas offsets; it checks that the painter never panics,
  never writes outside the canvas, and that partitioned repaints equal one full repaint;
- the composition-list builder: arbitrary trees of nested surfaces, rotations, visibility,
  Tile parents and caps; it checks the caps, that no entry is emitted for a surface under a
  hidden ancestor or below a Tile surface, and that no clip chain is longer than 8;
- the image decoders: TGA (every depth of SK §7.2), DDS (DXT1–DXT5 and the uncompressed
  formats of SK §7.4) and JPEG, each with the size limits and the crop of §4.6;
- the plugin image header probe of §4.4.

### 8. Additions to `render.md` (a new section "UI pass (M5)", written by the render implementer)

The section goes after the M4.5 sections. `render.md` now has §4 and §5 from M4.5, so the
render implementer picks the number.

| Item | Addition |
|---|---|
| UI textures | An upload path for **`Bgra8Unorm`, linear** textures (no sRGB decode), sampled without a hardware filter (the shader filters, below). Usages: `TEXTURE_BINDING` (read with `textureLoad`), `COPY_DST` (sub-rectangle uploads, and the destination of GPU copies), `COPY_SRC` (the source of the front-to-back copy and of the copy when a tile box grows or shrinks, Decision 1.2, 1.5) and `RENDER_ATTACHMENT` (the scissored clear of freed tiles). `TextureUpload` gains a pixel-format field, or a separate `UiSurface` type is added. A texture covers the tile box of its surface; the entry gives its offset in the canvas. |
| Sub-rectangle updates | `Renderer::update_texture_rect(id, x, y, w, h, bytes, row_stride)` for single-level textures, bounds-checked and returning `Result`. A GPU copy of one texture into another at an offset, for back textures and for growing or shrinking a tile box (Decision 1.2, 1.5). A scissored clear of a rectangle to transparent black. `FrameStats` gains `ui_upload_bytes`, `ui_surfaces`, `ui_quads` and the cap counters of §7, reported apart from the asset upload counters. |
| Blending on stored values | The UI pass renders into a **non-sRGB view** of the sRGB swap-chain and offscreen targets (`view_formats` with the `…Unorm` variant). It samples the surfaces unconverted, so blending works on stored bytes like the original. This is the fix that the second OPEN of `render.md` §4.8 suggests for the lit passes (render into a `Unorm` target and present unconverted), adopted here **for the UI pass only**. It differs from the Decision of `render.md` §4.1, under which the lit passes blend decoded values on sRGB targets; that Decision and its OPEN stay as they are. The swap chain is configured with the `…Unorm` variant in `view_formats` for this. Fallback where the backend refuses the view: the shader linearises its output, opaque pixels stay exact, and blended pixels become a recorded known difference. |
| UI pipeline | The shader reads texels with `textureLoad`; a texel outside the texture's box reads as transparent black. At 1 : 1 it reads one texel. When scaled or rotated, it reads four texels, premultiplies each by its alpha and weights them bilinearly (`linear`), or reads the nearest texel (`nearest`). For an entry with the external-content source flag the texels are already premultiplied and are not multiplied again. It multiplies the premultiplied result by the entry's opacity, applies the entry's alpha threshold (discard below it), and, for a Tile entry, applies repeat addressing: canvas position = ((position − box origin) ÷ display-node scale) modulo the canvas size, in the root's unrotated space, with the filter's neighbour texels wrapped too and the tile-box lookup applied after the wrap (Decision 1.6). A Tile entry inside a scaled panel is filtered like any scaled entry; unscaled it reads one texel. Blend: colour `One`/`OneMinusSrcAlpha`, alpha `One`/`OneMinusSrcAlpha` (premultiplied source-over). At 1 : 1 with opacity 1 this equals straight source-over within ±1, and replaces the frame's colour exactly where the surface alpha is 255. The values are named constants, so a frame capture that settles UL §9.6.1 changes one place. |
| Placement and transform | Destinations in integer physical pixels (the displayed boxes of UB §9.2.1, under the transition rule of Decision 1.5), with the texel-centre mapping that makes 1 : 1 copies exact. Each entry carries a transform chain: the rotation held by its own surface about its own centre (the minimap marker, a rotated child window or control), then the rotation held by each surfaced ancestor's surface about that ancestor's centre (three angles, order *x*, *y*, *z*, orthographic; LPA §13.20 and its OPEN). The angles are the surfaces' state (§2.3), not `GetRotation()` values. Entries derived from a root share the root's transform. |
| Clip | Per entry, a **clip chain** of at most 8 rectangles (Decision 1.6): the displayed boxes of its enclosing windows, from the nearest one up to the root, each with that window's transform. For each, the fragment's position is taken back into that window's own unrotated space and discarded outside the rectangle. The builder intersects consecutive rectangles that share one space, so an unrotated chain is one rectangle, which a screen scissor may replace. Root entries have no clip; a canvas above has its surface's chain. |
| Order | Composition entries are drawn in the given order, with no depth and no sorting. The screen-space passes after the world are, in order: the walker's overlays (HUD text and walker minimap, `walker.md` §6; with a game UI attached the walker minimap is off by default), the UI pass, the developer overlay (egui frame stats, ARCHITECTURE §13 "Diagnostics", planned), and the software cursor (Decision 1.6). |
| External content | Offscreen targets registered with `register_target_texture` can be composition entries (the portrait viewports). They are `Rgba8Unorm` (`OffscreenFormat::Rgba8Unorm`) at the element's layout-box size, cleared to transparent black each frame they are drawn, and hold **premultiplied** colour: the portrait's passes write alpha 1 for opaque draws and blend alpha with `One`/`OneMinusSrcAlpha` (Decision 1.7). The entry's source flag makes the UI shader skip its premultiply step. An sRGB target is refused as UI content (it would return decoded values into the non-sRGB UI view and darken the portrait). |
| Cursor | The software cursor of §2.5 is one textured quad drawn last in the frame, after the developer overlay, and not drawn while the pointer lock is on. Cursor visibility and grabbing are client work through the windowing layer, not renderer work: while the software cursor is in use, the client keeps the OS cursor hidden (§2.5), so only one cursor is seen. |
| Tests | Golden test at 1 : 1 and opacity 1 over an opaque background: screen pixel = canvas pixel where the canvas alpha is 255, unchanged where it is 0, ±1 otherwise. Sub-rectangle update test. Back-texture swap test (no frame shows a partly uploaded surface, and the entry's box changes in the frame of the swap). Tile-box test (texels outside the box read as transparent black; growing and shrinking keep the content). Scaled-quad test for both filters, including a transparent-black border that must not darken the edge. Alpha-threshold test. **Tile test**: a 32 × 32 canvas on a 200 × 100 box repeats 1 : 1 from the box's top-left (screen pixel (x, y) = canvas pixel (x mod 32, y mod 32)), and inside a panel scaled by 1.5 each repeat covers 48 × 48 pixels with no seam darkening. **Group transition test**: during a root's resize generation, a nested entry and its canvas above keep their old place relative to the old root box, and all three change in the frame of the swap. **Generation hold test**: a root job that moves content between the canvas below and the canvas above for a moved child completes while the child's first paint is withheld; no frame shows the root's new split before the child's new place, and the front texture is unchanged until the swap. Clip test for a nested entry outside its root, unrotated and with the root rotated by *z* = 30 (the nested entry turns with the root and is cut along the rotated edge). **Clip-chain test**: a stretch-mode control inside a child window extends past the child window and is cut at the child window's edge, not at the outer window's; with the child window itself extending past its parent window, both the child window and its control are cut at the parent's edge; and with the child window rotated by *z* = 30 the control is cut along the child window's rotated edge. **External content test**: a premultiplied `Rgba8Unorm` target with a half-covered edge texel composes without a dark fringe, and its stored bytes reach the frame unconverted at 1 : 1. **Round-minimap test** (GP §8.7.1): a synthetic surface with GP4's disc alpha composed over an opaque background shows the background unchanged at the four corners and the surface at the centre. Test that the non-sRGB view leaves stored bytes unconverted. |
| Open items | The composition blend state and the premultiplication (UL §9.6.1). The scale filter and mips (FO Q3). The rotation direction, order and projection (LPA §13.20), and the marker resampling (GP §8.6.3). The nested-surface composition (LPA §13.18 OPEN (1), §13.19 OPEN), including the clip chain beyond the nearest window. The external-content format and alpha convention (UW §12 OPEN). |

**Checks this record's composition path must pass for P1** (from GP; added to the
`tessera-ui` and client tests, and to the M5 checklist run):
1. The composition uses the surface's alpha (GP §8.7.1): the minimap is round, not square
   (the round-minimap test above, and P1 on screen).
2. The rim `Minimap_Frame` is painted without the viewport's clip, clipped only by the radar
   root (GP §8.7.2 Tessera rule): a painter test on a synthetic P1-shaped tree finds rim
   pixels outside the 170 × 170 viewport box in the root canvas.
3. The marker is a nested entry with a `z` rotation and points as GP §8.6.3 says: up for
   *h* = 0 (north) and right for *h* = 90, on screen.
4. While walking, the minimap's damage is the viewport box only (Decision 1.4), and while
   turning there is no P1 damage at all: the marker's coverage region is the same 40 × 40
   square at every heading (Decision 1.7). A `tessera-ui` test turns the marker through
   360° in 1° steps and finds no damage after the first rotation, and the canvas above
   holds the same rim pixels at every angle.

## Consequences

- **Crate boundaries.**
  - `tessera-ui` (L4) holds the sparse canvases, the compositor operations, the text painter,
    damage tracking, paint jobs, the nested-surface split, the caps and the composition
    list. It has no `wgpu` dependency. Its tests are exact byte comparisons against the
    specs' worked examples, plus the partition-invariance property.
  - `tessera-render` gains only the generic items of §8.
  - The client app moves rectangles from one to the other and owns the upload budget, the
    back textures, the tile-box textures, the cursor and the pointer grab (§2.5).
  - Image and font-atlas decoding stays on the asset workers (formats or material). The
    plugin VFS index of the whole `Plugins/` tree and the header probe of §4.4 belong to
    `tessera-plugins`.
  - **`plugins` → `ui`.** Plugin controls are `tessera-ui` elements (LPA §13.17), and the
    `Turbine.UI` bindings in `tessera-plugins` create and drive them (ROADMAP M5 gives
    `tessera-plugins` "UI controls and windows"; §4.4 runs the header probe inside
    `SetBackground`). ARCHITECTURE §3 has no `plugins` → `ui` sibling allowance today, and
    IB adds none (its `input` and `ui` stay independent). This record therefore proposes
    the allowance `plugins` → `ui` (L4), one-way: `ui` never depends on `plugins` or
    `script`. Its reason, for ARCHITECTURE §3: the plugin API's UI classes are thin
    bindings over `tessera-ui` elements, and the painter and composition stay in `ui`. The
    alternative, if the owner or the M5 plan prefers it, is that `tessera-ui` defines a
    trait for the element operations the bindings need and the client wires the two
    crates together, as IB does for `input` and `ui`. This record works with either; it
    asks the accepting session to choose one (owner question 1).
- **Supersedes plans, not accepted records.** These edits are for the session that accepts
  this record. This proposal makes none of them:
  - ARCHITECTURE §15 "Rendering" (a 2D draw list of quads, nine-slice frames and text runs
    drawn by `render`) is reworded: art is tiled at native size into CPU surfaces, and
    `render` composes surfaces.
  - ARCHITECTURE §15 also gets the image rules of §4.
  - ARCHITECTURE §3 gains the sibling allowance `plugins` → `ui` with the reason above, or
    records that the client wires the two (Crate boundaries).
  - ARCHITECTURE §13's frame structure gains the screen-space passes after the world: "…
    transparent → walker overlays when shown (HUD and walker minimap, `walker.md` §6; under
    the game UI) → UI → developer overlay (egui frame stats, planned) → software cursor →
    present" (Decision 1.6, §8 "Order").
  - `walker.md` §6 gets the rule for the walker's overlays with a game UI attached: drawn
    under the UI, the walker minimap off by default (`walker_minimap`), the HUD switchable
    (`walker_hud`) and off in checklist runs (Decision 1.6).
  - ARCHITECTURE §20.3 takes IB D8's wording for steps 1, 7 and 8 (IB, owners list). Step 1:
    UI input routing and plugin input handlers run there, against the previous frame's
    layout. Step 7: the UI tick of LPA §3.3 (hover refresh, `MouseHover`, the plugins'
    `Update` broadcast, the rest of the UI's per-frame work), then the plugin host's
    deferred work (sliced by whole items, 2 ms per frame), then the collector step in every
    apartment (`plugin-runtime.md` §9.2), then the band changes. Step 8: "UI: layout, paint damaged rectangles
    within the UI slice, build the composition list". This record and IB give the same
    text; step 8's paint and composition-list wording is this record's, which IB adopted.
    Step 10 draws the UI pass and the passes around it.
  - ARCHITECTURE §21 records the plugin header-probe exception of §4.4.
  - The ROADMAP M5 deliverable "nine-slice art" is reworded.
  - ROADMAP M5 gets an exit criterion for the U1–U5 budgets of §6, measured under the rules
    of `docs/perf/reference-machine.md`, with the scripted run of §6.
  - `docs/decisions/README.md` gets a row.
  - CLAUDE.md gets a layout line for `crates/ui`.
- **Agreement with IB.** This record and IB are written to be accepted together: input
  routing and plugin input handlers run in step 1, the UI tick and the plugin host's
  deferred work in step 7, layout and paint in step 8 (IB D8); U1 counts Tessera's own work
  in steps 1, 7 and 8 and excludes plugin Lua time; the scripted run has the 1000 Hz sweep;
  and the cursor during the lock follows IB D7 as revised (on only after the UI declined
  the Start and no item drag is in progress, no cursor-position and no cursor-left events
  during the lock, no cursor position fed at its end, and a withheld cursor-left fed right
  after the turning-off call), which §2.5 cites. "Coordination with IB" below says what is
  left.
- **Questions for the spec writers.** This record raises them; it edits no spec:
  1. LPA §13.19 OPEN: its Tessera rule (window opacity applies to nested surfaces) conflicts
     with the reading of UB §11.1 step 2 that Decision 1.7 uses. This record proposes "own
     opacity only". The same conflict exists with UL §5.9 (`ObjectOpacity` "fades the
     element with all its children") and UL §6.6 (a Fade item fades the element "together
     with its children"): under "own only", Fade items and `SetOpacity` leave nested
     surfaces unfaded (Decision 1.7). The spec writers should state which children a Fade
     item and `ObjectOpacity` reach when they have surfaces of their own, or mark it OPEN in
     UL §5.9 and §6.6.
  2. LPA §13.21 rule 1 ("children follow their own edge modes" when a window is scaled)
     against UB §9.2 step 6 and §9.2.1 (scaler types resize in Stretch; children not laid out
     again).
  3. UL §8.4 step 1 says object modes are "consulted only on root panels and elements flagged
     by code". LPA §13.19 and its §20 correction to UL §5.9 say that any non-zero object mode
     from a layout gives an element a surface. UL §6.6 and §16 item 11 should cite LPA §13.19,
     UW §4.5 and GP §8.2 step 3 as partial answers.
  4. UB §4.7's Tessera rule for one line step (an entry's height or 16 pixels) is superseded
     for lists and trees by UW §5.6 (BIN); UW §16 records it, and UB should point there.
  5. `UICore_Element_ShouldEraseBackground` (UL §5.9): trace its effect (Decision 1.3 OPEN).
  6. SK §6.10, §6.11 and D4: add the decode-time crop of §4.6 as a Tessera rule for skin
     images, as a recorded difference from "shown" for images between 4,097 and 16,384
     pixels on an axis.
  7. UL §17.2: add, per M5 panel, the number of non-root elements with a non-zero object
     mode (by mode). This bounds where surface contents are not byte-exact (Decision 1.7).
     The alpha-test meters need no census: UW §4.5 already says that none of the 4 is in
     the UL §17 panels.
  8. GP §8.6.3: record the Tessera rule of §2.3 (marker as a nested entry with a `z`
     rotation, premultiplied bilinear resampling) next to its OPEN.
  9. UB §4.9: add an OPEN for the visible cursor during the pointer lock, with the Tessera
     rule of §2.5.
  10. UL §8.4 OPEN: record the Tessera rule for native Tile with power-of-two sizes (§2.1:
      repeated at the display-node scale, not fitted to the box) and for native root panels
      in mode 0 (Decision 1.1: a stand-in surface at opacity 1).
  11. LPA §13.18 OPEN (2): record the same rule for plugin mode 2 (canvas = the base image's
      size, repeated over the control's displayed box at the display-node scale), and the
      Tile-parent rule of Decision 1.7 (no nested entries below a Tile surface).
  12. LPA §13.18 OPEN (1): its Tessera rule ("clipped to its window only") does not say
      what clips the content of a child window that extends past its own parent window.
      This record clips every nested entry by its nearest enclosing window **and** by each
      enclosing window above it (Decision 1.7, "Clip"). The spec writer should record that
      extension or a different rule.
  13. LPA §13.17 and §13.19: for a child `Window` after `SetStretchMode(0)`, §13.17 ("Where
      it is drawn": the nearest surface above) and §13.19's Tessera rule ("Tessera draws
      every shown window through a surface") point to different places. This record draws
      such a child into the nearest surface above it and keeps the stand-in surface for
      windows with no surface above (Decision 1.1). The spec writer should align §13.19's
      Tessera rule, and state that the stand-in surface takes no opacity (§13.19: "its
      opacity has no effect").
  14. UL §8.4 and UB §9.2.1: UB §9.2.1 says that the element resize of a surface-owning
      element in mode None leaves the layout box unchanged; LPA §13.19 condition 1 says a
      mode-0 element never owns a surface, so that branch cannot be reached, and this record
      applies the no-surface rule (UB §9.2.1 step 3) instead (§2.1). The spec writers should
      state which holds, or mark the None branch unreachable.
  15. LPA §13.18: the refresh of an existing surface on a mode change ("refreshed for the
      new mode") was not traced beyond that word. This record keeps the surface and treats
      a canvas-size change as a layout change of its group (Decision 1.8); the spec writer
      should record what the refresh does, or this rule as Tessera's.
  16. UL §9.6.3 and SK §7.4: say whether a skin or 0x41 image in a 32-bit format other than
      A8R8G8B8 (A8B8G8R8, the engine-specific R8G8B8A8) reaches the compositor as
      "32-bit ARGB" after loading. This record tags them *other*, so an overlay from them
      draws nothing (§4.2).
- **Exactness, stated honestly.**
  - Surface contents are byte-exact to the specs everywhere except inside the coverage
    regions of nested entries, for content drawn after them (Decision 1.7). Question 7 and
    the per-panel log bound how much of the M5 panels this affects.
  - The composition onto the 3-D frame is exact for alpha 0 and 255 at 1 : 1, and an
    approximation otherwise, until the OPEN of UL §9.6.1 is settled by a frame capture.
  - Scaled and rotated surfaces depend on the filter OPEN and on the missing mips.
  - During a generation that spans frames, a window and its nested parts are drawn
    together at their last swapped layout, which lags the current layout (Decision 1.5).
    A first paint may appear in bands after 100 ms, and it can shorten or hide an
    auto-fade-in, because the fade clock starts at the logical show (Decision 1.8).
  - A Tile surface is repeated at the display-node scale, not fitted to its box (Decision
    1.6, §2.1), which is a Tessera rule for UL §8.4 OPEN and LPA §13.18 OPEN (2).
  - Plugin stretch modes 2 and 4, the composition of nested surfaces and rotation follow the
    Tessera rules of LPA §13.18–§13.20; the marker follows §2.3.
  - Image formats other than 32-bit ARGB and 24-bit RGB follow the Tessera rules of §4.2;
    overlays draw only from A8R8G8B8 and alpha-only sources, as UL §9.6.3 says.
  - Under a UI scale, drawn positions and hit-test boxes of elements inside a surface can
    differ by up to one pixel per nesting level (§2.4).
- **Owner observations that this record needs**, to be added to
  `docs/checklists/owner-observations.md` by the accepting session:
  1. the chat window at UI scale 1.0, 1.5 and 0.5 (the filter and the mips, FO Q3);
  2. a panel that fades below its top level (non-root surfaces, UL §16 item 11);
  3. a frame capture, or failing that a screenshot over a known background, of a
     semi-transparent panel edge (the composition blend, UL §9.6.1);
  4. a test plugin (written by the owner) with a window at opacity 0.5 that holds:
     - a child in stretch mode 1 that extends past its parent control;
     - a later sibling that overlaps the child;
     - a child window that extends past the window, holding a stretch-mode control that
       extends past the child window (the clip chain, Decision 1.7);
     - the same window rotated by *z* = 30.

     This settles the opacity, the clip, the order and the rotation of nested surfaces
     (LPA §13.18 OPEN (1), §13.19 OPEN, §13.20 OPEN). It can share a run with the rotation
     test plugin that LPA §13.20 already asks for;
  5. during mouse-look in the original: is the cursor hidden, and where does it reappear
     after the right button is released (§2.5, UB §4.9)?
  6. on the reference machine with the compositor factor 1.333: is Tessera's OS cursor at
     UI cursor scale 1.0 the same size as the original's (§2.5)?
  7. the minimap marker for a character facing north and east, and the marker's edges when
     it is turned by 45° (GP §8.6.3, checklist P1-A).

  None of them blocks M5: each has a default in this record and one place in the code to
  change.
- **Performance risk** is concentrated in event frames, not in steady state. Paint jobs that
  are never restarted, the per-frame upload cap, back textures and the transition rule keep
  them inside the M3 50 ms rule and keep resizing panels updating. Moving bands to the
  workers changes the client but not `tessera-ui`'s painter interface. Option C would change
  the renderer and the client.
- **Tool UI is unchanged:** egui remains for the explorer and the planned developer
  overlays only.

## Coordination with IB (required follow-ups in `proposed-ui-input-bridge.md`)

This record and IB are meant to be accepted together. This revision does not edit IB. It
applies IB's own "Coordination" items 1–9 to this record. **Two small follow-ups in IB
remain** (items 6 and 7 below); neither changes IB's design.
1. **IB's owners list: done.** IB's Consequences entry for this record now only points to
   IB's "Coordination" section; the old request (drop "process input" from step 8, exclude
   plugin Lua time from U1, add a high-rate pointer sweep) is gone. Revision 2 of this record
   had done all three.
2. **ARCHITECTURE §20.3 steps 7 and 8: one wording, IB's.** This record adopts IB D8 and IB's
   owners list: step 7 runs the UI tick (hover refresh, `MouseHover`, the `Update`
   broadcast, the rest), then the plugin host's deferred work (sliced by whole items, 2 ms
   per frame), then the collector step in every apartment, then the band changes; step 8 is
   "UI: layout, paint damaged rectangles within the UI slice, build the composition list"
   (IB Coordination items 1–3 and 9). The Context, §6 row U1 and the Consequences say the
   same. No change to IB.
3. **The pointer lock: applied**, including IB Coordination items 7 and 8 (revision 5). §2.5
   now follows IB D7 as revised: the lock turns on only after the UI declined a `mouselook`
   Start and no item drag is in progress (during every item drag the Start is consumed, by
   the UI or by the client), and off after the End of the last hold that turned it on; no
   cursor-position and no cursor-left events are fed during the lock; no cursor position is
   fed at its end, and a withheld cursor-left is fed right after the turning-off call. Items
   2 and 3 of §2.5 are derived from that. The owner summary, the Context and the
   Consequences ("Agreement with IB") use IB's wording. **Standing request:** any later
   change to IB D7's lock-on point, grab, warp or position feed says that it also changes
   §2.5 of this record. IB D7 item 2's reference to "the recorded difference of
   `proposed-ui-rendering.md` §2.5" still points to §2.5 item 3.
4. **U1's scope: agreed**, with IB Coordination item 9 applied. IB D8 cites U1 for Tessera's
   own work inside the step-1 offers and in the step-7 tick, in steady state outside the
   content-event frames that U4 judges, and excludes plugin time (the input handlers, the
   `Update` broadcast, the deferred work and the collector step). §6 row U1 now says exactly
   this. No change to IB.
5. **Layout used by the hover refresh: applied** (IB Coordination item 5): §2.4 and Decision
   1.5 say that the step-1 hit tests and the step-7 hover refresh use the layout of the last
   completed step 8.
6. **Required follow-up in IB: mark items 7–9 applied.** IB's "Coordination" section still
   presents its items 7–9 as changes this record must make. Revision 5 makes them, so IB
   should mark them applied, as it did for items 1–6.
7. **Required follow-up in IB: the OS cursor under `ui_cursor = software`.** IB D7 item 2
   says that the client "hides the cursor" when the lock turns on and "shows the cursor"
   when it turns off. With the software cursor in use, §2.5 now keeps the OS cursor hidden
   for as long as that mode lasts, so "shows the cursor" means that the software cursor is
   drawn again, and the OS cursor stays hidden. IB D7 item 2 should add that parenthesis
   (one sentence), so the two records do not read as if the OS cursor reappears.

Nothing else in IB depends on what revision 5 changed. The source-format tags (§4.2), the
clip chain (Decision 1.6, 1.7), rotation as surface state (§2.3), the first-paint bound
(Decision 1.5, U4 (c)), the content-job rule, texture usages, external content, the
`SetStretchMode` refresh, mode-0 child windows and Tile parents concern drawing only. The
new hit-test rounding difference (§2.4 item 3) uses IB D8's layout and changes nothing in
IB's routing. The proposed `plugins` → `ui` allowance does not touch IB, whose `input` and
`ui` stay independent.

## Response to the first review (2026-10-06)

| # | Review item | Answer |
|---|---|---|
| 1 | Opacity counted twice | Fixed (Decision 1.6). The entry opacity is the current `ObjectOpacity` alone. The plugin window opacity is the same property (LPA §13.19), so the product in the draft counted it twice as well. |
| 2 | Nested surfaces contradict auto-fade | Fixed: nested surfaces are separate entries with their own opacity only (Decision 1.7), so a fade costs no repaint and is never applied twice. The review's premise that the predicate is empty in M5 no longer holds. LPA §13.18–§13.19 (BIN) give plugin controls surfaces for any stretch mode other than 0, and `SetStretchMode` is used by 80 distributions (PU §5.3; 81 by a whole-distribution text search, PU §3.5). UW §4.5 gives alpha-test meters one, and GP §8.2 gives the minimap marker one. So nested surfaces are designed fully here, not deferred. The record departs from LPA §13.19's Tessera rule and raises that with the spec writers. (Revision 2 corrects the count, which earlier read "over 100 distributions (PU §6.8)": the §6.8 counts per mode overlap and cannot be added.) |
| 3 | Modal dialog surface owner | Fixed (Decision 1.1). The dialog owns the surface; LPA §13.19's rule supports the review's reading. Agreement with a nuance: the container gets no canvas because it draws nothing; whether the original gives it one is unknown and invisible. |
| 4 | Large surfaces never upload | Fixed (Decision 1.5): multi-frame upload into a back texture, then a swap. CPU canvases are sparse 256 × 256 tiles (Decision 1.2), which bounds CPU memory. The GPU copy stays one texture per surface; since revision 2 it is sized to the tile bounding box (Decision 1.2). |
| 5 | 100 ms against 250 ms | Fixed (Decision 1.8, U4): one deadline of 100 ms. The logical show is immediate; the visual show waits at most 100 ms, and the difference is recorded. Revision 2 defines what is shown after 100 ms (Decision 1.5). |
| 6 | `SetBackground(path)` must fail synchronously | Fixed (§4.4): VFS index plus a bounded header probe, as a stated exception to ARCHITECTURE §21. A later decode failure is a stated deviation. |
| 7 | Image formats | Fixed (§4.1–§4.3, §4.6): skin formats, the canonical form, A8 per SK D17, `.png`/`.bmp` with PU O6, and large images. **Not followed as suggested:** an image over the size limit is cropped instead of replaced by the placeholder, because the crop is exact for every element up to 4,096 pixels on that axis. |
| 8 | LRU breaks exact repaints | Fixed (§4.5): pinning plus a hard cap. |
| 9 | Hiding and showing surfaces | Fixed (Decision 1.8): release on hide unless `KeepObjectWhenHidden`; a full repaint on show, through a first paint. |
| 10 | Unconditional erase | Fixed: OPEN in Decision 1.3, with a Tessera rule, and raised with the spec writers. |
| 11 | Scale, rotation, hit testing, cursor | Fixed. Per-surface resampling is labelled a Tessera default (§2.2). Rotation is three-dimensional, per LPA §13.20 and its Tessera rules (§2.3). Hit testing works on the displayed boxes that composition also uses (§2.4). The cursor is §2.5. |
| 12 | Smaller points | Fringes: premultiplied filtering in the shader (§2.2, §8). Mips: **not added** in M5, with the reason and an owner observation (§2.2). Mode-3 scissor: replaced by the nested-entry clip (Decision 1.6, 1.7, §8). Negative coordinates: canvas origin offset (§7). Zero size: no surface (Decision 1.1). Upload budgets: separate (§6). Worker bands: the interface claim is corrected (§6). |
| 13 | 40 ms spikes cite | Fixed: `walker.md` §9. |
| 14 | Option C storage formats | Fixed. |
| 15 | Missing consequences | ROADMAP exit criterion and the render section number: fixed. The new LPA OPEN on plugin background order is **no longer needed**: LPA §13.17 (BIN) settles the order (base, then overlay), and §5 follows it. |
| 16 | Stretch hit-testing citations | Fixed (§2.4): UB §4.1 step 3, §9.2.1 and §17 item 18, with FO §6.3 item 3. |
| 17 | "Mode 0 resets alpha" | Covered (§2.1): it is the default Replace of LPA §13.17, as LPA §13.18 now explains. |

## Response to the second review (2026-10-06, revision 2)

| # | Review item | Answer |
|---|---|---|
| 1 | Conflict with IB D8 | Fixed. (a) The Consequences now rewrite ARCHITECTURE §20.3 step 8 as "UI: tick (hover refresh, `MouseHover`), layout, paint damaged rectangles within the UI slice, build the composition list", and leave step 1 to IB. (b) U1 counts Tessera's own work in the step-1 offers and excludes plugin Lua time, which is reported as plugin time (§6). (c) The scripted run has a 1000 Hz pointer sweep over the toolbar and over a plugin window with `MouseMove` handlers (§6). IB's citation of U1 has since been corrected to §6 (third review); this record no longer asks for a change there. The context and the U4 slice also follow IB D8. *Revision 4: the tick has since moved to step 7 and step 8 is "UI: layout, paint damaged rectangles within the UI slice, build the composition list" (IB D8; fourth review, item 1).* |
| 2 | Rotation does not reach nested entries | Fixed as the review suggests. Every entry carries a transform chain: its own rotation, then each surfaced ancestor's rotation about that ancestor's centre (Decision 1.6, §2.3). Nested entries and canvases above therefore turn with their root. The clip is the root's rectangle tested in the root's own space by the shader, with the screen scissor kept only as a fast path for unrotated roots (Decision 1.6, §8). A render test covers a nested entry in a root rotated by *z* = 30 (§8). *Revision 5: the clip is now a chain of enclosing windows, nearest first (fifth review, item 2), and rotations are read from the surfaces (item 3).* |
| 3 | Multi-frame paints undefined; U4 not met as written | Fixed (Decision 1.5). Paint jobs run on a snapshot and are never restarted, so they always finish, and a resize-bar drag or a screen resize keeps updating the panel at the rate jobs finish. The transition rule keeps the old texture on the old box (size and scale of the last completed job; a pure move applies at once) and swaps texture and box in one frame. A first paint is the one case that may show completed bands, after the 100 ms of U4. The hit-test mismatch during a resize job is a recorded known difference (Decision 1.5, §2.4). U4 now has measurable parts (d) and (e) with the arithmetic behind them, and the scripted run drags the resize bars of P10 and P15 (§6). |
| 4 | `game-panels.md` not cited | Fixed. (a) The marker owns a surface (GP §8.2 step 3), so it is a nested entry with a `z` rotation and the premultiplied bilinear filter; the direction is fixed by GP §8.6.3's screen result (§2.3). (b) The scripted run walks and turns, including mouse-look and the run chord, and steady state includes that phase; the cost of the moving map is estimated, and turning costs nothing because the marker is an entry (§6). (c) The round minimap (composition with the surface alpha) and the unclipped rim are listed as checks of the composition path, with a render test and a painter test (§8). |
| 5 | Cursor during the pointer lock | Fixed (§2.5): a Tessera rule marked OPEN, aligned with IB D7. Hidden while locked; `Locked` grab where available, otherwise `Confined` with a move back to the frozen position on release, before the `select_right` pulse; the software cursor is not drawn while locked; focus loss ends it. Owner observation 5. *Revision 4: the lock now turns on only after the UI declined the Start, and no cursor-position event is fed (fourth review, item 2).* |
| 6 | Layering of several nested surfaces; quantity; caps | Fixed in part. Decision 1.7 now gives a general rule for any number of sibling, overlapping or deeply nested surfaces: one canvas above per nested entry, filled by "last covering entry before the pixel", composed in drawing order, recursively. Caps on surfaces (per plugin and in total), composition entries and nesting depth, with a visible fallback, are in §7. **Not done as asked:** the per-panel count of non-root surfaces. The specs do not contain it (UL §5.9 and §8.4 count all 302 layouts; UL §17.2 does not split object modes by panel), and the architect role reads no game data. The record asks the spec writers for the census (question 7) and has `tessera-ui` log it per panel at run time (Decision 1.7). |
| 7 | Composition scale from the boxes; §2.1 rows | Fixed (Decision 1.6, §2.1). The scale is displayed box ÷ canvas size for every surface, and the canvas is the layout box for every mode except plugin mode 2. Scaled CanvasResize panels (scaler types after UB §9.2 step 6) are drawn scaled. The None row now says that mode 0 wants no surface, apart from the shown-root-panel rule (Decision 1.1). Native Tile falls back to CanvasResize only when a side is not a power of two; the power-of-two case has a Tessera rule. |
| 8 | VFS index scope | Fixed (§4.4): the index covers the whole `Plugins/` tree, case-insensitive per LPA §4.4, rebuilt when the plugin manager refreshes. |
| 9 | Source format lost | Fixed (§4.2): a source-format tag is kept next to the pixels; an overlay with an image draws only for *ARGB32* and *alpha-only*. |
| 10 | GPU texture per surface at 4K | Fixed (Decision 1.2, 1.5): the texture is sized to the tile bounding box, empty tiles are never uploaded (merged rectangles are cut to allocated tiles), freed tiles are cleared on the GPU, and back textures exist only for uploads over the cap. A surface that would exceed U5 is no longer silent: a warning and a counter, a non-root fallback into the parent canvas, and a back texture that does not fit only delays the job (§7). |
| 11 | Missing damage triggers | Fixed (Decision 1.4, 1.1). (1) Showing, hiding, moving, resizing or rotating a nested surface damages its old and new coverage regions in the parent. (2) A surface under a hidden ancestor has no entry, keeps its canvas and counts toward U5 and the caps. (3) A cursor-image change is not surface damage; it sets the OS cursor or the software-cursor entry. |
| 12 | Distribution count | Fixed (Context and first-review item 2): 80 distributions (PU §5.3), 81 by text search (PU §3.5); the §6.8 per-mode counts overlap. |
| 13 | Fuzz targets; cursor size | Fixed. §7 names fuzz targets for the TGA, DDS and JPEG decoders, the header probe, the painter and the composition-list builder. §2.5 withdraws the division by the compositor factor, marks the size rule provisional, and offers the software cursor as the exact fallback (owner observation 6). |
| 14 | Check against UW, UB, LPA | Done. UW §5.7 and §6.9 (the hover push of `ScrollableControls_Mouse` at priority 0) are input-map changes, not damage (Decision 1.4); one wheel step is one line step (UW §6.8, Decision 1.4, §6). UB §9.2 step 6 and §9.2.1 drive §2.1. UB §4.9 drives §2.5. LPA §4.4, §13.17–§13.21 are followed as cited. |

## Response to the third review (2026-10-06, revision 3)

| # | Review item | Answer |
|---|---|---|
| 1 | MUST-FIX: turning a nested surface both damages and does not damage the parent | Fixed with option (a). An entry with its own rotation has an **angle-independent coverage region**: the square around the circle through its corners, valid for `x`/`y` tilts too under the orthographic projection (Decision 1.7, step 1). The marker's region is 40 × 40 at every heading. The first rotation an entry is given is one coverage change, and later turns are not damage (Decision 1.4). The "not surface damage" list now separates root surfaces (scale, position, rotation) from nested entries (moves and size changes are damage, own rotation after the first is not). §2.3, the §6 minimap estimate and P1 check 4 now follow from that rule, and check 4 has a 360° test. |
| 2 | MUST-FIX: Tile against the box-to-canvas scale | Fixed. Tile surfaces (native Tile with power-of-two sides, plugin mode 2) are exempt: canvas position = ((position − box origin) ÷ display-node scale) modulo the canvas size, so unscaled they are an exact 1 : 1 repeat (Decision 1.6). Inside a scaled panel the pattern scales with the panel and is filtered with wrapped neighbours. Decision 1.6, §2.1 (intro and both Tile rows, with the 32 × 32 on 200 × 100 example), §2.2, the §8 "UI pipeline" row and a §8 Tile test say so. Spec questions 10 and 11 ask for the rule to be recorded. |
| 3 | §2.5 restates IB D7 differently | Fixed. §2.5 cites IB D7's sequence as written (warp while the grab is held, release, show, then the cursor-position event at *F*) and adds only the drawing rules: no software-cursor entry while locked, its return at *F*, and a refused move, where only the OS cursor is out of place until the next motion (recorded difference). *Revision 4: superseded; IB D7 no longer feeds a cursor-position event (fourth review, item 2).* |
| 4 | Stale request about IB's U1 citation | Removed from the Consequences; second-review item 1 is annotated. |
| 5 | Alpha-test meters said to be in unknown panels | Fixed. UW §4.5 says that none of the 4 is in the UL §17 panels. Decision 1.7 drops them from the M5 concern and question 7 is narrowed. |
| 6 | U1 not derived for scrolling and chat | Fixed by classifying them. Frames with a scroll step or a new chat line are **content-event frames**: they are excluded from U1 and from U3's steady mean, and bounded by the 4 ms slice and the new U4 (f), with the arithmetic in §6 (P7 tree about 0.45–1.8 ms, P2 at design size 0.19–0.75 ms, a maximised chat log as a multi-frame content job). Notches of one frame merge into one repaint. The owner summary asks the owner to accept the split (question 2). |
| 7 | Nested entries out of line with a lagging root | Fixed. The transition rule now applies to the **root group** (Decision 1.5): the jobs of one layout change form a generation, the whole group keeps the last completed generation's geometry and textures, and everything swaps in one frame. Content-only jobs swap on their own. A §8 test covers it. |
| 8 | Coverage regions under a scaled parent | Fixed (Decision 1.7, step 3): footprints are taken in the root's unrotated space, then converted to the parent's canvas by subtracting its displayed origin, dividing by its composition scale (for a Tile parent its display-node scale), subtracting the canvas origin offset and rounding outward. *Revision 5: Tile surfaces no longer have nested entries (fifth review, item 14).* |
| 9 | Auto-fade against the withheld first paint | Fixed (Decision 1.8): the fade clock starts at the logical show, so messages, opacity and the hit-test exclusion follow the original's clock. The drawn fade can be shortened or not seen, which is a recorded difference. The alternative (a separate drawn-opacity clock) is named as the change to make if the owner prefers it. |
| 10 | MINOR: debug overlays not in ARCHITECTURE §13 | Fixed. Decision 1.6 and the §8 "Order" row name the overlays that stay on top (the walker's HUD and minimap, `walker.md` §6; the egui frame-stats overlay), and the Consequences add an overlay pass to ARCHITECTURE §13's frame structure. *Revision 4: the walker's overlays now draw under the game UI (fourth review, item 4).* |
| — | Consistency with IB | Checked against IB D7 and D8. The follow-ups IB needs are in "Coordination with IB": a stale owners-list entry, one wording for ARCHITECTURE §20.3 step 8, and, for information, §2.5's dependence on IB D7 and U1's narrower scope. *Revision 4: all of these are settled; see "Coordination with IB".* |

## Response to the fourth review (2026-10-06, revision 4)

| # | Review item | Answer |
|---|---|---|
| 1 | MUST-FIX: frame loop out of date with IB D8 (tick in step 7) | Fixed in all four places: (a) Context, "Input timing"; (b) §6 row U1 (Tessera's own work in the step-1 offers, the step-7 tick and step 8; plugin time in the offers, the `Update` broadcast and the deferred work excluded); (c) Consequences, ARCHITECTURE §20.3 now takes IB's wording for steps 1, 7 and 8; (d) Coordination item 2 now adopts IB's wording instead of asking IB to adopt the tick-in-step-8 one. Coordination item 4 now states IB D8's actual citation (offers and step-7 tick). ARCHITECTURE §20.3 gets one definition. |
| 2 | MUST-FIX: §2.5 restates an older IB D7 | Fixed. (1) The lock turns on only after the UI declined the `mouselook` Start and off after the End of the last hold that turned it on; a consumed Start (a right press during an item drag, UB §5.3 step 4) leaves the cursor visible, ungrabbed and hit-tested, so the drop works. (2) No cursor-position event is fed during the lock and nothing at its end; items 2 and 3 are re-derived from that withheld-events rule, and the `select_right` pulse is at *F* because every event of the turning-off call carries *F* (IB D7 item 1). IB Coordination item 4 is applied. |
| 3 | MUST-FIX: Decision 1.5 front textures contradict the group rule | Fixed. Content jobs and generation jobs now upload differently. A generation job's rectangles never reach a front texture before its generation swaps: they go into a back texture or are held in the CPU canvas (the surface starts no other job until the swap), and the generation swaps in the first frame in which every job is complete and the held rectangles fit the budget. A canvas-size or tile-box change always uses a back texture. No content job starts on a surface with an un-swapped generation job; its damage waits. A content job already running when the layout change arrives finishes as a content job. A generation now includes every parent surface whose nested entries change coverage, not only the root. A §8 "generation hold" test covers the reviewer's case. |
| 4 | MUST-FIX: Tessera overlays hide native panels | Fixed (Decision 1.6, §8 "Order", Consequences). With the game UI attached the walker's HUD and minimap draw **under** the UI pass, the walker minimap is off by default (`walker_minimap`), and the HUD has the setting `walker_hud` (on or off), off in checklist runs. The developer overlay (planned) stays above; the software cursor is drawn last of all, after it. The P1 and P4 arithmetic at 3840 × 2160 is in Decision 1.6. Owner question 6. |
| 5 | MUST-FIX: §2.1 row "0 None" resize | Fixed. A mode-0 element owns no surface, so UB §9.2.1 step 3 applies: the layout box takes the size and the children are laid out again; a root panel drawn through a Tessera surface resizes and fully repaints its canvas as for CanvasResize. Decision 1.1 says the same. |
| 6 | Coordination item 1 stale | Marked done (Coordination item 1). |
| 7 | §2.4 omits the step-7 hover refresh | Fixed (§2.4, Decision 1.5). |
| 8 | Opacity rule leaves Fade items and `SetOpacity` children unfaded | Named: Decision 1.7 states the consequences (P1 marker under a Fade, stretch-mode icons in a window at opacity 0) and the contradiction with UL §5.9 and §6.6; owner question 3 and spec question 1 carry them. The rule itself is unchanged; the alternative stays one switch. |
| 9 | "Root" used in two senses | Defined (Decision 1.1): root = a surfaced element with no surfaced ancestor; root panel = the UL/LPA notion. A rotated child window is a nested entry with an angle-independent coverage square, its rotation is entry data, and it inherits its ancestors' transforms (Decision 1.1, 1.7, §2.3). |
| 10 | §4.4 index rebuild is main-thread I/O | Fixed: the index is built and rebuilt on an asset worker and swapped in whole; the stated exception covers only the 64-byte header read. |
| 11 | Minimap estimate too low | Fixed: §6 shows the arithmetic per layer (≈ 0.08–0.26 ms with the erase, Mix colour taken as Over for lack of a probe figure) and flags U1's mean as at risk in the walking phase, with the worker band as the answer and owner question 2 as the alternative. |
| 12 | Alpha-test meter position is not damage | Fixed (Decision 1.4: excluded from the meter damage rule and listed with the non-damage changes). |
| 13 | First paint condition | Fixed: Decision 1.5 cites Decision 1.8's condition (first job and requested images), and Decision 1.8 says it is the one definition used by U4 (c). |
| 14 | egui overlay does not exist | Fixed: "planned" in Decision 1.6, §8 and the Consequences. |
| 15 | Worker count citation | Fixed: N = cores − 2 from ARCHITECTURE §21, 14 from `docs/perf/reference-machine.md`. |
| 16 | Canvas origin offset and off-screen windows | Fixed (§7): a surface within the limits keeps its full canvas with offset 0, also partly off-screen or at negative coordinates; first-review row 12's "negative coordinates" now reads in that sense. |
| 17 | The crop of §4.6 against SK §6.10–§6.11 | Added to owner question 5 and to §4.6; spec question 6 cites SK §6.10 too. |

## Response to the fifth review (2026-10-06, revision 5)

| # | Review item | Answer |
|---|---|---|
| 1 | MUST-FIX: §4.2 tags DXTn as *ARGB32*; other 32-bit orders and A4R4G4B4 untagged | Fixed (§4.2). The tag records the source format. Only A8R8G8B8 sources are *ARGB32*: 0x41 format 21, 32-bit TGA and DDS A8R8G8B8. A8 sources are *alpha-only*. DXT1–DXT5 (0x41 and DDS), A8B8G8R8, the engine-specific R8G8B8A8, A4R4G4B4, A1R5G5B5, both 10-bit formats and 16-bit TGA are *other*; every format without alpha is *no alpha*. A table gives each case. An overlay draws only for *ARGB32* and *alpha-only*, as UL §9.6.3 says. Spec question 16 asks whether other 32-bit orders reach the compositor as 32-bit ARGB. In practice overlay images come only from data-named 0x41 surfaces, all 32-bit ARGB in this install. |
| 2 | MUST-FIX: clipping by the root is not LPA §13.18's "clipped to its window only" | Fixed (Decision 1.6, 1.7 "Clip", §2.3, §8 "Clip" and a clip-chain test). Each nested entry is clipped by its nearest enclosing window (a root, or the surface of a root panel or stand-in surface), in that window's rotated space, as LPA §13.18 says. The record also clips by the enclosing windows above that one, which changes nothing unless a child window extends past its own parent window. This extension is recorded as spec question 12. For the M5 native panels the result is unchanged. |
| 3 | MUST-FIX: §2.3 puts `GetRotation()` angles on the window's entry | Fixed (§2.3, Decision 1.1, 1.6, 1.7 step 1, 1.8, §8). Rotation is state of the surface that received it. LPA §13.20 step 3 picks the window's own surface or the nearest ancestor's. A window remembers only a rotation that reached its own surface, and a new surface takes that remembered rotation. Composition reads the surfaces, never `GetRotation()`. The reviewer's example now gives LPA's result: *P* turns by 60, and *C* is shown unrotated relative to *P*. Stand-in surfaces of mode-0 windows are not targets. |
| 4 | MUST-FIX: the 100 ms first-paint promise breaks while a generation runs | Fixed by stating the real bound, with one improvement. The first-paint job of a nested surface shown during a generation starts at once. Only the parent's split waits for the next generation. U4 (c) now gives 100 ms for every root and for nested surfaces of a group with no running generation. Otherwise it gives the swap of the next generation: within 8 frames for groups up to 1038 × 802, and within 41 frames for the screen-sized plugin window after a screen resize. Decision 1.5 explains why joining the running generation is not possible. The scripted run measures both plugin-window cases. The owner summary item 4 and owner question 5 say so. |
| 5 | Content job may start before a back texture swaps | Fixed (Decision 1.5, content job step 4). No new job starts on the surface until its back texture is swapped in. The wait is bounded by the upload cap (at most 4 frames for a 3840 × 2160 canvas). |
| 6 | §8 cites render.md §4.1 for stored-value blending | Fixed. The UI pass adopts, for itself only, the fix that the second OPEN of render.md §4.8 suggests. It differs from render.md §4.1's Decision, which is unchanged. |
| 7 | Texture usages | Fixed (§8 "UI textures"): `TEXTURE_BINDING`, `COPY_DST`, `COPY_SRC`, `RENDER_ATTACHMENT`, each with its use. |
| 8 | External content format and alpha | Fixed (Decision 1.6 source flag, 1.7, §8 row and test). The target is `Rgba8Unorm` with stored values at the layout-box size. It is cleared to transparent black and holds premultiplied colour (opaque draws write alpha 1, alpha blended `One`/`OneMinusSrcAlpha`). The shader skips its premultiply step for it, and an sRGB target is refused. |
| 9 | Software cursor: OS cursor visible too | Fixed (§2.5, §8 "Cursor"). The OS cursor stays hidden while the software cursor is in use. IB follow-up 7 asks IB D7 to say the same. |
| 10 | `SetStretchMode` refresh on an existing surface | Fixed (Decision 1.8, §2.1 table, spec question 15). The surface is kept, so this is not a first paint. A change of canvas size, displayed box or coverage, or a change to or from Tile, is a generation of its group. Otherwise only the entry changes. |
| 11 | Mode-0 child `Window`: own surface or parent canvas; opacity | Fixed (Decision 1.1, spec question 13). A mode-0 root panel with no surface above it gets a stand-in surface at opacity 1. A mode-0 child window is drawn into the nearest surface above it, as LPA §13.17 says, with no entry, no opacity and no rotation. LPA §13.19's "opacity has no effect" holds in both cases. |
| 12 | UB §9.2.1 None branch against LPA §13.19 condition 1 | Added to §2.1 and as spec question 14. The reading is unchanged. |
| 13 | "most of them on root panels" unsourced | Dropped (Decision 1.7). |
| 14 | Tile parents with nested entries | Fixed (Decision 1.6, 1.7 "Tile parents"). A Tile surface has no nested entries. Elements below it are painted into its canvas as the cap fallback does, and external content below it draws nothing, each with a warning and a counter. The fuzz target checks it. |
| 15 | `plugins` → `ui` not allowed by ARCHITECTURE §3 | Fixed (Consequences, "Crate boundaries" and the edits list; owner question 1). The record proposes the one-way allowance with its reason. The alternative, a trait defined in `tessera-ui` and wired by the client, is left to the accepting session. |
| 16 | Hit-test boxes against drawn positions under a scale | Added as the third recorded case of §2.4: less than one pixel per nesting level, edge pixels only. It is not covered by any filter tolerance. Owner question 5 and the Exactness list carry it. |
| 17 | Mix colour and erase have no probe figure | Added (§6, "Probe gap"; owner question 2). The walking-phase U1 mean becomes a criterion only after a Tessera micro-benchmark measures both. |
| — | Consistency with IB | IB Coordination items 7–9 applied: the item-drag lock-on point, cursor-left withheld during the lock, and the collector step in step 7 and in U1. "Coordination with IB" now lists two follow-ups for IB: mark items 7–9 applied, and one sentence on the OS cursor under `ui_cursor = software`. |
