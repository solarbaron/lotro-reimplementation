<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Script tables (0x07) and scriptlets (0x0C), part 1: framing, animation nodes, clip selection

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review by spec-review:spec-world-r5:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer, package spec-world-r5: §5.2, paragraph "Times" pointing to `appearance.md` §5.7 step 4 and two world-conform implementer OPENs filed and answered (MeshFX times; unattached anchors read once, with a Tessera rule for screen-figure props); new OBS7 and T50; no earlier rule changed; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-r3:2 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, third revision of the day by the spec writer: the two non-blocking notes of the last review on §5.2 settled: the MeshFX nodes that set both anchor fields counted (new OBS6, row T49), the key-6 frame question made an OPEN callout with a Tessera rule; awaiting independent review). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-world-render:1 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, spec writer: §5.2 MeshFX anchor rule and §6.10 step 1 start-up scope from BIN9, rows T47–T48; awaiting independent review). Before: **Accepted** (2026-10-23, independent review 2026-10-23 by spec-review:spec-gameplay:2 (independent session), a session that wrote no part of the 2026-10-23 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23: §6.3, §7.8 point 1 and the §8.3 table brought in line with `appearance.md` §7.4.1 rule 4, row T46; awaiting independent review). Before: Accepted (2026-10-14, narrow review of the header-only revision passed). Before: Revised (2026-10-14, header-only revision: the **Last updated** field refreshed after the non-blocking note of the 2026-10-11 review; no fact, rule or section changed). Before: Accepted (2026-10-11; independent review of the P2 (BIN8) revision, commit 7b14f27, passed; see the review record). Before: Revised (2026-10-10, P2 spec writer: new §9.1 (StoryHook and Execute nodes at run time, for `combat-visuals.md`), §9 hook bullet, §4.3 Execute payload row, new BIN8 and OBS5 rows, T44–T45; awaiting independent review). Before: Accepted (2026-10-08, narrow review of the BIN7 revision passed) |
| **Last updated** | 2026-10-23 (spec-world-r5: §5.2 times pointer and anchor answer, OBS7, T50). Before: 2026-10-23 (§5.2 notes: OBS6, T49, key-6 OPEN). Before: 2026-10-23 (§5.2 MeshFX anchor, §6.10 step 1, BIN9, T47–T48). Before: 2026-10-23 (§6.3, §7.8, §8.3: scriptlet parameters reach node fields only through scriptlet-driven pairs). Before: 2026-10-14 (header only; last content change 2026-10-10, BIN8) |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (dirty room, a different session from the writer) |
| **Review record** | **Review 2026-10-23** of the 2026-10-23 spec-world-r5 revision (uncommitted diff of the eight owned specs against 913e1c9) by spec-review:spec-world-r5:1 (independent session), a session that wrote no part of it: **passed**, Accepted. Checklist: every changed statement is a functional fact or a labelled Tessera rule or reading; no code or pseudo-code, and the ordered steps of `particle-runtime.md` §11.2.6 describe behaviour (their order changes the output) rather than a listing; no copied prose and no identifiers beyond the install's interoperability labels (UNIQUEDB, property-dictionary and `[Render]` option names) and Tessera's own code names in implementer callouts; no game text, keys, hostnames or traffic; every new fact has a provenance row with a mark; unknowns stay OPEN (Q-RT13, Q16, the §10 and mode-10 OPENs of `environment.md`); nothing outside the research scope of decision 0034 §2. Spot checks: the executable's SHA-256 matches the BIN rows; the reviewer's own read-only M4 check (decision 0011, Ghidra 12.1.3 headless on a deleted copy of the shared project, same build) and data-section reads confirm the rain and snow parameter sets of §11.2.6 (every table value), the quality factors 0.3 and 0.6 with the built-in level 1, the count rule and its zero-count gate, the material keys 0x2B and 0x3B, the placement segment (moved by *L* / 2 along *f*, swept along −*f*, start returned without a hit), the 0.1 m reference threshold, the fall decrease, the wrap and the radial draw; the install's UNIQUEDB entries 0x2B and 0x3B (values and labels of OBS6) and the Steam prefix's `UserPreferences.ini` values (OBS-F3) match; RT-I21–RT-I23 and the `appearance.md` §5.7 worked example recompute; relative links and anchors of all eight files resolve. Not re-derived: the OBS14 and OBS7 counts, BIN13, BIN-E4 and BIN-F3 beyond their cross-references. Non-blocking notes for the next revision: (1) `particle-runtime.md` §11.2.6 step 2 ("its stored offsets move by the opposite amount"), the per-bone flag "kept with the animation state" (§11.2.5, rule 17, Q-RT13), the snow drop's "several flakes, each with an offset" and the camera's "first local axis" (step 7) lean towards the client's internal representation; restate them as behaviour (drops keep their world position; the marked bones; the camera's right axis); (2) rule 15's culled-owner case is a Tessera reading and may differ from the client if its "placed in the same frame" excludes culled parents; a later BIN pass could settle it; (3) the provenance wording of `appearance.md` BIN13 ("timed messages ... post") and `far-view.md` BIN-F3 ("the count and name list each enumerated option is registered with", "the routine that builds") is mechanism-level; prefer naming only the behaviour read; (4) `particles.md` P59 "otherwise 1" means *q* = 1 at any other level. Before: **Review 2026-10-23** of the 2026-10-23 spec-world-r3 revision (uncommitted diff against 3f0d57f) by spec-review:spec-world-r3:2 (independent session), a session that wrote no part of it: **passed**, Accepted. Checklist: every changed statement is a functional fact or a labelled Tessera rule; no code or pseudo-code and nothing that follows a decompiler listing; no copied prose and no identifiers beyond the interoperability labels of the install's data (UNIQUEDB, property-dictionary and UI-layout labels) and Tessera's own code names in implementer notes; no game text, keys, hostnames or traffic; new facts have provenance rows with marks; unknowns are OPEN callouts; nothing outside decision 0034 §2. Links and anchors of all eight changed specs resolve. T49 checked for consistency (the six flag classes and the 17 sum to 10,349; the key-6 count equals the attached-neither class); the new key-6 OPEN and its Tessera rule are stated as such. The MeshFX census itself was not re-run. Before: **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-world-render:1 (independent session), a session that wrote no part of it): **passed** CLEAN_ROOM.md §5.3, every item; contamination screen clean. BIN facts were checked for consistency with the cited sections and install artefacts, not re-traced. Spot checks (install, read-only): T47's census reproduced (11 of 28,555 setups hold a list-A element with key 6, list A in 2,737 setups; none is a setup of the 14 playable bodies of `appearance.md` §2.2); the §5.2 table agrees with `particle-runtime.md` §6.1 for unattached nodes. Non-blocking: the §5.2 notes "how many nodes set both was not counted" and "which frame the attachment then uses was not traced" could become an OPEN callout. Before: **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-gameplay:2 (independent session)): **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; T46 consistent with `skeleton-anim.md` K31 and `appearance.md` §15; the wider run is marked Reported. Earlier: Re-review 2026-10-07 of the revision after review: **passed**, Accepted. Review 2026-10-08 of the BIN5 (2026-10-07) and BIN6 (2026-10-08) revisions: **passed**, Accepted (two non-blocking notes). Full record in the review log (decision 0018). Writer's revision 2026-10-08 (non-blocking notes, BIN7). Narrow review 2026-10-08 of BIN7, T41, T42, §7.5 step 2, §8.2 step 3: **passed**, Accepted. Independent review 2026-10-11 of the P2 revision (BIN8; commit 7b14f27 on branch p2: header, §1 BIN8, OBS5, T44, T45, §4.3 Execute payload row, §9 hook bullet, new §9.1, change log), by a separate dirty-room session (AI-assisted; neither its author nor an implementer), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Functional facts and pointers only; callback labels are DID-map labels needed for interoperability; BIN8 names build, question and why M1–M3 were not enough. Spot-check, own read-only probe 2026-10-11: T45 reproduced exactly (Execute callbacks 625, 125, 88 none, 64, 10, 4, 2, 2, 1, 1, 1; result ports (`SCP_GetUpcomingTargets_Result`, 1) 623, (…, 0) 4, (`ExecuteScript_ArrayContains_Param_Result`, 0) 2), StoryHook types 4,130 / 2,480. T44 (BIN only) not re-derived. Non-blocking: the **Last updated** field still reads 2026-10-08. Narrow review 2026-10-14 of the header-only revision (commit c3e20b3 on branch specfix: Status, **Last updated**, one change-log row), by a session that wrote no part of this spec: **passed**, Accepted. The diff touches only the header and the change log; no fact, rule or section changed; the 2026-10-11 non-blocking note is resolved. |
| **Implements into** | `crates/formats` (0x07 and 0x0C decoders: framing of every node kind, full animation-node model, skip rules for everything else), `crates/anim` (layered clip playback driven by the node fields of §6), a small graph evaluator for clip selection (§8, §11; `crates/anim` or a new `script` module of `crates/entity`), `fuzz/` (one target for each of the two record types, §12), `apps/dattool` / `apps/explorer` (table and scriptlet inspection, animation browser) |
| **Companion specs** | [`records.md`](records.md) §1 (VLE, TSize, strings, StringInfo; this spec corrects P10 and the 0x0C row of §2.1, see §14). [`property-system.md`](property-system.md) §3–§5 (value encodings, property sets, entries with and without PID echo) and §8–§9 (enum mappers, DB-object types). [`havok-data.md`](havok-data.md) §2.3 (clip envelope: duration, track count, track weights), §4.4 (annotation tracks). [`skeleton-anim.md`](skeleton-anim.md) §2.2 (race table → avatar entity), §3.2 (positional binding), §7 (root motion, treadmill speeds). [`appearance.md`](appearance.md) §7 (NPC-like figures, idle properties). [`entity-descriptor.md`](entity-descriptor.md) §4.3 (weenie objects). Sound, VFX and camera node semantics are a later part of this spec (M6). |

---

<a id="s0"></a>
## 0. Purpose and terminology

This is part 1 of the script-table spec. It gives an implementer:

- the complete **framing** of 0x07 script-table records and 0x0C scriptlet records, so that
  every record can be walked to its last byte (§2–§4);
- the data layout of **every node kind**, so that kinds this part does not interpret (VFX,
  sound, camera, physics, game callbacks) can be skipped exactly (§5);
- the **animation node** in full (§6) and the control-flow nodes that choose between
  animations (§7);
- how the original reaches the idle, walk and run clips of an avatar or creature: from the
  entity's properties to its tables, through a shared locomotion template to the clip,
  including speed scaling, random variants and per-species overrides (§8);
- how animation events work (§9), a census over all records (§10), the algorithms in words
  (§11), invariants (§12), a synthetic example for tests (§13), corrections (§14) and open
  questions (§15).

Terms used throughout:

| Term | Meaning |
|---|---|
| **table** | A 0x07 record: a map from **script key** (a `u32` from enum mapper 0x23000097) to a list of root nodes, plus include links to other tables. |
| **scriptlet** | A 0x0C record: a list of root nodes plus a dependency list. A reusable sub-graph that tables and other scriptlets invoke by DID. |
| **node** | One step of a script graph. Each node has a **kind** (a `u8` code), kind-specific **data**, a common **base block**, a list of output **ports** and up to two **targets**. |
| **port** | An output of a node: a **port type** (`u8`), an optional payload (a time, an index, a weight, a parameter name, a switch case …) and child nodes that run when the port fires. |
| **PV** | A property value written as an *entry without echo* (`property-system.md` §5.2): `u32` PID, then the value encoded by that PID's type. A PID of 0 is a null entry with no value. |
| **property set** | An embedded property set (`property-system.md` §5.1 without the leading DID): a TSize count, then entries *with* PID echo. |
| **template** | A scriptlet that contains ScriptletParameter nodes (§7.8). The locomotion templates are the heart of clip selection (§8). |

Script-key and enum labels (`Startup`, `Walking`, `Earth_ControlledSurface` …) and property
names (`Slider_Propel` …) appear because they are identifiers stored in the data; an
implementation reads them at run time from the enum mappers and the property dictionary.
No game text appears. String-valued identifiers inside nodes (scriptlet-parameter names,
label names, file names) are **not** quoted; they are referred to by structure or by the DID
of the record that holds them.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install (Steam, client data current on
  2026-10-05) by throwaway probes in the dirty-room scratchpad. A Confirmed fact states
  what was decoded and how much of it.
- **Reported**: comes only from third-party material or from the client's own metadata and
  was not, or could not be, checked against data (usually because the case never occurs).
- **Decision**: a Tessera choice where the data is silent; always paired with an OPEN.

| Tag | Source |
|---|---|
| OBS | Observed 2026-10-05: **all 7,526** 0x07 records and **all 28,992** 0x0C records of `client_gamelogic.dat` walked node by node (632,847 nodes); all 40,643 0x47 entity property sets and all 383,332 0x79 property sets searched for script-table properties; the race table and its 14 player-avatar objects; the 775 landblock weenie objects and the 23 distant-battle unit objects; the envelopes (duration, track count, weight count) of all 25,395 0x05 clips; foot-contact speed estimates for the player skeletons taken from the `skeleton-anim.md` probes; the enum mappers named in this spec. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.4 (jar inside the `lotro-tools` clone; Apache-2.0 per its distribution), studied as bytecode for facts only: record outline, node-kind and port-type numbering, the layouts of 24 node kinds, port payloads of 20 (kind, port) pairs, target layout. It has no layout for the animation node and fails on about a third of the records of this install. |
| BIN | The client executable of the user's install (`x64/lotroclient64.exe`), disassembled in the dirty room for facts only (method M4, authorised retroactively for this question only by [decision 0009](../decisions/0009-script-table-disassembly.md)): the node-kind registration (code → class) and each node class's reflected field list (field order, value kind, bound PID, parent class). |
| BIN2 | The same executable, file version 4906.0071.1794.4035 (compiled 2026-09-27), disassembled again on 2026-10-05 under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md). Questions: what a held (`ManualUpdate`) animation node does with `ManualTime` (§6.7); how the layers of §6.2 combine (answer in `skeleton-anim.md` §6.6); what the deformer nodes 18 and 81 do (§6.9, `appearance.md` §6.4). Why M1–M3 were not enough: the data show that River-hobbit `scale_blend` clips are held at a time driven by a property, but not the unit of that time, the wrap rule or the blend; no published source covers these nodes; observing the client (M3) cannot give the rules. Only the offline animation code was read: the animation node's start and update, the playback component, the deformer nodes and the pose and skinning code. |
| RH | Dirty-room probe of 2026-10-05 that applies the rules of §6.7–§6.9, `skeleton-anim.md` §5.4 and §6.6 and `appearance.md` §6.4 to the real River-hobbit, DwarfStoutAxe, Dwarf and Human bodies (setups, clips, all vertices of the default parts). |
| BIN3 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-07 under [decision 0011](../decisions/0011-standing-m4-authorisation.md): the script runtime (switch, random, timer, scriptlet-parameter and script-resolution code), the animation node's start and refresh, the animation component's layers and clip records, and the movement code that sets the movement sliders. Questions (owner observations O14 and O16, and the OPENs of §15–§16): (1) the meaning of the two floats of a switch case and of the transition-port fields, and how a Switch changes branch; (2) how a script key is looked up across included tables and whether every closure table's `Startup` runs; (3) how scriptlet-parameter names are matched; (4) how a Random node picks; (5) what an absent property reads as; (6) what `StartTime`, `EndTime`, `RandomizeStartTime`, `SynchronizeTiming`, `ManualUpdate` and `DetachOnRampOut` do and when events fire; (7) how the movement sliders and ground speed are set. Why M1–M3 were not enough: the data give the fields and graphs but not their run-time semantics; no published source describes them; observing the client (M3) shows effects (a fidget after some minutes, a 0.2 s blend) but not the rules behind them. Only offline script, animation and movement-input code was read; nothing of the network, login, launcher or anti-tamper code. |
| OBS2 | Observed 2026-10-07 in the user install with the repository's decoders used as a tool: the driven pairs of all SetProperty (384 with driven pairs), MeshDeformer (49) and SliderMeshDeformer (8) nodes of all records; the element form of `Entity_AdditionalScriptTables` in all 0x47 and 0x79 sets; the property dictionary entries named in this spec; the kinds of the avatar tables named in §8; the fidget scripts of §8.6 in the Hobbit M closure (0x07001CAA, 0x07000433, 0x0700077F). |
| BIN4 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-07 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), in a fresh copy of the analysis project, to answer the review of 2026-10-07: the switch node's change, delay, fade and child-finished handlers and the reflected field list of the transition port; the animation component's play request (refusals); the movement controllers' slider steps and the player physics-control setup. Questions: (8) what a matching Switch transition port does with its children, `Wait`, `RampIn`, `RampOut` and its two `Delay` fields (§4.3, §7.1); (9) which refusals of §6.10 are reported as errors; (10) the slider time *T* of the player's own input path and the other slider resets (§8.4). Why M1–M3 were not enough: the data give the fields but not their run-time meaning (1,473 of the 1,480 transition ports have `Wait` = 1, but nothing in the data says what waiting means); no published source covers them; observing the client (M3) shows a blend or a pause, not the rule. Only offline script, animation and movement-controller code was read; nothing of the network, login, launcher or anti-tamper code. |
| OBS3 | Observed 2026-10-07 with the repository's decoders used as a tool: the tails of all 1,480 Switch transition ports (both `Delay` fields equal in 1,480; `Wait` = 1 in 1,473; `RampIn` 0 in 1,470, 0.1 in 6, 0.2 in 4, all non-zero ones with `Wait` = 1; last `Delay` positive in 1; every port has children); the `PlayerPhysicsControl` object of the `WEENIECONTENT` DID map (`skeleton-anim.md` OBS3). |
| BIN5 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-07 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), in a fresh copy of the analysis project, to settle edge cases that a clean-room audit of the River-hobbit height code found unstated or contradicted. Questions: (11) what the held-clip fraction does with values that are not finite, too large, or whose fractional part rounds to 1, and whether `StartTime` and `EndTime` crop a held clip (§6.7); (12) what a MeshDeformer with a missing clip or a time that is not finite does, and how many deformers an entity may hold (§6.9); (13) which string a SliderMeshDeformer reads when no pair drives it, and what two such nodes do (§6.9); (14) whether a layer limits its number of clips (§6.2). Why M1–M3 were not enough: none of these cases occurs in the data (every held node has `StartTime` 0, `EndTime` 1 and a finite `ManualTime` in range; every body-shape deformer names a clip; each avatar closure has one slider node), so the data cannot show the rule; no published source covers them; observing the running client (M3) cannot produce the inputs. Code read: the animation node's held-fraction routine, the animation component's held-time setter, its deformer and slider-set registration and removal, the deformation-pose builder, the layer's play routine and per-bone weight pass, and the animation middleware's control creation and time step. Nothing of the network, login, launcher or anti-tamper code. |
| BIN6 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-08 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), in a fresh read-only copy of the analysis project (deleted afterwards), to answer two clean-room implementation OPENs. Questions: (15) in which order the client starts the nodes of one script, so that "the most recently started" slider node of `appearance.md` §6.4 can be found (§7.5, §8.2, §11.3); (16) what a held one-shot clip does when its clip is not longer than the 1/30 s end crop (§6.7). Why M1–M3 were not enough: no avatar closure has two slider nodes and no held one-shot node names so short a clip, so the data cannot show either result; the start order is code, not data; no published source covers them; observing the running client (M3) cannot separate start orders that give the same picture. Code read: the script run routine and its three start passes, the per-table gathering of definitions and the table record's kind field, the routine that starts the children of one port, the Collection and Scriptlet node start routines, the port class of Collection ports and its ordering method, the held-time setter, the animation control's creation and the middleware's time step. Nothing of the network, login, launcher or anti-tamper code. |
| OBS4 | Observed 2026-10-08 with the repository's decoders used as a tool: every port of every Collection node in all 7,524 decodable tables and all 28,992 scriptlets (46,656 Collection nodes, 72,798 ports; ordinal order, repeated ordinals, port and child counts, Appearance nodes below the ports). |
| BIN7 | The same executable (file version 4906.0071.1794.4035), read again on 2026-10-08 under [decision 0011](../decisions/0011-standing-m4-authorisation.md) with Ghidra (headless, read-only fresh copy of the shared project, deleted afterwards), to answer two non-blocking items of the 2026-10-08 review. Questions: how a table's kind is compared with a list's kind while gathering definitions (§8.2 step 3); whether the Collection port sort keeps the container order of equal ordinals (§7.5 step 2). Why M1–M3 were not enough: every table kind of the install is a single bit and no Collection exceeds 26 ports, so the data cannot separate the alternatives; the sort is code, not data. Code read: the per-table gathering routine and the general sort routine used for Collection ports. Nothing of the network, login, launcher or anti-tamper code. |
| BIN8 | The same executable (file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-10 under [decision 0011](../decisions/0011-standing-m4-authorisation.md) in the dirty room (Ghidra 12.1.3 headless on a temporary project copy, deleted afterwards), for `combat-visuals.md`: the combat-presentation component that receives StoryHook events, counts them per story and hook type and releases strikes. **Question:** what a StoryHook node does at run time. **Why M1–M3 were not enough:** code only; the data stores the hook type. The client's network decoding was not read (decision 0004). |
| BIN9 | The same executable (file version 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee), read on 2026-10-23 under [decision 0011](../decisions/0011-standing-m4-authorisation.md) in the dirty room (Ghidra 12.1.3 headless on an own read-only copy of the shared analysis project, deleted afterwards). **Questions:** (1) where a MeshFX node attaches the entity it spawns (which of `HoldingLocation` and `BoneType` wins, what both 0 mean, what happens when the holder lacks the anchor), for `character-screens.md` §2.1.4; (2) during which part of a start-up script the `RampIn` replacement of §6.10 step 1 applies. **Why M1–M3 were not enough:** code only; the data store the fields, not their use. Nothing about network, login, launcher, patcher or anti-tamper code was read. |
| OBS5 | Observed 2026-10-10 with the repository's decoders used as a tool: every Execute node (kind 4) of all tables and scriptlets, its callback (DID map 0x28000007), its call properties and its result ports. |
| OBS6 | Observed 2026-10-23 in the user install (Steam), read in place and read-only, with an own throwaway decoder in the dirty-room scratch area: all 7,526 0x07 records (7,524 decode, as in §10.3) and all 28,992 0x0C records of `client_gamelogic.dat`, every MeshFX node (10,349), its `AttachToParent`, `HoldingLocation` and `BoneType` fields. |
| OBS7 | Observed 2026-10-23 in the same install, read in place and read-only, with a throwaway decoder in the dirty-room scratch area: the 0x07 tables (of the 7,524 that decode) whose nodes name `CharGen_Stage` (PID 0x10000C2F) in a field or driven pair, 32 tables, and the `AttachToParent` and `Persistent` fields of every MeshFX node in them. |
| EM | Enum mappers in the data: node kinds 0x2300019A (base of 0x2300009E), port types 0x230000A1, target types 0x2300019B (base of 0x2300009F), switch-port operator 0x230000A2, when operator 0x230000A3, math operator 0x230000A4, animation layer 0x230000D5, script IDs 0x23000097 (base 0x230000F8), `Movement_MovementStyle` 0x2300017F, `Entity_Element` 0x23000069 (base 0x230001B9), story-hook type 0x230000BB (base 0x2300013D). |
| DS | `research/notes/data-semantics.md` §10 (0x07 outline from LC, node-type names). |
| HD, SA, AP, ED, PS | `havok-data.md`, `skeleton-anim.md`, `appearance.md`, `entity-descriptor.md`, `property-system.md` as companion specs. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| T1 | 0x07 record: `u32` table kind (1, 2 or 4; **not** the DID), includes, scripts, per-script property sets, per-script DID lists, nothing after (§2) | DS, LC (outline); OBS (7,524 of 7,526 to the last byte; the 2 others fail inside one node kind, §10.3) | Confirmed. Corrects DS ("u32 type" read as a DID by `records.md` P10). |
| T2 | 0x0C record: VLE root count, root nodes, `u32` dependency count and DIDs, nothing after; no leading DID (§3) | OBS (28,992 of 28,992 to the last byte) | Confirmed. New; corrects `records.md` §2.1 ("possibly entity/prop data"). |
| T3 | Node frame: kind byte, data, base block, `u32` port count, ports, `u32` target count, targets; CinematicCamera puts the base block before the data (§4.1) | LC; OBS (632,847 nodes) | Confirmed, except the CinematicCamera data (OPEN, §10.3) |
| T4 | Base block = driven-property map, scriptlet-driven map, scriptlet-parameter list, `UseParentIntensity`, `IntensityMultiplier`, `IntensityMultiplierVariance`, `Probability`, node ID (§4.2) | LC (layout); BIN (field names and order); OBS (632,847 of 632,847) | Confirmed |
| T5 | Port frame and the payload of each (kind, port type) pair that carries one, with a `u32` echo of the port type after the payload (§4.3) | LC; OBS (32 (kind, port) pairs, 489,076 ports) | Confirmed for every observed pair |
| T6 | Target frame, value forms and the data of each target type (§4.4) | LC; OBS (14,515 targets incl. sub-targets) | Confirmed for every observed form. Corrects LC on sub-targets (§14). |
| T7 | Node-kind code → class for 66 codes (§5.1) | BIN (registration); EM (labels for all 66) | Confirmed (the two sources agree on every code) |
| T8 | Data layouts of 63 node kinds (§5.1–§5.3) | LC (24 kinds); BIN (field lists); OBS | Confirmed for the 56 kinds that occur (CinematicCamera excluded); Reported for 7 kinds with no instance; 3 kinds OPEN (7, 64, 78) |
| T9 | Animation node: 18 fields, 59 bytes, in the order of §6.1 | BIN (field list); OBS (27,217 of 27,217) | Confirmed. New (LC has no layout). |
| T10 | Animation-node ports: DEFAULT ports carry an `f32` event time in clip seconds; their children fire at that time (§6.4, §9) | OBS (24,507 ports; 24,178 times within the clip) | Confirmed (layout and time range); firing rule Reported by structure |
| T11 | `LocomotionDisplacement` is the authored ground distance per clip cycle; divided by the clip duration it approximates the foot-contact speed (§6.3) | OBS (158 clips: median ratio 1.07, middle half 0.98–1.34; walk 1.8–2.1 m/s, run 6.5–8.9 m/s) | Confirmed as a correlation, not an identity |
| T12 | Entities name their tables through `Entity_ClassScriptTable` and `Entity_AdditionalScriptTables`; include links form an acyclic graph (§8.1, §2.2) | OBS (11,170 entities; 6,465 links, longest chain 7) | Confirmed |
| T13 | Idle, walk, run, back, strafe and turn clips of all 14 avatar entities are bound by name to one shared locomotion template scriptlet, 0x0C000FFE, from the `Startup` script of a per-species table (§8.3) | OBS (14 of 14; all 435–478 clips reachable from each avatar's 291–293 bound parameters have the avatar's bone count as track count) | Confirmed |
| T14 | The template chooses by entity properties: `Entity_Element`, `Movement_MovementStyle`, `Slider_Propel`, `Slider_Strafe`, `Slider_Turn`, `Entity_Mode`, `Movement_SpeedMultiplier`, stealth, mount and cosmetic properties (§8.4) | OBS; BIN3 (T30) | Confirmed (structure); the slider update rule is BIN (T30), the rest of the movement code that sets them is OPEN |
| T15 | Walking and running are separate branches, not a speed blend; playback rate follows `Movement_SpeedMultiplier` through a driven property (§8.5) | OBS (14,278 animation nodes have `AnimationScript_Speed` driven); BIN3 | Confirmed (data); since 2026-10-07: the client never matches the rate to ground speed (BIN3, `skeleton-anim.md` §7.4) |
| T16 | Random choice uses `u32` weights on the ports of a Random node (§7.4) | OBS (137,690 ports; 96 % weight 1); BIN3 (T27) | Confirmed (layout and, since 2026-10-07, selection rule) |
| T17 | No clip carries a timed annotation; all animation events come from script ports (§9) | HD §4.4; OBS | Confirmed |
| T18 | Script lookup precedence across includes | BIN3 (T25) | Resolved 2026-10-07: there is no single winner; all definitions run (§8.2) |
| T19 | A node with `ManualUpdate` = 1 never advances its clip; its time is `ManualTime` read as a fraction of the played range, wrapped (looping) or limited by `LoopCount` (§6.7) | BIN2; OBS (all 30 such nodes; 7 have `ManualTime` driven by a property) | Confirmed 2026-10-05 (rule BIN only); edge cases and played length T36, T37 |
| T20 | Layers combine by a remaining-weight budget from the highest layer down (§6.2; rule in `skeleton-anim.md` §6.6) | BIN2 | Confirmed 2026-10-05 (BIN only). Replaces the §6.2 Decision. |
| T21 | River-hobbit and DwarfStoutAxe tables start, from `Startup`, a script that holds a layer-110 `scale_blend` clip at a time driven by `AC_Avatar_Height` (River-hobbit) or `AC_BoneScale_Test` (DwarfStoutAxe) (§6.8) | OBS (8 layer-110 nodes, their tables and scriptlets); BIN2 (§6.7 rule); RH (feet stay on the ground) | Confirmed 2026-10-05 |
| T22 | Species tables' `Startup` scripts hold a MeshDeformer node (18) driven by `AC_BoneScale_Test` (15 tables) and, for the Human, Elf, HighElf and River-hobbit bodies, a SliderMeshDeformer node (81) driven by `AC_BoneSlider_Value_Array` (8 tables); both deform the body in bind space (§6.9) | OBS (all 74 + 8 nodes); BIN2 | Confirmed 2026-10-05 |
| T23 | A switch case's two floats are `RampOut` (`SwitchScriptPort_RampOut`) then `Delay` (`SwitchScriptPort_Delay`); a transition port's tail is `FromIsDefault`, `ToIsDefault`, `Delay`, `Wait`, `RampIn`, `RampOut`, `Delay`, and its two `Delay` fields are one value in the client (the later one read wins) (§4.3) | BIN3 (field registration, stream order: a port's own fields, then the fields every switch port shares); BIN4 (both `Delay` fields bind the same storage); OBS (commonest transition tail has `RampOut` −1); OBS3 (the two `Delay` fields are equal in 1,480 of 1,480 ports) | Confirmed 2026-10-07 |
| T24 | A Switch changes branch after the new case's `Delay`; the outgoing branch fades out linearly over the left case's `RampOut` while the incoming one fades in as its complement (§7.1) | BIN3 | Confirmed 2026-10-07 (BIN only) |
| T25 | Looking up a script key gathers **every** definition in the include closure (pre-order, depth first, from each of the entity's tables in order); a definition whose `Script_CombinationType` is `Overriding` (1) stops the search below its own table; the default is `Additive` (2) (§8.2) | BIN3; EM (mapper of `Script_CombinationType`) | Confirmed 2026-10-07. Resolves T18. |
| T26 | Scriptlet-parameter names match **ignoring ASCII case**, against the innermost enclosing invocation of the scriptlet that holds the parameter node; an unbound name runs nothing (§7.8) | BIN3 | Confirmed 2026-10-07 |
| T27 | A Random node picks a port with probability weight / total, excluding the previous pick when `NoRepeats` = 1; a single port is always picked; a zero total picks nothing (§7.4) | BIN3 | Confirmed 2026-10-07 (was Reported) |
| T28 | An absent property reads as its dictionary default, or as the zero value of its type when the dictionary has none (§7.1, §11.3) | BIN3 | Confirmed 2026-10-07 (read rule BIN; its use by the Switch's case match Reported) |
| T29 | Clip lifecycle: start offset from `StartTime` (or a random fraction in [`StartTime`, `EndTime`]), end of a one-shot clip at `EndTime` of its last cycle, ramp-out scheduled to end there, `SynchronizeTiming` start origin shared in a layer, start-up clips without ramp-in, events beyond the clip duration never fire; of the refusals, a track-count mismatch and a one-shot clip on a shared object are reported as errors, a layer above 255 silently (§6.10, §6.4) | BIN3; BIN4 (refusals) | Confirmed 2026-10-07 (BIN only) |
| T30 | The movement code moves the sliders (`Slider_Propel`, `Slider_Strafe`, `Slider_Turn`, `Slider_Fly`) towards their targets at a constant rate of 1/*T* per second; for the player's own input *T* is the `GenericPhysicsController_RampTime` of the player's controller configuration (`skeleton-anim.md` §7.4, K28); a movement freeze resets the sliders over 0.1 s (§8.4) | BIN3; BIN4; OBS3 (values) | Confirmed 2026-10-07 (rule BIN; player *T* BIN4 with values from the data, revised 2026-10-07) |
| T31 | `LocomotionDisplacement` is multiplied by the mean of the entity's three scale components when a clip starts (§6.3) | BIN3 | Confirmed 2026-10-07 (BIN only) |
| T32 | In all 384 SetProperty nodes with driven pairs, the pair's field PID is the property being set (§7.7) | OBS2 | Confirmed 2026-10-07 |
| T33 | Every element of `Entity_AdditionalScriptTables` is a `ScriptTable_DBScriptTable` DID entry; no struct element exists (§8.1) | OBS2 | Confirmed 2026-10-07 |
| T34 | Avatars do play random idle fidgets: the shared `Startup` of 0x07001CAA reaches, after 20 s, a script that waits 180 s plus 15 s and then repeatedly picks a per-species fidget or a pause, at least 9 s apart, while the avatar stands idle (§8.6) | OBS2 (data path); node semantics Reported (Timer, Sequence) and BIN3 (Random) | Confirmed 2026-10-07 (path); timing Reported. Corrects §8.6 of 2026-10-05. |
| T35 | A matching transition port runs its children at once (after its `Delay`, if positive); with `Wait` = 1 and a child still running, its branch fades in over the port's `RampIn` against the old case's branch and the change to the new case waits until that branch has finished; the port's `RampOut` has no effect in this build (§4.3, §7.1) | BIN4 | Confirmed 2026-10-07 (BIN only) |
| T36 | Held fraction edge cases: the integer part *n* comes from a truncating conversion to a signed 32-bit integer. With `LoopForever` 0 a value that is not finite or whose magnitude is 2³¹ or more gives *f* = 0. With `LoopForever` 1 an infinite value gives 0, a negative fractional part whose sum with 1 rounds to 1 gives *f* = 1 (not wrapped again), and NaN reaches the player unchanged (§6.7) | BIN5 | Confirmed 2026-10-07 (BIN only); NaN on a looping node: Tessera Decision |
| T37 | A held clip's time is *f* × its played length, limited to [0, played length]; the played length is the tagfile duration minus the end crop of `havok-data.md` §2.3 (1/30 s when the node has `LoopForever` 0 and `LoopCount` 1, else 0); `StartTime` and `EndTime` do not enter. A time equal to the played length stays there (the middleware's time step wraps only a time beyond it) (§6.7) | BIN5; HD §2.3 | Confirmed 2026-10-07 (BIN only). Resolves the §6.7 step 2 OPEN; corrects the `LoopCount` 1 column of the §6.7 example |
| T38 | A MeshDeformer registers its clip, fraction and `UseBoneWeights` whatever they are; when the deformation pose is built, a clip that cannot be loaded (the null DID included) contributes nothing, and a clip whose track count differs from the bone count is reported as an error and contributes nothing; an entity holds any number of deformers; the time is duration × fraction limited to [0, duration] (§6.9) | BIN5 | Confirmed 2026-10-07 (BIN only); NaN fraction: Tessera Decision |
| T39 | A SliderMeshDeformer parses whatever string its field holds when the field is set (the stored string when no pair drives it); an entity holds one slider set, which the most recently started node replaces and the stopping of any slider node removes (§6.9) | BIN5 | Confirmed 2026-10-07 (BIN only) |
| T40 | A layer has no limit on its number of clips: when it holds 1,000 the client logs a warning and still plays the new clip (§6.2) | BIN5 | Confirmed 2026-10-07 (BIN only) |
| T41 | Start order of one script run: the gathered definitions of kind-1 tables first, then those of kind-2 tables, then kind-4 (which hold no scripts); within a kind, definitions in gathering order and each definition's root nodes in stored order; a Scriptlet node starts its record's roots in stored order; a port's children start in stored order; every node completes its own immediate starts before its next sibling starts. Gathering into the three lists: a table's kind is tested against the list's kind as a bit mask (a common set bit), a table that does not match is still descended into for its includes (a null include or one naming the table itself is skipped), and a root node already in a list is not added again (§8.2 step 3, §11.3) | BIN6; BIN7 (bit mask, include skips) | Confirmed 2026-10-08 (BIN only; gathering re-read 2026-10-08, BIN7); resolves the §8.2 step 3 OPEN. Stored order = the order of the loaded lists; the loader itself was not traced |
| T42 | A Collection starts its Default ports in **ascending ordinal** (unsigned 32-bit comparison), whatever their stored order; ports with equal ordinals keep the order of the client's port container, which was not traced (they keep it because the client sorts 32 ports or fewer by a stable insertion sort and no Collection has more than 26 ports, SI17; above 32 ports its sort is not stable); a Collection stopped during its own start starts no further ports (§7.5) | BIN6; BIN7 (sort stability); OBS4 | Confirmed 2026-10-08 (rule BIN only; census OBS4) |
| T43 | A held clip whose played length is 0 or less (a one-shot held node whose clip lasts at most 1/30 s) is held at local time 0: the setter limits the time to [0, played length] with the lower limit applied last, and the middleware's time step raises a negative local time to 0 and advances nothing when the played length is 0 (§6.7) | BIN6 | Confirmed 2026-10-08 (BIN only) |
| T44 | A StoryHook node hands its type and the run's `StorySystem_StoryID` to the performing entity's combat-presentation component and completes at once; the component counts the events per story and type and releases the strikes of that number (§9.1) | BIN8 | Confirmed (BIN) |
| T45 | Execute nodes: callbacks census; the payload of an Execute result port (type 5) is the PID of the callback's result property and the value that selects the port (§4.3, §9.1) | OBS5 | Confirmed (2026-10-10) |
| T46 | Under `appearance.md` §7.4.1 rule 4 (a Scriptlet node's parameters reach an inner node's field only through that node's scriptlet-driven pairs) the walk, run and walk-back clips of the 14 avatars of §8.3 are unchanged and their displacements are unchanged except three: DwarfStoutAxe walk 0x05007A0D (1.667 m over 0.833 s, 2.00 m/s) and run 0x05007A14 (4.2 m over 0.7 s, 6.00 m/s), and RiverHobbit F walk back 0x0500159E (−4.667 m over 1.816 s, −2.57 m/s), which showed no displacement before (§6.3, §8.3) | OBS (the repository's clip resolver, which applies rule 4, over the race table's avatars, 2026-10-23); the wider run over 206,374 starts is the implementer's, reported in `appearance.md` §15 | Confirmed 2026-10-23 (avatars); Reported (wider run) |
| T47 | MeshFX anchor (§5.2): attached nodes take `HoldingLocation` when it is not 0 or when `BoneType` is 0 (then holding-location key 6), else the bone of `BoneType`; unattached nodes take the bone of `BoneType` when it is not 0, else the holding location when not 0, else the holder's model origin; a holder without the anchor gets no entity (the spawned one is removed); key 6 is accepted by every holder that has a setup | BIN9 (1); the label of key 6 and the list-A census from the install (OBS, 2026-10-23: 11 of 28,555 setups hold a list-A element with key 6, none of them a playable body) | BIN; census Confirmed 2026-10-23 |
| T48 | The `RampIn` replacement of §6.10 step 1 holds only while the client is starting a start-up script: the clip requests made from the start of that script up to the moment its start returns take `RampIn` 0, and no later request of the same script does | BIN9 (2) | BIN |
| T49 | Of the 10,349 MeshFX nodes, 17 set both `HoldingLocation` and `BoneType` (all 17 with `AttachToParent` 1); by the two flags: attached with neither field 4,827, attached with a holding location only 4,446, attached with a bone only 738; unattached with neither 228, with a bone only 52, with a holding location only 41 (§5.2) | OBS6 | Confirmed 2026-10-23 |
| T50 | In the 32 script tables that name `CharGen_Stage`, 3 MeshFX nodes are unattached (`AttachToParent` 0), all three in one table and all with `Persistent` 0; the other MeshFX nodes of those tables are attached (§5.2) | OBS7 | Confirmed 2026-10-23; counts are observations |

---

<a id="s2"></a>
## 2. The 0x07 script-table record (Confirmed)

Population: 7,526 records, all in `client_gamelogic.dat`. All values are little-endian.
The record does **not** start with its own DID (like 0x05 and 0x0C; see §14).

<a id="s2-1"></a>
### 2.1 Layout

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `kind` | u32 | Table kind: 1 (1,685 records), 2 (5,483) or 4 (358). No other value occurs. |
| 2 | `include_count` | VLE (`records.md` §1.2) | Number of included tables |
| 3 | `includes` | u32 × `include_count` | DIDs of other 0x07 records (§2.2) |
| 4 | `script_count` | TSize (`records.md` §1.3) | Number of scripts |
| 5 | `scripts` | script × `script_count` | §2.3 |
| 6 | `props_count` | TSize | Number of per-script property sets |
| 7 | `props` | (u32 key, property set) × `props_count` | §2.4 |
| 8 | `dids_count` | TSize | Number of per-script DID lists |
| 9 | `dids` | (u32 key, i32 *n*, u32 × *n*) × `dids_count` | §2.5 |
| 10 | *(end)* | | Zero bytes remain in every decoded record |

A **script** is:

| Field | Type | Meaning |
|---|---|---|
| `key` | u32 | Script key: an enum key of mapper 0x23000097, whose base mapper 0x230000F8 labels the small keys (1 `Jump`, 5 `Startup`, 9, 12, 13, 22, 23 …). All 54,698 keys of this install have a label in one of the two. |
| `root_count` | VLE | Number of root nodes (0 or more) |
| `roots` | node × `root_count` | §4 |

<a id="s2-2"></a>
### 2.2 Kinds and includes

- **Kind 4** tables (358) have no scripts and no property sets; they carry only DID lists
  under keys (script declarations without bodies). Kinds 1 and 2 hold scripts; the data
  does not show what distinguishes them (OPEN, §15).
- 6,465 include links; every one names an existing 0x07 record; no table includes itself;
  the include graph has **no cycle**; the longest include chain is 7 tables; a table has at
  most 54 includes; 5,873 tables have none.
- An avatar class table reaches 148 or 149 tables through its includes (§8.2).

<a id="s2-3"></a>
### 2.3 Script order

Script keys are stored in TSize bucket order (`records.md` §1.3.1): key modulo
*B*[hint] is non-decreasing in 7,524 of 7,524 records. Observed hints: 0 (358 records, all
of kind 4 with no scripts), 1 (6,987), 2 (96), 3 (41), 5 (22) and others. A table has at most
828 scripts; 721 have none.

<a id="s2-4"></a>
### 2.4 Per-script property sets

Each entry is a `u32` key followed by an embedded property set (`property-system.md` §5).
The keys are always a subset of the script keys (7,524 of 7,524; equal in 7,451). 10,239
sets are non-empty; their properties are script-level settings: `Script_ReplicateToClient`
(8,069), `Script_ExposeAsScriptLinkEnd` (1,203), `Script_MaxNumInstances` (1,156),
`Script_ClientAuthorative` (491), `Script_Comment` (240), `Script_ScriptLinkEnd_Persistent`
(120), `Script_CombinationType` (68), a few others. None of them selects animation.

<a id="s2-5"></a>
### 2.5 Per-script DID lists

Each entry is a `u32` key, an `i32` count *n* (never negative) and *n* DIDs. The keys equal
the script keys in 7,178 records; in the other 346 (all kind-4 tables) there are lists but
no scripts. The DIDs are dependencies to preload: sound info 0x2A (38,723), particle
descriptors 0x39 (26,336), entities 0x47 (5,748), 0x31 (3,440), appearances 0x20 (3,262),
game objects 0x70 (2,290), clips 0x05 (1,594), 0x44 (699), 0x40, 0x33, 0x78, 0x0C. A
decoder exposes them; clip selection does not need them.

---

<a id="s3"></a>
## 3. The 0x0C scriptlet record (Confirmed)

Population: 28,992 records in `client_gamelogic.dat`; the DB-object type of
`ScriptletScript_File` is 0x15 `SCRIPTLET` (`property-system.md` §9). No leading DID.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `root_count` | VLE | Number of root nodes: 0 (13 records), 1 (21,445), 2 (6,436), 3 (1,025), 4 (9), 5 or more (64) |
| 2 | `roots` | node × `root_count` | §4 |
| 3 | `dep_count` | u32 | Number of dependency DIDs |
| 4 | `deps` | u32 × `dep_count` | Preload list: sound info 0x2A (42,858), clips 0x05 (27,783), 0x39, 0x70, 0x31, 0x20, 0x47 |
| 5 | *(end)* | | Zero bytes remain in 28,992 of 28,992 |

The dependency list is sorted ascending in 25,708 records and free of duplicates in 28,493;
neither is a rule. Every scriptlet DID named by a Scriptlet node (§7.8) exists (161,742
references); 4,041 Scriptlet nodes name DID 0, nearly all as placeholders under
ScriptletParameter nodes (§7.8).

Scriptlets hold 26,835 of the 27,217 animation nodes of the install; almost every clip has
its own small scriptlet with one animation node (§6.6).

---

<a id="s4"></a>
## 4. Node encoding

<a id="s4-1"></a>
### 4.1 Node frame (Confirmed, 632,847 nodes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `kind` | u8 | Node kind (§5.1). A code not in §5.1 makes the record undecodable. |
| 2 | `data` | per kind (§5) | Kind-specific fields |
| 3 | `base` | §4.2 | Common to all kinds |
| 4 | `particle_flag` | bool8 | **Only for kind 59 (Particle)**: one extra byte after the base block (0: 27,130 nodes, 1: 14,224). Meaning OPEN. |
| 5 | `port_count` | u32 | Observed 0–697 |
| 6 | `ports` | port × `port_count` | §4.3 |
| 7 | `target_count` | u32 | Observed 0 (619,936 nodes), 1 (12,645) or 2 (266). LC rejects counts above 2. |
| 8 | `targets` | target × `target_count` | §4.4 |

**Exception:** for kind 7 (CinematicCamera) the base block comes **before** the data
(LC; OBS on the base block). The CinematicCamera data itself is not resolved (§10.3).

<a id="s4-2"></a>
### 4.2 Base block (Confirmed, 632,847 of 632,847)

| Order | Field | Type | Meaning | Observed |
|---|---|---|---|---|
| 1 | `driven` | TSize *n*, then (u32 field PID, u32 source PID) × *n* | **Driven properties**: at run time the node field bound to *field PID* takes the current value of the entity property *source PID* | *n* = 0 in 609,525 nodes, 1 in 15,073, 2 in 7,987, 3+ in 262. Commonest pairs: `AnimationScript_Speed` ← `Movement_SpeedMultiplier` or `Combat_AnimationSpeedMultiplier`; `AnimationScript_UseBoneOpacity` ← `ScriptSystem_CurrentlyMoving`; light fields from same-named properties. |
| 2 | `scriptlet_driven` | TSize *n*, then (u32, u32) × *n* | Fields driven from scriptlet parameters (OPEN) | 5,387 pairs in all |
| 3 | `scriptlet_params` | u8 *n*, then u32 × *n* | Scriptlet parameter IDs (OPEN) | 312 values in all |
| 4 | `UseParentIntensity` | bool8 | | 1 in 632,744 nodes |
| 5 | `IntensityMultiplier` | f32 | Scales the node's effect (volume, opacity …) | 1.0 in 584,197; often 0.6–0.8 on sounds |
| 6 | `IntensityMultiplierVariance` | f32 | Random spread of the multiplier | 0.0 in 624,008 |
| 7 | `Probability` | f32 | Chance that the node runs when triggered | 1.0 in 624,715; 0.0 in 2,255; fractions (0.25, 0.5 …) in 5,877 |
| 8 | `node_id` | u32 | Editor node ID | 1 – 65,534; never 0; unique within a scriptlet (28,992 of 28,992) and within a table (7,515 of 7,524) |

The TSize hint bytes of fields 1 and 2 follow `records.md` §1.3; the driven map was written
with hint 1 in every node of the first 2,000 tables (so the empty map is `01 00`). BIN lists one more base field, a comment string, that is **not** serialized.

<a id="s4-3"></a>
### 4.3 Ports (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `port_type` | u8 | Port type (enum 0x230000A1, table below). **0 means "no port"**: nothing follows and the port contributes nothing (never observed; LC handles it). |
| 2 | `payload` | per (node kind, port type), table below | Absent for pairs not listed |
| 3 | `echo` | u32 | **Present only when a payload is present.** Must equal `port_type`. |
| 4 | `child_count` | u8 | Number of child nodes (0–255) |
| 5 | `children` | node × `child_count` | Run when the port fires |

Port types (enum 0x230000A1; the observed ones in bold): 1 Parameter, 2 Response, 3 RunOn,
4 EntityResident, **5 SwitchValue**, **6 Default**, 7 CompareTarget, 8 ImpulseTarget,
**9 FinalResult**, **10 NotRunning**, **11 Running**, 12 PropertySource, 13 OtherFinalResult,
**14 CurrentResult**, 15 ScriptletRuntime, 16 RightHandSide, 17 MoveToState, 18 Success,
19 Impulser, 20 LeftHandSide, 21 Source, **22 ForCurrentValue**, 23 SetPropertyObjects,
**24 WhenTest**, 25 Target, 26 EntityNotResident, 27 OtherCurrentResult,
**28 SwitchTransition**, 29 Failure, **30 AnyOtherValue**.

Payloads (every pair observed in the install; pairs not in the table have no payload and no
echo — confirmed for (4,6), (10,6), (12,22), (14,10), (14,11), (20,6), (22,6), (26,30),
(27,6), (50,6), (54,6), (63,6), (63,22)):

| Node kind | Port type | Payload | Ports | Meaning |
|---|---|---|---|---|
| 2 Collection | 6 | u32 | 72,798 | Ordinal label 1, 2, 3 … (all children start together) |
| 4 Execute | 5 | u32, u8 | 629 | The PID of the callback's result property and the value that selects the port (T45, §9.1) |
| 5 Run | 9, 14 | u8, u32 | 9 | Result filter (OPEN) |
| 6 Sequence | 6 | u32 | 32,149 | Step number; steps run in ascending order (§7.6) |
| 11 WaveformBlend | 6 | u32 | 1 | (OPEN) |
| 12 Switch | 5 | switch case, below | 77,134 | Case that fires when the switch property matches (§7.1) |
| 12 Switch | 6 | f32, f32 | 21,248 | Default case: `RampOut`, `Delay`, as in a switch case. LC reads them as two u32. |
| 12 Switch | 28 | transition, below | 1,480 | Fires when the property changes from one value to another (§7.1) |
| 15 DistanceCheck | 6 | f32, f32 | 86 | Distance band (OPEN which end is which; −1 occurs). LC reads u32, f32; the first value is an f32 bit pattern in all 86. |
| 17 Random | 6 | u32 | 137,690 | Weight (§7.4) |
| 19 Scriptlet | 6 | Pascal string | 60,055 | Parameter name bound to this port's children (§7.8) |
| 25 Label | 6 | u32 | 170 | (OPEN) |
| 26 HeadingCheck | 6 | f32, f32 | 146 | Heading band in degrees (−45/45, 45/135, 135/225 …) |
| 27 GameCallback | 5 | PV | 327 | Callback result value |
| 36 Animation | 6 | f32 | 24,507 | **Event time** in clip seconds (§6.4, §9) |
| 57 ScalarBlend | 6 | f32 | 7,194 | Blend anchor value (§7.3) |
| 63 When | 24 | when test, below | 1,821 | Condition (§7.2) |
| 65 Timer | 6 | f32 | 37,801 | Time in seconds after which the children fire |

Reported only (LC; no instance): none further — LC's pairs are all in the table above.

**Switch case** (payload of (12, 5)):

| Field | Type | Meaning |
|---|---|---|
| value count *v* | u8 | Observed 0–55 (1 in 71,168 of 77,134 cases) |
| values | PV × *v* | Values of the switch property that select this case |
| int count *m* | u8 | 0 (77,065) or 1 (69) |
| ints | i32 × *m* | (OPEN) |
| reserved | i32 | Always 0 (LC rejects other values) |
| operator | i32 | Enum 0x230000A2: 1 AllBitsSet, 2 AnyBitSet, 3 Equal (76,942 cases) |
| `RampOut` (`SwitchScriptPort_RampOut`) | f32 | Seconds over which this case's branch fades out when the switch leaves the case (§7.1). 0.0 in 76,821 cases; 0.2 and 0.3 in the locomotion templates (§8.4). Called `transition_time` before 2026-10-07. |
| `Delay` (`SwitchScriptPort_Delay`) | f32 | Seconds the switch waits, once the property matches this case, before it changes to it (§7.1). 0.0 nearly always; 0.1, 0.5, 0.75, 1.0 occur (for example 0.5 on the sliding case of the locomotion templates). Called `second_time` before 2026-10-07. |

The names are the property names the client binds to these fields (BIN3, T23); they are
needed to apply driven properties and are given for interoperability. Stream order of a
switch case: the case's own fields first (values, the int list `Names`, the reserved value,
the operator), then `RampOut`, then `Delay`. `RampOut` and `Delay` are the two fields that
every Switch port shares (case, Default and transition ports alike), and they always come
last.

**Switch transition** (payload of (12, 28)), BIN3 names: PV `ValueFrom`, PV `ValueTo`, bool8
`FromIsDefault` (PID 0x26A), bool8 `ToIsDefault` (0x87), f32 `Delay` (0x217), bool8 `Wait`
(`SwitchScriptPort_Wait`, 0x1CE), f32 `RampIn` (0x490), f32 `RampOut` (0x491), f32 `Delay`
(0x217; the shared Switch-port field of the same name). Commonest tail: (0, 0, 0.0, 1, 0.0,
−1.0, 0.0) in 1,230 of 1,480, a `RampOut` of −1 meaning "unset". Meaning of the fields
(BIN4, T23, T35; behaviour in §7.1):

- The two `Delay` fields are **one value** in the client: both bind the same storage, so the
  one read last (the final field) is the effective value. In this install they are equal in
  all 1,480 ports (OBS3); a positive value postpones the transition (positive in 1 port).
- `Wait` (1 in 1,473 ports): the change to the new case waits until the port's children have
  finished.
- `RampIn` (0 in 1,470 ports, 0.1 in 6, 0.2 in 4): seconds over which the port's branch fades
  in, and the old case's branch out, while the switch waits.
- `RampOut`: read but without effect in this build (the value is passed on and ignored).
- `ValueFrom`, `ValueTo`, `FromIsDefault`, `ToIsDefault` decide which property changes the
  port matches; the exact matching rule was not traced (OPEN, §15).

**When test** (payload of (63, 24)): PV (a constant of the tested property), i32
*rhs_property* (0 in most tests: compare with the constant; otherwise a PID whose value is
compared), i32 *operator* (enum 0x230000A3: 1 LessThanOrEqual, 2 GreaterThanOrEqual,
3 NotEqual, 4 LessThan, 5 Equal, 6 GreaterThan), i32 0, i32 0.

**Port order is not meaningful.** Payload values are stored mostly in descending order
(event times, steps, weights, anchors) but 3,700 nodes are unordered; consumers sort or
search by payload.

<a id="s4-4"></a>
### 4.4 Targets (Confirmed)

A target tells a node which entity it acts on. Layout:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `port_type` | u8 | Port type (enum 0x230000A1); observed 1, 3, 6, 7, 8, 16, 19, 20, 21, 23, 25 |
| 2 | `value` | see below | Only for the listed forms |
| 3 | `echo` | u32 | Only when `value` is present; equals `port_type` |
| 4 | `marker` | bool8 | Always 1 (LC rejects 0) |
| 5 | `target_type` | u8 | Enum 0x2300019B (table below) |
| 6 | `target_data` | per type | Below |
| 7 | `sub_count` | u32 | Observed 0–8 |
| 8 | `sub_targets` | target × `sub_count` | Same layout, recursively |

Value forms:

| Where | Port type | Value | Observed |
|---|---|---|---|
| top-level target of a Run node (5) | 1 | u32 | 7 |
| top-level target of a Scriptlet node (19) | 25 | Pascal string | 22 |
| top-level target of a GameCallback node (27) | 25 | u32 | 186 |
| top-level target of an Execute node (4) | 25 | u32 | 0 (Reported, LC) |
| **any sub-target** | 6 | u32 (an index) | 16 |

Sub-targets with other port types (16, 20, 25: 1,322 observed) carry no value, whatever the
node kind (correcting LC, §14).

Target data (types from the property dictionary; "u32" for enum and PID values):

| Type | Label | Data |
|---|---|---|
| 1 | Combination | `CombinationTarget_Operator` u32 (enum 0x230000A6) |
| 2 | Parameter | `ScriptletParameterScript_Name` Pascal string |
| 3 | Property | `PropertyTarget_PropertyName` u32 PID |
| 4 | ThePlayer | nothing |
| 5 | Sequence | `SequenceTarget_Cap` i32 |
| 6 | Random | `RandomTarget_Cap` i32 |
| 7 | Link | `LinkTarget_Name` Pascal string |
| 8 | Fit | `FitTarget_Cap` i32, `FitTarget_PropertyName` u32 PID, PV, `FitTarget_Operator` u32 (enum 0x230000A5) |
| 9 | Initiator | nothing |
| 10 | Filter | `FilterTarget_PropertyName` u32 PID, PV, operator u32 |
| 11 | Monitor | `MonitorTarget_MonitorName` u32 |
| 12 | DistanceFit | `DistanceFitTarget_Cap` i32, `DistanceFitTarget_PreferCloser` bool8, `DistanceFitTarget_MinimumRange` f32, `DistanceFitTarget_MaximumRange` f32 |
| 13 | This | nothing |
| 14 | Child | `ChildTarget_HoldingLocation` u32 (enum 0x23000068) |

No animation node has a target (27,217 of 27,217): animations always play on the entity
that runs the script.

<a id="s4-5"></a>
### 4.5 Defensive limits for hostile input

- Nesting: observed node depth stays far below 100; a limit of 256 nested nodes (ports
  and scriptlet invocations counted separately) is suggested.
- Counts: bound every count by the remaining bytes (a node needs at least 31 bytes, a port 1
  (a port of type 0 is a single byte), a PV 4, a target 7) before allocating.
- `bool8` fields other than 0/1, a port echo that differs from the port type, a target
  marker of 0, a switch-case reserved field other than 0, or an unknown node kind are
  malformed input; report and stop decoding that record.

---

<a id="s5"></a>
## 5. Node kinds

<a id="s5-1"></a>
### 5.1 Kind table (Confirmed codes; labels from enum 0x2300019A)

"Tables" and "scriptlets" count the nodes in all decoded 0x07 and 0x0C records. Status:
**C** = layout confirmed by decoding every instance to the end of its record; **R** =
Reported only (BIN field list; no instance in this install); **O** = OPEN.

| Code | Label | Tables | Scriptlets | Data | Status |
|---|---|---|---|---|---|
| 1 | Decal | 1,097 | 106 | §5.3 | C |
| 2 | Collection | 39,440 | 7,216 | none | C |
| 3 | EntityVFX | 3,855 | 7 | §5.3 | C |
| 4 | Execute | 886 | 37 | §5.2 | C |
| 5 | Run | 36,676 | 2,196 | §5.2 | C |
| 6 | Sequence | 14,764 | 393 | `LoopForever` bool8, `LoopCount` i32 | C |
| 7 | CinematicCamera | 2 tables | 0 | base block first; data unresolved | O (§10.3) |
| 9 | PhysicsImpulse | 31 | 6 | §5.3 | C |
| 10 | OnStop | 2,090 | 43 | none | C |
| 11 | WaveformBlend | 1 | 0 | `ScalarBlendScript_Persistent` bool8, `WaveformBlendScript_Waveform` wave | C |
| 12 | Switch | 24,549 | 6,458 | `SwitchScript_Property` u32 PID, `…_Persistent` bool8, `…_Autonomous` bool8, `…_Synchronized` bool8 | C |
| 13 | Light | 244 | 57 | §5.3 | C |
| 14 | WhenRunning | 1,771 | 723 | §5.2 | C |
| 15 | DistanceCheck | 13 | 14 | `Persistent`, `Synchronized`, `UsePhysics`: 3 × bool8 | C |
| 16 | StreakInfo | 92 | 0 | §5.3 | C |
| 17 | Random | 15,851 | 10,927 | `RandomScript_NoRepeats` bool8, `RandomScript_Autonomous` bool8 | C |
| 18 | MeshDeformer | 74 | 0 | `MeshDeformerScriptNode_AnimationAsset` DID (a clip), `…_AnimationTime` f32, `…_UseBoneWeights` bool8 | C |
| 19 | Scriptlet | 127,137 | 38,646 | §5.2 | C |
| 20 | Math | 236 | 55 | `Persistent` bool8, `Operator` u32 (enum 0x230000A4), `LHSValue` f32, `RHSValue` f32, `LHSProperty` u32, `RHSProperty` u32 | C |
| 21 | RemoveObjects | 15 | 0 | `SetPropertyScript_PropertyName` u32 PID, `SetPropertyScript_ServerOnly` bool8 | C |
| 22 | ScriptletParameter | 0 | 6,660 | `ScriptletParameterScript_Name` Pascal string | C |
| 23 | Raycast | 0 | 0 | §5.3 | R |
| 25 | Label | 211 | 13 | `LabelScript_Name` Pascal string | C |
| 26 | HeadingCheck | 36 | 9 | `Persistent` bool8, `Synchronized` bool8 | C |
| 27 | GameCallback | 3,991 | 523 | §5.2 | C |
| 28 | StoryHook | 1,602 | 5,008 | `StoryHookScript_Type` u32 (§9) | C |
| 29 | WhileResident | 0 | 0 | `WhileResidentScript_Persistent` bool8 | R |
| 30 | MotionBlurAfterEffect | 45 | 0 | §5.3 | C |
| 31 | Listener | 0 | 0 | `ListenerScript_MonitorName` u32, `ListenerScript_EventType` u32 | R |
| 32 | CameraShake | 332 | 127 | §5.3 | C |
| 33 | ModifyCompositorColor | 218 | 0 | `CompositorName` u32, `ColorName` u32, `Color` rgba | C |
| 34 | Conversation | 0 | 0 | §5.2 | R |
| 35 | OverlayAfterEffect | 280 | 0 | §5.3 | C |
| 36 | **Animation** | 382 | 26,835 | §6 | C |
| 37 | Streak | 221 | 87 | `StreakScript_Stop` bool8, `StreakScript_HoldingLocation` u32 | C |
| 38 | DepthFogAfterEffect | 9 | 0 | §5.3 | C |
| 39 | Color | 0 | 0 | `ColorScript_Propagate` bool8, `ColorScript_Color` rgba, `ColorScript_RampTime` f32 | R |
| 40 | SetProperty | 8,875 | 1,472 | §5.2 | C |
| 41 | MeshFX | 9,509 | 840 | §5.2 | C |
| 42 | AddObjects | 81 | 0 | as RemoveObjects | C |
| 43 | BloomAfterEffect | 99 | 0 | §5.3 | C |
| 44 | ModifyCompositorLayer | 199 | 0 | `CompositorName` u32, `LayerName` u32, `ImageNumber` i32, `Opacity` f32 | C |
| 45 | FireMissile | 183 | 0 | §5.2 | C |
| 46 | Detect | 26 | 0 | §5.3 | C |
| 47 | PlayerPhysics | 127 | 0 | §5.2 | C |
| 48 | Sound | 66,878 | 59,684 | §5.3 | C |
| 49 | SetResult | 0 | 0 | `SetResultScript_Result` u32 | R |
| 50 | While | 41 | 0 | §5.2 | C |
| 51 | MoveTo | 0 | 1 | §5.3 | C (one instance) |
| 52 | RemoveProperty | 13 | 0 | `RemovePropertyScript_PropertyName` u32 PID, `…_ServerOnly` bool8 | C |
| 53 | TextureCompositor | 39 | 0 | `CompositorName` u32, `ImageCompositorFile` DID, `MeshType` u32, `MaterialType` u32 | C |
| 54 | Beam | 2,620 | 120 | §5.3 | C |
| 56 | MonitorObjects | 29 | 0 | `MonitorName` u32, `OtherObjectScriptID` u32, `ThisObjectScriptID` u32, `Autonomous` bool8 | C |
| 57 | ScalarBlend | 933 | 1,524 | `ScalarBlendScript_Property` u32 PID, `ScalarBlendScript_Persistent` bool8 | C |
| 58 | Appearance | 29,386 | 406 | `AppearanceScript_Appearance` DID, `…_Key` u32, `…_Modifier` f32, `…_Persistent` bool8 | C |
| 59 | Particle | 38,993 | 2,361 | §5.3, plus the extra byte after the base block (§4.1) | C |
| 60 | EditMaterial | 2,873 | 13 | §5.3 | C |
| 61 | Goto | 279 | 40 | `GotoScript_Name` Pascal string | C |
| 62 | SoundEnvironment | 21 | 0 | `SoundEnvironmentScript_Name` u32, `…_Priority` i32 | C |
| 63 | When | 1,182 | 98 | as Switch (12) | C |
| 64 | ListenerEvent | 0 | 0 | a custom `Parameters` field, then `ListenerEventScript_EventType` u32 | O (field encoding unknown) |
| 65 | Timer | 19,355 | 2,240 | `LoopForever` bool8, `Duration` f32, `LoopCount` i32, `Autonomous` bool8, `AnimationScript_Speed` f32, `SpeedMultiplier` f32, `ToolsAnimationOverride` DID | C |
| 77 | AIPhysics | 0 | 0 | as PlayerPhysics | R |
| 78 | CustomLightingAfterEffect | 0 | 0 | §5.3, ending in a custom field | O (last field unknown) |
| 80 | TestCrash | 4 | 0 | none | C |
| 81 | SliderMeshDeformer | 8 | 0 | `BoneSlider_ValueArray_String` Pascal string, `BoneSlider_SliderDefinitionsFile` DID | C |

Codes 8, 24, 55, 66–76 and 79 are not registered by the client and do not occur. Type
notation: `bool8`, `u32`, `i32`, `f32` (`records.md` §1.1); `vec3` = 3 × f32; `rgba` = 4
bytes (`property-system.md` §4.7); `wave` = waveform (`property-system.md` §4.2, variable
length); `SI` = StringInfo (`records.md` §4, variable length); `pos` = Position
(`property-system.md` §4.6, variable length); Pascal string (`records.md` §1.4); DID = u32.

The data of a kind is the concatenation of its fields; there are no counts or lengths
around it, so a decoder must know every field type to skip a node. Fields bound to an
enum, PID or bitfield are plain u32. The parent classes of BIN contribute fields **before**
the class's own: Switch and When take their four fields from their common parent; Label
inherits from Collection (no fields); WaveformBlend takes `ScalarBlendScript_Persistent`.

<a id="s5-2"></a>
### 5.2 Kinds with custom fields (Confirmed unless marked)

| Kind | Layout in stream order |
|---|---|
| 4 Execute | `ExecuteScript_Callback` u32 (enum from DID map 0x28000007); `DefaultValue` = PV; `CallProperties` = property set |
| 5 Run | `version` u8 (always 1); `count` u8; `count` × { u32 target PID, u32 source PID (0 for none), PV (a null entry, i.e. a single u32 0, when there is no constant) }; then `RunScript_ID` u32 (a script key, enum 0x23000097), `RunScript_Persistent`, `RunScript_UnbindOnDeath`, `RunScript_Stop`, `RunScript_ExposeAsScriptLink` (4 × bool8), `RunScript_QueuingType` u32 (enum 0x2300009C). 38,361 of 38,872 Run nodes have count 0. |
| 14 WhenRunning | `Scripts`: u8 *n*, u32 script key × *n* (1 key in 2,331 of 2,494 nodes); `Names`: u8 *m*, u32 × *m* (*m* = 0 everywhere); `WhenRunningScript_Persistent` bool8 |
| 19 Scriptlet | `ScriptletScript_File` DID (a 0x0C record, or 0 for a placeholder); `Parameters` = property set (values handed to the scriptlet, §7.8). BIN lists a second DID field that is **not** serialized. 137,338 of 165,783 parameter sets are empty, 27,587 have one property, nearly always `AnimationScript_LocomotionDisplacement`. |
| 27 GameCallback | `GameScript_Package` DID; `DefaultValue` = PV; `CallProperties` = property set |
| 34 Conversation | `DefaultValue` = PV; `ConversationScript_DialogText` SI; `ConversationScript_EndText` SI; `ConversationScript_EmoteScript` u32; `ConversationScript_Speaker` DID. **Reported** (BIN; no instance). |
| 40 SetProperty | `SetPropertyScript_PropertyName` u32 PID; `PropertyValue` = PV whose PID equals the property name (equal in all 10,347 nodes; LC reads the pair as one entry with echo); `SetPropertyScript_ServerOnly` bool8 |
| 41 MeshFX | 20 fixed fields (`Entity` DID, `AttachToParent` bool8, `AllowPhysics` bool8, `FadeInTime`, `LifeTime`, `FadeOutTime`, `DestroyTime` 4 × f32, `HoldingLocation` u32, `BoneOffset` vec3, `OffsetRotationYaw`, `…Pitch`, `…Roll` 3 × f32, `RemoveRotation` bool8, `BoneType` u32, `Behavior` u32, `Persistent`, `OnWater`, `UseParentScale`, `UseParticleScale`, `IgnoreChildScale` 5 × bool8), `Scale` vec3; `Appearances`: VLE *n*, *n* × { u32 appearance DID (0x20), u32 appearance key, f32 modifier } (*n* = 0 in 10,214 of 10,349); `MeshIIDTargetProperty` u32. LC lacks `UseParentScale` and so misreads every MeshFX node of this install. |
| 45 FireMissile | 14 fixed fields (§5.3), then `ArcherDrivenProperties`: TSize *n*, *n* × { u32, u32, u32 *c*, and one more u32 only when *c* ≠ 0 }. Observed *n* = 0 in 174 nodes, 1 or 2 in 9; the element meaning is OPEN. |
| 47 PlayerPhysics, 77 AIPhysics | `PhysicsScript_ControllerType` u32; `PhysicsScript_Priority` f32; `ControllerProperties` = property set (generic physics-controller speeds, gravity …); `UNIQUEID` = 16 bytes (121 distinct values in 127 nodes) |
| 50 While | `WhileScript_PropertyName` u32 PID; `Values`: u8 *n*, PV × *n*; `Names`: u8 *m*, PV × *m*; `WhileScript_Inverse` bool8 |
| 64 ListenerEvent | a custom `Parameters` field, then `ListenerEventScript_EventType` u32. **OPEN** (no instance; field encoding unknown). |
| 78 CustomLightingAfterEffect | 21 fixed fields (§5.3), then a custom `CustomLighting_Groups` field. **OPEN** (no instance). |

**Where a MeshFX node attaches its entity (BIN9, T47; answers the implementer OPEN of
`character-screens.md` §2.1.4, 2026-10-23).** The node spawns its `Entity` and anchors it on the
holder (the entity running the script) by these rules; the placement frame (`BoneOffset`, the
three offset angles, `RemoveRotation`) and the scale are `appearance.md` §5.7:

| `AttachToParent` | `HoldingLocation` | `BoneType` | Anchor |
|---|---|---|---|
| 1 | not 0 | any | that holding location of the holder (list A, `setup.md` §6.3) |
| 1 | 0 | not 0 | the holder's bone of that bone type (`attach_map`, `setup.md` §6.2) |
| 1 | 0 | 0 | the holding location with key 6 (label `origin` in the base mapper of `skeleton-anim.md` §8.2) |
| 0 | any | not 0 | the holder's bone of that bone type, read once at placement |
| 0 | not 0 | 0 | that holding location, read once at placement |
| 0 | 0 | 0 | the holder's model origin, read once at placement |

- The two paths differ only for a node that sets both fields: an attached node then uses the
  holding location, an unattached one the bone. In this install 17 of the 10,349 MeshFX nodes
  set both, all of them attached, so they use the holding location; no unattached node sets
  both, so the reversed precedence never shows in this data (T49; counts are observations, the
  rule is read at run time).
- Key 6 is accepted by every holder that has a setup, whether or not its list A holds an
  element with that key; without one the anchor is the holder's model origin (Inferred). 4,827
  attached nodes set neither field and so use key 6 (T49). 11 of the 28,555 setups of this
  install hold such an element (none a playable body).

> **OPEN:** which frame a key-6 attachment uses on a holder whose setup's list A holds an element
> with key 6 (11 setups of this install, T47): that element's frame (its bone and offset, as for
> any other holding location of list A), or the model origin as for holders without one. Not
> traced. **Tessera rule:** that element's frame, as for any other holding location; the model
> origin only when list A has no key-6 element.
- When the holder has no setup, or lacks the named holding location or bone, the node shows
  nothing: the entity it spawned is removed.
- The table is the same as the effect attachment of `particle-runtime.md` §6.1 for unattached
  nodes; for attached nodes the precedence of the first two rows is reversed.

**Times.** What `FadeInTime`, `LifeTime`, `FadeOutTime`, `DestroyTime` and `Persistent` do at run
time (the fades of `Render_Opacity`, the scheduled fade-out, the removal and the stop of a
persistent node) is `appearance.md` §5.7 step 4 (BIN13 there).

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):**
> `appearance.md` §5.7 / `script-table.md` §5.2: no Accepted spec says what the MeshFX fields
> `FadeInTime`, `LifeTime`, `FadeOutTime` and `DestroyTime` do at run time, nor how they
> interact with `Persistent` and with the node stopping.
>
> **Answered (spec writer, 2026-10-23):** `appearance.md` §5.7 step 4 (revised the same day
> from a new BIN pass, rows A66 and A67 there); the paragraph "Times" above points to it.

> **OPEN (implementer, world-conform, 2026-10-23; proposed, filed by the spec writer):**
> `script-table.md` §5.2: on unattached MeshFX nodes the anchor should be read once at
> placement. Screen-figure props are held parts of the figure, so they keep following the
> anchor. No unattached node sets both fields in this install.
>
> **Answered (spec writer, 2026-10-23; T47, T50):** "read once at placement" is the client's
> rule for every holder, the creation screens' figures included (BIN9): an unattached node's
> entity takes the anchor's world frame (× the placement frame of `appearance.md` §5.7) at the
> moment it is placed and keeps it; it does not follow the holder's later movement, its
> animation or the screens' turntable. Following the anchor is therefore not the client's
> behaviour. **Tessera rule:** a prop of an unattached node may be built and drawn as a part of
> the figure, provided its world transform is the one computed at placement and stays fixed
> afterwards; only props of attached nodes follow the anchor every frame. In this install the
> case is rare on the screens: the 32 tables that name `CharGen_Stage` hold 3 unattached MeshFX
> nodes, all in one table and all with `Persistent` 0, so such a prop lives only its fade-in,
> life and fade-out (`appearance.md` §5.7 step 4; T50, counts are observations).

<a id="s5-3"></a>
### 5.3 Long fixed layouts (Confirmed unless the kind is marked R in §5.1)

Each line gives the field names (the common property-name prefix stripped) and types in
stream order.

| Kind | Prefix | Fields |
|---|---|---|
| 1 Decal | `DecalScript_` | Material DID, Origin vec3, Lifespan f32, Color rgba, Size wave, Size3D vec3, Rotation wave, Radius wave, LightingBlend u32, UseBackfaceCulling bool8, UsePreciseBackfaceCulling bool8, UseLandscapeBackfaceCulling bool8, BackfaceCullingAngle f32, UseHeightBasedOpacity bool8, HeightBasedOpacityRange f32, UseDepthBasedOpacity bool8, DepthBasedOpacityMin f32, DepthBasedOpacityRange f32, HoldingLocation u32, BoneType u32, BoneOffset vec3, RemoveBoneRotation bool8, Persistent bool8, Managed bool8, AttachToParent bool8, ScaleWithParent bool8, StickToParentEntity bool8, StickToWorldEntities bool8, StickToCells bool8, StickToLandscape bool8, StickToWater bool8, RenderPhase u32, RampInTime f32, RampOutTime f32, LODClass u32, DepthBias u32 |
| 3 EntityVFX | `EntityVFXScript_` | Type u32, Persistent bool8, RampInTime f32, SustainTime f32, RampOutTime f32, UseLocalTiming bool8, Intensity wave, Priority i32, ColorModify_Combiner u32, ColorModify_Color rgba, ColorModify_RedScale wave, ColorModify_GreenScale wave, ColorModify_BlueScale wave, OpacityModify_Combiner u32, OpacityModify_Opacity wave, AmbientLightBoost_AmbientLight rgba, ToonSilhouette_ColorAndOpacity rgba, ToonSilhouette_LineThickness wave, Rotate_RotationAxis u32, Rotate_CustomRotationAxis vec3, Rotate_Rotation wave, Rotate_RotationVelocity wave, LightShaft_LightSourceOrigin vec3, LightShaft_LightRange wave, LightShaft_Color rgba, LightShaft_Opacity wave, RedirectProperty u32 |
| 9 PhysicsImpulse | `PhysicsImpulseScript_` | IsJump bool8, ImpulseType u32, IsGlobalSpaceImpulse bool8, Impulse vec3, AirTime f32, ExpectedGravity f32 |
| 13 Light | — | Direction vec3, LightRange f32, LightDiffuse rgba, LightDiffuseIntensity wave, LightSpecular rgba, LightFalloff f32, ProjectorEnabled bool8, ProjectorTexture DID, ProjectorYaw wave, ProjectorPitch wave, ProjectorRoll wave, CoronaEnabled bool8, CoronaTexture DID, CoronaFadeSpeed f32, CoronaSize f32, CoronaOpacity f32, CoronaOcclusion bool8, CoronaOcclusionSize f32, KeyLight bool8, ShadowCaster bool8, Light_AllowBakedShadows bool8, LightScript_Lifespan f32, LightScript_FadeTime f32, LightScript_Offset vec3, LightScript_RunOnLocalPlayerOnly bool8, LightScript_OnlyInfluenceParent bool8, LightScript_NeverInfluenceParent bool8, LightScript_NeverCullOrOcclude bool8, LightScript_GlobalLight bool8, LightScript_HoldingLocation u32 |
| 16 StreakInfo | `StreakInfoScript_` | Color rgba, StartPoint vec3, EndPoint vec3, Material DID, LifeSpan f32, TrailDuration f32, FadeOutStartTime f32, MinSegmentTime f32, MinSegmentDistance f32, TextureScale f32, UseSplines bool8, SplineTesselation i32, SplinePointMinTime f32, SplinePointMinDistance f32 |
| 23 Raycast (R) | `RaycastScript_` | Persistent bool8, Synchronized bool8, CheckDynamicEntities bool8, NumRayGridPoints i32, CastBetweenTopLevelEntities bool8, EtherealToTypes u32 |
| 30 MotionBlurAfterEffect | `AfterEffectScript_` | *after-effect head*, MotionBlurAmount wave, MotionBlurHorizontalScaleSpeed wave, MotionBlurVerticalScaleSpeed wave |
| 32 CameraShake | `CameraShakeScript_` | FadeInTime f32, LifeTime f32, FadeOutTime f32, OffsetX wave, OffsetY wave, OffsetZ wave, MinDistance f32, MaxDistance f32, Persistent bool8 (BIN lists a custom `Offset` field before OffsetX that is **not** serialized: confirmed on 459 nodes) |
| 35 OverlayAfterEffect | `AfterEffectScript_` | *after-effect head*, OverlayTexture DID, OverlayMaterial DID, OverlayUseTextureAlpha bool8, OverlayOpacity wave, OverlayColor rgba, OverlayBlend u32, OverlayUTranslate wave, OverlayVTranslate wave, OverlayUScale wave, OverlayVScale wave, OverlayOpacityBlendsColorFromBlack bool8 |
| 38 DepthFogAfterEffect | `AfterEffectScript_` | *after-effect head*, DepthFogColor rgba, DepthFogNearDistance f32, DepthFogFarDistance f32, DepthFogSkyAmount f32 |
| 43 BloomAfterEffect | `AfterEffectScript_` | *after-effect head*, BloomAmount wave, BloomColor rgba |
| 45 FireMissile | `FireMissileScript_` | MissileDID DID, InitialHoldingLocation u32, InitialOffset vec3, InitialPitch f32, InitialSpeed f32, HeadingOffset f32, MaxRange f32, MaxHomingDegrees f32, Lifetime f32, AffectedByGravity bool8, Sticky bool8, AlwaysFireAtTarget bool8, NeverFireAtTarget bool8, EtherealTo u32, then the custom field of §5.2 |
| 46 Detect | `DetectScript_` | OtherObjectID u32, ThisObjectID u32, AllObjectsID u32, BoxMin vec3, BoxMax vec3, IgnoreRotation bool8, DetectionHeading f32, UseLOS bool8, LOSFromDetectTargetPortPosition bool8, Synchronized bool8, DetectionDynamicEntitiesOnly bool8, DetectionEtherealToTypes u32, RaycastCheckDynamicEntities bool8, RejectNotDetectableObjects bool8, RaycastEtherealToTypes u32, ExcludeScriptExecutorFromDetection bool8, ExcludeTargetPortEntityFromDetection bool8 |
| 48 Sound | `SoundScript_` | SoundID DID, FireAndForget bool8, MoveWithEntity bool8, UseCameraPosition bool8, MinRandomDistance f32, MaxRandomDistance f32, Offset vec3, Channel u32, RampOutTime f32, Pitch f32, PitchVariance f32 |
| 51 MoveTo | `MoveToScript_` | Move bool8, Turn bool8, Offset vec3, RestrictToForwardMovement bool8, AlignToTargetHeading bool8, TimeOut f32, Persistent bool8, SpeedMultiplier f32, FailureDistance f32, MinimumDistance f32, MaximumDistance f32, IsMoveVirtual bool8, ReceiveDynamicAvoidanceUpdates bool8, NoHardDestination bool8, FaceTargetAtEndOfMove bool8, AllowWaterMovement bool8, FailWhenMovementFrozen bool8, StayInZone bool8, MatchTargetVelocity bool8, DistanceLostBeforeFailure f32, UpdatePosition bool8, TurnSuccess f32, TurnOffset f32, UsePathplanning bool8, FindLineOfSight bool8, LOSEtherealToTypes u32, MoveRandomly bool8, GoStraightIfPathPlanningFails bool8, MoveToPosition pos |
| 54 Beam | `BeamScript_` | Persistent bool8, ColorAndOpacity rgba, TargetPosition pos, UseTargetPosition bool8, HeadMaterial DID, BodyMaterial DID, TailMaterial DID, BeamWidth f32, UntargetedBeamLength f32, SegmentSize f32, NumHeadSegments i32, NumTailSegments i32, TextureUScale f32, HeadStartPos f32, HeadAnimStartTime f32, HeadAnimSpeed f32, TailStartPos f32, TailAnimStartTime f32, TailAnimSpeed f32, RampInTime f32, SustainTime f32, RampOutTime f32, SourceHoldingLocation u32, SourceBoneType u32, SourceBoneOffset vec3, SourceUseBoundingBoxCenter bool8, TargetHoldingLocation u32, TargetBoneType u32, TargetBoneOffset vec3, TargetUseBoundingBoxCenter bool8, DetachBeforeRampingOut bool8, SourceDetachTime f32, TargetDetachTime f32, RandomStartRadius f32, RandomImpactRadius f32, BeamParticleSystem DID, HeadParticleSystem DID, BeamSegmentParticlesPerSecond i32, HeadParticlesPerSecond i32, HeadEntity DID, HeadEntityScaleSource u32 |
| 59 Particle | `ParticleScript_` | ParticleID DID, ForkParticleID DID, ShouldFade bool8, FarFadeMinDistance f32, FarFadeMaxDistance f32, CloseFadeMinDistance f32, CloseFadeMaxDistance f32, HoldingLocation u32, BoneType u32, BoneOffset vec3, DropEffect bool8, RemoveRotation bool8, OnWater bool8, ImmediateDisposal bool8, RedirectProperty u32 |
| 60 EditMaterial | `EditMaterialScript_` | MaterialProperty u32, MeshType u32, MaterialType u32, Priority i32, WaveformValue wave, ColorValue rgba, TextureValue DID, BoolValue bool8 |
| 78 CustomLightingAfterEffect | `AfterEffectScript_`, `CustomLighting_` | *after-effect head*, ReplaceSceneLighting bool8, BrightLight_Rim_AboveDistance f32, BrightLight_Front_Range f32, BrightLight_Rim_Range f32, BrightLight_Front_Color rgba, BrightLight_Rim_BehindDistance f32, DarkLight_Front_Falloff f32, DarkLight_Top_Color rgba, BrightLight_Rim_SideDistance f32, LowerAmbientLight rgba, BrightLight_Front_Falloff f32, DarkLight_Front_Range f32, UpperAmbientLight rgba, DarkLight_Front_Color rgba, then the custom field of §5.2 (R, O) |

The *after-effect head* shared by kinds 30, 35, 38, 43 and 78 is `Persistent` bool8,
`RampInTime` f32, `SustainTime` f32, `RampOutTime` f32, `UseLocalTiming` bool8,
`RunOnLocalPlayerOnly` bool8, `Priority` i32.

---

<a id="s6"></a>
## 6. The animation node (kind 36)

<a id="s6-1"></a>
### 6.1 Data layout (Confirmed, 27,217 of 27,217; always 59 bytes)

| Offset | Field (property) | Type | Meaning | Observed |
|---|---|---|---|---|
| 0 | `Asset` (`AnimationScript_AnimAsset`) | DID | The clip (0x05). | Exists in 27,215; 2 name missing clips |
| 4 | `LoopForever` (`TimerScript_LoopForever`) | bool8 | Loop until stopped | 1 in 10,223 |
| 5 | `LoopCount` (`TimerScript_LoopCount`) | i32 | Plays when not looping forever | 1 in 27,146; 2–600 in 71 (68 of them with `LoopForever` 0) |
| 9 | `RampIn` (`AnimationScript_RampIn`) | f32 | Blend-in time, seconds | 0 (10,155), 0.2 (8,943), 0.1 (4,984), 0.3 … up to 3.0 |
| 13 | `RampOut` (`AnimationScript_RampOut`) | f32 | Blend-out time, seconds | 0 (10,044), 0.2 (8,842), 0.1, 0.3 … up to 3.0 |
| 17 | `Layer` (`AnimationScript_Layer`) | u32 | Layer, enum 0x230000D5 (§6.2) | 40 (8,864), 30 (5,666), 20 (3,563), 10 (2,613) … |
| 21 | `UseBoneOpacity` (`AnimationScript_UseBoneOpacity`) | bool8 | Use per-bone opacity (§6.5) | 1 in 23,469; driven by `ScriptSystem_CurrentlyMoving` in 7,722 |
| 22 | `ManualUpdate` (`AnimationScript_ManualUpdate`) | bool8 | Hold the clip at `ManualTime` instead of playing it (§6.7) | 1 in 30 |
| 23 | `ManualTime` (`AnimationScript_ManualUpdateTime`) | f32 | Held time as a **fraction** of the played range, with a wrap rule (§6.7); used only when `ManualUpdate` = 1 | 0 in 27,197; 0.1–1.0 otherwise; driven by a property in 7 nodes (§6.8) |
| 27 | `StartTime` (`AnimationScript_StartTime`) | f32 | Start of the played range, a **fraction** of the clip (§6.10) | 0 in 27,172; 0.05–0.65 otherwise |
| 31 | `EndTime` (`AnimationScript_EndTime`) | f32 | End of the played range of a one-shot clip, a fraction (§6.10) | 1 in 27,172; 0.1–0.9 otherwise |
| 35 | `RandomizeStartTime` (`AnimationScript_RandomizeStartTime`) | bool8 | Start at a random fraction in [`StartTime`, `EndTime`] (§6.10) | 1 in 16 |
| 36 | `Speed` (`AnimationScript_Speed`) | f32 | Playback rate | 1.0 in 26,667; 0.5–2.0 otherwise; **driven** in 14,278 (§8.5) |
| 40 | `SpeedMultiplier` (`TimerScript_SpeedMultiplier`) | f32 | Second rate factor | 1.0 in 27,193 |
| 44 | `LocomotionDisplacement` (`AnimationScript_LocomotionDisplacement`) | vec3 | Ground displacement over one clip cycle, metres, entity frame (§6.3) | (0,0,0) in 26,329; only y in 550; x and y in 207; only x in 130; z in 1 |
| 56 | `DetachOnRampOut` (`AnimationScript_DetachOnRampOut`) | bool8 | Changes which part of the client handles the end of a one-shot clip (§6.10); visible effect OPEN | 1 in 1,991 |
| 57 | `SynchronizeTiming` (`AnimationScript_SynchronizeTiming`) | bool8 | For looping clips: start on the layer's shared time origin instead of now (§6.10, BIN3) | 1 in 27,111 |
| 58 | `RequiresUpdatingEvenWhenCulled` (`AnimationScript_RequiresUpdatingEvenWhenCulled`) | bool8 | Keep updating off screen (Reported by name; copied into the clip record, use not traced) | 1 in 57 |

BIN gives this order and these value kinds; LC has no reader for kind 36, which is why it
fails on every table that contains one. The client's constructor defaults (BIN, Reported)
are `RampIn` = `RampOut` = 0.1 and `Speed` = `SpeedMultiplier` = 1.0; serialized values
always override them. What the client does with each field when the node starts a clip is
§6.10 (BIN3). Fields that can be **driven** (base block, §4.2) in this install: `Speed`
(14,278 nodes), `UseBoneOpacity` (7,722), `Layer` (426), `StartTime` (39), `ManualTime` (7),
and `AnimAsset`, `EndTime`, `LocomotionDisplacement`, `TimerScript_SpeedMultiplier` once
each (OBS2).

<a id="s6-2"></a>
### 6.2 Layers (Confirmed values; combination rule BIN, T20)

Enum 0x230000D5 (base 0x230001AF adds 0 `Invalid`, 255 `tools`): 10 idle, 11
idle_adjustment, 13 mounted_idle, 14 idle_stance_blending, 15 idle_emote, 18 face_emote,
20 movement, 25 emote_movement_blend, 30 emote, 35 recovery, 37 monster_hits_low, 40
attack, 43 monster_hits, 51 mounted_warsteed_idle, 52 mounted_idle_emote, 54
mounted_movement, 55 mounted_movement_blend, 56 mounted_movement_combat_idle, 58
mounted_attack, 60 mounted_jumping, 65 mounted_falling, 75 mounted_locomotion_offset, 76
knockdown, 78 death, 80 chargen, 90 chargen_emote, 110 scale_blend.

The numeric order matches a priority order (idle < movement < emote < attack < death). The
avatar data relies on it: the base idle (layer 10) and a looping idle-emote (layer 15) are
started together by the `Startup` script, and locomotion clips (layer 20) start on top
whenever the character moves (§8.4).

**Combination (BIN2, T20).** The player keeps every started clip together with its layer
number. Per bone, layers are visited from the highest number down; each clip's weight is its
ramp envelope × the weight the higher layers left × its track opacity (§6.5), the clips of
one layer share the same leftover, and lower layers receive only what remains. All clips that
end up with weight are averaged; a bone whose weights add up to less than 0.1 is pulled to
the reference pose. The full rule, its 1/255 quantisation and worked examples are in
`skeleton-anim.md` §6.6 and §14.6–§14.8. Layers never add to each other (no additive mode;
`skeleton-anim.md` §6.6), and a layer is just a number: nothing but the order of the numbers
distinguishes `scale_blend` (110) from the others. Clips on one layer cross-fade by their
ramp envelopes (a new clip's `RampIn`, the old clip's `RampOut`). Starting a clip never stops
another clip of the same layer; several clips share a layer until their own branches stop
them. The client creates a layer when its first clip starts and refuses a layer number above
255 (BIN3). A layer has no limit on its number of clips: when it holds 1,000 the client logs a
warning and still plays the new clip (BIN5, T40).

<a id="s6-3"></a>
### 6.3 `LocomotionDisplacement` (Confirmed)

The vector is the authored ground distance of one clip cycle, in the entity frame: **x to
the right, y forward, z up**, in metres. Evidence:

- for 158 clips compatible with the three measured player skeletons, displacement y
  divided by the clip duration (`havok-data.md` §2.3 field 1) is close to the foot-contact
  speed of `skeleton-anim.md` §7.4: median ratio 1.07, middle half 0.98–1.34, extremes 0.64
  and 3.1. It is an authored value, not a measurement: the Elf M run clip 0x050093C0 states
  12.9 m over 1.466 s (8.8 m/s) while its feet slide at about 6.4 m/s;
- avatar walk clips give 1.8–2.1 m/s and run clips 6.5–8.9 m/s (table in §8.3);
- strafe bindings carry ±x only, diagonal bindings equal x and y, backward bindings −y.

So the clip's **natural speed** is taken as |displacement| / duration. A Scriptlet node's
parameter set (§7.8) often carries a displacement for the clips it invokes (26,132 parameter
sets carry it), but it reaches an animation node **only through a scriptlet-driven pair** of
that node (`appearance.md` §7.4.1 rules 1–4; 4,812 pairs on
`AnimationScript_LocomotionDisplacement`, A61 there); without such a pair the node keeps its
own stored value (5,614 cases, A61). (Revised 2026-10-23: the earlier reading, that the
parameter set overrides the node's own value by PID, is superseded by rule 4.)

**Effect of rule 4 (T46).** It changes displacements, not clip choices: over the 14 avatars of
§8.3 no clip changed and three displacements appear (DwarfStoutAxe walk 2.00 m/s and run
6.00 m/s, RiverHobbit F walk back −2.57 m/s); over 206,374 starts the implementer's run found
no primary clip changed on layers 10 and 20, displacements changed in 429 (state, layer)
cells of 389 starts, two objects choose another layer-15 idle clip and eight gain a layer-10
idle clip through a `ScriptletScript_File` pair (Reported, `appearance.md` §15). The original
uses the displacement only for its animation component's locomotion velocity
(`skeleton-anim.md` §7.4) and never matches a clip's rate to the ground speed; Tessera's own
speed matching takes the natural speeds that rule 4 gives.

**Entity scale (T31, BIN3).** When a clip starts, the client multiplies its displacement by
the mean of the entity's three scale components ((*sx* + *sy* + *sz*) / 3; 1 for an entity
without a scale). A River-hobbit at scale 0.7 therefore has natural speeds 0.7 times those
of §8.3. The client uses the scaled displacement for the locomotion velocity that its
animation component computes (`skeleton-anim.md` §7.4).

<a id="s6-4"></a>
### 6.4 Ports of an animation node (Confirmed)

Only port type 6 occurs (7,771 nodes have ports; 24,507 ports). Its payload is an `f32`
time in **clip seconds**: 24,178 times lie in [0, duration], 329 beyond the duration. Those
329 **never fire** (BIN3): the node schedules its ports like a Timer (§7.6), each cycle from
the cycle's start at port time ÷ rate, and a new cycle restarts the list before a port past
the cycle's end is reached; a one-shot clip ends first. The children run when playback passes that time (the
animation-event mechanism, §9). Children are Scriptlet nodes (13,358, mostly footstep and
effect scriptlets), Random (3,283), StoryHook (2,771), Sound (2,279), Particle (1,488),
Switch, Run, Collection, WhenRunning, Timer, CameraShake and Light nodes.

<a id="s6-5"></a>
### 6.5 `UseBoneOpacity` and track weights (Confirmed data, rule BIN2)

6,935 clips carry per-track weights (`havok-data.md` §2.3 field 8). The name suggests that
`UseBoneOpacity` = 1 makes the player apply them, but the data does not prove it: of the
animation nodes on weighted clips, 6,612 have it set and 1,125 do not; of those on
unweighted clips, 16,857 have it set (where it can change nothing). It is driven by
`ScriptSystem_CurrentlyMoving` in 7,722 nodes, which fits "upper-body only while moving".

**Rule (BIN2).** When `UseBoneOpacity` is 1 and the clip has a weight array, the clip's
weight for track *j* is that track's **opacity**: it multiplies the clip's weight on bone *j*
in the layer budget of §6.2 (`skeleton-anim.md` §6.6). Otherwise every opacity is 1. When the
flag changes during playback (driven nodes), the player moves between the two linearly over
0.2 s (`skeleton-anim.md` §6.6, K22, BIN3). Visual confirmation (emote while running,
legs keep running) is still welcome but no longer needed to implement.

<a id="s6-6"></a>
### 6.6 Where animation nodes are (Confirmed)

26,835 animation nodes are in scriptlets, 382 in tables. 25,356 distinct clips are named by
an animation node; together with Timer `ToolsAnimationOverride` (a tool-preview field, 953
clips), MeshDeformer nodes (32) and DID lists, 25,387 of the 25,395 clips are referenced
from script data. The 0x0C record of a clip is usually a single animation node, optionally
with event ports, so "play clip X with these settings" is "invoke scriptlet Y".

<a id="s6-7"></a>
### 6.7 Held clips: `ManualUpdate` and `ManualTime` (Confirmed census, rule BIN2, T19)

An animation node with `ManualUpdate` = 1 starts its clip like any other (layer, ramps,
opacity, §6.1–§6.5) but **never advances it**: its playback rate is 0 whatever `Speed` and
`SpeedMultiplier` say, so time-based events of §6.4 do not fire from the passing of time. Its
time comes from `ManualTime`, after the driven-property substitution of §4.2, as follows. Let
*v* be the value, *n* its integer part (*v* truncated toward zero and converted to a signed
32-bit integer) and *g* the fractional part of *v* (with the sign of *v*, so *v* = *n* + *g*
whenever *n* is representable).

1. **Fraction** *f*:
   - `LoopForever` = 1: *f* = *g*, plus 1 when *g* is negative. So 1.0 gives 0, 1.3 gives
     0.3, −0.2 gives 0.8. When *g* is negative but so close to 0 that *g* + 1 rounds to 1 in
     single precision, *f* is 1; it is not wrapped again (BIN5, T36).
   - `LoopForever` = 0: *f* = 0 when *n* < 0; *f* = *g* when 0 ≤ *n* < `LoopCount`; *f* = 1
     when *n* ≥ `LoopCount`. Then *f* is limited to [0, 1] by step 2 (so −0.2 gives 0).
   - Values that are not finite or whose magnitude is 2³¹ or more (BIN5, T36): with
     `LoopForever` 0 the integer conversion yields its out-of-range result, which is
     negative, so *f* = 0 (for +∞, −∞, NaN and also, for example, 3 × 10⁹). With
     `LoopForever` 1 an infinite value has fractional part 0, so *f* = 0, and a finite value of
     that size is a whole number, so *f* = 0 as well; NaN is passed on unchanged and the
     client then samples the clip at an undefined time. **Tessera Decision:** NaN on a
     looping node gives *f* = 0 (OPEN: no instance, and the client has no defined result).
2. **Clip time** = *f* × the clip's **played length**, limited to [0, played length] (BIN5,
   T37). The played length is the tagfile `duration` minus the end crop of `havok-data.md`
   §2.3: 1/30 s when the node has `LoopForever` 0 and `LoopCount` exactly 1 (its clip is then
   started for one cycle), otherwise 0. `StartTime` and `EndTime` do not enter: the client
   never sets a start crop, and `EndTime` only schedules the end of a played one-shot clip
   (§6.10 step 4), which a held clip does not have. The time from §6.10 step 5 that the clip
   starts with is replaced as soon as the fraction is applied. A time equal to the played
   length stays there: the player's time step does not move a held clip (rate 0) and wraps
   only times beyond the played length. (This replaces the presumed cropping formula of
   2026-10-05 and resolves its OPEN.)
   **Played length 0 or less** (BIN6, T43): a held node with `LoopForever` 0 and
   `LoopCount` 1 whose clip lasts at most 1/30 s has a played length of `duration` − 1/30 s,
   which is 0 or negative. The client takes the smaller of the played length and *f* × the
   played length, then the larger of that and 0, so the clip time is **0** for every *f*:
   the clip shows its first frame. The negative starting time that `havok-data.md` §2.3
   gives such a clip (lowered to `duration` − 1/30 s) does not survive: the middleware's
   time step raises any negative time to 0 and, with a played length of 0 or less, advances
   nothing and never ends the clip. No held node of this install names such a clip (the
   shortest clip of the install lasts 0.016 s).
3. The fraction is applied again on every update of the node and whenever a property that
   drives one of its fields changes, so a new value takes effect at once (no ramp).

The client sets no limit on held clips per entity: they are ordinary clips of their layers
(§6.2, T40).

> **Answered** (2026-10-07, BIN5; was the implementation OPEN on the three Tessera rules of
> the §16 review record and on step 2): steps 1 and 2 now state the client's rules. A
> `ManualTime` that is not finite gives 0 in the client too, except NaN on a looping node,
> where the Decision of step 1 applies. A looping fraction that rounds up to 1 is **not** 0
> in the client: it stays 1 and gives the end of the played length. The only non-finite
> fraction that can reach the player is that NaN, so the player's "non-finite reads 0" is the
> same Decision. The presumed cropping formula is withdrawn: use step 2 (no `StartTime` or
> `EndTime`; end crop 1/30 s for `LoopForever` 0 with `LoopCount` 1).

Census: 30 of 27,217 nodes are held: 13 on layer 40, 5 on layer 30, 4 on layer 80, 8 on
layer 110. `ManualTime` is driven by a property in 7 of them (§6.8; one layer-80 node by
`Character_Demo_Pose_Override_Time`). Worked example (synthetic, clip with a tagfile
`duration` of 2.0 s, `StartTime` 0, `EndTime` 1; played length 2.0 s in the first and last
columns, 2.0 − 1/30 = 1.9666667 s in the middle one, step 2; revised 2026-10-07):

| `ManualTime` | `LoopForever` 1 | `LoopForever` 0, `LoopCount` 1 | `LoopForever` 0, `LoopCount` 2 |
|---|---|---|---|
| 0.25 | 0.5 s | 0.4916667 s | 0.5 s |
| 0.95 | 1.9 s | 1.8683333 s | 1.9 s |
| 1.0 | 0.0 s | 1.9666667 s | 0.0 s |
| 1.3 | 0.6 s | 1.9666667 s | 0.6 s |
| −0.2 | 1.6 s | 0.0 s | 0.0 s |
| −10⁻⁹ | 2.0 s (*f* = 1) | 0.0 s | 0.0 s |
| 3 × 10⁹ | 0.0 s | 0.0 s | 0.0 s |
| +∞ | 0.0 s | 0.0 s | 0.0 s |
| NaN | 0.0 s (Decision) | 0.0 s | 0.0 s |

<a id="s6-8"></a>
### 6.8 Layer 110 `scale_blend`: River-hobbit and DwarfStoutAxe proportions (Confirmed, T21)

All 8 animation nodes on layer 110 are held clips (§6.7) used to change body proportions,
not to animate. Where they are and what drives them:

| Body | Where | Clip (duration) | `ManualTime` from | `LoopForever` | `RampIn`/`RampOut` |
|---|---|---|---|---|---|
| River-hobbit M | table 0x07002F08, script `Reproportion_Idle` (key 0x10002CC4): scriptlet 0x0C00AF73 | 0x05008E76 (1.666 s) | `AC_Avatar_Height` | 1 | 0.1 / 0.1 |
| River-hobbit M, mounted branch | same script: scriptlet 0x0C00AFCB | 0x05008ECC (1.666 s) | `AC_Avatar_Height` | 1 | 0.1 / 0.1 |
| River-hobbit F | table 0x07002F09, `Reproportion_Idle`: scriptlet 0x0C00AF74 | 0x05008E75 (0.416 s) | `AC_Avatar_Height` | 1 | 0 / 0 |
| River-hobbit F, mounted branch | same script: scriptlet 0x0C00AFCC | 0x05008ECB (0.416 s) | `AC_Avatar_Height` | 1 | 0 / 0 |
| DwarfStoutAxe | table 0x0700247C, `Reproportion_Idle`: scriptlet 0x0C009506 | 0x050079DF (1.666 s) | `AC_BoneScale_Test` | 0 (`LoopCount` 1) | 0 / 0 |
| DwarfStoutAxe, mounted on mount types 8, 16, 32 | same script: scriptlet 0x0C0095FB | 0x05007AC2 (1.666 s) | `AC_BoneScale_Test` | 0 (`LoopCount` 1) | 0 / 0 |
| River-hobbit F / M, UI viewport | binding tables 0x07002F1A / 0x07002F1C, script `Viewport_Idle` (key 0x10000524) | 0x05008ECB / 0x05008ECC | constant 1.0 (so fraction 0, §6.7) | 1 | 0 / 0, 0.1 / 0.1 |

All have `UseBoneOpacity` 1, `StartTime` 0, `EndTime` 1. The DwarfStoutAxe nodes
(`LoopForever` 0, `LoopCount` 1) hold their clips over a played length of `duration` − 1/30 s
(§6.7 step 2); the River-hobbit nodes (looping) over the full `duration`. The `Startup` script of each of the
three species tables runs `Reproportion_Idle` with a Run node; the script's root is a
persistent WhenRunning node on keys 0x10000147 (and, for the River-hobbit tables, also
0x10002EE4): while none of them runs (the normal, unmounted case) its NotRunning port starts
the first clip of the table above; while one runs, its Running port starts the mounted one
(DwarfStoutAxe: through a Switch on `Mount_CurrentMountType`, cases 8, 16, 32, Default
back to the normal clip). Other tables also run `Reproportion_Idle` (0x070013C5 under some
`Appearance_RandomAppearanceType` cases, 0x0700291D); they are not avatar tables.

**What the clips hold (Confirmed, all tracks).** 89 tracks (62 for DwarfStoutAxe), weight
array 0.01 on the tracks that matter and 0 elsewhere, so with the layer rule of §6.2 a track
moves only 2/254 of the way from the lower layers' value to the clip's value (effective
weight 0.00787 over a full-weight idle). The clips are authored about 100 times too large for
that: River-hobbit M root translation z −12.5 at fraction 0 rising to about 0 at fraction 1
(−9.80 at 0.30, −4.94 at 0.57, −0.09 at 0.95), knee and ankle translation z 6.0 above their
reference at fraction 0 falling to the reference at 1; River-hobbit F root the same and knee
12.41 above (ankle unchanged); eye-offset bones at scale 31.25 with translation × 31.25 and
four finger-base bones moved +0.8 to +0.9 along y, both constant. At the default height value 0
the River-hobbit pelvis therefore sinks about 0.098 m and the legs shorten by about the same,
so the feet stay on the ground (worked example §13.2). The mounted River-hobbit clips shorten
the legs only (no pelvis drop) and reach the reference by fraction 0.5. The DwarfStoutAxe clip
raises the root 0.099 m and lengthens hip, knee and ankle segments by the same amount (taller
on longer legs, feet on the ground), and widens collar, shoulder, elbow and wrist offsets
more the higher `AC_BoneScale_Test` is.

**Consequences for an implementation.** Start these nodes as §8.3 starts the idle (they are
part of the avatar's `Startup`), hold them by §6.7 and combine them by §6.2. A clip on layer
110 that is *played* over time instead makes the figure grow and shrink in a loop (River-hobbit
M: 1.666 s period), which is the M4 viewer symptom of `docs/checklists/m4-visual.md` note 1.
Expected standing heights: `appearance.md` §6.5.

<a id="s6-9"></a>
### 6.9 Deformer nodes: MeshDeformer (18) and SliderMeshDeformer (81) (Confirmed census, rule BIN2, T22)

These nodes do not play animation; they register a **bind-space deformation** of the
entity's skinned meshes with its animation component, kept while the node runs, and
recomputed when a field changes (`appearance.md` §6.4 gives the algorithm).

| Kind | Fields used | Instances |
|---|---|---|
| 18 MeshDeformer | `MeshDeformerScriptNode_AnimationAsset` (a clip), `MeshDeformerScriptNode_AnimationTime` (a **fraction** of the clip, limited to [0, 1], no wrap), `MeshDeformerScriptNode_UseBoneWeights` (use the clip's weight array as track weights; otherwise all tracks weight 1). Full property names from the dictionary (OBS2); every driven pair on these nodes names `MeshDeformerScriptNode_AnimationTime` (49 nodes) | 74, all in tables: 15 are body-shape deformers in `Startup` scripts (14 species tables below and 0x07001819), all with `AnimationTime` driven by `AC_BoneScale_Test` and `UseBoneWeights` 1 (`appearance.md` §6.4); the other 59 are in 9 non-avatar tables (32 in 0x07001AFB driven by `Slider_Custom2`, 2 driven by `Slider_Custom1`, the rest constant, for example 0.56, 1 and 0.15 in table 0x07001B8E) |
| 81 SliderMeshDeformer | `BoneSlider_ValueArray_String` (the field every driven pair of these nodes names, 8 of 8; the slider value string; the stored value is 70 characters: 34 × `80` and `00` at slider 24), `BoneSlider_SliderDefinitionsFile` (the 0x78 definitions record) | 8, all in `Startup` scripts of the Human, Elf, HighElf and River-hobbit species tables (0x07002DAD, 0x07002DB0, 0x07002F08, 0x07002F09, 0x07003012, 0x07003013, 0x0700301A, 0x0700301B), all with the string driven by `AC_BoneSlider_Value_Array` |

Each of the 14 avatar closures (§8.1) contains exactly one of these **species tables**
(Confirmed, 14 of 14; the fifteenth body-shape table, 0x07001819, is in no avatar closure):

| Avatar entity | Species table | Body-shape clip (§6.9 kind 18) | Slider definitions (kind 81) | Layer-110 script (§6.8) |
|---|---|---|---|---|
| Hobbit M 0x47000606 | 0x0700041F | 0x05000DF9 | – | – |
| Hobbit F 0x47000607 | 0x07000425 | 0x05000DFF | – | – |
| Human M 0x4700060A | 0x07002DAD | 0x050086F8 | 0x7800A9F1 | – |
| Human F 0x4700060B | 0x07002DB0 | 0x05008710 | 0x7800A9F2 | – |
| Elf M 0x4700060E | 0x07003013 | 0x050096B2 | 0x7800A9F1 | – |
| Elf F 0x4700060F | 0x07003012 | 0x050096B1 | 0x7800BEAB | – |
| Dwarf M 0x47000611 | 0x07000491 | 0x05001F42 | – | – |
| Beorning M 0x47007004 | 0x07001D83 | 0x0500686F | – | – |
| Beorning F 0x47007005 | 0x07001D85 | 0x05006870 | – | – |
| HighElf M 0x4700806C | 0x0700301A | 0x050096B2 | 0x7800A9F1 | – |
| HighElf F 0x47008071 | 0x0700301B | 0x050096CF | 0x7800BEAB | – |
| DwarfStoutAxe 0x4700979A | 0x0700247C | 0x050079E0 | – | `Reproportion_Idle` |
| River-hobbit M 0x4700CEA3 | 0x07002F08 (and binding table 0x07002F1C) | 0x05008E43 | 0x7800A9F3 | `Reproportion_Idle` |
| River-hobbit F 0x4700CEA4 | 0x07002F09 (and binding table 0x07002F1A) | 0x05008E42 | 0x7800A9F4 | `Reproportion_Idle` |

The three slider-definition records 0x7800A9F1, 0x7800A9F2 and 0x7800BEAB have identical
content apart from their DID. Root nodes of the species tables' `Startup` scripts
(Confirmed): always a Collection and the MeshDeformer; a Scriptlet node in the Hobbit, Dwarf
and Beorning tables; the SliderMeshDeformer and one or two Switch nodes in the Human, Elf and
HighElf tables; a Switch and the Run node of §6.8 in the DwarfStoutAxe table; a Switch on
`CharGen_Stage`, the SliderMeshDeformer, the Run node and an Appearance node in the
River-hobbit tables. An implementation that runs the `Startup` script of every closure table
(§8.2) reaches them.

**Edge cases (BIN5, T38, T39).** None of these occurs in the install.

- A MeshDeformer registers its clip, fraction and `UseBoneWeights` with the entity's
  animation component whatever they are. When the deformation pose is built
  (`appearance.md` §6.4 step 1), a clip that cannot be loaded, the null DID included,
  contributes nothing, and a clip whose track count differs from the skeleton's bone count
  is reported as an error and contributes nothing. With no contributing clip the pose is the
  reference pose. The component keeps any number of deformers.
- The deformer time is `duration` × fraction, limited to [0, `duration`]: +∞ gives the end of
  the clip and −∞ gives 0. A NaN fraction gives the client an undefined time. **Tessera
  Decision:** NaN reads as fraction 0 (OPEN: no instance).
- A SliderMeshDeformer parses whatever string its `BoneSlider_ValueArray_String` field holds,
  each time the field is set: the stored string when no driven pair names the field, the
  property's string when one does (the parse is `appearance.md` §6.3).
- An entity holds **one** slider set. Starting a SliderMeshDeformer replaces the set of any
  other one, and stopping any SliderMeshDeformer removes the set, even when another is still
  running. (Each avatar closure has exactly one such node, SI12.) Which of two nodes
  started in the same script run counts as the later one follows the client's start order
  (§8.2 step 3, §7.5; BIN6, T41, T42).
- A change of `BoneSlider_SliderDefinitionsFile` reloads the definitions record. Whether the
  client then parses the string again was not traced (OPEN: no node drives this field).

> **Answered** (2026-10-07, BIN5; was the implementation OPEN on a null
> `MeshDeformerScriptNode_AnimationAsset`): the client registers the entry, which then
> contributes nothing (first point above). That looks the same as registering nothing, so
> the implementation's choice is equivalent. Binding the driven pairs by the field PIDs of
> the three names is right (§4.2, SI13).

<a id="s6-10"></a>
### 6.10 What happens when an animation node starts a clip (BIN3, T29)

The client's rules, in order, for one animation node (after the driven-property
substitution of §4.2 and the scriptlet-parameter overrides of §7.8):

1. **Request.** The node asks the entity's animation component to play `Asset` on `Layer`
   with: target weight = the node's intensity; rate = `Speed` × `SpeedMultiplier`;
   displacement = `LocomotionDisplacement` × the entity's mean scale (§6.3); `LoopCount`,
   `LoopForever`, `RampIn`, `RampOut`, `UseBoneOpacity`, `ManualUpdate`, `SynchronizeTiming`,
   `DetachOnRampOut`, `RequiresUpdatingEvenWhenCulled`; a start fraction *f*₀ = `StartTime`,
   or, with `RandomizeStartTime` = 1, a uniformly random fraction in [`StartTime`,
   `EndTime`]; and `EndTime`. While the client is **starting** one of the entity's start-up
   scripts (§8.1, §8.2), `RampIn` is replaced by 0, so start-up clips appear at full weight at
   once. This covers only the requests made during that start, before it returns (T48): a
   clip that the same script starts later (after a Timer, a Sequence step that waits for an
   earlier node, a later Random choice or a Switch that changes branch) ramps in by its own
   `RampIn`.
2. **Refusals.** Nothing plays when the layer number is above 255, when the clip's track
   count differs from the skeleton's bone count (`skeleton-anim.md` §3.4), or when the
   entity is a shared object (one script instance serving many entities) and the clip is
   one-shot (`LoopForever` 0, `ManualUpdate` 0, rate above 0.0002). The track-count
   mismatch and the one-shot clip on a shared object are reported as errors (BIN4); a layer
   above 255 is refused silently.
3. **Start time** τ₀. τ₀ = now − *f*₀ × duration / rate when *f*₀ > 0 and the rate is
   above 0.0002, otherwise now. With `LoopForever` = 1 **and** `SynchronizeTiming` = 1: when
   the layer holds no looping clip with a rate above 0.0002, the layer remembers this τ₀ as
   its **origin**; otherwise the new clip takes the layer's remembered origin as its τ₀. (So
   a looping clip that replaces another synchronised looping clip on the same layer starts
   as if it had been started together with the first one.)
4. **End of a one-shot clip** (`LoopForever` 0, `ManualUpdate` 0, rate at least 0.0002):
   τ_end = τ₀ + (`LoopCount` × duration − (1 − `EndTime`) × duration) / rate. If τ_end has
   already passed, the clip is not started. A stop is scheduled at τ_end − `RampOut`, so the
   fade-out ends at τ_end (`skeleton-anim.md` §6.5). Looping clips and held clips have no
   end; `EndTime` does not affect them.
5. **Initial clip time.** min(now − τ₀, duration), at least 0. This is a time in seconds of
   wall time, not multiplied by the rate (for rate 1, `StartTime` × duration). From then on
   the clip time advances by elapsed time × rate and wraps at the duration for looping
   clips (`skeleton-anim.md` §6.3). A held clip (`ManualUpdate` = 1) has rate 0 and takes
   its time from §6.7.
6. **Weight.** The clip's overall weight ramps as `skeleton-anim.md` §6.5 states (linear
   `RampIn` from 0 to the target weight); its track opacities start as §6.5 here states.
7. **While playing.** When a driven field of the node changes, the node sends the new rate
   (applied at once), re-derives a held clip's time (§6.7) and sends the `UseBoneOpacity`
   flag, which the component blends over 0.2 s (`skeleton-anim.md` §6.6). A change of the
   node's intensity (for example a ScalarBlend weight or a Switch cross-fade, §7.1, §7.3)
   sets the clip's weight at once.
8. **Stop.** When the node's branch stops, or at the scheduled stop of step 4, the clip
   fades out over `RampOut` and is then removed; a `RampOut` below 0.0002 s removes it at
   once. The node is told that its clip has finished **when the fade-out begins**.
   `DetachOnRampOut` = 1 (or a `RampOut` of 0) makes the scheduled end of step 4 go to the
   entity's animation component instead of to the clip's layer; the visible difference was
   not traced (OPEN, §15).
9. **Events.** The node's ports fire as Timer ports (§6.4, §7.6, §9).

---

<a id="s7"></a>
## 7. Control-flow nodes used by clip selection

Structure and payloads are Confirmed (§4–§5); the run-time behaviour below is Reported:
inferred from the field and enum names and from how the data uses them. Tessera must
implement it this way until an owner observation says otherwise.
For the run-time semantics of these kinds, as the client runs them, see [`script-runtime.md`](script-runtime.md) (BIN; it supersedes this section where they differ, its §25).

<a id="s7-1"></a>
### 7.1 Switch (12)

Watches `SwitchScript_Property` on the entity. When the value matches a switch case (any
listed value under the case's operator: Equal; AllBitsSet / AnyBitSet for bit masks), that
case's children run; when no case matches, the Default port's children run (Reported).
`Persistent` = 1 keeps watching (re-evaluate on change); 0 evaluates once (Reported).
An absent property reads as its dictionary default, or as the zero value of its type
(false, 0, 0.0, DID 0, enum 0) when the dictionary has none (T28, BIN3, the script system's
property read); the case match then compares that value like any other (Reported).

**Changing branch (T24, BIN3; transition ports T35, BIN4).** When the property changes so
that another port (case or Default) matches:

1. Any change the switch is still delaying (step 2 or 3) is cancelled.
2. **Transition port.** If a transition port (28) matches the change from the old to the new
   value (matching rule OPEN, §4.3):
   1. if its `Delay` (§4.3) is positive, everything below is postponed by that many seconds;
      when the delay ends, step 2.2 runs and then, if the switch does not wait, it changes
      at once (step 4) to the port that matches the property at that moment, without that
      port's own `Delay`;
   2. its children start at once;
   3. if `Wait` is 0, or none of those children is still running right after starting, the
      switch goes on with step 3 as if no transition port had matched. The children are then
      part of neither branch of the switch (it neither fades nor stops them); whether they run
      to their end was not traced (OPEN; 7 such ports in this install);
   4. otherwise (`Wait` = 1 and a child still running) the switch **waits**: a fade still in
      progress is cut (the branch fading out stops at once); the current port's branch
      becomes the outgoing branch and the transition's children become the incoming branch;
      the two cross-fade linearly over the transition port's `RampIn` (*R*ᵢ), the outgoing
      branch at intensity × (*t*₁ − now) / *R*ᵢ and the incoming one at the complement, with
      *t*₁ = start + *R*ᵢ (an *R*ᵢ below 0.0002 puts the outgoing branch at intensity 0 at
      once); at *t*₁ the outgoing branch is stopped. The port matching the new value is
      remembered, and the switch has no current port meanwhile. When the **last running child
      of the transition branch finishes** (and no delay of step 3 is pending), the outgoing
      branch is stopped if it still runs, and the remembered port's children start at once,
      at full intensity and without a fade. A further property change during the wait goes
      through these steps again; the waiting transition branch is then stopped at once (the
      switch has no current port whose `RampOut` could fade it).
3. **Delay.** Otherwise, if the newly matching port's `Delay` is positive, the change is
   postponed by that many seconds; a further property change before then cancels it (step 1).
4. **Change.** At the change, the new port's children start at once. The old port's branch
   keeps running for the old port's `RampOut` seconds (*R*): from *t*₀ to *t*₁ = *t*₀ + *R*,
   the old branch runs at intensity × (*t*₁ − now) / *R* and the new branch at intensity × (1
   − that), both linear; at *t*₁ the old branch is stopped. With *R* below 0.0002, or with no
   current port (after a wait, step 2.4), the old branch stops at once and the new one runs
   at full intensity. The intensities reach the branches' clips as their weights at once
   (`skeleton-anim.md` §6.5), so this is a linear cross-fade of length *R*.
5. If the property changes again while a fade of step 4 is running, the branch still fading
   out is stopped at once, the branch that was coming in becomes the outgoing one, and the new
   fade (length = the `RampOut` of the port now being left) is placed so that the outgoing
   branch continues from its current intensity (worked example `skeleton-anim.md` §14.9).
   A transition port that matches during such a fade (step 2) instead cuts the fade without
   this continuity.

So in the locomotion templates (§8.4) walk ↔ run is a 0.2 s linear cross-fade (both cases
have `RampOut` 0.2), and the slide branch starts only after 0.5 s on an uncontrolled
surface (`Delay` 0.5). In this install 1,473 of the 1,480 transition ports wait (`Wait` = 1)
and 1,463 of those have `RampIn` 0, so their children replace the old branch at once and the
new case starts when they have finished. ForCurrentValue ports (22), the matching rule of
transition ports, `Autonomous`, `Synchronized`, the choice when several cases match, and the
case where no port at all matches the new value during a wait are OPEN (§15).

<a id="s7-2"></a>
### 7.2 When (63)

Same four fields as Switch. Each WhenTest port fires while its condition holds (property
*operator* constant, or *operator* the property named by `rhs_property`). Several tests can
hold at once; all their branches run. The Default port (6) is OPEN (presumably "no test
holds").

<a id="s7-3"></a>
### 7.3 ScalarBlend (57)

Blends its children by the float property `ScalarBlendScript_Property`. Each Default port
carries an anchor value; for the current property value *p*, the two anchors bracketing *p*
get weights by linear interpolation (the nearest anchor gets weight 1 outside the range),
the others weight 0. The locomotion templates use anchors −1, 0, 1 on `Slider_Propel`,
`Slider_Strafe` and `Slider_Turn`, and ±0.57 or −0.5 on the walking strafes and backs, and
nest blends: propel outside, strafe inside, turn innermost. The weights multiply into every
animation started below the port. Properties used (2,457 nodes): `Slider_Turn` (871),
`Slider_Strafe` (939), `Slider_Propel` (551), `ScriptSystem_Slider_Land` (94),
`Slider_ForwardSpeed` (2).

<a id="s7-4"></a>
### 7.4 Random (17)

Picks one Default port at random with probability proportional to its `u32` weight
(weights: 1 in 132,347 ports, 2, 3, 5, 10, 25, 50, 100, 150, 200 also occur; weight 0 in 23
ports means "never"). The client's rule (T27, BIN3):

1. A node with exactly one port always picks it.
2. Otherwise the total *T* is the sum of the port weights, leaving out the port picked last
   time when `RandomScript_NoRepeats` = 1 (25,890 of 26,778 nodes).
3. *T* = 0 picks nothing (and forgets the last pick). Otherwise an integer *k* is drawn
   uniformly from 0 … *T* − 1 and the ports are walked in a fixed order, adding the weights
   of the ports not left out; the first port whose running sum exceeds *k* is picked and
   remembered as the last pick. The order is that of the client's own port container, which
   is hashed and not shown to be the stored order; it does not change the distribution, so
   Tessera may walk the ports in stored order.

`Autonomous` is OPEN. The random number generator is the client's own and is not
reproduced; any uniform generator gives the same distribution.

<a id="s7-5"></a>
### 7.5 Collection (2) and OnStop (10)

Collection starts all children of all ports together (the port payload is only an ordinal).
OnStop runs its Default children when its parent branch stops.

**Start order (BIN6, T42; census OBS4).** "Together" means within the same start, one after
another, which matters wherever the later of two starts wins (equal-priority appearance
pieces, `appearance.md` §4.7; the slider set, §6.9):

1. The Collection gathers its Default ports (every Collection port in this install is a
   Default port with a u32 ordinal, SI17) and sorts them by **ascending ordinal**, compared
   as unsigned 32-bit numbers, whatever their stored order.
2. Ports with equal ordinals keep the order of the client's port container. That container
   is hashed and its order for equal ordinals was not traced (OPEN). The sort itself keeps
   that order: for 32 ports or fewer the client sorts by insertion, moving a port only past
   ports with a strictly greater ordinal, which is stable; above 32 ports it partitions,
   which is not stable (BIN7). No Collection of the install has more than 26 ports (SI17),
   so only the stable case occurs. **Tessera Decision:** equal ordinals keep their stored
   order.
3. Port by port, it starts the port's children in their stored order (§11.3). Each child
   completes its own immediate starts (its children that start at once, recursively) before
   the next child starts.
4. If the Collection is stopped while it is still starting (by one of its own children),
   it starts no further ports.

Census (OBS4): 46,656 Collection nodes, 17,934 with more than one port; their ordinals are
stored in **descending** order in 15,847, ascending in 112 and unordered in 1,975, so the
client usually starts the ports in the **reverse** of their stored order. 1,860 Collections
repeat an ordinal (394 use one ordinal for every port), and in 218 of them two ports with the
same ordinal both lead to Appearance nodes; of the 2,007 Collections with Appearance nodes
below two or more of their ports, 2,000 do not store their ports in ascending ordinal
order. No Collection has more than 26 ports, and one port of one Collection has more than
one child.

<a id="s7-6"></a>
### 7.6 Sequence (6) and Timer (65)

Sequence runs its ports one after another in ascending step number, each step when the
previous one finishes, `SequenceScript_LoopCount` times or forever. Timer runs for
`TimerScript_Duration` seconds (scaled by `AnimationScript_Speed` × `SpeedMultiplier`),
firing each port's children at its time; `ToolsAnimationOverride` names a clip for the
editor only and is not played.

<a id="s7-7"></a>
### 7.7 Run (5), WhenRunning (14), SetProperty (40), Goto (61) and Label (25)

Run starts (or, with `RunScript_Stop` = 1, stops) the script `RunScript_ID` on the target
entity, looked up through the entity's tables (§8.2); its parameter entries copy property
values into the run. WhenRunning fires Running (11) while any of its script keys is
running on the entity and NotRunning (10) otherwise. SetProperty sets an entity property
(locally unless `ServerOnly`); when its value is driven, the driven pair is the one whose
field PID equals `SetPropertyScript_PropertyName` (T32: 384 of 384 driven SetProperty nodes
have only such pairs). Goto jumps to the Label node with the same name in the same graph.
Running a script runs every definition the lookup of §8.2 gathers (T25). What a Run does when
the script is already running (`RunScript_QueuingType`, enum 0x2300009C: 1
`QueueInterruptable`, 2 `QueueRequeue`, 3 `DoNotQueue`, 4 `QueueNonInterruptable`; 3 is the
dictionary default and the value of nearly every Run node) is OPEN (§15).

<a id="s7-8"></a>
### 7.8 Scriptlet (19) and ScriptletParameter (22): templates

A Scriptlet node runs the root nodes of the 0x0C record `ScriptletScript_File`, in a scope
where:

1. its parameter **property set** is what the scriptlet-driven pairs of the nodes inside read
   (`appearance.md` §7.4.1 rules 1–4): a node's field takes a parameter only through such a
   pair of that node, and reads only the invoking Scriptlet node's own set (rule 3); no field
   is overridden by PID (rule 4). (Revised 2026-10-23: this point first said that the set
   overrides, for every node inside, the field bound to the same PID, for example
   `AnimationScript_LocomotionDisplacement` and `TimerScript_SpeedMultiplier`; that reading is
   superseded. Its effect on locomotion displacements is in §6.3.)
2. each of its Default ports (payload: a parameter **name**) binds that name to the port's
   children.

Inside the scriptlet, a ScriptletParameter node (only in scriptlets, 6,660 nodes) whose
`ScriptletParameterScript_Name` equals a bound name runs the bound children **in its place**
(with the caller's scope). If the name is not bound, its own Default port runs instead;
in the data that port holds a placeholder Scriptlet node with file 0 (3,960 cases), so an
unbound parameter does nothing. Of the 291–293 names each avatar binds, 280–283 equal one of the 292 parameter names of
0x0C000FFE exactly, 2 more (a walking and a running weapon variant) only when case is
ignored, and about 9 match none (they are unused). This is how one template serves every
species (§8.3).

**Matching rule (T26, BIN3).** Names are compared **ignoring ASCII case** (the earlier
Decision is now the client's rule). The search starts at the ScriptletParameter node and
walks up through its enclosing nodes to the **innermost** Scriptlet node (19) whose
`ScriptletScript_File` is the scriptlet record that contains this parameter node; only that
invocation's named ports are searched. If none matches, the parameter runs nothing (the
client does not run the parameter node's own Default port; in this install that port only
holds a placeholder, so the result is the same). Outer invocations are not searched. How
parameter **values** (point 1 above) combine across nested invocations was not traced
(OPEN, §16).

---

<a id="s8"></a>
## 8. How the original chooses idle, walk and run clips

<a id="s8-1"></a>
### 8.1 From an entity to its tables (Confirmed)

1. An entity's script tables are named by properties of its **0x47 entity descriptor**
   (`setup.md`; 11,170 of 40,643 descriptors have one):
   - `Entity_ClassScriptTable`, a struct with `ScriptTable_DBScriptTable` (a 0x07 DID; 11,096)
     and optionally `ScriptTable_StartupScriptNames` (an array of `ScriptTable_ScriptID`
     script keys; 1,610);
   - `Entity_AdditionalScriptTables` (148 descriptors), an array whose elements are bare
     `ScriptTable_DBScriptTable` DID entries (PID 0x2E), not structs: no struct element
     exists in any 0x47 or 0x79 set of the install (T33);
   - `Entity_InstanceScriptTable` (8 descriptors) and the `…_Override` properties (none
     set in descriptors).
2. A **game object** (0x70 with property set 0x79) adds its own tables through
   `Entity_AdditionalScriptTables` (same element form; 194,460 property sets name 515,833
   tables; 71 sets also carry `Entity_ClassScriptTable`). All 14 player-avatar objects (`skeleton-anim.md` §2.2)
   add exactly 0x07000000.
3. Every avatar class table lists `ScriptTable_StartupScriptNames` = (0x10000039,
   0x10000837): scripts to start when the entity is created. 0x10000039 (a sound start-up
   script) is defined in the shared table 0x0700042E of every avatar closure. 0x10000837
   (`movement`) is **defined in no table** of the install; it names a script the client
   implements in code, and tables only stop it (a door table stops it while the door
   animation plays). The data-driven animation starts from the `Startup` scripts instead
   (§8.3). Startup script names on all descriptors: 0x10000AD6 (1,432), 0x10000039 (109),
   0x10000837 (85), others 69.

<a id="s8-2"></a>
### 8.2 Include closure and lookup

The **closure** of a table is the table plus everything reachable through `includes`. The
avatar class tables (0x07000417–0x0700041D, 0x07001D84, 0x07001D86, 0x0700204E,
0x07002053, 0x0700247A, 0x07002F15, 0x07002F16) have closures of 148 or 149 tables holding
about 5,160 distinct script keys; in the Hobbit M closure 1,907 keys are defined more than
once, and for 1,851 of them the definitions differ. The species-specific tables are listed first among the class
table's includes and the shared ones are deeper.

**Lookup rule (T25, BIN3; replaces the 2026-10-05 Decision "first definition wins").**
Running a script key on an entity runs **all** of its definitions, gathered as follows:

1. If the entity has an instance-level definition of the key, it is taken first; if that
   definition's per-script property set (§2.4) has `Script_CombinationType` = 1
   (`Overriding`, enum 0x2300016D), nothing else is gathered.
2. Otherwise, for each of the entity's tables in order (class table, then additional
   tables, §8.1), the table and its includes are visited in **pre-order, depth first**, in
   stored include order. Every table that defines the key contributes its definition (its
   root nodes), unless the same definition was already gathered through another path. When
   a definition has `Script_CombinationType` = 1, the visit does not descend into **that
   table's** includes (its siblings are still visited). The default, and the value of every
   definition without the property, is 2 (`Additive`).
3. Definitions from tables of kind 2, kind 4 and kind 1 are gathered into separate lists
   (kind 4 tables hold no scripts, §2.2). The kind is the table's leading u32 (§2.1),
   tested against each list's kind **as a bit mask**: a table contributes to a list when its
   kind and the list's kind (1, 2 or 4) have a set bit in common (BIN7). A table whose kind
   does not match is still descended into for its includes; an include that is null or names
   the table itself is skipped. A root node already in a list is not added again. Avatar
   binding, species and fidget tables are of kind 1 (0x07000436, 0x07000466, 0x07002DAD,
   0x07001CAA, 0x0700077F …), class tables of kind 2, so both kinds run on the client.
   **Run order (BIN6, T41; resolves the OPEN of 2026-10-07):** the client starts the
   **kind-1** list first, then the **kind-2** list, then the kind-4 list. Within a list it
   starts the root nodes in list order: definitions in gathering order (step 2), and the
   roots of one definition in their stored order. Each root completes its own immediate
   starts before the next root starts (the order inside a node: §7.5, §11.3). The client has
   one further start path, which handles the kind-2 list first and then kind 1 and kind 4;
   it is selected by a per-run list of values whose purpose and source were not traced
   (OPEN, §15 item 1). Tessera uses the order above.
4. The result is cached per entity and key.

In this install 68 per-script property sets carry `Script_CombinationType`, all with value 1
(OBS2); every other definition is additive. So for the about 1,850 keys defined more than
once in an avatar closure, all definitions run, except below a table whose definition is
overriding. (The pre-order versus breadth-first question of T18 does not arise.)

**Start-up (BIN3).** The client runs an entity's start-up scripts through the same lookup,
so running key 5 (`Startup`) runs the `Startup` definitions of **every** table in the
closure, as the 2026-10-05 Decision assumed (27 tables of each of the 14 avatar closures have
one). Which code triggers key 5 for every entity was not traced (its effect, the avatars'
idle, is Confirmed by the data of §8.3). The client reports an error when one script is
listed as a start-up script twice. Clips started synchronously while a start-up script runs skip their ramp-in
(§6.10 step 1).

<a id="s8-3"></a>
### 8.3 The avatar locomotion binding (Confirmed, 14 of 14)

Every avatar's closure contains one table whose `Startup` script has a root Scriptlet node
that invokes the template **0x0C000FFE** with 291–294 named Default ports. Each port binds
one motion parameter of the template (idle, walk forward, run forward, walk and run back,
strafes, diagonal strafes, turns, jumps, falls, swimming, stealth, mounted and combat-stance
variants, character-creation idles …) to a small subtree of the species' clips. Default
clips — following the Default port of every Switch inside the bound subtree — and their
natural speeds (§6.3):

| Avatar entity | Class table | Binding table | Idle | Walk forward | Run forward | Walk back |
|---|---|---|---|---|---|---|
| Hobbit M 0x47000606 | 0x07000417 | 0x07000436 | 0x05000DF8 (15.0 s loop) | 0x05000E03, 1.85 m/s | 0x05000E61, 6.50 m/s | 0x05000E6C, −2.35 m/s |
| Hobbit F 0x47000607 | 0x07000418 | 0x0700045E | 0x05000DFD (20.0 s) | 0x050011E2, 1.85 | 0x050011C1, 6.57 | 0x050011CD, −2.35 |
| Human M 0x4700060A | 0x07000419 | 0x07000466 | 0x05001473 (15.0 s) | 0x05008D08, 2.02 | 0x05008D27, 6.50 | 0x0500145E, −2.57 |
| Human F 0x4700060B | 0x0700041A | 0x07000471 | 0x050015B4 (20.0 s) | 0x05008998, 2.00 | 0x0500899A, 8.85 | 0x0500159E, −2.57 |
| Elf M 0x4700060E | 0x0700041B | 0x07000483 | 0x050017FE (15.0 s) | 0x050093BC, 2.02 | 0x050093C0, 8.79 | 0x0500185D, −2.57 |
| Elf F 0x4700060F | 0x0700041C | 0x0700048B | 0x05001804 (20.0 s) | 0x050093A0, 1.82 | 0x05009397, 8.85 | 0x05001AF0, −2.57 |
| Dwarf M 0x47000611 | 0x0700041D | 0x07000494 | 0x05001D3C (15.0 s) | 0x05001D3B, 2.10 | 0x05001D18, 6.50 | 0x05001D24, −2.67 |
| Beorning M 0x47007004 | 0x07001D84 | 0x07000466 | as Human M | | | |
| Beorning F 0x47007005 | 0x07001D86 | 0x07000471 | as Human F | | | |
| HighElf M 0x4700806C | 0x0700204E | 0x07002048 | 0x050017FE | 0x050093BC, 2.02 | 0x050093DD, 8.79 | 0x0500185D, −2.57 |
| HighElf F 0x47008071 | 0x07002053 | 0x07002060 | 0x05001804 | 0x050093A0, 1.82 | 0x050093DB, 6.50 | 0x05001AF0, −2.57 |
| StoutAxe Dwarf 0x4700979A | 0x0700247A | 0x07002479 | 0x05007A13 (4.0 s) | 0x05007A0D, 2.00 (T46) | 0x05007A14, 6.00 (T46) | 0x05007A24 (none) |
| RiverHobbit M 0x4700CEA3 | 0x07002F15 | 0x07002F1C | 0x05008EE8 (5.1 s) | 0x05008EA9, 2.00 | 0x05008EA1, 6.50 | 0x0500145E, −2.57 |
| RiverHobbit F 0x4700CEA4 | 0x07002F16 | 0x07002F1A | 0x05008EE7 (5.1 s) | 0x05008EB6, 2.00 | 0x05008EAD, 6.50 | 0x0500159E, −2.57 (T46) |

Facts about this table:

- Every clip reachable from the bound idle, walk-forward, run-forward, walk-back, strafe
  and turn parameters of each avatar has a track count equal to that avatar's bone count
  (62 for hobbits and dwarves, 89 for the others): the binding is the species/body-type
  override, and nothing else is needed to pick the right skeleton's clips.
- The Beorning entities reuse the human binding tables; the race-table species share
  bindings exactly where they share setups (`skeleton-anim.md` §2.2).
- The binding table's `Startup` script has exactly two roots in all 14: the template
  invocation and a Switch. In 12 bindings the Switch is on `Combat_Violent`; its Default
  holds a Switch on `ScriptSystem_AvatarMood` (ten moods, each its own scriptlet) whose
  Default starts a looping **idle-emote** clip, on the layer its animation node names: 15
  (`idle_emote`) for the Hobbit, Dwarf and DwarfStoutAxe bodies (Hobbit M 0x05000DF7, 12 s),
  18 (`face_emote`) for the Human, Elf, Beorning, HighElf and River-hobbit bodies (Human M
  0x05001544, Elf M 0x050017FF); with `Combat_Violent` = 1 nothing is started there. In the
  two River-hobbit bindings the Switch is on `CharGen_Stage` instead.
- Besides the binding table, every avatar closure holds one **species table** whose
  `Startup` adds the body-shape deformer, the bone sliders and, for River-hobbit and
  DwarfStoutAxe, the layer-110 proportion clip (§6.8–§6.9). They are part of the idle look:
  without them a River-hobbit has the wrong height and face.
- Run-forward bindings are Switch trees on `Movement_SpeedMultiplier` (= 0.75 selects a
  slowed clip), `Entity_Mode` (combat stances), `Agent_Class` and `Combat_AnimationSet`;
  idle bindings switch on `Stealth_IsStealthed`. The 8.8 m/s values are the stated
  displacements of the female, Elf and High-elf run clips (§6.3 shows one that overstates
  its foot speed); whether the original moves those characters faster is OPEN (owner
  observation, §8.5).

<a id="s8-4"></a>
### 8.4 The template's decision tree (Confirmed structure)

0x0C000FFE (2,316 nodes) and the creature template **0x0C0000B9** (2,290 nodes; invoked by
186 Scriptlet nodes in the `Startup` scripts of 161 tables, with 1–190 named ports, median
42) have the same shape. In the order in which
the tree tests them, the properties that choose a motion parameter are:

| Step | Property | Values that matter (labels from the data) |
|---|---|---|
| 1 | `CharGen_Stage` (When) | `ClassSelect`, `CharacterSelect` → creation idles; `Undef` → the game tree |
| 2 | WhenRunning on a mounted-channelling script | running → mounted variants |
| 3 | `Physics_OnSlide` | 1 → sliding/water tree |
| 4 | `Entity_Element` (enum 0x230001B9) | `Earth_ControlledSurface` (5) → ground tree; `Water_*` (1–3) → swim tree; `Earth_NoSurface` (4) → fall; `Earth_UncontrolledSurface` (6) → slide |
| 5 | `Stealth_IsStealthed`, `AvatarVFX_Cosmetic_Locomotion` | stealth and cosmetic gaits |
| 6 | `Movement_SpeedMultiplier` (When ≤ 0.75) | slowed gait |
| 7 | `Agent_ClassificationFilter` | monster-player gait |
| 8 | `ScriptSystem_Slider_Land` (ScalarBlend 1 ↔ 0) | landing blend |
| 9 | `Movement_MovementStyle` (enum 0x2300017F: 1 `Walking`, 2 `Running`, 3 `Idle`) | `Walking` → walk branch (cross-fade 0.2 s); Default → run branch |
| 10 | `Slider_Propel` (ScalarBlend −1/0/+1, walking back at −0.5) | back / none / forward |
| 11 | `Slider_Strafe` (ScalarBlend −1/0/+1; ±0.57 walking) | left / none / right strafe |
| 12 | `Slider_Turn` (ScalarBlend −1/0/+1) | turn left / none / right, only at zero propel and strafe |
| 13 | `Entity_Mode` | `holding_container` and combat stances (`combat_1h`, `combat_2h`, `combat_bow` …) pick stance variants of idle, walk and run, then `Mount_CurrentMountType` and `Agent_Class` refine them |

At propel = strafe = turn = 0 the blend tree yields an empty Collection: the locomotion
layer plays nothing and the idle clips started by `Startup` show. The idle itself is a
parameter of the same template, reached through `Entity_Mode`, cosmetic and stealth
switches; its Default is the neutral idle of §8.3 (layer 10, looping).

The properties are set by the client's movement code (the native `movement` script of
§8.1): `Slider_Propel`, `Slider_Strafe` and `Slider_Turn` are floats in [−1, 1] from the
movement keys, `Movement_MovementStyle` is the walk/run toggle, `Entity_Element` the
surface state.

**Slider update rule (T30, BIN3).** The movement code never jumps a slider to its new value.
It sets a target and the property then moves towards it at a **constant rate of 1/*T* per
second** (so a change of size *d* takes *d* × *T* seconds: 0 → 1 in *T*, −1 → 1 in 2*T*);
the first value ever given to a property on an entity is taken at once. Observed *T*:

- 0.2 s for `Slider_Propel`, `Slider_Strafe`, `Slider_Fly` and `Slider_Turn` in one of the
  two movement-follower paths, which derive the sliders from the entity's displacement since
  the last update (propel = forward component, strafe = sideways component of the unit
  displacement; turn = ±1 while the heading changes);
- 0.5 s for `Slider_Propel` and `Slider_Strafe` (0.2 s for `Slider_Fly`) in the other
  follower path, which keeps `Slider_Turn` at its current value and sets the strafe target
  to 0 while the turn slider is beyond ±0.5 and the forward component is above 0.5;
- for the player's own input (BIN3, BIN4): propel and strafe come from the input direction
  (strafe *x*, propel *y*, each −1 … 1; when both |*x*| and |*y*| are at most 0.5, *y* is
  doubled), `Slider_Fly` goes to 0, and the turn slider follows the turn input of the frame
  (reset every frame). All of them use the same *T*: the `GenericPhysicsController_RampTime`
  of the player's controller configuration, which the client merges from the
  `PlayerPhysicsControl` object of the data by movement state (`skeleton-anim.md` §7.4, K28).
  Observed values: 0.13 s on the ground when running, 0.25 s when walking (and 0.01 falling,
  0.4 swimming, 0.5 sliding or on ice, 0.1 on an uncontrolled slope, 0.25 mounted or
  frozen). `Slider_ForwardSpeed`, `Slider_StrafeSpeed` and `Slider_FlySpeed` are set in the
  same step with the same *T*;
- when the player's movement is frozen while the client waits (`Movement_FrozenWhileWaitingForServer`
  becomes true), all seven sliders (`Slider_Strafe`, `Slider_Propel`, `Slider_Fly`,
  `Slider_Turn`, `Slider_ForwardSpeed`, `Slider_StrafeSpeed`, `Slider_FlySpeed`) go to 0 over
  **0.1 s** (BIN4);
- two further paths reset sliders: one sets propel, strafe, fly and turn to 0 over 0.2 s,
  another sets all seven to 0 at once (BIN4; when these run was not traced).

The movement code also sets `MoveKeys_Forward`, `MoveKeys_Back`, `MoveKeys_Left`,
`MoveKeys_Right` (booleans from the input direction), `MoveKeys_AnyMovement` and
`Movement_IsMoving` (moving faster than 0.02 per update in the follower paths),
`Entity_Element`, `Entity_FinalElement`, `Movement_MovementStyle`, `Slider_ForwardSpeed`,
`Slider_StrafeSpeed` and `Slider_FlySpeed` (BIN3; how each is computed beyond this list is
not specified here). Tessera's walk controller should move the sliders at the constant rate
above, with *T* read at run time from the merged controller configuration, instead of setting
them directly.

<a id="s8-5"></a>
### 8.5 Speed scaling (Confirmed data; no rate matching, BIN)

- Walking and running are **separate branches** selected by `Movement_MovementStyle`, with
  a 0.2 s cross-fade and `SynchronizeTiming` = 1 for phase matching; the data contains no
  blend between a walk clip and a run clip by speed.
- Every locomotion clip node has `AnimationScript_Speed` **driven** by
  `Movement_SpeedMultiplier` (14,278 animation nodes have a driven `Speed`; a few use
  `Combat_AnimationSpeedMultiplier`). So the playback rate is the entity's speed multiplier
  (1.0 normally) times the node's `SpeedMultiplier`.
- The clip's natural speed is |`LocomotionDisplacement`| / duration (§6.3).

- **The client never rescales the rate to the ground speed** (BIN3, `skeleton-anim.md`
  §7.4): the avatar's ground speed is a per-direction move speed × `Movement_SpeedMultiplier`
  from the movement code. For the player the per-direction move speeds come from the data,
  through the `PlayerPhysicsControl` object (`skeleton-anim.md` §7.4, K28; corrects the
  earlier "run-time values that the data files do not hold"). The feet slide by the
  difference.

> **Decision** (revised 2026-10-07): play locomotion clips at rate = driven `Speed` ×
> `SpeedMultiplier` (the client's rule). Move a **running** player with the client's rule and
> the merged configured speeds of `skeleton-anim.md` §7.4, read at run time, taking a full
> keyboard input as magnitude 1. Until the walk-mode input vector is known, move a
> **walking** character at the walk clip's natural speed (with the entity-scale factor of
> §6.3) × `Movement_SpeedMultiplier`, so the feet match. **OPEN** (`skeleton-anim.md` §7.4,
> owner observation O14.4): the input vector the keyboard gives per gait, and so the actual
> walking speed.

<a id="s8-6"></a>
### 8.6 Random idle variants (Confirmed path, revised 2026-10-07)

- **Avatars do play random idle fidgets** (T34; corrects the 2026-10-05 statement "avatars
  have no random idle", which missed paths through Run nodes). The idle and idle-emote clips
  of §8.3 are fixed per mood, but every avatar closure also contains the fidget driver
  below. Path in the Hobbit M closure (the other avatars use the same shared tables and their
  own species fidget table; OBS2):
  1. The `Startup` script of the shared table 0x07001CAA (kind 1, in every avatar closure)
     is a one-pass Sequence whose second step runs, among others, script 0x1000283D.
  2. 0x1000283D (also in 0x07001CAA) is a **looping** Sequence: step 2 a Timer of 20 s;
     step 4 a Switch on `WeenieType`. For the player-avatar values (31 and 63) a chain of
     persistent Switches stops the fidget scripts (Run with `RunScript_Stop` = 1 on
     0x100001BA and on the seven fidget keys) whenever any of
     `Script_PlayingInductionAnimation`, `Script_BlockRandomEmotes`,
     `Script_PlayingDancingAnimation`, `Script_IsEmoting`, `Movement_IsJumping`,
     `MoveKeys_AnyMovement`, `Combat_Violent` is true or `Entity_Element` is 1–4 (water,
     no surface); otherwise it runs 0x100001BA. For the NPC values (79, 65615, 131151) a
     similar chain (`NPC_PreventRandomEmotes`, `Patrol_Flying`,
     `MoveToController_PerformingMoveTo`, `MoveKeys_AnyMovement`, `Combat_Violent`,
     `NPC_PlayingStartingIdle`) runs 0x100009B7 instead.
  3. 0x100001BA (shared table 0x07000433) is a Timer of 180 s whose port at 180 s, while
     script 0x10000BB3 is not running (it is defined in no table of the closure), runs a
     Sequence: step 1 a Timer of 15 s; step 2, if `LifeState` is not 3 (`Incapacitated`) or
     5 (`Dead`), `Script_BlockRandomEmotes` is false and `Entity_Element` is 5 (on ground),
     a **looping** Sequence of step 2 a Random (`NoRepeats` 1) and step 5 a Timer of 9 s.
     The Random's ports: weight 100 → script 0x10000333, 10 → 0x100001C0, 10 → 0x1000041F,
     10 → 0x100003A8, 100 → 0x10000245, 100 → 0x100002BD, 200 → a Timer of 3 s (a pause).
  4. The six fidget scripts are defined in a per-species table (0x0700077F for Hobbit M,
     0x0700077A for Human M); each plays one-shot clips through scriptlets (layer 15 in the
     implementation's resolution, §16), with a different branch while mounted.

  **Predicted timing** (data path; Sequence and Timer semantics of §7.6 Reported, Random
  BIN3): no fidget during the first 20 + 180 + 15 = 215 s of standing still. From then on
  the looping Sequence alternates a pick (step 2) and a 9 s Timer (step 5), each step
  starting when the previous one has finished (§7.6, Reported). So consecutive picks are
  9 s plus the time the pick's step takes to finish apart: 3 s + 9 s = 12 s after a pause
  pick; after a fidget pick, 9 s plus the time until its Run step counts as finished, which
  is either at once or when the fidget script ends (not traced, §15 item 8). With
  `NoRepeats` = 1 the previous pick is left out of the draw (§7.4), so the chance of a
  fidget depends on the previous pick: 330 / 530 for the first pick; 330 / 330 (certain)
  after a pause; 230 / 430 after a weight-100 fidget; 320 / 520 after a weight-10 fidget.
  Any movement, jump, emote, combat or leaving the ground stops them, and the 180 s wait
  starts again when 0x1000283D next runs 0x100001BA. (The
  implementation's note of §16 quoted weights 10, 10, 5, 5, 1, 1, 1; the stored weights of
  0x100001BA in this install are the ones above.)
- **NPC-type weenies** using the same driver (step 2) run 0x100009B7 (0x07000433): on the
  ground and alive, after a random wait of 5 or 8 s, a looping Sequence picks (`NoRepeats` 0,
  all weights 1) among four fidget scripts, a 3 s pause and two ports whose node has
  `Probability` 0 (picked but never run), while `Slider_Propel` is 0, then waits a random
  7, 9 or 5 s.
- Creatures and NPCs with their own tables use Random nodes too: 572 entities reach looping
  idle clips and also Random-gated animations from `Startup` (106 more reach Random-gated
  animations only); the weights on those paths are 1 (8,392 port visits), 3 (348), 5
  (330), 2 (144), 10 (91), 4, 6, 20. Each pick runs one port's subtree, usually a scriptlet
  with a one-shot clip (a fidget) on a higher layer than the looping idle; when it ends the
  idle shows again. What re-triggers the Random node is the structure around it: in the two
  shared drivers above a looping Sequence with a Timer step; elsewhere it is not tabulated
  (OPEN, §15).
- NPC-like figures placed by the landblocks (`appearance.md` §7.2) carry
  `NPC_StartingIdleAnimation` (enum 0x23000110, 296 labels). Twelve SetProperty nodes in
  tables set `NPC_ActiveIdleAnimation` with its value driven by `NPC_StartingIdleAnimation`
  (6) or `NPC_SecondaryIdleAnimation` (6), and the creature templates (0x0C0000B9 and five
  others) contain Switch nodes on `NPC_ActiveIdleAnimation` with 150–276 cases. **OPEN**: the
  mapping of each idle key to its clip for a given rig has not been tabulated; the data path
  is complete, so an evaluator that follows §11.3 reaches the clip.

<a id="s8-7"></a>
### 8.7 Creatures and NPCs (Confirmed census, §10.4)

Creatures follow the same pattern as avatars: class table → closure → a binding table whose
`Startup` invokes 0x0C0000B9 with named ports and per-binding displacement values (for
example the creature binding 0x070001A5 gives swim strafes ±3.146 m per cycle along x and
diagonals 2.224 m along x and y).

---

<a id="s9"></a>
## 9. Animation events

- **Clips carry no events.** In the 5,602 clips with annotation tracks, no track holds a
  single timed annotation (`havok-data.md` §4.4, H9); track names only label bones. There
  is nothing to connect.
- **Events are script ports.** An animation node's Default port with time *t* (§6.4) is an
  event at clip time *t*; its children are the event's effects. In the avatar walk and run
  scriptlets the events are footstep scriptlets alternating between two records (0x0C000137
  and 0x0C000138) at the contact times, for example 0.07, 0.51, 0.97 and 1.43 s in the
  1.82 s walk 0x05008D08; mounted and holding variants wrap them in WhenRunning nodes.
- **Gameplay hooks** are StoryHook nodes (kind 28) under event ports: `StoryHookScript_Type`
  = 2 `Attack` (4,130 nodes) or 1 `Defend` (2,480) (enum 0x2300013D). They tell the game
  when a swing lands or a block happens: on the client they release the strikes of the
  performer's current story (§9.1). Tessera does the same with scripted strikes
  (`combat-visuals.md` §9.3); nothing is sent to a server.
- Event time is in clip seconds, before playback-rate scaling: at rate *r* the event fires
  *t*/*r* seconds of wall time after the cycle starts. A looping clip fires its events on
  every cycle. Events whose time exceeds the duration (329) never fire. (BIN3: the animation
  node schedules its ports with the Timer machinery, §6.4; was Reported and Decision.)

<a id="s9-1"></a>
### 9.1 StoryHook and Execute nodes at run time (BIN8, OBS5; 2026-10-10)

- **StoryHook (T44).** When a StoryHook node runs, it hands its `StoryHookScript_Type` and the
  run's `StorySystem_StoryID` (PID 0x48E) to the combat-presentation component of the entity
  the script runs on, and completes at once. The component counts the events per story and per
  type; the *n*-th event of a type releases the story's strikes of hook number *n* (an Attack
  event every strike, a Defend event only strikes of the defensive kind). A StoryHook in a
  script that does not belong to a story releases nothing. Rules and what a released strike
  shows: `combat-visuals.md` §4.2–§4.4.
- **Execute (T45).** Callbacks by node count over all tables and scriptlets (DID map
  0x28000007 labels): `GetUpcomingTargets` 625, `ExecuteScript_AssertMsg` 125, none 88,
  `ExecuteScript_DistantGroup` 64, `ExecuteScript_UpdateBallisticTargetReticle` 10,
  `GetUpcomingHitResult` 4, `CommonGeneratorScriptCallback` 2, `ExecuteScript_ArrayContains` 2,
  `ExecuteScript_DistantBattleDamageReport`, `ExecuteScript_TurnOffRunLock` and
  `SetupPlayerPhysics` 1 each. Result ports (type 5, 629): (`SCP_GetUpcomingTargets_Result`, 1)
  in 623, (`SCP_GetUpcomingTargets_Result`, 0) in 4, (`ExecuteScript_ArrayContains_Param_Result`,
  0) in 2. A `GetUpcomingTargets` node's call properties hold the array
  `SCP_GetUpcomingTargets_CombatResultType_Filter` (attack results, mapper 0x2300002A); its
  output array `SCP_Output_GetUpcomingTargets_TargetArray` is read by Property targets (§4.4) of
  the nodes below it. Their meaning and the Tessera rule: `combat-visuals.md` §4.6 and its
  rule 8.
- **Beam (kind 54).** Its Default port fires once, when the beam's head arrives
  (`particle-runtime.md` §11.1 step 7).

---

<a id="s10"></a>
## 10. Census (Confirmed, 2026-10-05)

<a id="s10-1"></a>
### 10.1 Records

| Record type | Records | Decoded to the last byte | Nodes | Animation nodes |
|---|---|---|---|---|
| 0x07 tables | 7,526 | 7,524 (99.97 %) | 457,902 | 382 |
| 0x0C scriptlets | 28,992 | 28,992 (100 %) | 174,945 | 26,835 |

LC's layouts alone decode 5,023 tables (66.7 %): it fails on every table with an animation
node, a MeshFX node, or a node kind it does not know, and on physics, execute, fire-missile
and sub-target details (§14).

<a id="s10-2"></a>
### 10.2 Node kinds

Counts per kind are in §5.1. The largest kinds over both record types: Scriptlet 165,783;
Sound 126,562; Collection 46,656; Particle 41,354; Run 38,872; Switch 31,007; Appearance
29,792; Animation 27,217; Random 26,778 (with 137,690 weighted ports); Timer 21,595. Ports:
489,076; targets including sub-targets: 14,515.

<a id="s10-3"></a>
### 10.3 Failure categories

| Category | Records | DIDs | Cause |
|---|---|---|---|
| CinematicCamera data | 2 tables | 0x070006E7, 0x07001BD9 | The base block decodes (driven pairs naming `CinematicCameraScript_*` fields); the data after it is about 60–80 bytes whose layout matches neither LC (no data) nor BIN's field list read straight. |

A decoder reports these as "unsupported node data (kind 7)" and skips the record. Nothing
animation-related is lost: both tables are camera sequences.

<a id="s10-4"></a>
### 10.4 Resolution statistics

"Startup animation" = an animation node reachable from the `Startup` script of any table in
the entity's closure, following Scriptlet invocations and parameter bindings (§11.3),
whatever the switch values.

| Population | Entities | Result |
|---|---|---|
| Playable species and body types (race table) | 14 | 14 resolve idle, walk forward, run forward, walk back, strafes and turns; 14 have matching track counts on every one of those clips (§8.3) |
| All 0x47 descriptors with a script-table property | 11,170 | idle and movement clips 790; idle only 920; movement only 307; other `Startup` animations 49; no `Startup` animation 9,027 (static props, effects, doors …); no class table 74; class table missing from the install 3 (e.g. 0x47001A71) |
| … of the 2,066 with `Startup` animations | 2,066 | every reachable clip pose-compatible with the entity's setup (`skeleton-anim.md` §3.3) in 1,602; some in 277; none in 187 (OPEN) |
| … with Random-gated `Startup` animations | 678 | 572 also have a looping idle |
| `PhysObj` of the 775 landblock weenie objects (26 distinct entities) | 26 | 2 idle and movement (the one monster type, 0x4700005B, and one other); 15 no `Startup` animation (housing-hook markers, the door); 9 no class table |
| `PhysObj` of the 23 distant-battle unit objects (22 distinct entities) | 22 | 18 idle and movement; 3 idle only; 1 no `Startup` animation |

---

<a id="s11"></a>
## 11. Algorithms in words

<a id="s11-1"></a>
### 11.1 Decoding a 0x07 record

1. Read `kind` (u32; accept 1, 2, 4; warn on others), the VLE include count and the include
   DIDs.
2. Read the script TSize; for each script read the key, the VLE root count and that many
   nodes (§11.2).
3. Read the props TSize; for each entry read the key and an embedded property set.
4. Read the DID-list TSize; for each entry read the key, an i32 count (reject negative or
   too-large counts) and the DIDs.
5. The payload must be exhausted; leftover bytes are an error.

Decoding needs the property registry (0x34000000, `property-system.md` §2): PVs and
property sets are typed by their PIDs, and an unknown PID makes the record undecodable.
Node data fields, by contrast, have fixed types (§5) and need no registry.

A 0x0C record is: VLE root count, nodes, u32 dependency count, DIDs, end.

<a id="s11-2"></a>
### 11.2 Decoding a node

1. Read the kind byte. Unknown kind → error.
2. If kind is 7: read the base block, then the data (unsupported: error). Otherwise read the
   data of the kind (§5, §6), then the base block; for kind 59 read one more bool8.
3. Read the u32 port count; for each port: read the port type; if 0, continue; if the pair
   (kind, port type) has a payload (§4.3), read it, then a u32 echo that must equal the port
   type; read the u8 child count and that many child nodes.
4. Read the u32 target count; for each target read it as in §4.4 (value only for the listed
   top-level forms, and for sub-targets whose port type is 6), recursing into sub-targets.

<a id="s11-3"></a>
### 11.3 Choosing the clips for an entity (Tessera evaluator)

1. Collect tables: the class table of the entity's 0x47, its additional tables, and the
   additional tables of its game object (avatars: 0x07000000). Build each table's closure
   (§8.2) and the lookup order.
2. Create the entity's property state from its descriptor, game object and instance
   property sets, and the movement inputs of §8.4 (`Entity_Element` = 5, `Movement_MovementStyle`
   = 2, sliders 0, `Movement_SpeedMultiplier` = 1.0, `Combat_Violent` = 0, `CharGen_Stage` = 0
   unless set).
3. Run the `Startup` script of every closure table once (Decision, §8.2), then each script
   named by `ScriptTable_StartupScriptNames` that the lookup finds (§8.1; skip names
   defined in no table, such as 0x10000837). Start the nodes in the client's order (BIN6,
   T41, T42): for one script run, the roots of the kind-1 definitions, then those of the
   kind-2 definitions (§8.2 step 3); a node's children port by port, children of one port in
   stored order, a Collection's ports by ascending ordinal (§7.5), each child with its
   immediate starts before the next. Where a later start wins (the slider set of §6.9,
   equal-priority appearance pieces), "later" means later in this order. Running a node:
   - Switch / When / ScalarBlend / Random / Sequence / Collection / Timer / OnStop /
     WhenRunning / Goto as in §7; skip a node with its subtree when a draw against
     `Probability` fails;
   - Scriptlet: push the parameter scope and the port-name bindings (§7.8) and run the
     scriptlet's roots; ScriptletParameter: run the bound subtree or its own Default port;
   - Animation: start the clip on its layer with the node's fields, after applying driven
     properties (base block field 1) and scope overrides, by the rules of §6.10; register
     its event ports (§9); a node with `ManualUpdate` = 1 is held at the time of §6.7 and
     re-derived whenever its driving property changes;
   - MeshDeformer (18) and SliderMeshDeformer (81): register the bind-space deformation of
     §6.9 (`appearance.md` §6.4) with the driven values; recompute it when they change;
   - Run: gather the key's definitions (§8.2) and run all of them; SetProperty: set the
     property and re-evaluate the switches that watch it;
   - a property the entity does not have reads as its dictionary default, or as the zero
     value of its type (T28);
   - every other kind: ignore in M4 (skip the subtree); sound, VFX and camera follow in M6.
4. Each frame, feed the movement inputs (moving the sliders at the constant rate of §8.4);
   a changed property re-evaluates the persistent Switch/When/ScalarBlend nodes that watch
   it, changing branches by the delay and cross-fade rule of §7.1.
5. Sample all running clips (`skeleton-anim.md` §10) and combine them by the layer rule of
   §6.2 (`skeleton-anim.md` §6.6). Held clips are sampled at their held time, not advanced.

---

<a id="s12"></a>
## 12. Invariants (Confirmed unless marked)

| # | Invariant |
|---|---|
| SI1 | A 0x07 record starts with u32 kind ∈ {1, 2, 4}; a kind-4 table has no scripts and no property sets. |
| SI2 | Every include names an existing 0x07 record; the include graph is acyclic. |
| SI3 | Script keys are in TSize bucket order; per-script property-set keys are a subset of the script keys. |
| SI4 | Every 0x07 and 0x0C record ends exactly after its last section (36,516 of 36,518; the 2 others are §10.3). |
| SI5 | A port echo equals its port type; a target marker is 1; a switch-case reserved field is 0. |
| SI6 | An animation node's data is exactly 59 bytes; its `Asset` has prefix 0x05 or is 0 (the null DID, 2 nodes: table 0x070001E8 script 0x100006A4 node 2119, table 0x0700062D script 0x100003A0 node 66; a null asset starts nothing); it has no target; its ports are of type 6 only. |
| SI7 | ScriptletParameter nodes occur only in scriptlets. |
| SI8 | Node IDs are non-zero and unique within a scriptlet (unique within a table in 7,515 of 7,524 tables, so not a rule there). |
| SI9 | Every Scriptlet node's file is 0 or an existing 0x0C record. |
| SI10 | For each of the 14 avatar entities, every clip bound to the template's idle, walk, run, back, strafe and turn parameters has as many tracks as the avatar's skeleton has bones. |
| SI11 | Every animation node on layer 110 has `ManualUpdate` = 1 (8 of 8); every node whose `ManualTime` is driven has `ManualUpdate` = 1 (7 of 7: 4 by `AC_Avatar_Height`, 2 by `AC_BoneScale_Test`, 1 by `Character_Demo_Pose_Override_Time`). |
| SI12 | Each avatar closure contains exactly one table with a MeshDeformer node in its `Startup` script (14 of 14), and for the Human, Elf, HighElf and River-hobbit avatars that table also holds the closure's only SliderMeshDeformer node (8 of 8; the Beorning tables have none). |
| SI13 | Every driven pair of a SetProperty node names the node's own `SetPropertyScript_PropertyName` as its field (384 of 384 nodes with driven pairs); every driven pair of a MeshDeformer node names `MeshDeformerScriptNode_AnimationTime` (49 of 49) and of a SliderMeshDeformer node `BoneSlider_ValueArray_String` (8 of 8). |
| SI14 | Every element of `Entity_AdditionalScriptTables` is an entry of PID 0x2E (`ScriptTable_DBScriptTable`) holding a DID. |
| SI15 | `Script_CombinationType` occurs in 68 per-script property sets, always with value 1 (`Overriding`). |
| SI16 | In every Switch transition port (12, 28) the two `Delay` fields are equal (1,480 of 1,480), and every transition port has at least one child (OBS3). |
| SI17 | Every port of every Collection node is a Default port (type 6) with a u32 ordinal (72,798 of 72,798 ports in 46,656 Collection nodes), and no Collection has more than 26 ports (OBS4). |

Fuzzing (§0): one target decodes arbitrary bytes as a 0x07 record, one as a 0x0C record; both
must return an error, never panic, and respect the limits of §4.5.

---

<a id="s13"></a>
## 13. Worked synthetic example (for tests)

All values are invented. A kind-2 table with one script, key 5, whose single root is an
animation node playing clip 0x05001234 in a loop on layer 10 with one event at 0.5 s whose
child is an empty Collection; one empty property set and one DID list for key 5. 165 bytes:

```
02 00 00 00 00 00 01 05 00 00 00 01 24 34 12 00
05 01 01 00 00 00 cd cc 4c 3e cd cc 4c 3e 0a 00
00 00 01 00 00 00 00 00 00 00 00 00 00 00 80 3f
00 00 00 80 3f 00 00 80 3f 00 00 00 00 00 00 00
40 00 00 00 00 00 01 00 01 00 01 00 00 01 00 00
80 3f 00 00 00 00 00 00 80 3f 0c 00 00 00 01 00
00 00 06 00 00 00 3f 06 00 00 00 01 02 01 00 01
00 00 01 00 00 80 3f 00 00 00 00 00 00 80 3f 0d
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
01 05 00 00 00 00 00 00 01 05 00 00 00 01 00 00
00 34 12 00 05
```

Reading it:

| Bytes | Meaning |
|---|---|
| `02 00 00 00` | kind 2 |
| `00` | no includes |
| `00 01` | script TSize: hint 0, 1 script |
| `05 00 00 00` `01` | key 5 (`Startup`), 1 root |
| `24` | node kind 36 (Animation) |
| `34 12 00 05` | `Asset` 0x05001234 |
| `01` `01 00 00 00` | `LoopForever` 1, `LoopCount` 1 |
| `cd cc 4c 3e` ×2 | `RampIn` 0.2, `RampOut` 0.2 |
| `0a 00 00 00` | `Layer` 10 (idle) |
| `01` `00` `00 00 00 00` | `UseBoneOpacity` 1, `ManualUpdate` 0, `ManualTime` 0.0 |
| `00 00 00 00` `00 00 80 3f` | `StartTime` 0.0, `EndTime` 1.0 |
| `00` | `RandomizeStartTime` 0 |
| `00 00 80 3f` ×2 | `Speed` 1.0, `SpeedMultiplier` 1.0 |
| `00 00 00 00` `00 00 00 40` `00 00 00 00` | `LocomotionDisplacement` (0, 2.0, 0): natural speed = 2.0 m / clip duration |
| `00 01 00` | `DetachOnRampOut` 0, `SynchronizeTiming` 1, `RequiresUpdatingEvenWhenCulled` 0 |
| `01 00` `01 00` `00` | base: no driven, no scriptlet-driven, no scriptlet params |
| `01` `00 00 80 3f` `00 00 00 00` `00 00 80 3f` | `UseParentIntensity` 1, intensity 1.0, variance 0.0, probability 1.0 |
| `0c 00 00 00` | node ID 12 |
| `01 00 00 00` | 1 port |
| `06` `00 00 00 3f` `06 00 00 00` | port type 6, event time 0.5 s, echo 6 |
| `01` | 1 child |
| `02` + base (`01 00 01 00 00 01 …` ID 13) + `00 00 00 00` + `00 00 00 00` | Collection node, no ports, no targets |
| `00 00 00 00` | animation node: 0 targets |
| `00 01` `05 00 00 00` `00 00` | props: 1 entry, key 5, empty property set |
| `00 01` `05 00 00 00` `01 00 00 00` `34 12 00 05` | DID lists: 1 entry, key 5, 1 DID 0x05001234 |

The same animation node without ports as the only root of a scriptlet, with the clip as
its dependency (99 bytes): `01`, the kind byte `24`, the 59 data bytes and the 22 base
bytes above (ID 12),
`00 00 00 00` (no ports), `00 00 00 00` (no targets), `01 00 00 00` `34 12 00 05`.

Tests:

- decoding both buffers consumes every byte; the animation fields equal the values above;
- an evaluator that runs key 5 starts clip 0x05001234 on layer 10, looping, and fires the
  event once per cycle at 0.5 s / rate (§9);
- changing byte 0x67 (the first byte of the port echo, `06` → `07`) makes the table
  malformed (SI5). (Corrected 2026-10-07: the port count is at 0x5E, the port type at 0x62,
  the payload at 0x63–0x66 and the echo at 0x67; the earlier 0x77 lies in the Collection
  child's base block.)
- a template test: a scriptlet whose root is a ScriptletParameter node named N with a
  placeholder Default child, invoked by a Scriptlet node whose Default port is named N with
  an animation child, starts that animation; invoked with no port named N, starts nothing
  (§7.8).

<a id="s13-2"></a>
### 13.2 Held proportion clip (§6.7, §6.8)

Synthetic, invented values. A two-bone skeleton: bone 0 the root at the origin, bone 1 its
child at local translation (0, 0, −0.5) (the "foot"). An idle clip on layer 10 holds the
reference pose, overall weight 1, no weights. A held node on layer 110: `ManualUpdate` 1,
`ManualTime` driven by property P, `LoopForever` 1, `UseBoneOpacity` 1, a clip of 2.0 s with
track weights (0.01, 0.01) whose bone 0 translation z is −10 + 10 × (time / 2.0) and whose
bone 1 translation z is −0.5 + 10 − 10 × (time / 2.0) (pelvis drop and leg shortening
authored to cancel). Layer weights (§6.2, `skeleton-anim.md` §6.6): 2/255 for layer 110,
252/255 for the idle.

| P | Clip time | Bone 0 z | Bone 1 local z | Bone 1 model z |
|---|---|---|---|---|
| 0 (the entity default) | 0 s | (2 × −10) / 254 = −0.07874 | (2 × 9.5 + 252 × −0.5) / 254 = −0.42126 | −0.5 (on the ground) |
| 0.5 | 1.0 s | −0.03937 | −0.46063 | −0.5 |
| 0.95 | 1.9 s | −0.00394 | −0.49606 | −0.5 |
| 1.0 | 0 s (wrap, §6.7) | −0.07874 | −0.42126 | −0.5 |

Tests: the pose does not change with elapsed time; changing P changes it at once; the same
clip *played* over time (rate 1, looping) moves bone 0 between −0.07874 and 0 every 2 s,
which is the wrong behaviour (the figure grows and shrinks).

---

<a id="s14"></a>
## 14. Corrections to earlier notes and specs

| Earlier statement | Where | Correction |
|---|---|---|
| Every record begins with its own DID | `records.md` P10, §1.6 | 0x07 begins with a u32 table kind (1, 2, 4); 0x0C has no prefix at all (like 0x05, `havok-data.md` §2.3). |
| 0x0C: "unknown, possibly entity/prop data" | `records.md` §2.1 | Scriptlets: script-node graphs plus a dependency list (§3). |
| 0x07: "u32 type ; VLE nIncludes …" | DS §10 | Confirmed outline; the first u32 is the table kind, and the tail sections are per-script property sets and DID lists. |
| 28 node types, the highest 57 | DS §10 | 66 registered codes up to 81 (§5.1); codes 16, 18, 20–23, 28–32, 34, 35, 37–39, 42–47, 49, 51, 53, 56, 58–65, 77, 78, 80 and 81 were missing. |
| Animation nodes are read like other LC nodes | LC | LC has no animation layout; the layout is §6.1. |
| MeshFX layout | LC | One more bool8 (`UseParentScale`) before `UseParticleScale` (§5.2). |
| Sub-targets inherit the parent node's target-value rule | LC | Sub-targets carry a value only when their port type is 6 (§4.4). |
| Execute, HeadingCheck, ScalarBlend, WaveformBlend carry no data | LC | They do (§5.1, §5.2). |
| "Clips are chosen by script and animation tables" | `skeleton-anim.md` §3.4 | There are no separate animation tables: clips are chosen by script graphs, mostly through scriptlet templates (§8). |
| "Animation events must come from script tables" | `havok-data.md` §4.4 | Confirmed: animation-node event ports (§9). |
| "Look a key up in pre-order; the first definition wins" (Decision) | this spec §8.2 (2026-10-05) | All definitions in the closure run (additive), except below a table whose definition is `Overriding` (§8.2, BIN3). |
| Switch case floats `transition_time` and `second_time` (meaning partly OPEN) | this spec §4.3 (2026-10-05) | `RampOut` and `Delay` of the case (§4.3, §7.1, BIN3). |
| "Avatars have no random idle" | this spec §8.6 (2026-10-05) | Avatars fidget through Run nodes after about 215 s of standing still (§8.6). |
| `Entity_AdditionalScriptTables` is "an array of such structs" | this spec §8.1 (2026-10-05) | An array of `ScriptTable_DBScriptTable` DID entries (§8.1, OBS2). |
| Byte 0x77 of the §13 example is the port echo | this spec §13 (2026-10-05) | It is byte 0x67 (§13). |

---

<a id="s15"></a>
## 15. Open questions

Numbers are kept from the 2026-10-05 list; resolved items say so.

1. *Resolved 2026-10-07 (BIN3, T25):* lookup across includes gathers every definition
   (additive, `Overriding` prunes, §8.2), and every closure table's `Startup` runs.
   *Resolved 2026-10-08 (BIN6, T41):* the client runs the kind-1 definitions first, then the
   kind-2 ones, each in gathering order (§8.2 step 3). Still open: when the second start
   path (kind 2 first) is taken, and the order among Collection ports with equal ordinals
   (§7.5).
2. **CinematicCamera data** layout (§10.3); ListenerEvent and CustomLighting custom fields
   (no instances).
3. Meaning of table kinds 1 and 2 beyond their start order (both run on the client, kind 1
   first, §8.2); of the switch-case int
   list (`Names`) and reserved value; the matching rule of transition ports (`ValueFrom`,
   `ValueTo`, `FromIsDefault`, `ToIsDefault`), whether the children of a non-waiting
   transition port run to their end, and a wait with no port matching the new value (§7.1);
   of ForCurrentValue ports; of
   `Autonomous` and `Synchronized`; which case runs when several cases match (the
   implementation runs the first in stored order, §16); of the Particle extra byte; of the
   FireMissile custom elements; of the base block's scriptlet-driven pairs and
   scriptlet-parameter list. *Resolved 2026-10-07:* the switch-case floats are `RampOut`
   and `Delay` (T23); the transition port's two `Delay` fields (one value), `Wait`, `RampIn`
   and `RampOut` (no effect) (T35, §4.3, §7.1).
4. *Resolved 2026-10-05 (BIN2):* layer combination (§6.2, `skeleton-anim.md` §6.6) and
   `UseBoneOpacity` (§6.5). *Resolved 2026-10-07 (BIN3):* the duration of a
   `UseBoneOpacity` change (0.2 s, `skeleton-anim.md` §6.6).
5. `ManualTime` is resolved (a fraction with a wrap rule, §6.7). *Resolved 2026-10-07
   (BIN3):* `StartTime`, `EndTime`, `RandomizeStartTime` and `SynchronizeTiming` of a played
   clip (§6.10) and events beyond the clip duration (never fire, §6.4). *Resolved
   2026-10-07 (BIN5, T37):* `StartTime` and `EndTime` do not crop a **held** clip; its played
   length is the duration minus the end crop (§6.7 step 2). Still open: the visible effect of
   `DetachOnRampOut` (§6.10 step 8).
6. *Resolved 2026-10-07 (BIN3):* there is no speed matching (§8.5). *Partly resolved
   2026-10-07 (BIN4):* the player's configured per-direction speeds come from the data
   (`skeleton-anim.md` §7.4). Still open (owner observation O14.4): the keyboard's input
   vector per gait and so the walking ground speed; and the avatar's default run clip where
   the bound Switch trees offer several (§8.3).
7. *Partly resolved 2026-10-07 (BIN3, T30):* sliders move at a constant rate of 1/*T* per
   second (§8.4). *Resolved 2026-10-07 (BIN4):* *T* for the player's own input is the merged
   `GenericPhysicsController_RampTime` (0.13 s running, 0.25 s walking here). Still open:
   the slider step of the `Mounted` controller type, when the 0.2 s and immediate slider
   resets run, and how the movement properties other than the sliders are computed.
8. *Partly resolved 2026-10-07:* the avatar and NPC fidget drivers re-trigger through
   looping Sequences with Timer steps (§8.6). Still open: what a Run does when its script is
   already running (`RunScript_QueuingType` 3 `DoNotQueue`, §7.7), when a Run step counts
   as finished for its Sequence (at once, or when the script it runs ends; this sets the
   interval between fidget picks, §8.6), when a Sequence step whose subtree holds persistent
   Switches counts as finished (§7.6), what re-triggers
   Random nodes in creature-specific tables, and a tabulation of `NPC_StartingIdleAnimation`
   keys to clips per rig.
9. *Resolved 2026-10-07 (BIN3, T26):* scriptlet-parameter names match ignoring ASCII case,
   in the innermost matching invocation (§7.8).
10. The 187 entities whose reachable clips have no pose-compatibility record (§10.4).

<a id="s15-1"></a>
### 15.1 Owner observation O16: status after BIN3 and BIN4 (2026-10-07)

| Part | Status | Answer or what is still needed |
|---|---|---|
| Which idle, walk and run clips play per species and sex | **Needs the owner** for the visual comparison | The data path is complete. §8.3 lists the clips; they come from `Startup` definitions, which the evaluator already runs in every closure table, so the lookup rule of §8.2 does not change them (keys reached through Run nodes may now gather several definitions). Only a side-by-side look can say "same" or "different". |
| Idle variants and fidgets | **Partly** | Answered by data and BIN: avatars do fidget (§8.6), with a predicted first pick after about 215 s (3.6 min) of standing still (a fidget with probability 330 / 530, otherwise a 3 s pause), then further picks as long as the character stays idle, on the ground, out of combat and not emoting: 12 s after a pause, and after a fidget 9 s once its Run step counts as finished (at once or at the fidget's end, not traced); after a pause the next pick is always a fidget (`NoRepeats`), after a fidget the chance of another fidget is 230 / 430 (weight-100 fidget) or 320 / 520 (weight-10 fidget). The step timing rests on the Reported Sequence semantics (§7.6). The owner is asked to confirm the timing: stand still for about **5 minutes**, not 1 (the checklist's "about a minute" is too short to see one), and note how often fidgets come. Still open: whether a re-run of the running driver restarts the 180 s wait (§15 item 8). |
| Mood idle-emotes | **Answered** by data | One looping idle-emote per mood (`ScriptSystem_AvatarMood`), none in combat (§8.3). |

---

<a id="s16"></a>
## 16. Implementation feedback (clean room, 2026-10-05)

`tessera-formats` (`script_table` module) was implemented from this spec alone. On the same
install its real-data test (`crates/formats/tests/script_table_real_data.rs`) reproduces §10:
7,524 of 7,526 tables and 28,992 of 28,992 scriptlets decode to the last byte, the 2 failures
are the §10.3 CinematicCamera category, and the node, animation-node, port and per-kind
counts equal §5.1 and §10.1–§10.2 exactly. Following §11.3, the resolver reproduces the
idle, walk and run clips of all 14 avatars in the §8.3 table, and the §10.4 classification of
the 26 landblock weenie entities (2 idle and movement, 15 no `Startup` animation, 9 no class
table). Points for the spec owner:

> **OPEN:** §13 says that changing **byte 0x77** (the first byte of the port echo) makes
> the table malformed. In the 165-byte example the animation node's port count starts at
> 0x5E, its port type is at 0x62, the payload at 0x63 and the echo at **0x67**; byte 0x77
> lies in the Collection child's base block (its `IntensityMultiplierVariance`), so changing
> it leaves the table decodable. The implementation's test uses 0x67. Please correct §13.

> **Answered** (2026-10-07): corrected; §13 now says 0x67 (checked against the 165 bytes).

> **OPEN:** SI6 says an animation node's `Asset` has prefix 0x05. Two animation nodes name
> the null DID (table 0x070001E8, script 0x100006A4, node ID 2119; table 0x0700062D, script
> 0x100003A0, node ID 66); these are probably the "2 name missing clips" of §6.1. The
> implementation accepts a null asset (not reported as unusual) and starts nothing for it.
> Please state SI6 as "0 or prefix 0x05".

> **Answered** (2026-10-07): SI6 now reads "prefix 0x05 or 0" and names both nodes; a null
> asset starts nothing.

> **OPEN** (§11.3, unset properties): the spec does not say what a Switch or When test
> reads for a property the entity's sets do not contain. The creature template 0x0C0000B9
> selects its whole game tree with a Switch on `CosmeticEntity_SourceWrapper` whose case is
> the property's dictionary default (DID 0); with "no value, take the Default port" no
> creature starts an idle clip in the idle state. **Decision** (implementation): an unset
> property reads its dictionary default; without a default the Switch takes its Default port
> and a When test fails (an option reads it as 0 instead). With this rule the creature of
> §10.4 (0x4700005B) starts an idle clip, and the avatars keep the §8.3 clips. Please
> confirm the client's rule.

> **Answered** (2026-10-07, BIN3, T28): the client's property read gives the dictionary
> default, and **without a default the zero value of the property's type** (false, 0, 0.0,
> DID 0, enum 0); the Switch and When then compare that value like any other (the compare
> itself is Reported). Replace "takes its Default port / the test fails" by "compares the
> zero value": that is the option the implementation already offers.

> **OPEN** (§8.3, §8.6): with the rule above and §11.3's "Run: look the key up and run that
> script", every avatar also starts **random idle fidgets**: the `Startup` script of table
> 0x07001CAA (in every avatar closure) runs a Sequence of Run nodes; script 0x1000283D runs
> 0x100001BA, which picks with weights (10, 10, 5, 5, 1, 1, 1; total 53) among seven scripts
> defined in a per-species table (0x0700077F for Hobbit M, 0x0700077A for Human M) that start
> one-shot clips on layer 15. A further layer-30 clip is started through scripts 0x10000827
> → 0x100008AF (table 0x07000435 for Hobbit M, 0x07000467 for Human M). §8.6 says avatars
> have no random idle; either the spec's analysis did not follow Run nodes or a gate on
> those paths (a property the client sets) keeps them from running. Which is it?

> **Answered** (2026-10-07): the analysis of 2026-10-05 did not follow Run nodes; avatars do
> fidget. §8.6 now gives the path (0x07001CAA → 0x1000283D → 0x100001BA → per-species
> scripts), its gates (movement, jumping, emotes, combat, water or air, blocked random
> emotes) and its timing (nothing for the first 215 s of standing still). The stored Random
> weights of 0x100001BA are 100, 10, 10, 10, 100, 100 and 200 (a 3 s pause), not 10, 10, 5,
> 5, 1, 1, 1; please re-check which node the resolver read.

> **Answered** (2026-10-05, spec writer; was: layers of the idle-emote and the layer-110
> clip per body, §8.3). The layer is always the animation node's `Layer` field; §8.3 now
> says 15 for Hobbit, Dwarf and DwarfStoutAxe and 18 for the others, as the implementation
> found. The layer-110 clips that the DwarfStoutAxe and River-hobbit bodies reach through
> `Reproportion_Idle` are **held** clips: never play them over time; hold them at the
> fraction §6.7 derives from `AC_Avatar_Height` (River-hobbit) or `AC_BoneScale_Test`
> (DwarfStoutAxe) and combine them by §6.2 (§6.8 has the nodes, clips and expected effect).
> Playing 0x05008E76 over time is what makes the River-hobbit grow and shrink in a loop.

> **OPEN** (§7.8, nested templates): a ScriptletParameter node inside a scriptlet that was
> itself invoked from inside the template could look for its name in the innermost
> invocation only or in every enclosing one. **Decision** (implementation): innermost first,
> then outward. Likewise for parameter **values** (§7.8 point 1): the innermost invocation
> that sets a PID wins, and a driven property (§4.2) wins over any parameter value. No
> avatar or creature result of §10.4 depends on either choice.

> **Answered in part** (2026-10-07, BIN3, T26): for **names**, only the innermost enclosing
> invocation of the scriptlet that holds the parameter node is searched; there is no outward
> search (drop "then outward"); an unbound name runs nothing. For parameter **values** and
> their precedence against driven properties the client was not traced; the
> implementation's choice stays a Tessera Decision (OPEN).

> **OPEN** (§7.1): when several switch cases match (bit-mask operators can do that), the
> implementation runs only the **first** matching case in stored order.

> Still OPEN (2026-10-07): not traced; keep the implementation's choice (listed in §15
> item 3).

> **OPEN** (§4.5): a port needs only **one** byte when its type is 0 ("no port", §4.3), so
> bounding port counts with 2 bytes per port could reject a valid record made of empty
> ports. The implementation bounds with 1 byte per port.

> **Answered** (2026-10-07): accepted; §4.5 now says 1 byte per port.

> **OPEN** (§7.6, static evaluation): for clip selection the implementation treats every
> step of a Sequence and every port of a Timer as a candidate (all their children "start"),
> since the order in time does not change which clips are reachable. A player that runs the
> graph in time must follow §7.6 instead.

> **Answered** (2026-10-07): accepted as a technique of the static resolver; it states no
> fact about the client. The live evaluator follows §7.6 and §6.10.

> **OPEN** (§8.1, §11.3 step 2): the spec does not say how the property sets of an entity's
> parent 0x47 descriptors (`setup.md` §3.1), its game object and its instance combine.
> **Decision** (implementation): parents first, then the entity, then the game object, then
> the instance; a later set's scalar values and its `Entity_ClassScriptTable` replace earlier
> ones (so the 71 object sets that carry a class table override their entity's), and
> `Entity_AdditionalScriptTables` and start-up names accumulate. None of the 14 avatar
> entities has a parent descriptor, so the §8.3 results do not depend on this choice.

> Still OPEN (2026-10-07): not traced; the implementation's order stays a Tessera Decision.
> Note that a class table supplied by a later set does not remove the gathered definitions of
> the earlier one if both are in the entity's table list (§8.2 gathers from every table).

> **OPEN** (§8.1 item 1, review 2026-10-05): §8.1 calls `Entity_AdditionalScriptTables` "an
> array of such structs", but a review probe of this install found every element to be a **bare
> `ScriptTable_DBScriptTable` DID** (no struct, no `ScriptTable_StartupScriptNames`): 104
> elements in 0x47 sets and 515,833 in 0x79 sets, no struct element at all. An evaluator that
> reads only structs drops every additional table, including the avatars' 0x07000000 of
> item 2. The implementation accepts both forms (a DID element, or a struct read like the
> class table). With 0x07000000 added, the avatar closures have 149–150 tables and the §8.3
> idle, walk and run clips are unchanged (the real-data test now asserts them against the
> §8.3 table); the §10.4 classification of the 26 weenie entities is unchanged too. Please
> state the element form.

> **Answered** (2026-10-07, OBS2, T33): every element is an entry of PID 0x2E
> (`ScriptTable_DBScriptTable`) holding a DID; no struct element exists (§8.1, SI14).
> Accepting both forms is harmless.

> **OPEN** (§7.7, §8.6, review): a SetProperty node's driven pairs (§4.2) name a *field
> PID*, but the spec does not say which field PID carries the node's value (the PV's own
> PID, a `SetPropertyScript_…` name, or another). **Decision** (implementation, unchanged):
> the source of the first driven pair whose source has a value replaces the constant,
> whatever field the pair names. A pair on another field (an `IntensityMultiplier`, say)
> would wrongly write its source into the target property. Please name the field.

> **Answered** (2026-10-07, OBS2, T32): the pair whose field PID equals
> `SetPropertyScript_PropertyName` carries the value; in this install every driven pair of
> every SetProperty node is such a pair (384 of 384, SI13). Use only those pairs.

> **Decision** (implementation, §11.3, review): the evaluator's property state holds only
> values a Switch or When test compares: SetProperty nodes whose value is a string, struct,
> array, waveform or StringInfo are ignored, and such dictionary defaults are not used.
> This keeps a node visit's cost independent of the value's size. A Switch whose cases
> compare strings would then take its Default port; none is known in this install.

> **OPEN** (header "Implements into", review): the clip-selection evaluator of §8 and §11.3
> (`MotionState`, `PropertyState`, `ClipResolver`) was implemented in
> `crates/formats` (`script_table::resolve`) for M4, next to the decoders, although the
> header names `crates/anim` or a `script` module of `crates/entity`. It is a static,
> one-shot evaluator; the live, incremental evaluator of §11.3 step 4 (M7) should not
> duplicate it. The placement needs a decision record (move it, or record that `formats`
> keeps the static evaluator), and then this header and the `crates/formats` line of
> `CLAUDE.md` updated to match.

> Not a format question: it needs the decision record the implementation asks for; this
> spec's header will follow that record.

> **OPEN** (implementation, River-hobbit height fix, §6.9): the MeshDeformer field names are
> given only by their endings (`…_AnimationAsset`, `…_AnimationTime`,
> `…_UseBoneWeights`). To apply a driven property the implementation must know which field
> a driven pair names; it recognises the field by its dictionary type (DID, float, bool;
> distinct for the three fields), not by name. Please give the full property names.

> **Answered** (2026-10-07, OBS2): `MeshDeformerScriptNode_AnimationAsset`,
> `MeshDeformerScriptNode_AnimationTime`, `MeshDeformerScriptNode_UseBoneWeights` (§6.9);
> every driven pair on these nodes names `MeshDeformerScriptNode_AnimationTime` (SI13).

> **OPEN** (implementation, §6.7): Tessera Decisions where the rule is silent: a
> `ManualTime` that is not finite gives fraction 0; a looping fraction that rounds up to 1
> (for a tiny negative value, *g* + 1) is taken as 0; when several held nodes start on one
> layer the most likely one (blend weight × chance) is kept, as for idle layers; held clips
> are combined like every other clip, on their own layer, in every motion state, and are
> never faded out while moving. `StartTime`/`EndTime` other than 0 and 1 are ignored (no
> held node of the install uses them; step 2 OPEN).

> **Answered in part** (2026-10-07, BIN3): the non-finite and rounding choices are accepted
> as Tessera rules. "When several held nodes start on one layer the most likely one is kept"
> is **not** the client's behaviour: starting a clip never stops another clip of the same
> layer (§6.2); all of them play and share the layer's budget. Held clips are combined like
> any other clip and are not faded out while moving, as the implementation does.

> **Corrected** (2026-10-07, BIN5): the rounding choice accepted above is not the client's.
> A looping fraction that rounds up to 1 stays 1 and gives the end of the played length
> (§6.7 step 1, T36). The non-finite choice matches the client except NaN on a looping node,
> which is now a Decision in §6.7.

> **OPEN** (implementation, §6.9 SliderMeshDeformer): when a node's value string is driven
> by a property the entity lacks (Elf, HighElf), the implementation applies the
> `appearance.md` §6.3 Decision (35 × `80`) rather than the node's stored string; when the
> node's string is not driven at all it uses the stored string. No such node exists in the
> install.

> **Answered** (2026-10-07, BIN5, T39): the client parses whatever string the field holds,
> so a node whose string is not driven uses its stored string, as the implementation does
> (§6.9). The absent driving property is the `appearance.md` §6.3 OPEN; its Decision stands.

> **OPEN** (implementation, 2026-10-07, engineering bounds; confirmation only): following
> the answer above, the resolver now reports **every** held node (no most-likely pick per
> layer), and held clips of one data layer play side by side on that layer and share its
> budget (§6.2). Defensive bounds that no spec states: a figure keeps at most 4 held clips
> (the install has at most one per avatar), and one layer plays at most 4 clips at a time
> (`skeleton-anim.md` §15 implementation note). Please say if the client has limits.

> **Answered** (2026-10-07, BIN5, T40): the client has no limit on held clips per figure or
> on clips per layer; a layer logs a warning when it holds 1,000 clips and still plays the new
> one. Tessera's bounds (4 held clips per figure, 4 clips per layer) are implementation
> limits, not client rules. The install never reaches them (at most one held clip per
> avatar, §6.7 census), so a dropped or refused clip is a deviation and should be logged.

> **OPEN** (implementation, 2026-10-07, after the BIN5 answers): §6.7 is implemented as
> stated (the truncating 32-bit integer part, a looping fraction that rounds up to 1 stays 1,
> the held time *f* × the played length with the 1/30 s end crop for `LoopForever` 0 and
> `LoopCount` 1, `StartTime` and `EndTime` not used), and the Tessera bounds now log what
> they drop or refuse. One case the rule leaves open: a held one-shot node (`LoopCount` 1)
> whose clip is shorter than the end crop has a played length of 0 or less (the shortest
> clip of the install is 0.016 s, though no held node uses one). Tessera then holds the clip
> at time 0 (Tessera guard, as for a clip without a usable duration). Please state the
> client's result if it matters (`havok-data.md` §2.3 lowers the starting time to
> `duration` − 1/30 s, which would then be negative).

> **Answered** (2026-10-08, BIN6, T43): the client gives the same result as the Tessera
> guard. Its held-time setter takes the smaller of the played length and *f* × the played
> length and then the larger of that and 0, so a played length of 0 or less gives time 0
> for every fraction. The negative starting time of `havok-data.md` §2.3 is raised to 0 by
> the middleware's time step, which also advances nothing and never ends the clip while
> the played length is 0 or less. §6.7 step 2 now states this; keep the guard (it is the
> client's rule, not only a guard). The other §6.7 items of this note are confirmed as
> implemented.

> **OPEN** (implementation, 2026-10-08, T41/T42 implemented; Tessera Decisions to
> confirm): every script run (`Startup`, a start-up name, a Run node) now runs **all**
> gathered definitions, kind-1 list, then kind 2, then kind 4, kinds compared as bit masks,
> non-matching tables descended into, null and self-naming includes skipped (also in the
> closure), and a Collection starts its Default ports by ascending ordinal (stable sort).
> Choices the spec leaves open: (1) the three lists are gathered by three separate
> pre-order visits, and an overriding definition (§8.2 step 2) stops the descent only in
> the lists its table contributes to; a kind-2 table with an overriding definition does
> not hide the kind-1 tables below it from the kind-1 list. Is that the client's rule, or
> does an overriding definition cut the descent for every list? (2) "Stopped while it is
> still starting" (§7.5 step 4) is modelled only as a Run node with `RunScript_Stop` = 1
> for the script run the Collection belongs to; the stopped run's later roots and later
> children of the same port still start. Does a stop also end those starts, and can a
> Collection be stopped any other way during its start? (3) Label (25) inherits from
> Collection (§5.1); Tessera starts its ports in stored order. Does it share the Collection
> start routine (ascending ordinal)? (4) Sequence steps start by ascending step number and
> Timer ports by ascending time (`appearance.md` §6.4: a later step counts as later); equal
> keys keep their stored order, a NaN time sorts last, ports of other types follow in
> stored order. (5) §8.2 step 1 (an instance-level definition taken first) is not
> implemented: `Entity_InstanceScriptTable` is gathered as an ordinary table after the class
> table. Is step 1's "instance-level definition" that table, or something else? (6) A
> start-up name whose gathering yields no definition is reported as not found.

---

<a id="s17"></a>
## 17. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-05 | §1 (BIN2, RH, T19–T22), §6.1, §6.2, §6.5, new §6.7–§6.9, §8.3, §12 (SI11–SI12), new §13.2, §15, §16 | Held clips (`ManualUpdate`/`ManualTime`), layer combination, the layer-110 `scale_blend` clips of River-hobbit and DwarfStoutAxe, the deformer nodes 18 and 81 and the per-avatar species tables, from data and the client (BIN2, decision 0011); idle-emote layers per body corrected (implementation OPEN answered). | Spec writer (dirty room) |
| 2026-10-06 | §7 | One-line cross-reference to `script-runtime.md` added to the lead-in; no fact changed. | Spec writer (dirty room) |
| 2026-10-07 | §1 (BIN3, OBS2, T14–T16, T18, new T23–T34), §4.3, §4.5, §6.1, §6.2, §6.3, §6.4, §6.5, §6.9, new §6.10, §7.1, §7.4, §7.7, §7.8, §8.1, §8.2, §8.4, §8.5, §8.6, §9, §11.3, §12 (SI6, new SI13–SI15), §13, §14, §15 (rewritten, new §15.1 owner item O16), §16 (implementation OPENs answered) | From the client (BIN3, decision 0011) and the data (OBS2): switch-case `RampOut` and `Delay` and the switch's delay and linear branch cross-fade; additive script lookup with `Overriding` pruning (replaces the first-wins Decision; every closure `Startup` runs); case-insensitive scriptlet-parameter matching in the innermost invocation; the Random pick rule; absent properties read as default or zero; the clip lifecycle (start offset, end and scheduled fade, synchronised start origin, start-up clips without ramp-in, events beyond the duration never fire); entity-scale factor on `LocomotionDisplacement`; slider update at a constant rate; no rate matching; the avatar fidget path and timing (corrects §8.6); SetProperty and deformer driven fields; element form of additional tables; §13 offset corrected. | Spec writer (dirty room) |
| 2026-10-07 (revision after review) | §1 (new BIN4, OBS3; T23, T29, T30, T34 revised; new T35), §4.3, §6.10, §7.1, §7.4, §8.4, §8.5, §8.6, §12 (new SI16), §15 (items 3, 6, 7, 8), §15.1 | Review of 2026-10-07 answered. §4.3: the internal class names removed; the switch-case stream order stated plainly (own fields, then `RampOut`, then `Delay`); the transition-port fields explained (both `Delay` fields are one value, `Wait`, `RampIn`, `RampOut` without effect; BIN4, OBS3). §7.1: what a matching transition port does (children start at once; with `Wait` = 1 their branch fades in over `RampIn` and the change to the new case waits until they finish; BIN4); the §15 OPEN narrowed to the matching rule and two edge cases. §6.10: the two refusals reported as errors named. §7.4: the port walk order is the client's container order, not shown to be the stored order. §8.4: the player's slider time is traced (the merged `RampTime`, 0.13 s running, 0.25 s walking here; BIN4) and the 0.1 s reset on a movement freeze added (review optional item). §8.5: correction, the player's per-direction speeds come from the data (`skeleton-anim.md` §7.4); Decision revised. §8.6 and §15.1 (O16): the fidget chance under `NoRepeats` corrected (330 / 530 only for the first pick; certain after a pause; 230 / 430 or 320 / 520 after a fidget) and the pick interval made consistent (12 s after a pause; after a fidget 9 s plus the untraced time until its Run step finishes). | Spec writer (dirty room) |
| 2026-10-07 (BIN5) | header; §1 (new BIN5, T19 note, new T36–T40); §6.2; §6.7 (steps 1 and 2, example table, implementation OPEN answered); §6.8; §6.9 (new edge cases, implementation OPEN answered); §15 item 5; §16 (one correction, two answers) | Edge cases from a clean-room audit of the River-hobbit height code, settled from the client (BIN5, decision 0011). §6.7: the integer part is a truncating 32-bit conversion, so non-looping values that are not finite or of magnitude 2³¹ or more give 0; looping infinities give 0; a looping fraction that rounds up to 1 stays 1 (corrects the Tessera rule "gives 0"); NaN on a looping node is undefined in the client and becomes a Tessera Decision (0); the held time is *f* × the played length, which is the duration minus the 1/30 s end crop for `LoopForever` 0 with `LoopCount` 1, and `StartTime`/`EndTime` do not enter (resolves the step 2 OPEN; the `LoopCount` 1 column of the example corrected, rows added). §6.8: the DwarfStoutAxe clips use that cropped length. §6.9: a deformer whose clip is missing or does not fit contributes nothing; any number of deformers; deformer time limited to [0, duration], NaN Decision; a slider node parses whatever string its field holds; one slider set per entity. §6.2: no clip limit per layer (warning at 1,000). The facts the audit found contradicted by code stand: several held clips share their layer (§6.2; re-checked in the layer's play routine, which changes no other clip, BIN5) and driven pairs bind by field PID (§4.2, SI13). | Spec writer (dirty room) |
| 2026-10-08 (BIN6) | header; §1 (new BIN6, OBS4, T41–T43); §6.7 step 2; §7.5 (new start-order paragraph and census); §8.2 step 3 (run order; OPEN resolved); §11.3 step 3; §12 (new SI17); §15 items 1 and 3; §16 (implementation OPEN on short held clips answered) | Two clean-room implementation OPENs answered from the client (BIN6, decision 0011). Start order of one script run: kind-1 definitions, then kind-2, then kind-4, roots in stored order, each node completing its immediate starts first; a Collection starts its ports by ascending ordinal (the reverse of the stored order in most Collections), equal ordinals in an untraced order (Tessera Decision: stored order); this fixes what "most recently started" means for `appearance.md` §6.4 and the order of equal-priority script appearance pieces. A held one-shot clip not longer than the 1/30 s end crop is held at time 0 (the Tessera guard is the client's result). | Spec writer (dirty room) |
| 2026-10-08 (BIN7) | header; §1 (new BIN7; T41, T42); §7.5 step 2; §8.2 step 3 | Non-blocking notes of the review of 2026-10-08. T41 now covers the gathering sentences of §8.2 step 3 (kind match, descent into includes, a root not added twice), and the kind is stated to be tested as a bit mask (a common set bit), with null and self-naming includes skipped (BIN7). §7.5 step 2 and T42 state why equal ordinals keep the container order: the client sorts 32 ports or fewer by a stable insertion sort, and no Collection has more than 26 ports (SI17); above 32 its sort is not stable. The Tessera Decision is unchanged. Status Revised. | Spec writer (dirty room) |
| 2026-10-10 (BIN8) | header; §1 (new BIN8, OBS5, T44, T45); §4.3 (Execute payload row); §9 (gameplay-hook bullet); new §9.1 | P2 revision for `combat-visuals.md`: StoryHook nodes at run time (they release strikes of the performer's story, BIN8), the Execute callback census and the meaning of the Execute result-port payload (OBS5), the Beam port. Status Accepted → Revised, awaiting review. The former sentence "in an offline client they are no-ops, and in M7 they become messages to Tessera's server" is replaced. | Spec writer (dirty room) |
| 2026-10-14 | header | Non-blocking note of the independent review of 2026-10-11: **Last updated** now gives the date of this edit and of the last content change (2026-10-10, BIN8). No fact, rule or section changed; no implemented behaviour changes. Status Accepted → Revised for a reviewer's confirmation. | Spec writer (dirty room) |
| 2026-10-14 | header | Narrow review of the header-only revision (independent spec reviewer): passed, status Accepted. No content change; no implemented behaviour changes. | Spec reviewer (dirty room) |
| 2026-10-23 | header, §1 (T46), §6.3, §7.8, §8.3 | Revision answering the narrow OPEN of `appearance.md` §15 (for this spec and `skeleton-anim.md`): §7.8 point 1 and §6.3 now follow `appearance.md` §7.4.1 rule 4 (parameters reach node fields only through scriptlet-driven pairs, never by PID); §6.3 states the effect on locomotion displacement (no clip choice changes, three avatar displacements appear, the wider implementer run reported); §8.3 table: DwarfStoutAxe walk 2.00 and run 6.00 m/s, RiverHobbit F walk back −2.57 m/s (own resolver probe, T46). Status Accepted → Revised, awaiting independent review. **No implemented behaviour changes** (the implementation already follows rule 4). | Spec writer (dirty room) |
| 2026-10-23 | header | Independent review (spec-review:spec-gameplay:2 (independent session)) of the 2026-10-23 revision: **passed**, CLEAN_ROOM.md §5.3 every item; status Revised → Accepted. Header status and review record only; no other text changed; no implemented behaviour changes beyond those the revision row states. | Spec reviewer (dirty room) |
| 2026-10-23 | header, §1 (BIN9, T47, T48), §5.2, §6.10 | Spec writer (dirty room); status Accepted → Revised, awaiting independent review. §5.2 gains the MeshFX anchor rule (attached: holding location first, else bone, both 0 → holding-location key 6; unattached: bone first, else holding location, else origin; a missing anchor shows nothing), answering the implementer OPEN of `character-screens.md` §2.1.4 (2026-10-23); the implementer's rule (bone first) holds for unattached nodes and differs for attached nodes that set both fields. §6.10 step 1: the `RampIn` replacement covers only the requests made while a start-up script is being started (T48), confirming the reading in `character-screens.md` §2.1.4. Notes for other owners: `appearance.md` §5.7 step 1 ("the bone's frame when the node names a `BoneType` instead of a location") is consistent with T47. | Spec writer (dirty room) |
| 2026-10-23 | header, §1 (OBS6, T49), §5.2 | Spec writer (dirty room); status Accepted → Revised, awaiting independent review. The two non-blocking notes of the review of the BIN9 revision: the MeshFX nodes that set both anchor fields are counted (17 of 10,349, all attached; T49 with the counts by flag), and the frame of a key-6 attachment on the 11 setups with a key-6 element becomes an OPEN callout with a Tessera rule (that element's frame). No BIN fact changed. |
| 2026-10-23 | header, §1 (OBS7, T50), §5.2 | Spec writer (dirty room), package spec-world-r5; status Accepted → Revised, awaiting independent review. §5.2: new paragraph "Times" (pointer to `appearance.md` §5.7 step 4, revised the same day); the world-conform implementer OPEN on the MeshFX times filed and answered by that pointer; the implementer OPEN on unattached anchors filed and answered: read once at placement holds for screen figures too, with a Tessera rule that lets such a prop be a figure part only with its placement transform frozen (3 such nodes in the 32 tables that name `CharGen_Stage`, T50). Implemented behaviour changes: unattached screen-figure props stop following their anchor. |
