<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M2 visual checklist (exit criterion 2)

Pass/fail record for [ROADMAP M2](../ROADMAP.md#m2) criterion 2: "a visual checklist of at
least 50 meshes across categories (buildings, props, armour, weapons, vegetation) passes: no
slivers, correct UVs, correct alpha-tested edges, correct dye regions". Text only: the
screenshots were kept in a scratch directory and deleted, and no game names are recorded.
Category descriptions below are our own words.

## Method

- Install: the owner's Steam install, data current on 2026-10-04, read in place
  (`TESSERA_GAME_DIR`).
- Tool: `tessera-explorer --select <DID> --view mesh --screenshot out.png --width 2100
  --height 1200 --no-persist` under `xvfb-run` (viewer render on the workstation's Vulkan
  adapter). Default viewer settings: lit shading, detail level 0, vertex colours off,
  diffuse colour 2000 off (both OPEN, `material.md` §19). Extra flags are listed per row.
- Every DID is a 0x79 object property set; the viewer follows its geometry link (`setup.md`
  §3.1, §3.2): `PhysObj` → entity → visual template → setup → meshes. Worn armour uses
  `--mesh-choice 1`, the first `Item_WornAppearanceMapList` element (`setup.md` §3.2).
- Candidates were picked by the item-class and weenie-type properties of the object
  property sets, so each row's category is the data's own classification.
- Each screenshot was viewed in full. Checked per row: no stray or stretched triangles
  (slivers), textures on the right faces without seams or swapped/flipped UVs, cut-out edges
  on alpha-tested materials (foliage, feathers, fringes, ironwork), and for the dye rows,
  that the dye colour lands only on the masked region (`material.md` §12.2). Skinned
  meshes are checked in bind pose (no skeleton yet). Follow-up renders (other angles, UV
  checker, wireframe, a highlighted draw) were made where a row says so.

## Result

**74 checks on 72 objects (the two dye previews re-use an armour object), 74 pass, 0 fail, with row C6 passing in the mesh view only** (re-checked 2026-10-23). The explorer's character view (`--character 0x04000110`) still draws the one-triangle stub of the same setup, so C6 counts as a mesh-view pass until `apps/explorer/src/character.rs` leaves such parts out (see the row).

| Category | Checks | Pass | Fail |
|---|---|---|---|
| Weapons and shields | 15 | 15 | 0 |
| Armour (worn, skinned, bind pose), incl. 2 dye previews | 13 | 13 | 0 |
| Creatures and NPCs (skinned, bind pose) | 15 | 15 (C6 in the mesh view only) | 0 |
| Vegetation | 9 | 9 | 0 |
| Buildings and large structures | 9 | 9 | 0 |
| Props and furniture | 13 | 13 | 0 |

The one earlier failure (row C6) was not a decoding error: the mesh decoded and drew as
stored. [`setup.md` §8.5](../specs/setup.md#s8-5) now gives the rule (one-triangle parts are
left out of models with several parts), and the mesh view applies it since 2026-10-23. The
explorer's character view does not apply it yet (row C6).

## Weapons and shields

| # | DID | Extra flags | What it is | Result | Note |
|---|---|---|---|---|---|
| W1 | 0x7900066F | | one-handed sword | pass | blade, guard and grip textured correctly; gloss highlight on the blade |
| W2 | 0x79014A77 | | two-handed axe | pass | long haft, double blade |
| W3 | 0x790383A4 | | spiked club | pass | spikes and banding aligned with the texture |
| W4 | 0x79039DD5 | | war hammer | pass | |
| W5 | 0x7902722C | | knife | pass | |
| W6 | 0x7901C852 | | flanged mace | pass | |
| W7 | 0x79001F43 | | bow | pass | grip wrap and limbs mapped correctly |
| W8 | 0x79028A8E | | crossbow | pass | |
| W9 | 0x7900ABD8 | | staff with forked head | pass | |
| W10 | 0x7901D6D7 | | spear with barbed head | pass | thin barbs, no stray triangles |
| W11 | 0x7901198F | | halberd | pass | |
| W12 | 0x790109BA | | javelin | pass | |
| W13 | 0x79013A37 | | rune-stone | pass | |
| W14 | 0x7901D692 | | rectangular shield | pass | metal trim and boss mapped correctly |
| W15 | 0x7904E6CD | | round shield | pass | painted emblem centred |

## Armour (worn appearance, skinned, bind pose)

| # | DID | Extra flags | What it is | Result | Note |
|---|---|---|---|---|---|
| A1 | 0x79002B7C | `--mesh-choice 1` | chest armour (plate over leather and mail) | pass | normal-mapped; dyeable material |
| A2 | 0x79002B7C | `--mesh-choice 1 --dye d02020` | same, red dye | pass | leather and the plate's field take the dye; mail, raised trim and rivets keep their colour; crisp mask edge |
| A3 | 0x7903E4DE | `--mesh-choice 1` | boots | pass | |
| A4 | 0x7902C146 | `--mesh-choice 1` | leggings with tassets | pass | |
| A5 | 0x7900F5DF | `--mesh-choice 1` | cloth hat | pass | |
| A6 | 0x790555F5 | `--mesh-choice 1` | gloves | pass | small at default framing (the bound spans both hands at T-pose width); re-checked from the front |
| A7 | 0x790492C0 | `--mesh-choice 1` | plate sabatons | pass | |
| A8 | 0x790059E1 | `--mesh-choice 1` | shoulder plates | pass | |
| A9 | 0x790698CC | `--mesh-choice 1` | cloak with metal collar | pass | |
| A10 | 0x790698CC | `--mesh-choice 1 --dye 2040d0` | same, blue dye | pass | cloth takes the dye; gold trim and gems keep their colour |
| A11 | 0x7900480E | `--mesh-choice 1` | long coat | pass | |
| A12 | 0x7902E622 | `--mesh-choice 1` | mail hauberk with tabard | pass | |
| A13 | 0x7901276D | `--mesh-choice 1` | horned helm | pass | floats at head height (bind-pose position), as expected |

## Creatures and NPCs (skinned, bind pose)

| # | DID | Extra flags | What it is | Result | Note |
|---|---|---|---|---|---|
| C1 | 0x790072C7 | | wolf | pass | fur, teeth and eyes mapped correctly |
| C2 | 0x790073D9 | | bear | pass | forelegs held straight out in bind pose |
| C3 | 0x79030469 | | large wolf-like beast | pass | |
| C4 | 0x79038EA0 | (also `--mesh-angle 30,60`) | giant spider | pass | legs straight out sideways in bind pose; has a one-triangle stub part too small to see |
| C5 | 0x79018631 | | troll with bark-like limbs | pass | |
| C6 | 0x79018D38 | (also `--mesh-angle 20,10`, `--mesh-select 2,0`) | goblin | pass (2026-10-23, mesh view) | was **fail**: part 2 (mesh 0x060068B9, one triangle, surface 0x310000E5) drew as a white triangle at the feet. Now the mesh view leaves one-triangle parts out of models with several parts (`setup.md` §8.5 steps 1–3, S14). Re-checked 2026-10-23 by a `--view mesh --mesh-angle 20,10 --screenshot` render on the reference laptop (window 1421 × 781, viewed in full): body drawn, no triangle at the feet, parts 1 and 2 (one triangle each) listed as "not drawn: a one-triangle placeholder part". The real-data test `checklist_row_c6_draws_no_placeholder_part` (`apps/explorer/src/scene.rs`) counts the same: 3 parts, 1 drawn, 2 left out. **Not covered:** the character view's bare-setup path (`--character 0x04000110`) does not apply the rule yet and still draws the triangle (follow-up in `apps/explorer/src/character.rs`, through `tessera_world::objects::drawn_parts`) |
| C7 | 0x790273B0 | | tree creature | pass | alpha-tested leaf cards with clean edges |
| C8 | 0x7904E65C | | salamander | pass | |
| C9 | 0x7900C0B8 | | large bird | pass | alpha-tested feather cards |
| C10 | 0x7906C15C | | boar | pass | |
| C11 | 0x7904DDD4 | | giant | pass | |
| C12 | 0x79031818 | | undead humanoid | pass | |
| C13 | 0x79007B2B | | half-orc humanoid | pass | cloak and armour mapped correctly |
| C14 | 0x79022A7A | | many-legged crawler | pass | alpha-tested leg fringe |
| C15 | 0x79060897 | | human NPC | pass | skin, hair, clothing mapped correctly |

## Vegetation

| # | DID | Extra flags | What it is | Result | Note |
|---|---|---|---|---|---|
| V1 | 0x790470AD | | deciduous tree | pass | alpha-tested leaf cards, clean cut-out edges |
| V2 | 0x790470B2 | | broad-crowned tree | pass | |
| V3 | 0x790545DF | (also `--mesh-shading uv`, `wire`, `--mesh-select 0,1`) | old leafless tree | pass | no leaf geometry exists (wireframe); the second draw is bark growth, alpha test off in its material, drawn opaque |
| V4 | 0x790735E2 | | group of conifers | pass | |
| V5 | 0x79043E97 | | flowering bush | pass | |
| V6 | 0x7907363F | | potted shrub | pass | |
| V7 | 0x7905FCF1 | | low fungal bush | pass | sparse alpha-tested blades |
| V8 | 0x790108E3 | | vegetable bed | pass | |
| V9 | 0x79055E5C | | decorated conifer | pass | |

## Buildings and large structures

| # | DID | Extra flags | What it is | Result | Note |
|---|---|---|---|---|---|
| B1 | 0x7906050D | | stone dwelling | pass | |
| B2 | 0x7907361A | | ruined domed rotunda | pass | open ironwork dome |
| B3 | 0x79043EA3 | | windmill tower | pass | one part, three detail levels; no sail part in this setup |
| B4 | 0x7905AF91 | | open stable with flower garlands | pass | alpha-tested flowers |
| B5 | 0x79063378 | | stone tower | pass | |
| B6 | 0x7906336A | | gate-house | pass | |
| B7 | 0x7907463F | | miniature building on a pedestal | pass | |
| B8 | 0x79060525 | | stone outbuilding | pass | |
| B9 | 0x7900D209 | | large striped tent | pass | stripes follow the canvas panels |

## Props and furniture

| # | DID | Extra flags | What it is | Result | Note |
|---|---|---|---|---|---|
| P1 | 0x790735E1 | | small table | pass | |
| P2 | 0x7900D0E3 | | ornate chest | pass | |
| P3 | 0x79032AB3 | | high-backed chair with shields | pass | |
| P4 | 0x79019CCC | | lantern post | pass | |
| P5 | 0x7906633D | | hanging sign | pass | |
| P6 | 0x79071FC3 | | tool rack | pass | |
| P7 | 0x790692A0 | | baskets | pass | |
| P8 | 0x790741C1 | | hanging banner | pass | |
| P9 | 0x79057FCF | | long planter with flowers | pass | |
| P10 | 0x79024D43 | | snowman | pass | |
| P11 | 0x7902AD41 | | chest with handles | pass | |
| P12 | 0x790471A3 | | arched door | pass | |
| P13 | 0x7900762A | | backpack | pass | |
