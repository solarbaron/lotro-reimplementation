<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M4 visual checklist (exit criteria 2 and 3)

Pass/fail record for [ROADMAP M4](../ROADMAP.md#m4) criteria 2 and 3:

- **Criterion 2:** "for every playable species and body type, an assembled character plays
  idle, walk and run without visible skinning errors";
- **Criterion 3:** "worn items replace body parts correctly and held items attach to the right
  bones on a checklist of at least 30 items".

Plus a few static NPC spots for the sandbox backend ("static NPCs placed where the data places
them"). Text only: screenshots stay in a scratch directory and are deleted; no game names are
recorded. Descriptions are our own words.

## Method

- Install: the owner's Steam install, read in place (`TESSERA_GAME_DIR`). Record the data date
  when the checks are run.
- Tools: the explorer's character viewer (M4 deliverable) for rows B, W, L and H; the client
  with the sandbox backend (world walker) for rows N. Default viewer settings as in
  [`m2-visual.md`](m2-visual.md) unless a row says otherwise. Note the exact command line
  of each run in the Note column the first time it is used.
- Assembly follows [`appearance.md`](../specs/appearance.md) §8: setup default parts,
  appearance lists, worn items by priority, held items at the wearer's holding locations
  ([`skeleton-anim.md`](../specs/skeleton-anim.md) §8.4). Clips are the ones
  `script-table.md` selects for idle, walk and run; record the clip DID in the row.
- Item DIDs are 0x79 object property sets. Expected parts are given for the Human male
  wardrobe body (`appearance.md` §2.3) unless stated; "stub" means the slot is hidden.
- Checked per body row: no stretched or exploding vertices, no gaps at part seams (neck,
  wrists, waist, ankles), no part left in bind pose while the rest moves, feet on the ground
  plane in idle, no visible foot sliding beyond what the clip itself has, correct overall
  height for the species (`appearance.md` §6.1).
- Checked per worn row: the item's own part is present, the slots the spec says it hides are
  hidden, the slots it does not touch keep their previous mesh, no body skin pokes through,
  dye default (`Item_ClothingColor`) applied.
- Checked per held row: the item is in the expected location, its grip in the hand bone's
  frame (blade or long axis along the forearm direction for in-hand locations), and it follows
  the bone through idle, walk and run.

## Result

Run 2026-10-05 (clean room) on the owner's Steam install as it was that day, with the
explorer's character viewer in its headless sheet mode (`--sheet`, M4): every row's figure
is assembled by `tessera-mesh`, its clip chosen by the script tables
(`script-table.md` §11.3), sampled by `tessera-anim` on the asset workers and drawn by
`tessera-render`'s skinned pipeline, the same path as the interactive view. Each sheet has
three camera angles (front-left 160°, side 70°, back 340°) by several play times; every
sheet was viewed. The interactive view reproduces a tile with `--screenshot out.png
--character … --wear … --clip … --time T --mesh-angle YAW,PITCH`. Sheets and screenshots
stayed in the clean scratch directory and were deleted.

| Group | Checks | Pass | Fail |
|---|---|---|---|
| B: bodies × idle, walk, run | 42 | 36 | 6 |
| W: single worn items | 22 | 22 | 0 |
| L: layering combinations | 8 | 8 | 0 |
| H: held items | 25 | 25 | 0 |
| N: static NPC spots (sandbox) | 7 | 7 | 0 |

Notes:

1. **B37–B42 (River-hobbit) fail only on "feet on the ground plane"**: skinning is clean,
   but with the two hidden `Femur_Scale` sliders applied as the `appearance.md` §6.3
   Decision says (local scale Z 0.8 on `lfemur`/`rfemur`, 1.15 on `lknee`/`rknee`, children
   inheriting the scale by full matrix composition, `skeleton-anim.md` §5.4) the legs get
   shorter while the pelvis height stays, so the feet float about 0.09 m above the ground
   (lowest bone 0.10 m at scale 0.7; 0.015 m without the sliders, like the other bodies).
   **OPEN** (O9, reported to the spec owner): the slider application frame and how a scaled
   bone's children and the root height follow. Everything else of these rows passes.
   A second unexplained input to these rows: the River-hobbit script tables also start a
   looping clip on layer 110 `scale_blend`, 0x05008E76 (a candidate of idle, walk and run
   for `riverhobbit,m`, weight 1; Human M and Elf F have none; `script-table.md` §8.3 OPEN
   already notes that River-hobbit bodies reach a layer-110 clip). The viewer does not play
   it. It has 89 tracks over 1.666 s, track weights 0.01 on the root, femur, knee, ankle,
   eye and finger bones and 0 elsewhere; its root translation runs from z −12.5 m at t = 0
   to −0.058 m at the end, its knee and ankle translations from +5.55 m to −0.42 m. Played
   alone it pulls the figure apart (lowest bone −8.52 m on `riverhobbit,m`, −12.17 m on
   `human,m` at t = 0, `--sheet … --clip 0x05008E76`, re-measured 2026-10-05). Layered over
   idle with the `skeleton-anim.md` §6.6 weighted rule (base + w·(layer − base)) it moves
   the lowest bone only from 0.104 m to 0.088–0.104 m at scale 0.7, so it does not explain
   the 0.09 m float (candidates, weights, ranges and the layered figures measured in review
   with a since-deleted test). **OPEN** (reported with O9, for `script-table.md` §6.2 or
   `appearance.md` §6.2/§6.3): what drives the `scale_blend` clip's time (perhaps
   `AC_Avatar_Height`, which only River-hobbit entities carry) and how that layer combines
   with the others.
2. **Standing heights** (bind-pose top × model scale) agree with `appearance.md` §2.2/§6.1
   within 1 cm for every body except DwarfStoutAxe (1.42 m against 1.36 m): its
   appearance list gives it another head, hair and beard than the Dwarf default body the
   §2.2 bind height was measured on; its scale is 1.0 as expected.
3. **Grip axis (H rows)**: every in-hand item sits at its location's bone and follows it;
   its long axis leaves the fist along the item's +Y, which `skeleton-anim.md` §8.3 gives as
   *forward* in the bind pose, that is across the forearm (the arms point sideways there).
   The method above asks for the long axis *along* the forearm. **OPEN** (reported): which
   of the two is meant; the rows pass on the location, bone and following checks.
4. Clip DIDs of the B rows equal the table of `script-table.md` §8.3 for all 14 bodies.
5. **N rows** were checked in the client's sandbox backend (world walker), not in the
   character viewer: `apps/client/tests/npc_spots_real_data.rs` (release, headless) spawns
   next to each spot and finds the spot's entity at the data's position and heading, drawn
   as a character, with every figure around it ready (values in `walker.md` §8.1); N1 and
   N4 were also viewed in walker screenshots (verification run, 2026-10-05). Their entities
   also assemble in the character viewer, and N1–N6 play an idle clip there
   (`--character 0x47004E6F`, `0x47004E75`, `0x47004FB6`, `0x47004E74`, `0x7006986E`; the
   N7 marker 0x47006915 has none). Agreement with the original client is O17 (and O12 for
   N5).
6. **Exit verification, 2026-10-05:** all 42 B sheets (idle, walk, run × 14 bodies, 4 play
   times × 2 angles) and one tile per W, L and H row, plus side views of the long weapons
   and stowed views, were re-rendered with the explorer and viewed; results as above. The
   stowed placement of H21 (medium shield) was wrong before this check (the item's
   equipment category has a bit above 31, which `tessera-formats` dropped, so it stayed in
   hand); it now stows on the back (`Sheath_Back_Shield`, bone 16) like the other shields,
   and `assembly_real_data` checks all 25 stowed rows without exceptions.
7. **Owner look, 2026-10-05:** the owner ran Tessera on the laptop (RTX 2070 Super Max-Q)
   and judged the characters' positions and heights fine by eye. This is a look at
   Tessera's own output, with no side-by-side screenshots and no `/loc` readings, and it
   does not say which bodies were looked at. So it does not settle the River-hobbit
   foot-contact rows (B37–B42, note 1) or the measured comparisons with the original client
   (O8, O9, O17).

## B. Bodies (criterion 2)

Body numbers are `appearance.md` §2.2. Scale = template `vec3` × entity scale (§6.1).

| # | Body | Avatar entity | Setup | Scale | Clip | Clip DID | Result | Note |
|---|---|---|---|---|---|---|---|---|
| B1 | P1 Hobbit (0x51) M | 0x47000606 | 0x04000551 | 1.0 | idle | 0x05000DF8 | pass | standing height 1.163 m; feet on the ground (lowest bone 0.016–0.017 m); `tessera-explorer --sheet B1.png --character hobbit,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B2 | P1 Hobbit (0x51) M | 0x47000606 | 0x04000551 | 1.0 | walk | 0x05000E03 | pass | standing height 1.163 m; `tessera-explorer --sheet B2.png --character hobbit,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B3 | P1 Hobbit (0x51) M | 0x47000606 | 0x04000551 | 1.0 | run | 0x05000E61 | pass | standing height 1.163 m; `tessera-explorer --sheet B3.png --character hobbit,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B4 | P2 Hobbit (0x51) F | 0x47000607 | 0x04000552 | 1.0 | idle | 0x05000DFD | pass | standing height 1.142 m; feet on the ground (lowest bone 0.015–0.015 m); `tessera-explorer --sheet B4.png --character hobbit,f --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B5 | P2 Hobbit (0x51) F | 0x47000607 | 0x04000552 | 1.0 | walk | 0x050011E2 | pass | standing height 1.142 m; `tessera-explorer --sheet B5.png --character hobbit,f --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B6 | P2 Hobbit (0x51) F | 0x47000607 | 0x04000552 | 1.0 | run | 0x050011C1 | pass | standing height 1.142 m; `tessera-explorer --sheet B6.png --character hobbit,f --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B7 | P3 Human (0x17) M | 0x4700060A | 0x04000553 | 1.0 | idle | 0x05001473 | pass | standing height 1.811 m; feet on the ground (lowest bone 0.022–0.023 m); `tessera-explorer --sheet B7.png --character human,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B8 | P3 Human (0x17) M | 0x4700060A | 0x04000553 | 1.0 | walk | 0x05008D08 | pass | standing height 1.811 m; `tessera-explorer --sheet B8.png --character human,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B9 | P3 Human (0x17) M | 0x4700060A | 0x04000553 | 1.0 | run | 0x05008D27 | pass | standing height 1.811 m; `tessera-explorer --sheet B9.png --character human,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B10 | P4 Human (0x17) F | 0x4700060B | 0x04000554 | 1.0 | idle | 0x050015B4 | pass | standing height 1.772 m; feet on the ground (lowest bone 0.009–0.009 m); `tessera-explorer --sheet B10.png --character human,f --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B11 | P4 Human (0x17) F | 0x4700060B | 0x04000554 | 1.0 | walk | 0x05008998 | pass | standing height 1.772 m; `tessera-explorer --sheet B11.png --character human,f --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B12 | P4 Human (0x17) F | 0x4700060B | 0x04000554 | 1.0 | run | 0x0500899A | pass | standing height 1.772 m; `tessera-explorer --sheet B12.png --character human,f --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B13 | P5 Elf (0x41) M | 0x4700060E | 0x04000555 | 1.03 | idle | 0x050017FE | pass | standing height 1.860 m; feet on the ground (lowest bone 0.023–0.023 m); `tessera-explorer --sheet B13.png --character elf,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B14 | P5 Elf (0x41) M | 0x4700060E | 0x04000555 | 1.03 | walk | 0x050093BC | pass | standing height 1.860 m; `tessera-explorer --sheet B14.png --character elf,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B15 | P5 Elf (0x41) M | 0x4700060E | 0x04000555 | 1.03 | run | 0x050093C0 | pass | standing height 1.860 m; `tessera-explorer --sheet B15.png --character elf,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B16 | P6 Elf (0x41) F | 0x4700060F | 0x04000556 | 1.02 | idle | 0x05001804 | pass | standing height 1.793 m; feet on the ground (lowest bone 0.014–0.015 m); `tessera-explorer --sheet B16.png --character elf,f --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B17 | P6 Elf (0x41) F | 0x4700060F | 0x04000556 | 1.02 | walk | 0x050093A0 | pass | standing height 1.793 m; `tessera-explorer --sheet B17.png --character elf,f --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B18 | P6 Elf (0x41) F | 0x4700060F | 0x04000556 | 1.02 | run | 0x05009397 | pass | standing height 1.793 m; `tessera-explorer --sheet B18.png --character elf,f --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B19 | P7 Dwarf (0x49) M | 0x47000611 | 0x04000557 | 1.0 | idle | 0x05001D3C | pass | standing height 1.357 m; feet on the ground (lowest bone 0.030–0.031 m); `tessera-explorer --sheet B19.png --character dwarf,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B20 | P7 Dwarf (0x49) M | 0x47000611 | 0x04000557 | 1.0 | walk | 0x05001D3B | pass | standing height 1.357 m; `tessera-explorer --sheet B20.png --character dwarf,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B21 | P7 Dwarf (0x49) M | 0x47000611 | 0x04000557 | 1.0 | run | 0x05001D18 | pass | standing height 1.357 m; `tessera-explorer --sheet B21.png --character dwarf,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B22 | P8 Beorning (0x72) M | 0x47007004 | 0x04000553 | 1.03 | idle | 0x05001473 | pass | standing height 1.868 m; feet on the ground (lowest bone 0.022–0.024 m); `tessera-explorer --sheet B22.png --character beorning,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B23 | P8 Beorning (0x72) M | 0x47007004 | 0x04000553 | 1.03 | walk | 0x05008D08 | pass | standing height 1.868 m; `tessera-explorer --sheet B23.png --character beorning,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B24 | P8 Beorning (0x72) M | 0x47007004 | 0x04000553 | 1.03 | run | 0x05008D27 | pass | standing height 1.868 m; `tessera-explorer --sheet B24.png --character beorning,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B25 | P9 Beorning (0x72) F | 0x47007005 | 0x04000554 | 1.03 | idle | 0x050015B4 | pass | standing height 1.810 m; feet on the ground (lowest bone 0.010–0.010 m); `tessera-explorer --sheet B25.png --character beorning,f --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B26 | P9 Beorning (0x72) F | 0x47007005 | 0x04000554 | 1.03 | walk | 0x05008998 | pass | standing height 1.810 m; `tessera-explorer --sheet B26.png --character beorning,f --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B27 | P9 Beorning (0x72) F | 0x47007005 | 0x04000554 | 1.03 | run | 0x0500899A | pass | standing height 1.810 m; `tessera-explorer --sheet B27.png --character beorning,f --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B28 | P10 HighElf (0x75) M | 0x4700806C | 0x04005EC4 | 1.05 | idle | 0x050017FE | pass | standing height 1.893 m; feet on the ground (lowest bone 0.023–0.023 m); `tessera-explorer --sheet B28.png --character highelf,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B29 | P10 HighElf (0x75) M | 0x4700806C | 0x04005EC4 | 1.05 | walk | 0x050093BC | pass | standing height 1.893 m; `tessera-explorer --sheet B29.png --character highelf,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B30 | P10 HighElf (0x75) M | 0x4700806C | 0x04005EC4 | 1.05 | run | 0x050093DD | pass | standing height 1.893 m; `tessera-explorer --sheet B30.png --character highelf,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B31 | P11 HighElf (0x75) F | 0x47008071 | 0x04005EC5 | 1.03 × 1.02 | idle | 0x05001804 | pass | scale composition OPEN (O8); standing height 1.843 m, model scale 1.0506; feet on the ground (lowest bone 0.015–0.015 m); `tessera-explorer --sheet B31.png --character highelf,f --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B32 | P11 HighElf (0x75) F | 0x47008071 | 0x04005EC5 | 1.03 × 1.02 | walk | 0x050093A0 | pass | standing height 1.843 m; `tessera-explorer --sheet B32.png --character highelf,f --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B33 | P11 HighElf (0x75) F | 0x47008071 | 0x04005EC5 | 1.03 × 1.02 | run | 0x050093DB | pass | standing height 1.843 m; `tessera-explorer --sheet B33.png --character highelf,f --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B34 | P12 DwarfStoutAxe (0x78) | 0x4700979A | 0x04000557 | 1.0 | idle | 0x05007A13 | pass | one body for both race-table entries; standing height 1.420 m (its appearance-list head, hair 0x0600BAC9 and beard stand taller than the bind height of §2.2; scale 1.0 as expected); assembly reports unpaired-dye ClassList(3) 0x20003092; feet on the ground (lowest bone 0.030–0.031 m); `tessera-explorer --sheet B34.png --character dwarfstoutaxe,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B35 | P12 DwarfStoutAxe (0x78) | 0x4700979A | 0x04000557 | 1.0 | walk | 0x05007A0D | pass | standing height 1.420 m; `tessera-explorer --sheet B35.png --character dwarfstoutaxe,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B36 | P12 DwarfStoutAxe (0x78) | 0x4700979A | 0x04000557 | 1.0 | run | 0x05007A14 | pass | standing height 1.420 m; `tessera-explorer --sheet B36.png --character dwarfstoutaxe,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B37 | P13 RiverHobbit (0x7D) M | 0x4700CEA3 | 0x0400772B | 0.7 | idle | 0x05008EE8 | fail | hidden thigh/knee sliders active (§6.3); skinning itself clean (no stretched or detached parts, seams closed), but the feet float about 0.09 m above the ground plane: the hidden `Femur_Scale` sliders, applied as the §6.3 Decision says, shorten the legs (lowest bone 0.104 m at scale 0.7; 0.015 m without the sliders); OPEN O9 (`appearance.md` §6.3 application frame) and `skeleton-anim.md` §5.4 (scale inheritance); standing height 1.259 m; `tessera-explorer --sheet B37.png --character riverhobbit,m --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B38 | P13 RiverHobbit (0x7D) M | 0x4700CEA3 | 0x0400772B | 0.7 | walk | 0x05008EA9 | fail | skinning itself clean (no stretched or detached parts, seams closed), but the feet float about 0.09 m above the ground plane: the hidden `Femur_Scale` sliders, applied as the §6.3 Decision says, shorten the legs (lowest bone 0.104 m at scale 0.7; 0.015 m without the sliders); OPEN O9 (`appearance.md` §6.3 application frame) and `skeleton-anim.md` §5.4 (scale inheritance); standing height 1.259 m; `tessera-explorer --sheet B38.png --character riverhobbit,m --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B39 | P13 RiverHobbit (0x7D) M | 0x4700CEA3 | 0x0400772B | 0.7 | run | 0x05008EA1 | fail | skinning itself clean (no stretched or detached parts, seams closed), but the feet float about 0.09 m above the ground plane: the hidden `Femur_Scale` sliders, applied as the §6.3 Decision says, shorten the legs (lowest bone 0.106 m at scale 0.7; 0.015 m without the sliders); OPEN O9 (`appearance.md` §6.3 application frame) and `skeleton-anim.md` §5.4 (scale inheritance); standing height 1.259 m; `tessera-explorer --sheet B39.png --character riverhobbit,m --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| B40 | P14 RiverHobbit (0x7D) F | 0x4700CEA4 | 0x0400772A | 0.7 | idle | 0x05008EE7 | fail | hidden thigh/knee sliders active (§6.3); skinning itself clean (no stretched or detached parts, seams closed), but the feet float about 0.09 m above the ground plane: the hidden `Femur_Scale` sliders, applied as the §6.3 Decision says, shorten the legs (lowest bone 0.103 m at scale 0.7; 0.015 m without the sliders); OPEN O9 (`appearance.md` §6.3 application frame) and `skeleton-anim.md` §5.4 (scale inheritance); standing height 1.230 m; `tessera-explorer --sheet B40.png --character riverhobbit,f --clip idle --times 0,3.7,7.4,11.1 --angles "160,10;70,5;340,10" --tile 300x440` |
| B41 | P14 RiverHobbit (0x7D) F | 0x4700CEA4 | 0x0400772A | 0.7 | walk | 0x05008EB6 | fail | skinning itself clean (no stretched or detached parts, seams closed), but the feet float about 0.09 m above the ground plane: the hidden `Femur_Scale` sliders, applied as the §6.3 Decision says, shorten the legs (lowest bone 0.098 m at scale 0.7; 0.015 m without the sliders); OPEN O9 (`appearance.md` §6.3 application frame) and `skeleton-anim.md` §5.4 (scale inheritance); standing height 1.230 m; `tessera-explorer --sheet B41.png --character riverhobbit,f --clip walk --times 0,0.3,0.6,0.9 --angles "160,10;70,5;340,10" --tile 300x440` |
| B42 | P14 RiverHobbit (0x7D) F | 0x4700CEA4 | 0x0400772A | 0.7 | run | 0x05008EAD | fail | skinning itself clean (no stretched or detached parts, seams closed), but the feet float about 0.09 m above the ground plane: the hidden `Femur_Scale` sliders, applied as the §6.3 Decision says, shorten the legs (lowest bone 0.098 m at scale 0.7; 0.015 m without the sliders); OPEN O9 (`appearance.md` §6.3 application frame) and `skeleton-anim.md` §5.4 (scale inheritance); standing height 1.230 m; `tessera-explorer --sheet B42.png --character riverhobbit,f --clip run --times 0,0.2,0.4,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |

## W. Worn items, one at a time (criterion 3)

Each on the Human male body (P3) over the default body; "Hobbit" notes give the Hobbit male
(P1) difference where it matters. Priorities in brackets (`appearance.md` §4.4).

| # | DID | Slot | What it is | Expected (Human M) | Result | Note |
|---|---|---|---|---|---|---|
| W1 | 0x790006C3 | Head | closed helmet | `Helmet` real (4); `Crest` stub (10) hides hair; `Ears` replaced by fitted ears 0x0600E55A (10). Hobbit: no ears part | pass | winners Ears 0x0600E55A p10, Helmet 0x0600D2BB p4; hidden Satchel, Beard, Shoulder, Cape, Crest; Hobbit M: Helmet 0x060001F3 p4; hidden Satchel, Shoulder, Cape, Crest; `tessera-explorer --sheet W1.png --character human,m --wear 0x790006C3 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (and `--character hobbit,m`) |
| W2 | 0x790005AF | Head | hat with fitted hair | `Helmet` real (4); `Crest` replaced by the fitted hair mesh (10), not the chosen style | pass | winners Crest 0x0600D3DC p10, Helmet 0x0600D445 p4; hidden Satchel, Beard, Shoulder, Cape; `tessera-explorer --sheet W2.png --character human,m --wear 0x790005AF --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W3 | 0x7900A91E | Head | open headgear | `Helmet` real (4) only; hair, ears, face unchanged | pass | winners Helmet 0x0600D378 p4; hidden Satchel, Beard, Shoulder, Cape; head close-up: `--sheet W3_head.png --character human,m --wear 0x7900A91E --clip idle --times 0 --angles "160,10;70,5;340,10" --tile 300x300 --focus 1.65,0.25`; `tessera-explorer --sheet W3.png --character human,m --wear 0x7900A91E --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W4 | 0x7900B58F | Head | open headgear, priority 3 | `Helmet` real (3); Hobbit record uses 4 | pass | winners Helmet 0x0600D463 p3; hidden Satchel, Beard, Shoulder, Cape; Hobbit M: Helmet 0x06002594 p4; hidden Satchel, Shoulder, Cape; `tessera-explorer --sheet W4.png --character human,m --wear 0x7900B58F --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (and `--character hobbit,m`) |
| W5 | 0x790564CF | Head | full helm | `Helmet` real (20); `Head` stub (15) hides the face; `Crest` and `Beard` stubs (10); fitted ears | pass | winners Ears 0x0600E55A p10, Helmet 0x0600D51D p20; hidden Head, Satchel, Beard, Shoulder, Cape, Crest; `tessera-explorer --sheet W5.png --character human,m --wear 0x790564CF --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W6 | 0x79000670 | Shoulder | shoulder guards | `Shoulder` real (1); nothing else changes | pass | winners Shoulder 0x0600D2A1 p1; hidden Satchel, Beard, Cape, Helmet; `tessera-explorer --sheet W6.png --character human,m --wear 0x79000670 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W7 | 0x79000420 | Chest | short tunic | `UpperBody` real (1) ending at the waist; legs, hands, feet unchanged | pass | winners UpperBody 0x0600D251 p1; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W7.png --character human,m --wear 0x79000420 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W8 | 0x79000960 | Chest | floor-length robe | `UpperBody` real (1) down to the ankles; `LowerBody` stub (10) | pass | winners UpperBody 0x0600D254 p1; hidden LowerBody, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W8.png --character human,m --wear 0x79000960 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W9 | 0x7900DA3B | Chest | robe that hides the feet | `UpperBody` real; `LowerBody` stub (10); `Feet` stub (10) | pass | winners UpperBody 0x0600D551 p1; hidden LowerBody, Feet, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W9.png --character human,m --wear 0x7900DA3B --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W10 | 0x79034B00 | Chest | long-sleeved top | `UpperBody` real; `Hands` replaced by the bare hands 0x0600D4B0 (10). Hobbit: `Hands` stub (10), hands modelled in the sleeves | pass | winners UpperBody 0x0600D3F4 p1, Hands 0x0600D4B0 p10; hidden Satchel, Beard, Shoulder, Cape, Helmet; Hobbit M: UpperBody 0x06002891 p1; hidden Hands, Satchel, Shoulder, Cape, Helmet; `tessera-explorer --sheet W10.png --character human,m --wear 0x79034B00 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (and `--character hobbit,m`) |
| W11 | 0x7900D693 | Chest | robe with its own shoes | `UpperBody` real; `LowerBody` stub (10); `Hands` bare (10); `Feet` real (3) | pass | winners UpperBody 0x0600D413 p1, Hands 0x0600D4B0 p10, Feet 0x0600F11C p3; hidden LowerBody, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W11.png --character human,m --wear 0x7900D693 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W12 | 0x7900D695 | Chest | chest piece with its own legs | `UpperBody` real; `LowerBody` real (10); `Feet` stub (10); `Hands` bare (10) | pass | winners UpperBody 0x0600D3F4 p1, LowerBody 0x0600E287 p10, Hands 0x0600D4B0 p10; hidden Feet, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W12.png --character human,m --wear 0x7900D695 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W13 | 0x79000412 | Back | plain cloak | `Cape` real (1) | pass | winners Cape 0x0600D2A6 p1; hidden Satchel, Beard, Shoulder, Helmet; cape follows the spine and legs through idle and run (checked also at `--clip run`); `tessera-explorer --sheet W13.png --character human,m --wear 0x79000412 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W14 | 0x7902A135 | Back | hooded cloak | `Cape` real (1); `Crest` stub (15); `Helmet` stub (15); fitted ears (10) | pass | winners Ears 0x0600E55A p10, Cape 0x0600D3C5 p1; hidden Satchel, Beard, Shoulder, Crest, Helmet; Helmet slot hidden (its stub); `tessera-explorer --sheet W14.png --character human,m --wear 0x7902A135 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W15 | 0x79000411 | Gloves | gloves | `Hands` real (1) | pass | winners Hands 0x0600D2A3 p1; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W15.png --character human,m --wear 0x79000411 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W16 | 0x7902146C | Gloves | gloves, priority 6 | `Hands` real (6) | pass | winners Hands 0x0600D6F2 p6; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W16.png --character human,m --wear 0x7902146C --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W17 | 0x790006BA | Legs | leggings | `LowerBody` real (1) | pass | winners LowerBody 0x0600D224 p1; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W17.png --character human,m --wear 0x790006BA --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W18 | 0x790361BB | Legs | leggings with footwear | `LowerBody` real (1); `Feet` real (7). Hobbit: no `Feet` part | pass | winners LowerBody 0x0600D238 p1, Feet 0x0600E4C0 p7; hidden Satchel, Beard, Shoulder, Cape, Helmet; Hobbit M: LowerBody 0x06004CE0 p1; hidden Satchel, Shoulder, Cape, Helmet; `tessera-explorer --sheet W18.png --character human,m --wear 0x790361BB --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (and `--character hobbit,m`) |
| W19 | 0x79000446 | Boots | tall boots | `Feet` real (2), shaft to about 0.5 m. Hobbit: priority 1 | pass | winners Feet 0x0600D265 p2; hidden Satchel, Beard, Shoulder, Cape, Helmet; Hobbit M: Feet 0x06000011 p1; hidden Satchel, Shoulder, Cape, Helmet; `tessera-explorer --sheet W19.png --character human,m --wear 0x79000446 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (and `--character hobbit,m`) |
| W20 | 0x79000780 | Boots | low shoes | `Feet` real (5), top at about 0.13 m | pass | winners Feet 0x0600D266 p5; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet W20.png --character human,m --wear 0x79000780 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W21 | 0x7901852B | Class | satchel (worn class item) | `Satchel` real (3); `PhysObj` not drawn | pass | winners Satchel 0x0600D6AF p3; hidden Beard, Shoulder, Cape, Helmet; assembly reports dye-inexact (the item's default code has no exact dye block, `material.md` §12.4 rule); no held part (PhysObj not drawn); `tessera-explorer --sheet W21.png --character human,m --wear 0x7901852B --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |
| W22 | 0x79058E21 | Weapon_Primary | battle gauntlets (worn weapon) | `Hands` real (5); `PhysObj` not drawn | pass | winners Hands 0x0600CACD p5; hidden Satchel, Beard, Shoulder, Cape, Helmet; no held part (PhysObj not drawn); `tessera-explorer --sheet W22.png --character human,m --wear 0x79058E21 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` |

## L. Layering combinations (criterion 3)

| # | Items worn together | Expected (Human M) | Result | Note |
|---|---|---|---|---|
| L1 | W8 + W17 | Robe shown; leggings hidden (`LowerBody` stub 10 > 1) | pass | winners UpperBody 0x0600D254 p1 from 0x79000960; hidden LowerBody, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet L1.png --character human,m --wear 0x79000960 --wear 0x790006BA --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L2 | W11 + W19 | Robe's own shoes shown (3 > 2); tall boots hidden | pass | winners UpperBody 0x0600D413 p1 from 0x7900D693, Hands 0x0600D4B0 p10 from 0x7900D693, Feet 0x0600F11C p3 from 0x7900D693; hidden LowerBody, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet L2.png --character human,m --wear 0x7900D693 --wear 0x79000446 --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L3 | W11 + W20 | Low shoes shown (5 > 3) | pass | winners UpperBody 0x0600D413 p1 from 0x7900D693, Hands 0x0600D4B0 p10 from 0x7900D693, Feet 0x0600D266 p5 from 0x79000780; hidden LowerBody, Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet L3.png --character human,m --wear 0x7900D693 --wear 0x79000780 --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L4 | W10 + W15 | Bare hands shown (10 > 1); gloves hidden | pass | winners UpperBody 0x0600D3F4 p1 from 0x79034B00, Hands 0x0600D4B0 p10 from 0x79034B00; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet L4.png --character human,m --wear 0x79034B00 --wear 0x79000411 --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L5 | W14 + W1 | Hood hides helmet and hair (15 > 4, 15 > 10) | pass | winners Ears 0x0600E55A p10 from 0x7902A135, Cape 0x0600D3C5 p1 from 0x7902A135; hidden Satchel, Beard, Shoulder, Crest, Helmet; `tessera-explorer --sheet L5.png --character human,m --wear 0x7902A135 --wear 0x790006C3 --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L6 | W14 + W5 | Full helm shown (20 > 15); face, hair, beard hidden | pass | winners Ears 0x0600E55A p10 from 0x7902A135, Cape 0x0600D3C5 p1 from 0x7902A135, Helmet 0x0600D51D p20 from 0x790564CF; hidden Head, Satchel, Beard, Shoulder, Crest; `tessera-explorer --sheet L6.png --character human,m --wear 0x7902A135 --wear 0x790564CF --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L7 | W18 + W19 | Leggings' footwear shown (7 > 2) | pass | winners LowerBody 0x0600D238 p1 from 0x790361BB, Feet 0x0600E4C0 p7 from 0x790361BB; hidden Satchel, Beard, Shoulder, Cape, Helmet; `tessera-explorer --sheet L7.png --character human,m --wear 0x790361BB --wear 0x79000446 --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` |
| L8 | 0x7900F859 (robe with bare hands at 5 and own shoes at 3) + W22 | Equal priority 5 on `Hands`: by the tie Decision (later slot wins) the gauntlets show. Then 0x7900F859 + W16: gloves at 6 beat the bare hands at 5 | pass | tie rule OPEN (O10); with W22: Hands 0x0600CACD p5 from 0x79058E21 (gauntlets, later slot wins the tie); with W16: Hands 0x0600D6F2 p6 from 0x7902146C (gloves 6 over bare hands 5); `tessera-explorer --sheet L8a.png --character human,m --wear 0x7900F859 --wear 0x79058E21 --clip walk --times 0,0.6 --angles "160,10;70,5;340,10" --tile 300x440` and the same with `--wear 0x7902146C` |

## H. Held items (criterion 3)

Location by `appearance.md` §5.3 (world rule of `skeleton-anim.md` §8.5); bones on the
Human male setup 0x04000553. For stowed placement see `appearance.md` §5.4 (Decision); check
the in-hand location first.

| # | DID | Slot | Category | Location | Bone (Human M) | Result | Note |
|---|---|---|---|---|---|---|---|
| H1 | 0x7900066F | Weapon_Primary | 1HSword | `Wpn_RHand` | 79 `Rweaponholder` | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H1.png --character human,m --wear 0x7900066F --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H2 | 0x790006D1 | Weapon_Primary | 1HAxe | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H2.png --character human,m --wear 0x790006D1 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H3 | 0x7902722C | Weapon_Primary | Dagger | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H3.png --character human,m --wear 0x7902722C --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H4 | 0x7901C852 | Weapon_Primary | 1HMace | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H4.png --character human,m --wear 0x7901C852 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H5 | 0x79039DD5 | Weapon_Primary | 1HHammer | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H5.png --character human,m --wear 0x79039DD5 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H6 | 0x790383A4 | Weapon_Primary | 1HClub | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H6.png --character human,m --wear 0x790383A4 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H7 | 0x79068E0D | Weapon_Secondary | 1HSword | `Wpn_LOffHand` | 65 `Lweaponholder` | pass | mirrored about its long axis; Wpn_LOffHand on bone 65 (`Lweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); in the left hand, turned half about y (§8.3); `tessera-explorer --sheet H7.png --character human,m --wear 0x79068E0D --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H8 | 0x790006CB | Weapon_Primary | 2HSword | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H8.png --character human,m --wear 0x790006CB --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H9 | 0x79014A77 | Weapon_Primary | 2HAxe | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H9.png --character human,m --wear 0x79014A77 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H10 | 0x790006EF | Weapon_Primary | 2HHammer | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H10.png --character human,m --wear 0x790006EF --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H11 | 0x79000831 | Weapon_Primary | 2HClub | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H11.png --character human,m --wear 0x79000831 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H12 | 0x7900ABD8 | Weapon_Primary | 2HStaff | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H12.png --character human,m --wear 0x7900ABD8 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H13 | 0x7901D6D7 | Weapon_Primary | Spear | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H13.png --character human,m --wear 0x7901D6D7 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H14 | 0x7901198F | Weapon_Primary | Halberd | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H14.png --character human,m --wear 0x7901198F --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H15 | 0x79013A37 | Weapon_Primary | Orb | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); small orb in the right fist (hand close-up `--focus 0.95,0.25`); `tessera-explorer --sheet H15.png --character human,m --wear 0x79013A37 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H16 | 0x7900AF50 | Weapon_Primary | Implement1H | `Wpn_RHand` | 79 | pass | Wpn_RHand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); held in the right fist (hand close-up `--focus 0.95,0.25`); `tessera-explorer --sheet H16.png --character human,m --wear 0x7900AF50 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H17 | 0x79001F43 | Weapon_Ranged | Bow | `Sheath_Back_Bow` | 16 `Thorax` | pass | in-hand location OPEN (O13); Sheath_Back_Bow on bone 16 (`Thorax`); follows the bone through idle, walk and run; on the back across the thorax; in-hand location OPEN (O13); `tessera-explorer --sheet H17.png --character human,m --wear 0x79001F43 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H18 | 0x79028A8E | Weapon_Ranged | Crossbow | `Wpn_ROffhand` (item override) | 79 | pass | paperdoll: `Sheath_Back_Crossbow`; Wpn_ROffhand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H18.png --character human,m --wear 0x79028A8E --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H19 | 0x790109BA | Weapon_Ranged | Javelin | `Wpn_ROffhand` (item override) | 79 | pass | Wpn_ROffhand on bone 79 (`Rweaponholder`); follows the bone through idle, walk and run; grip at the hand bone, long axis out of the fist (result note 3); `tessera-explorer --sheet H19.png --character human,m --wear 0x790109BA --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H20 | 0x7901D692 | Weapon_Secondary | Shield | `Wpn_Forearm` | 54 `Lelbow` | pass | Hobbit and Dwarf: `Lulna`; Wpn_Forearm on bone 54 (`Lelbow`); follows the bone through idle, walk and run; Hobbit M: bone 17 (`Lulna`), Dwarf M: bone 21 (`Lulna`) (`--character hobbit,m` / `dwarf,m`); `tessera-explorer --sheet H20.png --character human,m --wear 0x7901D692 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H21 | 0x7900F164 | Weapon_Secondary | MediumShield | `Wpn_Forearm` | 54 | pass | Wpn_Forearm on bone 54 (`Lelbow`); follows the bone through idle, walk and run; `tessera-explorer --sheet H21.png --character human,m --wear 0x7900F164 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H22 | 0x790006CF | Weapon_Secondary | HeavyShield | `Wpn_Forearm` | 54 | pass | Wpn_Forearm on bone 54 (`Lelbow`); follows the bone through idle, walk and run; `tessera-explorer --sheet H22.png --character human,m --wear 0x790006CF --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H23 | 0x79006BF8 | Weapon_Secondary | Shield | `Forearm` (item override) | 54 | pass | Forearm on bone 54 (`Lelbow`); follows the bone through idle, walk and run; `tessera-explorer --sheet H23.png --character human,m --wear 0x79006BF8 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H24 | 0x79001298 | Weapon_Ranged | Instrument | `Sheath_Back_Bow` (slot default) | 16 | pass | where instruments rest is OPEN (O13); Sheath_Back_Bow on bone 16 (`Thorax`); follows the bone through idle, walk and run; on the back (slot default); OPEN O13; `tessera-explorer --sheet H24.png --character human,m --wear 0x79001298 --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |
| H25 | 0x79015F4E | Class | Class | none in the data | – | pass | expected: not drawn in the world (§5.3 Decision); not drawn: assembly reports no-holding-location (§5.3 Decision); `tessera-explorer --sheet H25.png --character human,m --wear 0x79015F4E --clip idle --times 0,7.4 --angles "160,10;70,5;340,10" --tile 300x440` (also `--clip walk --times 0.3,0.9`, `--clip run --times 0.2,0.6`) |

## N. Static NPC spots (sandbox)

Physics or weenie SEDs (`entity-descriptor.md`) with the instance property sets of the
landblock weenie list (`appearance.md` §7.2). Positions are the SED's landblock-local
position (m). Expected appearance per `appearance.md` §7.4; idle animation enum values are
given for `script-table.md`.

| # | Landblock info | IID | Object → setup | Position | Expected | Result | Note |
|---|---|---|---|---|---|---|---|
| N1 | 0x80211B8F | 0x0111B8F00000002C | 0x47004E6F → 0x0400003E | 100.58, 19.27, 494.75 | Male NPC rig, scale 0.9; one worn piece (record 0x2000084F, key `Worn`, modifier 1): a full-body mesh in `UpperBody` with stubs in `Crest` and `Head` at priority 1, which replace the rig's default head and hair; prop 0x47000EB5 in `LHand`, prop 0x47002912 in `RHand`; idle enum 0x7 | pass | created at the data's position and heading (region-global 4420.58, 22899.27, 494.75; −2.4192 rad), drawn as an animated character (`npc_spots_real_data`, release); seated figure seen behind the barrels in `tessera-client --screenshot … --spawn 1,27,143,100.58,14.27 --heading 0 --pitch -10` |
| N2 | 0x8021678A | 0x011678A0000000A5 | 0x47004E6F → 0x0400003E | 48.32, 38.57, 357.86 | Same rig and piece as N1, scale 1.0; prop 0x470006D9 in `RHand`, prop 0x47000161 in `LHand`; idle enum 0x59 | pass | position and heading equal the data (16528.32, 22118.57, 357.86; −2.3226 rad); figure shown, 6 animated characters around |
| N3 | 0x80213FAA | 0x0113FAA0000000C8 | 0x47004E75 → 0x04000004 | 150.88, 124.19, 613.90 | Small NPC rig (1.17 m); three pieces (0x2000002D, 0x2000080A, 0x2000002E) each with a dye element (codes 0, 1, 0.06); idle enum 0x5E | pass | position and heading equal the data (10230.88, 27324.19, 613.90; 0.0 rad); figure shown |
| N4 | 0x80213B85 | 0x0113B85000000019 | 0x47004FB6 → 0x04000152 | 47.04, 66.28, 593.33 | Stocky NPC rig (1.35 m); two pieces with three dye elements; prop 0x47000650 at `Wpn_RHand`; idle enum 0x29 | pass | position and heading equal the data (9487.04, 21346.28, 593.33; −2.8798 rad) with `--content-layers 1,0x10000019` (the default layer `[1]` hides it, O3); stocky figure with the pick in its right hand seen in `--spawn 1,59,133,47.04,61.28 --heading 0 --pitch -10 --content-layers 1,0x10000019` |
| N5 | 0x80214F79 | 0x0114F79000000076 | 0x47004E74 → 0x04000004 | 151.07, 150.72, 402.49 | Random-appearance type 0x46 → default parts of the rig (Decision); prop 0x47000E36 in `RHand`; idle enum 0x5A | pass | look in the original: O12; position and heading equal the data (12791.07, 19510.72, 402.49; −0.6248 rad); default parts of the rig (random type, Decision) |
| N6 | 0x80253E07 | 0x0453E0700000001B | weenie 0x7006986E → 0x4700005B → 0x0400003E | 11.51, 49.93, 156.11 | The single monster weenie; random type 0xDB → default parts | pass | second SED IID 0x0453E0700000001A; position and heading equal the data (9931.51, 1169.93, 156.11; −2.0375 rad) with `--content-layers 1,0x10000536`; 432 placed figures around, all ready (release settles in seconds; a debug build needs several minutes) |
| N7 | 0x802235F6 | 0x04235F600000001D | weenie 0x7003E1BB → marker 0x47006915 | 104.50, 151.44, 450.85 | Distant-battle unit: marker only (debug view); formations OPEN | pass | marker only: created at the data's position (8584.50, 39511.44, 450.85; −1.5951 rad) and hidden like every distant-battle marker (`walker.md` §10.2) |
