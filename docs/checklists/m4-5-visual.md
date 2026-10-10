<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M4.5 visual and collision checklist (exit criteria 4, 5 and 6)

| | |
|---|---|
| **Status** | **Revised 2026-10-10** (spec writer, fourth change of the day, PUD of [§1](#s1)): W13's opacity is a range over the visible part of the quad (0.33–0.53, per pixel) with a measuring note (Tessera blends decoded values), W14's opacity range and its colour under Tessera's fog-colour Decision (lighter than the mud), new check W14-R; awaiting review. Before: **Accepted 2026-10-10** (review of the CAM2 revision (commit e4db0ed) passed `CLEAN_ROOM.md` §5.3, every item; see the review record; the walker side re-judges W9, L3, L5, W10, W15, W16 and W2). Before: **Revised 2026-10-10** (spec writer, third change of the day, CAM2 of [§1](#s1)): W9 accepts the sky dome's equator line below the horizon (`environment.md` §12); the L3 implementer OPEN closed (the eye of [§4.7.1](#s4-7-1)); expectations for the generated figures seen from L5 ([§4.6](#s4-6)); W10, W15, W16 and W2 revised from the review notes of 9d1d976 ([§4.7.1](#s4-7-1), [§4.8](#s4-8)); new owner items O91 and O92; awaiting review. Before: **Revised 2026-10-10** (spec writer, after the re-judgement on main `01deafd`): image cameras re-derived for T1, Y3, W2, S5 and S7 and L3's eye corrected ([§4.7.1](#s4-7-1)); W10's expectation corrected from the data (the outline is sharp from its eye); W15 and W16 judged by the measurements W15-R and W16-R ([§4.8](#s4-8)); awaiting review, then the walker side re-judges these rows. Before: Expected values complete (2026-10-06); scenery rows S1–S11 and walks GN1–GN4 re-derived 2026-10-07 ([§8](#s8)); the implementer's re-derivations of 2026-10-08 checked by the spec writer 2026-10-09 (GN1, GN2, CV2 and §2.2 confirmed; CV1–CV3 durations of 8 s confirmed, CHK2; L5 corrected, T6 moved with it, L7 redefined: [§8](#s8)); walker side (T6, L5, L7 on the image) and owner observations pending |
| **Last updated** | 2026-10-10 (spec writer, fourth change of the day: tag PUD in [§1](#s1); rows W13 and W14 of [§4.8](#s4-8)). Before: 2026-10-10 (spec writer, third change of the day: tag CAM2 in [§1](#s1); W9, the L3 closure and the generated figures of L5 in [§4.4](#s4-4) and [§4.6](#s4-6); W2 in [§4.7.1](#s4-7-1); W10, W15 and W16 in [§4.8](#s4-8)). Before: 2026-10-10 (spec writer, second change of the day: new tag CAM in [§1](#s1), new [§4.7.1](#s4-7-1) with cameras for T1, Y3, W2, S5, S7 and L3 and the finding that L3's pyramid is part of the bonfire's mesh, seen from inside a placed platform; L3's eye corrected in [§4.6](#s4-6); W10, W15 and W16 of [§4.8](#s4-8) revised; notes under §4.1, §4.2, §4.4 and §4.5). Before: 2026-10-10 (spec writer: rows W13 and W17 of [§4.8](#s4-8) revised with `water.md` §8.5.4 and §8.6.4 of that date; owner item O24 extended). Before: 2026-10-09 (spec writer, third water revision: row W7 of [§4.4](#s4-4) gains the expected lava look from `water.md` §8.6.5. Before, the same day, spec writer: new [§4.8](#s4-8), rows W10–W17 for non-shader water and water animation, W8 corrected. Earlier the same day, spec writer: CHK of [§1](#s1); GN1, GN2, CV2, the TOUCH and CAVE tags and the §2.2 revision confirmed, the two §2.4 OPENs closed; L5 corrected and T6 moved to an exposed paving of the same layer, because a placed platform covers the former spots ([§4.6](#s4-6)); L7 redefined on a camp with a light map and a night, the §7 OPEN closed). Before: 2026-10-08 (implementer: §2.2 pass conditions revised from `collision.md` §11.7; GN1 and GN2 stop points re-derived under the first-touch rule (TOUCH, [§1](#s1)) and CV2 moved to a walkable path in the same cave; both §2.4 OPENs answered, for the spec writer to confirm; cameras for judging T4, T5, T12, T15, T16, Y2, L2, L5 and L7 on the image ([§4.7](#s4-7)), L5 revised from `terrain-texturing.md` §9.4, OPEN for L7 in [§7](#s7)) Also 2026-10-09: spec writer, second check of the CV1–CV3 durations of 8 s (CHK2 of [§1](#s1)). |
| **Author role** | Spec writer (dirty room) |
| **Review record** | Claude review session (spec-reviewer, a fresh session that wrote no part of this checklist), 2026-10-10, of the CAM2 revision (commit e4db0ed on branch specs-21: header; §1 CAM2; §4.4 W9; §4.6 L3 closure and the L5 note on the generated figures; §4.7.1 W2; §4.8 W10, W15, W16; §8 row): **passed.** Functional expectations only, each tied to a spec section (`environment.md` §12, `appearance.md` §7.4, `water.md` §8.6.4 and §8.7, `material.md` §11.3); no code, no copied prose, no game text; owner items O26, O91 and O92 carry the comparisons with the original. Spot checks (read-only, own probes): the W9 line and the −7.8° band follow from the dome data (`environment.md` E40) and the eye; L3's eye 394.95 is 1.8 m above the platform top 393.15 of §4.7.1; the generated figures of L5 match the objects 0x47007170–0x47007178 (types, colour 7, idles) and the placeholder colour; W10: the streak mask 0x40000C5F's column means fall below 0.1 from 0.45 of a repeat and reach 0 at 0.8, the fill texture 0x40000B18's alpha mean is 0.90 and the mask 0x40000C5D's middle columns 0.86. The "about a sixth" of W10 (inner sheet's effective opacity) was not re-derived. The `appearance.md` revision of the same commit was sent back for a wording point in §7.4 step 3 (parameter 2013 in one scriptlet) that does not change the L5 expectation. Earlier: Claude review session (spec-reviewer agent), 2026-10-07, of the re-derivation of 2026-10-07 (§1 GEN row, §2.3–§2.4 GN1–GN4, §4.2 rows S1–S11 and callout, §4.3 intro, §8): **passed.** The rule the rows now follow (`scenery.md` §7.4 step 14b, S36) was re-checked in a separate copy of the Ghidra project. The totals of all 11 landblocks and the list indices of GN1–GN4 (16, 33, 176, 138) agree with the clean-room implementation's generated lists in the writer's scratch output, as do GN1's new tree and position. No game text, keys or code. The companion `scenery.md` revision has changes requested on §7.4 step 15 only (failure causes and frill-layer containment), which no row of this checklist depends on. Second review, Claude review session (spec-reviewer agent, a different session), 2026-10-07: **passed again.** S36 re-checked in a fresh copy of the Ghidra project. Independently of the writer's output, a fresh read-only build of the repository's crates generated the 11 landblocks of §4.2 on the install: totals 25, 315, 165, 309, 151, 111, 445, 166, 221, 80 and 186 as listed; GN1–GN4 at 0-based list indices 16, 33, 176 and 138 with the listed DIDs, positions, headings and scales; the changed nearest objects of S1, S2, S5, S6, S8 and S10 at the listed positions (S8's second at x 126.421, within rounding). The in-view counts and the frill rows were not re-measured. The open issues of `scenery.md` §7.4 step 15 still do not affect any row. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: review of the water-feature revision on branch `water-fix` (`git diff main...water-fix`, commits 88edc8f and 4be89e7) against CLEAN_ROOM.md §5.3 (new §4.8, rows W10–W17, row W8, companion specs, OPEN on the fly-start height): **passed**. Rows give eyes, DIDs and expectations derived from `water.md` §8.5–§8.7 and `material.md` §10.3; no game text or images; the rates of W10, W11, W12, W15 and W16 recomputed from the cited parameters (0.36 / 0.50 / 0.32 m/s; 0.20 / 0.32 m/s; 22.6 m/s former), the W14 opacity (0.62) recomputed from the eye and the puddle position. Non-blocking: W13 states (1 − sin 30°)^1.33 as "about 0.41"; that expression is 0.40 (0.40 also from the exact eye-to-puddle angle of 29.6°). Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: review of the third water revision's change to row W7 ([§4.4](#s4-4)), with §4.8 re-read: **passed**. W7's expected look follows `water.md` §8.6.5: brightness 2.4 to 0.9 recomputed for 1066 = 1.5; the emissive list 0x4001D1AF decoded (dark red mean colour, alpha 0–255); about six repeats per landblock from 1063 = 6. No game text or images. The owner question depends on the `water.md` §8.6.5 OPEN, whose wording `water.md` item B1 of this review does not affect. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: re-review of the whole `water-fix` branch up to commit 113462a (`git diff main...water-fix`) against CLEAN_ROOM.md §5.3. This checklist did not change after that review: **passed**. Re-checked from own probes: W10's inner-sheet rates and alpha (0x310004E6), W12's plane parameters (0x31006748), W15's and W17's flow-map parameters (0x31004359, 0x310066D5). Claude review session (spec reviewer, fresh session; not the writer), 2026-10-10: review of the changes of commit 9e9a29a (rows W13 and W17 of §4.8, change-log row; §4.4 unchanged) against CLEAN_ROOM.md §5.3: **passed**. W13's new expectation follows `water.md` §8.5.4 and W44 and is marked Inferred for the original; the opacity (1 − sin 30°)^1.33 = 0.40 recomputed; W17's Tessera look follows the revised §8.6.4 Tessera rule, and the river's bias 0.01, exponent 2 and fog amount 1 on 0x310066D5 were re-read with own probes. No game text or code. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-10: review of commit 9d1d976 on branch `specs-20` (§1 tag CAM, §4.7.1, rows T1, Y3, W2, S5, S7, L3, W10, W15, W16, notes under §4.1, §4.2, §4.4, §4.5, change-log row; `water.md` unchanged) against CLEAN_ROOM.md §5.3: **passed** (every item). Facts are camera positions, DIDs, measured heights and look expectations with the new CAM provenance; the cone under L3's bonfire is marked Inferred; no code, copied prose, game text, images or keys; nothing under a closed gate. Every new camera and the replaced ones were rendered with the walker built from this branch (release, `--fly --eye-height 1`, 1600 × 900, clock stopped; private images, deleted): T1 frames the cobbled road at the centre in shadow, the former camera a roof; Y3 the evening sky and fogged ridges past trees, the former camera walls; W2 open water over a stony bed with reeds on a bank at the right, bare ground and bank with `--disable water`, the former camera reeds; S5 a generated tree on the centre column, gone with `--disable scenery` while the placed column, walls and bushes stay; S7 a low cluster at about (381, 526), gone without scenery, and the rock at about (1296, 630) unchanged, both positions recomputed from the eye and §4.0; L3 the bonfire on the platform without a cone, the former eye the downward cone from below the platform; W10 sharp side edges at image *x* about 650–690 and 930–948 and a sharp crest. Recomputed: the T1, W2 and S5 eyes from the row geometry; the W15-R and W16-R rates (0.20, 0.32; 0.36, 0.50, 0.16 m/s) from `water.md` §8.7. Own probes (read-only): level 0 of 0x40000C5D is opaque in the middle (about 0.86) and fades at all four borders; the outer strips' mean opacity 0x40000C5E × 0x40000C5F × 0.757 is 0.023. Non-blocking: (1) W10 says 0x40000C5F's alpha "falls to 0 over the last tenth of each repeat across"; its column means at level 0 drop below 0.1 from about 0.4 of the repeat and reach 0 from about 0.8, so "over the second half" is closer (the conclusion, no edge mask, holds); (2) W10's new "not expected" item (an opaque sheet with no rock showing through): where the fill covers, it blends at about 0.9 × 0.86 ≈ 0.77 (`material.md` §11.3; 0x40000B18 alpha mean 0.90), so with the inner sheet the rock contributes only about a sixth; stating that the rock shows faintly would keep a correct image from failing (on this branch's image the rock is not distinguishable through the middle of the fall); (3) W15-R and W16-R name the walker diagnostic `still scroll`, which no spec defines; citing the `water.md` §8.7 rule it reports would make the check independent of the diagnostic's name; (4) W2's "(owner item: compare)" names no item of `owner-observations.md`. |
| **Serves** | [ROADMAP M4.5](../ROADMAP.md#m4-5-exit) exit criterion 4 (scripted walks on collision and the 1,000-walk soak, [§2](#s2)), criterion 5 (five performance walk spots, [§3](#s3)) and criterion 6 (visual rows, [§4](#s4)); the M3 reference-location check asked for by criterion 1 ([§5](#s5)) |
| **Companion specs** | [`collision.md`](../specs/collision.md) §5–§6, §9–§12; [`scenery.md`](../specs/scenery.md) §4–§8; [`water.md`](../specs/water.md) §6–§8; [`material.md`](../specs/material.md) §8.4, §10.3, §11.6; [`terrain-texturing.md`](../specs/terrain-texturing.md) §3–§4, §8, §10; [`environment.md`](../specs/environment.md) §5–§7, §11; [`lighting.md`](../specs/lighting.md) §8–§9; [`landblock.md`](../specs/landblock.md) §3–§6; [`coordinates.md`](../specs/coordinates.md) §5, §7, §9, §11 (normative) |
| **Owner list** | Items [O18–O27](owner-observations.md#o18) of the owner list compare these rows with the original client |

Places in all six regions with landblock data (1, 2, 3, 4, 5 and 14) where the data
predicts exact facts: where a walker is stopped, what it climbs, which cell it is in, where
it starts to wade and to swim, which terrain layers, generated objects, frills, water, sky
values and lights a renderer must show. Tessera's walker is checked against the
"expected" columns; the owner compares the original client with the same rows.

Text only. No game text: areas, objects and terrain types are named by DID and described
in our own words. No screenshots, captures or extracted data are committed; screenshots of
either client stay private in a scratch directory (same rules as
[`m3-reference-locations.md`](m3-reference-locations.md#s4)).

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | The owner's install, data current on 2026-10-06, read in place and read-only by throwaway dirty-room probes. Records decoded for this file: landblock data, block maps, landblock info and landblock properties of regions 1–5 and 14 (all records); 0x66 (all 16,819); 0x01 (all 1,057); 0x02, 0x11, 0x12, 0x19 (all); 0x16; 0x15000007, 0x1A (all 963), 0x35000000; the block-map descriptors 0x18; 0x31/0x30/0x2B for the lighting rows. |
| GEN | A dirty-room re-implementation of `scenery.md` §6–§7 (static pass) and §8.2 with the Tessera Decision of §8.6 (frill pass), written from the spec text only. It reproduces the synthetic worked example of `scenery.md` §14 exactly (vertex hash 0x7237E00C, the frequency, displacement, heading and scale draws, both quaternions). It reads the landblock's statics and their clearance volumes from landblock info (`scenery.md` §7.4 step 14a). **Corrected 2026-10-07:** until then its footprint test (`scenery.md` §7.4 step 14b) was inverted: it tested only the open markers and skipped the closed ones. The corrected generator follows step 14b, which a BIN re-check confirmed unchanged. Its static-pass lists equal those of the clean-room implementation object for object in all 11 landblocks of [§4.2](#s4-2). Its frill pass keeps the static pass's markers (`scenery.md` §8.2 item 5). The frill records of the six landblocks of [§4.3](#s4-3) are identical under the old and the corrected generator. Rows re-derived: [§8](#s8). |
| SWEEP | A geometric sweep of the Human male capsule (`collision.md` §10.6: radius 0.35 m, cap centres 0.35 m and 1.45 m above the feet) along a straight horizontal path, with the feet on the terrain (`landblock.md` §3.11) or on the highest walkable surface no more than the step-up height (0.5 m, BasePlayer, `collision.md` §10.3) above the current feet. A contact counts as blocking only when it lies above the step-up height. The collision set is `collision.md` §12.1: all terrain triangles, the 0x66 meshes of every blocking SED (§9.3) and every blocking generated object (§9.4) of the landblock and its eight neighbours, and, for interiors, the cell physics meshes (§5.3) and interior SEDs. Not Tessera's controller: it gives the geometric fact the controller must reproduce. |
| TOUCH | Implementer derivation, 2026-10-08, with Tessera's own collision queries (`tessera-physics` capsule contacts and sweeps over the collision sets the walker uses, `collision.md` §12.1) on the owner's install, read-only; a throwaway test, not committed. The Human male capsule of SWEEP moves along the start heading in 2 cm steps with the feet on the terrain (F1), refined by bisection to 1 mm. Every contact with a body other than the terrain or ceiling gets its normal by `collision.md` §11.3 (the face normal replaces the contact normal when both are steeper than 45° and the face is the steeper; Tessera's capsule-side orientation, §12.4 (x)). P is the first position where a contact that is **not walkable** (*u* ≤ cos 45°: steep, wall or overhang), at **any height**, has zero separation, unless the step-up of `collision.md` §11.5 from there (back off 0.05 m, rise 0.5502 m, §11.6 item 5, forward 0.05 m plus *L* for *L* = 0, 0.054 and 0.108 m, the run step's remaining length, descend 0.5502 m) lands on walkable support (§11.3.1, crease rule included) **past** the touch. This is the first-touch rule of `collision.md` §11.7 ("A steep contact below the step-up height"); SWEEP differs only in ignoring contacts below the step-up height. With the capsule resting on the terrain instead (its rest offset on a slope, convex radius 0, §12.5), every P moves by at most 0.012 m. **Confirmed 2026-10-09 by the spec writer (CHK):** the rule is the one of `collision.md` §11.7, and an independent probe reproduces every P of GN1–GN4 within 0.002 m ([§2.4](#s2-4)). A step-up that lands on walkable support but less than 0.001 m past the touch (GN3 and GN4: the forward cast is stopped by the face and the descent comes back to the start) makes no progress and leaves P where it is. |
| CAVE | Implementer search, 2026-10-08, with Tessera's own decoders (`tessera-formats` landblock data, `tessera-physics` terrain and ceiling meshes, `collision.md` §6.1, §6.4) on the owner's install, read-only; a throwaway test, not committed. Quads whose four ceiling samples are non-zero and whose two terrain and two ceiling triangles are covered by a stored layer (or a `whole` layer, `landblock.md` §3.8.2); straight 15 m paths from every start on a 0.5 m lattice in 15° heading steps, sampled every 0.25 m (0.05 m for the final row), on which every point and every point of two rings (radius 0.35 m and 0.175 m, 16 directions) lies on such quads, the steepest terrain triangle within those rings is at most 40°, and the ceiling-minus-floor distance under the axis (bit-0 ceiling diagonal) is at least 2.6 m. "Same cave": the quads with four non-zero ceiling samples, at least one of them more than 1 m above the floor sample, joined through shared edges. Objects: capsule contacts within 3 m of the path with every body other than terrain and ceiling (detail bodies included). **Confirmed 2026-10-09 by the spec writer (CHK)** for CV2 ([§2.4](#s2-4), Caves). |
| CHK | Spec-writer check, 2026-10-09, of the implementer's rows of 2026-10-08: throwaway dirty-room probes written from the specs (`collision.md` §11.3, §11.3.1, §11.5–§11.7; `landblock.md` §3.5, §3.11; `terrain-texturing.md` §9.4, §10; `environment.md` §11), not from Tessera's code, on the owner's install, read-only. Collision sets as in SWEEP, with the GEN generator as corrected on 2026-10-07. **Walks:** the Human male capsule moves along the heading in 2 cm steps with the feet on the terrain, refined by bisection to 0.1 mm; a contact counts when its separation is below 0.0001 m; its normal follows `collision.md` §11.3 (face normal turned toward the capsule); the step-up of §11.5 is cast with the full capsule against terrain and objects (back off 0.05 m, rise 0.5502 m, forward 0.05 m + *L* for *L* = 0, 0.054, 0.108 m, descend 0.5502 m) and judged by the support search of §11.3.1 (reach 0.08 m, crease rule); repeated with the capsule resting on the terrain. **Caves:** the CAVE conditions recomputed (sampling every 0.05 m, rings of 0.35 m and 0.175 m in 16 directions). **Lighting rows:** placed objects (every drawn SED and generated object of the block and its eight neighbours) as their mesh bounds from the setups, enlarged by 1 m (conservative), tested for cover above a point, for the sun ray (60 m toward the sun) and for the line from the eye; the render mesh of the platform found over the former L5 and T6 spots decoded (`mesh.md` §3–§8); light-map texels decoded from the DXT1 surface (`terrain-texturing.md` §10: row 0 south, column 0 west); keyframes blended per `environment.md` §11.3 with *D* = 2, *n* = 12,240. |
| CHK2 | Spec-writer check, 2026-10-09, of the CV1–CV3 durations of [§2.3](#s2-3): path lengths from the row coordinates; walking speed and acceleration from `collision.md` §10.3, §11.6 and §11.8; the uphill factor of the plane solver (`collision.md` §11.4) evaluated on the terrain under each path (`landblock.md` §3.11 heights, sampled every 0.01 m, with the repository's terrain decoder used as a tool on the owner's install, read-only); a throwaway dirty-room probe, not committed. |
| CAM | Spec-writer check, 2026-10-10, of the rows the re-judgement of that date could not judge or failed (T1, Y3, W2, S5, S7, W10, W15, W16; L3's bonfire): throwaway dirty-room probes on the owner's install, read-only, written from the specs (`mesh.md` §3–§8 for render meshes and their texture coordinates, `setup.md` §6 for setup parts, `landblock.md` §3.11 for terrain heights, `material.md` §9.2 and §11.2 for effective parameters and the alpha test, `texture-list.md` for the textures), with the repository's `tessera-dattool cat --raw` used as a tool to read records. Placed objects tested by vertical rays and eye-to-point segments against the detail-level-0 triangles of every drawn SED whose placed bounds cover the point; terrain occlusion sampled every 0.25 % of the segment. Every camera of [§4.7.1](#s4-7-1) rendered once with Tessera's walker to check what it frames (images private, deleted). |
| CAM2 | Spec-writer check, 2026-10-10 (third change of the day), answering the implementer OPENs of commits 34922e8 (W9's sky line) and d82be77 (L3) and the generated figures of commit 566679b, and the non-blocking notes of the review of 9d1d976: throwaway dirty-room probes on the owner's install, read-only (the sky dome and every sky surface, `environment.md` OBS7, E40–E42; the generated rigs of 3, 0xD7E7 and their dye chains, `appearance.md` OBS10, A58; the level-0 alpha of 0x40000C5F, 0x40000B18 and 0x40000C5D for W10). No new images were taken. |
| PUD | Spec-writer check, 2026-10-10 (fourth change of the day), of rows W13 and W14 after the re-judgement of that date (owner notes in the ROADMAP M4.5 status: W13's opacity varied over the patch, W14's puddles were blue-grey and lighter than the mud): a throwaway dirty-room probe on the owner's install, read-only, with the repository's decoders as tools (landblock info and heights of 0x80217574 and 0x8021E243, terrain by `landblock.md` §3.11, mesh 0x0600183D, surface 0x31000A2A by `material.md` §9.2, its diffuse 0x400017ED and normal map 0x40000DDB). The quad (4 vertices at ±1, UV 0–1) was sampled on an 81 × 81 grid in world space by each placement (#170 scale 8.7, #181 scale 5.0); a sample counts when it lies above the terrain and the line from the row's eye to it does not pass below the terrain; at each, δ, *a* of `water.md` §8.5.4 step 4 (with and without the normal map's bump term) and the diffuse alpha. W13: 1,174 of 6,561 samples seen, δ 20.4°–34.2°, distance 15.2–24.5 m, *a* 0.333–0.565 (0.333–0.525 where the shape alpha is 1). W14: 6,359 seen, δ 12.6°–25.8°, 10.2–20.3 m, *a* 0.467–0.720 (0.516–0.694 where the shape alpha is 1, median 0.633). The 8-bit example of W13-R uses the sRGB transfer function. No program text or client code was read. |
| CELL | The point-in-cell rule of `collision.md` §5.4 and §5.8.1 (tolerance −0.0002), evaluated at the feet raised by 0.05 m. Where two cells contain the point, both are listed. |
| WAT | `water.md` §6.2 (landblock water), §7.1 (volume entity) and the Tessera rule of §7.4. |
| ENV | `environment.md` §11.1–§11.3 with the day type *D* = 2 (the Tessera initial value, §11.2) and game day *n* = 12,240 (the day at clock *G* = 0, `environment.md` §14). |
| MAT | `material.md` §9.2 (effective parameters), §11.3 (blend family), §11.5 (emissive, glow, specular flags). |

| # | Fact used by this checklist | Sources | Mark |
|---|---|---|---|
| F1 | Heights: stored × 2,000 / 65,535 in single precision; terrain under (x, y) by the triangulation of `landblock.md` §3.11 | `landblock.md` §3.4, §3.11 | Confirmed (spec) |
| F2 | Mover: Human male capsule radius 0.35 m, height 1.80 m; step-up 0.5 m; walkable when the support normal's z exceeds cos 45°, sliding up to 85° | `collision.md` §10.3, §10.6, §11.3 | Confirmed (spec) |
| F3 | Speeds of the default avatar: run (BasePlayer forward) 6.5 m/s, walk (Walking layer) 5 m/s, in water (Swimming layer) 3.2 m/s | `collision.md` §10.3, §11.1 | Confirmed (data) |
| F4 | Float height of the default avatar (`human,m`): 1.42 m; wading while the water surface is at most that far above the feet, swimming beyond | `collision.md` §10.5, §11.2; `water.md` §7.4 | Confirmed (data); the switching rule is a Tessera Decision |
| F5 | A generated object blocks when its effective `Physics_NoPhysics` is not true, its setup has a 0x66 mesh or volumes and it is not ethereal to Player | `collision.md` §9.4 | Tessera Decision (spec) |
| F6 | Undrawn blocking SEDs: effective `Render_Draw`, `Render_DrawMeshes` or `Render_NeverAddToScene` hide the object; collision ignores the draw flags | `collision.md` §9.3; `entity-descriptor.md` §9 | Confirmed (spec) |
| F7 | Cells are entered by containment; portals link cells to cells | `collision.md` §5.8, §5.8.1 | BIN (spec) |
| F8 | Sky key per landblock = block-map key 0x1000000F through the region's 0x18 descriptor palette | `environment.md` §7.1; `landblock.md` §5.4 | Confirmed |

<a id="s1-1"></a>
### 1.1 Census facts found while choosing the spots (OBS, 2026-10-06)

These facts shaped the rows and are new; they are recorded here because no spec states them
yet.

1. **No interior cell can be walked into from the landscape.** Of the cells whose hull
   overlaps the square of their landblock, only cells of 0x8021FAF9 and 0x8021FAFA have a
   vertical face whose outside point (1.5 m out, at the face centre) lies within 0.6 m of
   the terrain at the cell's floor height (61 faces). Both landblocks are flat at height 0
   and have no terrain layers (nothing drawn): modular rooms laid out on an undrawn floor.
   Within 4 m of the floor the count is 233 faces in 19 landblocks, every one of them a
   block whose X or Y is 0xF3 or more (1,433 landblocks have cells). Inferred: interior
   cells are reached by teleport, not by walking in from outdoors. The interior walks of §2
   start inside a cell and cross a portal (doorway) into the next one. Neighbouring cell
   volumes can overlap at their shared portal, so a point in the doorway can be inside both
   (IN1, IN4; `collision.md` §5.8.1: Tessera takes the lower cell number there).
2. **Stair collision is mostly ramps.** The 0x66 meshes of placed stairs replace the treads
   by sloped faces (33 of the first 39 distinct stair meshes have more sloped than level
   faces). A walk that climbs real risers needs an object with level treads; the step
   walks use a platform-with-steps object (0x47000AE1) whose risers are 0.13–0.32 m.
3. **Day type 2 selects only the local default table.** No day group of 0x15000007 has day
   type 2 (the four groups have 0x10000001, 0x10000008, 0x10000004, 0x1000000B), so with the
   Tessera initial *D* = 2 the sky entry comes from the local default table. 76 of the 477
   sky keys used by the block-map palettes are in a day group but not in the default table,
   and 20 are in neither; with *D* = 2 those landblocks get no day description. The sky rows
   of §4.5 use keys that are in the default table. Owner item [O26](owner-observations.md#o26)
   asks for the day type.
4. **Sun direction is almost always constant within a day description.** In 947 of the
   963 day descriptions every keyframe has the same `sun_heading` and `sun_pitch` (612 of
   the 628 with more than one keyframe). In 0x1A000000, 0x1A000359, 0x1A000213 and
   0x1A000377 (rows Y1, Y3, Y4, L1–L3, L5) they are 15° and 150° (toward-sun vector
   (−0.224, −0.837, 0.5): elevation 30°, from bearing 195°). The time of day changes
   colours, brightness and fog, not the lighting direction; the moving sun and moon discs
   are sky objects (`environment.md` §11.4).

---

<a id="s2"></a>
## 2. Scripted walks (exit criterion 4)

<a id="s2-1"></a>
### 2.1 Walk format

One walk is one row of the table in [§2.3](#s2-3), or one file with the same fields given
to `tessera-client --walk-path FILE`. Both forms carry the same twelve fields.

**Table form.** The walk table is the first Markdown pipe table after the anchor `s2-3`
whose header row is exactly
`| id | category | region | landblock | cell | start | loc | heading | mode | inputs | duration | expect |`.
A parser reads every following row until the first line that does not start with `|`.
Cells are trimmed; a cell wrapped in backticks has the backticks removed.

**File form.** UTF-8 text, one `key = value` per line, keys as the column names, `#`
starts a comment, blank lines ignored. Unknown keys are an error. Example:

```
id = WL1
category = wall
region = 1
landblock = 0xA197
cell = 0
start = 127.81,89.59,389.26
loc = r1 lx1294 ly1212 ox127.81 oy89.59 oz389.26
heading = 0.0
mode = run
inputs = 0:fwd
duration = 3
expect = blocked P=127.81,94.77,389.26 d=0.10
```

**Fields.**

| Field | Grammar | Meaning |
|---|---|---|
| `id` | letters and digits | Unique walk name |
| `category` | `wall`, `undrawn`, `generated`, `step`, `steep`, `interior`, `cave`, `water`, `frill` | The criterion-4 category the walk counts for |
| `region` | decimal | Region *R* (`coordinates.md` §4) |
| `landblock` | `0xXXYY` | The landblock (X, Y) whose frame every coordinate of the row uses |
| `cell` | decimal | Interior cell of the start (`coordinates.md` §7); 0 = outdoors |
| `start` | `x,y,z` | Feet position at the start, block-local metres (`coordinates.md` §5.1, §6). z is the support height there (terrain, object or cell floor). The walker places the capsule's foot point at `start` within 0.02 m and lets it settle for 0.5 s of simulated time before the inputs begin. Interior coordinates may lie outside 0 … 160. |
| `loc` | `/loc` position text | The same start as the original's `/loc` would print it (`coordinates.md` §9), heading included; the owner types `/loc` there to compare. Redundant with `region`, `landblock`, `cell`, `start` and `heading`. |
| `heading` | degrees | Compass heading of the avatar at the start, 0 = north, 90 = east (`coordinates.md` §11.4) |
| `mode` | `walk`, `run`, `jump`, `swim` | `walk`: the Walking layer (`Movement_MovementStyle` 1); `run`: the default (BasePlayer) layer; `jump`: `run` with a jump input at every `jump` token of `inputs`; `swim`: reserved for walks that start in deep water (none in this file). Layers per `collision.md` §11.1. |
| `inputs` | `;`-separated tokens | `T:fwd`: hold forward from time T (s) on; `T:stop`: release forward at T; `T:jump`: press jump at T; `T:turn=H`: set the heading to H at T; `wp=x,y[;x,y…]`: steer: every step set the heading toward the current waypoint, hold forward, switch to the next waypoint within 0.25 m, release forward within 0.05 m of the last one. Times count from the end of the settle period. |
| `duration` | seconds | Simulated time after the settle period, in fixed steps of 1/60 s (`collision.md` §12.3) |
| `expect` | kind and `key=value` list, space-separated | The pass condition ([§2.2](#s2-2)) |

All positions in `expect` are feet positions in the frame of `landblock`. Numbers use a
point as the decimal separator; coordinates are separated by commas without spaces.

> **OPEN (implementer, 2026-10-06, `--walk-path`):** `wp=` does not say whether "release
> forward within 0.05 m of the last one" is decided once or every step. Released once, a
> walker coasts on under the controller's deceleration (about 0.4 m at walking speed, 20 m/s²
> from 4.9 m/s), which no `reach` tolerance covers. Tessera (`walker.md` §13.6) decides it
> every step: forward is held, toward the waypoint, whenever the feet are more than 0.05 m
> from it, so a walker that coasted past is steered back. Please confirm or state the
> intended rule.

> **OPEN (implementer, 2026-10-06 review, `--walk-path`):** §2.1 does not say what `T:stop`
> and `T:turn=H` do in a walk that also has `wp=`: the steering sets the heading and holds
> forward every step, so they would have no effect. Tessera refuses that combination (no
> row of §2.3 uses it); `T:fwd` and `T:jump` are accepted with waypoints (`jump` presses
> jump). Please confirm or state the intended precedence.

<a id="s2-2"></a>
### 2.2 Expected outcomes and pass conditions

Every walk also passes only if, at every step: the capsule penetrates no collision geometry
by more than 0.01 m (criterion 4); position and velocity are finite.

| Kind | Parameters | Passes when |
|---|---|---|
| `blocked` | `P` (stop point), `d` | At the **first touch** (the first step with forward held after which the capsule lies within 0.01 m of a blocking object) the feet are within `d` of P horizontally and vertically within 0.05 m below to 0.05 m plus the slope allowance (below) above P, and at no step up to the first touch was the feet's distance along the start heading greater than that of P plus `d`. After the first touch the walker may slide along or round the blocker while forward stays held (`collision.md` §11.7 items 2–3); the end point is reported only. P is where the capsule first touches a face of the blocker that it can neither ride onto nor step over: the TOUCH rule of [§1](#s1) (`collision.md` §11.7). For WL1–WL5 and UB1–UB3, P is SWEEP's, which TOUCH reproduces within 0.005 m; GN1–GN4 use TOUCH (2026-10-08). |
| `reach` | `E`, `d`, `risers` | The feet end within `d` of E horizontally and within 0.05 m vertically; `risers` lists the vertical steps (m) of the support profile on the way, in order, each below the step-up height, which the walker climbs without a jump. |
| `noclimb` | `B` (foot of the steep band on the path), `zmax`, `rmax` | The feet never rise above `zmax` (B.z + 0.60 m) and end no farther than `rmax` (distance of B + 0.60 m) from the start, measured horizontally along the path; the state is `Earth_UncontrolledSurface` (6) at least once. The band beyond B is steeper than 50° for at least 3 m (all its triangles between 50° and 85°). |
| `cells` | `A>B`, `E`, `d` | The cell lookup (`collision.md` §5.8.1) reports A at the start and B at the end; the feet end within `d` of E (horizontal) and 0.05 m (vertical). In the doorway both cells contain the feet; a report of either is allowed there. |
| `follow` | `E`, `d`, `dz`, `headroom` | At every step the feet are no more than `dz` below and no more than `dz` plus the slope allowance (below) above the terrain height under them (`landblock.md` §3.11), and the walker is never blocked; it ends within `d` of E. `headroom` is the smallest ceiling-minus-floor distance on the path (`landblock.md` §3.5, bit-0 diagonal), for information. |
| `wade_swim` | `in`, `swim`, `h`, `fh`, `d` | The state first becomes a water state (1, 2 or 3) within `d` of `in` (where the terrain under the path first goes below h) and first becomes `Water_NoSurface` (2) within `d` of `swim` (where it first goes below h − `fh`). Once 3 s have passed after `swim`, the feet stay within 0.25 m of h − `fh`. |
| `pass` | `E`, `d` | The walker ends within `d` of E; no object contact at any step; at every step the feet are no more than 0.05 m below and no more than 0.05 m plus the slope allowance (below) above the terrain under them. |

**Slope allowance** (`collision.md` §11.7, "Feet above the terrain on a slope"; added
2026-10-08). A capsule of radius *r* resting on terrain that slopes at *θ* has its feet
*r* · (sec *θ* − 1) above the terrain under its axis (Tessera's convex radius 0, `collision.md`
§12.5): 0.050 m at 29°, 0.132 m at 43.4°, 0.145 m at 45°. The allowance uses for *θ* the
steepest terrain triangle within *r* of the axis (sampled at the axis and on rings of radius
*r* and *r*/2, 16 directions each). On any terrain whose slope within *r* is at most *θ* the
capsule's rest offset is at most this value, because the surface there lies at most
tan *θ* · *ρ* above the point under the axis at horizontal distance *ρ*, and the largest
tan *θ* · *ρ* + √(*r*² − *ρ*²) is *r* · sec *θ*. Rows written for "feet on the terrain" (SWEEP,
F1) thus hold for a capsule on slopes up to the 45° walkable limit.

**Confirmed 2026-10-09 (spec writer).** Both revisions of 2026-10-08 (first touch for `blocked`,
slope allowance) follow `collision.md` §11.7: "Consequences for walk checks" and the *c* = 0
column of "Feet above the terrain on a slope" (0.050, 0.132 and 0.145 m as there).

Tolerances cover the SWEEP's simplifications (a capsule kept upright, contacts below the
step-up height ignored, 2 cm path sampling). Walks that fail by less than twice their
tolerance are reported to the spec writer, who re-checks the row before the implementation
is changed.

<a id="s2-3"></a>
### 2.3 The walks

32 walks: 5 walls, 3 undrawn blockers, 4 generated objects, 3 steps, 3 steep slopes,
4 interiors, 3 caves, 4 water entries, 3 frill areas. Every region with landblock data has
at least three walks.

| id | category | region | landblock | cell | start | loc | heading | mode | inputs | duration | expect |
|---|---|---|---|---|---|---|---|---|---|---|---|
| WL1 | wall | 1 | 0xA197 | 0 | 127.81,89.59,389.26 | `r1 lx1294 ly1212 ox127.81 oy89.59 oz389.26` | 0.0 | run | `0:fwd` | 3 | `blocked P=127.81,94.77,389.26 d=0.10` |
| WL2 | wall | 2 | 0x6EA5 | 0 | 120.73,60.31,517.46 | `r2 lx886 ly1323 ox120.73 oy60.31 oz517.46 h52.7` | 52.7 | run | `0:fwd` | 3 | `blocked P=124.86,63.45,517.46 d=0.10` |
| WL3 | wall | 3 | 0xA73F | 0 | 130.69,32.49,311.99 | `r3 lx1342 ly505 ox130.69 oy32.49 oz311.99 h270.0` | 270.0 | run | `0:fwd` | 3 | `blocked P=125.98,32.49,311.99 d=0.10` |
| WL4 | wall | 4 | 0xC755 | 0 | 30.26,122.36,155.40 | `r4 lx1593 ly686 ox30.26 oy122.36 oz155.40 h315.0` | 315.0 | run | `0:fwd` | 3 | `blocked P=26.82,125.79,155.40 d=0.10` |
| WL5 | wall | 5 | 0x757A | 0 | 66.39,69.64,228.76 | `r5 lx939 ly979 ox66.39 oy69.64 oz228.76 h180.0` | 180.0 | run | `0:fwd` | 3 | `blocked P=66.39,64.71,228.76 d=0.10` |
| UB1 | undrawn | 2 | 0x3B91 | 0 | 59.64,64.39,578.19 | `r2 lx474 ly1163 ox59.64 oy64.39 oz578.19` | 0.0 | run | `0:fwd` | 3 | `blocked P=59.64,69.04,578.19 d=0.10` |
| UB2 | undrawn | 4 | 0x7CBC | 0 | 78.03,89.18,165.80 | `r4 lx995 ly1508 ox78.03 oy89.18 oz165.80 h225.0` | 225.0 | run | `0:fwd` | 3 | `blocked P=74.74,85.89,165.80 d=0.10` |
| UB3 | undrawn | 14 | 0x1506 | 0 | 36.06,16.63,204.10 | `r14 lx169 ly48 ox36.06 oy16.63 oz204.10 h225.0` | 225.0 | run | `0:fwd` | 3 | `blocked P=32.78,13.35,204.10 d=0.10` |
| GN1 | generated | 1 | 0x669A | 0 | 130.61,77.52,558.62 | `r1 lx822 ly1235 ox130.61 oy77.52 oz558.62` | 0.0 | run | `0:fwd` | 3 | `blocked P=130.61,82.45,559.18 d=0.15` |
| GN2 | generated | 4 | 0x1A23 | 0 | 40.97,40.97,192.11 | `r4 lx210 ly282 ox40.97 oy40.97 oz192.11` | 0.0 | run | `0:fwd` | 3 | `blocked P=40.97,45.98,192.12 d=0.15` |
| GN3 | generated | 5 | 0x655A | 0 | 117.96,37.66,441.31 | `r5 lx813 ly721 ox117.96 oy37.66 oz441.31 h90.0` | 90.0 | run | `0:fwd` | 3 | `blocked P=120.52,37.66,441.69 d=0.15` |
| GN4 | generated | 14 | 0x1FE2 | 0 | 117.03,136.97,435.00 | `r14 lx253 ly1814 ox117.03 oy136.97 oz435.00 h180.0` | 180.0 | run | `0:fwd` | 3 | `blocked P=117.03,131.97,435.76 d=0.15` |
| ST1 | step | 1 | 0x5B6D | 0 | 47.23,59.12,374.46 | `r1 lx730 ly874 ox47.23 oy59.12 oz374.46 h180.0` | 180.0 | walk | `wp=47.23,49.67` | 6 | `reach E=47.23,49.67,375.64 d=0.15 risers=0.26/0.13` |
| ST2 | step | 1 | 0x7C6E | 0 | 98.09,76.44,356.27 | `r1 lx996 ly883 ox98.09 oy76.44 oz356.27 h180.0` | 180.0 | walk | `wp=98.09,66.99` | 6 | `reach E=98.09,66.99,357.50 d=0.15 risers=0.32/0.13` |
| ST3 | step | 1 | 0x0725 | 0 | 138.81,81.08,349.98 | `r1 lx62 ly300 ox138.81 oy81.08 oz349.98 h90.0` | 90.0 | walk | `wp=148.26,81.08` | 6 | `reach E=148.26,81.08,351.19 d=0.15 risers=0.17/0.26` |
| SL1 | steep | 2 | 0xB876 | 0 | 69.88,75.06,552.84 | `r2 lx1475 ly947 ox69.88 oy75.06 oz552.84 h160.9` | 160.9 | run | `0:fwd` | 6 | `noclimb B=71.65,69.96,556.72 zmax=557.32 rmax=6.00` |
| SL2 | steep | 14 | 0x0A04 | 0 | 74.02,54.65,243.65 | `r14 lx83 ly34 ox74.02 oy54.65 oz243.65 h349.1` | 349.1 | run | `0:fwd` | 6 | `noclimb B=72.98,60.05,244.97 zmax=245.57 rmax=6.10` |
| SL3 | steep | 3 | 0x696C | 0 | 34.50,32.40,529.09 | `r3 lx841 ly865 ox34.50 oy32.40 oz529.09 h89.3` | 89.3 | run | `0:fwd` | 6 | `noclimb B=40.00,32.47,532.31 zmax=532.91 rmax=6.10` |
| IN1 | interior | 1 | 0x91A6 | 9 | 40.86,-108.12,-212.90 | `r1 lx1160 ly1328 cInside ox40.86 oy-108.12 oz-212.90 h90.0` | 90.0 | walk | `wp=46.86,-108.12` | 3 | `cells 9>8 E=46.86,-108.12,-212.90 d=0.15` |
| IN2 | interior | 3 | 0x98D6 | 49 | -1.56,-1.01,-221.09 | `r3 lx1216 ly1712 cInside ox-1.56 oy-1.01 oz-221.09 h90.0` | 90.0 | walk | `wp=4.44,-1.01` | 3 | `cells 49>48 E=4.44,-1.01,-221.09 d=0.15` |
| IN3 | interior | 4 | 0xFC85 | 31 | 69.01,-5.38,-93.83 | `r4 lx2016 ly1064 cInside ox69.01 oy-5.38 oz-93.83 h270.0` | 270.0 | walk | `wp=63.01,-5.38` | 3 | `cells 31>35 E=63.01,-5.38,-93.83 d=0.15` |
| IN4 | interior | 2 | 0xF3EE | 32 | 0.00,-7.00,-91.26 | `r2 lx1944 ly1904 cInside ox0.00 oy-7.00 oz-91.26 h180.0` | 180.0 | walk | `wp=0.00,-13.00` | 3 | `cells 32>39 E=0.00,-13.00,-91.26 d=0.15` |
| CV1 | cave | 1 | 0xE13A | 0 | 107.61,127.21,209.02 | `r1 lx1805 ly470 ox107.61 oy127.21 oz209.02 h30.0` | 30.0 | walk | `wp=115.11,140.20` | 8 | `follow E=115.11,140.20,208.03 d=0.15 dz=0.05 headroom=3.37` |
| CV2 | cave | 3 | 0x5B60 | 0 | 68.99,35.50,371.59 | `r3 lx731 ly769 ox68.99 oy35.50 oz371.59 h300.0` | 300.0 | walk | `wp=56.00,43.00` | 8 | `follow E=56.00,43.00,364.14 d=0.15 dz=0.05 headroom=4.11` |
| CV3 | cave | 4 | 0xC9AE | 0 | 19.11,63.44,697.15 | `r4 lx1608 ly1395 ox19.11 oy63.44 oz697.15 h195.0` | 195.0 | walk | `wp=15.23,48.95` | 8 | `follow E=15.23,48.95,697.90 d=0.15 dz=0.05 headroom=4.04` |
| WA1 | water | 1 | 0x9E7F | 0 | 73.22,128.04,337.68 | `r1 lx1267 ly1022 ox73.22 oy128.04 oz337.68 h224.9` | 224.9 | run | `wp=44.99,99.71` | 16 | `wade_swim in=72.58,127.40 swim=70.50,125.31 h=337.255 fh=1.42 d=0.50` |
| WA2 | water | 2 | 0x5BC7 | 0 | 100.87,125.06,942.79 | `r2 lx733 ly1598 ox100.87 oy125.06 oz942.79 h85.1` | 85.1 | run | `wp=140.72,128.48` | 16 | `wade_swim in=105.00,125.41 swim=114.72,126.25 h=941.177 fh=1.42 d=0.50` |
| WA3 | water | 3 | 0x7E2C | 0 | 128.11,108.17,149.90 | `r3 lx1014 ly357 ox128.11 oy108.17 oz149.90 h172.1` | 172.1 | run | `wp=133.61,68.55` | 16 | `wade_swim in=128.69,103.96 swim=129.31,99.55 h=149.020 fh=1.42 d=0.50` |
| WA4 | water | 5 | 0xEFB1 | 0 | 37.10,135.01,150.60 | `r5 lx1913 ly1422 ox37.10 oy135.01 oz150.60 h186.0` | 186.0 | run | `wp=32.92,95.23` | 16 | `wade_swim in=36.50,129.29 swim=36.07,125.16 h=149.020 fh=1.42 d=0.50` |
| FR1 | frill | 1 | 0x4C66 | 0 | 85.96,27.56,343.87 | `r1 lx612 ly817 ox85.96 oy27.56 oz343.87 h90.0` | 90.0 | run | `wp=115.96,27.56` | 6 | `pass E=115.96,27.56,335.92 d=0.10` |
| FR2 | frill | 3 | 0x7A69 | 0 | 66.00,106.57,189.51 | `r3 lx979 ly845 ox66.00 oy106.57 oz189.51 h90.0` | 90.0 | run | `wp=96.00,106.57` | 6 | `pass E=96.00,106.57,190.39 d=0.10` |
| FR3 | frill | 5 | 0x6FD9 | 0 | 100.17,88.49,173.75 | `r5 lx893 ly1740 ox100.17 oy88.49 oz173.75` | 0.0 | run | `wp=100.17,118.49` | 6 | `pass E=100.17,118.49,174.40 d=0.10` |

> **Confirmed 2026-10-09 (spec writer, CHK2 of [§1](#s1); was: implementer note of
> 2026-10-09, below).** Durations of 8 s are right for all three rows; no other field changes.
> Each path is 15.000 m long (from the start and `wp` coordinates). Walk mode forward gives the
> input (0, 0.5) and the Walking layer, so the target speed is 0.5 × 5.0 = 2.5 m/s, reached
> after 2.5 / 65 ≈ 0.04 s (`ControlAcceleration` 65 from BasePlayer; `collision.md` §10.3,
> §11.6 items 1–2, §11.8), which costs about 0.02 s. Uphill, the plane solver keeps the part
> of the horizontal request along the walkable face (`collision.md` §11.4), a factor
> 1 − (*g*·*d*)² / (1 + \|*g*\|²) for terrain gradient *g* and heading *d*; downhill keeps the
> full speed. Over the terrain under each path, sampled every 0.01 m:
>
> | Row | Rise and fall on the path | Uphill length | Lowest factor | Time at 2.5 m/s | Duration |
> |---|---|---|---|---|---|
> | CV1 | 209.02 → 206.61 → 208.03 | 9.98 m (up to 13.3°) | 0.968 | 6.10 s | 8 s (margin 1.9 s) |
> | CV2 | 371.59 → 364.14, falling all the way (down to 33.1°) | 0 m | 1.000 | 6.02 s | 8 s (margin 2.0 s) |
> | CV3 | 697.15 → 696.81 → 697.90 | 8.84 m (up to 10.6°) | 0.966 | 6.09 s | 8 s (margin 1.9 s) |
>
> The implementer's measured 6.3 s lies between these times and the durations. At the old
> 4 s a walker covers at most 10 m of the 15 m and cannot reach E. The other walk-mode rows
> still fit: ST1–ST3 walk 9.45 m in 6 s (3.8 s at 2.5 m/s, plus the step-ups), IN1–IN4 walk
> 6.0 m in 3 s (2.4 s).
>
> Was: **Implementer note (2026-10-09):** the durations of CV1–CV3 were raised from 4 s to 8 s.
> Walk mode now moves at the original's walking speed, 2.5 m/s forward (`collision.md`
> §11.8: the command values are halved and the Walking layer applies), instead of Tessera's
> earlier full-input Walking speed of about 4.9 m/s; their 15 m paths take about 6.3 s. No
> other field changed. For the spec writer to confirm.

<a id="s2-4"></a>
### 2.4 What each walk meets

**Walls and undrawn blockers** (blocking SEDs, `collision.md` §9.3; every one Physics,
`Physics_NoPhysics` false, not ethereal to Player, no content-layer list, 0x66 mesh). The
wall plane is the face hit by a horizontal ray 1.0 m above the start ground along the
heading; its distance minus the capsule radius is the stop distance within 0.04 m.

| Walk | Landblock info, SED index | Object → collision mesh | Kind (our words) | Drawn | Wall plane: distance from start along the heading, normal (x, y) |
|---|---|---|---|---|---|
| WL1 | 0x8021A197 #23 | 0x470003D3 → 0x660000BC | house (M3-02 anchor), south wall | yes | 5.53 m, (0, −1) |
| WL2 | 0x80226EA5 #20 | 0x470093DF → 0x66003840 | long barn (M3-06 anchor), long side | yes | 5.54 m, (−0.83, −0.56) |
| WL3 | 0x8023A73F #523 | 0x47007788 → 0x66002B41 | large building (M3-11 anchor), east wall | yes | 5.07 m, (1, 0) |
| WL4 | 0x8024C755 #40 | 0x470085DA → 0x66003426 | long building (M3-14 anchor) | yes | 5.21 m, (0.71, −0.70) |
| WL5 | 0x8025757A #6 | 0x4700E31C → 0x66004BF4 | two-storey building (M3-18 anchor), north wall | yes | 5.28 m, (0, 1) |
| UB1 | 0x80223B91 #102 | 0x47000C82 → 0x66000379 | invisible wall panel; mesh 10 × 2 × 10 m, SED scale 1.95 | **no** | 5.00 m, (0, −1) |
| UB2 | 0x80247CBC #0 | 0x47000DD1 → 0x66000446 | invisible wall block; mesh 30 × 10 × 33.5 m, SED scale 0.75 | **no** | 5.00 m, (0.71, 0.71) |
| UB3 | 0x802E1506 #385 | 0x47000C82 → 0x66000379 | invisible wall panel; mesh 10 × 2 × 10 m, scale 1 | **no** | 4.99 m, (0.71, 0.71) |

**Generated objects** (GEN; `scenery.md` §7). Each object is the record at the given index
of the landblock's generated object list (static pass, vertices in the order of §7.2). The
path names the vertex scene, the index of the group child, the index of the node child and,
for nested scenes, the scene reference and its children. Heading is the compass bearing of
the object's yaw (`coordinates.md` §11.3–§11.4). A = X + Y + R + 5 (`scenery.md` §6.3).

| Walk | Landblock (A) | Object list index | Vertex (i, j) | Path | Object → setup → collision mesh | Position x, y, z | Rotation (w, x, y, z); heading | Scale | Kind (our words); collision extent (m) |
|---|---|---|---|---|---|---|---|---|---|
| GN1 | 1, 0x669A (262); no landblock info, so no statics | 16 | (13, 8) | 0x02000152 > 2 > 0 | 0x470035F4 → 0x04000671 → 0x66003EF4 | 130.612, 83.519, 559.263 | (0.98286, 0, 0, 0.18435); 338.75° | 1.1701 | tall tree; trunk 2.6 × 1.9, 36.0 high |
| GN2 | 4, 0x1A23 (70); 24 statics with clearance volumes | 33 | (4, 5) | 0x020009D1 > 2 > 0 | 0x4700988D → 0x04006806 → 0x6600395A | 40.970, 46.965, 192.134 | (0.99017, 0, 0, −0.13984); 16.08° | 1.0176 | tree; 2.5 × 3.7, 20.1 high |
| GN3 | 5, 0x655A (201); 4 statics with clearance volumes | 176 | (13, 4) | 0x02000E92 > 0 > 0 > 0x02000EF9 > 17 > 0 | 0x4700D780 → 0x04007CA0 → 0x66004813 | 123.961, 37.660, 440.486 | (0.99149, 0.04956, −0.11506, 0.03534), tilted to the slope; 356.54° | 1.0304 | rock outcrop with a steep face; 5.9 × 4.8, 3.65 high |
| GN4 | 14, 0x1FE2 (276); 25 statics with clearance volumes | 138 | (12, 13) | 0x0200030F > 3 > 0 | 0x47003B84 → 0x04002B43 → 0x660015B8 | 117.026, 130.965, 435.875 | (0.14215, 0, 0, 0.98985); 196.34° | 0.9556 | tree; 2.9 × 2.7, 12.0 high |

On every row the first contact on the path is the listed object, and no object with
collision volumes (boxes, spheres, capsules) lies within 3 m of the path.
If GN1–GN4 fail because the object is absent or elsewhere, compare the landblock's whole
generated list with the explorer's scenery view first: the spot then reports a
generation difference, not a collision one.

**Steps.** All three climb the same kind of object, a platform with steps and a ramp
(0x47000AE1, collision mesh of its setup), placed at three spots; its risers differ with
the terrain under it. Ramp faces on the way are at most 33.7°.

| Walk | SED | Risers (m), in order | Rise start → end (m) |
|---|---|---|---|
| ST1 | 0x80215B6D #354 | 0.26, 0.13 | 374.46 → 375.64 |
| ST2 | 0x80217C6E #272 | 0.32, 0.13 | 356.27 → 357.50 |
| ST3 | 0x80210725 #67 | 0.17, 0.26 | 349.98 → 351.19 |

**Steep slopes.** Terrain only; no landblock info, and no generated object within 20 m.
SL1: triangles of 53.5–58.6° rise 4.1 m within 3 m of B; SL2: 60.0–61.4°, 5.5 m; SL3:
56.3–62.0°, 4.7 m.

**Interiors** (CELL). Dungeon cells reached by portal ([§1.1](#s1-1) item 1). Each start is
inside the first cell, 3 m before the portal's centre, on the floor of its physics mesh;
the end is 3 m past the portal. Portal sizes are width × height of the portal polygon.

| Walk | Landblock info | Start cell (geometry) | Portal | End cell (geometry) | Doorway: both cells |
|---|---|---|---|---|---|
| IN1 | 0x802191A6 | 9 (0x01000063) | #0, 30 × 15 m | 8 (0x01000038) | 3.00–3.02 m from the start |
| IN2 | 0x802398D6 | 49 (0x010002FF) | #0, 20 × 15 m | 48 (0x010002FF) | at 3.00 m |
| IN3 | 0x8024FC85 | 31 (0x010002EF) | #0, 20 × 15 m | 35 (0x010002FA) | at 3.00 m |
| IN4 | 0x8022F3EE | 32 (0x010002DB) | #0, 20 × 15 m | 39 (0x01000307) | 3.00–3.02 m |

**Caves** (`landblock.md` §3.5, `collision.md` §6.4). Every quad on the path has drawn
terrain and drawn ceiling triangles and four non-zero ceiling samples; `Landblock_UseCeiling`
(block-map key 9) has the value 1. CV1 floor 206.61–209.02, ceiling 212.38–215.59; CV2 floor
364.14–371.59, ceiling 375.71–385.45; CV3 floor 696.81–697.90, ceiling 701.94–703.00.
CV2 (CAVE, re-derived 2026-10-08) runs down a passage of the same cave as the former CV2
path (the same group of joined ceiling quads, 176 quads): the terrain under the path and
within the capsule radius of it slopes at most 35.6° (walkable, `collision.md` §11.3), the
fall is 7.45 m over 15 m, and no object of any kind lies within 3 m of the capsule on the way.
**Confirmed 2026-10-09 (spec writer, CHK):** E = start + 15 m along 300° = (56.00, 43.00); terrain
371.590 at the start and 364.141 at E, falling all the way; every sample and both rings lie on
quads with drawn terrain and ceiling and four non-zero ceiling samples (quads (11, 8), (12, 7),
(12, 8), (13, 7)); steepest triangle within the capsule radius 35.55°; floor 364.14–371.59,
ceiling 375.71–385.45, smallest headroom 4.115 m; old and new path in the same group of 176
joined quads; 625 quads with drawn floor and ceiling; the block has no landblock info, and no
generated object lies within 6 m of the path.

**Water** (WAT). No landblock info in any of the four blocks.

| Walk | Water type, level *L* → height *h* | Surface | Volume entity | Water body |
|---|---|---|---|---|
| WA1 | 37, 86 → 337.255 | 0x31004359 | 0x47000754 | lake (84 % of the samples below *h*) |
| WA2 | 11, 240 → 941.177 | 0x31004359 | 0x47000754 | mountain lake |
| WA3 | 15, 38 → 149.020 | 0x31006653 | 0x47000754 | sea (region 3 sea level) |
| WA4 | 23, 38 → 149.020 | 0x31008DB5 | 0x47000754 | sea (region 5 sea level) |

**Frill areas.** No blocking SED or generated object within 3 m of the path; frill records
(GEN frill pass) within 1 m of the path line: FR1 30 (mostly 0x4700077C, 0x47006200; vertex
scene 0x0200000A, a frill-only scene: the block has no group objects at all), FR2 51
(0x47006206, 0x47006239, 0x47005A50, 0x4700077A), FR3 35 (0x4700605D, 0x47006201,
0x47006083). Frills never collide (`scenery.md` §8.1), so the walk passes with or without
them; the row checks that none of them was turned into an entity.

> **OPEN (implementer, 2026-10-06, first run of every walk; `walker.md` §13.8):** 19 of
> 32 walks pass, all with 0.0000 m penetration. For the spec writer (§2.2: re-check before
> the implementation changes), the 13 others:
> - **WL2** (near miss): the first stop is 0.009 m from P, but forward stays held for 3 s
>   against a wall 34° off the heading and the walker creeps 0.104 m along it (d 0.10).
> - **GN2, GN3, GN4**: the first stop is 0.196, 0.036 and 0.109 m from P (the objects
>   block where the row says), then the walker slides around the trunk or rock with
>   forward held and ends 0.29, 0.39 and 1.23 m from P. `blocked` measures the end; the
>   SWEEP keeps the capsule upright and stops at the first contact. Should `blocked`
>   measure the first stop for round obstacles?
> - **GN1**: nothing blocks the path; the walker passes P and runs on (no generated object
>   body within reach). A generation difference (the block has no landblock info) or a
>   world-side gap; to be compared with the explorer's generated list as §2.4 says.
>   **Answered 2026-10-07 (spec writer, [§8](#s8)):** a generation difference on the
>   checklist's side. The former tree (0x470035F3 at 126.899, 87.394) came from a
>   generator with an inverted footprint test, and the client does not place it
>   (`scenery.md` §7.4 step 14b). GN1 now walks into another tree of the same landblock,
>   which the walker's generated list holds at index 16. GN2–GN4 keep their objects and
>   stop points; only the list indices changed (33, 176, 138).
> - **CV2**: the start lies on terrain steeper than walkable for the 0.35 m capsule
>   (support classified steep from the first step); the walker slides 11 m down before
>   the input begins.
> - **CV1**: the end point lies on a steep stretch: at (114.3, 140.8) the walker's
>   support turns steep (state 6) and it slides back; it ends 2.3 m short and 1.96 m below
>   E.
> - **FR1** (near miss): on the slope near the start the capsule rests 0.131 m above the
>   terrain under its axis (a sphere on a ~43° slope touches it beside the axis), against
>   `pass`'s 0.05 m.
> - **SL1–SL3**: the walker never climbs (highest feet below B.z) but never slides either:
>   the controller meets a steeper-than-walkable band from walkable ground as a wall
>   (`collision.md` §12.3 OPEN (e)), so state 6 never occurs. Either the condition "state
>   6 at least once" or reading (e) needs a decision.
> - **ST2**: stopped at the first riser (0.32 m, below the 0.5 m step-up height) with no
>   step-up; **FR3**: stopped on near-flat terrain at (100.17, 114.77) with two terrain
>   contacts and full forward speed. Both are controller faults reported to the
>   `tessera-physics` owner (a step-up landing on a riser/tread edge is classified by the
>   riser's face normal), not checklist issues. **Fixed in the M4.5 exit verification
>   (2026-10-06, `collision.md` §12.3 OPEN (n), (o)): both pass, so 21 of 32 walks pass.**
>
> **Answered 2026-10-08 (implementer, from `collision.md` §11.7 as Accepted on
> 2026-10-08):** WL2, GN3 and GN4 pass with `blocked` measured at the first touch (§2.2 as
> revised: the original also slides along a face held forward, at *s* · sin *φ*). GN1
> passes the same way (first touch 0.127 m from P), with the slope allowance on P.z (the
> walker touches the tree on a 30–34° cross slope). CV1 and FR1 pass with the slope
> allowance of §2.2 (43° stretches). GN2 and CV2 stay open (OPENs below; both answered the
> same day).
>
> **Confirmed and closed 2026-10-09 (spec writer):** the §2.2 revision follows `collision.md`
> §11.7 (see §2.2); GN1, GN2 and CV2 are confirmed in the two notes below. SL1–SL3, ST2 and FR3
> were settled in `collision.md` (§11.3.1 reading, §12.3 step 6, OPEN (n) and (o)); 32 of 32
> walks pass (ROADMAP M4.5 criterion 4).

> **Closed 2026-10-09 (was OPEN, implementer, 2026-10-08, GN2):** the walker first touches its tree
> (generated#33) at y ≈ 46.00, 0.195 m before P (d 0.15), on the trunk's steep foot below
> the step-up height. `collision.md` §11.7 ("A steep contact below the step-up height")
> says the original stops there too: the cast gets the face's vertical plane and the
> step-up of §11.5 fails without walkable support. The SWEEP of §1 ignores contacts below
> the step-up height by definition, so its P lies past where any capsule following
> §11.4–§11.7 first stops. Please re-derive P for GN2 with that rule (first touch of a
> steep or wall contact at any height, unless a step-up of 0.5502 m, `collision.md` §11.6
> item 5, lands on walkable support), and check whether P of GN1, GN3 and GN4 changes under
> it (they pass today, first touches 0.127, 0.003 and 0.015 m from P).
>
> **Answered 2026-10-08 (implementer, TOUCH of [§1](#s1); for the spec writer to confirm):**
> GN2's first non-walkable touch is on the trunk's foot 0.114 m above the feet (contact
> normal *u* 0.445, about 64° from up) at (40.97, 45.98), 0.209 m before the former P. The
> step-up from there rises the full 0.5502 m, moves on 0.10–0.16 m and comes down on the
> foot, 0.20–0.28 m above the ground, where no support contact is walkable, so it fails
> (with *L* = 0 it lands back at the touch point, which is no progress). New P =
> 40.97,45.98,192.12. GN1 changes too: its first touch is on the trunk 0.28 m above the feet
> (*u* 0.195), 0.161 m before the former P, and the step-up fails the same way (it comes
> down 0.23–0.40 m above the ground on the trunk's foot): new P =
> 130.61,82.45,559.18. GN3 (the rock face, touched 1.37 m above the feet) and GN4 (the
> trunk, 0.35 m above the feet, vertical) keep their P within 0.001 m; so do WL1–WL5 and
> UB1–UB3 (within 0.005 m). Every first touch is with the listed object (generated list
> indices 16, 33, 176, 138). The walker's first touches are now 0.086, 0.025, 0.003 and
> 0.024 m from P.
>
> **Confirmed 2026-10-09 (spec writer, CHK of [§1](#s1)).** First non-walkable touch, feet on
> the terrain, with the listed object each time (generated list indices 16, 33, 176, 138):
> - GN1 at (130.610, 82.449, 559.179): the trunk 0.28 m above the feet, *u* 0.19 (face normal
>   chosen). Step-up: rises the full 0.5502 m and comes down 0.23 m (*L* = 0.054) and 0.40 m
>   (*L* = 0.108) above the ground on the trunk's foot without walkable support; with *L* = 0 it
>   lands back on the touch point. Fails: P = 130.61,82.45,559.18 as listed.
> - GN2 at (40.970, 45.981, 192.122): the trunk's foot 0.11 m above the feet, contact normal
>   *u* 0.64 replaced by the steeper face normal *u* 0.445. Step-up lands 0.19 and 0.28 m above
>   the ground on the foot, not walkable. Fails: P = 40.97,45.98,192.12 as listed.
> - GN3 at (120.519, 37.660, 441.687): the rock face 1.37 m above the feet, vertical. The
>   forward cast of the step-up is stopped by the face; the descent comes back to the ground at
>   the touch (0.0001 m past it): no progress. P as listed (0.002 m).
> - GN4 at (117.030, 131.970, 435.757): the trunk 0.35 m above the feet, vertical; the step-up
>   makes no progress. P as listed (0.0001 m).
>
> With the capsule resting on the terrain (rest offset, convex radius 0) the touches move by
> 0.012 m (GN1), 0.004 m (GN2) and less than 0.001 m (GN3, GN4) along the path. Closed.

> **Closed 2026-10-09 (was OPEN, implementer, 2026-10-08, CV2):** the straight path from start to E runs along a
> cave bank steeper than the 45° walkable limit for players (`collision.md` §11.3): a clean
> session sampled the terrain triangulation (`landblock.md` §3.11) every 0.25 m and found
> every sample between 48° and 61° (52.8° at the start). The walker's support is steep
> from the first step, so it slides about 11 m down before the input begins and ends
> 13.7 m from E; the original would slide too under §11.3. The row needs a new cave path on
> walkable floor (at most 45°; the slope allowance of §2.2 then covers the capsule's rest
> offset).
>
> **Answered 2026-10-08 (implementer, CAVE of [§1](#s1); for the spec writer to confirm):**
> the new CV2 starts at (68.99, 35.50) and walks 15 m on heading 300° down a passage of the
> same cave to (56.00, 43.00), on terrain of at most 35.6° within the capsule radius, under a
> ceiling whose smallest headroom on the path is 4.11 m (the former path had 6.41 m); see
> "Caves" above. 625 quads of the block meet the drawn-floor-and-ceiling condition. The path
> was chosen among the CAVE candidates for a low ceiling (smallest headroom below 12 m, the
> ceiling at most 25 m above the path's highest floor point) and a floor that changes height
> on the way, as the former row's did (its line has the largest fall among them, 7.45 m).
> The walk passes (ends 0.035 m from E, feet within the §2.2 band). For the
> `tessera-physics` owner,
> not a checklist issue: walking the same line **uphill** (heading 120° from E), the walker
> slows from about 3.8 to about 1.2 m/s horizontally once its axis reaches the quad edge
> x = 60.0, at about (60.0, 40.7), and is counted as blocked at t = 1.28 s. On a single
> walkable plane of slope *θ* the move of `collision.md` §11.4 keeps 5 cos² *θ* of a 5 m/s
> horizontal request (3.3 m/s at 35.6°), so the slowdown looks like a property of Tessera's
> slide (§12.4 (n), (u), (v)) rather than of the original's rules; not examined further.
>
> **Confirmed 2026-10-09 (spec writer, CHK):** the new path meets every CAVE condition and lies
> in the same cave as the former one (figures under "Caves" above). Closed.

<a id="s2-5"></a>
### 2.5 The 1,000-walk soak: how its landblocks are chosen

A rule, applied to the user's install at run time:

1. **Regions:** the regions whose descriptor exists and that have landblock data (in this
   install 1, 2, 3, 4, 5 and 14), in ascending order.
2. **Land landblocks of a region:** every landblock-data record 0x800RXXYY with
   1 ≤ X, Y ≤ 253 (so all eight neighbours can exist) such that
   - its terrain chunk has at least one layer (drawn terrain, `landblock.md` §3.8);
   - its 1,089 height samples are not all equal (excludes flat filler blocks such as the
     undrawn height-0 blocks of [§1.1](#s1-1));
   - it is not fully submerged: it has no landblock water (`water.md` §6.2), or at least a
     quarter of its samples lie above the water height.
   Sorted by DID.
3. **Seeded choice.** A SplitMix64 generator with seed 0x4D34352D534F414B (the bytes of
   "M45-SOAK"), one draw u = (next() ≫ 11) / 2⁵³ per random number. Walk *k* (0 ≤ *k* < *N*,
   *N* ≥ 1,000) uses region number *k* mod (number of regions) of step 1, then a landblock
   ⌊u · n⌋ of that region's list (n = list length), a start x and y each 16 + 128 u, and a
   heading 360 u.
4. **Start:** the feet are put on the highest walkable support found by a downward sweep
   of the capsule from 3 m above the terrain at (x, y); if the capsule overlaps geometry
   there, or the support is a cell floor, draw a new (x, y).
5. **Inputs:** 60 s of simulated time in segments of 2 + 4 u seconds. Per segment, by one
   draw: run forward (u < 0.50), walk forward (< 0.65), backward (< 0.75), strafe left
   (< 0.80), strafe right (< 0.85), run forward with a jump at the segment start (< 0.95),
   stand still (otherwise); at each segment start the heading changes by 180 u − 90 degrees.
6. **Failure:** feet more than 0.05 m below the terrain height (`landblock.md` §3.11) of the
   landblock that contains them while outdoors, or more than 0.05 m below the floor of the
   cell that contains them (the highest cell physics-mesh triangle below the feet), or any
   non-finite position or velocity. The criterion is 0 failures.

The landblock lists and the seed are logged with the result, so a failing walk can be
replayed alone.

> **OPEN (implementer, 2026-10-06, soak; `walker.md` §13.7):** (a) the order of the draws
> is not given; Tessera draws landblock, x, y, heading, the (x, y) redraws of step 4, then
> per segment its length, kind and heading change. (b) Step 4 does not bound the redraws,
> and some land landblocks have no valid start: 3 of the first 1,003 planned walks (region
> 4 (23, 78), region 2 (203, 114) and (94, 207)) found none in 256 draws; Tessera passes
> such a landblock over and plans the next walk. (c) A start whose sweep finds no support,
> or a support that is not walkable, is redrawn too. (d) "Backward" and "strafe" segments
> use the run layer. Please confirm or state the rule.

---

<a id="s3"></a>
## 3. Performance walk spots (exit criterion 5)

Headless 1920 × 1080, unpaced, default-settings profile (`walker.md` §11.8), every M4.5
system on, 60 s per spot, p99 frame time ≤ 16.7 ms on the reference machine. A spot's path
is a camera path, not a collision test: the camera flies at 6.5 m/s through the waypoints,
2 m above the higher of the terrain and the landblock water surface, looking along the
path with pitch −10°, and loops until 60 s have passed. Stationary spots turn the camera
at 6° per second (one full turn in 60 s). Game time: day 12,240, time of day 0.5
([§4.0](#s4-0)).

| Spot | Kind | Region, landblock | Start (x, y) and ground z | Heading | Path (block-local waypoints, closed loop) | Why this spot |
|---|---|---|---|---|---|---|
| P1 | dense forest with frills | 2, 0x7744 | (30.00, 30.00), 155.31 | 90 | (30, 30) → (130, 30) → (130, 125) → (30, 125) | 364 generated objects and 23,172 frill records in the block (GEN) |
| P2 | town | 1, 0x8FF3 | (30.00, 30.00), 615.00 | 90 | the same rectangle | 468 drawn Physics SEDs with 0x66 meshes and 35 Light SEDs inside the block square, the most outdoor Light SEDs of any region-1 landblock; light map 0x41221577 |
| P3 | lake shore | 1, 0x9E7F | (30.00, 30.00), 330.14 (under water *h* 337.255; the camera flies at 339.26) | 90 | the same rectangle | lake type 37, *h* 337.255, 84 % of the samples under water; shore and slopes at the north-east corner |
| P4 | interior | 1, 0x91A6, cell 9 | (40.86, −108.12), −212.90 | 90 | stationary, turning | the IN1 dungeon: 30 × 15 m portals to neighbouring cells, interior fog (row Y5) |
| P5 | vista at the full landscape extent | 3, 0x53D7 | (152.00, 40.00), 431.48 | 190 | stationary, turning | a summit; along heading 190 the sea (*h* 149.02) is in direct view from 960 m to 2,060 m (W9), so terrain and water are drawn to the 2,080 m extent |

---

<a id="s4"></a>
## 4. Visual rows (exit criterion 6)

<a id="s4-0"></a>
### 4.0 Conventions

- **Camera.** "Eye" is the camera position in the frame of the row's landblock; heading is
  a compass bearing and pitch is negative downward. Field of view 45° vertical
  (`walker.md` §11.8). Free-fly camera; the avatar is hidden.
- **Game time.** Day number *n* and time of day *t* (`environment.md` §11.1). The original's
  epoch is OPEN, so Tessera's clock is set to the row's values; day type *D* = 2 (the
  Tessera initial value, [§1.1](#s1-1) item 3). *n* = 12,240 unless stated.
- **Colours.** Keyframe colours are given as R, G, B in 0–255 after blending
  (`environment.md` §5.6, §11.3). Property colours are given as stored (bytes B, G, R, A,
  `property-system.md` §4.7) and as R, G, B.
- **Facts.** "Expected" lists what Tessera must use at that spot (record DIDs and values);
  the walker side is checked by a real-data test or by the explorer's inspector, the look
  by the owner (items O20–O27).

<a id="s4-1"></a>
### 4.1 Terrain layers

Terrain-type keys of the region's 0x19 table (`terrain-texturing.md` §3); layers listed in
draw order, bottom first, as stored in the chunk (§4.2), keeping only the layers whose
triangles cover the triangle under the target point. Features of the type entry
(`terrain-texturing.md` §3.3): N = normal texture; S = both specular booleans set; Sn =
gloss from the normal texture; O = `use_opacity_mask` (the opacity switch of §8.1 for a
layer that is not the first drawn). With S, the layer's specular switch of §8.1 is on
unless O takes the diffuse alpha and Sn is absent; no row has that case. "Quad" is the quad-grid key there (§4.6).
The camera of every row: eye 4.28 m from the target against the heading, 3 m above the
target's ground, pitch −35° (the view ray meets the ground at the target on flat ground).
Time *t* = 0.5.

| Row | Region, landblock (table) | Target x, y, z | Heading | Layer stack: key (diffuse / normal / mask; features) | Quad | Also expected |
|---|---|---|---|---|---|---|
| T1 | 1, 0x7574 (0x19000000) | 128.90, 98.63, 387.15 | 0 | 2 (0x40015178 / 0x40015179 / 0x4100E6BD; N) · 6 (0x40000E0F / – / 0x4100E6C4; rotation 45°) · 7 (0x40000E0F / – / 0x4100E6C5) · 675 (0x40018390 / 0x4001838E / 0x41134A3F; N) · **10** (0x40001160 / 0x40001161 / 0x4100E6C6; N S; rotation 300°, scale 0.15) | 10 | road: a cobblestone road type is the top layer and the quad's key |
| T2 | 1, 0x54A0 | 37.34, 111.30, 501.09 | 0 | **391** (0x40002FD8 / 0x40003051 / no mask; N S; scale 0.055) | 391 | single snow layer with specular, no mask |
| T3 | 2, 0x65B1 (0x19000001) | 139.41, 106.00, 990.33 | 90 | **850** (0x4000112B / 0x4000243A / 0x411D9C74; N S) | 850 | cliff-top layer, normal-mapped and specular |
| T4 | 2, 0x4F32 | 139.42, 64.30, 410.83 | 90 | **195** (0x40016521 / 0x40016522 / 0x41110016; N; scale 0.35) | 195 | stone road, the only layer under the target |
| T5 | 3, 0x9FE8 (0x19000002) | 44.36, 27.70, 188.97 | 180 | 0 (0x400010FC / – / no mask) · 297 (0x40017270 / 0x40017271 / 0x41160C8A; N) · 306 (0x40001138 / – / 0x41160913) · **57** (0x4001A9DD / 0x4001A9DE / 0x41160D9C; N S) | 297 | **light-mapped block**: `Landblock_LightMap` 0x4116792D, added to the lighting (`terrain-texturing.md` §10) |
| T6 | 3, 0xD7E8 | 36.67, 21.67, 165.80 | 195 | **67** (0x4001AE5F / 0x400156E1 / 0x41166A06; N S; specular colour white, power 8.0) | 67 | open paved ground, specular from the diffuse alpha (see L5); the only layer within 1.5 m of the target (from 2 m on, layers 45 and 241 overlap it in places), terrain flat (165.80–166.27 within 5 m), no placed or generated object within 15 m. **Moved 2026-10-09** (spec writer) from 0x7FBD (52.91, 49.90, 192.02, mask 0x4116F0C6), where the terrain lies under a drawn platform ([§4.6](#s4-6), L5) |
| T7 | 3, 0x49E7 | 24.31, 121.73, 171.53 | 0 | 290 (0x40003F04 / 0x40003F05 / 0x4116367F; N) · **274** (0x400035B1 / 0x400035B2 / 0x41163485; N) | 274 | cobbled road over rocky dirt |
| T8 | 4, 0x25BC (0x19000007) | 34.19, 44.50, 68.53 | 0 | 385 (0x4001D061 / 0x4001D03D / no mask; N) · 257 (0x4001D0BF / 0x4001D027 / 0x41223EBD; N) · **232** (0x4001D02A / 0x4001D021 / 0x41223EBE; N) | 232 | brick road |
| T9 | 4, 0xAC6E | 67.77, 93.62, 226.95 | 0 | **609** (0x40017840 / 0x40018055 / 0x4120E819; N S; scale 0.05) · 613 (0x4001E0A0 / 0x4001E0A1 / 0x4120E81A; N; scale 0.375) | 609 | **light-mapped block** (0x411AD5A6) |
| T10 | 5, 0x5C6C (0x19000007) | 97.57, 49.58, 331.29 | 0 | 1089 (0x40016A54 / 0x40016A55 / 0x41255730; N) · **1256** (0x40022F9F / 0x40022F9D / 0x41255731; N S Sn O; rotation 37°) | 1256 | gloss from the normal texture; the diffuse alpha is the opacity |
| T11 | 5, 0x3B85 | 89.66, 104.89, 174.87 | 0 | 691 (0x40016A56 / 0x4001534A / 0x412363B2; N S) · 884 (0x40020D37 / 0x40020BE8 / 0x41245C92; N S Sn O) · **936** (0x40020BE7 / 0x40020BE8 / 0x41245FD8; N S Sn O) | 936 | paved road, three road layers |
| T12 | 5, 0x4F62 | 124.98, 105.99, 319.65 | 0 | 1103 · 1245 (O; rotation 60°) · 1256 · 1257 · 1258 · 1259 (all four N S Sn O, rotations 37°, 73°, 141°, 169°) · 1268 · 1220 (O) · 1222 (O) — nine layers | 1222 | **light-mapped block** (0x4125EE9F); a deep stack |
| T13 | 14, 0x07CA (0x19000001) | 74.78, 120.78, 575.94 | 0 | **33** (0x400039F3 / 0x400039F2 / 0x41205A82; N S) | 33 | tiled road in a light-mapped block (0x412056D3) |
| T14 | 14, 0x0DCA | 133.79, 82.41, 600.47 | 0 | 29 (0x40003796 / 0x40003795 / 0x41205288; N S) · **78** (0x40003B5A / 0x40003BA4 / 0x41205404; N S) | 29 | **light-mapped block** (0x412054EE) |
| T15 | 1, 0xE13A | 111.36, 133.71, 206.67 (ceiling 215.55) | 30 | floor: 774, 775, 776 (all 0x400175A7 / 0x400175A8; N; projections 2, 1, 3, scales 0.01, 0.01, 0.03) · 644 (0x40016C5D / 0x40016C5E / 0x41112F0A; N O) | 776 | **ceiling chunk** above (row CV1's cave): ceiling layers 639, 640, 641, 774, 775, 776 (0x400175A7 / 0x400175A8, masks 0x41112F00, 0x41112F01, 0x41112F02, 0x411E8B5F, 0x411E8B60, 0x411E8B61); ceiling quad 641; `Landblock_CeilingLightMap` 0x4111307C and `Landblock_LightMap` 0x4111307D. Camera for this row: eye at the target + 1.8 m, heading 30, pitch +30° (looking up at the ceiling). |
| T16 | 4, 0xC9AE | 17.17, 56.20, 696.87 (ceiling 702.93) | 195 | **426** (0x4001D041 / – / 0x411AF0FF; no normal) | 426 | **ceiling chunk**: one ceiling layer, key 426 (mask 0x411AF102); light map 0x411AF104. Eye at target + 1.8 m, pitch +30°. |

Projections are 3 (XY) unless stated; scales are `u_scale` (= `v_scale` in every row).

Row T6 was moved on 2026-10-09: its former target lay under the top of a drawn platform 0.30 m
above the terrain, so an image of it showed the platform, not layer 67 (CHK; details under L5
in [§4.7](#s4-7)). The new target is open ground of the same table and key.

Row T1 is judged on the image from the camera of [§4.7.1](#s4-7-1) (a placed roof stands
between the camera above and the target). Rows T4, T5 and T12 are judged on the image from the cameras of [§4.7](#s4-7) (the camera
above leaves a placed object, a column plinth or a generated rock between eye and target);
T15 and T16 keep their camera and add an image without the sky's lighting (§4.7).

<a id="s4-2"></a>
### 4.2 Scenery

Generated objects (GEN static pass) seen from the eye: the count of objects whose position
lies 3–30 m from the eye inside ±45° of the heading, the four most common DIDs among them,
and the two nearest with their exact placement (heading = compass bearing of the yaw).
Eye 1.8 m above the ground, pitch −10°, *t* = 0.5. A row passes when the walker's
generated list for the landblock contains every listed object at the listed position
(within 0.01 m), heading (0.1°) and scale (0.0001), and the owner sees the same objects in
the original.

| Row | Region, landblock | Eye x, y (ground z) | Heading | In view (total in block) | Most common DIDs | Two nearest: DID, position, heading, scale, vertex, path |
|---|---|---|---|---|---|---|
| S1 | 1, 0x669A | 123.88, 62.62 (559.55) | 30 | 6 (25) | 0x470035F4 ×2, 0x470035F6, 0x470035F3, 0x47003675 | 0x470035F6 (134.757, 76.422, 555.839) 222.29° 1.2196 (13, 8) 0x02000152 > 4 > 0; 0x470035F4 (130.612, 83.519, 559.263) 338.75° 1.1701 (13, 8) 0x02000152 > 2 > 0 (GN1) |
| S2 | 1, 0xA197 | 96.27, 42.08 (408.96) | 90 | 35 (315) | 0x470092DC ×9, 0x470021A0 ×8, 0x470092DF ×6, 0x470008C5 ×4 | 0x470092DC (101.636, 43.146, 404.978) 170.70° 0.7830 (10, 4) 0x02000CC1 > 14 > 0; 0x470008C5 (102.067, 41.334, 405.268) 200.52° 1.1077 (10, 4) 0x02000CC1 > 0 > 0 |
| S3 | 2, 0xAEBC | 24.43, 43.55 (672.29) | 120 | 11 (165) | 0x47006181 ×6, 0x47006180 ×5 | 0x47006181 (33.282, 36.418, 673.242) 84.89° 0.7148 (3, 4) 0x0200090A > 0 > 0; 0x47006181 (35.800, 43.432, 669.749) 156.23° 1.1352 (4, 4) 0x0200090A > 0 > 0 |
| S4 | 2, 0x7744 | 57.18, 124.28 (158.89) | 270 | 16 (309) | 0x4700617F ×7, 0x47005A5C ×3, 0x47005FB6 ×3, 0x47005FEC | 0x47005A62 (54.338, 122.030, 158.063) 64.84° 1.0634 (5, 12) 0x02000340 > 2 > 0; 0x47005A5C (50.475, 128.619, 159.243) 11.21° 0.9836 (5, 13) 0x02000340 > 0 > 0 |
| S5 | 3, 0x79AD | 66.49, 77.24 (311.99) | 240 | 9 (151) | 0x47006A36 ×2, 0x4700720E, 0x47007212, 0x470007CC | 0x47007213 (61.890, 74.593, 311.986) 18.84° 0.8633 (6, 7) 0x020005BA > 16 > 0; 0x47005FEC (55.744, 77.442, 311.986) 344.47° 0.6526 (6, 8) 0x020005B6 > 30 > 0 |
| S6 | 3, 0x7A69 | 76.48, 26.84 (190.22) | 180 | 6 (111) | 0x47006180 ×2, 0x47006BEB, 0x47000796, 0x47006BEA | 0x47006BEA (83.824, 18.210, 189.609) 100.84° 1.0760 (8, 2) 0x02000425 > 22 > 0; 0x47006BEB (68.317, 13.195, 189.385) 158.61° 1.2235 (7, 1) 0x02000425 > 15 > 0 |
| S7 | 4, 0x1A23 | 85.68, 93.43 (188.55) | 150 | 47 (445) | 0x47008202 ×6, 0x47008207 ×6, 0x47008203 ×6, 0x47008206 ×6 | 0x47008203 (85.939, 86.901, 187.329) 225.05° 0.9025 (9, 9) 0x020009CA > 4 > 0; 0x47007F5A (92.493, 91.001, 187.369) 311.70° 1.0031 (9, 9) 0x020009CA > 2 > 0 |
| S8 | 4, 0x7ADD | 119.52, 103.74 (252.65) | 60 | 14 (166) | 0x47006A36 ×3, 0x470007CA ×3, 0x4700079E, 0x47000620 | 0x47006A36 (122.779, 107.565, 252.148) 122.83° 1.1231 (12, 11) 0x020005BA > 25 > 0; 0x47000620 (126.422, 101.922, 254.398) 163.79° 1.1500 (13, 10) 0x020005B6 > 5 > 0 |
| S9 | 5, 0x655A | 123.83, 31.49 (442.10) | 30 | 18 (221) | 0x47000CAA ×4, 0x47008481, 0x4700C56C, 0x4700C85D | 0x4700D780 (123.961, 37.660, 440.486) 356.54° 1.0304 (13, 4) 0x02000E92 > 0 > 0 > 0x02000EF9 > 17 > 0 (tilted, GN3); 0x47008481 (130.043, 34.487, 444.051) 81.28° 0.4816 (13, 3) 0x02000E92 > 3 > 0 > 0x02000EFC > 1 > 0 |
| S10 | 5, 0x6FD9 | 59.39, 48.53 (174.25) | 150 | 22 (80) | 0x47007AA5 ×6, 0x47007AA8 ×5, 0x47007AA0 ×4, 0x47007AA1 ×4 | 0x47007CB0 (59.346, 42.853, 174.406) 128.72° 1.2184 (6, 4) 0x0200071A > 15 > 0; 0x47007AA0 (58.741, 41.622, 174.458) 17.71° 1.0178 (6, 4) 0x0200071A > 1 > 0 |
| S11 | 14, 0x1FE2 | 113.69, 66.56 (450.63) | 300 | 22 (186) | 0x470009A6 ×4, 0x47005B1E ×3, 0x470009A9 ×3, 0x470009A8 ×2 | 0x470009A6 (109.528, 66.155, 450.852) 291.40° 0.5993 (11, 7) 0x02000310 > 18 > 0; 0x47005B1E (106.341, 65.737, 450.953) 88.19° 0.7630 (11, 7) 0x02000310 > 38 > 0 |

On the image, S5 and S7 are judged from the cameras of [§4.7.1](#s4-7-1) (added 2026-10-10):
placed bushes surround S5's eye, and a placed rock lies between S7's eye and its two nearest
objects. One nearest object of each row cannot be seen from any eye (S5's second lies inside
placed bushes, S7's first inside the placed rock); those two are judged by the data test only.

> **Answered 2026-10-07 (spec writer):** the verification of 2026-10-06 found that
> Tessera's generator, which follows `scenery.md` §7.4 step 14b, missed 6 of the 22 listed
> objects and gave lower totals than this table. The table was wrong. It had been made with
> a generator whose footprint test was inverted ([§1](#s1), GEN). A BIN re-check
> confirmed step 14b as written (`scenery.md` S36). With the corrected generator, the
> rows above match the clean-room implementation's generated lists object for object in
> all 11 landblocks. Every row's total changed (S9: 221 instead of 362). The in-view
> counts and most common DIDs are unchanged in S3, S7 and S11 and changed in the other
> rows. The six objects that Tessera missed (both of S1, one each of S5, S6, S8 and S10)
> are not placed by the client and have left the rows. The next nearest objects take their
> place ([§8](#s8)).

<a id="s4-3"></a>
### 4.3 Frills

Frill records of the GEN frill pass (`scenery.md` §8.2, with item 5 as revised on
2026-10-07, and the Tessera Decision of §8.6; density 1.0, so the counts are exact). The
records of all six landblocks are identical under the earlier Tessera Decision of §8.2 ([§8](#s8)). The eye is 1.8 m above the target, pitch
−20°, *t* = 0.5. Expected drawing at the default level 2 (`scenery.md` §8.3–§8.4): frills
out to 97 units, fading from 67 units, distance reduction from 40 units; frills tinted
toward the terrain colour (intensity 1.0 unless the object says otherwise).

| Row | Region, landblock | Target x, y (z) | Heading | Frill records in block | within 10 m / 30 m | Most common within 10 m | Vertex scene at the target; its frill layer's scene references |
|---|---|---|---|---|---|---|---|
| F1 | 1, 0x4C66 | 85.96, 27.56 (343.87) | 90 | 4,075 | 22 / 453 | 0x47006200 ×11, 0x47006069 ×2, 0x4700077C ×2, 0x47000780, 0x47000784 | (9, 3) 0x0200000A; 0x020000CF, 0x020003A2 (no group objects in the whole block: frills only) |
| F2 | 3, 0x7A69 | 66.00, 106.57 (189.51) | 90 | 22,945 | 291 / 2,268 | 0x47006239 ×57, 0x4700077A ×15, 0x47006204 ×14, 0x470060EA ×12, 0x470060EC ×11 | (7, 11) 0x02000423; 0x02000450, 0x02000489, 0x02000475 |
| F3 | 5, 0x6FD9 | 100.17, 88.49 (173.75) | 0 | 15,584 | 314 / 1,940 | 0x4700605D ×188, 0x47006083 ×61, 0x47006201 ×51, 0x4700607C ×8 | (10, 9) 0x020005FA; 0x020005F0, 0x020004D9 |
| F4 | 2, 0x7744 | 93.14, 63.84 (153.74) | 90 | 23,172 | 284 / 2,699 | 0x47006062 ×165, 0x47006206 ×44, 0x4700606A ×36, 0x4700607C ×17 | (9, 6) 0x02000340; 0x0200034A, 0x020003A1 |
| F5 | 4, 0x7ADD | 64.18, 53.06 (278.03) | 90 | 15,066 | 135 / 818 | 0x4700605D ×80, 0x47006083 ×26, 0x47006200 ×18, 0x470071D9 ×4 | (6, 5) 0x020005BE; 0x020005AF, 0x020004DA |
| F6 | 14, 0x1FE2 | 80.00, 80.00 (448.98) | 0 | 8,347 | 132 / 1,119 | 0x4700078C ×42, 0x4700078D ×34, 0x4700078B ×31, 0x47006204 ×10 | (8, 8) 0x02000310; 0x0200030E (referenced twice) |

<a id="s4-4"></a>
### 4.4 Water

Water heights by `water.md` §6.2; *t* = 0.5 unless stated.

| Row | Region, landblock | Eye x, y, z | Heading, pitch | Expected |
|---|---|---|---|---|
| W1 | 5, 0xEFB1 | 37.10, 135.01, 152.40 | 186, −10 | **sea level**: type 23, level 38 → *h* 149.020; surface 0x31008DB5; volume entity 0x47000754 at (0, 0, 149.020); the shore of walk WA4 |
| W2 | 3, 0x7E2C | 128.11, 108.17, 151.70 | 172, −10 | **sea level** of region 3: type 15, level 38 → 149.020; surface 0x31006653 (a different surface from W1 at the same height) |
| W3 | 2, 0x5BC7 | 100.87, 125.06, 944.60 | 85, −10 | mountain lake: type 11, level 240 → 941.177; surface 0x31004359 |
| W4 | 1, 0x9E7F | 73.22, 128.04, 339.50 | 225, −15 | lake: type 37, level 86 → 337.255; surface 0x31004359; 84 % of the block's samples under water, deepest sample 314.24 at (0, 130) |
| W5 | 2, 0x81F8 | 84.90, 53.70, 449.22 (10 m above *h*) | 90, −45 | **river**: type 11, level 112 → 439.216; a west–east band of 10 % of the samples, deepest 436.29 at (150, 50); the neighbours to the west (0x80F8) carry the same level, the others are dry |
| W6 | 14, 0x3DEB | 150.00, 51.70, 390.39 | 90, −30 | **river with a step**: this block level 97 → 380.392, its east neighbour 0x3EEB level 94 → 368.627; the two planes meet at the shared edge x = 160 with an 11.765 m drop; surface 0x31004359 in both |
| W7 | 4, 0x38A8 | 95.00, 113.30, 152.02 | 0, −30 | **lava**: type 21, level 38 → 149.020; surface 0x31006FAE; volume entity 0x47008188 (lava monitors 0x78005CDF / CE0 / CE1, `water.md` §7.1–§7.2); 40 % of the samples below *h*. Look (added 2026-10-09; `water.md` §8.6.5): drawn by the lava record 0x2B000810, opaque and unlit, a red emissive pattern repeating about six times per landblock, brightness × 2.4 where the emissive alpha is 0 down to × 0.9 where it is 1, fogged like other surfaces; it does not move in this install (neutral flow map); never near-white, reflective or rippled. Owner: does the original's lava move (`water.md` §8.6.5 OPEN)? |
| W8 | 3, 0x4DE9 | 40.20, 95.63, 167.60 | 0, −20 | **placed water**: SED 0x80234DE9 #67, object 0x47006F68 at (40.20, 101.63, 165.74), scale 0.70, a water-plane mesh with water-volume monitors (`water.md` §7.3); its surface 0x31006748 is a surface of the plain water record 0x2B0000A8, drawn by the water programs at the plane's triangles (`water.md` §8.5.2; corrected 2026-10-09, was "unlit with source-alpha / inverse-source-alpha blending (`material.md` §11.3)", which shows the near-white placeholder texture, see row W12). The landblock has no landblock water. |
| W9 | 3, 0x53D7 | 152.00, 40.00, 433.48 | 190, −8 | **distant water** (perf spot P5): the sea at *h* 149.020 (type 11, level 38, sea floor at height 0) is in direct view from 960 m (block 0x52D1) to 2,060 m (0x51CA); the distant-landscape sectors 0x805351CF, 0x805351CC and 0x805351C9 cover those blocks (`water.md` §9). Expected: unbroken water out to the 2,080 m landscape extent. The block's sky key 2 gives 0x1A000000, fog far 2,500 at *t* 0.5, above the extent, so *G* = 2,080 m (`walker.md` §11.3). **Sky below the horizon** (added 2026-10-10, `environment.md` §12): beyond the extent the sky dome shows from about −7.8° up to the horizon, and a thin darker line about 0.66° below the horizon (the dome's equator, where its wrapped texture mixes in the pole colour, E40) is expected there by the Tessera Decision of §12, not a defect; owner item [O26](owner-observations.md#o26) asks whether the original shows it. |

W2 is judged on the image from the camera of [§4.7.1](#s4-7-1) (added 2026-10-10): the row's eye
stands on the shore among frill reeds.

<a id="s4-5"></a>
### 4.5 Sky and fog

ENV: *D* = 2 (local default table), *n* = 12,240. *a*, *b* and *f* are the keyframe indices
and factor of `environment.md` §11.3. Fog near and far in world units. Eyes: Y1–Y3 1.8 m
above the M3 stand point of the landblock (`m3-reference-locations.md` §2.2), Y4 1.8 m
above the target of row T1; heading 0, pitch +10° (some sky in view).

| Row | Region, landblock | Sky key → day description | *t* | *a*, *b*, *f* | Fog near / far, colour | Sun brightness, colour; toward-sun vector | Ambient brightness, colour 1 / colour 2 |
|---|---|---|---|---|---|---|---|
| Y1 | 1, 0x5378 (M3-01) | 0x1000023B → 0x1A000359 | 0.5 | 4, 5, 0.5588 | 300.0 / 2,058.8, (88, 126, 146) | 0.55, (255, 243, 189); (−0.224, −0.837, 0.500) | 0.20, (226, 230, 240) / (226, 230, 240) |
| Y2 | 4, 0xC755 (M3-14) | 0x10000106 → 0x1A000269 | 0.25 | 1, 2, 0.4545 | 363.6 / 2,500.0, (15, 5.3, 0) | 0.50, (123.3, 48.0, 28.4); heading 7, pitch 50: (0.078, 0.638, 0.766) | 0.40, (89, 95, 120) / (31, 33, 42) |
| Y3 | 3, 0x693E (M3-10) | 0x10000084 → 0x1A000213 | 0.78 | 6, 7, 0.75 | 200.0 / 1,725.0, (140.3, 77.3, 67.8) | 0.55, (245.3, 174.0, 86.3); (−0.224, −0.837, 0.500) | 0.20, (226, 230, 240) / (226, 230, 240) |
| Y4 | 1, 0x7574 | 0x10000070 → 0x1A000000 | 0.0 | 0, 1, 0 | 1,000.0 / 2,500.0, (1, 2, 6) | 0.55, (74, 81, 158) (night); (−0.224, −0.837, 0.500) | 0.20, (125, 121, 147) / (125, 121, 147) |
| Y5 | 1, 0x91A6, cell 9 (IN1 start) | interior | any | – | **interior fog** from the landblock-info property set 0x802191A6: `InteriorFogType` 1 (Linear), `InteriorFogNear` 0.059, `InteriorFogFar` 90, `InteriorFogColor` stored (2, 14, 11, 255) = R 11, G 14, B 2; `InteriorAmbientLight` stored (8, 25, 8, 255) | – | The cell itself carries no fog properties; the landblock info does (`environment.md` §7.2, §12 last item). Eye at the IN1 start + 1.8 m, heading 90, pitch 0. |

Y2 and Y3 are judged on the image facing south (heading 180; Y2 [§4.7](#s4-7), Y3 [§4.7.1](#s4-7-1)): at heading 0 the eye
of Y3 faces the walls of M3-10's building, and the eye of Y2
faces the wall of M3-14's long building 1.5 m away.

Y1 and Y3 use the same sun direction (fact 4 of [§1.1](#s1-1)); the time of day shows in the
colours and fog. Y2's description has a different sun (7°, 50°) and ambient colour 2
darker than colour 1 (hemispheric term, `lighting.md` §8.1). Y4 is night: the sun colour
is a dim blue moonlight. For Y1 the global object limit *G* of `walker.md` §11.3 is the fog
far distance 2,058.8 m, just below the 2,080 m landscape extent (Tessera Decision of that
section: clip at fog far whenever fog is on).

<a id="s4-6"></a>
### 4.6 Lighting

| Row | Region, landblock | Eye x, y, z; heading, pitch | *t* (day description) | Expected |
|---|---|---|---|---|
| L1 | 1, 0x5378 | 60.00, 40.00, 465.05; 333, −15 | 0.5 (0x1A000359) | **sun shadow**: the barn of M3-01 (SED 0x80215378 #0, object 0x47000E92, top 467.36, 12.3 m above the ground 455.05) casts its shadow toward bearing 15° (away from the sun at bearing 195°, elevation 30°); the shadow of its top falls 21.3 m (12.3 × cot 30°) from the wall toward 15°, on flat ground. Shadow parameters per the Tessera Decision of `lighting.md` §9. |
| L2 | 1, 0x8FF3 | 100.00, 25.00, 617.00; 0, −5 | 0.0 (0x1A000377: fog 0 / 300, ambient 0.20 (190, 228, 195)) | **point lights at night**: Light SEDs #460 at (90.30, 42.41, 621.09), #462 (125.62, 48.18, 621.09), #483 (74.68, 34.44, 621.09), #464 (119.10, 80.34, 621.09), #474 (71.64, 72.15, 621.09), 6.1 m above the ground (615.00); each `LightRange` 10, `LightFalloff` 0.5 (attenuation *k* = 0.05, half strength at 4.5 m, `lighting.md` §8.3), `LightDiffuse` stored (3, 97, 251, 255) = R 251, G 97, B 3 (orange), `Light_BakedLandscapeLight` true. On objects within 10 m they add orange light; on the terrain they do not (Tessera Decision of `lighting.md` §8.3: the block has light map 0x41221577, which carries their light). Fog far 300: no object beyond 300 m is drawn (*G* = 300, `walker.md` §11.3). |
| L3 | 1, 0x4883 | 92.78, 41.64, 394.95; 0, −10 (corrected 2026-10-10 by the spec writer: 1.8 m above the placed platform the bonfire stands on; was 392.73, 1.8 m above the terrain and 0.42 m below the platform's top, [§4.7.1](#s4-7-1)) | 0.0 (0x1A000359: fog 200 / 700, ambient 0.40 (94, 91, 114)) | **emissive**: a camp bonfire (SED 0x80214883 #133, object 0x47005179, setup 0x04003A50) 6 m ahead; its surface 0x310045AE (lit, `ColorAndGloss`) has emissive texture 0x400166F3 (slot 3018) and glow colour (1, 1, 1, 1): it stays bright where the ambient is dark (`lighting.md` §5.9). |
| L4 | 2, 0x0340 | 128.00, 104.00, 244.53; 30, −15 | 0.5 (0x1A0001F3: one keyframe, sun brightness 0, fog 5 / 900, ambient 0.20 (32, 28, 21)) | **glow and emissive** in a dark place: a crystal cluster (SED 0x80220340 #18, object 0x470069D6, at (132.16, 110.19, 243.69), tilted) with surface 0x3100631F: emissive 0x40018902, glow mask (slot 3013) 0x40018902, glow colour (1, 1, 1, 1), drawn into the glow target (pass 0x08). Also: skull light poles 0x47005D98 near L3 (#297 at (73.22, 37.49, 389.71)) with glow surface 0x310051B1 (glow mask 0x40017652). |
| L5 | 3, 0xD7E8 | 38.45, 28.34, 169.78; 195, −25 (corrected 2026-10-09 by the spec writer; 2026-10-08: 0x7FBD, 64.59, 53.19, 196.00; 195, −25; first: 0x7FBD, 54.98, 57.63, 195.02; 195, −20) | 0.5 (sky key 2 → 0x1A000000: sun brightness 0.55, colour and specular white, ambient 0.20 (226, 230, 240)) | **specular**: row T6's terrain layer 67 (specular switch on, gloss from the diffuse alpha, specular colour white, power 8.0), open and flat, between the eye and the sun (bearing 195°, elevation 30°). The sun's mirror point P (36.67, 21.68, 165.80) lies 6.9 m ahead, 5° below the view centre (which meets the ground at (36.24, 20.11), also layer 67); no placed or generated object stands within 15 m of P, and P and the ground 2 m round it are in sunlight. Expected: **a faint white sheen** round P; `terrain-texturing.md` §9.4 (T44): the gloss is the diffuse alpha, 0–0.20 (mean 0.09), so the term adds about 0.09 per channel (at most 0.2) at the centre of the reflection, not a broad highlight. Comparison: the mirrored camera 34.88, 15.02, 169.78; heading 15, pitch −25 (P with the sun behind the eye; ground under it 166.20) shows the same paving round P without the sheen. The implementer's revision of 2026-10-08 (faint sheen, from §9.4) is confirmed; its spot is not: in 0x7FBD the paving round both mirror points and T6's target lies under the top of a drawn platform ([§4.7](#s4-7), note under the table). |
| L6 | 1, 0x3F7C | 47.66, 98.54, 452.12; 0, 0 | 0.5 | **blended (translucent) surface**: a cobweb (SED 0x80213F7C #9, object 0x470004F3 at (47.66, 104.54, 450.35)), surface 0x31000493: lit, `ColorAndOpacity` with alpha test off, so it blends by its texture alpha (`material.md` §11.3) and is drawn in the transparent pass. See W8 for an unlit alpha-blended surface. |
| L7 | 1, 0x7873 | 78.12, 21.00, 409.00 (20 m above the ground); 0, −89 (redefined 2026-10-09 by the spec writer; was 3, 0x9FE8, 44.36, 23.42, 191.97; 0, −35) | 0.0 (sky key 0x10000070 → 0x1A000000, keyframes *a* = 0, *b* = 1, *f* = 0: the night of row Y4, moonlight 0.55 (74, 81, 158) from bearing 195°, elevation 30°, ambient 0.20 (125, 121, 147) for both colours, fog 1,000 / 2,500) | **baked light map on terrain at night**: `Landblock_LightMap` 0x41102099 (DXT1, 128 × 128; 95 % of its texels black, the rest warm, largest (0.77, 0.48, 0.00)). Under the eye a camp: a campfire with Light SED #210 (object 0x470005C2 at (78.51, 19.33, 390.92), `LightRange` 15, `LightDiffuse` stored (0, 158, 255, 255) = R 255, G 158, B 0, `Light_BakedLandscapeLight` true) inside a ring of logs, between two large tents. The map's pool round the fire: 76 texels above 0.15 between x 71–84 and y 15–30, brightest (0.68, 0.41, 0.00) at (78.12, 18.12, 389.00); no blocking object covers that point. Expected: an **orange pool on the ground round the fire**, the map's texel added to the lighting sum (`terrain-texturing.md` §9.1 step 1, §10). Outside the pool the flat ground gets about (0.18, 0.18, 0.29) (ambient *X* = (0.10, 0.09, 0.12) plus the moonlight 0.55 × (74, 81, 158) / 255 × sin 30°, `lighting.md` §8.1 Tessera Decision *E₀* = *Cd* × *bd*); at the brightest texel about (0.85, 0.59, 0.29): on the same terrain layer red about 4.8 times, green about 3.3 times the value outside, **blue unchanged** (the map's blue is 0). The nine Light SEDs of the camp (#209–#216, #219) all carry `Light_BakedLandscapeLight`, and no light without it stands within 25 m of the pool (this block and its neighbours), so no point light reaches the terrain there (§10, T43). Comparisons: `--disable lights` (the terrain identical, only objects lose their orange light, as in L2) and the same camera at *t* 0.5 (daylight: ambient (226, 230, 240) × 0.2 and a white sun; the same pool is added but stands out far less). |

> **Closed 2026-10-10 (was OPEN, implementer, 2026-10-10, row L3, commit d82be77):** the
> eye stood inside the placed platform that carries the bonfire, so the buried cone of the
> bonfire's mesh showed. Reconciled with the spec writer's correction of the same day
> ([§4.7.1](#s4-7-1)): L3's eye is now 92.78, 41.64, 394.95 (1.8 m above the platform's top at
> 393.15; the implementer's floor reading of about 393.17 and proposed eye 394.90 agree within
> 0.05 m), and the expected look is the one of §4.7.1 (logs, embers bright with the emissive
> texture, the stone ring; no cone). Nothing else changes.

**Generated figures seen from L5 (added 2026-10-10, spec writer; `appearance.md` §7.4).** Since
the walker draws generated objects on a skinned rig as figures (commit 566679b), L5's camera
sees the 54 generated rigs of the neighbouring block 3, 0xD7E7. They are fallen soldiers,
objects 0x47007170–0x47007178 (`appearance.md` A58): random types 0xD8, 0xD6 and 0xF5,
three objects each, with lying-dead starting idles. **Expected:** the figures lie on the
ground in their dead idles. They are dressed by their random-type cases, and their cloth takes
a colour from the cases' random dye lists (`appearance.md` §7.4, "Random dye colours"). The
hooded type 0xF5 is in dark greens, olives and grey browns. Types 0xD8 and 0xD6 have
near-black upper bodies. Hair is brown, grey or black where it shows. **Not expected:** the
bind pose with the arms straight out, or a magenta tunic (the dye edits' stored placeholder,
seen on three of eight figures in the exit check of 2026-10-10). Owner item
[O92](owner-observations.md#o89) compares them with the original.

<a id="s4-7"></a>
### 4.7 Cameras for judging rows on the image (implementer, 2026-10-08)

On 2026-10-07 nine rows could not be passed on the walker's image: L5 failed, and T4, T5, T12,
T15, T16, Y2, L2 and L7 could not be judged from their cameras ([ROADMAP M4.5](../ROADMAP.md#m4-5-exit)
criterion 6). The cameras below were chosen for them with Tessera's walker and its headless
screenshot path (1600 × 900, free-fly, no HUD, avatar hidden, clock stopped, day 12,240,
*D* = 2): `apps/client/tests/visual_rows_real_data.rs`, table `CAMERAS`, rendered by
`row_cameras_render` into the directory named by `TESSERA_VISUAL_SHOTS` (private images, not
committed; `row_cameras_frame_their_targets` checks that each camera frames its target). Eyes
are block-local; a comparison image uses the same eye unless the row says otherwise. Sky
values were evaluated with the walker's own lookup (`camera_sky_key`, `environment.md` §11)
on the owner's install. Brightness figures are means of 0–255 output values.

| Row | Camera: eye; heading, pitch; *t* | Comparison | Seen | Result |
|---|---|---|---|---|
| T4 | 151.42, 64.30, 416.83; 270, −26.6; 0.5 (12 m east of the target, 6 m up) | `--disable layers` | The target lies in a court walled by placed objects; from the eye of §4.1 (west of the target) a wall fills the view at close range. Looking west into the court, one uniform tiled paving covers the ground at and round the target, with no second layer and no mask edge. With layers off the floor becomes the untextured height-shaded mesh, so the paving is the terrain layer. | pass |
| T5 | 44.36, 37.70, 193.97; 180, −26.6; 0.5 | – | The target lies under a placed curb at the foot of a column plinth (about 0.4 m inside the plinth's footprint, seen top-down). The ground of its triangle shows tiled paving with an ornamented border north of the curb and grass south of it, blended along mask edges, without holes. The scene is dim because of the block's day description (sky key 0x10000030 → 0x1A00022B: ambient brightness 0.30, colours (64, 56, 68) / (35, 26, 24); sun brightness 0.25), not because of the light map: 0x4116792D is black (`terrain-texturing.md` §10) and adds nothing visible. | pass (layers; the light map's DID is checked by the test) |
| T12 | 132.98, 105.99, 323.65; 270, −26.6; 0.5 | – | Looking west past the generated rock (now to the left of the view): dirt, pebble and cracked-earth layers blend at and round the target, under frills; no holes or seams. | pass |
| T15 | the row's camera: 111.36, 133.71, 208.47; 30, +30; 0.5 | `--disable sky` (the defaults of `lighting.md` §8.1: ambient 0.7, white) | Near black, mean (5.8, 7.9, 8.3). That is what the specs give: a ceiling receives the downward ambient *Y* plus its ceiling light map and unbaked lights (`terrain-texturing.md` §4.8, §4.10; `lighting.md` §8.1), and this cave's sky key 0x1000021A (→ 0x1A00018B) has ambient brightness 0.05 with colour (57, 82, 91) and no sun, so *Y* ≈ (0.011, 0.016, 0.018). Stretched ×8, the rock layers of the ceiling show with mask edges, and a lighter green-blue area at the left. Without the sky's lighting the same layers draw at mean (85, 87, 93); the ratio of the two images, (0.068, 0.091, 0.090), lies above *Y* / 0.7 = (0.016, 0.023, 0.026), so some light other than *Y* reaches the ceiling (its light map 0x4111307C, or unbaked lights). | pass |
| T16 | the row's camera: 17.17, 56.20, 698.67; 195, +30; 0.5 | `--disable sky` | Near black, mean (0.8, 1.1, 1.3). Sky key 0x100001ED → 0x1A000278: ambient 0.30, colour 2 (31, 33, 42), so *Y* ≈ (0.036, 0.039, 0.049); the block has no ceiling light map (0x411AF104 is the terrain's). The ratio of the two images, (0.060, 0.062, 0.076), matches *Y* / 0.7 = (0.052, 0.055, 0.070) within the rounding of values of 1–2: the ceiling is lit by *Y* alone, as specified. Stretched ×30, the single layer 426 draws (rock, smeared along the steep ceiling by the XY projection). | pass |
| Y2 | 33.74, 131.37, 157.20 (M3-14 stand point + 1.8 m); 180, +10; 0.25 | – | Red dawn sky; the distant ridges fade into the dark red fog; the north faces of rocks are lit red-brown by the sun behind the eye (heading 7°, 50° up); the ground is dark under the darker ambient colour 2. Outside the row: east and south of the stand point, window and door frames stand in the air without walls (also at heading 90); for the owner to compare (the row does not depend on it). | pass |
| L2 | 90.30, 42.41, 630.00 (15 m above the ground under light #460); 0, −89; 0.0 | `--disable lights` | The difference of the two images is orange on objects only (a building wall, the lamp, small rocks within about 10 m) and zero on the terrain: the lights reach objects, not the ground (`lighting.md` §8.3, `terrain-texturing.md` §10). Without the point lights the ground keeps a warm pool round the lamp: red 66 at 1 m from the lamp's foot, falling to about 20 from 8 m on, while blue stays at 15–18; an added light of about the hue (0.29, 0.11, 0) of the light map's pools (§10, OBS5). The fog limit *G* = 300 is not judged from this camera. | pass |
| L5 | 64.59, 53.19, 196.00; 195, −25; 0.5 (the sun's mirror point P (62.80, 46.50) is on layer 67, in sunlight) | the mirrored camera 61.01, 39.81, 196.00; heading 15, same pitch (P with the sun behind the eye) | Towards the sun the sunlit paving round P is brighter by (21.6, 20.6, 20.1) than from the mirrored camera (200 × 140 px boxes round P; (18.1, 17.8, 16.8) over 400 × 200 px round P): a white sheen of about the size §9.4 gives (gloss mean 0.09); no broad highlight. | pass under the revised row (§4.6); **void 2026-10-09**: P lies on a drawn platform, not on layer 67 (note below); re-judge with the cameras of the corrected row L5 in §4.6 |
| L7 | the row's camera: 44.36, 23.42, 191.97; 0, −35; 0.0 | the same at *t* 0.5 | The two images are identical (mean absolute difference 0): the block's sky key 0x10000030 gives 0x1A00022B with the same values at *t* 0 and 0.5 (sun brightness 0.25, ambient 0.30), so the block has no night; and its light map 0x4116792D is black (`terrain-texturing.md` §10), so no camera can show warm texels. | not judgeable as defined then; **row redefined 2026-10-09** (§4.6: block 0x7873, map 0x41102099, *t* 0.0 with a night); to be judged with the row's camera and comparisons |

**L5, diagnosis (2026-10-08): a row-definition problem, not a code bug and not a spec gap.**
The walker's terrain shader follows `terrain-texturing.md` §9.4 (Accepted 2026-10-07): for
key 67 the gloss is the diffuse alpha (gloss from normal is off), and the highlight *g* ·
sat(*n* · **H**)^*p* · *F₀* · *S* is added after the factor 2, with *p* = 8 and *F₀* = the sun's
specular colour (white, the Tessera Decision of `lighting.md` §8.1) × `specular_colour`
(white). The row failed for two reasons. (1) It expected "a broad highlight", which §9.4
rules out (T44: layer 67's diffuse alpha is 0–0.20, mean 0.09: a faint sheen). (2) From the
row's eye the sun's mirror point on the flat paving lies 5.2 m ahead, at about (53.6, 52.6),
in the shadow of the buildings to the south (a top-down image at noon shows it), and there
*S* = 0 removes the term (§9.4; `lighting.md` §9). The row's eye and expectation were
revised in §4.6 for the spec writer to confirm.

**L5 and T6, spec writer (2026-10-09, CHK).** Reason (1) and the revised expectation are
confirmed: §9.4 gives a faint sheen for key 67. The revised spot is not usable. In 0x7FBD
the paving round the new mirror point (62.81, 46.53), the former one (53.64, 52.61) and T6's
target (52.91, 49.90) lies under a drawn placed object: SED 0x80237FBD #115, object
0x470078AC, a stone platform whose render mesh 0x0600741C (drawn at detail level 0, surfaces
0x31006680 and 0x31006684) has a level top at 192.32, 0.30 m above the terrain (192.02),
over x 34.8–75.8, y 25.8–55.8; #116 is the same object at y 65.71 and covers y 50.7–80.7. So
the images of 2026-10-07 and 2026-10-08 showed the platform's top surface, and the sheen
measured on 2026-10-08 came from it, not from the terrain layer. L5 and T6 now use open,
flat paving of the same table and key in 0xD7E8, whose sky key 2 gives the same day
description 0x1A000000 and the same sun (§4.1, §4.6).

<a id="s4-7-1"></a>
#### 4.7.1 Cameras re-derived on 2026-10-10 (spec writer)

On 2026-10-10 the re-judgement on main `01deafd` ([ROADMAP M4.5](../ROADMAP.md#m4-5-exit)
criterion 6) could not judge T1, Y3, W2, S5 and S7 from their cameras, and saw L3's bonfire
drawn with an upside-down pyramid under its logs. The cameras below replace the row's camera
**for judging on the image only**; every expected value of the rows (layer stacks, sky values,
water heights, generated lists, in-view counts) stays as listed, computed from the row's own
eye. Placed-object facts come from throwaway dirty-room probes on the owner's install (CAM of
[§1](#s1), 2026-10-10: setups, render meshes at detail level 0, SED placements, terrain heights
by `landblock.md` §3.11); each camera was rendered with Tessera's walker (`--spawn R,X,Y,x,y,z
--fly`, 1600 × 900, `--no-hud`, clock stopped, *D* = 2, day 12,240; private images, deleted
after viewing), and the image positions below were projected with the 45° vertical field of
view of [§4.0](#s4-0). Eyes are block-local; the eye *z* is the camera itself.

| Row | Camera: eye; heading, pitch; *t* | Comparison | Why the row's camera fails | What this camera frames (checked by rendering) |
|---|---|---|---|---|
| T1 | 128.90, 102.91, 390.15; 180, −35; 0.5 (the §4.1 geometry turned round: 4.28 m north of the target, 3 m above its ground) | – | From the south (heading 0) the roof of a placed building stands between eye and target and fills the image centre. | The target at the image centre on the cobbled road, open to the eye; the building's wall beyond it, the target in its shadow at *t* 0.5. Judge the road layer 10 at the centre. |
| Y3 | 19.47, 44.71, 227.17 (the row's eye); 180, +10; 0.78 | – | At heading 0 (and 270) the walls of M3-10's building stand within a few metres of the stand point. | Facing south: the evening sky, the fog over the far ridges, trees in front of them; the sun (bearing 195°, elevation 30°) lies 15° right of and 20° above the view centre. Judge fog and sky colours as in the row (as Y2 in [§4.7](#s4-7)). |
| W2 | 129.78, 96.29, 151.70; 172, −10; 0.5 (12 m along the heading from the row's eye, over water 2.85 m deep: terrain 146.17, *h* 149.020) | `--disable water` | The row's eye stands on the shore (ground 149.90), inside a bed of frill reeds that fill the view. | Open sea from the near water (the stony bed seen through it) to the far shore; a few reeds stand on a bank at the right edge. In the comparison the sea floor and the bank lie bare. The far sea is very dark at this sky. Tessera has no reflection pass and puts the refraction where the reflected sky would be (surface 0x31006653 of the flow-map record 0x2B0007FD, `water.md` §8.6.4 Tessera rule), so toward grazing angles its far sea can be darker than the original's. Owner item [O91](owner-observations.md#o88): whether the original's far sea here is as dark or reflects the sky. |
| S5 | 74.88, 82.09, 316.99 (5 m above the ground 311.99, 15 m from the nearest object against the heading); 240, 0; 0.5 | `--disable scenery` | Placed bushes (object 0x4700061F, SED 0x802379AD #304 at (61.68, 76.04), about 11 × 7 m and 4.5 m high; object 0x470007CE, #303 and #306) surround the row's eye and both nearest objects; their leaves fill the view at 1.8 m. | The nearest object, tree 0x47007213 (10.6 m tall at its scale), stands on the image's centre column: foot at about (800, 812), crown top at about (800, 51), its foliage over the right half. The second, 0x47005FEC (about 1 m across), projects to about (1119, 737) but lies inside the boxes of the placed bushes #303 and #306 and behind the tree's foliage: not visible; it is judged by the data test only. In the comparison the tree and the other generated trees vanish and the placed walls and bushes stay. |
| S7 | 85.68, 93.43, 194.55 (6 m above the row's eye ground 188.55); 140, −45; 0.5 | `--disable scenery` | Not a hump of ground: a placed rock formation, object 0x47009A65 (SED 0x80241A23 #15 at (81.35, 89.32, 163.95), scale 1.5), lies between eye and objects. Its top surface over the nearest object is at 188.06. | The second object, 0x47007F5A at (92.493, 91.001), projects to about (381, 526): a low cluster on open ground, gone in the comparison (the rock's top lies at 186.47 there, below it). The nearest, 0x47008203 at (85.939, 86.901, 187.329), about 0.13 m tall, lies 0.73 m below the rock's top surface at its position, inside the rock: invisible from every eye in either client; it is judged by the data test only (about (1296, 630), on the rock). |
| L3 | 92.78, 41.64, 394.95; 0, −10; 0.0 (the row's eye, corrected in [§4.6](#s4-6)) | – | The former eye (392.73, 1.8 m above the terrain) lies 0.42 m below the top of a placed platform: object 0x4700049C (SED 0x80214883 #12 at (98.45, 43.47, 388.84), render mesh 0x060008DC), whose level top is at 393.15 over x 88–110, y 33–55 and which has a second, lower upward face at 392.40. The bonfire stands on that top. From inside the platform the top's back faces are culled, so the camera saw the logs from below and, under them, the downward cone of the bonfire's mesh (next paragraph). | The bonfire on the platform 6 m ahead, below the image centre: logs, embers bright with the emissive texture, the stone ring; no cone. |
| W10 | the row's eye (unchanged) | – | Not a camera problem: the row's expectation was wrong ([§4.8](#s4-8), W10, corrected). | – |

**L3: the pyramid is part of the bonfire's mesh.** The bonfire's render mesh 0x06004C8A (one
submesh, surface 0x310045AE) holds an ash mound and, under it, a cone of eight triangles whose
normals face down and out: its rim lies at the object's origin height (1.3–1.9 m from the
axis) and its apex 1.13 m below the origin (1.47 m at the placement's scale 1.3). It is the
mesh's real shape, meant to sit below the surface the fire stands on (Inferred). At SED #133 every cone
vertex lies 0.12–1.66 m below the platform's top, so from any eye above the platform the cone
is hidden; it shows only from an eye inside or under the platform, as the former L3 eye was.
Of the 3,073 placements of object 0x47005179, 2,959 lie inside their landblock's square; at 612
of these some cone vertex is more than 0.02 m above the terrain under it (most of them stand on
placed floors, as #133 does; not checked one by one).

<a id="s4-8"></a>
### 4.8 Non-shader water and water animation (spec writer, 2026-10-09)

Added for the owner's report of 2026-10-09 (waterfalls, "water texture" surfaces and puddles
drawn as a white texture; the shader water perhaps moving too fast). Facts from
[`water.md`](../specs/water.md) §8.5–§8.7 (rows W23–W34) and
[`material.md`](../specs/material.md) §8.4, §10.3, §11.6; records decoded on the owner's
install on 2026-10-09 (OBS2 of `water.md`). Eyes are block-local; free-fly camera, avatar
hidden, *t* = 0.5, *n* = 12,240, *D* = 2 ([§4.0](#s4-0)). Each row's camera was framed with
Tessera's walker (`--screenshot`, images private), which on that date showed the defects
named under "Not expected". The motion of W10–W12 is judged on a few seconds of running clock
(`--time-rate 1`), not on one image; W15 and W16 are judged by the measurements W15-R and W16-R
in their rows (revised 2026-10-10); speeds are on the ground (`water.md` §8.7).

| Row | Region, landblock; `/loc` of the eye | Eye x, y, z; heading, pitch | What is in view (DIDs) | Expected | Not expected |
|---|---|---|---|---|---|
| W10 | 1, 0x4D7F; `r1 lx621 ly1019 ox105.30 oy60.00 oz473.00` | 105.30, 60.00, 473.00; 0, +28 | **Tall waterfall** of three sheets at (105.3, 151.6, 468.9), about 62 m high: SED 0x80214D7F #3 outer (object 0x47002305, surfaces 0x310004E8 and 0x310004E6), #4 fill (0x47002168, surface 0x310004E7), #5 inner (0x47002169, surface 0x310004E6), all of record 0x2B0007A1 (lit, two layers multiplied); splash entity #0 (0x47002306) at the foot; landblock water below at *h* 470.588 (row W15) | Grey-white streaked sheets whose texture **streams continuously** along the fall at 0.4 repeats per second (outer sheet's second layer 0.6), with a slow sideways drift (0.15 and 0.3 repeats per second on the inner and outer sheets, a noise wobble on the fill) and a slow breathing of the horizontal tiling (period 4π s ≈ 12.6 s); the inner sheet half transparent (material alpha 0.51) and the outer one at 0.76, so the rock shows through them **only faintly** (revised 2026-10-10 from the review of 9d1d976): where the fill covers, it blends at about 0.9 × 0.86 ≈ 0.77 (its texture alpha mean 0.90 times the mask's middle 0.86, `material.md` §11.3), so together with the inner sheet the rock keeps only about a sixth of its brightness, and through the middle of the fall it may not be distinguishable; lit by the scene like any lit surface (`water.md` §8.5.3). **Outline from this eye: sharp at the sides and the top** (corrected 2026-10-10 from the data, CAM of [§1](#s1); the former text expected edges that dissolve softly). The soft-edged mask 0x40000C5D (second layer of 0x310004E6 and 0x310004E7) fades only over the sheets' texture margins, and from this eye those margins are out of sight. At the sides the three sheets curl back into the gorge, whose terrain hides each sheet from its side edge up to 0.05–0.25 of its width in texture *u*: the first visible point lies at image *x* about 650–693 on the left and 926–948 on the right (image rows 400–800), where the mask's alpha is already 0.6–1.0 (on the fill's right side above row 480 only 0.13–0.23, so the fill's alpha test at 0.3 removes it there and the inner sheet shows). At the top each sheet's crest (its highest vertex row, *v* 0.148, about 62 m above the foot) lies where the mask is opaque; the mask's top fade covers the lip that runs 8–16 m back from the crest to the cliff, behind the crest from this eye; the foot, where the mask fades over the lowest 5 m, lies below the image. So the sides end at the rock's edge and the top at the crest. The outer object's strips (0x310004E8) carry no edge mask: their second texture 0x40000C5F is a streak mask whose column means of alpha drop below 0.1 from about 0.45 of each repeat across and are 0 from about 0.8 (corrected 2026-10-10; was "falls to 0 over the last tenth"), each strip spans 0.9 repeats across, and their mean opacity is about 0.02 (at most about 0.7 in thin streaks): faint streaks out to 28 m either side of the axis | one static pale texture; a uniform white sheet; no rock seen through the inner sheet where it shows alone (the right side above image row 480, where the fill's alpha test removes the fill) |
| W11 | 1, 0x9832; `r1 lx1217 ly402 ox28.00 oy55.00 oz410.00` | 28.00, 55.00, 410.00; 0, +14 | **Stepped cascade** in a gorge, y 108–157, z 404–436: falls 0x47004B3F (#139, #141, #142, #149; surface 0x31003E4C of record 0x2B000817, layers **added**), opaque foam 0x47004BE2 (#138, #140, #144, #151; 0x31003E4E, record 0x2B0006F2, added, unlit), foam 0x47004B4A (#130; 0x31003E51, record 0x2B00078C, added, unlit, × vertex colour), splashes 0x47004B49 (#128, #137; 0x3100770D); pool below with landblock water *h* 403.922 (row W16) | Bright white falls (two textures added, so brighter than either alone) **streaming down** (layer 1 V at −0.1 repeats per second, layer 2 U at −0.15 and early V at −0.45) with churning foam at the foot (two foam layers drifting sideways by noise and down at 0.37 and 0.34 repeats per second); soft, translucent edges from the alpha product; foam visible through the mist | static white shapes; foam not moving; edges cut hard |
| W12 | 1, 0x61A5; `r1 lx778 ly1322 ox58.00 oy52.00 oz385.00 h43.0` | 58.00, 52.00, 385.00; 43, −35 | **Fountain basin water** under a ceiling: fountain 0x470078C0 (SED 0x802161A5 #150) with water plane 0x47006F16 (#142) at (65.98, 61.06, 377.37), surface 0x31006748 of the plain water record 0x2B0000A8 (`water.md` §8.5.2); a second fountain at x 36.3 (#151, #155) | The basin floor seen **through rippled water**: refraction of the frame copy distorted by three normal layers (normal map 0x40000DDE, scales 4, 8, 8; drifting 0.36, 0.50 and 0.32 m/s), tinted strongly toward (0.463, 0.678, 0.89) (fog amount 1.2), a broad soft highlight (power 3, colour (0.373, 0.482, 0.455)); the plane's ripples continue across the two basins because *B* is a world position | the near-white marbled placeholder 0x40000DDF lit like a solid surface (what the walker drew on 2026-10-09) |
| W13 | 1, 0x7574; `r1 lx941 ly928 ox105.80 oy15.00 oz390.00` | 105.80, 15.00, 390.00; 0, −25 | **Puddle**: SED 0x80217574 #170, object 0x470015E4 (setup 0x040011FE) at (105.77, 30.01, 381.46), scale 8.7, 0.23 m above the ground, surface 0x31000A2A of the mirror record 0x2B0000AA, diffuse 0x400017ED (pale, alpha = the puddle's shape); nine more puddles in the block | A **partly transparent patch** whose outline **follows the ground** on most sides: 80 % of this large quad lies below the terrain (up to 2.6 m; `water.md` W44), so the depth test hides it there, in the original too (Inferred, `water.md` §8.5.4, answer of 2026-10-10); the texture's rounded soft edge shows only where the quad's own edge lies above the ground. Its opacity is *a* × the shape alpha (material alpha 1), with *a* = (1 − sin δ)^1.33 **per pixel** (`water.md` §8.5.4 step 4; δ the angle below the horizon of the line of sight to that point; the normal map, nearly flat, moves *a* by less than 0.03 here), so it is not one value over the patch (corrected 2026-10-10, PUD of [§1](#s1); was "about 0.40", the value at the quad's centre, which lies below the ground): the part that shows is the quad's far side, 15–25 m from the eye at δ 20°–34°, and there *a* rises from about 0.33 at its nearest visible points to about 0.53 at the far edge (median 0.42); the shape alpha is 1 over 82 % of that part and falls to 0 only at the texture's rounded edge. **Measuring it** (W13-R): Tessera blends decoded (linear) values on its sRGB target (`render.md` §4.8 OPEN), so an opacity estimated from the ground's contrast in stored 8-bit values reads higher than *a* wherever the puddle is lighter than the ground (for example *a* = 0.42 over ground values 30–50 / 255 under a puddle colour of 110 / 255 reads as about 0.73); estimate it on decoded values, where the ground's contrast with the puddle divided by its contrast without it is 1 − *a* (within the diffuse's colour variation). An estimate of about 0.3–0.7 from stored values, as on 2026-10-10, is consistent with *a* = 0.33–0.53. Its colour is the reflection × the pale diffuse (with Tessera's Decision: the fog colour × the diffuse, W14), so the ground shows through; more mirror-like when seen at a grazing angle. Revised 2026-10-10: the former expectation (the texture's soft outline all round) did not allow for the terrain rising above this quad; a puddle that lies above the ground is row W14 | an opaque white polygon (what the walker drew on 2026-10-09); the quad drawn through the ground, lifted above it or projected onto it |
| W14 | 1, 0xE243; `r1 lx1813 ly539 ox100.50 oy78.00 oz318.50 h180.0` | 100.50, 78.00, 318.50; 180, −20 | **Puddles** in a mine yard: SED 0x8021E243 #181, object 0x470015E4 at (100.52, 63.84, 314.06), scale 5.0, surface 0x31000A2A; nine more puddles of the same object in the block | As W13 for the depth state and the per-pixel opacity; the puddle #181 lies almost wholly above the ground (202 of 6,561 sample points of its quad below it, PUD), so the texture's **soft outline** shows. Opacity *a* × the shape alpha with *a* from about 0.52 on the near side of #181 (10 m away, 26° below the horizon) to about 0.69 on the far side (20 m, 13°), median 0.63 where the shape is opaque (PUD; corrected 2026-10-10, was "about 0.62" for the centre alone). **Colour, under Tessera's Decision** (no reflection pass, `water.md` §8.5.4 Tessera rule): the reflection is replaced by the frame's fog colour *F* (white material colour), so the puddle's own colour *C* is *F* × the pale diffuse (mean (187, 182, 167) / 255), moved toward *F* by the fog factor, and it is blended over the ground at *a* × the shape alpha: patches in the fog's hue at a little under its brightness, **lighter than dark mud** and blue-grey under a blue-grey fog. Corrected 2026-10-10: the former "dark reflection-tinted patches" followed from no spec statement; the blue-grey patches of about 60–75 / 255 measured on 2026-10-10 are what this rule gives. **Check W14-R:** on decoded (linear) values, each pixel inside the shape ≈ *a*·*C* + (1 − *a*)·*G*, with *G* the same pixel without the puddle (blending on decoded values, `render.md` §4.8 OPEN). The original shows the reflected scene × the diffuse instead of *F*, which may be darker or lighter (owner item O24) | glowing white blobs; patches darker than the mud around them (the Tessera rule has no dark reflection to give them); one uniform opacity over the whole patch |
| W15 | 1, 0x4D7F; same eye as W10 | 105.30, 60.00, 473.00; 0, −12 | **Flow-map landblock water**: type 37, level 120 → *h* 470.588, surface 0x31004359 of record 0x2B0007FD (`water.md` §8.6): flow rates (1, 1) per second, layer 0 scale 10; layer 1 scale 2, speed (0.0015, 0.002); layer 2 scale 8, speed (−0.01, −0.0125) | **Judged by measurement, not on a still** (revised 2026-10-10: one image cannot show a speed). **Check W15-R:** for the water surface drawn at this camera (0x31004359), the normal-layer offsets that the walker computes for two material-clock times 1 s apart differ by the layer speeds, layer 1 by (0.0015, 0.002) repeats and layer 2 by (−0.01, −0.0125) repeats: 0.20 m/s and 0.32 m/s on the ground at scales 2 and 8 (`water.md` §8.7: \|speed\| × 160 / scale m/s), in opposite directions, within 1 %; layer 0 has no scroll rate of its own (it follows the flow phases, which this install's neutral flow map leaves without visible motion, `water.md` §8.6.3); the clock is real seconds; and every water draw at this camera whose layers have a rate in `water.md` §8.7 advances their offsets with the time input every frame (§8.7, "Time base"), none being drawn with a fixed offset. On the image, only the look: ripples that seem to stir rather than slide, refraction of the lake bed, Fresnel reflection toward the far side | layer 0 offset by 1 repeat per second of a 16 m tile (ripples sliding at about 22.6 m/s, the drawing of 2026-10-09); offsets that advance with frames or game time instead of seconds; an animated layer drawn with a fixed offset |
| W16 | 1, 0x9832; `r1 lx1218 ly404 ox50.00 oy80.00 oz407.50 h271.0` | 50.00, 80.00, 407.50; 271, −10 | **Plain landblock water**: type 1, level 103 → *h* 403.922, surface 0x3100068B of record 0x2B0000A8 (`water.md` §8.3); the pool of W11 | **Judged by measurement, not on a still** (revised 2026-10-10). **Check W16-R:** for the water surface drawn at this camera (0x3100068B), the three normal-layer offsets that the walker computes for two material-clock times 1 s apart differ by (0.005, 0.0075), (0.015, 0.02) and (−0.01, −0.0125) repeats: 0.36, 0.50 and 0.16 m/s on the ground at scales 4, 8 and 16 (`water.md` §8.3, §8.7), the third against the first two, within 1 %; the clock is real seconds; and every water draw at this camera whose layers have a rate in `water.md` §8.7 advances their offsets with the time input every frame (§8.7, "Time base"), none being drawn with a fixed offset. This row is the reference that the plain water family's rates are right when the time input is in seconds. On the image, only the look of plain water (ripples, refraction, reflection) | ripples moving tens of metres per second (a time input not in seconds); an animated layer drawn with a fixed offset |
| W17 | 5, 0x6758; `r5 lx827 ly707 ox70.00 oy65.00 oz396.00 h64.5` | 70.00, 65.00, 396.00; 64.5, −8 | **River** with the second flow-map record: type 17, level 100 → *h* 392.157, surface 0x310066D5 of record 0x2B0005EC (flow rates (1, 1), layer 0 scale 10, no layers 1 and 2) | An **almost flat** water surface (the flow map 0x400163D3 is neutral, so the flow normal is straight up, `water.md` §8.6.3): the river bed refracted without moving ripples, reflection rising toward grazing angles, fading out with the far-draw fade. Tessera without a reflection pass (`water.md` §8.6.4 Tessera rule, revised 2026-10-10: the record's own colour list, Fresnel bias 0.01, exponent 2): the bed seen through the water everywhere, darkened toward the record's `WaterFogColor` (0.129, 0.149, 0.133) where the view is steep and farther than 5–15 m, and the untinted bed toward grazing angles; no near-black band | ripples streaming at about 22.6 m/s in one direction (the drawing of 2026-10-09); owner item: whether the original shows moving ripples here settles the OPEN of `water.md` §8.6.3 |

Levels: W15 level 120 and W17 level 100 follow from *h* = level × 1000 / 255 (`water.md` §6.2).

> **OPEN (spec writer, 2026-10-09, for the walker implementer):** in the framing check, a
> `--place` line together with `--fly` put the eye 30 m above the line's height (the HUD showed
> `oz` + 30); the `/loc` lines above give the eye itself. Whether the offset is intended (a
> fly-start height) is the walker's to say (`walker.md` §1).

---

<a id="s5"></a>
## 5. M3 reference locations under the revised height rule

All 20 rows of [`m3-reference-locations.md`](m3-reference-locations.md#s2-2) were recomputed
with `landblock.md` §3.4 (stored × 2,000 / 65,535, single precision) and §3.11 (triangulated
height): every terrain height at the stand point and at the corner stays within the 0.05 m
tolerance of its listed value. The largest change is +0.030 m (M3-13 stand point: 217.690
against 217.66; the bilinear value under the new rule is 217.662); every other row moves by
−0.002 to +0.014 m. No erratum is needed; that file carries a one-line note.

---

<a id="s6"></a>
## 6. Procedure

<a id="s6-1"></a>
### 6.1 Walker side (implementer)

1. Criterion 4: a real-data test runs every walk of [§2.3](#s2-3) through `--walk-path`
   (table form) and checks [§2.2](#s2-2); then the soak of [§2.5](#s2-5). Record pass/fail
   per walk and the soak counts in the ROADMAP status table.
2. Criterion 5: run [§3](#s3) on the reference machine (`docs/perf/reference-machine.md`).
3. Criterion 6: check the "expected" facts of [§4](#s4) with real-data tests (layer
   stacks, generated lists, frill counts, water heights, blended sky values, lights and
   material flags); view each row once in the walker and keep the screenshots private.

<a id="s6-2"></a>
### 6.2 Original-client side (owner)

Items [O18–O27](owner-observations.md#o18): measurements of movement, the outcome of the
scripted walks in the original, and a side-by-side look at the visual rows. No
screenshots in the repository.

<a id="s7"></a>
## 7. Open points

> **OPEN:** Whether the original's collision answers match SWEEP where its controller is
> unknown (`collision.md` Q9): stop distances at walls, how far a steep slope is climbed
> before sliding, the exact wade-to-swim switch. Owner items O18 and O19 measure them.

> **OPEN:** The day type of the original on a given day (`environment.md` §11.2). Rows
> Y1–Y4 assume *D* = 2; owner item O26 records what the original shows.

> **Closed 2026-10-09 (was OPEN, implementer, 2026-10-08, row L7, for the spec writer):** L7 cannot show what it
> states. Its light map 0x4116792D is black (`terrain-texturing.md` §10: largest channel
> 0.010 over all texels), and the block's sky (key 0x10000030 by the walker's lookup,
> day description 0x1A00022B) gives the same lighting at *t* 0 and 0.5, with a sun of
> brightness 0.25, so "at night … the only light other than the ambient" does not hold
> either ([§4.7](#s4-7)). The row needs a block whose light map has non-zero texels and
> whose day description has a night, or it can be dropped in favour of L2, whose camera of
> §4.7 shows the light-map pools of block 0x8FF3 (map 0x41221577) on the ground with the
> point lights switched off.
>
> **Answered 2026-10-09 (spec writer, CHK):** both findings are confirmed (0x4116792D: largest
> channel 0.010; sky key 0x10000030 → 0x1A00022B without a night). L7 is redefined in
> [§4.6](#s4-6) on block 0x7873 of region 1: its light map 0x41102099 has a strong warm pool
> round a campfire whose light carries the baked flag, and its sky key 0x10000070 gives
> 0x1A000000 with *D* = 2, whose keyframe at *t* 0.0 is a night (the values of row Y4). The
> row is kept rather than dropped for L2: L2's day description 0x1A000377 is another sky, and
> L7 now checks the map alone on the terrain under a night sky, with the blue channel as a
> control. Chosen among the 3,536 light-mapped blocks with a day description under *D* = 2:
> 1,087 have a night (light at *t* 0 at most half that at *t* 0.5), 306 of them a map with
> 0.5–8 % of its texels above 0.15 and a peak of at least 0.4.

> **OPEN:** The interior doorway rows cross portals between cells only, because no cell of
> this install adjoins the landscape ([§1.1](#s1-1) item 1). If a later install adds
> walk-in interiors, add a row that enters one from outdoors (`collision.md` §5.8.1).

<a id="s8"></a>
## 8. Change log

| Date | Change |
|---|---|
| 2026-10-06 | First version: walks, performance spots, visual rows, M3 reference locations. |
| 2026-10-07 | **Generated objects re-derived** (spec writer; `scenery.md` footprint revision). The GEN generator of [§1](#s1) had the footprint test of `scenery.md` §7.4 step 14b inverted: it tested only open markers and skipped closed ones. A BIN re-check confirmed step 14b as written. With the corrected generator, the static-pass lists of the 11 landblocks of [§4.2](#s4-2) equal the clean-room implementation's object for object. **§4.2:** every row re-derived with the same eye, heading and cone. Totals: S1 36 → 25, S2 344 → 315, S3 173 → 165, S4 364 → 309, S5 252 → 151, S6 176 → 111, S7 471 → 445, S8 361 → 166, S9 362 → 221, S10 102 → 80, S11 201 → 186. In view: S1 11 → 6, S2 41 → 35, S4 19 → 16, S5 29 → 9, S6 17 → 6, S8 34 → 14, S9 35 → 18, S10 29 → 22 (S3, S7, S11 unchanged). New nearest objects: S1 (both; the second is GN1's tree), S5 (second), S6 (first becomes the former second), S8 (first becomes the former second), S10 (first becomes the former second); S2–S4, S7, S9 and S11 keep both. The OPEN under §4.2 is answered. **§2.3–§2.4:** GN1's former tree (0x470035F3, list index 27) is not placed by the client. GN1 now runs north into tree 0x470035F4 of the same landblock (list index 16), from (130.61, 77.52) to the stop point (130.61, 82.61, 559.19). The terrain on the way slopes 30–34° across the path, below the 45° walkable limit. No object with collision volumes lies within 3 m of the path, and the first contact is the tree. GN2–GN4 keep their object, start and stop point; their list indices change: 40 → 33, 283 → 176, 149 → 138. The GN1 item of the implementer's OPEN in §2.4 is answered. **Unchanged** (checked with the corrected generator): the first contact of every other outdoor walk (WL, UB, ST, SL, CV, WA, FR) and the count of generated objects within 3 m of their paths; the frill records of the six landblocks of [§4.3](#s4-3), identical record for record under the frill rule of `scenery.md` §8.2 item 5 as revised; hence rows F1–F6 and walks FR1–FR3. |
| 2026-10-08 | **Pass conditions revised from `collision.md` §11.7** (implementer; for the spec writer to confirm). §2.2 `blocked` is measured at the first touch of a blocking object, and the "never past P + d" condition applies up to it; after it the walker may slide along or round the blocker, which the original does too while forward is held (`collision.md` §11.7 items 2–3, "Consequences for walk checks"). New slope allowance *r* · (sec *θ* − 1) for the feet above the terrain in `follow` and `pass` and above P in `blocked` ("Feet above the terrain on a slope"). WL2, GN1, GN3, GN4, CV1 and FR1 now pass (30 of 32). New implementer OPENs in §2.4: GN2's P past the first touch of the trunk's steep foot; CV2's path not walkable. |
| 2026-10-08 | **GN1, GN2 and CV2 re-derived** (implementer; for the spec writer to confirm; answers both implementer OPENs of §2.4). New provenance tags TOUCH (stop points under the first-touch rule of `collision.md` §11.7, computed with Tessera's own collision queries) and CAVE (search for a cave path on walkable floor) in [§1](#s1); §2.2 `blocked` says which rows use which. GN2: P 40.97,46.19,192.12 → 40.97,45.98,192.12 (first touch on the trunk's steep foot, 0.114 m above the feet; the step-up of §11.5 fails). GN1: P 130.61,82.61,559.19 → 130.61,82.45,559.18 (the same cause, 0.28 m above the feet). GN3, GN4, WL1–WL5 and UB1–UB3: P unchanged (TOUCH within 0.005 m). CV2: new path in the same cave, start 68.99,35.50,371.59, heading 300°, to E 56.00,43.00,364.14, headroom 4.11 (was 18.75,101.57,369.92 to 31.74,94.07,370.97 along a 48–61° bank); "Caves" in §2.4 updated. All 32 walks pass. |
| 2026-10-08 | **Cameras for criterion 6** (implementer; for the spec writer to confirm). New [§4.7](#s4-7): cameras, comparison images and judgements for the nine rows that could not be passed on the image on 2026-10-07. T4, T5, T12 and Y2 get new cameras (no object between eye and target; Y2 faces south); T15 and T16 keep theirs, with an image without the sky's lighting showing the ceiling layers and the brightness matching the downward ambient; L2 looks down on the ground under light #460, with and without the point lights. **L5 revised** (eye 54.98, 57.63, 195.02; 195, −20 → 64.59, 53.19, 196.00; 195, −25; "a broad highlight" → "a faint white sheen", `terrain-texturing.md` §9.4): from the former eye the sun's mirror point lay in a building's shadow, and §9.4 gives a sheen, not a broad highlight. **L7** not judgeable as defined (black light map, no night in the block's day description): OPEN in [§7](#s7). |
| 2026-10-09 | **Spec-writer check of the rows of 2026-10-08** (new tag CHK in [§1](#s1)). **Confirmed:** the TOUCH and CAVE derivations and the §2.2 revision (first touch, slope allowance; `collision.md` §11.7); GN1 P 130.61,82.45,559.18 and GN2 P 40.97,45.98,192.12 (independent probe within 0.002 m; GN3 and GN4 unchanged); CV2's new path (35.55° steepest within the capsule radius, headroom 4.115 m, fall 7.45 m, the same 176-quad cave, no object within 6 m). The implementer OPENs of §2.4 (the first-run OPEN, GN2, CV2) are closed. **Corrected:** L5. Its revised expectation (a faint sheen, `terrain-texturing.md` §9.4) is right, but in 0x7FBD the mirror points of both versions and T6's target lie under a drawn platform (SED 0x80237FBD #115, top 0.30 m above the terrain); L5 moves to open paving of key 67 in 0xD7E8 (eye 38.45, 28.34, 169.78; mirror point 36.67, 21.68), and T6 to the same spot (target 36.67, 21.67, 165.80, mask 0x41166A06). The §4.7 judgement of L5 is void. **Redefined:** L7, on block 0x7873 (map 0x41102099, a campfire pool; sky 0x1A000000 at *t* 0.0, a night); the §7 OPEN is closed. To be re-judged on the image: T6, L5, L7. |
| 2026-10-09 | **CV1–CV3 durations confirmed** (spec writer, new tag CHK2 in [§1](#s1)). The implementer's change from 4 s to 8 s (walk mode at 2.5 m/s, `collision.md` §11.8) is right for all three rows: the 15 m paths need 6.10 s, 6.02 s and 6.09 s under the original's walking speed, acceleration and uphill slowdown; the note in [§2.3](#s2-3) is closed. ST and IN rows checked to still fit. |
| 2026-10-09 | **Non-shader water and water animation** (spec writer; owner report of 2026-10-09). New [§4.8](#s4-8), rows W10–W17: two waterfalls (W10 two-layer sheets of 0x2B0007A1, W11 a stepped cascade of 0x2B000817 with foam), a fountain basin (W12, a placed plain-water plane), two puddles (W13, W14, the mirror record 0x2B0000AA), and three landblock-water spots for the rates (W15 flow-map 0x2B0007FD, W16 plain 0x2B0000A8, W17 flow-map 0x2B0005EC), each with what Tessera drew on that date. Row W8 corrected (its surface is a plain water record surface, `water.md` §8.5.2). OPEN on the walker's fly-start height. |
| 2026-10-09 | **Lava look** (spec writer, third water revision answering the `water.md` review of 2026-10-09): row W7 of [§4.4](#s4-4) states the expected look of landblock lava from `water.md` §8.6.5 (lava record 0x2B000810: opaque, unlit, emissive pattern, static in this install) and asks the owner whether the original's lava moves. No other row changed; awaiting review. |
| 2026-10-10 | **Puddle and river rows** (spec writer, with the `water.md` revision of the same day). W13: the puddle at this camera lies 80 % below the terrain (`water.md` W44), so its outline follows the ground where the terrain rises above it, as Tessera draws it and, Inferred, as the original does; the soft alpha edge shows only where the quad lies above the ground; opacity 0.40 (was 0.41, the review's non-blocking note). W17: Tessera's look under the revised §8.6.4 Tessera rule added beside the original's. Owner item O24 of `owner-observations.md` now covers W10–W17. |
| 2026-10-10 | **Cameras and rows after the re-judgement on main `01deafd`** (spec writer, new tag CAM in [§1](#s1)). New [§4.7.1](#s4-7-1): image cameras for T1 (turned round: a placed roof stood between eye and target), Y3 (facing south, as Y2), W2 (12 m out over the sea, past the frill reeds), S5 (5 m up, 15 m back: placed bushes surround the eye) and S7 (6 m up: a placed rock, not a hump of ground, lies in front); the expected values of these rows are unchanged. S5's second and S7's first nearest object cannot be seen from any eye (inside placed bushes; inside the placed rock, 0.73 m below its top) and are judged by the data test only. **L3:** the eye was 0.42 m below the top of a placed platform; corrected to 394.95 (1.8 m above the platform). The upside-down pyramid seen under the logs is the real shape of the bonfire's mesh (a downward cone under its ash mound, apex 1.47 m below the origin at scale 1.3), hidden below the platform's top from any eye above it. **W10:** the expectation of soft dissolving edges was wrong for this eye: the gorge's terrain hides the sheets' side margins, where the mask fades, the top is each sheet's crest, where the mask is opaque, and 0x310004E8 has no edge mask (a faint streak mask, mean opacity about 0.02); `water.md` §8.5.3 is unchanged (its statement about 0x40000C5D holds in texture space). **W15, W16:** judged by the measurements W15-R and W16-R (offsets of the normal layers 1 s of material clock apart, against the speeds and ground rates of `water.md` §8.7; no still-scrolling draw), not by a still. Awaiting review. |
| 2026-10-10 | **Sky line, L3, generated figures and review notes** (spec writer, tag CAM2 in [§1](#s1)). **W9:** the thin line about 0.66° below the horizon is the sky dome's equator, where its wrapped texture mixes in the pole colour. It is accepted by the Tessera Decision of `environment.md` §12, and owner item O26 asks about it. **L3:** the implementer OPEN of commit d82be77 is closed. It agrees with the corrected eye 394.95 of [§4.7.1](#s4-7-1) (floor readings 393.15 and about 393.17). **L5:** a new note gives the expected look of the generated fallen soldiers of 3, 0xD7E7: lying in dead idles, cloth in the colours of their random dye lists, never the magenta placeholder (`appearance.md` §7.4); owner item O92. **W10:** the rock shows only faintly through the sheets; the "not expected" item now names the inner sheet's lone area; the streak-mask fade is corrected. **W15, W16:** the checks cite the time-base rule of `water.md` §8.7 instead of a walker diagnostic. **W2:** owner item O91 for the dark far sea. Awaiting review. |
| 2026-10-10 | **Puddle opacity and colour** (spec writer, fourth change of the day, tag PUD in [§1](#s1)), after the owner notes of the re-judgement of 2026-10-10. **W13:** the opacity *a* of `water.md` §8.5.4 step 4 is per pixel; over the part of the quad that shows (its far side, 20°–34° below the horizon) it runs from about 0.33 to 0.53, not one value of 0.40 (that was the centre, which lies below the ground); new measuring note W13-R: Tessera blends decoded values (`render.md` §4.8 OPEN), so a contrast estimate in stored values reads higher than *a* (the 0.3–0.7 of 2026-10-10 agrees with 0.33–0.53). **W14:** *a* from about 0.52 to 0.69 over #181; colour under Tessera's Decision is the fog colour × the pale diffuse, so the patches are lighter than the mud and take the fog's hue (the measured blue-grey 60–75 / 255 is expected); the former "dark reflection-tinted patches" had no spec basis and is removed; new check W14-R; "not expected" adds dark patches and a uniform opacity. `water.md` is unchanged. Awaiting review. |
