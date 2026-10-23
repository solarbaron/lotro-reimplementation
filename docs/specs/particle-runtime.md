<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Particle runtime: starting, attaching, running, stopping and drawing particle effects

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review by spec-review:spec-r8:2 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer, package spec-r8: rule 15 body carries the never-drawn owner rule (review note) and two new Tessera rules (owners whose cell is not seen, owners that can never be drawn), with the implementer callout filed and answered; §11.2.6 step 1 callout on reading `AtmosphericsDetail` filed and answered (pointer to `walker.md` §11.2, W26); no client fact added or changed; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-r7:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer, package spec-world-r7: the round-6 implementer callouts of §11.2.3 step 4, §11.2.5, §11.2.6 steps 2, 3 and 7, rule 15 and rule 17 filed and answered; new BIN-P (listings of passes (n) and (o) re-read) and rows RT65–RT67; §11.2.3 step 4, §11.2.5, §11.2.6 steps 2–3, rules 15 and 17 and Q-RT13 revised; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-r6:2 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer, package spec-world-r6: §11.2.5, §11.2.6 steps 2 and 7, rule 17, Q-RT13 and BIN-O restated as behaviour; rule 15 (static owners' `Render_Opacity` and culled owners Tessera readings, new Q-RT14) with a new implementer callout answered; rule 16 and its callout name the built `Post` stage of `render.md` §5.1; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-r5:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, tenth revision by the spec writer, package spec-world-r5: new BIN pass (o) and OBS6, rows RT58–RT64; new §11.2.6 (the drops of the weather code: count gate, rain simulation, parameters of both sets, materials); §11.2.2–§11.2.4 completed (the gate, the reference surface, the identity frame); rules 15–17 revised (culled owners, the creation viewport, drops and interim snow and bone rules); the world-conform implementer OPENs of §11.2 and §12 filed and answered; Q-RT13 narrowed; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-r3:2 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, ninth revision by the spec writer: new BIN pass (n) and OBS5, rows RT49–RT57; new §11.2 (weather systems and the global wind) and §11.3 (the effect preview of the options window, Q-RT5); Tessera rules 15–17 (parent opacity for `inherit_opacity`, systems of the pass after post-processing, weather systems); the rule 13 note on lamp lights updated; §10.2 and Q-RT7, Q-RT9 notes; new Q-RT12, Q-RT13; then review fixes: the anchor before §11 restored, §11.2.2–§11.2.3 restated as conditions, the world-space count of §11.2.4 step 6 corrected; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-render:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: Revised (2026-10-23, eighth revision by the spec writer: rule 13 corrected from a new BIN pass (m) and RT48; the two implementer OPENs of 2026-10-23 (§9.3 Q-RT3 box, rule 13 readings (a)–(d)) answered as Tessera rules; awaiting independent review). Before: Accepted (2026-10-14; independent review of the seventh revision, commit 48a0d3b, passed; see the review record). Before: Revised (2026-10-14, seventh revision, P2 follow-up by the spec writer: BIN pass (k) paraphrased (non-blocking note (1) of the 2026-10-11 review), new BIN pass (l) and RT45–RT47 answering the three implementer OPENs of §11.1 (ribbon vertices and texture coordinates, the beam systems' placement, emission and distance flag, the precision of the spawn count), §11.1 steps 5, 6 and 8 completed, new RT-I20). Before: Accepted (2026-10-11; independent review of the sixth revision, commit 7b14f27, passed; see the review record). Before: Revised (2026-10-10, sixth revision, P2 spec writer: new §11.1 answers Q-RT4 (beams) from BIN pass (k) and OBS4, new RT36–RT44, §5.1 beam row, new rule 13 (lights of lit systems, a Decision for `particles.md` Q8 (b)) and rule 14 (beam limits), RT-I17–RT-I19, §15 Q-RT4 answered, new Q-RT11; for `combat-visuals.md`; awaiting independent review). Before: Accepted (2026-10-10; review of the fifth 2026-10-10 revision (commit e67cc51) passed CLEAN_ROOM.md §5.3, every item; pointers only, RT35 reproduced; see the review record). Before: Revised (2026-10-10, fifth revision, spec writer: §9.1.3 item 1, §9.1.1 rule 3, §16 and RT30 point to the tested opacity of `material.md` §11.2 as revised 2026-10-10 instead of the former "opacity source"; new RT35 (no particle record tests a sum of two alphas); no rule for particle draws changed; awaiting review). Before: Accepted (2026-10-10; review of the fourth 2026-10-10 revision (commit 0cd700e) passed: §3.4 real-range precision steps and RT34 confirmed by an own reading of the client, RT33 citation confirmed; wording only, no rule changed). Before: Revised (2026-10-10, fourth revision: wording fixes from non-blocking notes (a) and (b) of the review of the third revision; §3.4 states the precision of each step of a real-range draw (new RT34), RT33 cites the BIN reading that the mode-3 mapping is the client's; no rule changed; awaiting review). Before: Accepted (2026-10-10; review of the third 2026-10-10 revision (commit 6aba648) passed: the implementer OPEN of §9.1.1 on how rule 12 is applied answered; (a) corrected, (b) and (c) confirmed; rule 12 completed, new RT32–RT33, RT-I15–RT-I16, Q-RT10 widened). Before: Revised (2026-10-10, third revision), Accepted (2026-10-10; review of the second 2026-10-10 revision (commit ba847ee) passed: the implementer OPEN of §9.1.1 on curve states answered from BIN pass (i) and OBS3 (one state per loaded surface record, not per 0x39 record); new RT31, rule 12, RT-I13–RT-I14, Q-RT10), Revised (2026-10-10, second revision), Accepted (2026-10-10, review of the first 2026-10-10 revision passed), Revised (2026-10-10), Accepted (2026-10-09), Revised (2026-10-09), Accepted (2026-10-08) |
| **Last updated** | 2026-10-23 (spec-r8: rule 15 body and callout, §11.2.6 step 1 callout). Before: 2026-10-23 (spec-world-r7: round-6 implementer callouts of §11.2 and rules 15 and 17 answered; BIN-P, RT65–RT67). Before: 2026-10-23 (spec-world-r6: §11.2.5, §11.2.6 steps 2 and 7, rule 17, Q-RT13 and BIN-O restated as behaviour; rule 15 (static owners' `Render_Opacity` and culled owners Tessera readings, new Q-RT14) with a new implementer callout answered; rule 16 and its callout name the built `Post` stage of `render.md` §5.1). Before: 2026-10-23 (tenth revision: BIN pass (o), §11.2.6, rules 15–17 revised). Before: 2026-10-23 (ninth revision: BIN pass (n), §11.2, §11.3, rules 15–17) |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted, not the author), 2026-10-08; independent reviewer of the 2026-10-09 revision (separate dirty-room session, AI-assisted, not the author), 2026-10-09; independent reviewer of the 2026-10-10 revision (separate dirty-room session, AI-assisted, neither the author nor an earlier reviewer), 2026-10-10; independent reviewer of the second 2026-10-10 revision, commit ba847ee (separate dirty-room session, AI-assisted, neither the author nor an earlier reviewer), 2026-10-10; independent reviewer of the third 2026-10-10 revision, commit 6aba648 (separate dirty-room session, AI-assisted, neither the author nor an earlier reviewer), 2026-10-10; independent reviewer of the fourth 2026-10-10 revision, commit 0cd700e (separate dirty-room session, AI-assisted, neither the author nor an earlier reviewer), 2026-10-10; independent reviewer of the fifth 2026-10-10 revision, commit e67cc51 (separate dirty-room session, AI-assisted, neither the author nor an earlier reviewer), 2026-10-10; independent reviewer of the seventh revision, commit 48a0d3b (separate dirty-room session, AI-assisted, neither the author nor an earlier reviewer), 2026-10-14; spec-review:spec-world-r6:2 (independent session) for the spec-world-r6 revision, 2026-10-23; package spec-r8 revision: spec-review:spec-r8:2 (independent session), which wrote no part of it, 2026-10-23: passed, Accepted |
| **Review record** | **Review 2026-10-23** of the package spec-r8 changes (uncommitted diff) by spec-review:spec-r8:2 (independent session), a session that wrote no part of them: **passed**, CLEAN_ROOM.md §5.3 every item on the changed text; contamination screen clean (no code, pseudo-code, disassembly prose, copied prose, game text, keys or hostnames); relative links and anchors of the seven owned files resolve; every answer is labelled a Tessera rule or Decision with no client fact added. Checked: rule 15 body (never-drawn owners moved from the world-r6 answer; cell not seen; never-drawable owners factor 1; loading owners may stay 0) agrees with its callout answers and with `mesh.md` §8.7; the §11.2.6 step 1 answer points to `walker.md` W26 and matches RT58 (built-in `Medium`, levels other than 0 and 1 give *q* = 1). Earlier: **Review 2026-10-23** of the spec-world-r7 changes (uncommitted diff) by spec-review:spec-world-r7:1 (independent session), a session that wrote no part of them: **passed**, CLEAN_ROOM.md §5.3 every item on the changed text; contamination screen clean (no code, pseudo-code, disassembly prose, copied prose, game text, keys or hostnames); relative links and anchors of the added text resolve. Checked: BIN-P names the version, SHA-256, tool, questions, why M1–M3 were not enough and what stayed unresolved; RT65–RT67 and the revised §11.2.3 step 4, §11.2.5 and §11.2.6 steps 2–3 state behaviour only; RT66 spot-checked against the dirty-room snow listing of pass (n) (a value fixed once per process at its first use); the §5.7 probe description matches §5.7; the `water.md` W18, §7.2–§7.3 coin sets, `mesh.md` §8.7–§8.8 and `scenery.md` §7.6 pointers hold; Tessera rules are labelled as such and the build gaps are marked as not conforming. Non-blocking: rule 15's body still says only "the previous drawn frame"; the never-drawn-owner rule lives in the callout answer. Earlier: **Review 2026-10-23** of the 2026-10-23 spec-world-r6 revision as revised after the not-passed review (uncommitted diff) by spec-review:spec-world-r6:2 (independent session), a session that wrote no part of it and did not do the earlier review: **passed**, Accepted. Checklist (CLEAN_ROOM.md §5.3) on the changed text: every statement is a functional fact or a labelled Tessera rule or reading; no code, pseudo-code or decompiler structure; no registers, offsets, call sequences, internal names or memory layouts; no copied prose, game text, keys, hostnames or traffic; provenance present; unknowns stay OPEN (Q-RT13, new Q-RT14); nothing outside decision 0034 §2. The blocking items of review spec-review:spec-world-r6:1 are resolved: rule 15 and its static-owner callout now label the count of `Render_Opacity` alone and the exclusion of `Translucent`, the `Render_Color` alpha and the unnamed second factor as a Tessera reading, with the client question as Q-RT14, and the callout separates the 1,563 count (an observation) from the value (read at run time). Spot checks by cross-reference: the 1,563 count matches `setup.md` §4.4; the two-factor shadow-casting opacity matches `mesh.md` §8.8; the sky tint's `Render_Opacity` matches `environment.md` §11.4 step 5; the `Post` stage matches `render.md` §5.1. Relative links and anchors of the seven owned files resolve. Non-blocking note: the suggested method of Q-RT14 (what writes the second opacity factor, the parent-opacity read) leans towards how the client represents opacity; a later revision may phrase it as the behaviour to observe (which owner changes alter an attached system's opacity). Earlier: **Review 2026-10-23** of the 2026-10-23 spec-world-r6 revision by spec-review:spec-world-r6:1 (independent session), a session that wrote no part of it: **not passed**; status stays Revised. Contamination screen clean; the behaviour restatements of BIN-O, §11.2.5, §11.2.6 steps 2 and 7, rule 17 and Q-RT13, the culled-owner Tessera reading of rule 15 and the rule 16 `Post` stage pass. Blocking (CLEAN_ROOM.md §5.3, provenance and "unknowns marked OPEN, not guessed"): §12 rule 15 and its new answered callout state as a client fact that `Render_Opacity` alone counts and that `Translucent` and the alpha of `Render_Color` do not, citing `mesh.md` §8.8; `mesh.md` §8.8 names only one of the render component's two opacity factors (`Render_Opacity`) and carries no such exclusion, and `appearance.md` §5.7 step 4 says how `Translucent` becomes a drawn opacity was not traced. The writer must either cite a source that establishes the exclusion or mark it as a Tessera reading (with an OPEN on the unidentified factor), and reword the callout's "observation, read at run time" parenthesis so that it is clear that the 1,563 count is the observation. Earlier: **Review 2026-10-23** of the 2026-10-23 spec-world-r5 revision (uncommitted diff of the eight owned specs against 913e1c9) by spec-review:spec-world-r5:1 (independent session), a session that wrote no part of it: **passed**, Accepted. Checklist: every changed statement is a functional fact or a labelled Tessera rule or reading; no code or pseudo-code, and the ordered steps of `particle-runtime.md` §11.2.6 describe behaviour (their order changes the output) rather than a listing; no copied prose and no identifiers beyond the install's interoperability labels (UNIQUEDB, property-dictionary and `[Render]` option names) and Tessera's own code names in implementer callouts; no game text, keys, hostnames or traffic; every new fact has a provenance row with a mark; unknowns stay OPEN (Q-RT13, Q16, the §10 and mode-10 OPENs of `environment.md`); nothing outside the research scope of decision 0034 §2. Spot checks: the executable's SHA-256 matches the BIN rows; the reviewer's own read-only M4 check (decision 0011, Ghidra 12.1.3 headless on a deleted copy of the shared project, same build) and data-section reads confirm the rain and snow parameter sets of §11.2.6 (every table value), the quality factors 0.3 and 0.6 with the built-in level 1, the count rule and its zero-count gate, the material keys 0x2B and 0x3B, the placement segment (moved by *L* / 2 along *f*, swept along −*f*, start returned without a hit), the 0.1 m reference threshold, the fall decrease, the wrap and the radial draw; the install's UNIQUEDB entries 0x2B and 0x3B (values and labels of OBS6) and the Steam prefix's `UserPreferences.ini` values (OBS-F3) match; RT-I21–RT-I23 and the `appearance.md` §5.7 worked example recompute; relative links and anchors of all eight files resolve. Not re-derived: the OBS14 and OBS7 counts, BIN13, BIN-E4 and BIN-F3 beyond their cross-references. Non-blocking notes for the next revision: (1) `particle-runtime.md` §11.2.6 step 2 ("its stored offsets move by the opposite amount"), the per-bone flag "kept with the animation state" (§11.2.5, rule 17, Q-RT13), the snow drop's "several flakes, each with an offset" and the camera's "first local axis" (step 7) lean towards the client's internal representation; restate them as behaviour (drops keep their world position; the marked bones; the camera's right axis); (2) rule 15's culled-owner case is a Tessera reading and may differ from the client if its "placed in the same frame" excludes culled parents; a later BIN pass could settle it; (3) the provenance wording of `appearance.md` BIN13 ("timed messages ... post") and `far-view.md` BIN-F3 ("the count and name list each enumerated option is registered with", "the routine that builds") is mechanism-level; prefer naming only the behaviour read; (4) `particles.md` P59 "otherwise 1" means *q* = 1 at any other level. Before: **Review 2026-10-23** of the 2026-10-23 spec-world-r3 revision (uncommitted diff against 3f0d57f) by spec-review:spec-world-r3:2 (independent session), a session that wrote no part of it: **passed**, Accepted. Checklist: every changed statement is a functional fact or a labelled Tessera rule; no code or pseudo-code and nothing that follows a decompiler listing; no copied prose and no identifiers beyond the interoperability labels of the install's data (UNIQUEDB, property-dictionary and UI-layout labels) and Tessera's own code names in implementer notes; no game text, keys, hostnames or traffic; new facts have provenance rows with marks; unknowns are OPEN callouts; nothing outside decision 0034 §2. Links and anchors of all eight changed specs resolve. Spot checks (own probes, read-only): (a) all 11,155 0x39 records of `client_general.dat`: `world_space` 7,540, `uses_video_post` 139, `inherit_opacity` 129, none with both (RT56); of the ten UNIQUEDB records eight are world-space and only the two impact systems (keys 0x2A, 0x100) local-space (§11.2.4 step 6); (b) both `UserPreferences.ini` files hold `VideoPostEffects` and `PrecipitationEffects` True (RT55); (c) the property dictionary holds `Volume_IsLava` as PID 0x3F6, `Volume_IsCoin` as 0xB6E and `UI_Shortcut_Data_ID`, and `client_general.dat` holds the `FilterParticles…` layout labels (§11.2.5, §11.3); (d) BIN, own read-only copy of the analysis project (deleted afterwards), executable SHA-256 equal to BIN-N's: the weather start-up creates the ten systems once (§11.2); the placement takes the rain branch only when *r* > *s*, uses the 0.05 gate and 2 × max(0, *r* − 0.5) > 0.001 for rain, max(0, *s* − 1) > 0.001 for snow, and the impact spawn with the 0.1, 0.9, 10 constants and the draw from −8 (§11.2.4 steps 1–5); the snow update keeps its gust ratio from its first use, the rain update recomputes it each time (§11.2.3 step 4); (e) RT29's 97 and rule 11 match rule 16. Non-blocking: (1) §11.2.4 step 5 could state the resulting chance, (*m* + 1) / (*m* + 9) with *m* the upper bound, besides the draw; the present wording is an algorithm step in plain words and acceptable; (2) the passages naming unnamed client state (§11.2.2 step 1, §11.2.3 step 4, §11.2.4 steps 2 and 6) are within the rules and carried into Q-RT13. Before: **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-world-render:1 (independent session), a session that wrote no part of it): **passed** CLEAN_ROOM.md §5.3, every item; contamination screen clean. BIN facts were checked for consistency with the cited sections and install artefacts, not re-traced. Spot checks: RT48's light factor matches `decals.md` §6.3 (without the direction term); §12 rule 13 and the §9.3 answer are consistent with §6.1 and RT48. Non-blocking: BIN-M names the existence of a kept dirty-room listing (no path, no content); records.md BIN2 has the same wording, so it is accepted as provenance. Before: Review 2026-10-09 of the revision (diff against main, CLEAN_ROOM.md §5.3): **passed**, Accepted. All nine checklist items hold for the changed text: functional facts, Tessera rules and Decisions only; no code or pseudo-code; no copied prose; property names only as interoperability identifiers; no game text, assets or keys; every new fact has a provenance row (BIN-S, OBS2; RT21–RT24) with its mark; unknowns are OPEN (Q-RT7, Q-RT8, each with a Decision). Spot-check, read-only on 2026-10-09 with the repository decoders as a tool: 11,155 records with prefix 0x39, 11,153 with a surface, 1,179 distinct surface records, 19 shader records, the surface and system counts of every row of the §9.1.1 table, effective 4004 on 8 / off 1,049 / not declared 122 and 4002 on 2 / off 1,177 all reproduced exactly (RT21). The BIN-S program readings (RT22) and the RT24 census were not re-derived. Non-blocking notes: (1) §9.1.1 rule 1 summarises `material.md` §11.2 per-draw steps 2–3 but not step 1 (alpha-to-coverage keeps the reference) or the 8-bit rounding; §11.2 remains authoritative, a pointer to its steps 1–4 would help; (2) a stray semicolon before the parenthesis in rule 1; (3) the Answered callout after §12 rule 7 interrupts the numbered list. Review 2026-10-10 of the 2026-10-10 revision (commit 6dd06d3: header, §1 BIN (h), BIN-S second pass, OBS3, RT20, RT25–RT30, §2, §9.1.1 notation and callout, new §9.1.2–§9.1.3, §12 rules 8, 10, 11, §13 RT-I10–RT-I12, §15 Q-RT9, §16, §17) against CLEAN_ROOM.md §5.3: **passed**, Accepted. All nine checklist items hold: functional facts, formulas in words and Tessera rules only; no code or pseudo-code (the shader formulas are arithmetic, not instruction listings); no copied prose; the option names and the 1160 label are interoperability identifiers already used by `install-layout.md` and `material.md` §8.0, and the option description is paraphrased; no game text, assets or keys; every new fact has a row with its mark (RT25 BIN, RT26–RT27 BIN-S + OBS, RT28–RT29 OBS, RT30 Decision); the unknowns are OPEN Q-RT9 with a Decision; nothing under a closed gate. Spot-checks, read-only on 2026-10-10: with the repository decoders, 11,155 records, 11,153 with a surface, 1,179 surfaces, 19 shader records; RT29 209 systems on 0x2B000084, 97 with `uses_video_post` 1; RT28 1,057 surfaces with 4004, all with a mode-1 1043, 1,148 with 1174 (43 at 0, 997 at 0.3); RT27 206 two-layer surfaces, 182 not identity (34 first set, 178 second), 171 animated (3, 170); RT26 and RT-I11: in each of the nine named records exactly one Direct3D 10/11 permutation has bit 1 of key word 1 and it alone samples the depth-stencil source, 0x2B000084 has bit-1 permutations for kinds 1, 3, 4 sampling the frame-buffer source and ZERO / ONE variants excluding it, the other nine records never use bit 1. RT25 re-derived in an own headless decompilation: the mask builder sets value 2 only from two frame flags (cleared and restored around a further draw by its caller), the render-target-effects byte and a non-zero `Render.SurfaceReflections` (variable matched to the option by its registration); the byte is set from `Render.VideoPostEffects`, two device flags and a hardware class of at least 2. The soft-particle factor (view depth − 0.1, divide by *k*, saturate, alpha only), the heat-distortion steps and the two-layer texture matrices (two matrices in the two-layer vertex programs, none in the one-layer ones) agree with the disassembled programs. **Non-blocking:** (1) the routine that sets the render-target-effects byte clears it again under one further device condition; it falls under "not traced further" and Q-RT9, but could be named there. (2) §9.1.1 notation leaves a short line ("*V* = the vertex colour, which carries the") after the inserted sentence; rewrap at the next edit. Earlier: review 2026-10-08 **passed**, Accepted; details and non-blocking notes in the review log (decision 0018). Review of the second 2026-10-10 revision (commit ba847ee: header, §1 BIN pass (i), OBS3 widened, RT20 widened, RT31, §9.1.1 implementer OPEN marked Answered, §12 rules 8 and 12, §13 RT-I13 and RT-I14, §15 Q-RT10, §16, change log), 2026-10-10, by a separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Every changed statement is a functional fact, a Tessera rule (rule 12) or an OPEN with its Decision (Q-RT10); no code or pseudo-code, copied prose, game data, keys or hostnames (RT31 names no record); BIN pass (i) names its build, question and why M1–M3 were not enough; RT31 carries its marks (Confirmed BIN + OBS; the shared loaded object per surface DID Inferred; other rebuild triggers OPEN); nothing falls under a closed gate. Spot-checks, read-only on 2026-10-10: an own probe with the repository decoders and the material resolver over all 11,155 0x39 records reproduced RT31 and RT-I14 exactly (11,153 with a surface, 1,179 distinct surfaces, 694 named by more than one record; non-constant curves among 1043 and the listed texture-matrix values: mode 2 in 154 surfaces, 4 in 25, 7 in 8, 3 in 3, 10 in 2, 6 in 1; no non-constant 1043; the two mode-10 surfaces are named by 2 and 1 records). In an own headless decompilation (Ghidra 12.1.3, temporary project copy, deleted afterwards) the shared curve evaluator keeps a mode-10 curve's start time and last value inside the curve object and mode 3 draws from the global generator with no per-curve state, as stated. RT-I13 recomputed from `material.md` §5.3.1 (0.5 at clock 10.5; 0.001 for a fresh state): matches. The per-surface effect build and its rebuild flag were not re-derived. **Non-blocking:** (a) the Inferred part of RT31 (one loaded surface object per DID) could be named in Q-RT10 or its own OPEN, so that an implementer sees it outside the provenance table. Review of the third 2026-10-10 revision (commit 6aba648: header, §1 OBS3 widened, RT20 widened, new RT32 and RT33, §9.1.1 per-draw mode-3 note and implementer OPEN answered, §12 rule 12, §13 RT-I15 and RT-I16, §15 Q-RT10, change log), 2026-10-10, by a separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Every changed statement is a functional fact (RT32), a Tessera rule or Decision (rule 12, RT33) or an OPEN with its Decision (Q-RT10, now also naming the Inferred shared object per DID and the release lifetime, which settles non-blocking note (a) of the previous review); no code or pseudo-code, copied prose, game data, keys or hostnames (RT32 gives counts, periods and key counts only, no DIDs); the third OBS3 probe is in the provenance table; RT32 Confirmed with its date, RT33 Decision; RT-I15 and RT-I16 are concrete (RT-I16 matches §3.4 and `material.md` §5.3.1 mode 3, *B* + *A* · *r*); nothing falls under a closed gate. Spot-checks, read-only on 2026-10-10: an own probe with the repository decoders and the material resolver over all 11,155 0x39 records reproduced RT32 and RT-I15 exactly (five mode-10 curves on two surfaces, all `loop` 1; four with `period` 1.2 s and 6, 6, 3, 3 keys on the surface named by two records, one with `period` 4 s and 3 keys on the surface named by one) and again RT31 (1,179 surfaces, 694 shared; modes 2/3/4/6/7/10 in 154/3/25/1/8/2 surfaces; no non-constant 1043). In an own headless decompilation (Ghidra 12, temporary project copy, deleted afterwards) the shared curve evaluator handles mode 3 by one call of the real-range draw with bounds −1 and 1 per evaluation and keeps nothing, so answer (a) and the mapping of rule 9 match the client. **Non-blocking:** (a) RT33 could cite that BIN reading, so that the mode-3 mapping reads as client behaviour rather than a Tessera choice; (b) outside this diff, §3.4: in the client's real-range draw the difference hi − lo is formed in single precision and the product *u* × (hi − lo) is rounded to single precision before lo is added in single precision; §3.4 says the whole sum is computed in double and then rounded, which can differ in the last bit (immaterial under rule 1, worth a wording fix at the next revision). Review of the fourth 2026-10-10 revision (commit 0cd700e: header, §1 BIN pass (j), RT33 widened, new RT34, §3.4, §13 RT-I16, change log), 2026-10-10, by a separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. The changed text states functional facts (the precision of each step of a real-range draw, RT34; the client's mode-3 mapping, RT33) and keeps the existing Decisions (keying, cap, preview lifetime); §3.4 gives the steps in plain words as arithmetic precision, not as an instruction listing; no copied prose, game data, keys or hostnames; BIN pass (j) names its build, question and why M1–M3 were not enough; RT33 and RT34 carry their marks; RT-I16 stays concrete and agrees with the four steps; nothing falls under a closed gate. Non-blocking notes (a) and (b) of the review of the third revision are resolved. Spot-check, own headless decompilation and disassembly on 2026-10-10 (Ghidra 12.1.3, temporary project copy, deleted afterwards) with the two bound constants read read-only from the client file: the shared curve evaluator's mode-3 branch makes one call of the real-range draw with bounds −1.0 and 1.0 and stores nothing in the curve; the real-range draw compares the two single-precision bounds and returns the first without a draw when they are equal, orders them into lo and hi, takes the double-precision draw, forms hi − lo in single precision, widens it, multiplies in double precision, rounds the product to single precision and adds lo in single precision, exactly as §3.4 and RT34 state. **Non-blocking:** (a) the equality test is unordered-aware, so a NaN bound does not take the no-draw path; bounds come from decoded data and rule 1 makes this immaterial, but §3.4 could say "equal (neither is NaN)" at a later edit. Claude review session (spec reviewer, a fresh session that wrote no part of this spec, its revision or the code, and did no earlier review of it), 2026-10-10: review of the revision of commit e67cc51 on branch specs-21 (header, §1 RT30 and RT35, §9.1.1 rule 3, §9.1.3 item 1, §16, §17 row) against CLEAN_ROOM.md §5.3: **passed**, Accepted (every item). Rules and pointers in words, no code or pseudo-code; no copied prose; sampler role names and DIDs are data names needed for interoperability, the Tessera identifiers are the project's own; no game text, keys or hostnames; the new tags state method, scope, the M4 part under decision 0011 with its question and why M1 was not enough; every new row has sources and a mark (OpenGL Confirmed, Direct3D 10/11 BIN); the clamp, the fade in *A*, the explorer bake, the unseen role combinations and the 16-group bound are labelled Tessera Decisions; anchors unchanged; nothing under a closed gate. Spot checks (read-only, own probes on the repository decoders and an own numeric evaluator of the OpenGL pixel text fed controlled texel, material and vertex alphas): 266 shader records named by surface records (241) or group overrides (136); 158 lit by the `material.md` §11.3 rule, 156 of them named, 136 by surface records, covering 11,639 surface records; their 5,432 OpenGL and 13,536 Direct3D 10/11 programs (OPA, SH-H3). In every one of the 5,432 OpenGL pixel programs the opacity textures reaching the output alpha are exactly the record's opacity-bearing roles and no other texel alpha reaches it, and in every colour pass (0x00–0x05, 0x08, 0x09, 0x3E–0x40) the output alpha equals the tested opacity (product, or sum in the listed DIDs) × one factor: the material alpha (in the base passes through the ambient × material input, E1) or, in vertex-colour records, the interpolated colour alpha (L41, MA61). The role split of the 156 reproduces (77 none, 53, 14 = 11 product + 3 sum, 6, 5 = 3 + 2, 1); 0x2B0007B4 and 0x2B0007CC are unlit, so the three lit `ColorAndOpacity2` sum records of L41 agree with the five of `material.md` §11.2. MA62: role sets 230 / 124 / 44 / 38 / 68 / 5 / 1 over the 510 records; 1,055 2000 entries (791 at 1, 11 at 0, 253 between, none non-finite); no group with slot `a` = 0 among 84,261; at most 5 groups match one surface within a block (one block 5, one 4, two 3). RT35: of the 19 particle records the eight two-layer ones test the product of the 3000 and 3001 alphas, the eight one-layer ones the 3000 alpha, 0x2B0007F2, 0x2B000726 and 0x2B000084 compare nothing; none is a sum record. The `render.md` identifiers and draw-uniform bits 16–26 match `crates/render`, and the M2 mask discards at or below the cutoff. The worked examples hold in single precision (*R* = 204 / 255 = 0.8; 0.4 + 0.4 is discarded, 0.5 + 0.6 kept). The Direct3D 10/11 facts (BIN) were not re-derived. Non-blocking: (N1) `material.md` §18 answer item 2 (a) says the block `alpha_ref` is 0.8 or 0.9 "in every block of this install" (§11.2 says the same); the §7.6 census of the same spec lists 0.1 (50), 0.6 (20) and 0.0 (7): 77 blocks of 12 records (0x2B000000–0x2B000005, 0x2B00000E, 0x2B00000F, 0x2B0003B9, 0x2B000459, 0x2B0005EE, 0x2B0006E5) that no surface record or group names; say "in every block of the records in scope". (N2) MA62 "436 have 2, 13,821 one" does not state its selection; over all 67,166 blocks with groups an own count by the binding rule gives 65,593 with one and 1,569 with two (the maximum and the blocks with 3–5 agree). (N3) `lighting.md` §5.2 item 2 names five adding `ColorAndOpacity2` records, two of which (0x2B0007B4, 0x2B0007CC) are unlit, so three concern the lit equation, as L41 says. (N4) L41 and MA61 list the colour passes; pass 0x42 of the same records outputs the tested opacity without the material factor (× the interpolated alpha in vertex-colour records), which one sentence could say so that "every pass" is not over-read. Independent review 2026-10-11 of the sixth revision (P2; commit 7b14f27 on branch p2: header, §1 BIN pass (k), OBS4, RT36–RT44, §5.1 beam row, §11, new §11.1, §12 rules 13 and 14, §13 RT-I17–RT-I19, §15 Q-RT4 answered, new Q-RT11, change log), by a separate dirty-room session (AI-assisted; neither its author nor an implementer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. §11.1 states the beam rules in plain words, not as a listing; every new row has its mark; rule 13 and Q-RT11 are labelled Decisions; nothing under a closed gate. Spot-checks 2026-10-11: own read-only probe with the p2 decoders reproduced RT44 and the census paragraph exactly (2,740 Beam nodes and every listed count); own headless decompilation (Ghidra 12.1.3, private project copy, deleted afterwards) with the constants read read-only from the client file confirmed step 5 (fractions, clamp, arrival at speed > 0.0002 and fraction > 0.9998), step 4 (0.01 at or below 0.0002, nothing beyond 1,000 m) and the detach test of step 3; RT36–RT38 and RT40–RT43 otherwise not re-derived. RT-I17 (*h* 0.5 at 0.15 s, arrival after 0.29994 s), RT-I18 (3 per update at 1/60 s, 6 after a pause) and RT-I19 (0.4 at the stop, clock preset 0.15, 0.1 s fade) recomputed: match. Non-blocking: (1) BIN pass (k) names the client's beam module by a name taken from a build-path string in the executable, which is not an interoperability identifier; paraphrase it at the next edit. (2) The census note "4 of the hook-carrying nodes have a head speed of 0" (`combat-visuals.md` §4.7): 2 have 0 and 2 have −25. (3) The implementer OPENs of §5.5 and §11.1 record the readings the code follows; answer them at the next revision. Independent review 2026-10-14 of the seventh revision (commit 48a0d3b on branch specfix: header, §1 BIN passes (k) and (l), RT45–RT47, §11.1 steps 5, 6 and 8, the ribbon-vertices paragraph and the three answered OPENs, §13 RT-I18 and RT-I20, change log), by a separate dirty-room session (AI-assisted; neither its author, an implementer nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Pass (k) no longer names a module from a build-path string (note (1) resolved); note (2) is resolved in step 5 and in `combat-visuals.md` §4.7; note (3) is resolved for §11.1. The ribbon paragraph states geometry, vertex content and coordinates in plain words, not as a listing; property names are interoperability identifiers; the Decisions of items (a)–(e) are labelled; RT45–RT47 carry their marks (RT46 names its Probable part); pass (l) names build, hash prefix, tool, questions and why M1–M3 were not enough; nothing under a closed gate. Spot checks, own headless decompilation (decision 0011; Ghidra 12.1.3, private project copy, deleted after use): the draw step places both systems with the beam's frame and clamped opacity before it evaluates the envelope; the ribbon is built only when the body material is loaded, the tail part first from the tail point, then the body, then the head part; the side vector is the unit cross product of the beam direction and the camera-to-tail direction, zero at or below 1e-6; each segment writes six unshared vertices of 32 bytes (position, colour, two coordinate pairs); pair 1 *u* is one running count across the three parts × `TextureUScale`, pair 2 restarts per part with scale 1 (the body's pair-2 scale is a creation-time constant 1.0, not a node field), *v* = 1 on the + side; the three draws run tail, body, head; creation sets a byte flag on both systems, and the system draw routine skips the far and near fade and cut of `particles.md` §8.13 when the byte at that offset is set (object identity Probable, as RT46 says). RT47 was not re-derived. RT-I20 recomputed (2, 7, 9; 2 + 3 + 1 segments) and the RT-I18 note recomputed (60 × the single-precision 0.05 rounds to exactly 3): match. **Non-blocking:** (1) `particles.md` §8.13 says the client applies the distance fade to every 0x39 system and that no code path switches it off; RT46 is such a path. A note for that spec's owner belongs in §16 (or `particles.md` §8.13 should be revised); this spec's own text is unambiguous. (2) The implementer OPEN of §5.5 (part of note (3) of the 2026-10-11 review) was not part of this revision and still stands. (3) The client also skips the ribbon when its bounds fail the view test; this is culling only and needs no rule. |
| **Implements into** | `apps/client` (Particle-node instances, effect attachment to entities, lingering and removal, the per-frame order), `crates/render` (particle draw calls, draw order inside a system), `crates/formats` (no new decoding; the Particle node's extra byte of `script-table.md` §4.1 gets its meaning here), a shared random source (§3.3) |
| **Companion specs** | [`particles.md`](particles.md) (record layouts §4, §6; the simulation of one 0x39 system §8; start gating §7.1.2; node record choice §7.1.1; fork effects §6.5). [`script-table.md`](script-table.md) §4.1 (the extra byte after a Particle node's base block), §5.3 (Particle node fields). [`script-runtime.md`](script-runtime.md) §2.1 (contexts: intensity, flags), §3 (node start, completion, stop). [`setup.md`](setup.md) §5.2 (render scale, `Model_ParticleScale`). [`skeleton-anim.md`](skeleton-anim.md) (holding locations, bone types). [`material.md`](material.md) §5.3.1 (clock, curve evaluation), §4 and §7 (surface records and their blend states). [`water.md`](water.md) (water surfaces). |

---

<a id="s0"></a>
## 0. Purpose

`particles.md` describes the two record families and how **one** running 0x39 system
simulates and draws its particles. This spec covers the run time around that: everything an
implementer needs to make effects appear, follow their owners and disappear as in the
original client. It lets an implementer:

- run the client's clock and random source for particles (§3);
- start an effect from a Particle node: the start gates, the stored "never-ending" byte, the
  attached and the dropped paths, the water probe and the redirect property (§5);
- attach a running effect to an entity each frame: bone, holding location, offset, removal of
  the bone's rotation, and the owner's two scale vectors (§6; this answers `particles.md` Q4);
- stop effects, let them linger without emitting, and remove them (§7);
- know which simulation facts live in `particles.md` (§8) and draw systems with the right
  calls, material and particle order (§9);
- handle fork effects and engine-started systems at the run-time level (§10, §11);
- check an implementation against a complete synthetic emitter over 2 s (§14).

Out of scope: the record layouts (`particles.md` §4, §6); the internal format and simulation
of the third-party fork effect image (`particles.md` Q1); the beam renderer (§11, OPEN); the
weather schedule (`particles.md` Q13).

Names. Node fields are written with the `ParticleScript_` labels of the master property
dictionary 0x34000000 (`script-table.md` §5.3), needed for interoperability. Other names are
this spec's own. No game text appears here; records are referred to by DID.

All counts are observations of one install, read at run time, never hard-coded. Numbers that
define how the client computes (the 2 s and 6 s timers, the 0.0002 tolerance, the probe
heights, the random generator's constants) come from the client executable and may be used as
constants ([decision 0012](../decisions/0012-numeric-constants.md)).

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed (BIN)** = read from the client executable (row BIN); **Confirmed (BIN +
OBS)** = read from the executable and agreeing with the data over all records of the kind;
**Confirmed** = observed in the install on 2026-10-08 (or the date given) over all records of the kind;
**Confirmed (BIN-S)** = read from the compiled programs stored in the shader records (row BIN-S);
**Inferred** = follows from names or consistent data, not traced in code; **Decision** = a
Tessera rule (§12); **Reported** = published sources only (none used).

| Tag | Source |
|---|---|
| BIN | **M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (re-checked 2026-10-08), analysed with Ghidra 12.1.3 in headless mode on a temporary copy of the analysis project (deleted afterwards), 2026-10-08, in the dirty room. **Questions:** (a) what the client does with each Particle-node field at run time, the start gates, the meaning of the extra byte after the node's base block, the dropped-effect path, the water probe and the redirect property; (b) how an attached effect's frame is computed each frame and which owner values feed the two scale vectors of `particles.md` §8.6.3; (c) how an effect is stopped, when it lingers and when it is removed; (d) the random generator particle code draws from and how it is seeded; (e) the draw calls of one system instance, the material they use and the order of particles inside a system; (f) every code path that creates a particle or fork system; (g) how fork draw batches select their shader. **Why M1–M3 were not enough:** all of these are code paths; the data holds parameters only; no published source describes them (M2); no running client and no frame capture are available to this session (M3). Only offline-client code for effects, scripts, scene placement and drawing was read; nothing of the network, login, launcher or anti-tamper code (decision 0004). Facts that rest on BIN alone are marked so. **Pass of 2026-10-10** (same build, Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards): (h) what decides whether a draw uses a shader record's permutations that read the scene depth or the frame-buffer image (the variants of §9.1.2): the routine that builds the frame's permutation feature mask, its inputs and the user options behind them, found from the option description that names heat distortion. Same reasons: a code path; the data holds only the permutations and their keys. **Pass (i), 2026-10-10** (same build, Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards): whose curve objects a particle draw evaluates, and where a mode-10 curve keeps its state: the particle draw routine and the arguments of its draw calls, the routine that builds a surface record's effect, the parameter-copy routine, the curve evaluator, the surface object's constructor and the writers of its rebuild flag, and the fetch that fills the 0x39 record's surface slot. Same reasons: a code path; the data holds only the curves. **Pass (j), 2026-10-10** (same build, Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards): the instructions of the real-range draw routine and the mode-3 call site of the shared curve evaluator. **Question:** the precision of each arithmetic step of a real-range draw, and which bounds mode 3 passes to it. Same reasons: a code path; floating-point precision cannot be observed in the data. **Pass (k), 2026-10-10** (same build, SHA-256 prefix re-checked; Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards; for `combat-visuals.md`): the Beam node's start routine, the client's beam renderer (creation, the per-frame update of the effect list, the ramp envelope shared with other timed effects, the endpoint and attachment routine, the ribbon builder, the head-entity placement) and the particle spawn routine it calls. **Question:** how a beam places and drives its particle systems and head entity, when its node's port fires, and when it ends (Q-RT4). Same reasons: code paths; the data stores only the Beam node's parameters. **Pass (l), 2026-10-14** (same build, SHA-256 of the client file re-checked, prefix d22363eb9aff0fb5; Ghidra 12.1.3 headless on a private copy of the analysis project, deleted afterwards; for the implementer OPENs of §11.1): the beam's draw step and ribbon builder, its segment writer, its creation and parameter copy, its particle spawn routine and its per-frame list update with the removal test, the flag setter the creation calls on its two systems and the system draw routine that reads that flag. **Questions:** (1) the ribbon's vertices, side direction, texture coordinates, part order and the condition for drawing it; (2) how the beam places its particle systems, whether it stops their emission, and what the flag it sets on them does; (3) the precision of the spawn count and of the spawn times; (4) which rate the removal test reads. **Why M1–M3 were not enough:** code paths; the data stores only the node's parameters; arithmetic precision and vertex layout cannot be observed in data, and no frame capture is available to this session. |
| BIN-M | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), pass (m), 2026-10-23.** Same executable (file version 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee), Ghidra 12.1.3 headless on an own read-only copy of the shared analysis project (deleted afterwards), plus the second-pass listing of the particle light sum kept in the dirty-room scratch area; two constants read from the data section. **Question:** what one light of a lit system's list contributes to a particle (attenuation, range cut-off, intensity, direction term), in particular the global directional light (rule 13 reading (a)). **Why M1–M3 were not enough:** the light sum is code; the records hold only `use_lighting`; no frame capture is available. Nothing about network, login, launcher, patcher or anti-tamper code was read. |
| BIN-N | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), pass (n), 2026-10-23.** Same executable (file version 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee, re-checked the same day), Ghidra 12.1.3 headless on an own read-only copy of the shared analysis project (deleted afterwards), plus constants and option initial values read from the executable's data section. **Questions:** (1) when the world view runs its rain and snow updates and what decides whether each of the ten UNIQUEDB weather and water systems is placed and receives particles (`particles.md` Q13); (2) where the target of the global wind of `particles.md` §8.8 comes from; (3) which code path is the "gameplay event handler" of §5.1 (Q-RT5) and which value names its effect; (4) the built-in initial values of the `[Render]` options `PrecipitationEffects` and `VideoPostEffects`. **Why M1–M3 were not enough:** these are code paths and initial values; the data hold only the ten systems and the keyframe curves; the preferences file holds only what the client chose to write; no running client or frame capture is available to this session. **Read:** the world view's frame update and its precipitation step, the rain and snow updates and their sky-exposure test, the placement of the weather and water systems, the figure-in-water effects of the player's figure, the composition of the environment state's precipitation values, the global-wind setter, the weather start-up, the option registration of the two options, and the event handler of the options window page that calls the scene's effect placement. Only offline rendering, effect and options-window code was read; nothing of the network, login, launcher, patcher or anti-tamper code, and not where the page's block lists are kept (Q-RT12). |
| BIN-O | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), pass (o), 2026-10-23** (package spec-world-r5). Same executable (file version 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee, re-checked the same day), Ghidra 12.1.3 headless on an own read-only copy of the shared analysis project (deleted afterwards), plus the two drop sets' parameter blocks, the streak colours, the quality factors and the initial value of `AtmosphericsDetail` read from the executable's data section. **Questions** (the world-conform implementer OPENs of 2026-10-23 and Q-RT13): the gate of a precipitation update (`particles.md` §8.8.1 "at least one drop"); the drop simulation of the rain and snow updates (counts, parameters, placement, fall, respawn, the reference height of §11.2.3 step 4, the streak geometry); the reference frame of the world-placed weather and water systems (§11.2.4 step 6); which bones §11.2.5 uses. **Read:** how a precipitation update gates its drop count and hands it on, the rain update in full, the parts of the snow update that use its parameters, where the two parameter sets, the streak colours and the two materials come from at start-up, the collision query and the water probe the updates use, the placement of the systems and the figure effects of §11.2.5 (wording restated as behaviour, 2026-10-23). **Why M1–M3 were not enough:** the drops are not records; the data hold only the ten systems and two surface records; their behaviour and parameters are code and data-section constants; no running client or frame capture is available. **Not resolved:** the snow flakes' own motion and geometry, which bones take part in §11.2.5, the collision filter (Q-RT13). Nothing about network, login, launcher, patcher or anti-tamper code was read. |
| BIN-P | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), pass (p), 2026-10-23** (package spec-world-r7). No new decompiler run: the listings that passes (n) and (o) made of the same executable (file version 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee; Ghidra 12.1.3 headless on own read-only copies of the shared analysis project, deleted afterwards), kept in the dirty-room scratch area, were read again: the rain update, the gust spawn of the snow update and the figure effects of §11.2.5. **Questions** (the world-r6 implementer callouts of 2026-10-23): whether a drop that is marked for placement but not placed in an update is still drawn; which point the radius test uses after the field origin moves, and which drops the move leaves where they are; when the snow update fixes its gust ratio; what "in water" means for the figure effects and where `Volume_IsLava` and `Volume_IsCoin` are read. **Why M1–M3 were not enough:** these are code paths; the drops are not records, and no running client or frame capture is available to this session. **Not resolved:** the initial field origin and the initial state of the drops, and where in the frame the drop quads are drawn (Q-RT13). Nothing about network, login, launcher, patcher or anti-tamper code was read. |
| OBS | User install (Steam), read in place and read-only on 2026-10-08: all 7,526 script tables (7,524 decodable, as in `script-table.md`) and 28,992 scriptlets of `client_gamelogic.dat`, every Particle node (41,354); all 11,155 records with prefix 0x39 and all 254 with prefix 0x44 of `client_general.dat`; the master property dictionary 0x34000000. |
| BIN-S | **M4 (decision 0011), shader programs.** The compiled programs stored in the 19 shader records that particle surfaces use (same build as BIN; the records are game data), disassembled with a public shader disassembler in the dirty room on 2026-10-09: every Direct3D 10/11 (kind 4) vertex and pixel program of the 19 records, and the OpenGL (kind 3) pixel programs of 0x2B00070F and 0x2B0007F2 as a cross-check (they agree). **Question:** how the alpha test and the blend's source alpha of a particle draw use the texture, the vertex colour and the material colour with the draw's tint (the implementer question of §9.1.1). **Why M1–M3 were not enough:** the data gives the parameters and render states but not what the programs compute; no frame capture is available (M3); no published source describes these programs (M2). **Second pass, 2026-10-10** (same tools): the Direct3D 10/11 vertex and pixel programs of 0x2B000084 (both variants), of the depth-reading variants of the nine records that have one, and the vertex programs of the eight two-layer records and of the one-layer records. **Questions:** what the heat-distortion and soft-particle variants compute (§9.1.2) and which texture coordinates each layer reads (§9.1.1, implementer item (d)). Same reasons. |
| OBS2 | User install (Steam), read in place and read-only on 2026-10-09 with throwaway probes in the dirty-room scratch area (the repository's `tessera-formats` decoders used as a tool for the script tables; the record layouts of `material.md` §4–§7 for surfaces and shader records): the `surface` field of all 11,155 records with prefix 0x39, the 1,179 distinct surface records they name, their materials and shader records (effective 4002, 4004, 1043 and 2000 by `material.md` §9.2); the driven pairs of all 41,354 Particle nodes of all script tables and scriptlets. |
| OBS3 | User install (Steam), read in place and read-only on 2026-10-10 with throwaway probes in the dirty-room scratch area (the repository's `tessera-formats` decoder used as a tool for the 0x39 records; the record layouts of `material.md` §4–§7): `uses_video_post` and `surface` of all 11,155 records with prefix 0x39; the permutation keys and samplers of the 19 shader records of §9.1.1; the effective values (`material.md` §9.2) of 1043, 1160, 1174, 4004 and the texture-matrix parameters 1001–1004, 1017–1020, 1033–1034 and 1070–1073 of every particle surface; later the same day, with the repository's `tessera-material` resolver as a tool, the mode of every curve among 1043 and those texture-matrix values that is not constant, and the number of 0x39 records naming each surface (RT31); in a third probe the same day, with the same tools, the `loop` flag, `period` and key count of every mode-10 curve among those values (RT32). |
| OBS4 | User install (Steam), read in place and read-only on 2026-10-10 with throwaway probes in the dirty-room scratch area (the repository's `tessera-formats` decoder used as a tool): the data of all 2,740 Beam nodes of all script tables and scriptlets, decoded by the field list of `script-table.md` §5.3 (the fields after `BeamScript_TargetPosition` read from the end of each node's data), their ports, port children and targets. |
| OBS5 | User install (Steam) and the owner's two `UserPreferences.ini` files (the Steam prefix and the `lotro-launcher` prefix, `install-layout.md` §8), read in place and read-only on 2026-10-23 with throwaway probes in the dirty-room scratch area (own decoders): all 11,155 0x39 records of `client_general.dat` (system flags) and the ten 0x39 entries of the DID map 0x28000002 (UNIQUEDB); all 7,526 script tables (7,524 decode) and 28,992 scriptlets of `client_gamelogic.dat` (every Particle node, 41,354); the property dictionary labels of the properties named in §11.2–§11.3; the `[Render]` section of both preferences files. |
| OBS6 | User install (Steam), read in place and read-only on 2026-10-23 with an own throwaway reader in the dirty-room scratch area: the DID map 0x28000002 (UNIQUEDB) of `client_general.dat`, keys 0x2B and 0x3B with their values and labels. |
| PS | [`particles.md`](particles.md) (Accepted 2026-10-08): layouts, censuses, the simulation of one 0x39 system (§8), start gating (§7.1.2), fork effects (§6.5). |
| SR | [`script-runtime.md`](script-runtime.md) §2.1, §3 (contexts, node start, completion, stop). |
| SE | [`setup.md`](setup.md) §5.2 (render scale; `Model_ParticleScale`, PID 0x5A0). |
| MA | [`material.md`](material.md) §5.3.1 (global clock; curve evaluation). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| RT1 | Particle and fork systems are created only by: the Particle node (attached path, dropped path, redirect restart), the weather code (10 UNIQUEDB systems), the beam renderer, and one gameplay event handler (§5.1) | BIN (the complete caller lists of the three creation routines) | Confirmed (BIN) |
| RT2 | The global clock is the double-precision client clock of `material.md` §5.3.1; particle code reads no other time source (§3.1) | BIN; MA | Confirmed (BIN) |
| RT3 | Particle randomness comes from the client's single global generator: L'Ecuyer's combined generator with a 32-entry shuffle table (constants in §3.3), seeded once at start-up from the wall-clock time in seconds; range draws as in §3.4 | BIN (generator, seeding routine and its two callers, the range routines) | Confirmed (BIN) |
| RT4 | The extra byte after a Particle node's base block is the node's stored never-ending classification: 0 = runs out by itself, 1 = never-ending; the client uses the stored byte and computes the classification only when writing a node or when the byte is missing (§5.3) | BIN (the node's serialisation routine); OBS (41,354 of 41,354 stored bytes equal the classification of §5.3: 0x39 nodes 26,196 × 0 and 13,634 × 1; 0x44 nodes 934 × 0 and 590 × 1) | Confirmed (BIN + OBS) |
| RT5 | The classification rule of a 0x39 effect takes the per-emitter maximum starting from 0, so only `physics_duration` > 0 or an emitter (active or not) with `emission_limit` = 0 and `time_limit` = 0 makes it never-ending; a 0x44 effect is never-ending when its `flag` = 1 (§5.3) | BIN; OBS (as RT4; record 0x39002566, whose per-emitter values are all negative, is stored as 0) | Confirmed (BIN + OBS) |
| RT6 | Node start: no effect DID fails the node; a finite effect is not started when the context intensity ≤ 0.0002 or context flag 0x40 is set; `ParticleScript_DropEffect` selects the dropped path (§5.4) | BIN | Confirmed (BIN) |
| RT7 | Attached path: target resolution through `ParticleScript_RedirectProperty`, the holding-location/bone pre-check, the effects interface, the start gating of `particles.md` §7.1.2, an asynchronous request; the node keeps an instance only for never-ending effects (§5.5) | BIN | Confirmed (BIN) |
| RT8 | Dropped path: the effect is placed once in the world at the attachment frame, with no start gating, no fade settings and no instance; the offset enters twice (§5.6) | BIN | Confirmed (BIN) |
| RT9 | `ParticleScript_OnWater` acts only on the dropped path: a vertical probe against water surfaces sets the effect's height to the highest water hit, or nothing is placed (§5.7) | BIN; OBS (130 nodes set it, 15 of them with `ParticleScript_DropEffect` = 1) | Confirmed (BIN + OBS) |
| RT10 | Attachment frame: holding location or bone or none, `ParticleScript_BoneOffset` in the bone's space, multiplied by the bone's current model-space matrix, rotation replaced by identity when `ParticleScript_RemoveRotation` = 1, then the owner's frame (§6.1–§6.3) | BIN | Confirmed (BIN) |
| RT11 | The two owner scale vectors of `particles.md` §8.6.3: *A* = the owner's render scale (`setup.md` §5.2), *B* = (*p*, *p*, *p*) with *p* = the owner's `Model_ParticleScale` (1 when absent or not a float) (§6.4) | BIN; SE | Confirmed (BIN) |
| RT12 | Stopping a never-ending effect: the node instance's stop hook stops it, with `ParticleScript_ImmediateDisposal` deciding whether it may linger (§7.1) | BIN | Confirmed (BIN) |
| RT13 | Release rule: a system whose last user lets go lingers (emission stopped, particles live on) when it is loaded, not disposed immediately, not of the shareable kind, and was drawn last at opacity ≥ 0.0002 or is world-space (§7.2) | BIN | Confirmed (BIN) |
| RT14 | Lingering systems are removed once their end test passes or 6 s after they were stopped, checked every frame; attached systems are checked every 2 s (§7.3, §7.4) | BIN (both constants and both loops) | Confirmed (BIN) |
| RT15 | Particles of an emitter are stored in creation order; a dying particle is replaced by the last one; they are drawn in storage order with no depth sort (§9.2) | BIN (spawn, update and vertex-writer routines) | Confirmed (BIN) |
| RT16 | A system instance is drawn with at most two draw calls (indexed quads, then triangles) using the material of its loaded surface record, tinted (1, 1, 1, instance opacity), at the global clock (§9.1) | BIN | Confirmed (BIN) |
| RT17 | Fork draw batches select one of the seven UNIQUEDB fork shaders by a blend-mode index 0–6 (§10.2) | BIN; PS §6.4 (the keys) | Confirmed (BIN) |
| RT18 | Census of Particle-node fields (§5.2) | OBS | Confirmed |
| RT19 | Tessera rules (§12) | This spec | Decision |
| RT20 | Whether an owner that is not drawn places its effects; the exact per-frame order; context flag 0x40; bounds culling; beams; the 0xC000 draw flag; an attached instance's own opacity; driven fields after the start; the frame-buffer image of distortion draws; other triggers of a surface effect rebuild, and how long a loaded surface record outlives its last user (Q-RT1–Q-RT10) | BIN (partial) | OPEN |
| RT21 | Particle surfaces: the 11,153 systems with a surface name 1,179 distinct surface records, which use 19 shader records, all unlit and drawn in one pass; their blend is SRCALPHA / INVSRCALPHA or SRCALPHA / ONE (one record also has a frame-buffer-less variant ZERO / ONE); effective alpha test on in 8 surfaces, off in 1,049, not declared in 122; effective depth write off in 1,177, on in 2 (§9.1.1) | OBS2 | Confirmed (2026-10-09) |
| RT22 | What the 19 records' programs compute: the alpha test reads the texture's opacity alone; the output alpha is the texture opacity × vertex alpha × material alpha in most records, with the exceptions of §9.1.1; colour, doubling and fog per record (§9.1.1) | BIN-S | Confirmed (BIN-S) |
| RT23 | The draw's colour multiplier (1, 1, 1, instance opacity) is the per-object tint that `material.md` §8.3 multiplies into material colour 2000, so the per-draw alpha-test rules of `material.md` §11.2 apply to particle draws with *A* = 2000 alpha × instance opacity (§9.1.1) | BIN (the draw call hands the surface's material, shader, transform and colour to the renderer, RT16); `material.md` §8.3, §11.2 | Inferred |
| RT24 | Driven pairs on Particle nodes: 2 of 41,354 nodes, both driving `ParticleScript_ParticleID` from a property with no stored value in the archives (`QuestState_True_Particle`, `QuestState_False_Particle`); no scriptlet-driven pairs (§5.4) | OBS2; `particles.md` §7.1.2 (the two properties have no stored value) | Confirmed (2026-10-09) |
| RT25 | The frame's permutation feature mask has bit 1 (value 2) set only when the client's render-target effects are active (the `Render.VideoPostEffects` option on and the graphics device qualifying), the `Render.SurfaceReflections` option is not 0, and two internal frame flags are set (cleared while an auxiliary view is drawn); permutations whose key word 1 has bit 1 are drawn only then, those whose word 2 has it only otherwise (§9.1.2) | BIN (2026-10-10: the mask builder, its two callers, the readers of both options, the option registrations); `lighting.md` §3.1 (key words) | Confirmed (BIN); the auxiliary view and the device conditions not traced |
| RT26 | Of the 19 particle shader records, 9 have a Direct3D 10/11 variant that requires bit 1 and reads the scene depth (soft particles) and 0x2B000084 has variants for three back ends that require bit 1 and read the frame-buffer image (heat distortion); every other permutation of the 19 excludes bit 1 or does not mention it; the soft-particle factor and the distortion formula of §9.1.2 | OBS3 (keys and samplers); BIN-S (second pass, the Direct3D 10/11 programs of every such variant) | Confirmed (BIN-S + OBS); the content of the depth image Inferred from `lighting.md` §11 |
| RT27 | The eight two-layer records read the first layer at texture matrix 0 and the second at texture matrix 1 (`material.md` §8.4), both applied to the vertex UV pair; the eleven one-layer records apply no texture matrix; on the 206 two-layer particle surfaces 182 have a matrix that is not the identity and 171 an animated one, nearly all in the second layer (§9.1.1) | BIN-S (second pass: every vertex program of the 19 records); OBS3 | Confirmed (BIN-S + OBS) |
| RT28 | Every particle surface that declares alpha test (4004) has a constant (mode 1) reference 1043 (1,057 surfaces); 1,148 particle surfaces declare 1174, 43 of them with value 0 (§9.1.1, §9.1.2) | OBS3 | Confirmed (2026-10-10) |
| RT29 | 209 systems draw 0x2B000084: 112 with `uses_video_post` = 0, 97 with 1 (§9.1.2) | OBS3 | Confirmed (2026-10-10) |
| RT30 | Particle draws take their alpha test and the source alpha of their blend from the programs of §9.1.1, not from the mesh rule for unlit records of `render.md` §4.3 (`Fixed`: the tested opacity of `material.md` §11.2 alone; wording revised 2026-10-10, was "the opacity-source rule"); in a premultiplied pass they follow `render.md` §6.10 "Fixed blends" with that source alpha (§9.1.3) | BIN-S (the programs, RT22); `render.md` §4.3, §6.10 | Decision, built on Confirmed (BIN-S) program facts |
| RT31 | Curve state of particle draws: a mode-10 curve keeps its start time and last value in the curve object; the curves a particle draw evaluates are copies held by the effect built for the loaded surface record (built at its first draw after loading, and on a marked rebuild), so there is one state per loaded surface record; mode 3 has no per-curve state. Census: non-constant curves among 1043 and the texture-matrix values of the 1,179 particle surfaces use modes 2 (154 surfaces), 4 (25), 7 (8), 3 (3), 10 (2; 3 systems, one surface named by two 0x39 records) and 6 (1); none is a 1043; 694 particle surfaces are named by more than one 0x39 record (§9.1.1) | BIN (pass (i)); OBS3 | Confirmed (BIN + OBS); that one surface DID gives one shared loaded object is Inferred (the object manager was not traced); other rebuild triggers OPEN (Q-RT10) |
| RT32 | Every mode-10 curve among 1043 and the texture-matrix values of the particle surfaces loops (`loop` 1): five curves on the two surfaces of RT31, four with `period` 1.2 s (3 or 6 keys) on the surface named by two 0x39 records and one with `period` 4 s (3 keys) on the other. So no particle surface holds a mode-10 value, and a state started afresh changes only the phase of the loop (§9.1.1, rule 12) | OBS3 (third probe) | Confirmed (2026-10-10) |
| RT33 | How rule 12 is applied: mode 3 takes its *r* directly from the shared stream (one real-range draw over [−1, 1] per mode-3 curve and evaluation, a new value at every draw), with no per-curve state and no generator seeded from the stream; states keyed by surface DID may be kept for the session; at most 4,096 surfaces keep states; a preview keeps them for its run (§9.1.1 answer of 2026-10-10, third revision; rule 12). The mode-3 mapping is the client's behaviour, not a Tessera choice: the client's shared curve evaluator handles a mode-3 curve by one call of the real-range draw of §3.4 with bounds −1 and 1 at each evaluation and keeps nothing in the curve (fourth revision) | RT31, RT32; §3.3, §3.4; `material.md` §5.3.1, §8.4; BIN pass (i) and pass (j) (the mode-3 call site and its two bounds); review of the third 2026-10-10 revision (same reading, own decompilation) | Mode-3 mapping Confirmed (BIN); keying, cap and preview lifetime Decision |
| RT34 | Precision of a real-range draw: the bounds and the result are single-precision values; the width hi − lo is formed in single precision, widened to double and multiplied by the draw *u* in double precision, the product is rounded to single precision, and lo is then added in single precision; the equality test that skips the draw compares the two single-precision bounds (§3.4) | BIN pass (j); review of the third 2026-10-10 revision (same reading, own decompilation) | Confirmed (BIN) |
| RT35 | None of the 19 particle shader records is one of the seven records whose tested opacity is the sum of two alphas (`material.md` §11.2 table, MA60); in the OpenGL programs of the eight two-layer particle records (0x2B000703, 0x2B00071B, 0x2B000742, 0x2B000776, 0x2B00077A, 0x2B0007AD, 0x2B000717, 0x2B00072D) the alpha test compares the product of the slot-3000 and slot-3001 alphas, in the one-layer records the slot-3000 alpha, and 0x2B0007F2, 0x2B000726 and 0x2B000084 have no comparison (§9.1.3) | `material.md` SH-H2 (a), SH-H3 (every OpenGL pixel program of these records, re-read 2026-10-10) | Confirmed 2026-10-10 |
| RT36 | A Beam node start resolves its target to the first entity its target returns; with no entity and `BeamScript_UseTargetPosition` = 0 no beam is made (the node instance exists without one). Otherwise the node's fields become a beam of the beam renderer, its two particle systems (when named) are created at once from their DIDs, and its head entity (when named) is created as a world object (§11.1) | BIN (pass (k)) | Confirmed (BIN) |
| RT37 | The beam's ramp envelope: states ramp-in, sustain, ramp-out, done, from `RampInTime`, `SustainTime`, `RampOutTime` and `Persistent`; a stop during ramp-in ramps out from the current level (§11.1 step 2) | BIN (pass (k)) | Confirmed (BIN) |
| RT38 | Endpoints: source and target attachment points (holding location or bone type, offset, bounding-box centre), fixed random offsets inside `RandomStartRadius` and `RandomImpactRadius`, detachment by `SourceDetachTime`, `TargetDetachTime` and `DetachBeforeRampingOut`; the target point of `BeamScript_TargetPosition` when `UseTargetPosition` = 1 (§11.1 step 3) | BIN (pass (k)) | Confirmed (BIN) |
| RT39 | Head and tail fractions move at `HeadAnimSpeed` and `TailAnimSpeed` metres per second from `HeadStartPos` and `TailStartPos` after `HeadAnimStartTime` and `TailAnimStartTime`, clamped to [0, 1]; the head has arrived when its speed is above 0.0002 and its fraction above 0.9998; on arrival the node fires its Default ports once (§11.1 steps 4–5, 7) | BIN (pass (k)) | Confirmed (BIN) |
| RT40 | The ribbon from the tail point to the head point: tail segments, body segments of `SegmentSize`, head segments, each with its material; not drawn below 0.1 m (§11.1 step 6) | BIN (pass (k)) | Confirmed (BIN) |
| RT41 | Beam particles: at random points along the drawn segment, floor(length / `SegmentSize` × `BeamSegmentParticlesPerSecond` × elapsed) per update, spread evenly in time; head particles: one at the head point per update in which `HeadParticlesPerSecond` × elapsed reaches 1, while the head is short of the target; elapsed above 1 s counts as 0.1 s (§11.1 step 8) | BIN (pass (k)) | Confirmed (BIN) |
| RT42 | The head entity is shown at the head point, turned along the beam, while the drawn segment is longer than 0.1 m and the envelope is above 0.0002 and not stopped; otherwise hidden (§11.1 step 9) | BIN (pass (k)) | Confirmed (BIN) |
| RT43 | A beam is removed when its envelope has finished and each of its particle systems is absent, has a zero rate or holds no live particle (§11.1 step 10) | BIN (pass (k)) | Confirmed (BIN) |
| RT44 | Census of the Beam node fields (§11.1, table) | OBS4 | Confirmed (2026-10-10) |
| RT45 | Ribbon vertices: one side vector per beam and frame (the unit cross product of the beam direction and the camera-to-tail direction, zero when shorter than 1e-6), half `BeamWidth` on each side, two triangles of six vertices per segment, vertex format position, colour and two texture-coordinate pairs; pair 1 *u* = segment count from the tail end along the whole ribbon × `TextureUScale` (the tail and head parts count 1 each in total), pair 2 *u* restarting per part without scale, *v* = 1 on the + side; three draws in the order tail, body, head; no ribbon without a `BodyMaterial` (§11.1 step 6) | BIN (pass (l)); OBS4 (all 2,740 nodes name all three materials, re-counted 2026-10-14) | Confirmed (BIN) |
| RT46 | The beam's draw step, once its materials have loaded, places both systems every frame with the beam's frame and opacity, whatever the envelope; the beam never stops or restarts their emission; at creation it sets on each system the flag that makes the system draw skip the camera-distance fade and cut of `particles.md` §8.13; the removal test's "rate" is the node's `BeamSegmentParticlesPerSecond` or `HeadParticlesPerSecond` (§11.1 steps 8, 10) | BIN (pass (l)) | Confirmed (BIN); that the flagged object is the one the draw routine receives is Probable (matching field offsets) |
| RT47 | Precision of the beam spawn count: clock times double, Δ rounded to single precision, ℓ / `SegmentSize` × *b* × Δ and its floor in single precision, no tolerance; each spawn adds the single-precision Δ / *n* to the double-precision spawn time (§11.1 step 8) | BIN (pass (l)) | Confirmed (BIN) |
| RT48 | One light of a lit system's list adds, per colour channel, its colour × clamp(DiffuseIntensity(*t*) × *σ*, 0, 1) × *a*, with *a* = 1 / (1 + 10 × Falloff × *d*² / Range²) for a point light, *d* the distance from the particle to the light, and *a* = 1 for any other light (the global directional light included); there is no cut-off at the range and no direction term; at most the first 5 entries of the list are read, the global light counting as one of them; the alpha factor is the ambient alpha alone (§12 rule 13). The same light factor as the decal path of `decals.md` §6.3 | BIN-M; constants 1.0 and 10.0 read from the executable | BIN; that the colour is the light's Diffuse is Inferred |
| RT49 | Target of the global wind: the environment state in use (`environment.md` §11.6–§11.7, its cross-fade included) carries the wind vector (value of keyframe curve 3, value of curve 4, 0); every update of the world view sets the global wind's target to it; the forced weather of `environment.md` §11.7 step 7 sets it to (0, 0, 0); at weather start-up the target is (0, 0, 0) (§11.2.1) | BIN-N (2) | BIN; the axes of the two components Inferred |
| RT50 | Precipitation step: runs in every update of the world view while the landscape is visible from the viewer (`environment.md` §11.6) and a client state flag of the world view holds (not traced); order: rain update, snow update, placement of the precipitation systems; then, while the world view is active, the figure-in-water effects and the placement of the three water systems (§11.2.2) | BIN-N (1) | BIN |
| RT51 | Rain and snow updates: fall direction, sky-exposure test of the camera and of the player's position every 0.5 s over 100 m, force recomputation of the gust system, spawns from the drops' ends into the vapour, ripple and gust systems (rain) or the gust system (snow), with the thresholds of §11.2.3 | BIN-N (1) | BIN |
| RT52 | Placement: the rain branch (rain value above the snow value) or the snow branch; gust and vapour above rain 0.05, rain camera systems above rain 0.5005, snow gust and camera systems above snow 1.001, camera systems only while the camera is exposed and not inside; the impact system's spawn chance; eight of the ten records world-space, the two impact systems local-space (§11.2.4) | BIN-N (1); `world_space` flags OBS5 | BIN; flags Confirmed (OBS5, 2026-10-23) |
| RT53 | Figure-in-water and rain-on-figure effects of the player's figure: per-bone chances and positions for vapour (rain), bubbles, ripples and coin ripples, with `Volume_IsLava` and `Volume_IsCoin` (§11.2.5) | BIN-N (1); labels OBS5 | BIN |
| RT54 | The §5.1 "gameplay event handler" is the event handler of the effect-filter page of the options window: activating an entry that carries an effect DID in `UI_Shortcut_Data_ID` places that effect once at the local player's position (§11.3) | BIN-N (3); labels OBS5 | BIN; the page's identity from the layout's element labels (OBS5) |
| RT55 | Built-in initial values: `PrecipitationEffects` on, `VideoPostEffects` on (data-section values; whether a hardware test changes them at first start was not traced); both preferences files hold `VideoPostEffects=True` and `PrecipitationEffects=True` | BIN-N (4); OBS5 | BIN; file values Confirmed 2026-10-23 |
| RT56 | 139 0x39 records set `uses_video_post` and 129 set `inherit_opacity`; no record sets both; 887 Particle nodes name a `uses_video_post` record (all 139 reached) and 281 an `inherit_opacity` record (all 129 reached; 272 attached, 9 dropped) | OBS5 | Confirmed 2026-10-23 |
| RT57 | Fork effects: 1,524 of the 41,354 Particle nodes (3.7 %) start a 0x44 effect, 219 distinct effects (`particles.md` §2.2: 1,517 by `ForkParticleID`, 7 by `ParticleID`) | OBS5 (recount); `particles.md` §2.2 | Confirmed 2026-10-23 |
| RT58 | A precipitation update uses *n* = ⌊3,000 × clamp(value, 0, 1) × *q*⌋ drops, *q* = 0.3, 0.6 or 1 for `AtmosphericsDetail` level 0 `Low`, 1 `Medium`, any other (`High` is 2; built-in `Medium`); with *n* = 0 it does nothing else (no exposure test, no force recomputation, no spawn) and reports no exposure (§11.2.6 step 1) | BIN-O | BIN |
| RT59 | The parameter sets of the rain and snow drops (speed 25 and 1.75 m/s, height 30 m, segment 100 m, radius 30 m, recentre 2.5 m, life 10–30 s, jitters 0.4 and 0.5 / 2 m, streak length 1.7 / 0.08 m, half width 0.04 m, the streak colours) (§11.2.6 table) | BIN-O (data-section constants) | BIN |
| RT60 | Rain drops: field origin moved after 2.5 m, drops beyond 30 m dropped; placement by a radial draw and a collision segment through the camera height along −*f*; the end moved onto the reference height within 0.1 m above it or below it; fall fraction, wrap with a jittered direction and end point, expiry; the reference height from the water probe of §5.7 at the camera (§11.2.6 steps 2–5) | BIN-O | BIN; the reported contact Inferred |
| RT61 | Rain streaks: one quad per drop from head = end − *d* × *h* to tail = head + *d* × min(*h*, 1.7) at ± 0.04 m along a camera axis, texture coordinates (0,0), (0,1), (1,1), (1,0), head and tail colours × the state's `colour_4` (§11.2.6 step 7) | BIN-O; `environment.md` E43 | BIN; the axis (right) Inferred; `colour_4` Probable |
| RT62 | The gust, vapour and water systems are placed at a reference point of the world view with the identity rotation; the spawn positions of §11.2.3, §11.2.5 and §11.2.6 are handed over relative to that point (§11.2.4 step 6) | BIN-O | BIN; what the point is not traced |
| RT63 | The drop materials are UNIQUEDB key 0x2B (`RainStripMaterial`, 0x31000007 here) for rain and key 0x3B (`Render_SnowMaterial`, 0x31000001 here) for snow | BIN-O (keys); OBS6 (values, labels) | BIN; values Confirmed 2026-10-23, read at run time |
| RT64 | The state blend of both fades of `environment.md` §11.6 blends the rain and snow values and the wind vector (*c*₃, *c*₄, 0) like every other number (§11.2.1) | `environment.md` BIN-E4, E43 | BIN |
| RT65 | A drop marked for placement keeps its previous state until a placement succeeds: in every update it falls, wraps, spawns by §11.2.3 step 4 and is drawn like any drop in use (every drop in use gives a quad in every update). When the field origin moves, only the drops in use that are not marked keep their world position; a drop already marked, and every drop beyond the first *n*, keeps its position relative to the origin. The radius test after the move is made at the drop's placement point *P*, not at its end point or lower end point (§11.2.6 steps 2–3) | BIN-P | BIN |
| RT66 | The snow update fixes its gust ratio *n* / 3,000 the first time in the client's process that a snow drop is tested for a gust spawn, and keeps it for the rest of the process (§11.2.3 step 4) | BIN-P | BIN |
| RT67 | For §11.2.5 the figure is in water when the water probe of §5.7, cast from the figure's position, finds a water surface, whose height is that hit's; `Volume_IsLava` and `Volume_IsCoin` are read from the figure's own properties, set only when Boolean and true (`water.md` W18) | BIN-P | BIN; that a volume's monitor gives the figure these values is Inferred |

---

<a id="s2"></a>
## 2. Where each run-time topic is specified

| Topic | Where |
|---|---|
| Clock, frame order, random source | §3 here |
| Emitter start (system start, `fast_forward`, restart) | `particles.md` §8.17 |
| Emitter activity window and end-of-effect test | `particles.md` §8.2 |
| Spawn rate and bursts, caps, catch-up | `particles.md` §8.3; distance emission §8.4 |
| Initial position and velocity distributions (shapes, direction, spread, speed) | `particles.md` §8.5 (random source: §3.3 here) |
| Forces: gravity, wind, mass; drag (the 0.95 damping of controller 4 only) | `particles.md` §8.7, §8.8 |
| Lifetime | `particles.md` §8.6 |
| Size, colour and alpha over life (keyframe channels), emitter fade envelope | `particles.md` §8.6.1, §8.6.2, §8.9 |
| Rotation | `particles.md` §8.12 (modes 3) and §8.7 (growth with age) |
| Texture-sheet animation | `particles.md` §8.10 |
| Billboarding and orientation modes | `particles.md` §8.12 |
| Blend modes | §9.1–§9.1.3 here (material of the surface record, `material.md`; the programs, permutation variants and opacity of particle draws); fork effects §10.2 |
| Sorting between systems and passes | `particles.md` §8.15; inside a system §9.2 here |
| Forks (child effects) | `particles.md` §6.3–§6.5; run-time level §10 here |
| Culling, LOD, caps | `particles.md` §7.1.2 (options), §8.11 (32,768 vertices), §8.13 (distance fade, not-drawn state); §9.3 here |
| How scripts start and attach effects | §5, §6 here |
| Stop, linger, removal | §7 here |
| Sharing of running systems | `particles.md` §8.16; release §7.2 here |

---

<a id="s3"></a>
## 3. Clock, frame order and random source

<a id="s3-1"></a>
### 3.1 Clock (BIN)

All particle times are values of the client's **global clock**: seconds since the client's
timer started, held in double precision (`material.md` §5.3.1). "Now" below is that clock.
The particle code reads no other time source; frame length enters only as the difference
between two readings. `particles.md` §8.1 adds one rule: a system's update uses the later of
its creation time and the clock.

<a id="s3-2"></a>
### 3.2 Per-frame work (BIN; order partly OPEN)

Each frame the client does the following particle work:

1. **Lingering sweep** (§7.3): once per frame, from the world renderer's update, every
   lingering system is tested for removal.
2. **Placement.** Each entity's effects component places each of its attached systems for the
   frame with the attachment frame of §6 (and an attached system's frame is recomputed whenever
   the owner's frame or pose changes). Lingering and dropped systems are placed by the scene
   they were registered with. The first placement of a system in a frame runs its update
   (`particles.md` §8.1, §8.16).
3. **Housekeeping** (§7.4): each effects component, at most every 2 s, removes its attached
   systems that have finished.
4. **Draw** (`particles.md` §8.15 for the order of systems; §9 here for one system).

> **OPEN (Q-RT1):** The order of steps 1–3 within a frame, and whether an entity that is
> culled (not drawn) still places its attached systems, were not traced. **Decision
> (Tessera):** run 1, 2, 3, 4 in that order each frame; an entity places its effects only when
> it is drawn, so the effects of an undrawn entity freeze like the not-drawn state of
> `particles.md` §8.13.

**Walker application of the Q-RT1 Decision (Tessera rule, 2026-10-09; answers the implementer
OPEN of that date).** The offline walker (`apps/client/src/walker/effects`) has no scene culling
of its own for effect owners, so "drawn" is defined by spawning:

- It scans the landblocks within 2 of the viewer for static entities and placed figures whose
  start-up scripts reach Particle nodes (content layers as the world's; interior entities only
  with `--interiors`), and spawns such an owner in its own script runtime when the viewer is
  within the owner's *reach* + 20 m, and despawns it beyond *reach* + 60 m or when its landblock
  leaves; the check runs every 0.25 s and spawns at most 16 owners per check.
- *Reach* = the largest `far_fade_end` of the 0x39 records its nodes name (fork nodes: their
  `FarFadeMaxDistance`) plus the node's `ParticleScript_BoneOffset` length, at most 500 m,
  60 m when none is known.
- A static owner counts as drawn while it is spawned (no frustum or per-object draw-distance
  test); a placed figure while the character layer draws it, and it is spawned only once its
  figure is built (its attachment points and pose come from there).
- Despawning stops the owner's never-ending nodes (§7.1) and releases its other effects (§7.2).

These are engineering bounds, not facts about the original; the per-system distance fade of
`particles.md` §8.13 still decides what is drawn inside the reach.

> **Answered (2026-10-09; was OPEN, implementer, 2026-10-09):** the walker's choices are
> confirmed and written above as the walker application of the Q-RT1 Decision.

<a id="s3-3"></a>
### 3.3 The random generator (BIN)

Every random number the particle code uses (shape points, spread angles, random start frames,
the point indices of controllers 1 and 3, the edge choice of the `Quad` shape) and every
evaluation of a mode-3 curve (`material.md` §5.3.1) is drawn from **one global generator**
that the whole client shares. It is the combined multiplicative congruential generator of
L'Ecuyer with a Bays–Durham shuffle, the algorithm published in *Numerical Recipes* as
"ran2" (general knowledge), with these constants (BIN):

| Constant | Value |
|---|---|
| Modulus 1, multiplier 1 | 2,147,483,563 and 40,014 (with Schrage's decomposition 53,668 / 12,211) |
| Modulus 2, multiplier 2 | 2,147,483,399 and 40,692 (52,774 / 3,791) |
| Shuffle table | 32 entries; index = previous output ÷ 67,108,862 (integer division) |
| Output scale | output × (1 / 2,147,483,563), capped at 1 − 1.2 × 10⁻⁷ |

The generator keeps two 32-bit states *s*₁ and *s*₂, the table and its previous integer
output *y*.

- **One draw.** Each state is multiplied by its multiplier and reduced modulo its modulus
  (computed without overflow; a negative intermediate result is made positive by adding the
  modulus). The table index *j* is *y* integer-divided by 67,108,862. The new *y* is the table
  entry at *j* minus the new *s*₂, and the table entry at *j* is replaced by the new *s*₁; when
  the new *y* is below 1, 2,147,483,562 is added to it. The draw's value is *y* divided by
  2,147,483,563 in double precision, capped as in the table, so it lies in (0, 1).
- **Seeding with *n*.** Both states are set to *n* (to 1 when *n* is 0). Then *s*₁ is advanced
  forty times by the first rule; counting the steps *k* = 39 down to 0, the value after step
  *k* is stored in table entry *k* whenever *k* < 32. Finally *y* is set to table entry 0.
- **When.** The client seeds the generator once at start-up with the current wall-clock time
  in whole seconds. One other routine reseeds it with a caller-supplied value (its caller was
  not traced). Every system of the client draws from the same stream, so the sequence a given
  effect sees depends on everything else the client did before.

<a id="s3-4"></a>
### 3.4 Range draws (BIN)

- **Real range [*a*, *b*]** (RT34): the bounds *a*, *b* and the result are single-precision
  values. If *a* = *b*, the result is *a* and **no** number is drawn. Otherwise, with
  lo = min(*a*, *b*) and hi = max(*a*, *b*), take one draw *u* (a double-precision value,
  §3.3) and:
  1. form the width hi − lo in single precision;
  2. widen the width to double precision and multiply it by *u* in double precision;
  3. round that product to single precision (round to nearest; the client was not seen to
     change the rounding mode);
  4. add lo to the rounded product in single precision; this sum is the result.

  The sum is **not** formed in double precision: computing lo + *u* × (hi − lo) wholly in
  double and rounding once can differ from the client in the last bit. Nothing in §12 needs
  bit-exact values from the client (rule 1), but Tessera's own fixed-seed tests (for example
  RT-I16) use these four steps.
- **Integer range [*a*, *b*]**: if *a* = *b*, *a* without a draw; otherwise lo + the integer
  part of *u* × (hi − lo + 1).

Every "random in [*x*, *y*]" of `particles.md` §8.5–§8.10 is one real-range draw; every "index
drawn uniformly" is one integer-range draw.

---

<a id="s4"></a>
## 4. Run-time objects (BIN)

| Object | Created by | Holds | Ends |
|---|---|---|---|
| **System** | a request that finds no shareable running copy (`particles.md` §8.16) or the weather and beam code | the loaded record, emitters, particles, the draw instances of the current frame (`particles.md` §8.16), a *stopped* flag with its time, an *immediate-disposal* flag, a reference count | when its last reference is released (§7.2) |
| **Attachment** | the owner's effects component when a requested system has loaded (§5.5) | the request handle, the bone index (or none), the attachment matrix (§6.1), the `RemoveRotation` byte, the system | when stopped (§7.1) or finished (§7.4) |
| **Pending request** | the attached path (§5.5) | a handle (an increasing counter), the record DID, the requester | when the record has loaded (served) or the request is stopped (§7.5) |
| **Lingering entry** | the release rule (§7.2) or the dropped path (§5.6) | the system, its fixed placement (frame, the two scale vectors, opacity) | §7.3 |
| **Node instance** | a Particle node whose effect is never-ending (§5.3) | the effect's handle, its owner's ID, the `ImmediateDisposal` byte, a listener for the redirect property | when the node is stopped (`script-runtime.md` §3.5) |
| **Shareable list** | every system of the shareable kind while it runs | running systems that may serve later requests (`particles.md` §8.16) | entries leave when the system is released |

Each entity with effects has one **effects component** holding its pending handles, its
attachments and a 2-s housekeeping timer (§7.4).

---

<a id="s5"></a>
## 5. Starting effects

<a id="s5-1"></a>
### 5.1 Entry points (BIN)

| Entry point | Path | Attached to an owner | Gated by `particles.md` §7.1.2 |
|---|---|---|---|
| Particle node, `ParticleScript_DropEffect` = 0 | §5.5 | yes | yes |
| Particle node, `ParticleScript_DropEffect` = 1 | §5.6 | no (placed once in the world) | **no** |
| Change of the redirect property of a running never-ending node | §5.8 | yes | yes |
| Weather code (the ten UNIQUEDB systems, `particles.md` §7.1.3, §8.8.1) | created synchronously at weather start-up | no | no |
| Beam renderer (Beam node's `BeamScript_BeamParticleSystem` and `BeamScript_HeadParticleSystem`) | created synchronously when the beam is made; its particles are spawned by the beam (§11.1) | placed in the beam's frame each frame | no (§11.1 step 1) |
| One event handler, of the effect-filter page of the options window (§11.3; called the "gameplay event handler" before 2026-10-23) | places the effect DID that an activated list entry carries in `UI_Shortcut_Data_ID` once at the local player's position | no | no |

No other code creates a particle or fork system: setups, animations and items start effects
only through scripts.

<a id="s5-2"></a>
### 5.2 The Particle node's fields at run time

| Field (`ParticleScript_…`) | Use | Default of a new node (BIN) | Census over 41,354 nodes (OBS) |
|---|---|---|---|
| `ParticleID`, `ForkParticleID` | the effect DID (`particles.md` §7.1.1) | none | `particles.md` §2.2 |
| `ShouldFade`, `FarFadeMinDistance`, `FarFadeMaxDistance`, `CloseFadeMinDistance`, `CloseFadeMaxDistance` | distance fade of **fork** effects only, attached path only (`particles.md` §6.5) | 1, 40, 60, 0, 0 | `ShouldFade` 0 in 57 nodes (all 0x39, so without effect); fade pairs other than (40, 60, 0, 0) in 78 nodes, all 0x39 (without effect) |
| `HoldingLocation`, `BoneType`, `BoneOffset` | attachment point (§6.1) | 0, 0, (0, 0, 0) | holding location only 9,650; bone only 6,756; both 88; offset ≠ 0 in 15,879 |
| `DropEffect` | selects the dropped path (§5.6) | 0 | 1 in 2,856 nodes (2,644 for 0x39, 212 for 0x44), every one of them finite (§5.3) |
| `RemoveRotation` | drop the bone's rotation (§6.2) | 0 | 1 in 7,334 nodes (3,200 of them with a holding location or bone) |
| `OnWater` | water probe on the dropped path (§5.7) | 0 | 1 in 130 nodes (15 dropped, 115 attached and so without effect) |
| `ImmediateDisposal` | no lingering after a stop (§7.1) | 0 | 1 in 3,584 nodes (3,020 never-ending; 564 finite and so without effect) |
| `RedirectProperty` | target redirection (§5.8) | a value the client reads once at run time from a game-data record (not traced) | one and the same PID, 0x10006F25 (an integer property, not an object reference), in 41,344 nodes; 0 in 10 |
| extra byte after the base block | stored never-ending classification (§5.3) | computed | 0 in 27,130, 1 in 14,224 |

The node's in-memory defaults matter only for nodes created by a writer; every stored node
carries all fields (`script-table.md` §5.3).

<a id="s5-3"></a>
### 5.3 The never-ending classification and its stored byte (BIN + OBS)

A Particle node classifies its effect as **finite** (it runs out by itself) or
**never-ending** (it runs until stopped).

- **Stored.** The byte after the node's base block (`script-table.md` §4.1, "`particle_flag`")
  holds the classification: 0 finite, 1 never-ending. When reading a node the client takes the
  byte as it is and does not recompute it. When writing a node, or when the in-memory value is
  "not computed" (value 255), the client computes it from the effect record as below.
- **Rule for a 0x39 effect.** Never-ending when `physics_duration` > 0, or when any emitter —
  active or not — has `emission_limit` = 0 and `time_limit` = 0. Otherwise compute for each
  emitter *v* = max(`time_limit`, (`emission_limit` integer-divided by `burst_count`) ×
  `emit_interval`) + *L*′ + `start_time` − `fast_forward`, with *L*′ = `w[0]` of `lifespan`,
  plus |`w[2]`| when `lifespan` is not mode 1; take the maximum of 0 and all *v*. The effect is
  never-ending only when that maximum is negative, which cannot happen because the maximum
  starts at 0: in practice only the first two conditions make an effect never-ending. (This
  makes precise the last sentence of `particles.md` §8.17.)
- **Rule for a 0x44 effect.** Never-ending when the record's `flag` = 1 (`particles.md` §6.1).
- **Data.** All 41,354 stored bytes equal this rule's result for the node's current effect
  record (OBS, RT4). Example: record 0x39002566 has `fast_forward` 10 and three emitters
  whose *v* are −2.1, −5.1 and −7.2; its nodes store 0.

<a id="s5-4"></a>
### 5.4 Starting a Particle node (BIN)

When the script runner reaches a Particle node (`script-runtime.md` §3.1 step 6):

1. **Effect DID.** Chosen by `particles.md` §7.1.1. If there is none, the start fails (no
   instance; the node completes at once, `script-runtime.md` §3.1).
2. **Classification** (§5.3).
3. **Intensity gate.** If the effect is **finite** and either the context's intensity
   (`script-runtime.md` §4; the same context value its intensity propagation writes, BIN) is
   ≤ 0.0002 or the context's flags word has bit 0x40 set, nothing
   is started and the node completes at once. A never-ending effect is not gated here.
4. If `ParticleScript_DropEffect` = 1: the dropped path (§5.6); the node completes at once.
5. Otherwise the attached path (§5.5).

The intensity is used only by this gate: the effect request carries no intensity, and the node
instance's intensity-change hook is the generic one, so a later intensity change does not reach
the effect either. A finite effect started at intensity 0.5 looks the same as one started at 1.

> **OPEN (Q-RT2):** Which client paths set bit 0x40 of a script context's flags word
> (`script-runtime.md` §2.1 lists bits 1 and 3 only). **Decision (Tessera):** treat it as clear.

**Driven node fields (OBS2, RT24).** Only 2 of the 41,354 Particle nodes have driven pairs,
and both drive `ParticleScript_ParticleID` from a property that has no stored value in the
archives (`QuestState_True_Particle`, `QuestState_False_Particle`; `particles.md` §7.1.2), so
its value can only come from the session at run time. At the start the node uses the field's
value at that moment (`script-runtime.md` §5.2–§5.3: every instance field is read from the
node copy when it is used). Whether a started effect follows a later change of the driven
field is Q-RT8.

**Walker rules (Tessera, 2026-10-09; answers the implementer OPEN of that date).** Where this
spec gives no fact for the offline walker (`tessera-scriptgraph` runs the node,
`apps/client/src/walker/effects` the paths):

- **(a) Redirect.** The redirect of §5.5 step 1 and §5.8 names "an existing object" only when it
  names an owner spawned in the walker's effects runtime; the offline walker has no other
  object IDs.
- **(b) Start gating.** `particles.md` §7.1.2 runs with the client's defaults
  (`DynamicParticles` 0, `StaticEnvironmentalObjects` on). With these the whitelist decides
  nothing, so the walker need not read it; a client that exposes the options reads it.
- **(c) Driven fields.** Driven pairs of Particle nodes are applied at the start like any
  node's (above); with no session the two driven nodes' source property is absent and the
  field keeps its stored value. A later change is not applied (Q-RT8 Decision).
- **(d) Opacity.** The instance opacity of attached and dropped placements is 1 until Q-RT7 is
  answered; `particles.md` §8.15's object opacity and `inherit_opacity` multiply it once the
  walker keeps a parent-object opacity.
- **(e) Water probe.** The probe of §5.7 tests the landblock water planes (`water.md` §8.1)
  only, not cell water or placed liquid volumes; this departs from the client, whose probe
  tests water surfaces in general (§5.7), and is accepted until the walker has those surfaces
  in one query.
- **(f) Scale vectors.** *A* = `Render_AdjustableScale` × `Model_Scale` for static entities
  (factors 3 and 4 of `setup.md` §5.2 need attachment parents, which the walker does not have)
  and the drawn model scale for placed figures; *p* is the product of `value_b` along the
  object's visual chain (`setup.md` §5.2).
- **(g) Observation (no rule).** At reference location M3-03 the 40 owners spawned asked for 2
  effects; the other owners' Particle nodes wait behind Switch nodes on properties the walker
  does not set, because it feeds no world state into these scripts.

> **Answered (2026-10-09; was OPEN, implementer, 2026-10-09):** (a), (b), (e), (f) are
> confirmed as the walker rules above; (c) is answered by the census (RT24: two nodes, both
> driving the effect DID from a session-only property) with the start-time rule above and
> Q-RT8; (d): which owner value gives an attached instance its own opacity was not traced and
> is Q-RT7 (Decision: 1); (g) is recorded as an observation.

> **Update (spec writer, 2026-10-23):** (d) is completed by rule 15 (§12): the parent object's
> opacity that `inherit_opacity` multiplies in is the owner's drawn opacity; the instance's own
> opacity stays 1 (Q-RT7).

<a id="s5-5"></a>
### 5.5 Attached path (BIN)

1. **Target.** The target is the script's entity, unless `ParticleScript_RedirectProperty` ≠ 0:
   then the client reads that property from the script's entity; if the entity has it and its
   value names an existing object, that object is the target. If not, the script's entity is.
2. **Attachment pre-check.** If `HoldingLocation` or `BoneType` is non-zero and the target has a
   drawn model with a skeleton: look up the bone type (when `BoneType` ≠ 0, which wins) or the
   holding location on it. If the lookup fails, nothing starts (go to step 5).
3. **Effects interface.** If the target has no effects component, nothing starts (go to
   step 5).
4. **Gating and request.** The start gating of `particles.md` §7.1.2 (owner kind, whitelist,
   `DynamicParticles`, `StaticEnvironmentalObjects`). If it passes, the client creates a
   **request** with a new handle and asks for the record. A shareable running copy may serve
   it at once (`particles.md` §8.16); otherwise the record is loaded asynchronously, and all
   requests for the same DID that arrive while it loads are served when it is ready. The
   handle is added to the component's pending list.
5. **Node result.** It depends only on the classification, not on whether anything started.
   A **finite** effect: the node completes at once; a started effect plays on by itself. A
   **never-ending** effect: the node leaves an **instance** holding the handle (an invalid
   handle when nothing started in steps 2–4), the target's ID and the `ImmediateDisposal` byte;
   when `RedirectProperty` ≠ 0 the instance also listens for changes of that property on the
   script's entity (§5.8). Such an instance runs until it is stopped, like every never-ending
   node, even when its effect could not start. The one exception: a script without an entity
   fails the start (no instance).
6. **When the record has loaded**, the component takes the handle off its pending list (if
   the handle is no longer pending, the effect was stopped meanwhile: the new system is
   discarded, §7.5). For a fork record it also copies the node's fade settings
   (`particles.md` §6.5). It then resolves the attachment again (§6.1). If the holding location
   or bone is not found on the owner's model this time, the client logs a warning and discards
   the system (the effect never appears). Otherwise it stores an **attachment** (§4), gives the
   system the owner's frame and render scale, and registers it with the owner's scene.

> **OPEN:** (implementer, 2026-10-11) step 1 does not say whether a Particle node's own
> targets (`script-table.md` §4.4) change the target. Combat scripts aim Particle nodes at
> the upcoming targets of a story through Property targets (type 3) or Random targets over
> them (type 6, `combat-visuals.md` §4.6). Until this is settled the walker
> (`tessera-scriptgraph`) applies step 1 unchanged to nodes without such targets (This,
> ThePlayer and the rest keep the script's entity); a node with a Property or Random target
> and no redirect property attaches to the first entity its targets resolve to, and when
> they resolve to none (an empty target array) nothing starts: a finite node completes, a
> never-ending node keeps an instance without an effect as in step 5.

<a id="s5-6"></a>
### 5.6 Dropped path (BIN)

A dropped effect is placed **once** at the attachment point and then stays in the world; it
does not follow the owner and the node keeps no instance.

1. The target is always the script's entity (no redirect).
2. **Attachment frame.** The client computes the attachment point's current world frame with
   the rules of §6.1–§6.3 (holding location or bone, `BoneOffset`, `RemoveRotation`), using the
   entity's current pose. If the attachment cannot be resolved, nothing is placed.
3. **Water** (only when `ParticleScript_OnWater` = 1): §5.7. If the probe finds no water,
   nothing is placed. On water the offset of step 4 is zero.
4. **Second offset.** The effect's frame is the attachment frame moved by `BoneOffset` along the
   attachment frame's own axes (the offset rotated by that frame's rotation). Because step 2
   already applied `BoneOffset` in the bone's space, a dropped effect without water is offset
   twice: for an effect with no holding location and no bone, about 2 × `BoneOffset` from the
   owner's origin in the owner's axes (BIN; 1,421 of the 2,856 dropped nodes have a non-zero
   offset, OBS). This is the original's arithmetic.
5. **Placement.** The system is created for the effect DID **without** the start gating of
   `particles.md` §7.1.2 and without the node's fade settings, with one fixed draw instance:
   the frame of step 4, scale vector *A* = the entity's render scale, *B* = (*p*, *p*, *p*)
   with *p* = the entity's `Model_ParticleScale` (§6.4), opacity 1. When loaded it becomes a
   **lingering entry that was never stopped** (§7.3): it is removed when its end test passes.

<a id="s5-7"></a>
### 5.7 Water probe (`OnWater`, BIN)

From the attachment point's position the client casts a vertical segment against water
surfaces only (`water.md`):

- if the point is under a water surface of the cell it is in (the client's under-water test,
  not traced further): from 40 m above the point to 1 m below it;
- otherwise: from the point to 10 m below it.

Of all hits it takes the one with the highest Z. The effect's position keeps its X and Y and
takes that hit's height (the hit plane's height at the hit). Without a hit nothing is placed.

<a id="s5-8"></a>
### 5.8 Redirect property (BIN)

`ParticleScript_RedirectProperty` names a property of the script's entity whose value is an
object ID (§5.5 step 1). For a never-ending effect the node instance listens for changes of
that property; on each change it stops the running effect as in §7.1 (with the node's
`ImmediateDisposal`) and starts it again on the newly named object by §5.5 steps 1–4,
replacing its stored handle and owner ID.

In this install the field holds the PID 0x10006F25 in 41,344 nodes, an integer property rather
than an object reference, and 0 in 10 (OBS). Unless an entity carries that property with the
ID of an existing object as its value, the effect stays on the script's entity.

---

<a id="s6"></a>
## 6. Attaching a running effect to its owner

<a id="s6-1"></a>
### 6.1 The attachment matrix (BIN)

When an attached effect is created (§5.5 step 6), the client builds a 4 × 4 **attachment
matrix** *M*ₐ and a bone index:

| Node fields | Bone index | *M*ₐ |
|---|---|---|
| `BoneType` ≠ 0 | the bone of that bone type on the owner's skeleton | identity rotation, translation = `BoneOffset` |
| `BoneType` = 0, `HoldingLocation` ≠ 0 | the holding location's bone (`skeleton-anim.md`) | the holding location's frame as a matrix, with `BoneOffset` added to its translation (not rotated by the holding location's rotation) |
| both 0 | none (the owner's model frame) | identity rotation, translation = `BoneOffset` |

`BoneOffset` is therefore in the space of the bone (or of the owner's model when there is no
bone), in metres, before the owner's render scale.

<a id="s6-2"></a>
### 6.2 Each frame (BIN)

Each time the owner's effects component updates its attachments (it is called from five
places in the owner's frame and pose updates, each passing the owner's render scale; which
events trigger them was not traced), for each attachment:

1. Local matrix *L* = *M*ₐ when there is no bone index; otherwise *M*ₐ multiplied by the
   bone's current model-space matrix of the pose (row vectors: *M*ₐ first).
2. If the attachment's `RemoveRotation` byte is set, the rotation part of *L* is replaced by
   the identity; its translation is kept. The effect then follows the bone's position but not
   its rotation; it still turns with the owner (step 3).
3. The system's frame is *L* followed by the owner's world frame. Its scale vectors are set by
   §6.4.

<a id="s6-3"></a>
### 6.3 What follows the owner

A world-space system (`world_space` = 1) is moved by these frames only at spawn: its particles
keep their world positions (`particles.md` §8.6.3). A local-space system draws its whole
particle set through the current frame, so it follows the owner completely
(`particles.md` §8.16). Emitters with their own `holding_location` or `bone`
(`particles.md` §8.5.5) attach their particles to the owner a second time, inside the system.

<a id="s6-4"></a>
### 6.4 The owner's two scale vectors (BIN; answers `particles.md` Q4)

Every draw instance carries the two vectors *A* and *B* that `scale_mode` combines
(`particles.md` §8.6.3):

- ***A*** = the owner's **render scale**, the per-axis scale the client draws the owner's
  model with (`setup.md` §5.2);
- ***B*** = (*p*, *p*, *p*), *p* = the owner's `Model_ParticleScale` (PID 0x5A0, a float; the
  model component serves the template's `value_b` under that label, `setup.md` §5.2). When the
  property is absent or not a float, *p* = 1.

So `Total` scales effects by the render scale times the particle scale, `VDescOnly` by the
particle scale alone, `None` not at all. Attached effects receive both vectors each time their
frame is updated; dropped effects receive them once (§5.6); lingering effects keep the last
ones (§7.2).

---

<a id="s7"></a>
## 7. Stopping, lingering and removal

<a id="s7-1"></a>
### 7.1 Stopping an attached effect (BIN)

A never-ending effect is stopped when its node instance is stopped (`script-runtime.md` §3.5:
the end of its script, a case change, an entity removal, and so on). Finite effects have no
instance and are never stopped by their script.

To stop it, the owner's effects component removes the handle from its pending list and, if the
effect is attached, removes the attachment. If `ParticleScript_ImmediateDisposal` = 1 it first
marks the system for **immediate disposal**. It then releases its reference (§7.2).

<a id="s7-2"></a>
### 7.2 The release rule (BIN)

When the last reference to a system is released (a stop, a finished attachment, an entity's
component going away; every path ends here):

1. The system leaves the shareable list.
2. It **lingers** when all of these hold:
   - its record is loaded;
   - it had no draw instance in the current frame, or its first instance's opacity is at least
     0.0002;
   - it is not marked for immediate disposal;
   - it is not of the shareable kind (`shareable` = 1, `world_space` = 0, `use_lighting` = 0,
     `particles.md` §8.16): a shared system never lingers;
   - it is registered with a scene that is still active (one further test of the scene was
     not traced).
3. If it lingers:
   - a local-space system that had at least one draw instance this frame keeps the first
     instance's transform, scale vectors and opacity as its fixed placement;
   - a world-space system keeps its own last frame;
   - a local-space system with no instance this frame does **not** linger after all.

   A lingering system is **stopped at now** (it records the stop time and from then on runs no
   emission, `particles.md` §8.1), registered with the scene and added to the lingering list
   (§7.3). Its particles go on moving, fading and dying.
4. Otherwise it is destroyed at once: its particles disappear.

<a id="s7-3"></a>
### 7.3 The lingering list (BIN)

Once per frame (§3.2 step 1) every lingering entry is removed (and its system destroyed) when
either:

- it has been stopped and more than **6 s** have passed since the stop time; or
- its end-of-effect test passes (`particles.md` §8.2 for 0x39, §6.5 for 0x44).

So a stopped never-ending effect disappears 6 s after its stop, even when its particles live
longer (1,044 emitters of never-ending 0x39 records have a constant `lifespan` above 6 s, up to
240 s, OBS); its particles that die earlier are simply gone. A dropped effect is never stopped,
so only its end test removes it; every dropped node of this install names a finite effect
(§5.2). Tearing a scene down also removes the lingering entries registered with it (BIN; that
the caller is the scene's teardown is Inferred).

<a id="s7-4"></a>
### 7.4 Housekeeping of attached effects (BIN)

Each effects component keeps the time of its last check. When more than **2 s** have passed
since then, it records now as the new check time and removes every attachment whose system has
**finished**: stopped more than 6 s ago, or its end test passes. The removal releases the
system (§7.2); since a finished system's end test already passed, a lingering entry made from it
goes at the next sweep (§7.3). A finite attached effect therefore disappears at the first check
after its end test passes, up to 2 s later; its particles are all dead by then (the end test
includes the lifetime).

<a id="s7-5"></a>
### 7.5 Effects stopped before their record loads (BIN)

A request whose handle is stopped while its record is still loading is removed from the pending
list; when the record arrives, the system made for it is discarded and nothing appears.

---

<a id="s8"></a>
## 8. Simulation

The simulation of a running 0x39 system is `particles.md` §8 without change: update cadence
and interpolation (§8.1), window and end test (§8.2), timed and distance emission (§8.3–§8.4),
spawning (§8.5), channels (§8.6), controllers (§8.7), forces (§8.8), fade envelope (§8.9),
atlas frames (§8.10), primitives (§8.11), billboards (§8.12), distance fade (§8.13), lighting
(§8.14), order and passes (§8.15), sharing (§8.16), start and restart (§8.17). This spec adds:

- the clock and the random source (§3);
- that a **stopped** system (§7.2) runs particle updates but no emission;
- the storage order of particles (§9.2), which decides the draw order inside an emitter;
- when a channel value is drawn, the age is now − the particle's spawn time at the draw (the
  clock of the draw, not of the last particle update), while the key that is current is the one
  reached at the last particle update (BIN).

---

<a id="s9"></a>
## 9. Drawing one system

<a id="s9-1"></a>
### 9.1 Draw calls and material (BIN)

For each draw instance of a 0x39 system that passes the distance test (`particles.md` §8.13)
the client writes the vertices of all emitters into the shared draw buffer (`particles.md`
§8.11: `Quad` emitters first, then `Triangle` emitters) and issues at most two draw calls:

1. the `Quad` vertices as an indexed triangle list, two triangles per quad (0, 1, 2) and
   (0, 2, 3) through a shared quad index buffer;
2. the remaining (`Triangle`) vertices as a non-indexed triangle list.

Both use the system's **loaded surface record** (`particles.md` §4.1) and its material and
shader as `material.md` describes them, with the instance's transform (`particles.md` §8.16;
the frames of local-space systems are `particles.md` Q8), a colour multiplier (1, 1, 1, instance
opacity) and the global clock as the material's time. The draw call receives no blend, depth or
cull state from the particle code (BIN: its arguments are the buffers, counts, the material and
shader of the surface record, the transform, the colour multiplier and the clock), so **these
states are those of the surface record's material** (`material.md` §7). Vertex colours carry
the particle colour of `particles.md` §8.6.2. The second call passes one more flag value
(0xC000) whose meaning was not traced (Q-RT6).

<a id="s9-1-1"></a>
#### 9.1.1 Texture, vertex colour and tint in the surface programs (OBS2, BIN-S; RT21–RT23)

**Which records.** The 11,153 systems with a surface use 1,179 distinct surface records and,
through them, 19 shader records. All 19 are unlit in the sense of `material.md` §11.3 (no
additive light passes) and draw in one pass; every render-state block of 18 of them is
SRCALPHA / INVSRCALPHA ("alpha" below) or SRCALPHA / ONE ("additive"). Effective values over
the 1,179 surfaces (`material.md` §9.2): alpha test (4004) off in 1,049, on in 8, not declared
in 122; depth write (4002) off in 1,177, on in 2 (Confirmed, OBS2).

**What the programs compute (BIN-S).** Notation: *T* = the colour-texture sample (slot 3000);
in the two-layer records *T*.alpha is the product of both layers' alphas and *T*.rgb the
product (0x2B000703, 0x2B00071B, 0x2B000742, 0x2B000776, 0x2B00077A) or the sum (0x2B0007AD,
0x2B000717, 0x2B00072D) of both layers' colours. In these eight records the first layer is read at
texture matrix 0 and the second at texture matrix 1 (`material.md` §8.4, both applied to the
vertex UV pair, the matrices evaluated at the draw's clock); the eleven one-layer records read
their texture at the vertex UV unchanged (BIN-S second pass, RT27). Of the 206 two-layer particle
surfaces, 182 have a matrix that is not the identity (178 in the second layer, 34 in the first)
and 171 an animated one (a curve that is not constant; 170 in the second layer), by their
effective values of 1001–1004, 1017–1020, 1033–1034 and 1070–1073 (OBS3).
*V* = the vertex colour, which carries the particle colour of `particles.md` §8.6.2. *M* = the
material colour 2000 times the draw's tint (`material.md` §8.3); the tint is the colour
multiplier (1, 1, 1, instance opacity) of §9.1 (RT23, Inferred).

| Shader record (surfaces, systems) | Blend | Alpha test reads | Output alpha | Output colour |
|---|---|---|---|---|
| 0x2B00070F (415, 3,281), 0x2B000779 (1, 44), 0x2B0007F1 (3, 10) | alpha | *T*.alpha | *T*.alpha × *V*.alpha × *M*.alpha | *T* × *V* × *M*, fogged |
| 0x2B0007EB (130, 1,003), 0x2B000760 (1, 3) | alpha | *T*.alpha | *T*.alpha × *V*.alpha × *M*.alpha | 2 × *T* × *V* × *M*, fogged |
| 0x2B00071B (46, 313), 0x2B0007AD (49, 158), 0x2B00072D (1, 1) | alpha | *T*.alpha (two layers) | *T*.alpha × *V*.alpha × *M*.alpha | *T* × *V* × *M*, fogged |
| 0x2B000703 (24, 726), 0x2B00077A (1, 1) | alpha | *T*.alpha (two layers) | *T*.alpha × *V*.alpha × *M*.alpha | 2 × *T* × *V* × *M*, fogged |
| 0x2B000755 (1, 10) | alpha | *T*.alpha | *T*.alpha × *M*.alpha (no vertex colour) | *T* × *M*, fogged |
| 0x2B0007CB (279, 2,979), 0x2B000701 (2, 7) | additive | *T*.alpha | *T*.alpha × *V*.alpha × *M*.alpha | *T* × *V* × *M*, not fogged |
| 0x2B000742 (64, 417), 0x2B000717 (20, 78), 0x2B000776 (1, 3) | additive | *T*.alpha (two layers) | *T*.alpha × *V*.alpha × *M*.alpha | *T* × *V* × *M*, not fogged |
| 0x2B0007F2 (121, 1,909), 0x2B000726 (1, 1) | additive | nothing (no 1043 declared; the role is `Color`) | *V*.alpha × *M*.alpha: the texture's alpha does not enter | *T* × *V* × *M*, not fogged |
| 0x2B000084 (19, 209) | alpha | nothing (the program has no discard, although the record declares 1043) | *T*.alpha × *V*.alpha (no *M*) | the frame-buffer image read at a position offset by the texture (heat distortion); the record's variant without a frame-buffer input blends ZERO / ONE and leaves the target unchanged |

"Fogged" is the distance fog of `lighting.md` §10 applied to the colour, (1 − *f*) × colour +
*f* × fog colour, after the doubling where there is one. In the permutations that have a depth
input the output alpha is further multiplied by the soft-particle factor of parameter 1174
(`material.md` §8.3). The OpenGL programs checked agree with the Direct3D 10/11 ones.

**Rules that follow.**

1. **Alpha test.** The test compares the texture's opacity *T*.alpha alone with the reference;
   (0x2B0007F2, 0x2B000726 and 0x2B000084 test nothing); the vertex colour, the material colour
   and the tint never remove texels (as for meshes,
   `material.md` §11.2: "vertex-colour alpha is applied after the test"). The reference is set
   per draw by `material.md` §11.2 ("What the client does per draw") with *A* = *M*.alpha: every
   particle permutation has `src_blend` SRCALPHA, so with alpha test off the reference is 0 and
   texels whose texture opacity is 0 are discarded; with alpha test on and depth write off (7 of
   the 8 surfaces) the test is switched off for the draw while *A* < 0.9, that is while the
   instance fades below 0.9; with depth write on the reference is scaled by *A*.
2. **Blend.** The fixed-function blend reads the program's output alpha of the table as source
   alpha. For the common records that is texture opacity × vertex alpha × tint alpha (times the
   2000 alpha), so a draw that multiplies texture, vertex colour and tint and blends by the
   result's alpha matches them; the exceptions are the rows for 0x2B0007F2 and 0x2B000726
   (texture alpha left out), 0x2B000755 (no vertex colour) and 0x2B000084.
3. **Meshes are not changed by this section.** `render.md` §4.3 (`Fixed` row) gives meshes with
   fixed blends the tested opacity of `material.md` §11.2 alone (§11.3), without *M*.alpha and the vertex
   alpha, as a Decision while the client's rule is OPEN in `material.md`. Particle draws follow
   this section instead, because their 19 records were read program by program. The
   observation that these unlit programs do multiply the vertex and material alphas into the
   blend's source alpha is offered to the owners of `material.md` and `render.md` (§16).

> **Answered (2026-10-10; was OPEN, implementer, 2026-10-09):** the five engineering choices,
> item by item.
> (a) **Confirmed** as Tessera rule 10: a surface whose shader record is not one of the 19 is
> drawn as the common row of its blend class (*T* × *V* × *M*; alpha *T*.alpha × *V*.alpha ×
> *M*.alpha; the test on *T*.alpha by rule 1; one layer at the vertex UV; no doubling; fogged
> unless the destination factor is ONE).
> (b) **Confirmed** as Tessera rule 11: 0x2B000084 is drawn in its variant without permutation
> feature bit 1, which leaves the target unchanged, so nothing is drawn. The client draws the
> distortion variant only while bit 1 is active (§9.1.2: render-target effects and surface
> reflections on); the formula is recorded there.
> (c) **Confirmed** as Tessera rule 11: no soft-particle factor. The client applies it only in the
> variants that require bit 1 (§9.1.2).
> (d) **Replaced:** the client applies texture matrix 0 to the first layer and texture matrix 1
> to the second in all eight two-layer records (notation above, RT27); 182 of the 206 two-layer
> particle surfaces have a matrix that is not the identity, 171 an animated one. Evaluate both
> per `material.md` §8.4 at the draw's clock and apply them; one-layer records apply none.
> (e) **Reference: replaced in part.** The client evaluates the 1043 curve at the clock for every
> draw (`material.md` §11.2). Every particle surface of this install has a constant reference
> (RT28), so a value resolved once is identical here; for a curve that is not constant, evaluate
> it per draw. The rest of (e) is confirmed: the reference is carried whenever the material's
> alpha test is on, also with depth write off, where rule 1's switch at *A* < 0.9 applies.
> **Fog: confirmed.** The programs read the frame's distance fog (`lighting.md` §10); a pass
> without frame lighting has none, which matches the client with fog off (it uploads distances
> so large that *f* stays 0, `lighting.md` §10).

> **Answered (2026-10-10, BIN pass (i), OBS3; RT31; was OPEN, implementer, 2026-10-10;
> engineering choice, the spec is silent):** Whose curve
> states (`material.md` §5.3.1: the random source of mode 3, the key search of mode 10) the
> per-draw evaluation of 1043 and of texture matrices 0 and 1 uses. The walker keeps one set
> per 0x39 record, shared by all its instances and kept while the record is cached (bounded),
> and evaluates at the material clock; the explorer preview evaluates at its own simulated
> time with fresh states per frame. Only surfaces with a curve that is not constant are
> re-evaluated (none for 1043 in this install, RT28).
> Answer: the client keeps the states **per loaded surface record**, not per 0x39 record.
> - Mode 3 keeps no state of its own: every evaluation draws from the client's one global
>   generator (`material.md` §5.3.1, §3.3 here; rule 1).
> - A mode-10 curve keeps its state (start time and last value) inside the curve object
>   itself (BIN). The curves a particle draw evaluates (1043 and the values of texture matrices
>   0 and 1) are copies held by the **effect the client builds for the loaded surface
>   record**. The draw call of §9.1 receives that surface's shader record and built effect
>   (RT16). The client builds the effect from the shader record's defaults and the surface's
>   materials (`material.md` §9.2), copying each curve. It does so the first time the surface
>   is drawn after it loads, and again whenever the surface is marked for a rebuild (Q-RT10).
>   A new build starts every mode-10 curve afresh.
> - So one state set serves every 0x39 record that names the surface, all their instances
>   and any other draw of that surface record. The 0x39 record reaches its surface through the
>   client's shared object fetch by DID (`particles.md` §4.1, P50), so all users of one
>   surface DID share one loaded surface while it is loaded. That the fetch returns the
>   already-loaded object is Inferred: the object manager behind it was not traced.
> - Every draw evaluates at the global clock (§9.1). Evaluations at one clock give the same
>   value, so drawing a surface several times per frame changes no mode-10 state; a mode-10
>   curve starts at the first evaluation after a build. A mode-3 curve is the exception: it
>   draws anew at every evaluation, so every draw of the surface gets its own value
>   (`material.md` §8.4: the items are evaluated for each permutation drawn).
> - The data (OBS3): over the 1,179 particle surfaces, the curves of 1043 and of the
>   texture-matrix values that are not constant use modes 2 (154 surfaces), 4 (25), 7 (8),
>   3 (3), 10 (2) and 6 (1); no 1043 is among them. The choice of owner is visible only on
>   the two mode-10 surfaces (3 systems). One of them is named by two 0x39 records, which
>   share one state in the client.
>
> Rule 12 states what Tessera does: key the states by surface record DID; a preview keeps
> its states for the length of its run. Fresh states every frame freeze a mode-10 curve at
> its start value.

> **Answered (2026-10-10, third revision; RT32, RT33; was OPEN, implementer, 2026-10-10;
> engineering choices under rule 12, the spec is silent):** How rule 12 is applied, for
> review: (a) mode 3: before each evaluation, the state of
> every mode-3 curve that the draw evaluates is replaced by a fresh one seeded from one draw
> of the shared stream (one draw per mode-3 curve per evaluation, `material.md` §5.3.1
> allows any uniform source), so nothing carries over between evaluations; (b) walker: the
> states are keyed by the surface DID (`surface`, else `extra_surface`) and live as long as
> the effects layer, because the walker's effect-record cache holds each surface's resolved
> material for the session and never drops it (so "discard with the cache entry" does not
> occur); at most 4,096 surfaces keep states, beyond that a draw uses fresh states;
> (c) explorer preview: the look is evaluated at every simulation step (30 per second) with
> states kept for the run and started afresh on a restart; each sheet column is a run of
> its own from time 0. Are these acceptable, or should the walker drop a surface's states
> at some other point (for example when no system drawing the surface remains)?
>
> Answer, item by item.
> - **(a) Corrected.** The client keeps no state for mode 3 at all: each evaluation takes one
>   number of the one global generator (§3.3) and turns it into *r* (RT31). Rule 9's last
>   bullet already fixes the mapping: *r* is one real-range draw over [−1, 1] of the shared
>   stream (§3.4), used as it is. Seeding a fresh generator from a draw and taking *r* from that
>   generator uses the same number of stream draws but not that mapping, and it runs a full
>   seeding (forty steps and a table fill, §3.3) per curve and evaluation for nothing. Take
>   *r* from the shared stream directly: one draw per mode-3 curve per evaluation, a new value
>   at every draw of the surface (above), and nothing kept between evaluations. The count of
>   draws is what the client does; the order in which the curves of one draw take their
>   numbers is not known and does not matter (rule 1: sequences cannot match the client).
>   Three particle surfaces have a mode-3 curve (RT31).
> - **(b) Confirmed.** Keying by the surface DID that the system draws (`surface`, else
>   `extra_surface`) and keeping the states for the life of the effects layer satisfies rule 12:
>   the rule ties the states to the cache entry, and a cache entry that is never dropped keeps
>   them for the session. No further drop point is required: not at material cache eviction
>   (where a walker does evict, rule 12 already drops the states with the entry), not at an
>   effect's stop and not at a landblock unload. Why the choice does not matter in this
>   install: every mode-10 curve of a particle surface loops (RT32), so a state kept longer
>   than in the client, or restarted earlier, shifts only the phase of a loop of 1.2 s or 4 s,
>   never a held value. In the client the state lives as long as the loaded surface record;
>   a loaded 0x39 record naming it keeps it in its loaded-surface slot (`particles.md` P50),
>   and whether the client keeps it after its last user is released is not known (Q-RT10).
>   Dropping a surface's states when no live system draws it any more is therefore also allowed, not
>   required. The cap of 4,096 surfaces is an engineering limit in the sense of rule 9 (this
>   install has 1,179 particle surfaces, so it is never reached); with fresh states beyond it a
>   mode-10 curve stays near its start value (position 0.1, `material.md` §5.3.1), which is
>   accepted for such input.
> - **(c) Confirmed.** A preview evaluating at each simulation step with the states kept for the
>   run, started afresh on a restart, and each sheet column as its own run from time 0, is
>   rule 12's preview clause. The step rate (30 per second) instead of a per-frame draw changes
>   only how often mode 3 draws and when a looping mode-10 curve restarts (at the first step at
>   or after a whole period), both within what a preview may show.

> **OPEN:** (implementation, 2026-10-10, rule 12 and RT33, engineering choices recorded for
> review) (a) When a surface is loaded Tessera builds a snapshot of its values at clock 0
> (`tessera-particles` `ParticleSurface::of`). Every value that follows the clock is evaluated
> again at each draw, so the snapshot of such a value is never drawn; in the snapshot a mode-3
> curve reads its centre value *B* (*r* = 0), so loading a surface takes nothing from the
> shared stream and uses no other random source. (b) The explorer preview now evaluates the
> look once per drawn frame, at that frame's clock and after the system's draw (the walker's
> order), instead of at every simulation step as answer (c) above allowed. Please confirm or
> correct.

<a id="s9-1-2"></a>
#### 9.1.2 Permutation feature bit 1: soft particles and heat distortion (BIN, BIN-S, OBS3; RT25, RT26, RT29)

**Selection (BIN, RT25).** The client picks the permutation of each draw with a feature mask
of the frame: a permutation is used when every bit of its key word 1 is in the mask and no bit
of its key word 2 is (`lighting.md` §3.1). Bit 1 (value 2) of the mask is set only when all of
the following hold, and is clear otherwise:

1. the client's render-target effects are active: the user option `Render.VideoPostEffects`
   (a switch whose description names heat distortion, motion blur and glare) is on, and the
   graphics device qualifies (two device capability flags and a hardware class of at least 2;
   not traced further);
2. the user option `Render.SurfaceReflections` (water and mirror reflections) is not 0;
3. two internal flags of the frame are set; the client clears them while it draws an auxiliary
   view and sets them again afterwards (which view was not traced).

The option names are those of the user's preference file, given for interoperability.

**Which particle permutations depend on it (OBS3, RT26).** Nine of the 19 records (0x2B00070F,
0x2B0007EB, 0x2B00071B, 0x2B0007AD, 0x2B000703, 0x2B0007CB, 0x2B000742, 0x2B000717,
0x2B0007F2) have a Direct3D 10/11 colour permutation that requires bit 1 and reads the scene
depth image, beside one that excludes bit 1 and reads no depth. 0x2B000084 has variants that
require bit 1 and read the frame-buffer image (Direct3D 9 shader model 2.0, OpenGL and Direct3D
10/11) and variants that exclude it (blend ZERO / ONE). The other nine records do not use bit 1.

**Soft-particle factor (BIN-S, the nine records; parameter 1174).** With *k* = the effective
1174 and *w* = the fragment's view depth (the clip-space w of the vertex, interpolated):

- if *k* ≤ 0, the factor is 1;
- otherwise the depth image is read at the fragment's pixel position divided by the first two
  values of the frame's viewport constant (by their use, the viewport's width and height),
  *D* = its first channel, and the factor is clamp((*D* − (*w* − 0.1)) / *k*, 0, 1).

The output alpha of §9.1.1 is multiplied by the factor; the colour is not. The depth image is,
by inference, the view depth minus 0.1 that lit and unlit programs write as their second output
(`lighting.md` §11), so the factor fades a particle where it comes within *k* metres of the scene
behind it. Effective 1174 on particle surfaces: 0.3 on 997 of 1,148, 0 on 43 (RT28).

**Heat distortion (BIN-S, 0x2B000084, Direct3D 10/11).** With *T* the colour-texture sample at the
vertex UV and *V* the vertex colour:

1. screen coordinates *S* = (*x* / *w* × 0.5 + 0.5, −*y* / *w* × 0.5 + 0.5) from the fragment's
   clip position (*x*, *y*, *w*);
2. offset: *S* + (*T*.alpha − 0.5) × *h* on both axes, *h* = the effective 1160
   (`c_HeatDistortionAmount`, 19 surfaces, 0.004 to 0.6);
3. the result is mapped into the frame-buffer image by the viewport's scale and offset and that
   image is sampled there;
4. output colour = the sample (no vertex colour, no material colour, no fog); output alpha =
   *T*.alpha × *V*.alpha; blend SRCALPHA / INVSRCALPHA.

Which image the frame-buffer input holds, and when the client copies it, was not traced, also for
the 97 of the 209 distortion systems that are drawn after post-processing (`uses_video_post` = 1,
`particles.md` §8.15; RT29). See Q-RT9.

**Tessera (rule 11).** Particle draws use the variants that exclude bit 1, as the client does with
either option off: no soft-particle factor, and 0x2B000084 draws nothing. `render.md` offers no
depth or colour copy of the scene to the particle stage; the facts above are what a later revision
needs if it does.

<a id="s9-1-3"></a>
#### 9.1.3 Opacity of particle draws against the mesh rule, and in premultiplied passes (Decision; RT30)

1. **Particle draws follow §9.1.1, not the mesh fixed-blend rule.** For a 0x39 system drawn by
   §9.1, the alpha test reads *T*.alpha (both layers' product in two-layer records) with the
   per-draw reference of `material.md` §11.2 steps 1–4 and *A* = *M*.alpha (§9.1.1 rule 1), and the
   SRCALPHA / INVSRCALPHA factors of the blend read the program's output alpha of the §9.1.1
   table (in the common rows *T*.alpha × *V*.alpha × *M*.alpha). *T*.alpha is the tested opacity
   of the `material.md` §11.2 table for these records: none of the 19 particle shader records is
   one of the seven records that test the **sum** of two alphas, and the eight two-layer records
   test the product (RT35). The mesh rules for unlit records do **not** apply: neither the
   `Fixed` row of `render.md` §4.3 (the factors read the tested opacity of `material.md` §11.2
   alone, without *M*.alpha and the vertex alpha) nor the `material.md` §11.3 sentence
   "'Opacity' in a factor is the tested opacity of §11.2 (its table), or 1 if there is none"
   (pointers revised 2026-10-10 after the `material.md` §11.2–§11.3 revision; the former wording
   was "the opacity source of §11.2"). The alpha test of a `Fixed` draw (`render.md` §4.3, with
   the caller's *R*) tests the same *T*.alpha, so it agrees with the particle programs (the
   former third difference, "alpha-tested particle surfaces keep their test", no longer
   applies since `Fixed` has its test). The difference shows in two places: the instance
   opacity and the distance fade (in *M*.alpha) and the particle colour's alpha with the fade
   envelope (in *V*.alpha) fade particles through the blend; and the `Color`-role records
   0x2B0007F2 and 0x2B000726, which have no opacity-bearing role, blend by *V*.alpha ×
   *M*.alpha, where the mesh rule would read 1. A renderer that maps a particle surface to a generic
   fixed-blend state must therefore feed that state the particle program's output alpha.
2. **Premultiplied passes.** A particle draw in a pass with the premultiplied option follows
   `render.md` §6.10 "Fixed blends" (R12), with the output alpha of item 1 as the source alpha:
   - the "alpha" rows (SRCALPHA / INVSRCALPHA, ADD) are source-over (its exception 1): colour as
     recorded, alpha with `One` / `OneMinusSrcAlpha`, so the stored alpha is *a* + (1 − *a*) ×
     the destination alpha, *a* the output alpha;
   - the "additive" rows (SRCALPHA / ONE, ADD) keep the destination alpha;
   - the ZERO / ONE variant of 0x2B000084 leaves the target unchanged (and draws nothing under
     rule 11);
   - no particle record of this install has a blend with destination factor ZERO (exception 2);
     a record outside the 19 (rule 10) that has one follows `render.md` §6.10 with the same
     output alpha.

   Colour and alpha are clamped to [0, 1] before blending (the doubled rows can exceed 1), as
   the blend unit of a normalised target does. Outside premultiplied passes the blend state of
   the surface's material is used as it is.

<a id="s9-2"></a>
### 9.2 Order of particles inside a system (BIN)

- Each emitter stores its live particles in one array, in creation order: a new particle is
  appended; when a particle dies the **last** particle of the array moves into its slot, and the
  next particle checked is the one now in that slot.
- Vertices are written emitter by emitter (in stored order, `Quad` emitters before `Triangle`
  emitters) and, inside an emitter, in array order. There is **no depth sort** of particles.

So after deaths the draw order inside an emitter is no longer creation order (example in
§14.3). Between systems the order is `particles.md` §8.15.

**Frames until `particles.md` Q8 (a) is answered (Tessera Decision, 2026-10-09; answers the
implementer OPEN of 2026-10-08 as far as it can be).**

- A local-space system simulates in its own frame (the identity at the origin: spawn,
  direction and gravity are not turned by the owner). Each draw instance moves the whole
  particle set by its transform, scaled per axis by *S* (`particles.md` §8.6.3), with the
  camera axes turned into the instance's frame for the billboards.
- The two anchors of distance emission (`particles.md` §8.4) follow the world position and
  orientation of the frame's first instance, also for a local-space system; each offset from a
  stepped anchor to the current one, and the `Line` vector between the stepped anchors, is
  turned back into the system's own frame by that instance's rotation (not divided by *S*, so
  the drawn trail scales with *S* like the particles). A moving owner therefore leaves a trail.
- A world-space system simulates in world space with the frame of the first instance of the
  frame and is drawn once, for the first instance that passes the distance test
  (`particles.md` §4.1, "drawn once").
- `use_lighting` multiplies by an ambient colour only (white unless the caller supplies one);
  the point lights wait for `particles.md` Q8 (b).

> **Answered in part (2026-10-09; was OPEN, implementer, 2026-10-08):** the choices are
> confirmed as the Decision above. What the client does with local-space frames, the light
> attenuation and the ambient source stay `particles.md` Q8 (a)–(b); no new BIN reading was made
> for them in this revision.

<a id="s9-3"></a>
### 9.3 Culling, LOD and caps

- **Distance:** per instance, `particles.md` §8.13 (far and near fade, not drawn beyond the far
  end; a system with no drawn instance freezes).
- **Client options:** `particles.md` §7.1.2 (`DynamicParticles`, `StaticEnvironmentalObjects`)
  and §8.8.1 (`PrecipitationEffects`). No other particle quality or density option was found:
  the request, update and draw paths read none (BIN).
- **Caps:** per emitter `max_particles` and `emission_limit` (`particles.md` §8.3); per system
  draw 32,768 vertices (`particles.md` §8.11). No global particle count is kept or limited (BIN:
  the creation, update and draw routines read none).

> **OPEN (Q-RT3):** Whether the record's bounds (`has_bounds`, `bounds_min`, `bounds_max`,
> `particles.md` §4.1) are used for view-frustum culling. The system exposes them to the scene
> code, but the scene's culling of particle systems was not traced. **Decision (Tessera):** the
> simulation never depends on the view frustum; the renderer may skip drawing an instance when
> `has_bounds` = 1 and the bounds, transformed by the instance and expanded by the largest drawn
> particle size, lie outside the frustum. With `has_bounds` = 0 an instance is never frustum-culled.

> **OPEN (implementer, tessera-particles / tessera-client, 2026-10-23):** how the Q-RT3
> Decision is applied, for review. The box is the axis-aligned box around the eight corners of
> `bounds_min`–`bounds_max` after the instance's effective scale *S* (`particles.md` §8.6.3),
> rotation and position, grown on every axis by the largest distance of a drawn vertex from
> its particle's centre in that frame (so "the largest drawn particle size" is measured on the
> vertices actually written, including streaks and pivots); a box with a non-finite value is no
> box (never culled). A culled instance is still simulated **and its vertices are still
> built** (the vertex step draws from the shared generator, rule 1, so skipping it would make
> the simulation depend on the view); it is only not queued for the GPU. Culling uses the
> walker's view frustum; the creation viewport does not cull. A **world-space** system draws
> the particles of all its instances once, at the positions they were emitted at: its box is
> the union of every passing instance's box (as above) and the axis-aligned box of the vertices
> written, so a trail left behind by a moving owner, or a second instance in view, is never
> culled with the first instance.

> **Answered (spec writer, 2026-10-23):** confirmed as Tessera rules; no client fact was
> traced for them (Q-RT3 stays open for the client's own culling). The box (the eight corners
> of `bounds_min`–`bounds_max` after *S*, rotation and position, grown per axis by the largest
> written vertex distance from its particle's centre; a non-finite box never culls), the
> culled instance that is still simulated and still builds its vertices (rule 1 forbids a
> view-dependent draw on the shared generator), the walker-only frustum (the creation
> viewport does not cull) and the world-space union box (every passing instance's box and the
> box of the written vertices) are the rule.

---

<a id="s10"></a>
## 10. Fork effects at run time

<a id="s10-1"></a>
### 10.1 Same life cycle (BIN)

A fork effect (prefix 0x44) goes through the same Particle-node start (§5), attachment (§6) and
stop, linger and removal (§7) as a 0x39 effect, with these differences: it is never-ending when
its `flag` = 1 (§5.3); its fade comes from the node on the attached path (`particles.md` §6.5);
it is always local-space, never shared, and its end test is the `tail_value` rule of
`particles.md` §6.5. Its simulation is the third-party library's (`particles.md` Q1). Nested
effects (§6.3 of `particles.md`) start and stop with their parent effect (`particles.md` §6.5).

<a id="s10-2"></a>
### 10.2 Shader by blend-mode index (BIN)

When the client creates a fork system it fetches the seven shader records of `particles.md`
§6.4 from UNIQUEDB. Each draw batch the library produces carries a **blend-mode index**; the
client draws the batch with:

| Index | UNIQUEDB key (label) |
|---|---|
| 0 | 0x15C (`…_Normal`) |
| 1 | 0x15E (`…_Emissive`) |
| 2 | 0x15F (`…_Subtractive`) |
| 3 | 0x15D (`…_DarkMap`) |
| 4 | 0x159 (`…_LightMap`) |
| 5 | 0x15A (`…_Emissive2`) |
| 6 | 0x15B (`…_Emissive3`) |
| any other | nothing is drawn |

For indices other than 2 the client binds textures of the effect, looked up by name among the
render surfaces of the record's `resources` (`particles.md` §6.4), to the shader's texture
parameters once, the first time the batch is drawn; for index 2 it skips that step. Where the library takes the index from, and the batch primitives, are
part of `particles.md` Q1.

**Without the fork library (Tessera rule, 2026-10-09; answers the implementer OPEN of
2026-10-08).** Tessera has no implementation of the third-party fork library (`particles.md`
§10.2), so a fork effect draws nothing. It counts as active until the `tail_value` rule of
`particles.md` §6.5 expires it (the exit "the library reports no live particles" is never
taken), and, because a fork effect is local-space and gets no draw instance, the release rule
of §7.2 step 3 destroys it at once instead of letting it linger. Both follow from drawing
nothing; revisit them when `particles.md` Q1 is answered.

> **Answered (2026-10-09; was OPEN, implementer, 2026-10-08):** confirmed as the Tessera rule
> above.

> **Answered in part (spec writer, 2026-10-23; `particles.md` §10 rule 2, "fork effects drawn or
> not"; OBS5, RT57):** what the install and the offline client tell: 1,524 of the 41,354
> Particle nodes (3.7 %) start one of 219 fork effects; the client draws them through a
> third-party particle library that the executable links (no library file named for it ships
> beside the executable; Inferred: linked into it), whose effect-image layout, simulation and drawing are `particles.md` Q1. No
> fact needed to draw a fork effect is known beyond `particles.md` §6.4–§6.5 and the shader
> choice above. **Tessera rule (unchanged):** fork effects are not drawn; they keep the life
> cycle of the rule above. Drawing them needs `particles.md` Q1 answered (an M4 pass on the
> library's loader, update and draw entry points, as Q1 suggests) or an owner decision on a
> stand-in look; neither is part of this revision.

---

<a id="s11"></a>
## 11. Engine-started systems

- **Weather:** the ten UNIQUEDB systems are created synchronously (loaded at once, not through
  a pending request) when the weather code starts, kept for its life and driven by it
  (`particles.md` §7.1.3, §8.8.1); when they are placed and receive particles, and where the
  global wind's target comes from, is §11.2 (since 2026-10-23; `particles.md` Q13 answered in
  part).
- **Beams:** a Beam node copies its fields into a beam object of the client's beam renderer,
  which creates its particle systems synchronously and drives them (§11.1).

> **Answered (2026-10-10; was OPEN Q-RT4):** how the beam renderer places and drives
> `BeamScript_BeamParticleSystem` and `BeamScript_HeadParticleSystem` and whether the start
> gating applies: §11.1 (RT36–RT43). The former Decision "beams draw no particle systems" is
> withdrawn; rule 14 replaces it.

> **Answered (spec writer, 2026-10-23, BIN pass (n), RT54; was OPEN Q-RT5):** the "gameplay
> event handler" of §5.1 (which event, which property names the effect) is the effect preview
> of the options window's effect-filter page: §11.3. It is not gameplay; the former Decision
> "not implemented" stands for the walker and the client, which have no such page.

---

<a id="s11-1"></a>
### 11.1 Beams (BIN pass (k); census OBS4)

Field names are those of `script-table.md` §5.3 without the `BeamScript_` prefix. Times are
seconds of the global clock (§3.1); lengths metres.

1. **Start (RT36).** The Beam node resolves its target (`script-table.md` §4.4) and takes the
   first entity. If there is none and `UseTargetPosition` = 0, no beam is made: the node
   instance exists, its ports never fire. Otherwise the client makes a beam with the node's
   fields, the performing entity as source and that entity (or none) as target. At that moment
   it creates the beam's particle systems from `BeamParticleSystem` and `HeadParticleSystem`
   (each when not 0) synchronously, not through the Particle node's start gating of
   `particles.md` §7.1.2, and loads the three materials; when `HeadEntity` is not 0 it creates
   that 0x47 entity as a world object, scaled by its own `Render_AdjustableScale` (1, 1, 1 when
   absent) times a vector that `HeadEntityScaleSource` selects from the source entity (1: a
   per-axis scale of the source; 3 and 4: two scalar scales of the source; other values:
   (1, 1, 1); which scales 1, 3 and 4 read is Q-RT11). The beam records its start time.
2. **Envelope (RT37).** With *r*ᵢ = `RampInTime`, *s* = `SustainTime`, *r*ₒ = `RampOutTime`,
   the beam keeps an elapsed time and an intensity *e*:
   - at the first update: done if already stopped; ramping in when *r*ᵢ > 0.0002, else
     sustaining;
   - ramping in: *e* = elapsed / *r*ᵢ; at *e* ≥ 1 sustaining; a stop now goes to ramping out
     with the ramp-out clock preset to (1 − *e*) × *r*ₒ (or to done, *e* = 0, when *r*ₒ ≤
     0.0002), so the fade continues from the current level;
   - sustaining: *e* = 1; a stop goes to ramping out (or done when *r*ₒ ≤ 0.0002); when
     `Persistent` = 0 and elapsed exceeds *r*ᵢ + *s*, likewise;
   - ramping out: *e* = 1 − (ramp-out clock) / *r*ₒ; done (*e* = 0) when the ramp-out clock
     exceeds *r*ₒ;
   - elapsed and, while ramping out, the ramp-out clock advance by the frame's time.
   The envelope is **finished** when done, or when `Persistent` = 0 and *r*ᵢ + *s* + *r*ₒ have
   passed since the start, or when `Persistent` = 1, the beam is stopped and *r*ₒ has passed
   since the stop. A Beam node instance that stops stops its beam (the instance's stop
   routine calls the beam's stop).
3. **Endpoints (RT38).** At creation, when `RandomStartRadius` > 0.0002, a fixed start offset
   is chosen: a direction from three uniform draws in [−1, 1] (normalised; a zero vector stays
   zero) times a uniform length in [0, `RandomStartRadius`]; the impact offset likewise from
   `RandomImpactRadius`. Each frame, with *t* = now − start (nothing happens while *t* < 0):
   - the **source point** is the source entity's attachment point for `SourceHoldingLocation`
     or `SourceBoneType` with `SourceBoneOffset` (as `particle-runtime.md` §6.1 attaches an
     effect; `SourceUseBoundingBoxCenter` selects the model's bounding-box centre) plus the
     start offset;
   - the **target point** likewise from the target entity with the `Target…` fields plus the
     impact offset; for a target without a skeleton, the bounding-box centre when either the
     holding location or the bone type is set; with no target entity, source point + the
     source frame's forward axis × `UntargetedBeamLength` (unreachable by step 1 except with
     `UseTargetPosition`); with `UseTargetPosition` = 1, the position `TargetPosition` plus
     the impact offset;
   - an end stays attached (recomputed every frame) until its detach time: the source detaches
     when `SourceDetachTime` > 0.0002 and *t* ≥ it, the target when `TargetDetachTime` > 0.0002
     and *t* ≥ it; with `DetachBeforeRampingOut` = 1 both detach once the envelope ramps out or
     is done. A detached end keeps its last point.
4. **Length.** *L* = |target point − source point|; when *L* ≤ 0.0002 the fractions below
   use *L* = 0.01. A beam longer than 1,000 m is neither moved nor drawn that frame.
5. **Head and tail (RT39).** Head fraction *h* = `HeadStartPos`; when *t* > `HeadAnimStartTime`,
   *h* = `HeadStartPos` + `HeadAnimSpeed` × (*t* − `HeadAnimStartTime`) / *L*; clamped to
   [0, 1]. The head **has arrived** once `HeadAnimSpeed` > 0.0002 and *h* > 0.9998 (a beam
   whose head speed is 0.0002 or less never arrives: of the 980 Beam nodes with a StoryHook
   under their port, 2 have head speed 0 and 2 have −25, `combat-visuals.md` §4.7). Tail fraction τ the same with `TailStartPos`,
   `TailAnimStartTime` and `TailAnimSpeed`. Head point = source point + (target − source) × *h*,
   tail point = source point + (target − source) × τ.
6. **Ribbon (RT40).** When τ ≤ *h* and the drawn segment (tail point to head point, length ℓ)
   is at least 0.1 m, the beam is drawn as a camera-facing ribbon of width `BeamWidth`, colour
   `ColorAndOpacity` with its alpha multiplied by *e* (each channel clamped to [0, 1]), texture
   repeated by `TextureUScale`: from the tail end, `NumTailSegments` segments with
   `TailMaterial`, then body segments with `BodyMaterial`, then `NumHeadSegments` segments
   with `HeadMaterial` (a count of 0 or a missing material leaves that part out). Each segment
   is *g* = min(`SegmentSize`, ℓ) long; when the head and tail segments together (*n* × *g*)
   are longer than ℓ, they shrink to ℓ / *n* each and there is no body; otherwise the body
   fills ℓ − *n* × *g* with whole segments and one shorter last one (none when the rest is at
   most 0.0002). The ribbon is built only when the beam has a `BodyMaterial`: without one,
   nothing of it is drawn, head and tail parts included (RT45; every Beam node of this install
   names all three materials). Its vertices and texture coordinates are given after step 10.
7. **Port (RT39).** In the first update after the head has arrived, the node instance fires
   the children of its Default ports (port type 6), once. Steps 3–6 run in the beam's draw
   step, which the client runs each frame for the beams of the active scene once both ends
   have been resolved; whether a beam outside the view still moves its head was not traced
   (Q-RT11). 947 Beam nodes have an Attack StoryHook there (`combat-visuals.md` §4.7).
8. **Particles (RT41).** Only while ℓ > 0.1 m, the envelope is not stopped and *e* > 0.0002:
   - **beam system** (when present, loaded, and `BeamSegmentParticlesPerSecond` *b* ≠ 0): let
     Δ = now − the last beam spawn time (first time: the creation time); if Δ > 1 s, the last
     spawn time becomes now − 0.1 s and Δ = 0.1 s. *n* = ⌊ℓ / `SegmentSize` × *b* × Δ⌋. For
     *k* = 1 … *n* the spawn time advances by Δ / *n* and one particle of the system's first
     emitter is spawned at that time at tail point + (head point − tail point) × *u*, *u* a
     uniform draw in [0, 1) of the shared stream (§3.3); the last spawn time keeps the last
     spawn's time. A spawn whose particle would already be dead at its time is skipped
     (`particles.md` §8.5);
   - **head system** (when present and `HeadParticlesPerSecond` *p* ≠ 0, and only when the beam
     system is present too, RT41): while the squared distance from the head point to the target
     point exceeds 0.1, let Δ = now − the last head spawn time (the same 1 s / 0.1 s catch-up);
     when ⌊*p* × Δ⌋ ≥ 1, one particle is spawned at the head point and the last head spawn time
     becomes now. At most one head particle per update;
   - both systems are placed each frame in the beam's frame with opacity
     `ColorAndOpacity` alpha × *e* (clamped), and simulate and draw as any system (§8, §9),
     except that the beam's systems skip the camera-distance fade and cut of `particles.md`
     §8.13 (the beam sets that flag on both at creation, RT46). The placement runs in the
     beam's draw step, every frame once the beam's materials have loaded, whatever the
     envelope's state; the beam never stops or restarts the systems' emission (RT46);
   - **precision (RT47):** clock times are double precision; each Δ above is the difference of
     two clock times rounded to single precision; ℓ / `SegmentSize`, then × *b*, then × Δ, and
     the floor are single-precision steps, with no tolerance; each beam spawn adds the
     single-precision quotient Δ / *n* to the double-precision spawn time, so the spawn times
     accumulate.
9. **Head entity (RT42).** When present: shown (`Render_Draw` true) while ℓ > 0.1 m, the
   envelope is not stopped and *e* > 0.0002, placed at the head point with its forward axis
   along the beam (source to target) and its up axis from world up (fallbacks when parallel);
   hidden otherwise.
10. **End (RT43).** A beam is removed when its envelope has finished and each of its two
    particle systems is absent, has a zero rate, or holds no live particle. The head entity
    goes with it.

**Ribbon vertices (RT45, BIN pass (l)).** For each frame in which step 6 draws the ribbon:

- **Side vector.** *a* = the unit direction from the source point to the target point; *c* =
  the unit direction from the camera position to the tail point; the side vector *s* = the
  unit vector of *a* × *c*, one for the whole beam in that frame (zero when the cross product,
  or the distance from the camera to the tail point, is 1e-6 or shorter: the ribbon then has
  no width).
- **Segment.** A segment from point *p*₀ to point *p*₁ along *a* is two triangles of six
  vertices (none shared) over the corners *p*₀ ± *s* × `BeamWidth` / 2 and *p*₁ ± *s* ×
  `BeamWidth` / 2: `BeamWidth` is the full width. Every vertex holds its position, the colour
  of step 6 (8 bits per channel after the clamp) and two texture-coordinate pairs.
- **Texture coordinates.** *v* = 1 at the corners on the + *s* side and 0 on the − *s* side, in
  both pairs. **Pair 1:** *u* = *U* × `TextureUScale`, where *U* starts at 0 at the tail end and
  runs along the whole ribbon without restarting: each tail segment adds 1 / `NumTailSegments`
  (the tail part adds 1 in total), each whole body segment adds 1, a shorter last body segment
  adds its length ÷ *g*, and each head segment adds 1 / `NumHeadSegments` (the head part adds
  1 in total). *U* counts segments, not metres. **Pair 2:** the same counting, restarted at 0
  at the start of each part and not multiplied by `TextureUScale` (tail 0 to 1, body 0 to its
  segment count, head 0 to 1).
- **Draws.** The parts are drawn as three draws in this order, each with its material when
  present (step 6): tail, body, head. Each frame, the beam's draw step first places its two
  particle systems (step 8) and then builds and draws the ribbon.

Which coordinate pair a ribbon material's texture reads, and the program that draws it, were not
traced.

> **Answered in part (2026-10-14, BIN pass (l), RT46; was OPEN, implementer, 2026-10-11):**
> whether a beam system's own emitters keep emitting after the envelope has finished. The
> original's beam places both systems in its draw step every frame until the beam is removed,
> whatever the envelope's state, and never stops or restarts their emission; "a zero rate" in
> step 10 is the node's `BeamSegmentParticlesPerSecond` (beam system) or
> `HeadParticlesPerSecond` (head system), as the walker reads it. Whether a beam system's own
> emitters emit besides the beam's spawns of step 8 was not traced. **Decision (Tessera):** the
> walker's reading stands: both systems are placed every frame until the beam is removed (at
> their last frame when the beam has no span), and their own emission is stopped as §7.2 step 3
> once the envelope has finished, so that step 10 is always reached. New (RT46): the beam's
> systems skip the camera-distance fade and cut of `particles.md` §8.13.

> **Answered (2026-10-14, BIN pass (l), RT45–RT47; was OPEN, implementer, 2026-10-11):** how
> the ribbon and the head entity are drawn. Item by item:
> (a) **Ribbon geometry.** Answered by the client (RT45, above), which differs from the
> walker's reading in three points: one side vector per beam from the camera-to-tail-point
> direction (not one per segment from its middle); pair-1 *u* runs along the whole ribbon and
> counts segments (the tail and head parts count 1 each), it does not restart per segment and
> is not scaled by metres; a camera on the beam's line gives no width. **Decision (Tessera):**
> the ribbon's texture is sampled with pair 1.
> (b) **Ribbon program.** Not traced beyond the vertex format (position, colour, two
> coordinate pairs; RT45). **Decision (Tessera):** the walker's reading stands: a ribbon
> material's surface record (0x31) is drawn through the particle pipeline as a particle
> surface (§9.1.1: the table's row, otherwise the rule-10 fallback of its blend class), with
> the step-6 colour as the vertex colour and a white tint; for a shader record that §9.1.1 does
> not list, a second colour layer whose alpha reaches the output (`ColorAndOpacity2`,
> `material.md` §10.3 MA54) is kept, RGB combined as `material.md` §10.3 states for the record,
> alphas multiplied.
> (c) **Order.** Within one beam, the client places the systems first and then draws the
> ribbon's tail, body and head (RT45). **Decision (Tessera):** ribbons are drawn in the particle
> stage before the systems; beams in creation order.
> (d) **Frames.** The client places both systems with one frame of the beam (RT46); its
> orientation was not traced. **Decision (Tessera):** the walker's reading stands: forward (+Y)
> from the source point to the target point, up (+Z) from world up, and +X along world +X when
> the beam is vertical; fixed at the tail point for the systems and at the head point for the
> head entity.
> (e) **Head entity.** Not traced beyond step 9. **Decision (Tessera):** the walker's reading
> stands: a static object of its 0x47 descriptor (its setup parts at the frame of (d), not
> animated), scaled by its `Render_AdjustableScale` along its chain (usable range of
> `entity-descriptor.md` §7, otherwise (1, 1, 1)) times the vector of Q-RT11, tinted by its
> chain's `Render_Color` and `Render_Opacity` (`material.md` §8.3), and left out when its draw
> flags suppress it (`entity-descriptor.md` §9).
> (f) **Spawn times.** Answered by the client (RT47): the times accumulate (each spawn adds the
> single-precision Δ / *n*), which differs from the walker's reading (spawn *i* at the last
> spawn time + Δ × *i* / *n*). Tessera follows the client.

> **Answered (2026-10-14, BIN pass (l), RT47; was OPEN, implementer, 2026-10-11):** the
> precision of step 8. Δ is the difference of two double-precision clock times rounded to
> single precision; the product ℓ / `SegmentSize` × *b* × Δ and its floor are single-precision
> steps with no tolerance. In RT-I18, updates 0.05 s apart on a double-precision clock give a
> Δ that rounds to the single-precision value of 0.05, and 6 × 10 × that value rounds to
> exactly 3 in single precision, so each update spawns 3; the test needs no 0.050000001 s
> spacing once the code computes in single precision.

> **OPEN (implementer, 2026-10-16):** RT-I18's "3 every time" does not follow from step 8 as
> written. The first update's three spawns each add the single-precision 0.05 / 3
> (0.0166666675), so the last spawn time ends at about 0.0500000026 s, not 0.05. The next Δ
> (0.1 − that) rounds to 0.049999997 in single precision, 60 × Δ rounds to 2.9999998 and the
> floor gives 2. The counts then alternate 3, 2, 3, 2. Which is the client's behaviour: does
> the last spawn time accumulate as step 8 says (giving 3, 2), or does something else (another
> rounding, a time kept apart from the accumulated spawn times) keep the count at 3? Tessera
> follows the step-8 text: accumulated spawn times and single-precision Δ, count and floor.
> The RT-I18 unit test asserts 3 for the first update and 3, 2, 3, 2 after it.

> **OPEN (implementer, 2026-10-16; engineering choices where §11.1 is silent):** (1) a beam
> whose `BodyMaterial` record has not loaded yet draws no ribbon that frame (the client draws
> once the materials have loaded, step 8); a body record that failed or is not drawable leaves
> only the body part out. (2) The beam's two systems are always new systems of their own, never
> a running shared copy of `particles.md` §8.16, and are never shared with other requests, so
> that the fade switch of step 8 stays with the beam. (3) A negative `BeamWidth` is drawn as
> written (corners *p* ± *s* × `BeamWidth` / 2, *v* = 1 at the "+" corner); a zero width draws
> nothing. (4) The tail and head counts in the pair-1 sums are the counts after the segment
> cap of rule 14.

Census of the 2,740 Beam nodes (OBS4, RT44): `Persistent` 1 in 769; `HeadEntity` in 1,050;
`BeamParticleSystem` in 571, `HeadParticleSystem` in 45 (2 of them without a beam system, so
their head particles never spawn); `BeamSegmentParticlesPerSecond` 10 in 2,092, 30 in 292, 0 in
118; `HeadParticlesPerSecond` 10 in 2,556, 60 in 130; (ramp in, sustain, ramp out) (0, 1, 0.25)
in 947, (0, 1, 0) in 426, (0, 0.35, 0.25) in 234, (0, 10, 0) in 123; (`HeadStartPos`,
`HeadAnimStartTime`, `HeadAnimSpeed`) (1, 0, 10) in 410, (0, 0, 40) in 328, (0, 0, 50) in 296,
(0, 0.01, 45) in 228, (1, 0, 0) in 220; `DetachBeforeRampingOut` 1 in 2,413;
`HeadEntityScaleSource` 2 in 2,557, 3 in 156, 1 in 27; `UseTargetPosition` 1 in 1; non-zero
`RandomImpactRadius` in 168, `RandomStartRadius` in 54. Port children: none 1,726, one 835, two 179.

<a id="s11-2"></a>
### 11.2 Weather and water systems (BIN pass (n); RT49–RT53, RT55)

The ten UNIQUEDB systems of `particles.md` §7.1.3 are created once, synchronously, when the
weather code starts (§5.1) and kept. They form three groups (UNIQUEDB keys; the DIDs are read at
run time, `particles.md` §7.1.3):

| Group | Systems (UNIQUEDB key, label) |
|---|---|
| Rain | 0x7B `CameraRainTorrentParticleSystem`, 0x2A `CameraRainImpactParticleSystem`, 0x6C `RainVaporParticleSystem`, 0x83 `RainGustParticleSystem` |
| Snow | 0xFD `CameraInBlizzardParticleSystem`, 0x100 `CameraSnowImpactParticleSystem`, 0xFE `SnowGustParticleSystem` |
| Water | 0x4A `WaterRipplesParticleSystem`, 0xA1 `WaterBubblesParticleSystem`, 0x1E7 `CoinSwimRipplesParticleSystem` |

The rain streaks and snow flakes themselves are **not** particle systems: the weather code
simulates and draws them as its own geometry. §11.2.6 states that simulation for rain, and for
snow its count, parameters and field (the snow flakes' own motion and geometry stay Q-RT13);
below, a "drop" is one rain streak or snow flake of it.

"One particle of emitter *k*" means: the client's single-particle spawn of `particles.md` §8.5
on the *k*-th emitter (counted from 1) of that system, at the current clock time, at the given
point, with no added velocity (the same spawn §11.1 step 8 uses).

<a id="s11-2-1"></a>
#### 11.2.1 The global wind (RT49)

1. The environment state of `environment.md` §11.7 carries a **wind vector**
   *w* = (*c*₃, *c*₄, 0), where *c*₃ and *c*₄ are the values of keyframe curves 3 and 4
   (`environment.md` §5.3 field 17; the "two further values from curves 3 and 4" of §11.7
   step 6). It is in the world frame, in the unit of the gravity vector of `particles.md` §8.8
   (the axes of the two components are Inferred from the zero third component: the two
   horizontal axes of `coordinates.md`).
2. Under the forced weather of `environment.md` §11.7 step 7, *w* = (0, 0, 0).
3. In every update of the world view, after the client has chosen the environment state in use
   (outdoor or indoor, `environment.md` §11.6) and applied its cross-fade, it sets the **target
   of the global wind** of `particles.md` §8.8 to that state's *w*. Setting the same target
   again restarts the ease from the current eased value, which leaves the exponential curve of
   `particles.md` §8.8 unchanged, so in effect the wind eases towards *w* with the 0.3 s
   half-life.
4. When the weather code starts, the global wind's value and target are (0, 0, 0).

The wind vector, like the rain and snow values, is part of the state that both fades of
`environment.md` §11.6 blend (RT64): during a sky-key cross-fade or a sun-change fade the target
of step 3 is the blended vector.

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):**
> `environment.md` §11.6 step 4 and §11.2.1 step 3: are rain, snow and the wind curves
> *c*₃/*c*₄ blended with the cross-fade weight and the sun-change fade like every other number
> of the state? Tessera now uses the new day description's values alone during both fades.
>
> **Answered (spec writer, 2026-10-23; RT64, `environment.md` BIN-E4 and E43):** yes, all four
> are blended linearly by the fade's weight in both fades; the walker must take the blended
> values for the precipitation of §11.2.2 and for the wind target of step 3. The answer is also
> recorded in `environment.md` §11.6.

<a id="s11-2-2"></a>
#### 11.2.2 When the weather work runs (RT50)

Each update of the world view (Inferred: once per frame, with the frame's camera):

1. While a state flag of the client that gates the whole step is clear (not traced; Inferred:
   the world view is not active), none of the following happens.
2. While the **landscape is visible** from the viewer (`environment.md` §11.6), the
   precipitation step runs: the rain update (§11.2.3) with the rain value *r*, then the snow
   update with the snow value *s*, then the placement of the rain and snow systems (§11.2.4).
   *r* ∈ [0, 1] and *s* ∈ [0, 2] are the precipitation values of the state in use
   (`environment.md` §11.7 steps 6–7).
3. Then, for the player's figure, the effects of §11.2.5 (under a condition of the figure's
   state that was not traced).
4. Then the three water systems are placed, whatever the weather (§11.2.4).

While the `[Render]` option `PrecipitationEffects` is off, neither the rain nor the snow update
has any effect: no fall direction, exposure test, force recomputation or spawn of §11.2.3 takes
place (`particles.md` §8.8.1). Its built-in initial value is on (RT55).

With the option on, each of the two updates still does nothing in an update in which its drop set
uses no drop: *n* = ⌊3,000 × clamp(value, 0, 1) × *q*⌋ = 0, with *q* from `AtmosphericsDetail`
(§11.2.6 step 1, RT58). It then makes no exposure test, recomputes no force, spawns nothing and
reports neither position as exposed; the placement of §11.2.4 still runs. At the built-in
`Medium` (*q* = 0.6) this happens for rain or snow values below 1 / 1,800.

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):**
> §11.2.2 / `particles.md` §8.8.1: a precipitation update runs only when it would draw at
> least one drop, and that drop count needs the drop simulation (Q-RT13). Tessera runs both
> updates on every step while `PrecipitationEffects` is on and the landscape is visible. Confirm
> or give the gate.
>
> **Answered (spec writer, 2026-10-23; BIN pass (o), RT58):** the gate is the count rule of
> §11.2.6 step 1, paragraph above; it needs only the value and `AtmosphericsDetail`, not the
> simulation. Running both updates on every step is not the client's behaviour when *n* = 0:
> then the update's exposure test, force recomputation and spawns are skipped and its exposure
> reports false.

<a id="s11-2-3"></a>
#### 11.2.3 The rain and snow updates (RT51)

Each update, rain with *r* and snow with *s*:

1. **Fall direction** *f*: the unit vector of (eased global wind + (0, 0, −9.8)), or (0, 0, −1)
   when that sum is 0.000001 long or shorter (`particles.md` §14 for rain; the snow update
   computes it the same way).
2. **Sky exposure**, when more than 0.5 s of the global clock have passed since the last test:
   from the camera's position and from the player's position, a collision test of the world
   along −*f* over 100 m. A position is **exposed** when the test finds nothing in the way; a
   position the client cannot place is not exposed. Each update keeps its own two results until
   its next test.
3. **Forces.** The forces of the rain gust system (key 0x83) are recomputed in every rain
   update; those of the snow gust system (key 0xFE) in a snow update only while *s* > 1.001
   (`particles.md` §8.8.1).
4. **Spawns from the drops.** In each update, for every drop whose remaining fall is at most
   1.7 (rain) or 0.08 (snow) in the drop simulation's length unit (Inferred: metres), at the
   drop's lower end point:
   - rain: one particle of emitter 2 of the vapour system (key 0x6C);
   - rain, when the drop **ended on the reference surface** (its end lay at most 0.1 m above
     the reference height, or below it, and was moved onto it; the reference height is the
     water surface found by the water probe of §5.7 at the camera's position, §11.2.6 steps 3
     and 5) and lies within 15 m horizontally of the camera: one particle of emitter 2 of the ripple system (key 0x4A), 0.1 m above the end
     point;
   - when the drop did not end on the reference surface: one particle of emitter 1 of the gust
     system (rain: key 0x83, in every rain update; snow: key 0xFE, only while *s* > 1.001) with
     probability *n* / 3000 × 0.015, *n* the number of drops of that update and 3000 its
     maximum. For rain, *n* is the current count. For snow, the ratio *n* / 3000 is the one at
     the first time in the client's process that a snow drop is tested for a gust spawn, and
     it does not change afterwards for the rest of the process: not on a world entry, a region
     change or a return to the character screens (RT66, revised 2026-10-23).

Snow drops spawn no vapour and no ripple particles: snow spawns only into the snow gust system,
and only while *s* > 1.001. Rain spawns into the vapour, ripple and rain gust systems.

The remaining fall of step 4 is *h* of §11.2.6 step 4, in metres (BIN-O).

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):** step 2:
> Tessera choices for the exposure test: the ray sees every collision body (`QueryFilter::all`),
> terrain only as far as the collision sets hold it, and a position whose landblock has no
> installed collision set counts as "cannot place", so not exposed.
>
> **Answered (spec writer, 2026-10-23):** the client's test is a swept collision query of its
> world from the position along −*f* over 100 m (BIN-O); which bodies its filter accepts was
> not traced (Q-RT13). The three choices are confirmed as **Tessera rules**, and the same query
> serves the drop placement of §11.2.6 step 3, where "cannot place" leaves the drop to be tried
> again in the next update.

> **OPEN (implementer, world-r6, 2026-10-23; proposed, filed by the spec writer):** step 4:
> Tessera fixes the snow gust ratio *n* / 3,000 at the first snow gust evaluation of the
> effects layer's life and does not reset it on a region change. Is the client's "session" the
> process, the world entry or the region?
>
> **Answered (spec writer, 2026-10-23; BIN-P, RT66):** the process. The client fixes the ratio
> once per run of the client, the first time a snow drop is tested for a gust spawn (a drop in
> use, not ending on the reference surface, with its remaining fall at most ℓ, while *s* > 1.001
> so that snow gust spawns can happen), and never resets it. **Tessera rule:** one value per process run of
> the client; an effects layer that is rebuilt (a new world entry, the character screens and
> back) keeps the value already fixed. Tessera's current reading, fixed for the effects layer's
> life, conforms only while that layer lives as long as the process.

<a id="s11-2-4"></a>
#### 11.2.4 Placing the systems (RT52)

After the two updates, in the same step:

1. **Branch.** When *r* is greater than *s*, the rain branch; otherwise (*r* ≤ *s*) the snow
   branch. Only the systems of one branch are placed in an update; the others are not placed
   and so not drawn (`particles.md` §8.13 not-drawn state).
2. **Camera exposure** *x*: true when the camera was exposed by the rain update's test or by the
   snow update's test (§11.2.3 step 2). **Inside**: the camera's position lies in an interior
   (a flag the world view sets each update from the camera's cell; Inferred meaning).
3. **Rain branch**, with the strength *g* = 2 × max(0, *r* − 0.5):
   - when *r* > 0.05: the gust system (0x83) and the vapour system (0x6C) are placed;
   - when *x* holds, the camera is not inside and *g* > 0.001: the torrent system (0x7B) and the
     impact system (0x2A) are placed at the camera (its position and orientation).
4. **Snow branch**, with *g* = max(0, *s* − 1):
   - when *g* > 0.001: the gust system (0xFE) is placed;
   - when also *x* holds and the camera is not inside: the blizzard system (0xFD) and the impact
     system (0x100) are placed at the camera.
5. **Impact spawns.** In the update in which the branch's impact system is placed: with *d* =
   −(*f* · *a*), *f* the branch's fall direction and *a* the camera's local +y axis in world
   space (Inferred: its forward axis); when *d* ≥ 0.1, an
   integer *k* is drawn uniformly from −8 to ⌊(*d* / 0.9)² × 10 × *g*⌋ inclusive, and when
   *k* ≥ 0 one particle of emitter 1 of the impact system is spawned at the system's own origin.
   So a camera looking up into the precipitation gets drops on the lens more often.
6. **Placement frames.** The camera systems are placed with the camera's frame. The gust and
   vapour systems, and the three water systems (§11.2.2 step 4), are placed at a reference
   point that the world view keeps, with the identity rotation; every spawn position of
   §11.2.3, §11.2.5 and §11.2.6 is handed to them relative to that point, so the point does not
   change where particles appear (BIN-O, RT62; what the point is was not traced). Of the ten
   records, eight are
   world-space in this install (`particles.md` §4.1 `world_space` = 1): the gust, vapour and
   three water systems, and also the torrent and blizzard systems; only the two impact systems
   (0x2A, 0x100) are local-space (OBS5, checked for all ten keys on 2026-10-23; data, read at
   run time). The flag matters for the camera systems: by the `world_space` rule of
   `particles.md` §4.1, torrent and blizzard particles keep the world position at which they
   were spawned while the camera moves on, whereas impact particles are drawn through the
   camera's frame and so stay on the lens (Inferred from that rule).

The systems emit by their own timed emission as well: in this install the torrent and blizzard
records have emit intervals of 0.025 s and 0.029 s / 0.005 s, the others 0 or 10,000 s and more,
so those receive their particles from the spawns of §11.2.3–§11.2.5 only (OBS5; data, read at run
time).

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):** step 6:
> the "reference frame of the weather code" was not traced. Tessera choice: the gust, vapour
> and three water systems are placed at the camera position with no rotation (all five are
> world-space, and none receives particles under rule 17).
>
> **Answered (spec writer, 2026-10-23; RT62):** the rotation is the client's (identity), and
> since the client hands every spawn position over relative to the reference point, the
> point's identity does not show for these world-space systems. The camera position is
> confirmed as a **Tessera rule**, with spawn positions converted to it, so that each particle
> appears at the world point §11.2.3, §11.2.5 and §11.2.6 give. Rule 17 now gives them particles.

<a id="s11-2-5"></a>
#### 11.2.5 The player's figure in rain and water (RT53)

Each update (§11.2.2 step 3), for the player's figure, for each bone of its skeleton that the
figure lets take part in these effects (only some bones take part, BIN-O; which bones, and what decides it, was not traced, Q-RT13) and
that has a parent bone, with the bone segment from the parent to the bone, scaled by the figure's
scale, in world space:

- **Rain on the figure** (only while the landscape is visible, the player's position was exposed
  by the rain update's test and *r* > 0.2): with probability *r* × 0.3, and when the segment's
  direction differs enough from the fall direction (1 − |direction · *f*| > 0.65, both in the
  figure's frame), one particle of emitter 1 of the vapour system at a uniform random point of the
  segment raised by 0.075 m, unless the figure stands in water and that point lies at or below
  the water surface.
- **Bubbles** (while the figure is in water, that is the water probe of §5.7 cast from the
  figure's position finds a water surface, RT67; `Volume_IsLava` and `Volume_IsCoin` both
  unset): with
  probability 0.15, one particle of emitter 1 of the bubble system (0xA1) at a uniform random
  point of the segment plus a uniform offset in [−0.1, 0.1] m on each axis, only when that point
  lies more than 0.4 m below the water surface.
- **Ripples** (while the figure is in water and `Volume_IsLava` is unset): with probability 0.25,
  when the segment crosses the water surface (one end above it, the other below), one particle
  of emitter 1 of the ripple system (0x4A), or of the coin-swim ripple system (0x1E7) while
  `Volume_IsCoin` is set, at the segment's horizontal midpoint, 0.05 m above the water surface.

"The water surface" in the three items is the height of the probe's hit (the highest hit,
§5.7); without a hit the figure is not in water, and a rain point is then never suppressed.
`Volume_IsLava` (PID 0x3F6) and `Volume_IsCoin` (PID 0xB6E) are read from the figure's own
properties, each set only when its value is a Boolean and true (`water.md` W18; RT67); they are
the flags of the volume sets of `water.md` §7.2 (Inferred: the values a volume's monitor gives
the figure while it is inside, `water.md` §7, `sound.md` §10.8). The probe and the flags are
independent: a probe hit with neither flag set counts as water. The dictionary labels are
interoperability identifiers. (Revised 2026-10-23.)

> **OPEN (implementer, world-r6, 2026-10-23; proposed, filed by the spec writer):** §11.2.5 and
> rule 17: Tessera takes the figure's water surface and `Volume_IsLava` from the walker's own
> water state at the avatar's feet (the liquid boxes and water plane of the collision view, none
> inside interior cells), not from the water probe of §5.7. `Volume_IsCoin` has no source in the
> walker and counts as unset. Confirm that "a water surface is found at its position" means the
> volume state, and name the source of `Volume_IsCoin`.
>
> **Answered (spec writer, 2026-10-23; BIN-P, RT67):** not the volume state. In the client
> "in water" and the surface height come from the water probe of §5.7 cast from the figure's
> position (a vertical segment against water surfaces only, from 40 m above to 1 m below when
> the position is under a water surface of its cell, else down 10 m; the highest hit), the same
> probe the walker already casts for `OnWater` effects and for the reference surface of §11.2.6
> step 5. The two flags are the figure's own property values. **Tessera rules:** (1) the walker
> casts that probe from the avatar's position for §11.2.5; its own water state at the feet is
> not a substitute (it misses a surface below the feet, for example under a jetty, which the
> probe finds within 10 m). (2) `Volume_IsLava` and `Volume_IsCoin` are true while the
> avatar's feet are inside a liquid box whose property set (`water.md` §7.2) carries the flag
> as a true Boolean; the coin flag therefore comes from the boxes of the three coin sets
> (`water.md` §7.2–§7.3), which the collision view must keep with each box. Until it does,
> `Volume_IsCoin` unset is a build gap, not a rule: the walker then draws the ordinary ripple
> system where the client draws the coin-swim one.

<a id="s11-2-6"></a>
#### 11.2.6 The drops of the weather code (BIN pass (o); RT58–RT62)

The weather code keeps two **drop sets**, one for rain and one for snow, each with room for
*N*max drops and its own parameter set. The values below are the client's built-in parameter
sets (BIN-O; constants of the client, `docs/decisions/0012-numeric-constants.md`); the two
materials are read from UNIQUEDB at run time.

| Parameter | Rain | Snow | Meaning |
|---|---|---|---|
| *N*max | 3,000 | 3,000 | most drops in use |
| *V* | 25 m/s | 1.75 m/s | fall speed |
| *H* | 30 m | 30 m | fall height above a drop's lower end point |
| *L* | 100 m | 100 m | length of the collision segments (the exposure test of §11.2.3 step 2 and step 3 below) |
| *R* | 30 m | 30 m | radius of the drop field around its origin |
| recentre distance | 2.5 m | 2.5 m | horizontal camera move that moves the field origin (step 2) |
| life | 10 to 30 s | 10 to 30 s | uniform draw; a drop is placed anew once its life has passed (step 4) |
| *j* | 0.4 | 0.4 | relative jitter of a drop's horizontal fall components (step 4) |
| *a* | 0.5 m | 2 m | horizontal jitter of a drop's lower end point (step 4) |
| ℓ | 1.7 m | 0.08 m | streak length; the "remaining fall" threshold of §11.2.3 step 4 |
| *w* | 0.04 m | 0.04 m | half width of a streak (step 7) |
| head colour (RGBA) | (0.83, 0.85, 0.9, 0) | (1, 1, 1, 1) | step 7 |
| tail colour (RGBA) | (0.83, 0.85, 0.9, 0.3) | (1, 1, 1, 1) | step 7 |
| material | UNIQUEDB key 0x2B `RainStripMaterial` | UNIQUEDB key 0x3B `Render_SnowMaterial` | surface records (0x31000007 and 0x31000001 in this install, OBS6) |

The UNIQUEDB labels are interoperability identifiers. Steps 1 and 2 hold for both sets; steps
3–7 are stated for **rain** (the rain update was read in full). The snow update uses its own
parameter set, the count rule of step 1, the field origin of step 2, the threshold ℓ and the
spawn rules of §11.2.3; how it places, moves and draws its flakes (one snow drop is drawn as
more than one flake, at places apart from each other around the drop) was not reduced to words
(Q-RT13).

1. **Count.** In each update the set uses the first *n* = ⌊*N*max × clamp(*v*, 0, 1) × *q*⌋
   drops, with *v* the rain value *r* (snow: the snow value *s*, clamped here to [0, 1]) and
   *q* from the `[Render]` option `AtmosphericsDetail` (levels 0 `Low`, 1 `Medium`, 2 `High`;
   built-in `Medium`): 0.3 at level 0, 0.6 at level 1, 1 at any other level. When *n* = 0 the
   update does nothing else: no exposure test, no force recomputation, no drop and no spawn of
   §11.2.3, and it reports neither the camera nor the player as exposed, so §11.2.4 places no
   camera system of that branch. Drops beyond the first *n* keep their state and take part
   again when *n* grows. This is the gate of `particles.md` §8.8.1 ("at least one drop").

   > **OPEN (implementer, walker-wiring-r7, 2026-10-23; proposed Decision, also `walker.md`
   > W26):** `[Render] AtmosphericsDetail` is read as a level name (`Low`, `Medium`, `High`; any
   > case) or as a level number 0 or above (any level other than 0 and 1 gives *q* = 1). Any
   > other present value counts as the built-in `Medium`; a missing key keeps the current level.
   > Confirm that numbers are accepted.
   >
   > **Answered (spec writer, 2026-10-23, round 8):** confirmed as a **Tessera rule**
   > (`walker.md` §11.2, W26 revised); the client's parsing of the stored value was not traced.
   > The owner's file stores the level name (an observation, read at run time).

2. **Field origin.** Drops are kept relative to a field origin *O*. In each update (with
   *n* ≥ 1), when the camera's position lies more than 2.5 m horizontally from *O*, *O* moves to
   the camera's horizontal position. Every drop in use that is not marked for placement keeps
   its world position (its points are moved by the opposite of the move), and such a drop whose
   **placement point** *P* (step 3) is now farther than *R* horizontally from *O* is marked for
   placement anew; the test is made at *P*, not at the end point or the lower end point, which
   differ from *P* horizontally under wind and by the jitter of step 4. A drop already marked,
   and every drop beyond the first *n*, keeps its position relative to *O* instead, so it moves
   with the field. Heights are not changed by the move (RT65, revised 2026-10-23). The client's
   first *O* was not read (Q-RT13; Tessera: the callout below).
3. **Placing a drop** (each drop marked for placement, in its turn in the update): a horizontal
   direction from two draws uniform in [−1, 1], made unit length (zero when 0.000001 long or
   shorter), and a distance (1 − *u*₁ · *u*₂) × *R* with *u*₁, *u*₂ uniform in [0, 1], give the
   point *P* = *O* + direction × distance. From the point at *P*'s horizontal position, at the
   camera's height, moved by *L* / 2 along the fall direction *f* of §11.2.3 step 1, a
   collision segment of length *L* is tested against the world along −*f* (the test of §11.2.3
   step 2). The drop's **end point** *E* is the contact the test reports (Inferred: the first
   one from the segment's start), or the segment's start when nothing is hit. When a reference
   surface exists (step 5) and *E* lies no more than 0.1 m above the reference height (or below
   it), *E* takes the reference height and the drop is marked as **ending on the reference
   surface**. The drop's lower end point becomes *E* (no jitter yet), its fall fraction *t* a
   draw uniform in [0, 1], its expiry the clock time plus a draw uniform in [10, 30] s, and it
   is in use. When the collision test cannot run (the client cannot place the point), the drop
   stays marked and is tried again in the next update; until a placement succeeds it keeps its
   previous state and takes part in steps 4, 6 and 7 like any drop in use: it falls, wraps,
   spawns and is drawn where it was (RT65; every drop in use gives one quad in every update).
   A drop keeps the fall direction it had; one placed for the first time has none (the zero
   vector) until its first wrap (step 4), so it is drawn with no length.
4. **Falling.** A drop's **remaining fall** is *h* = *t* × *H* above its lower end point. In its
   turn, when *t* ≤ 0, the drop wraps: *t* becomes *t* + 1 − trunc(*t*); its fall direction *d*
   becomes the unit vector of (*v*ₓ + *e*₁, *v*ᵧ + *e*₂, *v*_z), where *v* = eased global wind +
   (0, 0, −9.8) is the sum of §11.2.3 step 1 before it is made unit length and *e*₁, *e*₂ are
   uniform in [−*j*·*v*ₓ, *j*·*v*ₓ] and [−*j*·*v*ᵧ, *j*·*v*ᵧ]; its lower end point becomes
   *E* + (uniform in [−*a*, *a*], uniform in [−*a*, *a*], 0); and when its expiry has passed it is
   marked for placement anew (it is still drawn in this update). After the spawns and the
   drawing of steps 6–7, *t* decreases by Δ*t* × *V* / *H*, Δ*t* being the clock time since the
   set's previous update with *n* ≥ 1. So a rain drop falls its 30 m in 1.2 s. (The snow
   update scales its decrease by a further factor that was not reduced to words, Q-RT13.)
5. **Reference surface.** Once per update the water probe of §5.7 is cast at the camera's
   position; the height of the water surface it finds (the highest hit) is the reference height
   of steps 3 and §11.2.3 step 4. Without a hit there is no reference surface.
6. **Spawns.** The spawns of §11.2.3 step 4 use *h* (metres) and the drop's lower end point;
   their positions, and those of §11.2.5, are handed to the systems relative to the reference
   point of §11.2.4 step 6.
7. **Streak** (one quad per drop in use, every update): head = lower end point − *d* × *h*;
   tail = head + *d* × min(*h*, ℓ). The four vertices are head − *w*·*c*, tail − *w*·*c*,
   tail + *w*·*c* and head + *w*·*c*, *c* the unit world-space vector of the camera's
   horizontal screen direction (Inferred: pointing to the screen's right), so each streak is
   widened across the screen, with texture coordinates (0, 0), (0, 1), (1, 1), (1, 0) and
   vertex colours head, tail, tail, head colour of the table, each multiplied channel by channel
   by the environment state's `colour_4` (`environment.md` E43; Probable). So a rain streak is
   transparent at its upper end and reaches alpha 0.3 × the colour's alpha at its lower end,
   and shrinks to nothing as it reaches its end point.

Where in the frame the drop quads are drawn, and with which blend, follows the material; the
client's draw call was not read (Tessera: rule 17).

> **OPEN (implementer, world-r6, 2026-10-23; proposed, filed by the spec writer):** step 3:
> Tessera places at most 512 drops per update (`MAX_PLACEMENTS_PER_UPDATE`). Drops beyond the
> cap stay marked, the same as a drop whose collision test cannot run. A drop that is marked
> but not placed in an update is neither drawn nor spawns in it, including a drop re-marked
> after its expiry once its last wrap is past. Does the client keep drawing such a drop until
> it is placed again?
>
> **Answered (spec writer, 2026-10-23; BIN-P, RT65):** yes. The client places every marked
> drop in its turn in the same update (it has no cap), and a drop whose test cannot run keeps
> its previous state: it still falls, wraps, spawns by §11.2.3 step 4 and is drawn, every
> update, until a placement succeeds (step 3 now says so). **Tessera rule:** the cap of 512
> placements per update is accepted as an engineering bound (it spreads the first fill of up to
> 3,000 drops over at most six updates); a drop left marked by the cap is treated as the client
> treats a drop whose test cannot run, so it keeps its previous state and is drawn and spawns.
> Hiding it does not conform. A drop never placed yet has the zero fall direction and so draws
> nothing either way.

> **OPEN (implementer, world-r6, 2026-10-23; proposed, filed by the spec writer):** step 2:
> the spec does not give the first field origin. Tessera sets it to the camera's horizontal
> position at the first update with *n* ≥ 1. The "farther than *R*" test after a move uses the
> drop's lower end point (*E* plus its jitter), not *E*. Confirm or give the client's reading.
>
> **Answered (spec writer, 2026-10-23; BIN-P, RT65):** (1) the client's first origin and the
> initial state of its drops were not read (Q-RT13). **Tessera rule (confirmed):** *O* starts at
> the camera's horizontal position at the first update with *n* ≥ 1, and every drop starts
> marked for placement with the zero fall direction. Since *O* follows the camera after 2.5 m,
> any other start differs only in the first fill. (2) **replaced:** the client tests the drop's
> placement point *P* of step 3, which the move shifts with the drop; not *E* and not the lower
> end point (step 2 now says so). Only the drops in use that are not marked are shifted and
> tested; a marked drop and the drops beyond the first *n* keep their position relative to *O*.

> **OPEN (implementer, world-r6, 2026-10-23; proposed, filed by the spec writer):** step 7 and
> rule 17: the drop surfaces (keys 0x2B and 0x3B) are drawn through the particle programs with
> the same surface reading as beam ribbons (Decision (b) of §11.1). In the translucent stage they
> come after the beam ribbons and before the particle systems, in the caller's order. A drop
> whose fall direction is still zero gives no quad. Confirm the order, or state where the client
> draws them relative to its particle systems.
>
> **Answered (spec writer, 2026-10-23; Tessera rule, no client fact):** the client's draw of the
> drop quads was not read (Q-RT13), so its place relative to the particle systems is not known.
> **Tessera rule (confirmed):** the drop quads are drawn through the particle programs with the
> beam-ribbon surface reading, in the translucent stage of the world pass after the beam ribbons
> and before the particle systems, rain set before snow set, each set in drop order. Skipping
> the quad of a drop whose fall direction is zero is allowed: the client's quad for it has zero
> length and covers no pixel.

<a id="s11-3"></a>
### 11.3 The effect preview of the options window (BIN pass (n); RT54; answers Q-RT5)

The fourth entry point of §5.1 belongs to the **effect-filter page** of the client's options
window (the page whose layout elements are labelled `FilterParticles…` in the install's UI
layout data). The page records recently seen effects into a list and keeps two block lists, one
for the character and one for the account. When the player activates an entry of the page that
carries an effect DID in its `UI_Shortcut_Data_ID` value (an integer value; the label is an
interoperability identifier), the client places that effect **once**, unattached, at the local
player's position with the player's orientation, through the same scene placement as a dropped
effect (§5.6): a preview. The start gating of `particles.md` §7.1.2 is not applied.

How the block lists reach the start of an effect (whether a blocked DID is refused at §5.4, and
where the lists are kept) was not traced: Q-RT12.

**Tessera (rule 15 does not depend on it):** an effect-filter page is not part of any milestone
yet; if Tessera builds one, its preview follows the paragraph above.

<a id="s12"></a>
## 12. Tessera rules (Decision)

1. **Random source.** Implement the generator of §3.3 as one shared stream for particle code and
   mode-3 curves, seeded from the wall-clock seconds at start-up. Exact sequences cannot match
   the original (the stream is shared with unrelated code, `particles.md` §10.3), so tests use a
   fixed seed and check distributions, not values. A test or tool may give a system its own
   seeded stream.
2. **Stored classification.** Use the node's stored byte (§5.3); compute the rule only for
   nodes without one (synthetic data) or when writing. Do not "repair" stored bytes.
3. **Finite effects started at low intensity** are skipped (§5.4 step 3); intensity does not
   scale effects.
4. **Dropped effects** follow §5.6 exactly, including the doubled offset and the absence of
   start gating.
5. **Lingering:** follow §7.2–§7.4, including the 6-s cut and the 2-s housekeeping; the time
   points are taken from the global clock.
6. **Hostile classification input:** a 0x39 record with `burst_count` = 0 in an emitter that
   has `emission_limit` ≠ 0 would make the client divide by zero in §5.3; Tessera treats that
   emitter's term as `time_limit` alone (consistent with `particles.md` §10.6).
7. **Hostile dropped effects:** a never-ending effect on the dropped path (never in the data,
   §5.2) would never be removed by the client except with its scene. Tessera places it like any
   dropped effect, without stopping it (it emits normally), and removes the entry at the first
   lingering sweep (§7.3) more than 6 s after its placement; the effect thus shows its first
   6 s. (Revised 2026-10-09: the earlier wording "as if it had been stopped at once" could be
   read as stopping the effect at placement, which would show nothing.)

> **Answered (2026-10-09; was OPEN, implementer, 2026-10-09):** the removal-time reading is
> the rule (rule 7 reworded).
8. Q-RT1 to Q-RT10: the Decisions given in their callouts.
9. **Engineering limits for hostile records** (answers the implementer OPEN of 2026-10-08;
   the data never reaches them: a real-data run of all 11,155 records over 5 s peaks at 1,320
   live particles in one system):
   - at most 262,144 live particles per system, all emitters together; when the limit is
     reached, spawning stops as if every emitter were full;
   - at most 64 controller-4 steps per particle and update (a positive `time_step` far below
     1/120 s would otherwise make the 0.25 s catch-up loop unbounded; `particles.md` §10.5
     covers only non-positive and non-finite values);
   - at most 1,024 legs of a controller-1 walk per particle and update (repeated points make the
     walk of `particles.md` §8.7 land on zero-length legs without using up the budget);
   - a mode-3 curve draws its *r* as one real-range draw over [−1, 1] (§3.4).

> **Answered (2026-10-09; was OPEN, implementer, 2026-10-08):** the limits are recorded as
> Tessera rule 9.

10. **Unknown particle shader records** (none in this install; §9.1.1 answer (a)): a surface
    whose shader record is not one of the 19 of §9.1.1 is drawn as the common row of its blend
    class: colour *T* × *V* × *M*, output alpha *T*.alpha × *V*.alpha × *M*.alpha, the alpha test
    on *T*.alpha by §9.1.1 rule 1, one layer at the vertex UV, no doubling, fogged unless the
    destination factor is ONE; its opacity follows §9.1.3.
11. **Permutation feature bit 1** (§9.1.2): particle draws use the variants that exclude it, as
    the client does with `Render.VideoPostEffects` off or `Render.SurfaceReflections` at 0: no
    soft-particle factor, and 0x2B000084 draws nothing. (Revisit when `render.md` gives the
    particle stage a depth and colour copy of the scene; the formulas are in §9.1.2.)
12. **Curve states of particle draws** (§9.1.1, answer of 2026-10-10; RT31): keep one set of
    curve states per surface record DID, shared by every 0x39 record and every instance that
    draws that surface, and by any other draw of it. Create it when the surface's effective
    values are first evaluated; discard it with the surface's entry in the material cache, so a
    surface loaded again starts its mode-10 curves afresh, as a rebuilt effect does in the
    client. Evaluate at the material clock of `material.md` §5.3.1. Mode 3 keeps no state: at
    every evaluation, that is at every draw of the surface, each mode-3 curve takes its *r* as
    one real-range draw over [−1, 1] of the shared stream of rule 1 (§3.4, rule 9), not from a
    generator seeded from that stream. A preview or test that runs on its own simulated time
    keeps its states for the length of its run, never per frame. (Third revision, 2026-10-10,
    RT33.) A cache entry that is never dropped keeps its states for the session; no other drop
    point (effect stop, landblock unload) is required, and dropping a surface's states when no
    live system draws it is allowed. At most 4,096 surfaces keep states (an engineering limit,
    as rule 9); a draw of a further surface uses fresh states.

13. **Lights of lit systems** (`use_lighting` = 1, `particles.md` §8.14; a Decision for
    `particles.md` Q8 (b) where the client was not read, BIN where RT48 says so): the ambient
    colour is the frame's ambient term of `lighting.md` §5.3 at the owner (Tessera); the light
    list is the owner's light list of `lighting.md` §8.5 (the global light first, then the
    others in that list's order), cut to its first 5 entries, the global light counting as one
    (BIN, RT48). Each light contributes, per channel, its colour × clamp(DiffuseIntensity(*t*)
    × *σ*, 0, 1) × *a* (BIN, RT48): that is the record colour of `lighting.md` §8.3 (Diffuse ×
    the clamped intensity; that the colour field is Diffuse is Inferred, as in `decals.md` §6.3) times *a* = 1 / (1 + *k* · *d*²) with *k* = 10 × Falloff / Range²
    for a point light, at the distance *d* from the particle to the light, **without** the
    range cut-off of `lighting.md` §8.3 step 1 and without the smooth fade of step 3; for the
    global directional light (and any other non-point light) *a* = 1. No light has a direction
    term (particles have no normal). The alpha factor is the ambient alpha alone. Applies to
    the 1,061 lit 0x39 records, 526 of them reached by combat (`combat-visuals.md` §10).
    (Revised 2026-10-23: the range cut-off of the former text is not the client's.)
14. **Beams** follow §11.1, with these engineering limits for hostile records: at most 4,096
    beam particles spawned per beam and update, at most 1,024 ribbon segments per beam (further
    body segments are merged into the last), and a beam whose envelope never finishes because
    `Persistent` = 1 and its node is never stopped is removed with its node's run. A Beam node
    instance completes when its beam is removed (no beam: at once); `combat-visuals.md` relies
    on the port of step 7, not on completion.

> **OPEN (implementer, tessera-particles / tessera-client, 2026-10-23), rule 13:** readings
> and Tessera choices the code follows, for the spec writer to confirm or correct.
> (a) **The global light.** The scene's main light (sun or moon) is directional and has no
> position, so the point-light attenuation of rule 13 has no distance; Tessera counts it with
> attenuation 1 (its colour added in full, no direction term, since particles have no normal).
> Whether the client attenuates or angles it for particles is not stated.
> (b) **Reach.** `lighting.md` §8.5 step 2 measures the squared distance from a light to the
> object's world box; an effect owner here is tested with its **position** (its box is not
> known to the effect layer). Key lights first, nearest first in each group, then the cut to 5
> (the global light counts as one of the five).
> (c) **Ambient.** *A₀* of `lighting.md` §5.3 is clamped to [0, 1] per channel, alpha 1. In
> unlit frames (the walker's lighting switch off) lit systems keep the white ambient and no
> lights. The creation viewport (`character-screens.md` §2.1.1) uses its own ambient input and
> scene lights, no main light.
> (d) **When.** The list is built once per frame for each drawn owner, before its placement.
> A lingering system (released by its owner, §7.2) and a dropped system (§5.6 step 5, which
> never had an owner) get the list built the same way at their fixed placement position, each
> frame before they are placed; a beam's systems get the list at the beam's source. In unlit
> frames all of them get the empty list. The day and night lamp lights of
> `instance-scripts.md` §6 are not in the walker's light table yet, so they light no system.

> **Answered (spec writer, 2026-10-23; BIN pass (m), RT48, for (a); Tessera rules for
> (b)–(d)):** (a) the client gives every non-point light, the global directional light
> included, *a* = 1 and no direction term, so the reading is the client's; but each light,
> the global one included, is scaled by its clamped intensity (the record colour of
> `lighting.md` §8.3), and a point light has **no** cut-off at its range: rule 13 is revised
> accordingly. The 5-entry cut with the global light as one of the five is the client's.
> (b) testing the owner's position instead of its world box is confirmed as a Tessera rule,
> with key lights first and nearest first in each group (`lighting.md` §8.5 Decision).
> (c) the clamp of *A₀* to [0, 1] with alpha 1, the white ambient and empty list in unlit
> frames, and the creation viewport's own ambient and scene lights without a main light are
> confirmed as Tessera rules (the client's ambient source stays `particles.md` Q8 (a)).
> (d) building the list once per frame per drawn owner before its placement, at the fixed
> placement position for lingering and dropped systems and at the beam's source for a beam's
> systems, and the empty list in unlit frames, are confirmed as Tessera rules.

> **OPEN:** (implementer, walker-wiring, 2026-10-23; proposed, filed by the spec writer) rule 13
> and `decals.md` §9.1 (d): the walker's light table now holds the day and night lamp lights of
> `instance-scripts.md` §6, nearest first together with the Light-entity lights and capped at
> `MAX_TABLE_LIGHTS` = 255, so the light-list rules above reach them; the sentence of reading (d)
> saying they are absent is obsolete.

> **Answered (spec writer, 2026-10-23):** noted; the last sentence of reading (d) above ("The day
> and night lamp lights … light no system") no longer describes the walker. Lamp lights are
> point lights like any other (`instance-scripts.md` §9.4 last paragraph), so rule 13 applies to
> them unchanged. The table cap of 255 is a Tessera engineering bound (no client fact: the
> client keeps no such table; its per-object limits are `lighting.md` §8.5's); a lit system
> still takes at most 5 entries of its owner's list.

> **OPEN (implementer, world-conform, 2026-10-23; note, filed by the spec writer):** rule 13:
> `ParticleLight::range` is kept only for information; attenuation uses the record's *k* alone.
>
> **Answered (spec writer, 2026-10-23):** that is rule 13: the range enters only through
> *k* = 10 × Falloff / Range², and no cut-off or fade at the range applies.

15. **Parent opacity for `inherit_opacity`** (`particles.md` §8.15; a Tessera reading where the
    client's "parent object" was not traced; RT56): for a system attached to an owner (§5.5),
    the parent object is the owner's drawn object, and its current opacity is the opacity the
    owner is drawn with in that frame: the object layer's distance fade or cross-fade, a figure's
    fade, and any opacity the owner's scripts or properties set, multiplied together. "Opacity
    the owner's scripts or properties set" is the owner's `Render_Opacity` (dictionary default
    1): the value its setup's or placement's property set gives, then whatever the owner's
    scripts write to it later (the fades of `appearance.md` §5.7 step 4, SetProperty nodes,
    the sky tint of `environment.md` §11.4 step 5). This holds for static owners as for figures.
    **Tessera reading (owner properties):** `Render_Opacity` is the one owner property counted
    here because it is the one identified property of the render component's opacity:
    `mesh.md` §8.8 names it as one of that component's two factors in the opacity that decides
    shadow casting, and leaves the other factor unnamed. Whether anything else (that second
    factor, `Translucent`, the alpha of `Render_Color`) reaches the client's parent opacity
    was not traced (Q-RT14). Until Q-RT14 is answered Tessera counts none of them: `Translucent`
    only selects the translucent stage (`appearance.md` §5.7 step 4, a Tessera rule), and the
    tint alpha stays with the tint (`material.md` §8.3). (Revised 2026-10-23.)
    **Tessera reading (culled owners):** an owner that the draw pass culls in a frame (outside
    the view, its visibility cell not seen, its fade at 0) keeps that current opacity as the
    factor, 0 included, so a system follows its owner's fade out; the client's "placed in the
    same frame" condition may exclude culled parents (then factor 1), which was not traced.
    Tessera may take the value of the owner's previous drawn frame. **Never-drawn owners**
    (Tessera rule, moved here from the world-r6 callout answer below): an owner that has not
    been drawn since it spawned takes the fade the object layer would give it if it were drawn
    in this frame (an owner beyond its fade distance gives 0), not 1. **Owners whose cell is
    not seen** (Tessera rule, round 8): the factor is the fade the object layer holds for the
    object; while the object layer does not advance that fade (the object's visibility cell is
    not seen in this frame), the value of its last update is the factor, and the fade goes on
    from that value when the cell is seen again (`mesh.md` §8.7, including its at-once cases).
    While the object's parts are still loading, the factor may stay 0 until its first draw, so
    its systems appear with the object. **Owners that can never be drawn** (Tessera rule,
    round 8): an object none of whose parts can be drawn, because every part failed to load or
    decode, counts as an owner with no drawn object (next sentence); an object with at least
    one drawn part is a drawn object. When the
    owner has no drawn object at all (no setup, every part failed, or already removed), or the system has no owner
    (dropped, lingering after its release, engine-started), the factor is 1, as the client's
    "placed in the same frame" condition gives. The instance's own opacity stays 1 (Q-RT7). In
    this install 129 records set `inherit_opacity`, reached by 281 Particle nodes, 272 of them
    attached. (Revised 2026-10-23: culled owners and the one-frame lag.)

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):** rule 15:
> (1) a figure owner's parent opacity is the opacity from the character layer's last draw pass,
> which runs after the effects update, so it lags one frame; is the one-frame lag acceptable?
> (2) A figure the draw pass culls (fade reached 0, outside the frustum, or its visibility cell
> not seen) counts as drawn with its fade (0 included) in Tessera's reading, not as "not drawn"
> with factor 1; does the rule mean that? (3) Static owners use 1, because the object layer
> does not expose per-object fade or cross-fade; an accessor for an instance's current fade is
> needed in the object layer, and rule 15 is not yet built for static owners.
>
> **Answered (spec writer, 2026-10-23; Tessera rules, the client's "parent object" was not
> traced):** (1) yes, the previous frame's value is allowed (rule 15 now says so): the fades it
> carries change by about 1 % per frame at 60 frames per second (1.5 s fades). (2) yes: the
> factor is the owner's current fade and opacity whether or not the draw pass culls it; "not
> drawn" means no drawn object exists. Rule 15 is reworded. (3) rule 15 applies to static owners
> unchanged (the object layer's distance fade and cross-fade); factor 1 for them is a build gap,
> not a rule, and must not be reported as conforming.

> **OPEN (implementer, 2026-10-23; proposed, filed by the spec writer):** rule 15 for static
> owners: which owner properties count as "any opacity the owner's scripts or properties set"?
> The object layer applies no `Render_Opacity` or similar to static objects today.
>
> **Answered (spec writer, 2026-10-23; a Tessera reading, see rule 15 and Q-RT14):**
> `Render_Opacity` alone: the value the owner's property set gives (read at run time; in this
> install 1,563 entity descriptors carry the property, `setup.md` §4.4, an observation) and any
> later value the owner's scripts write to it, times the object layer's distance fade and level
> cross-fade (`mesh.md` §8.7–§8.8). `Translucent`, the alpha of `Render_Color` and the render
> component's unnamed second factor are not counted; whether the client counts any of them is
> Q-RT14. The culled-owner case is marked as a Tessera reading. While the object layer does not apply `Render_Opacity` to static objects,
> a static owner's factor lacks that term; that is a build gap of the object layer, not of rule
> 15 (the same term is part of the shadow-casting opacity of `mesh.md` §8.8).

> **OPEN (implementer, world-r6, 2026-10-23; proposed, filed by the spec writer):** rule 15:
> for a static owner the object layer gives no opacity for a frame in which it did not draw the
> object. Tessera keeps the value of the owner's last drawn frame, and an owner never drawn since
> it spawned counts as 1 (also when its distance fade would be 0). Is factor 1 acceptable for a
> never-drawn owner, or must the object layer expose its fade for objects it culls?
>
> **Answered (spec writer, 2026-10-23; Tessera rule, no client fact):** keeping the last drawn
> frame's value is confirmed (rule 15 allows it). Factor 1 for a never-drawn owner is **not**
> accepted where the owner's fade is below 1: the factor is the owner's current fade and
> opacity whether or not it is drawn (answer (2) above), and the distance fade and the level
> cross-fade of `mesh.md` §8.7 depend only on the owner's distance and its level state, not on
> drawing it. So the object layer must give that fade for an owner it has culled or not yet
> drawn (computed as it would be if the object were drawn this frame); an owner beyond its fade
> distance then gives 0 and its systems follow it out. Until the object layer gives it, factor 1
> for a never-drawn owner is a build gap and must not be reported as conforming; an owner whose
> fade would be 1 is unaffected.

> **OPEN (implementer, walker-leftovers, 2026-10-23; engineering note, filed by the spec
> writer):** rule 15: owners are now looked up by their entity index in the info record (the
> same index the walker's static owner IDs carry). If several objects in one block carry the
> same index, the first in list order answers. An index no object carries answers none
> (factor 1), with no fallback to the position match. The position, parts and rank match is
> used only when the caller passes no index or the block's objects carry none.
>
> **Answered (spec writer, 2026-10-23; Tessera rule, no client fact):** confirmed. A static
> owner is the object its landblock's info record places at that position of its entity list
> (`entity-descriptor.md`); within one record the index is unique by construction, so the "first
> in list order" case arises only when the object layer's list mixes entities of the info
> record with generated scenery objects (`scenery.md` §7.6), whose indices count a separate
> list. The lookup must tell the two lists apart (match the static entities' indices only
> against static entities); first in list order is then only the tie rule for inconsistent
> data. An index that no object of the block carries is an owner with no drawn object, factor 1
> by rule 15, with no position fallback. The position match is kept only for callers without an
> index.

> **OPEN (implementer, walker-wiring-r7, 2026-10-23; proposed, filed by the spec writer):**
> rule 15: the object layer gives a static owner its object's current draw-distance fade. Two
> cases are open. (a) An object whose cell is not seen this frame keeps the fade value of its
> last update, because the fade does not run outside the scene; Tessera reads this as the
> allowed "previous drawn frame" value. (b) An object whose parts never finish uploading (a
> failed mesh) stays at fade 0, so its `inherit_opacity` systems stay hidden for good. Should
> such an owner count as having no drawn object (factor 1)?
>
> **Answered (spec writer, 2026-10-23, round 8; Tessera rules, the client's "parent object" was
> not traced):** (a) confirmed, with a reason other than the one-frame lag: the factor is the
> opacity the owner's drawn object has, and in Tessera that is the fade the object layer holds
> for it; an object whose cell is not seen keeps its held fade, and the fade continues from it
> when the cell is seen again, so the systems and the object agree in every frame in which
> either can be seen. The "previous drawn frame" allowance is the one-frame lag and does not
> cover a value many frames old. (b) yes: an object every part of which failed to load or
> decode can never be drawn and counts as having no drawn object, factor 1. A part that is
> still loading is not a failure; until the object's first draw the factor may stay 0 (its
> systems then appear with the object). Rule 15 now says both, and its body also carries the
> never-drawn rule of the world-r6 answer above (review note of the round-7 review).
> **Implemented behaviour changes:** a failed mesh owner gives factor 1 instead of 0.
16. **Systems of the pass after post-processing** (`uses_video_post` = 1, `particles.md` §8.15;
    RT55, RT56): the client draws them only while its post-processing step is active, which with
    the built-in `VideoPostEffects` on and a qualifying device is the normal case (both of the
    owner's preferences files hold it on). Tessera has no post-processing step (`render.md` §5),
    so it draws them as if the step were active and drew nothing: in a separate particle stage
    after every other draw of the world pass (the far view and the translucent stage included)
    and before the UI, in the order of `particles.md` §8.15 among themselves, with the same
    depth test and the same §9 drawing as the main-pass systems. A Tessera option that mirrors
    `VideoPostEffects` = off skips them. 139 records set the flag, reached by 887 Particle
    nodes; 97 of them draw 0x2B000084 and so draw nothing under rule 11 (RT29); the other 42 draw
    normally. The rule holds in every view that draws particle systems, the creation viewport of
    `character-screens.md` §2.1 included: there too these systems come in a stage of their own
    after the viewport's sorted blended meshes, not inside its single premultiplied pass.
    As built, that stage is the `Post` stage of `render.md` §5.1 (Stages row): the last stage
    of a world pass, which the walker uses in the frame's last world pass and the creation
    viewport in its one pass; it comes after the viewport's sorted blended meshes, so it is
    the separate stage this rule asks for. (Revised 2026-10-23.)

> **OPEN (implementer, world-conform, 2026-10-23; status note, filed by the spec writer):**
> rule 16 (separate late stage) is built for the world view, not for the creation viewport:
> there the post systems are last in its single premultiplied pass, so they come before that
> pass's sorted blended meshes; a second pass in the viewport frame was not built.
>
> **Answered (spec writer, 2026-10-23):** rule 16 covers the creation viewport too (sentence
> added): the viewport needs the late stage after its sorted blended meshes. Until it is built
> the viewport does not conform to rule 16.
>
> **Update (spec writer, 2026-10-23, from the implementer's report):** built: the renderer has
> a stage after the sorted blended meshes in every world pass, and the creation viewport
> queues its post systems there in its one pass; `render.md` §5.1 (Stages row, `Post`) records
> it, and rule 16 names it. A second pass is not needed.
17. **Weather systems** (§11.2): the global wind follows §11.2.1 in every walker frame, so a
    system with physics flag 0x20 takes the environment's wind at its start (`particles.md`
    §8.8) and the two gust systems follow it in every update (§11.2.3 step 3). The camera systems (§11.2.4
    steps 3–5) follow the stated thresholds with the walker's camera; the sky-exposure test of
    §11.2.3 step 2 uses the walker's collision world (`collision.md`) along −*f* over 100 m from
    the camera and from the avatar. The walker keeps the two drop sets of §11.2.6: the count and
    gate of step 1 with `[Render] AtmosphericsDetail` (built-in `Medium`, `walker.md` §14.10
    `--render-pref` may set it), the field, placement, fall and reference surface of steps 2–5
    on its collision world and water probe, the spawns of §11.2.3 step 4 from them, and the
    streaks of step 7 drawn in the translucent stage of the world pass (`render.md` §5) with the
    material of UNIQUEDB key 0x2B, every frame in which the precipitation step runs. **Snow,
    until Q-RT13 is answered:** the snow set follows the same steps with the snow parameters
    and the key-0x3B material, one flake per drop drawn as a streak of length min(*h*, 0.08 m);
    this interim rule is Tessera's, not the client's flake geometry. The drop quads are drawn
    after the beam ribbons and before the particle systems of the translucent stage (Tessera
    rule, §11.2.6 callout). A drop left marked for placement, by a failed collision test or by
    Tessera's cap of 512 placements per update, is still drawn and spawns with its previous
    state (§11.2.6 step 3). The figure effects of §11.2.5 apply to the walker's avatar with the
    water probe of §5.7 cast from the avatar's position and the volume flags of the liquid box
    its feet are in (Tessera rules of the §11.2.5 callout; `walker.md` §13.5); in the client only some bones of the figure take part, and
    which ones was not traced (Q-RT13), so **Tessera lets every bone with a parent bone take
    part**. `PrecipitationEffects` off turns all of §11.2.2 step 2
    off, as in the client. (Revised 2026-10-23: drops, interim snow and bone rules; then the
    drop draw order, marked drops and the figure-effect sources.)

---

<a id="s13"></a>
## 13. Invariants

Against the install (all hold in this install, OBS) or synthetic fixtures:

- **RT-I1:** for every Particle node, the stored extra byte equals the classification of §5.3
  computed from the record its effect DID names (41,354 of 41,354).
- **RT-I2:** every node with `ParticleScript_DropEffect` = 1 has stored byte 0.
- **RT-I3:** the generator of §3.3 seeded with *n* > 0 produces the same outputs as the
  published ran2 initialised with −*n* (same algorithm and constants); checkable against an
  independent implementation of the published algorithm over the first 10,000 draws.
- **RT-I4:** a real-range draw with *a* = *b* consumes no number of the stream (§3.4).
- **RT-I5:** a stopped lingering system with no end condition is removed at the first sweep
  more than 6 s after its stop time; with `ImmediateDisposal` = 1 it is never in the lingering
  list.
- **RT-I6:** a system of the shareable kind is never in the lingering list.
- **RT-I7:** with `RemoveRotation` = 1 the rotation part of the attachment's local matrix is the
  identity for every pose.
- **RT-I8:** inside one emitter, the vertex order of a frame equals the array order of §9.2;
  after the death of particle *k* of *n*, the former last particle takes position *k*.
- **RT-I9:** an attached finite effect is removed at most 2 s (plus one frame) after its end
  test first passes.
- **RT-I10:** every particle surface that declares 4004 has a constant (mode 1) 1043 curve
  (1,057 of 1,057).
- **RT-I11:** among the 19 shader records of §9.1.1, the permutations whose key word 1 has bit 1
  are exactly those that sample the depth image (nine records, Direct3D 10/11 only) or the
  frame-buffer image (0x2B000084 only); every permutation of the 19 that samples neither has bit 1
  clear in word 1.
- **RT-I12** (synthetic): a two-layer particle surface whose second-layer translation 1019 is
  the constant 0.5 and whose other matrix values are the identity samples the second layer at
  (*u* + 0.5, *v*) and the first at (*u*, *v*); a one-layer surface with the same values samples
  at (*u*, *v*).
- **RT-I13** (synthetic; rule 12): two 0x39 records name one surface whose second-layer
  translation 1019 is a mode-10 curve with `loop` 0, `period` 1 s and the pairs (0, 0) and
  (1, 100). A system of the first record is drawn from clock 10 on, a system of the second from
  clock 10.5 on. Both read one state, started at clock 10: at clock 10.5 both read 0.5, and
  from clock 11 on both read the held last value. A state per 0x39 record would give the
  second system its start value 0.001 (position 0.1, `material.md` §5.3.1) at clock 10.5.
- **RT-I14:** among the 1,179 particle surfaces, the curves of 1043 and of the texture-matrix
  values (1001–1004, 1017–1020, 1033–1034, 1070–1073) that are not constant use modes 2, 3, 4,
  6, 7 and 10 only; exactly two surfaces have a mode-10 curve, and no 1043 curve is non-constant
  (OBS3, RT31).
- **RT-I15:** every mode-10 curve among those values has `loop` 1 (5 of 5 curves, on 2
  surfaces; OBS3, RT32).
- **RT-I16** (synthetic; rule 12): a surface with two mode-3 texture-matrix values, drawn three
  times at one clock with the shared stream seeded with a fixed *n*, advances the stream by
  exactly six draws; each value is *B* + *A* · *r* with *r* = −1 + 2*u* for the stream's next
  draw *u*, formed by the four steps of §3.4, so the three draws generally get different values; a surface without mode-3
  curves advances the stream by none.

- **RT-I17** (synthetic; §11.1): a beam from (0, 0, 0) to (12, 0, 0) with `HeadStartPos` 0,
  `HeadAnimStartTime` 0 and `HeadAnimSpeed` 40 has *h* = 0.5 at *t* = 0.15 s and arrives at the
  first update with *t* > 0.29994 s; its Default port fires in that update and never again.
- **RT-I18** (synthetic; §11.1 step 8): with `SegmentSize` 1, `BeamSegmentParticlesPerSecond`
  10, a drawn length of 6 m and updates every 0.05 s, each update spawns ⌊6 × 10 × 0.05⌋ = 3
  particles, at times 1/60 s apart, all on the drawn segment; after a 2 s pause the next update
  spawns ⌊6 × 10 × 0.1⌋ = 6. The products are single-precision steps (RT47), so updates exactly
  0.05 s apart on a double-precision clock give 3 every time.
- **RT-I20** (synthetic; §11.1 ribbon vertices): `NumTailSegments` 2, `NumHeadSegments` 1,
  `SegmentSize` 1, `TextureUScale` 2, a drawn length of 5.5 m: *g* = 1, the head and tail parts
  are 3 segments of 1 m, the body 2.5 m is two whole segments and one of 0.5 m. Pair-1 *u* is 0
  at the tail end, 2 where the tail part ends, 7 where the body ends and 9 at the head end;
  pair-2 *u* runs 0 to 1 over the tail part, 0 to 2.5 over the body and 0 to 1 over the head
  part; the segments are 2 tail, 3 body and 1 head, drawn tail first.
- **RT-I19** (synthetic; §11.1 step 2): `RampInTime` 0.5, `SustainTime` 1, `RampOutTime` 0.25,
  `Persistent` 0, stopped at elapsed 0.2: *e* = 0.4 at the stop, the ramp-out clock starts at
  0.15, so *e* falls linearly from 0.4 to 0 over the next 0.1 s.
- **RT-I21** (synthetic; §11.2.6 step 1): rain 0.5 at `AtmosphericsDetail` `Medium` gives
  *n* = ⌊3,000 × 0.5 × 0.6⌋ = 900 drops; snow 1.5 at `High` gives 3,000 (the value is clamped to
  1 for the count); rain 0.0005 at `Low` gives ⌊0.45⌋ = 0, so that update makes no exposure test,
  recomputes no force, spawns nothing and reports no exposure.
- **RT-I22** (synthetic; §11.2.6 steps 4 and 7, rain, updates 1/60 s apart): *t* falls by
  25 / (60 × 30) ≈ 0.013889 per update; a drop with *t* = 0.05 has *h* = 1.5 m ≤ 1.7, so it
  spawns by §11.2.3 step 4 and its streak is 1.5 m long, ending at its lower end point; with
  *t* = 0.5 (*h* = 15 m) the streak is 1.7 m long, from 15 m to 13.3 m above the end point along
  −*d*.
- **RT-I23** (synthetic; §11.2.6 step 3): draws *u*₁ = 1, *u*₂ = 1 place a drop at the field
  origin; *u*₁ · *u*₂ = 0 places it 30 m away; a contact 0.05 m above a reference height of 12 m
  gives an end point at 12 m marked as ending on the reference surface, and so does a contact at
  11 m (below it).

---

<a id="s14"></a>
## 14. Worked synthetic example: one emitter over 2 s

All values are invented for testing; they are not game data.

<a id="s14-1"></a>
### 14.1 Set-up

- **System:** `time_step` 0.1, `fast_forward` 0, `physics_duration` 0, `world_space` 1,
  `use_lighting` 0, far fade 40–60 m, near fade 0, `scale_mode` 1 (`Total`), `sorting_priority`
  5, `shareable` 0, a loaded surface.
- **Emitter:** `shape` 4 (`Point`), `primitive` 2 (`Quad`), `emit_interval` 0.25, `speed`
  constant 2.0, `direction` (0, 0, 1), spread 0, `emission_limit` 4, `max_particles` 10,
  `burst_count` 2, `lifespan` constant 1.0, `particle_scale` constant 1, rotation 0, `streak` 0,
  `start_time` 0.25, `time_limit` 0, `emission_distance` 0, `active` 1, `fade_in` 0.5, `fade_out`
  −1, 1 × 1 frames, no holding location or bone.
- **Keys:** K0 = (time 0, flags 0x770, scale 0.2 × 0.2, colour 0xFFFFFFFF, mass 1.0, controller
  2, physics flags 0x1). K1 = (time 1.0, flags 0x6, scale 0.6 × 0.6, colour 0x00FF0000, mass 1.0,
  controller 2, physics flags 0).
- **Owner:** at the origin with identity orientation, render scale (1, 1, 1),
  `Model_ParticleScale` absent (*p* = 1), drawn every frame at opacity 1, camera 10 m away.
- **Node:** `ParticleScript_ParticleID` = this record, `DropEffect` 0, no holding location,
  bone or offset, `ImmediateDisposal` 0, `RedirectProperty` 0, context intensity 1.
- **Frames:** the client clock reads 10.00 at the first placement and advances 0.04 s per frame
  (10.00, 10.04, …), so no event falls exactly on a frame.

<a id="s14-2"></a>
### 14.2 Start

- §5.3: no emitter has both limits 0; *v* = max(0, (4 div 2) × 0.25) + 1.0 + 0.25 − 0 = 1.75 ≥ 0,
  so the effect is **finite** and the node's stored byte is 0. §5.4: intensity 1 > 0.0002, so it
  starts; §5.5: the node completes at once (no instance).
- First placement at 10.00: system start *S* = 10.00; last particle-update, last-burst and
  last distance-emission times = *S* + 0.25 = 10.25 (`particles.md` §8.17).
- End test (`particles.md` §8.2): the system has ended once now > 10.00 + 0.25 + 4 × 0.25 / 2 +
  1.0 = 11.75.

<a id="s14-3"></a>
### 14.3 Timeline

| Frame (clock) | What happens |
|---|---|
| 10.00–10.24 | Emitter time < 0: outside the window; nothing. |
| 10.28 | Timed emission: the last burst (10.25) is before now, so 2 particles P1, P2 are spawned **at 10.25**; total 2; last burst → 10.50. Each new particle is advanced to the last particle-update time, 10.25 (age 0, position (0, 0, 0)). No particle update (0.03 s since 10.25 < 0.1). |
| 10.36 | Particle update (0.11 ≥ 0.1): P1, P2 at age 0.11; last update → 10.36. |
| 10.48 | Particle update: ages 0.23; last update → 10.48. |
| 10.52 | Spawn P3, P4 **at 10.50**, advanced to 10.48: age −0.02, so controller 2 places them at *z* = 2 × (−0.02) − 4.9 × 0.0004 = −0.04196 m. Total 4 = `emission_limit`; last burst → 10.75. |
| 10.56 | Drawn (see §14.4). |
| 10.60, 10.72, … | Particle updates every 0.12 s (every third frame). |
| 10.76 and later | Last burst 10.75 is before now, but cap = min(`emission_limit` 4, 10) = 4 and 4 particles live: room 0, so the last-burst time is set to now. After deaths room > 0, but the burst is min(2, 4 − 4) = 0 < 1: no emission ever again. |
| 11.32 | Update: P1, P2 at age 1.07 > 1.0 die. Array [P1, P2, P3, P4] → P4 moves into slot 0, then P2's slot takes P3: [P4, P3]. Draw order is now P4, P3 (§9.2). |
| 11.56 | Update: P3, P4 at age 1.06 die. |
| 11.76 | First frame with now > 11.75: the end test passes. |
| next 2-s check | Housekeeping (§7.4) removes the attachment; the release (§7.2) makes a lingering entry that the next sweep removes (§7.3) because its end test passes. |

<a id="s14-4"></a>
### 14.4 Drawn values at clock 10.56

Interpolation factor *f* = (10.56 − 10.48) / 0.1 = 0.8 (`particles.md` §8.1). Emitter time
0.31, so the fade envelope is 0.31 / 0.5 = 0.62 (`particles.md` §8.9). *m* = 1 (*A* = *B* =
(1, 1, 1)). Motion: controller 2, *z*(*τ*) = 2*τ* − 4.9*τ*² (gravity 9.8 × mass 1).

| Particle | Age at the draw | Previous / current *z* (at updates) | Drawn *z* | Width = height | Colour (R, G, B, A) | Drawn alpha |
|---|---|---|---|---|---|---|
| P1 (and P2) | 0.31 | 0.16071 (age 0.11) / 0.20079 (age 0.23) | 0.16071 + 0.8 × 0.04008 = 0.19277 m | 0.2 + 0.31 × 0.4 = 0.324 m | (1, 0.69, 0.69, 0.69) | 0.69 × 0.62 = 0.4278 |
| P3 (and P4) | 0.06 | −0.04196 / −0.04196 (no update since creation) | −0.04196 m | 0.224 m | (1, 0.94, 0.94, 0.94) | 0.94 × 0.62 = 0.5828 |

Colour and scale interpolate from K0 towards K1 (K1 has bits 0x2 and 0x4); mass stays 1.0 (K1
lacks 0x8). The quads face the camera (`particles.md` §8.12, default corners).

<a id="s14-5"></a>
### 14.5 Variants

- **Never-ending:** `emission_limit` 0 → stored byte 1; the node keeps an instance. If the
  script stops at clock 11.00 with `ImmediateDisposal` 0, the system lingers (world-space, §7.2
  step 3): no emission after 11.00, live particles continue, and the entry is removed at the
  first sweep after 17.00 (6 s). With `ImmediateDisposal` 1 every particle disappears at 11.00.
- **Intensity 0:** the finite original is not started at all (§5.4 step 3); the never-ending
  variant is.
- **Dropped:** `DropEffect` 1 with `BoneOffset` (0, 0, 1): the effect is placed once at about
  (0, 0, 2) (offset twice, §5.6) and stays there if the owner moves; removed as in §14.3 after
  11.75.
- **Owner scale:** render scale (2, 2, 2) and `Model_ParticleScale` 0.5: `Total` gives *S* =
  (1, 1, 1) and *m* = 1; `VDescOnly` gives *m* = 0.5 (width 0.162 m at clock 10.56 for P1);
  `None` gives *m* = 1.

---

<a id="s15"></a>
## 15. Open questions

> **OPEN (Q-RT1):** Per-frame order and culled owners (§3.2). Decision there.

> **OPEN (Q-RT2):** Source of context flag 0x40 (§5.4). Decision there.

> **OPEN (Q-RT3):** Frustum culling by the record's bounds (§9.3). Decision there.

Q-RT4 — answered 2026-10-10: §11.1.

> **OPEN (Q-RT11):** Which scales of the source entity `HeadEntityScaleSource` values 1, 3 and 4
> select for a beam's head entity (§11.1 step 1), and what the flag the beam sets on its
> particle systems at creation changes (not traced); and whether the draw step of §11.1 (which
> moves the head and detects the arrival) runs for a beam outside the view. **Decision
> (Tessera):** steps 3–7 run every frame for every beam, drawn or not; 1 → the source's
> render scale (`setup.md` §5.2), 3 and 4 → the mean of the source's render scale, other values
> → (1, 1, 1); the systems run as any system (their own timed emission included).

Q-RT5 — answered 2026-10-23: §11.3 (the effect preview of the options window).

> **OPEN (Q-RT6):** The meaning of the flag value 0xC000 that the `Triangle` draw call of §9.1
> passes and the `Quad` call does not. **Decision (Tessera):** draw both lists with the same
> pipeline state.

> **OPEN (Q-RT7):** Which owner value gives an attached draw instance its own opacity (the
> instance opacity that `particles.md` §8.15 multiplies by the object opacity), for example a
> fading owner's current opacity. Not traced. **Decision (Tessera):** 1. Suggested method: M4 on
> the effects component's placement routine (the opacity it hands to the instance).
> *Note (2026-10-23):* the other factor of `particles.md` §8.15, the parent object's opacity that
> `inherit_opacity` multiplies in, has a Tessera reading in rule 15; this question is only about
> the instance's own opacity.

> **OPEN (Q-RT8):** Whether a running effect follows a later change of a driven
> `ParticleScript_*` field (only `ParticleScript_ParticleID` is driven in the data, RT24): the
> Particle node's "fields changed" hook (`script-runtime.md` §5.3 step 2) was not traced.
> **Decision (Tessera):** fields are read at the start only, as for Sound nodes
> (`script-runtime.md` OPEN-3).

> **OPEN (Q-RT9):** For the bit-1 variants of §9.1.2: which image the frame-buffer input of
> 0x2B000084 holds and when the client copies it, in the main pass and in the pass after
> post-processing (97 of the 209 distortion systems, RT29); which auxiliary view clears the
> two frame flags of condition 3; and the device conditions of condition 1. Suggested method:
> M4 on the frame's render-target set-up and the callers of the mask builder. **Decision
> (Tessera):** rule 11 (the variants without bit 1), which needs none of these answers.
> *Note (2026-10-23):* where the 139 systems of the pass after post-processing are drawn in
> Tessera is rule 16; `VideoPostEffects` is on by default (RT55).

> **OPEN (Q-RT10):** When the client marks a loaded surface record's effect for a rebuild
> other than at its first draw after loading (§9.1.1 answer of 2026-10-10). A rebuild copies
> the curves again and so restarts every mode-10 curve of that surface. Also (added in the
> third 2026-10-10 revision): that all users of one surface DID get one loaded object is
> Inferred (RT31), and how long the client keeps a loaded surface record after its last user
> is released (at once, or in a cache of released objects) was not traced. Suggested method:
> M4 on the writers of the surface's rebuild flag and on the release path of the shared
> object fetch. **Decision (Tessera):** rule 12 (a new state set only when the surface is
> loaded again; kept for the session where the cache never drops the surface). In this
> install the answers change only the phase of looping curves (RT32).

> **OPEN (Q-RT12):** The effect-filter page of the options window keeps block lists of effect
> DIDs for the character and for the account (§11.3). Whether, and at which step of §5.4, a
> blocked DID is refused when an effect starts, and where the lists are kept between sessions,
> were not traced (the lists are kept outside the particle and options-window code that this
> pass read). **Decision (Tessera):** no effect is blocked; Tessera has no such page.

> **OPEN (Q-RT13):** What remains of the weather code's own drop simulation (§11.2): how the snow
> update places, moves and draws its flakes (more than one flake per drop, at places apart from
> each other, and a further factor on the fall); which bones of the figure take part in the
> effects of §11.2.5, and what decides it; the client state flag of §11.2.2 step 1 and the figure condition
> of step 3; which bodies the collision query of §11.2.3 step 2 and §11.2.6 step 3 sees and which
> contact it reports; where in the frame and with which blend the drop quads are drawn.
> Suggested method: M4 on the snow update, on what selects the bones of §11.2.5 and on the
> weather code's draw. **Decision (Tessera):** rule 17.
>
> **Answered in part (spec writer, 2026-10-23; BIN pass (o), RT58–RT63):** the drop counts and
> their quality factor, the rain simulation (field, placement, fall, respawn, reference
> surface, streak geometry, colours, material), the parameter sets of both sets and the snow
> material are §11.2.6; the reference frame of the world-placed systems is §11.2.4 step 6.
>
> **Answered in part (spec writer, 2026-10-23; BIN-P, RT65–RT67):** marked drops that are not
> placed (still drawn, §11.2.6 step 3), the radius test after a field move (at *P*, step 2), the
> snow gust ratio (once per process, §11.2.3 step 4) and the figure's water state for §11.2.5
> (the §5.7 probe and the figure's own flags). **Still open, added:** the initial field origin
> and the initial state of the drops (Tessera rule in the §11.2.6 callout).

> **OPEN (Q-RT14):** Which owner opacity the client's parent opacity of `inherit_opacity`
> reads (§12 rule 15): `mesh.md` §8.8 names `Render_Opacity` as one of the render component's
> two opacity factors; what sets the second factor, and whether `Translucent` or the alpha of
> `Render_Color` change the opacity a parent passes on, were not traced. Suggested method: M4 on
> what writes the render component's second opacity factor and on the parent-opacity read.
> **Decision (Tessera):** rule 15 (`Render_Opacity` alone).

Still open in `particles.md`: Q1 (fork effect image and simulation; the blend-mode index source
of §10.2), Q2, Q8 (local-space frames, light attenuation, streak in `world_rotation` mode), Q13
(weather schedule; answered in part by §11.2, the rest is Q-RT13 here). `particles.md` Q4 is answered by §6.4 here.

---

<a id="s16"></a>
## 16. Notes for other specs

- `particles.md` §13 Q4: answered by §6.4 (*A* = render scale, *B* = `Model_ParticleScale`).
- `particles.md` §8.17, last bullet: the per-emitter maximum starts at 0, so the "largest is
  negative" case cannot occur (§5.3); reviewer note (c) of its review log is answered there too
  (inactive emitters count).
- `script-table.md` §4.1: the Particle node's extra byte (`particle_flag`) is the stored
  never-ending classification (§5.3).
- `script-runtime.md` §3.1 / kind table: the Particle node's start, instance and stop hook are
  §5.4, §5.5 and §7.1 here; context flag 0x40 is read by it (Q-RT2).
- `setup.md` §5.2: "how an effect combines the two is for the particle specs" is §6.4 here.
- `material.md` §5.3.1: the "client's global random generator" of mode 3 is §3.3 here. The
  "start time and last value" of a mode-10 curve live in each copy of the curve; for surface
  records the copies belong to the effect built per loaded surface record (§9.1.1 answer of
  2026-10-10, RT31), which may be worth stating there for meshes too.

- `material.md` §11.3 (rule for unlit records) and §20.1 (output alpha of unlit blended
  records), `render.md` §4.3 (`Fixed` row, "whether *M*.alpha and the vertex alpha enter in the
  client is OPEN in `material.md`"): for the 19 unlit records of §9.1.1 the programs multiply the
  texture's opacity, the vertex alpha (where read) and the material alpha into the output alpha
  that the fixed blend reads, and the alpha test reads the texture's opacity alone (BIN-S). If
  any mesh surface uses these records, the same holds there.

- `water.md` §8.4 OPEN ("how the client chooses the permutation"), `material.md` §7.4 and
  `lighting.md` §3.1: feature bit 1 of the frame's permutation mask, which the water
  permutations with the frame-buffer copy and the depth texture require, is set only under the
  three conditions of §9.1.2 (`Render.VideoPostEffects` with a qualifying device,
  `Render.SurfaceReflections` not 0, outside an auxiliary view) (BIN, RT25). The other bits of the
  mask were not traced.
- `render.md` §4.3 and §6.10: §9.1.3 states how particle draws feed the fixed-blend factors
  (the program's output alpha, not the tested opacity alone) and how they behave in premultiplied
  passes; a renderer that routes particle surfaces through the `Fixed` alpha mode needs a way to
  pass that alpha.

- `environment.md` §5.3 (field 17) and §11.7 step 6, OPEN on the use of curves 3 and 4: they
  are the two horizontal components of the environment's wind vector, which becomes the target
  of the global wind (§11.2.1, RT49); the forced weather zeroes it.
- `environment.md` §11.7 (rain and snow values): their particle-side consequences are §11.2.2–
  §11.2.5 (rain above 0.05, 0.2 and 0.5; snow above 1.001).
- `walker.md` §13.5 and `water.md`: the figure-in-water effects of §11.2.5 need the avatar's
  water surface height and the `Volume_IsLava` / `Volume_IsCoin` flags of its volume.

These are suggestions for the owners of those specs; this spec edits none of them.

---

<a id="s17"></a>
## 17. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-23 | Header, §11.2.6 step 1 (callout), §12 rule 15 (body and callout) | Spec writer (dirty room), package spec-r8; status Accepted → Revised, awaiting independent review. Rule 15: the never-drawn owner rule of the world-r6 answer moved into the body (review note of the round-7 review); new Tessera rules: an owner whose visibility cell is not seen keeps the fade the object layer last gave it, a loading owner may stay at 0 until its first draw, an owner every part of which failed counts as having no drawn object (factor 1); walker-wiring-r7 callout filed and answered. §11.2.6 step 1: the `AtmosphericsDetail` reading callout filed and answered by pointer to `walker.md` W26 (level names, numbers 0 or above, other values `Medium`, missing key keeps the earlier source's level). Tessera rules only; no client fact. |
| 2026-10-23 | Header, §1 (new BIN-P; new RT65–RT67), §11.2.3 step 4 and callout, §11.2.5 and callout, §11.2.6 steps 2–3 and three callouts, §12 rule 15 (two callouts) and rule 17, §15 Q-RT13 | Spec writer (dirty room), package spec-world-r7; status Accepted → Revised, awaiting independent review. The round-6 implementer callouts (world-r6, walker-leftovers) filed and answered. From the listings of passes (n) and (o), read again (BIN-P): a marked drop that is not placed keeps its state and is still drawn and spawns; after a field move only unmarked drops in use keep their world position and the radius test is made at the placement point *P*; the snow gust ratio is fixed once per process; the figure effects use the §5.7 water probe at the figure's position and read `Volume_IsLava` / `Volume_IsCoin` from the figure's properties. Tessera rules: the 512-placement cap accepted with the client's treatment of unplaced drops; the first field origin at the camera; the drop quads after the beam ribbons and before the particle systems; the avatar's probe and liquid-box flags for §11.2.5; a never-drawn static owner takes its computed distance fade, not 1; static owners looked up by their info-record index, kept apart from generated objects. |
| 2026-10-23 | Header, §12 rule 15 and its static-owner callout, §15 (new Q-RT14) | Spec writer (dirty room), package spec-world-r6, after the not-passed review spec-review:spec-world-r6:1; status stays Revised, awaiting independent review. Rule 15's count of `Render_Opacity` alone is now labelled a Tessera reading: `mesh.md` §8.8 names only one of the render component's two opacity factors and excludes nothing, so the exclusion of `Translucent`, the alpha of `Render_Color` and the unnamed second factor is a Tessera choice, and the open client question is the new Q-RT14. The callout's parenthesis now separates the 1,563 count (an observation) from the value (read at run time). No new client fact. |
| 2026-10-23 | Header, §1 (BIN-O wording), §11.2.5, §11.2.6 (intro, steps 2 and 7), §12 rules 15–17 and callouts, §15 Q-RT13 | Spec writer (dirty room), package spec-world-r6; status Accepted → Revised, awaiting independent review. Review notes (1) and (2) of the spec-world-r5 review applied: the drop field's origin move, the bones that take part in §11.2.5 (rule 17, Q-RT13, BIN-O), the snow drop's flakes and the streak's width direction restated as behaviour; rule 15's culled-owner case marked as a Tessera reading. Rule 15: the owner opacity set by scripts or properties is `Render_Opacity` alone (static owners included); implementer callout filed and answered. Rule 16: the built stage recorded (`render.md` §5.1 `Post`), status update under its callout. No new client fact; no implemented behaviour change except the static-owner `Render_Opacity` term of rule 15. |
| 2026-10-23 | Header, §1 (new BIN-O, pass (o); new OBS6; new RT58–RT64), §11.2 intro, §11.2.1 (blended wind), §11.2.2 (count gate), §11.2.3 step 4 (reference surface) and implementer answer on step 2, §11.2.4 step 6 (identity frame, spawn positions relative to the reference point), §11.2.5 (bone flag), new §11.2.6 (the drops of the weather code), §12 rules 15–17 revised and four implementer callouts answered (rule 13 note, rule 15, rule 16, plus the §11.2 ones), §13 RT-I21–RT-I23, §15 Q-RT13 narrowed | Spec writer (dirty room), package spec-world-r5; status Accepted → Revised, awaiting independent review. Answers the world-conform implementer OPENs of 2026-10-23 for this spec: the precipitation gate is the drop count ⌊3,000 × clamp(value) × *q*⌋ ≥ 1 with *q* from `AtmosphericsDetail`; the exposure-test choices and the camera-position frame are Tessera rules; the wind, rain and snow follow both environment fades; culled owners keep their fade under rule 15 and a one-frame lag is allowed; rule 16 covers the creation viewport. Q-RT13 answered for the rain drops (parameters, field, placement, fall, respawn, reference height from the water probe, streaks, colours, material) and for the snow counts, parameters and material; interim Tessera rules for snow flakes and the marked bones. Implemented behaviour changes: precipitation gate, blended weather values, rule 15 for culled owners, rule 16 in the creation viewport, rain streaks and drop spawns, figure effects on every bone with a parent. |
| 2026-10-23 | Header, §1 (OBS5, RT52), §10.2/§11 boundary, §11.2.2, §11.2.3 steps 3–4, §11.2.4 step 6 | Spec writer (dirty room); fixes from the review of the ninth revision, status stays Revised, awaiting independent review. The `s11` anchor and the separator before §11 restored. §11.2.2–§11.2.3 restated as conditions and observable results (no change of meaning): with `PrecipitationEffects` off neither update has any effect; which gust system's forces are recomputed and when; snow spawns only into the snow gust system while *s* > 1.001; the snow gust probability keeps the drop ratio of the first such spawn of the session. §11.2.4 step 6: the count corrected (eight of the ten records world-space, including torrent and blizzard; only the two impact systems local-space; OBS5 re-checked all ten keys) and what the flag means for the camera systems. |
| 2026-10-23 | Header, §1 (new BIN-N, pass (n); new OBS5; new RT49–RT57), §5.1 (fourth entry point), §5.4 (update to walker rule (d)), §10.2 (fork effects), §11 (weather bullet, Q-RT5 answered), new §11.2 and §11.3, §12 (rule 13 implementer note filed and answered; new rules 15–17), §15 (Q-RT5 closed; notes on Q-RT7, Q-RT9; new Q-RT12, Q-RT13), §16 (notes for `environment.md`, `walker.md`, `water.md`) | Spec writer (dirty room); ninth revision, status Accepted → Revised, awaiting independent review. BIN pass (n): the global wind's target is the environment state's (curve 3, curve 4, 0) vector, set every update (RT49); the precipitation step, its gates and order (RT50); the rain and snow updates' fall direction, 0.5-s sky-exposure test and spawns into the vapour, ripple and gust systems (RT51); the placement branch and thresholds of the seven precipitation systems and the impact spawn chance (RT52); the figure-in-water and rain-on-figure effects (RT53); the former "gameplay event handler" is the effect preview of the options window's effect-filter page (RT54, Q-RT5); `PrecipitationEffects` and `VideoPostEffects` start on (RT55). OBS5: flag census and node reach of `uses_video_post` and `inherit_opacity` records (RT56), fork-effect node count (RT57). Tessera rules: 15 (parent opacity of `inherit_opacity`), 16 (systems of the pass after post-processing drawn in a late particle stage), 17 (weather systems in the walker). The round-2 implementer note on lamp lights in the walker's light table filed and answered under rule 13. `particles.md` §10 rule 2 (fork effects) answered in part in §10.2: unchanged, not drawn until `particles.md` Q1. |
| 2026-10-23 | Header, §1 (new BIN-M, pass (m); new RT48), §9.3, §12 (rule 13) | Spec writer (dirty room); status Accepted → Revised, awaiting independent review. Rule 13 corrected from BIN pass (m): a light adds colour × clamped intensity × 1 / (1 + 10 × Falloff × *d*² / Range²) for point lights, 1 for the global and other non-point lights, no range cut-off, no direction term, first 5 entries; the two implementer OPENs of 2026-10-23 answered: the Q-RT3 box and the culled-but-simulated instance (§9.3) and readings (b)–(d) of rule 13 confirmed as Tessera rules, reading (a) answered by RT48. For the owner of `particles.md`: RT48 answers the attenuation part of Q8 (b). |
| 2026-10-14 | Header | Independent review of the seventh revision (commit 48a0d3b): passed, status Accepted. Ribbon builder, segment writer, draw order, system placement and the distance-fade flag re-read in an own decompilation; RT-I18 and RT-I20 recomputed. Non-blocking: a §16 note for `particles.md` §8.13 (the beam's flag switches the fade off), the §5.5 implementer OPEN. No content change (header review record and status only). |
| 2026-10-14 | Header, §1 (BIN pass (k) paraphrased, new BIN pass (l), new RT45–RT47), §11.1 (steps 5, 6, 8; new ribbon-vertices paragraph; the three implementer OPENs answered), §13 (RT-I18 note, new RT-I20) | Spec writer (dirty room); seventh revision, status Accepted → Revised, awaiting review. Non-blocking note (1) of the 2026-10-11 review: pass (k) no longer names the client's beam module by a name taken from a build-path string. Implementer OPENs of §11.1 answered from BIN pass (l) where the client shows the fact, as Decisions otherwise: ribbon side vector, width, vertex format, two texture-coordinate pairs and part order (RT45); no ribbon without a body material; the beam's systems placed every frame until removal, emission never stopped by the beam, the systems' camera-distance fade skipped, the removal test's rate (RT46); single-precision spawn count and accumulated spawn times (RT47). Step 5 notes the 2 + 2 hook-carrying nodes with head speed 0 and −25. **Implemented behaviour that changes on main:** (i) ribbon geometry and texture coordinates (one side vector per beam from the camera-to-tail direction; pair-1 *u* by segment count along the whole ribbon, not restarted per segment); (ii) beam systems skip the camera-distance fade and cut of `particles.md` §8.13; (iii) the step-8 count in single precision and accumulated spawn times; (iv) a beam without a body material draws no ribbon (no such node in this install). Unchanged: the systems' placement and the stop of their emission after the envelope (now a Decision), "zero rate", the ribbon program, draw order between beams, the frames and the head entity (Decisions recording the walker's readings). |
| 2026-10-10 | Header, §1 (BIN pass (k), OBS4, RT36–RT44), §5.1, §11, new §11.1, §12 (rules 13, 14), §13 (RT-I17–RT-I19), §15 (Q-RT4 answered, new Q-RT11) | Spec writer (dirty room, P2); sixth revision for `combat-visuals.md`, status Accepted → Revised, awaiting review. **Q-RT4 answered** from BIN pass (k): Beam node start and target, the beam envelope, endpoints and detachment, random offsets, head and tail motion, arrival and the Default port, the ribbon, beam and head particle emission, the head entity, removal; census of all 2,740 Beam nodes (OBS4). The former Decision "beams draw no particle systems" is withdrawn. New rule 13: the lights of lit systems (a Decision for `particles.md` Q8 (b)); rule 14: beam limits and node completion. Implementers add beams to `crates/particles` and the walker's effects runtime. |
| 2026-10-10 | Header, §1 (RT30 reworded, new RT35), §9.1.1 rule 3, §9.1.3 item 1, §16 | Spec writer (dirty room); fifth revision, pointers only, status Accepted → Revised, awaiting review. Note N1 of the `material.md` review of commit d96e087: `material.md` §11.2–§11.3 replaced the "opacity source" by the tested opacity of a table (one alpha, the product of two, or the sum in seven records); §9.1.3 item 1, rule 3 of §9.1.1, §16 and RT30 now quote the revised rule; the `Fixed` row of `render.md` §4.3 now has an alpha test with the caller's *R* (its "no alpha test" is no longer quoted). New RT35: none of the 19 particle shader records is a sum record, the eight two-layer records test the product, so "both layers' product" in §9.1.1 and §9.1.3 stands. No rule for particle draws changed. |
| 2026-10-10 | Header, §1 (BIN pass (j), RT33 widened, new RT34), §3.4, §13 RT-I16 | Spec writer (dirty room); fourth revision, wording only, status Accepted → Revised, awaiting review. Non-blocking notes of the review of the third revision: (b) §3.4 now states each step of a real-range draw: width in single precision, product with *u* in double precision rounded to single, lo added in single precision (it said the whole sum was formed in double and rounded once, which can differ in the last bit); new RT34 from BIN pass (j); RT-I16 points to those steps. (a) RT33 cites the BIN reading (passes (i) and (j)) that mode 3 is one real-range draw over [−1, 1] per evaluation in the client's curve evaluator, so the mapping is client behaviour; its keying, cap and preview lifetime stay Decisions. No rule changed; implementers re-check only the order of rounding in the real-range draw. |
| 2026-10-10 | Header, §1 (OBS3 widened, RT20 widened, new RT32, RT33), §9.1.1 (notation rewrapped, per-draw mode-3 note, implementer OPEN answered), §12 rule 12, §13 (new RT-I15, RT-I16), §15 Q-RT10 | Spec writer (dirty room); third revision, status Accepted → Revised, awaiting review. **Implementer OPEN of §9.1.1 (how rule 12 is applied) answered:** (a) corrected: mode 3 takes *r* directly as one real-range draw of the shared stream per curve and evaluation (§3.4, rule 9), not from a generator seeded per evaluation; a new value at every draw; (b) confirmed: states per surface DID for the life of the effects layer satisfy rule 12, no drop at effect stop or landblock unload is required, dropping when no live system draws the surface is allowed, the 4,096-surface cap is an engineering limit; (c) confirmed: preview states kept per run, each sheet column a run. **New fact** (OBS3 third probe, RT32): all five mode-10 curves of particle surfaces loop (periods 1.2 s and 4 s), so the drop point changes only a loop's phase. Q-RT10 widened by the release lifetime of a loaded surface (reviewer note (a) of the second 2026-10-10 review). Implementers re-check the mode-3 source in the walker's and the explorer's particle surface evaluation. |
| 2026-10-10 | Header, §1 (BIN pass (i), OBS3 widened, RT20 widened, new RT31), §9.1.1 (implementer OPEN answered), §12 (rule 8, new rule 12), §13 (new RT-I13, RT-I14), §15 (new Q-RT10), §16 | Spec writer (dirty room); status Accepted → Revised, awaiting review. **Implementer OPEN of §9.1.1 (curve states) answered:** mode 3 has no per-curve state (global generator); a mode-10 curve keeps its state in the curve object, and the curves a particle draw evaluates are copies held by the effect the client builds for the loaded surface record (at its first draw after loading, and on a marked rebuild), so the client keeps one state per loaded surface record, shared by all 0x39 records naming it (BIN pass (i)). The walker's per-0x39-record states differ from that; the preview's fresh states per frame freeze mode-10 curves at their start. Census (OBS3): non-constant 1043 and texture-matrix curves of particle surfaces use modes 2, 3, 4, 6, 7 and 10; two surfaces have a mode-10 curve (3 systems; one surface named by two records). New rule 12 (states per surface record DID, dropped with the surface's cache entry); Q-RT10 (other rebuild triggers) with its Decision. |
| 2026-10-10 | Header, §1 (BIN pass (h), BIN-S second pass, new OBS3; RT20 widened; new RT25–RT30), §9.1.1 (texture matrices in the notation; implementer OPEN answered), new §9.1.2, new §9.1.3, §12 (rules 10–11), §13 (RT-I10–RT-I12), §15 (Q-RT9), §16 | Spec writer (dirty room); status Accepted → Revised, awaiting review. **Implementer OPEN (a)–(e) of §9.1.1 answered:** (a), (b), (c) confirmed as Tessera rules 10 and 11; (d) replaced: the two-layer records read layer 1 at texture matrix 0 and layer 2 at texture matrix 1, which are not the identity on 182 of the 206 two-layer particle surfaces (BIN-S, OBS3); (e) the client evaluates 1043 per draw, all particle references of this install are constant, fog confirmed. **New facts:** the conditions of permutation feature bit 1 (BIN, RT25), the soft-particle factor and the heat-distortion formula of its variants (BIN-S, RT26). **New Decision §9.1.3** (asked by the task of 2026-10-10): particle draws take the alpha test and the blend's source alpha from the §9.1.1 programs, not from the mesh opacity-source rule of `render.md` §4.3, and follow `render.md` §6.10 R12 in premultiplied passes with that alpha. Implementers re-check `crates/particles` `surface.rs` and `crates/render` `particles.rs` (texture matrices of the two-layer records; per-draw 1043 for non-constant curves). |
| 2026-10-09 | Header, §1 (BIN-S, OBS2; RT21–RT24; RT20 widened), §3.2, §5.4, new §9.1.1, §9.2, §10.2, §12 (rules 7–9), §15 (Q-RT7, Q-RT8), §16 | Spec writer (dirty room); status Accepted → Revised, awaiting review. New §9.1.1 from the programs of the 19 shader records that particle surfaces use (BIN-S) and a census of those surfaces (OBS2): the alpha test reads the texture's opacity alone; the blend's source alpha is texture opacity × vertex alpha × material alpha in most records, with three exceptions; colour, doubling and fog per record; the per-draw alpha-test rules of `material.md` §11.2 with *A* = tint alpha (Inferred). §5.4: census of driven pairs (2 nodes, both on the effect DID from a session-only property) and the walker rules (a)–(g); §3.2: the walker's "drawn" rule; §9.2: the local-space frames as a Decision until `particles.md` Q8; §10.2: fork effects without the library as a Tessera rule; §12: rule 7 reworded (removal-time reading), new rule 9 (engineering limits). All implementer OPENs of 2026-10-08 and 2026-10-09 marked Answered (§9.2 in part). New OPENs Q-RT7 (owner opacity of an attached instance) and Q-RT8 (driven fields after the start), each with a Decision. |
| 2026-10-09 | §3.2, §5.4, §9.2, §12 | Implementer OPEN notes only (clean room): the walker's owners and "drawn" (§3.2), its choices for redirect, gating, driven fields, opacity, water and scales (§5.4), the corrected distance-emission note of §9.2, the two readings of rule §12.7. No fact changed. |
| 2026-10-08 | all | First version, status Review. Run-time behaviour from a third M4 pass (row BIN) and a census of all 41,354 Particle nodes (OBS): entry points, clock and random generator, the stored never-ending byte, node start gates, attached and dropped paths, water probe, redirect, attachment frames, the owner's scale vectors, stop / linger / removal, particle storage order, draw calls, fork blend-mode table; worked example; Q-RT1–Q-RT6. |
