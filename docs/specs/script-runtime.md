<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Script runtime: how script-table node graphs run on an entity over time

| | |
|---|---|
| **Status** | **Revised** (2026-10-09, spec writer: §30.6 rewritten from the client (BIN-I: the PhysicsImpulse target ports are 8 `ImpulseTarget` and 19 `Impulser`, not 25; one target, one request; the impulser path; what the request carries; when the target applies it; OPEN-20 answered: the node completes on the target's completion notice); §3.1 step 7 and §3.2 made precise and §29 item 19 answered (R47); new rows R47–R51, sources BIN-I and OBS-I, OPEN-22; R44 corrected; review 2026-10-09 (another session, not the writer): **changes requested**, blocking items B3 and B4). Before: **Accepted** (2026-10-09: re-review of the §30 revision after the answers to B1 and B2 and the writer's note on N1–N4 (commits 9be154a, ca0a905): passed CLEAN_ROOM.md §5.3, every item; see the review record). Before: Revised (2026-10-09, spec writer: new §30 (the While node, the start outcome of PhysicsImpulse and the avatar's jump path; sources BIN-W and OBS-W, rows R41–R46, OPEN-19 to OPEN-21), the implementer OPEN of §8 answered, pointers in §18 and §29 item 12, §27 items 18–20; blocking items B1 and B2 of two reviews answered, N1–N4 taken up). |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (dirty room, a different session from the writer) |
| **Review record** | Review 2026-10-08: **passed**, Accepted. 8 BIN facts spot-checked in the client (R1, R5, §4.3, R8, R13, R21, R22, R24) and 3 data facts over every record (§12.3 limits, RI9, the music Switches of §6.2). OPEN-5 settled from the binary (§6.2, row R27); the order of the §3.1 steps corrected (R1). Full record in the review log (decision 0018). Narrow re-review 2026-10-08 (another session) of the R1 order note and the OPEN-5 lines (R27, BIN-R, OBS-R, §6.2, §6.3, §27 item 5, §29 item 1): **passed**; facts re-checked in the client and over every record; the "no value" clause of R27 and §6.2 narrowed to the Switch (wording). Review 2026-10-09 (another session, not the writer) of the owner-observation revision (BIN-O, R40, §8 "Random source", change-log row): **changes requested**. Facts spot-checked in the client (same build, private copy of the analysis project, deleted afterwards) and **all reproduced**: the Random port pick draws an integer in [0, sum − 1] through the integer helper, i.e. the whole part of *u* × sum; *u* comes from one global combined multiplicative congruential generator with a 32-entry shuffle table, strictly between 0 and 1; the seeding routine has exactly two callers, one seeding from the wall-clock time in whole seconds from a start-up routine, the other taking its seed from a script-machine value stack; nothing in the node path seeds per entity, object or node. Checklist: functional facts only, no code or pseudo-code, no copied prose (the name of the published generator design is general knowledge, not an identifier to copy), no game text, keys or hostnames, BIN-O meets decision 0011, nothing under a closed gate. **Blocking:** (1) §28: the new change-log row is separated from the table by a blank line and followed directly by the `---` rule, so it is not part of the table and renders as a heading; move it into the table (no blank line before it, one blank line after it). (2) §8 "Random source": "when it runs was not traced" states an unknown inline; make it a `> **OPEN:**` callout (or point to the O12 OPEN of `appearance.md` §7.4, which already names the possible reseed), so that §5.3 "unknowns are marked OPEN" holds. With (1) and (2) fixed the revision can be accepted without a further binary check. Writer's answer 2026-10-09: (1) the row moved into the §28 table; (2) the untraced timing of the second seeding caller is now §8 OPEN-18 (pointing to `appearance.md` §7.4 O12), listed in §27 item 17; no fact changed. Re-review 2026-10-09 (another session, neither the writer nor the earlier reviewer) of the writer's answer (§8 OPEN-18, §27 item 17, §28 table): **passed**, Accepted. Both blocking items fixed: the 2026-10-08 row is inside the §28 table; the untraced timing of the second seeding caller is a `> **OPEN:**` callout (OPEN-18) listed in §27, and its pointer to `appearance.md` §7.4 O12 and the instance-ID Decision match that spec. R40 spot-checked again in the client (same build, private copy of the analysis project, deleted afterwards) and **reproduced**: the seeding routine has exactly two callers, one seeding from the wall-clock time in whole seconds from a start-up routine, the other from the top of a value stack; the generator is a combined multiplicative congruential generator with a 32-entry shuffle table; the Random pick draws an integer in [0, sum − 1] as the whole part of *u* × sum. Checklist §5.3: functional facts only, no code or pseudo-code, no copied prose, no game text, keys or hostnames, every new fact has a provenance row (R40, BIN-O, decision 0011), unknowns marked OPEN, nothing under a closed gate. Non-blocking: the 2026-10-08 change-log row states the bytecode reseed without the Inferred mark that §8 gives it. Review 2026-10-09 (another session, not the writer) of the §30 revision (`git diff main -- docs/specs/script-runtime.md` on specs-11, commit d044cc8: header, BIN-W, OBS-W, R41–R46, §8 answer, §18 pointer, new §30, §27 items 18–20, §28 row, §29 item 12): **changes requested**. Spot checks (read-only, throwaway probes in the dirty scratch area with the repository's script-table decoder as a tool; the executable searched byte-wise, no Ghidra run). **Reproduced:** R43 and the §30.5 table over every record (7,524 tables decode, 2 rejected as stated): 41 While nodes in 17 tables; field bytes give `Names` count 0 and `Inverse` 0 in all; one port each, type 6, with 1 or 2 children; no targets; properties, dictionary types, defaults and `Values` (8 × `Movement_IsJumping` true, 4 × `Combat_Violent` true, 1 × `Script_PlayingCraftAnimation` true, 24 × 0x00000200, 3 × 0x04000000, 1 × both 0x00000200 and 0x00020000 on `CombatState_CurrentState`) match. R45 and §30.7 items 1–3: `Movement_ActionKey` in 0x07000433 and 0x070017E2 (Switch chain, cases, Label ports ordinal 1 OnStop with a Run of `Loc_JumpKeyReleased`, ordinal 2 Run of `Loc_JumpKeyPressed`, both Persistent 0); `Loc_JumpKeyPressed` in 0x0700001C, 0x070017E4, 0x07002B86 (cases 5, 3, 1, 2, 6; without 6 in 0x070017E4; Run of `Loc_Jumping` Persistent 1); `Loc_Jumping` on foot (WhenRunning Persistent 0, NotRunning port, Collection ordinal 1 SetProperty true and ordinal 4 While, the one-pass Sequence with steps 1–3, the 0.2 s Timer, the persistent Run of `Jump`); every Switch on these paths has `Persistent` 0, so consequence (c) holds; `Loc_JumpKeyReleased` is an empty Collection in all three tables; `Script_MaxNumInstances` 2 for `Loc_JumpKeyPressed` and `Loc_Jumping`. R46 (scripts): every SetProperty of `Movement_IsJumping` in all tables and the 28,992 scriptlets lies in `Loc_Jumping` or in the key-1 (`Jump`) definitions of six kind-1 tables that are in none of the 14 avatar class closures of `script-table.md` §8.2, so "for the player" holds. R46 (BIN, indicative): the property ID occurs once in the code section as an immediate operand (a call argument); its other five byte matches are jump displacements. BIN facts R41, R42, R44 were not re-read in the client; they are consistent with the data (the `Loc_Jumping` Sequence clears the flag inside the While's own branch). Checklist §5.3: functional facts only; no code or pseudo-code (the §30.3 and §30.6 steps are behaviour in plain words); no copied prose; script-key and property labels are interoperability identifiers, label names not quoted, no game text, keys or hostnames; every new fact has a row (BIN-W under decision 0011, offline script code); unknowns are OPEN-19 to OPEN-21 with Decisions, listed in §27; nothing under a closed gate. **Blocking:** **B1** §30.7 OPEN-19: "the jump scripts carry `Script_ClientAuthorative` true and `Script_ReplicateToClient` false" is not what the data hold: only `Loc_JumpKeyPressed` carries these two (in all three tables); the per-script set of `Loc_Jumping` holds only `Script_MaxNumInstances` 2 and that of `Movement_ActionKey` is empty. Name the script that carries them (and add the observation to OBS-W/R45 if it is kept as a fact). Non-blocking: (N1) §30.7 item 2 calls 0x0700001C, 0x070017E4 and 0x07002B86 "player locomotion tables", wording inherited from `collision.md` §11.10; only 0x0700001C (with 0x07000433) lies in the avatar class closures of `script-table.md` §8.2, the other two (and 0x070017E2) are reached from other class tables; consider "the locomotion tables that define `Loc_Jumping`". (N2) The six non-avatar `Jump` definitions set `Movement_IsJumping` true at start and false in an OnStop, a data pattern worth recording under OPEN-19 as a hint. (N3) Consequence (d) could add that the run of `Movement_ActionKey` (limit 1, held by its OnStop) also refuses a second start until something stops it, which item 1 implies. (N4) §30.5 first-children cell "Switch with Particle" reads as one node; the 0x00000200 ports start with a Collection, Timer, MeshFX, Switch or Particle. With B1 fixed the revision can be accepted without a further binary check. Writer's answer 2026-10-09: B1 fixed. The per-script property sets of the jump keys re-read over every decodable table (OBS-W): `Script_ClientAuthorative` true and `Script_ReplicateToClient` false are carried by `Loc_JumpKeyPressed` and also by `Loc_JumpKeyReleased`, in each of the three tables that define them (both with `Script_MaxNumInstances` 2); `Loc_Jumping` holds only `Script_MaxNumInstances` 2; `Movement_ActionKey` has an empty set in both its tables. OPEN-19 now names these scripts; the sets are stated in §30.7 items 1–2 and in R45. Non-blocking N1–N4 not taken up in this answer. No other fact changed. Re-review 2026-10-09 (another session, neither the writer nor the earlier reviewer) of the writer's answer (commit 6861362: header, OBS-W, R45, §30.7 items 1–2, OPEN-19, §28 row): **changes requested**. Spot check (read-only, own throwaway probe in the dirty scratch area with the repository's script-table decoder as a tool; no Ghidra run): the per-script property sets of keys 9, 0x10000426, 0x100002C5 and 0x1000024F over every script table (7,524 decode, 2 rejected, as stated). **Reproduced:** `Loc_JumpKeyPressed` and `Loc_JumpKeyReleased` carry `Script_ClientAuthorative` true, `Script_ReplicateToClient` false and `Script_MaxNumInstances` 2 in each of 0x0700001C, 0x070017E4 and 0x07002B86, and are defined in no other table; `Loc_Jumping` holds only `Script_MaxNumInstances` 2 in the same three tables; `Movement_ActionKey` has a present but empty set in 0x07000433 and 0x070017E2. B1 is fixed: OPEN-19, item 2, R45 and OBS-W state this correctly, and the new change-log row is inside the §28 table. Checklist §5.3: functional facts only; no code or pseudo-code; no copied prose; script-key and property labels are interoperability identifiers, no game text, keys or hostnames; the new facts have provenance (OBS-W, R45, Confirmed); unknowns stay OPEN-19 to OPEN-21; nothing under a closed gate. **Blocking:** **B2** §30.7 item 1: the new sentence "Its per-script property set is empty in both tables." follows the clause about `Loc_JumpKeyReleased`, so its nearest antecedent is `Loc_JumpKeyReleased`, whose set item 2 and OPEN-19 state as non-empty; only "both tables" hints that `Movement_ActionKey` (two tables) is meant. Name the script (for example: the per-script property set of `Movement_ActionKey` is empty in both its tables), so that item 1 cannot be read as contradicting item 2 on the very fact B1 was about. Non-blocking: N1–N4 of the earlier review still apply. With B2 fixed (wording only) the answer can be accepted without a further data or binary check. Writer's answer 2026-10-09: B2 fixed. §30.7 item 1 now reads "The per-script property set of `Movement_ActionKey` is empty in both its tables", and the following sentence names `Movement_ActionKey` instead of "this key", so item 1 no longer reads as contradicting item 2 and OPEN-19 about `Loc_JumpKeyReleased`. Wording only; no fact, row or OPEN changed. Non-blocking N1–N4 not taken up in this answer. Writer's note 2026-10-09: no blocking item remains open; N1–N4 taken up. (N1) §30.7 items 1–2 now say "the locomotion tables that define `Loc_Jumping`". (N2) OPEN-19 records as a hint, and R46 as a Confirmed fact, that the six `Jump` (key 1) definitions outside the avatar's class closures set `Movement_IsJumping` true in step 1 of a root Sequence and false in the child of a root OnStop (re-read over every decodable table and scriptlet; OBS-W). (N3) Consequence d adds that the run of `Movement_ActionKey` that reached its Label (limit 1, held by its OnStop) refuses a second start until it is stopped (Inferred from §12.3 and item 1). (N4) the §30.5 cell for the 24 nodes on 0x00000200 now lists the first children separately: one Collection (12), Timer (5) or MeshFX (2), or a Switch and a Particle (5), re-read over every While. No BIN fact, OPEN Decision or section number changed. Re-review 2026-10-09 (another session, neither the writer nor either earlier reviewer of §30) of the answer to B2 (commit 9be154a) and the writer's note on N1–N4 (commit ca0a905: header, OBS-W, R46, §30.5, §30.7 items 1–2, consequence d, OPEN-19, §28 rows): **passed**, Accepted. Spot check (read-only, own throwaway probe in the dirty scratch area with the repository's script-table decoder as a tool; no Ghidra run, no BIN fact changed): every SetProperty of `Movement_IsJumping` and every While node over all tables and scriptlets (7,524 tables decode, 2 rejected; 28,992 scriptlets decode). **Reproduced:** R46 and the OPEN-19 hint: besides `Loc_Jumping` (in 0x0700001C, 0x070017E4, 0x07002B86) the only setters are six `Jump` (key 1) definitions, each setting true as the child of step 1 of a root Sequence and false as the child of a root OnStop; no scriptlet sets the property. §30.5 (N4): of the 24 While nodes on 0x00000200, 12 start a Collection, 5 a Timer, 2 a MeshFX and 5 a Switch and a Particle; the other rows are unchanged and match. **B1** stays fixed (OPEN-19, item 2, R45). **B2** is fixed: §30.7 item 1 names `Movement_ActionKey` as the script with the empty set, so it no longer reads against item 2. N1: "the locomotion tables that define `Loc_Jumping`" is accurate. N3: the added sentence of consequence d follows from §12.3 (no `Script_MaxNumInstances` gives a limit of 1) and item 1, falls under the Inferred mark of the consequences, and agrees with `collision.md` §11.10 and its Decision item 2. Checklist §5.3: functional facts only; no code or pseudo-code; no copied prose; script-key and property labels are interoperability identifiers, label names not quoted, no game text, keys or hostnames; the new facts have provenance (OBS-W, R46, Confirmed); unknowns stay OPEN-19 to OPEN-21 with Decisions, listed in §27; the new change-log rows are inside the §28 table; nothing under a closed gate. Non-blocking (no change needed for acceptance): (N5) the §30.5 cell "Collection (one with a Timer)" for 0x04000000 means a Collection and a Timer as sibling first children of one node, not a Timer inside the Collection; (N6) two lines of §30.7 items 1–2 now exceed the wrapping width of the file (cosmetic). Review 2026-10-09 (another session, neither the writer nor an earlier reviewer of this spec) of the BIN-I revision (commit d0ba8f4: header, BIN-I, OBS-I, R44, R47–R52, §3.1 step 7, §3.2, §23 RI13–RI15, §27 items 19 and 21, §28 row, §29 item 19, §30.6): **changes requested**. Spot checks, read-only. Data (own throwaway probe in the dirty scratch area with the repository's script-table decoder as a tool; 7,524 tables decode, 2 rejected; 28,992 scriptlets decode): **reproduced** R52 and RI15 over every record: 37 PhysicsImpulse nodes (31 in tables, 6 in scriptlets), `ImpulseType` 2 and `IsGlobalSpaceImpulse` 0 in all, 18 with `IsJump` and none of them with a target, 6 with targets (5 with an `ImpulseTarget` of type This and an `Impulser` of type Property, 1 with an `ImpulseTarget` of type Property only), no node with a port; the only driven fields of these nodes are `AirTime` (8 nodes) and `Impulse` (5), so no driven pair overrides `ImpulseType` or `IsGlobalSpaceImpulse`. Client (same build, Ghidra 12 on a private copy of the analysis project, deleted afterwards): **reproduced** R47 (each start into a port group advances the group's counter and compares it after the kind start; first the parent-or-ancestor test, which stops the new instance with reason 0 and stops the group; then a moved counter, which stops the new instance and reports a failure even when no instance was left; then an already stopped group, whose reason the new instance gets; the port runner ends on a failed start or a stopped filled instance or ancestor, and on nothing else), R48 (port type 8 read before 19, first entity of each used, a present list with no entity logged and answered with a failure code that the generic start ignores, so the instance is kept and registered), §30.6 step 1 (bit 6: no instance, completion at once), R49 on the default path (vector `Impulse` × intensity or the jump vector; the request carries `ImpulseType`, `IsJump`, world-frame flag 0, bit 5 of the context's flags and the scheduler time plus the context's offset; `IsGlobalSpaceImpulse` not read) and the R51 consequence that an instance whose completion notice came back during its start is released, so the start completes at once. Not re-read: the impulser path, the target-side processing and the stop hook. Checklist §5.3: functional facts only; no code or pseudo-code (the §3.1 step 7 (a)–(d) checks and the §30.6 steps are behaviour in plain words); no copied prose (the client's log messages are not quoted); `ImpulseTarget` and `Impulser` are port-type labels of the data's enum mapper (interoperability); no game text, keys or hostnames; BIN-I meets decision 0011 and touches nothing about network, login or keys; nothing under a closed gate. **Blocking:** **B3** §30.6 paragraph "Completion": "Stopping the node earlier (§3.5), except on a teardown stop (reason 2), sends a stop notice to each entity still in its pending work" has no provenance: R50 and R51 do not state it and the BIN-I reading list names no stop hook of the node. Add it to a row (and the reading list) with its mark, or mark it Inferred and fold it into OPEN-22 (b). **B4** §28, the 2026-10-09 BIN-I row: "a present port that resolves to nothing fails the start" contradicts §30.6 step 2, R48 and the §29 item 19 answer, because by §0 ("start outcome") a failed start is handled exactly like completion at once, while the body says the instance is registered and stays running until stopped from above. Reword the row (for example: sends nothing and leaves the instance running, unarmed). Non-blocking: (N7) §30.6 "At the target": the inverse turn of a world-frame vector is marked Inferred inline; five nodes of this install take the impulser path, so either confirm the direction from the apply routine or add it to OPEN-22 as (c). (N8) R47 omits two facts that §3.1 step 7 (a) relies on and the client shows: in check (a) the port group is stopped as well, and the start reports success (the runner then ends through its ancestor test). (N9) §0 "start outcome" and the first paragraph of §3.2 ("failed starts never stop the runner") now need the qualifier "kind-start" failures, so that they do not read against the step 7 (b) failure. (N10) The §30.6 census could name the target type of the node with an `ImpulseTarget` only (Property, 3). With B3 and B4 fixed the revision can be accepted without a further binary check. |
| **Implements into** | `crates/scriptgraph` (`tessera-scriptgraph`, new in M6: the live, incremental evaluator of script tables and scriptlets: node instances, contexts, the scheduler clock, Switch/When/Random/Collection/Sequence/Timer/Run/WhenRunning/SetProperty/Goto/Label/OnStop/Scriptlet/ScriptletParameter semantics, the intensity chain and driven properties), `crates/audio` (Sound and SoundEnvironment node instances call into `tessera-audio` per `sound.md` §9), `apps/client` (one scheduler advance and one script update per frame, entity creation and removal, the player properties of `sound.md` §10.2) |
| **Companion specs** | [`script-table.md`](script-table.md) §2–§5 (record framing, node frame, base block, port payloads, node-kind layouts), §7 (the earlier, Reported, name-inferred behaviour that this spec replaces for the kinds it covers), §8.1–§8.2 (an entity's tables and their closure). [`sound.md`](sound.md) §6 (voices, ramp-out), §9 (Sound and SoundEnvironment nodes; §9.6 outcome of a start), §10 (music and ambience graphs), §11 (positional emitters). [`instruments.md`](instruments.md) §10.1 (node intensity; its Q8 is settled here, §4.2). [`property-system.md`](property-system.md) §2 (definitions; the `field_b` value 6 that marks script-parameter properties, §5.1 here), §8 (enum mappers). |

---

<a id="s0"></a>
## 0. Purpose and terminology

`script-table.md` gives the bytes of every node and, in its §7, only name-inferred behaviour.
`sound.md` §9–§11 says which graphs make music, ambient beds, reverb environments and
positional emitters. This spec gives the **continuous run-time semantics** of those graphs as
the original client runs them on an entity, so that a clean-room `tessera-scriptgraph` can
reproduce them exactly: when a node starts, how long it stays running, what it does when its
inputs change, when it completes, what stopping it does, and in which order all of this happens
within a frame.

Scope: every node kind used by the music, ambience and emitter graphs (§1, census row C1):
Collection (2), Run (5), Sequence (6), OnStop (10), Switch (12), WhenRunning (14), Random (17),
Scriptlet (19), ScriptletParameter (22), Label (25), SetProperty (40), Sound (48), Goto (61),
SoundEnvironment (62), When (63), Timer (65), plus what an offline runtime does with the kinds
it does not implement (§18). Animation-node playback, VFX and camera behaviour stay in their
own specs.

Terms:

| Term | Meaning |
|---|---|
| **node** | A node of a 0x07 table script or a 0x0C scriptlet (`script-table.md` §4). Read-only data, shared by every entity that runs it. |
| **instance** | The run-time object a node leaves behind when its start does not complete at once. It has a parent instance, a list of running child instances, a context, a few flags and kind-specific state. |
| **context** | The value an instance runs in: the **intensity** (a float), the entity the script runs on, a **run-parameter set** (a property set, §5.1), a flags word (§3.1) and a few identifiers. Contexts are copied on write: a node that changes any part of it (for example a new intensity) gets its own copy, and its children inherit that copy. |
| **run** | One execution of one script key on one entity: the object that owns the root instances of that script. Started by the client at entity creation (start-up names), by a Run node, or by game code. |
| **port group** | The set of instances started from one port (for example the children of the Switch case that is currently selected). Stopping a group stops all its instances with one reason. |
| **start outcome** | `sound.md` §9.6: a start leaves a *running* instance, *completes at once*, or *fails*; a failed start is handled exactly like completion at once. |
| **armed** | An instance whose own work is done and that completes as soon as it has no running child (§3.4). |
| **stop reason** | A small integer passed down a stop: 0 = ordinary stop or completion; 2 = teardown (§3.5). |
| **scheduler** | The one client-wide clock and event queue that drives Timers, Sequence loop pacing, Switch delays and cross-fade ends, and deferred Run starts (§20). |

Script keys and enum labels (`SND_music_system_startup` …) and property names are
interoperability identifiers read from the data. Label, Goto and scriptlet-parameter names are
strings stored in nodes; they are not quoted here. No game text appears.

---

<a id="s1"></a>
## 1. Provenance

Marks: **Confirmed** = checked over every record of the kind on 2026-10-06; **BIN** = rests on
disassembly of the client (method M4); **Reported** = from another spec or general reference;
**Inferred** = fits every observation but was not proven; **Decision** = a Tessera choice where
the original is unknown, always paired with an OPEN.

| Tag | Source |
|---|---|
| OBS | Observed 2026-10-06 in the user's install (Steam, client data current on that date) by throwaway probes in the dirty scratch area: all 7,526 0x07 and 28,992 0x0C records decoded with the `script-table.md` framing (7,524 tables decode; the 2 others are the CinematicCamera category); the property dictionary 0x34000000 (all 27,546 definitions, including `field_b`); **the run-time scope of this spec**: script `SND_music_system_startup` (0x10000039) of table 0x0700042E, the 73 scripts that its Run nodes start (65 in 0x0700042F, 8 in 0x0700042E; one more key it runs, 0x10000EE7, is defined in no table), scriptlets 0x0C009B74 and 0x0C00846F, the start-up scripts of the 972 emitter descriptors of `sound.md` §11 (310 scripts in 303 tables), and every scriptlet these invoke, recursively: **49,157 nodes**; the avatar class-table closures of `script-table.md` §8.2. |
| BIN3 | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)), this spec, 2026-10-06.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, PE time stamp 2026-09-27), read with a disassembler and a decompiler on address ranges only, in the dirty scratch area. **Read:** the generic node runner (port runner, node start with its gates and the intensity computation, the stop routine, child-stopped and child-finished handling, intensity propagation, the driven-property application at start and the property-change listener); the start routines and instance classes of Collection, Label, Random (with its port choice), Sequence, Timer (scheduling, cycle length, completion), Switch and When (evaluation, case matching, value and transition ports, the delayed switch and the cross-fade of the outgoing case, ForCurrentValue), WhenRunning, Run (request, reply, stop hook, intensity forwarding), SetProperty (the ServerOnly gate), Goto (label search), OnStop, Scriptlet and ScriptletParameter, the Sound instance's stop and intensity hooks; the script start on an entity (lookup through the tables, the per-key instance limit, the context flags) and the per-table lookup with `Script_CombinationType`; the scheduler (insert, cancel, advance) and the per-frame update that advances it; the client start-up value of the process-mode word. **Questions:** items 1–19 of this spec's brief (life cycle, intensity clamp, driven properties, Switch, When, Random, Collection, Sequence, Timer, Run, WhenRunning, SetProperty, Goto/Label, OnStop, scriptlet scope, unimplemented kinds, entity removal, the update clock, runaway guards). **Why M1–M3 were not enough:** the data hold the graphs but none of the run-time rules (order of starts and stops, completion, re-evaluation, timing); no published source describes them (M2: the third-party readers in the research clones decode records only); no running client is available to this session, and an audio capture (M3) could not separate completion order, scheduler pacing and intensity rules. Network, login, launcher, anti-tamper, voice and script-channel replication code were not examined; where a path leads there (cross-entity messages, server replay) this spec stops and says so. |
| BIN-R | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)), spec review, 2026-10-08.** Same client build as BIN3, read with a decompiler on address ranges only, in the dirty scratch area. **Questions:** the order of the node-start gates (R1), and what value a Switch sees for a property the entity does not have (OPEN-5). **Why M1–M3 were not enough:** the data hold no trace of either; `sound.md` §10.2, `script-table.md` T28 and the first version of this spec disagreed, and only the property read decides between them. |
| OBS-R | Observed 2026-10-08 in the same install (data unchanged) by the reviewer's own probes: the dictionary definitions of `Music_Monitor` and `HousingPropertyHook_IntMusic` (both enums, flags without 0x800); every Switch on them in script 0x10000039 of 0x0700042E (65 and 1, all persistent; no case value 0; 52 and 1 with a Default port); `Script_MaxNumInstances` in every per-script set of 0x0700042E and 0x0700042F; all Goto and Label nodes of the 7,524 decodable tables. |
| BIN-O | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)), owner-observation pass, 2026-10-08.** Same client build as BIN3, read with the Ghidra 12 decompiler on a private copy of the analysis project, in the dirty scratch area. **Question:** which random source the Random node draws from and how it is seeded (owner observation O12: does a random-appearance figure keep its look between visits). **Why M1–M3 were not enough:** the generator and its seeding are code; the data holds only the weights; the original cannot be observed here. Read: the Random node's port pick, the integer and unit draw helpers, the generator, its two seeding routines and their callers. Nothing about network, login, keys, launcher or anti-tamper. |
| BIN-W | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)), spec writer, 2026-10-09.** Same client build as BIN3, read with the Ghidra 12 decompiler on a private copy of the analysis project (deleted afterwards), in the dirty scratch area. **Questions:** (1) how the While node (kind 50) behaves at run time: when it starts its children, what it watches, when it completes, what `Inverse` and the `Names` list do; (2) whether a PhysicsImpulse node (kind 9) completes at once; (3) which client code writes `Movement_IsJumping`. **Why M1–M3 were not enough:** the data hold the 41 While nodes and the jump scripts but no run-time rule; no published source describes the node; the original cannot be observed here. Read: the While class's field registration, its class object (kind code and instance factory), the instance's start, its property-change handler and stop hook, the listener interface shared by the node instances, the generic value equality and dictionary-default helpers, the PhysicsImpulse instance factory and its start, the Run instance's reply handler, and a search of the code for the property IDs of `Movement_IsJumping` and `Entity_FinalElement` used as constants. Nothing about network, login, keys, launcher or anti-tamper. |
| OBS-W | Observed 2026-10-09 in the same install (data unchanged) by throwaway probes in the dirty scratch area, with the repository's script-table decoder as a tool: every While node of the 7,524 decodable tables (41 nodes in 17 tables) with its field bytes and ports; every node that sets, tests or is driven by `Movement_IsJumping` in all tables and scriptlets; every Run node that starts `Loc_Jumping`, `Loc_JumpKeyPressed`, `Loc_JumpKeyReleased` or `Jump`; the tables that define script keys 1, 9, 0x1000024F, 0x10000426 and 0x100002C5; the per-script property sets of these keys (re-read over every decodable table in the answer to review item B1); labels of enum mappers 0x23000097 and 0x230000F8. Re-read 2026-10-09 for the answer to the non-blocking review notes: the first children of every While port (§30.5) and the outline of the six `Jump` definitions that set `Movement_IsJumping` (R46). |
| BIN-I | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)), spec writer, 2026-10-09 (second pass).** Same client build as BIN3, read with the Ghidra 12 decompiler on a private copy of the analysis project (deleted afterwards), in the dirty scratch area. **Questions:** (1) whether a node is registered with its parent before or after its kind start, and what the runner's check after each child sees of an instance still starting (§29 item 19); (2) the PhysicsImpulse edge rules left open by §30.6: which target ports it reads, several or no resolved entities, what the request carries, what `IsGlobalSpaceImpulse` changes, when the target applies the request, and when the node completes (OPEN-20). **Why M1–M3 were not enough:** the data hold the nodes and their targets but no run-time rule; the original cannot be observed here, and none of the six nodes with targets runs on the avatar. **Read:** the generic node start, the port runner, the ancestor stop test, the port-group stop, the instance constructor, the arm routine; the PhysicsImpulse class's field registration, field accessors and port declaration list, its factory, start, message handler, and the two request kinds with their field lists, processing at the target, deferred path, apply routine and completion notice; the message dispatch and answer routines. Nothing about network, login, keys, launcher or anti-tamper. |
| OBS-I | Observed 2026-10-09 in the same install (data unchanged) by a throwaway probe with the repository's script-table decoder as a tool: every PhysicsImpulse node of the 7,524 decodable tables and 28,992 scriptlets (fields, targets with their port and target types, driven pairs, ports). |
| ST, SND, INS, PS | `script-table.md`, `sound.md`, `instruments.md`, `property-system.md` as companion specs. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| R1 | Node start gates in the order the client applies them: the same-node rule (§3.1 step 3), the node copy with its driven values (step 4), then on the copy the persistent-node gate (step 1) and the probability draw (step 2: passes when a uniform draw in [0, 1] is ≤ `Probability`; no draw when it is exactly 1), the intensity (step 5, §4), then the kind's start (step 6) | BIN3; BIN-R (order re-checked) | BIN. Corrected in review 2026-10-08 (the first version listed the gates in another order). |
| R2 | Children of a port start one after another in stored order; the runner stops early only when the instance being filled or one of its ancestors has been stopped meanwhile (§3.2) | BIN3 (generic runner); SND §9.6 (BIN2, same rule) | BIN |
| R3 | Completion: an armed instance stops itself with reason 0 when its last running child stops; the call chain is synchronous, so completion propagates up in the same call (§3.4) | BIN3 | BIN |
| R4 | Stop order: kind hook, then running children in list order, then property listeners, then the parent is told (§3.5) | BIN3 | BIN |
| R5 | Intensity factor = `IntensityMultiplier` unchanged when the variance is 0; otherwise multiplier + uniform(±variance), clamped to [0, 1] (§4.2) | BIN3 (factor routine, its three callers) | BIN. **Settles `instruments.md` Q8 in favour of `instruments.md`; corrects `sound.md` §9.3.** |
| R6 | A change of a driven `Script_IntensityMultiplier` gives the instance factor × (factor × parent intensity), and only when `UseParentIntensity` = 1 (§4.4) | BIN3 (listener and intensity setter; checked in assembly) | BIN |
| R7 | Driven properties are applied at start to a per-start copy of the node and then followed continuously through property listeners; script-parameter properties (`field_b` = 6) are read from the run-parameter set, all others from the entity (§5) | BIN3; OBS (390 definitions with `field_b` = 6, by name mostly script and callback parameters, callback outputs and signal arguments) | BIN / Confirmed census |
| R8 | Switch: first matching value case in port order, else the first Default port; no value → nothing; re-evaluation on change of the switch property (Persistent = 1); same case → nothing restarts (§6) | BIN3 | BIN |
| R9 | `SwitchScriptPort_RampOut` (the case's first float, ST "transition_time") keeps the old case running and cross-fades intensities over that time; `SwitchScriptPort_Delay` (second float) delays a case change and is cancelled by a further change (§6.1, §6.7–§6.8) | BIN3 (runtime use of both fields; the client's field lists); OBS (type match on 1,480 transition payloads) | BIN; payload-to-field mapping Inferred. Settles ST §15 item 3 for the case floats. |
| R10 | SwitchTransition ports: four matching tiers; `SwitchScriptPort_Wait` defers the new case until the transition's children finish; `SwitchScriptPort_RampIn` is the cross-fade of the old case meanwhile (§6.9) | BIN3; OBS (1,480 ports; 837 in scope, all with tail 0, 0, 0.0, 1, 0.0, −1.0, 0.0) | BIN |
| R11 | ForCurrentValue ports run on every evaluation of a new value, the previous run being stopped first (§6.10) | BIN3 | BIN |
| R12 | When: the first WhenTest port whose test holds, else the first Default port; one branch at a time (§7) | BIN3 | BIN. Corrects ST §7.2 ("all holding tests run"). |
| R13 | Random: integer draw over the sum of weights; NoRepeats excludes the port picked last time; the memory belongs to the node, shared by every entity; a single port is always taken (§8) | BIN3 | BIN |
| R14 | Collection and Label start their Default ports in ascending payload order and complete when no child is left (§9) | BIN3 | BIN |
| R15 | Sequence: ascending step numbers; a step finishes when none of its children is left running; instant steps chain in the same call; a new pass may not begin within 0.1 s of the previous pass; LoopCount × steps bounds the run (§10) | BIN3; OBS | BIN |
| R16 | Timer: cycle = max(`Duration`, latest port time) ÷ (`Speed` × `SpeedMultiplier`), at least 0.1 s when looping; ports beyond `Duration` fire; ports are skipped while the intensity is ≤ 0.0002; the Timer does not complete while children run (§11) | BIN3; OBS | BIN |
| R17 | Script lookup combines the definitions of every table in the entity's closure (table kinds 1, then 2, then 4; pre-order through includes; duplicates once); `Script_CombinationType` = 1 (`Overriding`) stops the descent into that table's includes (§12.2) | BIN3; OBS (68 per-script sets carry `Overriding`; 1,908 keys of the Hobbit M closure have more than one defining table) | BIN. Settles ST T18; corrects the ST §8.2 Decision. |
| R18 | `Script_MaxNumInstances`: an integer ≥ 2 allows that many concurrent runs of the key on one entity, anything else allows 1; a start beyond the limit is refused, not queued or restarted (§12.3) | BIN3; OBS (values in 1,155 sets) | BIN |
| R19 | Run node: Persistent 0 completes once the script is started; Persistent 1 stays running, forwards its intensity, and stops the script when it is stopped unless `UnbindOnDeath`; Stop 1 stops every run of the key on the target (§12.4) | BIN3; OBS (1,032 Run nodes in scope) | BIN |
| R20 | WhenRunning: Running port while any listed key runs on the target, else NotRunning; a branch is (re)started only when its group is idle; Persistent re-evaluates on script start and stop (§13) | BIN3 | BIN |
| R21 | SetProperty with `ServerOnly` = 1 does nothing in the client (its process mode lacks the server bit) (§14) | BIN3 (gate; the client sets its mode word to a value without that bit at start-up); OBS (25 such nodes in scope, all on render properties of emitters) | BIN |
| R22 | Goto starts the one Label of the same name found in the same record and script, as its own child, and stops nothing; the Label port payload is a Collection ordinal (§15) | BIN3; OBS (279 Goto nodes in tables, each with exactly one Label of its name in the same script) | BIN |
| R23 | OnStop runs its Default children when it is stopped (not on teardown); children that would stay running are stopped at once (§16) | BIN3 | BIN |
| R24 | ScriptletParameter binds to the nearest enclosing Scriptlet instance that invoked the record holding the parameter; names compare **ignoring case**; an unbound parameter runs nothing (§17) | BIN3 | BIN. Settles ST §7.8 OPEN (case); corrects ST §7.8 ("its own Default port runs instead"). |
| R25 | One scheduler, advanced once per frame to the frame time; due events fire in time order with the clock set to each event's time; ties in insertion order; per-entity frame updates follow (§20) | BIN3 | BIN |
| R26 | No guard against synchronous Goto recursion; looping Timers are paced to ≥ 0.1 s per cycle and Sequences to ≥ 0.1 s per pass; nested changes of one property are cut off at the second re-entry (§21) | BIN3; OBS (one table script with self-reaching labels, 0x07000031/0x10000AAC, gated by two When nodes) | BIN |
| R27 | A Switch (and a When) reads an unset property as its dictionary default when the definition carries one (flag 0x800), else as the zero value of its type; a Switch has **no value** only when the property ID has no dictionary definition. The value then goes through the normal case match, with the first Default port when no case matches (§6.2) | BIN-R (the Switch's property read, the value it starts from, the port choice); OBS-R (the 65 `Music_Monitor` and 1 `HousingPropertyHook_IntMusic` Switches of script 0x10000039) | BIN; agrees with `script-table.md` T28. Settles OPEN-5. |
| R40 | The Random node's draw comes from the client's single global generator (a combined multiplicative congruential generator with a 32-entry shuffle table), which is seeded once at client start from the wall-clock time in whole seconds and has one other seeding caller (apparently a game-logic bytecode operation, Inferred); nothing in the node's path seeds it per entity or per object (§8) | BIN-O | BIN (numbered from R40 to stay clear of parallel revisions) |
| R41 | While: the test (the watched property's value, prepared as for a Switch, equal to a `Values` entry or to the current value of a `Names` property; reversed by `Inverse`), and the start: the children of the first Default port start only when the test holds, else the node completes at once (§30.2, §30.3) | BIN-W | BIN |
| R42 | While: re-evaluates on every change of its property on the target and of its `Names` properties on the running entity; while the test holds nothing happens (a finished branch is not restarted, the node does not arm); the first time it fails the branch is stopped with reason 0 and the node completes (§30.4) | BIN-W | BIN |
| R43 | Census of the 41 While nodes: properties, values, `Names` empty and `Inverse` 0 in all, one Default port each (§30.5) | OBS-W | Confirmed |
| R44 | PhysicsImpulse at start: the impulse is sent as a request and the target is added to the instance's pending work; the node does not arm in its start (§30.6). **Corrected 2026-10-09 (BIN-I):** the targets come from target port types 8 and 19, not 25, and only the first entity of each is used, so there is one request (R48) | BIN-W; BIN-I | BIN; completion answered by R51 |
| R45 | The avatar's jump scripts: `Movement_ActionKey` (key 9) gates and runs `Loc_JumpKeyPressed`, which gates and runs `Loc_Jumping` (Persistent 1); `Loc_Jumping` sets `Movement_IsJumping` true, then a While on it runs a one-pass Sequence (impulse and a persistent Run of `Jump`; a 0.2 s Timer; `Movement_IsJumping` false) (§30.7). Per-script sets: `Loc_JumpKeyPressed` and `Loc_JumpKeyReleased` carry `Script_ClientAuthorative` true, `Script_ReplicateToClient` false and `Script_MaxNumInstances` 2 in every table that defines them; `Loc_Jumping` only `Script_MaxNumInstances` 2; `Movement_ActionKey` none | OBS-W | Confirmed (data); the client code that starts key 9 not traced |
| R46 | In the client's code the property ID of `Movement_IsJumping` occurs once, as a read in the controller; no code that writes it was found, and no script other than `Loc_Jumping` sets it for the player. The only other setters are six definitions of `Jump` (key 1) in tables outside the avatar's class closures: true in step 1 of a root Sequence, false in the child of a root OnStop (§30.7, OPEN-19) | BIN-W; OBS-W | BIN (search for the constant); Confirmed (scripts) |
| R47 | Node start (§3.1, §3.2): a new instance holds its parent link from its creation, before its kind start runs; it joins its parent's child list and its port group only after the kind start (step 7). Each start into a port group, and each stop of the group, advances a counter of the group; a new instance whose group counter moved during its start is stopped at once with reason 0 and the start counts as failed; one whose group was already stopped when its start began is stopped with the group's reason; one whose parent or an ancestor stopped meanwhile is stopped with reason 0. The runner's check after each child (§3.2) walks the parent links of the instance being filled, so it sees a stopped ancestor of an instance that is still starting, but not a stopped port group. A start that fails by the group counter ends the runner that made it (refines R2); kind-start failures and skips do not | BIN-I (the generic node start, the port runner, the stop test, the group stop, the instance constructor) | BIN. Answers §29 item 19 |
| R48 | PhysicsImpulse targets: a target of port type 8 (`ImpulseTarget`) names the entity that receives the impulse and one of port type 19 (`Impulser`) the entity whose frame it is given in; each defaults to the running entity; when such a target is present, its first resolved entity is used; when it resolves to none, nothing is sent (logged) and the instance stays running, unarmed, until it is stopped from above. The node class declares exactly these two target ports, each at most once, and no output port (§30.6 steps 2–3) | BIN-I (the start, the class's port declaration list) | BIN |
| R49 | The impulse request carries the vector, `ImpulseType`, `IsJump`, a "world frame" flag, a deferral flag (bit 5 of the starting context's flags) and a time (the scheduler's current time plus the context's synchronisation offset). Target = impulser: the vector is `Impulse` × intensity, or the jump vector, in the target's own frame, and `IsGlobalSpaceImpulse` is not read. Otherwise the node first asks the impulser and, on its answer, sends `Impulse` × intensity turned by the impulser's orientation into the world unless `IsGlobalSpaceImpulse` is 1, flagged as world frame (§30.6 steps 4–5) | BIN-I (the start, the impulser-answer handler, the request's field list, the field bindings of the node class) | BIN |
| R50 | A target on this client applies the request when it receives it, unless the deferral flag is set and the request's time lies after the scheduler's current time on a target with a synchronisation flag (not identified); then it applies it at that time. A world-frame vector is turned into the target's frame. Applied or not (no mover, or the jump rule of `collision.md` §11.10 item 2), the target then sends a completion notice to the starting entity (§30.6 step 6) | BIN-I (the request's processing, the deferred path, the apply routine, the notice) | BIN |
| R51 | Completion: the completion notice removes the target from the instance's pending work and arms it; with no child it completes (reason 0). A target applied at once on this client answers within the same call, so the node completes in the call that started it (§30.6 step 7) | BIN-I (the notice handler, the arm routine) | BIN. Answers OPEN-20 |
| R52 | Census of the 37 PhysicsImpulse nodes (31 in tables, 6 in scriptlets): `ImpulseType` 2 and `IsGlobalSpaceImpulse` 0 in all; 18 with `IsJump` (none with a target); 6 with targets (1 with an `ImpulseTarget` only, 5 with an `ImpulseTarget` and an `Impulser`), all with target types This (13) or Property (3); no node has a port (§30.6) | OBS-I | Confirmed 2026-10-09 |
| C1 | Census of the scope (§0): kinds and field values of Switch, When, Random, Sequence, Timer, Run, WhenRunning, SetProperty, Goto/Label, OnStop, driven pairs (tables in §6–§16) | OBS | Confirmed |

---

<a id="s2"></a>
## 2. Run-time objects

<a id="s2-1"></a>
### 2.1 What exists at run time (BIN)

| Object | Created by | Holds | Ends |
|---|---|---|---|
| run | a script start (§12.1) | the script key, the entity, a root context, the root instances, a state (running, stopping, stopped) | when its last root instance stops, or when it is stopped (§12.4, §19) |
| instance | a node start that does not complete at once | parent, node (a per-start copy when the node has driven fields, §5.2), context, list of running children, flags *armed* and *stopped*, kind state | when it stops (§3.5); completion is a stop with reason 0 |
| context | each script start; copied whenever a node changes it | intensity; entity; run-parameter set; flags; queuing type; a synchronisation time offset | with its last user |
| port group | Switch, When, WhenRunning, Sequence (current step), ForCurrentValue | the instances started from one port | stopped as a whole |

The default flags of a new context are binary 1010: bit 1 (cleared for scripts started by a
Run node) and bit 3, the **persistent-node permission** (§3.1 step 1). A Run node's script
inherits the Run's flags and gets bit 3 set when the Run has `Persistent` = 0. Every run in
this spec's scope therefore carries bit 3 (BIN3; how a context without it arises, for example
for scripts replayed from a server, was not traced).

<a id="s2-2"></a>
### 2.2 The node copy

When a node has driven pairs or scriptlet-driven pairs (`script-table.md` §4.2 fields 1–2), each
start makes a private copy of the node, writes the driven values into the copy (§5.2) and runs
the copy. Every instance created from that start reads its fields from its copy; a later change
of a source property writes into the same copy (§5.3). Nodes without driven pairs run on the
shared definition.

---

<a id="s3"></a>
## 3. Node life cycle (BIN, every kind)

<a id="s3-1"></a>
### 3.1 Starting one node

A parent starts a child node with the following steps (the parent is an instance, or the run
for a root). The numbers are kept for reference; the client applies them in the order **3, 4,
1, 2, 5, 6, 7** (R1): the same-node rule comes first, and the persistent-node gate and the
probability draw read the node copy, so a driven `Probability` counts (§5.2).

1. **Persistent-node gate.** If the node is "persistent" — a Switch or When with
   `Persistent` = 1, a WhenRunning with `Persistent` = 1, a Timer with `LoopForever` = 1 — and
   the context lacks the persistent-node permission (§2.1), the node is **skipped**. (Always
   passes in scope.)
2. **Probability.** If `Probability` ≠ 1.0, draw *u* uniformly in [0, 1]; the node is skipped
   unless *u* ≤ `Probability`. A node with `Probability` 0 practically never runs.
3. **Same node under the same parent.** If the parent already has a running child instance of
   this same node, the start is **skipped** (one exception, a context mode not used in scope,
   OPEN-1). Consequence: a looping Timer whose port fires again while the child started by the
   previous cycle is still running does not start a second copy (§11.5).
4. **Copy and driven values** (§2.2, §5.2).
5. **Intensity** (§4.1). If it differs from the parent context's intensity, the node gets its
   own context copy with the new intensity.
6. **Kind start.** The kind's start routine runs (sections 6–17). It either leaves an
   instance or none.
7. **Registration.** If an instance was left and it is not stopped, it is added to the
   parent's list of running children (and to the port group, when the start came through one).
   If the port group was stopped while this child was starting, the new instance is stopped
   at once with the group's reason. If the parent itself was stopped meanwhile, the new
   instance is stopped at once (reason 0).
   *Precisely (revised 2026-10-09, R47):* the instance is created with its parent link before
   step 6, but joins the parent's list and the port group only here. The checks, in order:
   (a) the parent or any of its ancestors has been stopped: the new instance is stopped
   (reason 0) and the port group is stopped too; (b) the port group received another start or
   was stopped while this child was starting (each start into a group and each stop of it
   advances a counter of the group): the new instance, if any, is stopped (reason 0) and the
   start counts as **failed**, even when the node left no instance; unlike a failed kind start,
   this failure ends the port runner that made the start (§3.2); (c) the group had already been stopped when this start began: the
   new instance is stopped with the group's reason; (d) otherwise it is registered.

Skipped (steps 1–3) and "no instance" (step 6) are both **completion at once** for the
parent. A kind start that fails (for example a Sound node with a null sound info,
`sound.md` §9.2 step 1) leaves no instance and is not reported (`sound.md` §9.6).

<a id="s3-2"></a>
### 3.2 Starting the children of a port

The children of one port are started one after another in stored order (§3.1 each). After
each child the runner checks whether the instance it is filling, or any ancestor of it, has
been stopped; if so it starts no further child of that port. This is how a child that changes
a property its own Switch watches (§14) cuts the rest of its port short. Probability skips and
failed starts never stop the runner.

*Precisely (revised 2026-10-09, R47):* the runner also stops after a child whose start failed
by §3.1 step 7 (b), the port group having received another start or been stopped during that
child's start. This is the path by which the case above is cut short: the Switch's
re-evaluation stops the case's group and starts the new case into it, so the start of the
child that set the property reports that failure. Kind-start failures (a Sound with a null
sound info and the like) and skips do not stop the runner.

The check follows parent links, which an instance has from its creation (§3.1 step 7, R47).
So it also sees an ancestor that stopped while the instance being filled is itself still in
its start and not yet registered. It does **not** see a stop of the port group that the
instance being filled is being started into: in that case the instance's remaining children
still start, and the instance is stopped together with them when its own start returns
(§3.1 step 7 (b)). Example: a Collection in a Switch case whose first child sets the Switch's
property starts its other children as well (a Collection has no port group of its own, so
nothing reports a failure to its runner); they are then stopped with the Collection, whose
own start reports the failure and ends the Switch's runner. In a
While (§30.4) the outcome differs, because the While itself completes when its group is
stopped and empty, so the Collection's runner finds a stopped ancestor.

<a id="s3-3"></a>
### 3.3 Running

A running instance waits for one of: a child stopping (§3.4), a scheduler event (§20), a
property change it listens to (§5.3, §6.6), a script start or stop it watches (§13), a
per-frame update (§20.2), or a stop from above (§3.5). Kinds that never need any of these
(Collection, Random, Label, Goto, Scriptlet, ScriptletParameter) are pure containers: they run
exactly as long as their children. A Run with `Persistent` 1 runs until it is stopped (§12.4).

<a id="s3-4"></a>
### 3.4 Completion

1. When a child stops (for any reason), the parent removes it from its list (the last entry
   takes the freed place, so list order is start order only until the first removal), lets
   its kind react (a Sequence and a Switch waiting on a transition are the kinds that do,
   §6.9, §10.3), and then, if the parent is **armed** and has no running child left and no
   pending deferred work, stops itself with the child's reason.
2. A kind arms its instance when its own work is done: Collection, Random, Label, Goto,
   Scriptlet, ScriptletParameter right after starting their children; Sequence after its last
   step was started; Timer after its last cycle; a non-persistent Switch/When/WhenRunning after
   its one evaluation; a Run with Persistent 0 or Stop 1 after its requests were answered.
3. A container whose start leaves no running child is released at once: it **completes at
   once**.
4. All of this is one synchronous call chain. A Sound that ends (`sound.md` §6.4) stops its
   instance; its Collection completes in the same call; the Sequence above it starts its next
   step in the same call (unless §10.4 defers it); and so on up to the run. **Completion
   propagates within the same frame**, at the moment the triggering event is handled.

<a id="s3-5"></a>
### 3.5 Stopping

Stopping an instance with a reason (BIN3), in this order:

1. If it is already stopped, nothing happens. Otherwise mark it stopped.
2. The kind's stop hook runs with the reason: a tracked Sound stops its voice with its
   ramp-out (`sound.md` §6.5; the reason does not matter); a SoundEnvironment unregisters its
   environment; a Timer, Sequence or Switch cancels its scheduler events; a Run with
   `Persistent` = 1 and `UnbindOnDeath` = 0 stops the script it started (§12.4); an OnStop
   runs its children (§16).
3. Every running child is stopped with the same reason, in list order (§3.4 item 1).
4. Property listeners and script watches are removed; per-frame updates end.
5. The run is told, then the parent is told (§3.4 item 1).

Reasons: 0 for completion, case changes, Run Stop, WhenRunning flips and ordinary stops; 2
for teardown, on which OnStop does nothing and a Sequence does not advance (§10.3). Which
client paths use reason 2 was not traced (OPEN-2).

> **OPEN-1:** The same-node rule of §3.1 step 3 is bypassed when the context's mode word is 3.
> Where that mode is set was not traced; no script start in scope sets it.
> **Decision (Tessera):** always apply the rule.

> **OPEN-2:** Which stops carry reason 2 (teardown). **Decision (Tessera):** use reason 2
> only for entity removal (§19) and client shutdown; everything else uses reason 0.

---

<a id="s4"></a>
## 4. Intensity (BIN; settles `instruments.md` Q8)

<a id="s4-1"></a>
### 4.1 At start

*i* = *factor* × *p*, where *p* is the parent context's intensity when `UseParentIntensity` = 1,
else 1.0, and *factor* is §4.2 computed on the (possibly driven, §5.2) node. A root's parent
intensity is the run's context intensity: 1.0 for scripts the client starts, the Run node's
intensity for scripts a Run node starts (§12.4).

<a id="s4-2"></a>
### 4.2 The factor

- `IntensityMultiplierVariance` exactly 0: *factor* = `IntensityMultiplier`, **unchanged and not
  clamped**.
- Otherwise: *factor* = clamp(`IntensityMultiplier` + *u*, 0, 1) with *u* uniform in
  [−variance, +variance].

This is the reading of `instruments.md` §10.1 (BIN-R) and contradicts `sound.md` §9.3 ("clamp
always"). All three places that compute a factor (node start, propagation to children, driven
multiplier change) use this one rule. In scope, variance ≠ 0 on 1,196 Sound, 401 Random,
5 Scriptlet and 1 Timer nodes; one Sound node has variance 0 and multiplier 400,000 (its
intensity is not clamped; its voice volume is, `sound.md` §9.2 step 7).

<a id="s4-3"></a>
### 4.3 Propagation

When an instance's intensity changes to *I*, every running child with `UseParentIntensity` = 1
gets *I*′ = *factor*(child) × *I*, with a **fresh** draw when the child's variance ≠ 0; the
child takes *I*′ only when |*I*′ − its current intensity| > 0.0002, and then propagates
further. Children with `UseParentIntensity` = 0 keep their intensity. A tracked Sound instance
sets its voice volume to its new intensity (`sound.md` §9.3). A Run instance forwards its new
intensity to the script it started (§12.4). A Timer whose intensity falls to ≤ 0.0002 skips its
ports from the next scheduled event on (§11.3).

<a id="s4-4"></a>
### 4.4 A driven `Script_IntensityMultiplier` changes

When the source property of a driven `Script_IntensityMultiplier` (PID 0x3A3) changes, the
instance writes the new value *m* into its node copy and then, **only if its
`UseParentIntensity` = 1**, sets its intensity to *factor* × (*factor* × *p*): the factor is
applied twice (*m*² × *p* when the variance is 0), *p* being the parent's current intensity
(1.0 for a root). With `UseParentIntensity` = 0 nothing changes. The usual 0.0002 threshold and
the propagation of §4.3 follow.

Example (music, `sound.md` §10.6): a track Collection whose multiplier is driven by
`Snd_SubMix_MusicVolume` starts at *v* × *p*; after the master fade sets the volume to 0.5 its
intensity is 0.25 *p*, at 0.05 it is 0.0025 *p*, and back at 1.0 it is *p*. The 1,546 driven
Collections in scope all have `UseParentIntensity` = 1.

---

<a id="s5"></a>
## 5. Driven properties at run time (BIN)

<a id="s5-1"></a>
### 5.1 Where a source value comes from

A property is a **script-parameter property** when its dictionary definition has
`field_b` = 6 (`property-system.md` §2.2; 390 definitions: script and callback parameters,
callback outputs, signal arguments, `Music_Note`, `Combat_AnimationSpeedMultiplier` …). The
value of a script-parameter property is looked up in the context's run-parameter set; every
other property is looked up on the entity the script runs on. The run-parameter set is
filled by the script's starter (Run parameter entries, §12.4), by every enclosing Scriptlet
node's parameter set (§17), and by the client for scripts it starts with parameters.

<a id="s5-2"></a>
### 5.2 At start

For each driven pair (field PID, source PID) of the node: if the source has a value, that value
is written into the node copy's field; if it has none, the field keeps its stored value. For
each scriptlet-driven pair (field PID, parameter PID): if the nearest enclosing Scriptlet
instance that invoked this node's record (§17) has the parameter PID in its parameter set, that
value is written into the field. The start then uses the copy (intensity, probability, kind
fields).

<a id="s5-3"></a>
### 5.3 Afterwards

Every instance created from a node with driven pairs listens to each source property **on the
entity** (script-parameter sources do not change during a run). On a change of a source:

1. The new value is written into the node copy's field. Every instance field is read from the
   copy when it is next used, so **every field follows**.
2. If at least one field changed, the kind's "fields changed" hook runs: a Timer reschedules
   its next event with the new `Speed` × `SpeedMultiplier` (§11.4); a tracked Sound, an
   animation and other effect kinds apply their own rules (`sound.md` §9.2 reads its fields at
   start; whether a playing voice follows a later `SoundScript_*` change was not traced,
   OPEN-3).
3. If the field was `Script_IntensityMultiplier`, §4.4.
4. Re-entry guard (§21).

The change is handled synchronously inside the property change (§20.3).

In scope, driven pairs are: `Script_IntensityMultiplier` ← `Snd_SubMix_MusicVolume` on 1,546
Collections, 2 Randoms and 1 Sequence; `Script_IntensityMultiplier` ←
`Script_IntensityMultiplier` (the entity's own value; not a script-parameter property) on 20
Collections and 5 Sound nodes; animation
speeds and start times, MeshFX entities and holding locations, compositor colours and a few
others on emitter effects; 68 scriptlet-driven `AnimationScript_LocomotionDisplacement` pairs.

> **OPEN-3:** Whether a tracked Sound applies a later change of a driven `SoundScript_*` field
> (sound info, pitch, distances) to its playing voice. None of the 31 Sound nodes with driven
> fields (`sound.md` §9.1) is in the music or ambience graphs. **Decision (Tessera):** read
> Sound fields at start only; follow intensity changes (§4.3).

---

<a id="s6"></a>
## 6. Switch (12) (BIN)

<a id="s6-1"></a>
### 6.1 Fields and port fields

Node: `SwitchScript_Property` (PID), `…_Persistent`, `…_Autonomous`, `…_Synchronized`.

Port fields. The client's field lists for the Switch family of ports: the value port has four
fields of its own (the case values, a list of property IDs, a reserved value, the operator); the
Default port has none of its own; both inherit `SwitchScriptPort_RampOut` and
`SwitchScriptPort_Delay` (in that order) from their common parent; the transition port has the
from and to values, `SwitchScriptPort_FromIsDefault` and `…_ToIsDefault`, and inherits
`SwitchScriptPort_Delay`, `SwitchScriptPort_Wait`, `SwitchScriptPort_RampIn` and then the common
parent's RampOut and Delay. Matching these lists to the
payloads of `script-table.md` §4.3 by type, a port payload holds the class's own fields first
and then each parent's, each in field-list order:

| Payload (`script-table.md` §4.3) | Fields |
|---|---|
| switch case (12, 5): PVs; ints; reserved i32; operator; f32; f32 | case values; **property IDs** whose current values are further case values (§6.3); reserved (always 0); operator; **`RampOut`** (the "transition_time"); **`Delay`** (the "second_time") |
| Default (12, 6): f32, f32 | **`RampOut`**, **`Delay`** |
| SwitchTransition (12, 28): PV, PV, bool8, bool8, f32, bool8, f32, f32, f32 | from, to, `FromIsDefault`, `ToIsDefault`, `Delay`, `Wait`, `RampIn`, `RampOut`, `Delay` again (the second Delay is stored over the first) |

The type sequence of all 1,480 transition payloads fits this mapping exactly; the mapping itself
is Inferred (the port reader was not read, OPEN-4). It makes the locomotion templates' first
float (0.2–0.3 s, `script-table.md` §8.4) a cross-fade (§6.8) with no delay.

Census in scope (C1): 2,402 Switch nodes; `Persistent` 1 in 2,253, 0 in 149; `Autonomous` 1 in
2,385; `Synchronized` 0 in all. Value cases: (RampOut, Delay) = (0, 0) in 5,924, (0.3, 0) 110,
(0.2, 0.1) 7, (0.5, 0.3) 2, (1.0, 0) 1, (0.2, 0) 1; operator `Equal` except 2 `AnyBitSet`; no
case has a property-ID list. Default ports: RampOut 0 in 1,392, 0.1 in 11, 0.2 in 7; Delay 0 in all.
837 transition ports, all (FromIsDefault, ToIsDefault, Delay, Wait, RampIn, RampOut, Delay) =
(0, 0, 0.0, 1, 0.0, −1.0, 0.0); their children are SetProperty nodes in 812 of them. No
ForCurrentValue or AnyOtherValue port on a Switch in scope.

<a id="s6-2"></a>
### 6.2 The value

A Switch reads `SwitchScript_Property` from its target entity (no target: the running entity),
or from the run-parameter set when it is a script-parameter property (§5.1). With
`Synchronized` = 1 the evaluation is tied to a time carried by the run (the run's
synchronisation offset added to the current time; 0 for every run in scope).

**Unset properties (R27, settles OPEN-5).** When the Switch starts it prepares the value of
`SwitchScript_Property` from the dictionary: the definition's default when it carries one
(flag 0x800, `property-system.md` §2.2), else the zero value of the property's type (0, 0.0,
false, enum 0, DID 0, property ID 0; a tri-state is 255, "undefined"; other types their empty
value). The value read from the entity (or the run-parameter set) replaces it only when there
is one. The Switch then matches that value like any other (§6.3): a case whose value equals it
is taken, else the first Default port. The Switch has **no value**, and so selects no port, only
when the property ID has no dictionary definition. A When prepares its value the same way (§7);
the "no value" rule is stated for the Switch only. The int-list
properties of a case (§6.3 step 1) are different: those without a value are skipped, with no
default.

Example from the music graph (OBS-R): `Music_Monitor` and `HousingPropertyHook_IntMusic` are
enums without a dictionary default, and no case of their 66 Switches in script 0x10000039 has
the value 0. On an avatar that has neither property the 53 Switches with a Default port take it,
and the other 13 select nothing.

> **Answered (review, 2026-10-08):** OPEN-5 is settled by R27 above; the earlier Decision ("without
> a default the Switch has no value") is withdrawn. The value read agrees with `script-table.md`
> T28. `sound.md` §10.2 ("absent → Default port") holds only when no case matches the default or
> zero value.

> **Answered (review, 2026-10-08):** the clean-room note below is resolved by R27: neither of
> its two rules is exact. Match the default or zero value against the cases first, then take
> the Default port; "no port" applies only to a property ID without a definition.

> **OPEN:** (clean-room implementation, 2026-10-07) This Decision conflicts with
> `sound.md` §10.2 ("a property that is absent on the player makes its Switch take the
> Default port") and with the rule `script-table.md` §16 actually states ("without a default
> the Switch takes its Default port"). On the reference install it silences all music:
> `Music_Monitor` and `HousingPropertyHook_IntMusic` have no dictionary default and the
> walker sets neither, so the Switches above the `Area_Music` Switch select nothing. See §29
> item 1; `tessera-scriptgraph` implements both rules (`UnsetPolicy`).

<a id="s6-3"></a>
### 6.3 Selecting the port

Given a value *v* (none, that is a property ID without a dictionary definition, §6.2 → no port):

1. Visit the ports in port order. For each value port: compare *v* with each listed PV under
   the case's operator (`Equal`; `AllBitsSet`, `AnyBitSet` on bit masks); then with the current
   value of each property in the int list (from the entity, or the run-parameter set for
   script-parameter properties; properties without a value are skipped). The first port with a
   match is selected.
2. If no value port matches, the **first Default port** is selected.
3. Otherwise no port.

Port order is the node's internal port order (OPEN-6); the comparison of values of different
types was not traced beyond integers, enums, booleans and DIDs, which are all the case values
in scope.

<a id="s6-4"></a>
### 6.4 Persistent 0

The Switch evaluates once at start, starts the selected port's children (§3.2) and arms. It
completes when those children are done, or at once when there are none (or no port). It does
not react to later changes.

<a id="s6-5"></a>
### 6.5 Persistent 1

The Switch evaluates at start, starts the selected port's children and **never arms**: it runs
until it is stopped, even when the selected branch has finished or nothing was selected. It
listens to `SwitchScript_Property` and to the int-list properties of its cases on the entity
and re-evaluates on every change (§6.6).

<a id="s6-6"></a>
### 6.6 Re-evaluation (no Delay, no RampOut, no transition port)

On a new value: select the port (§6.3). If it is the **same port** as the current one, nothing
happens: the running branch continues and is not restarted. Otherwise the current port group is
stopped (reason 0; tracked sounds ramp out over their own `RampOutTime`, `sound.md` §6.5) and
the new port's children start at once, in the same call. So a case change is "old branch
ramps out, new branch starts immediately" (`sound.md` §10.5).

<a id="s6-7"></a>
### 6.7 Delay

A persistent Switch uses its *transition* behaviour when any of its value or Default ports has a
Delay or RampOut above 0.0002 s or it has a SwitchTransition port (629 music Switches qualify
through transition ports alone; 19 persistent emitter Switches have Delays or RampOuts). Then,
on a new value:

1. A pending delayed change is cancelled.
2. The transition port for (old value → new value) is looked up (§6.9) and the new value is
   stored.
3. If that transition port has Delay > 0, the change is **deferred** by Delay seconds
   (scheduler, §20): at that time the transition is applied (§6.9) and the case selected for the
   value current then. If its Delay is 0 it is applied now; if it takes over (§6.9 Wait) the
   evaluation ends here.
4. The new case is selected. Same case: nothing happens (any half-done transition is dropped
   and the case's own children carry on at full intensity). No case, or a case whose Delay is
   0: switch now (§6.8). A case with Delay > 0: the switch to it is **deferred by the new case's
   Delay**.

A deferred change that is overtaken by another value change before it is due is cancelled
(step 1): Delay acts as a **debounce**, not as a fade. In scope only 9 emitter cases have a
Delay (0.1 s in 7, 0.3 s in 2).

<a id="s6-8"></a>
### 6.8 RampOut: the outgoing case keeps playing and cross-fades

Switching from the current case *A* to case *B*:

1. If an earlier cross-fade is still running, its outgoing branch is stopped now (reason 0).
2. *A*'s running children become the **outgoing branch**.
3. If *A*'s port has RampOut ≥ 0.0002 s and the outgoing branch still runs, it is kept for
   RampOut seconds (a fade window from *t*₀ to *t*₀ + RampOut, *t*₀ = now; when step 1
   interrupted a cross-fade, *t*₀ is shifted back so that the fade continues from the level it
   had reached). Otherwise it is stopped now.
4. *B*'s children start now.
5. During the window, every frame (§20.2): with *f* = (window end − now) / RampOut, the
   outgoing branch's children with `UseParentIntensity` = 1 get *factor* × *f* × *I*, the new
   branch's children *factor* × (1 − *f*) × *I* (*I* = the Switch's intensity).
6. At the window end the outgoing branch's intensity is set to 0 and it is stopped (reason 0);
   the new branch returns to *factor* × *I*.

So RampOut is an **intensity cross-fade between the old and the new case**. Tracked sounds in
the outgoing branch also apply their own `RampOutTime` when they are finally stopped, starting
from the faded volume. No music or ambience Switch in scope has a RampOut.

<a id="s6-9"></a>
### 6.9 SwitchTransition ports (28)

Matching (old value *o*, new value *n*), in tiers, first hit wins, port order inside a tier:

| Tier | `FromIsDefault` | `ToIsDefault` | Condition |
|---|---|---|---|
| 1 | 0 | 0 | from = *o* and to = *n* |
| 2 | 1 | 0 | to = *n* (any old value) |
| 3 | 0 | 1 | from = *o* (any new value) |
| 4 | 1 | 1 | always |

Comparison is equality. Applying a transition port: its children start (§3.2). Then, if
**`Wait` = 1 and at least one of them is still running**, the transition takes over: the
current case's children become the outgoing branch, cross-faded out over `RampIn` seconds
(§6.8 steps 3, 5 and 6 with RampIn as the window; with `RampIn` 0 its intensity drops to 0 at
once and it is stopped at the next scheduler pass), the transition's children become the current
branch, and the case that matches the new value is **started only when the last of the
transition's children has finished** (the outgoing branch is stopped then, if it still runs).
In every other case (children all completed at once, or `Wait` = 0) the normal case switch of
§6.7 step 4 follows immediately.

In the music graph every transition port has `Wait` = 1, `RampIn` = 0 and children that complete
at once (SetProperty of `Snd_Music_Advance`, `sound.md` §10.4), so each transition just runs its
SetProperty nodes and the case switch follows in the same call.

> **OPEN-4:** The payload-to-field mapping of §6.1 rests on the client's field lists and on the
> type sequence, not on the port reader itself. **Decision (Tessera):** use it. The transition
> port's RampOut (−1.0 in all 1,480) is not used by any routine read.

> **OPEN-6:** Port order inside a node is the order of an internal hash table keyed per port.
> **Decision (Tessera):** stored order. Only the 2 `AnyBitSet` cases in scope could match more
> than one case.

<a id="s6-10"></a>
### 6.10 ForCurrentValue ports (22)

Each time the Switch evaluates a **new value** (at start and on every change), it first stops
the children started by the previous ForCurrentValue run (reason 0) and then starts the
children of the first ForCurrentValue port again, before selecting the case. They run whatever
case matches. When nodes do the same. (Absent in scope; 28 ForCurrentValue ports on Switch
nodes and 24 on When nodes elsewhere.)

<a id="s6-11"></a>
### 6.11 AnyOtherValue (30), Autonomous

AnyOtherValue ports occur only on HeadingCheck nodes in the data (`script-table.md` §4.3); the
Switch and When routines never query that port type (BIN3). `SwitchScript_Autonomous`,
`RandomScript_Autonomous` and `TimerScript_Autonomous` are not read by any start, evaluation,
choice or scheduling routine (BIN3); their use, if any, lies in the client–server replay of
script state, which was not examined.

> **OPEN-7:** The meaning of the three `Autonomous` flags. **Decision (Tessera):** ignore them.

---

<a id="s7"></a>
## 7. When (63) (BIN)

A When is a Switch whose cases are WhenTest ports (`script-table.md` §4.3 "when test"): constant
PV, `rhs_property`, operator. Selection: visit the WhenTest ports in port order and take the
**first** whose test holds, else the first Default port, else none. Only one branch runs at a
time (correcting `script-table.md` §7.2).

| Operator (enum 0x230000A3) | Holds when (*p* = the property's value, *c* = the constant, or the current value of `rhs_property` when that is not 0) |
|---|---|
| 1 `LessThanOrEqual` | *p* < *c* or *p* = *c* |
| 2 `GreaterThanOrEqual` | *p* > *c* or *p* = *c* |
| 3 `NotEqual` | *p* ≠ *c* |
| 4 `LessThan` | *p* < *c* |
| 5 `Equal` | *p* = *c* |
| 6 `GreaterThan` | *p* > *c* |

For an array-valued property *p* is its element count compared with an integer constant.
`Persistent`, value source, re-evaluation and the same-port rule are as for Switch (§6.2–§6.6);
a When re-evaluates on changes of its own property only, not of `rhs_property`. Census: 26 When
nodes in scope, 20 persistent; tests: `GreaterThanOrEqual` 14, `LessThan` 12, `Equal` 6,
`GreaterThan` 5, `LessThanOrEqual` 5; none uses `rhs_property`.

---

<a id="s8"></a>
## 8. Random (17) (BIN)

1. Collect the Default ports. With exactly **one** port, take it (whatever its weight; the
   repeat memory is not touched).
2. Otherwise sum the `u32` weights of all ports, leaving out the port picked last time when
   `RandomScript_NoRepeats` = 1. If the sum is 0, pick nothing and clear the memory.
3. Draw an integer *r* uniformly in [0, sum − 1]; walk the ports (same order, same exclusion),
   accumulating weights, and pick the first port whose running sum exceeds *r*. Remember its
   index.
4. Start the picked port's children (§3.2) and arm. The Random completes when they are done,
   or at once when none is left running (a placeholder Sound, `sound.md` §9.6).

The repeat memory is a single index kept **with the node definition**: it is shared by every
entity and every run that uses the node, and it survives the Random's completion. A Random
with driven fields runs on a fresh copy each start (§2.2), so its memory does not carry over
(Inferred). A new pick happens at every start of the node (each pass of an enclosing Sequence,
each firing of an enclosing Timer port).

Census in scope: 3,182 Random nodes; `NoRepeats` 1 in 3,170; 62 have one port; weights 1
(16,265 ports), 2 (1,576), 3 (885), 4, 5, 6, 7, 9, 10, 20; no port is empty.

**Random source (BIN-O, R40).** The draw of step 3 is the whole part of *u* × sum, with *u* a uniform value between
0 and 1 from the client's **single global generator**: a combined multiplicative congruential
generator with a 32-entry shuffle table (the published "ran2" design). Every random draw of
the client that goes through it advances the same sequence. It is seeded **once at client
start from the wall-clock time** in whole seconds; the only other caller of the seeding
routine takes its seed from what appears to be the game-logic bytecode interpreter's value
stack (Inferred; `wlib-bytecode.md`; when it runs is OPEN-18 below). Nothing
in the Random node's path seeds it from the entity, the object or the node. Consequence
(Inferred): which port a Random picks depends on everything else the client has drawn since
it started, so the same figure created twice (for example on two visits) normally gets
different picks; Tessera cannot reproduce the original's sequence and keeps its own seeding
(Decision of `appearance.md` §7.4).

> **OPEN-18:** When the second caller of the seeding routine runs, and with which seed, was
> not traced: in particular whether game logic reseeds the generator before a figure's
> start-up script runs, which would make its Random picks repeatable. This is the possible
> reseed named in the O12 OPEN of `appearance.md` §7.4. **Decision (Tessera):** as there, a
> placed figure's Random nodes are seeded from its instance ID.

> **Answered (spec writer, 2026-10-09; was OPEN, implementer, 2026-10-09):** the original
> seeds nothing per entity, per object or per node (R40): the Random port pick comes from the
> client's one global generator. Both questions are therefore about
> Tessera's own seeding, and the implementation's choices are confirmed as Decisions:
> placed sound emitters keep the runtime's generator (they are not figures, and the original
> gives them no seed of their own), and a figure's own seed covers only its Random port picks;
> its `Probability` draws (§3.1 step 2) and intensity variances (§4.2) stay on the runtime's
> generator. No client fact is missing.
>
> Was: **OPEN (implementer, 2026-10-09):** `tessera-scriptgraph` draws a Random pick as the
> whole part of *u* × sum with *u* from the runtime's one generator (R40), and offers a
> per-entity seed for the OPEN-18 Decision (an entity spawned with its own seed draws its
> Random picks from its own generator; its other draws, `Probability` and variances, stay on
> the runtime's generator, since the Decision names only Random nodes). The client spawns no
> placed figure into the script runtime yet (random-appearance outfits from start-up scripts
> are not built, `appearance.md` §7.4), and its placed sound emitters keep the runtime's
> generator, as they are not figures. Whether emitters should also be seeded per object, and
> whether the figure seed should cover every draw of the figure, is not stated.

> **OPEN-8:** The order of the internal port table (§6 OPEN-6) decides which port a given
> draw selects; the distribution is unaffected. **Decision (Tessera):** stored order and the
> port's stored position as its index.

---

<a id="s9"></a>
## 9. Collection (2) and Label (25) (BIN)

Collection: collect the Default ports, sort them by their `u32` payload ascending, start each
port's children in that order (§3.2; the start ends early if the Collection is stopped
meanwhile, §3.2), arm. It completes when no child is left; at once if none was started.

A **Label is a Collection with a name**: started in normal flow (as the music graph's silence
branch is at start-up), it does exactly the same; its port payload is the same ordinal. A Goto
may also start it (§15).

Census in scope: 3,517 Collections; one port in 2,381, none in 469, up to 26.

---

<a id="s10"></a>
## 10. Sequence (6) (BIN)

<a id="s10-1"></a>
### 10.1 Steps

The steps are the Default ports, sorted by step number ascending (`u32`). Each step number is a
separate step even when two ports share a number (5 Sequences in scope; their relative order is
not defined, OPEN-6). Other port types are ignored. With *n* steps, `LoopForever` *L* and
`LoopCount` *k*, the run is limited to *n* × *k* step starts (an unsigned 32-bit product), unless
*L* = 1.

<a id="s10-2"></a>
### 10.2 Running the steps

At start, and every time the current step finishes:

1. If *n* = 0: with *L* = 1 the Sequence stays running forever doing nothing; with *L* = 0 it
   arms and completes.
2. While *L* = 1 or fewer than *n* × *k* steps have been started:
   - at the start of a pass (step index 0), apply the pacing rule (§10.4);
   - start the step's children (§3.2) and count the step;
   - if any of them is still running, wait (§10.3);
   - otherwise (all completed at once, or the step has no children) go straight on to the
     next step in the same call.
3. When the limit is reached, arm (complete when the last step's children are done).

So: `LoopCount` 1 = one pass; 0 = **nothing runs**, the Sequence completes at once;
a negative value is read unsigned and loops practically forever; `LoopForever` 1 ignores
`LoopCount`. Census in scope: (LoopForever, LoopCount) = (1, 1) in 2,260, (0, 1) in 1,324,
(0, 15) in 3, (0, 5) in 1; no step without children; 3 Sequences without ports.

<a id="s10-3"></a>
### 10.3 When a step finishes

A step finishes when the **last** of the children started by it stops (a child that stops with
reason 2 is ignored: the Sequence does not advance on teardown). Steps never overlap: the next
step starts in the same call as the last child of the previous one stops (§3.4 item 4).

<a id="s10-4"></a>
### 10.4 Pass pacing

When a new pass is about to begin and the previous pass began less than 0.1 s ago (scheduler
time, §20), the Sequence does not start it now but schedules itself for the time 0.1 s after
the previous pass began (or for the frame time when the clock lags it by more than 1.111 s,
whichever is later) and resumes there. This limits a looping Sequence whose steps all complete
at once to ten passes per second.

---

<a id="s11"></a>
## 11. Timer (65) (BIN)

<a id="s11-1"></a>
### 11.1 Fields

`TimerScript_LoopForever`, `…_Duration` (s), `…_LoopCount`, `…_Autonomous`,
`AnimationScript_Speed`, `TimerScript_SpeedMultiplier`, `…_ToolsAnimationOverride` (editor
only). Speed *s* = `Speed` × `SpeedMultiplier` (both 1.0 on all 1,682 Timers in scope). Ports: the
Default ports, sorted by time ascending.

<a id="s11-2"></a>
### 11.2 The cycle

Cycle length *C* = max(`Duration`, time of the latest port) ÷ *s*, raised to 0.1 s when
`LoopForever` = 1 and lowered to 0 when negative. Within a cycle that began at *t*₀ (scheduler
time), port *j* fires at *t*₀ + time*ⱼ* ÷ *s*, in ascending order, and the cycle ends at
*t*₀ + *C*. Ports at time 0 fire at the next scheduler pass after the start, not inside the start
(§20.1). A port whose time is at or beyond `Duration` fires (it lengthens the cycle); the cycle end
follows it. When a cycle ends the next begins at its nominal end time, so a looping Timer keeps
a fixed rhythm; when the scheduler lags the frame time by more than 1.111 s, whole cycles are
skipped to catch up (their ports do not fire).

<a id="s11-3"></a>
### 11.3 Firing a port; intensity

A port fires by starting its children (§3.2), with the Timer instance as their parent. While the
Timer's intensity is ≤ 0.0002 the ports are not scheduled: only cycle ends are. A looping Timer
whose intensity is ≤ 0.0002 or that has no ports schedules **nothing** (it just stays running);
so does any Timer whose speed is below 0.0002.

<a id="s11-4"></a>
### 11.4 Completion and loops

At each cycle end the cycle count increases. With `LoopForever` = 0 and `LoopCount` ≤ count
(unsigned comparison) the Timer arms: it completes **only when the children started by its
ports have all stopped** (at once if none runs). Otherwise the next cycle begins. So
`LoopCount` 0 and 1 both give one cycle; a negative `LoopCount` loops practically forever.
A change of a driven Timer field (§5.3) reschedules the next event from the current cycle's
start with the new speed.

<a id="s11-5"></a>
### 11.5 Overlap with children

A port's children run independently of the cycle. When the same port fires again (next cycle)
while the child instance it started last time still runs, that child node is **not** started
again (§3.1 step 3); other children of the port start normally.

Census in scope: (LoopForever, LoopCount, Autonomous) = (0, 1, 1) in 1,365, (1, 1, 1) in 308,
(1, 1, 0) in 9; 1,319 without ports; 81 with a port at 0; 29 with a port exactly at `Duration`;
170 with ports beyond `Duration`; durations 15 s (570), 10 s (245), 0 s (185), 20 s (177),
30 s (125) and others. Across all tables, 135 looping Timers have a cycle below 0.1 s (95
without ports, 40 with ports only at time 0); the latter fire every 0.1 s.

---

<a id="s12"></a>
## 12. Run (5) and script starts (BIN)

<a id="s12-1"></a>
### 12.1 Starting a script on an entity

1. Refused if the entity is being destroyed or not yet fully constructed.
2. **Lookup** (§12.2). A key that no table of the entity defines starts nothing (the start
   "fails"; for a Run node, §12.4).
3. **Instance limit** (§12.3). Refused when reached.
4. A new run is created with a context built from the start's parameters (intensity, flags,
   run-parameter set) and registered under its key.
5. **Pre-load.** A script with dependencies (`script-table.md` §2.5) may wait until the
   loader has its assets before its roots start, unless its per-script
   `Script_PreloadDependency` is set. This changes when a script starts, not what it does
   (OPEN-15).
6. The roots start (§3.1), in the order of §12.2: the roots gathered from kind-1 tables, then
   kind-2, then kind-4. If none is left running, the run ends at once.

Script channels (a client feature with its own data record, not examined) can prevent, queue or interrupt a start
according to `RunScript_QueuingType` (enum 0x2300009C: 1 `QueueInterruptable`, 2 `QueueRequeue`,
3 `DoNotQueue`, 4 `QueueNonInterruptable`). Every Run node in scope uses 3.

> **OPEN-15:** The loader conditions under which a run waits for its dependencies.
> **Decision (Tessera):** never wait; start the roots at once (Tessera's loaders tolerate
> assets that are still loading).

> **OPEN-9:** Which scripts belong to a script channel and what each queuing type does. That
> code was not read. **Decision (Tessera):** no channels; every start proceeds as `DoNotQueue`
> on a free channel.

<a id="s12-2"></a>
### 12.2 Lookup through the table closure

For a key, the client walks each table the entity names (class table, then additional tables;
this order is Inferred) and, from each, its includes depth-first in stored order (pre-order).
From **every** table that defines the key it takes the root nodes of that definition, appending those not yet taken
(a table reached twice contributes once). If a definition's per-script property set has
`Script_CombinationType` = 1 (`Overriding`, enum 0x2300016D), the walk does not descend into
that table's includes; `Additive` (2) or no value continues. The roots are kept in three lists
by the defining table's kind (1, 2, 4) and started in that order. The per-script property sets
are merged the same way.

This **replaces** the "first definition wins" Decision of `script-table.md` §8.2 (T18):
there is no precedence; all definitions run together. In scope it changes nothing:
`SND_music_system_startup` is defined once in each avatar closure (in 0x0700042E, reached up
to three times), and 73 of the 74 keys its Run nodes start are defined once (65 in 0x0700042F,
8 in 0x0700042E). Elsewhere it matters: 1,908 of the 5,136 keys of the Hobbit M closure have
more than one defining table, and the closure's 27 `Startup` definitions all run when key 5
starts.

<a id="s12-3"></a>
### 12.3 `Script_MaxNumInstances`

The limit is the integer value of `Script_MaxNumInstances` in the merged per-script properties
when it is ≥ 2, else 1 (absent, 1, 0 and −1 all give 1; −1 occurs in 308 sets). A start while
that many runs of the key are alive on the entity is **refused**: it does not restart the
running one and is not queued. Only the two instrument scripts of 0x0700042F set a limit (10)
in the music and ambience tables (0x0700042E, 0x0700042F), so every music, ambience and reverb
script runs at most once per entity.

<a id="s12-4"></a>
### 12.4 The Run node

Fields: parameter entries, `RunScript_ID`, `…_Persistent`, `…_UnbindOnDeath`, `…_Stop`,
`…_ExposeAsScriptLink`, `…_QueuingType`. Targets: the node's targets, else the running entity.

1. **Stop = 1:** every run of `RunScript_ID` on each target is stopped (reason 0). The Run
   completes at once.
2. **Otherwise** a start request goes to each target: the target's run inherits the Run's
   context (its intensity, flags, run-parameter set), then the Run's parameter entries are added
   (constant PVs, or the current value of a source property), the queuing type is taken from
   the node, and `RunScriptOutput_Source` is set to the starting entity. For the running entity
   the request is handled at once; for another entity it is deferred to the next scheduler pass
   and travels as a message.
3. **Persistent = 0:** the Run completes once its requests are made (fire and forget).
   **Persistent = 1:** the Run stays running **until it is stopped**; it does not complete when
   the script it started ends (no such path was found). Only a request that fails (key
   undefined, limit reached) counts as done, and when every target's request has failed the
   Run completes.
4. While running, a change of the Run's intensity is forwarded to each started run, whose root
   context takes it (§4.3).
5. When a persistent Run instance is **stopped** (its branch stops), it stops the runs it
   started, unless `UnbindOnDeath` = 1. A reply that arrives after the Run was stopped is
   answered with a stop, under the same condition.

So in the music graph a case's `SND_EAX_*` and ambient-bed Runs (`Persistent` 1, `sound.md`
§10.7) keep their reverb and bed alive exactly as long as the case, and an area change stops
them before the new case runs its own (the stop happens first in the same call, so a new case
that runs the same script restarts it, §12.3 allowing it). Census: (Persistent, UnbindOnDeath,
Stop) = (1, 0, 0) in 797, (0, 0, 0) 142, (0, 0, 1) 91, (1, 0, 1) 2; no parameter entries; 5 Run
nodes with a target; `ExposeAsScriptLink` 0 in all.

> **OPEN-10:** What `RunScript_ExposeAsScriptLink` does. **Decision (Tessera):** ignore.

---

<a id="s13"></a>
## 13. WhenRunning (14) (BIN)

The node watches the script keys of its `Scripts` list (and the keys named by the properties of
its `Names` list, empty everywhere) on its target entity (else the running entity). **Running**
= at least one of them has a live run there.

- On evaluation: if Running, the NotRunning group is stopped (reason 0) and, if the Running group
  has no live instance, the Running ports' (type 11) children start into it; otherwise the
  reverse with the NotRunning ports (type 10). A group that is still busy is not restarted; a
  group whose branch has finished is started again on the next evaluation even if the state is
  unchanged.
- `Persistent` = 0: one evaluation, then armed. `Persistent` = 1: re-evaluates whenever a run
  of a watched key starts or stops on the target, and never arms.

Census in scope: 26 nodes; `Persistent` 1 in 22; 24 watch one key, 2 watch six.

---

<a id="s14"></a>
## 14. SetProperty (40) (BIN)

The node sets `SetPropertyScript_PropertyName` on its targets (else the running entity) to its
PV, or to a driven value, and completes at once. The store notifies listeners when the value is
set, so persistent Switches, Whens and driven fields that watch the property react **inside the
SetProperty's start**, before the next sibling starts (§3.2 then decides whether the siblings
still start) (Inferred: the listener path is a direct call; no deferral was found).

`SetPropertyScript_ServerOnly` = 1: the node is skipped unless the process runs in server mode.
The client sets its process-mode word at start-up to a value without the server bit, so these
nodes **do nothing in the client** (25 in scope, all on render properties of emitters:
`Render_AlwaysSelectable`, `Translucent`, `Render_AdjustableScale`).

In scope 1,271 SetProperty nodes; 1,055 set `Snd_Music_Advance`, 81 the submix volumes, 24
`Snd_IsOn`.

---

<a id="s15"></a>
## 15. Goto (61) and Label (25) (BIN)

1. On its first start a Goto looks for Labels with its name in **its own record**: for a table,
   only the nodes of the same script key's definition in the same table; for a scriptlet, all
   its root trees. The result is kept with the node. None found: the Goto is skipped (completes
   at once). More than one: the first is used.
2. The Goto starts that Label node as **its own child** (§3.1, with the Label's own probability,
   intensity and driven fields; the context is the Goto's), and arms. It completes when the
   Label's branch completes.
3. **Nothing stops.** The branch that contains the Goto continues normally; the Label's own
   place in the graph is not involved, and a Label that is running there keeps running. A Goto
   is a call of the Label's subtree, not a jump.

The Label's port payload is the Collection ordinal (§9). In the music graph, the 27 Gotos inside
`Area_Music` cases call the silence Label (a Random over one-shot Timers, `sound.md` §10.1);
18 of them are Sequence steps, whose step then finishes when that silence ends, 8 are result
ports of GameCallback nodes (§18) and one is a Switch Default. All 279 Gotos
in tables have exactly one Label of their name in the same script.

---

<a id="s16"></a>
## 16. OnStop (10) (BIN)

Starting an OnStop leaves an instance that does nothing and never arms. When it is stopped with
reason ≠ 2 (its branch stops, its run stops, its Switch case changes), it starts the children of
its Default ports, as its own children, during its stop. Because it is already marked stopped,
each child that would stay running is stopped at once (§3.1 step 7): fire-and-forget work
(SetProperty, non-persistent Run, fire-and-forget Sound, Run with Stop) takes effect, while a
tracked sound would start and immediately ramp out. On teardown (reason 2) nothing runs.
In scope: 23 OnStop nodes, each holding one Run node.

---

<a id="s17"></a>
## 17. Scriptlet (19) and ScriptletParameter (22) scope (BIN)

**Scriptlet.** Loads its 0x0C record (file 0: completes at once). Copies the context and merges
the node's parameter set into the copy's run-parameter set, replacing earlier values of the same
PIDs; starts each root of the scriptlet in order with that context, as its children; arms.
Inside, the parameters are visible to script-parameter lookups (§5.1) and to scriptlet-driven
pairs (§5.2).

**ScriptletParameter.** Walks up its parent chain to the **nearest Scriptlet instance whose
record is the one that contains this parameter node** (so a parameter inside nested templates
binds to the invocation of its own template, not just the innermost one). None: skipped. Then it
looks for a Default port of that Scriptlet node whose name equals its own name **ignoring
case**; if found, it starts that port's children as its own children, in **its own** context
(the scriptlet's intensity and run parameters), and arms. If no port matches, **nothing runs**:
its own Default port (the file-0 placeholder Scriptlet in the data) is not used.

The children of a bound port belong to the caller's record; a ScriptletParameter among them
binds, by the same rule, to the Scriptlet instance that invoked the caller's record. This is
lexical scoping by record.

The generic start applies a Scriptlet node's parameter values to inner nodes only through the
two routes above; it does not overwrite every inner field whose PID appears in the parameter
set (correcting `script-table.md` §7.8 point 1 for the generic start). Whether the animation
node reads further values from the run parameters itself was not examined (outside this
spec's scope).

---

<a id="s18"></a>
## 18. Kinds an offline runtime does not implement

In the original every kind has a start routine; nothing is a no-op by design. What matters for
the graphs in scope is the **start outcome** the parent sees, because Sequence steps,
Collections and Timers wait for running children:

| Kind (in scope) | Original outcome | Tessera offline |
|---|---|---|
| GameCallback (27; 5 in the music graph, 2 in emitters) | Calls a game package and fires the result port (type 5) matching the returned value (the 5 music ones test quest status and lead to Gotos or Collections) | Decision: fire the result port whose PV equals the node's `DefaultValue`; none: complete at once (OPEN-11) |
| Execute (4), StoryHook (28) | A native callback / a gameplay hook (not traced) | complete at once |
| Animation (36; 187 in emitter scriptlets) | Running until the clip ends, ramps out when stopped (`script-table.md` §6) | Decision: running for the clip's played length ÷ rate (looping: until stopped), whether or not the animator shows it, so Sequence timing is kept; a missing clip completes at once (OPEN-16) |
| Particle (59), MeshFX (41), Decal (1), EntityVFX (3), Light (13), Beam (54), Appearance (58), EditMaterial (60), compositor nodes (33, 44, 53), PlayerPhysics (47), Math (20), ScalarBlend (57) | Effect-specific (not traced) | Decision: complete at once unless the kind is implemented; Math and ScalarBlend as in `script-table.md` §7 (OPEN-17) |
| SoundEnvironment (62) | Running until stopped (`sound.md` §9.4) | implemented |

> **OPEN-16:** Whether the original keeps an Animation instance running for its clip length
> on an entity that is not drawn (the node has `RequiresUpdatingEvenWhenCulled`). **Decision
> (Tessera):** as in the table.

> **OPEN-17:** How long the original keeps each effect kind of the table running (most
> effects with a `Persistent` field presumably until stopped). In scope these kinds occur only
> in the emitter graphs, never in the music or ambience graphs.
> **Decision (Tessera):** as in the table.

> **OPEN-11:** How the original evaluates a GameCallback and which port it fires. The game
> package code (WLib) was not traced and may depend on server state. **Decision (Tessera):** as
> in the table; an owner observation can tell whether quest-dependent music variants appear.

The While node (50) and the start outcome of PhysicsImpulse (9), which the avatar's jump
scripts use, are specified in §30 (2026-10-09).

---

<a id="s19"></a>
## 19. Entity removal

The client refuses new script starts on an entity that is being destroyed (BIN3) and the
entity's runs end with it (Inferred: the run objects belong to the entity's script component).
Stopping a run stops its root instances and therefore every instance below (§3.5): tracked
sounds ramp out (their position stops following the gone entity, `sound.md` §9.2 step 6),
SoundEnvironments unregister, Runs with `Persistent` 1 stop the scripts they started on other
entities, scheduler events are cancelled.

> **OPEN-12:** The stop reason used when an entity is destroyed, and whether fire-and-forget
> loops it started end too (`sound.md` §9.2 OPEN). **Decision (Tessera):** stop every run of the
> entity with reason 2 (no OnStop children), then end fire-and-forget voices owned by it.

---

<a id="s20"></a>
## 20. The update clock and the order within a frame (BIN)

<a id="s20-1"></a>
### 20.1 The scheduler

One scheduler serves all entities. It holds a current time *now* (seconds, double precision),
the target time of the current frame, and a queue of events, each with a due time, an owner
and a tag; an owner has at most one pending event per handle, and re-scheduling replaces it.
Events are ordered by due time, ties by insertion order.

**Advance to frame time *T*** (once per frame): while the earliest event is due at or before
*T*, remove it, set *now* to its due time (never backwards), and deliver it; events scheduled
during a delivery with a due time ≤ *T* are delivered in the same advance. Then *now* = *T*.
Scheduling never delivers at once: an event due "now" waits for the next advance, or is
delivered later in the current one if the current code runs inside it.

Users: Timer ports and cycle ends (§11), Sequence pass pacing (§10.4), Switch Delays and
cross-fade ends (§6.7–§6.9), Run requests to other entities (§12.4).

<a id="s20-2"></a>
### 20.2 One frame

1. The scheduler is advanced to the frame time (§20.1). All Timer, Sequence, Switch-delay and
   deferred-Run events of the frame fire here, in time order.
2. Each entity's frame update then runs its per-frame instances with the frame's time step:
   Switch cross-fades (§6.8) and the per-frame work of effect kinds (animation, voices'
   positions in `tessera-audio`).
3. Property changes made by game code (movement, area and music properties, `sound.md` §10.3)
   are handled whenever they are set, synchronously (§20.3); in Tessera they are set before
   step 1 (Decision, §22 step 2).

> **OPEN-13:** The source of the frame time (a real-time clock or a game clock that can pause),
> the order in which entities are updated in step 2, and when the game's own property changes
> happen relative to step 1. **Decision (Tessera):** a monotonic real-time clock in seconds
> since client start, not paused; entities in creation order; game property changes before
> step 1.

<a id="s20-3"></a>
### 20.3 Synchronous reactions

Starting, completing and stopping, intensity propagation, property-change reactions (Switch,
When, driven fields), WhenRunning reactions and Run starts on the running entity are all
handled in the call that triggers them. Only the users of §20.1 and the cross-fades of §6.8 are
deferred.

---

<a id="s21"></a>
## 21. Guards against runaway graphs (BIN)

| Risk | Guard in the original |
|---|---|
| Looping Timer with a zero-length cycle | cycle raised to 0.1 s (§11.2); a looping Timer without ports or with intensity ≤ 0.0002 schedules nothing (§11.3) |
| Looping Sequence whose steps all complete at once | at most one pass per 0.1 s (§10.4) |
| Scheduler falling behind (a long frame) | Timers skip whole cycles and Sequences resume at the frame time when the lag exceeds 1.111 s |
| A property change that re-triggers itself | each instance tracks the property IDs it is handling; one property may be handled at most two levels deep in one instance, and a third nested change of it is dropped with a warning |
| Goto loops, recursive Run or Scriptlet chains | **none found**: a Goto that reaches its own Label synchronously recurses without bound; Run recursion is bounded only by `Script_MaxNumInstances` (a running key cannot start again on the same entity) |

In the data, one table script (0x07000031, key 0x10000AAC) has Labels whose subtree reaches a Goto
of the same name, through a Collection and two When nodes. None is in scope.

> **OPEN-14:** Whether Tessera should reproduce the unbounded synchronous recursion of the
> original (no graph in scope recurses). **Decision (Tessera):** add a defensive bound of 256
> nested synchronous starts per call chain (as `script-table.md` §4.5 suggests for decoding);
> beyond it a start completes at once and is logged.

---

<a id="s22"></a>
## 22. Algorithms in words (Tessera evaluator)

1. **Per entity:** keep its tables (`script-table.md` §8.1), its property store, its runs by key,
   and the instances that need frame updates.
2. **Per frame:** apply the game's property changes for the frame (player position → area music
   properties, movement sliders …); advance the scheduler (§20.1); run frame updates (§20.2).
3. **Entity creation:** start `Startup` (key 5, as the `script-table.md` §8.2 Decision; one
   start runs the definitions of every closure table, §12.2) and each start-up script name, with
   default context (intensity 1.0, flags with the persistent-node permission), §12.1–§12.2.
4. **Node start:** §3.1; kind start per §6–§18; registration per §3.1 step 7.
5. **Instance bookkeeping:** list of running children, armed and stopped flags, kind state;
   completion and stop per §3.4–§3.5, synchronous.
6. **Intensity:** §4 exactly, including the double factor of §4.4.
7. **Driven fields:** per-start node copy, listeners, §5.
8. **Entity removal:** §19.

---

<a id="s23"></a>
## 23. Invariants

| # | Invariant | Kind |
|---|---|---|
| RI1 | A node instance never has two running children created from the same node (except in context mode 3). | rule (§3.1) |
| RI2 | An instance that is stopped has no running child once its stop returns. | rule (§3.5) |
| RI3 | A persistent Switch, When or WhenRunning instance never completes by itself. | rule |
| RI4 | A Sequence never has children of two steps running at once. | rule (§10.3) |
| RI5 | A looping Timer's port events are at least 0.1 s apart per port. | rule (§11.2) |
| RI6 | A non-looping Timer completes no earlier than both its cycle end and the end of every child it started. | rule (§11.4) |
| RI7 | With variance 0 an instance's start intensity is exactly multiplier × parent intensity (no clamp). | rule (§4.2) |
| RI8 | At most `max(1, Script_MaxNumInstances)` runs of a key live on one entity. | rule (§12.3) |
| RI9 | Every Goto in a table has exactly one Label of its name in the same script (279 of 279). | Confirmed |
| RI10 | Every transition port in scope has `Wait` 1, `RampIn` 0, `Delay` 0 and no wildcard (837 of 837). | Confirmed |
| RI11 | No music or ambience Switch in scope has a value or Default port with Delay or RampOut ≠ 0. | Confirmed |
| RI12 | `SND_music_system_startup` has exactly one defining table in each of the 14 avatar closures. | Confirmed |
| RI13 | A PhysicsImpulse start sends at most one impulse request, whatever its targets resolve to (added 2026-10-09). | rule (§30.6) |
| RI14 | A child started while its port group receives another start or is stopped never stays registered: it is stopped when its start returns, and the runner that started it starts no further child (added 2026-10-09). | rule (§3.1 step 7 (b), §3.2) |
| RI15 | Every PhysicsImpulse node of the install has `ImpulseType` 2 and `IsGlobalSpaceImpulse` 0, and only targets of port types 8 and 19 (37 of 37; added 2026-10-09). | Confirmed |

---

<a id="s24"></a>
## 24. Worked synthetic examples (for tests)

All DIDs, keys and sound infos are synthetic. Times in seconds of scheduler time; "frame" means
a scheduler advance at 60 Hz unless stated.

<a id="s24-1"></a>
### 24.1 A music program across an `Area_Music` change

Graph: root = persistent Switch on `Area_Music` (no Delay, no RampOut), cases *A* and *B*. Each
case = Sequence (`LoopForever` 1) with step 1 = Collection (multiplier driven by
`Snd_SubMix_MusicVolume`, `UseParentIntensity` 1) holding one tracked Sound (`RampOutTime` 4,
payload 60 s), step 2 = Random (two ports, weight 1, `NoRepeats` 1) over one-shot Timers of
10 s and 20 s (no ports). `Snd_SubMix_MusicVolume` = 1.0.

| Time | Event | State |
|---|---|---|
| 0 | `Area_Music` = *A*; the Switch starts case *A*: the Sequence starts pass 1, step 1; the Collection starts at intensity 1.0; the Sound starts | track *A* at 1.0 |
| 60 | the voice ends; Sound instance stops; Collection completes; step 1 finished; step 2 starts in the same call: the Random picks the 10 s Timer, say | silence |
| 70 | the Timer's cycle end fires; the Timer completes; the Random completes; step 2 finished; pass 2 begins (60 s+ after pass 1, no pacing): step 1 starts the track again | track *A* |
| 100 | `Area_Music` = *B*: same port? no → case *A*'s group stopped: Sequence → Collection → Sound stopped, the voice ramps out linearly over 4 s; then case *B*'s Sequence starts its step 1 in the same call | track *A* fading, track *B* at 1.0 |
| 104 | track *A*'s voice reaches 0 | track *B* |
| 130 | `Area_Music` = *B* again (unchanged value, or set again) | nothing restarts |

In the second silence of case *A* (had it run) the Random could not pick the 10 s Timer again
(`NoRepeats`), so it would have picked the 20 s one; this memory is shared with every other
entity running the same node.

<a id="s24-2"></a>
### 24.2 Master fade during the program (§4.4)

Continue 24.1 at 110 s with track *B* playing (Collection intensity 1.0): a script sets
`Snd_SubMix_MusicVolume` to 0.5 at 110.0 s and to 0.05 at 112.0 s. The Collection's intensity
becomes 0.25 at 110.0 s and 0.0025 at 112.0 s (squared, §4.4), and the Sound's voice volume
follows (its factor 1.0). Setting the volume back to 1.0 restores 1.0. A track Collection that
*starts* while the volume is 0.05 starts at 0.05.

<a id="s24-3"></a>
### 24.3 A placeholder in a Random (§8, `sound.md` §14.6)

A Random with three weight-1 ports, `NoRepeats` 1: ports 1 and 2 hold fire-and-forget Sounds
with sound infos, port 3 a fire-and-forget Sound with sound info 0. Starts of the Random:

| Start | Weights considered | Draw *r* | Pick | Result |
|---|---|---|---|---|
| 1 | 1, 1, 1 (sum 3) | 2 | port 3 | the Sound start fails; no child; the Random completes at once; memory = 3 |
| 2 | 1, 1 (port 3 excluded; sum 2) | 0 | port 1 | sound requested; completes at once; memory = 1 |
| 3 | port 1 excluded (sum 2) | 1 | port 3 | silent again |

If port 3 also held a SetProperty after the placeholder, the SetProperty would still run (§3.2).

<a id="s24-4"></a>
### 24.4 Timer with a port beyond its duration (§11)

One-shot Timer: `Duration` 2.0, ports at 0.0, 1.5 and 3.0, each holding a fire-and-forget Sound;
speed 1. Started at 10.0 (outside a scheduler advance): port 0.0 fires at the first advance
after 10.0 (scheduler time 10.0); 1.5 at 11.5; 3.0 at 13.0; the cycle end (*C* = 3.0) at 13.0,
after the port (insertion order); the Timer completes at 13.0. With `LoopForever` 1 the next cycle
begins at 13.0 with its port 0.0 at 13.0, and so on every 3.0 s.

<a id="s24-5"></a>
### 24.5 Instant steps and pass pacing (§10)

Sequence `LoopForever` 1 with two steps, each a SetProperty. Started at 5.0: pass 1 runs both
steps in the start call; pass 2 is due but less than 0.1 s has passed: scheduled for 5.1. At the
first advance reaching 5.1 pass 2 runs (both steps), pass 3 is scheduled for 5.2, and so on: ten
passes per second, twenty property sets per second.

<a id="s24-6"></a>
### 24.6 Switch with Delay and RampOut (§6.7–§6.8)

Persistent Switch on an integer property, case 1 (payload floats RampOut 0.5, Delay 0.2) holds a
looping tracked Sound *s₁*, case 2 (RampOut 0, Delay 0.2) holds *s₂*. Value 1 at 0.0: *s₁* plays. Value 2 at 1.0:
the switch to case 2 is deferred to 1.2. Value 1 at 1.1: the deferral is cancelled; case 1 is the
current case: nothing changes. Value 2 at 2.0: deferred to 2.2; at 2.2 case 1's branch becomes the
outgoing branch with a 0.5 s window (case 1's RampOut), *s₂* starts; each frame *s₁*'s intensity is
(2.7 − now)/0.5 and *s₂*'s 1 − that; at 2.7 *s₁*'s branch is set to 0 and stopped (its voice then
ramps out from 0), *s₂* is at 1.0.

---

<a id="s25"></a>
## 25. Corrections to other specs

| # | Spec, place | It says | BIN3 / OBS | Resolution |
|---|---|---|---|---|
| X1 | `sound.md` §9.3 | the factor is clamp(multiplier + *u*, 0, 1) always | clamped only when the variance ≠ 0 (§4.2); a driven multiplier change gives factor² × parent (§4.4); propagation re-draws the factor and ignores changes ≤ 0.0002 (§4.3) | §4 here; `instruments.md` Q8 settled |
| X2 | `script-table.md` §7.1, §4.3, §15 item 3 | `transition_time` is the cross-fade (by name); `second_time`, the int list, ForCurrentValue OPEN; Default when no case matches | `transition_time` = `SwitchScriptPort_RampOut`: an intensity cross-fade in which the outgoing case keeps running; `second_time` = `SwitchScriptPort_Delay`: a debounce delay; the int list = property IDs whose values are further case values; no value → no port; first matching case in port order; ForCurrentValue runs on every new value | §6 |
| X3 | `script-table.md` §7.2 | all holding WhenTests run | the first holding test only; Default when none holds | §7 |
| X4 | `script-table.md` §7.4 | NoRepeats: not twice in a row | per node definition, shared by all entities; single port always taken; zero sum picks nothing | §8 |
| X5 | `script-table.md` §7.5–§7.6 | OnStop runs when its parent branch stops; Timer runs for `Duration` | OnStop runs on its own stop, not on teardown, and running children are stopped at once; Timer cycle = max(Duration, last port) ÷ speed, ≥ 0.1 s looping; completes only after its children; Sequence LoopCount 0 runs nothing | §10, §11, §16 |
| X6 | `script-table.md` §7.7 | Goto jumps to the Label | Goto calls the Label's subtree as its child; nothing stops | §15 |
| X7 | `script-table.md` §7.8 | unbound parameter: own Default port runs; case sensitivity OPEN; parameter set overrides every inner field with the same PID | unbound: nothing runs; names compare ignoring case; parameters reach inner nodes through scriptlet-driven pairs and script-parameter lookups | §17 |
| X8 | `script-table.md` §8.2 (T18) | Decision: first definition wins, pre-order | all definitions in the closure run together (pre-order, deduplicated, by table kind 1, 2, 4); `Overriding` stops descent | §12.2 |
| X9 | `script-table.md` §15 item 3 | table kinds 1 and 2 not known | the kind orders the roots of a combined script (kind 1 first, then 2, then 4) | §12.2 (why the editor distinguishes them stays open) |
| X10 | `sound.md` §10.5 | Switch rule Reported | confirmed for Switches without Delay/RampOut, which is every music Switch | §6.6 |

The one-line pointers in `script-table.md` §7 and `sound.md` §9.3 refer here; their text is
otherwise unchanged.

---

<a id="s26"></a>
## 26. Owner observations (proposed, for the checklist maintainer)

`docs/checklists/owner-observations.md` is not edited here.

| Proposal | Question | How |
|---|---|---|
| RT-1 | Does the old area track keep fading while the new one starts (§6.6, 24.1)? | Cross an area border with music playing; note whether both are audible for a few seconds. (Same as `sound.md` S-1.) |
| RT-2 | Does walking back and forth across an area border within one track restart the music (§6.6)? | Cross out and straight back; a restart is audible as the track beginning again. Expected: the new area's music starts each time; returning starts the original area's program from step 1. |
| RT-3 | Master-fade loudness curve (§4.4) | Enter combat with music playing and record the client's output: at the end of the 3 s fade the music should be about −52 dB (0.05² × the track level), not −26 dB. |
| RT-4 | Quest-dependent music (§18 OPEN-11) | In an area whose music case holds a quest-status callback, compare the music with the quest done and not done. |
| RT-5 | Pacing of rapid ambient loops (§10.4, §11.2) | Listen to a dense frill emitter (a fire): no burst faster than ten per second. |

---

<a id="s27"></a>
## 27. Open questions (summary)

1. Context mode 3 bypassing the same-node rule (§3.5 OPEN-1).
2. Which stops use reason 2 (§3.5 OPEN-2).
3. Whether a playing voice follows later driven `SoundScript_*` changes (§5.3 OPEN-3).
4. The payload-to-field mapping of the Switch ports (§6.1, §6.9 OPEN-4).
5. A Switch's value for an unset property (§6.2 OPEN-5): **settled** in review 2026-10-08 (R27).
6. Internal port order (§6.9 OPEN-6, §8 OPEN-8).
7. The `Autonomous` flags (§6.11 OPEN-7).
8. Script channels and queuing types (§12.1 OPEN-9).
9. `RunScript_ExposeAsScriptLink` (§12.4 OPEN-10).
10. GameCallback evaluation offline (§18 OPEN-11).
11. Stop reason and fire-and-forget loops at entity removal (§19 OPEN-12).
12. Frame-time source and entity update order (§20 OPEN-13).
13. A defensive recursion bound (§21 OPEN-14, Decision only).
14. Loader conditions for a run's pre-load wait (§12.1 OPEN-15).
15. Animation instances on undrawn entities (§18 OPEN-16).
16. Run-time duration of effect kinds (§18 OPEN-17).
17. When and with which seed the second seeding caller reseeds the generator (§8 OPEN-18).
18. What resets `Movement_IsJumping` after a jump (§30.7 OPEN-19).
19. When a PhysicsImpulse instance completes (§30.6 OPEN-20): **answered** 2026-10-09 (R51).
20. How the `Names` entries of a While are stored (§30.1 OPEN-21).
21. Context flag bits 5 and 6, the target's deferral flag, and a stop notice for a deferred impulse (§30.6 OPEN-22).

---

<a id="s28"></a>
## 28. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-06 | all | First version (Review): run-time semantics of script-table node graphs from the client (BIN3, build 4906.0071.1794.4035) and a census of the music, ambience and emitter graphs (49,157 nodes); settles `instruments.md` Q8 and `script-table.md` T18 and the §7.8 case question; corrections X1–X10. | Spec writer (dirty room) |
| 2026-10-07 | §6.2, §29 | OPEN notes from the clean-room implementation of `tessera-scriptgraph` (unset properties, GameCallback data, driven field names, notification on an unchanged value, guards); no fact changed. | Implementer (clean room) |
| 2026-10-07 | §29 items 11, 13–17 | OPEN notes and Decisions after a code review: deferred Runs inside an advance, nested Switch re-evaluation, int-list changes without a transition, revised guards, teardown and reused entity IDs; no fact changed. | Implementer (clean room) |
| 2026-10-08 | header, §1 (R1, new R27, sources BIN-R and OBS-R), §3.1, §6.2, §6.3, §27 item 5, §29 item 1 | Review: status Accepted. OPEN-5 settled from the binary: an unset property reads its dictionary default, else its type's zero value, and is then matched like any value (Default port when no case matches); "no value" only for a property ID without a definition. The order in which the §3.1 steps are applied corrected (3, 4, 1, 2, 5, 6, 7); step numbers unchanged. **Behaviour change for implementers:** an unset property no longer means "no port"; implementations that select nothing for it must change (§6.2). | Spec reviewer (dirty room) |
| 2026-10-08 | header, §1 (R27), §6.2 | Narrow re-review of the R1 order note and the OPEN-5 lines: passed. Wording only: the "no value" clause of R27 and §6.2 now names the Switch; a When prepares its value the same way. No fact changed. | Spec reviewer (dirty room, another session) |
| 2026-10-08 | header, §1 (new source BIN-O, new R40), §8 | Owner-observation pass (O12): the Random node's random source is the client's global generator, seeded once from the wall-clock time at start-up (and reseedable by one bytecode operation); no per-object seed. Status Revised, awaiting review. | Spec writer (dirty room) |
| 2026-10-09 | header, §8, §27 item 17, §28 | Answer to the blocking items of the 2026-10-09 review: the 2026-10-08 row moved into this table; the untraced timing of the second seeding caller made a callout (new OPEN-18, pointing to `appearance.md` §7.4 O12, with Tessera's per-figure seed as its Decision). No fact changed. Status Revised, awaiting review. | Spec writer (dirty room) |
| 2026-10-09 | header, §1 (sources BIN-W, OBS-W; rows R41–R46), §8, §18, new §30, §27 items 18–20, §29 item 12 | Implementer questions answered. **New §30:** the While node from the client (test, start, re-evaluation, completion, census of the 41 nodes); the start of PhysicsImpulse (a request to each target; completion OPEN-20 with a Decision: at once); the avatar's jump scripts walked through with the rules of this spec, including the finding that `Loc_Jumping` does not clear `Movement_IsJumping` by itself (OPEN-19, with a Decision that `collision.md` §11.10 states). §8: the implementer OPEN on per-entity seeding answered (Tessera Decisions confirmed; the original seeds nothing per entity). No earlier fact changed. Status Revised, awaiting review. | Spec writer (dirty room) |
| 2026-10-09 | header, §1 (OBS-W, R45), §30.7 items 1–2, OPEN-19 | Answer to blocking item B1 of the 2026-10-09 review of §30: OPEN-19 no longer says that "the jump scripts" carry `Script_ClientAuthorative` true and `Script_ReplicateToClient` false; it names the scripts whose per-script sets hold them (`Loc_JumpKeyPressed` and `Loc_JumpKeyReleased`), re-read over every decodable table; `Loc_Jumping` holds only `Script_MaxNumInstances` 2, `Movement_ActionKey` an empty set. The sets added to §30.7 items 1–2 and R45. No other fact changed. Status Revised, awaiting re-review. | Spec writer (dirty room) |
| 2026-10-09 | header, §30.7 item 1 | Answer to blocking item B2 of the 2026-10-09 re-review: the sentence about the empty per-script property set now names `Movement_ActionKey` (it followed the clause about `Loc_JumpKeyReleased` and could be read as about that script), and the next sentence names the key it says is started by untraced client code. Wording only; no fact changed. Status Revised, awaiting review. | Spec writer (dirty room) |
| 2026-10-09 | header, §1 (OBS-W, R46), §30.5, §30.7 items 1–2, consequence d, OPEN-19 | Non-blocking review notes N1–N4 taken up (no blocking item was open): "player locomotion tables" replaced by "the locomotion tables that define `Loc_Jumping`"; the `Jump` definitions outside the avatar's closures that set and clear `Movement_IsJumping` recorded in R46 and as a hint in OPEN-19; consequence d states that a held run of `Movement_ActionKey` refuses a second start; the §30.5 first-children cell split by kind with counts. No BIN fact or Decision changed. Status Revised, awaiting review. | Spec writer (dirty room) |
| 2026-10-09 | header; §1 (sources BIN-I, OBS-I; rows R44 corrected, new R47–R52); §3.1 step 7; §3.2; §23 RI13–RI15; §27 items 19, 21; §29 item 19; §30.6 | Spec writer, second pass of the day, answering the implementers' open points. **§30.6 rewritten from the client (BIN-I):** the PhysicsImpulse node reads target port types 8 (`ImpulseTarget`) and 19 (`Impulser`), not 25 (**correction**); it uses the first entity of each, so it sends one request; a present port that resolves to nothing fails the start; on the default path the vector is in the target's frame and `IsGlobalSpaceImpulse` is not read; the impulser path; what the request carries; when the target applies it (at once unless deferred); OPEN-20 answered (the node completes on the target's completion notice, in the same call when applied at once); census of the 37 nodes; new OPEN-22. **§3.1 step 7 and §3.2 made precise (R47), answering §29 item 19:** the parent link exists from creation; a start whose port group moved fails and ends the runner; a group stop alone does not stop the runner of an instance still in its start. **Behaviour change for implementers:** the §29 item 19 Decision and the per-entity requests of PhysicsImpulse are replaced. Status Revised, awaiting review. | Spec writer (dirty room) |

---

<a id="s29"></a>
## 29. Implementation feedback (clean room, 2026-10-07)

`tessera-scriptgraph` was implemented from this spec, `script-table.md` §2–§5, §7, §8.1–§8.2,
§11.3, §15–§16 and `sound.md` §9–§11 alone. On the reference install its real-data test runs
`SND_music_system_startup` on a player avatar and sets `Area_Music` to 40 sampled case values
and one value with no case: tracked channel-2 tracks start for 33 of the 40, area changes stop
the playing track, the avatar keeps at most 230 instances, 20 sound environments register; a
sample of 400 emitter descriptors requests sounds. Points for the spec owner:

1. > **Answered** (review, 2026-10-08): settled by R27 in §6.2; neither rule below is exact.
   >
   > **OPEN:** (§6.2 OPEN-5) Which rule holds for a Switch or When whose property has no
   > value and no dictionary default: no port at all (this spec's Decision) or the Default
   > port (`sound.md` §10.2, `script-table.md` §16). With the first, the music graph plays
   > nothing in the walker (0 track starts in the real-data test); with the second it plays
   > (above). The implementation offers both (`UnsetPolicy::NoPort`, the default, following
   > this spec; `UnsetPolicy::DefaultPort`). A When without a value is treated like a
   > Switch. Please settle the rule, or say which properties the client must set and to
   > which value.
2. > **OPEN:** (§18, OPEN-11) `tessera-formats` keeps GameCallback (27) data opaque, so its
   > `DefaultValue` is not available to the runtime; GameCallback nodes complete at once
   > (reported once). The Decision of §18 needs the decoded `DefaultValue` PV
   > (`script-table.md` §5.2 lists the layout).
3. > **OPEN:** (§5.2, §2.2) Driven pairs name node fields by property ID, but this spec names
   > only `Script_IntensityMultiplier` among the base-block fields. The implementation
   > follows driven pairs on `Script_IntensityMultiplier`, the Timer fields (§11.1), the
   > Sound fields (`sound.md` §9.1) and the SoundEnvironment fields, looked up by those
   > names; pairs on `IntensityMultiplierVariance`, `Probability`, `UseParentIntensity` and
   > other kinds' fields are ignored. Please give the property names of the base-block
   > fields. A SetProperty's value is replaced by a driven pair whose field is the
   > SetProperty's own property (the field naming is open in `script-table.md` §16).
4. > **OPEN:** (§4.4) "factor × (factor × p)" with a variance ≠ 0: one draw used twice or
   > two draws? The implementation draws once (*f*² × *p*).
5. > **OPEN:** (§14, §5.3, §20.3) Whether setting a property to the value it already has
   > notifies listeners. The implementation notifies only on a change. It matters for §4.4:
   > a driven multiplier set again to the same value would otherwise square again (0.5 at
   > start gives intensity 0.5; set to 0.5 again it would become 0.25).
6. > **OPEN:** (§12.2) "The per-script property sets are merged the same way": the
   > implementation takes, per property, the first value met in start order (kind 1, 2, 4;
   > pre-order). Please state the merge rule.
7. > **OPEN:** (§16 vs §3.2) The port runner stops after a child when the instance it fills
   > is stopped; an OnStop is already stopped when it runs its children, so read literally
   > only its first child would start. The implementation starts every Default child of an
   > OnStop (each one that stays running is stopped at once, §3.1 step 7).
8. > **OPEN:** (§6.9) A new value while a transition with `Wait` = 1 has taken over (its
   > children still run): the implementation switches at once (the transition's branch is
   > stopped without a window, the new case starts).
9. > **OPEN:** (§11.3) A looping Timer whose intensity was ≤ 0.0002 when a cycle began
   > schedules nothing; whether a later rise of its intensity restarts it is not stated. The
   > implementation does not restart it.
10. > **OPEN:** (§6.8 step 5) Children of the new case start at factor × *I* and take
    > factor × (1 − *f*) × *I* at the next per-frame update; whether the original starts them
    > at the faded level is not stated (a voice is audible at full gain for one frame).
11. Guards beyond OPEN-14 (Tessera; no fact changed): a stack-depth guard per public call
    (1 MiB by default) also ends synchronous loops that do not pass through nested starts
    only (WhenRunning nodes that start and stop each other's scripts, chains of property
    reactions); 200,000 node starts per call; 100,000 scheduler events per advance (see
    item 16); 256 cycles of a zero-length non-looping Timer at one scheduler time (a
    negative `LoopCount` loops practically forever, §11.4); a pacing resume of a Sequence is
    compared with the same sum that scheduled it, so floating-point rounding cannot
    reschedule it at the same time forever.
12. Unsupported here and completing at once (reported once): target types other than
    `ThePlayer` (4) and `This` (13) (the music graph has Run nodes with a `Child` (14)
    target), and the effect kinds of §18 (While — specified in §30 since 2026-10-09 —, EditMaterial, TextureCompositor,
    ModifyCompositorLayer/Color, MeshDeformer, MonitorObjects occur in the walked graphs).
    ScalarBlend starts the children of the one or two ports whose anchors bracket the
    property's value, once, without weighting their intensity (OPEN-17).
13. > **OPEN:** (§12.4 step 2 vs §20.1) A Run request to another entity is "deferred to the
    > next scheduler pass", while §20.1 delivers an event due "now" later in the current
    > advance when it is scheduled inside one. Read with §20.1, two entities whose Runs set
    > properties that each other's persistent Switches watch would bounce without end inside
    > one advance. **Decision (Tessera):** a request made during an advance is handled in the
    > next advance (one step per frame); a request made outside an advance (a property change
    > set by the client, a script start) is handled in the next advance as before. Please
    > state when the client handles the message.
14. > **OPEN:** (§6.6, §6.10, §16) Stopping the current case can change the Switch's own
    > property (an OnStop's SetProperty runs during the stop), and so can a ForCurrentValue
    > child at start. The nested re-evaluation selects and starts the case for the newer
    > value. **Decision (Tessera):** the outer evaluation then ends without starting the case
    > it had selected, so one case runs (§7 "only one branch runs at a time"); and an
    > instance that is being stopped (still listed while its stop runs) does not count as a
    > running child for §3.1 step 3, so the nested evaluation may restart the old case's
    > nodes. Please confirm the original's order.
15. > **OPEN:** (§6.5, §6.7, §6.9) A persistent Switch in transition mode re-evaluates when an
    > int-list property of a case changes while its own value stays the same. §6.7 steps 1–3
    > and the transition tiers of §6.9 are stated for a new value. **Decision (Tessera):**
    > no transition is looked up or applied; only step 4 (case selection) runs. A transition
    > port with `Wait` = 0 whose children still run after it is applied leaves them running
    > outside any case (they are stopped with the Switch).
16. Guards revised after review (Tessera; no fact changed): a run started by a Run node
    continues the Run instance's depth, so `max_instance_depth` (256) bounds chains of runs
    that start runs (and the recursive stop of such a chain); a Timer cycle counts toward
    the zero-length guard when it does not move the cycle start forward (length 0, or below
    the resolution of the scheduler time), not only for a length of exactly 0; one Timer
    begins at most 1,024 cycles per advance, further cycles wait for the next advance and
    the lag catch-up of §11.2 applies; one advance delivers at most 100,000 events and
    starts at most 200,000 nodes over all its events; one advance moves the clock by at most
    3,600 s; a looping Sequence pass that would begin at the same scheduler time as the
    previous one (only at clock values where 0.1 s no longer changes the time) ends the
    Sequence.
17. > **OPEN:** (§19) Whether anything can start on an entity during its teardown (a
    > persistent WhenRunning of the entity re-evaluates when another run of the entity stops
    > first). **Decision (Tessera):** no node starts on an entity being destroyed; such starts
    > complete at once. Listeners and run watches that nodes of other entities register on
    > an entity outlive it: after a despawn they read no entity value (the default), and when
    > the same entity ID is spawned again (the player after a zone load) they hear it again.
18. Implementation after R27 and the corrected §3.1 order (clean room, 2026-10-08):
    `tessera-scriptgraph` drops its `UnsetPolicy` switch; a Switch or When starts from the
    dictionary default, else the type's zero value (§6.2), and the start steps run in the
    order 3, 4, 1, 2, 5, 6, 7 (a start skipped by the same-node rule makes no probability
    draw; the persistent-node gate reads a driven `TimerScript_LoopForever` from the copy).
    > **OPEN:** (§6.2, §7) "the 'no value' rule is stated for the Switch only": what a When
    > selects when its property ID has no dictionary definition. **Decision (Tessera):** as
    > for a Switch, no port (not even Default).
    >
    > **OPEN:** (§6.2) The zero value of a type code the dictionary may carry but the format
    > does not define (`property-system.md` §2.2 types other than 1–22). **Decision
    > (Tessera):** no zero value; a Switch on such a property, unset, has no value. Empty
    > values used for the other non-scalar types: empty string, struct, array, bitfield and
    > waveform (subtype 0), zero vector, colour and position, a StringInfo with an empty
    > literal.
    >
    > **OPEN:** (§6.2, §6.3 step 1) "those without a value are skipped, with no default":
    > read as "only a value actually set counts" (a run-time write or a stored property set
    > of the entity; for script-parameter properties the run-parameter set), so an int-list
    > property whose only value is its dictionary default is skipped. Please confirm.
    > Related to item 5: setting such a property to its default value is no change of the
    > value the entity reads, so the Switch is not told and does not re-evaluate.
19. Implementation of §30 (clean room, 2026-10-09): `tessera-formats` decodes the While
    fields of `script-table.md` §5.2; `tessera-scriptgraph` runs While (§30.2–§30.4) and
    PhysicsImpulse (§30.6, completing at once by OPEN-20, the request handed to the effect
    sink); the static clip resolver applies the same While test and stops the branch at a
    SetProperty that makes it fail; the walker's jump table is read by running
    `Movement_ActionKey` per state (§30.7), which gives 6.0 m/s for state 5, 3.0 m/s for
    1–3 and none for 4 and 6 on the reference install.
    > **OPEN:** (§3.2, §30.4) A node whose start is still in progress when the port group it
    > is being started into is stopped: §3.1 registers an instance with its parent only in
    > step 7, after its kind start, so a Collection or Sequence of a While's branch whose own
    > first child (a SetProperty) makes the While's test fail is not yet in the group that
    > the While stops, and by §3.2 alone it would go on starting its next children before
    > step 7 stops it. §30.4 says the later siblings do not start. **Decision (Tessera):**
    > the runner's check after each child (§3.2) also counts an instance as stopped when it
    > is not yet registered and the group it is being started into has been stopped, so
    > nothing further starts below it. Was this read from the client, or does the original
    > register the instance before its kind start?
    >
    > **Answered** (spec writer, 2026-10-09; BIN-I, R47; §3.1 step 7 and §3.2 made precise).
    > Read from the client: the original registers the instance with its parent and its port
    > group **after** its kind start, as §3.1 step 7 says, but creates it with its parent link
    > first, and the runner's check after each child walks those links. In the While case the
    > While stops its port group, which is still empty because the Collection is not yet
    > registered, then arms with no child and no pending work and so **completes in the same
    > call**; the Collection's runner then finds a stopped ancestor and starts no further
    > child, and the Collection is stopped at its own step 7. A port group stopped while its
    > owner keeps running (a Switch changing case because a child set its property) is not
    > seen by the runner of a child that is still starting: that child's remaining children
    > do start, and they are stopped with it when its start returns, which then reports a
    > failure that ends the owner's runner (§3.1 step 7 (b)). **Change for implementers:**
    > replace the Decision above by the client's rule: the runner stops after a child when the
    > instance being filled or an ancestor (through parent links set at creation) is stopped,
    > or when the child's start failed by §3.1 step 7 (b); a group stop alone does not stop
    > the runner of an instance still in its start.
    >
    > **PhysicsImpulse edge rules** (spec writer, 2026-10-09; BIN-I, R48–R51): §30.6 now
    > states them. **Changes for implementers:** read only targets of port types 8
    > (`ImpulseTarget`) and 19 (`Impulser`), not every target of the node; use the first
    > entity of each list and send one request, not one per entity; a present list that
    > resolves to nothing sends nothing and leaves the instance running until it is stopped
    > from above (not completion at once); on the default path the vector is in the target's
    > frame whatever `IsGlobalSpaceImpulse` says; the impulser path of §30.6 step 5 applies
    > when the two entities differ. Completion at once stays correct for an applied impulse
    > (OPEN-20 answered).

---

<a id="s30"></a>
## 30. While (50), PhysicsImpulse (9) and the avatar's jump path (BIN-W, OBS-W)

This section answers the implementer question of 2026-10-09 (`collision.md` §12.5, choice 3):
the avatar's on-foot jump script passes a While node, which §18 did not cover, and its jump
node is a PhysicsImpulse.

<a id="s30-1"></a>
### 30.1 While: fields and ports

Fields (`script-table.md` §5.2): `WhileScript_PropertyName` (a property ID), `Values` (a list
of PVs), `Names` (a list) and `WhileScript_Inverse` (bool). At run time the client reads each
`Names` entry as a property ID (BIN-W). Ports: the Default ports (type 6), of which **only the
first in port order** is used, and optionally a Target port (type 25) that names another entity
to watch. Every While of this install has exactly one Default port and no other port (R43).

> **OPEN-21:** `script-table.md` §5.2 lists the `Names` entries as PVs, while the run-time code
> reads each entry as a 4-byte property ID. No While of this install has a `Names` entry, so
> the stored form cannot be checked against data. **Decision (Tessera):** decoders keep the
> layout of `script-table.md` §5.2; the runtime uses each entry's property ID (for a PV, its
> PID) as the name of a watched property.

<a id="s30-2"></a>
### 30.2 The test (R41)

1. **Value.** The While reads `WhileScript_PropertyName` from its target (the running entity
   unless a Target port names another) and prepares the value as a Switch does (§6.2, R27):
   the entity's value when it has one, else the dictionary default, else the zero value of the
   property's type. A property ID without a dictionary definition gives no value, which matches
   nothing.
2. **Match.** The value matches when
   - it equals one of the `Values` entries: same property ID and equal value. Integers, enums,
     booleans, bit fields, property IDs, DIDs and floats compare as whole values, so a bit-field
     entry must equal all 32 bits (it is not a mask); or
   - it has the same type as, and equals, the current value of one of the `Names` properties,
     read from the running entity, or from the run-parameter set for a script-parameter
     property (as the int list of a Switch case, §6.3).
3. **Result.** The test **holds** when the value matches and `Inverse` is 0, or when it does
   not match and `Inverse` is 1.

<a id="s30-3"></a>
### 30.3 Start (R41)

After the steps of §3.1 (gates, copy, intensity), the While's start:

1. fixes its target (the running entity, or the entity the Target port names);
2. starts listening to `WhileScript_PropertyName` on the target and to each `Names` property
   on the running entity;
3. evaluates the test once;
4. if the test **fails**, arms; as nothing runs it **completes at once** and no child starts;
5. if the test **holds**, starts the children of its first Default port into its port group
   (§3.2) and does **not** arm.

<a id="s30-4"></a>
### 30.4 Afterwards (R42)

- Every change of a watched property, reported by the target or the running entity, makes the
  While evaluate the test again (it reads its own property afresh; which watched property
  changed does not matter).
- **Test holds:** nothing happens. The running branch continues; a branch that has already
  finished is **not** started again, and the While does not arm, so it keeps running with no
  child.
- **Test fails:** the port group is stopped with reason 0 (§3.5), the While arms, and it
  completes as soon as no child and no pending work is left, normally in the same call.
  Later changes do nothing more: a While never starts its branch a second time.
- So a While runs while its test holds, outlives its branch, and ends the first time the test
  fails, or when it is stopped from above (§3.5); its stop removes its listeners.
- Changes are seen synchronously: a SetProperty inside the While's own branch that makes the
  test fail stops the branch inside that SetProperty's start (§14), so later siblings of that
  SetProperty do not start (§3.2).
- Intensity changes reach the branch as for every container (§4.3). A property that keeps
  re-triggering itself is cut off by the guard of §21.

<a id="s30-5"></a>
### 30.5 Census (OBS-W, R43)

41 While nodes in 17 of the 7,524 decodable tables; `Names` empty and `Inverse` 0 in all;
one Default port each, with 1 or 2 children.

| Property (dictionary type; default) | `Values` | Nodes | First children |
|---|---|---|---|
| `Movement_IsJumping` (Bool; none) | true | 8 | Sequence (the jump scripts, §30.7) |
| `Combat_Violent` (Bool; none) | true | 4 | Appearance |
| `Script_PlayingCraftAnimation` (Bool; false) | true | 1 | Switch |
| `CombatState_CurrentState` (Bitfield32; 0x00040000) | 0x00000200 | 24 | one Collection (12 nodes), Timer (5) or MeshFX (2); or a Switch and a Particle (5) |
| `CombatState_CurrentState` | 0x04000000 | 3 | Collection (one with a Timer) |
| `CombatState_CurrentState` | 0x00000200, 0x00020000 | 1 | Collection |

These are observations of this install; a runtime reads the nodes from the data.

<a id="s30-6"></a>
### 30.6 PhysicsImpulse (9) at run time (R44, R48–R52; revised 2026-10-09)

Fields: `script-table.md` §5.3; what the impulse does to the mover: `collision.md` §11.6
item 3 and §11.10. The node class declares exactly two **target ports**, each allowed once,
and **no output port** (R48): port type 8 (`ImpulseTarget`, the entity that receives the
impulse) and port type 19 (`Impulser`, the entity in whose frame the impulse is given). Port
type 25 (`Target`) is not read by this node. *(Corrected 2026-10-09: the first version of this
section named a Target port of type 25.)* After the steps of §3.1, the start:

1. does nothing and completes at once when bit 6 of the context's flags is set (a flag that
   §2.1 does not name; where it is set was not traced, and the default flags leave it clear);
2. fixes the **target** and the **impulser**, both the running entity by default. When the
   node has targets of port type 8, they are resolved (`script-table.md` §4.4) and the
   **first** resulting entity becomes the target. Then the same for port type 19 and the
   impulser. Further entities of either list are ignored: a node sends **one** impulse, never
   one per entity. When a list that is present resolves to no entity, the client logs it,
   sends nothing and reports a failure code that the node start (§3.1) does not act on: the
   instance is registered like any other, is never armed and has no pending work, so it
   **stays running until it is stopped from above** (§3.5) and keeps its parent from
   completing meanwhile;
3. reads the request time: the scheduler's current time (§20.1) plus the context's
   synchronisation offset (§2.1);
4. **target = impulser** (the default, and every node without targets): sends one impulse
   request (described below) to the target with the vector
   - for `IsJump` 0: `Impulse` × the node's intensity (the context's intensity after §3.1
     step 5), **in the target's own frame**; `IsGlobalSpaceImpulse` is **not read** on this
     path;
   - for `IsJump` 1: the vertical vector (0, 0, (`AirTime` + 0.2) × `ExpectedGravity`) of
     `collision.md` §11.10 item 2, which the intensity does not scale;
   and adds the target to the instance's pending work (§3.4);
5. **target ≠ impulser:** first sends a request to the impulser (carrying the request time)
   and adds the impulser to the pending work. The impulser answers with its frame (position
   and orientation) at the request time; the answer removes the impulser from the pending
   work. The node then computes `Impulse` × its intensity (`IsJump` does not change the vector
   on this path), turns it by the impulser's orientation into the world **unless**
   `IsGlobalSpaceImpulse` is 1 (then it is taken as a world vector unchanged), and sends the
   impulse request to the target, flagged as a **world-frame** vector, with the same request
   time; the target is added to the pending work. An impulser that cannot give its frame
   answers with a failure; the node then arms and completes without sending an impulse;
6. does not arm.

**The impulse request** (R49) carries: the vector; `ImpulseType`; `IsJump`; a world-frame
flag (0 on the path of step 4, 1 on the path of step 5); a **deferral flag**, bit 5 of the
starting context's flags (clear by default, §2.1; where it is set was not traced); and the
request time.

**At the target** (R50), when the target entity is present on this client, the request is
handled when it arrives (synchronously for a request sent during a script start): if the
deferral flag is set, the target carries a synchronisation flag (not identified) and the
request time lies after the scheduler's current time, the request waits for a scheduler event
at that time; otherwise it is applied at once. Applying: a world-frame vector is first turned
into the target's frame (by the target's current orientation; that it is the inverse turn
is Inferred from `collision.md` §11.6 item 3, which keeps the stored impulse in the mover's
frame); then the vector is added to the target mover's stored impulse with the request's `ImpulseType` (`collision.md` §11.10 items 1–2, including the rule that an
`IsJump` request adds nothing while the stored impulse already has a vertical part). Applied
or not (no mover, or that rule), the target then sends a **completion notice** to the
starting entity. A target that is not present on this client is reached through the client's
message path, which is out of scope here.

**Completion** (R51; answers OPEN-20). The completion notice removes the target from the
instance's pending work and arms the instance; having no child, it completes (reason 0). With
the default flags and a target present on this client, the target applies the impulse and its
notice comes back within the call that started the node, so the node **completes in that same
call**. A deferred request completes the node when the scheduler reaches the request time.
Stopping the node earlier (§3.5), except on a teardown stop (reason 2), sends a stop notice to
each entity still in its pending work.

**Census** (OBS-I, R52): 37 nodes (31 in tables, 6 in scriptlets); `ImpulseType` 2 and
`IsGlobalSpaceImpulse` 0 in all; 18 have `IsJump` 1, none of them a target (the avatar's
jump nodes of §30.7 among them); 6 have targets: one an `ImpulseTarget` only, five an
`ImpulseTarget` of type This (13) and an `Impulser` of type Property (3). No node has a port.

> **OPEN-20 (answered 2026-10-09, R51):** When the PhysicsImpulse instance completes: on the
> target's completion notice, which comes back in the same call when the target applies the
> impulse at once (above). The earlier **Decision (Tessera)** ("the node applies its impulse
> during its start and completes at once") matches the client whenever the deferral flag is
> clear and the target is present, which is every case of an offline client with the default
> flags; on the jump path this changes nothing (§30.7 b).

> **OPEN-22:** (a) Where bits 5 and 6 of a context's flags are set (§2.1 names bits 1 and 3
> only), and which flag of a target allows a deferred request; (b) what a target does with
> the stop notice of a stopped node (paragraph "Completion") for a request it has deferred.
> **Decision (Tessera):** treat bits 5 and 6 as clear, so no node is skipped by step 1 and no
> request is deferred; a stopped node cancels nothing that has already been applied.

<a id="s30-7"></a>
### 30.7 The avatar's jump path (data, OBS-W; R45, R46)

Script keys (labels of enum 0x23000097 and its base 0x230000F8; interoperability
identifiers): `Movement_ActionKey` (9), `Loc_JumpKeyPressed` (0x10000426),
`Loc_JumpKeyReleased` (0x100002C5), `Loc_Jumping` (0x1000024F), `Jump` (1),
`Mounted_Channeling_Loop` (0x10000147). Table DIDs are this install's; read them at run time.

1. **`Movement_ActionKey`** (defined in 0x07000433 and 0x070017E2): a Switch on
   `Movement_FrozenWhileWaitingForServer` (case false) → a Switch on `Movement_IsJumping`
   (case false) → a Switch on `Entity_FinalElement` (cases 5, 1, 2, 3) → a Label whose ports
   start, in ordinal order, (1) an OnStop whose child runs `Loc_JumpKeyReleased` and (2) a Run
   of `Loc_JumpKeyPressed` (both Runs Persistent 0). Every other case selects nothing and the
   run ends at once. The OnStop keeps the run alive until it is stopped; its stop then runs
   `Loc_JumpKeyReleased`, whose definitions in the locomotion tables that define `Loc_Jumping` are empty
   Collections. The per-script property set of `Movement_ActionKey` is empty in both its
   tables. Which client code starts `Movement_ActionKey` is not traced (`collision.md` §11.10).
2. **`Loc_JumpKeyPressed`** (the locomotion tables that define `Loc_Jumping`: 0x0700001C, 0x070017E4, 0x07002B86;
   per-script set: `Script_ClientAuthorative` true, `Script_ReplicateToClient` false,
   `Script_MaxNumInstances` 2; `Loc_JumpKeyReleased` has the same set in the same three
   tables): a Switch on `ScriptSystem_ActiveMovementBlocked` (case true: nothing) → a Switch on `Movement_ActiveMovementFrozen` (case true: an empty
   Collection) → a Switch on `Movement_IsJumping` (case false) → a Switch on
   `Entity_FinalElement` (cases 5, 3, 1, 2, 6; in 0x070017E4 without 6) → a Label → a Run of
   `Loc_Jumping` with Persistent 1.
3. **`Loc_Jumping`** (the same tables; `Script_MaxNumInstances` 2), on foot: a WhenRunning on
   `Mounted_Channeling_Loop` (Persistent 0), NotRunning port → a Collection with two ports:
   ordinal 1, a SetProperty of `Movement_IsJumping` to true; ordinal 4, a While on
   `Movement_IsJumping` with `Values` {true} → a one-pass Sequence: step 1, a Switch on
   `Movement_CannotJump` (case false) → a Switch on `Entity_FinalElement` → a PhysicsImpulse
   (`IsJump`) and a Run of `Jump` with Persistent 1; step 2, a Timer of 0.2 s without ports;
   step 3, a SetProperty of `Movement_IsJumping` to false. The mounted branches have the same
   shape with gait cases (`collision.md` §11.10).

Consequences under the rules of this spec (Inferred: each step is a stated rule, the chain was
not observed in the original):

- **a.** The Collection starts its ports in ordinal order (§9), so `Movement_IsJumping` is
  true when the While starts; its test holds and the impulse branch starts in the same call
  as the script. The impulse is requested in the frame in which `Loc_Jumping` starts.
- **b.** Step 1 holds the persistent Run of `Jump` (the take-off clips of the race's
  animation table). A persistent Run does not complete when the script it started ends
  (§12.4 step 3), so steps 2 and 3 are not reached by the scripts alone and
  `Movement_IsJumping` stays true.
- **c.** While it is true, the `Movement_IsJumping` Switches of items 1 and 2 select nothing,
  so no further jump starts.
- **d.** When `Movement_IsJumping` becomes false by any means, the While stops its branch
  with reason 0, the persistent Run then stops `Jump` (§12.4 item 5), and the While, the
  Collection, the WhenRunning and the run of `Loc_Jumping` end in the same call. The
  persistent Run in `Loc_JumpKeyPressed` stays running (§12.4 step 3), so with the instance
  limit of 2 a third start of `Loc_JumpKeyPressed` would be refused (§12.3). Likewise the run of
  `Movement_ActionKey` that reached its Label is held by its OnStop (item 1) and its limit is 1
  (empty per-script set, §12.3), so a second start of `Movement_ActionKey` is refused until
  something stops that run.
- **e.** In the client's code the property ID of `Movement_IsJumping` occurs once, as a read
  (the "not jumping" condition of `collision.md` §11.6 item 4); no code that writes it was
  found, and no script other than `Loc_Jumping` sets it for the player (R46).

> **OPEN-19:** What ends a jump in the original: what sets `Movement_IsJumping` back to false
> and what ends the runs of `Loc_Jumping` and `Loc_JumpKeyPressed`. By b–e the scripts do
> neither, yet the original allows one jump after another. Candidates: a client path that
> writes properties or stops runs generically (not found by the constant search), the server
> (the per-script sets of `Loc_JumpKeyPressed` and `Loc_JumpKeyReleased` carry
> `Script_ClientAuthorative` true and `Script_ReplicateToClient` false; that of `Loc_Jumping`
> holds only `Script_MaxNumInstances` 2 and that of `Movement_ActionKey` is empty, items
> 1–3), or a completion rule of persistent Runs that §12.4 does not state. A hint from the data
> (OBS-W, R46): the six definitions of `Jump` (key 1) that set `Movement_IsJumping`, all in
> tables outside the avatar's class closures (`script-table.md` §8.2), set it to true in step 1
> of a root Sequence and back to false in the child of a root OnStop, so there the flag is
> cleared when the run of `Jump` is stopped; no `Jump` definition reached by the avatar sets it.
> Suggested methods: M3 (owner: jump repeatedly on flat ground; jump again while still in the air) and
> M4 (the script start and stop paths of the player's own entity). **Decision (Tessera):**
> as `collision.md` §11.10 states: the controller sets `Movement_IsJumping` to false at the
> end of the first physics update at least 0.2 s after the take-off update whose
> `Entity_Element` is not 4, and the runs of `Loc_Jumping` and `Loc_JumpKeyPressed` that the
> jump started are then stopped with reason 0.
