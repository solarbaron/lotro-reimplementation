<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Instance scripts: per-instance script tables, member-override properties and instance switches

| | |
|---|---|
| **Status** | Accepted (2026-10-09) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (dirty room, a different session from the writer) |
| **Review record** | 2026-10-09, spec reviewer (fresh session): **pass**, §5.3 checklist met, 9 spot checks independently confirmed (dictionary, 0x47, 0x07000689, 0x0C000FCC, landblock census, BIN); non-blocking notes in the review log (decision 0018). |
| **Implements into** | `crates/formats` / `crates/props` (typed view of `Entity_InstanceScriptTable` and its conversion into the node model of `script-table.md` §4–§5, §3 and §4.5 here), the script evaluator of `script-runtime.md` (an entity's table set with instance definitions, §4), `crates/world` (day and night lights of placed objects, §6; instance switches, §8), `apps/explorer` (per-instance script and override view, §9.6), a later server or sandbox backend in `crates/net` (spawned objects carry these properties, §9.5) |
| **Companion specs** | [`property-system.md`](property-system.md) §3–§5 (value encodings, struct and array forms, bucket order), §2.2 (`field_b`, defaults). [`landblock.md`](landblock.md) §4.1, §4.6 (links), §4.8 (the weenie list). [`entity-descriptor.md`](entity-descriptor.md) §4.5.2 (per-instance property sets), §9.2 (an entity's property lookup, E26). [`script-table.md`](script-table.md) §2–§5 (binary script tables, node frame, port and target layouts, node kinds), §8.1–§8.2 (an entity's tables). [`script-runtime.md`](script-runtime.md) §3 (node life cycle), §5 (driven properties), §6–§7 (Switch, When), §12 (script starts and lookup), §17 (scriptlet parameters). [`lighting.md`](lighting.md) §8.3 (light fields and light records). [`environment.md`](environment.md) §7.3 (time of day), §11 (the clock that publishes `World_PresentTimeOfDay`). [`sound.md`](sound.md) §10.3.5 (`Volume_IsOutside`). [`wlib.md`](wlib.md) §8 (attribute name hashes). |

---

<a id="s0"></a>
## 0. Purpose and terms

Placed and spawned objects carry, in their per-instance property sets, three groups of
properties that decide what the object's scripts do:

1. **`Entity_InstanceScriptTable`**: a complete script table written as a tree of properties
   instead of as a 0x07 record. Almost all of them wire an object's script-link events to
   scripts on linked objects ("when this lever fires, run that script on the door").
2. **Member-override properties** whose names have the form *Owner*`@`*Holder*`.`*member*
   (for example `Light.Script@Entity.Night.m_fLightRange`): ordinary dictionary properties
   that the object's script graphs read as per-instance variables (day and night light
   colours, door states, time-of-day behaviour).
3. **Instance switches** such as `vfx_Fire_Color` or `Script_LightSource_Enable`: enum,
   boolean and tri-state properties that Switch nodes in the object's tables branch on.

This spec gives an implementer the layout and census of all three (§2, §3, §5, §7, §8), the
client's handling of an instance script table at entity creation (§4, BIN), the complete day
and night light mechanism of placed lamps and windows (§6, which answers the open item of
`lighting.md` §8.3), algorithms in words (§9), invariants (§10), synthetic examples (§11),
notes for other specs (§12) and the open questions (§13).

| Term | Meaning |
|---|---|
| **instance property set** | One entry of a landblock-info record's weenie list (`landblock.md` §4.8): a u64 IID and an embedded property set. `entity-descriptor.md` §4.5.2 calls it the per-instance set. Every SED IID has one; the others belong to objects only a server places. |
| **SED-IID set** | An instance property set whose IID is the IID of a static entity of the same record. |
| **server-only set** | An instance property set whose IID no SED of the record places (spawned NPCs, generators, event boxes, doors created by a server). |
| **IST** | Short for an `Entity_InstanceScriptTable` value. |
| **property tree** | A struct or array value whose fields or elements are further structs and arrays (`property-system.md` §4.10–§4.11). |
| **table set** | The script-table object an entity owns at run time: its own script definitions plus an ordered list of included 0x07 tables (§4.1). |
| **member override** | A property whose dictionary name contains `@` (§5). |
| **instance switch** | A per-instance property read as `SwitchScript_Property` by Switch or When nodes of the object's tables (§8). |

Property names, enum labels and script-key labels appear because they are identifiers stored
in the data (interoperability); an implementation reads them from the property dictionary
and the enum mappers at run time. PIDs given in parentheses are this install's values. No
game text appears; string values (link names, comments, scriptlet-parameter names) are
referred to by structure, never quoted.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed 2026-10-08 in the user's install (Steam, client data current on that date) by throwaway probes in the dirty scratch area that use the repository's decoders as tools. **All 49,636** landblock-info records of regions 1–5 and 14 decoded (0 failures): every property set in them (3,253,862 SED sets, 3,768,840 instance property sets, 1,324,469 link sets, 104,271 cell sets, 49,636 landblock sets); **all 40,643** 0x47 entity descriptors, **all 383,332** 0x79 and **all 1,616** 0x78 property sets of `client_gamelogic.dat`; the property dictionary 0x34000000 (all definitions whose name contains `@`, all `ScriptTable_*` definitions and the definitions named in this spec); all 7,526 0x07 and 28,992 0x0C records (7,524 and 28,992 decode, as in `script-table.md` §10.3), scanned for nodes that read the properties of this spec; the enum mappers named in this spec; the WLib class library 0x56000000 (all attribute name hashes). |
| BIN | **M4, [decision 0011](../decisions/0011-standing-m4-authorisation.md)**, 2026-10-08. Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035, read with the Ghidra 12 decompiler on a private copy of the shared analysis project (deleted after use), address ranges only, in the dirty scratch area. **Questions:** (1) where the client reads `Entity_InstanceScriptTable` and how it turns the property tree into script definitions; (2) how those definitions enter an entity's script lookup (order, kinds, `Script_CombinationType`); (3) how an entity's class and additional tables and start-up scripts are gathered; (4) what a driven field gets when its source property is absent; (5) whether the client treats `@` names specially; (6) which light fields `Light_LODClass` and `Light_DrivenByDayCycle` fill. **Why M1–M3 were not enough:** the data hold the trees and the graphs but not how the client merges them with the entity's tables, which definition wins, or when the tree is applied; no published source describes instance script tables; no running client was available to this session, and observing one (M3) could show a door opening but not the merge rule. **Read:** the script component's set-up from an entity's properties, its activation and its property-change handler for the two properties it watches, the property-tree script parser, the node and target builders, the table-set constructor, copy and include-list setter, the table-set lookup, the script start, the driven-field application at node start, the entity property read's dispatch by `field_b`, the light class's field registration. Nothing of the network, login, launcher or anti-tamper code. |
| ST, SR, PS, ED, LB, LI, EN, SN | `script-table.md`, `script-runtime.md`, `property-system.md`, `entity-descriptor.md`, `landblock.md`, `lighting.md`, `environment.md`, `sound.md` as companion specs. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| I1 | ISTs occur in 83,200 instance property sets (275 SED-IID, 82,925 server-only), in the own property sets of 275 SEDs and in 8 0x47 descriptors; in no link, cell, landblock, 0x78 or 0x79 set (§2) | OBS | Confirmed 2026-10-08 |
| I2 | Every SED IID has exactly one instance property set; the SED's own set equals it or is a subset of it (2,298,059 equal, 955,794 subset) (§2.1) | OBS; ED §4.5.2 | Confirmed 2026-10-08 |
| I3 | The IST is a struct with `ScriptTable_Scripts` and `ScriptTable_StartupScriptNames`; the schema of §3.2 is the dictionary's child lists, and every value of the install fits it (§3) | OBS (dictionary; 3,047,352 entries in all ISTs) | Confirmed 2026-10-08 |
| I4 | Node kinds over all ISTs (instance sets and SED own sets, 184,451 nodes): Run 184,329, SetProperty 55, Particle 11, Timer 11, Collection 9, Sequence 8, Scriptlet 6, MeshFX 6, EntityVFX 5, Animation 3, Sound 3, Switch 2, Appearance 2, Random 1; nesting at most one level of children (§3.4) | OBS | Confirmed 2026-10-08 |
| I5 | 184,214 Link targets name a link; in 182,928 a Script link (type 12) of the same record with that `name` has the IST owner as `end_id_2`; 50 match only as `end_id_1`; 1,236 match no link with the owner as an end (§3.5) | OBS | Confirmed 2026-10-08 |
| I6 | The client builds an entity's table set from `Entity_ClassScriptTable_Override` or else `Entity_ClassScriptTable`, then `Entity_AdditionalScriptTables_Override` or else `Entity_AdditionalScriptTables`; null and repeated DIDs are dropped (§4.1) | BIN | BIN |
| I7 | Start-up order on activation: `Startup` (5), then the script named by `Entity_PlayerPhysicsScript` if non-zero, then the class table's `ScriptTable_StartupScriptNames`, then the IST's, in stored order; a key with a live run is not started twice (§4.2) | BIN | BIN |
| I8 | The IST's scripts are parsed when the script component's listener for `Entity_InstanceScriptTable` reports a value; they go into the entity's own copy of its table set, a key replacing any earlier instance definition of the same key (§4.3) | BIN | BIN; whether a value present at activation is reported at once: OPEN (§13 Q1) |
| I9 | Lookup: instance definitions of a key are taken first, as kind-2 roots, before the included tables' kind-2 roots; an instance definition whose merged per-script properties carry `Script_CombinationType` = 1 (`Overriding`) hides every included table's definition of that key (§4.4) | BIN | BIN |
| I10 | Node building from a tree: kind from `ScriptTable_NodeType` through the client's kind registry, fields from `ScriptTable_NodeProperties` by PID, ports only of types the kind declares, targets from `ScriptTable_NodeTargets`; a node that fails is left out of its list (§4.5) | BIN | BIN |
| I11 | The 319 dictionary names containing `@` are not bound to WLib attributes: hashing the member part with `records.md` §1.5 matches no attribute hash of any class for 315 names; 4 generic member names also occur as attribute names of unrelated classes; the client contains none of these names (§5.1) | OBS (all 319 names, all 2,554 attribute hashes); BIN (string search) | Confirmed 2026-10-08 |
| I12 | Member overrides and instance switches are read by ordinary script nodes (Switch or When `…_Property`, driven sources, SetProperty) of the tables of the object the entity is built from; census of readers and coverage in §5.3, §7, §8 | OBS | Confirmed 2026-10-08 |
| I13 | Day and night lights of placed objects: 36 descriptors reach table 0x07000689 or 0x07001A46, whose `Startup` runs scriptlet 0x0C000FCC to pick script 0x10000794 (day) or 0x10000235 (night); each holds a Switch on `…Day.m_bLightActivated` or `…Night.m_bLightActivated` and a Light node with seven driven fields (§6) | OBS | Confirmed 2026-10-08 |
| I14 | All `Light.Script@Entity.*` values sit on Physics SEDs and their instance sets, on 9 server-only sets and on 34 descriptors; none on any Light SED; `Light_DrivenByDayCycle` sits only on Light SEDs and no script node reads it (§6.1, §6.6) | OBS | Confirmed 2026-10-08 |
| I15 | A driven field is written only when the entity property read reports a value; otherwise the node keeps its stored field (§6.4) | BIN (driven application); ED E26 (the read's two sources) | BIN; that the read reports no value for an absent property (no dictionary default) is Inferred, §13 Q2 |
| I16 | `Light_LODClass` (PID 0x1DC) and `Light_DrivenByDayCycle` (0x707) fill two fields of the client's light object, its level-of-detail class and its day-cycle switch: the light class registers those fields with these PIDs, as it does for the fields of `lighting.md` §8.3 (§6.6, §8.2) | BIN | BIN; their consumers OPEN |
| I17 | The entity property read chooses its store by the definition's `field_b`: 4, 3 and 8 are three stores of the entity, 6 the run-parameter set, 7 and 9 a service lookup (§6.2, §12) | BIN (the read's dispatch) | BIN |

---

<a id="s2"></a>
## 2. Where the properties live (Confirmed census, 2026-10-08)

<a id="s2-1"></a>
### 2.1 Instance property sets and SEDs

The weenie list of a landblock-info record (`landblock.md` §4.8) holds 3,768,840 instance
property sets. 3,253,853 carry the IID of a SED of the same record, every SED IID exactly once
(9 SEDs repeat the IID of another SED of the same record). The remaining 514,987 are
server-only. For every SED, its own embedded property set is equal to its instance property
set (2,298,059 SEDs) or a strict subset of it with equal values (955,794); never anything
else. `entity-descriptor.md` §9.2 (E26) states that an entity built from a SED reads its
per-instance properties from the instance property set when there is one, so the instance
property set is the authoritative per-instance layer; the SED's own copy matters only to a
reader that ignores the weenie list.

<a id="s2-2"></a>
### 2.2 Census by context

Counts are property sets that carry the property at the top level.

| Property or family | SED-IID sets | Server-only sets | 0x47 descriptors | 0x79 sets | Other contexts |
|---|---|---|---|---|---|
| `Entity_InstanceScriptTable` | 275 | 82,925 | 8 | 0 | none (links, cells, landblock sets, 0x78) |
| `Light.Script@Entity.Night.m_cLightDiffuse` | 12,774 | 9 | 34 | 0 | none |
| `Light.Script@Entity.Day.m_fLightRange` | 6,188 | 9 | 34 | 0 | none |
| `TimeOfDay@Entity.m_eTimeOverride` | 42,429 | 20,094 | 421 | 24 | none |
| `TimeOfDay@Entity.m_bIgnoreInsideOutside` | 1,940 | 21 | 125 | 0 | none |
| `State@Entity.m_bIsActive` | 2,426 | 56 | 1 | 0 | none |
| `Toggle.Script@Entity.m_bOpen` | 9 | 1,490 | 1 | 0 | none |
| `vfx_Fire_Color` | 7,551 | 151 | 1 | 0 | none |
| `vfx_Fx_Switch` | 1,316 | 272 | 127 | 0 | none |
| `vfx_Color` | 476 | 195 | 41 | 0 | none |
| `vfx_Water_Color` | 372 | 0 | 43 | 0 | none |
| `Script_LightSource_Enable` | 278 | 583 | 10 | 0 | none |
| `Script_ViewMountMeshFX` | 0 | 461 | 0 | 0 | none |
| `ScriptSystem_LightsActivated` | 188 | 367 | 38 | 0 | none |
| `ScriptSystem_TimeofDay_Activation` | 378 | 0 | 0 | 0 | none |
| `Light_LODClass` | 9,992 | 53 | 7 | 0 | none |
| `Light_DrivenByDayCycle` | 868 | 0 | 0 | 0 | none |

No instance property set, SED set, link set, cell set or landblock set carries
`Entity_ClassScriptTable`, `Entity_AdditionalScriptTables` or either `…_Override` property; an
instance names extra script behaviour only through the IST. The `…_Override` properties occur
nowhere in the install.

The SED-IID sets are what a client builds its static world from. The server-only sets are the
templates of objects a server creates; a client sees their properties only when a server sends
them (§9.5).

---

<a id="s3"></a>
## 3. `Entity_InstanceScriptTable`: a script table as a property tree

<a id="s3-1"></a>
### 3.1 Encoding

The value is an ordinary property value (`property-system.md` §4): structs in the table form
(TSize header, entries of u32 key, u32 PID and value, in bucket order of the struct's hint),
arrays in the list form (u32 count, elements of u32 PID and value). Nothing beyond
`property-system.md` is needed to walk it; this section gives the meaning of each level. All
struct headers in ISTs of this install use hint 0 (184,360 node structs, every other struct
too), and every struct is in bucket order.

<a id="s3-2"></a>
### 3.2 Schema

The dictionary's child lists close every level except the three property bags, which are open
(`property-system.md` §2.4). "Type" is the property type code (`property-system.md` §3).

| Property (PID) | Type | Children (dictionary) | Meaning |
|---|---|---|---|
| `Entity_InstanceScriptTable` (0x430) | 11 struct | `ScriptTable_Scripts`, `ScriptTable_StartupScriptNames` | The instance table |
| `ScriptTable_Scripts` (0x7E) | 12 array | `ScriptTable_ScriptEntry` | The scripts |
| `ScriptTable_StartupScriptNames` (0x301) | 12 array | `ScriptTable_ScriptID` | Script keys to start at activation (§4.2) |
| `ScriptTable_ScriptEntry` (0x438) | 11 struct | `ScriptTable_ScriptID`, `ScriptTable_ScriptProperties`, `ScriptTable_ScriptRoots` | One script definition |
| `ScriptTable_ScriptID` (0x25B) | 8 enum (mapper 0x23000097) | — | The script key, as in a 0x07 record (`script-table.md` §2.1) |
| `ScriptTable_ScriptProperties` (0xAF7) | 11 struct, open | any | The per-script property set (`script-table.md` §2.4) |
| `ScriptTable_ScriptRoots` (0x2AB) | 12 array | `ScriptTable_Node` | Root nodes of the script |
| `ScriptTable_Node` (0x356) | 11 struct | `ScriptTable_NodeType`, `ScriptTable_NodeProperties`, `ScriptTable_NodeOutputs`, `ScriptTable_NodeTargets` | One node |
| `ScriptTable_NodeType` (0x56A) | 8 enum (mapper 0x2300009E) | — | Node kind: the codes and labels of `script-table.md` §5.1 |
| `ScriptTable_NodeProperties` (0x437) | 12 array, open | any | Field values of the node or of a target, one element per field, each element's PID being the field's bound PID (§3.3) |
| `ScriptTable_NodeOutputs` (0x5B9) | 12 array | `ScriptTable_NodePort` | Output ports |
| `ScriptTable_NodePort` (0x17E) | 11 struct | `ScriptTable_PortType`, `ScriptTable_PortProperties`, `ScriptTable_Nodes`, `ScriptTable_Node` | One port |
| `ScriptTable_PortType` (0x2AA) | 8 enum (mapper 0x230000A1) | — | Port type (`script-table.md` §4.3) |
| `ScriptTable_PortProperties` (0x300) | 12 array, open | any | Port payload fields, one element per field |
| `ScriptTable_Nodes` (0x5B7) | 12 array | `ScriptTable_Node` | Child nodes of a port |
| `ScriptTable_NodeTargets` (0x5B8) | 12 array | `ScriptTable_TargetPort` | Targets of a node |
| `ScriptTable_TargetPort` (0x436) | 11 struct | `ScriptTable_PortType`, `ScriptTable_PortProperties`, `ScriptTable_NodeTargetEx`, `ScriptTable_NodeTarget`, `ScriptTable_TargetType` | One target with its port |
| `ScriptTable_NodeTargetEx` (0xD2) | 11 struct | `ScriptTable_TargetType`, `ScriptTable_NodeProperties`, `ScriptTable_NodeTargets` | The target proper: its type, its data fields, its sub-targets |
| `ScriptTable_TargetType` (0x2D) | 8 enum (mapper 0x2300009F) | — | Target type: the codes and labels of `script-table.md` §4.4 |
| `ScriptTable_NodeTarget` (0x658) | 12 array | (none) | Declared; no value in the install |
| `Script_DrivenProperties` (0x220) | 12 array | `Script_DrivenPropertyEntry` | Inside `ScriptTable_NodeProperties`: the node's driven map |
| `Script_DrivenPropertyEntry` (0x318) | 11 struct | `Script_DrivenProperty` (0x5D9), `Script_DriverProperty` (0x669) | One driven pair: the field PID and the source PID (`script-table.md` §4.2 field 1) |
| `LinkTarget_Name` (0x449) | 1 string | — | Inside a Link target's `ScriptTable_NodeProperties`: the link name |

<a id="s3-3"></a>
### 3.3 Correspondence with the binary node model

A tree node carries the same information as a node of a 0x07 record (`script-table.md` §4),
in a different shape:

| Binary node (`script-table.md`) | Tree form |
|---|---|
| kind byte (§4.1) | `ScriptTable_NodeType` |
| kind-specific data (§5): fields in a fixed order, each bound to a PID | one element of `ScriptTable_NodeProperties` per field, the element's PID being the field's PID; order free; a field may be absent (§4.5, §13 Q3) |
| base block field 1, driven map (§4.2) | an element `Script_DrivenProperties` in `ScriptTable_NodeProperties` |
| base block fields 4–7 (`UseParentIntensity`, `IntensityMultiplier`, `IntensityMultiplierVariance`, `Probability`) | presumably elements with the PIDs bound to those fields (`Script_UseParentIntensity`, `Script_IntensityMultiplier`, `Script_Probability`, …); none occurs in the install |
| base block fields 2, 3, 8 (scriptlet-driven map, scriptlet parameters, node ID) | no counterpart in the install |
| ports (§4.3): type, payload, children | `ScriptTable_NodeOutputs` elements: `ScriptTable_PortType`; payload fields as `ScriptTable_PortProperties` elements (for example `SequenceScriptPort_Order`, `RandomScriptPort_Weight`, `TimerScriptPort_Time`, `SwitchScriptPort_Values`, `SwitchScriptPort_Operator`, `SwitchScriptPort_RampOut`, `SwitchScriptPort_Delay`, `SwitchScriptPort_Names`); children in `ScriptTable_Nodes` (or one child in `ScriptTable_Node`) |
| targets (§4.4): port type, value, target type, target data, sub-targets | `ScriptTable_NodeTargets` elements: `ScriptTable_PortType` and `ScriptTable_PortProperties` for the port side; `ScriptTable_NodeTargetEx` with `ScriptTable_TargetType`, the data fields in its `ScriptTable_NodeProperties` (for a Link target the one element `LinkTarget_Name`) and sub-targets in its `ScriptTable_NodeTargets` |
| SetProperty value PV (§5.2) | an element whose PID is the property being set (for example `Render_Draw` with a boolean), next to `SetPropertyScript_PropertyName` |

<a id="s3-4"></a>
### 3.4 Census

**Sets.** Instance property sets with an IST: 83,200. Of these, 13,997 hold an empty struct, 8
hold only `ScriptTable_StartupScriptNames` (one key each), 2 hold an empty
`ScriptTable_Scripts`, and 69,193 hold at least one script. Scripts per set: 1 (51,294), 2
(11,259), 3 (3,454), 4 (1,129), 5 (776), 6 (509), 7 (197), 8 (137), 9 (153), 10 or more (285).
Among the 275 SED-IID sets: 192 empty structs, 2 empty arrays, 81 with scripts. The 8 0x47
descriptors: 5 empty, 2 with start-up names only, 1 with an empty `ScriptTable_Scripts`.

**Script entries.** 104,531 in instance sets (plus 81 in SED own sets). No set repeats a key.
`ScriptTable_ScriptProperties` is present in 61,453 entries and holds exactly
`Script_ScriptLinkEnd_Persistent` = false and `Script_Comment` (an empty string) in all, plus
`Script_CombinationType` = 1 (`Overriding`) in 2. 337 distinct keys occur; by their labels in
mapper 0x23000097, 90,235 entries use keys of the script-link event family (labels starting
`ScriptLinks@`, for example `ScriptLinks@Relay::OnOutput` 18,546, `ScriptLinks@Generator::OnCleared`
6,627, `ScriptLinks@NPC::OnDeath` 6,428, `ScriptLinks@Usage::OnUsed` 4,816), 51 use `Startup`
(5), and the rest other keys. Roots per entry: 1 (69,943), 2 (22,435), 3 (5,177), 4 (2,698),
5 (1,213), 6 (732), 7 (441), 8 (330), 9 (294), 10 or more (1,268).

**Nodes.** 184,315 root nodes and 45 child nodes in instance sets; 91 root nodes in SED own
sets. Children occur only one level deep. Every node struct has `ScriptTable_NodeType` and
`ScriptTable_NodeProperties`; 184,221 have `ScriptTable_NodeTargets`, 25 have
`ScriptTable_NodeOutputs`.

| Kind | Code | Nodes (instance sets) | Fields seen in `ScriptTable_NodeProperties` |
|---|---|---|---|
| Run | 5 | 184,240 | `RunScript_ID` (all), `RunScript_Persistent` (183,576, all false), `RunScript_Stop` (4, all true) |
| SetProperty | 40 | 55 | `SetPropertyScript_PropertyName` (55), `SetPropertyScript_ServerOnly` (9, true), the value PV (render, selection, cosmetic, stealth, floaty-text and `State@Entity.m_bState_1`–`_4` properties) |
| Particle | 59 | 11 | `ParticleScript_ParticleID` (11), `…_BoneOffset`, `…_DropEffect`, `…_BoneType` |
| Timer | 65 | 11 | `TimerScript_Duration` (8) |
| Collection | 2 | 9 | none |
| Sequence | 6 | 8 | none |
| Scriptlet | 19 | 6 | `ScriptletScript_File` (6) |
| EntityVFX | 3 | 5 | `EntityVFXScript_Type` and colour, opacity, rotation, light-shaft and silhouette waveforms |
| MeshFX | 41 | 4 | `MeshFXScript_Entity`, `…_Persistent`, `…_BoneOffset`, `…_BoneType`, `…_HoldingLocation`, `…_Scale` |
| Animation | 36 | 3 | `AnimationScript_AnimAsset`, `…_Layer`, `…_RampIn`, `…_RampOut`, `Script_DrivenProperties` (6 driven pairs in all) |
| Sound | 48 | 3 | `SoundScript_SoundID` |
| Switch | 12 | 2 | `SwitchScript_Property`, `SwitchScript_Persistent` (false) |
| Appearance | 58 | 2 | `AppearanceScript_Appearance`, `…_Key`, `…_Modifier` |
| Random | 17 | 1 | none |

SED own sets add 89 Run and 2 MeshFX root nodes. Totals over every IST: 184,451 nodes.

**Ports.** 43 ports on 25 nodes: Sequence (6) and Collection (2) ports of type 6 with
`SequenceScriptPort_Order` (27), Timer ports of type 6 with `TimerScriptPort_Time` (10), Random
ports of type 6 with `RandomScriptPort_Weight` (2), Switch ports of type 5 with value, operator,
names, ramp-out and delay (2) and of type 6 with ramp-out and delay (2).

**Targets.** 184,221 targets in instance sets, all with port type 6 and an empty
`ScriptTable_PortProperties`, no sub-targets and no `ScriptTable_NodeTarget`: Run nodes with
target type 7 (`Link`) 184,208 and 13 (`This`) 7; SetProperty nodes with type 7, 6. Every Link
target carries one non-empty `LinkTarget_Name`. SED own sets: 82 Link and 6 This targets.

**References.** Every DID inside an IST names an existing record: clips 0x05 (3), appearances
0x20 (2), particle descriptors 0x39 (11), scriptlets 0x0C (6), sound info 0x2A (3), entity
descriptors 0x47 (6).

<a id="s3-5"></a>
### 3.5 Link targets and landblock links

A Link target names a link of `landblock.md` §4.6 by its `name`. For the 184,214 Link targets
of instance sets, searching the links of the same landblock-info record:

| Match | Targets |
|---|---|
| a link of that name has the IST owner as `end_id_2` (field 4 of `landblock.md` §4.6) | 182,928 (one such link 181,870; two 980; three 78) |
| a link of that name has the owner only as `end_id_1` | 50 |
| no link of that name has the owner as an end (433 of them: no link of that name in the record) | 1,236 |

Every matching link has link type 12 (`Script`). So the script-link pattern is: an instance
defines a script under an event key; when that key starts on the instance, its Run node
starts `RunScript_ID` on the entity at `end_id_1` of each Script link that bears the target's
name and starts at the instance (`end_id_2`). This also settles, for Script links, which end
of `landblock.md` §4.6 is the source (§12).

> **OPEN (Q4):** how the client resolves a Link target (which links it searches, how it treats
> several links of one name, the 50 reversed and 1,236 unmatched names, links that cross
> landblocks). Not traced. **Decision (Tessera):** the targets are the `end_id_1` entities of
> all Script links of the owner's landblock-info record whose `name` equals the target name
> (byte comparison) and whose `end_id_2` is the owner, in link order; none found means the node
> has no target and does nothing.

---

<a id="s4"></a>
## 4. How the client uses an instance script table (BIN)

<a id="s4-1"></a>
### 4.1 The entity's table set

When an entity's script component is first activated, it reads, through the entity's
property lookup (`entity-descriptor.md` §9.2: the per-instance set, then the object's chain):

1. `ScriptTable_ScriptChannels` (a DID; channel set-up, outside this spec).
2. The class table: `Entity_ClassScriptTable_Override` if present, otherwise
   `Entity_ClassScriptTable`. From the struct it takes `ScriptTable_DBScriptTable` (a 0x07 DID;
   the null DID is ignored) as the first table, and appends its `ScriptTable_StartupScriptNames`
   keys to the start-up list.
3. From `Entity_InstanceScriptTable` it reads **only** `ScriptTable_StartupScriptNames` here,
   appending its keys to the start-up list after the class table's.
4. The additional tables: `Entity_AdditionalScriptTables_Override` if present, otherwise
   `Entity_AdditionalScriptTables`. Each element that is a DID value, not null and not already
   in the list is appended.
5. With exactly one table, the entity shares a cached, read-only table set for that DID; with
   several, it gets a new read-only table set whose include list is the gathered DIDs. The
   include-list setter removes null and repeated DIDs by moving the list's last element into
   the removed slot, so the order can change when a duplicate occurs (none occurs in practice:
   step 4 already skips repeats). With none, the entity gets a new, empty and writable table
   set.

A start-up key element must be an enum or property-ID value (type 8 or 10); any other element
type is reported as an error and skipped. Elements of `Entity_AdditionalScriptTables` that are
not DIDs are skipped.

The table set has the shape of a 0x07 table: its **own** script definitions (initially none)
and per-script property sets, and the ordered **includes** of step 5. Its own definitions are
where the IST's scripts go (§4.3).

<a id="s4-2"></a>
### 4.2 Start-up scripts

When the entity is activated (on every activation path except the one that re-attaches scripts
that are already running), the component starts, in this order:

1. key 5 (`Startup`);
2. the key held by `Entity_PlayerPhysicsScript` (PID 0xA9D), when non-zero;
3. the start-up list of §4.1 (class table's keys, then the IST's keys), in stored order.

A key that already has a live run on the entity does not get a second run: in one of the
component's two modes the client reports it as a start-up script defined more than once and
starts nothing, in the other it hands the start to the existing run (that hand-over was not
traced further). Every other start follows `script-runtime.md` §12.1.

In the install, `Startup` is defined in the ISTs of 51 instance sets (and of 8 SEDs' own sets);
`ScriptTable_StartupScriptNames` occurs in 8 instance sets (one key each, all server-only) and
in 2 descriptors.

<a id="s4-3"></a>
### 4.3 Merging the IST's scripts

At activation the component registers itself as a listener for `Entity_InstanceScriptTable`
(and for `ScriptTable_ScriptChannels`) on the entity, **before** it starts the start-up scripts
of §4.2. When the listener reports a value of `Entity_InstanceScriptTable`:

1. If the entity's table set is the shared, read-only one, the component replaces it by a
   private copy (same includes, same own definitions, writable).
2. The value must be a struct; its `ScriptTable_Scripts` must be an array. For each element in
   stored order:
   1. The element must be a struct with a `ScriptTable_ScriptID` whose value is an enum or a
      property ID (type 8 or 10); otherwise the element is reported and skipped.
   2. Each element of `ScriptTable_ScriptRoots` is built into a node (§4.5); a root that fails
      to build is left out. The remaining roots, in order, become the definition.
   3. If the table set already holds an own definition with that key, it is removed; the new
      definition is inserted. So within one IST a repeated key keeps the **last** definition
      (no IST of the install repeats a key).
   4. If the element has `ScriptTable_ScriptProperties` as a struct, its fields become the
      own per-script property set of that key.
3. The included tables are not touched.

> **OPEN (Q1):** whether registering the listener reports the value already present on the
> entity (so that an IST stored in the instance property set is merged at activation, before
> `Startup` runs), or only later changes. The registration call carries a flag whose meaning
> was not traced. The component has a second mode, in which it starts the start-up scripts
> already during set-up and registers no listener at all; which entities use which mode was
> not traced either. **Decision (Tessera):** merge the IST of the entity's per-instance set at
> activation, after building the table set (§4.1) and before the start-up scripts (§4.2); merge
> again, from step 1, whenever the property changes.

<a id="s4-4"></a>
### 4.4 Lookup of a script key with instance definitions

`script-runtime.md` §12.2 describes how a key is looked up through an entity's tables. With
the table set of §4.1 the client does this:

1. Start three root lists (kinds 1, 2 and 4) and an empty merged per-script property set.
2. If the table set has an **own** definition of the key (an instance script): merge its own
   per-script property set into the merged set, and append its roots to the **kind-2** list.
   Then, if the merged set holds `Script_CombinationType` as an enum or property-ID value equal
   to 1 (`Overriding`), stop: the included tables are not searched.
3. Otherwise (no own definition, or not `Overriding`): for each included table in include-list
   order, gather its definitions of the key through its closure exactly as `script-runtime.md`
   §12.2 says, appending kind-2 roots to the kind-2 list (after the instance roots), kind-4 roots
   to the kind-4 list and kind-1 roots to the kind-1 list, and merging per-script properties.
4. The run starts the kind-1 roots, then kind-2, then kind-4 (`script-runtime.md` §12.1 step 6);
   `Script_MaxNumInstances` and the other per-script settings are read from the merged set.

So the include order of an entity's tables is **class table first, then the additional tables
in stored order** (this replaces the "Inferred" order of `script-runtime.md` §12.2), and an
instance definition runs **in addition to** the class and additional tables' definitions of
the same key unless it is `Overriding` (2 entries in the install do that).

<a id="s4-5"></a>
### 4.5 Building a node from a tree

For a `ScriptTable_Node` struct:

1. `ScriptTable_NodeType` and `ScriptTable_NodeProperties` must both be present; the kind value
   must be an enum or property ID. The kind is looked up in the client's node-kind registry
   (the classes of `script-table.md` §5.1); an unregistered kind fails the node.
2. The node's fields are set from the elements of `ScriptTable_NodeProperties`: every element
   goes into a property set that is applied to the node's reflected fields (each field is bound
   to a PID, `script-table.md` §5); elements of property-ID type (type 10, for example
   `SetPropertyScript_PropertyName`) are handed to the node's field object one by one first, and
   a refusal fails the node.
3. Ports: for each `ScriptTable_NodePort`, the port type (an enum or property ID; otherwise
   reported and taken as 0) is matched against the port declarations of the node's kind; a port
   is created only for a declared type and only while the node holds fewer ports of that type
   than the declaration allows (unlimited for most). The port's payload fields are set from
   `ScriptTable_PortProperties`, and its children built from `ScriptTable_Node` (one child) or
   `ScriptTable_Nodes` (a list), recursively. Whether a refused port is reported was not traced.
4. Targets: each `ScriptTable_TargetPort` gives the port type and payload (as for ports) and a
   `ScriptTable_NodeTargetEx` with the target type, its data fields from its
   `ScriptTable_NodeProperties` and sub-targets from its `ScriptTable_NodeTargets`, built the
   same way.
5. A node that fails is left out of the list that would have held it.

> **OPEN (Q3):** the value of a node field that `ScriptTable_NodeProperties` does not give (the
> node object's initial value). The data cannot show it, and the node classes' initial values
> were not traced. In the install, 664 Run nodes give no `RunScript_Persistent` (all others store
> false), and no Run node gives `RunScript_UnbindOnDeath`, `RunScript_ExposeAsScriptLink` or
> `RunScript_QueuingType`. **Decision (Tessera):** an absent field takes its PID's dictionary
> default when the definition has one (`RunScript_Persistent` true, `RunScript_QueuingType` 3,
> the other two false), otherwise the zero value of its type (`script-table.md` T28 reading).

<a id="s4-6"></a>
### 4.6 What runs where

The client evaluates instance definitions like any other definition: when a key starts on the
entity (`script-runtime.md` §12), its instance roots run. The script-link keys of §3.4 are
started when the corresponding game event happens on the object (a relay firing, an NPC dying,
an object being used); judging by the key labels (Inferred), most of those events come from
server-side game logic, so in an offline client most instance scripts never start. The ones
that can start offline are the `Startup` definitions (51 instance sets) and keys started by
the object's own tables.

---

<a id="s5"></a>
## 5. Member-override properties

<a id="s5-1"></a>
### 5.1 Naming and what they are

319 dictionary names contain `@`. Their shape is *Owner*`@`*Holder*, followed by `.` or `::`
and a member part: `Light.Script@Entity.Day.m_fLightRange`, `Toggle.Script@Entity.m_bOpen`,
`State@Entity.m_bIsActive`, `TimeOfDay@Entity.m_eTimeOverride`, `VO@NPC.m_eDemeanor`,
`Behavior@NPC::Vitals::DisplayGroundFX`, `Emote.Sit@Chair.m_bChairInUse`. The member part
often starts with a type letter (`m_b` boolean, `m_c` colour, `m_f` float, `m_e` enum, `m_i`
integer, `m_w` waveform, `m_iid` instance ID) and the dictionary type agrees.

They are **ordinary properties**. Nothing maps them to WLib class members: hashing the member
part (or the whole name, or the owner part) with `records.md` §1.5 gives no attribute hash of
any WLib class (`wlib.md` §8) for 315 of the 319 names. The other 4 (`m_iidEntity`,
`m_iidPlayer`, `m_eRace`, `m_iCount`) are generic member names that also occur, with a
compatible value type, as attributes of six unrelated classes; nothing found links the
properties to those attributes. The client executable contains none of the 319 names, so it
cannot parse them. The names follow a convention (Inferred from the names and their readers):
the "owner" is a script module, the "holder" the kind of object whose state it is, the member
the variable. Script-key labels use the same
convention (`ScriptLinks@NPC::OnDeath`).

An implementation therefore treats a member override like any property: stored in the
per-instance set (or on the object), found by the entity's property lookup
(`entity-descriptor.md` §9.2), read by script nodes (§5.3). No special parser is needed.

<a id="s5-2"></a>
### 5.2 Families (Confirmed census, instance property sets)

| Family | Properties stored (instance sets) | Largest counts | Readers (0x07/0x0C records with a node reading them) |
|---|---|---|---|
| `Light.Script@Entity.Day.*` / `…Night.*` | 16 (8 per variant) | Night: `m_cLightDiffuse` 12,783, `m_fLightRange` 12,722, `m_fLightFalloff` 12,614, `m_wLightDiffuseIntensity` 12,565, `m_cLightSpecular` 3,966, `m_bLightActivated` 588, `m_bKeyLight` 59, `m_bShadowCaster` 38. Day: `m_fLightRange` 6,197, `m_wLightDiffuseIntensity` 3,012, `m_bLightActivated` 2,859, `m_fLightFalloff` 2,744, `m_cLightDiffuse` 2,727, `m_cLightSpecular` 149, `m_bKeyLight` 76, `m_bShadowCaster` 18 | 0x07000689, 0x07001A46 (§6) |
| `TimeOfDay@Entity.*` | 2 | `m_eTimeOverride` 62,523; `m_bIgnoreInsideOutside` 1,961 | 0x07001A4E, scriptlets 0x0C000FCC and 0x0C0082D6 and every table that invokes them |
| `State@Entity.*` | 18 | `m_bIsActive` 2,482; `m_eFacing` 115; `m_bState_1` 76; `m_bRenderLights` 65; `m_fCount` 50; `m_iCount_NetShared` 50; `m_iState_1` 43; `m_bIsActivated` 38 | `m_bIsActive`: Switch nodes in 64 records, written by 150 SetProperty nodes; `m_bRenderLights`: 0x07001041, 0x07001060; others: a few records each |
| `Toggle.Script@Entity.*` | 10 | `m_bOpen` 1,499; `m_bTurnOffPhysicsOnOpen` 302; `m_iCountdown` 194; `m_bBecomeUnselectableOnUse` 33; `m_eCountdownType` 31; `m_bTurnOffPhysicsOnClose` 26; `m_iPersistentActivateDelay` 25 | `m_bOpen`: Switch nodes in 5 records, written by 8 SetProperty nodes; the other Toggle members: no Switch or driven reader |
| `RandomBox@Entity.*`, `LogicBox@Entity.*` | 5 | `m_bRepeatsAllowed` 718; `m_eCondition1` 50 | none in 0x07/0x0C |
| `VO@NPC.*` | 2 | `m_eDemeanor` 353 (and 36,750 0x79 sets); `m_eRace` 238 (5,947 0x79 sets) | none in 0x07/0x0C |
| `Behavior@NPC::…`, `Emote.Sit@Chair.*`, `MPControl@Entity.*` | 3 | `DisplayGroundFX` 675 (1,220 in 0x79); `m_bChairInUse` 187; `m_eOwnershipScriptID` 146 | few |
| `Detection.Monster@Entity.*`, `MiniGame@Volume.*` | 0 | `m_eDetectedGenus` in 23,189 0x79 sets; one `MiniGame@Volume` value in a 0x78 set | — |

The dictionary types and defaults of the families used offline are in §6.3 (light) and §7.

<a id="s5-3"></a>
### 5.3 How they take effect

A member override changes behaviour only through a node that reads it:

- a **Switch** or **When** whose `SwitchScript_Property` is the override: the node branches on
  the per-instance value; an absent property reads as its dictionary default, else as the zero
  value of its type (`script-runtime.md` §6.2, R27);
- a **driven pair** whose source is the override: at node start the field takes the per-instance
  value when the entity has one, else keeps the node's stored field (§6.4, I15);
- a **SetProperty** whose `SetPropertyScript_PropertyName` is the override: the node writes the
  variable (state machines of doors and switches).

Coverage over the SEDs' own sets (the object a SED places reaches, through its class and
additional tables and their includes and scriptlets, a node that reads the property):
`Light.Script@Entity.*` read for all but 6 SEDs per property; `TimeOfDay@Entity.m_eTimeOverride`
read for 29,278 of 42,429 (for the others no node of the object's tables reads it; any other
use by the client, `environment.md` §7.3, is outside this spec);
`TimeOfDay@Entity.m_bIgnoreInsideOutside` 1,920 of 1,940; `State@Entity.m_bIsActive` 2,399 of
2,426; `Toggle.Script@Entity.*` on SEDs: none read (the 9 SEDs that store `m_bOpen` place
objects whose tables do not read it). Most Toggle, State, RandomBox and LogicBox values sit on
server-only sets; a client meets them only as properties a server sends (§9.5).

---

<a id="s6"></a>
## 6. Day and night lights of placed objects (Confirmed structure; answers `lighting.md` §8.3 OPEN (b))

<a id="s6-1"></a>
### 6.1 Which entities

36 0x47 descriptors name one of the two **light-glow tables** as their class table
(`Entity_ClassScriptTable`): 35 name 0x07000689, 1 (0x470064BE) names 0x07001A46; no
descriptor reaches them through additional tables or includes. 12,826 Physics SEDs place one
of these descriptors: 12,740 place 0x4700165A, 71 place 0x470064BE, 15 place two of the
others. Of the 12,826, 12,768 carry `Light.Script@Entity.Night.m_cLightDiffuse` in their
instance set, 39 carry other `Light.Script@` values but no night diffuse, 19 carry none. These are lamps, lanterns, lit
windows and similar objects. All `Light.Script@` values in the install sit on Physics SEDs,
their instance sets and these descriptors (I14); Light SEDs (`lighting.md` §8.3) carry none.

<a id="s6-2"></a>
### 6.2 Choosing day or night

Both tables define `Startup` (5) as a Scriptlet node invoking scriptlet 0x0C000FCC (in
0x07001A46 under a Switch that runs it only while `QuestState_PhasedOut` is false). The
invocation binds four of the scriptlet's parameter names to persistent Run nodes: two of them
run script 0x10000794 (label `Light_Glow_Off`, the **day script**), two run 0x10000235 (label
`Light_Glow_On`, the **night script**). The scriptlet decides which names fire
(`script-runtime.md` §6, §7, §15, §17 give the node rules). Its structure in this install:

| `TimeOfDay@Entity.m_eTimeOverride` (mapper 0x23000161, default 2) | Further test | Scripts that run |
|---|---|---|
| 3 `Never` | — | day |
| 1 `Night` | — | night |
| 6 `Day` | — | day |
| 4 `Dusk` | — | day and night |
| 7 `Dawn` | — | day and night |
| 5 `Always` | — | night |
| 2 `Automatic` (also when absent) | `TimeOfDay@Entity.m_bIgnoreInsideOutside` (default false) false and `Volume_IsOutside` = 0 (inside) | none |
| 2 `Automatic` | `m_bIgnoreInsideOutside` false and `Volume_IsOutside` = 1, or `m_bIgnoreInsideOutside` true: `World_PresentTimeOfDay` *t* | *t* ≤ 0.25: night; 0.25 < *t* ≤ 0.35: day and night; 0.35 < *t* ≤ 0.7: day; 0.7 < *t* ≤ 0.77: day and night; *t* > 0.77: night |
| 2 `Automatic` | `m_bIgnoreInsideOutside` false and `Volume_IsOutside` neither 0 nor 1 (unset, 255) | none |

The thresholds 0.25, 0.35, 0.7, 0.77 and the comparisons (`GreaterThan` 6, `LessThanOrEqual`
1 of mapper 0x230000A3) are data of scriptlet 0x0C000FCC and must be read from it at run time,
not hard-coded. `World_PresentTimeOfDay` (0x682) is the day fraction the client publishes
(`environment.md` §11); `Volume_IsOutside` (0x53B, tri-state, default 255) is supplied by the
client (`sound.md` §10.3.5, OPEN there). Both have `field_b` 3, so the client reads them from
the entity's engine-supplied store, not from the instance set. The Switches on
`m_eTimeOverride`, `Volume_IsOutside` and the When nodes on the time are persistent (the
Switch on `m_bIgnoreInsideOutside` is not: it is evaluated once per start), so a change
switches the running script: the persistent Run node of the old case is stopped (which
stops the script it started, `script-runtime.md` §12.4) and the new case's Run node starts its
script.

<a id="s6-3"></a>
### 6.3 The day and night scripts

Each script (in both tables) is one persistent Switch on its `m_bLightActivated` member with
a single value case `true`, whose child is a Light node (kind 13, `script-table.md` §5.3). The
Light node's stored fields and driven pairs are the same in all four scripts except that the
day script's sources are the `Day` members and the night script's the `Night` members:

| Light field (field PID) | Stored value in the node | Driven from (`Day` / `Night` member) | Dictionary default of the member |
|---|---|---|---|
| `LightDiffuse` (0x3FD) | white (255, 255, 255, 255) | `…m_cLightDiffuse` | white |
| `LightRange` (0x3FE) | 10.0 | `…m_fLightRange` | 25.0 (min and max also defined) |
| `LightFalloff` (0xF5) | 1.0 | `…m_fLightFalloff` | 1.0 |
| `LightDiffuseIntensity` (0x4F0) | constant 0.5 (waveform subtype 1) | `…m_wLightDiffuseIntensity` | constant 0.5 |
| `LightSpecular` (0x4EF) | white | `…m_cLightSpecular` | white |
| `KeyLight` (0x455) | false | `…m_bKeyLight` | false |
| `ShadowCaster` (0x31D) | false | `…m_bShadowCaster` | false |
| (Switch property, not a light field) | — | `…m_bLightActivated` | **Day: false; Night: true** |

The other stored fields of the four Light nodes: direction (0, 0, 0), projector and corona
off, `Light_AllowBakedShadows` true, `LightScript_Lifespan` 0, `LightScript_FadeTime` 0.25,
`LightScript_Offset` (0, 0, 0), the five `LightScript_` flags false, `LightScript_HoldingLocation`
0. While a Light node runs, the entity has that light (light record rules: `lighting.md` §8.3);
when its script stops, the node stops and the light goes.

> **OPEN (Q6):** the Light node's own run-time rules: what `LightScript_FadeTime` (0.25 here)
> and `LightScript_Lifespan` do, how `LightScript_Offset` and the holding location place the
> light. Not traced. **Decision (Tessera):** add and remove the light at once, at the entity's
> position (offset 0).

**The full list of varying fields** is therefore the seven driven fields above, plus
`m_bLightActivated`, per variant. No other `Light.Script@Entity` member exists in the
dictionary.

<a id="s6-4"></a>
### 6.4 Values when a member is absent

Every member is looked up on the entity: the per-instance set first, then the placed object's
chain (`entity-descriptor.md` §9.2, E26). The two descriptors that 12,811 of the 12,826 SEDs
place (0x4700165A, 0x470064BE) hold no `Light.Script@` value. The other 34 descriptors hold
six members per variant (`m_bLightActivated` = true, `m_cLightDiffuse`, `m_cLightSpecular`,
`m_fLightFalloff`, `m_fLightRange`, `m_wLightDiffuseIntensity`; 6 of them also
`Night.m_bShadowCaster` = false), but only 15 SEDs place them.

- **`m_bLightActivated`** is read by the Switch: when the lookup finds nothing it reads the
  dictionary default. So on the common descriptors the night script lights the lamp by default
  (Night default true) and the day script does not (Day default false); on the 34 others both
  are true unless the instance says otherwise. 2,859 SED-IID sets set `Day.m_bLightActivated`
  (true 639, false 2,220); 588 set `Night.m_bLightActivated` (true 571, false 17).
- **The seven driven fields** are written at node start only when the entity's property read
  reports a value (BIN, I15). When neither the instance set nor the object chain holds the
  member, the field keeps the Light node's **stored** value (range 10, not the dictionary's 25;
  Decision Q2 below). In the install this happens for the lamps of 0x4700165A and 0x470064BE
  whose instance sets lack a member: for example the range of a lamp without
  `Night.m_fLightRange`, and the `KeyLight` and `ShadowCaster` of every lamp that does not set
  them.
- A driven field also follows later changes of its source on the entity
  (`script-runtime.md` §5.3).

> **OPEN (Q2):** whether the entity's property read reports a value (the dictionary default)
> for a property that neither the per-instance set nor the object chain holds. E26 names only
> the two sources; the per-entity store's read routine was not traced. The answer changes the
> range of a lamp whose instance set and descriptor both lack the range member (10 or 25).
> **Decision (Tessera):** no value; the field keeps the node's stored value.

<a id="s6-5"></a>
### 6.5 Census of the 12,826 light-glow SEDs (SED-IID sets)

| `m_eTimeOverride` | `Day.m_bLightActivated` | `Night.m_bLightActivated` | SEDs |
|---|---|---|---|
| absent (2) | absent (false) | absent (true) | 9,743 |
| absent | false | absent | 1,858 |
| absent | true | absent | 440 |
| absent | false | true | 362 |
| absent | absent | true | 109 |
| absent | true | true | 98 |
| absent | true | false | 17 |
| 5 `Always` | absent / true | absent / true | 91 / 7 / 2 |
| 2 `Automatic` | absent / true | absent | 21 / 69 |
| 1 `Night` | absent | absent | 9 |

`TimeOfDay@Entity.m_bIgnoreInsideOutside` is set on 1,940 SED-IID sets (true 1,773): interior
lamps that should follow the clock even inside. Night values over all instance sets: diffuse
223 distinct colours,
range 87 distinct (8.5 in 2,898, 4 in 2,224, 9 in 1,914), falloff 0.1–1.0, intensity 71
distinct waveforms (mostly subtype 7 noise, a flicker). Day range is 1.0 in 3,387 sets (a lamp
that is not activated by day anyway).

<a id="s6-6"></a>
### 6.6 Relation to Light SEDs and `Light_DrivenByDayCycle`

`Light_DrivenByDayCycle` (0x707) is stored on 868 Light SEDs (true on 250) and nowhere else,
and no script node reads it. It is a field of the client's light object (I16), so it concerns
the Light SED's own light; its effect (the run-time scale σ of `lighting.md` §8.3 is a
candidate) was not traced (Q5, §8.2). It does **not** select the `Light.Script@Entity` variants:
those never occur on Light SEDs. A Light SED keeps its own `LightDiffuse`, `LightRange` and
the other fields of `lighting.md` §8.3 at all times of day, unless its time-of-day visibility
(`TimeOfDay@Entity.m_eTimeOverride` on 149 Light SEDs, `environment.md` §7.3) removes it.

---

<a id="s7"></a>
## 7. Other member overrides an offline client meets

| Property | Type, default | SED-IID sets | Read by | Effect |
|---|---|---|---|---|
| `TimeOfDay@Entity.m_eTimeOverride` | enum 0x23000161, default 2 `Automatic` | 42,429 (5: 35,501; 3: 4,978; 2: 1,559; 1: 389; 6: 2) | scriptlets 0x0C000FCC, 0x0C0082D6 (and the tables invoking them), table 0x07001A4E | time-of-day selection of §6.2 for objects whose tables use these scriptlets; for others, `environment.md` §7.3 |
| `TimeOfDay@Entity.m_bIgnoreInsideOutside` | bool, default false | 1,940 | the same two scriptlets | §6.2 |
| `State@Entity.m_bIsActive` | bool, no default (absent reads false) | 2,426 (true 2,349) | Switch nodes in 64 records; 150 SetProperty nodes write it | Switch branches (active or inactive effect sets) and SetProperty state changes |
| `State@Entity.m_bRenderLights` | bool, no default | 0 (65 server-only) | 0x07001041, 0x07001060 | Switch |
| `Toggle.Script@Entity.m_bOpen` | bool, default false | 9 (1,490 server-only) | 0x07001579, 0x070006CB, 0x07000F44, 0x070013B8, 0x0700157B | open or closed branch of a toggling object |

---

<a id="s8"></a>
## 8. Instance VFX and lighting switches

<a id="s8-1"></a>
### 8.1 Switch properties

| Property (PID) | Type | Default | Values in instance sets (all) | Records reading it | Coverage on SEDs |
|---|---|---|---|---|---|
| `vfx_Fire_Color` (0x1000086E) | enum 0x2300014B (`Invalid` 0, `purple` 1, `white` 2, `blue` 3, `green` 4, `red` 5, `black` 6, `yellow` 7, `redDeep` 8) | none (0) | 5: 2,142; 3: 2,038; 4: 1,794; 1: 752; 8: 317; 0: 239; 2: 229; 6: 153; 7: 38 | 17 tables (Switch; 3 SetProperty nodes write it) | 7,351 of 7,551 |
| `vfx_Fx_Switch` (0x10001597) | enum 0x230001D5 (`Invalid` 0, `On` 1, `Off` 2) | none | 1: 894; 2: 618; 0: 76 | 104 tables (Switch; 7 SetProperty nodes write it) | 925 of 1,316 |
| `vfx_Color` (0x10002D4C) | enum 0x2300014B | none | 3: 396; 8: 63; 5: 60; 2: 55; 4: 51; 6: 29; 7: 9; 0: 7; 1: 1 | 25 tables | 223 of 476 |
| `vfx_Water_Color` (0x100015B5) | enum 0x230001D7 (`Invalid` 0, `Default` 1, `Green` 2) | none | 2: 370; 1: 2 | 33 tables | 367 of 372 |
| `Script_LightSource_Enable` (0x100010A5) | bool | none (false) | true 547, false 314 | 38 tables | 261 of 278 |
| `Script_ViewMountMeshFX` (0x10005023) | bool | false | true 461 (server-only) | 25 tables, among them the avatar class tables 0x07000417–0x0700041D | — |
| `ScriptSystem_LightsActivated` (0x100004D9) | tri-state | 255 | 1: 335; 0: 193; 255: 27 | 11 records (Switch; 9 SetProperty nodes write it) | 98 of 188 |
| `ScriptSystem_TimeofDay_Activation` (0x10000876) | tri-state | 255 | 255: 376; 0: 2 | none | 0 of 378 |

"Coverage on SEDs" counts the SEDs whose own set carries the property and whose placed
object's tables read it. A Switch on one of these properties takes the per-instance value
when present and otherwise the default or zero value (`script-runtime.md` §6.2); a Switch with
no matching case and no Default port runs nothing. The values select variants of the effects
the object's tables define (fire colours, effect on or off, water colour); the variants
themselves are the tables' nodes (particles, mesh effects, lights), interpreted by the specs
of those node kinds.

`ScriptSystem_TimeofDay_Activation` is read by no node of the install and its PID occurs as
no instruction operand in the client's code (BIN search). On SEDs whose
`ScriptSystem_LightsActivated` is not read by their object's tables (90 of 188), it has no
offline effect found.

> **OPEN (Q7):** the meaning of `ScriptSystem_TimeofDay_Activation` (378 SED-IID sets, value
> 255 in 376). **Decision (Tessera):** ignore.

<a id="s8-2"></a>
### 8.2 Light fields stored as properties

`Light_LODClass` (0x1DC, enum mapper 0x2300008D, default 3) is stored on 9,941 Light SEDs, 51
Physics SEDs and their instance sets (10,045 instance sets in all, mostly 0x10000010: 9,405). It
fills the light object's level-of-detail class field (I16). No script node reads it; how the
client uses a light's LOD class (presumably to cull or fade distant lights) was not traced.

> **OPEN (Q5):** what the client does with a light's level-of-detail class and day-cycle
> fields (I16). **Decision (Tessera):** ignore both, as `lighting.md` §8.3 does today for the
> run-time scale σ.

---

<a id="s9"></a>
## 9. Algorithms in words (Tessera)

<a id="s9-1"></a>
### 9.1 Building an entity's script tables

1. Find the entity's per-instance set: the instance property set with the entity's IID in its
   landblock-info record, else the SED's own set (`entity-descriptor.md` §9.2).
2. Read, through the entity's property lookup, the class table, the class table's start-up
   names, the IST's start-up names and the additional tables (§4.1). Build the include list:
   class table DID first (if not null), then each additional DID not yet present.
3. Create the entity's table set with no own definitions and that include list.
4. Merge the IST (§9.2) under the Decision of §4.3 Q1.
5. Start key 5, then `Entity_PlayerPhysicsScript` if non-zero, then the start-up list (§4.2),
   skipping keys with a live run.

<a id="s9-2"></a>
### 9.2 Merging an IST

1. If the value is not a struct, or it has no `ScriptTable_Scripts` array, stop.
2. For each script entry in order: skip it if it is not a struct or its key is missing or not
   an enum or property-ID value; build each root node (§9.3) and keep those that build; replace
   any own definition of that key with the new one; if `ScriptTable_ScriptProperties` is a
   struct, make its fields the key's own per-script property set.
3. Invalidate any cached lookups of the entity (the table set changed).

<a id="s9-3"></a>
### 9.3 Building a node

1. Require `ScriptTable_NodeType` (an enum or property-ID value naming a kind that Tessera's
   evaluator implements or can skip, `script-runtime.md` §18) and `ScriptTable_NodeProperties`;
   otherwise the node fails.
2. Start the node with the field values of Decision Q3 (§4.5); set each field whose PID
   appears in `ScriptTable_NodeProperties` to that element's value. A `Script_DrivenProperties`
   element becomes the driven map (pairs of `Script_DrivenProperty`, `Script_DriverProperty`).
   For SetProperty, the element whose PID equals `SetPropertyScript_PropertyName` is the value
   to set.
3. Build ports and targets as in §4.5 steps 3–4; for a Link target take `LinkTarget_Name` from
   the target's `ScriptTable_NodeProperties` and resolve it as §3.5 decides.
4. Bound the work against hostile input: nesting depth (the install needs 2 levels of nodes;
   a limit of 32 is ample, together with `property-system.md` §5.3's value-depth limit), and
   the counts of `property-system.md` §5.3.

<a id="s9-4"></a>
### 9.4 Day and night lights

For an entity whose tables reach 0x07000689 or 0x07001A46, running the tables with the script
evaluator of `script-runtime.md` reproduces §6 exactly. A world renderer without a full
evaluator may compute the same result directly:

1. Read `m_eTimeOverride` (default 2) and, for 2, `m_bIgnoreInsideOutside` (default false),
   the entity's indoor or outdoor state and the day fraction *t*; decide by the table of §6.2
   (taking thresholds and comparisons from scriptlet 0x0C000FCC) whether the day script, the
   night script, both or neither run.
2. For each running variant whose `m_bLightActivated` (per-instance value, else object value,
   else the dictionary default) is true, add one point light at the entity with the fields of
   §6.3: each field from the variant member found by the entity's lookup, else the Light
   node's stored value (Decision Q2).
3. Re-evaluate when *t* crosses a threshold, when the indoor or outdoor state changes, or when
   any of the members changes.

The light then follows the light-record rules of `lighting.md` §8.3 like any other point
light.

<a id="s9-5"></a>
### 9.5 Spawned objects (server or sandbox)

A server that creates an object from a server-only instance property set sends that set's
properties with the object, the IST, member overrides and switches included, so that the
client builds the same table set (§9.1) and its tables see the same variables. An
implementation must not drop properties it does not understand: they are inputs of the
object's scripts.

<a id="s9-6"></a>
### 9.6 Explorer view

For a selected SED or instance property set, a per-instance script view shows:

1. the entity's table set as §4.1 builds it: the class table and the additional tables in
   include order, then the instance definitions of the IST (§4.3), each script with its key
   (and the key's label from mapper 0x23000097), its per-script properties and its node tree,
   using the same node display as for 0x07 records;
2. for each Link target, the target name and the entities it resolves to (§3.5), with their
   object DIDs, or a mark when it resolves to none;
3. the member overrides of the set grouped by owner and holder (§5.1), each with the value
   that the entity's lookup finds and where it comes from (instance set, object chain or
   dictionary default), and the records of the object's closure that read it (§5.3);
4. the instance switches of §8 with their enum labels and the reading tables;
5. for light-glow objects (§6), the day and night light that §9.4 gives for a chosen day
   fraction and indoor or outdoor state.

---

<a id="s10"></a>
## 10. Invariants (Confirmed 2026-10-08 unless marked)

- **IS1:** every IST value decodes with `property-system.md` and fits the schema of §3.2 (every
  struct field and array element PID is a listed child, apart from the three open bags).
- **IS2:** every `ScriptTable_ScriptEntry` has `ScriptTable_ScriptID` (an enum value) and
  `ScriptTable_ScriptRoots`; no IST repeats a script key.
- **IS3:** every `ScriptTable_Node` has `ScriptTable_NodeType` and `ScriptTable_NodeProperties`;
  every kind value is a registered kind of `script-table.md` §5.1.
- **IS4:** every target has port type 6, an empty `ScriptTable_PortProperties` and target type 7
  or 13; every type-7 target holds exactly one non-empty `LinkTarget_Name`.
- **IS5:** every matching link of a Link target (§3.5) has link type 12.
- **IS6:** every DID in an IST names an existing record.
- **IS7:** an IST, a member override or a switch of §2.2 occurs only in instance property sets,
  SED sets, 0x47 descriptors and (for the families shown) 0x79 sets.
- **IS8:** every SED's own property set equals its instance property set or is a subset of it
  with equal values.
- **IS9:** every `Light.Script@Entity.*` value sits on a Physics SED (or its instance set or a
  server-only set or a 0x47 descriptor); none on a Light SED; for each such property, every
  Physics SED that stores it except 6 places an object whose tables reach 0x07000689 or
  0x07001A46.
- **IS10:** no script node of the install reads `Light_DrivenByDayCycle`, `Light_LODClass` or
  `ScriptSystem_TimeofDay_Activation`.
- **IS11 (BIN):** an instance definition is looked up before the included tables'
  definitions of the same key and runs with them unless it is `Overriding`.

---

<a id="s11"></a>
## 11. Worked synthetic examples (for tests)

The bytes use this install's PIDs; a test builds its own dictionary with these PIDs and types
(§3.2; `RunScript_ID` 0x26E enum, `RunScript_Persistent` 0x189 bool), or remaps them.

<a id="s11-1"></a>
### 11.1 A script-link IST (181 bytes, one top-level table entry)

One script under key 0x10000001 whose single root is a Run node that starts key 0x10000002,
not persistent, on the entity at the other end of the Script link named `LinkA`:

| Offset | Bytes | Meaning |
|---|---|---|
| 0 | `30 04 00 00 30 04 00 00` | table key and PID 0x430 (`Entity_InstanceScriptTable`) |
| 8 | `00 01` | struct: hint 0, 1 field |
| 10 | `7E 00 00 00 7E 00 00 00` | key and PID 0x7E (`ScriptTable_Scripts`) |
| 18 | `01 00 00 00` | array of 1 element |
| 22 | `38 04 00 00` | element PID 0x438 (`ScriptTable_ScriptEntry`) |
| 26 | `00 02` | struct: hint 0, 2 fields (bucket order: 0x2AB mod 11 = 1, 0x25B mod 11 = 9) |
| 28 | `AB 02 00 00 AB 02 00 00 01 00 00 00` | `ScriptTable_ScriptRoots`, array of 1 |
| 40 | `56 03 00 00` | element PID 0x356 (`ScriptTable_Node`) |
| 44 | `00 03` | struct: hint 0, 3 fields |
| 46 | `6A 05 00 00 6A 05 00 00 05 00 00 00` | `ScriptTable_NodeType` = 5 (Run) |
| 58 | `B8 05 00 00 B8 05 00 00 01 00 00 00` | `ScriptTable_NodeTargets`, array of 1 |
| 70 | `36 04 00 00 00 03` | element `ScriptTable_TargetPort`, struct of 3 fields |
| 76 | `AA 02 00 00 AA 02 00 00 06 00 00 00` | `ScriptTable_PortType` = 6 (`Default`) |
| 88 | `D2 00 00 00 D2 00 00 00 00 02` | `ScriptTable_NodeTargetEx`, struct of 2 fields |
| 98 | `2D 00 00 00 2D 00 00 00 07 00 00 00` | `ScriptTable_TargetType` = 7 (`Link`) |
| 110 | `37 04 00 00 37 04 00 00 01 00 00 00` | target's `ScriptTable_NodeProperties`, array of 1 |
| 122 | `49 04 00 00 05 4C 69 6E 6B 41` | element `LinkTarget_Name` = `LinkA` (VLE length 5) |
| 132 | `00 03 00 00 00 03 00 00 00 00 00 00` | `ScriptTable_PortProperties` (0x300), empty array |
| 144 | `37 04 00 00 37 04 00 00 02 00 00 00` | node's `ScriptTable_NodeProperties`, array of 2 |
| 156 | `6E 02 00 00 02 00 00 10` | `RunScript_ID` = 0x10000002 |
| 164 | `89 01 00 00 00` | `RunScript_Persistent` = false |
| 169 | `5B 02 00 00 5B 02 00 00 01 00 00 10` | `ScriptTable_ScriptID` = 0x10000001 |

Expected result with a landblock-info record holding a Script link (type 12) named `LinkA`
with `end_id_2` = the owner and `end_id_1` = entity B: the owner's table set gets one own
definition of 0x10000001 with one Run root; looking up 0x10000001 yields that root first in
the kind-2 list, followed by any definitions in the owner's class and additional tables;
starting 0x10000001 on the owner sends a start request for 0x10000002 to B, which B handles at
the next scheduler pass (`script-runtime.md` §12.4 step 2), and the Run node completes at once
(`Persistent` false). With a second Script link named `LinkA` from the owner to entity C, the
Decision of §3.5 starts 0x10000002 on B and on C, in link order. Adding
`Script_CombinationType` = 1 to the entry's `ScriptTable_ScriptProperties` makes the lookup of
0x10000001 return the instance root only.

<a id="s11-2"></a>
### 11.2 A lamp at night and by day

Entity: a Physics SED whose object's class table is 0x07000689; its instance set holds only
`Light.Script@Entity.Night.m_fLightRange` = 8.5 and `…Night.m_cLightDiffuse` = (87, 213, 255,
255); its descriptor holds no `Light.Script@` value (Decision Q2 applies).

| Situation | Scripts running | Light |
|---|---|---|
| outdoors, *t* = 0.10 | night | one point light: diffuse (87, 213, 255, 255), range 8.5, falloff 1.0, intensity constant 0.5, specular white, key light off, no shadow |
| outdoors, *t* = 0.50 | day | none (`Day.m_bLightActivated` absent, default false) |
| outdoors, *t* = 0.30 | day and night | the night light only (the day Switch has no case for false) |
| inside a cell, *t* = 0.10, `m_bIgnoreInsideOutside` absent | none | none |
| same, with `m_bIgnoreInsideOutside` = true | night | the night light |
| any *t*, `m_eTimeOverride` = 5 | night | the night light |
| outdoors, *t* = 0.50, with `Day.m_bLightActivated` = true added | day | one point light with all stored values: white, range **10.0**, falloff 1.0, intensity 0.5 |

---

<a id="s12"></a>
## 12. Notes for other specs (for their writers; this spec does not edit them)

- **`lighting.md` §8.3**, Tessera Decision on σ and the implementer OPEN (b) of 2026-10-06:
  the day and night variants are not properties of Light SEDs and are not gated by
  `Light_DrivenByDayCycle`. They are per-instance variables of placed Physics objects read by
  the Light nodes of tables 0x07000689 and 0x07001A46 (§6). The varying fields are exactly
  `LightDiffuse`, `LightRange`, `LightFalloff`, `LightDiffuseIntensity`, `LightSpecular`,
  `KeyLight` and `ShadowCaster`, plus the activation switch; the values for an absent member
  are in §6.4. `tessera-world`'s reading of `…m_fLightRange` and `…m_bLightActivated` on Light
  SEDs with `Light_DrivenByDayCycle` never matches data (no Light SED carries a variant).
  `Light_LODClass` and `Light_DrivenByDayCycle` are light-object fields (I16) whose consumers
  are still open.
- **`environment.md` §7.3** says lights carry day and night variants "on 2,700 – 12,800 light
  entities"; they are on placed Physics objects (lamps, windows), not on Light SEDs (§6.1).
- **`script-table.md` §8.1** item 1 counts `Entity_InstanceScriptTable` on 8 descriptors; the
  property is also on 83,200 instance property sets (§2.2), and the client merges it into the
  entity's table set as own kind-2 definitions (§4.3–§4.4).
- **`script-runtime.md` §12.2**: the order of an entity's tables is BIN now (class table, then
  additional tables in stored order, repeats and nulls dropped, §4.1); instance definitions
  come first among the kind-2 roots, and an `Overriding` instance definition hides the tables'
  definitions (§4.4). Start-up order: §4.2.
- **`landblock.md` §4.6** OPEN "which end is which": for Script links (type 12), `end_id_2` is
  the entity whose instance script names the link and `end_id_1` the entity the script runs on
  (182,928 of 184,214 link targets, §3.5).
- **`property-system.md` §2.3**: `field_b` is the store the client reads a property from (I17:
  3, 4 and 8 are three stores of the entity, 6 the run-parameter set, 7 and 9 a service
  lookup); this spec relies on it for `Volume_IsOutside` and `World_PresentTimeOfDay`
  (`field_b` 3) and for the member overrides (`field_b` 4).

---

<a id="s13"></a>
## 13. Open questions

| # | Question | Section | Decision until answered |
|---|---|---|---|
| Q1 | Does registering the IST listener report the value already present (merge at activation, before `Startup`)? | §4.3 | merge at activation, before the start-up scripts |
| Q2 | Does the entity property read report the dictionary default for a property that neither the per-instance set nor the object chain holds? | §6.4 | no: a driven field keeps the node's stored value |
| Q3 | Initial values of node fields that a tree node does not give | §4.5 | dictionary default of the field's PID, else zero |
| Q4 | Client resolution of Link targets (search scope, several links of one name, reversed or unmatched names, cross-landblock links) | §3.5 | all Script links of the record with that name starting at the owner, in link order |
| Q5 | What the client does with a light's `LODClass` and `DrivenByDayCycle` fields | §6.6, §8.2 | ignore both (as `lighting.md` does today for σ) |
| Q6 | The Light node's own run-time rules (`LightScript_FadeTime`, `LightScript_Lifespan`, `LightScript_Offset`, holding location) | §6.3 | add and remove the light at once; offset 0 |
| Q7 | Meaning of `ScriptSystem_TimeofDay_Activation` (378 SED-IID sets, read by no node) | §8.1 | ignore |

---

<a id="s14"></a>
## 14. Change log

| Date | Change |
|---|---|
| 2026-10-08 | First version (spec writer, dirty room): census over every landblock-info record, 0x47, 0x78 and 0x79 set; schema and node mapping of `Entity_InstanceScriptTable`; BIN of the client's table set, start-up order, IST merge, lookup and node building; member overrides as ordinary properties; day and night lights; instance switches; examples; Q1–Q7. Status Review. |
