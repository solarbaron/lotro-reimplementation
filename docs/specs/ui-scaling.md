<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI scaling: coordinate spaces, the UI scale, fitting layouts to any window, crisp text and art

| | |
|---|---|
| **Status** | Review (2026-10-16, first version; awaiting an independent review by a session that wrote no part of it) |
| **Last updated** | 2026-10-16 |
| **Author role** | Spec writer (dirty room). A Tessera **design** spec for [decision 0040](../decisions/0040-ui-scaling.md), like `tessera-protocol.md`; the only install input is a read-only census of the layout records (§1, rows SC1–SC6) |
| **Reviewer** | — (pending) |
| **Implements into** | `crates/ui` (`tessera-ui`: UI screen, displayed boxes, fit rules, keep-on-screen, saved placements, painter at the content scale, derived glyphs, scaled images), `crates/settings` (the `[UI]` keys of §4.4 and §7), `crates/render` (no contract change; §6.6), `crates/plugins` (Display and scaling views, §7.7), `crates/input` (four Tessera actions, §7.3), `apps/client` (`ui_host`: pointer conversion, live change, the Interface window, the `/uiscale` command, `--ui-scale` and `--ui-safe-aspect`), `apps/explorer` (optional preview control), `fuzz/` (settings keys, configurations) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (**UL**: design size §5.1, edge modes §5.8, run-time layout §8, image drawing §9.6, saved locations §9.9, panel catalogue §17). [`ui-behaviour.md`](ui-behaviour.md) (**UB**: hit test §4, tooltips §6, drag and drop §7, window manager §8, the original's UI scale §9). [`ui-widgets.md`](ui-widgets.md) (**UW**: the slider-mode scroll bar §6). [`fonts.md`](fonts.md) (**FO**: font records §3, placement §6, layout §7, drawing §8, census §10, fallback font §11). [`render.md`](render.md) §6 (**RD**: UI pass). [`lua-plugin-api.md`](lua-plugin-api.md) (**LPA**: coordinates §13.1, `Display` §13.10, scale §13.21, game-styled controls §14.2, font enum A.2, scaling groups A.7). [`keymap.md`](keymap.md) (**KM**). [`character-screens.md`](character-screens.md) (**CS**) §2.2. [`travel-windows.md`](travel-windows.md) (**TW**) §5.2. Decisions [0014](../decisions/0014-ui-input-bridge.md) (**IB**), [0015](../decisions/0015-ui-rendering.md) (**UR**), [0020](../decisions/0020-chat-command-routing.md), [0031](../decisions/0031-parity-panel-goldens.md), [0040](../decisions/0040-ui-scaling.md) |

---

<a id="s0"></a>
## 0. Purpose

Decision 0040 makes Tessera's UI scaling its own: the layout records, element trees and art
still come from the user's install unchanged, but how they are sized and placed on the screen
is decided here, not copied from the original (which scales whole panels as magnified
pictures, UB §9.2, FO §6.3). This spec lets an implementer build:

- three coordinate spaces and the **one** space in which hit testing, tooltips, drag and drop,
  saved window positions and the plugin API work (§3);
- a user **UI scale** with an automatic default, stored in Tessera's settings file and applied
  live (§4);
- the rules that fit a layout designed for one size to any window from 1280 × 720 to at least
  7680 × 4320 and from 4:3 to 32:9, with a configurable **safe area** for wide screens, no
  degenerate boxes and every window kept on screen (§5);
- **crisp** drawing: text laid out and rasterised at the scaled size, art resampled once at
  the target size and kept pixel-aligned (§6);
- the options and Tessera's own **Interface** window that holds the scale slider (§7);
- performance budgets at 4K and above (§8), a test plan with golden images and invariants
  (§9), and worked examples (§10).

Nothing here is a fact about the original client beyond what the Accepted specs already say.
Counts from the census are observations of one install on one date; rules never depend on
them.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Design** = decided by this spec for decision 0040 (normative, no observation
behind it). **Project** = a fact or rule of another Tessera spec or decision, restated here.
**Confirmed** = observed in the user's install, read-only and in place, with the date.

| # | Facts | Source | Mark |
|---|---|---|---|
| — | Everything not listed below: coordinate spaces, the UI scale and its default, the design floor, the safe area, fit containers, the backdrop rule, keep-on-screen, saved placements, the pixel-centre mapping, image and text resampling, glyph derivation, options, the Interface window, budgets at scale, test plan | This spec, 2026-10-16, for decision 0040. No observation of the original's scaling was used, by the decision's rule | Design |
| SC1 | Layout design sizes: 1024 × 768 (283 layouts), 1440 × 900 (8), 2560 × 1080 (5), 1280 × 960 (2), 1280 × 1024 (1), 1280 × 1600 (1), 127 × 127 (1), 0 × 0 (1). The 2560 × 1080 layouts are the intro, data-patch, loading and two session-play screens; the 1440 × 900 ones include the four character screens | UL §5.1 (Confirmed 2026-10-05); re-observed 2026-10-16 with a throwaway probe over the layout records (archives unchanged since 2026-10-04, file times checked 2026-10-16) | Confirmed |
| SC2 | Edge mode 6 occurs on 26 elements, **all** with `UICore_Element_ObjectMode` Stretch: (6, 5, 6, 5) 17 (3 of them top-level), (5, 5, 6, 5) 4, (6, 5, 5, 5) 4, (1, 1, 1, 6) 1. The (5, 5, 6, 5) and (6, 5, 5, 5) elements are the left and right side panels of the character and mission-controller screens | UL §5.8 census; object modes and placement by the probe of SC1 (301 of 302 records decoded by the probe; the 302nd, design 1024 × 768, holds none, as the counts agree with UL §5.8) | Confirmed 2026-10-16 |
| SC3 | Top-level elements: 3,139 in the 301 decoded records; object mode absent on 2,980, Stretch 78, CanvasResize 65, None (0) 16. Most common edges (0, 0, 0, 0) 2,680, (5, 5, 5, 5) 151, (5, 0, 5, 0) 121. Top-level elements whose design box covers their layout's design area with edges (5, 5, 5, 5): 16 (object mode None 12: the intro, disconnect, loading, three character, race-and-class, item-selection, map-window, PvP and two session-play roots; Stretch 4: two credits fields, a theatre background, a movie-player foreground); with (6, 5, 6, 5): 3 | Probe of SC1 | Confirmed 2026-10-16 |
| SC4 | Stretch elements with edges (5, 5, 5, 5) that cover their parent's design box: 12 non-root elements (viewports, fade and darken overlays, map overlays, theatre mattes) and 4 roots | Probe of SC1 | Confirmed 2026-10-16 |
| SC5 | Run-time layout simulation (UL §8.4 steps 2–6, fit containers of §5.4 kept at their design size, every top-level element laid out as a screen-level panel, own children only, inherited children not expanded): for the 1024 × 768 layouts, no element that is non-degenerate at the design size becomes degenerate at UI screens of 1024 × 768 and 1365 × 768; 51 do at 1280 × 720 (height below 768); 10 do at 1920 × 1080 and every larger size tried (up to 5120 × 1440), all templates or code-sized fields whose edge pairs shrink as the parent grows. Without the fit-container rule the loading screen's meter and label become degenerate at 1600 × 900 (as UL §8.4 reports); with it, none of the loading screen's elements does at any size tried | Probe of SC1 (simulation in the dirty scratch area) | Confirmed 2026-10-16 (method as stated) |
| SC6 | `loadscreen` tree: `LoadScreenMain` 2560 × 1080 (5, 5, 5, 5) → `LoadScreenImage` 2560 × 1080 (6, 5, 6, 5) Stretch with one image → `NormalBackground` 1024 × 768 at (768, 312) (5, 5, 5, 5) Stretch → meter 199 × 14 at (410, 735), label 205 × 20 at (407, 732) (base font `verdana12`), ring, overlay. `chargen_appearance` 1440 × 900: root (5, 5, 5, 5); `ControlsParent` (6, 5, 6, 5) holding the create, race-change, advanced-start and name-entry parts; left panel 500 × 900 at (0, 0) (5, 5, 6, 5); right panel 500 × 900 at (940, 0) (6, 5, 5, 5) whose child `CharGen_Appearance_Panel` is 280 × 816 at (186, 44) | Probe of SC1; agrees with UL §8.4 (P5 note), TW §5.2, CS §2.2 | Confirmed 2026-10-16 |
| SC7 | Fonts are bitmap records at one pixel size each (line height 10–36); the plugin `Font` enum names every one of the 48 records, and each name ends in the record's line height | FO §0, §3, §10; LPA Appendix A.2 | Project |
| SC8 | Image layers are drawn at native size, tiled from the element's top-left and clipped, never stretched; of 10,350 Image items on typed elements the image equals the element's size in 6,127, is smaller on an axis in 3,000 (tiled pieces) and larger in 1,223 (clipped) | UL §9.6 | Project |
| SC9 | The original's scale model (display nodes, layout box and displayed box, element resize in Stretch, the scaler's fit and origin rules, `UIScaleDefinition_Array` with Min 0.4, Max 4.0, 73 stops) | UB §9.1, §9.2, §9.2.1; FO §6.3 | Project |
| SC10 | Plugin coordinates are pixels, a window's position is relative to the screen (`Display`); `Display.GetWidth/GetHeight` give the UI area; `GetUIGroupScale`; window scaling members and rules | LPA §13.1, §13.10, §13.21 | Project |
| SC11 | Budgets U1–U5, the 4 ms paint slice, jobs and generations, "Workers if needed", surface limits | UR §6, §7, Decision 1.5; RD §6 | Project |
| SC12 | Committed goldens on main: 18 panel goldens in `crates/ui/tests/golden` (220 × 84 to 1024 × 768, painted at 1 : 1), 5 client UI goldens at 1024 × 768, render and terrain goldens without game UI | The repository at a45e0e8, read 2026-10-16 | Project |

---

<a id="s2"></a>
## 2. Requirements and where they are met

Every requirement of decision 0040 §2, its §3 and its Consequences:

| Decision 0040 | Requirement | Met by |
|---|---|---|
| §2.1 | Any window from 1280 × 720 to at least 7680 × 4320 and from 4:3 to 32:9 | §4.3 (design floor), §5.1–§5.5 (reference boxes, fit rules), §10 (examples), §9.2 (configurations C1–C12) |
| §2.1 | No degenerate or negative box | §5.7 (guarantee by construction), invariant I5 |
| §2.1 | No window or panel placed beyond the visible screen | §5.6 (fit to screen), §5.8 (keep-on-screen, saved placements), invariant I6 |
| §2.2 | A user scale setting (slider) | §4.1, §7.2 |
| §2.2 | Stored in Tessera's settings file | §4.4 |
| §2.2 | Applied live, without a restart | §4.5 |
| §2.2 | Default derived from window height and the OS scale factor | §4.2 |
| §2.3 | Ultrawide: HUD anchored to a configurable safe area, user picks its width | §5.1, §5.2, §5.3, §7.1 (`ui_safe_aspect`) |
| §2.4 | Text laid out and rasterised at the scaled size | §6.4, §6.5 |
| §2.4 | Art sampled so it stays sharp; right on 4K and high-DPI | §6.1–§6.3, §6.7 |
| §2.5 | One coordinate space for hit testing, tooltips, drag and drop, saved positions, plugin screen and element coordinates | §3.5, §3.6, §5.8, §7.7 |
| §2.5 | Plugins and saved layouts keep working when scale or window size changes | §5.8, §7.7, invariants I8, I13 |
| §2.6 | Golden images at several sizes, aspect ratios and scales | §9.2–§9.4 |
| §3 | Further options, each recorded | §7.1 (table with Required / Optional) |
| §1, Consequences | Deviation listed for the P8 parity checklist | §11 |
| Consequences | `ui-layout.md` §8.4 step 6 answered; stop-gap stays until built | §5.9, §12 |
| Consequences | `tessera-ui`, UI host, `tessera-settings`, plugin display views change | "Implements into" above; §12 |

---

<a id="s3"></a>
## 3. Coordinate spaces

<a id="s3-1"></a>
### 3.1 Physical pixels

The render target's pixels: integers, origin at the top-left of the window's inner area, *x*
to the right, *y* down. *Wp* × *Hp* is the window's inner size in physical pixels (the size
the renderer draws, UL §8.5 Tessera rule). Pixel (*X*, *Y*) has its centre at
(*X* + ½, *Y* + ½) (RD §6.6 "Pixel rule"). Only the renderer, the input layer and the
mapping of §3.6 work in physical pixels.

<a id="s3-2"></a>
### 3.2 UI units and the UI screen

A **UI unit** is Tessera's logical pixel. With the effective scale *Se* (§4.3):

- the **UI screen** *F* is *Wu* × *Hu* UI units, with *Wu* = round(*Wp* ÷ *Se*) and
  *Hu* = round(*Hp* ÷ *Se*) (round half up, computed in 64-bit floats);
- the **axis factors** are *Kx* = *Wp* ÷ *Wu* and *Ky* = *Hp* ÷ *Hu* (physical pixels per
  UI unit). They differ from *Se* by at most *Se* ÷ 2048 (*x*) and *Se* ÷ 1536 (*y*), because
  *Wu* ≥ 1024 and *Hu* ≥ 768 (§4.3); they exist so that *F* covers the window exactly, with no strip left at
  the right or bottom edge.

The whole element tree lives in UI units: the screen root has the size of *F* (it replaces the
physical screen size of UL §8.5 and UB §9.4), layout boxes are integers in UI units, and the
arithmetic of UL §8 is unchanged. At *Se* = 1, one UI unit is one physical pixel.

<a id="s3-3"></a>
### 3.3 Design units

The numbers in a layout record: an element's rectangle (UL §5.3) and the layout's design area
(UL §5.1). Inside an element tree a design unit **is** a UI unit: a 400-unit-wide element in
the record is 400 UI units wide in its layout box. Design units differ from UI units only
where a layout's design area is mapped to a reference box of another size by the run-time
layout (UL §8.3–§8.4) or by the fit rules of §5.4.

<a id="s3-4"></a>
### 3.4 Node scales and the content scale

Some elements carry a **node scale** that enlarges or reduces their subtree as a whole, around
the element's displayed top-left corner. Tessera has four kinds:

| Kind | Value | Uniform | Set by |
|---|---|---|---|
| Fit factor *f* | real > 0 | yes | a fit container (§5.4) |
| Per-window scale *p* | 0.5–2.0 for a group scale (§7.4); any positive value from a plugin's `SetScale` | yes | the optional group scale (§7.4) or a plugin `Window:SetScale` (§7.7) |
| Fit-to-screen factor *q* | 0 < *q* ≤ 1 | yes | a root panel larger than *F* (§5.6) |
| Stretch vector (*sx*, *sy*) | real > 0 each | no | an element resize of a Stretch surface (UB §9.2.1), a plugin stretch mode (LPA §13.18) |

For an element *e*, *m*(*e*) = (*mx*, *my*) is the product of the node scales of *e* and of
every ancestor of *e* (each uniform kind counts on both axes). Its **uniform part** *u*(*e*)
is the product of the uniform kinds only. The element's **content scale** is

*c*(*e*) = *Se* × *u*(*e*).

Text and art inside *e* are rasterised at *c* (§6). A stretch vector is not part of *c*: it is
applied when the surface is composed (§6.6), because a stretch was asked for by data or a
plugin as a picture stretch.

**Displayed box.** Each element has a layout box (UL §8, integers, relative to its parent's
layout box) and a **displayed box** in UI units relative to the screen, as real numbers:

1. the screen root's displayed box is (0, 0, *Wu*, *Hu*);
2. a fit container's displayed box is the result of §5.4;
3. any other element *e* with parent *P*, layout box left *l*, width *w* (relative to *P*):
   displayed left = (left of *P*'s displayed box) + *l* × *mx*(*P*). When *e* has no node
   scale of its own, its displayed right end is (left of *P*'s displayed box) +
   (*l* + *w*) × *mx*(*P*); when it has one, it is displayed left + *w* × *mx*(*e*). Top and
   bottom likewise with *my*. All in 64-bit floats.

Computing both ends from the parent's origin with the same expression makes the shared edge
of two abutting siblings the same number, bit for bit (§3.6 consequence 2).

Displayed boxes are half-open: [left, left + width) × [top, top + height). They replace the
truncating derivation of UB §9.2.1 for Tessera; with every node scale 1 they equal the layout
boxes made absolute, so all integer rules of UL and UB hold unchanged there.

<a id="s3-5"></a>
### 3.5 The one space

Every consumer below uses **UI units relative to the UI screen *F***, with the displayed boxes
of §3.4:

| Consumer | Rule |
|---|---|
| Pointer hit testing (UB §4.1–§4.2) | The pointer's position is the centre of its physical pixel mapped by §3.6; an element contains it when it lies in the element's displayed box. Alpha tests and text hit tests convert it as §3.6 says |
| Hover, capture, clicks, wheel (UB §4.3–§4.7) | as hit testing |
| Click and drag thresholds of the UI (UB §7.1, the double-click distance of UB §3.3 where the UI applies it) | UI units. Thresholds applied inside the input layer (KM) stay in its own units |
| Tooltips: placement, size, cursor offset (UB §6.4, §6.5) | UI units; the cursor offset is the cursor's physical size ÷ *Ky* |
| Drag and drop: icon position, drop target (UB §7) | UI units |
| Moves and resizes (UB §8.1, §8.2) | pointer offsets in UI units; for a target under a node scale they are divided by the target's *m* before the layout-box arithmetic (this is the Tessera rule of the UB §8.2 OPEN, now with exact division) |
| Saved positions and sizes (§5.8) | UI units, anchored |
| Plugin API: `Display` size and mouse position, window positions, control positions, `PointToScreen`, `PointToClient`, `GetMousePosition` (LPA §13.1, §13.2, §13.10) | UI units (§7.7) |
| World-anchored UI (floating names, floating text, combat cues) | the projected point in physical pixels divided by (*Kx*, *Ky*), drawn at *c* = *Se* |
| Layout (UL §8), screen root (UL §8.5), screen-size changes (UB §9.4) | UI units; *F* is the screen |

<a id="s3-6"></a>
### 3.6 Mapping between UI units and physical pixels (the pixel-centre rule)

- **Pointer → UI units.** A pointer in physical pixel column *X* has UI coordinate
  *ux* = (*X* + ½) ÷ *Kx*; likewise *uy* = (*Y* + ½) ÷ *Ky*. Where an integer is needed (the
  plugin API, integer comparisons of UB), it is ⌊*ux*⌋, ⌊*uy*⌋.
- **Edge → physical.** A UI coordinate *L* on the *x* axis maps to the physical column
  **E*x*(*L*)** = the smallest integer *X* with (*X* + ½) ÷ *Kx* ≥ *L*. In closed form this is
  ⌈*L* × *Kx* − ½⌉; an implementation computes the closed form and corrects it by one where
  the comparison disagrees, so that both sides use the same arithmetic. *y* likewise with *Ky*.
- **Box → physical.** A displayed box [*L*, *R*) × [*T*, *B*) covers the physical pixels with
  columns E*x*(*L*) … E*x*(*R*) − 1 and rows E*y*(*T*) … E*y*(*B*) − 1 (empty when
  E*x*(*R*) ≤ E*x*(*L*) or likewise in *y*). This is the element's **physical box**.

Consequences (invariants I3, I4):

1. A physical pixel lies in an element's physical box **exactly** when the pointer at that
   pixel's centre hits the element's displayed box. Drawing and hit testing agree everywhere,
   at every scale, with no tolerance (this removes the recorded difference of UR §2.4 item 3
   for Tessera).
2. Two sibling elements without node scales of their own whose layout boxes abut (one's
   right end is the other's left) share the edge in physical pixels: no gap and no overlap,
   because §3.4 computes both ends as the same number and both map it the same way.
3. An element whose displayed box is narrower than one physical pixel may cover no pixel; it
   is then neither drawn nor hit.

**Inside an element.** The alpha test (UB §4.1 step 3) converts a point to the element's own
unscaled pixels as ⌊(*ux* − displayed left) ÷ *mx*(*e*)⌋ and likewise in *y* (no tolerance
test is needed: the division is exact when *m* is 1). Text hit tests work in physical pixels
relative to the element's physical box (§6.4.6).

---

<a id="s4"></a>
## 4. The UI scale

<a id="s4-1"></a>
### 4.1 Range and steps

The user scale *S* is either **auto** (§4.2) or a number from 0.50 to 4.00 in steps of 0.05
(71 values). A stored or typed number is rounded to the nearest step (a value exactly halfway
rounds up) and clamped into the range. The slider shows it as a percentage ("150 %").

<a id="s4-2"></a>
### 4.2 Default: auto

With *S* = auto, the value used is

*S*auto = the largest multiple of 0.05 not above max(1.0, *σ*, *Hp* ÷ 1440), clamped to
0.50–4.00,

where *σ* is the operating system's scale factor for the window's monitor (the windowing
layer's scale factor; 1.0 when it is not finite or not positive) and *Hp* the window's inner
height in physical pixels. "Not above" uses a tolerance of 10⁻⁶ so that 1.5 stays 1.5.

Reasons: up to 1440 rows a UI unit stays one pixel (the layouts were designed for 768–1080
rows); above that the interface grows with the window, so a 2160-row window gets 1.5 and a
4320-row window 3.0; a user who has told the OS to enlarge everything gets at least that
factor.

| Window (physical) | *σ* | *S*auto | *Se* (§4.3) |
|---|---|---|---|
| 1280 × 720 | 1.0 | 1.00 | 0.9375 |
| 1920 × 1080 | 1.0 / 1.25 / 1.333 | 1.00 / 1.25 / 1.30 | same |
| 2560 × 1440, 3440 × 1440, 5120 × 1440 | 1.0 | 1.00 | 1.00 |
| 3840 × 2160 | 1.0 / 2.0 | 1.50 / 2.00 | same |
| 5120 × 2880 | 2.0 | 2.00 | 2.00 |
| 7680 × 4320 | 1.0 | 3.00 | 3.00 |
| 1600 × 900 window on a monitor at *σ* 1.5 | 1.5 | 1.50 | 1.171875 |

*S*auto is re-evaluated when *σ* changes (the window moved to another monitor) and when the
window's size has been stable for 250 ms after a change, never during a live resize drag.

<a id="s4-3"></a>
### 4.3 Effective scale and the design floor

*Se* = min(*S* (or *S*auto), *Wp* ÷ 1024, *Hp* ÷ 768).

So the UI screen is never smaller than 1024 × 768 UI units, the design size of 283 of the
302 layouts (SC1): no layout of that size is ever laid out into a smaller screen, which is
where shrinking edge pairs make boxes degenerate (SC5: 51 elements at a 720-unit height,
none at 768). *Se* need not be a multiple of 0.05. When *Se* is below the user's *S*, the
scale is **limited**, and the Interface window says so (§7.2). A window smaller than
1024 × 768 physical pixels gets *Se* < 1; there is no lower bound other than *Se* > 0.

<a id="s4-4"></a>
### 4.4 Storage

In section `[UI]` of Tessera's settings file (`crates/settings`, the section that already
holds `ui_scale_filter` and `ui_scale_cursor`):

| Key | Value | Default | Meaning |
|---|---|---|---|
| `ui_scale` | the string `"auto"` or a number 0.5–4.0 | `"auto"` | *S* (§4.1, §4.2) |
| `ui_safe_aspect` | §5.1 | `"21:9"` | width of the safe area |
| `[UI.panels]` | §5.8 | empty | saved placements |

Rules:

- A malformed value takes the default and is reported once as a load warning, like the other
  `[UI]` keys. Unknown keys are kept.
- **Migration.** When `ui_scale` is absent and the older key `ui_scale_global` holds a valid
  number, that number (rounded and clamped by §4.1) becomes *S*; the next save writes
  `ui_scale` and removes `ui_scale_global`. `ui_scale_global` is not read otherwise: the
  original's global factor no longer exists in Tessera (§7.7).
- The file is written when a change settles (§4.5), not at every slider step, and through the
  settings writer off the main thread.

<a id="s4-5"></a>
### 4.5 Applying a change live

A change of *S* (slider, command, action, settings reload) or of *Se* (window resize, monitor
change) runs these steps; nothing waits for a restart.

1. Compute *Se*, *F*, *Kx*, *Ky* and the safe area *A* (§5.1).
2. **Layout, in the same frame.** If *F* changed, resize the screen root to *F*: every root
   panel is laid out again by the rules of §5 (this is the screen-size path of UL §8.5 and
   UB §9.4 in UI units), saved placements are applied again (§5.8), and keep-on-screen runs
   (§5.8). Hover is refreshed in the new geometry. Plugins see the new `Display` size (§7.7).
3. **Preview, in the same frame.** Every surface keeps its last complete picture and is
   composed onto its new physical box, resampled by the GPU with `ui_scale_filter` (RD §6.7).
   No surface is ever blank because of a scale change (invariant I9).
4. **Settle.** When *Se* has not changed for 150 ms, or the slider is released, every surface
   whose content scale changed gets a new paint generation at the new *c* (UR Decision 1.5:
   jobs in bands within the paint slice, old picture shown until the swap). A job that was
   started for an older content scale runs to completion (UR: jobs are never restarted) but
   its picture is swapped in only if its scale is still current; otherwise the next
   generation starts at once.
5. Write the settings file (§4.4).

**The Interface window stays put while its slider is dragged.** From the press on the slider
to its release, the Interface window (§7.2) gets a compensating per-window factor
*S*press ÷ *Se* (its physical size and position do not change), so the slider does not move
away from the pointer. At the release the factor is removed and the window is placed so that
the slider's thumb stays under the pointer, then keep-on-screen runs.

<a id="s4-6"></a>
### 4.6 Window size and OS scale changes

A window resize changes *Wp* × *Hp*; an OS scale change changes *σ* (and, on most platforms,
the window's physical size). Both run §4.5 from step 1. Pointer positions recorded by the
input layer are rescaled by the input layer itself (KM §7.7, §7.8.9); the UI receives the
converted positions of §3.6 afterwards.

---

<a id="s5"></a>
## 5. Fitting layouts to the screen

This section replaces the two readings of UL §8.4 step 6's OPEN (§5.9).

<a id="s5-1"></a>
### 5.1 Reference boxes: the full screen and the safe area

- **Full screen** *F*: (0, 0, *Wu*, *Hu*).
- **Safe area** *A*: the full height of *F* and a centred width
  *Aw* = min(*Wu*, round(*Hu* × *a*)), at *Ax* = ⌊(*Wu* − *Aw*) ÷ 2⌋, where *a* is the safe
  aspect ratio from `ui_safe_aspect`:

| `ui_safe_aspect` | *a* |
|---|---|
| `"4:3"` | 4 ÷ 3 |
| `"16:10"` | 1.6 |
| `"16:9"` | 16 ÷ 9 |
| `"21:9"` (default) | 2.4 (so every common 21:9 screen, 2560 × 1080, 3440 × 1440, 3840 × 1600, is used in full) |
| `"full"` | no limit (*A* = *F*) |
| a number | that ratio, clamped to 4 ÷ 3 … 4.0 |

Because *Hu* ≥ 768 and *a* ≥ 4 ÷ 3, *A* is never narrower than 1024 UI units. On screens no
wider than *a* the safe area is the whole screen, so the setting matters only on wider
screens (for example on 32:9 with the default, a centred 21:9 area).

<a id="s5-2"></a>
### 5.2 Root panels: which reference box

A **full-screen root** is a root panel (UL §9.1) whose design box covers its layout's design
area (left ≤ 0, top ≤ 0, right ≥ design width − 1, bottom ≥ design height − 1) and whose edges
are (5, 5, 5, 5), or whose left and right edges are both 6 (SC3: 16 and 3 top-level elements
in this install). Every other root panel is a **safe-area panel**.

- A full-screen root is laid out by UL §8.3–§8.4 with the screen-level row, *N* = *F*.
- A safe-area panel is laid out by the same rules with *N* = (0, 0, *Aw* − 1, *Hu* − 1), and
  its result is moved right by *Ax*. So anchored HUD panels (edge modes 4 and 5) sit at the
  safe area's edges, centred ones (mode 1) at its centre, proportional ones (mode 3) scale
  with it, and panels with edge mode 0 keep their design offset from the safe area's left.
- Modal-dialog containers (UL §9.1) and the screen root are full-screen; a dialog inside its
  container is laid out against the container's safe box (§5.3).
- Plugin windows are not laid out by edges; their positions are the plugin's (§7.7).

<a id="s5-3"></a>
### 5.3 Children of a full-screen root

Each **direct child** of a full-screen root is laid out against one of two boxes:

- the root's **full box**, when the child is a backdrop: its design box covers the root's
  design box, or its left and right edges are both 6 (a full-cover fit container, §5.4);
- otherwise the root's **safe box**: the part of the root's current box that lies inside *A*,
  as a box of its own. The child is laid out by UL §8.4 with *O* = the root's design box and
  *N* = the safe box's size, and moved right by the safe box's offset inside the root.

Deeper descendants follow the ordinary rules. Example: the side panels of the character
screens are direct children that are not backdrops, so on a 32:9 screen they sit at the
edges of the safe area while the 3D scene viewport and the full-cover `ControlsParent` span
the screen (§10.2, row C8).

<a id="s5-4"></a>
### 5.4 Fit containers

A **fit container** is an element whose object mode is Stretch and which has at least one
edge in mode 6 (SC2: 26 elements, all Stretch). Its layout box always keeps its **design
size**, so its children are laid out exactly as designed (they are never laid out again
because the container changed size), and the container gets a uniform fit factor *f* (§3.4).
Its displayed box has the size (*w* × *f*, *h* × *f*), with *w* × *h* its design size, and is
placed by the type:

**Type A — both left and right edges 6** (full-cover fit, 17 elements, all (6, 5, 6, 5)).
Let *P* be the box it is laid out in (its parent's current box, or *F* for a root) and *G* the
**guard box**: *P* intersected with the safe area *A* (in *P*'s coordinates). Let *C* be the
**content box**: the union of the design boxes of the container's direct children, cut to the
container's design box (*Cw* × *Ch*).

1. *f*contain = min(*Pw* ÷ *w*, *Ph* ÷ *h*); *f*cover = max(*Pw* ÷ *w*, *Ph* ÷ *h*).
2. If the container has no children, *f* = *f*cover. Otherwise
   *f*guard = min(*Gw* ÷ *Cw*, *Gh* ÷ *Ch*) and
   *f* = max(*f*contain, min(*f*cover, *f*guard)).
   So the container covers *P* whenever its content still fits in the guard box, and never
   becomes smaller than a contain fit.
3. Place it centred in *P* on both axes. Then on each axis: if the scaled content box starts
   before *G*, move the container by the difference; else if it ends after *G*, move it back
   by the difference. If the content box is larger than *G* on that axis (possible only when
   *f* = *f*contain > *f*guard), centre the content box on *G* instead.
4. On each axis where the container is at least as large as *P*, clamp its position so that
   it still covers *P* (left ≤ 0 and left + *w* × *f* ≥ *Pw*; top likewise). When step 2
   chose *f* ≤ *f*guard, steps 3 and 4 never conflict (the content box lies inside the
   container, so a position exists that satisfies both).
5. The part of *P* that the container does not cover (letterbox bars) is filled with opaque
   black when *P* is a full-screen root, so no stale picture or world shows through.

This is the original's aspect mode on screens up to the design aspect (on 16:9 it fills the
height and crops the sides, as UL §8.4 notes for mode 6), and it covers wider screens instead
of letterboxing them, as far as the content allows (§10.1).

**Type B — exactly one of left and right is 6** (side panels, 8 elements). The height drives:
compute the top and bottom edges by UL §8.4 steps 2–5 in the box it is laid out in (*P*:
the parent's current box or the safe box of §5.3), giving the height *H*; then
*f* = min(*H* ÷ *h*, *Pw* ÷ *w*). The displayed width is *w* × *f*. A panel whose right edge
is 6 is anchored at its computed left edge; one whose left edge is 6 is anchored at its
computed right edge (exclusive end = computed right + 1). Its top is the computed top.

**Type C — no horizontal edge is 6, a vertical edge is** (1 element, (1, 1, 1, 6)). The width
drives: *W* from the left and right edges by UL §8.4 steps 2–5, *f* = min(*W* ÷ *w*,
*Ph* ÷ *h*); a panel whose bottom edge is 6 is anchored at its computed top, one whose top is
6 at its computed bottom, one with both 6 is centred on the computed box's vertical centre.
Its left is the computed left.

**Content scale.** Everything inside a fit container is rasterised at
*c* = *Se* × *u* with *f* included (§3.4), so scaled screens stay crisp. A consequence by
design: a full-window screen (loading, character screens) follows the **window**, not the
slider. Its content scale depends on the window's size only, because *f* compensates for a
change of *Se* (§10.1, §10.2). The user's scale enlarges everything else; the optional text
size (§7.5) also reaches text on these screens.

<a id="s5-5"></a>
### 5.5 Stretch elements resized by the layout; backdrop images

An element in object mode Stretch **without** an edge in mode 6, whose layout box the
run-time layout changes (UL §8.4 step 6), takes the new size into its layout box and lays its
children out again (the reading UL §8.4 records as implemented); no stretch vector is set.
This is a Tessera rule; it keeps such elements crisp.

**Backdrop images.** UL §9.6 draws images at native size and tiles them. For such an element,
an image layer whose native size equals the element's **design** size (the image was made
for it) is drawn instead as a **cover** fit of the current box: scaled uniformly by
max(current width ÷ design width, current height ÷ design height), centred and clipped to the
element (then resampled at *c* as §6.3 says). Images of any other size tile as UL §9.6 says.
This keeps full-screen backgrounds, mattes and overlays of Stretch elements (SC3, SC4) whole
on every aspect ratio, where tiling would repeat them.

An **element resize** of a Stretch surface by code, a resize bar or a plugin (UB §9.2.1) still
sets a stretch vector (§3.4): that stretch was asked for as a picture stretch.

<a id="s5-6"></a>
### 5.6 Panels larger than the screen

After its layout, when a root panel's displayed box (with its per-window scale) is wider than
*Wu* or taller than *Hu*, it gets a fit-to-screen factor
*q* = min(1, *Wu* ÷ displayed width, *Hu* ÷ displayed height) (§3.4), which shrinks it
uniformly so that it fits; *q* is recomputed whenever *F*, its size or its per-window scale
changes, and is 1 again as soon as it fits. This is the original's fit step (UB §9.2 step 2)
applied to every root, so no panel is ever larger than the screen. Example: the world map
panel (UL §17.2 P10, 1038 × 802) on a 1365 × 768 UI screen gets *q* = 768 ÷ 802 ≈ 0.9576.

A plugin window gets *q* the same way; its layout size (`GetSize()`) does not change.

<a id="s5-7"></a>
### 5.7 No degenerate boxes

The rules above prevent the known causes (the design floor of §4.3, fit containers of §5.4);
one final guard makes the guarantee hold for any data:

After UL §8.4 steps 2–5 (and before step 6), when an element's computed width or height is
below 1 UI unit, that axis takes the element's **design** extent, reduced to its parent's
current extent on that axis when larger, and the element is moved on that axis so that it
lies inside the parent's current box (0 … parent extent). A box of positive size, even one
that overflows its parent, is left as computed.

Because the screen root is at least 1024 × 768 and every parent is laid out before its
children, every element's layout box has a width and height of at least 1 UI unit
(invariant I5). The guard is the stop-gap rule of UL §8.4 made permanent and general; with
§5.4 the loading screen no longer needs it (SC5).

A displayed box can still be smaller than one physical pixel when *Se* × *m* is small; such an
element covers no pixel (§3.6 consequence 3), which is not a degenerate box.

<a id="s5-8"></a>
### 5.8 Keeping windows on screen; saved placements

**Keep-on-screen.** For a root panel with displayed box (*x*, *y*, *w*, *h*) (after §5.6, so
*w* ≤ *Wu* and *h* ≤ *Hu*): *x* = min(max(*x*, 0), *Wu* − *w*); *y* = min(max(*y*, 0),
*Hu* − *h*). The panel is moved (its layout position changes by the same amount), never
resized. It runs:

1. after a root panel is laid out (attached, or the screen root changed size);
2. after a saved placement is applied;
3. after a change of *Se*, *F*, *A* or a per-window scale;
4. at each move step and resize step (UB §8.1 step 3 and §8.2 step 4.6 already clamp to the
   screen; in Tessera they use these displayed boxes);
5. for **plugin windows** only in case 3, and only for a window that lay entirely inside the
   old *F*: a plugin may place its own window partly off screen with `SetPosition`, and
   Tessera never overrides that explicit position, except to bring back a window that the
   screen's shrinking pushed out.

The clamp uses *F*, not *A*: the user may drag panels into the wings of a wide screen.

**Saved placements** (the Tessera store of UB §8.4 and UL §9.9, in place of the original's
normalized corners). Key: the root's layout DID and element ID, written
`"0xLLLLLLLL/0xEEEEEEEE"` under `[UI.panels]`. Value (a table):

| Field | Type | Meaning |
|---|---|---|
| `ref` | `"safe"` or `"screen"` | the reference box *R*: *A* for safe-area panels, *F* for full-screen roots |
| `ax` | `"left"`, `"centre"`, `"right"` | horizontal anchor |
| `dx` | number, UI units, two decimals | offset from the anchor |
| `ay` | `"top"`, `"centre"`, `"bottom"` | vertical anchor |
| `dy` | number | offset from the anchor |
| `w`, `h` | integers, UI units | layout size, present only with `UICore_Element_save_size` |
| `shown` | bool | present only with `UICore_Element_save_visibility` |

- **Saving** (at the end of a user move or resize of an element with the save properties,
  UB §8.4 step 2): the anchor is chosen by where the displayed box's centre lies in *R*: in
  the left third → `left`, *dx* = box left − *R* left; in the right third → `right`,
  *dx* = *R* right − box right (exclusive ends); otherwise `centre`, *dx* = box centre −
  *R* centre. Vertically the same with top, centre, bottom.
- **Applying** (when the panel is attached, and again after every change of *F* or *A*): the
  panel's displayed position is computed back from the anchor, offset and its current
  displayed size; its layout size is set from `w`, `h` through the ordinary resize when
  present; its visibility from `shown`; then keep-on-screen runs, then UI message 0x2E
  (UL §9.9 step 5).
- So a panel docked near an edge stays at the same distance from that edge, in UI units, on
  every window size and scale, and a panel near the centre stays centred (invariant I8).
- **Migration.** An entry in the earlier normalized-corner form (UL §9.9 "The saved entry",
  if such entries were ever written) is converted once at the current configuration: its
  corners become UI units by UL §9.9 step 1 with *W* × *H* = *Wu* × *Hu*, then it is saved by
  the rule above.

<a id="s5-9"></a>
### 5.9 What happens to UL §8.4 step 6's OPEN

UL §8.4 step 6 asked whether a Stretch element resized by the layout keeps its layout box
and gets a scale vector, or takes the new size. Tessera answers by decision 0040, for its own
behaviour:

- fit containers (mode-6 Stretch elements): layout box kept at the design size, a **uniform**
  fit factor, placement by §5.4;
- other Stretch elements: layout box takes the new size, children laid out again, backdrop
  images covered (§5.5);
- the degenerate-box guard of §5.7 replaces the stop-gap rule, generalised.

The stop-gap stays in force until this spec is Accepted and built.

---

<a id="s6"></a>
## 6. Crispness

<a id="s6-1"></a>
### 6.1 Canvas sizes

Every surface (UR Decision 1.1) is painted by the CPU painter into a canvas of the size of its
**physical box**: (E*x*(*R*) − E*x*(*L*)) × (E*y*(*B*) − E*y*(*T*)) for its displayed box
without the stretch vector of its own node (a stretched surface is painted at its unstretched
size × *c* and stretched when composed). The limits of UR §7 apply in physical pixels (at most
the device's largest texture dimension and 16,384 on each axis; beyond that only the on-screen
part is stored, reported once). A Tile surface (UL §8.4 Tessera rules) is painted at its tile
size × *c*, rounded to whole pixels, and repeated at that period.

<a id="s6-2"></a>
### 6.2 Element boxes inside a canvas

An element painted into a surface's canvas occupies its physical box (§3.6) minus the
surface's physical origin. Clip rectangles, meter fills (UW §4), scroll offsets (UW §5) and
any other extent that a rule gives in layout units are converted the same way: the layout
coordinate is made absolute and displayed by §3.4, then mapped by §3.6. Line widths that the
specs give as one pixel (the caret, an underline of FO §8.5, the stamped outline of FO §8.3,
the focus or selection frame) become max(1, round(*c*)) physical pixels.

<a id="s6-3"></a>
### 6.3 Images

For an image layer (UL §9.6: base image or overlay surface; also animation frames, cursor
images, skin images, plugin backgrounds) of native size *iw* × *ih* texels on an element of
layout size *w* × *h* and physical box *PW* × *PH*, each axis is handled on its own (*x*
shown):

1. **Fitted axis** (*iw* = *w*: the image was made for this element, SC8): the image is
   resampled to exactly *PW* pixels: scale *s* = *PW* ÷ *iw*, clamp addressing.
2. **Tiled axis** (*iw* < *w*): scale *s* = *c*; the image starts at the element's physical
   left edge (plus the run-time image offset × *c*, normally 0) and repeats with a period of
   *iw* × *c* physical pixels, wrap addressing; the last partial tile is clipped.
3. **Clipped axis** (*iw* > *w*): scale *s* = *c*, drawn once from the left edge, clipped.

Because every frame piece starts at its element's physical edge and abutting elements share
physical edges (§3.6), frames made of corner, edge and middle elements (the data's form of
nine-slice art) stay closed and pixel-aligned at every scale, and a piece made for its element
never shows a wrapped extra column.

**Filter** (setting `ui_art_filter`, §7.6; default `sharp`), applied per axis with that axis's
*s*, on **premultiplied** texels (as UR §2.2, so transparent neighbours never darken an edge):

| *s* | `sharp` (default) | `smooth` | `nearest` |
|---|---|---|---|
| exactly 1 on both axes | exact copy: the result is byte-identical to the 1 : 1 drawing of UL §9.6 | same | same |
| an integer ≥ 2 (within 10⁻⁶) | each texel becomes an *s* × *s* block (nearest) | bilinear | nearest |
| above 1, not an integer | **sharp bilinear**: sample position *t* = (destination offset + ½) ÷ *s* − ½ in texels, *i* = ⌊*t*⌋, *φ* = *t* − *i*; the weight of texel *i* + 1 is clamp((*φ* − ½) × *s* + ½, 0, 1), of texel *i* one minus that. Each texel boundary turns into at most one blended pixel | bilinear (weight *φ*) | nearest |
| below 1 | **area**: each destination pixel averages the texels its footprint [*d* ÷ *s*, (*d* + 1) ÷ *s*) covers, partial texels weighted by the covered fraction | area | nearest |

After resampling, the result is converted back to straight alpha (colour ÷ alpha, rounded to
the nearest 8-bit value; colour 0 where alpha is 0) and composited with the draw mode
arithmetic of UL §9.6.1–§9.6.3 unchanged, pixel by pixel. The alpha test (UB §4.1 step 3)
still reads the **native** image at the unscaled point (§3.6), so hit results do not depend
on the filter.

**Mips and caching.** The area filter needs no mip chain. An implementation may keep
premultiplied mip levels or a cache of scaled copies (keyed by image, both scales, filter and
addressing) as an optimisation if the output stays within one step per channel of the rule
above; such caches count toward the decoded-image budget of §8.

**Coloured fills** (overlay colour fills, back colours) fill the physical box exactly; no
sampling is involved. **External content** (3D viewports and portraits, RD §6.10) is rendered
at the size of its physical box, so it is never resampled.

<a id="s6-4"></a>
### 6.4 Text

Text is laid out and drawn in physical pixels at the size it is shown, from glyphs chosen or
derived for that size (FO §6.3's 1 : 1-then-magnify rule is replaced for Tessera).

<a id="s6-4-1"></a>
#### 6.4.1 Target size

For a font record of line height *h* used by an element with content scale *c* (§3.4) and
text factor *x* (1.0 unless the optional text size of §7.5 applies), the **target height** is
*t* = ⌊*h* × *c* × *x* + ½⌋, at least 6. At *c* × *x* = 1 exactly, *t* = *h* and the font is
used as it is (invariant I7).

<a id="s6-4-2"></a>
#### 6.4.2 Font ladders

A **ladder** is the set of font records that differ only in size: the records whose plugin
`Font` enum names (LPA Appendix A.2, interoperability identifiers) are equal after removing
the trailing decimal digits. In this install (SC7) that gives:

| Ladder | Line heights |
|---|---|
| `TrajanPro` | 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26, 28 |
| `TrajanProBold` | 16, 22, 24, 25, 30, 36 |
| `BookAntiqua` | 12, 14, 16, 18, 20, 22, 24, 26, 28, 32, 36 |
| `BookAntiquaBold` | 12, 14, 18, 19, 22, 24 |
| `Verdana` | 10, 12, 14, 16, 18, 20, 22, 23 |
| `VerdanaBold`, `LucidaConsole`, `FixedSys`, `Arial` | one record each |

The ladder is built at run time from the enum and the records present (a record's line height
is read from the record, FO §3.2, not from its name). A font DID that the enum does not name
is a ladder of its own.

<a id="s6-4-3"></a>
#### 6.4.3 Glyph source for a target height

In this order:

1. **Exact.** A record of the same ladder with line height *t*: use it unchanged (its own
   glyphs, metrics and atlases).
2. **Downsampled.** Otherwise, if the ladder has a record taller than *t*, take the
   **shortest** record taller than *t* and derive every glyph by the area filter of §6.3
   (scale *k* = *t* ÷ its line height < 1) from its foreground atlas, and its outline atlas
   if it has one.
3. **Distance field.** Otherwise (*t* is taller than every record of the ladder), take the
   **tallest** record and derive glyphs with *k* = *t* ÷ its line height > 1 by the distance
   method of §6.4.4.
4. With `ui_text_source = "tessera"` (§7.1, optional), every game font is replaced by
   Tessera's bundled font (decision 0022) rasterised directly at *t* (FO §11 item 4 now
   allows the scaled size); layout then uses that font's metrics.

Tessera's own text (FO §11 item 2) is always rasterised directly at *t* by its rasteriser.

<a id="s6-4-4"></a>
#### 6.4.4 Deriving glyphs

**Metrics** of a derived glyph, from the source glyph's *A*, *W*, *B*, *T*, *H* (FO §3.2) and
*k*, with r(*v*) = ⌊*v* + ½⌋:

- left = r(*A* × *k*), right = r((*A* + *W*) × *k*), so *W*′ = right − left and *A*′ = left;
- advance = r((*A* + *W* + *B*) × *k*), *B*′ = advance − right;
- top = r(*T* × *k*), bottom = r((*T* + *H*) × *k*), *T*′ = top, *H*′ = bottom − top;
- line height *t*; baseline r(baseline × *k*), at most *t* − 1; outline borders *bx*′ =
  *by*′ = r(*bx* × *k*).

Rounding the edges, not the sizes, keeps neighbouring metrics consistent.

**Coverage, downsampled** (*k* < 1): the area filter of §6.3 on the coverage values, mapping
the source box [*X* … *X* + *W*) to the derived box.

**Coverage, distance method** (*k* > 1):

1. Pad the source glyph's coverage box with 4 source pixels of zero coverage on each side.
2. For each source texel, a signed distance in source pixels: inside when coverage ≥ ½. For
   a texel with coverage strictly between 0 and 1 the distance is coverage − ½; for any other
   texel it is the Euclidean distance from its centre to the centre of the nearest texel of the
   other class, minus ½, positive inside and negative outside; clamped to ±4.
3. For each derived pixel, take its centre back into source coordinates (÷ *k*), interpolate
   the distance bilinearly, multiply by *k*, and the coverage is clamp(that + ½, 0, 1).

The distance method keeps edges sharp (one blended pixel across an edge) at the cost of
slightly rounded corners; above *k* = 2 fine detail is lost (§13 OPEN). An implementation
may use another method if it meets invariants T1–T3 (§9.5).

**Outline.** A font with an outline atlas (FO §8.2) gets its outline coverage derived the
same way; a font without one keeps the stamped outline (FO §8.3) at radius max(1, round(*c*)).

<a id="s6-4-5"></a>
#### 6.4.5 Layout and measuring

Line layout, breaking, alignment, lists and truncation (FO §7, UB §10) run unchanged, in
physical pixels, with the chosen or derived metrics. Inputs in layout units (the element's
width, margins, indents, the bullet offset) are converted to physical pixels by §6.2. Glyph
positions stay whole pixels and glyphs are drawn 1 : 1 from the chosen or derived coverage
(FO §6.2), composited by FO §8.4 unchanged.

Where a rule sizes or places an element from a text extent (tooltip size, UB §6.4; list rows
sized by text; scroll ranges of text, UW §5), the extent in UI units is the physical extent ÷
*c*, rounded up. Because derived metrics are rounded per glyph, a line may break at another
word than at *c* = 1; this is accepted (invariant I7 guarantees identity only at *c* = 1).

<a id="s6-4-6"></a>
#### 6.4.6 Text hit tests

A text element's hit tests (FO §6.3 item 3: character, link, selection) take the pointer's
physical pixel, subtract the element's physical box origin, add the scroll offsets converted
to physical pixels (§6.2) and look the point up in the physical layout. No division by a
scale is needed.

<a id="s6-5"></a>
### 6.5 Glyph cache

Glyphs are derived on demand, one glyph at a time, and cached by (font DID, target height,
source kind, outline or fill). The cache is bounded (it counts toward the decoded-image LRU of
§8) and evicts least recently used heights. Deriving counts toward the frame's paint slice
(UR §6 U4 (b)); a client may derive the printable ASCII range of the fonts on screen on the
asset workers ahead of a settle (§4.5 step 4).

<a id="s6-6"></a>
### 6.6 Composition (RD §6)

The renderer's contract does not change. Each composition entry's destination is the physical
box of its surface's displayed box (with the stretch vector), so:

- in the normal case the canvas has exactly the size of the destination and the entry takes
  the exact 1 : 1 path of RD §6.6 (no filtering at all);
- an entry is scaled only during a scale-change preview (§4.5 step 3), for a stretch vector
  (§3.4), and when rotated (RD §6.6); these use `ui_scale_filter` as today;
- Tile entries repeat their canvas at its physical size (1 : 1).

<a id="s6-7"></a>
### 6.7 Cursor

The cursor image is built at trunc(32 × *Se* × `ui_scale_cursor`) physical pixels square
(`ui_scale_cursor` keeps UB §9.3's range rule), resampled from its native image by §6.3 with
a fitted axis, and the hot spot is scaled by the same factor and rounded. This replaces the
size rule of UB §9.3 and UR §2.5 for Tessera (they applied no OS or UI factor). The software
cursor of RD §6.11 is drawn at that size.

---

<a id="s7"></a>
## 7. Options and the Interface window

<a id="s7-1"></a>
### 7.1 Options

| Option | Settings key (`[UI]`) | Values | Default | Status |
|---|---|---|---|---|
| UI scale | `ui_scale` | `"auto"`, 0.5–4.0 step 0.05 | `"auto"` | **Required** (§4) |
| Safe-area width | `ui_safe_aspect` | `"4:3"`, `"16:10"`, `"16:9"`, `"21:9"`, `"full"`, or a ratio 1.333–4.0 | `"21:9"` | **Required** (§5.1) |
| Saved placements | `[UI.panels]` | §5.8 | empty | **Required** (§5.8) |
| Reset panel positions | — (a button) | removes every `[UI.panels]` entry and lays the roots out again | — | **Required** (a way back from any placement) |
| Art filter | `ui_art_filter` | `"sharp"`, `"smooth"`, `"nearest"` | `"sharp"` | Optional (§7.6) |
| Composition filter | `ui_scale_filter` (exists) | `"linear"`, `"nearest"` | `"linear"` | exists; now used only for previews, stretch and rotation (§6.6) |
| Text size | `ui_text_scale` | 0.80–1.50 step 0.05 | 1.00 | Optional (§7.5) |
| Text source | `ui_text_source` | `"game"`, `"tessera"` | `"game"` | Optional (§6.4.3 item 4) |
| Per-window scale | `[UI.group_scale]` | §7.4 | empty | Optional (§7.4) |
| Cursor scale | `ui_scale_cursor` (exists) | as now | 1.0 | exists; now a factor on top of *Se* (§6.7) |

Not offered, with reasons: a separate pre-game scale (the original's `UIScale_Pregame`): the
pre-game screens are full-window screens that follow the window (§5.4); importing the
original's `UserPreferences.ini` scales: they mean something else in Tessera (§7.7, §11).

<a id="s7-2"></a>
### 7.2 The Interface window (required)

The original's Options panel (UL §17.2 P7) is shown with its own content only, because its
pages are built by game code and the templates are not yet named (UL §17.5 OPEN (d)). The scale
controls therefore live in **Tessera's own Interface window**:

- **How it is built.** A root window made of the game-styled controls of LPA §14.2 (window
  frame with title and close box, labels, a horizontal slider-mode scroll bar of UW §6, check
  boxes and buttons), from the user's own layout data, with Tessera's neutral art when the data
  are missing (LPA §14.2.6). It is a safe-area panel with edge mode 0 at the safe area's centre
  on first opening, and saves its placement (§5.8).
- **Rows** (strings are Tessera's own, English; not game text):

| Row | Control | Text | Behaviour |
|---|---|---|---|
| 1 | label | "Interface scale" | — |
| 2 | slider, 71 stops (UW §6.5) | value label "150 %"; with auto: "Auto (150 %)" | moving it sets *S* live (§4.5); keyboard steps of 0.05 through the slider's arrow buttons |
| 3 | check box | "Automatic" | on: *S* = auto, the slider shows *S*auto and is ghosted |
| 4 | label, shown only when limited (§4.3) | "Limited to 94 % by the window size" (the number is 100 × *Se*, rounded) | — |
| 5 | label | "Wide-screen area" | — |
| 6 | five buttons as a choice group | "4:3", "16:10", "16:9", "21:9", "Full width" | sets `ui_safe_aspect`; a stored custom ratio shows none selected |
| 7 | button | "Reset panel positions" | §7.1 |
| 8 (optional) | slider | "Text size" with "100 %" | §7.5 |
| 9 (optional) | choice group | "Art: Sharp / Smooth / Pixel" | §7.6 |
| 10 (optional) | button | "Reset window scales" | clears `[UI.group_scale]` |

- **Where it is reached:** the chat command `/uiscale` (§7.3) with no argument, the action
  `ui_interface_window` (§7.3), and, once UL §17.5 OPEN (b) binds menu buttons, the main menu.

<a id="s7-3"></a>
### 7.3 Command, actions, command line

- **Chat command** `uiscale`, a client built-in of the decision-0020 dispatcher ("Client
  options"): no argument opens the Interface window; `auto` sets *S* to auto; a number with or
  without `%` (`1.5`, `150`, `150%`; a number above 4 is read as a percentage) sets *S*;
  `+` and `-` step by 0.05; `aspect <value>` sets `ui_safe_aspect`; anything else posts a
  usage line. It works offline and online alike and never sends anything.
- **Actions** (Tessera actions in KM's `global` context, **unbound** by default because the
  free keys are taken: KM §8.2 binds Minus and Equal with every modifier):
  `ui_scale_up` (+0.05), `ui_scale_down` (−0.05), `ui_scale_auto`, `ui_interface_window`.
  KM lists them when this spec is built (§12).
- **Command line** of `tessera-client`: `--ui-scale <auto|N>` and
  `--ui-safe-aspect <value>` override the settings for the run without saving them (for
  goldens and checklists, §9).

<a id="s7-4"></a>
### 7.4 Per-window scale (optional)

The original lets a player scale panel groups (UB §9.1, §9.2). Tessera's optional form:

- A native root panel with `UIComponent_UIScaler_Group` *g* (LPA Appendix A.7) takes the
  per-window factor *p* = `[UI.group_scale]` entry *g* (a table from group number to factor),
  1.0 when absent. Range: the group's `UIScaleDefinition_Min`/`Max` from the data (UB §9.1),
  intersected with 0.5–2.0; steps of 0.05.
- *p* is a uniform node scale (§3.4): the panel's layout box keeps its size, its children are
  not laid out again, its content is rasterised at *Se* × *p* (crisp), and the origin rule
  (UB §9.2 step 4, origin from `UIComponent_UIScaler_Origin_X/_Y`) keeps the origin point in
  place in UI units; then §5.6 and keep-on-screen.
- Gesture: the action `ui_window_scale_up` / `_down` over a panel (unbound; a suggested binding
  is Ctrl + wheel, subject to KM, §13), and row 10 of the Interface window to reset.

<a id="s7-5"></a>
### 7.5 Text size (optional)

`ui_text_scale` *x* (§6.4.1) enlarges text without enlarging boxes. To avoid truncating fixed
labels it applies only to text that can reflow or scroll: tooltip text (tooltips size
themselves to their text, UB §6.4), the chat window's log and entry line, and Text elements
that are multi-line (`UICore_Text_one_line` false) and inside a Scrollable (UW §5). All other
text uses *x* = 1.

<a id="s7-6"></a>
### 7.6 Art filter (optional)

`ui_art_filter` selects the column of §6.3: `sharp` (default), `smooth` (for users who prefer
soft art) or `nearest` (pixel art). It applies to painting; `ui_scale_filter` keeps applying
to GPU-scaled composition (§6.6).

<a id="s7-7"></a>
### 7.7 Plugin API

- **Coordinates** (LPA §13.1): plugin "pixels" are UI units. `Display.GetWidth()`,
  `GetHeight()`, `GetSize()` return *Wu*, *Hu*; `GetMouseX/Y/Position()` return ⌊*ux*⌋,
  ⌊*uy*⌋ (§3.6). A window's position is relative to *F*'s top-left (not to the safe area).
  Control positions are relative to their parent's layout box as before.
- **Under a node scale** (a window's `SetScale`, a per-window or fit-to-screen factor):
  `PointToScreen` returns ⌊displayed position⌋ of the point (§3.4); `PointToClient` and
  control-relative `GetMousePosition` return ⌊(*u* − displayed left) ÷ *m*⌋ per axis. With
  every node scale 1 these are the integer results of today.
- **The global factor is 1 for plugins.** Tessera's UI scale is applied below the UI-unit
  space, so plugins never see it except as the `Display` size. In LPA §13.21 rules 3, 4 and 6
  and in UB §9.2 step 1, `UIScale_Global` is 1.0; `GetUIGroupScale` for the `Global` group
  returns 1.0; a change of *S* or *Se* fires no `UIScaleChanged`. A plugin that multiplied its
  sizes by a global scale therefore does not scale twice.
- **`Window:SetScale(f)`** sets the window's per-window factor *p* = *f* (a uniform node scale,
  crisp; LPA §13.21 rule 5 with UB §9.2 steps 2–5 in UI units, step 6 replaced by the node
  scale: the window's layout box and children are unchanged, which also settles the rule-1
  question of LPA §13.21 for Tessera). `GetScale()` returns *f*.
- **Group values.** With the optional §7.4, a marked window of group *g* takes *p* = the group's
  value (LPA §13.21 rule 2) and `GetUIGroupScale(g)` returns it; `UIScaleChanged` fires as
  LPA §13.21 rule 8 says when a group value changes.
- **Display changes.** When *F* changes, plugin windows that were inside *F* are kept on screen
  (§5.8 item 5); the plugin sees the moves through its ordinary `PositionChanged` events.

---

<a id="s8"></a>
## 8. Performance budgets

UR §6's budgets U1–U5 stay as they are at content scale 1. At larger scales the CPU work
grows with **physical** pixels (about *c*² per surface), while composition becomes cheaper
(most entries take the 1 : 1 path). Reference figures (UR §6 probe: 1–4 ns per pixel per
layer, an overdraw of about 3):

| Quantity | *c* = 1 | *c* = 1.5 | *c* = 2 | *c* = 3 |
|---|---|---|---|---|
| HUD set (UL §17 P1–P5, P11, P14; 0.72 M layout pixels) | 0.72 Mpx | 1.61 Mpx | 2.86 Mpx | 6.44 Mpx |
| Two windows (P7, P9; 0.73 M layout pixels) | 0.73 Mpx | 1.64 Mpx | 2.93 Mpx | 6.58 Mpx |
| Screen-sized plugin window at 3840 × 2160 | 8.29 Mpx at every scale | | | |
| Full repaint of all of the above, 3 × 1–4 ns per pixel | 29–117 ms | 35–138 ms | 42–169 ms | — |
| Minimap damage while walking (170 × 170 layout pixels) | 29 kpx | 65 kpx | 116 kpx | 260 kpx |

Budgets (reference machine, decision 0010; measured with UR §6's scripted run at the named
configurations C5, C6 of §9.2 in addition to its own):

| # | Measure | Budget |
|---|---|---|
| S1 | U1 (UI main-thread time, steady state) | unchanged at every scale: mean ≤ 0.3 ms, p99 ≤ 1.0 ms. Where the damage of a frame exceeds 30,000 physical pixels (the minimap at *c* > 1), its paint runs on the asset workers (UR §6 "Workers if needed"), so the main thread stays inside U1 |
| S2 | U2 (GPU time of the UI pass) | ≤ 0.3 ms at 3840 × 2160 (unchanged); indicative ≤ 1.2 ms at 7680 × 4320 |
| S3 | U3 (upload bytes per frame) | steady-state mean ≤ 256 KiB × max(1, *Se*²); hard cap 8 MiB × max(1, *Wp* × *Hp* ÷ 8,294,400) |
| S4 | Scale change (§4.5) at 3840 × 2160 | layout and preview in the frame of the change; every surface's new picture swapped in within 60 frames of the settle; U4 (a) (no frame over 50 ms) and U4 (b) (paint ≤ 4 ms per frame on the main thread) hold throughout; no blank surface (I9) |
| S5 | U4 (c)–(e) in physical pixels | (c) unchanged for surfaces up to 1 Mpx physical; (d) a job of a surface up to 0.83 Mpx physical completes within 4 frames, larger ones within 4 frames per 0.83 Mpx; (e) unchanged (it is already physical) |
| S6 | U5 (memory) | canvases ≤ 128 MiB and GPU surface textures ≤ 256 MiB, each × max(1, *Wp* × *Hp* ÷ 8,294,400); decoded images, scaled-image and glyph caches together ≤ 64 MiB × max(1, *Se*²) unpinned; pinned unchanged. The settings `canvas_mib`, `texture_mib`, `image_lru_mib` override these defaults as today |
| S7 | Glyph derivation | ≤ 20 µs per glyph on the main thread (it counts toward the slice); a whole ladder step for the printable ASCII range ≤ 5 ms on a worker |

Why S6 scales with the window and not with *Se*: the UI screen's area times *Se*² is the
window's physical area, so the physical pixels the UI can cover are bounded by the window, at
any scale.

> **OPEN (owner):** S1–S7 are proposed budgets for this work's exit, not M5 criteria. The
> owner accepts or changes them with this spec's review. They are first measured indicatively
> on the loaded reference machine and re-measured idle, as UR §6's are.

---

<a id="s9"></a>
## 9. Golden images, migration and tests

<a id="s9-1"></a>
### 9.1 Migration of existing goldens

- Every committed golden (SC12) is painted at content scale 1 on a UI screen of at least
  1024 × 768: the panel goldens of `crates/ui` at 1 : 1, the client UI goldens at 1024 × 768.
  At *Se* = 1, with no fit factor other than 1, §6.3 and §6.4 are the exact 1 : 1 rules
  (invariants I2, I7), so these goldens stay valid **provided the tests pin the scale**: a
  test that builds a UI sets *S* = 1.0 explicitly, never `auto` (auto reads the OS factor).
- Tests whose screen size changes meaning: a UI test at a size below 1024 × 768 or with a
  height below 768 now runs at *Se* < 1 (the design floor). None is committed; the client's
  default `--size 1280x720` gives *Se* = 0.9375, so local real-data goldens and checklist
  screenshots taken at 1280 × 720 change and are re-blessed (their decision-0031 tier-2 names
  gain the configuration, §9.4).
- Goldens that include a fit container at a size other than its design size (the loading
  screen, the character screens) change by design. The local loading-screen golden that
  captured a blank root surface (CLAUDE.md P5 open item) is replaced by C1–C9 captures of
  §9.4.
- `render` and `terrain` goldens draw no game UI and do not change.

<a id="s9-2"></a>
### 9.2 Named configurations

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
| C11 | 7680 × 4320 | 3.0 | 21:9 | 16:9 | 3.0 | 2560 × 1440 (headless only) |
| C12 | 3840 × 2160 | 1.0 | 21:9 | 16:9 | 1.0 | 3840 × 2160 |

<a id="s9-3"></a>
### 9.3 Committed synthetic goldens (decision 0031 tier 1)

Synthetic layouts and art generated by the tests, Tessera's fallback widgets, a **synthetic
bitmap font ladder** (font records in FO §3 form generated by the test from the bundled font
at heights 10, 12, 14, 16 and 20), painted by the CPU painter (exact comparison, one step per
channel) and, where composition is under test, by the headless renderer:

| Golden | Content | Configurations |
|---|---|---|
| G1 frame | a nine-element frame with 2-pixel tiled edges, 16 × 16 corners, a fitted title bar | C1, C2, C3, C5, C6 |
| G2 text | the same paragraph at the ladder's exact, downsampled and distance-derived heights | *c* = 1, 1.25, 1.5, 2, 3 |
| G3 fit A | a 2560 × 1080 full-cover fit container with a centred-bottom content box | C1, C2, C7, C8, C9, C10 |
| G4 fit B | a 1440 × 900 root with left and right side panels | C1, C7, C8, C10 |
| G5 HUD | four anchored panels (modes 4/5, 1 and 0) | C2, C8, C9 |
| G6 too large | a 1100 × 800 panel (fit to screen) | C1 |
| G7 preview | a frame in the middle of a scale change (old canvases composed scaled) | C2 → C3 |

<a id="s9-4"></a>
### 9.4 Local real-data goldens (decision 0031 tier 2)

With `TESSERA_GAME_DIR`, named `<subject>@<window>@<scale>[@<aspect>]` (decision 0031:
"the choice is part of `<name>`"):

| Subject | Configurations |
|---|---|
| Loading screen (TW §5.2) | C1, C2, C5, C7, C8, C10, C12 |
| Character appearance screen (CS §2.2) | C1, C2, C5, C7, C8, C10 |
| HUD set (UL §17 P1–P5, P11, P14) | C1, C2, C4, C5, C6, C8, C9 |
| World map panel P10 (fit to screen at C1) | C1, C5 |
| Interface window (§7.2) | C1, C5 |

<a id="s9-5"></a>
### 9.5 Invariants

Each is testable on synthetic layouts at random configurations (property tests) and on the
install's layouts at C1–C12:

- **I1** *Wu* ≥ 1024, *Hu* ≥ 768 and *Se* ≤ *S* for every window size ≥ 1 × 1 and every *S*.
- **I2** At *Se* = 1 with every node scale 1, the painter's canvas of every surface is
  byte-identical to the canvas painted before this spec (the existing goldens pass).
- **I3** For every element and every physical pixel: the pixel lies in the element's physical
  box ⇔ the hit test at the pixel's centre is inside its displayed box (§3.6).
- **I4** Sibling elements without node scales of their own whose layout boxes abut, abut in
  physical pixels (§3.6 consequence 2).
- **I5** After layout, every element's layout box has width ≥ 1 and height ≥ 1 UI unit.
- **I6** After any of the events of §5.8 items 1–4, every shown root panel's displayed box
  lies inside *F*.
- **I7** At *c* × *x* = 1 every glyph comes from the font's own record; at a height with an
  exact ladder record, from that record.
- **I8** A saved placement applied at the configuration where it was saved restores the same
  displayed position (within 0.01 UI units); applied at another configuration, the distance
  from the anchored edge in UI units is unchanged unless keep-on-screen moved the panel.
- **I9** During a scale change no frame shows a surface without a picture that it showed
  before the change.
- **I10** *S*auto follows the table of §4.2 for the listed inputs.
- **I11** With safe aspect *a*, every safe-area panel laid out by edges (not moved by the
  user) has its displayed box inside *A* when it fits in *A* at its design size.
- **I12** For a safe-area panel not limited by §4.3 or §5.6, its physical size does not
  decrease when *S* increases.
- **I13** Plugin `Display.GetSize()` equals (*Wu*, *Hu*), and `GetUIGroupScale(Global)` is 1.0.
- **T1** A derived glyph that is a solid rectangle in the source stays a solid rectangle with
  at most one partial pixel row or column per edge.
- **T2** A derived glyph's ink box is within one pixel of the source ink box scaled by *k*.
- **T3** Downsampled glyphs keep total coverage within 1 % of the source total × *k*².

<a id="s9-6"></a>
### 9.6 Other tests

- **Fuzz:** the settings keys of §4.4 and §7.1 (`settings_ui` target: any TOML value, no
  panic, defaults and warnings); a configuration target (any *Wp*, *Hp* in 1–16,384, any *S*,
  any aspect) over synthetic layouts, asserting I1, I5, I6 and no panic.
- **Unit:** the pixel-centre mapping at *K* = 0.937729, 1, 1.5, 2.25, 3 (I3, I4); the Type A,
  B and C fits with the numbers of §10; the anchors of §5.8; the `/uiscale` argument forms.
- **Client:** `--ui-scale` and `--ui-safe-aspect` reach the UI; a scripted scale change
  measures S4.

---

<a id="s10"></a>
## 10. Worked examples

All UI-unit values to two decimals; physical boxes as (left, top, width, height) by §3.6.
Defaults otherwise (`ui_safe_aspect` 21:9, no text size, no per-window scale).

<a id="s10-1"></a>
### 10.1 Loading screen (SC6)

`LoadScreenMain` is a full-screen root (§5.2): its box is *F*. `LoadScreenImage` is a Type A
fit container (*w* × *h* = 2560 × 1080) whose content box is `NormalBackground`
(1024 × 768 at (768, 312)); *G* = *A*.

| Window, *S* | *Se*, *F* | *f*contain / *f*cover / *f*guard → *f* | Image box (UI) | *c* | Meter, physical | Label text |
|---|---|---|---|---|---|---|
| 1280 × 720, 1.0 or 1.5 | 0.9375, 1365 × 768 | 0.5332 / 0.7111 / 1.0 → 0.7111 | (−227.72, 0, 1820.44 × 768) | 0.667 | (572, 698, 133, 9) | *t* = 8: downsampled from `Verdana10` |
| 3440 × 1440, 1.0 | 1.0, 3440 × 1440 | 1.3333 / 1.3438 / 1.875 → 1.3438 | (0, −11.25, 3440 × 1451.25): centred, then moved up 5.63 so the content's bottom meets *G* | 1.344 | (1583, 1396, 267, 18) | *t* = 16: exact `Verdana16` |
| 3440 × 1440, 1.5 | 1.5, 2293 × 960 | 0.8889 / 0.8957 / 1.25 → 0.8957 | (0, −7.36, 2293 × 967.36) | 1.344 | (1583, 1396, 267, 18) | *t* = 16 |
| 3840 × 2160, 1.0 | 1.0, 3840 × 2160 | 1.5 / 2.0 / 2.8125 → 2.0 | (−640, 0, 5120 × 2160) | 2.0 | (1716, 2094, 398, 28) | *t* = 24: distance-derived from `Verdana23` |
| 3840 × 2160, 1.5 | 1.5, 2560 × 1440 | 1.0 / 1.3333 / 1.875 → 1.3333 | (−426.67, 0, 3413.33 × 1440) | 2.0 | (1716, 2094, 398, 28) | *t* = 24 |
| 5120 × 1440, 1.0 (C8) | 1.0, 5120 × 1440, *A* 3456 at 832 | 1.3333 / 2.0 / 1.875 → 1.875 | (160, −585, 4800 × 2025): content fills *A*'s height; 160-unit black bars at the sides | 1.875 | (2369, 1378, 373, 26) | *t* = 23: exact `Verdana23` |

The meter and label keep their design places inside `NormalBackground` at every size, so the
stop-gap of UL §8.4 is not needed. At 3840 × 2160 the scale 1.0 and 1.5 rows give the same
picture: the loading screen follows the window (§5.4).

<a id="s10-2"></a>
### 10.2 Character appearance screen side panels (SC6)

The root is full-screen; the side panels are Type B fit containers laid out in the root's safe
box. *f* = min(*Hu* ÷ 900, *Aw* ÷ 500); the left panel is anchored at the safe box's left, the
right panel ends at its right.

| Window, *S* | *Se*, *F* | *f* | *c* | Left panel (UI) → physical | Right panel (UI) → physical | `CharGen_Appearance_Panel`, physical |
|---|---|---|---|---|---|---|
| 1280 × 720, 1.0 or 1.5 | 0.9375, 1365 × 768 | 0.8533 | 0.8 | 0 … 426.67 → 0 … 400 | 938.33 … 1365 → 880 … 1280 | (1029, 35, 224, 653): inside the window (UL §8.4 and CS §2.2 report it 65 pixels beyond the edge today) |
| 3440 × 1440, 1.0 | 1.0, 3440 × 1440 | 1.6 | 1.6 | 0 … 800 | 2640 … 3440 | (2938, 70, 448, 1306) |
| 3440 × 1440, 1.5 | 1.5, 2293 × 960 | 1.0667 | 1.6 | 0 … 533.33 → 0 … 800 | 1759.67 … 2293 → 2640 … 3440 | (2938, 70, 448, 1306) |
| 3840 × 2160, 1.0 | 1.0, 3840 × 2160 | 2.4 | 2.4 | 0 … 1200 | 2640 … 3840 | (3086, 106, 672, 1958) |
| 3840 × 2160, 1.5 | 1.5, 2560 × 1440 | 1.6 | 2.4 | 0 … 800 → 0 … 1200 | 1760 … 2560 → 2640 … 3840 | (3086, 106, 672, 1958) |
| 5120 × 1440, 1.0 (C8) | 1.0, 5120 × 1440 | 1.6 | 1.6 | 832 … 1632 | 3488 … 4288 | (3786, 70, 448, 1306) |

Text in the panels uses *t* = ⌊*h* × *c* + ½⌋: for example a 16-pixel bold label is 13 pixels at
*c* = 0.8 (downsampled; its ladder has only 16), 26 at 1.6 and 38 at 2.4 (distance-derived).

<a id="s10-3"></a>
### 10.3 HUD on a 32:9 screen (C8)

*F* = 5120 × 1440, *A* = 3456 wide at 832. The toolbar (UL §17.2 P3, 1024 × 200, edges
(1, 4, 1, 5)) is laid out in *N* = 3456 × 1440: left = half(3456) − half(1024) = 1216, moved by
832 → 2048, top = 568 + 672 = 1240: centred on the screen at the bottom. The minimap (P1,
224 × 280 at (822, 12), edges (4, 5, 5, 4)): left = 822 + 2432 = 3254 → 4086 on screen, right
edge 4310 (its design box reaches 22 units beyond the design width, UL §17.3), well inside
*F*. With `ui_safe_aspect = "full"` (C9) the minimap's left would be 822 + 4096 = 4918 and its
right edge 5142, beyond *F*: keep-on-screen moves it to 4896.

<a id="s10-4"></a>
### 10.4 Saved placement across a scale change

A quest log saved at C2 (*F* = *A* = 1920 × 1080) with its displayed box at (1500, 40,
400 × 500): its centre (1700, 290) lies in the right third and the top third, so `ax` =
`right`, `dx` = 1920 − 1900 = 20; `ay` = `top`, `dy` = 40. At C3 (*F* = *A* = 1536 × 864) it
is restored at left 1536 − 20 − 400 = 1116, top 40 (physical (1395, 50, 500, 625) at
*Se* = 1.25): still 20 units from the right edge and 40 from the top.

<a id="s10-5"></a>
### 10.5 Pixel-centre mapping at C1

*Kx* = 1280 ÷ 1365 ≈ 0.937729. A displayed left edge at 609.97 maps to column
⌈609.97 × 0.937729 − ½⌉ = ⌈571.49⌉ = 572; the pointer in column 571 has
*ux* = 571.5 ÷ 0.937729 ≈ 609.45 < 609.97 (outside), in column 572 *ux* ≈ 610.52 (inside).

---

<a id="s11"></a>
## 11. Deviation from the original (for the P8 parity checklist)

Owner-accepted deviation, decision 0040 §1: Tessera's UI sizing and placement differ from the
original on purpose:

- one global UI scale applied under the UI-unit space, with crisp text and art, instead of
  per-panel magnification (UB §9.2, FO §6.3);
- a 1024 × 768 design floor, a safe area on wide screens, fit containers with a uniform
  factor and a cover fit, backdrop images covered instead of tiled, panels fitted to and kept
  on the screen, anchored saved placements;
- the original's `UIScale_Global`, `UIScale_Pregame` and `GOP_UIScale_*` preferences are not
  read; plugins see a global factor of 1.0.

Layout data, element trees, art, fonts and their metrics at scale 1 are the original's.

---

<a id="s12"></a>
## 12. Notes for other specs

Added as short notes in this revision (no rule of those specs changed; each takes effect for
Tessera when this spec is Accepted and built):

| Spec | Section | Note |
|---|---|---|
| `ui-layout.md` | §8.4 step 6 OPEN, §8.5 Tessera rule, §9.6, §9.9 | answered and superseded by §5, §6.3, §5.8 |
| `ui-behaviour.md` | §8.3 OPEN, §8.4, §9 (Tessera rules of §9.2, §9.2.1, §9.3, §9.4) | §5.8, §3.4, §6.7, §4.5 |
| `render.md` | §6.6, §6.16 (scale filter OPEN) | §6.6: destinations from §3.6; the 1 : 1 path is the normal case |
| `lua-plugin-api.md` | §13.1, §13.10, §13.21 | §7.7 |
| `fonts.md` | §6.3 Tessera Decision, §11 item 4 | §6.4 |
| `character-screens.md` | §2.2 implementer OPEN | §5.4 Type B, §10.2 |

When this spec is built, `keymap.md` §8 gains the actions of §7.3 and §7.4 (unbound), and the
settings module's documentation lists the keys of §7.1.

---

<a id="s13"></a>
## 13. Open questions

> **OPEN (owner):** the default safe aspect (21:9 as 2.4) and the auto-scale formula of §4.2
> (one UI unit per pixel up to 1440 rows, then height ÷ 1440, at least the OS factor) are
> design choices to confirm by use on the owner's screens.

> **OPEN (owner):** whether full-window screens should follow the slider at all. This spec
> says no (§5.4: they follow the window; the text size reaches their reflowing text). A
> "scale full-window screens" option could multiply *f* by *S* ÷ *S*auto, at the cost of
> cropping their content on small windows.

> **OPEN:** derived glyphs above *k* = 2 (for example `Arial12` or `LucidaConsole12` at
> *c* = 3) lose fine detail with the distance method. Whether such text should switch to the
> bundled font automatically above a threshold, or stay as derived, is for the owner to judge
> from goldens G2 and the C11 captures.

> **OPEN:** default key bindings for `ui_interface_window`, `ui_scale_up`, `ui_scale_down`,
> `ui_scale_auto` and the per-window actions; none is bound because the obvious keys are
> taken (KM §8.2). Ctrl + wheel for the per-window scale must first be checked against KM's
> wheel bindings.

> **OPEN:** once UL §17.5 OPEN (d) names the Options panel's templates and pages, the
> Interface window's rows may move onto a page of P7; until then the window of §7.2 is
> the place.

> **OPEN:** ladders rest on the plugin enum names (§6.4.2). Installs with other language
> archives (FO §3.1 OPEN) may hold other fonts; any font the enum does not name is its own
> ladder and is derived from itself.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-16 | First version (status Review), for decision 0040: coordinate spaces and the pixel-centre rule (§3), UI scale with auto default, design floor, storage and live change (§4), reference boxes, safe area, full-screen roots, fit containers, backdrop images, fit to screen, degenerate-box guard, keep-on-screen and anchored placements (§5), canvases, images, derived glyphs, composition and cursor at scale (§6), options and the Interface window (§7), budgets S1–S7 (§8), goldens, migration and invariants (§9), worked examples (§10), deviation note (§11), notes for other specs (§12), open questions (§13). Census rows SC1–SC6 from a read-only probe of the install's layout records. |
