<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Water: water tables, fog-zone tables, landblock water, water volumes and drawing water

| | |
|---|---|
| **Status** | **Revised** (2026-10-10, spec writer: fourth revision of the day answering item B1 of the review of the third revision: the two counts of row W54 were swapped, now 191 records satisfy both W52 rules and 319 neither; no other fact changed; awaiting review; see §15). Before: **Revised** (2026-10-10, spec writer: answers to the two OPENs of §14 left by the second revision: which draws of a placed entity apply `Render_DepthBias_Type` (now traced, `decals.md` §6.5; tag BIN6, row W53), and which W52 rule decides when rules (i) and (ii) disagree (census of all 510 shader records, tag OBS5, row W54); matching sentences in §8.5.4 and §8.5.5; see §15; review 2026-10-10: **changes requested**, item B1 of the third revision in the review record; awaiting revision). Before: **Accepted** (2026-10-10, review of the second revision of the day passed CLEAN_ROOM.md §5.3, every item; see the review record). Before: **Revised** (2026-10-10, spec writer: answers to the second implementer OPEN of 2026-10-10 in §14 (items (a)–(d): lookup order and reach of `Render_DepthBias_Type`, an interface rule for 0x2B000442 from decoded data, the alpha test of 0x2B0007D0), with the matching sentences of §8.5.4 and §8.5.5; new tags OBS4, BIN5 and SH4, rows W49–W52, invariant WA19; awaiting review; see §15). Before: **Accepted** (2026-10-10, review of the revision answering the implementer OPENs passed CLEAN_ROOM.md §5.3, every item; see the review record). Before: **Revised** (2026-10-10, spec writer: answers to the implementer OPENs of 2026-10-09 and 2026-10-10 in §8.5.4 (puddles cut by the ground, checklist row W13), §8.5.5 (0x2B00063E), §8.6.4 (colour of 0x2B0005EC without a depth texture, checklist row W17) and §14 (a)–(f), and to the non-blocking notes (1) and (2) of the last re-review (W39 count, the `w[5]` term of 1182 and 1186, the §8.5.3 mode-4 wording); new tags SH3 and OBS3, rows W42–W48, invariants WA17–WA18; awaiting review; see §15). Before: **Accepted** (2026-10-09, re-review of the fourth water-feature revision passed CLEAN_ROOM.md §5.3, every item; see the review record). Before: **Revised** (2026-10-09, spec writer: non-shader water and animation rates, see §15; second revision the same day, checked against Tessera's own whole-world fallback probe: tags TP and BIN4, rows W35–W39, §8.3 how the client fills *O*, §8.5.1 drawn-world counts and liquid objects, §8.5.3 four more fall surfaces, §8.5.4 the six static mirrors, new §8.7.1–§8.7.2, WA12–WA15; review 2026-10-09: **changes requested**, items B1–B2 in the review record; third revision the same day answering B1–B2 and the three non-blocking notes: the lava record 0x2B000810 is its own family with its formula traced, new §8.6.5, rows W40–W41, WA16; re-review 2026-10-09: **changes requested**, item B1 of the third revision in the review record; fourth revision the same day answering that B1 and non-blocking note (1): W37, the §8.7.1 rule and census and the §8.7 rate table cover the lava family (1186, 1187 on 0x31006FAE) and the older bump water, §8.6.5 step 6 and a new texture-matrix paragraph; awaiting re-review). Before: **Accepted** (2026-10-09, re-review of the revision answering B1–B2: passed CLEAN_ROOM.md §5.3, every item; see the review record). Before: Revised (2026-10-09, spec writer: revision answering the review of 2026-10-09, B1 (§7.4 Tessera rule lists four differences) and B2 (§7.3 answer reworded to agree with W22), and the non-blocking §14 note). Before: Revised (2026-10-09, spec writer: §7.4 merged OPEN answered by BIN3 (j), new W22, W19 reworded, the no-support answer corrected, step 3 sign of *v* made explicit; review 2026-10-09: **changes requested**, items B1–B2 in the review record). Before: Accepted (2026-10-08; two parallel revisions of that date, each re-reviewed and accepted: the volume-flag, element and monitor answers, rows W18–W20, and the §7.4 revision on entering deep water from BIN2, row W21 (W18 in that revision); merged 2026-10-09 with one disagreement between them recorded as an OPEN in §7.4). First accepted 2026-10-06. |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review sessions (spec reviewer, fresh sessions; separate from the writer's session) |
| **Review record** | Second re-review of the 2026-10-06 edit to §7.4, 2026-10-06: **passed**, stays Accepted. Re-review of the 2026-10-08 revision (rows W18–W20), 2026-10-08 (fresh reviewer session): **Accepted** (CLEAN_ROOM.md §5.3, every item; W19 spot-checked in the binary). Re-review 2026-10-08 of the parallel §7.4 revision (BIN2, row W21, then W18) (Claude review session, spec-reviewer role, fresh session): **passed**, Accepted; all CLEAN_ROOM.md §5.3 items hold. Spot check: steps 1–4 agree with `collision.md` §10.3 (Swimming row), §11.1 step 9, §11.6 items 3, 4, 7 and §12.4 (p), §12.5; the worked example recomputed (0.098, 0.037, 0.013, 0.003 m, line in the fifth update at 60 per second; 0.19, 0.07, 0.02 m, fourth at 30). Non-blocking: in §7.4 step 3, *v* in 0.4 · (*v* + 20 *t*) is the signed vertical velocity (negative downward), not the "downward speed" the sentence names; the example fixes the sign. Merge of both revisions, 2026-10-09: no fact changed; the merged text has not been reviewed as a whole. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: review of the BIN3 revision (commit a654745, `git diff main -- docs/specs/water.md` on specs-9) against CLEAN_ROOM.md §5.3, **changes requested**. Items that hold: no code or pseudo-code (the update of *v*_z in §7.4 step 3 is a formula), no addresses, middleware class names, copied prose, game text, keys or hostnames; nothing under a closed gate; BIN3 has a provenance row with question, method and why M1–M3 were not enough; W19 and W22 are marked. Spot checks passed (read-only, against the cited sources): W19, W22 and BIN3 agree with `collision.md` §11.3.2 (a) and (b), C28, C49, C64 and BIN question 70 on this branch; §7.4 step 1 cites W22 consistently; the step 3 formula recomputed from Gravity −20 m/s² and damper 0.4 at *t* = 1/60 s and 15 m/s gives the example's 0.098, 0.037, 0.013, 0.003 m and the rise in the fifth update, and the earlier non-blocking note on the sign of *v* is resolved. The binary was not re-read in this review. Blocking: **B1** §7.4 Tessera rule says it differs from the client (after controller updates) "in three cases", but a fourth follows from the stated rules: in water deeper than the float height on support that is not walkable, Tessera's Decision ("swims (state 2) when the water surface is more than its float height above its feet"; `collision.md` §12.3 step 7) gives 2 where the client (W22, table of `collision.md` §11.3.2) gives 3. List all four cases (the matching `collision.md` §12.5 row "Water states" has the same gap). **B2** §7.3, "Answer (2026-10-08, BIN, W19)": "for the movement state the client reads only the mover's `Volume_IsLiquid`" contradicts W22 (after controller updates the in-water input also needs the float-height depth). Reword so it agrees with §7.4 (both routines read `Volume_IsLiquid`, so a coin volume still counts as water for the movement state; the controller's routine adds the depth test). Non-blocking: the §14 OPEN on containment and buoyancy cites only W19 for the answered state rule; add W22. Full records in the review log (decision 0018). Spec writer, 2026-10-09: revision answering B1, B2 and the non-blocking note (see the change log). Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: re-review of that revision (commit d145d39, `git diff main -- docs/specs/water.md` on specs-9) against CLEAN_ROOM.md §5.3, **Accepted** (every item). **B1** resolved: the §7.4 Tessera rule lists four differences after controller updates; checked case by case against the Tessera Decision (§7.4, `collision.md` §12.3 step 7) and the client table with the W22 in-water input (`collision.md` §11.3.2): shallow water gives 1/5, 3/6, 2/4 (Tessera/client) by support, deep water gives 2/1 and 2/3 on walkable and other support, and both give 2 without support in deep water, as stated; `collision.md` §12.5 row "Water states" lists the same four. **B2** resolved: the §7.3 coin-volume answer says both routines read `Volume_IsLiquid` and that the controller's routine adds the float-height test (W22), consistent with W19, W22 and §7.4. Non-blocking §14 note resolved (cites W19 and W22). No remaining text in §7.3, §7.4, §13 or §14 states the superseded "no depth input for the movement state" or "state 2 whatever the depth" without the position-update qualifier. No new facts, so no new provenance needed; no code or pseudo-code, no copied prose, addresses, middleware class names, game text, keys or hostnames; nothing under a closed gate. The binary was not re-read in this review (no new BIN claim). Non-blocking: in the narrow band where the depth lies between the float height minus 0.0002 m and the float height, the client is in water while Tessera wades; both give 1 or 3 with support and 2 without, so no fifth difference arises. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-09: review of the water-feature revision on branch `water-fix` (`git diff main...water-fix`, commits 88edc8f and 4be89e7) against CLEAN_ROOM.md §5.3, **changes requested**. Items that hold: every new statement is a functional fact (formulas, rates, counts, DIDs, parameter values); no code or pseudo-code (§8.5.4, §8.5.5 and §8.6.2 are numbered formulas, no program text, registers or listing order reproduced); the `c_…` parameter labels are marked as interoperability names; no game text, keys, hostnames or traffic; nothing under a closed gate; every new fact has a provenance row (W23–W39, tags OBS2, SH2, LIT, TP and BIN4, BIN4 with question, method and why M1–M3 were not enough) and a mark defined in the legend (Probable included); WA7–WA15 are testable; unknowns are OPEN (O reference, lava, bump-water pixel stage, splash source, permutation choice, run-time flow map). Spot checks (read-only, own probes on the install, independent of the writer's scratch): the OpenGL programs of 0x2B0005EC agree with §8.6.2 steps 1–6 (1/160 world scale, phases 1057 *T* and 1058 *T* + ½, noise offset ½ × the noise at ½ *B*, jump 0.7 per cycle, triangle weight 2 |½ − *f*₁|, *k* = 2 |*F*|²) and with its far-draw-fade alpha and the 100 m later fog start; the OpenGL programs of 0x2B0007FD agree with §8.6.4 steps 1–6 (bias multiplies, exponent inverted, undistorted reflection at +0.003, *L*꜀ and tint as stated, alpha 1) and have no layers 1 and 2; its Direct3D 10/11 programs, and not those of 0x2B0005EC, carry the constants 0.0004, 8000, 0.098, 2.5 and 0.005 of §8.6.2 step 7 and §8.6.4; the OpenGL programs of 0x2B0000AA agree with §8.5.4 (coordinates, fog without upper clamp, steps 1–5); W28: the flow map's one level is 32 × 32 DXT1 with texels (125, 127, 0) and (128, 126, 0) only, \|*F*\| ≤ 0.020; TP: `tessera-dattool fallback --all` reproduces 23 surfaces without a usable base texture and 98 / 9 / 16 water, mirror and liquid-distortion surface records on exactly the records of `material.md` §11.6. Recomputed: the rate tables of §8.7 and §8.7.2, the wet-landblock sums of §8.2 (89,913; 85,059), and the five worked examples of §12 (all agree). BIN4 (W38) and the Direct3D colour terms beyond the constants were not re-read. **Blocking:** **B1** record 0x2B000810 (lava, 656 wet landblocks) is treated inconsistently: §8.2 "Program families" puts it in the flow-map family "described in §8.6" and the answered callout there says a renderer gives each record the formula of its family (§8.6), routing rule 1 of §8.5 sends flow-map records to the water drawing, and WA7 counts it in that family; but §8.6 says it is not traced and its Tessera rule covers only 0x2B0007FD and 0x2B0005EC, and `material.md` §11.6 excludes 0x2B000810 from the water records ("falls outside"). State one status for it (family membership by interface only, drawing OPEN) and give a Tessera Decision for drawing landblock lava until it is traced, consistent with `material.md` §11.6. **B2** §8.6.3 OPEN asks the owner about "row W12 of `m4-5-visual.md` §4.8 (a 0x2B0005EC river)"; in that checklist W12 is the fountain basin and the 0x2B0005EC river is W17 (whose text points back to this OPEN). Non-blocking: (1) §8.7.1 census column "drawn placements (TP)" holds TP draw counts (3,409 = 2,067 + 712 + 630), not placements; (2) §12 "Curve-valued scale": the direction reverses every π seconds only approximately (the constant 0.005 term shifts the zero crossings); (3) the checklist rows W10–W17 share their labels with this spec's provenance rows W10–W17; references from other specs qualify them, but a bare "W12" in this spec reads as the provenance row. Spec writer, 2026-10-09: third revision answering B1, B2 and the three non-blocking notes (see the change log); awaiting re-review. Claude review session (spec reviewer, fresh session; neither the writer nor the reviewer of the earlier round), 2026-10-09: re-review of the third revision (commit 8c578bc; the whole branch read, `git diff main...water-fix`) against CLEAN_ROOM.md §5.3, **changes requested**. Items that hold: the earlier **B1** is resolved: 0x2B000810 has one status throughout (§8.2 family table and sampler row, the answered §8.2 callout, §8.5 routing paragraph, §8.6 intro, §8.6.3, §8.6.4 OPEN, §8.6.5, §8.7 table, §13, §14, WA11, WA16) and agrees with the `material.md` §11.6 lava row and MA53; the earlier **B2** is resolved (§8.6.3 cites checklist row W17); the three non-blocking notes are taken up. W40 and W41 have provenance (SH2 extended with question, method and why M1–M3 were not enough) and marks; §8.6.5 gives formulas and steps in words, with no program text, registers or listing; no game text, keys or hostnames; nothing under a closed gate. Spot checks (read-only, own probes): the Direct3D 10/11 programs of 0x2B000810, disassembled with a public library, have two water permutations with one identical pixel program; the vertex stage passes UV set 0 unchanged and gives *B* at 1/160, the phases 1057 *T* and 1058 *T* + ½ and the distance-fog factor; the pixel stage agrees with §8.6.5 steps 1–7 term by term (noise at ½ *B*, jump 0.7 per cycle, *F* × *f*ᵢ, *k* = 2 |*F*|², weight 2 |½ − *f*₁|, rotation by 1188, emissive at UV + 1063 × *B*, *a* = clamp(1 + *E*ₐ − *k*, 0, 1), brightness 2 × 1066 × (1.8 − *a* − ½ *E*ₐ), foam weight a clamped product, fog mix with a final clamp, alpha 1, six textures sampled, no normal map and no light input). The `material.md` §11.6 rules reproduced over all 510 shader records from their sampler and constant names: the lava rule selects 0x2B000810 alone and is disjoint from the water and liquid-distortion rules, which select the ten and four records of the table. 0x31006FAE: 1057 = 1058 = 0.1, 1063 = 6, 1066 = 1.5, 1188 = 0, flow map 0x400163D3, 4002 = 4003 = 1; colour list 0x4001D168 has alpha 255 in every texel of all nine levels; emissive list 0x4001D1AF has alpha 0–255 at its top level and a dark red mean colour. WA16 and the "What this means" paragraph of §8.6.5 recomputed (2.4 to 0.9 for 1066 = 1.5; with *E*ₐ = 0 the brightness does not depend on *T*). The Direct3D 9 programs were not read. **Blocking:** **B1** W37 and the §8.7.1 census say they list every curve-valued parameter outside the texture matrices ("no other"), but the lava surface 0x31006FAE has 1186 and 1187 as mode-4 curves (amplitude 0.001), which §8.6.5 step 6 reads; and with lava now its own family, the §8.7.1 rule ("a parameter of a water-family, mirror or liquid-distortion surface") no longer says that its curves are evaluated at the clock every frame. Add the row, widen W37's scope and extend the rule to the lava family (no visible effect in this install, since the flow map's third channel is 0). Non-blocking: (1) 0x31006FAE also carries scrolling texture-matrix curves (1017–1020) and scales of 6 (1001–1004) that only the low-detail permutation applies; §8.6.5 could say so, because under that permutation the lava would scroll (relevant to the §8.6.5 OPEN); (2) the lava drawing reaches the implementers only through the §8.6.5 Tessera rule and the §13 note: `render.md` §5.7 OPEN (1) and R-D12, and `walker.md` §14.6, still name only the two flow-map records; add lava there in the next revision. Fourth revision (spec writer, 2026-10-09) answering B1 and non-blocking note (1); note (2) left to the next revision of `render.md` and `walker.md` (the §13 row stands); awaiting re-review. Claude review session (spec reviewer, fresh session; neither the writer nor a reviewer of the earlier rounds), 2026-10-09: re-review of the fourth revision (commit 113462a; the whole branch read, `git diff main...water-fix`) against CLEAN_ROOM.md §5.3: **passed**, **Accepted** (every item). The earlier **B1** is resolved: W37 names its scope (124 surface records of the water, lava, older bump-water, mirror and liquid-distortion families, texture-matrix ids excluded) and lists 1186 and 1187 on 0x31006FAE; the §8.7.1 rule covers the lava family and the older bump water; §8.6.5 step 6 and the §8.7 rate table cite the per-frame rule. Non-blocking note (1) is taken up by the §8.6.5 texture-matrix paragraph. No code or pseudo-code (formulas and numbered steps in words; no program text, registers or listing order), no copied prose, addresses, game text, keys or hostnames; the `c_…` labels and sampler names are marked as interoperability names; nothing under a closed gate; every new fact has a provenance row and a mark of the legend; WA7–WA16 are testable; unknowns are OPEN. Spot checks (read-only, own probes written for this review, independent of the writer's scratch): OBS2's record totals (510 / 24,091 / 13,781 / 81,135); W37 re-run over all 124 records reproduces every row exactly (1045 mode 2 on 0x31002A44–A46, mode 7 on 0x31008446, 0x3100890B, 0x3100890C; 1059, 1061 on 0x3100338F; 1063–1065 on 0x3100328B, 0x3100824C; 1105 on the three puddles; 1182, 1183 on 21 and 1186, 1187 on 18 0x2B0007FD surfaces plus 0x31006FAE; nothing else), and the per-family surface counts of the §8.2 table; effective values of 0x31006748 (§8.5.2: normal map, scales 4, 8, 8, fog amount and colour, Fresnel, specular, alpha 0.75, depth write off), 0x31000A2A and the six static mirrors (§8.5.4, W36), 0x310004E6 (§8.5.3 row), 0x31004359 and 0x310066D5 (§8.7, checklist W15, W17) and 0x31006FAE (W41 and the §8.6.5 texture-matrix paragraph: 1057 = 1058 = 0.1, 1063 = 6, 1066 = 1.5, 1001–1004 = 6, 1017–1020 at −0.02 and +0.025, 4002 = 4003 = 1); the OpenGL programs of 0x2B0000AA agree with §8.5.4 (coordinates, fog without upper clamp, steps 1–5); the §8.7 and §8.7.2 rates and the §12 examples recomputed. BIN4 (W38) and the Direct3D programs were not re-read. Non-blocking: (1) W39 says 21 of the 66 plain-water surface records have a layer faster than 1.5 m/s; this review counts 20 (19 with constant parameters, plus 0x3100328B at the base of its scale curves); re-check the count or state how curve-valued and zero-scale layers were counted; (2) 1186 of 0x31006FAE (and of the flow-map surfaces) has `w[5]` = 0.2, which by `material.md` §5.3.1 adds 0.2 rad/s to its angular rate (1.2 rad/s on the lava surface, 0.7 on the flow-map ones), not the 1 rad/s of the §8.7.1 census row and §8.7 lava row; and §8.5.3 states the mode-4 value without the `w[1]`, `w[3]`, `w[5]`, `w[7]` terms (say that they are 0 in the rows tabled, or cite §5.3.1); no implementation consequence, since the rule evaluates the curves by §5.3.1; (3) carried over: `render.md` R-D12 and §5.7 OPEN (1) and `walker.md` §14.6 still name only the two flow-map records (§13 row stands). Spec writer, 2026-10-10: revision answering notes (1) and (2) here and note (3) in `render.md` §5.7, §7 and `walker.md` §14.6, together with the implementer OPENs of commits 7e775a3 and afaf0ca (see §15); awaiting review. Claude review session (spec reviewer, fresh session; not the writer), 2026-10-10: review of the revision of commit 9e9a29a (§1 tags SH3, OBS3, rows W39, W42–W48; §8.5.3 mode-4 wording; §8.5.4 W13 answer; §8.5.5 alpha test and recognition; §8.6.4 colour of 0x2B0005EC, no-reflection permutations, revised Tessera rule and answer; §8.7, §8.7.1 rates of 1182–1187; §11 WA17–WA18; §13 rows; §14 answers (a)–(f); §15 row) against CLEAN_ROOM.md §5.3: **passed**, Accepted (every item). Formulas are stated in words and the mathematical notation of the accepted §8.6.4 list; no program text, listing or code; `NoiseTexture`, `BiasForward1`, `WaterFogColor` and the parameter labels are data names needed for interoperability; SH3 states method, questions (l)–(n) and why M1–M3 were not enough for the Direct3D part (decision 0011); every new row has a source and a mark; the W13 consequence is marked Inferred and the rule built on it is a Tessera rule; the remaining unknowns (Direct3D term order of 0x2B0007FD, reflection pass, depth-bias application) stay OPEN; nothing under a closed gate. Spot checks (read-only, own probes): the OpenGL reflection permutation of 0x2B0005EC reads the reflection at the 1047-distorted point plus (0, 0.003), tints toward `WaterFogColor` by clamp(0.1 × eye distance − 0.5, 0, 1) × 1068, uses bias + (1 − bias) × (…)^exponent, has no *L*꜀ on the specular and declares no fake-light diffuse constant, and its fog vertex stage starts 100 m later; the OpenGL no-reflection permutations of both flow-map records declare no Fresnel constants and mix no reflection (W42, W43); its OpenGL water permutations blend ONE / ZERO with cull none and the Direct3D 10/11 ones source alpha / inverse source alpha; every permutation of 0x2B0000AA has depth bytes 7 and 1, and all nine mirror surfaces have 4001 = 4002 = 1 (W44, depth state); the 23 samplers holding 0x400163D4 are all `NoiseTexture`, source 0, 9 + 9 + 5 in 0x2B0005EC, 0x2B0007FD, 0x2B000810 and the only non-zero defaults there (W47); the liquid records' transform items, the Direct3D 10/11 vertex input signatures (a colour input only in 0x2B00063E; 1, 1 and 90 programs in the others) and the equal declared ids, sampler names and items of 0x2B0000CE and 0x2B000A8D in their water permutations (W46); the 0x2B00063E OpenGL pixel program multiplies the texel by the vertex colour before the material alpha, as W45 states for the Direct3D programs; the 1182–1187 curve rates with `w[5]` (0.2 on 20 of 21 curves of 1182 and on every 1186, lava 1186 1.2 rad/s); the W39 recount gives 20 surfaces; W17's river parameters. Not re-measured: the puddle placements against the height grid (W44 counts), W45 and W48 in the Direct3D bytecode. Non-blocking: (1) the OpenGL water permutations of both flow-map records still list `DepthStencilTexture` in their sampler tables (`material.md` §7.5) although their programs declare no depth sampler; "without a depth texture" in §8.6.4 and W42 is about the programs, which could be said; (2) *a* names the emissive weight in §8.6.5 (and `render.md` R-W2) and the thickness weight of 0x2B0005EC in §8.6.4; distinct letters would avoid a mix-up. Review of the second revision of 2026-10-10 (commit 2655f5f: header, tags OBS4, BIN5, SH4, rows W49–W52, the §8.5.4 note, §8.5.5 step 5 and the recognition callout, WA19, the §14 answer and new OPEN, the §15 row), 2026-10-10 (independent reviewer session, not the author): **passed**. Checked: CLEAN_ROOM.md §5.3 items on the changed text: functional facts only (lookup order, storage, rasteriser-state contents, interface census, alpha-test rule); no code or pseudo-code, the program reading is reduced to words with no program text; no copied prose; identifiers only data-dictionary and sampler names needed for interoperability; no game text, keys, hostnames or traffic; each new fact has a dated provenance row (Confirmed for the census, BIN for the Ghidra facts with decision 0011 named and questions listed); unknowns kept as OPEN with a Tessera default; no closed gate. Spot-checked independently and read-only with the repository's decoders as tools: `Render_DepthBias_Type` over all 3,253,862 SEDs of all 49,636 landblock-info records (own set, instance set, 0x47 chain, and for the 33,676 Weenie SEDs the 0x70 set and `PhysObj` chain): found only on 52,028 Physics SEDs, 49,338 in both sets with equal values, 2,690 only along the chain, 1 (0x802515A0, object 0x4700C32A) with 4 in its sets and 3 on the object; the client's order and Tessera's give the same value for every SED; the per-value histogram matches `decals.md` §6.5 (W50); PIDs 0x7F0–0x7F5 named as stated and the two definition sets carry only slope-scale values, |value| ≤ 10 (W51 PIDs, §14 (b)); the four liquid-distortion records: 0x2B000442 has five bindings with semantics 33–37 (ids 1001, 1002, 1017, 1018, 1033), all naming its one permutation without programs, whose items are of types 3, 2, 1, and no type-5 item anywhere; 0x2B0004B5 and 0x2B00063E have type-5 items bound with 38–44 in their three permutations with programs, 0x2B0007D0 in all 162 (90 Direct3D 10/11, 36 OpenGL, 36 Direct3D 9 shader model 2.0) (W52, §14 (c)); the 90 Direct3D 10/11 vertex programs of 0x2B0007D0 have no colour input (one in 0x2B00063E does); of its 36 OpenGL pixel programs, 34 discard when the reference minus the alpha of the second sampler's texel at the distorted coordinates is ≥ 0, read straight from the fetch before any multiply, and 2 have no alpha comparison and write texel alpha × material alpha (W52, §8.5.5 step 5, §14 (d)). Not re-derived: the BIN5 facts (W49, W51; taken on their provenance) and the Direct3D 10/11 pixel-program count 88 of 90. Non-blocking: (1) `decals.md` §6.5 extends the entity lookup of E26 to 0x70 objects by citing E24 (the SED lookup); harmless here since no 0x70 source carries the property, but the next revision could say which row covers weenie entities. Review of the third revision of 2026-10-10 (commit 29d5643: header, tags BIN6 and OBS5, rows W53–W54, §8.5.4 puddle rule, §8.5.5 note, the two answered OPENs of §14, §15 row; independent reviewer session, not the author): **changes requested**. Items that hold: functional facts only; no code, pseudo-code, addresses, program text or copied prose; no game text, keys, hostnames or traffic; BIN6 names decision 0011, question (s) and why M1–M3 were not enough; OBS5 is dated; the disagreement rule is marked a Tessera Decision; no closed gate. Spot checks: W53 against the binary in an own Ghidra session (the entity node's per-pass draw sets the stored entry, entry 2 in pass 0x0A for 0, every part, reset to 0; initial value 0; details in the `decals.md` fifth review): matches. W54 re-counted independently over all 510 shader records with an own parser of `material.md` §5 and §7 (zero leftover bytes in all): rules (i) and (ii) never disagree; 191 records have neither a binding with a semantic of 38–44 nor a type-5 item, 319 have both; the W46 property (a texture-transform item for matrix 0 in a permutation with programs) holds in 313 records and the binding form (a binding with a semantic of 33–44 naming a permutation with programs) in the same 313; the rules differ from W46 in exactly the 8 records named (7 with all 38–44 bindings and type-5 items in permutations without programs, and 0x2B0000B7 with items of types 1–3 bound with 33–37); among the four liquid-distortion records only 0x2B000442 satisfies the rules. **Blocking: B1** W54 says "191 records satisfy neither, 319 both". With the rules as W52 and §8.5.5 define them (rule (i) holds when **no** declaration has a binding with a semantic of 38–44, rule (ii) when **no** permutation has a type-5 item), it is the other way round: 191 records satisfy both rules and 319 neither. The row contradicts itself (0x2B0000B7, which it says satisfies both, is one of the 191), and a real-data test written from it would fail. Swap the two counts, or say "191 records have neither a 38–44 binding nor a type-5 item, 319 have both". No other finding. |
| **Implements into** | `crates/material` and `crates/render` (routing of placed water surfaces by program family, §8.5; flow-map water, §8.6; animation rates, §8.7), `crates/formats` (decoders for the water table, prefix 0x16, and the fog-zone table, prefix 0x17; one fuzz target each), `crates/world` (per-landblock water state, water volumes, "is under water" query), `crates/terrain` or `crates/render` (water quads per landblock), `crates/render` (water pipeline), the movement controller of `collision.md` §12 (water states), `apps/explorer` (water table and per-landblock water view) |
| **Companion specs** | [`environment.md`](environment.md) §3 (region descriptor: slots 5 and 6 name the two tables). [`landblock.md`](landblock.md) §3.4 (height conversion), §5 (block maps; keys 3, 4 and 7 are read here), §7 (distant-landscape sector records). [`material.md`](material.md) §4 (surface records 0x31), §5–§8 (shader records 0x2B, parameter entries, labels; §8.6: `c_`-labelled parameters feed the program constant of the same name). [`collision.md`](collision.md) §9–§12 (volumes, movement states, controller). [`scenery.md`](scenery.md) §7.4–§7.5 (height modes and water ranges that read the water plane). [`entity-descriptor.md`](entity-descriptor.md) (placed static entities). [`property-system.md`](property-system.md) §5 (0x78 property sets), §7 (enum mappers). [`records.md`](records.md) §1 (VLE), §2.1 (prefix → DB type table). |

---

<a id="s0"></a>
## 0. Purpose

This spec is the single place that describes water. With it an implementer can:

- decode the region's **water table** (prefix 0x16) and **fog-zone table** (prefix 0x17)
  to the last byte (§4, §5);
- decide, for every outdoor landblock, whether it has **landblock water**, its water type and
  its surface height, exactly as the original client does (§6);
- place the **water volumes** that turn a mover into a wading or swimming one (§7);
- draw the water surface with the original's material chain, geometry and shading inputs
  (§8), including distant water (§9);
- shade placed water (fountains, pools, puddles, waterfalls, rapids) by the program family of
  each surface (§8.5), draw the flow-map water that covers most wet landblocks (§8.6), and
  animate every kind of water at the original's rates on the original's time base (§8.7);
- answer the questions other systems ask about water: scenery height checks, the encounter
  pass, "is this point under water" (§10).

It also records which statements in other specs about water were wrong (§13).

All counts are observations of one install on one date and change with patches. Tests assert
the invariants (§11), not the counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install (Steam) on 2026-10-05 by throwaway probes in the dirty-room scratchpad: all 5 records of prefix 0x16 and all 5 of prefix 0x17 decoded to the last byte; all 7 region descriptors; all 390,150 block maps (every key and value) and the minimum and maximum of all 390,150 height grids of regions 1–5 and 14; all 33,159 distant-landscape sector records (kind 5) searched for their water pairs; all 3,244,086 placed static entities of all 49,636 landblock-info records (object DID, position, world-building label); all 40,643 entity descriptors (0x47) and all 1,616 stand-alone property sets (0x78) checked for liquid-volume properties; the 76 surface records (0x31) named by the water tables and their shader records (0x2B). |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read with objdump and capstone in the dirty room on 2026-10-05. **Questions:** (a) which DB object type the client registers for each DID range (to settle what prefixes 0x16 and 0x17 are); (b) the deserialisers of the water table, the fog-zone table and the region descriptor; (c) how a landblock decides it has water: which block-map keys, how a stored level becomes a height, the presence rule, the plane; (d) who consumes that state: the water-volume entity, the "under water" test, the scenery and encounter passes; (e) how water geometry is built for drawing; (f) how the fog-zone table is looked up. **Why M1–M3 were not enough:** block-map keys 3, 4, 6 and 7 have no descriptor entry in the data (`landblock.md` §5.6), so nothing in the data says which table reads them or how a stored value becomes a height; the earlier specs disagreed about which slot is water (§13); no published source describes these records; no running client is available to this session for M3. Only offline world code was read: nothing about network, login, keys, launcher or anti-tamper. **Second use, 2026-10-08 (same build; Ghidra headless on a copy of the analysed project, deleted afterwards; answering the implementer OPENs of 2026-10-07):** (g) which value types the client's readers of the volume flags accept; (h) the routine that sets a mover's `Entity_Element` and the inputs it reads; (i) how a monitor's `Monitor_Offset` and box corners are read. **Why M1–M3 were not enough:** the dictionary gives each flag one type, so the data never exercises another; the state rule and the monitor reader are run-time behaviour no record stores; no running client or published source is available. Read: the volume-flag readers, the element routine and its support-flag accessors, the monitor-array reader. |
| BIN2 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Same client build (file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), read with Ghidra in the dirty room on 2026-10-08. **Question (g):** what the player controller does to a mover's downward speed and displacement when the mover enters water (the clean-room OPEN of the M4.5 exit verification in §7.4). Read: the player controller's update, its velocity and displacement routine, its water-surface routine, the move request and the routine that publishes the movement state; the constants they load were read from the executable. **Why M1–M3 were not enough:** the data hold only the layer parameters (`collision.md` §10.3), not how the controller applies them to a mover entering water; no running client is available to this session (M3). Only offline movement code was read: nothing about network, login, keys, launcher or anti-tamper. |
| BIN3 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Same client build, read with Ghidra 12 headless on a private copy of the analysis project (deleted afterwards) on 2026-10-09; recorded in `collision.md` §1 as question 70. **Question (j):** whether the routine of W19 and the controller routine of BIN2 that set `Entity_Element` are one routine or two, and which applies after the player controller's updates. **Why M1–M3 were not enough:** both earlier readings were of code; the data holds no state rule; the original cannot be observed here. |
| OBS2 | Observed in the user's install (Steam, data unchanged since 2026-10-04) on 2026-10-09 by throwaway probes in the dirty-room scratchpad: all 510 shader records (0x2B), 24,091 materials (0x30), 13,781 surface records (0x31) and 81,135 texture lists (0x40) of `client_general.dat` decoded again and every surface record's effective parameter set formed (`material.md` §9.2); the water type, height and surface record of all 89,913 wet landblocks of §6.4; all 3,253,835 entity rows of all landblock-info records (every SED type) joined to their setups, meshes and surface records; the texture levels named by the surfaces of §8.5 and §8.6 decoded and looked at. The framing of the checklist cameras of `m4-5-visual.md` §4.8 was checked with Tessera's own walker (`--screenshot`), images private. |
| SH2 | The programs of the shader records 0x2B0000A8, 0x2B0000A9, 0x2B0000AA, 0x2B000442, 0x2B0004B5, 0x2B00063E, 0x2B0005EC, 0x2B0007D0, 0x2B0007FD and of the two-layer records of `material.md` §10.3 in the same install, read on 2026-10-09: the OpenGL programs as text (method M1, as SH) and the Direct3D 10/11 programs (kind 4) through a public disassembler library, which is method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md) as for `material.md` BIN-S (the compiled programs stored in the shader records). **Question:** how the water-family, mirror, liquid and two-layer records animate and shade (§8.5–§8.7). **Why M1–M3 were not enough for the Direct3D part:** the OpenGL text of 0x2B0007FD omits terms that its Direct3D 10/11 programs contain (§8.6.2), so the bytecode had to be read; no running client is available here for M3. Reduced to words; no program text or listing is reproduced. Facts marked "programs" were found in both back ends; facts marked "D3D" rest on the Direct3D 10/11 programs only. **Added 2026-10-09 (third revision):** the two Direct3D 10/11 water permutations of the lava record 0x2B000810, same method and day; question: how landscape lava is drawn (§8.6.5); why M1–M3 were not enough: the record's only OpenGL program is its low-detail two-colour permutation, so the water permutations exist only as compiled Direct3D programs (its Direct3D 9 permutations were not read). |
| LIT | [`lighting.md`](lighting.md) row L25 (BIN-L (i)): the programs' time input E20 is the client's global clock in seconds, reduced modulo 3,600 once it exceeds 3,600; `material.md` §5.3.1 (the same clock drives the float curves). |
| TP | Tessera's own real-data probe of 2026-10-09 (clean-room code on branch `water-fix`, commit b1da85a: the fallback diagnostic of `material.md` §19 and a test that walks every landblock of the install with the walker's draw rule), run on the user's install; its counts were read by the spec writer and the facts used here were re-checked against the records with the dirty-room probes of OBS2 on the same day. Counts are **submesh draws** over all 390,150 landblocks (one draw per part instance and submesh), not placements, so they differ from the placement counts of §8.5.1. |
| BIN4 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Same client build (file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), Ghidra 12 headless on a private copy of the analysis project (deleted afterwards), 2026-10-09. **Question (k):** how the engine fills the horizontal offset *O* of §8.3 (the program constant the water programs subtract from the world position). **Why M1–M3 were not enough:** *O* is an engine input that no record stores; no published source describes it; no running client is available for M3. Read: the engine-constant name table, the routine that returns each engine constant's storage, the routine that writes *O*, and the landblock-difference helper it calls. Only offline rendering code was read: nothing about network, login, keys, launcher or anti-tamper. |
| SH3 | The programs of the flow-map records 0x2B0005EC and 0x2B0007FD and of the liquid-distortion records 0x2B000442, 0x2B0004B5 and 0x2B00063E in the same install (data unchanged since 2026-10-04), read again on 2026-10-10 from the dirty-room extraction of OBS2: the OpenGL programs as text (method M1, as SH: the reflection and the no-reflection water permutations of both flow-map records, every colour term and the vertex stage of 0x2B0005EC) and the Direct3D 10/11 pixel programs through the public disassembler library of SH2 (method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), as SH2: the two water permutations of 0x2B0005EC without the ripple map, the view-depth input of 0x2B0007FD, the alpha test of the three unlit liquid records). **Questions:** (l) the Fresnel form and colour terms of each flow-map record, and how 0x2B0005EC is coloured without a depth texture (implementer OPEN of §8.6.4); (m) which alpha the liquid programs test (implementer OPEN of §8.5.5); (n) what "view depth" is in the depth terms (§14 OPEN (e)). **Why M1–M3 were not enough for the Direct3D part:** the OpenGL programs of 0x2B0005EC have no depth sampler, so the depth form exists only as Direct3D bytecode, and the liquid records' alpha test is only in their Direct3D programs (their OpenGL text was not re-read); no running client is available here for M3. Reduced to words; no program text or listing is reproduced. |
| OBS3 | Observed in the user's install (Steam, data unchanged since 2026-10-04) on 2026-10-10 by throwaway probes in the dirty-room scratch area (the repository's decoders used as tools, read-only): every entity row of the landblock-info records of regions 1–5 and 14 whose object is one of the five puddle objects of §8.5.1 (1,739 placements, 784 of them outdoors), with the effective `Render_DepthBias_Type` (row, object and template chain) and the height grid of its landblock sampled at 21 × 21 points of the placed quad; the five puddle objects' properties; the depth-bias definition sets 0x780001F5 and 0x780001F6 and enum mappers 0x23000297, 0x23000298 (`decals.md` §6.5); the render-state blocks of every permutation of 0x2B0000AA, 0x2B0005EC and 0x2B0007FD; the declared ids, sampler tables and extra state lists of every permutation of the plain-water, plain-water-on-UV and liquid-distortion records; the sampler defaults of all 510 shader records; every curve of the parameters 1182–1187 on the flow-map and lava surfaces; the rates of all 66 plain-water surface records recomputed for W39. |
| OBS4 | Observed in the user's install (Steam, data unchanged since 2026-10-04) on 2026-10-10 by throwaway probes in the dirty-room scratch area (the repository's decoders used as tools, read-only): `Render_DepthBias_Type` on **every** SED of all 49,636 landblock-info records of regions 1–5 and 14 (3,253,862 SEDs of every type), looked up in the SED's own set, in the landblock's instance property set for its IID, along the 0x47 chain of the object it names and, for the 33,676 Weenie SEDs, in the 0x70 object's property set and its `PhysObj` chain; the client's order (`entity-descriptor.md` §9.2, E26) compared with Tessera's order for every SED; the declarations, bindings (semantic and named permutation) and extra state items of every permutation of the four liquid-distortion records of `material.md` §11.6; the property names of PIDs 0x7F0–0x7F5 in the master dictionary. |
| BIN5 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Same client build (file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), Ghidra 12 headless on a private copy of the analysed project (deleted afterwards), 2026-10-10. **Questions:** (o) how `Render_DepthBias_Type` reaches an entity's render component and in which form it is stored; (p) which draws of an entity apply it; (q) which of the three values of a depth-bias table entry reach the rasteriser. **Why M1–M3 were not enough:** the data hold only the property values and the definition sets; how they reach a draw is run-time behaviour no record stores; no running client is available for M3, and a frame capture would show a bias but not which property chose it. Read: the render component's watched-property list and its property-change handler, the depth-bias definition loader, the rasteriser-state cache of one graphics back end and the two draw routines that build its key, the back end's depth-bias index setter, and a search for the readers of the component's stored value and for the setter's call sites. Only offline rendering code was read: nothing about network, login, keys, launcher or anti-tamper. |
| SH4 | The programs of the lit liquid-distortion record 0x2B0007D0 in the same install, read on 2026-10-10: all 36 OpenGL pixel programs as text (method M1, as SH) and all 90 Direct3D 10/11 pixel programs through the public disassembler library of SH2 (method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), as SH2), each traced from its alpha comparison back to the value compared. **Question (r):** which alpha the alpha test of 0x2B0007D0 compares (implementer OPEN of §14, item (d)). **Why M1–M3 were not enough for the Direct3D part:** the OpenGL text covers one back end only; no running client is available here for M3. Reduced to words; no program text or listing is reproduced. |
| BIN6 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Same client build (file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), Ghidra 12 headless on a private copy of the analysed project (deleted afterwards), 2026-10-10; recorded in `decals.md` as tag BIN3 (questions 14–18 there). **Question (s):** which draws of a placed entity, in which passes, apply the depth-bias entry its render component holds (the OPEN of §14 after BIN5 (p)). **Why M1–M3 were not enough:** run-time behaviour with no trace in any record; no running client for M3. Read: the render component's constructor, the graphics device's method table, its depth-bias index setter and every call of it, the entity scene node's draw routines and the part draw down to the per-pass permutation lookup. BIN5's search for the setter's call sites had looked at the wrong slot of the device's method table. Only offline rendering code was read: nothing about network, login, keys, launcher or anti-tamper. |
| OBS5 | Observed on 2026-10-10 by a throwaway probe on the dirty-room extraction of OBS2 (all 510 shader records; install data unchanged since 2026-10-04): for every record, whether a declaration has a binding with a semantic of 38–44 (rule (i) of W52), whether a permutation holds a type-5 extra state item (rule (ii)), whether a binding with a semantic of 33–44 names a permutation with programs, and whether a permutation with programs holds a texture-transform item (type 1, 2, 3 or 5) for texture matrix 0 (the property of W46). |
| SH | The water shader records (prefix 0x2B) of the same install carry the OpenGL back end's programs as text. Those of shader record 0x2B0000A8 (used by 43 of the 76 water surfaces) were read in the dirty room (method M1, data files) and reduced to the words of §8.3–§8.4; the sampler lists of the other five water shader records were read. No program text is reproduced. |
| LC | LotroCompanion `lotro-tools` (`RegionLoader`, readable source; facts only): names region slot 5 "water" and slot 6 "fog". Reported only; confirmed here by BIN. |
| ENV, LB, COL, SCN, MAT | `environment.md`, `landblock.md`, `collision.md`, `scenery.md`, `material.md` (same install). |

Marks: **Confirmed** = checked on every record of the kind in this install on the date given
(for BIN facts: the data agrees on every record); **BIN** = rests on the disassembly only;
**Probable** = supported by BIN and data together but not proven for every case; **Reported**
= published sources only; **Inferred** = a conclusion drawn from Confirmed or BIN facts that
no probe or listing has shown directly (the reasoning is given beside it; implementers treat
it like a Tessera Decision until it is confirmed); **Tessera Decision** = Tessera's own rule
where the original's is not known (never a statement about the original).

| # | Fact | Sources | Mark |
|---|---|---|---|
| W1 | Region slot 5 names the water table (prefix 0x16, DB type 0x17 WATER_DESC); slot 6 names the fog-zone table (prefix 0x17, DB type 0x16 FOG_DESC) (§3) | BIN (a), (b), (c), (f); LC; OBS | Confirmed |
| W2 | Water-table layout (§4.1–§4.2) | BIN (b); OBS (5 of 5, zero leftover bytes, 125 entries) | Confirmed |
| W3 | Fog-zone-table layout (§5.1–§5.2) | BIN (b); OBS (5 of 5, zero leftover bytes) | Confirmed |
| W4 | Roles of the water-entry fields: surface record, volume entity; four further DIDs and a float unused by this install (§4.2) | BIN (c), (d), (e); OBS | Confirmed for surface and entity; the rest OPEN |
| W5 | The water table names the block-map keys it reads: level key at record offset 8, type key at offset 12; the block map's raw 16-bit value is used, no 0x18 palette (§6.1) | BIN (c); OBS (keys 4 and 7 in every block map; no 0x18 descriptor lists them) | Confirmed |
| W6 | Water height = level × 1000 / 255 world units, computed in 32-bit floats (§6.2) | BIN (c); OBS (§6.5, §9) | Confirmed |
| W7 | A landblock has water exactly when its type selects an entry that is not "dry" and the height is strictly above the block's lowest terrain sample (§6.2) | BIN (c); OBS (the distant-sector water tables of all 12,925 sectors with wet landblocks contain every pair this rule predicts, §9) | Confirmed |
| W8 | 89,913 of 390,150 landblocks have water (§6.4) | OBS (every block map and height grid) | Confirmed |
| W9 | A point is under landblock water when its height is more than 0.0002 below the surface (§6.3) | BIN (d) | BIN |
| W10 | Each wet landblock creates the water type's volume entity at block-local (0, 0, *h*); its three monitor boxes define surface, wading and deep zones (§7.1) | BIN (d) (placement); OBS (properties of the three volume entities) | Placement BIN; boxes Confirmed |
| W11 | 3,895 placed static entities (37 distinct 0x47 records) carry liquid volumes of their own (§7.3) | OBS | Confirmed |
| W12 | Landscape water is drawn as one horizontal 160 × 160 quad per wet landblock at its water height, gathered per distant-landscape sector and water type (§8.1, §9) | BIN (e) | BIN; the only landscape-water geometry builder found (Probable that it serves all distances) |
| W13 | Water texture coordinates come from the world position, block units (1/160), per-layer scale and scroll speed; three normal-map layers; vertex waves; Fresnel mix of refraction and reflection (§8.3–§8.4) | SH (program text of 0x2B0000A8); MAT §8.0 (labels) | Confirmed for 0x2B0000A8 (formula in data); other water shaders Probable |
| W14 | Fog-zone values are looked up at the landblock and its east, north and north-east neighbours; only entries flagged active are used (§5.3) | BIN (f) | BIN |
| W15 | Block-map key 3 is 0 in every landblock, so no fog zone is active anywhere in this install (§5.4) | OBS | Confirmed |
| W16 | Shader record 0x2B0000A8 is named by 55 surface records (0x31), 43 of them water surfaces of the tables; its water permutations take positions only and composite the frame-buffer copy, its low-detail permutations sample two colour layers and are lit per vertex by ambient, hemispheric ambient and the per-vertex lights (§8.2) | OBS (all surface records, 2026-10-05); SH (sampler lists, render-state bytes and constant lists of every permutation of 0x2B0000A8) | Confirmed |
| W17 | The specular direction of §8.4 step 6 is the main light's travel direction when the main light is directional, else an engine default direction (§8.4) | `lighting.md` §4.2, row L24 (BIN-L (g)) | BIN |
| W18 | The client's readers of `Volume_IsLiquid`, `Volume_IsLava` and `Volume_IsCoin` count a flag as set only when the value found is a Boolean (type 21) and true; all volume flags are Boolean in the dictionary (§7.2) | BIN (g); OBS (dictionary types, 2026-10-08) | BIN (readers); Confirmed (types) |
| W19 | One of the client's two routines that set `Entity_Element` (the one the position-update paths call, BIN3 (j)) gives 1, 3 or 2 when the mover's own `Volume_IsLiquid` is true (walkable support, other support, no support) and 5, 6 or 4 otherwise, with no depth or float-height input; every `Volume_IsLiquid` set counts, coin sets included (§7.4; `collision.md` §11.3.2 (b)) | BIN (h); BIN3 (j) | BIN |
| W20 | A monitor's `Monitor_Offset` (a vector) is added to both box corners; the corners and the offset are used only when they are vectors (§7.1) | BIN (i) | BIN |
| W21 | The original does not cancel or clamp a downward speed or a falling displacement in water; a mover entering deep water is slowed only by the Swimming layer's gravity and once-per-update damper, and the first update whose displacement rises puts its feet on the float line (§7.4) | BIN2 (g); `collision.md` §11.1 step 9, §11.6 items 3, 4, 7 (BIN) | BIN |
| W22 | After every update of the player controller the client sets `Entity_Element` by the same table with an in-water input that also needs the water surface at least the float height minus 0.0002 m above the feet; this routine, not W19's, decides the state of a moving player (§7.4; `collision.md` §11.3.2 (a), C64) | BIN2; BIN3 (j) | BIN |
| W23 | Wet landblocks by the shader record of their surface: 0x2B0007FD 83,929; 0x2B0000A8 4,093; 0x2B0005EC 1,130; 0x2B000810 656; 0x2B0000CE 61; 0x2B000559 44 (of 89,913) (§8.2) | OBS2 | Confirmed |
| W24 | The time input *T* of every water-family, mirror and liquid program is E20: the global clock in seconds, reduced modulo 3,600 above 3,600; the texture-matrix curves of the two-layer records use the same clock (§8.7) | LIT; SH2 (every such program reads the one time constant of the engine) | BIN |
| W25 | The Direct3D 10/11 water programs of 0x2B0000A8 compute the normal-layer coordinates exactly as its OpenGL text (§8.3) | SH2 (D3D, all four kind-4 water permutations) | Confirmed (programs) |
| W26 | The flow-map records 0x2B0007FD and 0x2B0005EC use 1057 and 1058 as the rates, in cycles per second, of two flow phases, not as scroll speeds; layer 0 is the normal map at *B* × `Normal0Scale` displaced along the flow vector of the flow map (read at the mesh's UV set 0) by the phase, with a noise phase offset and a jump of 0.7 per cycle; the two phases are blended by a triangle weight; the flow normal is scaled by 2 \|*F*\|² (§8.6.2) | SH2 (OpenGL text and Direct3D 10/11 programs of both records agree) | Confirmed (programs) |
| W27 | The Direct3D 10/11 water programs of 0x2B0007FD add normal layers 1 and 2 (*B* × `Scale`ₖ + *g* × `Speed`ₖ × *T*, *g* = clamp(0.0004 × view depth, 1, 10)) weighted by (1 − \|*F*\|) × (1 − foam weight); its OpenGL programs and every program of 0x2B0005EC have no layers 1 and 2 (§8.6.2) | SH2 (D3D; OpenGL text for the absence) | BIN |
| W28 | All 27 surface records of the flow-map family and the lava record (0x2B0007FD 21, 0x2B0005EC 5; lava 0x2B000810 1, §8.6.5) name flow map 0x400163D3 (one 32 × 32 level whose two flow channels lie within 125–128 of 255: \|*F*\| ≤ 0.02), and their noise samplers hold texture list 0x400163D4 as the sampler default (`material.md` §7.5) with no binding (§8.6.3) | OBS2 | Confirmed |
| W29 | Of the 3,253,835 entity rows of landblock info, 6,105 placements of 105 objects use a water-family record, 1,841 of 14 objects the mirror record 0x2B0000AA (puddles and a few static mirrors, W36), 593 of 63 objects a liquid-distortion record (§8.5.1) | OBS2 | Confirmed |
| W30 | The slot-3000 and 3001 defaults of the water records, texture lists 0x40000DDF and 0x40000DE0, are near-white images; the water permutations of those records sample neither; only the low-detail `Color` / `Color2` permutations do (§8.5.2) | OBS2 (levels decoded); SH (sampler lists) | Confirmed |
| W31 | Mirror record 0x2B0000AA (puddles): colour = reflection × diffuse × material colour, alpha = Fresnel × diffuse alpha × material alpha, the normal map scrolled in UV space (§8.5.4) | SH2 (programs) | Confirmed (programs) |
| W32 | Liquid-distortion records 0x2B000442, 0x2B0004B5, 0x2B00063E and the lit 0x2B0007D0: three scrolled layers of a distortion normal map offset the colour texture, which itself scrolls with layer 2 (§8.5.5) | SH2 (both back ends for 0x2B0004B5; D3D for the other three) | Confirmed (programs) for 0x2B0004B5; BIN for the others |
| W33 | Older bump-water records 0x2B0000A9 and 0x2B0000DC: normal layers at the mesh UV × `BumpTextureScale` × (1, 2, 4) plus `BumpSpeed` × *T* × (1, 4, 8); plain water on mesh UV (0x2B00084B, 0x2B00089A, 0x2B000A8D): layer *k* at UV × `Scale`ₖ + `Speed`ₖ × *T* (§8.5.6) | SH (OpenGL vertex text of 0x2B0000A9 and 0x2B00084B; constant lists of the others) | Confirmed for 0x2B0000A9 and 0x2B00084B; the others Probable |
| W34 | Waterfall sheets, foam, spouts and fountain overlays are two-layer `ColorAndOpacity` records whose texture matrices are animated by float curves and whose layers combine per record (multiply or add, `material.md` §10.3) (§8.5.3) | OBS2; SH2 (programs) | Confirmed |
| W35 | Drawn with the walker's draw rule over every landblock, 15 water-record surfaces (6,133 draws in 1,471 landblocks), 8 mirror-record surfaces (3,491 draws) and 11 liquid-distortion surfaces (647 draws) reach placed meshes; 109 two-layer surfaces with animated texture matrices (43,373 draws) belong to waterfalls, spouts, foam and similar streaming water; every placed object with liquid-volume monitors (§7.3; 39 objects with parts) draws at least one water-record surface (§8.5.1) | TP; OBS2 (records re-checked) | Confirmed |
| W36 | Mirror record 0x2B0000AA has 9 surface records: the 3 puddle surfaces (animated normal scroll) and 6 static ones with other Fresnel, distortion and diffuse values, four of them with Fresnel bias 1 (§8.5.4) | OBS2 (all 13,781 surface records) | Confirmed |
| W37 | Curve-valued parameters (a curve of a mode other than 1; texture-matrix ids 1001–1006, 1017–1022, 1033–1035, 1070–1075 excluded) of all surface records of the water families (plain water, plain water on mesh UV, flow map), the lava family, the older bump water, the mirror record and the liquid-distortion records (124 surface records, §8.2): 1045 (mode 2) on 0x31002A44–0x31002A46; 1059, 1061 (mode 4) on 0x3100338F; 1063–1065 (mode 4) on 0x3100328B and 0x3100824C; 1045 (mode 7) on 0x31008446, 0x3100890B, 0x3100890C; 1105 (mode 2) on the three puddles; 1182, 1183 (mode 4) on all 21 and 1186, 1187 on 18 of the 0x2B0007FD surfaces; 1186, 1187 (mode 4, amplitude 0.001) on the lava surface 0x31006FAE; no other (§8.7.1; scope widened to the lava family and the older bump water 2026-10-09, fourth revision) | OBS2 (all 13,781 surface records; re-run 2026-10-09 over the 124 records of these families) | Confirmed |
| W38 | The engine recomputes *O* whenever a position is handed to the world view (Inferred: the viewer's position): 160 m times the landblock difference between the viewer's position and a reference position held by a per-region object, minus a horizontal offset stored in that object; a change of region replaces the object; when both positions are in the same landblock the difference is zero (§8.3) | BIN4 (k) | BIN |
| W39 | Every plain-water rate follows from the surface's own data, including the fast ones (up to 11.7 m/s on 0x31000CA1, 5.3–6.7 m/s on 0x31002A44–0x31002A46, 6.3 m/s on 0x3100068D and 0x31000CF3); of the 66 plain-water surface records, 20 have a layer faster than 1.5 m/s (18 with constant speeds and scales, plus 0x3100328B and 0x3100338F counted at the base value `w[0]` of their curve-valued scales or speeds; a layer with scale 0 has no rate in m/s and is not counted; corrected 2026-10-10, was 21), and only 9 of those are used by wet landblocks (0x31002F37 42, 0x31003E6C 25, 0x3100664C 21, 0x31000C80 5, 0x3100328B 2, 0x31003E6D 2, 0x31000697 2, 0x31002A44 1, 0x31000692 1); a normal-layer scale of 0 occurs (0x31000698 layer 1, one wet landblock) (§8.7.2) | OBS2 (all 66 plain-water surface records, all 89,913 wet landblocks); TP (rates recomputed); OBS3 (count of 20 recomputed, 2026-10-10) | Confirmed |
| W40 | Lava record 0x2B000810, Direct3D 10/11 water permutations: two colour layers (slots 3000, 3001) read at the two flow phases of §8.6.2 steps 1–4 and blended as step 5; an emissive layer (slot 3018) at UV + 1063 × *B* with no time term, weighted by clamp(1 + emissive alpha − 2 \|*F*\|², 0, 1); brightness 2 × 1066 × (1.8 − that weight − ½ emissive alpha); foam as §8.6.2 step 7, clamped; distance fog of §8.3 and a final clamp; output alpha 1; no light, frame-buffer, reflection or depth input; 1057 and 1058 are flow rates, the other declared water parameters are not read; both permutations have the same pixel program (§8.6.5) | SH2 (D3D, both kind-4 water permutations) | BIN |
| W41 | 0x2B000810 is the only shader record of the 510 that declares 1057 and has a `FlowTexture` sampler but no `FrameBufferTexture` sampler; it has one surface record, 0x31006FAE, used by landblock water only (656 wet landblocks, W23; no placed mesh); its bindings name 3000, 3001 (`ColorAndOpacity`, `ColorAndOpacity2`), 3018 (`Emissive`), 3011 (`NormalTexture`, not sampled by the Direct3D 10/11 programs), 3075 (flow, clamp), 3079 (foam); the noise sampler has no binding and the default 0x400163D4; 3013 is declared without a binding; the render-state block of its water permutations is source alpha / inverse source alpha, depth test nearer-or-equal, depth write on, cull none; on 0x31006FAE the colour layers are one texture list (0x4001D168) whose alpha is 255 in every texel of all 9 levels, and the emissive list (0x4001D1AF) has alpha from 0 to 255; its only OpenGL program is the low-detail `Color` / `Color2` permutation (§8.6.5) | OBS2 (all 510 shader records; the one surface record; every level of the colour list decoded) | Confirmed |
| W42 | Colour of 0x2B0005EC in its OpenGL programs, which have no depth sampler (its no-depth form): refraction at *S* + *N*.xy × 1048; reflection at *S* + *N*.xy × 1047 + (0, 0.003) (distorted, unlike 0x2B0007FD); tint toward `WaterFogColor` itself by clamp(0.1 × eye distance − 0.5, 0, 1) × 1068 (no fake-light diffuse colour anywhere in the record, so no *L*꜀); Fresnel 1050 + (1 − 1050) × max(1 − \|*v* · *N*\|, 0)^1051; colour × `WaterColor` × 1066 plus the specular term with exponent 1054 and no *L*꜀; distance fog starting 100 m later; output alpha the far-draw fade; render state ONE / ZERO, cull none. Its Direct3D 10/11 water permutations use the same Fresnel form (multiplied by the reflection-brightness factor of §8.6.4), tint by the water thickness, and scale the colour by 1 + (1066 − 1) × *a* with *a* = clamp(thickness / 1170 + 0.1, 0, 1), which is also the output alpha before the far-draw fade (§8.6.4) | SH3 (OpenGL text of both water permutations and the vertex stage; Direct3D 10/11 pixel programs of both permutations without the ripple map) | Confirmed (programs) for the OpenGL form; BIN for the Direct3D terms |
| W43 | The OpenGL no-reflection water permutations of 0x2B0007FD and 0x2B0005EC have no Fresnel and no reflection term: colour = tinted refraction × `WaterColor` × 1066 + specular, then fog; the Fresnel forms of §8.6.4 (0x2B0007FD: bias × (…)^(1 / exponent); 0x2B0005EC: bias + (1 − bias) × (…)^exponent) occur only in the reflection permutations (§8.6.4) | SH3 (OpenGL text, both records) | Confirmed (programs) |
| W44 | Puddles are depth-tested and write depth in every permutation of 0x2B0000AA (render-state byte 3 = 7, byte 4 = 1; 4001 = 4002 = 1 on the three puddle surfaces); the depth-bias table's definitions are slope-scale values only, from −10 to +10 (`decals.md` §6.5); object 0x470015E4 has no `Render_DepthBias_Type` (54 of its 832 placements set 3 on the row), objects 0x47006B5E and 0x4700C32A–0x4700C32C set 3 (one row of 0x4700C32A sets 4); checklist row W13's placement (0x80217574 #170, scale 8.7) sets none and 80 % of its quad (353 of 441 sample points) lies below the height grid, up to 2.6 m; of the 784 outdoor puddle placements 666 lie wholly above it and 118 partly below (§8.5.4) | OBS3 | Confirmed |
| W45 | The unlit liquid-distortion programs (0x2B000442, 0x2B0004B5, 0x2B00063E) apply their alpha test to the output alpha: texel alpha × material alpha, in 0x2B00063E after the texel was multiplied by the vertex colour (RGBA); a pixel is discarded when that alpha is at or below the reference (§8.5.5) | SH3 (Direct3D 10/11 pixel programs of all three) | BIN |
| W46 | Of the four liquid-distortion records only 0x2B000442 (world-position layers) has no texture-transform item in its permutations with programs (its one permutation without programs has translation, rotation and scale items for matrix 0, like that of 0x2B0004B5); 0x2B0004B5, 0x2B00063E and 0x2B0007D0 carry a type-5 item for matrix 0 in every permutation with programs; and 0x2B00063E is the only one of the four whose vertex programs take a colour input (its one Direct3D 10/11 vertex program; none of the 1, 1 and 90 of the others; `lighting.md` §5.13). The plain-water record 0x2B0000CE and the plain-water-on-UV record 0x2B000A8D have the same declared ids, sampler names and extra state lists in their water permutations; they differ only inside the programs (§14) | OBS3; SH3; `lighting.md` L13 | Confirmed |
| W47 | In every water permutation of 0x2B0005EC, 0x2B0007FD and 0x2B000810 the only sampler with a non-zero default texture is `NoiseTexture` (source 0, no binding, default 0x400163D4: 9 + 9 + 5 entries); no other sampler of the 510 shader records holds 0x400163D4 (§8.6.1, §14) | OBS3 | Confirmed |
| W48 | The view depth that the depth, glitter-fade and layer-speed terms of the flow-map programs read is the clip-space *w* of the vertex position, interpolated to the pixel; the depth texture is compared with it in the same units (§8.6.2 step 7, §8.6.4, §14) | SH3 (Direct3D 10/11 programs of 0x2B0007FD and 0x2B0005EC) | BIN |
| W49 | `Render_DepthBias_Type` (PID 0x7F2) is one of the properties the entity's render component watches; it learns the value through the property-change notifications of the entity's layered property collection, as `Render_LODClass` (`walker.md` §11.4.3), so the value is the first one the entity's lookup order finds (`entity-descriptor.md` §9.2, E26); the component stores the delivered 32-bit integer unchanged, with no range check (§14) | BIN5 (o) | BIN |
| W50 | Census of `Render_DepthBias_Type` by where it is found: only on Physics SEDs or their objects (52,028 of 2,380,699); every one of them has an instance property set; 49,338 carry it in both their own set and their instance set, always with the same value; 2,690 find it only along the 0x47 chain; 1 carries it in its own set and along the chain with different values (row 4, chain 3); no Weenie SED, 0x70 object or `PhysObj` chain of one, and no SED of another type, carries it. The client's order and Tessera's order (instance set, own set, chain) give the same value for every SED (§14) | OBS4 | Confirmed |
| W51 | One graphics back end builds a rasteriser-state object per key of fill mode, cull mode, depth-bias index (key bits 4–7, so 0–15) and two flags, and fills it with all three values of the table entry (absolute, slope-scale, maximum; PIDs 0x7F3, 0x7F5, 0x7F4); the current index is set by a device call and read when a draw builds the key; which draws of a placed entity set the index from its render component was **not found** (no reader of the stored value and no matching call site in the session's search) (§14) | BIN5 (p), (q) | BIN; consumer OPEN |
| W52 | Every one of the 90 Direct3D 10/11 permutations of 0x2B0007D0 has a vertex program without a colour input; 88 of its pixel programs (all but the two depth-only ones of pass 0x09) and 34 of its 36 OpenGL pixel programs (all but the two of pass 0x09) discard when the alpha of the `ColorAndOpacity` texel, sampled at the distorted coordinates of §8.5.5 step 4, is at or below the reference, before any multiply; the output alpha is that texel alpha × the material alpha. Among the four liquid-distortion records, 0x2B000442 is the only one none of whose declarations has a binding with a semantic of 38–44 (its five transform bindings use 33–37 and all name its one permutation without programs) and the only one with no type-5 extra state item in any permutation (§8.5.5, §14) | SH4; OBS4 | Confirmed (programs; census of the 4 records) |
| W53 | An entity's scene node draws all its parts, in every pass it draws (the depth pass 0x09 and the shadow-map pass 0x0A included), with the depth-bias entry its render component holds, entry 2 in pass 0x0A when that entry is 0; the entry starts at 0; own-program surfaces (water, mirror, liquid distortion, lava) are parts like any other (`decals.md` §6.5, DC26–DC28) (§8.5.4, §14) | BIN6 | BIN |
| W54 | Over all 510 shader records, rules (i) and (ii) of W52 select the same records (191 records satisfy both, 319 neither; a rule holds when the record has no binding with a semantic of 38–44, or no type-5 item); a binding with a semantic of 33–44 names a permutation with programs in exactly the 313 records whose permutations with programs hold a texture-transform item for matrix 0 (the W46 property, any item type); rules (i) and (ii) differ from that property in 8 records outside the liquid-distortion family (7 whose 38–44 bindings and type-5 items all sit in permutations without programs, and 0x2B0000B7, whose transforms use items of types 1–3 only) (§8.5.5, §14) | OBS5 | Confirmed |

---

<a id="s2"></a>
## 2. Where water comes from

| Source | Data | Drawn by | Volumes (swimming) | § |
|---|---|---|---|---|
| **Landblock water** (seas, large lakes and rivers at a flat level per landblock) | Region water table (0x16) + block-map keys 7 (type) and 4 (level) + the height grid's minimum | One quad per wet landblock (§8.1) with the type's surface record (0x31) | The type's volume entity, placed per wet landblock (§7.1) | §4, §6–§8 |
| **Placed water** (ponds, pools, water in interiors, fountains, falls, puddles) | Static entities in landblock info whose 0x47 chain carries a water mesh and, for many, liquid-volume monitors | Placed like any static entity (`entity-descriptor.md`), but each surface is shaded by the program family of its shader record (§8.5): water records (§8.3–§8.4, §8.6), the mirror record (puddles), liquid-distortion records, or two-layer records with animated texture matrices (waterfall sheets, foam, spouts) | The entity's own monitor boxes (§7.3) | §7.3, §8.5 |
| **Distant water** | The water pairs inside distant-landscape sector records (kind 5) | The same quads, built per 3 × 3-landblock sector (§9) | none | §9 |

Not water: the **fog-zone table** (0x17, slot 6). It is documented here (§5) because earlier
specs took it for the water table.

There is **no** water level in the height grid, in landblock properties or in any stored
property: the dictionary properties `WaterHeight` and `WaterType` have no stored value
anywhere (`environment.md` §7.4), and `Volume_WaterEntityOverRide` (a string the client
queries before creating the volume entity, BIN (d)) is stored in no record either (raw search
of every cell archive and of `client_gamelogic.dat`, OBS).

---

<a id="s3"></a>
## 3. Finding the tables (Confirmed)

The region descriptor (`environment.md` §3) holds one DID map per table kind. The client asks
each map for key 0 only (BIN (b)). Slots 5 and 6:

| Slot | Prefix | DB type (code, label) | Kind | Region 1 | Regions 2, 14 | Region 3 | Regions 4, 5 | Region 6 |
|---|---|---|---|---|---|---|---|---|
| 5 | 0x16 | 0x17 WATER_DESC | Water table (§4) | 0x16000000 | 0x16000004 | 0x16000005 | 0x1600000A | 0x1600000E |
| 6 | 0x17 | 0x16 FOG_DESC | Fog-zone table (§5) | 0x17000000 | 0x17000001 | 0x17000002 | 0x17000007 | 0x1700000A |

Note that the DID prefix and the DB type code are crossed: prefix 0x16 holds DB type 0x17 and
prefix 0x17 holds DB type 0x16 (BIN: the client registers DID range 0x16000000–0x16FFFFFF
for type 0x17 and 0x17000000–0x17FFFFFF for type 0x16; the water code loads the slot-5 DID
as type 0x17 and the fog code loads the slot-6 DID as type 0x16). The type labels are data
(enum mapper "DBOType", `property-system.md` §9). The full prefix → type table is in
`records.md` §2.1.

The DIDs above are observations; a reader takes them from the region descriptor at run time.
All ten records are in `client_general.dat` and start with their own DID.

---

<a id="s4"></a>
## 4. Water table (prefix 0x16, DB type 0x17)

<a id="s4-1"></a>
### 4.1 Record layout (Confirmed, 5 of 5, zero leftover bytes)

All integers little-endian, packed. VLE is `records.md` §1.2.

| Offset | Size | Type | Field | Meaning | Observed |
|---|---|---|---|---|---|
| 0 | 4 | DID | `did` | Own DID (0x16…) | |
| 4 | 4 | u32 | `reserved` | Read into the table object; no reader of it was found (BIN) | 0 in all 5 |
| 8 | 4 | u32 | `level_key` | Block-map key whose value is the **water level** (§6.1) | 4 in all 5 |
| 12 | 4 | u32 | `type_key` | Block-map key whose value is the **water type** (§6.1) | 7 in all 5 |
| 16 | 1 | bool8 | `has_default` | A default entry follows | 1 in all 5 |
| 17 | 33 or 5 | entry | `default` | Water-type entry (§4.2), used when the type matches no listed entry | key 0, `dry` = 1 in all 5 (5 bytes) |
| … | 1–5 | VLE | `count` *n* | Number of listed entries | 47, 19, 23, 31, 5 |
| … | | entry × *n* | `entries` | §4.2, back to back | keys 1 … *n* in stored order in all 5 |

<a id="s4-2"></a>
### 4.2 Water-type entry (Confirmed layout; roles per field)

| Order | Size | Type | Field | Meaning | Observed (125 listed entries) |
|---|---|---|---|---|---|
| 1 | 4 | u32 | `key` | Water-type number; compared with the block map's type value (§6.2) | 1 … *n* |
| 2 | 1 | bool8 | `dry` | 1 = this type means "no water" and **nothing else follows**; 0 = fields 3–5 follow. A reader rejects other values (BIN). | 0 in all 125; 1 in the 5 default entries |
| 3 | 4 | DID | `surface` | Surface record (prefix 0x31, `material.md` §4) of the water surface (§8.2) | 76 distinct, all exist |
| 4 | 16 | 4 × DID | `extra` | Four further DIDs. The client preloads them with the others and copies them into the landblock's water state, but no reader of them was found (BIN). | 0 in all 125 |
| 5 | 4 | DID | `volume_entity` | Entity descriptor (prefix 0x47) of the water-volume entity (§7.1) | 0x47000754 (120), 0x470025A2 (2), 0x47008188 (3) |
| 6 | 4 | f32 | `value` | Copied into the landblock's water state; no reader found (BIN) | 24.0 in all 125 |

Entry size: 5 bytes when `dry` = 1, 33 bytes when `dry` = 0. When the client creates an entry
before reading it, it fills the DIDs with its "no DID" value, `value` with 24.0 and `dry` with 0
(BIN); a reader never needs these defaults because every field it uses is stored.

> **OPEN:** Meaning of the four `extra` DIDs and of `value` (24.0 everywhere). Neither
> affects anything in this install.

<a id="s4-3"></a>
### 4.3 Census (Confirmed)

| Record | Regions | Size (bytes) | Entries | Lava entries (`volume_entity` 0x47008188) | Entries with 0x470025A2 |
|---|---|---|---|---|---|
| 0x16000000 | 1 | 1,574 | 47 | – | keys 23, 43 |
| 0x16000004 | 2, 14 | 650 | 19 | – | – |
| 0x16000005 | 3 | 782 | 23 | key 21 | – |
| 0x1600000A | 4, 5 | 1,046 | 31 | key 21 | – |
| 0x1600000E | 6 | 188 | 5 | key 5 | – |

The 76 distinct surface records use six shader records (§8.2).

<a id="s4-4"></a>
### 4.4 Decoder rules and hostile input

- Consume exactly: DID, three u32, `has_default`, optional default entry, VLE count, entries.
- `has_default` and `dry` are bool8 (0 or 1); reject other values.
- Bound *n* by the bytes left (an entry is at least 5 bytes).
- Keys need not be sorted or dense; build a key → entry index. When two listed entries share a
  key, the client uses the **first** in stored order (BIN: its look-up walks the list).

---

<a id="s5"></a>
## 5. Fog-zone table (prefix 0x17, DB type 0x16)

<a id="s5-1"></a>
### 5.1 Record layout (Confirmed, 5 of 5, zero leftover bytes)

| Offset | Size | Type | Field | Meaning | Observed |
|---|---|---|---|---|---|
| 0 | 4 | DID | `did` | Own DID (0x17…) | |
| 4 | 4 | u32 | `reserved` | | 0 in all 5 |
| 8 | 4 | u32 | `zone_key` | Block-map key whose value selects the fog zone (§5.3) | 3 in all 5 |
| 12 | 1 | bool8 | `has_default` | A default entry follows | 1 in all 5 |
| 13 | varies | entry | `default` | §5.2 | key 0 |
| … | 1–5 | VLE | `count` *n* | | 9 (0x17000000), 0 (the other four) |
| … | | entry × *n* | `entries` | §5.2 | keys 1 … 9 |

Records are 137 bytes (default only) or 1,244 bytes (0x17000000).

<a id="s5-2"></a>
### 5.2 Fog-zone entry (Confirmed layout; meanings OPEN unless stated)

A "colour" is a u32 0xAARRGGBB stored little-endian (`environment.md` §5.6). An "override
triple" is 9 bytes: u8 `mode`, u32, u32.

| Order | Size | Type | Field | Observed (10 entries) |
|---|---|---|---|---|
| 1 | 4 | u32 | `key` | 0 (defaults), 1–9 |
| 2 | 63 | 7 × override triple | `overrides` | modes 3 in all default entries; 3, 4 or 6 in the others; both u32 always 0 |
| 3 | 4 | f32 | `near` | 9,000 (region 1 default), 500 (other defaults), −50 … 50 (zones) |
| 4 | 4 | f32 | `far` | 10,000 (defaults), 50 … 1,600 (zones) |
| 5 | 4 | f32 | `amount` | 0 (defaults), 0 … 0.9 (zones) |
| 6 | 4 | f32 | `value_60` | 1.0 in all |
| 7 | 1 | u8 | `active` | 0 in the defaults, 1 in every zone. The client uses a zone only when this is non-zero (BIN). |
| 8 | 4 | colour | `colour` | white in the defaults; nine different opaque colours in the zones |
| 9 | 4 | f32 | `light_a` | 1.0 (0.7 in one zone) |
| 10 | 4 | colour | `colour_a1` | |
| 11 | 4 | colour | `colour_a2` | Present when the archive's version for this type is ≥ 4; otherwise a copy of `colour_a1` (BIN). Present in this install. |
| 12 | 4 | f32 | `light_b` | 1.0 (0.8 in one zone) |
| 13 | 4 | colour | `colour_b` | |
| 14 | 4 | colour | `colour_c` | Version ≥ 2 (present) |
| 15 | 1 | bool8 | `flag_c` | Version ≥ 2 (present); 0 in all |
| 16 | 4 | colour | `colour_d` | |
| 17 | 4 | DID | `did_e` | Version ≥ 3 (present); 0 in all |
| 18 | 0 or 4 | u32 | `value_e` | Present only when `did_e` ≠ 0; absent in all |
| 19 | 1 | bool8 | `flag_f` | 0 in all |
| 20 | 1 | u8 | `value_g` | 0 in all |

Every entry of this install is 123 bytes: the 4-byte key and 119 bytes of fields 2–20 (the
version-dependent fields are all present; `value_e` is absent). Field names are Tessera's.
The shape of fields 3–16 (a fog near and far distance, a fog amount, a fog colour, then light
brightness and colour pairs) parallels the day-description keyframe of `environment.md` §5.3;
the seven override modes plausibly say, per channel, whether the zone's value replaces,
scales or leaves the sky's value (Inferred).

> **OPEN:** Meaning of the override modes (3, 4, 6), of fields 9–20, and of the archive
> version that selects the optional fields (the client asks the archive for a per-type
> version; a reader that assumes the newest layout reads every record of this install).

<a id="s5-3"></a>
### 5.3 How the client uses the fog-zone table (BIN)

For an outdoor landblock (X, Y): read the block-map value of `zone_key` at the landblock
itself, at (X, Y + 1), at (X + 1, Y + 1) and at (X + 1, Y). For each value, take the entry
with that key (first match), else the default. If **none** of the four entries has `active`
≠ 0, the landblock has no fog zone and the sky's fog applies unchanged (`environment.md`
§11.3). Otherwise the client copies the four entries as the landblock's four corner values
and notes whether all four values are equal; how it blends the corners across the block and
with the sky's fog is OPEN (bilinear across the block is the natural reading).

<a id="s5-4"></a>
### 5.4 Census (Confirmed)

Block-map key 3 is 0 in all 390,150 block maps, so every landblock selects the default
entry, whose `active` is 0: **no fog zone is active anywhere in this install**. The nine
zones of 0x17000000 are unused data.

**Tessera rule (Decision):** decode the table and ignore it while key 3 is 0 everywhere.

---

<a id="s6"></a>
## 6. Landblock water

<a id="s6-1"></a>
### 6.1 The block-map keys (Confirmed)

The water table's `type_key` (7) and `level_key` (4) are **block-map keys** (`landblock.md`
§5). The value used is the raw u16 stored in the block map for that key; no 0x18 descriptor
palette is involved (no descriptor lists keys 4 or 7). Observed values:

| Region | Water table | Type values (key 7) | Level values (key 4) |
|---|---|---|---|
| 1 | 0x16000000 | 0–47 (0 in 57,027 blocks) | 0–255, 111 distinct |
| 2 | 0x16000004 | 0–19 and 37 (0 in 40,124) | 0–255, 182 distinct |
| 3 | 0x16000005 | 0–22 (0 in 1,975) | 0–151, 98 distinct |
| 4 | 0x1600000A | 0–24 (0 in 4,250) | 0–170, 119 distinct |
| 5 | 0x1600000A | 0–31 (0 in 6) | 0–163, 107 distinct |
| 14 | 0x16000004 | 0–11 (0 in 64,376) | 0–211, 40 distinct |

Type value 37 in region 2 (one block) names no entry of 0x16000004 and falls to the dry
default.

<a id="s6-2"></a>
### 6.2 Deciding a landblock's water (algorithm in words; BIN, Confirmed by data)

Inputs: the landblock (R, X, Y); its region's water table *W* (§3); its block map; its height
grid.

1. If the region has no water table, the landblock has no water.
2. Read the block-map value *t* of *W*.`type_key`. If the block map lacks that key, the
   landblock has no water.
3. Choose the entry: the first listed entry whose `key` = *t*; if none, the default entry; if
   there is no default either, the landblock has no water.
4. If the chosen entry's `dry` = 1, the landblock has no water. (Type 0 always lands here in
   this install: no table lists key 0, and every default is dry.)
5. Read the block-map value *L* of *W*.`level_key`. If the block map lacks that key, the
   landblock has no water.
6. Water height *h*, in world units of the landblock frame: convert *L* to a 32-bit float,
   multiply by 1000.0, then multiply by the 32-bit float nearest 1/255
   (0.003921568859368563), rounding each product to 32 bits. So *h* = *L* × 1000 / 255 within
   one unit in the last place: level 38 → 149.0196, level 86 → 337.2549, level 255 → 1,000.0.
7. Let *m* be the lowest of the 1,089 height samples of the block, converted with
   `landblock.md` §3.4. The landblock **has water** exactly when *h* > *m* (strictly). The
   highest sample plays no part.
8. If it has water, its water state is: the water type *t*, the entry's `surface`, `extra`,
   `volume_entity` and `value`, and the **water plane** z = *h* over the whole block
   (normal +Z), §6.3.

The client evaluates this once per landblock when it loads the landblock data (BIN). A
ceiling grid (`landblock.md` §3.5) does not enter the rule.

<a id="s6-3"></a>
### 6.3 The water plane and "under water" (BIN)

The client stores the plane as a normal (0, 0, 1) and a distance −*h*, so the signed distance
of a landblock-frame point *p* is *p*.z − *h*.

- **Under landblock water:** *p*.z − *h* < −0.0002. Points within 0.0002 of the surface count
  as not under water. The encounter pass (§10) and the client's "is this position under
  water" query use this test; when the test fails, the query goes on to a physics-world query
  (Inferred: for the liquid volumes of §7, so placed water counts too).
- The plane covers the whole 160 × 160 block, also where the terrain rises above it; a
  consumer that needs "water at this point" compares *h* with the terrain height there
  (scenery does, `scenery.md` §7.5).

> **OPEN (implementer, 2026-10-06, `tessera-world` water module):** with the Inferred
> second step (liquid volumes), a point within 0.0002 below *h*, or exactly at *h*, is
> "under water" after all: the volume entity's surface box (§7.1) reaches up to *h*
> inclusive. Whether the client's physics-world query uses inclusive box bounds, and so
> whether the 0.0002 tolerance of the plane test has any effect on the full query, is not
> known. `tessera-world` keeps both steps as stated (plane test, then boxes with inclusive
> bounds, the containment rule of `tessera-physics`).

<a id="s6-4"></a>
### 6.4 Census: where this install has water (Confirmed, 2026-10-05)

| Region | Landblocks | Dry type (entry dry) | Typed, level not above the lowest sample | **Wet** | Wet, level at or above the highest sample (fully submerged) | Wet with a ceiling |
|---|---|---|---|---|---|---|
| 1 | 65,025 | 57,027 | 3,225 | **4,773** | 1,634 | 14 |
| 2 | 65,025 | 40,125 | 20,885 | **4,015** | 848 | 398 |
| 3 | 65,025 | 1,975 | 32,794 | **30,256** | 25,328 | 8 |
| 4 | 65,025 | 4,250 | 35,611 | **25,164** | 18,860 | 411 |
| 5 | 65,025 | 6 | 39,570 | **25,449** | 20,505 | 152 |
| 14 | 65,025 | 64,376 | 393 | **256** | 0 | 17 |
| **All** | 390,150 | | | **89,913** | 67,175 | 1,000 |

Largest groups (type, level → height; landblock range; two example DIDs of the landblock
data records):

| Region | Type | Level → *h* | Wet blocks | X range | Y range | Examples |
|---|---|---|---|---|---|---|
| 1 | 29 | 86 → 337.255 | 1,353 | 42–121 | 170–197 | 0x80012AAB, 0x80012AAC |
| 1 | 37 | 86 → 337.255 | 899 | 47–224 | 65–199 | 0x80012F68, 0x80012F69 |
| 1 | 1 | 21 → 82.353 | 583 | 0–64 | 0–64 | 0x80010001, 0x80010002 |
| 2 | 11 | 96 → 376.471 | 880 | 52–179 | 14–222 | 0x800234D7, 0x800235D7 |
| 3 | 11 | 38 → 149.020 | 19,084 | 0–254 | 61–254 | 0x80030043, 0x80030044 |
| 4 | 11 | 38 → 149.020 | 16,330 | 0–254 | 0–254 | 0x80040005, 0x80040006 |
| 5 | 23 | 38 → 149.020 | 23,747 | 0–254 | 0–254 | 0x80050000, 0x80050001 |
| 14 | 11 | 96 → 376.471 | 37 | 3–21 | 170–184 | 0x800E03B8, 0x800E04B8 |

Level 38 (149.02 units) is the sea level of regions 3–5; land blocks there stand at 150
units or more and so stay dry although their type is set. Lava (§7.2) covers 121 wet
landblocks in region 3 and 535 in region 4 (type 21).

These are observations; an implementation computes water with §6.2 from the data and never
hard-codes a table of wet landblocks.

<a id="s6-5"></a>
### 6.5 Validation against placed objects (Confirmed)

Placed static entities whose world-building label names floating water plants: of the 1,067
that stand in wet landblocks, the median of *z* − *h* is −0.007 units; 543 lie within 5 cm of
*h* and 917 within 0.5 units. (The other 1,326 stand in landblocks without landblock water;
placed water, §7.3, is the Probable reason.)
The distant-sector water pairs (§9) give an independent check of §6.2 on every wet landblock.

---

<a id="s7"></a>
## 7. Water volumes

<a id="s7-1"></a>
### 7.1 The landblock water-volume entity (placement BIN; properties Confirmed)

For every wet landblock, when its static entities are created, the client creates one
client-side entity from the water type's `volume_entity` at block-local position (0, 0, *h*)
(BIN (d); the code sets no rotation or scale, Inferred: identity and 1). The entity is not
drawn as water (its setup holds a single-triangle mesh); its purpose is its **monitor
volumes**: its 0x47 record carries
`Entity_Monitors`, an array of `Entity_Monitor` structs, each an axis-aligned box in the
entity's frame (`Monitor_BoxMin`, `Monitor_BoxMax`, `Monitor_ScaleWithEntity` true) that
applies a property set (`Monitor_VolumePropertiesToApply`, a 0x78 DID) to objects inside it.
`Monitor_Name` uses enum mapper 0x23000096 (1 WaterSurfaceVolume, 2 WaterVolume, 3
WaterWadeVolume; labels are data).

| Entity | Monitor | Box min (x, y, z) | Box max (x, y, z) | Property set |
|---|---|---|---|---|
| 0x47000754 (water) | surface (1) | (0, 0, −0.5) | (160, 160, 0) | 0x78000075 |
| | wading (3) | (0, 0, −1) | (160, 160, −0.5) | 0x78000074 |
| | deep (2) | (0, 0, −1,600) | (160, 160, −1) | 0x78000073 |
| 0x470025A2 (water, deep only) | deep (2) | (0, 0, −1,600) | (160, 160, 0) | 0x78000073 |
| 0x47008188 (lava) | surface (1) | (0, 0, −0.5) | (160, 160, 0) | 0x78005CDF |
| | wading (3) | (0, 0, −1) | (160, 160, −0.5) | 0x78005CE0 |
| | deep (2) | (0, 0, −1,600) | (160, 160, −1) | 0x78005CE1 |

Relative to the water surface *h*: the surface zone is the top 0.5 units, the wading zone
0.5–1 unit below the surface, the deep zone from 1 unit down to 1,600 units below.

> **OPEN (implementer, 2026-10-06, `tessera-world` water module):** three monitor fields
> are not covered here: (a) `Monitor_Offset` (vec3, on 382 of 464 monitors per `sound.md`
> §10.3.4): whether and how it moves the box. Tessera (Decision): the box is
> (min + offset, max + offset) in the entity frame, before the entity's scale, rotation and
> position. With this rule the boxes of all three landblock volume entities of this install
> still cover the whole 160 × 160 block from *h* down (checked by the `tessera-world` test
> `volume_entities_cover_their_landblock`). (b) A monitor without
> `Monitor_ScaleWithEntity`: Tessera treats it as true (scales with the entity).
> (c) Whether `Entity_Monitors` is inherited along the 0x47 parent chain: Tessera reads it
> from the nearest level of the chain like other object properties (`scenery.md` §9.2); in
> this install no placed volume entity inherits its monitors.
>
> **Partial answer (2026-10-08, BIN, W20):** (a) the client adds `Monitor_Offset` to both
> corners of the box, as the Decision does; where the entity's scale is applied relative to
> that sum was not traced. (b) and (c) were not traced; the Decisions stand.

<a id="s7-2"></a>
### 7.2 Volume property sets (Confirmed)

| Set | Properties (all boolean true) |
|---|---|
| 0x78000073 | `Volume_IsLiquid`, `Volume_WaterDeep`, `PathMap_IsWater` |
| 0x78000074 | `Volume_IsLiquid`, `Volume_WaterWading` |
| 0x78000075 | `Volume_IsLiquid`, `Volume_WaterSurface` |
| 0x78005CE1 | `Volume_IsLiquid`, `Volume_IsLava`, `Volume_LavaDeep`, `PathMap_IsWater` |
| 0x78005CE0 | `Volume_IsLiquid`, `Volume_IsLava`, `Volume_LavaWading` |
| 0x78005CDF | `Volume_IsLiquid`, `Volume_IsLava`, `Volume_LavaSurface` |

Three further sets carry `Volume_IsCoin` with coin surface, wading and deep flags (one each);
no landblock water type uses them.

These are the sets `collision.md` §10.4 names for the water movement states.

**Value types (BIN, W18; answers the implementer OPEN of 2026-10-07).** Every reader of these
flags that was traced (`Volume_IsLiquid` in the movement-state routine and two other places,
`Volume_IsLava`, `Volume_IsCoin`) counts a flag as set only when the value found is a Boolean
(type 21) and true; a value of another type reads as not set. The dictionary declares every
volume flag as Boolean, and each value is decoded with that type (`property-system.md` §5.3),
so a value of another type does not occur in data read with the dictionary. The implementer's
Decision is the client's behaviour.

<a id="s7-3"></a>
### 7.3 Placed water volumes (Confirmed census)

48 entity descriptors (0x47) carry monitors with the water or lava sets above: the three of
§7.1 and 45 others. Placed static entities reference 37 of those (3,895 entities; the most used
two are referenced 1,017 and 569 times; 0x470025A2 is also placed twice by hand). Their boxes
are entity-local and smaller (for example ±15 × ±15 units with the same three depth bands, or
±8 × ±8 with bands near the surface). Apart from the three landblock volume entities, all of
them have a real mesh (4 to 12,880 vertices; regular grids such as 7 × 7 and 9 × 13 vertices
are common), drawn with the entity (`Translucent` true and render phase 2 on the most used
ones). They are placed by their static-entity frame (`entity-descriptor.md`) and scaled with
the entity when `Monitor_ScaleWithEntity` is true.

> **OPEN (implementer, 2026-10-06, `tessera-world` real-data census):** the three coin sets
> of §7.2 also carry `Volume_IsLiquid` (their flags read as liquid without a water or lava
> depth flag). Two placed 0x47 objects use them (19 placements). Counting every placed
> Physics entity whose object has monitors applying a `Volume_IsLiquid` set gives 3,914
> entities and 39 objects; without the coin sets it gives the 3,895 and 37 above. Whether a
> coin volume counts as water for movement (§7.4) and for the "under water" query is not
> known. Tessera (Decision): every `Volume_IsLiquid` box is liquid, as the rule of §7.4
> states.
>
> **Answer (2026-10-08, BIN, W19; reworded 2026-10-09, W22):** both of the client's routines
> that set the movement state read the mover's `Volume_IsLiquid` (§7.4), and that flag is set by
> coin sets as well, so a coin volume counts as water for the movement state, as the Decision
> says. After a controller update the in-water input also needs the water surface at least the
> float height minus 0.0002 m above the feet (W22); the routine of the position-update paths
> reads `Volume_IsLiquid` alone (W19). Whether the
> "is this position under water" query (§6.3, §10) counts it was not traced; the Decision
> stands for that query.

<a id="s7-4"></a>
### 7.4 From volumes to movement states (state rule BIN; containment and buoyancy OPEN)

`collision.md` §10.4 lists the mover states Water_ControlledSurface (1), Water_NoSurface (2) and
Water_UncontrolledSurface (3).

**Client rule (BIN, W19, W22; revised 2026-10-09).** The monitor volumes above apply their
property sets to the movers inside them, so a mover inside a liquid zone has `Volume_IsLiquid`
true. The client sets `Entity_Element` from an in-water input and the support flags of
`collision.md` §11.3.1: in water, 1 on walkable support, 3 on other support and 2 without
support; out of water 5, 6 and 4 (`collision.md` §11.3.2). Two routines apply this table
(BIN3 (j)):

- **After every update of the player controller** (W22): the mover is in water when its
  `Volume_IsLiquid` is true **and** the water surface found by the controller lies at least its
  float height minus 0.0002 m above its feet. A mover in a liquid volume whose feet are less than
  its float height below the surface is out of water (5, 6 or 4); one on walkable ground deeper
  than that is in state 1, however deep.
- **When a mover is placed at a position handed to it** (the position-update paths, W19): the
  mover is in water when its `Volume_IsLiquid` is true; depth and float height are not inputs.

For a walking or swimming player the first routine decides the state from its first update
on. Which point or shape of the mover the monitors test, how the controller's water-surface
query is formed, and how the float height keeps a swimmer at its depth (the client's buoyancy)
were not traced (`collision.md` Q9, §11.7).

> **Answered (2026-10-09, BIN3 (j); was OPEN, merge of 2026-10-09):** the two readings of
> 2026-10-08 read two different routines and are both right for their routine (W19, W22;
> `collision.md` C64). The state of a moving player follows the controller's routine, whose
> in-water input includes the float-height test; step 1 of "Entering deep water at speed" below
> therefore stands, and the answer to the no-support OPEN below is corrected.

**Tessera rule (Decision):** a mover is in water when its feet are inside any liquid box; it
swims (state 2) when the water surface is more than its float height above its feet
(`collision.md` §11.2), otherwise it wades (state 1, or 3 on steep support); without support it
swims (the OPEN below). This differs from the client (after controller updates) in four
cases:

1. in water shallower than the float height, on walkable support: Tessera gives 1, the client
   5;
2. in water shallower than the float height, on support that is not walkable: Tessera gives 3,
   the client 6;
3. in water shallower than the float height, without support: Tessera gives 2, the client 4;
4. in water deeper than the float height, on any support: Tessera gives 2, the client 1 on
   walkable support and 3 on support that is not walkable.

Without support in water deeper than the float height both give 2. Tessera keeps its test
because the client's way of lifting a swimmer off the bottom (the Swimming layer's upward
gravity of `collision.md` §11.1 step 9, applied in state 1 too, with a buoyancy that was not
traced) is not known, and Tessera's float line (`collision.md` §12.4 (p)) needs the deep-water
case to be a swimming state.

> **OPEN (implementer, 2026-10-06, `tessera-physics` controller):** the rule above does not
> say which state a mover in water **without** support takes when the surface is less than
> its float height above its feet (jumping or stepping off a ledge into shallow water).
> Tessera chooses state 2 (swimming) whenever there is no support. The feet are the
> mover's position (the bottom of its capsule, `collision.md` §10.6). While swimming, the
> controller keeps the feet from rising above the surface minus the float height and lets a
> swimmer above that line sink to it (with gravity −20 the swimming layer would otherwise
> lift the mover out of the water); the surface is the water plane's level where the liquid
> boxes end there, else the top of the liquid boxes above the feet.
>
> **Answer (2026-10-08, BIN, W19; corrected 2026-10-09, BIN3 (j), W22):** after a controller
> update the client gives state 2 to a mover without support only when the water surface is at
> least its float height minus 0.0002 m above its feet; in shallower water it gives state 4
> (out of water, falling). Only a mover just placed by the position-update paths gets state 2
> whatever the depth. Tessera's choice of state 2 is therefore the client's state in deep water
> and differs in shallow water (the first difference listed under the Tessera rule above). The
> float-line handling stays a Tessera Decision (the client's buoyancy was not traced).

**Water surface for the float line (answers the implementer OPEN of 2026-10-07).** The client
computes no "surface above the feet" for the state (rule above, BIN); its buoyancy was not
traced. The surface that Tessera's float line uses is therefore Tessera's own construction, and
the implementer's rule is confirmed as the Tessera Decision: the surface is the top of the
**contiguous** liquid above the feet, computed exactly from the heights at which the vertical
line through the feet is inside each `Volume_IsLiquid` box. Starting at the feet, each box
whose span begins at most 1 mm above the top found so far extends it, and a larger dry gap ends
the liquid. When a landblock water plane lies over the point and its level is within 1 mm of
that top, the surface is the plane's level (the liquid ends at the plane, §6.3). The 1 mm
tolerance absorbs the rounding of boxes placed in single precision. Example: feet at 0 with
boxes from −1 to 0.6 and from 0.8 to 5 give the surface 0.6. (The released doubling-probe
search is replaced.)

> **OPEN (implementer, 2026-10-06, controller review):** `collision.md` §11.1 step 9 applies
> the Swimming layer (gravity −20) in states 1, 2 and 3. How its upward gravity interacts
> with support in states 1 and 3 is not traced (`collision.md` Q9). Tessera (Decision): in
> state 1 the mover stands on walkable support, so its vertical speed is 0. In states 2 and 3
> the controller uses a **float line**, the surface minus the float height. Below the line
> or on it, the layer's gravity applies and the feet stop at the line, with the upward speed
> zeroed. Above the line, the magnitude of the gravity (at least 1 m/s²) pulls the mover down,
> and the feet stop at the line with the speed zeroed, so a swimmer does not overshoot it.
> As a result, a mover dropping or stepping into deep water settles at the line without
> sinking below it, and a mover on steep support in water shallower than its float height
> slides down instead of being lifted. The original's buoyancy and its integration are not
> known.


> **OPEN (implementer, 2026-10-06, walker walk mode):** whether a mover inside an interior
> cell (`collision.md` §5.8.1) can be in landscape water is not stated. The landblock volume
> entity's boxes reach far below the water plane (§7.1), so a dungeon cell under a lake lies
> inside them: walk IN1 of `m4-5-visual.md` (a cell 660 m below a lake's plane) swam up out
> of its cell. Tessera (Decision, `walker.md` §13.5): no landscape water while the cell
> lookup at the feet + 0.05 m finds a cell. Please state the client's rule (BIN), and
> whether placed volumes inside cells (§7.3) still apply there.

**Entering deep water at speed (BIN2, W21; step 3 sign made explicit 2026-10-09).** This answers the clean-room OPEN of the M4.5
exit verification (2026-10-06), which asked how the original slows a mover that enters deep
water at speed. The original has **no** rule that cancels or limits a downward speed in
water. Its player controller (`collision.md` §11.6) slows such a mover only through the
parameters of the Swimming layer:

1. The mover counts as in water at the end of an update when its `Volume_IsLiquid` is true
   and the water surface lies at least its float height minus 0.0002 m above its feet; its
   state then becomes 1, 3 or 2 by its support (walkable, other, none) (`collision.md` §11.6
   item 7, §11.3.2 (a); W22). The update in which this happens still ran with the previous layers.
2. From the next update on, the layering adds Swimming (StealthedSwimming when stealthed)
   (`collision.md` §11.1 step 9); in this install that gives gravity −20 m/s² and a damper
   of 0.4 on each world axis (`collision.md` §10.3; read at run time).
3. Every update subtracts Gravity × *t* from the vertical part of the physics velocity, then
   multiplies each world component by the damper, once per update whatever *t* is; the
   displacement uses the result (`collision.md` §11.6 items 3–4). Let *v*_z be the vertical
   component of the physics velocity, positive **upwards** (so *v*_z < 0 while the mover falls;
   falling at 15 m/s is *v*_z = −15 m/s). With Gravity −20 m/s² each update makes
   *v*_z ← 0.4 · (*v*_z + 20 *t*): the Swimming layer's negative gravity pushes upwards, and
   the damper shrinks what is left. A falling mover's downward speed |*v*_z| thus becomes
   0.4 · (|*v*_z| − 20 *t*) per update while that is positive (example below). The horizontal parts of the physics velocity (for
   example speed gained sliding down a bank) shrink by the same factor; the control velocity
   is not damped.
4. A falling displacement is never changed: nothing stops the mover at the float line or at
   the surface on the way down. Only a displacement that **rises** (and not during a jump) is
   replaced so that the feet end at the float line in that update, however far below it they
   are (the move of `collision.md` §11.4 still sweeps that displacement); the vertical physics speed is set to 0 when the line was already within 0.0002 m of
   the feet (`collision.md` §11.6 item 4).

Example (Swimming values of this install, a mover flagged in water while falling at
15 m/s): at 60 updates per second it sinks a further 0.098, 0.037, 0.013 and 0.003 m in the
next four updates (0.15 m in all) and is put on the float line in the fifth; at 30 updates
per second it sinks 0.19, 0.07 and 0.02 m (0.28 m) and is put on the line in the fourth.
Because the in-water test needs the feet at or below the float line, and the update that
sets it moved at the old speed, the feet may already be up to one update's fall below the
line when it starts (0.25 m at 15 m/s and 60 updates per second). The damper acts once per
update, so the depth depends on the update rate; at Tessera's 1/60 s step the 60-per-second
figures apply.

**Tessera's current rule (Decision, until aligned).** `collision.md` §12.4 (p): the water
takes up any downward speed and displacement at once, so the feet stop at the float line; a
mover that reaches deep water while sliding (state 6, `collision.md` §11.3 mechanism) has its
vertical speed zeroed there and takes state 2. This differs from the original, in which the
mover dives briefly below the line (example above), takes state 1, 2 or 3 by its support, and
is then placed on the line. `collision.md` §12.5 has no row for this difference yet (its
"Rising in water" row covers step 4 only); aligning (p) with steps 1–4 belongs to that
section's implementer OPEN.

---

<a id="s8"></a>
## 8. Drawing water

<a id="s8-1"></a>
### 8.1 Landscape water geometry (BIN; Tessera rule for the near field)

The only code found that turns a water table's `surface` into geometry is the distant-landscape
sector builder (BIN (e)). For a sector of 3 × 3 landblocks (§9) and for each entry of the
sector's water list, it builds one mesh holding a quad for every landblock of the sector that
has water of that entry:

- corners at block-local (0, 0), (160, 0), (0, 160), (160, 160) offset by the landblock's
  position in the sector (160 units per landblock), all at height *h*;
- vertex normal straight up;
- texture coordinates (*i* / 3, 1 − *j* / 3) over the sector, *i* and *j* = 0 … 3 the corner's
  landblock-edge index along X and Y;
- two triangles per quad, drawn with the entry's surface record.

No separate near-field water mesh builder was found (OPEN whether the original uses these
sector quads at every distance; it is the Probable reading). **Tessera rule (Decision):** at
every distance draw, for each wet landblock of §6, one 160 × 160 quad at *h* with the water
type's surface record, texture coordinates as above taken from the landblock's place in its
3 × 3 sector (sector origin at landblock X − X mod 3, Y − Y mod 3); subdivide it into a grid
(for example 32 × 32 quads) only if vertex waves (§8.3) are wanted. Terrain above the plane
hides the water through the depth test; no shoreline clipping is needed.

<a id="s8-2"></a>
### 8.2 Material chain (Confirmed)

Water surface record (0x31, `material.md` §4) → shader record (0x2B) + materials (0x30) →
effective parameter set (`material.md` §9). The 76 water surfaces use six shader records:

| Shader record | Water surfaces | Samplers of its water programs |
|---|---|---|
| 0x2B0000A8 | 43 (of the 55 surface records that name it) | normal map; frame-buffer copy (refraction); reflection; depth (some permutations); an interactive ripple normal map (some) |
| 0x2B0007FD | 21 | as above plus flow map (3075), foam map (3079) and a noise texture |
| 0x2B0000CE | 5 | normal map; frame-buffer copy; depth (some); ripple map (some) |
| 0x2B0005EC | 5 | normal map, flow map, noise, frame-buffer copy, reflection, depth |
| 0x2B000559 | 1 | normal map, frame-buffer copy, reflection, environment cube map, depth |
| 0x2B000810 | 1 | two colour-and-opacity layers, emissive, flow, foam, noise (a normal map is bound but not sampled; no frame-buffer copy, reflection or depth; corrected 2026-10-09, W41) |

Every one also has a low-detail permutation that samples two colour textures (3000, 3001)
with the vertex texture coordinates, lights them per vertex and fogs them. The last record,
0x2B000810, draws lava, not water (§8.6.5).

<a id="s8-2-1"></a>
**Program families (added 2026-10-09; SH2, OBS2, W23).** The six records above do not share one
program. The formula of §8.3–§8.4 holds for the **plain water** family only; the flow-map
family has its own (§8.6). Every shader record of this install that declares a `c_Water…`
parameter (1044–1069, 1170–1173, 1182–1190, 2004–2011) or a flow or foam slot (3075, 3079)
belongs to one of the families below; the surface counts are of all 13,781 surface records.

| Family | Shader records (surface records: landblock-water table / placed) | Wet landblocks | Coordinates of the normal layers | Described in |
|---|---|---|---|---|
| Plain water | 0x2B0000A8 (43 / 12), 0x2B0000CE (5 / 5), 0x2B000559 (1 / 0) | 4,093 + 61 + 44 | world position, *B* × `Scale`ₖ + `Speed`ₖ × *T* | §8.3–§8.4 |
| Plain water on mesh UV | 0x2B00084B (0 / 1), 0x2B00089A (0 / 1), 0x2B000A8D (0 / 1) | 0 | mesh UV set 0, UV × `Scale`ₖ + `Speed`ₖ × *T* | §8.5.6 |
| Flow map | 0x2B0007FD (21 / 0), 0x2B0005EC (5 / 0) | 83,929 + 1,130 | world position for the normal map, mesh UV set 0 for the flow map | §8.6 |
| Lava (flow-map interface without a frame-buffer copy; revised 2026-10-09, W40, W41) | 0x2B000810 (1 / 0) | 656 | world position for the two colour layers and the emissive layer, mesh UV set 0 for the flow map and added to the emissive coordinates | §8.6.5 |
| Older bump water | 0x2B0000A9 (0 / 2), 0x2B0000DC (0 / 1) | 0 | mesh UV set 0 | §8.5.6 |
| Liquid distortion | 0x2B000442 (0 / 2), 0x2B0004B5 (0 / 8), 0x2B00063E (0 / 2); lit: 0x2B0007D0 (0 / 4) | 0 | mesh UV set 0 through texture matrix 0 (0x2B000442: world position) | §8.5.5 |
| Window (glass with a depth tint) | 0x2B0009E1–0x2B0009E5, 0x2B0009E8 (0 / 10) | 0 | – | not water; `material.md` §8.7 |
| Other | 0x2B000A8C (0 / 1): a two-layer additive effect that reads only `c_WaterOverbrightAmount` | 0 | – | `material.md` §10.3 |

So 85,059 of the 89,913 wet landblocks (95 %) are drawn by the flow-map family, not by the
formula of §8.3–§8.4.

> **Answered 2026-10-09 (SH2; was the OPEN below on undeclared parameters):** the records that
> lack parameters do not use them. 0x2B0005EC has no normal layers 1 and 2 and no waves in any
> program; 0x2B0007FD has no waves in any program and reads layers 1 and 2 in its Direct3D
> 10/11 programs only (§8.6.2, W27); 0x2B000810 (lava) has no waves and no normal layers at
> all (§8.6.5, W40). A renderer gives each record the formula of its family (§8.3–§8.6.5) and
> never fills an undeclared id with the 0x2B0000A8 default.

**Which permutation a statement covers (0x2B0000A8, Confirmed, W16).** Statements about
0x2B0000A8 elsewhere in the spec set describe different permutations of the same record:

| Permutations | Inputs and samplers | Lighting | Output and render state |
|---|---|---|---|
| Water permutations: 8 of the record's 13 (four Direct3D 11, all with a depth sampler, two of them also with the ripple map; two shader-model-2 and two OpenGL, without depth, one of each pair with a reflection sampler) | read only the position (the four Direct3D 11 vertex programs also declare a normal and one UV set in their input signature, unused; the program's only declared input register is the position); normal map, frame-buffer copy, optionally reflection, depth, ripple map | none from the light slots; only the fake directional light of §8.4 step 6 | §8.4; without a depth sampler source ONE, destination ZERO; with one, source alpha and inverse source alpha; cull byte NONE (both faces) |
| Low-detail permutations: the other 5 (one Direct3D 11 and one OpenGL with programs and the two colour samplers 3000, 3001; three without programs whose extra state lists describe two texture stages, `material.md` §7.4, §7.7) | position, normal and one UV set (three vertex inputs); the two colour layers through texture matrices 0 and 1 | ambient, hemispheric ambient, the per-vertex point-light array and the extra directional light (constants read by the OpenGL program; `lighting.md` §5, §7) | output alpha the material colour's alpha (2000); source alpha and inverse source alpha; cull byte CW (back faces culled) |

So "a two-layer, `Color`-role record lit per vertex and blended by 2000 alpha" (`material.md`
§20.1, `lighting.md` §4.3) is true of the low-detail permutations only, and "positions only,
frame-buffer composite" (§8.3–§8.4) of the water permutations only. Of the 55 surface records
that name 0x2B0000A8, 43 are water surfaces of the water tables and 12 are not; the latter are
drawn through whichever permutation the renderer selects (OPEN, §8.4).

The water parameters are the `c_Water…` labels of enum mapper 0x230001A6 (`material.md` §8.0;
flag-1 parameters feed the program constant of the same name, `material.md` §8.6). Those the
programs of 0x2B0000A8 read:

| Id | Label (interoperability) | Use (§8.3–§8.4) | Shader default in 0x2B0000A8 |
|---|---|---|---|
| 1044, 1045, 1046 | `c_WaterWaveSpeed`, `…WaveFrequency`, `…WaveAmplitude` | vertex waves | 1.0, 0.2, 0.0 |
| 1057–1062 | `c_WaterNormal0SpeedU` … `c_WaterNormal2SpeedV` | scroll of the three normal layers | 0.005, 0.0075, 0.015, 0.02, −0.01, −0.0125 |
| 1063–1065 | `c_WaterNormal0Scale` … `2Scale` | tiling of the three normal layers | 2, 4, 8 |
| 1047 | `c_WaterReflectionBumpDistortionAmount` | reflection offset | 0.1 |
| 1048 | `c_WaterRefractionBumpDistortionAmount` | refraction offset | 0.04 |
| 1050, 1051 | `c_WaterFresnelBias`, `…Exponent` | Fresnel | 0.1, 2.0 |
| 1054 | `c_WaterSpecularPower` | specular exponent | 80 |
| 1066 | `c_WaterOverbrightAmount` | colour scale | 1.0 |
| 1067 | `c_WaterFakeSpecularThreshold` | sparkle threshold | 0.25 |
| 1068 | `c_WaterFogAmount` | under-water tint of the refraction | 1.0 |
| 2006 | `c_WaterSpecularColor` | specular tint | white |
| 2009 | `c_WaterColor` | colour multiplier | white |
| 2010 | `c_WaterFogColor` | under-water tint colour | black |
| 2011 | `c_WaterShorelineColor` | near-camera refraction tint | white |
| 3011 | NormalMap | the normal texture of all three layers | |

Materials of individual surfaces override these as usual. Other declared water parameters
(1170–1173, 1182–1190, 2004, 2005, 2007, 2008) are read by the flow, foam and depth permutations; their
formulas were not traced (OPEN).

> **Answered 2026-10-09 (see "Program families" above):** Three of the six water shader records do not declare every parameter of the
> table above (clean-room M4.5 real-data run, 2026-10-06): 0x2B0005EC and 0x2B000810 lack
> 1044–1046 (waves), 1059–1062 and 1064, 1065 (normal layers 1 and 2); 0x2B0007FD lacks
> 1044–1046. `tessera-material` (`WaterMaterial`) fills an undeclared id with the
> 0x2B0000A8 default of this table and lists it as undeclared. Whether those records' programs
> use a single normal layer and no vertex waves (so a renderer following §8.3–§8.4 should
> skip layers 1 and 2 and the waves for them) is not stated.

<a id="s8-3"></a>
### 8.3 Vertex stage (SH, shader record 0x2B0000A8)

The water programs (all but the low-detail permutations) read **only the vertex position** (the Direct3D 11 ones also declare a normal and one UV set in their input signature but never read them, §8.2; a vertex layout that supplies position, normal and UV satisfies every permutation). With *P*
the world position (in the program's frame the vertical axis is the second component; below,
x and y are horizontal and z vertical as everywhere in Tessera), *T* the effect time in
seconds, and *B* = ((*P*.x, *P*.y) − *O*) / 160, where *O* is an engine-supplied horizontal
offset (the program constant `c_ContiguousWorldBlockOffset`; Inferred: the origin of the
renderer's current block grid), so *B* advances by one per landblock:

- **Normal-layer coordinates:** layer *k* (0, 1, 2) samples the normal map at
  *B* × `Scale`ₖ + (`SpeedU`ₖ, `SpeedV`ₖ) × *T*.
- **Waves:** the vertex height gets the sum over *k* = 0 and 1 of
  *A*ₖ · sin *s*ₖ, with *A*ₖ = `WaveAmplitude` × (1 − 0.5 *k*) and
  *s*ₖ = ((−1 + 0.3 *k*) · *x* + 0.7 *k* · *y*) · `WaveFrequency` · (*k* + 1)
  + `WaveSpeed` · *T* · (0.5 + 0.8 *k*) (the program reduces *s*ₖ to [−π, π) before taking
  the sine), using the vertex's **model** x and y. The surface normal is tilted by the
  matching derivatives. With the default amplitude 0 there are no waves; on a single quad
  per landblock they are not visible either.
- **Distance fog factor:** clamp((distance to the eye − fog near) / fog range, 0, 1), with the
  frame's distance-fog values (`lighting.md` §10).

**Tessera rule (Decision):** take *O* = (0, 0) in the region frame, so *B* is the region
position in landblock units and the normal layers continue seamlessly across landblocks.

**How the client fills *O* (added 2026-10-09; BIN4, W38).** *O* is one of the engine's
per-draw inputs (not a material parameter). The client rewrites it whenever a position is
handed to its world view (Inferred: the viewer's position; written so below): *O* = 160 m × (landblock *x*, *y* of a reference position − landblock *x*,
*y* of the viewer's position) − a horizontal offset stored with that reference. The reference
belongs to an object that the client keeps per region and replaces when the region changes; with
both positions in the same landblock the landblock term is (0, 0). So *O* changes in whole
landblocks as the viewer crosses landblock edges. Inferred (the frame of the vertex positions
was not traced): the world positions the programs receive are relative to the viewer's
landblock, so the change of *O* cancels the change of frame and *B* is a continuous function of
the world position measured from the per-region reference; the Tessera rule above has the same
property with a different reference. With constant `Scale`ₖ the choice of reference only
shifts the pattern. It becomes visible when a scale is a curve (§8.7.1), because the pattern
then moves at *B* × d`Scale`ₖ/d*t*.

> **OPEN (2026-10-09):** which position the per-region reference holds (the region's first
> landblock, the landblock where the viewer entered the region, or another) and what the stored
> horizontal offset is; whether the vertex positions are relative to the viewer's landblock; and
> what a second routine that also writes *O* (another rendering path) sets.
> Only the size of *B* depends on it, which matters for the two surfaces with curve-valued
> scales (§8.7.1).

**Time base and back ends (added 2026-10-09; W24, W25).** *T* is the engine's time input E20:
the global clock in seconds, reduced modulo 3,600 once it exceeds 3,600 (`lighting.md` L25),
not a frame count and not the game's time of day. The Direct3D 10/11 water programs of
0x2B0000A8 compute the same layer coordinates and waves as the OpenGL text. So for the plain
water family the layer *k* pattern moves over the ground at (`SpeedU`ₖ, `SpeedV`ₖ) × 160 /
`Scale`ₖ metres per second (one normal-map repeat covers 160 / `Scale`ₖ m); the rates of the
landblock-water surfaces are tabled in §8.7. These statements, and §8.3–§8.4 as a whole,
cover the plain water family only (§8.2 "Program families"); the flow-map records, which
draw 95 % of the wet landblocks, follow §8.6.

<a id="s8-4"></a>
### 8.4 Pixel colour (SH, shader record 0x2B0000A8)

1. **Normal:** sample the normal map at the three layer coordinates, map each sample to
   2 · (rgb − 0.5), add the three, normalise (*n*, tangent space), and rotate *n* into world
   space with the surface frame. Keep the first two components of the normalised
   tangent-space *n* (before the rotation) for the sparkle term of step 6.
2. **Screen coordinates** *S* of the pixel in [0, 1]².
3. **Refraction:** sample the frame-buffer copy (the scene drawn so far) at *S* plus the world
   normal's two horizontal components × `RefractionBumpDistortionAmount`. Call it *F*.
   Tint it: *U* = *F* + `FogAmount` · (`FogColor` − *F*). Near the camera blend toward the
   shoreline tint: with *w* = clamp((eye distance − 5) × 0.1, 0, 1),
   refraction colour = *F* · `ShorelineColor` + *w* · (*U* − *F* · `ShorelineColor`).
4. **Reflection:** sample the reflection texture at *S* plus the horizontal normal components
   × `ReflectionBumpDistortionAmount`, plus 0.003 on the vertical screen axis. In the
   permutations without a reflection texture the unmodified refraction sample *F* is used
   instead.
5. **Fresnel:** with *v* the unit vector toward the eye, *f* = `FresnelBias` + (1 −
   `FresnelBias`) · (1 − |*v* · *n*|)^`FresnelExponent`; colour = refraction + *f* ·
   (reflection − refraction).
6. **Specular:** *s* = clamp(*n* · normalize(*v* − *L*), 0, 1)^`SpecularPower` +
   4 · (clamp(*g*ₓ − `FakeSpecularThreshold`, 0, 1) + clamp(*g*ᵧ − `FakeSpecularThreshold`,
   0, 1)). The second summand is the sparkle term; it is added **after** the exponent and is
   not itself clamped, so *s* can exceed 1 (at most 9). Here *n* is the world-space normal of
   step 1, *v* the unit vector toward the eye, and *g* the first two (tangent-space,
   horizontal) components of the normalised normal-map sum of step 1, before its rotation into
   world space.
   *L* is the engine's directional "fake light" direction (program constant
   `c_FakeDirLight_WorldSpaceDirection`): the main light's **travel** direction (pointing away
   from the light, so −*L* points toward it) when the main light is directional, and an engine
   default direction otherwise (`lighting.md` §4.2, BIN-L (g); W17). For an outdoor scene
   whose main light is the sun, −*L* is the toward-sun vector of `lighting.md` §8.1.
7. **Output:** colour × `WaterColor` × `OverbrightAmount` + *s* × the fake light's specular
   colour × `SpecularColor`; then mixed toward the distance-fog colour by the fog factor of
   §8.3. Output alpha = the alpha of the material colour (2000).

Render state of these permutations (`material.md` §7.6): the ones without a depth texture write
with source ONE and destination ZERO (they composite the frame-buffer copy themselves); the
ones with a depth texture and the low-detail ones blend with source alpha and inverse source
alpha; all write depth; all but the low-detail ones draw both faces.

> **OPEN:** How the client chooses the permutation (the user's water-reflection,
> shoreline-effect and interactive-water settings are the likely switches; the permutation
> keys are opaque, `material.md` §7.4), how the reflection texture is rendered (a mirrored
> view about the plane z = *h*, presumably), what the depth-texture permutations compute
> (soft shorelines are the likely use; parameters 1170–1173), the flow and foam formulas of
> 0x2B0007FD and 0x2B0005EC, and the draw order of water relative to other translucent
> objects.

> **OPEN (implementer, 2026-10-06, `tessera-render` water pipeline):** step 1 rotates the
> normal-map sum "into world space with the surface frame" without saying which way the
> tangent-space axes point on the landscape plane, i.e. whether the world normal is
> *n*.x · **T** + *n*.y · **B** + *n*.z · **N** or uses the −*n*.y of `lighting.md` §5.2.4
> (**T** along +x, **B** along +y, the directions in which the layer coordinates of §8.3
> grow). Tessera uses the `lighting.md` §5.2.4 sign (`docs/specs/render.md` §5.3). The sign
> mirrors the ripples' shading and the refraction offset; it does not change the Fresnel or
> sparkle magnitudes.

> **OPEN (implementer, 2026-10-06):** step 6 needs the fake light's direction and specular
> colour when the main light is not directional; `lighting.md` E18 names "a default
> direction" and "an engine specular colour" without values. Tessera uses straight down with
> a black specular colour (no highlight) in that case (`docs/specs/render.md` §5.3).

**Tessera rule (Decision), until those are settled:** draw water after opaque geometry with a
copy of the colour buffer as the refraction source, implement steps 1–7 with the reflection
replaced by the refraction sample (the original's no-reflection path), and use alpha 1.

<a id="s8-5"></a>
### 8.5 Placed water meshes

*Revised 2026-10-09 (W28–W34).* Placed water (pools, fountains, troughs, puddles, waterfalls,
rapids, water in interiors) is ordinary static geometry placed by `entity-descriptor.md`. How
each of its surfaces is shaded depends on the **program family of the surface's shader
record**, not on the generic rules of `material.md` §11: a water record's slot 3000 is a
near-white placeholder (§8.5.2), a puddle's diffuse texture is a white shape whose colour
comes from the reflection (§8.5.4), and a waterfall's look comes from two scrolling texture
layers (§8.5.3). The earlier text of this section ("the same shading applies") held for the
plain water family only.

**Rule (normative):** a renderer selects the shading of a placed surface by its shader record:

1. a record of the plain water, plain-water-on-UV, older-bump or flow-map family of §8.2:
   the water drawing of §8.3–§8.4, §8.5.6 or §8.6, with the surface's water parameters; the
   placed mesh's own vertices are used, its world position feeds *B* where the family uses
   *B*;
2. the mirror record 0x2B0000AA: §8.5.4;
3. a liquid-distortion record: §8.5.5;
4. every other record (the two-layer waterfall records included): the generic path of
   `material.md` §9–§11 and `lighting.md` §5, **with** both colour layers (`material.md`
   §10.3) and the animated texture matrices (`material.md` §7.7, §8.4, §5.3.1).

The lava record (0x2B000810 in this install; added 2026-10-09, W41) is not of kind 4: it is
drawn by §8.6.5, on landblock water and on any placed mesh alike (no placed mesh uses it in this
install). Records of kinds 1–3 and the lava record are recognised from their interface
(declared ids and sampler names), as `material.md` §11.6 states. Drawing a family-1, -2 or -3
surface with the generic path, or a family-4 surface without its
second layer and texture matrices, gives the near-white, static surfaces described in §8.5.2
to §8.5.4; drawing the lava record with the generic path shows its colour layer at the mesh
UV (one repeat per 3 × 3 sector on landblock quads, §8.1) instead of the emissive pattern of
§8.6.5.

<a id="s8-5-1"></a>
#### 8.5.1 Census (OBS2, W29)

Placements are entity rows of landblock info (all regions); the same object can carry
several surfaces. Object and surface DIDs identify; the descriptions are ours.

| Kind | Example objects (0x47) → surface records (placements) | Shader record, family | § |
|---|---|---|---|
| Fountain basins, reflecting pools, troughs: the water plane | 0x47006F16, 0x47006F68 and 44 more → 0x31006748 (1,914); 0x47004F1A → 0x31004121 (413); 0x4700654A → 0x31005ADB (694); 0x470038E1 → 0x3100770C (105) | 0x2B0000A8, plain water | §8.5.2 |
| Round pools | 0x47001983 and 26 more → 0x3100053C (2,948) | 0x2B0000A9, older bump water | §8.5.6 |
| Puddles | 0x470015E4, 0x47006B5E, 0x4700C32A → 0x31000A2A (1,573); 0x4700C32B → 0x31008161 (65); 0x4700C32C → 0x31008162 (143) | 0x2B0000AA, mirror | §8.5.4 |
| Waterfall sheets (inner, outer and fill meshes of a fall) | 0x470003E6–0x470003E8, 0x47002168, 0x47002169, 0x47002305 … → 0x310004E6 (3,525), 0x310004E7 (2,251), 0x310004E8 (2,942) | 0x2B0007A1, lit two-layer, layers multiplied | §8.5.3 |
| Inner sheet of a small fall | 0x4700102C → 0x31007856 (277) | 0x2B0007F7, lit two-layer, multiplied | §8.5.3 |
| Cascades, rapids and splashes of a stepped fall | 0x47004B3E, 0x47004B3F, 0x47004B41 … → 0x31003E4C (327); 0x47004B49 → 0x3100770D (69) | 0x2B000817, lit two-layer, layers **added** | §8.5.3 |
| Foam on those falls | 0x47004B4A, 0x47004B42 → 0x31003E51 (556) | 0x2B00078C, unlit two-layer, added, vertex colour | §8.5.3 |
| Opaque foam, a coastal fall | 0x47004BE0–0x47004BE5 … → 0x31003E4E (306), 0x31003E52 (84); 0x4700D2C0 … → 0x31008F62 (67) | 0x2B0006F2, unlit two-layer, added | §8.5.3 |
| Fountain overlays and water spouts | 0x47006F17 → 0x31006749 (480); 0x47004F1B … → 0x31004122 (533); 0x4700CF6A … → 0x3100502D (124) | 0x2B0007E5, lit two-layer, multiplied | §8.5.3 |
| A gate waterfall | 0x47004037 → 0x31003228 (132) | 0x2B0004B5, liquid distortion | §8.5.5 |
| Thin falls with distortion | 0x47003F1D … → 0x31003174 (102), 0x31003175 (67) | 0x2B0007D0, lit liquid distortion | §8.5.5 |
| Flowing-water and mud marks | 0x4700158A, 0x470036F5, 0x47000F8B and 136 more → 0x31000798 (16,500) | 0x2B00078B, unlit `Color`, no transform: a static texture | `decals.md` (the flowing-water marks are Decal SEDs, type 2) |

> **OPEN:** The splash and mist entities placed at the foot of falls (for example 0x47000D28,
> 0x470003E5, 0x470041C4) name the generic placeholder setups 0x04000000 and 0x0400000D;
> where their visible splash comes from (effects of `particles.md`, scripts) was not traced.

**What a renderer meets over the whole world (added 2026-10-09; TP, W35).** Counted as submesh
draws with the walker's draw rule over all 390,150 landblocks (so the numbers differ from the
placements above):

| Group | Surface records | Draws | Largest (draws) |
|---|---|---|---|
| Water records on placed meshes (§8.5.2, §8.5.6) | 15 (0x2B0000A8, 0x2B0000A9, 0x2B0000CE) | 6,133 in 1,471 landblocks | 0x3100053C (2,986), 0x31006748 (1,914), 0x31005ADB (685), 0x31004121 (413) |
| Mirror record (§8.5.4) | 8 of its 9 | 3,491 | 0x31000A2A (2,067), 0x31008162 (712), 0x31008161 (630) |
| Liquid-distortion records (§8.5.5) | 11 | 647 | 0x31000AFB (234), 0x31003228 (132), 0x31003174 (102) |
| Two-layer records with animated texture matrices, waterfall-like (§8.5.3) | 109 | 43,373 | 0x31006637 (8,205) and 0x31006638 (5,470) on object 0x47006CBE; 0x310004E6 (3,519); 0x3100048A (3,328) and 0x31000489 (2,551) on 0x470003E9; 0x310004E8 (2,933); 0x310004E7 (2,244) |

Every drawn surface of 0x2B0000A8, 0x2B0000A9 and 0x2B0000CE keeps the near-white defaults
0x40000DDF / 0x40000DE0 in slots 3000 / 3001 (0x31008E9F, which sets its own slot 3000, §8.5.2,
is not among the drawn ones). Of all 13,781 surface records only 23 have no usable slot-3000
texture, and the two of those that are drawn (0x31000482, 0x31000908) are on rock and building
meshes, not water (`material.md` §18 OPEN). So placed water, puddles and falls do not look white
for lack of a texture: they look white or grey because their own drawing (§8.5.2–§8.5.6) is
replaced by the generic one, which shows the placeholder or the white shape image, and because
the generic one leaves out the second layer and the animated texture matrices (§8.5.3).

**Liquid objects (W35).** Every placed object that carries liquid-volume monitors (§7.3; 39 such
objects with visible parts) draws at least one water-record surface (26 surface uses, 3,833
draws) besides ordinary lit or unlit parts (basin rims and similar; 9 uses, 95 draws).

<a id="s8-5-2"></a>
#### 8.5.2 Water records on placed meshes (W30)

The water planes of fountains, pools and troughs use the landscape's water records
(0x2B0000A8 here). The record's water permutations (§8.2 table) read only the vertex
position and sample the normal map, the frame-buffer copy and, when present, the reflection
and depth textures; which permutation the client picks under which setting is OPEN (§8.4),
and that it picks a water permutation whenever the landscape water is drawn with one is
Inferred (the same record serves both). *B* comes from the plane's world position,
so a pool's ripples continue the landscape's pattern. On every placed surface of 0x2B0000A8 and
0x2B0000A9 the slots 3000 and 3001 keep their shader defaults, the texture lists 0x40000DDF and
0x40000DE0: near-white images (mean level-2 colour about (245, 243, 243) and (228, 235, 237)).
Three other placed water surfaces set slot 3000: 0x31008E9F (0x2B0000CE) and 0x310081C1,
0x31008768 (0x2B00084B, 0x2B00089A; a 4 × 4 white image). Only the low-detail `Color` /
`Color2` permutations sample them, scrolled by the record's animated 1017–1020 curves.

Parameters of the most common plane, 0x31006748 (effective, `material.md` §9.2): normal map
0x40000DDE, scales 4, 8, 8 with the default speeds of §8.2, fog amount 1.2, fog colour
(0.463, 0.678, 0.89), Fresnel bias 0.3 and exponent 5, specular power 3 with colour
(0.373, 0.482, 0.455), material colour alpha 0.75, depth write off (4002 = 0).

**Tessera rule (Decision):** draw these surfaces through the water pipeline (§8.3–§8.4 with
the Tessera rules there) at the placed mesh's own triangles; never through the generic lit
path, which shows the near-white placeholder.

<a id="s8-5-3"></a>
#### 8.5.3 Waterfall sheets, foam and spouts (W34)

These are generic two-layer records (`ColorAndOpacity` on slot 3000, `ColorAndOpacity2` on
slot 3001). Their look rests on three things that a renderer without texture matrices and
without a second layer loses:

1. **Two layers.** The colour is the two texels multiplied (0x2B0007A1, 0x2B0007F7,
   0x2B0007E5) or **added** (0x2B000817, 0x2B00078C, 0x2B0006F2), per record
   (`material.md` §10.3); the opacity is the **product** of both alphas, times the material
   colour's alpha (and the vertex alpha in 0x2B00078C). In the waterfall surfaces the second
   texture is often a soft-edged mask: 0x40000C5D, the second layer of 0x310004E6 and
   0x310004E7, is white with an alpha that is opaque in the middle and fades to 0 at all four
   borders, so the sheet's edges dissolve.
2. **Animated texture matrices.** Each layer has its own matrix (layer 1 matrix 0, layer 2
   matrix 1, `material.md` §8.4) whose values are float curves evaluated at the global clock
   every frame (`material.md` §5.3.1): mode 2 is a steady scroll (rate `w[6]` texture repeats
   per second), mode 4 a sine, mode 7 smooth noise.
3. **Translucency.** The material colour's alpha is below 1 on most sheets and depth write is
   off on the inner and outer sheets.

Effective values of the sheets of the common falls (effective parameter set; curve
parameters as `w[0..9]` of `material.md` §5.3, only the non-zero ones):

| Surface | Layer 1 matrix (0) | Layer 2 matrix (1) | Textures 3000 / 3001 | 2000 (RGBA) | Other |
|---|---|---|---|---|---|
| 0x310004E6 (inner, part of outer) | U translation mode 2, `w[2]` 1, `w[6]` 0.15 (0.15 per second); V translation mode 2, `w[6]` −0.4; U scale mode 4, `w[0]` 2, `w[2]` 0.2, `w[6]` 0.5 (2 + 0.2 sin 0.5*t*) | identity | 0x40000C5C / 0x40000C5D | (0.741, 0.769, 0.788, 0.51) | depth write off, alpha test off |
| 0x310004E7 (fill) | U translation mode 7, `w[2]` 1, `w[6]` 0.1 (noise drift); V translation mode 2, `w[2]` 0.1, `w[6]` −0.4; U scale mode 4 (1 + 0.2 sin 0.5*t*); V scale mode 4 (1 + 0.15 sin 0.5*t*) | identity | 0x40000B18 / 0x40000C5D | (0.722, 0.733, 0.769, 1) | alpha test on, threshold 0.3 |
| 0x310004E8 (outer) | U translation mode 2, `w[6]` 0.3; V translation mode 2, `w[6]` −0.4 | V translation mode 2, `w[6]` −0.6 | 0x40000C5E / 0x40000C5F | (1, 1, 1, 0.757) | two-sided, depth write off, alpha test off |
| 0x31003E4C (stepped falls) | V scale 0.5; V translation mode 2, `w[6]` −0.1 | U scale 2, V scale 0.89; U translation mode 2, `w[6]` −0.15; V early translation mode 2, `w[6]` −0.45 | 0x4001DACB / 0x4001793D | (1, 1, 1, 1) | layers added; two-sided, depth write off |
| 0x31003E51 (foam) | scale (2, 1.2); U translation mode 7 (`w[2]` 0.37, `w[6]` 0.3); V translation mode 2, `w[6]` −0.37 | scale (2, 0.8); U translation mode 7 (`w[2]` −0.33, `w[4]` 0.25, `w[6]` 0.25); V translation mode 2, `w[6]` −0.34 | 0x40019DAC / 0x40019DAC | (1, 1, 1, 0.679) | unlit, layers added, × vertex colour |
| 0x31006637 (object 0x47006CBE; added 2026-10-09, the most drawn, TP) | U early translation −0.5; U scale mode 4 (1 + 0.2 sin *t*); V scale 1.4; U translation 0.5; V translation mode 2, `w[6]` −0.7 | U early translation −0.5; U scale mode 4 (0.8 + 0.1 sin(*t* + 0.5π)); V scale 1.1; U translation 0.5; V translation mode 2, `w[6]` −0.5 | 0x4001793D / 0x4001793D | (1, 1, 1, 1) | record 0x2B0007F7, lit, layers multiplied; depth write on, alpha test off |
| 0x31006638 (object 0x47006CBE; added 2026-10-09) | U early translation 0.5; U scale mode 4 (0.7 + 0.1 sin *t*); V scale mode 4 (2 + 0.05 sin *t*); U translation 0.5; V translation mode 2, `w[6]` −0.7 | U early translation −0.5; U scale mode 4 (0.55 + 0.05 sin(*t* + 0.3π)); V scale mode 4 (1.6 + 0.06 sin(*t* + 0.5π)); U translation −0.5; V translation mode 2, `w[6]` −0.5 | 0x4001793D / 0x4001793D | (1, 1, 1, 1) | as 0x31006637 |
| 0x3100048A (object 0x470003E9; added 2026-10-09) | V scale mode 4 (1 + 0.2 sin 0.5*t*); U translation mode 2, `w[6]` −0.5; V translation mode 2, `w[6]` 0.7 | V scale mode 4 (1 + 0.2 sin(0.5*t* + 0.25π)); V translation mode 2, `w[6]` 0.75 | 0x40000B18 / 0x40000B1A | (1, 1, 1, 1) | record 0x2B0007A1, multiplied; two-sided, depth write off |
| 0x31000489 (object 0x470003E9; added 2026-10-09) | U scale mode 4 (1.8 + 0.2 sin 0.5*t*); V scale 0.6; U translation mode 4 (0.5 sin 0.2*t*); V translation mode 2, `w[6]` 0.487 | identity | 0x4001793D / 0x40000B19 | (1, 1, 1, 0.637) | record 0x2B0007A1, multiplied; two-sided, depth write off |

The mode-2 translation `w[6]` is a rate in texture repeats per second (`material.md` §5.3.1:
the result is `w[0]` + `w[1]`·*t* + `w[6]`·*t* + …); the V rates (from −0.7 to +0.75 repeats
per second in the rows above) make the texture stream along the sheet. Mode-4 values are
`w[0]` + `w[2]` sin(`w[6]` *t* + π `w[4]`) in every row of the table, whose curves all have
`w[1]`, `w[3]`, `w[5]` and `w[7]` = 0 (OBS3); in general (`material.md` §5.3.1) `w[1]` and `w[3]`
add *t*-proportional terms to the base and the amplitude, and the phase is (`w[6]` + `w[7]` *t*) *t*
+ π `w[4]` + `w[5]` *t*, so `w[5]` adds to the angular rate, as on the foam and depth offsets of §8.7.1 (corrected 2026-10-10:
the former sentence gave the formula without saying that those terms are 0); every curve of the table is
evaluated at the global clock each frame (§8.7). That the stream runs downward on screen follows from the
meshes' UV layout and is Inferred.

<a id="s8-5-4"></a>
#### 8.5.4 Puddles: the mirror record 0x2B0000AA (W31)

Parameters (labels from enum mapper 0x230001A6, interoperability): 1100
`c_MirrorReflectionBumpDistortionAmount`, 1101 `c_MirrorFresnelExponent`, 1102
`c_MirrorFresnelBias`, 1103 `c_MirrorNormalTextureScale`, 1104 `c_MirrorNormalSpeedU`, 1105
`c_MirrorNormalSpeedV`, 1106 `c_MirrorFresnelBumpAmount`; slots 3000 (diffuse), 3011 (normal
map), and the engine's reflection texture (sampler source 3, `material.md` §7.5). Values of the
three puddle surfaces 0x31000A2A, 0x31008161, 0x31008162: 1100 = 0.006, 1101 = 1.33, 1102 = 0,
1103 = 1, 1104 = 0, 1105 = a mode-2 curve with `w[6]` = 0.00001 (its value is 0.00001 × *t*),
1106 = 0.05; normal map 0x40000DDB; diffuse 0x400017ED, 0x4001E99B, 0x4001E99A: pale or white
images (mean colour about (186, 181, 166), (255, 255, 255), (253, 253, 253)) whose alpha is the
puddle's shape.

Per vertex: the normal-map coordinates are (UV + (*S*ᵤ, *S*ᵥ) × *T*) × `NormalTextureScale`,
with UV the mesh's UV set 0 and *S* the evaluated 1104 and 1105 (a curve value multiplied by
*T* again, so for the puddles the offset grows as 0.00001 × *t* × *T*); the diffuse uses UV set 0
unchanged; the view vector is carried in the tangent frame (tangent, binormal, normal) of the
vertex; the fog factor is (distance − fog near) / fog range, at least 0 and not limited to 1
(the program does not clamp it from above).

Per pixel:

1. *n* = the normal-map texel; *S* = the pixel's screen position in [0, 1]² (vertical axis
   downward).
2. Reflection *R* = the reflection texture at *S* + 2 (*n*.xy − 0.5) × 1100, times the material
   colour's RGB.
3. Diffuse *D* = slot 3000 at UV.
4. Fresnel normal *m* = (0, 0, 1) + 1106 × (2*n*.x − 1, 2*n*.y − 1, 2*n*.z − 2) (tangent
   space); *f* = (1 − |*v* · *m*|)^1101 with *v* the unit tangent-space view vector;
   *a* = 1102 + (1 − 1102) × *f*.
5. Colour = *D* × *R*, mixed toward the fog colour by the fog factor; output alpha =
   *a* × *D*.alpha × material alpha. Blending source alpha / inverse source alpha, depth write
   on, back faces culled.

So a puddle shows the reflected scene inside its shape, nearly invisible when looked at from
above (*f* near 0) and mirror-like at grazing angles. The record's permutations without a
reflection sampler (one per back end) write nothing (blend ZERO / ONE).

**Tessera rule (Decision), until a reflection pass exists (§8.4 OPEN):** take *R* as the
distance-fog colour times the material colour (a sky-like tint), keep steps 3–5, and draw in
the transparent phase. Never draw the diffuse as an opaque or plain alpha-blended colour: its
RGB is white by design.

**The record's other surfaces (added 2026-10-09; W36, TP).** 0x2B0000AA has nine surface
records. Besides the three puddles, six are static mirrors (1104 = 1105 = 0, so the normal map
does not move; 1103 = 1; normal map 0x40000E21 except 0x31006DFF, which names its own):

| Surface | 1100 | 1101 | 1102 | 1106 | Diffuse (mean luminance, TP) | Material colour | Drawn on (TP: draws) |
|---|---|---|---|---|---|---|---|
| 0x31000A4E | 0 | 2 | 0 | 0 | 0x40001828 (0.23) | white | 0x47001644 (22) |
| 0x31000A4F | 0 | 2 | 0 | 0 | 0x40000008 (1.00) | white | 0x47001644 (22) |
| 0x31000CBE | 0 | 4 | 1 | 0 | 0x40000008 (1.00) | white | 0x47007615, 0x47002303, 0x4700830A (15) |
| 0x31007E55 | 0 | 4 | 1 | 0 | 0x400010F0 | white | not drawn |
| 0x31002770 | 0.08 | 4 | 1 | 0.25 | 0x40001F87 (0.82) | (0.651, 0.651, 0.651, 1) | 0x47002D92 (22) |
| 0x31006DFF | 0.08 | 4 | 1 | 0.25 | 0x4001CEFD (0.82) | white | 0x47007E50 (1) |

The same steps 1–5 apply. With 1102 = 1 step 4 gives *a* = 1 at every angle, so those four
surfaces are opaque wherever the diffuse alpha and the material alpha are 1: a mirror whose
colour is the reflection × the diffuse. With 1100 = 0 the reflection is not distorted and with
1106 = 0 the Fresnel normal is (0, 0, 1). The Tessera rule above applies to all nine; on the
opaque mirrors it gives a flat fog-tinted surface until a reflection pass exists.

> **Answered 2026-10-10 (OBS3, W44; the original's look Inferred; was OPEN, implementer,
> 2026-10-10; checklist row W13 of `m4-5-visual.md` §4.8, for the spec writer or the owner):** the puddle object 0x470015E4 draws mesh 0x0600183D, a single flat
> 2 × 2 quad (17.4 m across at scale 8.7) whose centre lies 0.23 m above the ground. Where the
> terrain under it rises above the quad, the depth test hides the quad, so Tessera shows the
> polygon where quad and terrain intersect (hard edges, one soft corner where the diffuse
> alpha's rounded edge is visible) instead of the texture's soft outline that row W13
> expects. The shape is the same at every mesh detail level and through the generic and the
> mirror drawing, so it is not the §8.5.4 formula. Steps 1–5 give depth write on and back
> faces culled but say nothing about the depth test or a depth offset. Question: does the
> original also clip puddle quads against the terrain, or does it keep them visible some
> other way, for example a depth bias from `Render_DepthBias_Type` on the puddle's SED or
> object (`decals.md` §6.5 and its OPEN (c), not implemented in Tessera), the depth test off
> for this record, or placing or projecting the quad onto the ground? Tessera draws them as
> stated (depth test on, no bias) until answered.
>
> **Answer.** Nothing in the data keeps this quad out of the terrain, and Tessera's drawing is
> the expected one:
>
> 1. **Depth state.** Every permutation of 0x2B0000AA tests depth (render-state byte 3 = 7,
>    nearer or equal) and writes it (byte 4 = 1), and the three puddle surfaces set 4001 = 1 and
>    4002 = 1 (`material.md` §7.6, §8.5). The depth test is on.
> 2. **No bias for this puddle.** Object 0x470015E4 has no `Render_DepthBias_Type`, and the
>    W13 placement (0x80217574 #170) sets none. The other puddle objects (0x47006B5E,
>    0x4700C32A–0x4700C32C) and 54 of the 832 placements of 0x470015E4 set type 3
>    (`BiasForward1`, slope-scale −0.8; one row type 4). The table holds slope-scale values only
>    (|value| ≤ 10, `decals.md` §6.5): a bias proportional to the depth slope of the quad, a
>    fraction of the depth spanned by one pixel's footprint, which settles coplanar surfaces but
>    cannot lift a quad out of terrain that stands metres above it.
> 3. **Placement.** The client creates a placed entity at its stored position and rotation,
>    with no snapping of *z* to the ground (`entity-descriptor.md` §9.3, row E27, BIN), and
>    the scale is the row's `Render_AdjustableScale`; nothing projects the quad onto the
>    ground. 80 % of the W13 quad (353 of 441 sample points) lies below
>    the height grid, by up to 2.6 m: this large puddle is sunk into a dip, so its visible
>    outline is where the ground lies below the plane.
> 4. **Most puddles are not cut.** Of the 784 outdoor puddle placements, 666 lie wholly above
>    the height grid and 118 partly below (W44). Puddles that lie above the ground show the
>    diffuse alpha's soft outline (checklist row W14).
>
> **Consequence (Inferred, owner item O24 in `owner-observations.md`):** the original also
> shows the W13 puddle cut along the ground where the terrain rises above it, with the soft
> alpha edge only where the quad's own edge lies above the ground. **Rule:** draw puddles with
> the depth test and depth write of their record; where the object or row sets
> `Render_DepthBias_Type`, apply that table entry to the puddle's draws as `decals.md` §6.5
> states ("Which draws apply it"); never turn the depth test
> off, raise the quad or project it onto the ground for puddles. Checklist row W13 is corrected
> accordingly. (Added 2026-10-10, second answer: the value is found by the entity's lookup
> order and the default covers every draw of the entity, §14 items (a) and (b); W49–W51.
> Third answer, 2026-10-10: the original's draws were traced, BIN6, W53: the entity's entry
> applies to every part of the entity in every pass its scene node draws.)

<a id="s8-5-5"></a>
#### 8.5.5 Liquid-distortion records (W32)

0x2B000442, 0x2B0004B5 and 0x2B00063E (unlit; samplers `LiquidNormalTexture` on slot 3011
and `LiquidDiffuseTexture` on slot 3000) and 0x2B0007D0 (lit; samplers `Distortion` on slot
3026 and `ColorAndOpacity` on slot 3000) read the water parameters 1044–1046 (waves), 1048
(refraction distortion), 1057–1065 (layer speeds and scales) and, unlit, 1066 (overbright).

1. **Coordinates.** UV′ = the mesh's UV set 0 through texture matrix 0 (0x2B0004B5,
   0x2B00063E, 0x2B0007D0; `material.md` §8.4), or *B* = (world position − *O*) / 160
   (0x2B000442). Layer *k* (0, 1, 2) = UV′ × `Scale`ₖ + (`SpeedU`ₖ, `SpeedV`ₖ) × *T*.
2. **Waves** (unlit records): the vertex height changes as in §8.3 (same formula, model
   coordinates).
3. **Distortion.** Sum the three layer samples of the normal (or distortion) texture, each
   mapped to 2 (rgb − 0.5); normalise; take its first two components × 1048.
4. **Colour texture** at layer 2's coordinates plus that offset. So the colour texture scrolls
   with layer 2 and wobbles with the sum.
5. **Output** (unlit): colour × material colour × 1066, fogged; alpha = texel alpha × material
   alpha (× vertex colour, RGBA, in 0x2B00063E: the texel is multiplied by the vertex colour
   first); the alpha test of the program discards when that output alpha is at or below the
   threshold (W45, clarified 2026-10-10). 0x2B0007D0 lights the distorted colour like any lit `ColorAndOpacity`
   surface (`lighting.md` §5). Its alpha test compares the alpha of the distorted
   `ColorAndOpacity` texel alone, before the material alpha multiplies it, as `lighting.md`
   §5.12 states for lit records; it has no vertex-colour input, so no vertex multiply occurs
   (W52, added 2026-10-10). Output alpha = that texel alpha × material alpha.

   > **Answered 2026-10-10 (SH3, OBS3, W45, W46; was OPEN, implementer, 2026-10-10):** the
   > spec gives no interface rule that tells 0x2B00063E apart; Tessera recognises the
   > vertex-colour record by its DID, as it does 0x2B000442 for step 1. Tessera multiplies
   > colour and alpha by the vertex colour after the alpha test, which reads the texel alpha
   > alone (which alpha the program tests is not stated).
   >
   > **Answer.** (1) **Order, corrected:** the three unlit programs test the **output** alpha.
   > The colour texel (RGBA) is first multiplied by the vertex colour (0x2B00063E only), its
   > alpha then by the material alpha, and the pixel is discarded when that product is at or
   > below the reference; colour × material colour × 1066 and the fog follow (W45). So in
   > 0x2B00063E the vertex alpha takes part in the test, unlike the lit records of `lighting.md`
   > §5.13, which test before the vertex multiply. (2) **Recognition:** within the
   > liquid-distortion records of `material.md` §11.6, 0x2B00063E is the one whose programs
   > take a vertex colour, the same property `lighting.md` §5.13 uses for every record (L13,
   > Direct3D 10/11 vertex input signatures); and 0x2B000442 is the one whose permutations with
   > programs carry no texture-transform item, which is why its layers cannot read UV′ (W46).
   > The second is visible in the decoded record (§7.7 of `material.md`); a renderer may use it
   > in place of the DID. The first needs the program's input signature, which Tessera does not
   > read (`material.md` §7.4); **Tessera Decision:** keep recognising 0x2B00063E by its DID
   > (an observation of this install, listed with the other per-record lists of §14 (a)).
   >
   > **Added 2026-10-10 (OBS4, W52; §14 second implementer OPEN, item (c)):** the W46 rule needs
   > per-permutation items. Two record-wide forms select the same record and survive a merged
   > binding table: among the liquid-distortion records, 0x2B000442 is the only one (i) none of
   > whose declarations has a binding with a semantic of 38–44, and (ii) with no type-5 extra
   > state item in any permutation (`material.md` §7.3.1, §7.7). Either may replace the DID.
   >
   > **Added 2026-10-10, third revision (OBS5, W54; §14 implementer OPEN on `tessera-material`):**
   > the two rules never disagree on this install: over all 510 shader records they select the
   > same records. Neither is a test of the client, which recognises no record and runs each
   > record's programs; the property that decides how step 1 forms the coordinates is W46's:
   > whether the record's permutations with programs change texture matrix 0. When the two
   > rules disagree on a record, that record lies outside the census of this spec: treat it as
   > reading UV′ (world-position coordinates only when **both** rules hold), and log it. The
   > binding form of the W46 property, "some binding with a semantic of 33–44 names a
   > permutation with programs", matches W46 on every one of the 510 records and may replace
   > both rules where the decoded interface keeps each binding's permutation index and whether
   > that permutation has programs. Rules (i) and (ii) are valid only inside the
   > liquid-distortion family: 0x2B0000B7, which is not a liquid record, has transforms in its
   > programs yet satisfies both rules (its items are of types 1–3, bound with 33–37).

Example: the gate waterfall 0x31003228 (0x2B0004B5) has texture matrix 0 scale (0.3, 0.2) and
V translation mode 2 `w[6]` −0.001; speeds (0.005, −0.1), (0.015, −0.2), (−0.01, −0.2); scales
2, 4, 8; distortion 1; overbright 1.15; diffuse 0x40004801. Its colour texture therefore streams
at 0.2 repeats per second along V at eight times the matrix's tiling.

<a id="s8-5-6"></a>
#### 8.5.6 Plain water on mesh UV and older bump water (W33)

**Plain water on mesh UV** (0x2B00084B, 0x2B00089A, 0x2B000A8D): as §8.3–§8.4 (Probable for
the pixel stage) except that layer *k* reads the normal map at UV × `Scale`ₖ + (`SpeedU`ₖ,
`SpeedV`ₖ) × *T*, UV the mesh's UV set 0.

**Older bump water** (0x2B0000A9, 0x2B0000DC; parameters 1055 `c_WaterBumpTextureScale` and
1056 `c_WaterBumpSpeed`, labels for interoperability): with *U* = UV × 1055 and *b* = 1056 × *T*,
the three normal-layer coordinates are *U* + (*b*, *b*), 2*U* + (4*b*, 4*b*) and 4*U* + (8*b*,
8*b*). The round pools 0x3100053C use 1055 = 0.5 and 1056 = 0.003. The pixel stage reads
`c_WaterDeepColor`, `c_WaterShallowColor`, `c_WaterReflectionAmount`,
`c_WaterRefractionAmount` and `c_WaterFresnelBumpAmount` (2004, 2005, 1052, 1053, 1049; labels
for interoperability) besides the common ones; its formula was not traced.

> **OPEN:** The pixel formula of the older bump-water records (2 records, 3 placed surfaces).
> **Tessera rule (Decision):** draw them with §8.4 using the coordinates above, `WaterColor`
> white and the refraction tint of their `c_WaterDeepColor`.

<a id="s8-6"></a>
### 8.6 Flow-map water: 0x2B0007FD and 0x2B0005EC (W26–W28)

These two records draw 85,059 of the 89,913 wet landblocks (W23). They are not the plain
water of §8.3–§8.4: their normal is built from a flow map, and they use 1057 and 1058 as
rates of a cycle, not as scroll speeds. The lava record 0x2B000810 uses the same flow phases
for colour layers instead of a normal map and has no water colour; it is described separately
in §8.6.5 and nothing else in §8.6.1–§8.6.4 applies to it.

<a id="s8-6-1"></a>
#### 8.6.1 Inputs

- Per vertex: position; UV set 0 (on landscape quads the sector coordinates of §8.1).
- Slots: 3011 normal map; 3075 flow map (sampler address clamp); 3079 foam map
  (0x2B0007FD only); a noise texture on a sampler without binding, named `NoiseTexture`
  (interoperability name; the only sampler with a default texture in these permutations, W47),
  whose default texture (`material.md` §7.5) is 0x400163D4; the frame-buffer copy; the reflection texture and the
  depth texture in some permutations; the sun-shadow inputs in the Direct3D 10/11 permutations
  of 0x2B0007FD.
- Parameters: 1057, 1058 (flow rates); 1063 (`Normal0Scale`); 0x2B0007FD also 1059–1062,
  1064, 1065 (layers 1 and 2), 1170 `c_WaterShorelineMaxDepth`, 1171 `c_WaterFogMinDepth`, 1173
  `c_WaterFogDepthFactor`, 1182, 1183 `c_WaterFoamOffsetU`/`V`, 1184, 1185
  `c_WaterFoamTileU`/`V`, 1186, 1187 `c_WaterDepthOffsetU`/`V`, 1188 `c_NormalWaveRotation`,
  1189 `c_DepthBasedOpacityOffset`, 1190 `c_FoamAlphaFadeFactor` (labels for
  interoperability); the colour parameters of §8.2. Neither record has waves.

<a id="s8-6-2"></a>
#### 8.6.2 Normal

With *B* = ((*P*.x, *P*.y) − *O*) / 160 as in §8.3 and *T* as in §8.7:

1. **Flow vector** *F* = 2 × (the flow map's first two channels at UV) − 1.
2. **Phase offset** *q* = ½ × the noise texture's first channel at ½ *B* (one noise repeat per
   two landblocks).
3. **Two phases.** *p*₁ = *q* + 1057 × *T*; *p*₂ = *q* + 1058 × *T* + ½. For each, *f*ᵢ = the
   fractional part of *p*ᵢ and *i*ᵢ = *p*ᵢ − *f*ᵢ (the cycle number).
4. **Layer-0 coordinates** of phase *i*: *c*ᵢ = 1063 × *B* + 0.7 × (*i*ᵢ, *i*ᵢ) + *F* × *f*ᵢ.
   Within a cycle the normal map slides along *F* by *f*ᵢ repeats; at each new cycle it jumps
   by 0.7 repeats on both axes. 0x2B0007FD (Direct3D 10/11) then rotates both *c*ᵢ by the
   angle 1188 (radians): (*x*, *y*) → (*x* cos θ + *y* sin θ, −*x* sin θ + *y* cos θ); 1188 is
   0 on all 21 surfaces.
5. **Blend.** Sample the normal map at *c*₁ and *c*₂; mix them with weight *w* = 2 × |½ − *f*₁|
   toward the second (the first alone at mid-cycle of phase 1, the second alone when phase 1
   wraps).
6. **Flow normal.** With *t* the mixed texel (r, g, b) and *k* = 2 |*F*|²: horizontal x =
   *k* (2r − 1), horizontal y = *k* (2g − 1), vertical = 1 + *k* (2b − 2). With no flow
   (*F* = 0) it is straight up.
7. **Layers 1 and 2** (0x2B0007FD, Direct3D 10/11 programs only, W27). With *g* =
   clamp(0.0004 × the pixel's view depth, 1, 10) (1 within 2,500 m): layer *k* coordinates =
   *B* × `Scale`ₖ + *g* × (`SpeedU`ₖ, `SpeedV`ₖ) × *T*; each texel *t*ₖ gives
   (2r − 1, 2g − 1, 2b − 1) (horizontal x, horizontal y, vertical). Foam weight *φ* = (the flow
   map's third channel at UV + (1186, 1187)) × (the foam map's alpha at (UV + (1182, 1183)) ×
   (1184, 1185)). Add both layer vectors, each × (1 − |*F*|) × (1 − *φ*), to the flow normal.
8. **Normalise.**

The OpenGL programs of 0x2B0007FD and all programs of 0x2B0005EC stop after step 6 (no
layers 1 and 2, no foam).

<a id="s8-6-3"></a>
#### 8.6.3 What this means in this install (W28; Inferred)

All 27 flow-family and lava surfaces use flow map 0x400163D3, whose flow channels give |*F*| ≤ 0.02,
and its third channel is 0. So *k* ≤ 0.0008: the flow normal is straight up for practical
purposes, the phases move the normal map by at most 0.02 repeats per cycle, and the foam weight
is 0. Consequently:

- in the Direct3D 10/11 programs of 0x2B0007FD the visible ripples are those of layers 1 and 2,
  moving at their (slow) speeds (§8.7);
- 0x2B0005EC, and 0x2B0007FD through its OpenGL programs, give an almost flat surface: no
  moving ripples; refraction, reflection and Fresnel only.

> **OPEN:** Whether the client binds another texture to the flow sampler of landscape water at
> run time (the sector coordinates of §8.1, which span a 3 × 3 sector, suggest a per-sector
> map; the data binds only 0x400163D3). An owner observation settles it: in the original, does
> the water at checklist row W17 of `m4-5-visual.md` §4.8 (a 0x2B0005EC river) show moving
> ripples? (Corrected 2026-10-09: the former text named checklist row W12, the fountain basin.)
> The same answer decides whether landblock lava moves (§8.6.5).

<a id="s8-6-4"></a>
#### 8.6.4 Colour

**OpenGL programs of 0x2B0007FD** (SH2, programs):

1. *S* = screen position; refraction *F*ᵣ = the frame-buffer copy at *S* + *N*.xy × 1048 (*N*
   the normal of §8.6.2, horizontal components); reflection *R* = the reflection texture at
   *S* + (0, 0.003), not distorted.
2. *L*꜀ = clamp(*D*ₗ + (1066 − 1) (1 − *D*ₗ), 0, 1) with *D*ₗ the fake light's diffuse colour.
3. Tint: with *w* = clamp(0.1 × eye distance − 0.5, 0, 1) × 1068, the refraction becomes
   *F*ᵣ + *w* × (*L*꜀² × `WaterFogColor` − *F*ᵣ).
4. Fresnel *f* = 1050 × (max(1 − |*v* · *N*|, 0))^(1 / 1051): the bias **multiplies** and the
   exponent is **inverted**, unlike §8.4 step 5.
5. Colour = mix(tinted refraction, *R*, *f*) × `WaterColor` × 1066 + *L*꜀ ×
   sat(*N* · normalize(*v* − *L*))^1054 × fake specular colour × `WaterSpecularColor`.
6. Fog: mixed toward the fog colour by clamp((distance − fog near − 100) / fog range, 0, 1)
   (the fog starts 100 m later than for other surfaces). Alpha 1.

**Direct3D 10/11 programs of 0x2B0007FD** (SH2, D3D) differ:

- Refraction depth: *θ*₀ = the depth texture at *S* minus the pixel's view depth (the water's
  thickness along the view); *o* = 1189 + 0.001; the refraction offset *N*.xy × 1048 is scaled by
  clamp(*θ* / |*o*|, 0, 1), tested again at the offset point, and the unoffset point is used when
  the offset one lies in front of the water; pixels with negative thickness are discarded.
- Depth colouring: shoreline colour (2011) toward fog colour (2010) by smoothsteps of the
  thickness against 1170 and 1171; the refraction is tinted by clamp((*θ* − 1171) × 1173, 0, 1)
  × 1068 × those colours' alphas.
- Fresnel as the OpenGL step 4, multiplied by clamp(mean of the reflection's RGB +
  smoothstep(clamp(distance / 75, 0, 1)), 0, 1).
- Specular: exponent fixed at 25 (1054 and 1067 are not read), plus a sun-disc term
  (*N* · −*L*)^8000 × the reflection's red, plus a glitter term from the noise texture at
  0.1 *B* + (*u*, *u*), *u* = fractional part of 0.005 *T* (one cycle per 200 s), masked by the
  normal layers' alpha and faded out by view depth; all three × *L*꜀ × fake specular colour ×
  `WaterSpecularColor` × (1 − *φ*).
- Foam: the foam texel × *L*꜀ × 2.5 × the sun-shadow factor (`lighting.md` §9, weighted by the
  depth-dependent fog amount), mixed in by *φ* × 1190; the specular terms are not shadowed.
- **Output alpha** = clamp(*θ* / ((1 + 0.098 × view depth) × |*o*|), 0, 1): soft shores.
  Blend source alpha / inverse source alpha (the OpenGL permutations write with ONE / ZERO).

**0x2B0005EC** (SH2, SH3; W42, W43; expanded 2026-10-10): the normal of §8.6.2 steps 1–6. The
record never reads the fake light's diffuse colour, so it has no *L*꜀ anywhere.

*OpenGL programs* (Confirmed, programs). They have no depth sampler: they are the record's own
form without a depth texture.

1. *S* = screen position; refraction *F*ᵣ = the frame-buffer copy at *S* + *N*.xy × 1048;
   reflection *R* = the reflection texture at *S* + *N*.xy × 1047 + (0, 0.003) (distorted,
   unlike 0x2B0007FD).
2. Tint: with *w* = clamp(0.1 × eye distance − 0.5, 0, 1) × 1068, the refraction becomes
   *F*ᵣ + *w* × (`WaterFogColor` − *F*ᵣ) (toward the fog colour itself, not *L*꜀² × it).
3. Fresnel *f* = 1050 + (1 − 1050) × (max(1 − |*v* · *N*|, 0))^1051, the form of §8.4 step 5.
4. Colour = mix(tinted refraction, *R*, *f*) × `WaterColor` × 1066 +
   sat(*N* · normalize(*v* − *L*))^1054 × fake specular colour × `WaterSpecularColor` (no *L*꜀,
   no sparkle term).
5. Fog as step 6 of the 0x2B0007FD list (starting 100 m later), each channel clamped to 0–1;
   output alpha = 1 − clamp((distance − *s*) × *r*, 0, 1) with (*s*, *r*) the engine's far-draw
   fade. These permutations write with ONE / ZERO and draw both faces, so the alpha has no
   effect.

The no-reflection OpenGL permutation computes steps 1, 2, 4 and 5 with mix(tinted refraction,
*R*, *f*) replaced by the tinted refraction: no Fresnel and no reflection term (W43). The same
holds for the no-reflection OpenGL permutation of 0x2B0007FD.

*Direct3D 10/11 programs* (BIN, W42; they have a depth sampler): the thickness *θ*₀ and the
discard of pixels with negative thickness as for 0x2B0007FD; the refraction is read at the
offset point unconditionally, and the tint uses the larger of *θ*₀ and the thickness at the
offset point: clamp((*θ* − 1171) × 1173, 0, 1) × 1068 toward `WaterFogColor`; the reflection is
read as in step 1; the Fresnel of step 3 is multiplied by clamp(mean of the reflection's RGB +
smoothstep(clamp(distance / 75, 0, 1)), 0, 1); with *a* = clamp(*θ*₀ / 1170 + 0.1, 0, 1), the
colour is mix(tinted refraction, *R*, that Fresnel) × `WaterColor` × (1 + (1066 − 1) × *a*) plus
the specular term of step 4, then fogged; output alpha = *a* × the far-draw fade (soft shores);
blend source alpha / inverse source alpha.

> **OPEN:** The exact order and weights of the Direct3D 10/11 colour terms of 0x2B0007FD
> (depth colouring, glitter mask and fade, shadow weighting) were read once and not
> cross-checked (those of 0x2B0005EC were re-read on 2026-10-10, W42); the reflection pass and
> the depth texture's contents are engine behaviour (§8.4 OPEN). (Lava, 0x2B000810: §8.6.5.)

**Tessera rule (Decision), until those are settled** (revised 2026-10-10, answering the
implementer OPEN below): use the normal of §8.6.2, with layers 1 and 2 for 0x2B0007FD (its
Direct3D 10/11 form) and steps 1–6 only for 0x2B0005EC (which has no layers 1 and 2). For the
colour, each record follows **its own** OpenGL reflection-permutation list, with the reflection
*R* replaced by the untinted refraction sample *F*ᵣ (the substitution of the §8.4 Tessera rule),
alpha 1:

- 0x2B0007FD: steps 1–6 of the first list (Fresnel bias × (…)^(1 / exponent); tint toward
  *L*꜀² × `WaterFogColor`; specular × *L*꜀);
- 0x2B0005EC: steps 1–5 of its own list (Fresnel bias + (1 − bias) × (…)^exponent; tint toward
  `WaterFogColor`; no *L*꜀).

Those lists need no depth texture: they are the records' own forms without one (W42), so the
depth tint of the Direct3D programs is not approximated. The substitution *R* = *F*ᵣ is
Tessera's, not the original's no-reflection path: the original's no-reflection permutations of
both records drop the Fresnel mix and show the tinted refraction alone (W43). A renderer with a
reflection pass uses *R* as stated. Never apply §8.3's scroll formula to 1057 and 1058 of these
records.

> **Answered 2026-10-10 (SH3, W42, W43; was OPEN, implementer, 2026-10-10, `tessera-render`
> water pipeline; checklist row W17 of `m4-5-visual.md` §4.8):** the Tessera rule above gave both records "the colour of the
> OpenGL list", whose step 4 is the Fresnel of 0x2B0007FD (bias × (1 − |*v* · *N*|)^(1 / exponent)),
> while the paragraph on 0x2B0005EC states that its own programs use the §8.4 step 5 form
> (bias + (1 − bias) × (1 − |*v* · *N*|)^exponent). Applied literally to the river surface
> 0x310066D5 (Fresnel bias 0.01) the multiplied form keeps *f* ≤ 0.01, so beyond about 15 m
> the step 3 tint (fog amount 1) pulls the whole river to *L*꜀² × `WaterFogColor`, a
> near-black band, where row W17 expects the bed seen through the water with reflection
> rising toward grazing angles. **Tessera (implementer choice until answered):** each record
> keeps its own Fresnel form (0x2B0007FD step 4; 0x2B0005EC §8.4 step 5, told apart by
> whether the record declares layers 1 and 2), the rest of the OpenGL list (refraction,
> *L*꜀, tint, colour, specular, fog) unchanged for both. Spec writer: confirm this reading of
> the Tessera rule, or state the colour of 0x2B0005EC without a depth texture (its own depth
> tint needs one) another way.
>
> **Answer.** The reading is **confirmed** for the Fresnel form: in both back ends 0x2B0005EC
> uses bias + (1 − bias) × (1 − |*v* · *N*|)^exponent and 0x2B0007FD bias ×
> (1 − |*v* · *N*|)^(1 / exponent) (W42, W43); the former single "colour of the OpenGL list"
> was the 0x2B0007FD list only. It is **corrected** for the rest: 0x2B0005EC has no *L*꜀, tints
> toward `WaterFogColor` itself and has no *L*꜀ on its specular, and its reflection offset uses
> 1047 (no effect while *R* = *F*ᵣ). Without a depth texture 0x2B0005EC is coloured by its own
> OpenGL list (steps 1–5 above), which replaces the depth tint by the eye-distance weight of
> step 2; the revised Tessera rule states both records. Telling the two records apart by
> whether they declare layers 1 and 2 (1059–1062, 1064, 1065) separates them in this install;
> so does the foam slot 3079, which only 0x2B0007FD declares. Either is an observation of this
> install's two flow-map records, not a format rule. On the W17 river 0x310066D5 (bias 0.01,
> exponent 2, fog amount 1, `WaterFogColor` (0.129, 0.149, 0.133)) the revised rule gives the
> bed seen through the water, darkened toward that fog colour where the view is steep and
> farther than 5–15 m, and the untinted bed toward grazing angles (checklist row W17 updated).

<a id="s8-6-5"></a>
#### 8.6.5 Lava: 0x2B000810 (added 2026-10-09, third revision; W40, W41)

**Status.** 0x2B000810 is the record of landblock lava: one surface record (0x31006FAE) of the
landblock-water tables, 656 wet landblocks (W23), no placed mesh (W41). It shares part of the
flow-map interface (1057, 1058, 1063, the flow, foam and noise slots and the flow, foam and
depth parameters) but has no frame-buffer, reflection or depth sampler and no water colour.
It is its own family ("Lava" in §8.2), not a member of the flow-map family and not a water
record of `material.md` §11.6; that section's lava row recognises it from its interface: it
declares 1057, has a `FlowTexture` sampler and has no `FrameBufferTexture` sampler (only
0x2B000810 of the 510 shader records, W41). The formula below is that of its Direct3D 10/11
water permutations (W40, read once, D3D). Its only OpenGL program is the low-detail
`Color` / `Color2` permutation of §8.2; its Direct3D 9 programs were not read.

**Inputs.**

- Per vertex: position; UV set 0 (on landscape quads the sector coordinates of §8.1), used as
  stored (no texture matrix).
- Slots: 3000 and 3001 (`ColorAndOpacity`, `ColorAndOpacity2`), 3018 (`Emissive`), 3075 flow
  map (clamp), 3079 foam map, the noise sampler without binding (default 0x400163D4, as
  §8.6.1). 3011 is bound but not sampled; 3013 is declared without a binding. Address modes and
  filters as `material.md` §7.5 and §7.3.1.
- Parameters read: 1057, 1058 (flow rates), 1063 (scale of the colour layers and of the
  emissive layer), 1066 (brightness), 1182–1187 (foam and depth offsets, foam tiling), 1188
  (rotation). The other declared water parameters (1047, 1048, 1050, 1051, 1054, 1067, 1068,
  1170, 1171, 1173, 1189, 1190, 2006, 2009–2011) are not read.
- No light input of any kind (no fake light, no ambient, no shadow), no frame-buffer copy.

**Colour.** With *B*, *T*, *F*, *q*, the phases *p*ᵢ, their fractions *f*ᵢ and cycle numbers
as in §8.6.2 steps 1–3, and *k* = 2 |*F*|²:

1. **Phase coordinates** *c*₁, *c*₂ as §8.6.2 step 4, rotation by 1188 included.
2. **Flow colour** *C* = the RGB of slot 3000 at *c*₁ mixed with the RGB of slot 3001 at *c*₂,
   weight *w* = 2 × |½ − *f*₁| toward the second (as §8.6.2 step 5, but each phase reads its
   own colour layer).
3. **Emissive texel** *E* = slot 3018 at UV + 1063 × *B* (RGB and alpha *E*ₐ). These
   coordinates have no time term.
4. **Emissive weight** *a* = clamp(1 + *E*ₐ − *k*, 0, 1); base colour *X* = *C* + *a* × (*E*.rgb − *C*).
5. **Brightness** *m* = 2 × 1066 × (1.8 − *a* − 0.5 *E*ₐ); colour *Y* = *m* × *X*.
6. **Foam** weight *φ* = clamp(the foam map's alpha at (UV + (1182, 1183)) × (1184, 1185),
   multiplied by the flow map's third channel at UV + (1186, 1187), 0, 1); *Z* = *Y* + *φ* ×
   (foam RGB − *Y*). On 0x31006FAE, 1186 and 1187 are sine curves; like every curve-valued
   parameter they are evaluated at the clock for every frame drawn (§8.7.1, W37).
7. **Fog and output:** each channel of *Z* + *d* × (fog colour − *Z*), clamped to 0–1, with *d*
   the distance-fog factor of §8.3 (no 100 m shift as in §8.6.4); output alpha 1.

Render state (W41): the stored block of the water permutations blends source alpha / inverse
source alpha (opaque in effect, alpha 1), tests depth, writes depth and culls nothing; the
surface's flags 4002 and 4003 are 1 and agree.

**Texture matrices (added 2026-10-09, fourth revision; OBS2).** 0x31006FAE also carries texture
matrices for sets 0 and 1: scales 1001–1004 of 6 and translations 1017–1020 that are mode-2
curves (−0.02 per second on both axes of set 0, +0.025 per second on both axes of set 1;
`material.md` §8.4, §5.3). Only the low-detail permutations of 0x2B000810 (the five that have an
extra state list, `material.md` §7.7) bind them; its water permutations have no extra state
list and read UV set 0 as stored (W40). So under the water permutations (steps 1–7) lava has no
texture-matrix motion, while under a low-detail permutation its two colour layers would scroll.
This matters for the OPEN below; the Tessera rule above does not use the low-detail
permutation.

**What this means in this install (Inferred from W28, W41).** The flow map is neutral, so *k*
≤ 0.0008 and the foam weight is 0 (its third channel is 0); *E*ₐ ≥ 0 makes *a* ≥ 0.9992, so the
colour layers contribute at most 0.08 % and the surface is the emissive layer alone:
*m* × *E*.rgb with *m* = 3 × (0.8 − 0.5 *E*ₐ) for 1066 = 1.5 (from 2.4 where *E*ₐ = 0 to 0.9
where *E*ₐ = 1), fogged. Nothing in that result depends on *T*: **landblock lava does not move
in this install**; its pattern repeats about 6 times per landblock along each axis (1063 = 6
from *B*, plus or minus ⅓ from the sector UV, which adds one repeat per 3 × 3 sector). The 1057 and 1058 values (0.1 on 0x31006FAE) only
move the colour layers, which the emissive weight hides. These are observations of one install;
a renderer reads every value at run time.

**Tessera rule (Decision), while the OPEN below stands:** draw the lava record with steps 1–7
(the Direct3D 10/11 form), on the landblock quads of §8.1 and on any placed mesh, opaque with
depth write and both faces; never with the generic path, the plain-water formula of
§8.3–§8.4 or the flow-water colour of §8.6.4. Do not substitute the low-detail permutation.

> **OPEN (2026-10-09):** whether the original's landblock lava moves (it does if the client binds
> a run-time flow map, the §8.6.3 OPEN; an owner observation at checklist row W7 of
> `m4-5-visual.md` §4.4 settles both); the Direct3D 9 programs of 0x2B000810 and which
> permutation the client picks for lava under each user setting (§8.4 OPEN).

<a id="s8-7"></a>
### 8.7 Animation rates and the time base (W24–W27; compared with the former text)

**Time base.** Every program above reads the engine's time input *T* = E20: seconds of the
client's global clock, reduced modulo 3,600 once above 3,600 (`lighting.md` L25). The float
curves of texture matrices (§8.5.3) and of every curve-valued water, lava, mirror or liquid
parameter (§8.7.1) are evaluated at the same clock, with the same reduction (`material.md` §5.3.1), every
frame. Nothing water
related runs on the game's time of day. A renderer passes real elapsed seconds, not frames or
game time.

| Family | What moves | Rate (units) |
|---|---|---|
| Plain water (§8.3) | normal layer *k* | (`SpeedU`ₖ, `SpeedV`ₖ) repeats per second; on the ground × 160 / `Scale`ₖ m/s |
| Plain water (§8.3) | waves | phase `WaveSpeed` × (0.5 + 0.8 *k*) rad/s; amplitude 0 on most surfaces |
| Flow map (§8.6) | flow phases | 1057 and 1058 cycles per second (cycle 1 / rate s); the normal map slides by \|*F*\| repeats per cycle and jumps 0.7 per cycle |
| Flow map, 0x2B0007FD D3D (§8.6) | normal layers 1, 2 | *g* × (`SpeedU`ₖ, `SpeedV`ₖ) repeats per second, × 160 / `Scale`ₖ m/s; *g* = 1 within 2,500 m |
| Flow map, 0x2B0007FD D3D | glitter noise | 0.005 noise repeats per second (0.1 repeat per landblock) |
| Flow map, 0x2B0007FD D3D | foam and depth lookups | offsets 1182, 1183, 1186, 1187: sine curves (mode 4), amplitude 0.0001–0.01 UV, angular rate `w[6]` + `w[5]` from 0.1 to 1.2 rad/s (1182: 0.3–1.2; 1183: 0.1–1; 1186: 0.7 or 1.2; 1187: 0.5 or 0.6; corrected 2026-10-10, was 0.5–1 rad/s, which left out the `w[5]` = 0.2 of 1182 and 1186 and the slower 1182, 1183 curves) |
| Lava (§8.6.5) | colour layers; emissive layer | 1057 and 1058 cycles per second as the flow phases; the emissive layer does not move (no time term); in this install the emissive layer hides the colour layers, so nothing visible moves |
| Lava (§8.6.5) | depth lookup of the foam weight | offsets 1186, 1187: sine curves (mode 4), amplitude 0.001 UV, 1.2 rad/s (1186: `w[6]` 1 + `w[5]` 0.2; corrected 2026-10-10, was 1) and 0.6 rad/s (1187) (§8.7.1); no visible effect in this install (the flow map's third channel is 0) |
| Older bump water (§8.5.6) | three layers | 1056 × (1, 4, 8) repeats per second of 1055 × (1, 2, 4) tiles of UV |
| Mirror (§8.5.4) | normal map | (1104, 1105) × 1103 repeats per second; for the puddles 0.00001 × *t* (the curve) × *T* |
| Liquid distortion (§8.5.5) | three layers, colour with layer 2 | (`SpeedU`ₖ, `SpeedV`ₖ) repeats per second of UV′ × `Scale`ₖ |
| Two-layer records (§8.5.3) | each layer's texture matrix | mode-2 curves: `w[6]` repeats per second; modes 4 and 7: `w[6]` rad/s |

**Rates of the main landblock-water surfaces** (OBS2, effective values; metres per second on
the ground, length of the vector; "former" is what the formula of §8.3, applied to every water
record, gives):

| Surface (record) | Wet landblocks | Original: visible layers | Former: §8.3 on all records |
|---|---|---|---|
| 0x31004359 (0x2B0007FD) | 24,946 | layer 1: scale 2, speed (0.0015, 0.002): 0.20 m/s; layer 2: scale 8, (−0.01, −0.0125): 0.32 m/s; flow phases 1 cycle per second, not visible (§8.6.3) | layer 0: scale 10, speed (1, 1): 22.6 m/s |
| 0x31008DB5 (0x2B0007FD) | 24,089 | layer 1: scale 1, (0.0025, 0.005): 0.89 m/s; layer 2: scale 12, (−0.01, −0.005): 0.15 m/s; phases 0.2 per second | layer 0: scale 6, (0.2, 0.2): 7.5 m/s |
| 0x31008B7F (0x2B0007FD) | 17,276 | as 0x31008DB5: 0.89 and 0.15 m/s | 7.5 m/s |
| 0x31006653 (0x2B0007FD) | 14,380 | layer 1: scale 2, (0.0035, 0.005): 0.49 m/s; layer 2: scale 12, (−0.02, −0.0125): 0.31 m/s | layer 0: scale 7.5, (0.2, 0.2): 6.0 m/s |
| 0x3100068B (0x2B0000A8) | 1,678 | layer 0: scale 4, (0.005, 0.0075): 0.36 m/s; layer 1: scale 8, (0.015, 0.02): 0.50 m/s; layer 2: scale 16, (−0.01, −0.0125): 0.16 m/s | the same |
| 0x310066D5 (0x2B0005EC) | 761 | no moving ripples (flow only, §8.6.3) | one layer: scale 10, (1, 1): 22.6 m/s |

So for 95 % of the wet landblocks the former text made the ripples drift 6–23 m/s in one fixed
direction, where the original shows ripples drifting at 0.15–0.9 m/s (Direct3D 10/11) or none.
For the plain water records the former formula and the rates are right; the time base must be
seconds (W24).

<a id="s8-7-1"></a>
#### 8.7.1 Curve-valued water parameters (added 2026-10-09; W37, W38)

A parameter of a surface of the water families (plain water, plain water on mesh UV, flow
map), the lava family (§8.6.5), the older bump water (§8.5.6), the mirror record (§8.5.4) or a
liquid-distortion record (§8.5.5) may be a float curve (`material.md` §5.3) instead of a
constant (lava and older bump water added 2026-10-09, fourth revision). **Rule (normative):** evaluate every such curve at
the global clock *t* (`material.md` §5.3.1) for every frame drawn and hand the value to the
program in place of the constant; never evaluate it once at load or at *t* = 0. The program
then uses the value exactly where it uses a constant, so a curve-valued speed is multiplied by
*T* again and a curve-valued scale multiplies *B* (or the UV). *t* and *T* are the same clock
with the same reduction modulo 3,600 s, so every product restarts together once an hour.

Census (OBS2, all 13,781 surface records, of which 124 belong to the families above; every
curve of a mode other than 1; texture-matrix ids excluded, they are §8.5.3's; W37). No other
parameter of these families is a curve of a mode other than 1 in this install; the plain water
on mesh UV, the older bump water and 0x2B0005EC have none.

| Parameter (§8.2 labels) | Mode | Surfaces | Value | Effect | Wet landblocks / TP draws (W35) |
|---|---|---|---|---|---|
| 1045 `WaveFrequency` | 2 | 0x31002A44, 0x31002A45, 0x31002A46 (0x2B0000A8) | 1 + 0.005 *t* | the wave pattern of §8.3 tightens from frequency 1 to 19 over the hour, then returns to 1; amplitude 1046 = 0.005, so the waves stay below 0.008 m | 1 / 0 (0x31002A44 only) |
| 1045 `WaveFrequency` | 7 | 0x31008446, 0x3100890B, 0x3100890C (0x2B0004B5) | 1 + 0.2 *N₁*(0.1 *t*) | the liquid's vertex waves (§8.5.5 step 2; amplitude 0.5, speed 0.35) wander slowly in frequency | – / not drawn |
| 1059, 1061 `Normal1SpeedU`, `Normal2SpeedU` | 4 | 0x3100338F (0x2B0000A8) | 0.01 − 0.0001 sin 0.25*t*; −0.01 + 0.0001 sin 0.25*t* | the U offset of layers 1 and 2 is the value × *T*; its rate of change is the value + *T* × its derivative, so the small sine grows with *T* (example in §12) | 0 / 0 |
| 1063, 1064, 1065 `Normal0Scale` … `Normal2Scale` | 4 | 0x3100328B (0x2B0000A8) | 2 + 0.05 sin(*t* + 0.33π); 1 − 0.025 sin(2*t* + 0.66π); 2.5 + 0.05 sin(2*t* + 0.99π) | the tiling breathes; at a point the pattern moves at *B* × d`Scale`ₖ/d*t* + `Speed`ₖ repeats per second, which depends on the size of *B* (§8.3, W38) | 2 / 0 |
| 1063, 1064, 1065 | 4 | 0x3100824C (0x2B0000CE, placed) | 8 + 0.1 sin(*t* + 0.33π); 4 − 0.05 sin(2*t* + 0.66π); 10 + 0.1 sin(2*t* + 0.99π) | as above | – / not drawn |
| 1105 `MirrorNormalSpeedV` | 2 | the three puddles (0x2B0000AA) | 0.00001 *t* | §8.5.4 | – / 3,409 draws (2,067 + 712 + 630) |
| 1182, 1183; 1186, 1187 (foam and depth offsets) | 4 | all 21; 18 of the 0x2B0007FD surfaces | sines, amplitude 0.0001–0.01, angular rate `w[6]` + `w[5]` 0.1–1.2 rad/s (`w[5]` = 0.2 on 20 of the 21 curves of 1182 and on all 18 of 1186, 0 elsewhere; corrected 2026-10-10) | §8.6.2 step 7 (foam weight 0 in this install, §8.6.3) | all flow-map landblocks |
| 1186, 1187 (depth offsets; added 2026-10-09, fourth revision) | 4 | 0x31006FAE (0x2B000810, lava) | sines, amplitude 0.001, 1.2 rad/s (1186: `w[6]` 1 + `w[5]` 0.2; corrected 2026-10-10, was 1 rad/s) and 0.6 rad/s (1187) | §8.6.5 step 6: they offset the flow-map lookup of the foam weight; no visible effect in this install (the flow map's third channel is 0, §8.6.5) | 656 / 0 (landblock water only, W41) |

**Tessera rule (Decision) for curve-valued scales on world-position families** (0x3100328B,
0x3100824C), while the reference of *O* is OPEN (§8.3): measure *B* for these surfaces from the
south-west corner of the landblock that contains the camera (so |*B*| stays within the view
distance in landblocks), not from the region origin; the pattern then shifts once when the
camera crosses a landblock edge (by the fractional part of `Scale`ₖ). With the region origin,
|*B*| is about 135 on the two landblocks of 0x3100328B (region 2, landblocks (39, 129) and
(39, 130)) and the breathing would sweep the pattern at hundreds of metres per second. Every other surface keeps *O* = (0, 0) of §8.3.

<a id="s8-7-2"></a>
#### 8.7.2 Rates fixed by the data (added 2026-10-09; W39)

The plain-water rates of §8.3 come from each surface's own values. Most layers drift
0.01–2.8 m/s; some surfaces are much faster by their data, for example 0x31000CA1 (layers at
6.6, 6.4 and 11.7 m/s: speeds (0.075, 0.035), (0.14, 0.08), (−0.3, −0.5) at scales 2, 4, 8),
the placed 0x31006DE4 (8.0 m/s), 0x31002A44–0x31002A46 (5.3, 6.7 and 2.7 m/s at scale 6) and
0x3100068D, 0x31000CF3 (6.3 m/s). Most fast surfaces are table entries that no wet landblock
uses (W39 lists the nine fast ones in use, 101 landblocks in all). **Rule:** draw them at these
rates; the original computes the same coordinates (W25), so nothing is to be clamped or scaled.

A normal-layer scale of 0 is legal (0x31000698, layer 1, speed (0, 0); one wet landblock): that
layer reads the same texel at every point (at coordinates `Speed`ₖ × *T*), so it adds a uniform
normal that changes only with time. A rate in metres per second is undefined for it; an
implementation never divides by a scale.

---

<a id="s9"></a>
## 9. Distant-landscape sectors and their water (kind 5)

The landblock-family records `0x805RXXYY` (`landblock.md` §7) are DB type 0x41
DISTANT_LANDSCAPE_SECTOR (BIN (a)); one exists per 3 × 3 group of landblocks, at X and Y
divisible by 3. Among other things the client reads from each (BIN (e)):

- a **sector water list**: a VLE count, then that many pairs of (u8 water type, f32 height);
- for each of the sector's nine landblocks, a "has water" byte and the index of its pair in
  the list.

The pairs are exactly the (type, *h*) values of §6.2: in every one of the 12,925 sector
records whose 3 × 3 landblocks include a wet one, every distinct (type, *h*) of those
landblocks occurs as a byte pair in the record's trailing section, with *h* bit-identical to
the 32-bit result of §6.2 step 6 (OBS). The rest of the record's layout is OPEN
(`landblock.md` §7).

**Tessera rule (Decision):** derive distant water from the block maps with §6.2, which gives
the same pairs, instead of decoding kind 5.

> **OPEN (implementer, 2026-10-06):** the sector pair stores the water type as one byte,
> while the block-map type value is a u16. How the client narrows a type value above 255 is
> not stated (none occurs: the largest observed type is 47). `tessera-world` keeps the low
> 8 bits.

---

<a id="s10"></a>
## 10. Other consumers of landblock water (BIN)

| Consumer | What it reads | Spec |
|---|---|---|
| Procedural scenery | The plane and the "has water" flag: `height_mode` 1 snaps to *h*; the water-depth and above/below-water ranges use *w* = *h* | `scenery.md` §7.4 step 11, §7.5, §11 (which points here) |
| Encounter pass | Whether the cell centre is under water (§6.3) and the water type *t* | `scenery.md` §7.7 (out of scope offline) |
| "Under water" query (camera, effects) | §6.3, then liquid volumes in the physics world | §7 |
| Movement | The volumes of §7 | `collision.md` §10.4, §12.3 step 7 |

---

<a id="s11"></a>
## 11. Invariants

| # | Invariant | Status |
|---|---|---|
| WA1 | Every 0x16 record parses exactly with §4.1–§4.2; `has_default` and `dry` are 0 or 1 | Confirmed, 5 of 5 |
| WA2 | Every 0x17 record parses exactly with §5.1–§5.2 using the newest layout | Confirmed, 5 of 5 |
| WA3 | Every non-dry entry's `surface` is an existing 0x31 record and `volume_entity` an existing 0x47 record | Confirmed (125 of 125) |
| WA4 | Every block map has the water table's `type_key` and `level_key` and the fog table's `zone_key` | Confirmed (390,150) |
| WA5 | For every wet landblock, the (type, *h*) pair occurs in the distant-sector record of its 3 × 3 group | Confirmed (12,925 sectors) |
| WA6 | §6.2 is a pure function of the water table, the block map and the height grid (deterministic) | BIN |
| WA7 | Every surface record whose shader record declares a `c_Water…` parameter, a mirror parameter (1100–1106) or slot 3075 or 3079 belongs to exactly one family of §8.2 "Program families" or §8.5 (mirror, liquid distortion) | Confirmed (OBS2, all 13,781 surface records) |
| WA8 | Plain water family: a normal layer's coordinates at *T* + 1 minus those at *T* equal (`SpeedU`ₖ, `SpeedV`ₖ) exactly, independent of position (§8.3) | Synthetic |
| WA9 | Flow-map family: with *F* = 0 the normal of §8.6.2 steps 1–6 is straight up for any normal map and any *T*; with layers 1 and 2 (step 7) the normal varies with *T* only through those layers (§8.6.2) | Synthetic |
| WA10 | Flow-map family: the layer-0 coordinates are continuous within a cycle and jump by exactly 0.7 on both axes (plus −*F*) when a phase wraps (§8.6.2 step 4) | Synthetic |
| WA11 | Every flow-family and lava surface names a flow map whose two flow channels give \|*F*\| ≤ 0.02 in this install (observation, read at run time, never assumed) (§8.6.3) | Confirmed (27 of 27) |
| WA12 | Every placed object with liquid-volume monitors (§7.3) has at least one surface of a water-family record (§8.2) | Confirmed (TP, 39 of 39 objects with parts) |
| WA13 | Curve-valued parameters (§8.7.1): with a constant curve (mode 1) the drawing at clock *t* equals the drawing with that constant; with a mode-2 speed *S*(*t*) = *a* + *b t*, the layer offset at *T* is (*a* + *b T*) × *T* (not *a* × *T*); a value evaluated at *t* = 0 and held fails this for every *b* ≠ 0 | Synthetic |
| WA14 | A normal-layer scale of 0 gives the same layer texel at every point of the surface at a given *T*, and no division by zero or non-finite value arises anywhere in the water, mirror or liquid drawings (§8.7.2) | Synthetic |
| WA15 | Mirror surfaces with 1102 = 1 have output alpha = diffuse alpha × material alpha at every view angle (§8.5.4) | Synthetic |
| WA16 | Lava (§8.6.5), with constant parameters, the flow map's third channel 0 and fog factor 0: with *F* = 0 the colour equals clamp(2 × 1066 × (0.8 − 0.5 *E*ₐ) × *E*.rgb, 0, 1) for any colour layers and any *T*; with *F* ≠ 0 and *E*ₐ = 0 it varies with *T* through the colour layers only; output alpha is 1 in every case; exactly one shader record of the install satisfies the lava recognition rule of `material.md` §11.6 (observation, read at run time) | Synthetic; Confirmed (OBS2, 1 of 510) |
| WA17 | Flow-map Fresnel per record (§8.6.4): for 0x2B0007FD-form records *f* = 0 at \|*v* · *N*\| = 1 and *f* = bias at \|*v* · *N*\| = 0; for 0x2B0005EC-form records *f* = bias at \|*v* · *N*\| = 1 and *f* = 1 at \|*v* · *N*\| = 0; with *R* = *F*ᵣ and *w* = 0 (eye distance ≤ 5 m) both records give *F*ᵣ × `WaterColor` × 1066 plus their own specular term; the 0x2B0005EC colour never depends on the fake light's diffuse colour | Synthetic |
| WA18 | Liquid distortion (§8.5.5): a pixel whose texel alpha × vertex alpha × material alpha is at or below the reference is discarded in 0x2B00063E even when the texel alpha alone is above it; the output alpha equals that product | Synthetic |
| WA19 | Lit liquid distortion (0x2B0007D0, §8.5.5): a pixel whose distorted `ColorAndOpacity` texel alpha is above the reference is kept even when texel alpha × material alpha is at or below it, and its output alpha is that product; a pixel whose texel alpha is at or below the reference is discarded at any material alpha. `Render_DepthBias_Type` (§14 (a)): for a synthetic SED with an instance set, a value on the 0x47 object and none in either set gives the object's value; a value in both sets and on the object gives the sets' value | Synthetic |

Hostile input: bound every count by the bytes left; reject flag bytes other than 0 and 1;
treat a non-finite `value` or fog float as malformed; never index the entry list by key
without checking it exists.

> **OPEN:** Which fog-zone floats "fog float" covers, and whether the client itself rejects
> non-finite values (no BIN statement). The `tessera-formats` decoder (M4.5) rejects a
> non-finite value in every f32 of a fog-zone entry (`near`, `far`, `amount`, `value_60`,
> `light_a`, `light_b`) and in the water entry's `value`; all 10 entries and 125 water
> entries of this install are finite. It keeps `active` (§5.2 field 7, typed u8, not bool8)
> as a raw byte and treats any non-zero value as active, and keeps the override `mode`
> bytes raw.

---

<a id="s12"></a>
## 12. Worked examples (synthetic)

**Water table.** A table with `level_key` 4, `type_key` 7, a dry default and two entries
(key 1: surface 0x31000001, volume entity 0x47000001, value 24.0; key 2: dry) is
4 + 4 + 4 + 4 + 1 + 5 + 1 + 33 + 5 = 61 bytes: DID, 0, 4, 7, byte 1, (key 0, byte 1),
VLE 2, (key 1, byte 0, 0x31000001, four zero DIDs, 0x47000001, 24.0), (key 2, byte 1).

**Landblock water.** Block map: key 7 → 1, key 4 → 38. Height grid minimum stored value
3,000 → 3,000 × 2,000 / 65,535 = 91.55 units. *h* = 38 × 1000 / 255 = 149.0196 > 91.55, so the
block has water of type 1 at 149.0196. A point at block-local z = 148.5 is under water
(148.5 − 149.0196 = −0.5196 < −0.0002) and inside the wading box of a 0x47000754-like volume
entity (−1 ≤ −0.5196 < −0.5 relative to the entity at *h*). With key 4 → 23 instead,
*h* = 90.196 < 91.55: no water although the type is set.

**Fog zones.** A zone table with `zone_key` 3 whose entries are all inactive, or block maps
whose key 3 is 0, leaves the sky's fog unchanged.

**Plain water rate (§8.3, §8.7).** Layer 0 with `Normal0Scale` 4 and speeds (0.005, 0.0075):
one repeat covers 160 / 4 = 40 m, so the pattern moves (0.2, 0.3) m/s, 0.36 m/s along the
diagonal. At *T* = 10 s a point with *B* = (2.5, 1.0) samples (10.05, 4.075).

**Flow phases (§8.6.2).** Rate 0.2 (both 1057 and 1058), noise offset *q* = 0.1, *T* = 7 s:
*p*₁ = 0.1 + 1.4 = 1.5, so *f*₁ = 0.5, *i*₁ = 1; *p*₂ = 0.1 + 1.4 + 0.5 = 2.0, so *f*₂ = 0,
*i*₂ = 2. The blend weight 2 × |0.5 − 0.5| = 0 takes the first sample alone, at
1063 × *B* + (0.7, 0.7) + 0.5 *F*. At *T* = 9.5 s, *p*₁ = 2.0: the first phase wraps to *f*₁ = 0,
*i*₁ = 2, so its coordinates go from 1063 *B* + (0.7, 0.7) + *F* (just before) to
1063 *B* + (1.4, 1.4), a jump of (0.7, 0.7) − *F*; there the weight is 1, so the jump is hidden
behind the second sample.

**Puddle normal scroll (§8.5.4).** 1105 is a mode-2 curve with `w[6]` = 0.00001, so at *t* =
*T* = 100 s its value is 0.001 and the V offset is 0.001 × 100 = 0.1 repeats (× 1103 = 1); at
1,000 s it is 10 repeats, moving 0.02 repeats per second.

**Curve-valued speed (§8.7.1).** 0x3100338F, layer 1 U: *S*(*t*) = 0.01 − 0.0001 sin 0.25*t*,
scale 5 (one repeat = 32 m). At *t* = *T* = 100 s, *S* = 0.010013 and the offset is
*S* × *T* = 1.0013 repeats; the U offset changes at *S* + *T* × d*S*/d*t* = 0.0075 repeats per
second (0.24 m/s). At *T* = 3,000 s, *S* = 0.009925, the offset is 29.78 repeats and the rate is
0.060 repeats per second (1.9 m/s): the term *T* × d*S*/d*t* (amplitude 0.000025 *T*, period
8π ≈ 25 s) outgrows *S*. A renderer that evaluates *S* once at *t* = 0 gets 0.01 repeats per
second at all times.

**Curve-valued scale (§8.7.1).** 0x3100328B, layer 0: `Scale`₀(*t*) = 2 + 0.05 sin(*t* + 0.33π),
speed (0.005, 0.0075). At *t* = 0, `Scale`₀ = 2.043 and d`Scale`₀/d*t* = 0.0255 per second. At
*B* = (0, 0) the U coordinate changes at 0.005 repeats per second (0.39 m/s on the ground); at
*B* = (2, 0) at 2 × 0.0255 + 0.005 = 0.056 repeats per second (4.4 m/s). There the rate is
0.1 cos(*t* + 0.33π) + 0.005 repeats per second, so the direction reverses about every π
seconds; the constant 0.005 shifts the zero crossings, and the intervals alternate between
about π + 0.1 s and π − 0.1 s.

---

<a id="s13"></a>
## 13. Corrections and notes for other specs

| Spec | Statement | Correction |
|---|---|---|
| `environment.md` §4, §8 (before 2026-10-05 revision) | 0x16 "terrain type → surface palette", 0x17 "water normal map or FOG_DESC" with a texture list 0x400043FA / 0x4000460C | 0x16 is the water table (§4), 0x17 the fog-zone table (§5). The "texture lists" were four bytes straddling the f32 values 500.0 (or 9,000.0) and 10,000.0 of the fog entry; neither record holds a texture list. Revised in `environment.md` the same day. |
| `scenery.md` §11, S14, §15 item 11 | Water from the slot-6 table (0x17) and block-map key 3; "no landblock has a water surface"; `height_mode` 1 never places anything | Water comes from the slot-5 table (0x16) and keys 7 (type) and 4 (level); key 3 is the fog-zone key. 89,913 landblocks have water (§6.4), so `height_mode` 1 nodes and the water ranges of `scenery.md` §7.5 apply there. The height formula (level × 1000 / 255) and the presence rule there are right. **Done:** `scenery.md` §11, S14 and §15 item 11 now point to §6.2–§6.3 (scenery revision of 2026-10-05). |
| `terrain-texturing.md` §11, §15 (before its 2026-10-05 revision) | Slot 5 (0x16) is the water table naming a water-shader surface and a 0x47 physics object; "no terrain record holds a water level" | Right about the table. Water levels are block-map values (keys 4 and 7), §6; the 0x47 object is the volume entity of §7.1. **Done:** `terrain-texturing.md` §11 now only points to this spec. |
| `material.md` §20.1 (rows "§18 unlit blended records" and "§11.3 unlit") | 0x2B0000A8 described as a `Color`-role record blended by 2000 alpha and lit by ambient, hemispheric and per-vertex lights in pass 0x00; "442 surfaces of the 0x2B0000A8 kind" | Only 55 surface records name 0x2B0000A8 (43 water). The description fits its low-detail permutations only; its water permutations take positions only, composite the frame-buffer copy themselves and (without a depth texture) write with ONE/ZERO (§8.2 table, §8.4). If "of the 0x2B0000A8 kind" means a family of similar records, the count should name that family. For the material owner. |
| `lighting.md` §4.3 (last sentence) | 0x2B0000A8 is a "two-layer record" whose colour layers read UV set 0 and differ only in the texture matrix | True of its low-detail permutations only; its water permutations read no UV set (positions only, §8.3). For the lighting owner. |
| `landblock.md` §5.6 | Keys 3, 4, 6, 7, 0x10000012 unresolved | Resolved there (revised the same day): 3 fog zone, 4 water level, 7 water type (this spec), 6 encounter zone. |
| `collision.md` §10.4 OPEN, Q8; §12.3 step 7 | Where water volumes are placed and how deep water is | §6.2 (landblock water), §7.1 (volume boxes), §7.3 (placed volumes). The state rule is BIN (§7.4, `collision.md` §11.3.2); the containment test and buoyancy stay OPEN. |
| `terrain.md` §9 OPEN (water) | No water level to shade the shore against | §6 gives the level per landblock. |
| `walker.md` §9 OPEN | Water not drawn | §8 gives the geometry and shading. |
| `material.md` §8.7 OPEN | Water effect-family parameters | §8.2–§8.4 for the parameters the 0x2B0000A8 programs read; the flow, foam and depth parameters stay OPEN. **2026-10-09:** flow, foam and depth parameters answered in §8.6; mirror parameters in §8.5.4. |
| This spec, §8.3–§8.4 and §8.5 (before the revision of 2026-10-09) | The 0x2B0000A8 formula was stated as the water drawing; §8.5 said placed water uses the water shaders "and the same shading applies" | The formula covers the plain water family only (§8.2 "Program families"); the flow-map records, 95 % of the wet landblocks, follow §8.6, and placed surfaces follow the routing rule of §8.5 (water records, mirror, liquid distortion, two-layer records with animated matrices). |
| `render.md` §5.3 (Inputs, Vertex stage), `walker.md` §14.6 | Every landscape-water record is drawn with §8.3–§8.4, records without layers 1 and 2 with one layer at `Normal0Speed`; undeclared ids take the 0x2B0000A8 defaults | For 0x2B0007FD and 0x2B0005EC use §8.6 (1057 and 1058 are cycle rates, not scroll speeds; the former drawing drifts 6–23 m/s, §8.7). For the render and walker owners; departure row R-D12 in `render.md` §7. |
| `walker.md` §14.5 ("Texture transforms and second colour layers are not drawn"), §14.6 ("Placed water meshes are ordinary objects") | Placed water drawn by the generic lit path without second layer and texture matrices | Route placed surfaces by §8.5; draw both colour layers and the animated matrices (`material.md` §8.4, §10.3). This is why waterfalls and puddles show static, near-white surfaces and fountain water a near-white texture. For the walker owner. |
| `lighting.md` §5.2 item 2 (Tessera Decision "multiply for `…Opacity2` roles") | Multiply the two colour layers of `ColorAndOpacity` records | Several waterfall and foam records add them; the per-record table is `material.md` §10.3 (revised there the same day). |
| `m4-5-visual.md` row W8 | Surface 0x31006748 "unlit with source-alpha / inverse-source-alpha blending" | It is a plain water record surface drawn by the water programs (§8.5.2); the row is corrected there. |
| `walker.md` §14.6 (the walker's water parameters, as built on 2026-10-09: evaluated once at *t* = 0) | Water parameters taken as constants | Curve-valued parameters are evaluated at the clock every frame (§8.7.1); 0x3100328B is drawn by two wet landblocks with breathing scales and 0x31002A44 by one with a changing wave frequency. For the walker owner (OPEN in `walker.md` §14.6). |
| This spec, §8.5.4 (before 2026-10-09, second revision) | The mirror record was described by its three puddle surfaces only | Six static mirror surfaces use it too, four of them with Fresnel bias 1 (opaque mirrors) (§8.5.4, W36). |
| `material.md` §11.6 (as accepted 2026-10-09) | The lava record 0x2B000810 "falls outside" the records the generic path must not draw | It is not a water record but must not be drawn by the generic path either: lava row and recognition rule added there (revised the same day); drawing §8.6.5. |
| `render.md` §5.7 OPEN (1), `walker.md` §14.6 OPEN (R-D12) | Name the flow-map records 0x2B0007FD and 0x2B0005EC as the landscape-water records to move off §8.3 | Landblock lava (0x2B000810, 656 wet landblocks) also leaves §8.3: draw it by §8.6.5. **Done 2026-10-10:** `render.md` R-D12, §5.7 OPEN (1) and both `walker.md` §14.6 OPENs name lava. |
| This spec, §8.2, §8.5, §8.6 (before 2026-10-09, third revision) | 0x2B000810 listed in the flow-map family and as sampling a frame-buffer copy, yet "not traced" | Its own family, no frame-buffer copy, formula in §8.6.5 (W40, W41). |
| This spec, §8.6.4 Tessera rule (before 2026-10-10) | Both flow-map records drawn with "the colour of the OpenGL list", which was the list of 0x2B0007FD | Each record keeps its own OpenGL list: 0x2B0005EC has the §8.4 step 5 Fresnel form, no *L*꜀ and a tint toward `WaterFogColor` itself (W42); its OpenGL list is its form without a depth texture. |
| This spec, W39, §8.7 and §8.7.1 rates of 1182, 1186 (before 2026-10-10) | 21 fast plain-water surfaces; foam and depth offsets at 0.5–1 rad/s, lava 1186 at 1 rad/s | 20 fast surfaces (counting method in W39); the `w[5]` term adds 0.2 rad/s to 1182 and 1186: 0.1–1.2 rad/s, lava 1186 at 1.2 rad/s. |
| `m4-5-visual.md` row W13 (as written 2026-10-09) | The puddle shows the texture's soft outline | That puddle lies 80 % below the ground (W44); its outline follows the ground where the terrain rises above the quad, as Tessera draws it (§8.5.4 answer). Corrected there the same day; the expression (1 − sin 30°)^1.33 is 0.40, not 0.41. |

---

<a id="s14"></a>
## 14. Open questions

> **OPEN:** The four `extra` DIDs and the float `value` of a water entry (§4.2).

> **OPEN:** Fog-zone override modes, the remaining fog-zone fields, the corner blending, and
> the archive version that selects optional fog fields (§5). Moot in this install (§5.4).

> **OPEN:** Whether the original draws near water with the sector quads of §8.1 or with
> another builder; the reflection pass; the permutation choice; the depth, flow and foam
> formulas; water draw order (§8.1, §8.4).

> **OPEN:** Which point or shape of a mover the liquid monitor volumes test, and the client's
> buoyancy (how the float height keeps a swimmer at its depth). The state rule itself is
> answered (§7.4, BIN, W19 and W22; `collision.md` §11.3.2, Q9).

> **OPEN:** The full layout of the distant-landscape sector record (§9, `landblock.md` §7).

> **OPEN (2026-10-09):** Whether the client binds a run-time texture to the flow sampler of
> landscape water (§8.6.3), which also decides whether landblock lava moves (§8.6.5); the exact
> Direct3D 10/11 colour terms of the flow-map records (§8.6.4); the Direct3D 9 programs of the
> lava record 0x2B000810 (§8.6.5); the pixel formula of the older bump-water records
> (§8.5.6); the source of the splash and mist at the foot of falls (§8.5.1); which permutation
> (with or without reflection, depth and interactive ripples) the client chooses for water and
> puddles under each user setting (§8.4).

> **OPEN (2026-10-09, second revision):** the reference position and stored offset behind *O*
> (§8.3, W38), which decide how fast the curve-valued scales of 0x3100328B and 0x3100824C move
> (§8.7.1; Tessera Decision there).

> **Answered 2026-10-10 (OBS3, SH3, W46–W48; was OPEN, implementer, 2026-10-09,
> `tessera-material`, `tessera-render` and the walker; engineering choices, no format fact):** (a) §8.2 tells plain water from plain water on mesh
> UV (0x2B00084B, 0x2B00089A, 0x2B000A8D) and §8.5.5 the liquid-distortion record whose layers
> read the world position (0x2B000442) only by DID; no interface rule is given, so Tessera
> recognises these four records by DID (`tessera_material::water::PLAIN_UV_RECORDS`,
> `tessera_material::liquid::LIQUID_WORLD_RECORDS`) and every other water record by the
> interface rules of `material.md` §11.6 (older bump water: declares 1056; flow-map water: a
> 3075 slot, with layers 1 and 2 when it declares 1059). An interface rule would let a
> renderer do without fixed DIDs. (b) §8.6.1 names "a sampler without binding" as the noise
> source; Tessera takes the first non-zero default texture of a sampler that no texture
> binding names, in permutation and sampler order (at most eight kept per record). (c) The
> mirror and liquid-distortion drawings of placed meshes multiply the walker's draw-distance
> fade into the material alpha; the water and lava drawings, which write with ONE / ZERO, do
> not fade. (d) A water-table entry whose surface is a mirror or liquid-distortion record (none
> in this install) is drawn as plain water with the 0x2B0000A8 defaults of §8.2. (e) The view
> depth of §8.6.2 step 7 and of the depth fog is the clip-space *w* of the pixel. (f) Texture
> lists of these drawings are uploaded as sRGB like every walker texture and re-encoded to the
> stored values after sampling (flow, foam and noise maps included), so the flow vector of
> §8.6.2 step 1 reads the stored channels.
>
> **Answer**, item by item:
>
> - **(a)** For 0x2B000442 an interface rule exists: of the liquid-distortion records it is the
>   one whose permutations with programs carry no texture-transform item (`material.md` §7.7),
>   so its layers cannot read UV′ and use *B* (W46); a renderer may use that in place of the
>   DID. For the plain water on mesh UV no rule exists in the decoded record: 0x2B000A8D has the
>   same declared ids, sampler names and extra state lists in its water permutations as the
>   plain-water record 0x2B0000CE, and the difference (UV input, no block-offset constant) lies
>   inside the programs, which Tessera does not read (`material.md` §7.4). **Tessera Decision
>   (kept):** recognise 0x2B00084B, 0x2B00089A and 0x2B000A8D by DID, and 0x2B00063E likewise
>   (§8.5.5); these lists are observations of this install and are re-checked when the data
>   changes.
> - **(b)** Confirmed as equivalent in this install: in every water permutation of the three
>   records that have a noise input the only sampler with a non-zero default texture is the one
>   named `NoiseTexture` (interoperability name), unbound, holding 0x400163D4 (W47). Tessera's
>   rule picks exactly that texture. The name is the more direct key; either is acceptable.
> - **(c)** **Tessera Decision (kept).** How the original hands an object's 1.5 s fade
>   (`mesh.md` §8.7; the opacity product of §8.8, row M46) to the material draw is not traced,
>   so no client rule says whether the water drawings fade; with ONE / ZERO writes an alpha
>   fade cannot show in any case.
> - **(d)** **Tessera Decision (kept);** no such entry occurs in this install.
> - **(e)** **Confirmed as the original's** (W48): the programs read the clip-space *w* of the
>   vertex position, interpolated to the pixel, and compare the depth texture with it.
> - **(f)** **Tessera Decision (kept).** The original samples and filters the stored values (no
>   sRGB textures or targets, `material.md` §20.1, BIN-T (g)); decoding and re-encoding gives
>   the stored value at each texel and differs only in filtering between texels. Sampling
>   these lists through a non-sRGB view would match exactly (`render.md` §4.8 last OPEN).

> **Answered 2026-10-10, second revision (OBS4, BIN5, SH4, W49–W52; (b) partly, its rest is the
> OPEN below; was OPEN, implementer, 2026-10-10, `tessera-world`, `tessera-render` and the walker;
> engineering choices made while conforming to the revision of 2026-10-10, no format fact):**
> (a) For the §8.5.4 puddle rule, Tessera reads `Render_DepthBias_Type` in the lookup order of
> the LOD class (`walker.md` §11.4.3: the instance set of the weenie list, the row's own set,
> then the 0x47 chain); `decals.md` §6.5 says only "from the entity's properties". (b) The
> entry's slope-scale value is applied to every own-program draw (water, mirror, liquid
> distortion, lava) of an entity that sets it, not only to the mirror record; the table's
> absolute and maximum values are not applied (none is set in this install; `decals.md` §6.5
> OPEN (b)); the magnitude is capped at 64. Generic-path draws of entities still apply no
> depth bias (`decals.md` §6.5 OPEN (c) is not implemented for them). (c) 0x2B000442 is still
> recognised by DID: the W46 interface rule needs the texture-transform items per permutation
> with programs, which Tessera's decoded interface does not keep (`material.md` §7.7 merged
> table). (d) The unlit liquids test their output alpha (W45); the lit record 0x2B0007D0 keeps
> the test on the texel alpha before any vertex multiply (`lighting.md` §5.12, §5.13), which
> W45 does not cover. Spec writer: confirm (a) and (d), or state the original's rule.
>
> **Answer**, item by item:
>
> - **(a) Confirmed as equivalent in this install.** The client's render component watches
>   `Render_DepthBias_Type` and learns it through the property-change notifications of the
>   entity's layered property collection, the same path as `Render_LODClass` (`walker.md`
>   §11.4.3; W49). The value it gets is therefore the first one the entity's lookup order finds
>   (`entity-descriptor.md` §9.2, E26): the per-instance set (the landblock's instance property
>   set for the IID when one exists, otherwise the SED's own set; never both), then the
>   physics object's 0x47 chain from the named object towards its parents; for a 0x70 object,
>   its property set and then its `PhysObj` chain (E24). Tessera's order (instance set, row
>   set, chain) differs from this only for a SED whose instance set lacks a value that its own
>   set holds. That never happens for this property here (W50): every SED that finds a value
>   has an instance set, the 49,338 that carry the value on the SED carry the same value in the
>   instance set, 2,690 find it only along the chain, and the one SED whose own value differs
>   from its object's (0x802515A0, object 0x4700C32A: 4 against 3) gets 4 in both orders.
>   Tessera may keep its order. To follow the client exactly, skip the row's own set when the
>   instance set exists. An entity none of whose sources holds the property receives no
>   notification; the client's initial value in the component was not resolved to a table
>   entry, so **Tessera Decision:** entry 0 (no bias).
> - **(b) Partly answered.** **Table values (BIN5, W51):** a graphics back end builds one
>   rasteriser-state object per depth-bias index (key bits 4–7, so 0–15) and fills it with all
>   three values of the entry, absolute, slope-scale and maximum, so the original applies the
>   absolute and maximum values too. How a back end converts the absolute value to its depth
>   units was not traced. Both are absent (0) in every definition of this install
>   (`decals.md` §6.5), so leaving them out changes nothing here. If an install sets them,
>   apply the maximum as the rasteriser's bias clamp and keep the absolute value OPEN
>   (`decals.md` §6.5 OPEN (b)). The magnitude cap of 64 is a hostile-input bound; it never
>   binds here (|value| ≤ 10). **Which draws (OPEN, see below):** the component stores the
>   delivered integer unchanged (W49), but no reader of the stored value was found, so the
>   original's choice of draws is not known. **Tessera default (revised):** `decals.md` §6.5
>   OPEN (c) applies the entry to every draw of the entity, so apply it to generic-path draws
>   as well as own-program draws (water, mirror, liquid distortion, lava). Tessera's current
>   state (own-program draws only) is a partial implementation of that default.
> - **(c) Answered (OBS4, W52):** two rules on data that a record-wide merged table keeps
>   select exactly 0x2B000442 among the liquid-distortion records of `material.md` §11.6:
>   (i) none of its declarations has a binding with a semantic of 38–44 (its five transform
>   bindings, ids 1001, 1002, 1017, 1018 and 1033, use semantics 33–37, and all five name its
>   one permutation without programs); (ii) no permutation holds a type-5 extra state item
>   (its only items are of types 3, 2 and 1, in that permutation). 0x2B0004B5 and 0x2B00063E
>   carry a type-5 item bound with 38–44 in each of their three permutations with programs,
>   and 0x2B0007D0 in each of its 162. These rules describe four records of this install, not
>   a client test (the client runs each record's programs and recognises nothing). A renderer
>   may use either rule in place of the DID. The DID remains acceptable as a Tessera Decision,
>   re-checked when the data changes.
> - **(d) Confirmed (SH4, W52):** 0x2B0007D0 discards when the alpha of the `ColorAndOpacity`
>   texel, sampled at the distorted coordinates of §8.5.5 step 4, is at or below the
>   reference. The test runs before any multiply: in all 88 Direct3D 10/11 and 34 OpenGL pixel
>   programs that test (the two depth-only programs of pass 0x09 in each back end have no
>   test). The record's vertex programs take no colour input, so no vertex multiply occurs at
>   all. Output alpha = texel alpha × material alpha. Tessera's rule is the original's; W45
>   (unlit records) is unchanged.

> **Answered 2026-10-10, third revision (BIN6, W53; was OPEN, 2026-10-10, BIN5, W51):** which
> draws of a placed entity the client draws with the depth-bias entry its render component
> holds: every draw of the entity, only some passes (for example not the shadow or depth
> passes), or none outside special paths. The session found neither a reader of the
> component's stored value nor a call of the back end's index setter that takes it.
> **Tessera default:** every draw of the entity, generic path included (`decals.md` §6.5
> OPEN (c)).
>
> **Answer.** Every draw of the entity's scene node, in every pass it draws: the node sets the
> device's index to the component's entry, draws all the object's parts (own-program
> surfaces such as water, mirror, liquid distortion and lava are parts like the generic ones)
> and resets it to 0. The depth pass 0x09 uses the entry unchanged; the shadow-map pass 0x0A
> uses it too, with entry 2 when the entry is 0. The component's entry starts at 0. BIN5 had
> searched the wrong slot of the device's method table. The rules, with the cross-fade case,
> the other users of the index and Tessera's shadow-pass deviation, are in `decals.md` §6.5
> ("Which draws apply it", "Other draws" and its answered implementer OPEN).

> **Answered 2026-10-10, third revision (OBS5, W54; was OPEN, implementer, 2026-10-10,
> `tessera-material`; engineering choice, no format fact):** Tessera now recognises the
> world-position liquid of §8.5.5 step 1 by rule (i) of W52 alone (a liquid-distortion
> record none of whose declarations has a binding with a semantic of 38–44; bindings with the
> older semantics 33–37 do not count), not by its DID. Rule (ii) (no type-5 extra state item)
> is not used because the decoded interface does not keep the extra state lists; a real-data
> test checks that both rules select only 0x2B000442 in this install. If a later install has
> a record for which the two rules disagree, which one decides is open.
>
> **Answer.** Neither rule is the client's: the client runs each record's programs and
> recognises nothing. What decides is the W46 property, whether the record's permutations
> with programs change texture matrix 0. On this install the two rules never disagree: over
> all 510 shader records they select the same records (W54), and among the liquid-distortion
> records both select 0x2B000442 alone (W52). A disagreement can only be a record with a
> type-5 item but no binding with a semantic of 38–44, since every such binding names a
> type-5 item (`material.md` MT12). **Rule for a disagreement (Tessera Decision):** world
> position only when both rules hold; otherwise read UV′ through texture matrix 0, the
> family's usual coordinates (three of the four records), and log the record, since it lies
> outside this spec's census. Tessera's use of rule (i) alone stays acceptable while it
> cannot read rule (ii), with the real-data test as it is. A better test that the decoded
> interface may already allow: "no binding with a semantic of 33–44 names a permutation with
> programs" holds for exactly the records whose programmed permutations change no texture
> matrix 0, on all 510 records (W54); it needs each binding's permutation index and whether
> that permutation has programs (`material.md` §7.3, §7.4). Use any of these rules only
> inside the liquid-distortion family of `material.md` §11.6: 0x2B0000B7, outside it,
> satisfies (i) and (ii) although its programs transform UV with items of types 1–3.

---

<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version. Settles what prefixes 0x16 and 0x17 are (BIN); full layouts with censuses; landblock water rule and census (89,913 wet landblocks); water volumes; drawing (geometry BIN, shading SH); distant water pairs; corrections to `environment.md`, `scenery.md`, `terrain-texturing.md`, `landblock.md`, `collision.md`, `terrain.md`, `walker.md`, `material.md`. |
| 2026-10-05 | Revision after review (changes requested). Legend: **Inferred** defined. New rows W16 (0x2B0000A8 users and permutations, OBS + SH) and W17 (fake-light direction, from `lighting.md` §4.2). §8.2: 0x2B0000A8 user count (55 surface records, 43 water) and a permutation table saying which statements cover which permutation; the "other declared water parameters" list no longer includes 2006, which the table already covers. §8.4 step 6: *L* is the main light's travel direction (or an engine default), adopted from `lighting.md` §4.2. §10: scenery row updated. §13: scenery and terrain-texturing rows marked done; correction rows added for `material.md` §20.1 and `lighting.md` §4.3. |
| 2026-10-06 | Second revision after re-review (changes requested). §8.4 step 6 restated as one formula: the sparkle term 4 · (clamp(*g*ₓ − threshold) + clamp(*g*ᵧ − threshold)) is added to the specular term after the exponent, outside the clamp (SH: the OpenGL water program and all four Direct3D 11 water pixel programs of 0x2B0000A8 agree); *L* is defined in a separate sentence. §8.4 step 1: the sparkle input is the normalised tangent-space normal (was wrongly "unnormalised sum"). §8.2 permutation table and §8.3: the water programs read only the position; the four Direct3D 11 vertex programs also declare a normal and one UV set in their input signature, unused (SH, all four checked). |
| 2026-10-06 | Movement edit (M4.5 criterion 4). §7.4: the float-line Decision (OPEN (p)) now states the state-6 interaction — a mover that reaches deep water while sliding (state 6) has its downward speed zeroed at the float line and switches to state 2; cross-references `collision.md` §11.3 mechanism and §12.3 OPEN (p). No layout or shading change. |
| 2026-10-08 | Revision answering the implementer OPENs of 2026-10-07 from the binary (BIN second use, questions (g)–(i)). Status Revised. §7.2: the volume flags count only as Boolean true (W18), confirming the implementer's Decision. §7.4: new client rule (BIN, W19): `Entity_Element` from the mover's `Volume_IsLiquid` and its support flags, with no depth or float-height input; Tessera's float-height test kept as a Decision with the one case where it differs stated; the contiguous-liquid surface of the 2026-10-07 OPEN confirmed as the Tessera Decision for the float line. §7.1: partial answer to the 2026-10-06 OPEN (offset added to both corners, W20). §7.3: coin volumes count as water for the movement state (BIN). §7.4: the 2026-10-06 no-support OPEN answered (state 2 is the client's state). §13 row and §14 OPEN narrowed. New rows W18–W20. |
| 2026-10-08 | Revision (status Revised). §7.4: the clean-room OPEN of the M4.5 exit verification (how the original slows a mover entering deep water at speed) is answered from BIN2 (g) and closed: no cancelling or clamping of a downward speed or falling displacement in water; the Swimming layer's gravity and once-per-update damper slow the mover, and the first rising update puts the feet on the float line; worked example at 60 and 30 updates per second; Tessera's current rule (`collision.md` §12.4 (p), with the state-6 case) is kept as a Decision until aligned, and its difference from the original is stated. New tag BIN2, row W18 (renumbered W21 in the merge of 2026-10-09). No layout or shading change. |
| 2026-10-09 | Merge of the two parallel 2026-10-08 revisions: rows W18–W20 and BIN questions (g)–(i) kept; the other revision's tag BIN2 kept and its row W18 renumbered W21 (references in the header and §7.4 updated); header and review record combined; the disagreement between W19 (no float-height input) and BIN2 step 1 of §7.4 (and `collision.md` §11.6 item 7) recorded as an OPEN in §7.4. No fact changed. |
| 2026-10-09 | Spec writer (dirty room); status Accepted → Revised, awaiting review. New tag BIN3 (Ghidra, question (j), recorded in `collision.md` as question 70) and row W22; W19 reworded. §7.4: the merged OPEN on the in-water input is answered (two routines set `Entity_Element`: after every controller update the input includes the float-height test, W22; the position-update paths use `Volume_IsLiquid` alone, W19); the client rule paragraph is rewritten around both; the Tessera rule lists its three differences from the client; the 2026-10-08 answer to the no-support OPEN is corrected (state 4 in water shallower than the float height after controller updates); step 1 of "Entering deep water at speed" cites W22; step 3 states that *v*_z is positive upwards and gives the update for a falling mover's downward speed. |
| 2026-10-09 | Spec writer (dirty room), revision answering the review of 2026-10-09 (no new research; facts from W19, W22 and `collision.md` §11.3.2, §12.3 step 7). **B1:** §7.4 Tessera rule lists four differences from the client after controller updates (the fourth: on support that is not walkable in water deeper than the float height Tessera gives 2, the client 3), as a numbered list; the matching `collision.md` §12.5 row "Water states" is corrected the same way. **B2:** §7.3 answer to the coin-volume OPEN reworded: both routines read `Volume_IsLiquid`, so coin volumes count as water for the movement state; the controller's routine adds the float-height test (W22). Non-blocking: §14 OPEN on containment and buoyancy cites W22. Status stays Revised, awaiting re-review. |
| 2026-10-09 | Spec writer (dirty room), non-shader water and animation rates (owner report of 2026-10-09: waterfalls, "water texture" surfaces and puddles drawn as a white texture; the shader water perhaps moving too fast). Status Accepted → Revised, awaiting review. New tags OBS2, SH2, LIT; rows W23–W34. §2: placed-water row. §8.2: program families of every water-parameter record and their wet-landblock counts; the OPEN on undeclared parameters answered. §8.3: time base (E20, seconds modulo 3,600) and the Direct3D 10/11 agreement; the formula limited to the plain water family. §8.5 rewritten with a normative routing rule and new §8.5.1–§8.5.6 (census; water records on placed meshes and their near-white slot-3000 placeholder; two-layer waterfall records; the mirror record of puddles; liquid-distortion records; plain water on mesh UV and older bump water). New §8.6 (flow-map water: inputs, normal, what the install's neutral flow map implies, colour) and §8.7 (rates of every family, rates of the main landblock-water surfaces compared with the former drawing). §11: WA7–WA11. §12: three worked examples. §13: six correction rows. §14: one OPEN. Checklist rows W10–W17 in `m4-5-visual.md` §4.8. |
| 2026-10-09 | Spec writer (dirty room), second revision of the water-feature revision, after Tessera's clean-room diagnosis of the same day (commit b1da85a: per-surface fallback reasons and a whole-world draw census). No section number or anchor changed; status stays Revised, awaiting review. New tags TP (the clean probe's counts, re-checked with the dirty probes) and BIN4 (Ghidra, question (k)); rows W35–W39; W29 wording (the mirror record also serves static mirrors). §8.3: how the client fills *O* (per-viewer-update landblock difference to a per-region reference, W38) and an OPEN on the reference. §8.5.1: what the walker meets over the whole world (15 water-record, 8 mirror, 11 liquid-distortion and 109 two-layer animated surfaces), why it looks white (generic path, not missing textures: only 23 surface records lack a base texture, the two drawn ones are not water), liquid objects. §8.5.3: rows for 0x31006637, 0x31006638, 0x3100048A, 0x31000489 (the most drawn falls) and the mode-4 formula. §8.5.4: the six static mirror surfaces. New §8.7.1 (every curve-valued water parameter evaluated at the clock each frame; census; Tessera Decision for curve-valued scales) and §8.7.2 (fast plain-water rates are the data's; scale 0). §11: WA12–WA15. §12: two examples. §13: two rows. §14: one OPEN. |
| 2026-10-09 | Spec writer (dirty room), third revision of the water-feature revision, answering the review of 2026-10-09 (changes requested). **B1:** the lava record 0x2B000810 has one status: its own family ("Lava" in the §8.2 program-family table, no longer listed in the flow-map family), recognised from its interface by the new lava row of `material.md` §11.6 (revised the same day), and drawn by the new §8.6.5, whose formula was traced from its Direct3D 10/11 water permutations (SH2 extended; rows W40, W41) with a Tessera Decision (draw steps 1–7, never the generic path) and an OPEN (whether lava moves in the original; Direct3D 9 programs; permutation choice). The §8.2 sampler row for 0x2B000810 corrected (no frame-buffer copy; the normal map is bound but not sampled); the §8.2 answered callout, §8.5 routing (lava paragraph after the rule, numbering kept), §8.6 intro, §8.6.4 OPEN, §8.7 rate table (lava row), W28, WA11 and the §14 OPEN updated; new WA16; three §13 rows. **B2:** the §8.6.3 OPEN now names checklist row W17 of `m4-5-visual.md` §4.8 (the 0x2B0005EC river), not W12 (the fountain basin). Non-blocking: (1) the §8.7.1 census column is "TP draws" and the puddle cell gives 3,409 draws (2,067 + 712 + 630); (2) §12 "Curve-valued scale" says the direction reverses about every π seconds, with intervals of about π ± 0.1 s; (3) checklist rows are cited as "checklist row W… of `m4-5-visual.md`" wherever this spec names them. Checklist row W7 of `m4-5-visual.md` §4.4 gains the expected lava look. No existing section number or anchor changed (§8.6.5 is new); status stays Revised, awaiting re-review. |
| 2026-10-09 | Spec writer (dirty room), fourth revision of the water-feature revision, answering the re-review of 2026-10-09 (changes requested). **B1:** W37 widened to every surface record of the water families, the lava family, the older bump water, the mirror record and the liquid-distortion records (124 records; census re-run over all of them, texture-matrix ids named) and gains 1186, 1187 (mode 4, amplitude 0.001) on the lava surface 0x31006FAE; §8.7.1 rule extended to the lava family and the older bump water, census intro states the scope and that no other curve occurs, new census row for 0x31006FAE; §8.7 time-base sentence and a lava row for the depth offsets in the rate table; §8.6.5 step 6 says 1186 and 1187 are curves evaluated every frame. Non-blocking (1): new §8.6.5 paragraph on the lava surface's texture matrices (scales 6, mode-2 translations), which only the low-detail permutations bind (OBS2). Non-blocking (2) left to the next revision of `render.md` and `walker.md` (the §13 row stands). No section number or anchor changed; status stays Revised, awaiting re-review. |
| 2026-10-10 | Spec writer (dirty room), answers to the implementer OPENs of commits 7e775a3 and afaf0ca and to the non-blocking notes (1) and (2) of the re-review of the fourth revision. Status Accepted → Revised, awaiting review. New tags SH3 (OpenGL text and Direct3D 10/11 programs of the flow-map and liquid records re-read; Direct3D under decision 0011) and OBS3 (puddle placements against the height grid, depth-bias data, permutation interfaces, sampler defaults, curve census, W39 recount); rows W42–W48; W39 corrected (20 fast plain-water surfaces, counting method stated). **§8.6.4:** the 0x2B0005EC colour written out for both back ends (no *L*꜀, tint toward `WaterFogColor`, Fresnel of §8.4 step 5, reflection distorted by 1047; Direct3D depth tint, brightness and soft-shore alpha); the no-reflection permutations of both records have no Fresnel term (W43); the Tessera rule gives each record its own OpenGL list with *R* = *F*ᵣ, which needs no depth texture; the implementer OPEN answered (Fresnel reading confirmed, *L*꜀ and tint corrected). **§8.5.4:** the W13 OPEN answered: puddles are depth-tested by their record, the W13 puddle has no depth bias and lies 80 % below the ground, so the original is Inferred to cut it too; rule: keep the depth test, apply `Render_DepthBias_Type` as the `decals.md` §6.5 default where set. **§8.5.5:** the alpha test reads the output alpha (vertex alpha included in 0x2B00063E, W45); recognition by interface for 0x2B000442, by DID (Tessera Decision) for 0x2B00063E. **§8.5.3:** mode-4 wording states that `w[1]`, `w[3]`, `w[5]`, `w[7]` are 0 in the table. **§8.7, §8.7.1:** rates of 1182, 1183, 1186, 1187 include `w[5]` (0.1–1.2 rad/s; lava 1186 1.2 rad/s). **§14:** implementer OPEN (a)–(f) answered ((b), (e) confirmed; (a) partly by an interface rule; (c), (d), (f) kept as Tessera Decisions). §11: WA17, WA18. §13: three rows; the lava row marked done. No section number or anchor changed. |
| 2026-10-10 | Spec writer (dirty room), second revision of the day, answering the implementer OPEN of 2026-10-10 in §14 (items (a)–(d)). Status Accepted → Revised, awaiting review. New tags OBS4 (depth-bias census over every SED; interface census of the four liquid-distortion records), BIN5 (Ghidra, questions (o)–(q): how `Render_DepthBias_Type` reaches the render component, which draws apply it, which table values reach the rasteriser) and SH4 (all OpenGL and Direct3D 10/11 programs of 0x2B0007D0, question (r)); rows W49–W52; invariant WA19. **§14:** the OPEN is marked answered: (a) the client's order is the entity lookup order of `entity-descriptor.md` §9.2 and gives the same value as Tessera's for every SED (W49, W50); (b) all three entry values reach the rasteriser state (W51); the absolute value's scaling stays with `decals.md` §6.5 OPEN (b); which entity draws apply the entry is a new OPEN, with the Tessera default extended to generic-path draws; (c) two record-wide interface rules for 0x2B000442 (W52); (d) the lit record tests the texel alpha alone and has no vertex colour (W52). **§8.5.4:** one sentence pointing to §14 (a), (b). **§8.5.5:** step 5 states the 0x2B0007D0 alpha test; the recognition answer gains the record-wide rules. `decals.md` §6.5 revised the same day with the same facts. No section number or anchor changed. |
| 2026-10-10 | Spec writer (dirty room), third revision of the day, answering the two OPENs of §14 left by the second revision. Status Accepted → Revised, awaiting review. New tags BIN6 (Ghidra, question (s); recorded in `decals.md` as BIN3) and OBS5 (interface census of all 510 shader records); rows W53, W54. §14: the OPEN on which entity draws apply `Render_DepthBias_Type` answered (every draw of the entity's scene node in every pass, entry 2 in the shadow-map pass for entry 0; details and Tessera's shadow-pass deviation in `decals.md` §6.5); the `tessera-material` implementer OPEN answered (the rules never disagree on this install; on a disagreement, world position only when both hold; a binding rule that matches W46 on every record; the rules hold only inside the liquid-distortion family). §8.5.4: the puddle rule cites the traced draws. §8.5.5: note on the rules' agreement and the decisive property. No section number or anchor changed. |
| 2026-10-10 | Spec writer (dirty room), fourth revision of the day, answering item B1 of the review of the third revision (changes requested). Row W54: the two counts were swapped; under the W52 and §8.5.5 definitions (a rule holds when the record has no binding with a semantic of 38–44, or no type-5 item) 191 records satisfy both rules and 319 neither, which agrees with 0x2B0000B7 satisfying both. Re-checked with the OBS5 probe the same day. Status stays Revised, awaiting review. No section number or anchor changed. |
