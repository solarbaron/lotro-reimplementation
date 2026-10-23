<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Particle systems: particle-system descriptors (0x39) and fork-system descriptors (0x44)

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review by spec-review:spec-world-r5:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer, package spec-world-r5: §8.8.1 gives the drop-count gate (P59, from `particle-runtime.md` BIN-O) and the world-conform implementer OPEN is filed and answered there; Q13 points to `particle-runtime.md` §11.2.6; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-r3:2 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer: pointers only, no fact of this spec changed: the wind source of §8.8, the "when and where" of §7.1.3 and part of Q13 now point to `particle-runtime.md` §11.2 (its BIN pass (n)); §10 rule 2 (fork effects not drawn) re-examined and kept, see the note under it; awaiting independent review). Before: Accepted (2026-10-11; independent review of the P2 revision, commit 7b14f27, passed; see the review record). Before: Revised (2026-10-10, P2 spec writer: pointers only, for `combat-visuals.md`: Q8 (b) has a Tessera Decision in `particle-runtime.md` rule 13; beam-driven systems are specified in `particle-runtime.md` §11.1; §14 note; no fact of this spec changed; awaiting independent review). Before: Accepted (2026-10-10; review of the second 2026-10-10 revision (commit ba847ee) passed: the implementer OPEN of §8.6.1 on the controller-1 base fix and the implementer OPEN of 2026-10-09 answered from a fifth BIN pass; the controller-1 row of the base-fix table, §8.6.1 step 4, the controller-1 row and the rotation growth of §8.7 corrected; new rows P57–P58; PS10 fixed, new PS11). Before: Revised (2026-10-10, second revision), Accepted (2026-10-10, review of the first 2026-10-10 revision passed), Revised (2026-10-10), Accepted (2026-10-09) |
| **Last updated** | 2026-10-23 (spec-world-r5: §8.8.1 count gate, P59). Before: 2026-10-23 (pointers to `particle-runtime.md` §11.2; note under §10 rule 2) |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room sessions, AI-assisted, not the author): first review, a re-review by a third session, and a final review by a fourth session that neither wrote the spec nor made either earlier review, all 2026-10-07. The 2026-10-08 revision was reviewed by a further separate session (neither the author nor an earlier reviewer), 2026-10-08. The 2026-10-10 revision was reviewed by a further separate session (neither its author nor an earlier reviewer), 2026-10-10. The second 2026-10-10 revision (commit ba847ee) was reviewed by a further separate session (neither its author nor an earlier reviewer), 2026-10-10. |
| **Review record** | **Review 2026-10-23** of the 2026-10-23 spec-world-r5 revision (uncommitted diff of the eight owned specs against 913e1c9) by spec-review:spec-world-r5:1 (independent session), a session that wrote no part of it: **passed**, Accepted. Checklist: every changed statement is a functional fact or a labelled Tessera rule or reading; no code or pseudo-code, and the ordered steps of `particle-runtime.md` §11.2.6 describe behaviour (their order changes the output) rather than a listing; no copied prose and no identifiers beyond the install's interoperability labels (UNIQUEDB, property-dictionary and `[Render]` option names) and Tessera's own code names in implementer callouts; no game text, keys, hostnames or traffic; every new fact has a provenance row with a mark; unknowns stay OPEN (Q-RT13, Q16, the §10 and mode-10 OPENs of `environment.md`); nothing outside the research scope of decision 0034 §2. Spot checks: the executable's SHA-256 matches the BIN rows; the reviewer's own read-only M4 check (decision 0011, Ghidra 12.1.3 headless on a deleted copy of the shared project, same build) and data-section reads confirm the rain and snow parameter sets of §11.2.6 (every table value), the quality factors 0.3 and 0.6 with the built-in level 1, the count rule and its zero-count gate, the material keys 0x2B and 0x3B, the placement segment (moved by *L* / 2 along *f*, swept along −*f*, start returned without a hit), the 0.1 m reference threshold, the fall decrease, the wrap and the radial draw; the install's UNIQUEDB entries 0x2B and 0x3B (values and labels of OBS6) and the Steam prefix's `UserPreferences.ini` values (OBS-F3) match; RT-I21–RT-I23 and the `appearance.md` §5.7 worked example recompute; relative links and anchors of all eight files resolve. Not re-derived: the OBS14 and OBS7 counts, BIN13, BIN-E4 and BIN-F3 beyond their cross-references. Non-blocking notes for the next revision: (1) `particle-runtime.md` §11.2.6 step 2 ("its stored offsets move by the opposite amount"), the per-bone flag "kept with the animation state" (§11.2.5, rule 17, Q-RT13), the snow drop's "several flakes, each with an offset" and the camera's "first local axis" (step 7) lean towards the client's internal representation; restate them as behaviour (drops keep their world position; the marked bones; the camera's right axis); (2) rule 15's culled-owner case is a Tessera reading and may differ from the client if its "placed in the same frame" excludes culled parents; a later BIN pass could settle it; (3) the provenance wording of `appearance.md` BIN13 ("timed messages ... post") and `far-view.md` BIN-F3 ("the count and name list each enumerated option is registered with", "the routine that builds") is mechanism-level; prefer naming only the behaviour read; (4) `particles.md` P59 "otherwise 1" means *q* = 1 at any other level. Before: **Review 2026-10-23** of the 2026-10-23 spec-world-r3 revision (uncommitted diff against 3f0d57f) by spec-review:spec-world-r3:2 (independent session), a session that wrote no part of it: **passed**, Accepted. Checklist: every changed statement is a functional fact or a labelled Tessera rule; no code or pseudo-code and nothing that follows a decompiler listing; no copied prose and no identifiers beyond the interoperability labels of the install's data (UNIQUEDB, property-dictionary and UI-layout labels) and Tessera's own code names in implementer notes; no game text, keys, hostnames or traffic; new facts have provenance rows with marks; unknowns are OPEN callouts; nothing outside decision 0034 §2. Links and anchors of all eight changed specs resolve. Pointer-only revision: §7.1.3, §8.8, §8.8.1, §10 rule 2 note and Q13 agree with `particle-runtime.md` §11.2, RT49 and RT57; no fact of this spec changed. Before: Final review 2026-10-07: **passed**, Accepted. Review of the 2026-10-08 revision, 2026-10-08: **passed**, Accepted. Review of the 2026-10-09 revision (diff against main: header, §1 BIN third pass and P53, §8.5.2, §8.6.1, change log), 2026-10-09, by a separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **changes requested**. The new facts are functional, in plain words, with no code, copied prose or game data, and the BIN third pass names its questions (18–21) and why M1–M3 were not enough; a read-only spot-check of the second-pass listings in the dirty scratch area agrees with P53 (the age raised to 0, a key current when the age is at least its `time`, controller then channels then physics flags, no motion without a controller key). **Blocking:** (1) The new Tessera rule of §8.6.1 (a channel without a current key uses scale (1, 1), opaque white, mass 1) has no provenance entry and is not in §10, although the legend defines Decision as a §10 rule and P36 lists the §10 rules: add it to §10 as a new numbered rule (section numbers of §10.1–§10.10 kept) and widen P36 to name it, with §8.6.1 pointing to it. (2) The first sentence of §8.6.1 still says keys are visited "as the particle's age passes their `time`", which contradicts the answered rule (an equal age counts; the age is raised to 0 first): reword it to match step 2. **Non-blocking:** (a) the §8.5.2 bullet has one over-long line (wrap it); (b) the writer may check whether the key walk and the physics-flag copy run for every emitter that has keyframes or depend on emitter-level conditions computed at creation; that question predates this revision and is not a condition of acceptance. Full record in the review log (decision 0018). Writer's response, 2026-10-09: blocking (1) fixed by new rule §10.11, P36 widened to §10.1–§10.11, §8.6.1 pointing to it; blocking (2) fixed by rewording the first sentence of §8.6.1 (and the table heading) to step 2's rule; non-blocking (a) wrapped; (b) not examined in this revision, recorded as OPEN Q15. Re-review of the revised text (diff against main: header, §1 BIN third pass, P36, P53, §8.5.2, §8.6.1, §10.11, §13 Q15, change log), 2026-10-09, by a further separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3 and the two blocking items above: **passed**, Accepted. Blocking (1) is fixed: §10.11 states the Decision for a channel without a current key, §10.1–§10.10 keep their numbers, P36 names §10.1–§10.11 and §8.6.1 points to the rule. Blocking (2) is fixed: the first sentence of §8.6.1 and the table heading now match step 2 (age raised to 0, an equal age counts). Every changed statement is a functional fact in plain words with a provenance row (P53 BIN third pass, P36 Decision); no code, pseudo-code, copied prose, game data, keys or hostnames; the remaining unknown is OPEN Q15, not guessed; nothing falls under a closed gate. **Non-blocking:** (a) the second line of rule §10.11 is over-long (wrap it at the next edit); (b) the Answered callout notes that the run-time simulation's exploding direction differs from §8.5.2; implementers re-check that code against §8.5.2 (Revised-to-Accepted change). Review of the 2026-10-10 revision (commit 6dd06d3: header, §1 BIN fourth pass, legend, P36, P53 mark, P54–P56, §8.6.1, §8.7 rows 3 and 1 and the rotation growth, §10.11–§10.12, §11 PS10, §13 Q15, change log), 2026-10-10, by a separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Every changed statement is a functional fact or a numbered Tessera rule; the base-fix table and the formulas are plain words and arithmetic, not a listing; no copied prose, game data, keys or hostnames; the BIN fourth pass names its build, questions (22, 23) and why M1–M3 were not enough; each new fact has a row (P54, P55 Confirmed (BIN); P56 Confirmed OBS; §10.12 under P36 Decision); Q15 is closed with its answer and no unknown is guessed; nothing falls under a closed gate. Spot-checks, read-only on 2026-10-10: P56 reproduced exactly with the repository decoder over all 11,155 records and 32,789 emitters (6 emitters with two controller keys closer than `time_step`, 1 of them with controller 3 first; 12 controller-3 keys followed by another controller key; no shared `time`; 33 controller-3 keys, 18 with physics flag 0x8; no emitter with two controller-1/3 keys; 350; 1,151 emitters without a controller 1–4 key, all with a 0x200 key). P55 re-derived in an own headless decompilation of the routine that derives the per-particle state: the key-walk and previous-position state exist exactly when the keyframe count is not 0, the physics-flag field exists when any keyframe has controller 1–4 whatever its bits, and controllers 1 and 3 share one small per-particle index field. The base-fix motions of P54 were not re-derived. **Non-blocking:** (a) PS10 gives the first key bits 0x100 only, so the particle never receives physics flag 0x8 (that needs bit 0x200, §8.6.1) and the invariant does not exercise "whatever physics flag 0x8 says"; give the key bits 0x100 | 0x200. (b) Base-fix row for a leaving controller 1: "run once to the new key's time (its travel budget measured from the emitter's previous particle-update time)" mixes a particle age and a clock time; state the span exactly or mark it OPEN (not reached by the data). (c) Rotation growth at a base fix: say whether the world-rotation start value (§8.5 step 9) is raised too, or only the rotation of §8.12 item 3. (d) §8.6.1 step 4 lists the evaluations that fill the slot but not the controller-1 base fix; say whether that walk fills an empty slot (not reached). (e) A few new lines exceed the usual width (PS10). Review of the second 2026-10-10 revision (commit ba847ee: header, §1 BIN fifth pass, P54 widened, P57–P58, §8.6.1 step 4, the controller-1 row of the base-fix table and the paragraph after it, two callouts marked Answered, §8.7 controller-1 row and rotation growth, §11 PS10 and PS11, change log), 2026-10-10, by a separate dirty-room session (AI-assisted; neither the author nor an earlier reviewer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Every changed statement is a functional fact in plain words and arithmetic (the base-fix row is a list of effects, not a listing); no code or pseudo-code, copied prose, game data, keys or hostnames; the BIN fifth pass names its build, questions (24, 25) and why M1–M3 were not enough; each new fact has a row (P57, P58 Confirmed (BIN)); the 2026-10-10 review notes (a)–(e) are taken up; nothing is guessed and nothing falls under a closed gate. Spot-checks, read-only on 2026-10-10, in an own headless decompilation (Ghidra 12.1.3, temporary copy of the project, deleted afterwards) of the per-particle state routine, the spawn routine, the particle update, the key switch, the controller-1 walk and the point-placement routine: P57 holds (the key switch runs the controller-1 walk with the span (spawn time + the new key's `time`) minus the emitter's stored previous update time, unclamped; an empty slot takes index 0, or a random index with physics flag 0x8, without touching the key's counter, the particle is placed on base + that point and walks from there; the base becomes the result; the 0x200 flag copy runs after the controller part; the particle update runs the walk on every update under the key, and the spawn-time update passes a span of 0). P58 holds (the per-particle state holds the `rotation` triple or the `world_rotation` triple, never both, chosen as stated; growth with age and the base-fix raise write only that one). PS11 recomputed by hand from §8.7 (0.5477 m, slot 1; 9.4523 m from index 1): matches. **Non-blocking:** (a) the Status text named review notes (a)–(d) while the change log names (a)–(e) (Status rewritten by this review). Independent review 2026-10-11 of the P2 revision (commit 7b14f27 on branch p2: header, §13 Q8 note, §14 note, change log), by a separate dirty-room session (AI-assisted; neither its author nor an implementer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Pointers only, no fact of this spec changed; the targets (`particle-runtime.md` rule 13 and §11.1) exist and say what the notes state. |
| **Implements into** | `crates/formats` (decoders for prefix 0x39, §4, and the outer layer of prefix 0x44, §6), `apps/explorer` (particle preview, §8; fork records shown as metadata, §10), `crates/render` (particle pipeline, §8.10–§8.12), `apps/client` (effects started by script-table Particle and Beam nodes, weather systems of §7.1.3), `fuzz/` (both decoders) |
| **Companion specs** | [`records.md`](records.md) §1.2 (VLE), §1.3 (TSize), §1.4 (Pascal string), §2.1 (prefix table; rows 0x39 and 0x44), §7 (enum mappers), §8 (DID maps). [`property-system.md`](property-system.md) §2 (master property dictionary), §4.2 (waveform = float curve), §6.3 (embedded property sets), §9 (DB-object-type codes), §17.1 (VLE forms). [`material.md`](material.md) §4 (surface records 0x31), §5.3 (float curve layout and evaluation), §7 (shader records 0x2B). [`script-table.md`](script-table.md) §5.3 (Particle node, kind 59, and Beam node, kind 54). [`skeleton-anim.md`](skeleton-anim.md) (holding locations and bone types). [`environment.md`](environment.md) §9 (kinds of prefixes 0x39 and 0x44). [`coordinates.md`](coordinates.md) (axes: Z is up). |

---

<a id="s0"></a>
## 0. Purpose

This spec lets an implementer:

- decode every particle-system descriptor (prefix 0x39, 11,155 records) to the last byte (§3, §4);
- decode the outer layer of every fork-system descriptor (prefix 0x44, 254 records) to the last
  byte, and locate the embedded third-party effect image and the textures it names (§6);
- follow the DID graph into and out of both families (§7);
- run the original's particle simulation for 0x39 records: emission timing and limits, spawn
  shapes, spread, keyframe channels, the motion controllers, gravity and wind, the emitter fade
  envelope, texture-atlas frames and the drawn primitive (§8);
- draw them as the original does: billboard construction, distance fade, lighting, draw order and
  passes, shared instances (§8.12–§8.17);
- know which values select or tune effects in gameplay contexts, and which client options decide
  whether an effect starts at all (§7.1.2, §9).

Out of scope: the node semantics of the scripts that start effects (`script-table.md`); the
surface, material and shader programs that draw particles (`material.md`, `lighting.md`); the
internal layout and simulation of the embedded third-party effect image of 0x44 records (§6.2,
OPEN Q1). The run time around a system (starting effects from scripts, attachment to entities,
stopping, lingering and removal, the random source, particle order inside a system) is
[`particle-runtime.md`](particle-runtime.md).

Field labels. Most fields of the 0x39 format have a **property label** in the master property
dictionary 0x34000000 (`PSDesc_…` for the system, `PEDesc_…` for an emitter); the tables give
it so that an implementation can show the dictionary's name and so that the field can be
matched with the properties of the same name (interoperability). Fields without a label carry
this spec's own lower-case name. Enum labels (for example `Sphere`) are data, read at run time
from the named enum mapper. No game text appears in this spec: records are referred to by DID,
emitter names and texture names are not reproduced.

All counts are observations of one install. Numbers that define how the client computes from
the data (gravity −9.8, the 0.0002 tolerance, the 0.95 damping, the 0.25 s catch-up limit, the
0.25 s interpolation reset, the 0.3 s wind half-life, the 32,768-vertex draw buffer, the
488-byte header size, the 1 s / 100-frame restart rule, the limit of 5 lights) come from the
client executable and may be used as constants
([decision 0012](../decisions/0012-numeric-constants.md)).

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: observed in the user's install on 2026-10-07 (rows P1–P37) or 2026-10-08 (rows
  P38 and later, unless the row gives another date) by throwaway probes in the dirty scratch
  area, over **all** records of the kind (every 0x39 and 0x44 record; every script table and
  scriptlet) unless a sample is stated.
- **Confirmed (BIN)**: read from the client executable (row BIN). **Confirmed (BIN + OBS)**:
  read from the executable and agreeing with the data over all records.
- **Inferred**: follows from names or from consistent data, not traced in code.
- **Decision**: a Tessera rule (§10), not a fact about the original.
- **Reported**: published material only (none used: no published source describes these
  formats).

| Tag | Source |
|---|---|
| OBS | User install (Steam), data current on 2026-10-07, read in place and read-only. Decoded in full: all 11,155 records with prefix 0x39 and all 254 with prefix 0x44 (`client_general.dat`); all 7,526 script tables (7,524 decodable) and 28,992 scriptlets of `client_gamelogic.dat` (every Particle and Beam node); the master property dictionary 0x34000000; enum mappers 0x230000AC, 0x230000AD, 0x230000AE, 0x23000068, 0x23000086; DID map 0x28000002 (UNIQUEDB); property set 0x780060C9; a DID-existence check of every referenced DID against all 29 archives of the install. |
| BIN | **M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), analysed with Ghidra 12.1.3 in headless mode on 2026-10-06/07 in the dirty room. **Questions:** (1) the field order and encodings of the 0x39 deserialiser and of its emitter and keyframe parts (the format has no tags, no lengths and no own DID, and its curves, booleans and counts are variable or unaligned, so field boundaries cannot be found from data alone); (2) the outer deserialiser of DB type 0x56 (0x44 records) and its version-dependent parts; (3) the client's particle runtime: emission, spawn shapes, spread, keyframe channels, motion controllers, forces, fade envelope, texture frames, primitive vertex count, the end-of-effect test; (4) defaults of a new emitter and keyframe; (5) how a fork effect resolves the DIDs it names. **Why M1–M3 were not enough:** (1) and (2) need field boundaries that the data does not mark; (3)–(5) exist only as code; no published source describes them (M2); no running client and no frame capture is available to this session (M3). Only the offline client's format and rendering code was read; nothing of the network, login, launcher or anti-tamper code (decision 0004). Facts resting on BIN alone are marked so. **Second pass, 2026-10-08** (same build, Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards), for the OPEN questions of the Accepted text: (6) the time arguments of the catch-up `lifespan` and of the draw-time `pivot` / `origin_offset` evaluations; (7) `scale_mode` and the owner's scale; (8) what the client does with `force_draw`, `uses_video_post`, `draw_reflection`, `use_lighting`, `inherit_opacity`, `sorting_priority`, `shareable` and the fade distances; (9) `fast_forward` and `physics_duration`; (10) keyframe bits 0x80/0x400 and physics bits 0x02/0x04; (11) billboard construction and orientation; (12) emitter start state, emission scheduling and the distance-emission anchors; (13) the dynamic-particle whitelist and the client options that gate effect starts; (14) the trailing DID of 0x39 records; (15) a Particle node whose particle field names a 0x44 record; (16) the force-field list and the controller-1 start index; (17) the 0x44 `flag`, `tail_value` and `transform`. Same reasons: all of these exist only as code; the data holds parameters only; no capture is available. **Third pass, 2026-10-09** (same build; the decompiled listings of the particle update, keyframe-switch, spawn and vertex-writer routines made in the second pass, kept in the dirty-room scratch area, re-read): (18) when a keyframe becomes current, at a particle's creation and when the age equals a key's `time`; (19) the order of a key's effects when it carries several bits; (20) the channel values while no key is current; (21) which shape point an exploding emitter's radial direction uses. Same reasons: these are code paths, and the data never reaches the edge cases (every first key has `time` 0). **Fourth pass, 2026-10-10** (same build, Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards; the second-pass listings of the key-switch, controller and particle-update routines re-read, and the routine that derives an emitter's per-particle state from its keyframes decompiled): (22) which point the base fix of a controller-3 key uses when the particle never ran that controller, and when the per-particle point index is filled and emptied; (23) whether the key walk and the physics-flag copy depend on emitter-level conditions computed at creation (Q15). Same reasons: these are code paths that the data reaches rarely (22) or not observably (23). **Fifth pass, 2026-10-10** (same build, Ghidra 12.1.3 headless on a temporary copy of the analysis project, deleted afterwards; the key-switch, controller-1, controller-3 placement, particle-update, spawn, emitter-update and vertex-writer routines decompiled again and compared with the second-pass listings, which they match, and the routine that derives an emitter's per-particle state re-read): (24) the time span of the base fix of a leaving controller-1 key, and whether it fills an empty point-index slot; (25) which of the `rotation` and `world_rotation` angles the client keeps per particle, grows and raises at a base fix. Same reasons: (24) is a code path that the data never reaches (no key with bit 0x100 has controller 1), (25) one whose result the data cannot show. |
| PS | [`property-system.md`](property-system.md): dictionary layout (§2), waveform = float curve (§4.2), embedded property sets (§6.3), DB-object-type codes and ranges (§9), VLE forms (§17.1). |
| MA | [`material.md`](material.md) §5.3: float-curve layout and evaluation (shared routine, confirmed again here by BIN). |
| ST | [`script-table.md`](script-table.md) §5.3: Particle node (kind 59) and Beam node (kind 54) fields; node counts. |
| REC | [`records.md`](records.md): VLE (§1.2), TSize (§1.3), Pascal string (§1.4), enum mapper (§7), DID map (§8). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| P1 | Prefix 0x39 = DB type 0x1E (PSDESC), prefix 0x44 = DB type 0x56 (FORKPSDESC); neither record starts with its own DID (§2.1) | PS §9; BIN (type registry, factories) | Confirmed (BIN + OBS) |
| P2 | 0x39 record layout: system part, emitter list, bounds, priority, surface, trailing DID (§4.1) | BIN (field order and encodings); OBS (11,155 of 11,155 records decode with zero leftover bytes) | Confirmed (BIN + OBS) |
| P3 | Emitter layout, 41 fields (§4.2) | BIN; OBS (32,789 emitters) | Confirmed (BIN + OBS) |
| P4 | Keyframe layout, 10 fields (§4.3) | BIN; OBS (121,087 keyframes) | Confirmed (BIN + OBS) |
| P5 | Fields are packed with no alignment and no padding; bool8 is one byte 0 or 1 (§3.1) | BIN; OBS | Confirmed (BIN + OBS) |
| P6 | Every curve field is the float curve of `material.md` §5.3; a vector field is three curves X, Y, Z (§3.2, §3.3) | BIN (the same deserialiser and evaluator); MA; OBS (1,314,142 curves, modes 1–10 only) | Confirmed (BIN + OBS) |
| P7 | Colour: u32 with A in bits 24–31, R 16–23, G 8–15, B 0–7; each channel / 255 (§3.4) | BIN | Confirmed (BIN) |
| P8 | Property labels and enum mappers of the fields (§4.5) | OBS (dictionary 0x34000000); BIN (each registered field names its PID) | Confirmed (BIN + OBS) |
| P9 | Enum labels: shape (0x230000AC), primitive (0x230000AD), scale mode (0x230000AE) (§4.6) | OBS | Confirmed |
| P10 | 0x39 censuses (§5) | OBS | Confirmed |
| P11 | 0x44 outer layout: blob, flag, name, name table, property set, transform, resource table, version-3 tail (§6.1) | BIN; OBS (254 of 254 decode with zero leftover bytes) | Confirmed (BIN + OBS) |
| P12 | Blob framing facts: words 0 and 1, 488-byte header, string block (§6.2) | OBS (254 of 254); BIN (loader size checks) | Confirmed for the stated facts; internal layout OPEN (Q1) |
| P13 | Fork resource resolution: render surfaces, shader records, nested fork descriptors; preload dependencies (§6.4) | BIN; OBS | Confirmed (BIN + OBS) |
| P14 | Fork effects draw with seven shader records taken from UNIQUEDB keys 0x159–0x15F (§6.4) | BIN (keys and DB type); OBS (map entries) | Confirmed (BIN + OBS) |
| P15 | References into 0x39/0x44 from script nodes, the dynamic-particle whitelist and UNIQUEDB (§7.1) | OBS | Confirmed |
| P16 | References out of 0x39 (surface records) and 0x44 (render surfaces, nested 0x44) (§7.2) | OBS; BIN (DB types requested) | Confirmed (BIN + OBS) |
| P17 | Update cadence: particles are updated once at least `time_step` seconds have passed; behaviour with `time_step` ≤ 0 or not a number (§8.1) | BIN | Confirmed (BIN) |
| P18 | Emitter activity window and end-of-effect test, including `burst_count` = 0 (§8.2) | BIN | Confirmed (BIN) |
| P19 | Timed emission: bursts of `burst_count` every `emit_interval` seconds, caps, catch-up; no timed emission when `emit_interval` = 0; nothing emitted when `burst_count` = 0; 32-bit wrapping running total, at most cap passes per update, and the behaviour with a negative, too small or not-a-number `emit_interval` (§8.3) | BIN (the interval test, the burst test, the catch-up arithmetic, the unsigned total); OBS (615 emitters with interval 0, 507 of them with distance emission; all others finite, 0.000263 to 100,000, none negative) | Confirmed (BIN + OBS) |
| P20 | Distance emission, including `burst_count` = 0, the two anchors, the step and spawn-time arithmetic and the loop bounds (§8.4) | BIN (first pass; anchors, times and bounds second pass) | Confirmed (BIN) |
| P21 | Spawn: shapes, direction, exploding, spread, speed (§8.5) | BIN | Confirmed (BIN) |
| P22 | Keyframe channels, flag bits 0x2/0x4/0x8/0x10/0x20/0x40/0x100/0x200, key selection by particle age, the per-key interpolation targets computed when an emitter is created and the target tests (§8.6.1) | BIN; OBS (flag census) | Confirmed (BIN + OBS) |
| P23 | Motion controllers 2, 3, 5, 6 as used by the data; 1 (point path, speed = length of the spawn velocity) and 4 (stepped integration with force fields; a force-field lifetime below 0.0002, negative values included, never runs out) present in code and used by no controller key (§8.7) | BIN; OBS (controller census) | Confirmed (BIN + OBS); the force-field list is never filled and the controller-1 start index range are in P47 |
| P24 | Forces: gravity (0, 0, −9.8) and the wind vector, each scaled by the mass channel; the wind vector eases towards its target with a 0.3 s half-life, starts at zero, and takes its target from the environment update. Each emitter stores both vectors, computed at system start; the rain precipitation update recomputes them for the UNIQUEDB 0x83 system and the snow update for the 0xFE system (snow value above 1.001), once per run; no other system is recomputed; particle updates read the stored vectors, so live particles follow a recomputation (§8.8, §8.8.1) | BIN (the start routine, the recompute routine and its only two callers, the force terms of controllers 2 and 4) | Confirmed (BIN); environment wind source and the weather schedule OPEN (Q13) |
| P25 | Emitter fade envelope from `fade_in`, `fade_out`, `time_limit`, `fast_forward` (§8.9) | BIN | Confirmed (BIN) |
| P26 | Texture-atlas frame selection (§8.10) | BIN | Confirmed (BIN) |
| P27 | Primitive: 3 vertices for `Triangle`, 4 for `Quad`; one shared draw buffer of 32,768 vertices per system draw, `Quad` emitters first, then `Triangle` emitters, drawing stops when the buffer is full (§8.11) | BIN (vertex counts, buffer creation size, draw order and stop rule) | Confirmed (BIN) |
| P28 | Constructor defaults of an emitter and of a keyframe (§4.7) | BIN; OBS (most common values agree) | Confirmed (BIN) |
| P29 | `PSDesc_FrameRate` and `PEDesc_ParticlesPerSec` are views (reciprocals) of `time_step` and `emit_interval`, not stored fields (§4.5) | BIN | Confirmed (BIN) |
| P30 | Weather and water systems in UNIQUEDB (§7.1.3); the weather code creates one instance of each at initialisation | OBS; BIN (the weather initialisation reads all ten keys) | Confirmed (BIN + OBS) |
| P31 | Record counts, DID ranges, payload byte totals, size ranges, and the compression flag clear in every directory entry of both prefixes (§2.1) | OBS (directory entries and payloads of all 11,155 + 254 records) | Confirmed |
| P32 | Reachability of 0x44 records: 38 named by other 0x44 records, 35 only that way, all 254 reachable (§6.3) | OBS (all `resources` tables and all Particle nodes) | Confirmed |
| P33 | Drawn size (scale channel × the owner-scale factor of §8.6.3 × `particle_scale` when that curve is not the constant 1) and drawn colour (colour channel, alpha × fade envelope, each channel clamped to [0, 1]) (§8.6.2) | BIN | Confirmed (BIN) |
| P34 | `pivot` and `origin_offset`: evaluated at the draw clock, axis use in the camera-aligned frame (§8.12) | BIN (first pass; time and axes second pass) | Confirmed (BIN) |
| P35 | No field of either family carries a gameplay value; gameplay reaches the records only by selection through script nodes, the whitelist and UNIQUEDB (§9) | OBS and BIN (the complete field lists of §4 and §6.1); ST (node fields); §7 | Inferred from the confirmed layouts and references |
| P36 | Tessera rules: complete decoding, opaque fork images, curve and random evaluation, Particle-node record choice, invalid `time_step`, `burst_count` = 0, controller 4 without force fields, bounded timed and distance emission with hostile values, the two fields without effect, the values of a keyframe channel without a current key, a point index beyond a controller-3 key's points (§10.1–§10.12) | This spec | Decision |
| P37 | `snap` values and the other emitter censuses (§4.7, §5.2), last-key time against `lifespan` (§5.3) | OBS (all 32,789 emitters and 121,087 keyframes) | Confirmed |
| P38 | System start state: start = first placement time − `fast_forward`; each emitter's last particle-update, last-burst and last distance-emission times start at system start + `start_time` (§8.3, §8.17) | BIN (second pass) | Confirmed (BIN) |
| P39 | Catch-up `lifespan` evaluated at the last-burst time on the client clock (§8.3) | BIN (second pass: the register holding the time at the call) | Confirmed (BIN) |
| P40 | System update once per frame on the first placement, gated by `physics_duration` and by the not-drawn state; render interpolation of positions; new particles advanced to the emitter's last particle-update time (§8.1, §8.5) | BIN (second pass) | Confirmed (BIN) |
| P41 | `scale_mode` selects which of the owner's two scale vectors apply; world-space and local-space use (§8.6.3) | BIN (second pass); OBS (`scale_mode` census, §4.6) | Confirmed (BIN); source of the two vectors OPEN (Q4) |
| P42 | Billboard: camera-aligned frame, corners, `pivot` and `origin_offset` axes, `rotation` / `world_rotation` / `rotation_velocity` selection, `velocity_facing`, `streak` (§8.12) | BIN (second pass) | Confirmed (BIN) |
| P43 | Distance fade from the four fade fields; not drawn beyond the far end or inside the near start; such systems are not updated until drawn again (§8.13) | BIN (second pass) | Confirmed (BIN) |
| P44 | `use_lighting`: ambient colour plus up to 5 point lights multiply the particle colour; lit systems are never shared (§8.14, §8.16) | BIN (second pass) | Confirmed (BIN); light attenuation formula and the source of the ambient colour OPEN (Q8) |
| P45 | Draw order by `sorting_priority` and the two passes selected by `uses_video_post`; `inherit_opacity`; `force_draw` and `draw_reflection` have no reader (§8.15) | BIN (second pass: the sort, both pass calls, the opacity routine, a scan of every one-byte read at the two offsets) | Confirmed (BIN) |
| P46 | Sharing of running systems (`shareable`, `world_space` = 0, `use_lighting` = 0) and per-frame draw instances (§8.16) | BIN (second pass); OBS (2,583 records meet the condition) | Confirmed (BIN + OBS) |
| P47 | The client's force-field list is never filled (only read and freed); controller 1 random start index uniform in [0, point count − 1]; keyframe bits 0x80 and 0x400 and physics bits 0x02 and 0x04 are read by no runtime routine (§8.6.1, §8.7, §8.8) | BIN (second pass: every reference to the list; the random call's arguments; the key-switch, target-builder, spawn, update and controller routines) | Confirmed (BIN) |
| P48 | Effect start gating: the whitelist (UNIQUEDB key 0x1E2), the `DynamicParticles` option (values 0–4, default 0) and the static-environment option (default on) (§7.1.2) | BIN (second pass); OBS (UNIQUEDB key 0x1E2 → 0x780060C9) | Confirmed (BIN + OBS) |
| P49 | A Particle node's effect DID is `ParticleScript_ParticleID` when non-zero, else `ParticleScript_ForkParticleID`; the DID's DB type picks a particle system or a fork system; when both are non-zero on load the fork field is cleared (§7.1.1) | BIN (second pass) | Confirmed (BIN) |
| P50 | The trailing DID of a 0x39 record is the saved form of the loaded-surface slot; the post-load step overwrites the slot from `surface` when `surface` ≠ 0; a system without a loaded surface is not drawn (§4.1, §8.11) | BIN (second pass) | Confirmed (BIN) |
| P51 | 0x44 `flag` = effect loops until stopped; `tail_value` = lifetime window in seconds; `transform` composed with the owner frame; the Particle node's fade settings apply only to fork effects (§6.1, §6.4) | BIN (second pass) | Confirmed (BIN); bounds and version source OPEN (Q2) |
| P52 | `fast_forward` > 1 s restarts a system that was not placed for more than 100 frames; `physics_duration` stops updates after that many seconds and marks the effect as never-ending for the Particle node (§8.17) | BIN (second pass); OBS (836 records with `fast_forward` > 1; `physics_duration` 0 in all) | Confirmed (BIN + OBS) |
| P53 | Keyframe visiting: no key is current at creation; at every particle update (the first one right after the spawn included) each later key in stored order becomes current when the age, raised to 0 if negative, is at least its `time`; a key applies its controller (0x100) first, then its channel bits, then its physics flags (0x200); the random point index of controllers 1 and 3 is drawn later, at the first particle update under that controller; without a controller key a particle does not move; an exploding emitter's radial direction uses the shape point after the attachment of §8.5.5 and before `origin` (§8.5.2, §8.6.1) | BIN (third pass) | Confirmed (BIN); the point-index timing refined by P54 (one slot per particle, filled once) |
| P54 | One point-index slot per particle, shared by controllers 1 and 3: empty at creation, filled once by the first controller-1 or controller-3 evaluation (a particle update under such a key, or the base fix when leaving a controller-3 key; the base fix when leaving a controller-1 key fills it too, P57), never emptied; the base fix of a controller-3 key whose slot is still empty takes that key's counter and advances it by one, whatever physics flag 0x8 says; with a filled slot it uses the slot; with no points the base does not move; controller 3 uses the slot without reducing it modulo the key's point count, controller 1 reduces it; the base-fix motion of each leaving controller (2 with the stored mass of the current mass key, 5, 3, 1; none for the others); at every base fix the starting value of the rotation growth rises by the new key's `time` × `rotation_velocity` (§8.6.1, §8.7) | BIN (fourth pass: the key-switch routine, the controller-1 and controller-3 routines, the particle update, the spawn routine) | Confirmed (BIN) |
| P55 | The key walk (and the previous-position history of §8.1) runs for an emitter exactly when it has at least one keyframe; the per-particle physics flags, and with them the 0x200 copy, exist only when at least one keyframe of the emitter, whatever its bits, has `controller` 1, 2, 3 or 4; nothing else reads physics flags (§8.6.1) | BIN (fourth pass: the routine that derives the emitter's per-particle state at creation, the particle update, the key switch, every controller) | Confirmed (BIN) |
| P56 | Census of controller keys: in 6 emitters two successive controller keys are closer in `time` than the system's `time_step` (in 1 of them the first has controller 3); 12 controller-3 keys are followed by another controller key; no two controller keys of an emitter share a `time`; 33 controller-3 keys, 18 of them with physics flag 0x8; no emitter has two controller keys with controller 1 or 3; 350 emitters whose `rotation_velocity` is not constant 0 have two or more controller keys; 1,151 emitters have no keyframe with `controller` 1–4, and each of them has a key with bit 0x200 (§8.6.1) | OBS (all 32,789 emitters, 2026-10-10) | Confirmed (2026-10-10) |
| P57 | Base fix of a leaving controller-1 key: the point-path walk runs once under that key, from the particle's current position, with the budget *s* × ((spawn time + the new key's `time`) − the emitter's previous particle-update time), that time being stored only after all particles of the update; the span is not clamped; an empty point-index slot is filled first, as at a controller-1 update (index 0, or a uniform index in [0, point count − 1] when physics flag 0x8 is set), with no key counter changed, and the particle is placed on that point; the result becomes the particle's position and the new base; with no points the base stays. Every controller-1 update, the first under the key included, walks its budget (now − previous particle-update time) after any placement (§8.6.1, §8.7) | BIN (fifth pass: the key-switch routine, the controller-1 and placement routines, the particle update, the emitter update) | Confirmed (BIN) |
| P58 | The client keeps one angle triple per particle: `world_rotation` when some component of it is not constant 0, otherwise `rotation` when some component of `rotation` or `rotation_velocity` is not constant 0, otherwise none; the growth with age and the base-fix raise of §8.7 apply to that one only, and it is the one §8.12 item 3 draws (§8.7) | BIN (fifth pass: the per-particle state routine, the spawn routine, the particle update, the key switch, the vertex writer's rotation choice) | Confirmed (BIN) |
| P59 | A precipitation update runs only while its drop set uses at least one drop: *n* = ⌊3,000 × clamp(value, 0, 1) × *q*⌋ ≥ 1, *q* = 0.3, 0.6 or 1 by the `[Render]` option `AtmosphericsDetail` (levels 0 `Low`, 1 `Medium`, otherwise 1; built-in `Medium`); with *n* = 0 it does not recompute its system's forces (§8.8.1) | `particle-runtime.md` BIN-O, RT58 | BIN |

---

<a id="s2"></a>
## 2. The families at a glance

<a id="s2-1"></a>
### 2.1 Prefixes, DB types and counts

| Prefix | Archive | DB type (label) | Records | Payload bytes | Size range | Own DID at start |
|---|---|---|---|---|---|---|
| 0x39 | general | 0x1E (PSDESC) | 11,155 (DIDs 0x39000000–0x3900387D, with gaps) | 24,164,578 | 77–45,677 | No: the first field is a float (§4.1) |
| 0x44 | general | 0x56 (FORKPSDESC) | 254 (DIDs 0x4400002F–0x44000229, with gaps) | 3,187,797 | 1,201–87,951 | No: the first field is a blob length (§6.1) |

All records of both prefixes are stored uncompressed (the directory entry's compression flag
is clear in all 11,409, `dat-container.md`). Both are DB objects of the client's object
system: the 0x39 decoder is a generic field-list reader driven by a registered field list
(BIN); the 0x44 decoder is a fixed reader (BIN). The field order of §4 is the registration
order.

<a id="s2-2"></a>
### 2.2 DID graph (summary; details in §7)

| From | Field | To | Count (OBS) |
|---|---|---|---|
| Script table / scriptlet, Particle node (kind 59) | `ParticleScript_ParticleID` | 0x39 | 39,830 nodes (11,041 distinct records) |
| Particle node | `ParticleScript_ParticleID` | 0x44 | 7 nodes, all 0x44000091 (started as a fork effect, §7.1.1) |
| Particle node | `ParticleScript_ForkParticleID` | 0x44 | 1,517 nodes (219 distinct) |
| Beam node (kind 54) | `BeamScript_BeamParticleSystem` | 0x39 | 571 nodes (91 distinct) |
| Beam node | `BeamScript_HeadParticleSystem` | 0x39 | 45 nodes (9 distinct) |
| Property set 0x780060C9 | `Render_DynamicParticleWhitelistEntry` | 0x39 | 272 (272 distinct) |
| DID map 0x28000002 (UNIQUEDB) | 10 labelled keys | 0x39 | 10 |
| 0x39 | `PSDesc_Material` (`surface`) | 0x31 surface record | 11,153 (2 records hold 0) |
| 0x39 | trailing DID (`extra_surface`) | (DB type 0x36, surface) | 0 in all 11,155 (§4.1) |
| 0x44 | resource table | 0x41 render surface | 254 entries (all exist, in the surface archives) |
| 0x44 | resource table | 0x44 | 74 entries (38 distinct, all exist) |
| UNIQUEDB | keys 0x159–0x15F | 0x2B shader record | 7 (used by fork effects, §6.4) |
| UNIQUEDB | key 0x1E2 | property set 0x780060C9 (the whitelist, §7.1.2) | 1 |

Every Particle node names **exactly one** of the two descriptor fields: 39,837 nodes name only
`ParticleScript_ParticleID` and 1,517 only `ParticleScript_ForkParticleID` (41,354 Particle
nodes in all, agreeing with ST).

---

<a id="s3"></a>
## 3. Shared primitives

<a id="s3-1"></a>
### 3.1 Scalars

All multi-byte values are little-endian. Fields follow each other **without alignment or
padding** (a float may start at any byte offset).

| Name | Size | Meaning |
|---|---|---|
| f32 | 4 | IEEE-754 single |
| f64 | 8 | IEEE-754 double (0x44 tail only) |
| u32 / i32 | 4 | Unsigned / signed integer |
| bool8 | 1 | 0 or 1. The client's reader fails the record when the byte is greater than 1 (BIN); no such byte occurs (OBS). |
| VLE | 1, 2 or 4 | Count or length, `records.md` §1.2 with the forms of `property-system.md` §17.1 (the particle reader has no 0xE0 escape, BIN). Every VLE in both families uses the 1-byte form (OBS). |
| Pascal string | VLE + bytes | `records.md` §1.4: VLE byte length, then that many Latin-1 bytes, no terminator |
| TSize | 1 + VLE | `records.md` §1.3: hint byte (0–22), then a VLE count (0x44 tables only) |
| DID | 4 | u32 data ID |

<a id="s3-2"></a>
### 3.2 Float curve (`curve`)

Every field typed `curve` is the float curve of `material.md` §5.3 (`property-system.md` §4.2
calls it a waveform): a u32 `mode`; mode 1 carries one f32; every other mode carries ten f32
`w[0..9]`; mode 10 then adds f32 `period`, bool8 `loop` and a u32 count *k* of (f32 value,
f32 position) pairs. The particle reader is the same routine (BIN) and rejects a `loop` byte
above 1. Evaluation is `material.md` §5.3.1; the time argument used for each particle curve is
stated in §8.

Mode census over all 1,314,142 curves of all 0x39 records (OBS): 1 (1,189,510), 2 (7,247),
3 (95,676), 4 (12,892), 5 (71), 6 (653), 7 (5,050), 8 (2,992), 10 (51). Modes 0, 9 and above 10
do not occur. The 51 mode-10 curves have 2 (5), 3 (39) or 4 (7) pairs and `loop` 0 or 1.

<a id="s3-3"></a>
### 3.3 Curve vector (`vec`)

Three `curve` values in the order X, Y, Z (24 bytes when all three are mode 1). Each component
has its own dictionary label (`…X`, `…Y`, `…Z`).

<a id="s3-4"></a>
### 3.4 Colour (`argb`)

A u32: alpha in bits 24–31, red 16–23, green 8–15, blue 0–7. The client turns each channel into
a float by dividing by 255 (BIN). Example: 0xFFFF8000 is opaque orange (1.0, 0.502, 0.0).

<a id="s3-5"></a>
### 3.5 Plain vector (`vec3f`)

Three f32 X, Y, Z (12 bytes), used for bounds.

---

<a id="s4"></a>
## 4. Particle-system descriptor (prefix 0x39)

<a id="s4-1"></a>
### 4.1 Record layout

The record is the system part, then the emitter list, then the tail. Offsets are fixed up to
and including `emitter_count` (offset 38) and relative after it. The emitter list starts at
offset 39 only while `emitter_count` uses the 1-byte VLE form (values 0–127, the form of every
record in this install, PS8); with the 2-byte form it starts at 40 and with the 4-byte form at
42 (§3.1, §4.4).

| Offset | Size | Type | Field (own name) | Property label | Meaning, units |
|---|---|---|---|---|---|
| 0 | 4 | f32 | `time_step` | — (the view `PSDesc_FrameRate` = 1 / `time_step`) | Update interval of the system, seconds (§8.1) |
| 4 | 4 | f32 | `fast_forward` | `PSDesc_FastForward` | Seconds the system is started in the past (pre-simulation, §8.17); start of the fade envelope (§8.9) |
| 8 | 4 | f32 | `physics_duration` | `PSDesc_PhysicsDuration` | Seconds after system start during which the system is updated; 0 = always (§8.17); 0 in every record (OBS) |
| 12 | 1 | bool8 | `world_space` | `PSDesc_WorldSpace` | 1: particles keep world positions and are drawn once; 0: the particle set is drawn through each draw instance's transform and may be shared (§8.6.3, §8.16); attachments need 1 (§8.5.5). Exact frames: Q8 |
| 13 | 1 | bool8 | `force_draw` | `PSDesc_ForceDraw` | No reader in the client's simulation or drawing (§8.15) |
| 14 | 1 | bool8 | `uses_video_post` | `PSDesc_UsesVideoPost` | Draw in the pass after post-processing instead of the main pass (§8.15) |
| 15 | 1 | bool8 | `draw_reflection` | `PSDesc_DrawReflection` | No reader in the client's simulation or drawing (§8.15) |
| 16 | 1 | bool8 | `use_lighting` | `PSDesc_UseLighting` | Particle colours are lit (§8.14); a lit system is never shared (§8.16) |
| 17 | 1 | bool8 | `inherit_opacity` | `PSDesc_InheritOpacity` | Multiply by the parent object's opacity (§8.15) |
| 18 | 4 | f32 | `far_fade_start` | `PSDesc_StartFadeDist` | Metres from the camera (§8.13) |
| 22 | 4 | f32 | `far_fade_end` | `PSDesc_StopFadeDist` | Metres; not drawn beyond (§8.13) |
| 26 | 4 | f32 | `near_fade_start` | `PSDesc_StartNearFadeDist` | Metres; when > 0, not drawn at or inside (§8.13) |
| 30 | 4 | f32 | `near_fade_end` | `PSDesc_StopNearFadeDist` | Metres (§8.13) |
| 34 | 4 | u32 | `scale_mode` | `PSDesc_ParticleScale` (enum 0x230000AE) | Which of the owner's scale vectors apply (§8.6.3) |
| 38 | VLE | VLE | `emitter_count` *n* | — | Number of emitters, 0–64 observed |
| 39 (1-byte VLE) | var | emitter × *n* | `emitters` | — | §4.2 |
| +0 | 1 | bool8 | `has_bounds` | — | 1 when `bounds_min`/`bounds_max` are valid |
| +1 | 12 | vec3f | `bounds_min` | — | Bounding box of the effect in the system frame (Inferred), metres |
| +13 | 12 | vec3f | `bounds_max` | — | |
| +25 | 4 | i32 | `sorting_priority` | `PSDesc_SortingPriority` | 0–57 observed; draw order, higher values first (§8.15) |
| +29 | 1 | bool8 | `shareable` | `PSDesc_CanBeShared` | A running copy may serve later requests for the same record (§8.16) |
| +30 | 4 | DID | `surface` | `PSDesc_Material` (type 20, DB type 0x36) | Surface record (prefix 0x31) the particles are drawn with (`material.md` §4), or 0 |
| +34 | 4 | DID | `extra_surface` | — | Saved form of the loaded-surface slot: when non-zero the reader loads that DB type 0x36 surface record into the slot; the post-load step then replaces the slot with `surface` whenever `surface` ≠ 0 (BIN). So it matters only when `surface` = 0. 0 in every record (OBS) |
| +38 | — | — | *(end)* | | Zero leftover bytes in every record |

The smallest possible record (no emitters) is 77 bytes; one record has that form (0x39000E39).

<a id="s4-2"></a>
### 4.2 Emitter

An emitter is 41 fields in this order, packed (§3.1). Offsets are relative to the start of the
emitter and hold when every curve before the field is mode 1 (8 bytes); otherwise they shift.

| Rel. offset (all mode 1) | Type | Field (own name) | Property label | Meaning, units |
|---|---|---|---|---|
| 0 | vec | `origin` | `PEDesc_OriginX/Y/Z` | Spawn offset added to the shape point, metres, emitter frame (§8.5) |
| 24 | vec | `origin_offset` | `PEDesc_OriginOffsetX/Y/Z` | Offset added to every vertex at draw time (§8.12) |
| 48 | vec | `pivot` | `PEDesc_PivotOriginX/Y/Z` | Pivot of the drawn primitive (§8.12) |
| 72 | u32 | `shape` | `PEDesc_Shape` (enum 0x230000AC) | Spawn shape (§4.6, §8.5.1) |
| 76 | u32 | `primitive` | `PEDesc_ParticleShape` (enum 0x230000AD) | Drawn primitive (§4.6, §8.11) |
| 80 | f32 | `emit_interval` | — (the view `PEDesc_ParticlesPerSec` = 1 / `emit_interval`, 0 when the interval is 0) | Seconds between bursts (§8.3) |
| 84 | curve | `speed` | `PEDesc_Velocity` | Initial speed along the spawn direction, metres per second (§8.5.4) |
| 92 | vec | `direction` | `PEDesc_DirectionX/Y/Z` | Spawn direction before spread, emitter frame; normalised by the client (§8.5.2) |
| 116 | u32 | `emission_limit` | `PEDesc_EmissionLimit` | Total particles the emitter may emit; 0 = unlimited (§8.3) |
| 120 | curve | `lifespan` | `PEDesc_LifeSpan` | Particle lifetime, seconds (§8.6) |
| 128 | curve | `shape_size` | `PEDesc_Scale` | Size of the spawn shape, metres (§8.5.1) |
| 136 | curve | `min_spread` | `PEDesc_MinSpread` | Lower spread angle, degrees (§8.5.3) |
| 144 | curve | `max_spread` | `PEDesc_MaxSpread` | Upper spread angle, degrees |
| 152 | curve | `particle_scale` | `PEDesc_ParticleScale` | Per-particle size factor drawn at spawn (§8.12) |
| 160 | vec | `rotation` | `PEDesc_RotationX/Y/Z` | Camera-relative rotation of the primitive, radians, drawn at spawn; used only while `world_rotation` is constant 0 (§8.12) |
| 184 | u32 | `max_particles` | `PEDesc_MaxParticles` | Live-particle cap (§8.3) |
| 188 | bool8 | `exploding` | `PEDesc_Exploding` | Direction radial from the emitter (§8.5.2) |
| 189 | bool8 | `lock_world_rotation` | `PEDesc_LockWorldSpaceRotation` | With `world_space` = 1: orient by the particle's own frame captured at spawn instead of the system's current orientation (§8.12) |
| 190 | curve | `streak` | `PEDesc_Streak` | Per-particle trail length in seconds, drawn at spawn (§8.12) |
| 198 | vec | `world_rotation` | `PEDesc_WorldRotationX/Y/Z` | Rotation of the primitive in the system frame, radians, drawn at spawn; when not constant 0 it replaces camera facing (§8.12) |
| 222 | vec | `rotation_velocity` | `PEDesc_RotationVelocityX/Y/Z` | Per-particle angular velocity drawn at spawn; rotation grows linearly with age (§8.12) |
| 246 | u32 | `burst_count` | `PEDesc_BlastCount` | Particles per burst (§8.3) |
| 250 | f32 | `start_time` | `PEDesc_StartTime` | Seconds after the system starts before the emitter is active (§8.2) |
| 254 | f32 | `time_limit` | `PEDesc_TimeLimit` | Length of the active window, seconds; 0 = unlimited (§8.2) |
| 258 | f32 | `emission_distance` | `PEDesc_EmissionDistance` | Metres of travel per extra burst; 0 = off (§8.4) |
| 262 | curve | `snap` | `PEDesc_ParticleSnap` | Pull towards the anchor, controller 4 only (§8.7) |
| 270 | bool8 | `inclusive` | `PEDesc_Inclusive` | Fill the shape (1) or use its outline (0) (§8.5.1) |
| 271 | bool8 | `active` | — | The emitter takes part at all (§8.2) |
| 272 | f32 | `fade_in` | `PEDesc_FadeIn` | Seconds (§8.9) |
| 276 | f32 | `fade_out` | `PEDesc_FadeOut` | Seconds; start of the fade-out; negative = none (§8.9) |
| 280 | bool8 × 3 | `constrain_x`, `constrain_y`, `constrain_z` | `PEDesc_ConstrainX/Y/Z` | Zero the force-field contribution on that axis (controller 4, §8.7) |
| 283 | u32 | `frames_u` | `PEDesc_NumUFrames` | Atlas columns (§8.10) |
| 287 | u32 | `frames_v` | `PEDesc_NumVFrames` | Atlas rows (§8.10) |
| 291 | u32 | `frame_rate` | `PEDesc_FramesPerSec` | Frames per second (§8.10) |
| 295 | bool8 | `random_start_frame` | `PEDesc_RandomizeStartFrame` | §8.10 |
| 296 | VLE | `keyframe_count` *k* | — | 1–21 observed |
| 297 | keyframe × *k* | `keyframes` | — | §4.3 |
| +0 | u32 | `holding_location` | `PEDesc_EntityHoldingLocation` (enum 0x23000068) | Attachment point on the owner (§8.5.5) |
| +4 | u32 | `bone` | `PEDesc_EntityBoneType` (enum 0x23000086) | Attachment bone on the owner (§8.5.5) |
| +8 | bool8 | `stick_to_bones` | `PEDesc_StickToBones` | §8.5.5 |
| +9 | bool8 | `velocity_facing` | `PEDesc_VelocityFacing` | Align the primitive with the particle's screen-space motion (§8.12) |
| +10 | Pascal string | `name` | `PEDesc_Name` | Editor name; empty in 10,596 of 32,789 emitters |

An emitter whose curves are all mode 1 and which has no keyframe and an empty name is 308
bytes; each mode-1 keyframe with no points adds 60 bytes (§4.3).

<a id="s4-3"></a>
### 4.3 Keyframe

| Rel. offset | Size | Type | Field (own name) | Meaning, units |
|---|---|---|---|---|
| 0 | 4 | f32 | `time` | Particle age at this key, seconds (§8.6) |
| 4 | 4 | u32 | `flags` | Channel bits (§8.6.1) |
| 8 | 4 | f32 | `scale_x` | Primitive width factor |
| 12 | 4 | f32 | `scale_y` | Primitive height factor |
| 16 | 4 | argb | `colour` | §3.4 |
| 20 | 4 | f32 | `mass` | Multiplier of the forces of §8.8 (1 = full gravity, negative = rises) |
| 24 | 4 | u32 | `controller` | Motion controller (§8.7) |
| 28 | 4 | u32 | `physics_flags` | Force and point-selection bits (§8.8) |
| 32 | 4 | u32 | `point_count` *p* | |
| 36 | 12·*p* | vec3f × *p* | `points` | Point set for controllers 1 and 3 (§8.7), metres, emitter frame |
| 36 + 12·*p* | var | vec | `position` | Path curves for controller 5 (§8.7) |

The keyframe fields have no dictionary labels; the names are this spec's.

<a id="s4-4"></a>
### 4.4 Decoding procedure

1. Read the fixed system part (offsets 0–37), then the emitter count (VLE).
2. For each emitter, read its fields in the order of §4.2: curves per §3.2 (mode, then one or
   ten floats, then for mode 10 the period, loop byte, pair count and pairs), vectors as three
   curves, counts as VLE (keyframes) or u32 (points), the name as a Pascal string.
3. Read the tail (§4.1, +0 to +37).
4. The payload must now be exhausted; leftover bytes mean the record is not this format.

Limits for hostile input (Tessera rule): fail on a bool8 above 1, a mode-10 `loop` above 1,
or any count that cannot fit in the remaining bytes (a curve needs at least 8 bytes, a mode-10
pair 8, a keyframe 60, a point 12, an emitter 308); the client fails the record on the same
conditions (BIN). A curve mode of 0 or above 10 never occurs; the client reads ten floats for
any mode other than 1 (`property-system.md` §4.2), and a Tessera reader may either do the same
or reject the record. Observed maxima: 64 emitters per record, 21 keyframes per emitter, 1,203 points per
keyframe.

<a id="s4-5"></a>
### 4.5 Labels and dictionary types

The dictionary defines every label of §4.1 and §4.2 (OBS). Their types agree with the stored
encodings: curves are type 3 (waveform), f32 fields type 9, bool8 fields type 21, enums type 8
with the enum mapper given, counts and limits type 15 (integer), `PSDesc_Material` type 20 with
DB-object-type 0x36, `PEDesc_Name` type 1 (string). Two labels are views computed by the client
from stored fields rather than stored themselves (BIN): `PSDesc_FrameRate` (PID 0x2A9) is
1 / `time_step`, and `PEDesc_ParticlesPerSec` (PID 0x2C) is 1 / `emit_interval` (0 when the
interval is 0).

<a id="s4-6"></a>
### 4.6 Enums

| Field | Mapper | Keys and labels (OBS) | Use (OBS counts) |
|---|---|---|---|
| `shape` | 0x230000AC | 1 `Line`, 2 `Disc`, 3 `Sphere`, 4 `Point`, 5 `Quad` | Point 14,859; Disc 9,267; Sphere 7,424; Quad 1,001; Line 238 |
| `primitive` | 0x230000AD | 1 `Triangle`, 2 `Quad` | Quad 32,768; Triangle 21 |
| `scale_mode` | 0x230000AE | 1 `Total`, 2 `VDescOnly`, 3 `None` | Total 10,818; None 316; VDescOnly 21 |
| `holding_location` | 0x23000068 (123 keys) | `skeleton-anim.md` labels; 0 = `Invalid` (none) | 0 in 27,151; 99 other keys; all values are keys of the mapper |
| `bone` | 0x23000086 (87 keys) | 0 = `Default` (none) | 0 in 26,980; 45 other keys; all values are keys of the mapper |

`controller` and `physics_flags` have no mapper; their values are given in §8.7 and §8.8.

<a id="s4-7"></a>
### 4.7 Defaults of a new emitter and keyframe (BIN)

A writer creating new data starts from the client's defaults: `max_particles` 500; `shape` 4
(Point); `primitive` 2 (Quad); `emit_interval` 0.1; `burst_count` 1; `lifespan`, `speed`,
`shape_size`, `particle_scale` constant 1.0; `direction` constant (0, 0, 1); `snap` constant
0.1; every other curve constant 0; `inclusive` and `active` 1; `fade_in` 0; `fade_out` −1;
`start_time`, `time_limit`, `emission_distance` 0; `frames_u` and `frames_v` 1; `frame_rate`
30; all other bool8 fields 0; `holding_location` and `bone` 0; empty name. A new keyframe:
`time` 0, `flags` 0x7F0, scales 1.0, colour opaque white, `mass` 1.0, `controller` 2,
`physics_flags` 1, no points, `position` constant 0. The most common stored values agree
(`max_particles` 500 in 7,955 emitters, `frame_rate` 30 in 25,874, `snap` 0.1 in 32,627 of
32,789; §5.2 lists the other `snap` values).

---

<a id="s5"></a>
## 5. Census of prefix 0x39 (OBS, all records)

<a id="s5-1"></a>
### 5.1 System part

| Field | Observation |
|---|---|
| `time_step` | 18 distinct values: 1/30 (8,488), 0.1 (1,201), 1/15 (836), 1/60 (236), 0.05 (156), 1/24 (101), 1/32 (37), and 11 rarer values down to 1/120 |
| `fast_forward` | 0 in 9,646; non-zero in 1,509, up to 90 |
| `physics_duration` | 0 in all 11,155 |
| bool8 fields | `world_space` 7,540; `force_draw` 220; `uses_video_post` 139; `draw_reflection` 43; `use_lighting` 1,061; `inherit_opacity` 129 (records with value 1) |
| far fade | `far_fade_start` 1–4,000, `far_fade_end` 20–5,000; start ≤ end in all records; (40, 60) is the most common pair |
| near fade | 0 and 0 in 11,152 records; 3 records set them (start ≤ end in 2, start > end in 1) |
| `emitter_count` | 0 (1 record), 1 (4,379), 2 (3,058), 3 (1,342), 4 (903) … up to 64; 32,789 emitters in all |
| `has_bounds` | 1 in 7,345 (min ≤ max on every axis in all of them); 0 in 3,810, of which 3,602 store zero bounds and 208 store non-zero bounds |
| `sorting_priority` | 0–57; 5 (8,052), 4 (1,334), 6 (736) most common |
| `shareable` | 1 in 9,119 |
| `surface` | prefix 0x31 in 11,153 (all exist); 0 in 2 |
| `extra_surface` | 0 in all |

<a id="s5-2"></a>
### 5.2 Emitters (32,789)

| Field | Observation |
|---|---|
| `emit_interval` | 0 in 615 emitters (no timed emission, §8.3): 507 of them have a non-zero `emission_distance` (486 active, 21 inactive), 108 active ones have neither and emit nothing; otherwise finite, from 0.000263 to 100,000 (55 emitters below 0.001); none negative or not a number; 1.0 (3,351) and 0.1 (3,029) most common |
| `emission_limit` | 0 in 18,940; 108 other values |
| `max_particles` | 1–100,000; 500 (7,955), 100 (1,934), 1 (1,712) most common |
| `burst_count` | 1–300; 1 (26,620), 2 (1,103), 3 (875), 5 (723), 10 (721) … (46 values); 0 never occurs |
| `exploding` | 1 in 7,528 |
| `lock_world_rotation` | 1 in 66 |
| `start_time` | 0 in 22,232; −0.009 to 118 |
| `time_limit` | 0 in 25,190; up to 500 |
| `emission_distance` | 0 in 32,152; up to 46 |
| `snap` | Constant (mode 1) in all 32,789: 0.1 (32,627), 0.2 (111), 0.01 (32), 0 (12), 2000 (7) |
| `inclusive` / `active` | 1 in 23,114 / 31,158 |
| `fade_in` | 0 (31,639), 1.0 (1,131), 0.1–0.5 (19) |
| `fade_out` | −1 (32,393), 0 (312), −2 (71), positive (13) |
| constrain bytes | all 0 except (0,1,0) and (0,0,1) once each |
| `frames_u` × `frames_v` | 1 × 1 (27,088), 4 × 8 (2,217), 8 × 8 (925), 2 × 4 (707), 6 × 6 (456), 4 × 4 (410), 8 × 4 (313), 2 × 2 (263), 0 × 0 (106), others below 110 |
| `frame_rate` | 30 (25,874), 0 (1,014), 24, 60, 15, 16, 45 … (53 values) |
| `random_start_frame` | 1 in 6,976 |
| `keyframe_count` | 1–21; 4 (16,287), 3 (12,415), 5 (2,572); no emitter has 0 |
| `holding_location` / `bone` | non-zero in 5,638 / 5,809; either non-zero in 11,430 |
| `stick_to_bones` | 1 in 3,833 |
| `velocity_facing` | 1 in 1,091 |
| `name` | empty in 10,596; lengths up to 38 bytes |

Curve modes by field (non-constant curves): `speed` 15,788 of 32,789 (mostly mode 3, random);
`particle_scale` 26,620 (mostly mode 3); `rotation` Z 19,249; `rotation_velocity` Z 14,078;
`lifespan` 177; `shape_size` 2,506; `snap` none (its values are in the table above).

<a id="s5-3"></a>
### 5.3 Keyframes (121,087)

| Field | Observation |
|---|---|
| `time` | The first key of every emitter has time 0. Times are non-decreasing in all emitters except 4 (in records 0x39001745, 0x39001746, 0x39001CA8), where one or two keys go backwards. Of the 32,612 emitters with a constant `lifespan`, the last key's time equals it exactly in 28,578, is earlier in 3,873 (54 of them by less than 0.0001) and later in 161; the other 177 emitters have a non-constant `lifespan`. |
| `flags` | First key: 0x770 (32,776) or 0x7F0 (13). Bits over all keys: 0x4 (84,210), 0x2 (40,684), 0x200 (33,347), 0x100 (33,302), 0x400 (32,963), 0x40 (32,885), 0x20 (32,832), 0x10 (32,791), 0x8 (3,985), 0x80 (13). Keys after the first carry the set bits rarely: 0x200 (558), 0x100 (513), 0x400 (174), 0x40 (96), 0x20 (43), 0x10 (2). |
| `scale_x`, `scale_y` | −8 to 120 and −60 to 150; equal in 98,058 keys |
| `colour` | 18,950 distinct; 0x00FFFFFF (8,775), 0xFFFFFFFF (7,684), 0x00000000 (1,180) |
| `mass` | 1.0 (84,524), 0 (8,298), −0.1 (2,443) …; range −6 to 15 |
| `controller` | 1 (68,758), 2 (29,601), 6 (19,751), 5 (2,944), 3 (33). On keys with flag 0x100 (the only keys where it takes effect, §8.6.1): 2 (29,601), 5 (2,944), 6 (724), 3 (33). |
| `physics_flags` | 0 (99,569), 0x1 (20,069), 0x5, 0x8, 0x9, 0x4, 0x7, 0x25, 0xD, 0x21, 0x20, 0x3, 0x24 |
| `point_count` | 0 in 120,405; 144 in 649; up to 1,203; 104,784 points in all; keys with points use controller 2 (649) or 3 (33) |

---

<a id="s6"></a>
## 6. Fork-system descriptor (prefix 0x44)

A 0x44 record wraps the native effect image of a third-party particle library that the client
links, plus the tables the client needs to resolve the image's textures and child effects.

<a id="s6-1"></a>
### 6.1 Record layout

| Order | Type | Field (own name) | Meaning |
|---|---|---|---|
| 1 | u32 | `blob_length` *L* | Bytes of the effect image |
| 2 | *L* bytes | `blob` | The effect image (§6.2) |
| 3 | bool8 | `flag` | 1 = the effect loops until it is stopped; 0 = it runs out by itself (§6.4). The reader fails a byte above 1 (BIN). 1 in 92 records, 0 in 162 |
| 4 | Pascal string | `name` | Empty in all 254 |
| 5 | TSize + (Pascal, Pascal) × *n* | `name_table` | Short name → asset path. Hint 0; *n* = 1 (180) or 2 (74). Every key equals the last path component of its value (328 of 328). Values end in a texture extension (254) or an effect extension (74). |
| 6 | embedded property set | `properties` | `property-system.md` §6.3 form (TSize header then entries); the hint byte and count are 0 in all 254 records (bytes `00 00`) |
| 7 | u32 *m*, *m* bytes | `transform` | *m* = 64 in all: 16 f32, a 4 × 4 matrix stored row after row. Rows 0–2 are the identity rotation in 216 records; the last row holds a translation (each component within ±0.8) and 1.0. Placed in front of the owner's frame (§6.4) |
| 8 | TSize + (DID, Pascal) × *r* | `resources` | DID → short name. Hint 0; *r* = 1 (180) or 2 (74). Prefixes: 0x41 render surface (254 entries, one per record), whose name is a key of `name_table` (254 of 254); 0x44 nested fork descriptor (74), whose name is a `name_table` key without its effect extension (74 of 74) |
| 9 | f64 | `tail_value` | Lifetime window in seconds (§6.4). 0.2167–20.0167 |
| 10 | vec3f | `bounds_min` | Bounding box (Inferred; no reader found, Q2); min ≤ max in 242 records, not in 12 |
| 11 | vec3f | `bounds_max` | |
| — | — | *(end)* | Zero leftover bytes in all 254 records |

Version rule (BIN): the reader works with two version numbers, one for the record and one for
items 4–7. With an item version below 3, item 6 is a u32-counted list of property values
instead of a property set; with an item version below 2, item 7 is absent; with a record
version below 3, items 9–11 are absent. The client takes both versions from the read context,
not from the record bytes: it asks the context for the version registered under a four-byte
tag, 0x46505344 for the record version and 0x4643444E for the item version (an unset item
version, value 0, counts as 3). When writing, the client writes version 3 for items 4–7 and
refuses to write a record version below 3. Every record of this install decodes completely
with the version-3 reading (OBS), so a reader decodes only that form and fails anything else.
Where the read context gets its versions for a stored record is OPEN (Q2); it does not change
the decoding of this install.

<a id="s6-2"></a>
### 6.2 The effect image (`blob`)

Facts established over all 254 records (OBS) and from the loader's checks (BIN):

- The image is 991–87,701 bytes. The client copies it, doubles the buffer (pointer widening)
  and rejects an image shorter than 488 bytes (0x1E8) (BIN).
- u32 at image offset 0 is 0x50 and u32 at offset 4 is 0x1E8 in every record.
- The image starts with a 488-byte header of scalars and (count, offset) pairs; offsets are
  byte offsets from the start of the image, which the loader turns into pointers (BIN).
- u32 at header offset 0x120 is 488 in every record (the first byte after the header). Every
  short name of `name_table` and of `resources` occurs inside the byte range that starts there
  and is as long as the u32 at header offset 0x11C (254 of 254); that this u32 is the length of
  a name block is Inferred.

> **OPEN (Q1):** The internal layout of the effect image beyond these facts (the meaning of
> the 488-byte header, its sub-structures and their relocation, which blend mode an effect uses,
> its emitters and their parameters) and the library's simulation. The image belongs to a
> third-party library; reconstructing it is a large separate task. Suggested method: M4 on the
> loader that starts at the size checks above and on the library's update and draw entry points
> that the client registers when it creates a fork effect.

<a id="s6-3"></a>
### 6.3 How 0x44 records relate to each other

38 distinct 0x44 records are named by `resources` entries of other 0x44 records (74 entries);
35 of them are reached only that way, 3 are also named by Particle nodes. Together with the 219
records named by Particle nodes, every 0x44 record is reachable (OBS).

<a id="s6-4"></a>
### 6.4 Resource resolution (BIN)

When the client prepares a fork effect it walks `resources` and, for each DID, asks the
database for the DID's type:

- DB type 5 (render surface, prefix 0x41): loads the texture and binds it to the effect under
  the entry's short name.
- DB type 0x51 (shader record, prefix 0x2B): accepted and kept.
- DB type 0x56 (another fork descriptor, prefix 0x44): loaded and prepared recursively; its
  own resources are resolved first.
- Any other type: the entry fails and the effect is not prepared.

Every DID of `resources` is also declared as a preload dependency of the record. For a 0x39
record the only declared dependency is `surface` (BIN).

The client draws fork effects with seven shader records that it fetches from UNIQUEDB (DID map
0x28000002) by fixed keys, one per blend mode (BIN for the keys and the DB type 0x51; OBS for
the entries): 0x159 `ForkMaterial_BlendMode_LightMap` → 0x2B0006F0; 0x15A `…_Emissive2` →
0x2B0007DB; 0x15B `…_Emissive3` → 0x2B0006EA; 0x15C `…_Normal` → 0x2B000713; 0x15D `…_DarkMap`
→ 0x2B0007D7; 0x15E `…_Emissive` → 0x2B0006EC; 0x15F `…_Subtractive` → 0x2B00075D. The labels
are the DID map's own; a client resolves by key.

Fork effects receive the same gravity and wind vectors as §8.8, rotated into the effect's frame
(BIN); what the library does with them is part of Q1.

<a id="s6-5"></a>
### 6.5 Fork effect lifetime, placement and fade (BIN)

These facts concern the outer record and the client's handling of a fork effect; the library's
own simulation stays Q1.

- **Image name.** The client asks the library for a record's effect by a name made of the
  record's DID written as `0x` followed by eight upper-case hexadecimal digits (how the loaded
  image is registered under that name is part of Q1).
- **`flag` and `tail_value`.** Each running fork effect has a start time and, once it has been
  told to stop, a stop time. Let *w* be the effect's age (seconds since the system started)
  minus its age at the stop (0 while not stopped). The effect has **expired** when it is not
  (looping and not stopped) and *w* > `tail_value`; looping means `flag` = 1. While it is looping
  and not stopped, or while *w* ≤ `tail_value`, the system counts as running as long as the
  library reports the effect active or holding live particles. So a non-looping effect ends at
  the latest `tail_value` seconds after it starts, a looping one `tail_value` seconds after it is
  stopped, and either ends earlier once the library has no live particles. When the first
  effect of a fork system has finished, the client stops all the others (the nested effects of
  §6.3).
- **Particle-node handling.** The node that starts the effect treats a fork record with
  `flag` = 1 as never-ending and keeps a controller that stops it when the script ends; with
  `flag` = 0 it lets the effect run out (`script-table.md` §5.3).
- **`transform`.** The effect's matrix is the stored matrix multiplied (rows as vectors, stored
  matrix on the left) by a basis built from the owner's frame: row 0 = the owner's X axis, row 1
  = the owner's Z axis, row 2 = the owner's Y axis negated, row 3 = the owner's position with
  each component divided by the owner's scale on that axis (a scale component below 2⁻²³ counts
  as 1), with 0, 0, 0, 1 as the last column. Each axis is normalised first. So the library's
  second axis is the owner's up axis.
- **Fade.** A fork system takes its distance-fade settings from the Particle node that started
  it (`ParticleScript_…` fade fields: the fade switch, far fade start and end, near fade start
  and end). Without a node the far fade defaults are 40 and 60 metres. A 0x39 system ignores
  these node fields and uses its own (§8.13).
- **Fixed answers.** A fork system is always local-space, never shared, never in the
  post-processing pass, never inherits opacity, and sorts with priority 0 (§8.15, §8.16).

---

<a id="s7"></a>
## 7. References (DID graph)

<a id="s7-1"></a>
### 7.1 Into the families

<a id="s7-1-1"></a>
#### 7.1.1 Script nodes (OBS over 7,524 tables and 28,992 scriptlets; 2 tables do not decode, ST)

- **Particle node (kind 59):** 41,354 nodes. `ParticleScript_ParticleID` (PID 0x271, type 20,
  DB type 0x1E) names a 0x39 record in 39,830 nodes and 0x44000091 in 7 nodes;
  `ParticleScript_ForkParticleID` (PID 0x81C, DB type 0x56) names a 0x44 record in 1,517
  nodes. Exactly one of the two is non-zero in every node (§2.2).
- **Which record a Particle node starts (BIN).** The node's effect DID is
  `ParticleScript_ParticleID` when that is not 0, otherwise `ParticleScript_ForkParticleID`
  (0 when both are 0, and then nothing starts). When a node is loaded with both fields non-zero,
  the client clears the fork field, so the particle field wins. The client then looks up the
  DID's DB type and creates a particle system for type 0x1E or a fork system for type 0x56; any
  other type creates nothing. The declared DB type of the field plays no part. The 7 nodes that
  put 0x44000091 in the particle field therefore start fork effect 0x44000091 exactly as if it
  were in the fork field.
- **Beam node (kind 54):** `BeamScript_BeamParticleSystem` (PID 0x481) in 571 nodes and
  `BeamScript_HeadParticleSystem` (PID 0x38E) in 45 nodes, all 0x39.
- The per-script DID lists of script tables name these DIDs too (ST §2.5: 26,336 0x39 entries
  and 699 0x44 entries), as do scriptlet preload lists (ST §3).

Coverage: 11,123 of the 11,155 0x39 records are named by a script node; all named DIDs exist.
Of the other 32: 10 are UNIQUEDB entries (§7.1.3), 20 are only in the whitelist (§7.1.2), and
2 (named nowhere that was searched) are unreferenced.

<a id="s7-1-2"></a>
#### 7.1.2 Dynamic-particle whitelist

Property set 0x780060C9 (gamelogic) holds an array `Render_DynamicParticleWhitelistArray`
(PID 0xB2C) of 272 `Render_DynamicParticleWhitelistEntry` values (PID 0xB2B, type 20, DB type
0x1E), 272 distinct 0x39 DIDs, all present. These are the 272 values of DB type 0x1E that
`property-system.md` §9.1 counts in 0x78 sets. The client finds the set through UNIQUEDB key
0x1E2 (label `DynamicParticleWhitelist`, → 0x780060C9 in this install, OBS) and reads every
element of the array that has a DID value into a lookup set when its particle code starts (BIN).

**Start gating (BIN).** When a script node asks to start an effect, the client checks the object
that runs the script (the owner) before it requests the system:

1. No owner object, or an owner that lacks the client-side part that holds its drawn state (not
   identified further): nothing starts.
2. The owner is a static-environment object (its 64-bit instance ID has top byte 0x01, 0x05 or
   0x81): the effect starts only when the client option `StaticEnvironmentalObjects`
   (`[Render]`, a switch, default on) is on. The whitelist does not apply.
3. Any other owner (a dynamic object): the effect starts when its DID is in the whitelist.
   Otherwise it depends on the client option `DynamicParticles` (`[Render]`, an integer with
   five values, default 0):

| `DynamicParticles` | Effects that are not whitelisted start on |
|---|---|
| 0 | every owner |
| 1 | the local player's own character only |
| 2 | the local player's character, and the object the local player has selected (the local player's `Entity_Selection` property, PID 0x34F) |
| 3 | the selected object only |
| 4 | no owner |
| any other value | every owner (treated like 0) |

The option names are interoperability identifiers of the client's settings; the client's own
labels for the five values are not reproduced. The DID checked is the node's effect DID
(§7.1.1), so the same rules apply to fork effects; a whitelisted DID starts regardless of the
option. An effect that does not start returns no handle to the script.

Other dictionary properties of type 20 with DB type 0x1E (`QuestState_True_Particle`,
`QuestState_False_Particle`, `Filter_ParticleEntry`) have no stored value in the property sets
(0x78–0x7F), objects (0x70), DID maps or singleton tables of the general and gamelogic archives
(OBS).

<a id="s7-1-3"></a>
#### 7.1.3 UNIQUEDB (DID map 0x28000002)

Ten labelled keys name 0x39 records: 0x2A `CameraRainImpactParticleSystem` → 0x39000001; 0x4A
`WaterRipplesParticleSystem` → 0x39000004; 0x6C `RainVaporParticleSystem` → 0x39000002; 0x7B
`CameraRainTorrentParticleSystem` → 0x39000000; 0x83 `RainGustParticleSystem` → 0x39000003;
0xA1 `WaterBubblesParticleSystem` → 0x39000005; 0xFD `CameraInBlizzardParticleSystem` →
0x390010DA; 0xFE `SnowGustParticleSystem` → 0x390010DD; 0x100 `CameraSnowImpactParticleSystem`
→ 0x390010DB; 0x1E7 `CoinSwimRipplesParticleSystem` → 0x39002FBB (OBS). These are the
weather and water effects the client runs itself. The weather code creates one instance of each
when it initialises, and two of them get their forces recomputed by the precipitation updates
(§8.8.1) (BIN). When each one is placed and where it receives particles is `particle-runtime.md`
§11.2 (2026-10-23); what remains open is Q13 there and here.

<a id="s7-2"></a>
### 7.2 Out of the families

| From | To | Rule |
|---|---|---|
| 0x39 `surface` | 0x31 surface record (`material.md` §4) | The client requests DB type 0x36 (surface record) (BIN). 11,153 non-zero, all exist. |
| 0x39 `extra_surface` | DB type 0x36 | Always 0; used only when `surface` = 0 (§4.1). |
| 0x39 `holding_location`, `bone` | enum keys, not DIDs | §4.6 |
| 0x44 `resources` | 0x41 render surface (in `client_surface*.dat*`), 0x44 | §6.4; all 328 entries exist |

No 0x39 record names a 0x44 record, and no 0x44 record names a 0x39 record (OBS).

---

<a id="s8"></a>
## 8. Runtime behaviour of 0x39 systems (BIN)

This section describes what the client does with a 0x39 record once a script node or the
engine starts it. Times are seconds. "Now" is the client's global clock (`material.md` §5.3.1).
The **system start** is the time of the system's first placement minus `fast_forward` (§8.17);
an emitter's **emitter time** is now − (system start + `start_time`).

<a id="s8-1"></a>
### 8.1 Update cadence

**System update.** A running system is updated at most once per frame, when its owner places
it for that frame (the first draw instance of the frame, §8.16). The update is skipped:

- while the system is in the not-drawn state of §8.13 (all its instances failed the distance
  test in the last draw); the draw that brings it back runs the update before drawing;
- when `physics_duration` ≠ 0 and the time since the system start (§8.17) exceeds it;
- for the emission step only, after the owner has told the system to stop emitting.

"Now" for the update is the later of the system's creation time and the client clock. The update
visits the emitters in stored order; for each it first runs emission (§8.2 window, then timed
emission §8.3, then distance emission §8.4) and then the particle update below (BIN).

**Particle update.** The particles of an emitter are updated only when at least `time_step`
seconds have passed since that emitter's previous particle update; otherwise the update is
skipped and the particles keep their state (BIN). The previous particle-update time starts at
system start + `start_time` (§8.17).

**Drawn position (interpolation).** For every emitter that has at least one keyframe (all
emitters of this install), each particle keeps its position before the last particle update.
When drawing, its position is previous + *f* × (current − previous), with *f* = (now − the
emitter's last particle-update time) / `time_step` (not clamped). The previous position is set
equal to the current one when the particle is created, and again after any particle update whose
step (now − previous particle-update time) exceeds 0.25 s, so large steps are not smoothed (BIN).

Values that do not occur in the data (OBS: every `time_step` lies between 1/120 and 0.1) but
that hostile input can carry (BIN):

- `time_step` ≤ 0: the test always passes, so the particles are updated on every call;
  controller 4 (§8.7), which advances in steps of `time_step`, would then never finish its
  step loop.
- `time_step` not a number: the test never passes and the particles are never updated.

Tessera applies rule §10.5 instead.

<a id="s8-2"></a>
### 8.2 Activity window and end of effect

1. An emitter with `active` = 0 never emits and is ignored by the end test.
2. An active emitter emits only while 0 ≤ emitter time and, when `time_limit` is not within
   0.0002 of 0, emitter time ≤ `time_limit`. Before its window opens it only keeps its
   position history up to date (so that §8.4 does not see a jump).
3. The system has **ended** when every active emitter satisfies one of:
   - `time_limit` is set and now ≥ system start + `start_time` + `time_limit` + *L*;
   - `emission_limit` ≠ 0, `emit_interval` ≠ 0 and now > system start + `start_time` +
     `emission_limit` × `emit_interval` / `burst_count` + *L*,
   where *L* is `w[0]` of the `lifespan` curve (its constant for mode 1, its base term
   otherwise). A system with no active emitter has ended at once. A system whose emitters have
   neither limit runs until its owner stops it.
4. `burst_count` = 0 (never in the data, OBS minimum 1): the client computes the second
   condition's time as `emission_limit` divided by (`burst_count` × (1 / `emit_interval`)),
   which is a division by zero and yields positive infinity, so the condition never holds and
   only `time_limit` can end that emitter (BIN). Such an emitter emits nothing (§8.3, §8.4).
   Rule §10.6 keeps this result without dividing.

<a id="s8-3"></a>
### 8.3 Timed emission

Per emitter, the client keeps the time of the last burst and a running total of emitted
particles.

Timed emission runs only when `emit_interval` is not exactly 0 and the last-burst time is not
later than now. With `emit_interval` = 0 the emitter has **no** timed emission at all (BIN);
it emits only by distance (§8.4), if `emission_distance` ≠ 0, and otherwise never. When it
runs:

1. **Cap.** cap = `max_particles`; if `emission_limit` ≠ 0 and `emission_limit` ≤
   `max_particles`, cap = `emission_limit`. Room = cap − live particles (0 if negative).
2. **Catch-up.** If more than *L* (the `lifespan` curve evaluated with the last-burst time on
   the client clock as its time argument, not the emitter time; P39) has passed since the last
   burst, the last-burst time jumps to now − *L*, and the running total
   grows by the integer part (towards zero) of (skipped seconds / `emit_interval`), multiplied
   by `burst_count`. Particles that would already have died are never created, but they still
   count against `emission_limit`.
3. If `emission_limit` ≠ 0 and the running total already exceeds it, stop.
4. While the last-burst time is before now:
   - if room is 0, set the last-burst time to now and stop;
   - burst = `burst_count`, reduced to `emission_limit` − running total when that is smaller;
     if burst < 1, set the last-burst time to now and stop;
   - spawn min(burst, room) particles **at the last-burst time** (§8.5), add them to the total
     and subtract them from room;
   - advance the last-burst time by `emit_interval`.
5. With `burst_count` = 0 the burst of step 4 is 0, below 1, so the emitter never spawns a
   particle; the catch-up of step 2 adds 0 (BIN).

The running total is a 32-bit unsigned count, and the client's additions to it wrap modulo 2³²
(BIN). Each pass of step 4 spawns at least one particle and lowers room by as many, so one update
makes at most cap passes, whatever the interval.

Values of `emit_interval` that do not occur in the data (OBS: 0 in 615 emitters, otherwise a
finite number from 0.000263 to 100,000) but that hostile input can carry (BIN):

- **Negative.** The value is not 0, so timed emission runs. Step 4 moves the last-burst time
  backwards, so the loop ends only when room reaches 0 or a burst is below 1. Step 2 adds a
  negative count to the running total. Below 0 the total wraps to a large value, and when
  `emission_limit` ≠ 0, step 3 then stops all later timed emission.
- **Positive but too small to move the last-burst time** (adding it leaves the time unchanged at
  the clock's precision). Here too the loop ends only when room reaches 0 or a burst is below 1.
- **Not a number.** After the emitter's first burst the last-burst time is not a number. The test
  "not later than now" then fails, so the emitter never emits by time again.

Tessera applies rule §10.8 instead.

The steady rate is therefore `burst_count` / `emit_interval` particles per second. Particles
spawned at a time in the past are advanced at once (§8.5 step 9) and reach now in the same
system update's particle update.

**Start value.** The last-burst time starts at system start + `start_time`, where the system
start already lies `fast_forward` seconds before the first placement (§8.17). The first update
inside the window emits every burst from that time to now, so the emitter starts with particles
of all ages up to the smaller of that gap and *L* (step 2 drops the older part). With
`fast_forward` = 0 the gap is only the delay before the first update, so the emitter starts
nearly empty and fills at the steady rate (BIN).

<a id="s8-4"></a>
### 8.4 Distance emission

When `emission_distance` ≠ 0, each update also emits along the path the emitter moved since
the previous update (BIN). The emitter tracks two **anchors**, each with its value at the
previous update:

- the **system anchor**: the system's position;
- the **line anchor**: for a `Line` emitter (shape 1), the system's position plus the line
  vector of §8.5.1 (normalise(`direction`) × `shape_size`, both evaluated at the client clock)
  rotated by the system's orientation; for any other shape the same point as the system anchor.

Before the emitter's window opens (§8.2) both previous values simply follow the current ones.
When the window is open, after timed emission:

1. Remember the previous update time *t*ₚ and set the stored previous update time to now.
2. Skip the rest unless `emission_limit` = 0 or the running total is at most `emission_limit`.
3. *d*₁ and *d*₂ = the distances each anchor moved since the previous update. Skip the rest
   unless *d*₁ or *d*₂ is at least 0.0002.
4. *n* = the integer part of max(*d*₁, *d*₂) / `emission_distance`. Skip the rest when *n* = 0.
   Each step moves the previous system anchor by *d*₁ / *n* and the previous line anchor by
   *d*₂ / *n* along their own paths.
5. Room = cap − live particles (cap as in §8.3 step 1; 0 if negative). While room > 0:
   - the step time *t* = *t*ₚ + *k* × (*t*ₚ − now) / *n* for the *k*-th step (*k* = 1, 2, …),
     limited to at most now. The client adds the negative interval, so successive steps are
     dated further **before** *t*ₚ, not between *t*ₚ and now; this is the original's arithmetic;
   - stop when both anchors are closer than `emission_distance` to their current positions;
   - burst = `burst_count`, reduced to `emission_limit` − running total when that is smaller;
   - move both previous anchors one step;
   - if burst < 1, set the stored previous update time to now and stop;
   - spawn min(burst, room) particles at time *t* (§8.5), each offset to the stepped system
     anchor; for a `Line` emitter the vector from the stepped system anchor to the stepped line
     anchor, rotated back into the system frame, replaces the line vector of §8.5.1. Add them to
     the running total and subtract them from room.

So one update spawns at most cap particles by distance and takes at most *n* + 1 steps
(`burst_count` = 0 ends the loop at the first step, §8.2 item 4).

<a id="s8-5"></a>
### 8.5 Spawning one particle

Every curve in this section is evaluated at the **emitter time of the spawn** (BIN), except
where stated.

1. *L* = `lifespan`. If spawn time + *L* ≤ now, the particle is not created.
2. Shape point *s* (§8.5.1).
3. Attachment (§8.5.5).
4. If the emitter already holds its maximum number of live particles, stop.
5. Direction *d* (§8.5.2), then normalise it; a zero vector (length ≤ 10⁻⁶) becomes (0, 1, 0).
6. *s* += `origin`.
7. Spread (§8.5.3).
8. Position = the system's frame applied to *s*, plus the distance-emission offset of §8.4
   (zero for timed emission). Velocity = *d* × `speed` (§8.5.4).
9. Per-particle values drawn now: `particle_scale`, `streak`, `rotation`, `world_rotation`,
   `rotation_velocity` (one evaluation each), and a uniform random number in [0, 1) when
   `random_start_frame` = 1. The new particle is then updated once to the emitter's **last
   particle-update time** (§8.1), not to now (§8.6); that time can be earlier than the spawn
   time, and then the motion of §8.7 is evaluated once with a negative age. If that update finds
   it dead it is removed. The particle reaches now at the emitter's next particle update, which
   in the same system update follows emission whenever `time_step` has elapsed (BIN, P40).

<a id="s8-5-1"></a>
#### 8.5.1 Shapes

Let *z* = `shape_size`. Random numbers are uniform. "Normalise" divides by the length.

| `shape` | Point *s* (emitter frame, Z up) |
|---|---|
| 1 `Line` | The emission step passes a line vector *v* = normalise(`direction`) × *z*, with `direction` and `shape_size` evaluated at the absolute clock (BIN, a quirk of this call site). If \|*v*\|² ≥ 0.0002: *s* = normalise(*v*) × random in [0, *z*]; otherwise *s* = 0. |
| 2 `Disc` | *u* = normalise(random in [−1, 1], random in [−1, 1], 0); if `inclusive`, *u* ×= random in [0, 1]; *s* = *u* × *z* / 2. Radius *z* / 2 in the XY plane. |
| 3 `Sphere` | *u* = normalise of three randoms in [−1, 1]; if `inclusive`, *u* ×= random in [0, 1]; *s* = *u* × *z* / 2. |
| 4 `Point` | *s* = 0. |
| 5 `Quad` | *s* = (random in [−*z*/2, *z*/2], random in [−*z*/2, *z*/2], 0). If `inclusive` = 0, an integer *e* uniform in 0–3 moves the point to an edge: 0 sets x = −*z*/2, 1 sets y = −*z*/2, 2 sets x = *z*/2, 3 sets y = *z*/2. |

The random directions are normalised cube samples (not uniform on the sphere) and the filled
disc and sphere scale the radius linearly (not by area or volume). This is the original's
distribution.

<a id="s8-5-2"></a>
#### 8.5.2 Direction

- `exploding` = 0: *d* = `direction`, rotated by the system's orientation.
- `exploding` = 1: if any component of the shape point *s* exceeds 0.0002 in magnitude, where
  *s* is the point after the attachment of step 3 (§8.5.5: moved into the bone or holding frame
  when the emitter has one) and before `origin` is added in step 6 (BIN third pass, P53),
  *d* = *s* rotated by the system's orientation (radial from the emitter);
  otherwise *d* = three randoms in [−1, 1] (not rotated).

<a id="s8-5-3"></a>
#### 8.5.3 Spread

*a*₀ = `min_spread`, *a*₁ = `max_spread` (degrees). If *a*₁ − *a*₀ ≤ 0.0002, no spread. Otherwise
pick *a* uniform in [*a*₀, *a*₁]; if *a* is 0, no spread. Otherwise tilt *d* by *a* degrees about
the axis *d* × *u*, where *u* is the part of +Z perpendicular to *d* (normalised), or the part
of +Y when *d* is parallel to Z; then turn the tilted direction about the original *d* by a
uniform random angle in [0, 2π). The result is a cone between the two angles around *d*.

<a id="s8-5-4"></a>
#### 8.5.4 Speed

Velocity = normalised spread direction × `speed` (metres per second). A negative speed points
backwards.

<a id="s8-5-5"></a>
#### 8.5.5 Attachment

If `stick_to_bones` = 0 and (`bone` ≠ 0 or `holding_location` ≠ 0), the client resolves that
bone or holding location on the owning entity and moves the shape point into its frame; if it
cannot be resolved and no earlier result is cached, the particle is not spawned. With
`stick_to_bones` = 1 the attachment is applied when drawing instead: each particle of the
emitter is drawn at the attachment point's current position (the point's translation, scaled per
axis as in §8.6.3, then placed by the system's frame) instead of its simulated position, and so
is its streak anchor; size, colour, rotation and texture frame still come from the particle
(BIN). Both paths require
`world_space` = 1: in a system with `world_space` = 0 an attached emitter spawns nothing
(`stick_to_bones` = 0) or draws nothing (`stick_to_bones` = 1) (BIN). The data respects this:
11,409 attached emitters are in world-space systems and 21 are not (OBS). Holding locations
and bones are those of `skeleton-anim.md`.

<a id="s8-6"></a>
### 8.6 Particle life and keyframe channels

A particle's **age** is now − its spawn time. It dies when age > *L*, *L* being `lifespan`
evaluated at the emitter time of its spawn.

<a id="s8-6-1"></a>
#### 8.6.1 Flag bits and channels

Keyframes are visited in stored order at each particle update: a key becomes current once the
particle's age, raised to 0 when negative, is at least the key's `time` (an equal age counts;
steps 1–5 below give the full rule). A key takes part in a channel according to its `flags`:

| Bit | Effect when the key becomes current |
|---|---|
| 0x10 or 0x02 | The key becomes the particle's current **scale** key |
| 0x20 or 0x04 | The key becomes the current **colour** key |
| 0x40 or 0x08 | The key becomes the current **mass** key |
| 0x100 | The key becomes the current **controller** key: its `controller`, `points` and `position` govern motion from now on (§8.7). Before switching, the client fixes the particle's position at the key time as the new base position (the **base fix**, below). |
| 0x200 | The particle's physics flags become the key's `physics_flags` |

Interpolation targets: when an emitter is created, the client computes for every key *K* and
per channel a **target key** *T* (BIN): the first key after *K* in stored order whose
interpolate bit is set (0x02 scale, 0x04 colour, 0x08 mass); when no later key has that bit,
*T* is *K* itself. The last key's targets are always itself. Between *K* (time *t*₀) and *T*
(time *t*₁):

- *f* = (age − *t*₀) / (*t*₁ − *t*₀), clamped to [0, 1]; when *t*₁ − *t*₀ ≤ 0.0002 (scale,
  colour) or |*t*₁ − *t*₀| ≤ 0.0002 (mass), *f* = 1 (scale, colour) or 0 (mass);
- value = (1 − *f*) × *K* + *f* × *T*, per component (`scale_x`, `scale_y`; the four colour
  floats; `mass`).

The test the client makes when drawing is on the **target** key: for scale and colour it
interpolates only when *T* has the channel's interpolate bit, and otherwise uses *K*'s value;
for mass it always interpolates. Because *T* lacks the bit only when *T* is *K* itself, and
because *T* = *K* gives *f* = 1 with value *T* (scale, colour) or *f* = 0 with value *K*
(mass), all three cases reduce to one rule: **when no key after the current key has the
channel's interpolate bit, the value is the current key's.**

The set bits 0x10/0x20/0x40/0x100/0x200 are what the first key uses (0x770 sets all of them).
Bits 0x80 and 0x400 (0x400 on almost every first key) have **no effect**: neither the key-switch
routine, the target builder, the spawn routine, the particle update nor the vertex writer tests
them (BIN, P47). A reader keeps them and ignores them.

Without keyframes no channel exists and mass is 1 (not observed: every emitter has a key).

**When a key becomes current (BIN third pass, P53).**

1. A new particle has no current key: its scale, colour, mass and controller references are
   empty and its physics flags are 0.
2. At every particle update (the one that follows the spawn, §8.5 step 9, included) the client
   takes the age, raises it to 0 when it is negative, and walks the keys in stored order,
   starting after the current one (at the first key when none is current). A key becomes
   current when that age is **at least** its `time` (an age equal to the `time` counts); the walk
   stops at the first key whose `time` is greater. Because every first key of the data has
   `time` 0, the first key is current from the first update on, also when that update runs at a
   negative age.
3. A key that becomes current applies its bits in this order: the controller switch (0x100;
   the position is first fixed with the previous controller, as the table says), then the
   scale, colour and mass bits, then the physics flags (0x200).
4. The point index of controllers 1 and 3 (§8.7) is not taken at the switch. Each particle has
   **one** point-index slot, shared by both controllers, empty at creation. It is filled once,
   by the first evaluation that needs it: a particle update that runs controller 1 or 3, or the
   base fix of a leaving controller-1 or controller-3 key (below; each fills it in its own way).
   It is never emptied again, so later controller-1 and controller-3 keys of the same particle
   start from the index already in the slot (BIN fourth pass, P54; fifth pass, P57). A particle
   update that fills the slot sees the physics flags of the key that set the controller,
   whichever order step 3 uses; a base fix that fills it sees the physics flags current before
   the new key applies its own 0x200 bit (step 3).
5. Without a current controller key a particle does not move (the update runs no controller).

**The base fix (BIN fourth pass, P54).** When a key with bit 0x100 becomes current while another
controller key is current, the client first moves the particle's base position *p*₀ (§8.7) by
the motion of the **leaving** key over *τ* = the new key's `time` − the leaving key's `time`, and
the new controller then starts from that base:

| Leaving controller | New base |
|---|---|
| 2 | *p*₀ + *v* *τ* + ½ *a* *τ*², with the force *a* of §8.8 scaled by the `mass` of the current mass key as stored (not interpolated) |
| 5 | *p*₀ + (`position` X, Y, Z evaluated at *τ*) |
| 3 | When the leaving key has no points: *p*₀ unchanged. Otherwise, if the particle's point-index slot is empty, the slot takes the leaving key's counter and the counter advances by one, wrapping at the key's point count; this is done **whatever physics flag 0x8 says** (no random draw in the base fix). Then *p*₀ + the point of the leaving key at the slot's index, rotated by the system's orientation. |
| 1 | When the leaving key has no points: *p*₀ unchanged. Otherwise the point-path walk of §8.7 runs once under the leaving key (BIN fifth pass, P57): (a) if the particle's point-index slot is empty, it is filled as at a controller-1 update: index 0, or, when physics flag 0x8 is set, an index drawn uniformly from 0 to (point count − 1); no key counter changes; and the particle is placed on *p*₀ + that point, rotated by the system's orientation; (b) the walk starts from the particle's **current** position (where the last update or step (a) put it) with the travel budget *b* = *s* × *D*, where *D* = (the particle's spawn time + the new key's `time`) − the emitter's previous particle-update time. *D* is a difference of two clock times, not an age; the previous particle-update time is the one stored before the running update began (§8.1: the emitter stores the new time only after all its particles are updated); *D* is not clamped, and when *b* ≤ 0.0002 the walk moves nothing. (c) The particle's position becomes the walk's result, and the new base is that position. |
| 6, 4 and any other value | *p*₀ unchanged |

So when one walk makes two controller keys current at once and the first has controller 3
(the case of the implementer OPEN of 2026-10-09), the second key's base fix fills the empty slot
from the first key's counter, in order, and places the base on that point. The census of P56 shows
that the data can reach it: in 1 emitter a controller-3 key is followed by another controller
key less than `time_step` later, so one particle update can pass both; in 12 cases a
controller-3 key is followed by another controller key at all, where the slot is normally
already filled.

A leaving controller-1 key finds the slot empty in the same case: one walk passes the
controller-1 key and a later controller key before any update has run controller 1. Its base
fix then fills the slot (index 0 or, with physics flag 0x8, a random index), places the particle
on that point and walks the budget of the row above. Unlike the controller-3 base fix it draws
the random index when flag 0x8 is set and leaves the key's counter alone. The data never reaches
this case (no key with bit 0x100 has `controller` 1, PS9); PS11 is a synthetic check.

**A channel without a current key** (not reached by the data). The client keeps no value for
it: its vertex writer reads the scale and colour through the empty reference, and the
controller-2 switch reads the mass the same way, so the original has no defined result.
**Tessera rule (Decision, [§10.11](#s10)):** use scale (1, 1), opaque white (1, 1, 1, 1) and
mass 1, the values of an emitter without keys, and no motion without a controller key.

**Emitter-level conditions (BIN fourth pass, P55).** When an emitter is created the client
derives from its keyframes which per-particle state its particles carry:

- the key walk of steps 2–3, and the previous position of §8.1, exist exactly when the emitter
  has at least one keyframe (every emitter of this install, P56);
- the physics flags exist, and a key's 0x200 bit is copied, only when at least one keyframe of
  the emitter, whatever its bits, has `controller` 1, 2, 3 or 4. In the other emitters (1,151,
  each with a 0x200 key, P56) the copy is skipped. Nothing else reads the physics flags
  (controllers 5, 6 and the rest use none), so the skip has no visible effect, and an
  implementation may copy 0x200 for every emitter.

> **Answered (2026-10-10, BIN fourth pass; was OPEN Q15, review of 2026-10-09):** the key walk
> runs for every emitter that has keyframes; the physics-flag copy depends on an emitter-level
> condition (a key with `controller` 1–4) that changes nothing visible. See the paragraph above.

> **Answered (2026-10-10, BIN fourth pass; was OPEN, implementer, 2026-10-09):** neither of
> the two choices. If the particle's point-index slot is empty (no controller-1 or controller-3
> evaluation has filled it), the base fix fills it from the **leaving** key's counter and
> advances that counter by one, in order, without a random draw even when physics flag 0x8 is
> set; the base then moves to that point (rotated by the system's orientation). If the slot is
> filled, the base fix uses it as it stands. With no points the base stays. The slot is shared
> by controllers 1 and 3 and never emptied (step 4). The simulation's choice ("the index as it
> stands, 0 when empty; the counter not advanced") therefore differs when the slot is empty:
> follow the base-fix table above. P56: the data can reach the case in 1 emitter.

> **Answered (2026-10-10, by the base-fix table and the Answered callout above; was OPEN,
> implementer, 2026-10-09):** When one walk (step 2) makes two controller keys current
> at once and the first of them has controller 3, the second key's base fix (step 3: "the
> position is first fixed with the previous controller") runs controller 3 before that
> controller ever ran, so its point index (step 4) was never drawn. Which point does the base
> fix use then: the particle's index from an earlier controller-3 key (0 when there was
> none), or none at all (the base stays where it is)? The data never reaches it (every first
> key has `time` 0 and no two controller keys share a time in practice). The run-time
> simulation (`crates/particles`, `sim.rs`) uses the particle's index as it stands (0 when no
> controller-3 key has run) and does not advance the key's counter for it.
> Answer: neither; the controller-3 row of the base-fix table gives the client's rule (fill an
> empty slot from the leaving key's counter, advance the counter, no random draw). The
> implementer's note of 2026-10-10 below confirms that the simulation now follows it.

> **Answered (2026-10-10, BIN fifth pass, P57–P58; was OPEN, implementer, 2026-10-10):** The
> base-fix row for a leaving controller 1 leaves two
> points open (not reached by the data; the review of 2026-10-10 notes (b) and (d)): (1) the
> span of "run once to the new key's time": a particle age or a clock time, and measured from
> which time; (2) whether that walk fills an empty point-index slot (controller 1 never ran an
> update before the switch), and from which index. The run-time simulation (`crates/particles`,
> `sim.rs`) reads the span as the clock time of the new key (spawn time + its `time`) minus the
> emitter's previous particle-update time, at least 0; walks from the current position with
> the filled slot; and with an empty slot leaves the base and the slot unchanged. It also
> raises the starting value of both `rotation` and `world_rotation` at every base fix (review
> note (c); only one of them is drawn, §8.12 item 3, so the choice is not visible). The note of
> 2026-10-09 just above is answered by the base-fix table; the simulation now follows it.
> Answer, item by item (the controller-1 row of the base-fix table states the full rule):
> (1) **Confirmed.** The span is a clock-time difference: the new key's clock time (spawn time
> + its `time`) minus the emitter's previous particle-update time, the one stored before the
> running update. The client does not clamp it, but a span at or below 0 gives a budget at or
> below 0, which moves nothing, so "at least 0" is the same. The walk starts from the
> particle's current position with the filled slot, as the simulation does.
> (2) **Differs.** The base fix fills an empty slot: index 0, or, when the particle's physics
> flag 0x8 is set (the flags current before the new key's own 0x200 bit), an index drawn
> uniformly from 0 to (point count − 1); it does not touch the key's counter. It places the
> particle on *p*₀ + that point, rotated by the system's orientation, then walks the budget;
> the result becomes both the particle's position and the new base. With no points the base
> stays. Follow the table instead of leaving the base and the slot unchanged.
> (3) **Rotation (review note (c)).** The client keeps only one angle per particle, the one
> §8.12 item 3 draws (`world_rotation` when some component of it is not constant 0, else
> `rotation`), and raises only that one (§8.7, P58). Raising both draws the same, as the note
> says.
> Also corrected in §8.7 (controller 1 row): the first update under a controller-1 key walks
> its travel budget right after the placement on the start point, not only the later updates.

> **Answered (2026-10-09, BIN third pass; was OPEN, implementer, 2026-10-07):** (1) a key
> becomes current when the age equals its `time` (step 2: "at least"; the age is raised to 0
> first); (2) a channel that no visited key has set has no defined value in the client; the
> Tessera values are those of the rule above, which replaces "absent, left to the caller".

> **Answered (2026-10-09, BIN third pass; was OPEN, implementer, 2026-10-08):** the run-time
> simulation's choices compared with the client. Making the first key current at creation
> "whatever its `time`" equals the client's step 2 for every key of the data (all first keys
> have `time` 0); for a first key with a later `time` the client waits until the age reaches it,
> and an implementation follows step 2. Visiting a later key once the age is at least its
> `time` is the client's rule. Applying 0x200 before 0x100 in one key gives the same result as
> the client's order (step 3), because the controller's random point index is drawn later
> (step 4). The defaults for a channel without a key and "no motion without a controller key"
> are the rule above (the second is the client's behaviour, the first a Decision). The radial
> direction of an exploding emitter is the one place where the simulation differs: the client
> uses the shape point **after** the attachment of §8.5.5 (and before `origin`), not before
> it (§8.5.2, corrected wording).

<a id="s8-6-2"></a>
#### 8.6.2 Drawn size and colour

Drawn width and height = `scale_x` and `scale_y` of the scale channel × the owner-scale factor
*m* of §8.6.3 × the particle's `particle_scale` value when that curve is not the constant 1.
Drawn colour = the colour channel, with alpha multiplied by the emitter's fade envelope (§8.9),
then multiplied by the lighting of §8.14 when `use_lighting` = 1; each channel is then clamped
to [0, 1]. The system's opacity (§8.13, §8.15) multiplies the alpha in the draw call.

<a id="s8-6-3"></a>
#### 8.6.3 The owner's scale and `scale_mode` (BIN)

The owner supplies two scale vectors with every draw instance (§8.16), here *A* and *B*.
`scale_mode` selects the effective scale *S*:

| `scale_mode` | *S* |
|---|---|
| 1 `Total` | *A* × *B*, component by component |
| 2 `VDescOnly` | *B* |
| 3 `None`, and any other value | (1, 1, 1) |

- `world_space` = 1: *m* = the largest component of *S*; it scales the primitive (§8.6.2). In
  addition each particle's drawn position is scaled per axis about the system position recorded
  when the particle spawned: by *S*, or by the first vector after selection (*A* for `Total`,
  (1, 1, 1) otherwise) when the emitter has a bone or holding location and `stick_to_bones` = 0,
  or not at all when `stick_to_bones` = 1. With `stick_to_bones` = 1 the attachment offset itself
  is scaled per axis by that first vector.
- `world_space` = 0: *m* = 1; instead the whole particle set is drawn through the instance's
  transform, which scales by *S* per axis (non-uniform) about the instance's origin.

Which owner properties feed *A* and *B* is OPEN (Q4).

<a id="s8-7"></a>
### 8.7 Motion controllers

The current controller key (§8.6.1) chooses how the position changes. *p*₀ is the base
position (spawn position, or the position fixed at the last controller key), *v* the spawn
velocity, *τ* = age − time of the current controller key, *a* the force of §8.8.

| `controller` | Motion | Used by data |
|---|---|---|
| 2 | Ballistic: *p* = *p*₀ + *v* *τ* + ½ *a* *τ*². At a later controller key the new base is this position; the velocity stays the spawn velocity (the accumulated *a τ* is dropped). | 29,601 controller keys |
| 5 | Path curves: *p* = *p*₀ + (`position` X, Y, Z evaluated at *τ*). | 2,944 |
| 3 | Point placement: *p* = *p*₀ + one point of `points` rotated by the system's orientation. The index is the particle's point-index slot (§8.6.1 step 4). When the slot is empty, it is filled from a counter kept on the key (starting at 0): without physics flag 0x8 the particle takes the counter and the counter advances by one (wrapping); with flag 0x8 the particle takes the counter and the counter is then replaced by an index drawn uniformly from 0 to (point count − 1) (BIN). The base fix fills an empty slot in order, without the random draw (§8.6.1, P54). The index is then fixed for the particle's life, also under later controller-3 keys. The client uses the slot as an index into the current key's `points` without reducing it modulo the point count, so an index left by a key with more points reads past the list (BIN fourth pass); Tessera applies rule §10.12. With no points, *p* = *p*₀. | 33 |
| 6 (and any value not listed) | No motion: *p* = *p*₀. | 724 |
| 1 | Point path: the particle walks along the points of `points`, each rotated by the system's orientation and added to *p*₀. Its speed *s* is the length of its spawn velocity, that is \|`speed`\| at spawn (§8.5.4), stored per particle when any key of the emitter has controller 1. On its first update under the key, when the particle's point-index slot (§8.6.1 step 4) is still empty, the slot takes the start point: index 0, or, when physics flag 0x8 is set, an index drawn uniformly from 0 to (point count − 1) (BIN, P47), and the particle is placed on that point as controller 3 places it. With a filled slot (from an earlier controller-1 or controller-3 key) it continues from the slot's index, reduced modulo the point count (BIN fourth pass, P54). As it walks, the slot holds the index of the point it is moving toward or has just reached. On every update under the key, the first one included (right after the placement), it gets a travel budget *b* = *s* × (now − the emitter's previous particle-update time, the one stored before this update) and walks from its current position; at the spawn-time update of §8.5 step 9 that span is 0 (BIN fifth pass, P57). A base fix that leaves a controller-1 key runs the same walk once with the span of §8.6.1. While *b* > 0.0002: with *q*² the **squared** distance to the current target point, if *b* ≤ *q*² the particle moves √*b* towards that point and *b* becomes 0; otherwise it lands on the point, *b* decreases by *q*², and the target becomes the next point (wrapping after the last). (The budget is compared with, and reduced by, squared distances; this is the original's arithmetic.) With no points, *p* = *p*₀. | none (value 1 occurs only on keys without bit 0x100) |
| 4 | Stepped integration in steps of `time_step` (at most 0.25 s of catch-up per update). Each step forms a force term: *a* (mass-scaled gravity and wind, not multiplied by the step) plus, for every world force field whose radius contains the particle, a vector along (particle − field centre) of length strength × (1 − distance / radius) (zero when the distance is at most 0.0002), reversed for fields marked as attracting, with its X, Y or Z part zeroed when `constrain_x`, `constrain_y` or `constrain_z` is set. Then position += step × (velocity + force term) + (base position − position) × `snap`(age) × step, and velocity = (velocity + force term) × 0.95. Force fields: the client keeps one global list of force-field objects, each with a centre, a radius, a strength, an attracting flag and a lifetime. At each update, and only for emitters that have a key with controller 4, it gathers the entries that were refreshed in the current or previous frame, whose lifetime has not run out, and that pass one more check (BIN). A lifetime below 0.0002 (strictly; 0 and every negative value included) never runs out; any other lifetime has run out once at least that many seconds have passed since the entry's time stamp (BIN). **No code in this build adds an entry to the list**: its only references are this read and its release at exit, so the list is always empty and only *a*, `snap` and the damping act (BIN, P47). | none (value 4 occurs on no key at all) |

In all controllers, when `rotation_velocity` is not constant 0, the particle's angles (§8.5 step
9) grow by age × its `rotation_velocity` value. The client keeps **one** angle triple per
particle, the one that §8.12 item 3 draws: the `world_rotation` drawn at spawn when some
component of `world_rotation` is not constant 0; otherwise the `rotation` drawn at spawn when
some component of `rotation` or of `rotation_velocity` is not constant 0; otherwise none. The
other one is not stored, so it neither grows nor is raised (BIN fifth pass, P58). At every base
fix (§8.6.1) the spawn value that this growth starts from also grows by the new controller key's
`time` × the particle's `rotation_velocity` value, so the angle jumps by that amount when a second
or later controller key becomes current; the growth with age then continues from the raised value
(BIN fourth pass, P54). An implementation that also grows and raises the angle that is not drawn
draws the same. 350 emitters with a `rotation_velocity` that is not constant 0 have two or more
controller keys (OBS, 2026-10-10).

Controllers 1 and 4 are never used by the data of this install (OBS: no key with bit 0x100 has
controller 1, and no key at all has controller 4); the facts above matter only for other data.

<a id="s8-8"></a>
### 8.8 Forces and physics flags

| `physics_flags` bit | Meaning |
|---|---|
| 0x01 | Apply gravity: the world vector (0, 0, −9.8) m/s², rotated into the system frame |
| 0x20 | Apply wind: the client's current wind vector, rotated into the system frame (BIN). There is one global wind vector. It starts at (0, 0, 0). Its **target** is replaced on every update of the environment (sky and weather) by the environment's current wind vector; each time, the eased value at that moment becomes the new starting value. The value eases from the starting value towards the target exponentially with a **half-life of 0.3 s** (a client constant): *t* seconds after the last target change it is start + (target − start) × (1 − 2^(−*t* / 0.3)), and once it is within 0.0002 of the target on every axis it equals the target. The environment's wind vector is (keyframe curve 3, keyframe curve 4, 0) of the environment state in use, set as the target in every update of the world view (`particle-runtime.md` §11.2.1, RT49; 2026-10-23). |
| 0x08 | Random start point for controllers 1 and 3 |
| 0x02, 0x04 | No effect: no runtime routine that reads the particle's physics flags tests them (BIN, P47) |

*a* = mass × (gravity if 0x01) + mass × (wind if 0x20), mass from the mass channel (§8.6.1).

Each emitter stores its own copy of both vectors (BIN):

- **At system start** the client computes them for every emitter of the system: the gravity
  vector, and the eased value of the global wind at that moment, each rotated into the system
  frame. A vector whose length is 0.0002 or less is stored as it is, without rotation.
- **Recomputation.** The client has one routine that repeats this computation for every emitter
  of a running system, from the eased global wind at the moment of the call. Only the weather
  code calls it, for the systems listed in §8.8.1. Every other system keeps the vectors from its
  start for its whole life.
- **Use.** Each particle update reads the vectors stored on the emitter at that moment
  (controller 2 for its ½ *a* *τ*² term, controller 4 for each step). A recomputation
  therefore also changes the motion of particles that are already alive.

<a id="s8-8-1"></a>
#### 8.8.1 Systems whose forces the weather code recomputes (BIN)

When it initialises, the weather code creates one instance of each of the ten UNIQUEDB systems
of §7.1.3 and keeps them for the life of its state. It has two precipitation updates, one for
rain and one for snow, and each time it runs them it passes them some of those systems:

| Precipitation update | System whose gravity and wind it recomputes (all emitters) | Condition |
|---|---|---|
| Rain | UNIQUEDB key 0x83 (`RainGustParticleSystem`, 0x39000003 in this install) | none beyond the update running |
| Snow | UNIQUEDB key 0xFE (`SnowGustParticleSystem`, 0x390010DD in this install) | the weather's snow value exceeds 1 by more than 0.001; otherwise the snow update recomputes no system |

- The DIDs are read from UNIQUEDB at run time; the ones above are observations.
- Each precipitation update recomputes its system **once per run**, before it does anything
  else with the system.
- A precipitation update runs only when all of these hold:
  - the `[Render]` option `PrecipitationEffects` (a switch, default on) is on, and further
    client flags whose sources were not traced allow precipitation (Q13);
  - it would draw at least one rain streak or snow flake: the count is
    ⌊3,000 × clamp(value, 0, 1) × *q*⌋, the value being the weather's rain or snow value and
    *q* 0.3, 0.6 or 1 for the `[Render]` option `AtmosphericsDetail` at level 0 (`Low`), 1
    (`Medium`, the built-in value) or any other level (P59; `particle-runtime.md` §11.2.6
    step 1, revised 2026-10-23).
- The rain update also spawns particles directly from the 0x6C (`RainVaporParticleSystem`) and
  0x4A (`WaterRipplesParticleSystem`) systems and from the gust system, but it does not
  recompute the forces of the first two.
- No other code recomputes the forces of a running 0x39 system. That includes the other UNIQUEDB
  systems, script-started effects and the whitelist effects.

Labels are given as interoperability identifiers of the UNIQUEDB keys. The schedule of the two
updates (every update of the world view while the landscape is visible), the use of the rain and
snow values (thresholds 0.05, 0.2, 0.5 and 1.001) and the gates are `particle-runtime.md`
§11.2.2–§11.2.4 (2026-10-23), and the drops themselves are its §11.2.6; the remaining items are
Q13 below.

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):**
> `particle-runtime.md` §11.2.2 / §8.8.1: a precipitation update runs only when it would draw
> at least one drop, and that drop count needs the drop simulation (Q-RT13). Tessera runs both
> updates on every step while `PrecipitationEffects` is on and the landscape is visible.
> Confirm or give the gate.
>
> **Answered (spec writer, 2026-10-23; P59):** the count is the formula of the bullet above; it
> needs the rain or snow value and `AtmosphericsDetail` only, not the simulation. An update whose
> count is 0 recomputes no force and does nothing else (`particle-runtime.md` §11.2.2, revised
> the same day); running it anyway is not the client's behaviour.

<a id="s8-9"></a>
### 8.9 Emitter fade envelope

With *t* = emitter time, *F* = `fast_forward`, *i* = *F* + `fade_in`, *o* = *F* + `fade_out`:

1. Opacity *α* = 1.
2. If *i* > 0.0002 and *F* < *t* < *i*: *α* = (*t* − *F*) / (*i* − *F*).
3. If `time_limit` > 0.0002, *o* > 0.0002, *t* > *o* and |`time_limit` − *o*| > 0.0002:
   *α* ×= 1 − (*t* − *o*) / (`time_limit` − *o*).
4. Clamp *α* to [0, 1]. It multiplies the alpha of every particle of the emitter when drawn.

With the common `fade_out` = −1 and *F* = 0 there is no fade-out.

<a id="s8-10"></a>
### 8.10 Texture-atlas frames

When `frames_u` > 1 or `frames_v` > 1, each vertex's texture coordinates (*u*_c, *v*_c) ∈ {0, 1}
(the primitive's corner) are mapped to one cell of a `frames_u` × `frames_v` atlas. Let *U* =
`frames_u`, *V* = `frames_v`, *R* = `frame_rate`, *x* = the particle's age, *r* = its random
number (§8.5 step 9).

1. If `random_start_frame` = 1: if *R* = 0, *k* = integer part of *U* × *r* × *V*; add
   *k* div *V* to *u*_c and *k* mod *V* to *v*_c (a fixed random cell). If *R* ≠ 0, add *R* × *r*
   to *x*.
2. *u* = *u*_c / *U*; if *R* / *V* > 0.0002, add (⌊*x* × *R* / *V*⌋ mod *U*) / *U*.
3. *v* = *v*_c / *V*; if *R* > 0, add (⌊*x* × *R*⌋ mod *V*) / *V*.

So the frame counter ⌊*x* *R*⌋ runs down the rows (*v*) first and moves one column (*u*) every
*V* frames, looping over all *U* × *V* cells. (The client computes the modulo with
floating-point periods; the integer form above gives the same cell.)

<a id="s8-11"></a>
### 8.11 Primitive

`primitive` 1 (`Triangle`) draws 3 vertices per particle and 2 (`Quad`) draws 4 (BIN). Any
other value draws nothing.

Draw buffer (BIN): the client writes the vertices of one particle system into a single shared
dynamic vertex buffer created with room for **32,768 vertices** (8,192 quads), a client
constant. For each system it first draws every `Quad` emitter in stored order, then every
`Triangle` emitter in stored order. The vertices requested are capped at the buffer's size;
once an emitter's live particles do not all fit in the room left, the particles that fit are
drawn, and that emitter's other particles and every later emitter of the system are not drawn
in that frame.

A system is drawn only while its surface slot holds a loaded surface record (`surface`, or
`extra_surface` when `surface` = 0, §4.1); otherwise it draws nothing (BIN). The 2 records with
`surface` = 0 (and `extra_surface` = 0) draw nothing (OBS).

Corner texture coordinates (BIN): vertex 0 (0, 0), vertex 1 (0, 1), vertex 2 (1, 1), vertex 3
(1, 0), before the atlas mapping of §8.10. A `Triangle` particle uses vertices 0–2; a `Quad`
particle is drawn as the triangles (0, 1, 2) and (0, 2, 3).

<a id="s8-12"></a>
### 8.12 Billboard construction (BIN)

The client builds each primitive in the **camera frame**: *R* = the camera's right axis, *F* =
its forward axis (away from the viewer), *U* = its up axis. *P* is the particle's drawn position
(§8.1, §8.6.3) in that frame, *w* and *h* its drawn width and height (§8.6.2). `pivot` = (*pₓ*,
*p_y*, *p_z*) and `origin_offset` = (*oₓ*, *o_y*, *o_z*) are evaluated once per particle per draw
with the client clock at draw time ("now") as the curve time and phase 0, not with the particle's
age; this matters only for non-constant curves (185 `origin_offset` components, §5).

**Default corners.** For a corner with horizontal sign *a* and vertical sign *b* (each −½ or +½)
the vertex is *P* + *R* · *w* (*pₓ* + *a*) + *F* · *p_y* + *U* · *h* (*p_z* + *b*). Vertex 0 is
(−½, +½), vertex 1 (−½, −½), vertex 2 (+½, −½), vertex 3 (+½, +½): top-left, bottom-left,
bottom-right, top-right. So `pivot` X and Z shift the primitive in units of its width and height
(0 = centred on the particle), and `pivot` Y shifts it in metres along the view direction.

**`origin_offset`** is added to every vertex after all of the steps below: + *R* · *oₓ* + *F* ·
*o_y* + *U* · *o_z*, in metres.

**Modes.** The data selects the mode per emitter (constant-0 tests are on the curves as stored):

1. **`velocity_facing` = 1.** *d* = the particle's motion (current minus previous position,
   §8.1) in the camera frame, normalised (0 when its length is 10⁻⁶ or less); *s* = normalise(*d*
   × *F*). Vertex 0 = *P* + ½*h* *d* − ½*w* *s*, vertex 1 = *P* − ½*h* *d* − ½*w* *s*, vertex 2 =
   *P* − ½*h* *d* + ½*w* *s*, vertex 3 = *P* + ½*h* *d* + ½*w* *s*, each plus the pivot term *R* ·
   *w* *pₓ* + *F* · *p_y* + *U* · *h* *p_z*. The texture's top edge leads in the direction of
   motion. This mode takes precedence over `streak`.
2. **`streak` not constant 0** (and `velocity_facing` = 0). Each particle has an **anchor** *Q*,
   interpolated like its position (§8.1): for controllers 2 and 5 the position the particle had
   `streak` seconds earlier on its current controller path (the formula of §8.7 at max(0, *τ* −
   `streak`)); for controller 3 its current position; for controllers 1, 4 and 6 the anchor keeps
   its last value (the spawn position unless an earlier controller moved it). Let *n* = the
   offset *P* − *Q* in its *R* and *U* components, normalised; when its length is 10⁻⁶ or less the
   default corners are used. Otherwise "current XX" below is a default corner around *P* and
   "anchor XX" the same corner around *Q* (its *F* component *Q_F* + *p_y*). With *nᵣ*, *nᵤ* the
   two components:

   | Direction of *n* | Vertices 0, 1, 2, 3 |
   |---|---|
   | \|*nᵣ*\| ≤ 0.38268 or ≥ 0.92388, \|*nᵣ*\| ≤ \|*nᵤ*\|, *nᵤ* > 0 (moving up) | current TL, anchor BL, anchor BR, current TR |
   | same, *nᵤ* ≤ 0 (moving down) | anchor TL, current BL, current BR, anchor TR |
   | \|*nᵣ*\| ≤ 0.38268 or ≥ 0.92388, \|*nᵣ*\| > \|*nᵤ*\|, *nᵣ* > 0 (moving right) | anchor TL, anchor BL, current BR, current TR |
   | same, *nᵣ* ≤ 0 (moving left) | current TL, current BL, anchor BR, anchor TR |
   | otherwise (diagonal), *nᵣ* and *nᵤ* of the same sign | current BR, current TL, anchor TL, anchor BR |
   | otherwise (diagonal), opposite signs | anchor BL, current BL, current TR, anchor TR |

   For the sign test a component counts as positive above 0.0002 and as negative at 0 or below.
   The limits 0.38268 and 0.92388 are sin 22.5° and cos 22.5° (client constants). The primitive
   thus stretches from the anchor to the particle and keeps its screen-aligned texture.
3. **Rotation.** When every component of `world_rotation` is constant 0 and some component of
   `rotation` or of `rotation_velocity` is not, the angles *r* = `rotation` drawn at spawn
   (radians), plus age × `rotation_velocity` when that is not constant 0. When some component of `world_rotation` is
   not constant 0, `rotation` is ignored and *r* = `world_rotation` drawn at spawn, plus age ×
   `rotation_velocity`. With *a* = −*r*ₓ, *b* = −*r_y*, *c* = −*r_z*, the client forms the matrix
   *M* whose rows (components along *R*, *F*, *U*) are
   - (cos *c* cos *b*, sin *b*, −sin *c* cos *b*),
   - (sin *c* sin *a* − cos *c* sin *b* cos *a*, cos *b* cos *a*, sin *b* cos *a* sin *c* + cos *c* sin *a*),
   - (sin *b* sin *a* cos *c* + sin *c* cos *a*, −cos *b* sin *a*, cos *c* cos *a* − sin *b* sin *a* sin *c*),

   builds the corners (by the default rule or mode 1 or 2) around the origin instead of *P*,
   multiplies each corner as a row vector by the matrix, and then moves it to *P*.
   - For `rotation` the matrix is *M* itself, so the primitive still faces the camera: with only
     *r_z* ≠ 0 it turns counter-clockwise by *r_z* in the screen plane (about *F*); *r*ₓ alone
     tilts it about *R* and *r_y* alone turns it about *U*.
   - For `world_rotation` the matrix is *M* followed by an orientation frame *O* and then the
     camera transform, so the primitive keeps a fixed orientation in the world instead of facing
     the camera. *O* is the system's current orientation, or, when `world_space` = 1 and
     `lock_world_rotation` = 1, the system's orientation captured when the particle spawned.
     `lock_world_rotation` has no other effect.

How the camera frame and *O* combine with the translation of a local-space system is part of Q8.

<a id="s8-13"></a>
### 8.13 Distance fade and the not-drawn state (BIN)

For every draw instance (§8.16) the client takes *d* = the distance from the camera to the
instance's position and the instance's opacity *α* (§8.15), then:

1. If *d* > `far_fade_end`, or `near_fade_start` > 0 and *d* ≤ `near_fade_start`, the instance
   is not drawn.
2. Else if *d* > `far_fade_start` and `far_fade_end` − `far_fade_start` > 0.0002: *α* ×= 1 −
   (*d* − `far_fade_start`) / (`far_fade_end` − `far_fade_start`).
3. Else if *d* ≤ `far_fade_start`, `near_fade_start` > 0, `near_fade_start` < *d* ≤
   `near_fade_end` and `near_fade_end` − `near_fade_start` > 0.0002: *α* ×= (*d* −
   `near_fade_start`) / (`near_fade_end` − `near_fade_start`).

*α* multiplies the alpha of every vertex of that instance. The client applies this to every 0x39
system; no code path switches it off. When no instance of a system was drawn in a frame (step 1
for all of them), the system enters the **not-drawn state**: its updates (§8.1) are skipped until
a draw passes step 1 again; that draw first runs one update and then draws. A system far away
therefore freezes, and the catch-up of §8.3 limits what it emits when it comes back.

<a id="s8-14"></a>
### 8.14 Lighting (`use_lighting`, BIN)

With `use_lighting` = 1 each particle's colour (after the fade envelope) is multiplied, channel
by channel, by a light sum: the red, green and blue factors are the ambient colour's channel plus,
for each of at most 5 point lights, the light's colour channel × the light's attenuation at the
distance between the particle and the light; the alpha factor is the ambient colour's alpha. The
owner supplies the ambient colour and the light list each frame: the ambient colour is the mean of
two colours of the scene's current lighting state, and the light list holds the scene's global
light (when there is one) followed by the lights that affect the owner. Which two lighting colours
are averaged and the attenuation function are Q8. With `use_lighting` = 0 the colour is not
lit. A lit system is never shared (§8.16).

<a id="s8-15"></a>
### 8.15 Draw order, passes and opacity (BIN)

- **Order.** Each frame the client collects the systems placed in that frame, sorts them by
  `sorting_priority` in ascending order with a stable sort, and draws them from the end of the
  sorted list: the highest priority first, the lowest last. Systems of equal priority are drawn in
  the order of the client's list of running systems (oldest first).
- **Passes.** Systems with `uses_video_post` = 0 are drawn in the main scene pass (when particle
  drawing is enabled). Systems with `uses_video_post` = 1 are drawn in a second pass after the
  scene's post-processing step, and only while post-processing is active with its render target;
  otherwise they are not drawn at all. 139 records set it (OBS).
- **Opacity.** The opacity of a draw instance is the instance's own opacity × the opacity of the
  scene object that carries the effect. That object's opacity is 1, multiplied by its parent
  object's current opacity when `inherit_opacity` = 1 and the parent was placed in the same
  frame. The distance fade (§8.13) multiplies the result.
- **`force_draw` and `draw_reflection`** are read by no code of the particle simulation, the
  particle draw passes or the scene renderer (every one-byte read at their offsets in the
  executable was checked); they are reachable only through the generic property interface. They
  have no visible effect in this build.

<a id="s8-16"></a>
### 8.16 Sharing and draw instances (BIN)

- **Requests.** When a script starts an effect, the client searches its running systems, newest
  first, for one made from the same DID that can be shared: `shareable` = 1, `world_space` = 0
  and `use_lighting` = 0. If it finds one, the new request becomes another user of that running
  system (the same particles); otherwise it creates a new system. 2,583 records meet the condition
  (OBS).
- **Instances.** Every frame each user that places the system adds a **draw instance**: its
  transform, its two scale vectors (§8.6.3) and its opacity. The system's particles are drawn once
  per instance, each time through that instance's transform (for `world_space` = 0). The first
  instance of a frame triggers the system update (§8.1) and supplies the frame that emission,
  spawning and attachments use in that update.

<a id="s8-17"></a>
### 8.17 System start, `fast_forward`, `physics_duration` and restart (BIN)

- **Start.** At the system's first placement the client sets the system start to that time minus
  `fast_forward`, and every emitter's last particle-update time, last-burst time and last
  distance-emission time to system start + `start_time`. With `fast_forward` > 0 the system thus
  behaves as if it had started `fast_forward` seconds earlier (pre-simulation through §8.3), and
  the fade envelope (§8.9) starts at emitter time `fast_forward`.
- **Restart.** When `fast_forward` > 1 and more than 100 frames have passed since the system was
  last placed, the next placement restarts it: all live particles and running totals are cleared,
  the start is set again as above, and the pre-simulation repeats. 836 records have
  `fast_forward` > 1 (OBS).
- **`physics_duration`.** When it is not 0, the system update (emission and particle updates)
  runs only while (now − system start) ≤ `physics_duration`; afterwards the particles stay as they
  are. 0 in every record (OBS).
- **How the Particle node classifies a 0x39 effect.** The node that starts it treats the effect
  as never-ending (keeping a controller that stops it when the script ends, `script-table.md`
  §5.3) when `physics_duration` > 0, or when some emitter has `emission_limit` = 0 and
  `time_limit` = 0. Otherwise it computes, per emitter, max(`time_limit`, (`emission_limit`
  integer-divided by `burst_count`) × `emit_interval`) + *L*′ + `start_time` − `fast_forward`,
  where *L*′ is `w[0]` of `lifespan` plus |`w[2]`| when the curve is not constant; the effect is
  never-ending only when the largest of these is negative.

<a id="s9"></a>
## 9. Values that carry gameplay meaning

The particle records hold only presentation data: no record of either family carries a
gameplay value (damage, range, cost, duration of an effect on a character). Gameplay reaches
them only by **selection**:

- a skill, effect, item or creature runs a script whose Particle or Beam node names the record
  (§7.1.1); the node's own fields (`ParticleScript_HoldingLocation`, `ParticleScript_BoneType`,
  fade distances (fork effects only, §6.5), `ParticleScript_DropEffect`, `ParticleScript_OnWater`,
  `ParticleScript_ImmediateDisposal`, `ParticleScript_RedirectProperty`, base-block intensity,
  ST §5.3) decide where and how long it plays;
- the owner's kind and the client options of §7.1.2 decide whether it starts at all;
- the whitelist of §7.1.2 decides which effects survive a reduced dynamic-particle setting;
- weather and water effects come from UNIQUEDB keys (§7.1.3).

Timing values that a player can perceive (effect length) follow from `time_limit`,
`emission_limit`, `lifespan`, `start_time` and the end test of §8.2.

---

<a id="s10"></a>
## 10. Tessera rules (Decision)

1. Decode 0x39 records completely (§4.4) and reject any leftover byte.
2. Decode the outer layer of 0x44 records (§6.1) with the version-3 reading; keep the blob as
   opaque bytes. Until Q1 is answered, an effect named by `ParticleScript_ForkParticleID` is
   not simulated: the explorer shows the record's tables and bounds, and the client draws
   nothing for it.
   *Note (spec writer, 2026-10-23):* re-examined for the question "are fork effects drawn or
   not": the rule stands. 1,524 Particle nodes (3.7 %) start one of 219 fork effects
   (`particle-runtime.md` RT57); nothing beyond §6.4–§6.5 is known that would let Tessera draw
   them, so drawing them waits for Q1 or for an owner decision on a stand-in look
   (`particle-runtime.md` §10.2).
3. Evaluate curves with `material.md` §5.3.1 at the times given in §8; mode 3 and every
   "random" of §8 may use any uniform generator (the original's sequence is shared with
   unrelated code and cannot be reproduced).
4. A Particle node starts the record chosen by §7.1.1 (particle field first, then fork field,
   by the DID's DB type), as the client does. A 0x44 record named in the particle field is a fork
   effect and falls under rule 2.
5. `time_step` that is not a finite number above 0 (never in the data; §8.1): use 1/120 s, the
   smallest observed value, for the update cadence of §8.1 and the step of controller 4. The
   client would update on every call (≤ 0) or never (not a number), and controller 4 would not
   terminate.
6. `burst_count` = 0 (never in the data): follow the client. The emitter spawns nothing by
   timed or distance emission (§8.3 step 5, §8.4), and the emission-limit condition of §8.2
   never ends it; implement that condition as "never holds" instead of dividing by zero.
7. Controller 4 (never in the data) runs with an empty force-field list, as in the client
   (§8.7): only *a* (gravity and wind), `snap` and the 0.95 damping act.
8. Timed emission with hostile values (§8.3; never in the data):
   - an `emit_interval` that is negative or not a finite number counts as 0, so the emitter has
     no timed emission;
   - for a finite positive `emit_interval`, follow §8.3, and stop step 4 when advancing the
     last-burst time leaves it unchanged (then set it to now);
   - before step 1, clamp cap to 100,000, the largest `max_particles` in the data (OBS);
   - compute the step-2 increment in 64-bit arithmetic and saturate the running total at
     2³² − 1 instead of wrapping.

   One update then spawns at most 100,000 particles per emitter. The client would also run step
   4 for the excluded values (§8.3), and its cap is limited only by the 32-bit fields.
9. Distance emission (§8.4) uses the same clamped cap as rule 8, and an `emission_distance` that
   is negative or not a finite number counts as 0 (off). The client's loop already ends when room
   reaches 0 or after *n* + 1 steps.
10. `force_draw` and `draw_reflection` are decoded and shown, and have no effect (§8.15).
11. A keyframe channel without a current key (§8.6.1; not reached by the data): the client
    reads it through an empty reference and has no defined result. Use scale (1, 1), opaque
    white (1, 1, 1, 1) and mass 1, the values of an emitter without keys. A particle without a
    current controller key does not move, as in the client.
12. A point index at or beyond the point count of the controller-3 key that uses it (§8.7; the
    slot was filled under an earlier controller-1 or controller-3 key with more points; not
    reached by the data, where no emitter has two controller keys with controller 1 or 3, P56):
    the client reads past the key's point list. Reduce the index modulo the key's point
    count, as controller 1 does, for the placement and for the base fix; the slot itself keeps
    its value.

---

<a id="s11"></a>
## 11. Invariants

Checkable against the install (all hold in this install, OBS) or against synthetic fixtures:

- **PS1**: every 0x39 record decodes by §4.4 with zero leftover bytes (11,155 of 11,155).
- **PS2**: every bool8 is 0 or 1; every curve mode is 1–8 or 10; every mode-10 loop byte is 0 or
  1.
- **PS3**: `extra_surface` = 0; `surface` is 0 or a DID with prefix 0x31 that exists.
- **PS4**: `far_fade_start` ≤ `far_fade_end` in every record.
- **PS5**: `has_bounds` = 1 implies `bounds_min` ≤ `bounds_max` on each axis.
- **PS6**: `shape` ∈ 1–5, `primitive` ∈ 1–2, `scale_mode` ∈ 1–3; `holding_location` and `bone`
  are keys of their mappers.
- **PS7**: every emitter has at least one keyframe; the first key has time 0 and flags 0x770
  or 0x7F0.
- **PS8**: all VLEs of both families are in the 1-byte form.
- **PS9**: `time_step` is a finite number above 0 (1/120 to 0.1 observed); `burst_count` ≥ 1;
  `emit_interval` is 0 or a finite number above 0 (0.000263 to 100,000 observed);
  `max_particles` ≤ 100,000; `snap` is a constant (mode 1) curve; no keyframe has `controller` 4,
  and no keyframe with bit 0x100 has `controller` 1.
- **PS10** (synthetic; §8.6.1 base fix): an emitter whose first key (time 0, bits
  0x100 | 0x200, `controller` 3, two points *P*₀ and *P*₁, physics flags 0x8) is followed by a
  second controller key at time 0.01 with `controller` 6, in a system with `time_step` 0.05 and
  the identity orientation:
  a particle spawned at least 0.01 s before the emitter's last particle-update time (a catch-up
  burst, §8.3) makes both keys current in the walk of its spawn-time update (§8.5 step 9); its
  base becomes spawn position + *P*₀, the first key's counter becomes 1 (no random draw), and the
  particle then stays at spawn position + *P*₀. A second particle created the same way gets
  *P*₁, a third *P*₀ again (as long as no particle of the emitter runs controller 3 itself, which
  would replace the counter by a random index).
- **PS11** (synthetic; §8.6.1 base fix of a leaving controller 1, §8.7 controller 1): an emitter
  with three keys, all with bit 0x100 and physics flags 0 (the first also with bit 0x200): at
  time 0 `controller` 6; at time 0.02 `controller` 1 with the two points (0, 0, 0) and
  (10, 0, 0); at time 0.03 `controller` 6; constant `speed` 10 (so *s* = 10), `time_step` 0.05,
  the identity orientation. A particle spawned at the clock time *T* of one of the emitter's
  particle updates and next updated at *T* + 0.05 passes the second and third keys in that
  update's walk. The third key's base fix fills the empty slot with index 0, places the
  particle on its spawn position, and walks *b* = 10 × (*T* + 0.03 − *T*) = 0.3: it reaches
  point 0 at once, then moves √0.3 ≈ 0.5477 m towards point 1. Its base and position become
  spawn position + (0.5477, 0, 0), the slot holds 1, and the particle stays there. With physics
  flags 0x8 on the first key the start index is drawn from {0, 1} instead, and from index 1 the
  particle ends at spawn position + (9.4523, 0, 0).
- **FK1**: every 0x44 record decodes by §6.1 with zero leftover bytes (254 of 254).
- **FK2**: blob words 0 and 1 are 0x50 and 0x1E8; the u32 at blob offset 0x120 is 488.
- **FK3**: `transform` length is 64 and its element 15 is 1.0; `name` is empty; `properties` is
  empty.
- **FK4**: each `name_table` key equals the last path component of its value; each `resources`
  DID has prefix 0x41 or 0x44 and exists; a 0x41 entry's name is a `name_table` key, and a 0x44
  entry's name plus the effect extension is one.
- **RF1**: every Particle node names exactly one of `ParticleScript_ParticleID` and
  `ParticleScript_ForkParticleID`.
- **RF2**: every DID that a Particle or Beam node, the whitelist or UNIQUEDB names exists.

---

<a id="s12"></a>
## 12. Worked synthetic examples

Values below are invented for testing; they are not game data.

<a id="s12-1"></a>
### 12.1 Minimal 0x39 record (77 bytes)

`time_step` 0.05, `world_space` 1, far fade 30–60, `scale_mode` 1, no emitters, no bounds,
`sorting_priority` 5, `shareable` 1, `surface` 0x31001234:

```
cd cc 4c 3d 00 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 f0 41 00 00 70 42 00 00 00 00 00 00
00 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
05 00 00 00 01 34 12 00 31 00 00 00 00
```

Decoding: bytes 0–3 0.05; 4–11 two zero floats; 12 `world_space` 1; 13–17 five zero bools;
18–21 30.0 (note the odd offset); 22–25 60.0; 26–33 two zero floats; 34–37 `scale_mode` 1;
38 emitter count 0; 39 `has_bounds` 0; 40–63 six zero floats; 64–67 `sorting_priority` 5; 68
`shareable` 1; 69–72 `surface` 0x31001234; 73–76 `extra_surface` 0.

<a id="s12-2"></a>
### 12.2 One emitter, one keyframe (447 bytes)

Same system part with emitter count 1, then one emitter whose curves are all mode 1, then the
same tail. Field offsets from the start of the record:

| Offset | Field | Value |
|---|---|---|
| 39 | `origin` (3 × mode 1) | (0, 0, 0) |
| 63 | `origin_offset` | (0, 0, 0) |
| 87 | `pivot` | (0, 0, 0) |
| 111 | `shape` | 3 (Sphere) |
| 115 | `primitive` | 2 (Quad) |
| 119 | `emit_interval` | 0.1 |
| 123 | `speed` | constant 2.0 |
| 131 | `direction` | (0, 0, 1) |
| 155 | `emission_limit` | 0 |
| 159 | `lifespan` | constant 1.5 |
| 167 | `shape_size` | constant 0.5 |
| 175 / 183 | `min_spread` / `max_spread` | 0 / 30 |
| 191 | `particle_scale` | constant 1.0 |
| 199 | `rotation` | (0, 0, 0) |
| 223 | `max_particles` | 50 |
| 227 / 228 | `exploding` / `lock_world_rotation` | 0 / 0 |
| 229 | `streak` | constant 0 |
| 237 | `world_rotation` | (0, 0, 0) |
| 261 | `rotation_velocity` | (0, 0, 0) |
| 285 | `burst_count` | 2 |
| 289 / 293 / 297 | `start_time` / `time_limit` / `emission_distance` | 0 / 0 / 0 |
| 301 | `snap` | constant 0.1 |
| 309 / 310 | `inclusive` / `active` | 1 / 1 |
| 311 / 315 | `fade_in` / `fade_out` | 0 / −1 |
| 319 | constrain bytes | 0, 0, 0 |
| 322 / 326 / 330 | `frames_u` / `frames_v` / `frame_rate` | 2 / 2 / 8 |
| 334 | `random_start_frame` | 0 |
| 335 | keyframe count (VLE) | 1 |
| 336 | key: `time` | 0 |
| 340 | key: `flags` | 0x770 |
| 344 / 348 | key: `scale_x` / `scale_y` | 0.2 / 0.2 |
| 352 | key: `colour` | 0xFFFF8000 |
| 356 | key: `mass` | −0.1 |
| 360 / 364 | key: `controller` / `physics_flags` | 2 / 1 |
| 368 | key: `point_count` | 0 |
| 372 | key: `position` | (0, 0, 0) |
| 396 / 400 | `holding_location` / `bone` | 0 / 0 |
| 404 / 405 | `stick_to_bones` / `velocity_facing` | 0 / 0 |
| 406 | `name` | length 2, then 2 bytes |
| 409 | tail (`has_bounds` …) | as in §12.1 from offset 39 |

<a id="s12-3"></a>
### 12.3 Behaviour of example 12.2

- **Emission:** 2 particles every 0.1 s = 20 per second; with lifespan 1.5 about 30 are alive,
  below the cap of 50. No `time_limit` and no `emission_limit`: the system never ends by itself
  (§8.2).
- **Spawn:** sphere of radius 0.25 m, filled (`inclusive`); direction +Z tilted by 0–30°.
- **Motion:** controller 2 with gravity: *a* = −0.1 × (0, 0, −9.8) = (0, 0, 0.98). A particle
  launched straight up at 2 m/s is at height 2 × 1 + ½ × 0.98 × 1 = 2.49 m above its spawn point
  after 1 s.
- **Frames:** at age 0.3 s the frame counter is ⌊0.3 × 8⌋ = 2, so the row index is 2 mod 2 = 0
  and the column index is ⌊0.3 × 8 / 2⌋ mod 2 = 1: the vertex at corner (0, 0) gets texture
  coordinates (0.5, 0.0).
- **Fade envelope** (variant with `fade_in` 0.5, `fade_out` 2.0, `time_limit` 3.0): at emitter
  time 0.25 *α* = 0.5; at 1.0 *α* = 1; at 2.5 *α* = 1 − 0.5 / 1.0 = 0.5.
- **Channels** (variant with a second key at time 1.0, `flags` 0x2, scale 1.0): at age 0.25 the
  drawn scale is 0.2 + 0.25 × (1.0 − 0.2) = 0.4; the colour stays 0xFFFF8000 because the second
  key lacks bit 0x4.

<a id="s12-4"></a>
### 12.4 Minimal 0x44 outer layer

A record with a 488-byte blob (header only) and one texture:

| Order | Bytes | Meaning |
|---|---|---|
| 1 | `e8 01 00 00` | `blob_length` 488 |
| 2 | 488 bytes | blob (opaque here) |
| 3 | `00` | `flag` 0 |
| 4 | `00` | `name` empty |
| 5 | `00 01`, then `05 "a.dds"`, then `08 "fx/a.dds"` (lengths as VLE) | `name_table`, hint 0, one pair |
| 6 | `00 00` | `properties` empty |
| 7 | `40 00 00 00` + 64 bytes | `transform` (identity: 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1) |
| 8 | `00 01`, then `34 12 00 41`, then `05 "a.dds"` | `resources`: 0x41001234 → "a.dds" |
| 9 | 8 bytes | `tail_value` (f64) |
| 10–11 | 24 bytes | bounds |

---

<a id="s13"></a>
## 13. Open questions

Questions are numbered for stable reference; closed ones keep their number with a pointer to the
answer.

> **OPEN (Q1):** Internal layout of the 0x44 effect image (§6.2) beyond the framing facts, and
> the third-party library's simulation and drawing: the meaning of the 488-byte header and its
> sub-structures and relocation, the emitters and their parameters, which UNIQUEDB blend-mode
> shader (§6.4) an effect uses, and what the library does with the gravity and wind vectors.
> Suggested method: M4 on the library loader (the routine with the 488-byte size check, which
> widens the header to 552 bytes in memory) and on the library update and draw entry points that
> the fork system calls. Rule §10.2 applies until then.

> **OPEN (Q2):** Two 0x44 items remain: (a) what the client does with the bounds (items 10–11; no
> reader was found among the reads at their in-memory offsets; a reader that copies the whole
> block was not searched for), and (b) where the read context gets the record and item versions
> (tags 0x46505344 and 0x4643444E, §6.1) when it reads a stored record. Neither changes the
> decoding of this install. `flag`, `tail_value` and `transform` are answered in §6.1 and §6.5.

Q3 — closed 2026-10-08: catch-up `lifespan` time in §8.3 step 2; `pivot` / `origin_offset` time
in §8.12.

> **OPEN (Q4):** Which properties of the owning object feed the two scale vectors *A* and *B* of
> §8.6.3 (the label `VDescOnly` suggests that *B* comes from the visual description). The
> selection by `scale_mode` and its use are answered in §8.6.3. Suggested method: M4 on the code
> that fills the draw instance before the per-frame placement call, or M3 (an effect on a
> creature drawn at a non-unit scale, compared across the three `scale_mode` values).

Q5 — closed 2026-10-08: §4.1, §8.13–§8.16.

Q6 — closed 2026-10-08: §8.17.

Q7 — closed 2026-10-08: no effect (§8.6.1, §8.8).

> **OPEN (Q8):** Three drawing details remain: (a) the frames of a local-space system
> (`world_space` = 0): which of the two transforms that the owner supplies with each draw instance
> places the particles at spawn and which one draws them, and how the translation of the
> orientation frame *O* of §8.12 combines with the particle position in `world_rotation` mode;
> (b) the light attenuation function of §8.14 and which two colours of the scene's lighting state
> form the ambient colour (see `lighting.md`); (c) in `world_rotation` mode, how the streak anchor
> of §8.12 mode 2 is used (the client takes it without the camera transform there). Billboard
> construction, axes, modes, `stick_to_bones` and `lock_world_rotation` are answered in §8.5.5 and
> §8.12.
>
> *Note (2026-10-10, P2):* for (b), `particle-runtime.md` rule 13 gives a Tessera Decision (the
> frame's ambient term and the owner's light list of `lighting.md`, at most 5 lights, the light
> records' attenuation without the smooth fade), so that lit systems can be drawn; the client's
> own attenuation and ambient source stay OPEN here.

Q9 — closed 2026-10-08: §8.1 (scheduling), §8.3 (start value), §8.4 (anchors).

Q10 — closed 2026-10-08: §7.1.2.

Q11 — closed 2026-10-08: §4.1, §8.11.

Q12 — closed 2026-10-08: §7.1.1; rule §10.4 now follows the client.

> **OPEN (Q13):** These questions remain about the UNIQUEDB weather and water systems (§7.1.3)
> and the global wind of §8.8 (see `environment.md`):
>
> - when and where each system emits; the rain update's direct spawns from the 0x6C, 0x4A and
>   0x83 systems (§8.8.1) were seen but not traced in detail;
> - how often the weather code runs its rain and snow precipitation updates (every frame or
>   otherwise), and so how often it recomputes the forces of the 0x83 and 0xFE systems;
> - what the weather's rain and snow values mean and where they come from;
> - which client flags, besides the `PrecipitationEffects` option, enable precipitation;
> - how the environment computes the wind vector that becomes the target of the global wind.
>
> Already known: the easing of the global wind (0.3 s half-life, §8.8); which systems are
> recomputed and under which condition (§8.8.1); that no other code recomputes a running system;
> and that the precipitation update returns at once when the `[Render]` option
> `PrecipitationEffects` (a switch, default on) is off (BIN, 2026-10-08).
>
> *Answered in part (2026-10-23, `particle-runtime.md` BIN pass (n)):* when each system is placed
> and which spawns it receives, how often the updates run, what the rain and snow values do on
> the particle side, and how the environment's wind vector is formed are `particle-runtime.md`
> §11.2. Still open: the weather code's own drop simulation and the rest listed in
> `particle-runtime.md` Q-RT13, and which client flag besides `PrecipitationEffects` gates the
> step (`particle-runtime.md` §11.2.2 step 1).
>
> *Answered further (2026-10-23, `particle-runtime.md` BIN pass (o)):* the drop count and its
> quality factor (§8.8.1, P59) and the rain drop simulation are `particle-runtime.md` §11.2.6;
> what stays open is listed in its Q-RT13 (snow flakes, the marked bones, the gating flag, the
> collision filter, the drops' draw call).

Q14 — closed 2026-10-08: §8.7 (the force-field list is never filled; controller-1 start index).

Q15 — closed 2026-10-10: §8.6.1, "Emitter-level conditions" (the key walk runs for every
emitter with keyframes; the physics-flag copy needs a key with `controller` 1–4 and has no
visible effect otherwise).

<a id="s14"></a>
## 14. Notes for other specs

- `records.md` §2.1: rows 0x39 and 0x44 can point here; both layouts are decoded (0x44 outer
  layer only). Its "layouts of 0x39 … 0x44 OPEN" sentences are answered by §4 and §6.1.
- `environment.md` §9: the 0x39/0x44 OPEN is answered here; the weather particle systems are
  the UNIQUEDB entries of §7.1.3. The precipitation updates of §8.8.1 read the same eased global
  wind as §8.8. The rain update's fall direction is the unit vector of (eased wind + (0, 0,
  −9.8)), or (0, 0, −1) when that sum is 0.000001 long or shorter (BIN). This is a weather fact
  for that spec, not a particle rule.
- `script-table.md` §5.3: the Particle node's two DID fields are mutually exclusive (§2.2); 7
  nodes put a 0x44 DID in the 0x39 field, and the client starts it as a fork effect (§7.1.1); the
  node's fade fields apply only to fork effects (§6.5).
- `property-system.md` §9.1: the 272 type-0x1E values in 0x78 sets are the whitelist of §7.1.2,
  found through UNIQUEDB key 0x1E2; the 10 UNIQUEDB values are §7.1.3.
- `environment.md`: the precipitation update returns at once when the `[Render]` option
  `PrecipitationEffects` is off (§8.8.1).
- `script-table.md` §5.3: the Particle node's start gating (§7.1.2) and its never-ending
  classification of 0x39 and 0x44 effects (§6.5, §8.17).
- `lighting.md`: §8.14 needs the point-light attenuation and the ambient colour of the scene's
  lighting state (Q8 b).
- Tessera's settings: the client options `DynamicParticles`, `StaticEnvironmentalObjects` and
  `PrecipitationEffects` (`[Render]`) change which effects run (§7.1.2, §8.8.1).
- `particle-runtime.md` §11.1 (2026-10-10): the particle systems a Beam node names
  (`BeamScript_BeamParticleSystem`, `BeamScript_HeadParticleSystem`) are created by the beam,
  without the start gating of §7.1.2, and receive their particles from the beam (spawned at
  points along the beam or at its head); whether they also emit by themselves is
  `particle-runtime.md` Q-RT11; they otherwise simulate and draw by §8. `combat-visuals.md` §10.1 categorises every record for P2.

These are suggestions for the owners of those specs; this spec does not edit them.

---

<a id="s15"></a>
## 15. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-07 | all | First version, status Review. Layouts of 0x39 (complete) and 0x44 (outer layer) from M4 (row BIN) confirmed over all records (OBS); censuses; DID graph; runtime behaviour of 0x39 systems (BIN); Q1–Q13. |
| 2026-10-07 | §0, §1, §4.1, §4.7, §5.2, §5.3, §8.1–§8.4, §8.6.1, §8.7, §8.8, §8.11, §10, §11, §13 | Review issues answered (status stays Review). (1) `snap`: 0.1 in 32,627 of 32,789 emitters, not all; other values added to §5.2 (OBS, all emitters). (2) Controller 1 speed = length of the spawn velocity, and its squared-distance walk; controller 4 force-field list and its gathering rule, source OPEN as new Q14 (both controllers unused by the data, stated); wind half-life 0.3 s, start value and target from the environment update (BIN); draw buffer 32,768 vertices, `Quad` emitters before `Triangle` emitters (BIN). (3) `time_step` ≤ 0 or not a number (§8.1, BIN) and `burst_count` = 0 (§8.2 item 4, §8.3 step 5, §8.4, BIN), Tessera rules §10.5–§10.6. (4) §8.6.1: interpolation targets as computed by the client (next later key with the bit, else the key itself), the target-bit test, and the single resulting rule. (5) Provenance rows P31–P37 added (§2.1, §6.3, §8.6.2, §8.12, §9, §10 as Decision, censuses); P17–P20, P22–P24, P27 widened. (6) §4.1: offset 39 holds only for the 1-byte VLE form. Corrections found while re-measuring: §8.3 said `emit_interval` = 0 tops the emitter up every update; the client skips timed emission entirely (BIN; 507 of the 615 such emitters emit by distance, 108 emit nothing, OBS). §8.3 catch-up rounds the quotient before multiplying by `burst_count`. §5.3 last-key-time counts were measured with a 0.0001 tolerance; now exact (28,578 / 3,873 / 161 + 177 non-constant). New invariant PS9; new rule §10.7. |
| 2026-10-07 | §1 (P19, P23, P24, P30, P36), §5.2, §7.1.3, §8.3, §8.7, §8.8, new §8.8.1, §10.8, §11 (PS9), §13 (Q13), §14 | Re-review answered (status stays Review). **Blocking:** §8.8 said gravity and wind are computed at system start and kept for the system's life. That is not true for every system. Each emitter stores both vectors, computed at start. The weather code's rain precipitation update recomputes them for every emitter of the UNIQUEDB 0x83 system, and the snow update does the same for the 0xFE system when the snow value exceeds 1.001, once per run of each update. No other caller exists. Particle updates read the stored vectors, so live particles follow a recomputation (BIN, the start and recompute routines and both callers re-read). New §8.8.1 lists the systems and conditions. The weather code creates all ten UNIQUEDB systems at initialisation (§7.1.3, P30). P24 is widened. Q13 now asks how often the precipitation updates run, what the rain and snow values mean, and where the enabling flags come from. **Non-blocking:** (a) §8.3 now gives the 32-bit wrapping running total and the bound of at most cap passes per update. It also gives the client's behaviour with a negative, too small or not-a-number `emit_interval` (BIN). New Tessera rule §10.8 bounds the work. The `emit_interval` census was re-measured over all 32,789 emitters: 615 are 0, the rest are finite from 0.000263 to 100,000, and none is negative. `max_particles` is at most 100,000 (§5.2, PS9). (b) §8.7: a force-field lifetime below 0.0002, negative values included, never runs out (BIN). A rain fall-direction note for `environment.md` was added to §14. |
| 2026-10-08 | Header, §0, §1 (BIN second pass; P12, P20, P23, P33, P34, P36 widened; new P38–P52), §2.2, §4.1, §4.2, §6.1, new §6.5, §7.1.1, §7.1.2, §8 intro, §8.1, §8.3, §8.4, §8.5 step 9, §8.5.5, §8.6.1, §8.6.2, new §8.6.3, §8.7, §8.8, §8.8.1, §8.11, §8.12 (rewritten), new §8.13–§8.17, §9, §10.4, §10.7, new §10.9–§10.10, §13, §14 | **Status Revised** (Accepted text changed). OPEN questions answered from a second M4 pass and new censuses: Q3 (catch-up `lifespan` at the last-burst clock time; `pivot`/`origin_offset` at the draw clock), Q5 (`uses_video_post` pass, `use_lighting`, `inherit_opacity`, `sorting_priority` order, `shareable` sharing, the distance fade; `force_draw` and `draw_reflection` have no reader), Q6 (`fast_forward` start and restart, `physics_duration`), Q7 (no effect), Q9 (start values, scheduling, anchors), Q10 (whitelist via UNIQUEDB 0x1E2 and the two start-gating options), Q11 (saved loaded-surface slot), Q12 (node DID resolution by DB type), Q14 (the force-field list is never filled; controller-1 index range). Partly answered: Q2 (`flag`, `tail_value`, `transform`), Q4 (`scale_mode` selection), Q8 (billboard construction and modes). Remaining OPEN: Q1, Q2 (bounds, version source), Q4 (source of the two scale vectors), Q8 (local-space frames, light attenuation and ambient, streak in `world_rotation` mode), Q13. **Corrections to Accepted text:** §8.5 step 9 said a new particle is updated to now; the client updates it to the emitter's last particle-update time (BIN). §8.4 said the spawn time is interpolated between the two updates; the client dates successive steps before the previous update (BIN), and the loop also ends when room reaches 0. §8.7 controller 3 with physics flag 0x8: the particle takes the key's counter, which is then re-drawn at random. Rule §10.4 now follows the client instead of starting nothing. |
| 2026-10-09 | Header, §1 (BIN third pass, questions 18–21; new P53), §8.5.2, §8.6.1 | Spec writer (dirty room); status Accepted → Revised, awaiting review. The two implementer OPENs of §8.6.1 (2026-10-07: equal age and a channel without a key; 2026-10-08: the run-time simulation's choices) answered and marked Answered: a key becomes current at a particle update when the age, raised to 0, is at least its `time`; no key is current at creation; a key applies its controller, then its channels, then its physics flags; controller point indices are drawn later; no motion without a controller key; a channel without a key has no defined value in the client (Tessera Decision: scale (1, 1), opaque white, mass 1). §8.5.2: the exploding direction uses the shape point after the attachment and before `origin` (the simulation's "before the attachment" differs). No other fact changed; section numbers kept. |
| 2026-10-09 | Header, §1 (P36), §8.5.2, §8.6.1, new §10.11, §13 (Q15) | Spec writer (dirty room); status stays Revised, awaiting re-review. Review of the morning revision answered. **Blocking:** (1) the §8.6.1 Decision for a channel without a current key (scale (1, 1), opaque white, mass 1; no motion without a controller key, which is the client's behaviour) is now rule §10.11; §10.1–§10.10 keep their numbers; P36 names it; §8.6.1 points to §10.11. (2) The first sentence of §8.6.1 said keys are visited "as the age passes their `time`"; it now states step 2's rule (age raised to 0, an equal age counts), and the table heading reads "when the key becomes current". **Non-blocking:** (a) §8.5.2 line wrapped; (b) whether the key walk and the physics-flag copy depend on emitter-level conditions computed at creation is recorded as OPEN Q15 (not examined; predates the revision). No fact about the original changed. |
| 2026-10-10 | Header, §1 (BIN fourth pass, questions 22–23; legend; P36 widened; new P54–P56), §8.6.1 (0x100 row, step 4, new "base fix" table, new "Emitter-level conditions", two callouts answered), §8.7 (controller 3 and 1 rows, rotation growth), new §10.12, §10.11 wrapped, §13 (Q15 closed) | Spec writer (dirty room); status Accepted → Revised, awaiting review. **Implementer OPEN of 2026-10-09 answered** (controller 3 and the base fix): the base fix of a leaving controller-3 key fills an empty point-index slot from that key's counter, in order and without a random draw, and advances the counter; a filled slot is used as it stands; no points leave the base. **Correction to Accepted text:** §8.6.1 step 4 said the index is drawn at the first update under the new controller; the client keeps **one** slot per particle, shared by controllers 1 and 3, filled once and never emptied, so later controller-1 and controller-3 keys start from it (§8.7 rows rewritten to match). **New facts:** the base-fix motion per leaving controller (§8.6.1 table); the rotation growth's starting value rises by the new key's `time` × `rotation_velocity` at every base fix (§8.7; 350 emitters can show it); controller 3 does not reduce the slot modulo the point count (new rule §10.12). **Q15 closed:** the key walk exists exactly for emitters with keyframes; the physics-flag copy only when some key has `controller` 1–4, without visible effect otherwise (P55). Census P56 (OBS, all emitters): the base fix with an empty slot is reachable in 1 emitter. Implementers re-check `crates/particles` (the base fix with an empty slot, the shared slot, rotation at a base fix). |
| 2026-10-10 | Header, §1 (BIN fifth pass, questions 24–25; P54 widened; new P57–P58), §8.6.1 (step 4, controller-1 row of the base-fix table, new paragraph after it, two callouts answered), §8.7 (controller-1 row, rotation growth), §11 (PS10, new PS11) | Spec writer (dirty room); status Accepted → Revised, awaiting review. **Implementer OPEN of 2026-10-10 answered** (base fix of a leaving controller 1): (1) the span is a clock-time difference, the new key's clock time (spawn time + its `time`) minus the emitter's previous particle-update time, not clamped (a budget at or below 0.0002 moves nothing), and the walk starts at the particle's current position: the simulation's reading is confirmed; (2) the base fix fills an empty point-index slot as a controller-1 update does (index 0, or a random index with physics flag 0x8; no key counter), places the particle on that point, walks, and takes the result as position and new base: the simulation's "leave base and slot unchanged" differs; (3) the client keeps and raises only the angle that §8.12 item 3 draws (P58), so raising both is not visible. The implementer OPEN of 2026-10-09 is marked answered by the controller-3 row of the base-fix table. **Correction:** §8.7 said a controller-1 particle starts walking on the update after its placement; it walks its budget on the same update (P57). Review notes of 2026-10-10: (a) PS10's first key now has bits 0x100 | 0x200; (b) and (d) answered by the controller-1 row and step 4; (c) answered by the rotation-growth paragraph; (e) PS10 rewrapped. |
| 2026-10-10 | Header, §13 Q8 (note), §14 | Spec writer (dirty room, P2), pointers only, status Accepted → Revised, awaiting review: Q8 (b) points to the Tessera Decision of `particle-runtime.md` rule 13; §14 points to the beam-driven systems of `particle-runtime.md` §11.1. No fact or rule of this spec changed. |
| 2026-10-23 | Header, §7.1.3, §8.8, §8.8.1, §10 rule 2 (note), §13 Q13 | Spec writer (dirty room); pointers only, status Accepted → Revised, awaiting independent review. The wind source of §8.8, the placement and spawns of the UNIQUEDB weather and water systems (§7.1.3, §8.8.1) and part of Q13 now point to `particle-runtime.md` §11.2 (its BIN pass (n)); §10 rule 2 re-examined for the question whether fork effects are drawn: unchanged (not drawn until Q1). No fact of this spec changed. |
| 2026-10-23 | Header, §1 (P59), §8.8.1, §13 Q13 | Spec writer (dirty room), package spec-world-r5; status Accepted → Revised, awaiting independent review. §8.8.1: the "at least one drop" condition stated as the count ⌊3,000 × clamp(value, 0, 1) × *q*⌋ with *q* from `AtmosphericsDetail` (from `particle-runtime.md` BIN-O, RT58); the world-conform implementer OPEN on the gate filed and answered; pointer to `particle-runtime.md` §11.2.6. Implemented behaviour changes: precipitation updates with a zero count are skipped. |
