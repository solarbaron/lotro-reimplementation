<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M5 visual checklist (exit criterion 2)

**Review record:** Claude review session (spec-writer agent), re-review 2026-10-06: **passed**. Both items are repaired. P2-T now says 6 fonts, matching an independent resolver with property replacement by PID. P6-A names the six text buttons the layout shows by default, the two hidden by `UICore_Element_hide` (`MainMenu_LeaveMPButton`, `MainMenu_EndCinematic_Button`) and the close box, describes the gap between Exit and Return correctly, and points to the OPEN `ui-layout.md` §17.5 item (6) with a concrete rule for a differing reference. Every A-row surface count (P1–P15) equals the spec's §17.2 Art-surface cell, which includes cursor images, and the independent recount. Still no game text (element names are layout element-ID labels), and every row is concrete. Earlier record follows.

**Earlier review record:** Claude review session (spec-writer agent), review 2026-10-06: **changes
requested** (two items). The checklist contains no game text. The element names it uses come
from layout element-ID labels; screenshots stay private. Each row is concrete: tolerances,
screen sizes, root rectangles and the D-row elements are named. Verified against the data:
- every G-row rectangle (§17.3);
- the 0xBF Grayscale overlays (D3), the Colorize overlays without an image on the friends
  header button (D4), the five draw-mode-5 images of the refresh button (D5) and the
  0x3CFF0000 Over fill (D6);
- the DrawAfterChildren buttons (D2) and the text-element counts.

Issues:
1. P2-T says "8 fonts referenced". The resolved panel references 6. This error comes from the
   `ui-layout.md` §17.2 count; see that spec's review record.
2. P6-A says "nine text buttons". The menu's button field holds **eight** text buttons, each
   with left, middle and right pieces. Two of them have `UICore_Element_hide` true in the
   layout, so only six show unless game code shows the other two. The ninth button is the close
   box, which is not a text button. State the expected visible set. If game code shows or hides
   these buttons, mark that as OPEN rather than counting them.

Also, if the spec changes its "Art surfaces" cells instead of the note (cursor images), update
the A rows to the new numbers.

**Repair (2026-10-06, spec writer):** item 1: P2-T now says 6 fonts (the spec's §17.2 count was
recounted with property replacement by PID). Item 2: P6-A now names the six text buttons shown
by default, the two hidden ones and the close box, and points to the new OPEN `ui-layout.md`
§17.5 item (6). The spec changed its Art-surface cells to include cursor images, so the A rows
of P1, P2, P4, P5, P7, P8, P9, P10, P11, P12, P13 and P14 carry the new numbers. Awaiting
re-review.

**Revision (2026-10-23, spec writer):** the P15 rows are rewritten for the bag window `containerdisplay` / `PlayerBag1_Template` of `ui-layout.md` §17.2–§17.4 (U42, U45), with the root geometry and the counts of those sections; the self-check row P15 points to P15-G. **Reviewed 2026-10-23** (spec-review:spec-ui-input:2, an independent session that wrote no part of it): **passed**. The rows agree with `ui-layout.md` §17.2–§17.4: root (599, 164, 197, 183) at every screen size, edges (0, 0, 0, 0), flag 0, z 0, `loc`; 61 elements, 2 text elements, 7 string refs, 3 fonts, 47 surfaces, 67 states; the store button hidden as a Tessera choice; bags 2–6 the same layout (U45). No game text; element names are layout labels.

Pass/fail record for [ROADMAP M5](../ROADMAP.md#m5) criterion 2: "at least 10 native panels
render with the right art and geometry (checklist compared with screenshots taken by the
owner; screenshots stay private)".

The panels, their layouts, root elements and expected values come from
[`ui-layout.md`](../specs/ui-layout.md) §17 (catalogue P1–P15), §8 (layout), §9.3 (drawing
order), §9.6 (images and draw modes) and §9.7 (button states). Text only: the owner's
screenshots and Tessera's screenshots stay in a private scratch directory and are deleted after
the comparison. No game text is recorded here; a text row records only whether text is present,
in the right place, font, size, colour and justification.

## Method

- **Install and date.** The owner's Steam install, read in place (`TESSERA_GAME_DIR`). Record
  the data date when the checks are run. Expected values in this file are observations of the
  install on 2026-10-06; if the data changed, recompute them from the spec before judging.
- **Reference screenshots (owner).** Taken in the original client at a known screen size (record
  it), interface scale 1.0 (`UIScale_Global` in the owner's preferences was 1.00 on
  2026-10-06), no user skin active. For each panel: one screenshot in its normal look, plus the
  states listed in its S row (rollover, pressed, ghosted, …) where the owner can produce them.
- **Saved positions.** The original restores panels that carry save properties
  (`ui-layout.md` §9.9) to where the player left them. A position check (G row, "position")
  is judged only when the owner's screenshot shows the panel at its default place (for example
  on a character whose panels were never moved); otherwise mark the position part "n/a (saved)"
  and judge the size only. Sizes are always judged; a panel with `save_size` that the owner
  resized is judged against the owner's size and noted.
- **Tessera side.** The client with the sandbox backend (world walker), opening each panel by
  `ui-layout.md` §17.1 at the same screen size as the reference, with no saved location and no
  skin. Note the exact command line in the Note column the first time it is used.
- **Geometry (G).** Root rectangle (x, y, width, height) in screen pixels as computed by
  `ui-layout.md` §8.4 (values for 1920 × 1080 and 2560 × 1440 are in §17.3; compute other sizes
  the same way). Tolerance 0 pixels for Tessera's own computed rectangle (log it); against the
  screenshot, ±1 pixel for edges read by eye. Children: frame pieces meet without gaps or
  overlaps; nothing is drawn outside the root (§9.3).
- **Art (A).** Every image the panel shows is the same image as in the reference, at native
  size, tiled (never stretched) along frame edges and backgrounds (§9.6), with no missing or
  placeholder art. Draw modes other than plain alpha blending follow §9.6.2–§9.6.3; judge by eye
  (the final composition onto the frame is OPEN in §9.6.1, so exact pixel values are not
  required).
- **Text (T).** Labels and static text appear where the reference has them, in the same font
  face and size class, colour and justification (`fonts.md`); no text is missing or replaced by
  a token. Text that the sandbox fills (names, values) is judged for placement and style only.
- **States (S).** The default state after opening (§5.7, §9.2 step 8), and the listed button
  states (§9.7) under the pointer: normal, rollover, pressed, and ghosted where the panel has
  ghosted elements. Drag-box frames (P1, P4, P5, P11, P14) follow the Tessera choice of
  `ui-layout.md` §17.5 until the owner's reference settles that OPEN; record what the
  reference shows.
- **Drawing rules (D).** A few rows check §9.3 and §9.6 directly on elements of the panels.

A panel passes when its G, A, T and S rows pass. The criterion passes when at least 10 panels
pass.

## Result

Not yet run. Fill in the table below and the Result columns when the owner's screenshots and
Tessera's M5 client are available.

| Group | Checks | Pass | Fail | n/a |
|---|---|---|---|---|
| G: geometry | 15 | | | |
| A: art | 15 | | | |
| T: text | 15 | | | |
| S: states | 15 | | | |
| D: drawing rules | 8 | | | |
| Panels passing all four rows | 15 | | | |

## Tessera-side self-check (2026-10-08, clean-room implementer)

**Owner comparison pending.** This section records only Tessera's side of criterion 2: each
panel opened by `ui-layout.md` §17.1 in the client's walker with the game UI attached, its
root rectangle as the client computed it (logged by the client), and a look at Tessera's own
screenshot against the A, T and S rows above (no reference screenshot was available, so the
Result columns of the rows stay empty until the owner's screenshots exist). The screenshots
were kept in a private scratch directory and deleted after the check. Data of the owner's
install as of 2026-10-08; no skin; 1920 × 1080; no cursor in screenshots.

Command line (one run per panel; `P5` also selects a sandbox target so that it shows):

```sh
tessera-client --open-panel P<n> --no-ui-hud --walker-hud off --size 1920x1080 \
    --screenshot <scratch>/p<n>.png --data-dir <scratch>/data
```

The HUD set together: `tessera-client --ui --walker-hud off --size 1920x1080 --screenshot
<scratch>/hud.png`. Real-data tests of the same host: `cargo test --release -p
tessera-client --test ui_panels_real_data` (HUD set, Escape → P6, `/plugins manager` → P12,
minimap position update) and `--test panels_real_data` (G rows at 1920 × 1080 and
2560 × 1440, A-row census).

| Panel | G (computed root, 1920 × 1080) | A (Tessera screenshot) | T | S (default state) | Self-check |
|---|---|---|---|---|---|
| P1 | (1718, 12, 224, 280) = row | rim, buttons, compass, player marker; map tile drawn | coordinate and area labels empty (client fills neither yet) | default; drag-box frame hidden | G, A pass; T partial |
| P2 | (5, 756, 313, 199) = row | frame, tab strip, entry art | two tab labels, the system welcome line, channel label in the entry | General tab selected | pass |
| P3 | (448, 880, 1024, 200) = row | toolbar background, quickslot frames, experience bar frame, menu buttons | no visible text in the sandbox | `toolbar_noncombat` | pass (T not judgeable) |
| P4 | (2, 2, 280, 300) = row | portrait ring, morale and power frames and fills | name and level empty (sandbox data absent) | meters as the layout sets them | G, A pass; T not judgeable |
| P5 | (279, 2, 305, 300) = row | frame and rank art; a light placeholder fill where the target portrait goes | the layout's default name text, drawn twice (game code would set it) | shown with a target, hidden without | G pass; A, T partial |
| P6 | (844, 401, 232, 278) = row | frame, title bar, close box, six buttons with the gap | five labels; the fourth button (`MainMenu_Logout_Button`) has no label | normal | G, A pass; T partial (one label missing) |
| P7 | (224, 120, 577, 534) = row | frame, search field, page buttons, side art | title and page button labels; two of the three bottom buttons have no label | first page button highlighted | G, A pass; T partial |
| P8 | (262, 118, 587, 577) = row | frame, five tabs, filters, empty list, buttons | tab, field and button labels | Fellowing tab selected | pass |
| P9 | (9, 118, 770, 550) = row | frame, category icons, empty trees, buttons | title, option and button labels; the first bottom button has no label | normal | G, A pass; T partial |
| P10 | (0, 0, 1038, 802) = row | frame, buttons and the top-level world map | menu labels and map names; title bar empty (no area) | normal | G, A pass; T partial |
| P11 | (1566, 223, 340, 550) = row | nothing visible (empty tracker; drag-box frame hidden per §17.5) | — | — | G pass; A, T, S not judgeable |
| P12 | (0, 0, 600, 400) = row | frame, tree, about and options tabs, buttons | tab and button labels; the about pane shows the layout's placeholder plugin entry | normal | G, A pass; T partial |
| P13 | (0, 0, 500, 580) = row | frame, lineage tree art, description pane, buttons | lineage labels and buttons; title bar empty | normal | G, A pass; T partial |
| P14 | (0, 650, 420, 55) = row | nothing visible (empty bar; drag-box frame hidden per §17.5) | — | — | G pass; A, T, S not judgeable |
| P15 | (599, 164, 197, 183) = `ui-layout.md` §17.3 = row P15-G | frame, title, three icons, search field | title label | store button hidden | pass against the spec |

Summary (Tessera side only): G 15 / 15; A pass for 12 panels, 1 partial (P5), 2 not
judgeable (P11, P14); T pass for 3 panels, partial for 8 (labels that game code would set or
that the client does not resolve yet), not judgeable for 4; S default states as expected for
13. Rollover and pressed states (S rows) were not exercised in screenshots; they are covered
by the synthetic behaviour tests of `tessera-ui`. Criterion 2 waits for the owner's screenshots.

Re-check after the review fixes of 2026-10-08 (Tessera side): session chat lines now wait
for the chat window until the UI attaches, so the sandbox's welcome line is always in P2's
General log (before, it depended on timing); lines with a sender are posted too, each
reaching `Turbine.Chat.Received` once. Checked with `tessera-client --ui --walker-hud on
--size 1280x720 --screenshot <scratch>/hud_on.png --data-dir <scratch>/data` (welcome line
in the General log; screenshot deleted after the check) and by `cargo test --release -p
tessera-client --test ui_review_real_data` (one `Received` per session line through the
chat window, `ui_scale_global` on P7 opened after the HUD, no string table read on the main
thread). The walker's screenshot action (F12 / PrintScreen) now includes the UI pass and,
like `--screenshot`, never the software cursor.

## P. Panels

Columns: the check id, the panel (§17.2 number), what is expected, the owner's reference
(screenshot id in the private scratch directory, screen size), the result (pass, fail, n/a) and
a note.

### P1 Minimap (`gameplayactivatables` / `GamePlay_RadarBox`, child based on `radar` / `MinimapWindow`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P1-G | Root (1718, 12, 224, 280) at 1920 × 1080, (2358, 12, 224, 280) at 2560 × 1440: pinned to the top-right corner (edges 4, 5, 5, 4) | | | |
| P1-A | Minimap rim and buttons from the layout; 167 distinct surfaces referenced (not all visible). The map image inside the rim is OPEN (§17.4); judge the rim, buttons and frame only | | | |
| P1-T | 3 text elements in place and style | | | |
| P1-S | Buttons on the rim: normal and rollover images; drag-box frame per §17.5 | | | |

### P2 Chat window (`chatwindow` / `NewChatWindow1`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P2-G | Root (5, 756, 313, 199) at 1920 × 1080, (5, 1116, 313, 199) at 2560 × 1440: pinned to the bottom-left corner (edges 5, 4, 4, 5). Position and size usually saved (n/a (saved) allowed) | | | |
| P2-A | Window frame, tab strip and entry-line art; 13 distinct surfaces | | | |
| P2-T | Chat text and entry line use the chat fonts (6 fonts referenced); one tab label | | | |
| P2-S | Tab normal and selected looks; buttons rollover | | | |

### P3 Toolbar (`toolbar` / `ToolbarField`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P3-G | Root (448, 880, 1024, 200) at 1920 × 1080, (768, 1240, 1024, 200) at 2560 × 1440: centred horizontally, bottom-anchored (edges 1, 4, 1, 5); keep-inside flag set | | | |
| P3-A | Toolbar background, quickslot frames, experience bar frame and menu buttons; 150 distinct surfaces. Class pips hidden in the sandbox (§17.4) | | | |
| P3-T | 4 text elements in place and style | | | |
| P3-S | Default state `toolbar_noncombat`; menu buttons normal, rollover, pressed | | | |

### P4 Player vitals (`toolbar` / `AvatarPanel`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P4-G | Root (2, 2, 280, 300) at every screen size (edges 5, 5, 4, 4: top-left) | | | |
| P4-A | Panel frame, morale and power bar frames and fills; 313 distinct surfaces referenced. Portrait content optional (§17.4) | | | |
| P4-T | Name and level text; meter numbers if shown by the reference | | | |
| P4-S | Drag-box frame per §17.5; meters full in the sandbox | | | |

### P5 Target vitals (`toolbar` / `OpponentPanel`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P5-G | Root (279, 2, 305, 300) at every screen size (edges 5, 5, 4, 4) | | | |
| P5-A | Target frame, vitals bars, class and rank icons as the selected figure needs; 572 distinct surfaces referenced | | | |
| P5-T | Target name and level text | | | |
| P5-S | Shown when a sandbox figure is selected, hidden with no selection (Tessera choice, §17.4); drag-box frame per §17.5 | | | |

### P6 Main menu (`mainmenu` / `MainMenuField`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P6-G | Root (844, 401, 232, 278) at 1920 × 1080, (1164, 581, 232, 278) at 2560 × 1440: centred (edges 1, 1, 1, 1); width 232, one less than the design width 233 (§17.3) | | | |
| P6-A | Menu frame, title bar and close box, and the six text buttons that the layout shows by default (`MainMenu_Help_Button`, `MainMenu_Options_Button`, `MainMenu_EditUI_Button`, `MainMenu_Logout_Button`, `MainMenu_Exit_Button`, `MainMenu_Return_Button`), each with left, middle and right pieces, at their stored positions, with the empty slot of the hidden `MainMenu_LeaveMPButton` between Exit and Return. `MainMenu_LeaveMPButton` and `MainMenu_EndCinematic_Button` are hidden (`UICore_Element_hide`); 36 distinct surfaces. Whether game code shows, hides or moves these buttons is OPEN (`ui-layout.md` §17.5 item (6)): if the reference shows a different set or no gap, record it in the Note and judge the six buttons only | | | |
| P6-T | Button labels drawn on top of the button frame pieces (D2), in the menu font and colours | | | |
| P6-S | Each button: normal, rollover, pressed images (§9.7); a pressed button plays its click sound | | | |

### P7 Options (`options` / `BaseOptionsField`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P7-G | Root (224, 120, 577, 534) at every screen size (edges 0, 0, 0, 0); position may be saved | | | |
| P7-A | Window frame, page tree, scroll bars, check boxes, sliders and buttons built from the layout's templates; 58 distinct surfaces | | | |
| P7-T | Title, tree labels and button labels in place; page entries are Tessera's own (§17.4), judged for style only | | | |
| P7-S | Okay, Revert and Default buttons: normal, rollover, pressed; check box on and off | | | |

### P8 Social (`social` / `Social_Field`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P8-G | Root (262, 118, 587, 577) at every screen size; position may be saved | | | |
| P8-A | Window frame, tabs, list headers, empty lists and buttons; 106 distinct surfaces | | | |
| P8-T | 61 text elements: tab labels, column headers, button labels (lists empty) | | | |
| P8-S | Tabs: selected and unselected; ghosted buttons desaturated (D3); header button highlight tinted (D4) | | | |

### P9 Quest log (`quest` / `QuestPanel`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P9-G | Root (9, 118, 770, 550) at every screen size; position and size may be saved | | | |
| P9-A | Window frame, empty category tree, detail pane frame, buttons; 122 distinct surfaces | | | |
| P9-T | Title and button labels; empty log | | | |
| P9-S | Buttons normal, rollover, pressed; buttons that need a selected quest are ghosted if the reference shows them so | | | |

### P10 World map (`map` / `MapPanel`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P10-G | Root (0, 0, 1038, 802) by layout; larger than a 1024 × 768 screen; position and size usually saved | | | |
| P10-A | Map window frame, resize bars and buttons; 25 distinct surfaces. The map image itself is OPEN (§17.4); judge the frame only | | | |
| P10-T | One text element in place | | | |
| P10-S | Buttons normal and rollover | | | |

### P11 Quest tracker (`gameplayactivatables` / `GamePlay_QuestTrackerBox`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P11-G | Root (1566, 223, 340, 550) at 1920 × 1080, (2206, 223, 340, 550) at 2560 × 1440 (edges 4, 5, 5, 4) | | | |
| P11-A | Tracker box art; 12 distinct surfaces; nothing tracked in the sandbox | | | |
| P11-T | One text element (the drag-box title, part of the frame, §17.5) | | | |
| P11-S | Drag-box frame per §17.5 | | | |

### P12 Plugin manager (`pluginmanager` / `PluginManager`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P12-G | Root (0, 0, 600, 400) by layout; the original may place it elsewhere when it opens it (not traced) | | | |
| P12-A | Window frame, plugin tree, info pane with about and options tabs, toolbar buttons; 93 distinct surfaces | | | |
| P12-T | Tab labels and button labels on top of the button frames (D2); plugin names from the user's plugin folder | | | |
| P12-S | Load and Unload buttons normal, rollover, pressed; refresh button in its five states (D5) | | | |

### P13 Character journal (`character_journal` / `CharacterJournalPanel`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P13-G | Root (0, 0, 500, 580) by layout; position may be saved | | | |
| P13-A | Window frame, page frame, lineage displays, buttons; 77 distinct surfaces | | | |
| P13-T | 12 text elements: titles and the character's name and level (sandbox values) | | | |
| P13-S | Buttons normal, rollover, pressed | | | |

### P14 Extra shortcut bar (`gameplayactivatables` / `GamePlay_Undocked_CtrlBar`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P14-G | Root (0, 650, 420, 55) at every screen size (edges 0, 0, 0, 0); position usually saved | | | |
| P14-A | Bar frame and empty slot frames as the bar lays them out; 16 distinct surfaces | | | |
| P14-T | One text element (the drag-box title, part of the frame, §17.5) | | | |
| P14-S | Drag-box frame per §17.5; empty slots | | | |

### P15 Bag (`containerdisplay` / `PlayerBag1_Template`)

| # | Expected | Reference | Result | Note |
|---|---|---|---|---|
| P15-G | Root (599, 164, 197, 183) at 1024 × 768, 1920 × 1080 and 2560 × 1440 (`ui-layout.md` §17.3; edges 0, 0, 0, 0; flag byte 0; z 0); location saved (`loc`). Bag 1 is judged; bags 2–6 (`PlayerBag2_Template` … `PlayerBag6_Template`) are the same layout with other roots (U45) | | | |
| P15-A | 61 resolved elements, 47 distinct surfaces: the frame (`BackpackBackground`, based on `Box_Silver_BlueGradient`), the title bar with its bag icon, the mini bag icon, the drag bar, the close box (`Backpack_Base_Close_Button`), the resize bar (`Inventory_ResizeBar`) with its grip, the toolbar (`Backpack_Toolbar`: edit-mode, lock-mode and sort buttons, the search field with its find icon) and the empty item list (`Backpack_ItemList`) | | | |
| P15-T | 2 text elements: the title (`Bag_TitleText`) and the search entry (`ToolbarFind_Entry`, empty); 7 string refs and 3 fonts in the panel | | | |
| P15-S | 67 states over the panel; default states; the store button (`BuyInvSpace_Button`, `BuyNowButton`) hidden (Tessera choice, `ui-layout.md` §17.4); the item list shows no items | | | |

> **Resolved (spec writer, 2026-10-23):** the rows above now describe the bag window of
> `ui-layout.md` §17.2–§17.4 (`containerdisplay` / `PlayerBag1_Template`, U42, U45) with its
> geometry and counts; the earlier rows for the retired `backpack` / `RealBackpackField` panel
> are gone. As the implementer noted on 2026-10-08: root (599, 164, 197, 183) at every screen
> size, 47 distinct surfaces, 2 text elements, the store button hidden.

## D. Drawing rules

| # | Rule (`ui-layout.md`) | Where to look | Expected | Reference | Result | Note |
|---|---|---|---|---|---|---|
| D1 | Sibling order, lower z in front (§9.3) | Any window title bar and close button (P7, P8, P9, P12) | Title bar and close button drawn over the window frame | | | |
| D2 | `UICore_Element_DrawAfterChildren` (§9.3, §9.6) | P6 menu buttons, P7 Okay/Revert/Default, P12 Load/Unload | Label text on top of the button's frame pieces | | | |
| D3 | Grayscale overlay (§9.6.3) | P8 ghosted buttons, for example the anonymous toggle in its ghosted state | Button desaturated and lightened toward its brightest channel (colour alpha 0xBF), not darkened | | | |
| D4 | Colorize overlay without image (§9.6.3) | P8 friends header button in highlight and rollover | Button art takes the hue of the overlay colour, keeping its own brightness | | | |
| D5 | Draw mode 5 "Overlay" on an image (§9.6.2, Mix colour) | P12 refresh button | The icon replaces the colour under it by its alpha; the destination's alpha stays | | | |
| D6 | Over with a colour fill (§9.6.3) | P12 about-pane header without an image | A faint red fill (alpha 0x3C) over the pane background | | | |
| D7 | Tiling at native size (§9.6) | Long frame edges of P7 or P8 | Edge pieces repeat; corners not stretched | | | |
| D8 | Clipping to the parent (§9.3) | Any scrolled list or the P2 chat log | Nothing drawn outside the list's or window's box | | | |
