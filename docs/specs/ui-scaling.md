<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI scaling: coordinate spaces, the UI scale, fitting layouts to any window, crisp text and art

| | |
|---|---|
| **Status** | Accepted (revision 5, 2026-10-16: an independent re-review by a session that wrote no part of it passed with no blocking item; RV4-B1 and RV4-N1 verified against the install. Implementers may build from it; the `ui-layout.md` §8.4 stop-gap stays in force until this spec is built, §5.9) |
| **Last updated** | 2026-10-16 |
| **Author role** | Spec writer (dirty room). A Tessera **design** spec for [decision 0040](../decisions/0040-ui-scaling.md), like `tessera-protocol.md`; the only install input is a read-only census of the layout records (§1, rows SC1–SC6, SC13, SC14) |
| **Reviewer** | First review 2026-10-16, independent review session: changes requested (B1–B6, answered in revision 2). Second review 2026-10-16: changes requested (R2-B1, R2-B2, answered in revision 3). Third review 2026-10-16: changes requested (RV3-B1–RV3-B3, answered in revision 4). Fourth review 2026-10-16: changes requested (RV4-B1 blocking, RV4-N1 non-blocking, answered in revision 5). Independent re-review 2026-10-16 (a session that wrote no part of the spec): **passed**, no blocking item. RV4-B1 verified with a read-only `tessera-formats` probe: `MPMissionController_MainWindow` is 1440 × 900 at (1, 0) in a 1440 × 900 design area, edges (5, 5, 5, 5), object mode None, no dialog-framework PID in the record → safe-area panel; the §10.8(a) C8 arithmetic, I18 and the §9.4 C8 golden are mutually consistent and C1/C2/C5 are unaffected (*A* = *F*). RV4-N1 verified: the theatre progress fit root's single child is 247 × 49 at (411, 735), its §10.6 X extent (247 at 411) unchanged. Accepted |
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
| — | Everything not listed below: coordinate spaces, the UI scale and its default, the design floor, the safe area, fit containers, fit groups and the magnification bound, the backdrop rule, keep-on-screen, saved placements, the pixel-centre mapping, image and text resampling, glyph derivation, options, the Interface window, budgets at scale, test plan | This spec, 2026-10-16, for decision 0040. No observation of the original's scaling was used, by the decision's rule | Design |
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
| SC13 | Type A fit containers (both horizontal edges 6): 17, all (6, 5, 6, 5) Stretch. Under the grouping rule of §5.4 they form 14 fit groups: three of two members (the theatre layout's two fit roots `StoryField` and `Theater_LoadingProgress_Parent`, both 1024 × 768 at (0, 0); the map layout's `MapDecoration`, 1168 × 768 at (−73, 0) in the root `MapWindow`, with `MapBackground`, 1024 × 768 at (0, 0) in `MapField`, which fills `MapWindow` with edges (5, 5, 5, 5); the mission-controller layout's two scale parents, one inside a naming field that fills their common parent with edges (5, 5, 5, 5)) and 11 of one. No Type A container lies inside another. Content boxes used in §10: `StoryField` covers its whole design area; `Theater_LoadingProgress_Parent` has one child, 247 × 49 at (411, 735); the third fit root, `BaseTutorialChooserField` (layout 1440 × 900), has one child, 735 × 231 at (224, 0). **One fit container lies inside another** (the only one): the Type C `MPMissionController_NamingWindow` (1, 1, 1, 6), Stretch, 411 × 488 at (516, 206), is a direct child of the Type A `MPMissionController_NamingScaleParent`. Mission-controller layout (1440 × 900): root `MPMissionController_MainWindow` (5, 5, 5, 5), object mode None, 1440 × 900 at (1, 0) (so it does **not** cover its design area, §5.2; a safe-area panel, not a full-screen root, and the record holds none of the dialog-framework properties, so not modal either) → `MPMissionController_Window` (5, 5, 5, 5), 1440 × 900 at (0, 0), holding `MPMissionController_NamingField` (5, 5, 5, 5), 1440 × 900 at (0, 0) → `MPMissionController_NamingScaleParent`, and `MPMissionController_ScaleParent`; both scale parents 1440 × 900 at (0, 0). Their direct children (content of §10.8): the naming window and a 265 × 86 field at (610, 0); in `ScaleParent` 100 × 100 at (671, 780) and 257 × 80 at (590, 700) | Probe of SC1 (every Type A container and every pair in one layout checked against the grouping rule; ancestor chains of all 26 mode-6 elements listed; the mission-controller root's rectangle, and the whole mission-controller record scanned for the dialog-framework PIDs, re-probed 2026-10-16) | Confirmed 2026-10-16 |
| SC14 | Drag bars and resize bars (UB §8.1, §8.2): 134 and 49 in the 301 decoded records; none has a fit container as its target (UB §8.1 step 1, §8.2 step 2) or lies inside a fit container. The 302nd record holds no element with an edge in mode 6 (SC2), hence no fit container, so this holds for the whole install | Probe of SC1, 2026-10-16 (the UB census counts 135 and 52 over all records) | Confirmed 2026-10-16 |

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
| §2.3 | Ultrawide: HUD anchored to a configurable safe area, user picks its width; nothing stretched across the full width | §5.1, §5.2, §5.3, §7.1 (`ui_safe_aspect`); full-screen art: the magnification bound of §5.4 (factors follow the height, not the width), §10.7, invariant I17 |
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

<a id="s3-2-zero"></a>
**A window with a zero side.** *Se*, *F*, *Kx* and *Ky* are computed only from sizes with
*Wp* ≥ 1 and *Hp* ≥ 1. While the window's inner size has a zero side (a minimised window, on
some platforms), **nothing changes**: *Se*, *F*, *Kx*, *Ky*, the safe area *A* (§5.1), every
layout box, fit factor, fit-to-screen factor, displayed box and saved placement keep the
values they had at the last size with both sides non-zero, and none of §4.5 runs (no layout,
no keep-on-screen, no saved placement applied again, no `Display` change for plugins, no
repaint at a new content scale). Nothing is drawn, and pointer events that arrive in that
time are ignored. Rules:

1. The UI host never replaces a zero side by a fallback size (such as 1 × 1, or a default
   window size): laying out at a fallback screen would run keep-on-screen (§5.8 items 1, 3
   and 5) and the placement rules there, and move native panels and plugin windows for good.
2. A change of *S* in that time (a command, a settings reload) is stored (§4.4) and takes
   effect, with the window's size, when both sides are non-zero again; a change of *σ* is
   likewise kept for then. *S*auto and the 250 ms stable-size timer of §4.2 ignore sizes with
   a zero side (they neither count as a size nor restart the timer).
3. When both sides are non-zero again, §4.6 runs once for the new size. A window restored to
   the size it had before has the same *Se*, *F* and *A*, so nothing is laid out again and
   nothing moves (invariant I1).
4. Before the window has had a size with both sides non-zero for the first time (it started
   minimised), the UI screen does not exist: the UI host builds and lays out the UI at the
   first such size, never at a fallback.

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
2. a fit container *e* with parent *P* (for a fit root, *P* is the screen root): §5.4 gives
   its fit factor *f* and its **fitted position** (*x*, *y*), real numbers in **P's layout
   coordinates** (the coordinates of *P*'s children: origin at *P*'s layout top-left, one
   unit per layout unit of *P*, frame offsets of §5.2 included). Then
   displayed left = (left of *P*'s displayed box) + *x* × *mx*(*P*), displayed top = (top of
   *P*'s displayed box) + *y* × *my*(*P*), and the displayed size is
   (*w* × *f* × *mx*(*P*), *h* × *f* × *my*(*P*)), with *w* × *h* the container's design
   size; this equals *w* × *mx*(*e*) by *h* × *my*(*e*), since *m*(*e*) = *f* × *m*(*P*). So a
   fit container under a node scale (inside another fit container, a per-window or plugin
   window scale, a fit-to-screen factor) scales and moves with its parent. When *m*(*P*) = 1
   (a fit root, or a parent with no node scale on it or above it), *P*'s layout coordinates
   are UI units relative to *P*'s displayed top-left;
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
| Moves and resizes (UB §8.1, §8.2) | Pointer offsets (*dx*, *dy*) are differences of the real UI coordinates of §3.6, and the layout position and size they change are in two different spaces (§3.4 item 3): a target's layout position is in its **parent's** layout coordinates and is displayed through *m*(parent); its layout size is displayed through its **own** *m*(target). **Move** (UB §8.1 step 3): the target's displayed top-left becomes its displayed top-left at the press plus (*dx*, *dy*), clamped by UB §8.1 step 3 into the parent's displayed box (half-open, with the target's displayed size); its new layout position is (new displayed top-left − parent's displayed top-left) ÷ *m*(parent) per axis, rounded to the nearest integer (halves up). The offset is thus divided by the parent's *m*, never by the target's own scale: for a root panel the parent is the screen root (*m* = 1), so a root with a per-window factor, a plugin `SetScale` factor or *q* < 1 (§5.6; the world map at C1, *q* ≈ 0.9576) moves by the full offset and stays under the pointer. **Resize** (UB §8.2 step 4): *W0*, *H0* are the target's layout size and *dx*, *dy* are divided by the target's *m* (*dx* by *mx*, *dy* by *my*; exact division), so steps 4.1–4.4 work in the target's layout units; the raw size of step 4.1 and the size after step 4.4 are rounded to the nearest integer (halves up). Step 4.5 keeps the opposite **displayed** edge: for a handle that moves the left (top) edge, the layout left (top) changes by (old layout width − new layout width) × *mx*(target) ÷ *mx*(parent), rounded (heights with *my*); otherwise the layout position stays. The caps of step 4.6 compare the resulting displayed box with *F*. This replaces the Tessera rule of the UB §8.2 OPEN (which divided by the target's scale, truncating). With *Se* = 1 and every node scale 1 both rules are UB's. **Fit containers** are never move or resize targets: their boxes come from §5.4, so a move or resize step whose target is a fit container is skipped (SC14: none in this install) |
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

*Se* = min(*S* (or *S*auto), *Wp* ÷ 1024, *Hp* ÷ 768), for *Wp* ≥ 1 and *Hp* ≥ 1 (a window
with a zero side changes nothing, §3.2).

So the UI screen is never smaller than 1024 × 768 UI units, the design size of 283 of the
302 layouts (SC1): no layout of that size is ever laid out into a smaller screen, which is
where shrinking edge pairs make boxes degenerate (SC5: 51 elements at a 720-unit height,
none at 768). *Se* need not be a multiple of 0.05. When *Se* is below the user's *S*, the
scale is **limited**, and the Interface window says so (§7.2). A window smaller than
1024 × 768 physical pixels gets *Se* < 1; there is no lower bound other than *Se* > 0 (at
least 1 ÷ 1024 for a window of 1 × 1).

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

**The Interface window stays put while its slider is dragged.** So that the slider does not
move away from the pointer, the Interface window (§7.2) is held in physical pixels from the
press on the slider to its release:

- **At the press**, record the effective scale *Se*,press (§4.3, after any limit, not the
  user's *S*), the axis factors *Kx*,press and *Ky*,press (§3.2), and the physical position of
  the window's displayed top-left as real numbers,
  (*X*p, *Y*p) = (displayed left × *Kx*,press, displayed top × *Ky*,press).
- **After every change of *Se* until the release** (at the end of step 2 above, once *F*,
  *Kx* and *Ky* are the new ones and after saved placements have been applied, so these rules
  win for this window):
  1. the window gets an extra uniform node scale (§3.4, multiplied with any per-window factor
     it already has) of *Se*,press ÷ *Se*;
  2. its displayed top-left is set to (*X*p ÷ *Kx*, *Y*p ÷ *Ky*), converted with the **new**
     axis factors, by moving its layout position by the difference (its parent is the screen
     root, *m* = 1, so a UI-unit difference is a layout difference, §3.5 "Moves and
     resizes");
  3. §5.6 and keep-on-screen (§5.8) are not run for it: its physical box is the one it had at
     the press, which lay inside the window.
- **Outcome.** The factor is exactly 1 while *Se* equals *Se*,press, so a change of *S* that
  §4.3 limits does nothing to the window. Example: at C1 with *S* = 1.0 the press records
  *Se*,press = 0.9375; dragging to 150 % leaves *Se* at 0.9375 (limited), so the factor stays
  1 and the window does not move. While *Se* does change, the window's content scale stays what
  it was at the press (*Se* × *Se*,press ÷ *Se*; no repaint during the drag), its physical top-left stays where it was up to the
  pixel-centre rounding of §3.6 (at most one pixel), and its physical width and height change
  only by the difference between the axis factors and *Se* (§3.2: each axis factor is within
  *Se* ÷ 1536 of *Se*, at the press and now), that is by less than 0.14 % of the extent.
- **At the release** the extra factor is removed, the window is placed so that the centre of
  the slider's thumb stays under the pointer, then §5.6 and keep-on-screen run. When *Se* at
  the release equals *Se*,press, removing the factor of 1 changes nothing.

<a id="s4-6"></a>
### 4.6 Window size and OS scale changes

A window resize changes *Wp* × *Hp*; an OS scale change changes *σ* (and, on most platforms,
the window's physical size). Both run §4.5 from step 1. A size with a zero side is not a
change: nothing runs until both sides are non-zero again (§3.2 "A window with a zero side"). Pointer positions recorded by the
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
in this install). A layout whose design area has a zero width or height (SC1 holds one) has
no full-screen roots. Every other root panel is a **safe-area panel**.

- A full-screen root with edges (5, 5, 5, 5) is laid out by UL §8.3–§8.4 with the
  screen-level row, *N* = *F*; its direct children are laid out by §5.3.
- A full-screen root whose left and right edges are both 6 is a **fit root**: it is a Type A
  fit container (§5.4) whatever its object mode (all three in this install are Stretch, SC2)
  and is placed by §5.4 with *P* = *F* and *G* = *A*, together with every other fit root of
  the same layout as one **fit group** (§5.4), so that fit roots that are layers of one screen
  share one factor and one placement. Its children
  keep their design layout (§5.4); §5.3 does not apply to it. It is exempt from §5.6 and from
  keep-on-screen (§5.8), and no saved placement is applied to it, because its displayed box
  may reach beyond *F* by design (a cover fit). Drawing and hit testing are clipped to *F*:
  the part of its displayed box outside *F* is neither drawn nor hit, and its canvas need hold
  only the part inside the window (the on-screen rule of §6.1, applied at any size). The part
  of *F* that its group does not cover is filled by the group's letterbox bars (§5.4 step 6).
- A safe-area panel is laid out by the same rules in the **frame** of the safe area,
  *N* = (0, 0, *Aw* − 1, *Hu* − 1), and its result is moved right by *Ax* (the frame
  offset; see "Layout frames" below). So anchored HUD panels (edge modes 4 and 5) sit at the
  safe area's edges, centred ones (mode 1) at its centre, proportional ones (mode 3) scale
  with it, and panels with edge mode 0 keep their design offset from the safe area's left.
- Modal-dialog containers (UL §9.1) and the screen root are full-screen; a dialog inside its
  container is laid out against the container's safe box (§5.3).
- Plugin windows are not laid out by edges; their positions are the plugin's (§7.7).

**Stretch roots (UL §8.4 step 1).** For Tessera under this spec, the Stretch-parent step of
UL §8.4 (step 1: a child of a Stretch root keeps its current box) is **not applied**: the
children of a root panel in object mode Stretch are laid out by steps 2–6 like any other
children, because Tessera never stretches a root's picture to follow the screen (§5.5). For
the 4 Stretch full-screen roots of SC3 that means: their direct children are laid out by
§5.3 (full box or safe box), deeper descendants by the ordinary rules. For a Stretch
safe-area panel whose layout box the run-time layout changes, §5.5 applies (its children
are laid out again). The children of a fit root keep their design layout through §5.4, so
step 1 is not needed there either.

<a id="s5-2-frames"></a>
**Layout frames (re-layout without drift).** A panel laid out against a reference box that is
not at its parent's origin (a safe-area panel in *A*, a child laid out in its root's safe box
by §5.3) is laid out in that box's own coordinates, its **frame**, and then moved by the
frame's offset (*Ax* for *A*; the safe box's left inside the root for §5.3; 0 vertically).
Each such element records the offset that was added at its last layout (0 before its first
layout). Every later layout of it, whatever triggers it (a change of *F*, *A*, the root's
size, a saved placement being applied again), runs in this order:

1. take the element's current box (its position relative to the parent, as code, the user
   or keep-on-screen left it) and subtract the recorded offset, which puts it back into the
   frame;
2. run UL §8.4 steps 2–5 in the frame with the frame's *N*; step 4 (mode-0 edges take the
   current box) reads the box of item 1, and the guard of §5.7 works in the frame;
3. run UL §8.4 step 6, then add the **current** frame offset and record it.

So an edge in mode 0, including one whose position the user dragged (without a saved
placement) or keep-on-screen moved, keeps its distance from the frame's left edge across any
number of re-layouts: the old offset is never added twice. At the first layout the current
box is the design box, which is already in the frame. Example: a mode-0 panel at design
left 100 is at 932 at C8 (*Ax* = 832, recorded); after a switch to `"full"` (C9,
*Ax* = 0) it is laid out from 932 − 832 = 100 and ends at 100; back at C8 it is at 932
again. Panels laid out with *N* = *F* (frame offset 0) are unaffected.

<a id="s5-3"></a>
### 5.3 Children of a full-screen root

Each **direct child** of a full-screen root is laid out against one of two boxes:

- the root's **full box**, when the child is a backdrop: its design box covers the root's
  design box, or its left and right edges are both 6 (a full-cover fit container, §5.4);
- otherwise the root's **safe box**: the part of the root's current box that lies inside *A*,
  as a box of its own (the whole root's box when that part is empty). The child is laid out
  by UL §8.4 with *O* = the root's design box and *N* = the safe box's size, in the safe box's
  frame, and moved right by the safe box's offset inside the root (layout frames, §5.2).

This section applies to full-screen roots with edges (5, 5, 5, 5) only, in any object mode
(it replaces UL §8.4 step 1 for the Stretch ones, §5.2); a fit root keeps its children's
design layout (§5.4).

Deeper descendants follow the ordinary rules. Example: the side panels of the character
screens are direct children that are not backdrops, so on a 32:9 screen they sit at the
edges of the safe area while the 3D scene viewport and the full-cover `ControlsParent` span
the screen (§10.2, row C8).

<a id="s5-4"></a>
### 5.4 Fit containers

A **fit container** is an element whose object mode is Stretch and which has at least one
edge in mode 6 (SC2: 26 elements, all Stretch), or a fit root (§5.2). Its layout box always keeps its **design
size**, so its children are laid out exactly as designed (they are never laid out again
because the container changed size), and the container gets a uniform fit factor *f* (§3.4).
Everything in this section works in the **layout coordinates of the container's parent**
(§3.4 item 2; for a fit root, the screen root's, which are UI units): the reference box *P*,
the guard box, the sizes compared and the **fitted position** that each type computes. §3.4
item 2 then gives the displayed box: the parent's displayed top-left plus the fitted position
× *m*(parent), with the size *w* × *f* × *m*(parent) by *h* × *f* × *m*(parent), *w* × *h*
the container's design size. When the parent has no node scale above or on it, *m* is 1 and
the fitted position is in UI units. The fit is placed by the type:

**Type A — both left and right edges 6** (full-cover fit, 17 elements, all (6, 5, 6, 5); 3
of them are fit roots, §5.2). Type A containers are fitted in **fit groups**, so that
containers that are layers of one picture share one factor and one placement.

*Reference box.* Let *P* be the box a Type A container is laid out in: its parent's current
box (for a direct child of a full-screen root, the root's full box of §5.3, which is the
root's own box), or *F* for a fit root. *P* is given in the parent's layout coordinates, so
it is (0, 0, *Pw*, *Ph*) with *Pw* × *Ph* the parent's current layout size (*Wu* × *Hu* for a
fit root). **P's design coordinates** are the coordinates in which the container's design
rectangle is given: its parent's design box (UL §5.3), or the layout's design area for a fit
root.

*The safe area in P's coordinates.* The safe area *A* (§5.1) is a box of UI units relative
to *F*. For the guard box below it is taken into the parent's layout coordinates: with
(*dl*, *dt*) the parent's displayed top-left and (*mx*, *my*) = *m*(parent), *A*'s left and
right ends *Ax* and *Ax* + *Aw* become (*Ax* − *dl*) ÷ *mx* and (*Ax* + *Aw* − *dl*) ÷ *mx*,
its top and bottom 0 and *Hu* become (0 − *dt*) ÷ *my* and (*Hu* − *dt*) ÷ *my* (real
numbers). For a fit root, and for a parent at the screen's origin without node scales, this is
*A* itself. Example: §10.8 (a Type A container in a panel with a per-window factor of 2 that
straddles the safe area's left end).

*Fit groups.* Two Type A containers *X* and *Y* built from the same instance of one layout
are **grouped** when

- both are fit roots of that layout; or
- neither is a fit root, both lie in the same root panel, and, with *Q* their lowest common
  ancestor: no element strictly between *Q* and *X*, or strictly between *Q* and *Y*, is a
  fit container or carries a node scale (§3.4); their parents have the same design size; and
  the parents' current layout boxes, made relative to *Q*, are equal (where a parent is *Q*
  itself, its box relative to *Q* is (0, 0, *Q*'s width, *Q*'s height)).

A **fit group** is a largest set of Type A containers connected by this relation; a container
grouped with no other is a group of one. All members of a group have the same *P* (as a box)
and the same *P* design coordinates, and their parents have the same displayed top-left and
the same *m*: no element between *Q* and a member's parent carries a node scale, and the
parents' boxes relative to *Q* are equal (fit roots all have the screen root as parent). So
one fitted position, in *P*'s layout coordinates, and §3.4 item 2 give every member's
displayed box. Groups are formed at every layout from the current boxes,
over all members attached to the screen, **whether shown or not**, so showing or hiding one
layer never moves another. In this install (SC13) the 17 Type A containers form 14 groups:
the theatre layout's two fit roots, the map layout's decoration with its map background, and
the mission controller's two scale parents, plus 11 groups of one. No Type A container lies
inside another, but one Type C container lies inside a Type A group (SC13, §10.8); nested
fits follow "Nesting and fitting order" below.

*Group boxes.* In *P*'s design coordinates:

- the **group box** *U* is the smallest box holding the design rectangles of all members;
  *w* × *h* is its size (*Uw* × *Uh*);
- the **content box** *C* is the smallest box holding, for every member, the design boxes of
  that member's direct children that have a width and height of at least 1, each cut to that
  member's design rectangle and taken into *P*'s design coordinates (member's design position
  plus the child's); *Cw* × *Ch* is its size, and its position is taken relative to *U*. *C*
  is **empty** when no member has such a child or every cut leaves nothing;
- the **guard box** *G* is *P* intersected with the safe area *A* taken into *P*'s
  coordinates (above), or *P* itself when that intersection is empty.

*Factor and placement of a group:*

1. *f*contain = min(*Pw* ÷ *w*, *Ph* ÷ *h*); *f*cover = max(*Pw* ÷ *w*, *Ph* ÷ *h*);
   *f*cap = *ρ* × *Ph* ÷ *h*, the **magnification bound**, with *ρ* = 4 ÷ 3.
2. If *C* is empty, *f* = min(*f*cover, *f*cap). Otherwise
   *f*guard = min(*Gw* ÷ *Cw*, *Gh* ÷ *Ch*), where a term whose divisor is 0 (a content box of
   zero extent on that axis) is left out (both cannot be 0 for a non-empty *C*), and
   *f* = max(*f*contain, min(*f*cover, *f*guard, *f*cap)).
   So *f* is the cover factor unless the content box, scaled by it, would be larger than the
   guard box on some axis, or the bound is lower; it is never smaller than the contain factor
   and, because *f*contain ≤ *Ph* ÷ *h* < *f*cap, never larger than *f*cap. The guard is a test
   of **size** only: whether the scaled content box also lies inside *G* depends on its
   position, which steps 3 and 4 decide.
3. Place the scaled group box (*w* × *f* by *h* × *f*) centred in *P* on both axes. Then, if
   *C* is not empty, on each axis: if the scaled content box starts before *G*, move the group
   box by the difference; else if it ends after *G*, move it back by the difference. If the
   content box is larger than *G* on that axis (possible only when *f* = *f*contain >
   *f*guard), centre the content box on *G* instead.
4. On each axis where the scaled group box is at least as large as *P*, clamp its position so
   that it still covers *P* (left ≤ 0 and left + *w* × *f* ≥ *Pw*; top likewise), moving it
   by the least amount. Step 4 **wins** over step 3. Outcome:
   - when some position on that axis satisfies both step 3 (content inside *G*) and step 4
     (group box covers *P*), the clamp lands on such a position, because it moves the group
     box by the least amount from a position that satisfies step 3;
   - otherwise the content box reaches beyond *G* into the rest of *P* (the wings of a wide
     screen or of a narrow safe area). Example: at C2 with `ui_safe_aspect = "4:3"`
     (*A* 1440 wide at 240), a 1024 × 768 fit root whose only child is 300 × 200 at (0, 284)
     gets *f*contain 1.4063, *f*cover 1.875, *f*cap 1.875, *f*guard 4.8, so *f* = 1.875; step 3
     moves the 1920 × 1440 group box to left 240, but step 4 must put it at left 0 to cover
     *F*, so its content spans 0 … 562.5, in the left wing, although *G* starts at 240.
   When step 3 has put the content box inside *P* on an axis, it stays inside *P* after
   step 4: after a clamp to the left the group box starts at *P*'s left edge, so the content
   starts at or after it, and the content's right end only moved left; a clamp to the right
   is the mirror image (top and bottom likewise).
5. **Members.** Every member *X* gets the fit factor *f* and the fitted position
   (left of the placed group box + (*X*'s design left − *U*'s left) × *f*, top of the placed
   group box + (*X*'s design top − *U*'s top) × *f*), in *P*'s layout coordinates; §3.4
   item 2 maps it, with *m*(parent), to its displayed box. So every member's displayed box is
   the image of its design rectangle under the group's one map from *P*'s design coordinates
   to UI units. Its layout box keeps its design size; its layout position is not used for
   display. In a group of one, *U* is the member's own design rectangle and its fitted box
   is the placed group box.
6. **Letterbox bars.** When *P* is *F* (a fit-root group) or a full-screen root's full box
   (*P*'s element is the full-screen root, or a descendant whose current box equals the
   root's, as for the map group), the part of *P*'s displayed box that no **shown** member's
   displayed box covers (the **letterbox bars**) is filled with opaque black, so no stale picture or world
   shows through. The bars are drawn once per group: as opaque black fills directly below the
   picture of the group's lowest shown member in the drawing order (clipped to *F*); a group
   with no shown member draws none. For hit testing, the bars of a fit-root group count as
   part of its lowest shown fit root, so a pointer in a bar hits that root (not its children)
   and never reaches anything behind a full-screen screen; in any other group a bar lies in
   *P*'s element, which the ordinary hit test finds.

**About the bound.** *f*cap depends on *P*'s height only: however wide the window, a group's
design height is magnified to at most 4 ÷ 3 of *P*'s height, so at most a quarter of the
design height is cut off and sparse content is never enlarged with the window's width.
*ρ* = 4 ÷ 3 is the cover-to-contain ratio of a 4:3 design on a 16:9 screen: the bound never
takes effect while *P*'s aspect ratio is at most 4 ÷ 3 times the group box's (up to 16:9 for
the 1024 × 768 layouts, about 2.13 for 1440 × 900, about 3.16 for 2560 × 1080). On wider
screens a group that would cover only by magnifying past the bound stops at the bound and gets
letterbox bars at the sides (§10.1 at C8, §10.7). *ρ* is a design choice (§13 OPEN).

This is the original's aspect mode on screens up to the design aspect (on 16:9 it fills the
height and crops the sides, as UL §8.4 notes for mode 6), and it covers wider screens instead
of letterboxing them, as far as the content and the magnification bound allow (§10.1, §10.6,
§10.7).

**Type B — exactly one of left and right is 6** (side panels, 8 elements). The height drives:
compute the top and bottom edges by UL §8.4 steps 2–5 in the box it is laid out in (*P*:
the parent's current box or the safe box of §5.3), giving the height *H*; then
*f* = min(*H* ÷ *h*, *Pw* ÷ *w*). The displayed width is *w* × *f*. A panel whose right edge
is 6 is anchored at its computed left edge; one whose left edge is 6 is anchored at its
computed right edge (exclusive end = computed right + 1). Its top is the computed top. These
edges, *H*, *Pw* and the anchored left (computed right + 1 − *w* × *f* for a left edge 6)
are in the parent's layout coordinates; they give the fitted position of §3.4 item 2.

**Type C — no horizontal edge is 6, a vertical edge is** (1 element, (1, 1, 1, 6)). The width
drives: *W* from the left and right edges by UL §8.4 steps 2–5, *f* = min(*W* ÷ *w*,
*Ph* ÷ *h*); a panel whose bottom edge is 6 is anchored at its computed top, one whose top is
6 at its computed bottom, one with both 6 is centred on the computed box's vertical centre.
Its left is the computed left. As for Type B, all of these are in the parent's layout
coordinates and give the fitted position of §3.4 item 2.

<a id="s5-4-nesting"></a>
**Nesting and fitting order.** Layout runs parents before children (UL §8). A fit container
is fitted (its factor and fitted position, and for Type A its whole group) once its parent's
layout is done and before its own children are laid out; its children are then laid out in
its layout box, which keeps its design size. Hence:

1. The layout of everything inside a fit container, including any fit container among its
   descendants, is the same at every screen size: the outer factor reaches the inside only
   through *m* (§3.4), never through the layout. An inner fit container's factor and fitted
   position are computed in its parent's layout coordinates, which lie inside the outer
   container's design-size box.
2. The inner container's displayed box is its parent's displayed top-left plus its fitted
   position × *m*(parent), with the size *w* × *f* × *m*(parent) (§3.4 item 2), where
   *m*(parent) includes the outer fit factor. So it scales and moves with the outer
   container's picture, and its content scale is *Se* × (outer *f*) × (inner *f*) × any other
   uniform scale (§3.4).
3. A Type A group inside a fit container takes its guard box from *A* taken into its
   parent's coordinates (above), and its group is fitted after the outer container's group.
   Groups never span a fit container (the grouping rule), so this order is always defined.

In this install (SC13) the one case is the Type C naming window inside the Type A naming scale
parent of the mission-controller group. Its parent keeps its 1440 × 900 design size, so UL
§8.4 steps 2–5 give the naming window (left, top and right edges in mode 1, centring) the
same result at every screen size: left half(1440) − half(411) = 515, right
half(1440) + half(411) − 1 = 924, so *W* = 410, and top half(900) − half(488) = 206; with
*Ph* = 900 its own factor is *f* = min(410 ÷ 411, 900 ÷ 488) = 0.9976, and it is anchored at
its computed top (its bottom edge is 6): fitted position (515, 206). It is displayed through
the group's factor (§10.8).

**Zero extents.** A fit factor is computed only from positive sizes. A fit container is
**not fitted** when its design width or height is 0, or when a box it is fitted to has a zero
extent on an axis the factor reads (Type A: *Pw* or *Ph*; Type B: *H* or *Pw*; Type C: *W*
or *Ph*; this can happen only inside a parent that is empty by design, §5.7). A Type A
container that is not fitted takes no part in a fit group (it is left out before groups are
formed, so it never contributes to *U* or *C*); because the members of a group share *P*, a
zero extent of *P* leaves the whole group not fitted, and the *U* of a group of fitted members
always has a positive width and height. A container that is not fitted gets *f* = 1, keeps
its design-size layout box (empty on a zero axis, §5.7), has as its fitted position the left
and top that UL §8.4 steps 2–5 and the guard of §5.7 compute for it, and draws no letterbox bars. No
division by a zero extent occurs anywhere in this section: *f*guard leaves out zero terms
(Type A step 2) and an empty content box skips the guard (step 2) and the content moves
(step 3).

**Content scale.** Everything inside a fit container is rasterised at
*c* = *Se* × *u* with *f* included (§3.4), so scaled screens stay crisp. A consequence by
design: a full-window screen (loading, character screens) follows the **window**, not the
slider. Its content scale depends on the window's size only, because *f* compensates for a
change of *Se* (§10.1, §10.2); this holds for the bound too: for a *P* of the screen's height,
*f*cap × *Se* = *ρ* × *Hu* × *Se* ÷ *h*, which is *ρ* × *Hp* ÷ *h* up to the rounding of *Hu*
(§3.2). The user's scale enlarges
everything else; the optional text size (§7.5) also reaches text on these screens.

<a id="s5-5"></a>
### 5.5 Stretch elements resized by the layout; backdrop images

An element in object mode Stretch **without** an edge in mode 6, whose layout box the
run-time layout changes (UL §8.4 step 6), takes the new size into its layout box and lays its
children out again (the reading UL §8.4 records as implemented); no stretch vector is set.
This is a Tessera rule; it keeps such elements crisp.

**Backdrop images.** UL §9.6 draws images at native size and tiles them. For such an element,
an image layer whose native size equals the element's **design** size (the image was made
for it; design width and height at least 1) is drawn instead as a **cover** fit of the current box: scaled uniformly by
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

A fit root (§5.2) gets no *q*: its displayed box may be larger than *F* on an axis by design
(a cover fit, within the magnification bound of §5.4), and it is clipped to *F* instead.

<a id="s5-7"></a>
### 5.7 No degenerate boxes

The rules above prevent the known causes (the design floor of §4.3, fit containers of §5.4);
one final guard makes the guarantee hold for any data:

After UL §8.4 steps 2–5 (and before step 6), when an element's computed width or height is
below 1 UI unit, that axis takes the element's **design** extent, reduced to the extent of
the box it is laid out in (its parent's current box, or the frame's *N* of §5.2) on that axis
when larger, and the element is moved on that axis so that it lies inside that box
(0 … extent of the box). A box of positive size, even one that overflows its parent, is left
as computed. No extent is ever negative.

**Empty by design.** When the guard's result on an axis is 0, because the element's design
extent on that axis is 0 or because the box it is laid out in has a zero extent there (its
parent is itself empty by design), the element is **empty by design** on that axis: its
layout box has extent 0 there, its position on that axis is clamped into the box it is laid
out in like any guarded axis, and it is neither drawn nor
hit (its half-open displayed box contains no point). This is not a degenerate box: the data
made it empty, the layout did not. Its children are laid out by the same rules; one with a
positive computed extent keeps it (overflowing its empty parent), one below 1 becomes empty
by design too. Fit containers with a zero extent are not fitted (§5.4 "Zero extents").

Because the screen root is at least 1024 × 768 and every parent is laid out before its
children, every element's layout box has a width and height of at least 1 UI unit, except on
an axis where it is empty by design, where the extent is exactly 0 (invariant I5). The guard
is the stop-gap rule of UL §8.4 made permanent and general; with §5.4 the loading screen no
longer needs it (SC5).

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
3. after a change of *Se*, *F*, *A* or a per-window scale (a window size with a zero side is
   not such a change, §3.2);
4. at each move step and resize step (UB §8.1 step 3 and §8.2 step 4.6 already clamp to the
   screen; in Tessera they use these displayed boxes);
5. for **plugin windows** only in case 3, and only for a window that lay entirely inside the
   old *F*: a plugin may place its own window partly off screen with `SetPosition`, and
   Tessera never overrides that explicit position, except to bring back a window that the
   screen's shrinking pushed out.

The clamp uses *F*, not *A*: the user may drag panels into the wings of a wide screen. Fit
roots (§5.2) are exempt: they are never moved by keep-on-screen and are clipped to *F*
instead.

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
- the degenerate-box guard of §5.7 replaces the stop-gap rule, generalised, with elements of
  zero design extent kept empty by design;
- the Stretch-parent step of UL §8.4 (step 1) is not applied: children of Stretch roots are
  laid out by steps 2–6, those of full-screen roots by §5.3, and a fit root keeps its
  children's design layout (§5.2).

The stop-gap stays in force until this spec is Accepted and built.

> **OPEN (implementer):** the §5.9 degenerate-box guard is now **built**: `tessera-ui`'s
> run-time layout (`crates/ui/src/layout.rs`, `fn runtime`) calls
> `scaling::degenerate_guard(computed, design, box_extent)` in place of the stop-gap, with a
> code comment pointing here. The trigger is unchanged from the old stop-gap (a width or height
> below 1 UI unit), so non-degenerate boxes are byte-identical (invariant I2); the only
> behavioural change is that an element with a **zero** design or box extent is now left empty
> by design (extent 0) instead of being forced to 1. A **spec writer** should now update the
> `ui-layout.md` §8.4 step 6 note (and its "stop-gap stays in force" wording), which this spec
> replaced, to point at §5.9 as the built answer rather than describing an open stop-gap.

> **OPEN (implementer):** this is the **first live-wiring increment** on top of the merged pure
> geometry core (`crates/ui/src/scaling`), and it does not yet build all of §5–§6, in the same
> staged way §9.3 (G2) and §9.6 already record for the pure core. **Built now:** the UI screen
> *F* and safe area *A* are stored on `Ui` (`Ui::set_ui_screen`, identity default = `Se` 1,
> *A* = *F* = the screen size, so I2 holds); the screen root is resized to *F*'s UI-unit size so
> every root is laid out again in UI units on a scale or window change; every surface is composed
> onto its **physical box** (§3.6, §6.6) via the axis factors, so the whole UI scales to the
> window (1 : 1 at `Se` = 1; GPU-scaled with `ui_scale_filter` otherwise, the §4.5-step-3 /
> §6.6 composed path); `Ui::physical_box` gives §3.6 physical boxes for the host's hit test and
> draw; the degenerate guard (§5.9) and keep-on-screen for saved placements (§5.8) run through
> the module. **Deferred to a following increment (not missing facts; volume and regression
> risk against the committed goldens):** (a) the §5.2–§5.4 fit-container, fit-group and
> safe-area-panel *layout* integration into the live tree (today a root is laid out against *F*
> as before; fit roots, Type A/B/C fitting and letterbox bars are not yet applied in the live
> pass) and §5.6 fit-to-screen; (b) the §6.1–§6.5 **crisp** painting (canvases painted at the
> content scale, per-axis image resampling, glyphs chosen/derived at the target height and
> cached), which this increment approximates by GPU-scaling the UI-unit canvas at composition
> (soft, not crisp, at `Se` ≠ 1). The pure core each part calls is already present and tested.

> **OPEN (implementer):** the anchored `SavedPlacement` **store** of §5.8 is now **built**.
> `crates/ui`'s window manager (`crates/ui/src/wm.rs`) records a placement as a
> `scaling::SavedPlacement` (an anchor to a `RefBox` + a UI-unit offset, via
> `SavedPlacement::from_box`) and restores it with `SavedPlacement::position` followed by
> keep-on-screen (§5.8 "Applying"), choosing the reference box `R` per root (the full screen `F`
> for a full-cover (5, 5, 5, 5) root, else the safe area `A`). `SavedStore` keeps both the
> anchored `placements` (authoritative) and the old normalized-corner `entries` (retained for
> `to_xml` and as the §5.8 "Migration" fallback `apply_saved` uses when a key has only an old
> entry). The persisted form is the `[UI.panels]` `PanelPlacement` of `crates/settings`
> (`crates/settings/src/ui.rs`), which mirrors `SavedPlacement` field-for-field; because
> `tessera-ui` does not depend on `tessera-settings`, the field-copy bridge between the in-memory
> store and the settings table is left to the client's UI host (out of a
> `crates/ui`-plus-`crates/settings` change). **Settings-layer migration limitation:** §5.8
> "Migration" converts an old normalized-corner entry "at the current configuration", which needs
> `Wu` × `Hu`; the settings loader has no screen, so `PanelPlacement::from_legacy` recovers only
> the anchor *directions* (scale-free, from which third the box centre falls in) and the
> visibility, and docks the window to the anchor (offset 0), keeping its laid-out size. The exact
> UI-unit offset is then re-established the next time the user moves the window. This is a
> defensive path: no released schema wrote normalized `[UI.panels]` entries.

> **OPEN:** (implementer, tessera-ui, 2026-10-23) the **live-layout follow-up** of the
> "first live-wiring increment" note above is built in `crates/ui` (`scale.rs`, `ui.rs`,
> `wm.rs`, `pointer.rs`): multi-member Type A fit groups (§5.4 grouping rule, steps 1–5, hidden
> members included, regrouped when a member is detached; checked against §10.6 at C1, C2, C7,
> C8, C9 and a two-level group like the SC13 map group); §5.3 per-child safe boxes for the
> direct children of full-screen (5, 5, 5, 5) roots, with layout frames; the UL §8.4 step-1
> Stretch-parent skip is no longer applied once a UI screen is set (§5.2, §5.9; at the identity
> it stays, so I2 holds); §5.6 *q* multiplies an existing node scale (per-window, plugin
> `SetScale`, stretch) and applies to plugin windows (`GetSize()` unchanged); §5.8 items 1, 3
> and 5 run after every root layout and every change of *Se*, *F* or *A* (saved placements
> applied again after a change of *F* or *A*, not of *Se* alone; a plugin window inside the old
> *F* is kept inside the new one, one placed off screen is left alone); a change of the safe
> area alone lays the roots out again. **Not
> built — the drawing half of §5.4 step 6.** The letterbox bars are computed
> (`Ui::letterbox_bars`: per group, physical rectangles clipped to the window and the lowest
> shown member they go below) and hit as §5.4 step 6 and I3 say, but they are not yet entries
> of the composition list: its entry kinds (`EntryTexture::Surface`, `External`) carry only
> textures, the list is built in the paint/surface layer, and `tessera-render`'s UI pass
> (`UiEntry`) and the client's recorder (`apps/client/src/ui_host/gpu.rs`) have no untextured
> fill. Adding a solid-fill entry needs a `tessera-render` change (or a client-side 1 × 1 black
> texture) plus the client recorder, outside `crates/ui`; until then the UI host must draw the
> bars itself from `Ui::letterbox_bars` (the CPU goldens do). Engineering choices recorded:
> a fit group holds at most 64 members, the first 64 in drawing order (members past the cap
> are fitted as groups of one, the same cut whichever member is laid out; the letterbox bars
> are still worked out once for the whole uncut group); one scan for groups visits at most
> 4,096 elements of a root panel (or 4,096 screen children for fit roots), and containers
> beyond it are groups of one (the install's largest group has 2); one group's bars are split
> into at most 256 rectangles (past that the remaining holes are not cut out, so a bar may lie
> under a member's picture, never over it). Groups are found as one hash partition per root
> panel (the §5.4 relation is equivalent to equal keys: the nearest fit container, scaled
> element or root panel at or above the parent, the parent's box relative to it and the
> parent's design size), cached for the layout path and checked on every use.

> **OPEN:** (implementer, tessera-ui, 2026-10-23) the live tree still derives displayed boxes
> by the truncating integer rule of UB §9.2.1 (`displayed_from`, node scales stored as `f32`),
> not as the 64-bit real numbers of §3.4. Fit factors, groups and bars follow §5.4, but the
> boxes they produce can lose up to one UI unit at an edge: the §10.1 image at C1
> (*f* = 768 ÷ 1080) is displayed 767 units high, so a one-pixel bar row is left at the bottom
> of the window, and a Type B panel whose design right edge meets its fit-root parent's ends one
> unit short of it (G4 at C1). Building the §3.4 real-valued displayed box in the live tree
> (hit test, painter, composition and physical boxes together) is a separate increment; no
> spec fact is missing. Under a node scale *m* the same rule gives a displayed width of
> ⌊(*w* + 1) × *m* − 1⌋, up to *m* − 1 units more than *w* × *m*: the §10.8 (b) container *X* is
> displayed 2402 units wide instead of 2400 (its left 232 and its child's left 832 are exact).

> **OPEN:** (implementer, tessera-ui, 2026-10-23) choices made in the review round of the
> live-layout follow-up, for a spec writer to confirm or correct:
> (a) **Fit containers under a node scale** (§3.4 item 2): the tree reads only the nearest
> node scale (UB46), so a fit container stores *f* × *m*(parent) as its node scale and keeps
> *f* apart (`Ui::fit_factor`). Its inner fit containers are fitted again whenever its scale
> or box changes, and the fit containers of a root panel are fitted again after a per-window,
> plugin `SetScale`, stretch or fit-to-screen change of the root and after **every move or
> resize of the root** (a drag, keep-on-screen, a saved placement), because the Type A guard
> box depends on the parent's displayed top-left and *m* (§10.8 (b) describes a panel the user
> dragged across *A*'s edge). §5.4 says only that groups are formed "at every layout"; does
> the fit follow moves and per-window scale changes, as built, or only re-layouts?
> (b) **Modal dialogs** (§5.2 "a dialog inside its container is laid out against the
> container's safe box (§5.3)"): built as a root panel laid out against the screen: a dialog
> that covers its layout's design area with edges (5, 5, 5, 5) is laid out against the full
> box (*N* = *F*, its children by §5.3); any other one in the safe area's frame with *O* = its
> layout's design area (the container's design box is the screen size at creation, which is
> no design coordinate system for the dialog), then §5.6 and keep-on-screen. §5.3 tests a
> backdrop against "the root's design box"; for a dialog the layout's design area was used.
> (c) **Fit containers that are safe-area roots** (a root whose edges make it Type A, B or C
> but which is not a fit root): laid out in the safe area's frame (§5.2), so a Type A one has
> *P* = *A* (in the screen's coordinates; §5.4 "the box it is laid out in" read over "its
> parent's current box", which for a root would be *F*) and is a group of one (the grouping
> rule names only fit roots and non-root containers); all of them get §5.6 and keep-on-screen
> like other safe-area roots, *q* multiplying the fit factor.
> (d) **Saved placements applied again** after a change of *F* or *A* (§4.5 step 2, §5.8
> "Applying") set the position and size only; the `shown` field is applied when the panel is
> opened (by the host's `apply_saved`), not again at every resize, so a panel that code hid
> or showed since (a target frame without a target) keeps that state. §5.8 lists visibility
> among what "Applying" sets "again after every change of *F* or *A*"; is that intended?
> (e) **Saving** (§5.8 "Saving"): the UI records the anchored placement itself at the end of
> a user move or resize of an element with the save properties (the same release that raises
> UB §8.4 step 2's "layout changed" notification), so the store the placements are applied
> from again is the user's last placement.

---

<a id="s6"></a>
## 6. Crispness

<a id="s6-1"></a>
### 6.1 Canvas sizes

Every surface *s* (UR Decision 1.1) is painted by the CPU painter into a canvas. Let
*σ*(*s*) be the product of the stretch vectors (§3.4) of *s* and of every ancestor of *s*: the
non-uniform part of *m*(*s*). With *w* × *h* the layout size of *s*:

- **Unstretched surface** (*σ*(*s*) = (1, 1), the normal case): the canvas has the size of
  its **physical box**, (E*x*(*R*) − E*x*(*L*)) × (E*y*(*B*) − E*y*(*T*)) for its displayed
  box [*L*, *R*) × [*T*, *B*), and is composed 1 : 1 (§6.6).
- **Stretched surface** (*σ*(*s*) ≠ (1, 1)): the canvas is painted at the surface's
  **unstretched** size: its **unstretched box** is (0, 0, *w* × *u*(*s*), *h* × *u*(*s*)) in
  UI units (§3.4, uniform node scales only), and the canvas is E(*w* × *u*(*s*)) ×
  E(*h* × *u*(*s*)) pixels, where E(*v*) is the pixel-centre edge mapping of §3.6 with the
  factor *Se* on both axes (the smallest integer *X* with (*X* + ½) ÷ *Se* ≥ *v*). The stretch
  is applied once, when the canvas is composed onto the physical box of the surface's full
  displayed box (§6.6).

The limits of UR §7 apply in physical pixels (at most the device's largest texture dimension
and 16,384 on each axis; beyond that only the on-screen part is stored, reported once). A fit
root (§5.2) may store only its on-screen part at any size. A Tile surface (UL §8.4 Tessera
rules) is painted at its tile size × *c*, rounded to whole pixels, and repeated at that
period.

> **OPEN (implementer):** `crates/ui` now allocates each surface's canvas at its physical box
> and composes it 1 : 1 onto that box (§6.6), with the display list and damage mapped to canvas
> pixels by the per-coordinate edge map of §3.6 (so they agree with the physical boxes and the
> nested-entry coverage regions). **Images** are resampled by the §6.3 per-axis filter
> **selection** (sharp bilinear at a scale ≥ 1, the area filter below 1, chosen per axis kind
> from the element's layout extent). **Text** is the remaining soft path: the painter resamples
> each glyph's **native** coverage by the uniform content scale *c* = (*mx*·*Kx*, *my*·*Ky*)
> (straight bilinear), so scaled text is correctly sized but soft, not yet crisp. The §6.4
> **derived-glyph** machinery (target-height source selection, derived metrics and the §6.5
> glyph cache) is built as additive entry points in `crates/ui/src/text` (`layout_scaled`,
> `draw_scaled`) but is **not yet wired** into the surface/painter draw path; wiring it is the
> next increment, after which this soft text path is retired. Also, the uniform resample scale
> *c* can differ from the per-coordinate edge spacing of the mapped geometry by at most one pixel
> at non-integer scales; reconcile when the derived-glyph path lands. At the identity (*Se* = 1
> with *A* = *F*, or no UI screen) everything is byte-identical to the pre-scaling path
> (invariant I2).

> **OPEN (implementer, 2026-10-23):** the element tree keeps one node scale per element and
> does not record its kind (§3.4), so `crates/ui` treats a node scale with equal factors on
> both axes as uniform and only one with different factors as a stretch vector *σ*. The
> full-screen map window (`map-windows.md` §3.1 Tessera rule: "scaled uniformly by the largest
> factor that fits") is fitted by an element resize of its Stretch surface, which by §5.5 (last
> paragraph) sets a stretch vector; with equal factors (1.171875 × 1.171875 at 1600 × 900) its
> map canvas is therefore painted at the physical box (1200 × 900, crisp, composed 1 : 1)
> rather than at its unstretched box (1024 × 768, stretched at composition) as the
> "stretched surface" rule above says. Question: does an equal-factor stretch vector count as
> uniform for §6.1 and §6.2 (and so for *c*), or must the tree record the kind of each node
> scale so that every element resize of a Stretch surface is painted unstretched? If the
> latter, should the map window's full-screen fit use a uniform kind of §3.4 instead of an
> element resize?

<a id="s6-2"></a>
### 6.2 Element boxes inside a canvas

An element *e* painted into the canvas of surface *s* occupies its **canvas box**:

- in an **unstretched** surface (§6.1): *e*'s physical box (§3.6) minus the canvas origin
  (the top-left pixel of *s*'s physical box, or of the stored part when only part is stored,
  §6.1);
- in a **stretched** surface: *e*'s box mapped **without** the stretch vectors *σ*(*s*). Its
  displayed box is computed by §3.4 inside *s*, starting from *s*'s unstretched box of §6.1
  (origin 0, scale *u*(*s*)) instead of *s*'s displayed box, with the node scales of the
  elements between *s* and *e* applied as usual, and its edges are mapped to canvas pixels by
  E of §6.1 (factor *Se*). Elements painted into *s* carry no stretch vector of their own: an
  element with one owns a surface (§3.4). So the elements land at their unstretched places in
  the unstretched canvas, and the picture is stretched once, at composition.

Clip rectangles, meter fills (UW §4), scroll offsets (UW §5) and any other extent that a rule
gives in layout units are converted with the same mapping as the canvas box: the layout
coordinate is displayed by §3.4 (in a stretched surface, from the unstretched box), then mapped
to canvas pixels. Wherever this section, §6.3 and §6.4 speak of an element's physical box or
physical pixels inside a canvas, they mean its canvas box and canvas pixels. Line widths that the
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
scale is needed. In a stretched surface (§6.1) the pointer is first taken into the canvas:
its UI coordinate (§3.6) minus the surface's displayed left, divided by *σx*(*s*), times
*Se* (likewise *y*); that point minus the element's canvas box origin is looked up.

<a id="s6-5"></a>
### 6.5 Glyph cache

Glyphs are derived on demand, one glyph at a time, and cached by (font DID, target height,
source kind, outline or fill). The cache is bounded (it counts toward the decoded-image LRU of
§8) and evicts least recently used heights. Deriving counts toward the frame's paint slice
(UR §6 U4 (b)); a client may derive the printable ASCII range of the fonts on screen on the
asset workers ahead of a settle (§4.5 step 4).

> **OPEN (implementer):** the key names a "font DID", but Tessera's own fallback faces
> (FO §11 item 2) have no DID. The `tessera-ui` text module folds a Tessera face's
> (file, code-set, pixel-size) triple into the 32-bit DID field and sets the top bit so it
> cannot alias a real data DID; two distinct Tessera faces could still in principle collide and
> serve one another's derived glyph (data records, the common case, never collide). Confirm the
> intended key for DID-less faces, or widen the key.

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
| G8 fit group | two layered 1024 × 768 fit roots, one with content over its whole area and one with a small content box near the bottom (§10.6), and a sparse 1440 × 900 fit root (§10.7) | C1, C2, C7, C8, C9 |

> **OPEN (implementer):** G2's painted-paragraph form — the same paragraph **laid out and
> painted** at the content scales *c* = 1, 1.25, 1.5, 2, 3 over the text layout and painter — is
> not yet committed: this increment builds the §6.4.4 glyph derivation (the pure core) but not
> the §6 text layout/paint integration that selects derived glyphs at the content scale. The
> committed golden (`crates/ui/tests/ui_scaling_glyph_golden.rs`,
> `golden/uiscale_glyph_ladder.png`) exercises the derivation directly — one synthetic ladder
> glyph at an exact, two downsampled and two distance-derived heights — which is the heart of G2
> but not the painted paragraph at the named content scales. Add the painted-paragraph G2 over
> the headless renderer once the painter lands; until then the derivation golden is the stand-in.

<a id="s9-4"></a>
### 9.4 Local real-data goldens (decision 0031 tier 2)

With `TESSERA_GAME_DIR`, named `<subject>@<window>@<scale>[@<aspect>]` (decision 0031:
"the choice is part of `<name>`"):

| Subject | Configurations |
|---|---|
| Loading screen (TW §5.2) | C1, C2, C5, C7, C8, C10, C12 |
| Theatre layout, story and progress layers shown together (§10.6) | C2, C7, C8 |
| Character appearance screen (CS §2.2) | C1, C2, C5, C7, C8, C10 |
| HUD set (UL §17 P1–P5, P11, P14) | C1, C2, C4, C5, C6, C8, C9 |
| World map panel P10 (fit to screen at C1) | C1, C5 |
| Interface window (§7.2) | C1, C5 |
| Mission-controller screen with the naming window shown (§10.8; a safe-area panel, so no letterbox bars — the C8 golden must show none) | C1, C2, C8 |

<a id="s9-5"></a>
### 9.5 Invariants

Each is testable on synthetic layouts at random configurations (property tests) and on the
install's layouts at C1–C12:

- **I1** *Wu* ≥ 1024, *Hu* ≥ 768 and *Se* ≤ *S* for every window size ≥ 1 × 1 and every *S*.
  For any sequence of window sizes, a size with a zero side (*Wp* = 0 or *Hp* = 0) leaves
  *Se*, *F*, *Kx*, *Ky*, *A* and every layout box, fit factor, displayed box and saved
  placement as they were before it, and no division by zero occurs; a minimise followed by a
  restore to the same size moves nothing (§3.2).
- **I2** At *Se* = 1 with every node scale 1, the painter's canvas of every surface is
  byte-identical to the canvas painted before this spec (the existing goldens pass).
- **I3** For every element and every physical pixel: the pixel lies in the element's physical
  box ⇔ the hit test at the pixel's centre is inside its displayed box (§3.6). For a fit root
  (§5.2) read both as clipped to *F* (window), and its group's letterbox bars as part of the
  group's lowest shown fit root (§5.4 step 6).
- **I4** Sibling elements without node scales of their own whose layout boxes abut, abut in
  physical pixels (§3.6 consequence 2).
- **I5** After layout, every element's layout box has width ≥ 1 and height ≥ 1 UI unit,
  except on an axis where it is empty by design (§5.7), where the extent is exactly 0; no
  extent is ever negative. Every fit factor is finite and positive.
- **I6** After any of the events of §5.8 items 1–4, every shown native root panel's displayed
  box lies inside *F*, except fit roots (§5.2), which are clipped to *F*; the displayed boxes of
  the shown fit roots of one fit group together with the group's letterbox bars cover *F*. A plugin window that lay entirely inside
  *F* before a change of *Se*, *F* or *A* lies inside the new *F* after it; other plugin
  windows are where the plugin put them (§5.8 item 5).
- **I7** At *c* × *x* = 1 every glyph comes from the font's own record; at a height with an
  exact ladder record, from that record.
- **I8** A saved placement applied at the configuration where it was saved restores the same
  displayed position (within 0.01 UI units); applied at another configuration, the distance
  from the anchored edge in UI units is unchanged unless keep-on-screen moved the panel.
- **I9** During a scale change no frame shows a surface without a picture that it showed
  before the change.
- **I10** *S*auto follows the table of §4.2 for the listed inputs.
- **I11** With safe aspect *a*, every safe-area panel that meets all of these has its
  displayed box inside *A*: laid out by edges only (never moved by the user, a saved
  placement or keep-on-screen; no node scale, *q* = 1); every edge in mode 0, 1, 3, 4 or 5;
  its design box inside its layout's design area; and that design area no larger than *A*
  (design width ≤ *Aw*, design height ≤ *Hu*; true for every 1024 × 768 layout). A design box
  that overhangs its design area is excluded: the minimap of §10.3 (22 units beyond the
  design width) ends at 4310 at C8, beyond *A*'s right end 4288, though it is narrower
  than *A*.
- **I12** For a safe-area panel with no node scale, not limited by §4.3 or §5.6, whose layout
  width and height do not depend on the reference box (UL §8.4 steps 2–5 give the same
  extents for every *N*: for example left and right edges both anchored to the same side, or
  both centred, and no edge in mode 3 or 6, and not touched by the guard of §5.7): its
  physical width and height do not decrease by more than one physical pixel when *S*
  increases (the one pixel is the rounding of §3.6). A panel whose size follows *N* is
  excluded: one with edges (5, 5, 5, 5) and 100-unit insets in a 1024 × 768 layout is
  1720 physical pixels wide at C2 and 1670 at C3.
- **I14** Layout frames (§5.2): laying out a configuration twice gives the same boxes, and a
  change from one configuration to another and back (for example C8 → C9 → C8, or C2 → C3 →
  C2) returns every element laid out by edges, and every panel the user moved without a
  saved placement and not moved by keep-on-screen in between, to the same layout box.
- **I15** Type A (§5.4): when some position satisfies both step 3 and step 4 on an axis, the
  scaled content box lies inside *G* on that axis; when step 3 put it inside *P*, it lies
  inside *P* after step 4.
- **I16** Fit groups (§5.4): after every layout, all members of a fit group have the same fit
  factor *f*, and one map from *P*'s design coordinates to UI units, (*x*, *y*) ↦
  (*Ox* + *x* × *f* × *mx*, *Oy* + *y* × *f* × *my*) with (*mx*, *my*) the members' common
  *m*(parent), gives every member's displayed box from its design rectangle (within 10⁻⁶ UI
  units); so the same design point of two layers is displayed at the
  same place. Showing or hiding a member changes no member's displayed box or factor. Two
  Type A containers that meet the grouping rule are never fitted with different factors.
- **I17** Magnification bound (§5.4): every Type A factor satisfies *f* ≤ 4 ÷ 3 × *Ph* ÷ *h*
  (*h* the group box's design height), so for a *P* of a given height no Type A factor grows
  with *P*'s width; and *f* ≥ *f*contain. For the tutorial chooser of §10.7 at C7, C8 and C9
  *f* is 2.1333 (within 10⁻⁴).
- **I18** Nested fits (§3.4 item 2, §5.4 "Nesting and fitting order"): every fit container's
  displayed box equals its parent's displayed top-left + fitted position × *m*(parent), with
  the size (*w* × *f* × *mx*(parent), *h* × *f* × *my*(parent)) (within 10⁻⁶ UI units), and its
  fitted position and factor do not depend on the factors of fit containers above it. For the
  naming window of §10.8 at C1, C2, C5 and C8 the displayed box and content scale match the
  table (within 0.01 UI units and 10⁻⁴). The mission-controller scale-parent group lies inside
  a safe-area panel (§10.8(a)), so its reference box *P* is the safe area *A* (= *F* at C1, C2,
  C5; *A* = 3456 at 832 at C8) and §5.4 step 6 draws it no letterbox bars at any size; at C8
  its *f*A is 1.6364 (the guard term, unchanged by the safe area) with *f*cover 2.4, and the
  scale parents' displayed box (1381.82, 0, 2356.36 × 1472.73) and the naming window's
  displayed box match the corrected C8 row.
- **I19** Moves (§3.5): during a move step whose target is not clamped, the target's displayed
  top-left differs from its displayed top-left at the press plus the pointer offset by at most
  half a layout unit × *m*(parent) on each axis, whatever the target's own node scale; a
  resize keeps the displayed edge opposite the handle within the same bound.
- **I13** Plugin `Display.GetSize()` equals (*Wu*, *Hu*), and `GetUIGroupScale(Global)` is 1.0.
- **T1** A derived glyph that is a solid rectangle in the source stays a solid rectangle with
  at most one partial pixel row or column per edge.
- **T2** A derived glyph's ink box is within one pixel of the source ink box scaled by *k*.
- **T3** For a downsampled glyph whose source box *W* × *H* and derived box *W*′ × *H*′ are
  both at least 1 × 1 (coverage in 0 … 1): the sum of the derived coverage is within
  *W*′ × *H*′ ÷ 510 (half an 8-bit step per derived pixel) plus 1 % of the reference value,
  where the reference value is the source sum × (*W*′ × *H*′) ÷ (*W* × *H*). The area filter
  of §6.4.4 maps the source box onto the rounded derived box, so the ratio of the boxes, not
  *k*², is the exact scale of the total (they differ by up to about 8 % for a 1-pixel stem at
  *k* = 0.917); that filter meets T3 with the quantisation term alone, and the 1 % is for
  other methods.

<a id="s9-6"></a>
### 9.6 Other tests

- **Fuzz:** the settings keys of §4.4 and §7.1 (`settings_ui` target: any TOML value, no
  panic, defaults and warnings); a configuration target (any *Wp*, *Hp* in 1–16,384, any *S*,
  any aspect) over synthetic layouts, including elements, fit containers and layout design
  areas with zero extents, layered and nested fit containers (a Type B or C inside a Type A,
  a Type A inside a panel with a per-window factor) and sequences of window sizes that include
  zero sides (*Wp* or *Hp* = 0, in 0–16,384), asserting I1 (both parts), I5, I6, I14, I16,
  I17, I18 and no panic.

  > **OPEN (implementer):** the `ui_scale_config` fuzz target (the pure `tessera_ui::scaling`
  > core of this increment) asserts the parts of these the stateless geometry can establish:
  > I5, I6 (proxy), I16, I17, I18, the pure-core slice of I14 (recomputing a size, and
  > `A → B → A`, is byte-identical), a Type C fitted inside a Type A member box (a B/C inside a
  > Type A), and the pure half of I1 part 2 (a zero side yields no screen and leaves the
  > retained last-good state intact). The **stateful** halves — that a zero side leaves a
  > *retained* `Se`/`F`/`A` and every laid-out box, factor and saved placement unchanged, a
  > minimise→restore moving a live panel nothing, and the frame-system re-layout determinism of
  > I14 over the live element tree — belong to the §5–§6 layout/paint host, which this increment
  > does not build. Extend the configuration fuzz (or add a stateful companion) to drive a size
  > sequence through that host and assert them once it lands.

  > **OPEN:** (implementer, tessera-ui, 2026-10-23) the stateful companion is built: the same
  > `ui_scale_config` input now also drives a live `tessera_ui::Ui` (layered fit roots, a
  > full-screen root with a Type A backdrop, a Type B panel and a safe-box child, a safe-area
  > panel with a node scale, a plugin window; zero extents allowed) through sizes with zero
  > sides, minimise/restore, shows/hides, drags and node-scale changes, asserting I1 part 2,
  > I5, I6 (native roots after a change of *Se*, *F* or *A*), I16 and restore idempotence.
  > `crates/ui/tests/ui_scaling_live.rs` holds the matching unit tests (zero sides and
  > minimise/restore, the C8 → C9 → C8 round trip of a dragged mode-0 panel, I14 for a
  > right-anchored safe-area panel). The note above is superseded for these parts.
- **Unit:** the pixel-centre mapping at *K* = 0.937729, 1, 1.5, 2.25, 3 (I3, I4); the Type A,
  B and C fits with the numbers of §10; the Type A example of §5.4 step 4 (content in the
  left wing at C2 with the 4:3 safe area, I15); a fit root at C8 (bars drawn, no *q*, not
  moved, clipped); the fit groups of §10.6 (one factor, one placement, bars drawn once below
  the lower shown layer, hiding a layer moves nothing, I16) and a group across two tree
  levels like the map group of SC13; the magnification bound of §10.7 at C2, C4, C7, C8, C9
  and C12 (I17); the Interface-window drag of §4.5 at C1 with a limited scale (factor 1, no
  movement) and from C2 to C3 (physical top-left held, size within 0.14 %); the nested fits
  of §10.8 (the naming window at C1, C2, C5, C8, and the Type A container under a per-window
  factor at C8 with *G* taken into the parent's coordinates, I18); moves and resizes under
  node scales (§3.5, I19): a root with *q* < 1 (the world map at C1) and one with a plugin
  `SetScale` of 1.5 follow the pointer by the full offset, a child panel inside a parent with
  a per-window factor of 2 moves by the offset ÷ 2, a resize of the root with *q* < 1 changes
  its layout size by the offset ÷ *q* and keeps the opposite displayed edge; a minimise and
  restore (sizes 0 × 0, *Wp* × 0 and 0 × *Hp*, then the old size) that changes nothing, and a
  change of *S* while minimised that takes effect at the restore (§3.2, I1); fit
  containers with *w*, *h*, *Cw* or *Ch* of 0 and an empty content box (§5.4 "Zero extents");
  the layout-frame example of §5.2 (I14); a stretched surface whose children are painted at
  their unstretched canvas boxes (§6.2); the anchors of §5.8; the `/uiscale` argument
  forms.
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

| Window, *S* | *Se*, *F* | *f*contain / *f*cover / *f*guard / *f*cap → *f* | Image box (UI) | *c* | Meter, physical | Label text |
|---|---|---|---|---|---|---|
| 1280 × 720, 1.0 or 1.5 | 0.9375, 1365 × 768 | 0.5332 / 0.7111 / 1.0 / 0.9481 → 0.7111 | (−227.72, 0, 1820.44 × 768) | 0.667 | (572, 698, 133, 9) | *t* = 8: downsampled from `Verdana10` |
| 3440 × 1440, 1.0 | 1.0, 3440 × 1440 | 1.3333 / 1.3438 / 1.875 / 1.7778 → 1.3438 | (0, −11.25, 3440 × 1451.25): centred, then moved up 5.63 so the content's bottom meets *G* | 1.344 | (1583, 1396, 267, 18) | *t* = 16: exact `Verdana16` |
| 3440 × 1440, 1.5 | 1.5, 2293 × 960 | 0.8889 / 0.8957 / 1.25 / 1.1852 → 0.8957 | (0, −7.36, 2293 × 967.36) | 1.344 | (1583, 1396, 267, 18) | *t* = 16 |
| 3840 × 2160, 1.0 | 1.0, 3840 × 2160 | 1.5 / 2.0 / 2.8125 / 2.6667 → 2.0 | (−640, 0, 5120 × 2160) | 2.0 | (1716, 2094, 398, 28) | *t* = 24: distance-derived from `Verdana23` |
| 3840 × 2160, 1.5 | 1.5, 2560 × 1440 | 1.0 / 1.3333 / 1.875 / 1.7778 → 1.3333 | (−426.67, 0, 3413.33 × 1440) | 2.0 | (1716, 2094, 398, 28) | *t* = 24 |
| 5120 × 1440, 1.0 (C8) | 1.0, 5120 × 1440, *A* 3456 at 832 | 1.3333 / 2.0 / 1.875 / 1.7778 → 1.7778 | (284.44, −480, 4551.11 × 1920): centred, then moved up 240 so the content's bottom meets *G*; 284.44-unit black bars at the sides (the bound, not the content, limits *f*) | 1.778 | (2379, 1381, 353, 25) | *t* = 21: downsampled from `Verdana22` |

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
edge 4310 (its design box reaches 22 units beyond the design width, UL §17.3), inside *F*
but 22 units beyond *A*'s right end 4288 (which I11 allows for, §9.5). With
`ui_safe_aspect = "full"` (C9) the minimap's left would be 822 + 4096 = 4918 and its right
edge 5142, beyond *F*: keep-on-screen moves it to 4896.

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

<a id="s10-6"></a>
### 10.6 Layered fit roots: the theatre layout (SC13)

The theatre layout (1024 × 768) has two fit roots, `StoryField` (content: its whole design
area) and `Theater_LoadingProgress_Parent` (one child, 247 × 49 at (411, 735)). They are one
fit group (§5.4): *U* is 1024 × 768 and *C* covers it, so *f*guard is the contain factor of
*G* and both layers get the same *f* and the same displayed box. The table gives the group's
displayed box and the progress child's displayed position and width, in UI units:

| Configuration | *f*contain / *f*cover / *f*guard / *f*cap → *f* | Both fit roots | Progress child |
|---|---|---|---|
| C1 (1365 × 768) | 1.0 / 1.3330 / 1.0 / 1.3333 → 1.0 | (170.5, 0, 1024 × 768) | (581.5, 735), 247 wide |
| C2 (1920 × 1080) | 1.4063 / 1.875 / 1.4063 / 1.875 → 1.4063 | (240, 0, 1440 × 1080) | (817.97, 1033.59), 347.34 wide |
| C7 (3440 × 1440) | 1.875 / 3.3594 / 1.875 / 2.5 → 1.875 | (760, 0, 1920 × 1440) | (1530.63, 1378.13), 463.13 wide |
| C8 (5120 × 1440) | 1.875 / 5.0 / 1.875 / 2.5 → 1.875 | (1600, 0, 1920 × 1440) | (2370.63, 1378.13), 463.13 wide |

The progress child sits where the story layer's own design places it (at C2: 411 × 1.4063 to
the right of the shared left edge 240, 735 × 1.4063 down), and the parts of *F* left and right
of the shared box are the group's letterbox bars, drawn once below the lower shown layer.
Revision 2 fitted each root on its own: the progress root, with its small content box, got
the cover factor (1.875 at C2, 3.359 at C7, 5.0 at C8), so at C2 its child was at
(770.63, 1018.13) and 463.13 wide, out of line with the story layer.

<a id="s10-7"></a>
### 10.7 Sparse content: the magnification bound (tutorial chooser)

`BaseTutorialChooserField` is a fit root (a group of one) in a 1440 × 900 layout whose only
child is 735 × 231 at (224, 0) (SC13). Its content is small, so *f*guard is large and the
factor would follow the cover factor, which on wide screens grows with the window's width.
The bound *f*cap = 4 ÷ 3 × *Hu* ÷ 900 stops it. *t* is the target height (§6.4.1) of a text in
a 12-pixel font inside the root, as an illustration:

| Configuration | *f*contain / *f*cover / *f*guard / *f*cap → *f* | Displayed box (UI) | *t* for 12 px | Revision 2 *f* |
|---|---|---|---|---|
| C2 (1920 × 1080) | 1.2 / 1.3333 / 2.6122 / 1.6 → 1.3333 | (0, 0, 1920 × 1200) | 16 | 1.3333 |
| C4 (2560 × 1440) | 1.6 / 1.7778 / 3.483 / 2.1333 → 1.7778 | (0, 0, 2560 × 1600) | 21 | 1.7778 |
| C7 (3440 × 1440) | 1.6 / 2.3889 / 4.6803 / 2.1333 → 2.1333 | (184, 0, 3072 × 1920), bars 184 wide | 26 | 2.3889 |
| C8 (5120 × 1440) | 1.6 / 3.5556 / 4.702 / 2.1333 → 2.1333 | (1024, 0, 3072 × 1920), bars 1024 wide | 26 | 3.5556 |
| C9 (5120 × 1440, full) | 1.6 / 3.5556 / 6.2338 / 2.1333 → 2.1333 | (1024, 0, 3072 × 1920), bars 1024 wide | 26 | 3.5556 |
| C12 (3840 × 2160) | 2.4 / 2.6667 / 5.2245 / 3.2 → 2.6667 | (0, 0, 3840 × 2400) | 32 | 2.6667 |

In every row the box is first centred and then moved down so that the content's top meets
*G*'s top, and its height covers *F* (step 4). At 1440 rows the factor is 2.1333 on 21:9 and
32:9 alike, whatever the safe-area setting: it follows the window's height. Up to 16:9 (C2,
C4, C12) the bound does not take effect and the root covers *F* as before.

<a id="s10-8"></a>
### 10.8 Fit containers under a node scale

**(a) The naming window inside its scale parent (SC13).** The mission-controller root
`MPMissionController_MainWindow` is **not** a full-screen root: its design box is 1440 × 900 at
(1, 0) in a 1440 × 900 design area, so under §5.2 ("left ≤ 0 …") it does not cover its design
area, and it is not in the Confirmed SC3 set of covering roots. A read-only probe of the
layout (2026-10-16) found none of the dialog-framework properties (`UICore_Dialog_modal` and
the two overrides, §5.9 of UL) anywhere in the record, so it is not a modal dialog either. It
is therefore a **safe-area panel** (§5.2), laid out against the safe area *A*. Its child
`MPMissionController_Window` has edges (5, 5, 5, 5) and design box covering the root's design
box, so it fills the root's box, which is *A*; the naming field fills it in turn. The two
scale parents form one fit group (§5.4) with *P* = *A* (in the parent's layout coordinates,
*m* = 1 for the parent, whose displayed left is *A*'s left *Ax*), *U* = 1440 × 900 at (0, 0)
and the content box *C* = 411 × 880 at (516, 0) (the union of the naming window, the field at
(610, 0) and the two children of the other scale parent). On screens no wider than the
safe-area aspect ratio, *A* = *F* (so *P* = *F*); this holds at C1, C2 and C5. Only on a wider
screen does *A* lie strictly inside *F*, as at C8 (*A* = 3456 wide at 832 in the 5120-wide
window). Because the group lies inside a safe-area panel, not a full-screen root, §5.4 step 6
draws it **no letterbox bars** at any size; the part of *F* outside *A* is not this group's
concern (it is filled by whatever the pregame screen draws behind the panel). Order (§5.4
"Nesting and fitting order"): the group is
fitted first; then the naming window, a Type C container, is fitted in its parent's
1440 × 900 layout box: fitted position (515, 206), own *f* 0.9976 (§5.4), the same at every
size. Its displayed box is the scale parent's displayed top-left + (515, 206) × *f*A, with the
size 411 × 0.9976 × *f*A by 488 × 0.9976 × *f*A, where *f*A is the group's factor
(*m*(parent) = *f*A), and its content scale is *Se* × *f*A × 0.9976:

| Configuration | *f*contain / *f*cover / *f*guard / *f*cap → *f*A | Scale parents (UI) | Naming window (UI) | *c* | Naming window, physical |
|---|---|---|---|---|---|
| C1 (1365 × 768) | 0.8533 / 0.9479 / 0.8727 / 1.1378 → 0.8727 | (54.14, 0, 1256.73 × 785.45) | (503.59, 179.78, 357.82 × 424.85) | 0.8162 | (472, 169, 336, 398) |
| C2 (1920 × 1080) | 1.2 / 1.3333 / 1.2273 / 1.6 → 1.2273 | (76.36, 0, 1767.27 × 1104.55) | (708.41, 252.82, 503.18 × 597.45) | 1.2243 | (708, 253, 504, 597) |
| C5 (2560 × 1440 at *Se* 1.5) | 1.6 / 1.7778 / 1.6364 / 2.1333 → 1.6364 | (101.82, 0, 2356.36 × 1472.73) | (944.55, 337.09, 670.91 × 796.60) | 2.4486 | (1417, 506, 1006, 1195) |
| C8 (5120 × 1440, *A* = 3456 at 832) | 1.6 / 2.4 / 1.6364 / 2.1333 → 1.6364 | (1381.82, 0, 2356.36 × 1472.73), no bars | (2224.55, 337.09, 670.91 × 796.60) | 1.6324 | (2225, 337, 670, 797) |

In each row the content's top (the field at design top 0) would start above *P* after
centring, so the group box is moved down to top 0 (§5.4 step 3); the content's height,
880 × *f*A, then fills *P*'s height (always *F*'s, since the safe area spans the full height).
At C8 the factor binds on the guard (height) term, which the safe area does not change, so
*f*A stays 1.6364 even though *f*cover drops to 2.4 (it was 3.5556 under the old full-screen
reading), and the scale parents' and the naming
window's **displayed** boxes are the same as they would be against *F*: the safe area is
centred in *F*, so centring the 2356.36-wide group box in *A* (at 832 + (3456 − 2356.36) ÷ 2 =
1381.82) lands at the same absolute left as centring it in *F*, and the content fits with no
horizontal move either way. The only C8 changes from a full-screen reading are *f*cover and
the absence of bars. The naming window keeps its design place in the scaled
picture: at C2 it lies 515 × 1.2273 = 632.05 units right of the scale parents' left edge, as
the other scale parent's children lie at their design places × 1.2273. Read without *m*(parent)
(as revision 3's wording allowed), it would have been displayed at C2 as 410 × 486.81 at
(515, 206) with a content scale of 0.9976, out of line with its screen (and so at every size
other than *f*A = 1).

**(b) A Type A container in a panel with a per-window factor (synthetic, C8).** A safe-area
root panel *R*, 800 × 600 in layout and design size, has a per-window factor *p* = 2 and was
dragged by the user so that its displayed box is (432, 100, 1600 × 1200), across the safe
area's left end (*A* = 3456 wide at 832). Inside it, a Type A container *X* (a group of one),
design 800 × 400 at (0, 100), has one child 200 × 100 at (200, 150).

1. *P* = (0, 0, 800, 600) in *R*'s layout coordinates; *U* = *X*'s design rectangle;
   *C* = 200 × 100 at (200, 150) relative to *U*.
2. *A* taken into *R*'s coordinates (§5.4): left (832 − 432) ÷ 2 = 200, right
   (4288 − 432) ÷ 2 = 1928, top (0 − 100) ÷ 2 = −50, bottom (1440 − 100) ÷ 2 = 670; so
   *G* = (200, 0) … (800, 600), 600 × 600.
3. *f*contain = 1, *f*cover = 1.5, *f*cap = 4 ÷ 3 × 600 ÷ 400 = 2, *f*guard =
   min(600 ÷ 200, 600 ÷ 100) = 3, so *f* = 1.5. The 1200 × 600 group box centred in *P* is at
   (−200, 0); its content spans 100 … 400 horizontally and starts before *G*, so the box moves
   right by 100 to (−100, 0) (step 3), which step 4 keeps (−400 ≤ −100 ≤ 0). Fitted position
   (−100, 0).
4. Displayed (§3.4 item 2, *m*(*R*) = 2): left 432 + (−100) × 2 = 232, top 100, size
   800 × 1.5 × 2 = 2400 by 400 × 1.5 × 2 = 1200; *m*(*X*) = 3, content scale 3. The child's
   displayed left is 232 + 200 × 3 = 832: exactly the safe area's left end.

Without taking *A* into *R*'s coordinates (reading 832 … 4288 as if it were in *R*'s units),
*P* ∩ *A* would be empty, *G* would fall back to *P*, nothing would move the box, and the
child would be displayed at 632, 200 units into the left wing.

---

<a id="s11"></a>
## 11. Deviation from the original (for the P8 parity checklist)

Owner-accepted deviation, decision 0040 §1: Tessera's UI sizing and placement differ from the
original on purpose:

- one global UI scale applied under the UI-unit space, with crisp text and art, instead of
  per-panel magnification (UB §9.2, FO §6.3);
- a 1024 × 768 design floor, a safe area on wide screens, fit containers with a uniform
  factor and a cover fit (layers of one screen fitted as one group, magnification bounded by
  the screen's height, letterbox bars beyond the bound), backdrop images covered instead of tiled, panels fitted to and kept
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
| `ui-layout.md` | §8.4 step 1 (Stretch parents), §8.4 step 6 OPEN, §8.5 Tessera rule, §9.6, §9.9 | answered and superseded by §5 (step 1: §5.2), §6.3, §5.8 |
| `ui-behaviour.md` | §8.2 OPEN (Tessera rule for resizes under a scale), §8.3 OPEN, §8.4, §9 (Tessera rules of §9.2, §9.2.1, §9.3, §9.4) | §3.5 "Moves and resizes", §5.8, §3.4, §6.7, §4.5, §3.2 (zero-sized window) |
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

> **OPEN (owner):** the magnification bound of §5.4, *ρ* = 4 ÷ 3. It keeps every 4:3 layout
> covering screens up to 16:9 and stops sparse full-screen content from growing with the
> window's width (§10.7), at the price of black side bars on wider screens: at 32:9 the
> loading screen shows 284-unit bars (§10.1, C8) and the tutorial chooser 1024-unit bars. A
> larger *ρ* fills more of a wide screen with more magnified content; *ρ* = 1 would never
> crop the design height at all. The owner confirms the value by use on wide screens (goldens
> G3, G8 and the C7, C8 captures).

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
| 2026-10-16 | **Revision 2** (answers the first review's B1–B6; status stays Review). **B1:** full-screen roots with left and right edges 6 are **fit roots** (§5.2): placed by §5.4 with *P* = *F*, *G* = *A*; children keep their design layout; exempt from §5.3, §5.6, keep-on-screen (§5.8), saved placements and I6; clipped to *F*; their letterbox bars are drawn and hit as the fit root (§5.4 step 5). UL §8.4 step 1 (Stretch parents) is not applied: children of the 4 Stretch full-screen roots are laid out by §5.3, of other Stretch roots by steps 2–6 (§5.2, §5.3, §5.9; note added to UL §8.4). **B2:** layout frames (§5.2 "Layout frames"): re-layout subtracts the recorded frame offset before UL §8.4 steps 2–5 and adds the current one after, so mode-0 edges and user moves never drift; §5.3 uses it; new invariant I14. **B3:** empty by design (§5.7): a zero design extent or a zero-extent parent gives extent 0, never negative, not drawn or hit; §5.4 "Zero extents" (fit containers with a zero size are not fitted, *f* = 1), *f*guard leaves out zero terms, an empty content box skips the guard and the content moves, an empty *G* falls back to *P*; I5 restated; backdrop rule needs a design size of at least 1 (§5.5). **B4:** I11 restricted (edges 0/1/3/4/5, design box inside a design area no larger than *A*, no moves or node scales; minimap example), I12 restricted to panels whose size does not depend on *N*, with a one-pixel rounding allowance, I6 exempts fit roots and states the plugin-window rule, I3 reads fit roots clipped to *F*, T3 uses the box ratio *W*′*H*′ ÷ *WH* with a quantisation tolerance; §10.3 notes the minimap beyond *A*. **B5:** §5.4 Type A step 2 is a size test only; step 4 wins over step 3, lands inside *G* whenever a position satisfies both, otherwise the content reaches into the wings (worked counterexample at C8); new invariant I15. **B6:** §6.1/§6.2: a surface under a stretch vector of its own or of an ancestor is painted at its unstretched box and its elements at canvas boxes mapped without the stretch; §6.4.6 takes the pointer into such a canvas. §9.6 tests extended for all of the above. |
| 2026-10-16 | **Revision 3** (answers the second review's R2-B1 and R2-B2; status Revised, §5.2 state still Review). **R2-B1 (a):** Type A fit containers are fitted in **fit groups** (§5.4): all fit roots of one layout, and non-root Type A containers in one root panel whose parents have equal design sizes and equal current boxes relative to their lowest common ancestor (no fit container or node scale in between), share one factor and one map from *P*'s design coordinates to UI units, computed from the group box *U* (union of the members' design rectangles) and the union of the members' content; members shown or not, so toggling a layer moves nothing; letterbox bars drawn once per group below its lowest shown member and hit as its lowest shown fit root (§5.4 steps 5–6; §5.2, §5.6, I3, I6 follow). Census SC13: 14 groups, three of two (theatre fit roots, map decoration and background, mission-controller scale parents). Worked example §10.6, invariant I16. **R2-B1 (b):** magnification bound *f*cap = 4 ÷ 3 × *Ph* ÷ *h* (§5.4 steps 1–2, "About the bound"): Type A factors follow the height, not the width; the loading screen at C8 now has *f* 1.7778 and 284-unit bars (§10.1, *f*cap column added), the step-4 counterexample moves to C2 with a 4:3 safe area; worked example §10.7 (tutorial chooser), invariant I17, owner OPEN on *ρ* (§13); §2 and §11 mention it. **R2-B2:** the Interface window during a slider drag (§4.5) is compensated by *Se*,press ÷ *Se* (the effective scale at the press), its physical top-left is held by converting it with the new *Kx*, *Ky* after each change, §5.6 and keep-on-screen wait for the release; the C1 limited case gives a factor of 1. §9.3 golden G8, §9.4 theatre subject, §9.6 tests for all of the above. Note added to `ui-layout.md` §8.4. |
| 2026-10-16 | **Revision 4** (answers the third review's RV3-B1–RV3-B3; status Revised, §5.2 state still Review). **RV3-B1:** §3.4 item 2 now gives a fit container's displayed box as its parent's displayed top-left + its **fitted position** (in the parent's layout coordinates) × *m*(parent), with the size design size × *f* × *m*(parent); §5.4 works in the parent's layout coordinates throughout (Type A *P*, the safe area taken into the parent's coordinates for *G*, step 5 fitted positions, step 6 bars in *P*'s displayed box; Types B and C give fitted positions); group members share their parents' displayed top-left and *m*; new paragraph "Nesting and fitting order" (parents first, inner fits in the outer design-size box, displayed through the outer factor). SC13 records the Type C `MPMissionController_NamingWindow` inside the Type A `MPMissionController_NamingScaleParent` and the mission-controller group's content; worked example §10.8 (the naming window at C1, C2, C5, C8, fitted position (515, 206) and own factor 410 ÷ 411 from the centring edges; a synthetic Type A under a per-window factor of 2 at C8); invariant I18 and unit tests; real-data golden subject added (§9.4); I16 includes *m*(parent). **RV3-B2:** §3.5 "Moves and resizes": a move divides the pointer offset by the **parent's** *m* (1 for a root panel), a resize divides by the target's *m* and keeps the opposite displayed edge; integer rounding stated; fit containers are never move or resize targets (SC14: no drag or resize bar targets or lies inside one in the install); §4.5 notes the Interface window's parent is the screen root; invariant I19 and unit tests. **RV3-B3:** §3.2 "A window with a zero side": a window size with a zero side changes nothing (*Se*, *F*, *Kx*, *Ky*, *A*, layout, fit factors, displayed boxes, placements kept; §4.5 does not run; no fallback size; *S* changes wait for a non-zero size; the UI is first built at the first non-zero size); §4.3, §4.6 and §5.8 item 3 refer to it; I1 extended; fuzz configurations include zero sides. Note added to `ui-behaviour.md` §8.2. |
| 2026-10-16 | **Revision 5** (answers the fourth review's one blocking and one non-blocking item; status Revised, §5.2 state still Review). **RV4-B1 (§10.8(a), SC13, I18, §9.4):** a read-only probe confirmed the mission-controller root `MPMissionController_MainWindow` is 1440 × 900 at (1, 0) in a 1440 × 900 design area, so under §5.2 it does **not** cover its design area and is not in the Confirmed SC3 covering set; the record holds none of the dialog-framework properties, so it is not modal. It is a **safe-area panel**, laid out against the safe area *A*, so the scale parents' fit group has *P* = *A* (= *F* at C1, C2, C5; *A* = 3456 at 832 at C8), not *F*. SC13 now states the root's rectangle and classification; §10.8(a) is reworded accordingly. The §10.8(a) C8 row is recomputed against *A*: *f*cover 3.5556 → 2.4 and the group draws **no** letterbox bars (§5.4 step 6 applies only inside *F* or a full-screen root). The scale parents' displayed box (1381.82, 0, 2356.36 × 1472.73), the naming window (2224.55, 337.09, 670.91 × 796.60), *c* 1.6324 and the physical box are unchanged, because the safe area is centred in *F* and the content fits with no horizontal move either way, and *f*A 1.6364 binds on the guard (height) term, which the safe area leaves alone (noted in §10.8(a)). C1, C2 and C5 are unaffected (*A* = *F* there). I18 states the no-bars classification and the corrected C8 values; the §9.4 mission-controller subject notes that the C8 golden shows no bars. **RV4-N1 (SC13, §10.6):** the theatre child `Theater_LoadingProgress_Parent` (one child of the progress fit root, at (411, 735)) is 247 × 49, not 247 × 33; the §10.6 numbers use only its X extent (247 at 411) and are unchanged. |
| 2026-10-16 | **Accepted** at revision 5 after an independent re-review (a session that wrote no part of the spec). RV4-B1 verified: a read-only `tessera-formats` probe confirmed `MPMissionController_MainWindow` (0x100010ab in record 0x22000031) is 1440 × 900 at (1, 0) in a 1440 × 900 design area, edges (5, 5, 5, 5), object mode None, with no dialog-framework property anywhere in the record, so it is a safe-area panel (not a full-screen root, not modal; §5.2/SC3 consistent). The corrected §10.8(a) C8 row (*f*A 1.6364 bound on the guard/height term, *f*cover 2.4, scale parents (1381.82, 0, 2356.36 × 1472.73), naming window (2224.55, 337.09, 670.91 × 796.60), physical (2225, 337, 670, 797), no letterbox bars), invariant I18 and the §9.4 C8 golden are mutually consistent; C1/C2/C5 are unaffected (*A* = *F*). RV4-N1 verified: the theatre progress fit root (`Theater_LoadingProgress_Parent`, layout 0x22000059) has one child, 247 × 49 at (411, 735); the §10.6 X extent (247 at 411) is unchanged. No rule, number or section changed; status Revised → Accepted. |
