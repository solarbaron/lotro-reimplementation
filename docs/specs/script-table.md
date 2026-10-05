<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Script tables (0x07) and scriptlets (0x0C), part 1: framing, animation nodes, clip selection

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
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
| T14 | The template chooses by entity properties: `Entity_Element`, `Movement_MovementStyle`, `Slider_Propel`, `Slider_Strafe`, `Slider_Turn`, `Entity_Mode`, `Movement_SpeedMultiplier`, stealth, mount and cosmetic properties (§8.4) | OBS | Confirmed (structure); the client code that sets those properties is OPEN |
| T15 | Walking and running are separate branches, not a speed blend; playback rate follows `Movement_SpeedMultiplier` through a driven property (§8.5) | OBS (14,278 animation nodes have `AnimationScript_Speed` driven) | Confirmed (data); rate matching to ground speed OPEN |
| T16 | Random choice uses `u32` weights on the ports of a Random node (§7.4) | OBS (137,690 ports; 96 % weight 1) | Confirmed (layout); selection rule Reported by name |
| T17 | No clip carries a timed annotation; all animation events come from script ports (§9) | HD §4.4; OBS | Confirmed |
| T18 | Script lookup precedence across includes | none | OPEN; pre-order and breadth-first disagree on about 590 keys per avatar (§8.2) |

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
| 4 Execute | 5 | u32, u8 | 629 | Callback result value and flag (OPEN) |
| 5 Run | 9, 14 | u8, u32 | 9 | Result filter (OPEN) |
| 6 Sequence | 6 | u32 | 32,149 | Step number; steps run in ascending order (§7.6) |
| 11 WaveformBlend | 6 | u32 | 1 | (OPEN) |
| 12 Switch | 5 | switch case, below | 77,134 | Case that fires when the switch property matches (§7.1) |
| 12 Switch | 6 | f32, f32 | 21,248 | Default case; same two floats as a switch case. LC reads them as two u32. |
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
| `transition_time` | f32 | 0.0 in 76,821 cases; 0.2 and 0.3 in the locomotion templates (§8.4) |
| `second_time` | f32 | 0.0 nearly always; 0.1, 0.5, 0.75, 1.0 occur (OPEN) |

**Switch transition** (payload of (12, 28)): PV *from*, PV *to*, bool8, bool8, f32, bool8,
f32, f32, f32. Commonest tail: (0, 0, 0.0, 1, 0.0, −1.0, 0.0) in 1,230 of 1,480.

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
- Counts: bound every count by the remaining bytes (a node needs at least 31 bytes, a port 2,
  a PV 4, a target 7) before allocating.
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
| 18 | MeshDeformer | 74 | 0 | `…_AnimationAsset` DID (a clip), `…_AnimationTime` f32, `…_UseBoneWeights` bool8 | C |
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
| 22 | `ManualUpdate` (`AnimationScript_ManualUpdate`) | bool8 | Hold a fixed time instead of playing | 1 in 30 |
| 23 | `ManualTime` (`AnimationScript_ManualUpdateTime`) | f32 | Held time (fraction, OPEN) | 0 in 27,197; 0.1–1.0 otherwise |
| 27 | `StartTime` (`AnimationScript_StartTime`) | f32 | Start of the played range, a **fraction** of the clip | 0 in 27,172; 0.05–0.65 otherwise |
| 31 | `EndTime` (`AnimationScript_EndTime`) | f32 | End of the played range, a fraction | 1 in 27,172; 0.1–0.9 otherwise |
| 35 | `RandomizeStartTime` (`AnimationScript_RandomizeStartTime`) | bool8 | Start at a random phase | 1 in 16 |
| 36 | `Speed` (`AnimationScript_Speed`) | f32 | Playback rate | 1.0 in 26,667; 0.5–2.0 otherwise; **driven** in 14,278 (§8.5) |
| 40 | `SpeedMultiplier` (`TimerScript_SpeedMultiplier`) | f32 | Second rate factor | 1.0 in 27,193 |
| 44 | `LocomotionDisplacement` (`AnimationScript_LocomotionDisplacement`) | vec3 | Ground displacement over one clip cycle, metres, entity frame (§6.3) | (0,0,0) in 26,329; only y in 550; x and y in 207; only x in 130; z in 1 |
| 56 | `DetachOnRampOut` (`AnimationScript_DetachOnRampOut`) | bool8 | (OPEN) | 1 in 1,991 |
| 57 | `SynchronizeTiming` (`AnimationScript_SynchronizeTiming`) | bool8 | Phase-match with the clip it replaces on the same layer (Reported by name) | 1 in 27,111 |
| 58 | `RequiresUpdatingEvenWhenCulled` (`…`) | bool8 | Keep updating off screen | 1 in 57 |

BIN gives this order and these value kinds; LC has no reader for kind 36, which is why it
fails on every table that contains one. The client's constructor defaults (BIN, Reported)
are `RampIn` = `RampOut` = 0.1 and `Speed` = `SpeedMultiplier` = 1.0; serialized values
always override them.

<a id="s6-2"></a>
### 6.2 Layers (Confirmed values, priority rule Reported)

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

> **Decision** (until owner observation): a clip on a higher layer overrides a lower one
> bone by bone, weighted by its ramp-in/ramp-out and, where `UseBoneOpacity` is 1, by the
> clip's track weights (§6.5). Clips on one layer replace each other with a cross-fade of
> the new clip's `RampIn`. **OPEN**: whether some layers add instead of override.

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
parameter set (§7.8) usually supplies the displacement for the clip it invokes (26,132
parameter sets carry it) and then overrides the node's own value (they differ in 12,328 of
26,391 pairs examined).

<a id="s6-4"></a>
### 6.4 Ports of an animation node (Confirmed)

Only port type 6 occurs (7,771 nodes have ports; 24,507 ports). Its payload is an `f32`
time in **clip seconds**: 24,178 times lie in [0, duration], 329 beyond the duration (OPEN
whether they never fire or wrap). The children run when playback passes that time (the
animation-event mechanism, §9). Children are Scriptlet nodes (13,358, mostly footstep and
effect scriptlets), Random (3,283), StoryHook (2,771), Sound (2,279), Particle (1,488),
Switch, Run, Collection, WhenRunning, Timer, CameraShake and Light nodes.

<a id="s6-5"></a>
### 6.5 `UseBoneOpacity` and track weights (OPEN)

6,935 clips carry per-track weights (`havok-data.md` §2.3 field 8). The name suggests that
`UseBoneOpacity` = 1 makes the player apply them, but the data does not prove it: of the
animation nodes on weighted clips, 6,612 have it set and 1,125 do not; of those on
unweighted clips, 16,857 have it set (where it can change nothing). It is driven by
`ScriptSystem_CurrentlyMoving` in 7,722 nodes, which fits "upper-body only while moving".

> **Decision:** when `UseBoneOpacity` is 1 and the clip has weights, multiply each track's
> blend weight by the clip's weight for that track; otherwise use 1. **OPEN** (owner
> observation): play an emote while running and check whether the legs keep running.

<a id="s6-6"></a>
### 6.6 Where animation nodes are (Confirmed)

26,835 animation nodes are in scriptlets, 382 in tables. 25,356 distinct clips are named by
an animation node; together with Timer `ToolsAnimationOverride` (a tool-preview field, 953
clips), MeshDeformer nodes (32) and DID lists, 25,387 of the 25,395 clips are referenced
from script data. The 0x0C record of a clip is usually a single animation node, optionally
with event ports, so "play clip X with these settings" is "invoke scriptlet Y".

---

<a id="s7"></a>
## 7. Control-flow nodes used by clip selection

Structure and payloads are Confirmed (§4–§5); the run-time behaviour below is Reported:
inferred from the field and enum names and from how the data uses them. Tessera must
implement it this way until an owner observation says otherwise.

<a id="s7-1"></a>
### 7.1 Switch (12)

Watches `SwitchScript_Property` on the entity. When the value matches a switch case (any
listed value under the case's operator: Equal; AllBitsSet / AnyBitSet for bit masks), that
case's children run; when no case matches, the Default port's children run. When the
property changes, the running branch stops (with its ramp-outs) and the new branch starts;
the case's `transition_time` (0.2 or 0.3 s in the locomotion templates) is the cross-fade.
`Persistent` = 1 keeps watching (re-evaluate on change); 0 evaluates once. Switch-transition
ports (28) fire once when the property changes from *from* to *to*. ForCurrentValue ports
(22) and `Autonomous` / `Synchronized` are OPEN.

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
ports means "never"). `RandomScript_NoRepeats` = 1 (25,890 of 26,778 nodes) means the
same port is not picked twice in a row; `Autonomous` OPEN.

<a id="s7-5"></a>
### 7.5 Collection (2) and OnStop (10)

Collection starts all children of all ports together (the port payload is only an ordinal).
OnStop runs its Default children when its parent branch stops.

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
(locally unless `ServerOnly`). Goto jumps to the Label node with the same name in the same
graph.

<a id="s7-8"></a>
### 7.8 Scriptlet (19) and ScriptletParameter (22): templates

A Scriptlet node runs the root nodes of the 0x0C record `ScriptletScript_File`, in a scope
where:

1. its parameter **property set** overrides, for every node inside, the field bound to the
   same PID (for example `AnimationScript_LocomotionDisplacement`, `TimerScript_SpeedMultiplier`);
2. each of its Default ports (payload: a parameter **name**) binds that name to the port's
   children.

Inside the scriptlet, a ScriptletParameter node (only in scriptlets, 6,660 nodes) whose
`ScriptletParameterScript_Name` equals a bound name runs the bound children **in its place**
(with the caller's scope). If the name is not bound, its own Default port runs instead;
in the data that port holds a placeholder Scriptlet node with file 0 (3,960 cases), so an
unbound parameter does nothing. Of the 291–293 names each avatar binds, 280–283 equal one of the 292 parameter names of
0x0C000FFE exactly, 2 more (a walking and a running weapon variant) only when case is
ignored, and about 9 match none (they are unused). **Decision:** compare names ignoring
ASCII case, so that those two bindings take effect. **OPEN**: the client's rule. This is how
one template serves every species (§8.3).

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
   - `Entity_AdditionalScriptTables`, an array of such structs (148 descriptors);
   - `Entity_InstanceScriptTable` (8 descriptors) and the `…_Override` properties (none
     set in descriptors).
2. A **game object** (0x70 with property set 0x79) adds its own tables through
   `Entity_AdditionalScriptTables` (194,460 property sets name 515,833 tables; 71 sets also
   carry `Entity_ClassScriptTable`). All 14 player-avatar objects (`skeleton-anim.md` §2.2)
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

> **Decision:** look a key up in pre-order: the table itself, then each include in stored
> order, depth first; the first definition wins. **OPEN** (T18): breadth-first order gives a
> different winner for about 590 keys per avatar closure; the client's rule could be
> settled from the executable or by an owner observation of a species-specific emote.

> **Decision:** when an entity is created, run the `Startup` script (key 5) of **every**
> table in the closure once, in the same order. The avatar's locomotion binding lives in an
> included table (§8.3), so running only the class table's own `Startup` would leave the
> avatar frozen. 27 tables of each of the 14 avatar closures have a `Startup` script. **OPEN** whether
> the client runs all of them.

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
| StoutAxe Dwarf 0x4700979A | 0x0700247A | 0x07002479 | 0x05007A13 (4.0 s) | 0x05007A0D (no displacement) | 0x05007A14 (none) | 0x05007A24 (none) |
| RiverHobbit M 0x4700CEA3 | 0x07002F15 | 0x07002F1C | 0x05008EE8 (5.1 s) | 0x05008EA9, 2.00 | 0x05008EA1, 6.50 | 0x0500145E, −2.57 |
| RiverHobbit F 0x4700CEA4 | 0x07002F16 | 0x07002F1A | 0x05008EE7 (5.1 s) | 0x05008EB6, 2.00 | 0x05008EAD, 6.50 | 0x0500159E (none) |

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
  Default starts a looping **idle-emote** clip on layer 15 (for Hobbit M 0x05000DF7, 12 s);
  with `Combat_Violent` = 1 nothing is started there. In the two River-hobbit bindings the
  Switch is on `CharGen_Stage` instead.
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
surface state. **OPEN**: their exact update rules (key ramping, analogue values) are not in
the data; Tessera's walk controller sets them directly (−1, 0, 1).

<a id="s8-5"></a>
### 8.5 Speed scaling (Confirmed data; matching rule OPEN)

- Walking and running are **separate branches** selected by `Movement_MovementStyle`, with
  a 0.2 s cross-fade and `SynchronizeTiming` = 1 for phase matching; the data contains no
  blend between a walk clip and a run clip by speed.
- Every locomotion clip node has `AnimationScript_Speed` **driven** by
  `Movement_SpeedMultiplier` (14,278 animation nodes have a driven `Speed`; a few use
  `Combat_AnimationSpeedMultiplier`). So the playback rate is the entity's speed multiplier
  (1.0 normally) times the node's `SpeedMultiplier`.
- The clip's natural speed is |`LocomotionDisplacement`| / duration (§6.3).

> **Decision:** play locomotion clips at rate = driven `Speed` × `SpeedMultiplier`, and move
> the character at the clip's natural speed × `Movement_SpeedMultiplier` (so the feet match).
> **OPEN** (`skeleton-anim.md` §7.4, owner observation): the avatar's actual walk and run
> ground speeds and whether the original rescales the rate when they differ from the natural
> speed.

<a id="s8-6"></a>
### 8.6 Random idle variants (Confirmed)

- Avatars have **no random idle**: the idle and idle-emote clips of §8.3 are fixed per
  mood. The only Random node on an avatar's `Startup` paths picks between two mounted
  fidgets.
- Creatures and NPCs use Random nodes: 572 entities reach looping idle clips and also
  Random-gated animations from `Startup` (106 more reach Random-gated animations only); the weights on those paths are 1 (8,392 port
  visits), 3 (348), 5 (330), 2 (144), 10 (91), 4, 6, 20. Each pick runs one port's subtree,
  usually a scriptlet with a one-shot clip (a fidget) on a higher layer than the looping
  idle; when it ends the idle shows again. **OPEN**: what re-triggers the Random node (a
  Timer or Sequence loop around it, where present, or the end of the fidget).
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
  when a swing lands or a block happens; in an offline client they are no-ops, and in M7 they
  become messages to Tessera's server.
- Event time is in clip seconds, before playback-rate scaling: at rate *r* the event fires
  *t*/*r* seconds of wall time after the clip starts (Reported by structure). A looping clip
  fires its events on every cycle (Decision). Events whose time exceeds the duration (329)
  never fire (Decision; OPEN).

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
   defined in no table, such as 0x10000837). Running a node:
   - Switch / When / ScalarBlend / Random / Sequence / Collection / Timer / OnStop /
     WhenRunning / Goto as in §7; skip a node with its subtree when a draw against
     `Probability` fails;
   - Scriptlet: push the parameter scope and the port-name bindings (§7.8) and run the
     scriptlet's roots; ScriptletParameter: run the bound subtree or its own Default port;
   - Animation: start the clip on its layer with the node's fields, after applying driven
     properties (base block field 1) and scope overrides; register its event ports (§9);
   - Run: look the key up (§8.2) and run that script; SetProperty: set the property and
     re-evaluate the switches that watch it;
   - every other kind: ignore in M4 (skip the subtree); sound, VFX and camera follow in M6.
4. Each frame, feed the movement inputs; a changed property re-evaluates the persistent
   Switch/When/ScalarBlend nodes that watch it, stopping branches (ramp-out) and starting
   branches (ramp-in, cross-fade).
5. Sample all running clips (`skeleton-anim.md` §10) and combine them by layer and blend
   weight (§6.2).

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
| SI6 | An animation node's data is exactly 59 bytes; its `Asset` has prefix 0x05; it has no target; its ports are of type 6 only. |
| SI7 | ScriptletParameter nodes occur only in scriptlets. |
| SI8 | Node IDs are non-zero and unique within a scriptlet (unique within a table in 7,515 of 7,524 tables, so not a rule there). |
| SI9 | Every Scriptlet node's file is 0 or an existing 0x0C record. |
| SI10 | For each of the 14 avatar entities, every clip bound to the template's idle, walk, run, back, strafe and turn parameters has as many tracks as the avatar's skeleton has bones. |

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
- changing byte 0x77 (the first byte of the port echo, `06` → `07`) makes the table
  malformed (SI5);
- a template test: a scriptlet whose root is a ScriptletParameter node named N with a
  placeholder Default child, invoked by a Scriptlet node whose Default port is named N with
  an animation child, starts that animation; invoked with no port named N, starts nothing
  (§7.8).

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

---

<a id="s15"></a>
## 15. Open questions

1. **Lookup precedence** across includes (T18, §8.2) and whether every closure table's
   `Startup` runs.
2. **CinematicCamera data** layout (§10.3); ListenerEvent and CustomLighting custom fields
   (no instances).
3. Meaning of table kinds 1 and 2; of the switch-case second float, int list and
   transition tail; of ForCurrentValue ports; of `Autonomous` and `Synchronized`; of the
   Particle extra byte; of the FireMissile custom elements; of the base block's
   scriptlet-driven pairs and scriptlet-parameter list.
4. **Layer combination** (override versus additive; §6.2) and **`UseBoneOpacity`** (§6.5).
5. `ManualTime`, `StartTime`, `EndTime` units for the 45 non-default nodes (fractions are
   assumed), `DetachOnRampOut`, and events beyond the clip duration (§6.4).
6. **Speed matching** between clip rate and ground speed, and the avatar's default run
   clip where the bound Switch trees offer several (§8.3, §8.5; owner observation).
7. The client's update rules for `Slider_Propel`, `Slider_Strafe`, `Slider_Turn` and
   `Movement_MovementStyle` (§8.4).
8. What re-triggers Random idle fidgets on creatures, and a tabulation of
   `NPC_StartingIdleAnimation` keys to clips per rig (§8.6).
9. Case sensitivity of scriptlet-parameter name matching (§7.8).
10. The 187 entities whose reachable clips have no pose-compatibility record (§10.4).

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

> **OPEN:** SI6 says an animation node's `Asset` has prefix 0x05. Two animation nodes name
> the null DID (table 0x070001E8, script 0x100006A4, node ID 2119; table 0x0700062D, script
> 0x100003A0, node ID 66); these are probably the "2 name missing clips" of §6.1. The
> implementation accepts a null asset (not reported as unusual) and starts nothing for it.
> Please state SI6 as "0 or prefix 0x05".

> **OPEN** (§11.3, unset properties): the spec does not say what a Switch or When test
> reads for a property the entity's sets do not contain. The creature template 0x0C0000B9
> selects its whole game tree with a Switch on `CosmeticEntity_SourceWrapper` whose case is
> the property's dictionary default (DID 0); with "no value, take the Default port" no
> creature starts an idle clip in the idle state. **Decision** (implementation): an unset
> property reads its dictionary default; without a default the Switch takes its Default port
> and a When test fails (an option reads it as 0 instead). With this rule the creature of
> §10.4 (0x4700005B) starts an idle clip, and the avatars keep the §8.3 clips. Please
> confirm the client's rule.

> **OPEN** (§8.3, §8.6): with the rule above and §11.3's "Run: look the key up and run that
> script", every avatar also starts **random idle fidgets**: the `Startup` script of table
> 0x07001CAA (in every avatar closure) runs a Sequence of Run nodes; script 0x1000283D runs
> 0x100001BA, which picks with weights (10, 10, 5, 5, 1, 1, 1; total 53) among seven scripts
> defined in a per-species table (0x0700077F for Hobbit M, 0x0700077A for Human M) that start
> one-shot clips on layer 15. A further layer-30 clip is started through scripts 0x10000827
> → 0x100008AF (table 0x07000435 for Hobbit M, 0x07000467 for Human M). §8.6 says avatars
> have no random idle; either the spec's analysis did not follow Run nodes or a gate on
> those paths (a property the client sets) keeps them from running. Which is it?

> **OPEN** (§8.3): the second root of the binding table's `Startup` starts a looping clip on
> layer **15** for the Hobbit, Dwarf and DwarfStoutAxe bodies, but on layer **18**
> (`face_emote`) for the Human, Elf, Beorning, HighElf and River-hobbit bodies (Human M
> 0x05001544, Elf M 0x050017FF); §8.3 calls them all idle-emotes on layer 15. The
> DwarfStoutAxe and River-hobbit bodies further reach a layer-110 (`scale_blend`) clip
> through Run nodes. Please state the layers per body.

> **OPEN** (§7.8, nested templates): a ScriptletParameter node inside a scriptlet that was
> itself invoked from inside the template could look for its name in the innermost
> invocation only or in every enclosing one. **Decision** (implementation): innermost first,
> then outward. Likewise for parameter **values** (§7.8 point 1): the innermost invocation
> that sets a PID wins, and a driven property (§4.2) wins over any parameter value. No
> avatar or creature result of §10.4 depends on either choice.

> **OPEN** (§7.1): when several switch cases match (bit-mask operators can do that), the
> implementation runs only the **first** matching case in stored order.

> **OPEN** (§4.5): a port needs only **one** byte when its type is 0 ("no port", §4.3), so
> bounding port counts with 2 bytes per port could reject a valid record made of empty
> ports. The implementation bounds with 1 byte per port.

> **OPEN** (§7.6, static evaluation): for clip selection the implementation treats every
> step of a Sequence and every port of a Timer as a candidate (all their children "start"),
> since the order in time does not change which clips are reachable. A player that runs the
> graph in time must follow §7.6 instead.

> **OPEN** (§8.1, §11.3 step 2): the spec does not say how the property sets of an entity's
> parent 0x47 descriptors (`setup.md` §3.1), its game object and its instance combine.
> **Decision** (implementation): parents first, then the entity, then the game object, then
> the instance; a later set's scalar values and its `Entity_ClassScriptTable` replace earlier
> ones (so the 71 object sets that carry a class table override their entity's), and
> `Entity_AdditionalScriptTables` and start-up names accumulate. None of the 14 avatar
> entities has a parent descriptor, so the §8.3 results do not depend on this choice.

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

> **OPEN** (§7.7, §8.6, review): a SetProperty node's driven pairs (§4.2) name a *field
> PID*, but the spec does not say which field PID carries the node's value (the PV's own
> PID, a `SetPropertyScript_…` name, or another). **Decision** (implementation, unchanged):
> the source of the first driven pair whose source has a value replaces the constant,
> whatever field the pair names. A pair on another field (an `IntensityMultiplier`, say)
> would wrongly write its source into the target property. Please name the field.

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
