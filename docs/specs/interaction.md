<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Interactive world objects: hotspots, the use pipeline, channelling and defeat

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate session, not the author) |
| **Review record** | Review 1, 2026-10-07: changes requested. Review 2, 2026-10-07: changes requested. Review 3, 2026-10-08 (separate dirty-room review session, not the author): **passed**, status Accepted (four minor notes). Revision 4, 2026-10-08 (spec writer): Accepted text changed (§21), status Revised. Review 4, 2026-10-08 (separate dirty-room review session, not the author): **passed**, status Accepted (minor notes only). Details in the review log (decision 0018). |
| **Implements into** | A gameplay-data layer on `crates/props` and `crates/wstate` (decoders and stores for the record kinds of §4–§12; crate placement is the implementer's choice); the client (cursor, use-range test and cast bar of §3.4 and §3.8.4, cosmetic-pet following of §7.4); `crates/net` sandbox backend (use, channel and defeat flows, §16); `apps/explorer` (hotspot and channelling cards, hotspot overlay, §17); `apps/dattool` (census commands) |
| **Companion specs** | [`wstate.md`](wstate.md) (object records, bundles, links, available-property lists); [`wlib.md`](wlib.md) (class library, method tables §6.4, bytecode chunk §4.1); [`property-system.md`](property-system.md) (property sets, labels, DID maps §8); [`items.md`](items.md) §5.10 (usage behaviour objects and requirements on items); [`creatures.md`](creatures.md) (templates, generator chain §6); [`combat-data.md`](combat-data.md) (effects, progressions §4, induction and channelling durations §7.9); [`quests.md`](quests.md) §6.3 (effect-application objects, class 700); [`map-data.md`](map-data.md) (map-note and quest-marker tables §4–§5, place keys §3 and §10); [`entity-descriptor.md`](entity-descriptor.md) §4.9 (`Entity_CreationType`); [`collision.md`](collision.md) (ethereality §9.2, entity cylinders); [`appearance.md`](appearance.md) §5.5 (held props) |

---

<a id="purpose"></a>
## Purpose

Static entities draw placed world objects, but nothing so far lets a player act on them.
This spec gives an implementer what the data files hold for that, and what the client does
with it where the executable settles it:

1. the **use pipeline** (§3): how an object names its usage action, cursor, context menu,
   permissions, requirements, induction (cast bar), channelling state and the effects it
   starts; the usage-action, mouse-over, context-menu, permission, plugin, induction and
   channelling records that the pipeline points at; the client's own range test and cursor
   choice;
2. **hotspots** (§4), the largest group of interactive world objects (area-effect objects
   such as campfires and crafting facilities): their template layout, their compiled effect
   list in the object record, their instance properties and where they are placed;
3. smaller groups: server-side controller entities (§5), held-prop loadouts of creatures
   (§6), cosmetic pets and their client-side following (§7), defeat and rally-point records
   (§8), multi-user activity stations (§9), plaques and level-up actions (§10), target
   markings (§11) and the player trigger-response set (§12);
4. the WEENIECONTENT labels involved (§13), the dictionary families (§14) and the reference
   graph (§15);
5. the rules a Tessera sandbox server and the explorer follow where the data stop (§16–§17).

It also marks the **boundary with the bytecode**: usage plugins and usage actions have
little or no data; their behaviour is the script code of their classes (§3.7, §3.2.3). That
code is out of scope here.

Class indices, DIDs and counts are **observations** of one install (§1). Code finds records
by the rules of §2.2 and reads every value at run time; it never hard-codes the observed
class indices or DIDs.

---

<a id="provenance"></a>
## 1. Provenance

Marks:

- **Confirmed**: observed in the user's install by throwaway probes in the dirty-room
  scratch area, over **every** record of the kind named, on the date given.
- **Reported**: from published third-party material only.
- **BIN**: read from the client executable under method M4 (decision 0011).
- **Decision**: a Tessera choice where the data and the client are silent.

| Key | Source |
|---|---|
| OBS | User install (Steam), read-only, 2026-10-07. Probes built on a snapshot of the project's own decoders (read-only use) and Python, over: all 383,343 object records (prefixes 0x70 and 0x76) with their root classes and ancestry, and the property sets (0x79) of every 0x70 record; all 0x78 property sets; the class library 0x56000000 (class table, method tables, the bytecode chunk searched for 32-bit PID literals); the property dictionary 0x34000000; the WEENIECONTENT DID map 0x2800000E and the root DID map; every enum mapper named here; all 49,636 landblock-info records (`client_cell_*.dat`: landblock property sets, static entities and their property sets, all instance property sets of the weenie lists, all link property sets). Every object record of §2.1 was decoded completely (no unknown native class). Revision 2 (same day) added probes over all 383,332 0x79 sets for `Physics_GameRadiusOverride` and `Render_AdjustableScale`, over all 1,779 induction and 401 channelling records for the setback and modifier structures, over every `Mod_Entry` in every 0x79 set whose `Mod_Modified` is one of the induction or channelling modifier or ignore properties, and over the two setback progressions. |
| LC | LotroCompanion `lotro-tools` (read for facts only; no code or prose taken): its cosmetic-pet loader (the chain from a skill's toggle effects through an effect's `Mod_Array` to a cosmetic-entity record and on to a creature template) and its map-marker classifier (crafting facilities appear as items or as hotspots). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`), analysed headless in the dirty room on 2026-10-07, on a copy of the project's analysis database (deleted after use). **Questions:** (a) which properties decide the distance at which the player may use an object, and how that distance is measured; (b) when the client shows the "out of range" mouse-over cursor; (c) which properties the client reads to show another entity's induction (cast) bar; (d) how the client moves a cosmetic pet that follows its owner; (e) which of the interaction PIDs of §14 occur as constants in the client code at all; (f) which WEENIECONTENT keys the client resolves for usage actions. **Why M1–M3 were not enough:** the data hold the property values but not the formulas, defaults or fallbacks that combine them; several of the properties involved are never stored in the data (`Usage_UsageExpireDist`, `Mouseover_Range`, the induction start and end times), so their use cannot be observed; an in-game observation (M3) could show one cursor change or one cast bar at a time but not the constants or the order of fallbacks. **Read:** a scan of all instructions for the 32-bit IDs of the 757 dictionary names of §14 (most hits in the 0x1000xxxx range turned out to be user-interface element IDs and were discarded after reading the routine), the use-range routine and the entity distance routine it calls, the mouse-over range routine and its two callers, the property getter they use (only to see where it looks), the induction-bar routine, the cosmetic-pet follow routines, and two routines that compare `Usage_Action` with WEENIECONTENT entries. **Revision 2 (same build, 2026-10-07, on a fresh copy of the analysis database, deleted after use):** (g) how the client builds each entity's cylinder for the gap distance (base radius, height and offset, and the radius override); (h) whether native client code computes induction durations from the modifier lists or applies setbacks; (i) what the shortcut-usability routine of §3.4.3 does with the resolved `UsageAction_DoNothing` record. **Why M1–M3 were not enough:** the cylinder is run-time state built from several sources with constants that no record stores; whether the client or the server does the induction arithmetic cannot be seen from the data; the running client is not available to this session. **Read:** the cylinder builder and the base-cylinder routine it calls, the bounding-box helper and the box-centre helper, three constants, the induction-property hits of the (e) scan and the routines around them, the shortcut-usability routine and the list walk it calls. Nothing from network, login, launcher or anti-tamper code was read. **Revision 3 (same build, 2026-10-08, on a copy of the analysis database, deleted after use):** (j) where the base radius *r*₀, height *h*₀ and scale *s* of §3.4.1 come from; (k) what the client does with a hovered entity and its out-of-range flag; (l) how the follow routine of §7.4 chooses and uses a trail point; (m) what the entity tree is that the outermost-parent rule (§3.4.2) and the shortcut routine (§3.4.3) walk; (n) whether the remaining (e) constant hits for mouse-over and loadout PIDs are property reads. **Why M1–M3 were not enough:** these are run-time rules of the client that no record stores, and the running client is not available to this session. **Read:** the model component's two size functions and the setup reader (to place the two fields), the physics component's scale routine, the mouse-over distance routine, the hover hand-off routines, the follow routine with its trail-point test, size getters, liquid probe and ground placement, the tree walk and the tree iterator, the WEENIECONTENT resolver, and the routines behind the (e) hits for `Mouseover_CursorOutOfRange`, `Mouseover_MouseoverList` and the three loadout PIDs (all four turned out to be user-interface element lookups, not property reads). Nothing from network, login, launcher or anti-tamper code was read. **Revision 4 (same build, 2026-10-08, on a copy of the analysis database, deleted after use):** (o) whether the entity property lookup used by the range test substitutes dictionary defaults; (p) how the client picks the hovered entity and when it recomputes the out-of-range flag; (q) what the native functions that the mouse-over class code calls do (cursor set and reset, entity lookup, set membership, level read); (r) who writes the pet trail of §7.4, when, and in which order; (s) which relation the entity tree of §3.4.2 records. **Why M1–M3 were not enough:** these are run-time rules of the client that no record stores, and the running client is not available to this session. **Read:** the hover update routine and its caller, the property lookup and the script-side getters it shares a dispatch with, the registered native functions named in §3.4.6, the pet registration routine, the trail insert routine and its two callers, the trail-point builder, and the outermost-ancestor walk (question (s) was not settled). Nothing from network, login, launcher or anti-tamper code was read. |
| BC | Class-library bytecode (record 0x56000000, chunk 1) of the user's install, disassembled in the dirty room on 2026-10-08 with the instruction set of [`wlib-bytecode.md`](wlib-bytecode.md) (Accepted), by throwaway probes; this is data (method M1). Read: the hover functions of the mouse-over controller class (class 277 here, functions 1, 2, 3, 5, 9, 13, 18, 21, 25; revision 4 also 6, 7, 8, 10, 11, 14, 15, 16, 22, 24 and the mouse-over directory class's functions 84–86), the usage-action virtual slots 94, 97 and 99 with every override, the entity slots 218 and 305 (common bodies), the activity-collection hover helpers, and a literal scan of the whole code block for the PIDs named in §3.8.5, §3.9.3, §4.6, §5.3, §6 and §14; the context-menu click handler (class 4 here, function 1) and the default, take and examine handlers' menu functions (§3.5); the class-1199 function of §6. Revision 4 (2026-10-08) added: slot 286 and slot 305 of every world-entity class, the slot-97 and slot-103 overrides of the seven usage-action classes of §3.4.5 with the functions they call (down to the quest-book functions, which were not read further), and the mouse-over directory functions. Native functions are unspecified (`wlib-bytecode.md` §7.1); where this spec says what one returns, that is inferred from its arguments and from the properties read on its result, and says so. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| I1 | Census of the 129 root classes and 4,998 object records in scope, and of the 1,816 pipeline records of §3 (§2.1) | OBS | Confirmed 2026-10-07 |
| I2 | Signature rules that find each group without class indices (§2.2) | OBS | Confirmed 2026-10-07 |
| I3 | Bundle-mirror rule: the object records of §3–§11 repeat parts of their property set in compiled form; the correspondences of §2.4 hold in every record | OBS | Confirmed 2026-10-07 |
| I3a | Attribute hashes and types of classes 701, 1626 and 3427 (class attribute lists, `wlib.md` §8) and the value of each attribute in every record (§2.4) | OBS (attribute lists, 2026-10-08; values of the class-701 and class-1626 attributes from the census of I14, 2026-10-07; the player action handler's bundle against its property set, 1 of 1, 2026-10-08) | Confirmed 2026-10-08 |
| I4 | Every `Usage_Action` value (195,930) names a record of the usage-action subtree; per-class referrer counts (§3.2) | OBS | Confirmed 2026-10-07 |
| I5 | Mouse-over directory layout; 39 mouse-over types, each with a cursor and an out-of-range cursor (§3.3) | OBS | Confirmed 2026-10-07 |
| I6 | Use-range test: explicit range, else `Usage_UsageExpireDist`, else `Usage_MinUsageDist` (3.0 when no value) plus 3.0 m; gap distance between the two entities' cylinders, each position moved by its cylinder's offset; the lookup's routing by `field_b` (§3.4.1) | BIN (a), (g), (o); OBS (neither `Usage_UsageExpireDist` nor any explicit range is stored) | BIN only |
| I6a | Cylinder of an entity: base radius = *r*₀ × scale + 0.05 m, base height = *h*₀ × scale + 0.1 m, offset = centre of the collision shape's bounding box in the entity's own frame; radius replaced by `Physics_GameRadiusOverride` × the first component of `Render_AdjustableScale` when the override is a float above 0 (§3.4.1) | BIN (g) | BIN only |
| I6c | *r*₀ and *h*₀ are the horizontal radius and the height of `physics_bounds` of the setup that the entity's model is built from; *s* = z of `Physics_AdjustableScale` × z of `Model_Scale` (each (1, 1, 1) when absent), replaced by 1 when not finite or below 0.0002; the offset is not scaled; with the outermost-parent flag every input comes from the outermost ancestor (§3.4.1, §3.4.2) | BIN (j), (m) | BIN only |
| I6b | `Physics_GameRadiusOverride` (float, default −1) is stored in 54 top-level 0x79 sets, all of mob templates (class 1723 here), values 0.5–13, all above 0; never nested; 52 of them store `Render_AdjustableScale` with three equal components (0.65–8), 2 do not; 3 landblock instance sets also store it (2, 3, 10) (§3.4.1) | OBS (all 383,332 0x79 sets; all instance sets of all landblock-info records) | Confirmed 2026-10-07 |
| I7 | Out-of-range cursor: shown when the local player's `Mouseover_Range` is positive and the gap distance to the hovered entity is at least that value (§3.4.2) | BIN (b), (p); OBS (`Mouseover_Range` is never stored) | BIN only |
| I7a | Cursor distance: both cylinders built with the outermost-parent rule; distance 0 when the hovered entity is the local player; no distance when the hovered entity cannot be found (§3.4.2) | BIN (b), (m) | BIN only |
| I7b | Hover hand-off to class code; mouse-over eligibility (override, visibility, phase, quest item, selectability), activity-collection type, usage-action type rule (pending targeted item, drama, plain) and its overrides (§3.4.4); every world-entity class shares the selectability and usage-action functions (§3.4.4) | BIN (k) for the hand-off; BC for the rules | BC (rules); BIN only (hand-off) |
| I7c | Hovered-entity choice (the player and its shapeshift entity are skipped), recomputation of the flag on every hover update (§3.4.2) | BIN (p) | BIN only |
| I7d | `Mouseover_Range` is written onto the local player by the class code when an entity is hovered: the entity's `Usage_MinUsageDist_ClientOverride` when present, else its `Usage_MinUsageDist`, else 2.0 (§3.4.2, §3.4.6); `Usage_MinUsageDist_ClientOverride` is never stored | BC (slot 286 of all 100 world-entity classes; controller function 7); OBS (all 0x79, 0x78 and landblock sets, prefix scan of 2026-10-07) | BC; Confirmed 2026-10-07 (not stored) |
| I7e | The extra rules of the seven usage-action classes that override the plain type rule, including the NPC service rule (§3.4.5); the quest-book, harmful-interaction, monster-play and house-keeper tests they call are named, not specified | BC; BIN (q) for the native functions named there | BC (rules); BIN (natives) |
| I7f | Cursor and effect step: directory lookup with the fallback from `Mouseover_CursorOutOfRange` to `Mouseover_Cursor`, hotspot offsets, cursor set and reset, the mouse-over effect script, `Mouseover_Entity`, `Mouseover_State`, `Mouseover_Script`, `Mouseover_ActiveScriptID` (§3.4.6) | BC; BIN (q) | BC; BIN (natives) |
| I8 | The client resolves `UsageAction_DoNothing` and `UsageAction_QuestNPC` through WEENIECONTENT keys; it compares the latter with an entity's `Usage_Action`; the shortcut routine discards the resolved `UsageAction_DoNothing` DID (the resolution yields a DID only and opens no record) and tests only that the target has a non-null `Usage_Action` and is the local player or one of its descendants in the client's entity tree, searched depth first (§3.4.3) | BIN (f), (i), (m); OBS (labels and keys) | BIN only |
| I9 | Context-menu handler, permission and take-permission records: layouts and censuses (§3.5, §3.6); handler choice and the reads of the click handler (§3.5) | OBS; BC (§3.5 click handler and menu functions) | Confirmed 2026-10-07; BC |
| I10 | Usage-plugin directory: 77 plugins, one per requirement property; directory map keyed by `UsagePlugin_UsagePropertyName`; plugin classes have no attributes and 4–21 functions of their own (§3.7) | OBS | Confirmed 2026-10-07 |
| I11 | Induction records: layout, census, bundle mirror (§3.8) | OBS | Confirmed 2026-10-07 |
| I11a | Induction modifier and setback data: `Induction_Duration_AreaChallengeLevelOverride` stored as float in all 1,779; every setback structure holds exactly one of a time (824 structures) or a progression (130); 763 records hold times only, 118 progressions only, none both; time records all use `ModifyStartTime`, progression records 114 `ModifyDuration` and 4 `ModifyStartTime`; event census; the two setback progressions (float, minimum index 1, 10 entries, non-zero only in the first one or two); setback times 0.7–20 and not related to the duration; modifier and ignore properties: types, defaults and the operators effects apply to them (§3.8.1, §3.8.5) | OBS (the `Mod_Entry` census of §3.8.5 corrected and re-run over all 0x79 sets on 2026-10-08) | Confirmed 2026-10-07, census 2026-10-08 |
| I11b | No native routine refers as a constant to the induction modifier lists, the setback array, policy, progression or ignore property, the area-challenge override or the setback count, and the bytecode holds no literal of them except the multiplier-list PID (two sites; the one read only hands the PID on); class 1195's own code only initialises its six attributes and returns the two modifier lists; the induction is timed by the server-sent start and end (§3.8.5) | BIN (e), (h); BC | BIN + BC |
| I12 | Induction bar: start and end times, title fallback (`Induction_TitleString`, `Channeling_TitleString`, `Skill_Name`) and interruptible flag (§3.8.4) | BIN (c) | BIN only |
| I13 | Channelling-state records: layout, census, bundle mirror, referrers (§3.9) | OBS | Confirmed 2026-10-07 |
| I13a | Channelling setback event and time census, ignore property, duration-multiplier properties (4) with their defaults and operators, barred-action census (§3.9.1, §3.9.3); which channelling PIDs occur as literals in the class code (§3.9.3) | OBS; BC (literals) | Confirmed 2026-10-07; BC |
| I14 | Hotspot templates: layout, property census, available-property list, compiled effect list (classes 701, 700, 699, 1626) and its correspondence to `EffectGenerator_HotspotEffectList` (§4.1–§4.5) | OBS | Confirmed 2026-10-07 |
| I14a | All 76 hotspot `PhysObj` descriptors resolve to setups; every setup has 1 or 2 mesh parts (46 distinct meshes); collision kinds and bounds as §4.6 | OBS (all 2,329 templates and their descriptor chains, 2026-10-08) | Confirmed 2026-10-08 |
| I15 | Hotspot placement routes: generator profiles, genesis effects, landblock instance sets, map-note and quest-marker rows (§4.6) | OBS; `map-data.md` §4.5, §5.3 | Confirmed 2026-10-07 |
| I16 | Controller entities (relays, tripwires, property boxes, patrol boxes, managers): census and instance sets (§5) | OBS | Confirmed 2026-10-07 |
| I17 | Cosmetic loadouts: layout, census, referrers (§6); where the loadout PIDs occur in the class code (§6) | OBS; BC; BIN (n) | Confirmed 2026-10-07; BC |
| I18 | Cosmetic pets and the pet directory: layout, census, bundle mirror (§7.1–§7.3) | OBS | Confirmed 2026-10-07 |
| I19 | Cosmetic-pet chain skill → toggle effect → `CosmeticEntity_CosmeticPet_ForwardSource` → pet record → creature template (§7.3) | LC (skill-to-effect step); OBS (effect-to-pet and pet-to-template steps, all records) | Effect and template steps Confirmed; skill step Reported |
| I20 | Client-side following: speed default 5.0 m/s, a trail of the owner's last 40 positions, movement state from the threshold table (§7.4) | BIN (d) | BIN only |
| I20b | Trail recording: first point at registration, a point per owner position change more than 0.01 m from the newest point, built from the owner's position and `Entity_Element` with a ground probe, kept only for elements 1, 2 and 5; newest point first, oldest dropped at 40; reset on a second owner notification (§7.4) | BIN (r) | BIN only |
| I20a | Follow step: hold rule, trail-point test (two clear segments at half the pet's height, distance closest to `CosmeticEntity_Distance`), placement on the owner when no point passes, move limited to pace × step time, heading, liquid and ground placement, `CosmeticEntity_Distance` default 0, thresholds equal to −1 skipped, state 0 = `Slow` (§7.4) | BIN (l); OBS (enum labels) | BIN only |
| I21 | Defeat records, defeat-effect progressions, landblock and override references (§8); `Death_DefeatEffectProgression` is no literal of the class code (§8.3) | OBS; BC | Confirmed 2026-10-07; BC |
| I22 | Activity collections and activities: layouts, census, bundle mirror (§9); `DistributedCharging_RequiredWorkUnits` is no literal of the class code (§9.3) | OBS; BC | Confirmed 2026-10-07; BC |
| I23 | Plaques, level-up actions, target markings, trigger responses (§10–§12) | OBS | Confirmed 2026-10-07 |
| I24 | WEENIECONTENT labels of §13 and their targets | OBS | Confirmed 2026-10-07 |
| I25 | Dictionary families: defined and stored names (§14); PID literals in the native code and in the bytecode (counts only); the (e) hits read in revision 3 that are user-interface element IDs (§14) | OBS; BIN (e), (n); BC | Confirmed (counts) |
| I26 | Tessera rules D1–D17 (§2.2, §3.4.1, §3.4.2, §3.4.5, §16, §17) | Tessera | Decision |
| I27 | Instance-set census of §3.1: 13,527 landblock instance sets with `Usage_*` properties, their static-entity types and the counts of the most common properties | OBS (all instance property sets of all 49,636 landblock-info records) | Confirmed 2026-10-07 |

---

<a id="s1a"></a>
## 1a. Terms

| Term | Meaning |
|---|---|
| **Template** | An object record (prefix 0x70, `wstate.md`) with its property set at DID + 0x09000000 (`property-system.md` §6.2). Placed objects are created from templates at run time. |
| **Instance set** | A property set in a landblock-info weenie list (`landblock.md` §4.8); it overrides or adds properties for one placed instance. |
| **Bundle mirror** | The part of an object record's bundle (`wstate.md` §4) that repeats a property of the same record's property set in a compiled form (§2.4). |
| **Usage action** | The record named by an object's `Usage_Action`; its class carries the use behaviour (§3.2). |
| **Requirement property** | A `Usage_*` property that restricts who may use an object (level, class, quest state, …). |
| **Usage plugin** | A record that names one requirement property; its class code evaluates it (§3.7). |
| **Induction** | A timed action before a use or skill completes (a "cast"), described by an induction record (§3.8). |
| **Channelling state** | A lasting state the user is put into (carrying, playing, gathering), described by a channelling record (§3.9). |
| **Gap distance** | The distance between two entities' cylinders as the client measures it (§3.4.1). |

Property, enum and label names are the install's own dictionary and mapper labels, quoted as
interoperability identifiers.

---

<a id="s2"></a>
## 2. Scope and census

<a id="s2-1"></a>
### 2.1 Classes in scope (Confirmed, I1)

All 383,343 object records were classified by root class and ancestry (`wlib.md` §9). The
groups of this spec:

| § | Group | Root classes (this install) | 0x70 records | 0x70 bytes | 0x79 bytes |
|---|---|---|---|---|---|
| §4 | Hotspot templates | 1170 | 2,329 | 1,907,500 | 1,692,999 |
| §5 | Controller entities | 1445, 1510, 1892, 1995, 2328, 2516, 2592, 3168, 4192, 5812 | 26 | 4,596 | 14,405 |
| §6 | Cosmetic loadouts | 1198 | 969 | 34,884 | 78,838 |
| §7 | Cosmetic pets, pet directory | 3698, 4091 | 564 | 35,536 | 110,280 |
| §3.9 | Channelling states | 961 | 401 | 29,074 | 216,682 |
| §8 | Defeat records | 1035 | 388 | 13,968 | 21,394 |
| §9 | Activity collections and activities | 3789; 3793, 3811, 3838 (below 3790) | 120 | 67,335 | 82,123 |
| §3.7 | Usage plugins and their directory | 3227 and 30 classes below it; 3228 | 78 | 5,517 | 2,533 |
| §3.2, §10 | Usage actions (mouse-over types, plaques, level-up actions) | 75 classes below 1597 (1597 itself has no record) | 112 (+1 singleton 0x76) | 31,016 | 35,880 |
| §11 | Target markings | 1844, 1845 | 11 | 459 | 598 |
| | **Total** | **129 classes** | **4,998** | **2,129,885** | **2,255,732** |

Pipeline records outside those groups, also specified here (§3):

| § | Group | Root classes | 0x70 records |
|---|---|---|---|
| §3.5 | Context-menu handlers | 14 classes below 506 (1529–1542) | 14 |
| §3.6 | Usage permissions, take permissions | 17 classes below 1615, 3 below 1550 | 20 (+2 singletons 0x76) |
| §3.8 | Induction actions | 1195 | 1,779 (255,348 + 423,602 bytes) |
| §3.3, §3.5, §12 | Mouse-over directory, context-menu globals, player trigger responses | 1750, 1727, 3035 | 3 |

The three 0x76 records (0x76000614, 0x7600060B, 0x76000643) are the singletons of classes
1556, 1547 and 1603 (`wlib.md` §7); they have no property set. Excluded: classes 794, 803
and 1113 (`creatures.md`, `economy.md`, `items.md`).

Ancestry (nearest first): 1170 → 1733 → 787 → 1726 → 488 → 223 → 89 → 485 → 484; the
controller classes descend from 1733 too (1892 through 3668, 1995 through 1210, 2328, 4192
and 5812 through 788, 2516 through 1113); every other class here descends from 486
(→ 89 → 485 → 484), the plugins through 3227, the activities through 3790, the usage actions
through 1597, the handlers through 506, the permissions through 1615 and 1550.

<a id="s2-2"></a>
### 2.2 Finding the records without class indices (rule, I2; Decision D1)

Class indices are build-specific. A reader finds each group by these signatures. Each finds
every record of its group; where it also finds records outside the group, the table says so:

| Group | Signature |
|---|---|
| Hotspot templates | `WeenieType` labelled `Hotspot` (2,333 records: the 2,329 of §4 and 4 decoration items of the general item class, `items.md` §2). A reader that wants §4's layout takes the root class that holds most of them. |
| Controller entities | `WeenieType` labelled `Relay`, `Tripwire`, `WorldPropertyBox`, `Landmark` or `Generator`, or `Entity` with `NeverHousekeep`, together with `SeeMeVisibilityMask` and `Entity_CreationType` 8 (§5). Also finds the 50 generator templates of `creatures.md` §6 and about 20 single management records of other classes; §5 covers the 26 records of the ten classes listed there. |
| Usage actions | Every record named by any `Usage_Action` (§3.2), plus the WEENIECONTENT `UsageAction_*` targets; every record whose root class descends from their nearest common ancestor (the usage-action root class) |
| Mouse-over directory, context-menu globals, pet directory, target-mark control, plugin directory, trigger-response set | The WEENIECONTENT labels `MouseoverDirectory`, `ContextMenuSystemGlobals`, `CosmeticPetDirectory`, `TargetMarkControl`, `UsagePluginDirectory`, `PlayerTriggerResponseSet` (§13) |
| Usage plugins | The entries of the plugin directory's `UsagePluginDirectory_UsagePluginArray` |
| Context-menu handlers | Targets of `CSM_ActionHandler` and the WEENIECONTENT labels ending in `ActionHandler` |
| Permissions | Targets of `Usage_Permission`, `Usage_TakePermission` and the labels `UsagePermission_*`, `TakePermission_*` |
| Induction actions | Records holding `Induction_Duration` (1,779) |
| Channelling states | Records holding `Channeling_Duration` (401) |
| Defeat records | Records holding `Death_RallyPoint` (388) |
| Loadouts | Targets of `Inventory_Cosmetic_LoadoutSet` and `Inventory_Cosmetic_AppliedLoadout` (all 969) |
| Cosmetic pets | Records holding `CosmeticEntity_CosmeticEntity` (563) |
| Activity collections, activities | Targets of `MultiUsage_ActivityCollection` (39); targets of `MultiUsage_Activity` (81) |
| Plaques | Records holding `PlaqueDisplay_ServerInfo_Array` (30) |

> **Decision D1:** Tessera indexes these groups by the signatures above when a store is
> built, and warns when a signature finds records in a class it did not find before.

<a id="s2-3"></a>
### 2.3 Common shapes of the object records (Confirmed)

Every record in scope follows `wstate.md` §3. Shapes:

| Group | Imports | Bundle items | Link section | Available-property list |
|---|---|---|---|---|
| Hotspots | none | 1 (83) or the compiled effect list of §4.3 (2,246; 6 to 32 items) | §4.2 | all 2,329 (9–34 names) |
| Controller entities | none | 1 (25); 2 in class 1995 (root + an empty `array_u32`) | §5.1 | §5.1 |
| Loadouts, defeat records, target marks, usage actions without data, handlers, permissions | none | 1 (root with no attributes, or attributes all null) | none | none |
| Channelling states | none | 2 (§3.9.2) | none | none |
| Induction actions | 118 import their setback progressions (§3.8.2) | 3–11 | none | none |
| Cosmetic pets | none | 2 (§7.1) | none | none |
| Activity collections | none | 6–46 (§9.2) | none | none |
| Activities | one: the usage-permission record (§9.3) | 9–25 | none | none |
| Plugin directory | 77 (one per plugin, flag 16) | 80 (§3.7.2) | none | none |
| Plugins | none | 1 | none | none |

No record of these groups decodes partly; their reserved header fields have the values of every object record (`wstate.md` §3.1).

<a id="s2-4"></a>
### 2.4 The bundle-mirror rule (Confirmed, I3)

Where a record's bundle holds more than its root object, the bundle repeats properties of
the record's own property set in a compiled form that the class code uses at run time. The
correspondences below were checked value by value in every record of each class (the
activity bundles of §9.3 match by structure; their values were not compared one by one):

| Group | Bundle part | Equals | Records |
|---|---|---|---|
| Hotspots | class-701 object and its class-700 elements (§4.3) | `EffectGenerator_HotspotEffectList`, in stored order (effect DID and spellcraft per element) | 2,246 of 2,246 with a list; the 83 without a list have a null root reference |
| Induction | §3.8.2 table | modifier lists, user-effect list, setback times, setback progressions, setback ignore-property | 1,779 of 1,779 |
| Channelling | §3.9.2 table | setback times, setback ignore-property | 401 of 401 |
| Cosmetic pets | `set_u32` | `CosmeticEntity_TrickArray` (as a set) | 563 of 563 (empty in 562) |
| Pet directory | `array_u32` | `CosmeticEntity_SkillList`, in order | 1 of 1 (563 entries) |
| Plugin directory | §3.7.2 | `UsagePluginDirectory_UsagePluginArray` and each plugin's `UsagePlugin_UsagePropertyName` | 1 of 1 (77 entries) |
| Mouse-over directory | `refmap_u32` | `Mouseover_MouseoverList` keyed by `Mouseover_Type` | 1 of 1 (39 entries) |
| Activity collections | §9.2 | `MultiUsage_ActivityPatchBay` | 39 of 39 |
| Target-mark control | `array_u32` | `TargetMarking_ControlData_Array`, in order | 1 of 1 (10) |
| Plaques | `refmap_u32` | `PlaqueDisplay_ServerInfo_Array` keyed by `PlaqueDisplay_Server` | 30 of 30 (29 entries each) |
| Level-up actions | `array_u32` | `Quest_QuestsToComplete` | 7 of 7 holding it (the eighth has a null reference) |
| Player action handler | `refmap_u32` keyed by `CSM_Player_UIToggleOption_GOP` | `CSM_Player_UIToggleOptions` | 1 of 1 |

**Rule for readers.** Read the property set; the bundle adds nothing a reader needs, except
in one place: in hotspots, a class-699 object can carry the data list of the *referenced
effect* (§4.3), which the hotspot's own set does not repeat. A byte-exact writer must still
reproduce the bundle. This spec gives that only in part:

| Coverage | Bundle parts |
|---|---|
| Correspondence, attribute hashes and constant fields given | induction root (§3.8.2) with its containers and DID items; channelling root (§3.9.2); plugin directory (§3.7.2); mouse-over directory map (§3.3); activity-collection root, group and socket objects (§9.2); hotspot root (§4.2), the class-701 object (all 13 attributes, §4.3) and the class-1626 objects (§4.3); class-700 elements (§4.3, `quests.md` §6.3); class-699 element data (§4.3); cosmetic-pet `set_u32` (§7.1); pet directory (§7.2); plaques (§10.1); level-up actions (§10.2); target-mark control (§11); the player action handler's root and its class-3427 objects (§3.5) |
| Named by class only, values not compared | the activity bundles (classes 1602, 3841, 3825, 1339, 3812/3813, §9.3); the trigger-response bundle (classes 3033, 3034, 3039, 3040, 3042, §12) |

> **OPEN:** A byte-exact bundle writer for the groups of the last row: their attribute
> hashes, constant fields and value-by-value correspondences with the property set were not
> listed (the activity bundles match the set by structure only, §9.3; the trigger-response
> bundle was not compared, §12). Readers do not need them.

---

<a id="s3"></a>
## 3. The use pipeline

<a id="s3-1"></a>
### 3.1 Overview and the usage properties of world objects

What an object needs for a player to act on it, and where each part comes from:

| Step | Property on the object (template set, instance set) | Points at | § |
|---|---|---|---|
| What happens on use; mouse-over type | `Usage_Action` (and its copy `Usage_Action_Shadow`, `items.md` §4.3) | usage-action record | §3.2 |
| Cursor | the action's `Mouseover_Type` → mouse-over directory | 0x41 cursor images | §3.3, §3.4.2 |
| Range | `Usage_MinUsageDist`, `Usage_UsageExpireDist` | — | §3.4.1 |
| Context menu | `CSM_ActionHandler` | handler record | §3.5 |
| Who may use / take | `Usage_Permission`, `Usage_TakePermission` | permission records | §3.6 |
| Requirements | `Usage_*` requirement properties, `DefaultPermissionBlobStruct` | evaluated by usage plugins | §3.7 |
| Induction (cast bar) | `Induction_ActionDID` | induction record | §3.8 |
| Channelling | `Usage_AppliedChannelingState`, `Usage_RequiredChannelingState`, `Usage_BarringChannelingState` | channelling record | §3.9 |
| Effects | `EffectGenerator_UsageEffectList`, `EffectGenerator_HotspotEffectList` | effect records (`combat-data.md`) | `items.md` §5.9, §4 |
| Multi-user stations | `MultiUsage_ActivityCollection` | activity collection | §9 |
| Selectable | `IsSelectable`, `Selection_Script` | — | §5.3 |

**Instance sets (Confirmed, I27).** 13,527 landblock instance sets carry `Usage_*` properties: 7,745
with no static entity, 5,599 on static entities of type 7, 135 of type 9, 46 of type 6 and 2
of type 31. The most common are travel and door properties: `PortalDestination` (9,067),
`Usage_LayerArray` (8,284), `Inventory_IsOpen` (4,350), `Usage_PlayersCanClose` (1,954),
`Usage_PlayersCanOpen` (1,863), `Usage_Lockable` (1,714), `Usage_AllowedInCombat` (1,633),
`Usage_Locked` (1,382), `Usage_QuestRequirements` (1,340), `Usage_AICanUseDoors` (1,046),
`Usage_UseWhenCollided` (828), `Usage_OpenOnUnlock` (765), `Usage_LockOnClose` (544),
`Usage_AICanIgnoreLock` (515), `Usage_HousingPermission_Requirement` (488),
`Usage_SessionPlayerCanUse` (379), `Usage_AppliedChannelingState` (354: 342 channelling
records, 12 induction records), `Usage_MonsterPlayerCanUse` (289), `Induction_ActionDID` (279),
`Usage_RestrictedEffect` (272). An implementer merges the instance set over the template's
set as `entity-descriptor.md` §4.9 describes for lookups.

Of the 181 `Usage_*` names the dictionary defines, 142 are stored somewhere (§14).

<a id="s3-2"></a>
### 3.2 Usage-action records (Confirmed, I4)

<a id="s3-2-1"></a>
#### 3.2.1 Census

Every `Usage_Action` value in the install (195,930 values, all in 0x79 sets; none in
instance sets) names a record whose root class descends from the usage-action root (1597).
112 records in 75 classes; class 1556 has two records plus a singleton, 3904 has 30
(plaques, §10.1) and 3859 has 8 (level-up actions, §10.2); every other class has exactly one
record. 110 of the 112 are named by some `Usage_Action`. Property sets hold at most four
properties:

| Property | Type | Records | Values |
|---|---|---|---|
| `Mouseover_Type` | enum (mapper 0x23000056) | 89 | 24 `Use` in 74; others: 1, 6, 8, 9, 16, 18, 19, 26, 29, and 0 (`Undef`) in one |
| `Mouseover_TargetedActionType` | enum (same mapper) | 3 | 20, 42, 44 |
| `Mouseover_DramaActionType` | enum (same mapper) | 4 | 25 |
| `Usage_TargetActionWithDialog` | bool | 12 | true in 11 |
| `Usage_TargetActionDialogDescription`, `…_Multi` | StringInfo | 8, 1 | dialog text |
| `Usage_LevelupUsageAction_LevelToAdvance`, `Quest_QuestsToComplete` | int, array of `Quest_DataID` | 8, 7 | §10.2 |
| `PlaqueDisplay_ServerInfo_Array` | array | 30 | §10.1 |

21 usage-action records have an empty property set.

<a id="s3-2-2"></a>
#### 3.2.2 Who names which action

The ten most-named actions (records naming them, by naming class):

| Action class | Records naming it | Named by | WEENIECONTENT label |
|---|---|---|---|
| 1567 | 91,154 | equippable items (classes 795, 799, 802, 796) | — |
| 1556 | 35,314 | general items 23,893, recipe scrolls 6,055, class 1210 2,733, hotspots 2,329, controller entities and many single records | `UsageAction_DoNothing` (one of its two records) |
| 1578 | 19,804 | mob templates (1723) | — |
| 3957 | 15,411 | general items (796) | — |
| 1580 | 15,330 | NPC templates 15,295, items 26, neighbourhood NPCs 6, mobs 3 | — |
| 2110 | 4,805 | general items | — |
| 2815 | 2,988 | class 2814 | — |
| 1553 | 1,720 | container templates (794) | `UsageAction_Container` |
| 1589 | 1,277 | NPC templates 1,272, mobs 5 | `UsageAction_QuestNPC` |
| 2210 | 918 | items | — |

Doors (1722) name class 1559 (253) or 1551 (1); travel points (815) class 1599 (226) or
1987 (2); housing hooks (1989) class 1999 (744). `UsageAction_Corpse` (class 3687) and
`UsageAction_DecorationItem` (class 2026) are named by no stored property; they are used at
run time (the data do not show by whom).

<a id="s3-2-3"></a>
#### 3.2.3 Boundary with the bytecode

Usage-action records carry no behaviour data beyond the mouse-over type. Their classes have
no attributes, except the plaque and level-up classes (one each, §10); the root usage-action class has a virtual-function table of 103 slots
(`wlib.md` §6.4) and declares 38 functions, and each action class adds or overrides a few
(class 1556: 9 own functions, 2 slots that differ from its parent; class 1567: 6 and 3). What
"use" does for a given action is the code in those slots.

> **OPEN (bytecode):** The behaviour of each usage-action class (what the server does on use)
> lives in the class library's bytecode. [`wlib-bytecode.md`](wlib-bytecode.md) specifies the
> instructions but not the native functions the code calls (its §7.1 OPEN), and no spec reads
> the use functions of these classes. (Their mouse-over functions are read in §3.4.4.) A
> Tessera server dispatches by action instead (Decision D2, §16.1).

<a id="s3-3"></a>
### 3.3 The mouse-over directory (Confirmed, I5)

WEENIECONTENT `MouseoverDirectory` names one record (class 1750 here). Its property set has
one property:

| Property | Type | Content |
|---|---|---|
| `Mouseover_MouseoverList` | array of `Mouseover_MouseoverStruct` | 39 structs, one per mouse-over type |

Each struct:

| Field | Type | Meaning |
|---|---|---|
| `Mouseover_Type` | enum (mapper 0x23000056) | The key; 39 distinct keys (1–14, 16–29, 31, 34–42, 44) |
| `Mouseover_Cursor` | DID → 0x41 (render surface) | Cursor shown while the hovered object is in range |
| `Mouseover_CursorOutOfRange` | DID → 0x41 | Cursor shown while it is out of range (§3.4.2); equal to the in-range cursor for 8 types |
| `Mouseover_Script` | enum (mapper 0x23000097) | One value for all 39 (a mouse-over effect script key) |

The dictionary also defines `Mouseover_CursorHotspotX` and `…Y` (int, default 0) as members
of the struct; no struct stores them.

Bundle mirror: the root's reference attribute 0x013ADFB3 names a `refmap_u32` whose keys are
the 39 `Mouseover_Type` values and whose values are `property` objects (native class 39)
holding the matching struct.

<a id="s3-4"></a>
### 3.4 What the client does (BIN, BC)

> **Note (P3, 2026-10-10, [`npc-windows.md`](npc-windows.md) §3.1; no fact of this spec
> changed):** this section gives range, cursor and mouse-over, not which input uses an
> entity. That is in `npc-windows.md` §3.1 (BIN, BC): the input action `USE` uses the entity
> of the local player's `Entity_Selection` (class-library function 0x01D60005 here); a click on
> the world selects the clicked entity after the selectable test of §3.4.4; a left
> double-click then uses it, and a right click or `AutoLootAll` on a non-player entity uses
> it (`AutoLootAll` first sets `Usage_DoMetaUsage`).

<a id="s3-4-1"></a>
#### 3.4.1 Use range (I6)

The client's test whether the local player is close enough to use a target:

1. The range *R* is the caller's explicit range when that is above 0.
2. Otherwise, if the target has `Usage_UsageExpireDist` as a float above 0, *R* is that value.
3. Otherwise *R* = *m* + 3.0, where *m* is the target's `Usage_MinUsageDist` when the lookup
   finds a float value, and 3.0 when it finds none.
4. The player is in range when the gap distance *d* (below) is at most *R*. When *d* cannot
   be computed, the test treats it as 0 (in range).

Units are metres. `Usage_UsageExpireDist` is not stored anywhere in the data; in practice
*R* = `Usage_MinUsageDist` + 3.0. Observed `Usage_MinUsageDist` values: 2 on most templates
(dictionary default 2), 4 on NPCs (16,549 of 16,605), 3 or 3.5 on containers, 8 on doors and
travel points, 25 or 12 on housing hooks.

**Cylinders (I6a).** For the test the client builds one cylinder per entity (the local
player first, the target second). A cylinder has a position (the entity's current position:
cell, landblock and local coordinates, `coordinates.md`), an offset vector *o*, a radius *r*
and a height *h*, all in metres:

1. **Base radius and height:** *r* = *r*₀ × *s* + 0.05 and *h* = *h*₀ × *s* + 0.1 (I6c):
   - *r*₀ is the horizontal radius and *h*₀ the height of `physics_bounds` (`setup.md` §6.5:
     the eighth and the seventh of its eight floats) of the setup that the entity's model is
     built from (the setup the entity's 0x47 chain names, `setup.md` §3.1);
   - *s* is the entity's physics scale: the z component of its `Physics_AdjustableScale`
     (PID 0x118, vector) times the z component of its `Model_Scale` (PID 0x3C3, vector), each
     taken as (1, 1, 1) when the entity has no such vector. When the product is not finite, or
     is below 0.0002, *s* = 1. (The x and y components are not used; the client warns when x
     differs from y, or x from z, by more than 0.0002.)
2. **Offset:** *o* is the centre of the axis-aligned bounding box of the entity's collision
   shape (the shape built from that setup's collision volumes), computed in the entity's own
   frame with an identity transform (no rotation, no translation, no scale): per axis, half
   the sum of the box's minimum and maximum. The offset is neither rotated by the entity's
   heading nor multiplied by *s*.
3. **Radius override:** when the entity's `Physics_GameRadiusOverride` (PID 0x692, float,
   dictionary default −1) is found as a float above 0, *r* is replaced by that value times
   *k*, where *k* is the first (x) component of the entity's `Render_AdjustableScale` (PID
   0x33B, vector, default (1, 1, 1)) when the client can read that property as a vector, and
   1 otherwise. The override replaces the whole base radius (no 0.05 m is added). Height and
   offset are not changed. When the override is absent, not a float, or not above 0, the base
   radius stays.

`Physics_GameRadiusOverride` is stored on 54 mob templates in this install (values 0.5–13,
all above 0; 52 of them also store `Render_AdjustableScale`, always with three equal
components; I6b). No other 0x79 set stores it; three landblock instance sets carry it
(values 2, 3 and 10); 0x78 sets were not searched. Read it like any other property (instance
set over template).

**Gap distance.** The client measures between the two cylinders:

1. Both positions must be valid (`coordinates.md`) and lie in the same region and instance;
   otherwise no distance is produced.
2. Add each cylinder's offset *o* to its position (all three axes). Every following step uses
   these moved positions.
3. Horizontal: bring both to one frame by adding 160.0 m per landblock of difference in x
   and in y, take the horizontal distance between the two moved positions, subtract
   *r*₁ + *r*₂, and clamp at 0.
4. Vertical, with *z* the moved heights: if *z*₁ ≤ *z*₂, the gap is *z*₂ − (*z*₁ + *h*₁),
   otherwise *z*₁ − (*z*₂ + *h*₂); clamp at 0.
5. *d* = √(horizontal² + vertical²).

The same rule applies to the local player: its *r*₀ and *h*₀ come from the setup of its own
body model, not from Tessera's movement capsule (`collision.md`).

> **Decision D13 (no setup):** an entity whose model names no setup, or whose setup has no
> collision (all eight `physics_bounds` floats 0), gets *r*₀ = *h*₀ = 0 and *o* = (0, 0, 0),
> so its cylinder is radius 0.05 m and height 0.1 m before the override of step 3.

**Property lookup (BIN o).** The client's entity property lookup routes each PID by the
`field_b` byte of its dictionary definition (`property-system.md` §2.2, §2.3): values 3, 4, 7,
8 and 9 select different property providers of the entity, and any other value finds nothing.
`Usage_MinUsageDist`, `Usage_UsageExpireDist`, `Physics_GameRadiusOverride`,
`Render_AdjustableScale` and `Mouseover_Range` all have `field_b` = 4 (the provider that
23,331 of the 27,546 definitions use). The lookup reports "not found" when the provider
reports no value.

> **OPEN:** Whether the `field_b` = 4 provider substitutes the dictionary default (2.0 for
> `Usage_MinUsageDist`) for a property the entity does not store, which would make step 3 give
> 5.0 instead of 6.0. The provider's own lookup routine was not identified. A hint, not a
> proof: the class library reads the same property with an explicit fallback of 2.0 (§3.4.2),
> which it would not need if the lookup supplied the default. Every usable template in this
> install stores the property, so the case does not arise for data-defined objects.

> **Decision D14 (missing range property):** Tessera's lookup returns only stored values
> (instance set over template), never the dictionary default, so an entity without
> `Usage_MinUsageDist` has *R* = 3.0 + 3.0 = 6.0 m.

<a id="s3-4-2"></a>
#### 3.4.2 Cursor (I7)

The client chooses between a mouse-over type's two cursors (§3.3) as follows: it reads
`Mouseover_Range` from the **local player** (not from the hovered object). When that value
is absent or not above 0, the in-range cursor is used. Otherwise it computes the cursor
distance from the player to the hovered entity; when that distance could be computed and is
at least `Mouseover_Range`, the out-of-range cursor is used (the flag is set).

**Where `Mouseover_Range` comes from (I7d, BC).** It is never stored in the data. The class
code writes it onto the local player each time an entity is hovered with a mouse-over type
other than 0 (§3.4.6 step 3): the value is the hovered entity's
`Usage_MinUsageDist_ClientOverride` (float, no dictionary default) when the entity has it,
else its `Usage_MinUsageDist`, else 2.0. All 100 world-entity classes of this install (the
entity root, class 787 here, and its descendants) use this rule; the root itself skips the
override. `Usage_MinUsageDist_ClientOverride` is stored nowhere in this install (OBS), so in
practice the cursor range is the entity's `Usage_MinUsageDist`, without the 3.0 m that the
use range of §3.4.1 adds.

**Hover updates (I7c, BIN p).** The client recomputes the hover whenever it updates the list
of entities under the cursor (the "periodic re-check" of §3.4.4):

1. The hovered entity is the first entity of that list. When it is the local player itself,
   or the entity named by the local player's `Shapeshift_MeshIID`, the second entity of the
   list is taken instead (none when the list has only one).
2. The flag is computed as above, with the `Mouseover_Range` the local player holds at that
   moment.
3. When the hovered entity differs from the previous one, the client hands off "leave" for
   the previous one (when it still exists) and "enter" for the new one (when there is one) with
   the flag (§3.4.4). When the entity is the same and only the flag changed, it hands off the
   flag change.

Because step 2 uses the range written for the previously hovered entity, the flag handed off
on "enter" can be stale; the next update recomputes it with the new entity's range and hands
off the change.

**Cursor distance (I7a).** It is the gap distance of §3.4.1 with one difference: both
cylinders are built under the **outermost-parent rule**. Each side (the local player and the
hovered entity) is replaced by the last entity of its parent chain in the client's entity
tree (the entity itself when it has no parent), and that ancestor supplies everything: the
position, *r*₀, *h*₀, *s*, the offset, and the `Physics_GameRadiusOverride` and
`Render_AdjustableScale` lookups. Two special cases: when the hovered entity **is** the local
player, the distance is 0 at once; when the hovered entity cannot be found, no distance is
produced (the in-range cursor stays). The use-range test of §3.4.1 does **not** apply this
rule.

> **OPEN:** Which relation the client's entity tree records (the parent links that the
> outermost-parent rule follows and whose descendants §3.4.3 searches): item containment,
> equipment, a held or attached object, a rider on a mount, or several of these. The tree's
> structure was read (each entity holds a link to its parent and a map of its children, and
> the walk to the outermost ancestor follows the parent links until one is null); the code
> that creates the links was not found in revision 4 either (BIN s).

> **Decision D15 (entity tree):** Until the relation is known, a Tessera entity's parent is
> the entity that holds it in its inventory, equips it or carries it as a held prop, when
> Tessera's entity store records that; every other entity has no parent and is its own
> outermost ancestor. With no such links, §3.4.3's "the local player or one of its
> descendants" reduces to the local player itself.

<a id="s3-4-3"></a>
#### 3.4.3 Usage-action identity (I8)

The client resolves WEENIECONTENT entries by their **key** within the map that the root DID
map files under key 0xA (`property-system.md` §8.1). A routine that acts on a selected entity
resolves key 0x10000042 (`UsageAction_QuestNPC`) and, for an entity that is not a creature,
goes on only when the entity's `Usage_Action` equals that record. A routine that checks
whether a shortcut's target is usable (for shortcuts that name an entity) resolves key
0x10000033 (`UsageAction_DoNothing`) but discards the result in this build. The resolution
only looks the key up in the DID map and yields a DID; it opens no record, so it has no
other effect. The routine's answer is "usable" exactly when the target exists, has a
non-null `Usage_Action`, and is the local player itself or one of its descendants in the
client's entity tree (§3.4.2), which the routine searches depth first from the player.
Readers that need these actions resolve them the same way (by key or by label), never by
DID.

The relation that the entity tree records is the OPEN of §3.4.2.

<a id="s3-4-4"></a>
#### 3.4.4 Mouse-over type and eligibility (I7b)

**Hand-off (BIN).** The client's native code does not choose the mouse-over type. It passes
the hover to the class library's code through three function IDs (`wlib-bytecode.md` §6.4):
when the cursor enters an entity, 0x01150001 with the out-of-range flag of §3.4.2 and the
entity's instance ID; when the periodic re-check finds that the flag changed, 0x01150002
with the new flag and the value 1; when the cursor leaves, 0x01150003 with the instance ID.
In this install the three IDs resolve to functions 1, 2 and 3 of class 277 (the mouse-over
controller). The rules below are that code (BC).

**Rules for a hovered entity *E* (BC).** The controller looks *E* up by its instance ID and
decides, in this order:

1. **Eligible?** *E* gets a mouse-over when *E*'s `Mouseover_AllowMouseoverOverride` is true
   (bool, default false). Otherwise:
   1. when the bit of value 2 is set in the `CanSeeVisibilityMask` of the object that native
      function 0x00E70001 returns (by inference the local player; the value 2 is the
      `Hidden` value of `SeeMeVisibilityMask`, §5.3), *E* is eligible;
   2. otherwise, when *E*'s `QuestState_PhasedOut` is true, it is not;
   3. otherwise, when *E*'s `Usage_IsQuestItem` is true, *E* is eligible exactly when its
      `Quest_IsItemQuestSelectable` is true (false when absent);
   4. otherwise *E* is eligible when it is **selectable**: `IsSelectable` is true and, when
      *E* holds `ClientIsSelectableOverride`, that value is true as well. (All 100
      world-entity classes of this install, the entity root and its descendants, share this
      function. The other classes that define their own version are effect, skill and similar
      classes outside the entity family, which are not world entities.)

   An ineligible *E* gets no mouse-over (the controller is called with type 0).
2. **Activity station.** When *E* has a non-null `MultiUsage_ActivityCollection` (§9.1), the
   type is that collection's `MultiUsage_Mouseover` (0 when absent), and step 3 is skipped.
3. **Usage action.** Otherwise *E*'s effective `Usage_Action` names the usage-action record
   (§3.2); with none (absent, 0, or a DID that does not load as a usage-action record), there
   is no mouse-over. All world-entity classes share this lookup. The action computes the type,
   given *E*:
   1. When the local player's `Usage_PendingTargetedUsageItem` (instance ID, no dictionary
      default, 0 when absent; read from the same object as in step 1.1) is non-zero, an item is waiting for a
      target. The type is then the `Mouseover_TargetedActionType` of the **pending item's**
      usage action, provided that *E*'s action has `Usage_TargetActionWithDialog` = true;
      otherwise (no such item, no action, flag false or property absent) it is 0.
   2. Otherwise the type is the action's `Mouseover_DramaActionType` when *E*'s
      `Drama_Performing` is true, and the action's `Mouseover_Type` when it is not; 0 when the
      action does not hold that property.
4. The controller then shows the cursor for that type and flag from the mouse-over
   directory (§3.3) and starts the mouse-over effect, as §3.4.6 describes. A type of 0 means
   "no mouse-over" there.

Of the 92 classes of the usage-action family (the root and its descendants), 85 use step 3
as a whole unchanged and 90 use the targeted part, rule 3.1, unchanged. The exceptions (class
indices of this install): for the targeted case, classes 3058 and 3957 always give 42
(`Disenchant`); classes 1553, 1577, 1580, 1587, 1589, 2026 and 3687 replace step 3 as a whole
(rules 3.1 and 3.2 together) with a rule that runs extra checks before or after it (1553 reads
`Loot_ShouldSparkle`; 1577 and 1580 use step 3 and fall back to another function of theirs
when it gives 0; 1589 and 3687 first run a test on the hovered entity and the local player
and give 0 when it fails; 1587 runs the same test but also accepts an entity whose
`Drama_Performing` is true; 2026 has its own branch). §3.4.5 gives these rules.

<a id="s3-4-5"></a>
#### 3.4.5 Usage-action classes with their own type rule (BC, I7e)

For seven usage-action classes, step 3 of §3.4.4 (rules 3.1 and 3.2 together, called "step
3" below) is wrapped in an extra rule; the extra checks therefore apply also while an item is
waiting for a target. Class indices are those of this install (find them through the records
named, §2.2); "E" is the hovered entity, and a property read with no value counts as 0 or
false unless stated:

| Class (WEENIECONTENT label) | Named by | Rule |
|---|---|---|
| 1553 (`UsageAction_Container`) | container templates | When E's `Loot_Permissions_Rule` (enum) is not 0 and E's `Loot_ShouldSparkle` is not true, 0. Otherwise step 3. |
| 1589 (`UsageAction_QuestNPC`) | NPC and mob templates | When quest test A (below) fails, 0. Otherwise step 3. |
| 3687 (`UsageAction_Corpse`) | no stored property (run time) | As 1589. |
| 1587 | items | When quest test A fails and E's `Drama_Performing` is not true, 0. Otherwise step 3. |
| 2026 (`UsageAction_DecorationItem`) | no stored property (run time) | When E's `HousingSystem_HouseKeeperIID` names an entity the client knows and that entity's house-keeper test for the local player passes, step 3. Otherwise, when E's `HousingSystem_DecorationItem_OldUsageActionDID` names a record that loads as a usage action, that record's step 3 (as overridden in this table for its class) applied to E. Otherwise step 3. |
| 1580 | NPC templates (15,295) and a few items, neighbourhood NPCs and mobs | Step 3; when it gives 0, the NPC service rule (below). |
| 1577 | mob templates | Step 3; when it gives 0: when E is not an agent, 0. An agent is an entity whose `WeenieType` has all four low bits set (value & 15 = 15: `Agent` 15 and the types built on it, such as `Player` 31 and `NPC` 79). For an agent, when the harmful-interaction test between the local player and E raises no objection, 19 `Combat_AutoAttack`; otherwise the NPC service rule. |

**NPC service rule.** It gives 0 at once when E's `Combat_Violent` is true, when E's
`AI_Usage_CanNotBeUsed` is true, or when E's `Drama_Performing` is true. Otherwise it keeps
a count *n* (starting at 0) and a type *T* (starting at 0) and goes through the rows below in
order. Each row whose condition holds adds 1 to *n* and sets *T*. "E has P" means that E
stores P, whatever its value.

| # | Condition | *T* |
|---|---|---|
| 1 | quest test A, B or C passes | 6 `NPC_Quest` |
| 2 | E has `Vendor_InventoryList` or `Repair_CostModifier` | 18 `NPC_Vendor` |
| 3 | E has `AppearanceMod_NPCIsBarber` | 34 `NPC_AppearanceMod` |
| 4 | E has `Barter_ProfileArray` | 27 `NPC_Barter` |
| 5 | E has `NPCComment_DIDCommentArray` | 35 `NPC_NPCComment` |
| 6 | E's `NPC_Is_Trait_Manager` is true | 5 `NPC_TraitManager` |
| 7 | E has `TravelWebWC` | 9 `NPC_Travel` |
| 8 | E has `HousingSystem_PurchaseTemplateDID` | 28 `NPC_HousingPurchase` |
| 9 | E's `HousingSystem_IsEscrowNPC` or `HousingSystem_IsChestNPC` is true | 31 `NPC_HousingEscrow` |
| 10 | E's `Craft_OfferedVocationArray` contains the local player's `Craft_Vocation`; or E has `Hobby_OfferedHobbiesArray`; or E's `Craft_OfferedCraftGuildArray` is not empty | 12 `NPC_VocationTrainer` |
| 11 | E's `Craft_OfferedProfessionArray` holds an entry that the local player's `Craft_ProfessionArray` does not | 43 `NPC_ProfessionTrainer` |
| 12 | E has `Skirmish_SkillTrainer_Skirmish` with a non-zero value; or else, when the monster-play test of the local player passes, E's `SkillTrainer_MPTrainer` is true, and when it fails, E's `SkillTrainer_Class` equals the local player's `Agent_Class` | 3 `NPC_SkillTrainer` |
| 13 | E has `PerkVendor_InventoryDID` | 17 `NPC_PerkVendor` |
| 14 | E has `MPMissionController_Mission` | 14 `NPC_MPMissionController` |
| 15 | E has `MPMissionController_Session` | 22 `NPC_MPCritterMissionController` |
| 16 | E has `NPC_Is_Banker` | 2 `NPC_Banker` |
| 17 | E has `NPC_Is_Auctioneer` | 13 `NPC_Auctioneer` |
| 18 | E's `NPC_Is_Neighbor` is true | 13 `NPC_Auctioneer`, and *n* grows by 2 instead of 1 |
| 19 | E has `NPC_Is_Notary` | 11 `NPC_Notary` |
| 20 | E has `PrivateEncounter_TravelArray` | 21 `NPC_PrivateEncounterTravel` |
| 21 | E has `ItemAdvancement_ReforgeTable` | 37 `NPC_ItemAdvancementForge` |
| 22 | E's `ItemAdvancement_AllowRunicCombination` is true | 36 `NPC_RunicCombination` |
| 23 | E's `NPC_ActionPicker_LevelBestower_LevelArray` holds an entry above the local player's `Advancement_Level` (1 when absent) | 41 `NPC_LevelBestower` |
| 24 | E has `NPC_ColorTextFile`, or quest test D passes | when row 1 held and *n* = 1: *T* stays 6 and *n* does not change; otherwise *n* grows by 1 and *T* = 4 `NPC_ColorText` |

After the last row, when *n* > 1, *T* = 10 `NPC_Multiple` (so a neighbour, row 18, always ends
as 10). The rule's result is *T*. The type numbers are keys of the mouse-over type mapper
(0x23000056); the labels are quoted for identification.

**Tests named above.** They are class code of other systems, named here only by what they
read:

- **Quest tests A–D** (the local player's quest book, class 1753 here): A asks the book
  whether E takes part in the player's quests (several relations checked in turn); B and C walk
  E's `QuestDispenser_ActiveQuests` entries (`QuestDispenser_Quest`) against the book; D is a
  further book test on E.
- **Harmful-interaction test** (class 434 here, function 120): compares interaction-permission
  data built for the local player and for E; "no objection" is a result of 0.
- **Monster-play test** of the local player: two native functions (by inference whether the
  player is in monster play, from the property chosen next).
- **House-keeper test** (classes 2005 and 2007 here, the entity named by
  `HousingSystem_HouseKeeperIID`): a test of the local player against that house's data. On
  an entity of any other class the same call tests whether the entity's `LifeState` is 4
  (`Alive`).

> **OPEN:** The rules of quest tests A–D, the harmful-interaction test, the monster-play test
> and the house-keeper test. They belong to the quest, combat-permission and housing systems,
> whose class code no spec reads yet (`wlib-bytecode.md` §7.1 leaves the native functions they
> call unspecified).

> **Decision D16 (tests of other systems):** Until those systems are specified, Tessera
> evaluates quest tests A, B and C as passing exactly when E stores a non-empty
> `QuestDispenser_ActiveQuests`, quest test D as failing, the monster-play test as failing and
> the house-keeper test as failing.

> **Decision D17 (attack cursor):** The harmful-interaction test raises no objection exactly
> when Tessera's combat rules let the local player attack E.

<a id="s3-4-6"></a>
#### 3.4.6 Cursor and mouse-over effect (BC, BIN q; I7f)

The controller keeps its state as properties of the **local player**; no stored property
set holds any of them at the top level:

| Property | Type | Meaning |
|---|---|---|
| `Mouseover_Entity` | instance ID | The entity last handed to the controller (0: none) |
| `Mouseover_State` | bit field (32 bits) | The flag word; the bit of value 1 is the out-of-range flag of §3.4.2 |
| `Mouseover_Script` | enum (mapper 0x23000097) | The mouse-over effect script playing (0: none); on the player it is run-time state, the stored values are the directory's (§3.3) |
| `Mouseover_ActiveScriptID` | int | The running effect's instance number (−1: none) |
| `Mouseover_Range` | float | §3.4.2 |

**Hand-offs.** "Enter" with flag word *f* and entity *E* runs §3.4.4 to get the type *T* (0
when *E* is not eligible or cannot be found), then the **show** step with (*E*, *f*, *T*). A
"flag change" with the new flag and the mask 1 replaces the masked bit of `Mouseover_State`
by the new flag and runs "enter" again for the entity in `Mouseover_Entity` with that word.
"Leave" runs the **clear** step.

**Show** (*E*, *f*, *T*):

1. Remember the entity in `Mouseover_Entity` as the previous entity *P*, and the player's
   `Mouseover_Script` and `Mouseover_ActiveScriptID` (0 and −1 when absent).
2. When *T* = 0: run **clear**, then set `Mouseover_Entity` to *E* (so an ineligible entity
   is still recorded), and stop.
3. Set `Mouseover_Entity` to *E*, and set the player's `Mouseover_Range` from *E* (§3.4.2),
   when *E* can be found.
4. **Cursor lookup.** Find the mouse-over directory (the record that WEENIECONTENT key
   0x1000007E names, §3.3) and, through its compiled map, the struct for *T*. When the bit of
   value 1 is set in *f*, take its `Mouseover_CursorOutOfRange`, or `Mouseover_Cursor` when the
   struct lacks that member; otherwise take `Mouseover_Cursor`. The hotspot is
   (`Mouseover_CursorHotspotX`, `Mouseover_CursorHotspotY`), each 0 when the struct lacks it.
   The lookup fails when there is no directory, no struct for *T* or no cursor member.
5. When *T* = 19 (`Combat_AutoAttack`), run the harmful-interaction test of §3.4.5 between
   the local player and *E*; an objection counts as a failure here.
6. When the lookup succeeded without such an objection, the client sets the cursor to that
   image (a 0x41 DID) with that hotspot, in pixels from the image's top left (by inference);
   otherwise it resets the cursor to its default.
7. **Effect.** When *T*'s struct has a `Mouseover_Script` *S*: if *S* differs from the
   remembered script, or *E* differs from *P*, stop the remembered effect on *P* (when the
   remembered script is not 0 and *P* still exists), start *S* on *E* with a new instance
   number, and set `Mouseover_Script` to *S* and `Mouseover_ActiveScriptID` to that number (0
   and −1 instead when *E* cannot be found); if both are unchanged, do nothing. When the
   struct has no `Mouseover_Script`, stop the remembered effect on *P* and set the two
   properties to 0 and −1.
8. Set `Mouseover_State` to *f*.

The effect does not depend on step 5: an attack cursor that is refused still plays the
effect.

**Clear.** When `Mouseover_Entity` is 0, nothing happens. Otherwise the cursor is reset to its
default, the remembered effect is stopped on that entity (when its script is not 0 and the
entity still exists), and `Mouseover_Script` = 0, `Mouseover_ActiveScriptID` = −1,
`Mouseover_State` = 0 and `Mouseover_Entity` = 0.

Native functions behind these steps (BIN q): the cursor set and reset are notifications to the
client's cursor listeners carrying (image DID, hotspot x, hotspot y) and nothing; the
directory is found through the WEENIECONTENT map like §3.4.3; an effect start allocates a
script instance number and starts the script on the entity; an effect stop names the entity,
the script and the instance number.

Tessera's client follows **show** and **clear** as written and plays the effect script with its
script-table runtime (`script-table.md`).

<a id="s3-5"></a>
### 3.5 Context-menu handlers (Confirmed, I9)

`CSM_ActionHandler` names one of 14 handler records (one per class, 1529–1542 here). Their
property sets hold the menu's StringInfo texts (titles and option labels), one property per
option:

| Handler (WEENIECONTENT label) | Properties | Named by |
|---|---|---|
| `ActivatorActionHandler` | `CSM_Activator_Option_Trigger`, `CSM_Menu_Title`, `CSM_Option_MultiUsage` | no set |
| `AdminActionHandler` | `CSM_Admin_Option_Smite`, `CSM_Menu_Title` | no set |
| `ChestActionHandler` | `CSM_Chest_Option_Open`, `…_Close`, `CSM_Menu_Title` | containers |
| `ConsumableActionHandler` | `CSM_Item_Option_Consume`, `CSM_Menu_Title` | items |
| `CorpseActionHandler`, `CSM_CorpseActionHandler` (one record) | `CSM_Corpse_Option_Loot`, `…_Close`, `CSM_Menu_Title` | no set |
| `DefaultActionHandler` | `CSM_Default_Option_Use`, `CSM_Menu_Title` | doors, NPCs, items, keys, context-menu globals |
| `DoorActionHandler` | `CSM_Door_Option_Open`, `…_Close`, `CSM_Menu_Title` | doors, items |
| `EquippableActionHandler` | `CSM_Item_Option_Equip`, `…_Unequip`, `CSM_Menu_Title` | equippable items |
| `ExamineActionHandler` | `CSM_Examine_Option_Examine` | no set |
| `MonsterActionHandler` | `CSM_Monster_Option_Attack`, `CSM_Menu_Title`, `CSM_Option_MultiUsage` | mobs |
| `NPCActionHandler` | `CSM_NPC_Option_Interact`, `CSM_Menu_Title`, `CSM_Option_MultiUsage` | NPCs, mobs, items |
| `PlayerActionHandler` | 28 `CSM_Player_*` and `CSM_Option_Duel` texts, plus `CSM_Player_UIToggleOptions` (below) | player-like templates, context-menu globals |
| `TakeableActionHandler` | `CSM_Item_Option_Pickup` | no set |
| `TextItemActionHandler` | `CSM_TextItem_Option_Read`, `…_ReadEdit`, `…_Edit`, `…_Copy`, `…_Paste`, `CSM_Menu_Title` | text items |

`CSM_Player_UIToggleOptions` is an array of structs (`CSM_Player_UIToggleOption_RequiredType`
enum, `…_String` StringInfo, `…_GOP` a PID, optional `…_Ripcord` DID → ripcord record,
`map-data.md` §11); two structs here. Its bundle mirror: the root's reference attribute
0x06AFFAD3 names a `refmap_u32` keyed by the `_GOP` PID whose values are class-3427 objects
with three attributes: 0x09E20BDC (reference) → a `string_info` object equal to `…_String`,
0x02355935 (int) = `…_RequiredType`, 0x00334924 (int) = `…_Ripcord` DID or 0 (I3a).

WEENIECONTENT `ContextMenuSystemGlobals` names one record (class 1727 here) with two
properties: `CSM_ActionHandler` (→ `DefaultActionHandler`) and `CSM_PlayerActionHandler`
(→ `PlayerActionHandler`), the handlers used when an object names none.

The dictionary defines 70 `CSM_*` names; 60 are stored, all in these records, and
`CSM_ActionHandler` is also stored on items and templates (`items.md` §5.10).

**What the click handler does (BC).** The context-menu system's click handler (class 4
here, function 1) finds the clicked object by its instance ID and:

1. takes the handler DID from the clicked object's `CSM_ActionHandler`, or, when the object
   has none, the `CSM_ActionHandler` of the `ContextMenuSystemGlobals` record; it loads that
   record as a handler (the handler root class, 506 here) and stops with a logged error when
   any of these is missing;
2. reads `CSM_DisplayInactiveOptions` (bool) from the **clicked object** (not from the
   handler) and passes it to the new menu;
3. asks the handler to build the menu (one function per handler class, the same virtual
   slot in all 14) and shows the menu when the build succeeds and a check on the menu passes
   (by inference: that it has options).

The default handler (`DefaultActionHandler`) adds one option whose text is the clicked
object's own `CSM_Default_Option_Use` when the object stores it, else the handler record's,
enabled or not by a further test on the object; it then lets the take and examine handlers
add their options, and sets the menu title from the handler's `CSM_Menu_Title` (with a value
of the object inserted, by inference its name). The take handler adds its
`CSM_Item_Option_Pickup` option only when a class-code test on the object passes; the
examine handler always adds `CSM_Examine_Option_Examine`, enabled.

> **OPEN:** The rest of the menu assembly: which options each of the other handler classes
> adds for which object state, how an option is marked enabled or inactive (the default
> handler runs a further test on the object for that), what an inactive option looks like
> when `CSM_DisplayInactiveOptions` is true, what the take handler's test is, and the
> command each option triggers. These are the handler classes' menu functions and the menu
> natives they call, which were not read further.

<a id="s3-6"></a>
### 3.6 Permission and take-permission records (Confirmed, I9)

`Usage_Permission` (200,041 values) and `Usage_TakePermission` (196,412 values) name records
with **empty** property sets and no bundle data; their meaning is their class.

| Record (label) | Class | Named by (values) |
|---|---|---|
| 0x7000044C | 1605 | equippable items (91,154) |
| `UsagePermission_Default` 0x70000427 | 1603 | hotspots (2,305), containers, doors, travel points, recipes, most items (68,849 in all) |
| `UsagePermission_AlwaysAllow` 0x7000024B | 1600 | mobs, most NPCs, housing hooks, player-like templates (36,634) |
| 0x7000064C | 1601 | class-1210 records, controller entities, 24 hotspots (2,877) |
| 13 further records | 1608, 1609, 1612, 3055, 3144, 3188, 3195, 3204, 3685, 4076 (`UsagePermission_HousingDecorationItem`), 4116, 4184, 5722 | items and player-like templates, 0–154 each (the housing-decoration permission is named by no stored property) |
| 0x70000441 | 1548 | take permission of items (99,292) |
| `TakePermission_CantTake` 0x7000024C | 1547 | hotspots, creatures, controller entities and other world objects (54,679) |
| 0x70000426 | 1546 | take permission of items (42,441) |

Class 1603 (`UsagePermission_Default`) declares 97 functions of its own (139 slots); the
permission base class 1615 13, class 1600 7, the take-permission base 1550 8 and class 1547 5.

> **OPEN (bytecode):** What each permission class allows: the class functions are bytecode
> that no spec reads (same limits as §3.2.3). A Tessera server applies Decision D3 (§16.1).

<a id="s3-7"></a>
### 3.7 Usage plugins and the plugin directory (Confirmed, I10)

<a id="s3-7-1"></a>
#### 3.7.1 Plugins

A usage plugin is a record whose property set names **one requirement property**:

| Property | Type | Records | Meaning |
|---|---|---|---|
| `UsagePlugin_UsagePropertyName` | PID | all 77 | The requirement property this plugin evaluates (unique per plugin) |
| `UsagePlugin_TargetPropertyName` | PID | 9 | The property of the user that the requirement is compared with |
| `Usage_Operator` | enum (mapper 0x23000039) | 3 | The comparison (`EqualTo`, `NotEqualTo`, `GreaterThanOrEqualTo` here) |
| `UsagePlugin_ErrorType` | enum | 9 | The error reported when the requirement fails |
| `UsagePlugin_ExaminationString` | StringInfo | 9 | Examination (tooltip) text |
| `UsagePlugin_ShowExaminationWhenUsable` | bool | 9 | true in 4 |

The 77 requirement properties are: `Usage_AllowedInCombat`, `…AllowedInPvMP`,
`…AllowedUserList`, `…AllowedWhileMounted`, `…BarShapeshiftedCharacters`,
`…BarrAdvancedStartCharacters`, `…BarredAccountPropertyArray`, `…BarringAlignment`,
`…BarringChannelingState`, `…BarringCraftRecipe`, `…BarringEquivalenceClass`, `…BarringSkill`,
`…BarringSkill_Array`, `…BarringTrait`, `…BarringTrait_Array`, `…FellowshipGroupSize_Max`,
`…FellowshipGroupSize_Min`, `…HousingPermission_OwnerOnlyRequirement`,
`…HousingPermission_Requirement`, `…IsPlayerInAFellowship`, `…IsPlayerInARaid`,
`…IsQuestItem`, `…MaxLevel`, `…MinGloryRank`, `…MinLevel`, `…MinLevel_FloatToCap`,
`…MinMountLevel`, `…MonsterPlayerCanUse`, `…MustBeInKinship`, `…MustBeInKinshipNeighborhood`,
`…MustBeInPersonalNeighborhood`, `…MustMeetAreaChallengeLevel`, `…MustNotBeInKinship`,
`…NonSubscriber_RequiredWebstoreItem`, `…PlayerCanUse`, `…PropertyCondition_Array`,
`…QuestRequirements`, `…QuestRequirements_OrList`, `…RequiredAccountPropertyArray`,
`…RequiredAccountToken`, `…RequiredAlignment`, `…RequiredChannelingState`,
`…RequiredClassList`, `…RequiredCraftMastery`, `…RequiredCraftMasteryLocked`,
`…RequiredCraftProfession`, `…RequiredCraftProficiency`, `…RequiredCraftProficiencyLocked`,
`…RequiredCraftTier`, `…RequiredCraftTool`, `…RequiredEffect`, `…RequiredEquivalenceClass`,
`…RequiredFaction`, `…RequiredNeighborRelationship`, `…RequiredGenus`, `…RequiredHobby`,
`…RequiredMountType`, `…RequiredPurchasedTrait_Array`, `…RequiredRaces`, `…RequiredRaidLocks`,
`…RequiredTrait`, `…RequiredTrait_Array`, `…RequiredTrait_OrList`,
`…RequiresExpansionPack1_Legacy`, `…RequiresExpansionPack2_Legacy`, `…RequiresFacing`,
`…RequiresFullAccessToken`, `…RequiresSubscriber`, `…RequiresSubscriberOrUnsub`,
`…RestrictedEffect`, `…SessionPlayerCanUse`, `…Target_RequiredMaxLevel`,
`…Target_RequiredMinLevel`, `…ValidClassificationType`, `…WorldEvent_AllConditionList`,
`…WorldEvent_AnyConditionList`, `…IsPurchaseProfessionSlotUsageItem` (each with prefix
`Usage_`).

The nine plugins with a target property are comparison plugins (classes 3348, 3604, 3658,
3660 below an intermediate class 3603): `Usage_BarringChannelingState` and
`Usage_RequiredChannelingState` against `Channeling_CurrentChannelingDID` (operators
`NotEqualTo`, `EqualTo`), `Usage_MinMountLevel` against `Mount_Level`
(`GreaterThanOrEqualTo`), `Usage_RequiredClassList` against `Agent_Class`,
`Usage_RequiredRaces` against `Agent_Species`, `Usage_RequiredGenus` against `Agent_Genus`,
`Usage_RequiredCraftTool` against `Inventory_CraftToolCategory`,
`Usage_BarShapeshiftedCharacters` against `Shapeshift_MeshIID`,
`Usage_BarrAdvancedStartCharacters` against `Character_LeveupUsageAction_Tier`.

37 plugins share the base plugin class (3227) and differ only in their property name; the
others are each the only record (or one of two) of a class of their own.

<a id="s3-7-2"></a>
#### 3.7.2 The directory

WEENIECONTENT `UsagePluginDirectory` names one record (class 3228 here):

- Property set: `UsagePluginDirectory_UsagePluginArray`, an array of 77 DIDs
  (`UsagePluginDirectory_UsagePluginArray_Entry`), each a plugin.
- Object record: 77 imports (DB-object type 0x45, flags 16), one per plugin; the bundle is
  the root (two reference attributes), a `refarray` (native 176) of 77 references to DID
  items in array order (attribute 0x05076D19), the 77 DID items, and a `refmap_u32`
  (attribute 0x095017C8) whose keys are the plugins' `UsagePlugin_UsagePropertyName` PIDs
  and whose values reference the DID item of that plugin.

So the directory is a map **requirement PID → plugin**. To evaluate an object's
requirements, a reader takes each `Usage_*` property of the object (and of its
`DefaultPermissionBlobStruct`), looks its PID up in the map and runs that plugin.

<a id="s3-7-3"></a>
#### 3.7.3 Boundary with the bytecode

Plugin classes have **no attributes** (0 slots) and a virtual-function table of 97–103
slots; each declares 4–21 functions of its own and overrides 3–12 slots of its parent. The
evaluation of a requirement is that code. The dictionary IDs of all five `UsagePlugin_*`
names occur as 32-bit literals in the bytecode chunk (once each, OBS); the native client
reads `UsagePluginDirectory_UsagePluginArray` in five routines (BIN e).

> **OPEN (bytecode):** The evaluation rule of each plugin (for example how `Usage_MinLevel`
> compares, which error it reports, how the comparison plugins apply `Usage_Operator`). The
> plugin functions are bytecode that no spec reads, and they call native functions that
> `wlib-bytecode.md` §7.1 leaves unspecified. A Tessera server implements the rules of
> Decision D4 (§16.1) until such a reading exists.

<a id="s3-8"></a>
### 3.8 Induction actions (Confirmed, I11)

<a id="s3-8-1"></a>
#### 3.8.1 Property set

1,779 records (class 1195 here), 15–21 properties each. Named by `Induction_ActionDID`
(objects, templates and instance sets; 20,665 non-null values), `Skill_InductionAction` (skills,
`combat-data.md` §7.9), `CraftRecipe_InductionAction`, `PrivateEncounter_LeaveInstanceInduction`,
`ItemAdvancement_Imbuement_InductionActionDID`, `PaperItemService_InductionActionDID`, quest
objectives and 12 `Usage_AppliedChannelingState` values in instance sets; 1,776 of 1,779 are
named by something.

| Property | Type | Records | Values / meaning |
|---|---|---|---|
| `Induction_Duration` | float, s | all | 0.1–60; 3 (474), 2 (276), 5 (253), 4 (206) |
| `Induction_Duration_AdditiveModifierList` | array of `Induction_Duration_ModifierProperty` (PID) | 54 | 1 or 2 PIDs (47, 7), 22 distinct; properties of the user that change the duration (how: OPEN, §3.8.5) |
| `Induction_Duration_MultiplierModifierList` | same | 509 | 1–3 PIDs (72, 414, 23), 29 distinct; 45 records hold both lists |
| `Induction_Duration_AreaChallengeLevelOverride` | float (dictionary type float, default 0) | all | 0.0 (1,767), 60.0 (12); meaning OPEN (§3.8.5) |
| `Induction_Interruptable` | bool | all | true 1,230 |
| `Induction_AllowedWhileMoving` | bool | all | true 34 |
| `Induction_AllowedInCombat` | bool | all | false 108 |
| `Induction_FailOnRestart` | bool | all | true 882 |
| `Induction_SetbackPolicy` | enum | all | 1 `ModifyDuration` (116), 2 `ModifyStartTime` (1,663) |
| `Induction_SetbackEventsArray` | array of `Induction_SetbackStructure` | 881 | 954 structures, each `Induction_EventID` (enum, mapper 0x23000154) plus **exactly one** of `Induction_SetbackTime` (float; 824 structures) or `Induction_SetbackProgression` (DID → progression record, `combat-data.md` §4; 130 structures). No record mixes the two: 763 records hold only times, 118 only progressions (§3.8.5). No event repeats within a record. |
| `Induction_SetbackIgnoreProp` | PID | all | 0 in 1,700; `Skill_Induction_Setback_ResoluteAim` in 78 (76 of them with setbacks), `ItemSet_Loremaster_Induction_Removal` in 1; both bool properties, default false |
| `Induction_AnimationScriptID`, `Induction_MountedAnimationScriptID`, `Induction_TargetAnimationScriptID` | enum (script keys) | all, 34, all | 410 distinct animation keys |
| `Induction_FXID`, `Induction_FailureFXID`, `Induction_TargetFXID`, `Induction_TargetFailureFXID` | enum (script keys) | 333, 395, 89, 1 | |
| `Induction_TitleString`, `Induction_CompletionString`, `Induction_FailureString` | StringInfo | all | |
| `Induction_ProgressBarElement` | enum (UI element) | all | one value (`GamePlay_ProgressBar`) |
| `Induction_AI_Hints` | bitfield (40 bits) | all | |
| `Induction_UserEffectList` | array of `EffectGenerator_EffectStruct` (`EffectGenerator_EffectID`) | 101 | effects applied to the user (121 elements) |

<a id="s3-8-2"></a>
#### 3.8.2 Object record and bundle mirror

The root object (class 1195) has six attributes; every value is a mirror (§2.4):

| Attribute hash | Type | Content |
|---|---|---|
| 0x0649E1D3 | reference | `array_u32` (native 25) of the additive-modifier PIDs in order; null when the list is absent |
| 0x02A4CC63 | reference | `array_u32` of the multiplier-modifier PIDs in order; null when absent |
| 0x0A6A3D23 | reference | `refarray` (native 182) of class-700 objects, one per `Induction_UserEffectList` element in order; null when absent |
| 0x0C1658F0 | int | the `Induction_SetbackIgnoreProp` PID (0 for none) |
| 0x0FAF94BE | reference | `refmap_u32` (native 35): `Induction_EventID` → reference to a DID item holding that event's `Induction_SetbackProgression` (always present, may be empty) |
| 0x08BD33A0 | reference | `map_u32` (native 11): `Induction_EventID` → `Induction_SetbackTime` as f32 bits (always present, may be empty) |

The class-700 objects have the attributes of `quests.md` §6.3 with the effect DID in
0x8CF9154; in all 121: 0x35D2793 = 512, 0x8BBFE54 = 0, 0xCF31763 = 0, spellcraft −1.
The 118 records with a setback progression import it (DB-object type 0x45, flags 16).

<a id="s3-8-3"></a>
#### 3.8.3 Well-known inductions

WEENIECONTENT `PickupInductionAction`, `TravelInductionAction` and
`LayerTransferInductionAction` name three induction records (observed durations 2 s, 60 s
and 10 s; read them at run time).

<a id="s3-8-4"></a>
#### 3.8.4 The induction bar (BIN, I12)

For an entity whose induction is shown, the client takes the bar's start and end from the
entity's `Skill_InductionAction_StartTime` and `Skill_InductionAction_EndTime`
(timestamps; never stored in the data, sent by the server) and the induction record from the
entity's `Skill_InductionAction`. From that record:

- **Interruptible**: true when the record has `Induction_Interruptable` = true, or else
  when it has `Channeling_Interruptable` = true; false otherwise. The bar uses one of two
  styles accordingly.
- **Title**: `Induction_TitleString`; if absent, `Channeling_TitleString`; if the text is
  still empty and the entity has `Skill_DefaultSkill`, that skill's `Skill_Name`.

So the same bar serves induction records (§3.8) and channelling records (§3.9).

<a id="s3-8-5"></a>
#### 3.8.5 Duration modifiers and setbacks: what the data show (I11a, I11b)

The client does not compute induction durations or setbacks: no native routine refers as a
constant to the modifier lists, the setback array, its policy, progression or ignore
property, the area-challenge override or `Induction_CurrentSetbackCount` (BIN e, h), and the
bar follows the start and end times the server sends (§3.8.4). In the class library's code
none of these PIDs occurs as a literal except the multiplier-list PID (twice: in a skill
function that only hands the PID on to another function, and in one function of another class that
was not read), and the induction class's own code
(class 1195 here) only fills its six attributes (§3.8.2) and returns the two modifier lists
(BC). The rules therefore live in the server, so only the data side is known:

**Modifier properties.** Every PID in either modifier list is a float property with
dictionary default 0. The most common are `Skill_InductionDuration_AllSkillsMod` (in 481
multiplier lists), `Skill_InductionDuration_MonsterSkillsMod` (380 multiplier lists) and
`Skill_InductionDuration_AllSkillsMod_Add` (17 additive lists). Effects and traits change
them through `Mod_Entry` structs (`combat-data.md` §5). Over every entry in every 0x79 set
that modifies one of them (a property in both lists counted with the multiplier group;
census re-run 2026-10-08):

| Group | Operator | Inline entries (values) | Progression-driven entries |
|---|---|---|---|
| multiplier-list properties | `Add` | 287 (−1.0 to 2.0) | 1,521 |
| | `Subtract` | 116 (0 to 1.0) | 4 |
| | `Multiply` | 25 (0 to 2.0) | 0 |
| | `Set` | 1 (0.5) | 0 |
| additive-list properties | `Add` | 462 (−4.5 to −0.25) | 1 |
| | `Subtract` | 12 (0.2 to 1.5) | 1 |
| | `Set` | 1 (0.0) | 0 |

One property (`Skill_InductionDuration_Loremaster_Trait_LightofHope`) appears in both kinds
of list. These magnitudes fit "additive list = seconds added" and "multiplier list = a
fraction of the duration added", but no source states it.

**Setbacks.** Event IDs in the 954 structures: `Combat_OnTargetOfHit` (881: in every record
with an array), `Combat_OnTargetOfCriticalHit` (23), `Combat_OnSelfParry` (18),
`Combat_OnSelfBlock` (18), `Combat_OnSelfEvade` (14). Policy against content:

| Records | Setback content | `Induction_SetbackPolicy` |
|---|---|---|
| 763 | times only (1, 2, 4 or 5 structures) | all `ModifyStartTime` |
| 118 | progressions only (1 or 5 structures) | 114 `ModifyDuration`, 4 `ModifyStartTime` |
| 898 | no setback array | 896 `ModifyStartTime`, 2 `ModifyDuration` |

Setback times range from 0.7 to 20 (most common 5 in 396 structures, 7 in 157, 10 in 106)
and do not follow the induction's duration (a 0.5 s induction can carry a setback time of 5).
The 130 progression structures name two progression records, both float progressions
(`combat-data.md` §4) with minimum index 1 and 10 entries, non-zero only in the first entry
(one record) or the first two (the other), with values below 1. The dictionary also defines
`Induction_CurrentSetbackCount` (int, default 0); no induction record stores it. `Induction_Interruptable` is
false in 31 records that have setbacks, so the two are independent in the data.

> **OPEN (server):** How the server combines the modifier lists with `Induction_Duration`:
> which value of the user each PID contributes, whether several multiplier entries add or
> multiply, and the order of the two lists. Neither the client's native code nor the class
> library's code computes it (above); only official-server behaviour or an owner observation
> of cast times with known modifiers would settle it. Tessera's rule: Decision D9 (§16.1).

> **OPEN (server):** The unit of `Induction_SetbackTime` (seconds, tenths of a second or a
> percentage of the duration all fit some records), what `ModifyStartTime` and
> `ModifyDuration` do with it, whether a setback progression is indexed by the setback count,
> and any limit on the number of setbacks. No client code reads them (above); an owner
> observation of the cast bar while being hit would settle the unit. Tessera's rule:
> Decision D10 (§16.1).

> **OPEN (server):** The meaning of `Induction_Duration_AreaChallengeLevelOverride` (0.0 or
> 60.0). No client code reads it (above). Tessera ignores it (D9).

<a id="s3-9"></a>
### 3.9 Channelling states (Confirmed, I13)

<a id="s3-9-1"></a>
#### 3.9.1 Property set

401 records (class 961 here), 38–44 properties each. All 401 are named somewhere:
`QuestEvent_ChannelingState` in quests (169 distinct through objectives, 12 through failure
conditions), `Skill_Toggle_ChannelingState`
(129), `Effect_ExitChannelingState_StateToExit` (65), `Effect_EnterChannelingState_StateToEnter`
(16), `Item_Music_Channeling_State` (38, `instruments.md`), `Usage_AppliedChannelingState`
(items, interactive objects, 54 in instance sets), `Usage_RequiredChannelingState`
(hotspots 14, items, instance sets), `Usage_BarringChannelingState` and requirement structs.

| Property | Type | Records | Values / meaning |
|---|---|---|---|
| `Channeling_Duration` | float, s | all | 1–99,999; 99,999 in 106 and 9,999 in 19 (effectively unlimited) |
| `Channeling_Duration_Multiplier_PropertyName` | PID | all | 0 in 397 |
| `Channeling_Interruptable`, `…_InterruptedByMovement`, `…_InterruptedBySwimming`, `…_InterruptedByTeleport`, `…_CanBeAbandoned` | bool | all | true in 345, 122, 342, 76, 302 |
| `Channeling_BarredActions` | bitfield 32 | all | bits `Emotes` 1, `Combat` 2, `Usage` 4, `Crafting` 8; 0xF in 140 |
| `Channeling_SetbackEventsArray` | array of `Channeling_SetbackStructure` (`Channeling_EventID`, `Channeling_SetbackTime`) | 211 | 326 elements; no event repeats within a record |
| `Channeling_SetbackIgnoreProp` | PID | all | 0 in 400 |
| `Channeling_AdditionalEffectDID`, `…AdditionalEffectSpellcraft` | DID → effect, float | 57, 4 | |
| `Channeling_StartScriptID`, `…_Success_Script`, `…_Failure_Script`, `…_Abandoned_Script`, `…AnimationScriptID` and fifteen target-side and FX variants | enum (script keys) | all | |
| `Channeling_TitleString`, `…CompletionString`, `…FailureString`, `…AbandonString`, `…AbandonDialogString` | StringInfo | all | |
| `Channeling_ProgressBarElement` | enum (UI element) | all | three values (plain, mounted, music) |
| `Channeling_ProhibitWeaponVisibility` | bool | all | true 282 |
| `Channeling_Player_Cosmetic_Slot_Entity`, `…_HoldingLocation`, `…_Scale`, `…_OffsetRotationPitch`/`Roll`/`Yaw`, `…_Behavior`, `…_UseParticleScale` | DID → 0x47, enum, vector, floats, enum, bool | 278, 282, 108, 35/22/21, 8, all | A prop the user holds while channelling (`appearance.md` §5.5) |
| `Channeling_AI_Hints` | bitfield (40 bits) | all | |
| `EffectGenerator_OnRemoval_Effect`, `Effect_ForceHotspotUpdate` | struct, bool | 1, 1 | |

<a id="s3-9-2"></a>
#### 3.9.2 Object record

Root (class 961) with two attributes: 0x0C1658F0 (int) = `Channeling_SetbackIgnoreProp`, and
0x08BD33A0 → `map_u32` (native 11): `Channeling_EventID` → `Channeling_SetbackTime` as f32
bits (empty when the array is absent). These two hashes are the same as in induction records
(§3.8.2).

<a id="s3-9-3"></a>
#### 3.9.3 Setbacks, barred actions and duration multipliers: what the data show (I13a)

- **Setback events** (326 structures in 211 records): `Combat_OnTargetOfHit` 207,
  `Combat_OnTargetOfCriticalHit` 68, `Combat_OnTargetOf_SuperCriticalHit` 19,
  `Combat_OnSelfParry`, `Combat_OnSelfEvade`, `Combat_OnSelfBlock` 8 each,
  `Combat_OnPerformCriticalHit` 4, `Combat_OnConjunctionSuccess`, `Combat_OnSelf_PartialBlock`,
  `Combat_OnTarget_PartialParry` 1 each, and 0 (`Undef`) once.
- **Setback times:** 0.1 to 99,999; most common 10 (117), 99,999 (42), 30,000 (41), 5 (25),
  1 (23), 3,600 (18). Values of 9,999 and above match the "unlimited" durations of §3.9.1.
- **Ignore property:** non-zero in one record (`Skill_Induction_Setback_ResoluteAim`, the
  bool also used by inductions).
- **Duration multiplier** (`Channeling_Duration_Multiplier_PropertyName`, 4 records): four
  different float properties, dictionary default 0 in three and 1 in one; effects change them
  only with `Add` (inline values −0.4 to 4.5; the rest progression-driven).
- **Barred actions:** 0x0 in 18 records, `Combat` 23, `Emotes | Combat` 6, `Usage` 1,
  `Combat | Usage` 109, `Emotes | Crafting` 9, `Emotes | Combat | Crafting` 95, all four 140.

> **OPEN (server):** What a channelling setback does (shortens the remaining time, ends the
> state, or something else), and the unit of `Channeling_SetbackTime`; how the duration
> multiplier is applied; which requests each barred-action bit refuses; what the interrupt
> flags count as movement, swimming or teleport. In the class library's code
> `Channeling_SetbackTime`, `Channeling_SetbackEventsArray` and `Channeling_BarredActions` do
> not occur as literals; `Channeling_Duration` and `Channeling_Duration_Multiplier_PropertyName`
> occur once each, in functions of the channelling class, and
> `Channeling_InterruptedByMovement` in four functions of two other classes, none of which
> was read (BC). Tessera's rules: Decision D12 (§16.1).

---

<a id="s4"></a>
## 4. Hotspots

<a id="s4-1"></a>
### 4.1 What a hotspot is

A hotspot template is an object whose `WeenieType` is `Hotspot`: an area-effect object that
applies effects to entities in or near it (campfires, crafting facilities, farmland,
quest-area markers). In this install all 2,329 templates of class 1170 have:

- `Hotspot_IsHotspot` = true, `Entity_CreationType` = 8 (`UniqueDynamicImmobile`): the client
  never creates a hotspot from landblock data (`entity-descriptor.md` §4.9); it appears only
  when a server spawns it;
- `Usage_Action` = `UsageAction_DoNothing`, `Usage_TakePermission` = `TakePermission_CantTake`;
- a `PhysObj` (76 distinct 0x47 descriptors; two cover 1,957 templates);
- `SeeMeVisibilityMask` 1 `Obvious` (1,054) or 2 `Hidden` (1,275) (§5.3).

Decoration items also carry `Hotspot_IsHotspot` = true (5,043 records of the general item
class: 5,004 decorations with `Hotspot_EnableVisualization` = true, 39 non-inventory
objects; 62 of them have a hotspot effect list; `items.md`).

<a id="s4-2"></a>
### 4.2 Object record (Confirmed)

| Part | Content |
|---|---|
| Root (class 1170) | Three reference attributes: 0x0BCFA314 (null in all), 0x035B97AF (null in all), 0x04343517 → the class-701 effect-generator object of §4.3, null in the 83 templates without a hotspot effect list |
| Link section | 2,328 records: (dir 0, type 12, limit 250), (dir 1, type 12, limit 250), (dir 0, type 8, limit 1), all flag 0, no PIDs, terminator 3; one record: (0, 8, 1) and (1, 0x10000007, 250, PID `LinkName`) |
| Available-property list | All 2,329. Union of 41 names: `Hotspot_AffectsMonsterPlayers`, `…AffectsMonsters`, `…AffectsPets` (2,325), `…AffectsPvPPlayers` (1), `…AffectsRegularPlayers`, `…AffectsSessionPlayers` (6), `…Box_MaxExtent`, `…Box_MinExtent` (2,325), `…CycleInterval`, `…Effect_IgnoreConsideration`, `…Effect_IgnoreProbability`, `…Effect_Spellcraft`, `…FX_Active`, `…FX_ActiveIntensity`, `…FX_Inactive`, `…FX_InactiveIntensity`, `…FX_OnActivation`, `…FX_OnActivationIntensity`, `…FX_OnDeactivation`, `…FX_OnDeactivationIntensity`, `…FX_OnTrigger`, `…FX_OnTriggerIntensity`, `…Policy`, `…Trigger_Delay` (2,326 each), `…Effect_SourceLevel` (2,325), `…RadiusOverride` (2,328), `…StartsActive`, `…TargetFilterPermissions`, `…Trigger_MaxCount` (all), `HotSpot_EffectChannel_Driven_Tier` (108), `EffectGenerator_HotspotEffectList`, `EffectMod_ModType_DamageMultModifier_Add`, `EffectMod_ModType_HealMultModifier_Add` (2,325), `MapNote_Enabled` (15), `PEMessageSystem_ResponderRegistration` (5), `QuestDispenser_RoleConstraint` (247), `WorldEvent_ActivateEventList`, `WorldEvent_DeactivateEventList`, `WorldEvent_ToggleEventList` (118), `WorldEvent_FireOnInitialize` (114), `State@Entity.m_iCount_NetShared` (1). |

The available-property list names the properties an instance may set (`wstate.md` §7).
`Hotspot_TargetFilterPermissions` is a struct whose 32 member names are `Usage_*`
requirement properties (the same names the plugins of §3.7 evaluate); `Hotspot_VisualizationData`
(never stored) groups `Hotspot_InteractionPerms` (19 `Interaction_Permission_*` members) and
`Hotspot_Characteristics`.
Most hotspot behaviour properties (`Hotspot_Policy`, `…CycleInterval`, `…Trigger_*`, `…FX_*`,
`…Affects*`, `…StartsActive`, `…RadiusOverride`) appear **only** there: no template, instance
set or other record in the install stores a value for them (§14). Their values come from the
dictionary defaults or from the server.

<a id="s4-3"></a>
### 4.3 The compiled effect list (Confirmed, I14)

In the 2,246 templates with `EffectGenerator_HotspotEffectList` the bundle holds:

| Item | Class | Content |
|---|---|---|
| effect generator | 701 (script, 13 attributes) | 0x07D7CE15 → the key multimap; 0x0C9D580F → the element map; 0x035D2793 (int) = 0x00100010 (all); 0x07BB0CF2 (int) = the largest element key; the int attribute 0x0BF23944 = 0; the eight reference attributes 0x0377E823, 0x09C0A1D4, 0x0D407C53, 0x0617E478, 0x08F6F088, 0x073C68C8, 0x049EC7B5 and 0x0E683629 null (I3a) |
| key multimap | 13 `multimap_u32` | one entry per element: key 64, value = the element key |
| element map | 35 `refmap_u32` | element key → its class-700 object |
| element | 700 (script, 15 attributes, `quests.md` §6.3) | effect DID (0x8CF9154), spellcraft (0x2712104), its own key (0x8BBFE54), 0xCF31763 = 64, 0x35D2793 = 513; 0x7359352 → a class-699 object or null |
| element data | 699 (script, 1 attribute 0x097FACFE) | → a `property_set` (native 166) |
| — | 1626 (script, 5 × u64, all 0: 0x041F1C86, 0x00735FF4, 0x036DFFF6, 0x0F96B506, 0x08DCC216) | one per element (0x53151DF) |

Rules (all 2,246):

- Element keys are 3, 4, 5, … in list order; the multimap holds the same keys; 0x07BB0CF2 is
  the last key (lists of 1 to 7 elements: 1,792 of length 1, 297 of 2, 96 of 3, 45 of 4, 4 of
  5, 7 of 6, 5 of 7; 2,951 elements).
- The *i*-th element's effect DID and spellcraft equal the *i*-th `EffectGenerator_EffectStruct`'s
  `EffectGenerator_EffectID` and `EffectGenerator_EffectSpellcraft` (−1 when absent).
- A class-699 object is present for 627 elements: for the 349 structs that carry
  `EffectGenerator_EffectDataList`, its set holds that list's entries
  (`EffectGenerator_EffectChannel`); for 278 elements whose **effect record** (class 749
  here) carries its own `EffectGenerator_EffectDataList`, its set holds the fields of the
  effect's list (destination, scene and private-instance flag; same PIDs, count checked in
  all 278), copied from the effect, not from the hotspot set.

<a id="s4-4"></a>
### 4.4 Property set census (Confirmed, I14)

35–61 top-level properties per set (55 in 1,976). Besides the mirrors and class-wide copies
that every object carries (`creatures.md` §3.2.1; `MC_Trait_*`, `BigBattle_*`, `Boss_Zone_1`,
`Skirmish_RoleCombo` and their `Forward*` copies, constant in all 2,329):

| Property | Records | Values |
|---|---|---|
| `Name` | 2,329 | StringInfo (2,311 distinct) |
| `Description` | 2,310 | StringInfo |
| `PhysObj` | 2,329 | §4.1 |
| `Physics_EtherealityType` | 2,328 | `Default` 2,321, `ClosedDoor` 3, 0 in 4 |
| `Physics_EtherealToType`, `Physics_PlacementEtherealToType` | 2,328 | 0x2E880003 / 0x2E980003 in 2,299: players, creatures, NPCs, corpses and the camera pass through (`collision.md` §9.2) |
| `Physics_AdjustableScale`, `Render_AdjustableScale` | 2,279, 2,280 | uniform; 1.0 in 1,727 |
| `Physics_NoPhysics`, `Physics_NoPathmaps` | 1, 1 | true |
| `IsSelectable` | 2,329 | false 2,298, true 31 |
| `Selection_Script` | 31 | one key (`ui_selected_fx`) |
| `UI_FloatyText_HideFloatingName` | 2,309 | true 1,549 |
| `Entity_AdditionalScriptTables` | 2,329 | 2,517 script-table DIDs (0x07; `script-table.md`) |
| `Entity_ClassScriptTable` | 2 | |
| `EffectGenerator_HotspotEffectList` | 2,246 | §4.3; effects of classes 708 (419 elements), 718 (290), 715 (104) and others; 2,221 distinct effects |
| `EffectMod_ModType_DamageMultModifier_Add`, `…HealMultModifier_Add` | 2,309 | 0 in 2,202 / 2,305 |
| `Effect_ForceHotspotUpdate`, `Effect_Additional_Mod_Properties`, `Effect_IsEnchantable` | 19, 17, 1 | |
| `Hotspot_IsHotspot` | 2,329 | true |
| `Hotspot_Box_MinExtent`, `Hotspot_Box_MaxExtent` | 2,309 | vectors (m); (0, 0, 0) in 2,294 |
| `Hotspot_EnableVisualization` | 2,289 | true 986 (dictionary default true) |
| `Hotspot_EnableInactiveVisualization` | 2,226 | true 17 (default true) |
| `HotSpot_InteractionPermissions_Enabled` | 2,309 | true 192 |
| `HotSpot_EffectChannel_Driven_Tier` | 108 | true 103 |
| `Hotspot_MustRequireEpicQuest` | 20 | true |
| `Usage_Action`, `Usage_Action_Shadow` | 2,329 | `UsageAction_DoNothing` |
| `Usage_MinUsageDist` (+ `_Shadow`) | 2,329 | 2 (2,326), 4 (3) |
| `Usage_Permission` | 2,329 | `UsagePermission_Default` 2,305, class 1601 24 |
| `Usage_TakePermission` | 2,329 | `TakePermission_CantTake` |
| `Usage_CancelsSafeMode`, `Usage_InventoryRequired`, `Usage_UseWhileMoving`, `Usage_ShouldApplyEffectsToTarget` | 2,309 | true / false / false / false |
| `Usage_RequiredGenus` | 2,309 | 0 in 2,305 |
| `Usage_RequiredChannelingState` | 15 | channelling records (14 distinct) |
| `Usage_RequiredEffect`, `Usage_RestrictedEffect` | 20, 18 | effects |
| `Usage_QuestRequirements`, `Usage_RequiredRaces`, `Usage_RequiredClassList`, `Usage_IsQuestItem`, `Usage_BarringTrait`, `Usage_AllowedWhileMounted`, `Usage_MonsterPlayerCanUse` | 14, 8, 2, 15, 1, 1, 1 | |
| `Induction_ActionDID` | 2,309 | null in 2,308 |
| `MultiUsage_ActivityCollection` | 3 | §9 |
| `MapNote_Enabled`, `MapNote_Level`, `MapNote_StringInfo`, `MapNote_Type` | 15 | map-note type bit 44 in all 15 (`map-data.md` §4) |
| `QuestDispenser_RoleConstraint`, `Quest_RelatedQuest`, `QuestEvent_ScriptCallback_Key` | 4, 3, 3 | |
| `Render_Color`, `Render_Opacity`, `Appearance_ClassList`, `Entity_IsGloballyVisible`, `BigBattle_Interactive_Disarm_Mod` | 30, 2, 1, 1, 3 | |
| `WeenieType`, `Entity_CreationType`, `SeeMeVisibilityMask` | 2,329 | §4.1 |

Of the 51 `Hotspot_*` names in the dictionary, 10 are stored anywhere (and 2 of the 3
`HotSpot_*`); §14.

<a id="s4-5"></a>
### 4.5 Instance sets (Confirmed)

749 landblock instance sets (none with a static entity) carry `Hotspot_*` properties; they
configure generator-placed hotspots. 340 of them are the source end of a Generator link
(type 8, `creatures.md` §6.3); 108 take part only in Script links (type 12); 301 have no
link. Contents: `Hotspot_Box_MaxExtent` (644), `Hotspot_Box_MinExtent` (601),
`Hotspot_EnableVisualization` (105, true 12), `HotSpot_EffectChannel_Driven_Tier` (8; 143
in all instance sets), `Physics_AdjustableScale` and `Render_AdjustableScale` (749),
`Render_Color` (69), `Entity_ContentLayers` (219), `Entity_InstanceScriptTable` (98),
`QuestDispenser_RoleConstraint` (19), `Physics_NoPhysics` (8), an
`EffectGenerator_HotspotEffectList` (4), `MapNote_Enabled` (1).

<a id="s4-6"></a>
### 4.6 Where hotspots come from (Confirmed, I15)

800 of the 2,329 templates are named by another property set or object record:

| Route | Templates | Notes |
|---|---|---|
| `Effect_Genesis_SummonedObject` of summoning effects (class 719 here) | 795 | each such effect also imports the hotspot (DB-object type 0x45) |
| Generator profiles (`GeneratorProfile_WSLEntity`, `creatures.md` §6.5) | 4 | |
| One instance set's `EffectGenerator_HotspotEffectList` | 1 | names a hotspot template where an effect is expected (data error; skip it) |

The map-note table names class-1170 entities in 320 rows and the quest-marker table in 1,718
rows (`map-data.md` §4.5, §5.3); those rows carry positions. The remaining templates are
placed only by the server.

**What a spawned hotspot draws (I14a).** All 76 `PhysObj` descriptors of the hotspot
templates resolve through their 0x47 chains to setups (`setup.md` §3.1), and every one of
those setups has mesh parts: 73 descriptors one part, 3 descriptors two parts, 46 distinct
meshes in all. The two most common kinds are setups with one collision sphere whose
`physics_bounds` give radius 0.40 m and height 0.80 m (4 descriptors, 1,232 templates) or
radius 0.15 m and height 0.30 m (15 descriptors, 965 templates);
by collision kind: one sphere 27 descriptors (2,235 templates), none 21 (38), collision mesh
17 (29), box 6 (8), capsule 5 (19). The client draws a spawned hotspot like any entity, from these meshes. No client
code names `Hotspot_EnableVisualization`: its PID is no native constant (BIN e) and no
literal of the class library's code (BC), so the client does not add a marker of its own for
it. Tessera draws the descriptor's meshes and ignores the property (an explorer may use it,
§17).

> **OPEN (server):** How a placed hotspot's area is defined when `Hotspot_Box_*` are zero
> (2,294 templates): presumably `Hotspot_RadiusOverride` (default −1) or the effects' own
> radii. No client code names `Hotspot_Box_MinExtent`, `Hotspot_Box_MaxExtent` or
> `Hotspot_RadiusOverride` (no native constant, no literal of the class library's code; BIN
> e, BC), so the area is applied by the server only.

---

<a id="s5"></a>
## 5. Controller entities

<a id="s5-1"></a>
### 5.1 Census (Confirmed, I16)

26 templates in 10 classes. All have `SeeMeVisibilityMask` = 2 `Hidden` (one tripwire 1
`Obvious`), `Entity_CreationType` = 8, `IsSelectable` = true, `Selection_Script`,
`Usage_Action` = `UsageAction_DoNothing`, `Usage_MinUsageDist` = 2, `PhysObj`, and
`Physics_EtherealityType` = `Physics_EtherealToType` = `Default`; 24 (all but the two
generators) have `NeverHousekeep` = true. Their names are developer labels.

| Class | Records | `WeenieType` | Distinguishing data | Link schema | Available properties (instance-settable) |
|---|---|---|---|---|---|
| 1445 | 5 | `Relay` | — | (0, 8, 1), (0/1, 12, 250) | `Relay_Active`, `…DeactivateOnOutput`, `…DeactivateOnReset`, `…FuseLength`, `…RecoveryAfterFuseCancelled`, `…RecoveryTime`, `PropertyResponse_EntityPropertyResponseMap`; in some `Relay_FuseLength_Min`/`_Max`, `WorldEvent_*` lists, `PEMessageSystem_ResponderRegistration` |
| 1510 | 10 | `Tripwire` | `Usage_AllowedWhileMounted` | same (2 without the type-8 entry) | `Tripwire_Active`, `…Box_MinExtent`, `…Box_MaxExtent`, `…Radius`, `…DeactivateOnTrip`, `…Accepts*` (players, monster players, PvP, session players, pets, monsters), `…ExclusivelyAcceptedEntityTypeList`, `…Broadcast_Radius`, `…Broadcast_TriggerName`, `…Billboard_Text`, `DefaultPermissionBlobStruct`, `QuestDispenser_RoleConstraint`, `WorldEvent_*` |
| 1892 | 1 | `WorldPropertyBox` | — | (0/1, 12, 250) | `WorldPropertyBox_WorldEvent`, `…IgnorePropertyAdded`, `WorldEvent_CustomConditionArray` |
| 2592 | 3 | `WorldPropertyBox` | `PatrolBox_Patrol_Selection_Array` (2): elements with `PatrolBox_Order_Number` (enum), `PatrolBox_Order_Weight` (int), `PatrolBox_Patrol_Name` (string) | (0/1, 12, 250) | `PatrolBox_Patrol_Selection_Array` |
| 1995 | 1 | `Landmark` | `Area_AllowSummoning`; second bundle item an empty `array_u32` | (0, 0x1000000D, 1, flag 1) | `Landmark_Radius`, `MapNote_*` (9 names) |
| 2516 | 2 | `Generator` | `Generator_IsAGenerator`, `Generator_IsScaling`, `AI_LinkedCombat`, `Item_Class`, `WorldEventGenerator_ResponseMap_Array` (world event → response callback) | (1, 8, 250), (0, 2, 100, `LinkName`), (0, 7, 100) | 31 names: 21 `Generator_*`, `GeneratorProfile_Properties`, `AI_DelayAggro`, `AI_LinkedCombat`, `WorldEventGenerator_ResponseMap_Array`, 6 `WorldEvent_*` |
| 2328, 3168, 4192, 5812 | 1 each | `Entity` | singleton managers (population tracking, ripcords, scripts, neighbours) | none | none |

<a id="s5-2"></a>
### 5.2 Instance sets (Confirmed)

Landblock instance sets (none with a static entity) configure placed controllers:

- 230 carry `Tripwire_*` — every one `Tripwire_Billboard_Text`; 155 a
  `DefaultPermissionBlobStruct` with `Usage_QuestRequirements`; 222
  `Entity_CreationType` = 8. 192 are the source of a Generator link.
- 10,867 carry `WorldPropertyBox_WorldEvent` (→ world-event records of classes 1815 and 1887);
  3,637 a `WorldEvent_CustomConditionArray`; 7,305 an instance script table. They are joined
  by Script links (type 12).
- 33 carry `PatrolBox_Patrol_Selection_Array`.
- `Relay_*`, `RiddleDoor_*` and all but one `Interaction_*` name are stored nowhere (§14).

<a id="s5-3"></a>
### 5.3 Visibility, housekeeping and selection

- `SeeMeVisibilityMask` (enum mapper 0x2300029B: 1 `Obvious`, 2 `Hidden`) is stored on 2,329
  hotspots, the 26 controllers, 2,731 records of class 1210, the 50 generator templates
  (`creatures.md`), 54 NPC templates, 41 items and one record each in 22 other classes.
- `NeverHousekeep` (bool) is stored on 24 controllers, 2,733 class-1210 records, 1,495 items,
  228 travel points, 96 class-3725 records, 15 NPC templates and a few single records.
- The PID of `SeeMeVisibilityMask` does not occur as a constant in the client's native code
  (BIN e), nor as a literal in the class library's code (BC: its one apparent match is a
  class index). The companion `CanSeeVisibilityMask` (PID 0x5B4, enum, no default) is read by
  the class code in one place: the mouse-over eligibility test of §3.4.4, where the bit of
  value 2 on the local player makes every entity eligible.
- **Selectable** (BC, the common implementation of §3.4.4 step 1.4): an entity is selectable
  when `IsSelectable` is true and, when it holds `ClientIsSelectableOverride` (bool), that
  value is true as well.
- `NeverHousekeep` (PID 0xCD) is no native constant the scan could tell apart from a small
  number; in the class library's code the value 0xCD occurs three times as a short constant,
  each time directly before a message string, which is not the pattern of a property read
  (BC).

> **OPEN (server or owner observation):** What `SeeMeVisibilityMask` = `Hidden` does in the
> original client. No client code names the property (above), so the client does not filter
> by it itself; presumably the server does not send hidden objects to players whose
> `CanSeeVisibilityMask` lacks the bit (inference). An owner observation can settle whether
> hidden hotspots and controllers are ever visible to a normal player. A Tessera client does
> not draw entities whose effective mask is `Hidden` (Decision D5, §16.2).

> **OPEN (server):** The meaning of `NeverHousekeep` (inferred from the name only, not a
> source: the server never cleans such objects up). No client use was found (above).

---

<a id="s6"></a>
## 6. Cosmetic loadouts (Confirmed, I17)

969 records (class 1198 here), 2–12 properties, bundle = root only. Each describes the held
props of one creature look:

| Property | Type | Records | Values |
|---|---|---|---|
| `Inventory_Cosmetic_Slot1_Entity` | DID → 0x47 | 969 | 421 distinct |
| `Inventory_Cosmetic_Slot1_HoldingLocation` | enum (holding locations, `skeleton-anim.md`) | 969 | `Wpn_RHand` 954, `Wpn_LOffHand` 8, `Wpn_LHand` 5, `RHand` 2 |
| `Inventory_Cosmetic_Slot1_Scale` | vector | 154 | uniform scales, 20 distinct |
| `Inventory_Cosmetic_Slot1_Behavior` | enum (script keys) | 139 | 6 values (weapon-glow and ghost effects) |
| `Inventory_Cosmetic_Slot1_OffsetRotationRoll`, `…Yaw` | float, degrees | 1, 2 | 180; 330 |
| `Inventory_Cosmetic_Slot2_Entity`, `…_HoldingLocation`, `…_Scale`, `…_Behavior`, `…_OffsetRotationRoll` | as slot 1 | 522, 522, 85, 64, 1 | locations `Wpn_LOffHand` 193, `Wpn_LHand` 171, `Wpn_Forearm` 103, `Wpn_ROffhand` 48 |
| `Inventory_Cosmetic_Slot3_Entity`, `…_HoldingLocation`, `…_OffsetRotationPitch` | | 48, 48, 1 | `Wpn_LHand` |
| `Combat_DefaultStrikingImplementType_Melee`, `…_Ranged`, `…_Thrown` | enum | 940, 186, 48 | weapon kind for combat animation and sound |
| `Material_WeaponMaterial`, `Material_OffHandMaterial` | enum (impact materials) | 941, 286 | |

Referrers: `Inventory_Cosmetic_LoadoutSet` (arrays of 2–10 loadouts) on mob templates
(7,699 templates per `creatures.md` §4.2; 59,410 elements naming 887 distinct loadouts), on
class-1932 records (345 elements, 61 loadouts) and class-763 records (92 elements, 4
loadouts); `Inventory_Cosmetic_AppliedLoadout` on 888 mob templates (106 loadouts). Every
loadout is named.

**Applying a loadout.** The slots of a loadout use the same names as a creature's own
`Inventory_Cosmetic_Slot*` properties (`creatures.md` §4.2, `appearance.md` §5.5): applying a
loadout copies its properties onto the creature. The native constants equal to the PIDs of
`Inventory_Cosmetic_LoadoutSet`, `…_Loadout` and `…_AppliedLoadout` (BIN e) are user-interface
element IDs, not property reads (BIN n). In the class library's code `…_AppliedLoadout`
occurs only in one function of class 1199 here (which hands the PID to a native function
and reads the property), `…_Loadout` not at all, and `…_LoadoutSet` once, in a function that
otherwise reads loot properties (not read further; BC). No client code was found that picks
a loadout from the set.

> **OPEN (server):** How a loadout is chosen from `Inventory_Cosmetic_LoadoutSet` (random per
> spawn, by state, by script). No client code was found that chooses (above); by inference the server
> sets `Inventory_Cosmetic_AppliedLoadout` and the client applies it. Tessera picks one
> uniformly at spawn unless `Inventory_Cosmetic_AppliedLoadout` is set (Decision D6,
> §16.3).

---

<a id="s7"></a>
## 7. Cosmetic pets

<a id="s7-1"></a>
### 7.1 Pet records (Confirmed, I18)

563 records (class 3698 here), 6–7 properties:

| Property | Type | Records | Values |
|---|---|---|---|
| `CosmeticEntity_CosmeticEntity` | DID → creature template | 563 | 562 distinct; all mob templates (class 1723 here) |
| `CosmeticEntity_Name_Initial` | StringInfo | 563 | the pet's initial name |
| `CosmeticEntity_MovementType` | enum | 563 | 2 `Following` |
| `CosmeticEntity_Distance` | float, m | 563 | 3 (559), 4, 5 |
| `CosmeticEntity_Following_Speed` | float, m/s | 563 | 10 |
| `CosmeticEntity_Movement_ThresholdMappings` | array of `CosmeticEntity_Movement_Entry` (`CosmeticEntity_Movement_Threshold` float, m/s (§7.4), `CosmeticEntity_Movement_ThresholdState` enum) | 562 | three entries in descending order: 15 → `Super`, 10 → `Fast`, 6 → `Normal` in 561; 15, 7, 3 with the same states in one |
| `CosmeticEntity_TrickArray` | array of DIDs → class 3982 | 1 | 2 |
| `Item_Quality` | enum | 562 | `Common` |

Bundle: root attribute 0x0CF76368 → a `set_u32` (native 18) holding the trick DIDs (empty in
562).

<a id="s7-2"></a>
### 7.2 The pet directory (Confirmed)

WEENIECONTENT `CosmeticPetDirectory` names one record (class 4091 here) with
`CosmeticEntity_SkillList`: 563 `CosmeticEntity_SkillID` DIDs, all skills (class 827 here).
Bundle: root attribute 0x0548DFD3 → `array_u32` of the same DIDs in order.

<a id="s7-3"></a>
### 7.3 Chain from skill to creature (I19)

A cosmetic pet is summoned by a skill (LC: the skill's `Skill_Toggle_Effect_List` names a
toggle effect, Reported). The effect's `Mod_Array` holds a `Mod_Entry` with
`CosmeticEntity_CosmeticPet_ForwardSource` → a pet record (in effects of class 734 here: 562
`Mod_Entry` values naming 562 distinct pet records, Confirmed); 3 pet records are also named
by `AI_Pet_Version_NewPetDID`
(class 3984). The pet record's `CosmeticEntity_CosmeticEntity` names the creature template
that gives the pet's look and classification (Confirmed). One pet record is named by nothing.

<a id="s7-4"></a>
### 7.4 Following (BIN, I20)

The client moves a following pet itself (I20, I20a). Setup: when a pet entity appears, the
client reads its `CosmeticEntity_MovementType`: 2 `Following` uses the follow step below,
1 `Attached` uses another behaviour, which reads `CosmeticEntity_Attached_OwnerHoldingLocation`
(applied to the owner) and `CosmeticEntity_Attached_HoldingLocation` (applied to the pet) and
was not read further (no pet record of this install is `Attached`); other values get no
client movement. For a
following pet it reads `CosmeticEntity_Distance` *D* (0 when absent) and
`CosmeticEntity_Following_Speed` from the pet record; a speed that is absent or not above 0
becomes **5.0** m/s (the dictionary default of 20 is not used). The client keeps a trail of
the owner's recent positions (cell, landblock, local position and orientation) in a ring of
at most **40** entries.

**Trail (BIN r, I20b).** The trail belongs to the pet and is kept newest first:

1. **Trail point.** A point is built from the owner: its current position and its
   `Entity_Element` (0 when absent). When that element is neither 5 (`Earth_ControlledSurface`)
   nor 2 (`Water_NoSurface`), the point is put on the ground by the client's downward
   placement probe (limited to 50 m, the probe of step 7 of the follow step). When the probe
   succeeds, the point's element becomes 5 if it was 4 (`Earth_NoSurface`) or 6
   (`Earth_UncontrolledSurface`), and 1 (`Water_ControlledSurface`) otherwise; when it fails,
   the point keeps the owner's position and element.
2. **Start.** When the pet is registered (its entity appears), the trail holds one point built
   from the owner.
3. **Recording.** Each time the owner's position changes (by inference: the notification the
   client registers on the owner passes the owner and its new position), and the new position
   is valid: when the trail is not empty and the
   new position lies at most **0.01 m** (three-dimensional distance, across landblocks) from
   the newest point, nothing is recorded. Otherwise a point is built (step 1) and, only when
   its element is 1, 2 or 5, inserted as the **newest** point; when the trail already holds 40
   points, the **oldest** is dropped first.
4. **Reset.** On a second notification registered on the owner (not identified; by inference a
   teleport or similar jump), the client empties the trail, records one point built from the
   owner, and moves the pet to that point (by a placement function of the following
   behaviour that was not read in detail).

The ring's start index is the newest point, so step 3 of the follow step below tests the
points from newest to oldest.

**Follow step.** Each step has a step time Δt (seconds, from the caller) and a **pace** *v*:
the length of the owner's current velocity, or the pet's following speed when that length is
(near) zero. Sizes: for an entity, *ρ* is its setup radius × *s* and *η* its setup height ×
*s* (the *r*₀, *h*₀ and *s* of §3.4.1, without the 0.05 m and 0.1 m). In order:

1. When the owner has no valid position, nothing happens. When the pet has none, the pet takes
   the owner's position and orientation, and the step ends.
2. **Hold.** When the pet's `Entity_Element` is 1 (`Water_ControlledSurface`), 2
   (`Water_NoSurface`) or 5 (`Earth_ControlledSurface`), and the distance *d* between pet and
   owner satisfies (*ρ*_owner + *ρ*_pet) < *d* < *D*, the pet keeps its position, turns to
   face the owner, its `MoveToController_PerformingMoveTo` becomes false, and the step ends.
3. **Trail point.** Every stored trail point *P* is tested. With all three positions raised by
   half the pet's height (*η*_pet / 2), *P* passes when a segment query of the client's physics
   world (a line-of-sight test) reports no hit between the pet and *P* and none between *P*
   and the owner. For a
   passing point the score is | |*P* − owner|² − *D*² |; the point with the smallest score is
   chosen (scores of 999,999 and more never win; on a tie the point tested first stays). The
   points are tested in ring order from the ring's start index, which is the newest point
   (above), so on a tie the newest point wins.
4. When no point passes, the pet takes the owner's position and orientation, and the step
   ends. (This is what happens after the owner teleports or moves out of sight.)
5. **Move.** Let *w* be the vector from the pet to the chosen point. When |*w*| ≤ *v* × Δt the
   pet moves onto the point (taking the point's cell and landblock). Otherwise, when |*w*| is
   at least 0.0002 m, the pet moves *v* × Δt along *w* from its own position.
6. **Heading.** When the pet moved at most 0.02 m in this step, it turns to face the owner and
   `MoveToController_PerformingMoveTo` becomes false; otherwise it faces along its movement and
   the flag becomes true. Only the heading (rotation about the vertical) changes.
7. **Height.** At the new position the client probes for a liquid surface. When one is found
   and the pet's position lies more than *η*_pet / 2 below it, the pet is raised so that its
   middle sits at the surface (by the depth minus *η*_pet / 2) and its `Volume_IsLiquid`
   becomes true. Otherwise `Volume_IsLiquid` becomes false, the pet is placed on the highest
   surface hit by a downward probe that starts 0.8 m above the new position and reaches at
   most 50 m down (unchanged when the probe hits nothing), and is then raised by 0.05 m.
8. **Movement state.** The client walks `CosmeticEntity_Movement_ThresholdMappings` in stored
   order and takes the `CosmeticEntity_Movement_ThresholdState` of the **first** entry whose
   threshold is at most *v*, skipping entries whose threshold equals −1 (the dictionary
   default; within 0.0002) and entries whose state is not an enum value. When no entry
   matches, the state is 0, which the state mapper labels `Slow`. It sets the pet's
   `CosmeticEntity_MovementState` to the result. The thresholds are therefore paces in m/s:
   with the stored order of §7.1, `Super` from 15 m/s, `Fast` from 10 m/s (a pet whose owner
   stands still and whose speed is 10 is `Fast`), `Normal` from 6 m/s, `Slow` below. Steps 1,
   2 and 4 end the step before this, so the state changes only when the pet moves along the
   trail.

The client also reads `CosmeticEntity_Owner` and `CosmeticEntity_SourceWrapper`, which are not
stored in the data.

> **OPEN:** What the liquid probe of step 7 accepts as a liquid surface; which client event
> triggers the trail reset (step 4 of the trail); whether the follow step uses the element
> stored with each trail point; how each movement state maps to an animation (script tables,
> `script-table.md`). Tessera's rule for the reset: Decision D7 (§16.3).

---

<a id="s8"></a>
## 8. Defeat and rally points (Confirmed, I21)

<a id="s8-1"></a>
### 8.1 Defeat records

388 records (class 1035 here), bundle = root only (no attributes):

| Property | Type | Records | Values |
|---|---|---|---|
| `Death_RallyPoint` | string (place key) | 388 | 357 distinct; every one resolves in the place table (`map-data.md` §3, §10) |
| `Death_DefeatEffectProgression` | DID → progression (class 1389 here) | 388 | two records: one (named by 376) whose 160 elements are all null, one (named by 12) whose 160 elements all name the same effect |
| `WorldEvent_AllConditionList` | array of `WorldEvent_Condition` (`WorldEvent_WorldEvent` → class 1815, `WorldEvent_Operator`, `WorldEvent_ConditionValue`) | 4 | world-event conditions (when they apply is OPEN, §8.3) |

The progression gives `Death_DefeatEffect` by the defeated character's level
(`combat-data.md` §4; minimum index 1, 160 elements).

<a id="s8-2"></a>
### 8.2 Who names a defeat record

Only 27 of the 388 are named in the data:

- `Death_DefeatInfo` in **landblock property sets**: 60 landblocks (regions 1, 3 and 4);
  59 name 12 distinct defeat records, one names an area record (class 690) instead (data
  error; ignore it);
- `PrivateEncounterTemplate_DefeatInfoOverride` (4 distinct, 26 values);
- `Death_DefeatInfo_Override` in effects' `Mod_Array` entries (13 values, 12 distinct).

The other 361 defeat records are named by nothing in the data; how they are reached (for
example per area) is not in the data.

Related records: two class-692 records (area-like) hold `Death_PlayerDefeatArray` and
`Death_MonsterPlayerDefeatArray`, arrays of `Death_DefeatInfoPropertyName` naming the PIDs
`Death_Flippable_PlayerDefeatInfo_1`…`_3` and `Death_Flippable_MPDefeatInfo_1`…`_3`; one
landblock stores `Death_Flippable_MPDefeatInfo_1` = 0.

<a id="s8-3"></a>
### 8.3 Other defeat data

| Data | Where | Content |
|---|---|---|
| `Death_CorpseMobile` | mob templates (20,161), class 4026 | true in all |
| `Death_NeverSayDie` | mobs (true in 5), player-like templates, class 4026 | |
| `Death_CorpseIsUsable` | 96 mobs | true 95 |
| `Death_SelfDefeat_AnnounceOverride` | 58 mobs | StringInfo |
| `Death_ImmuneToDefeat`, `Death_SelfReviveBlocked` | `Mod_Array` entries of effects | true |
| WEENIECONTENT `FallingInjuryEffect` | an effect (class 734) | a harmful debuff that sets `Combat_CanEvade`, `…CanParry`, `…CanBlock` false and multiplies `Mount_Movement_Max_Speed` |
| WEENIECONTENT `DefeatBlockSelfReviveEffect` | an effect (class 2477 below 734) | a 1,800 s real-time debuff (equivalence class `BlockSelfRevive`), duration modified by `Effect_Self_Revive_Cooldown` |
| WEENIECONTENT `SelfReviveToken` | an item | stackable (max 50), destroyed on use, applies one usage effect |

The dictionary defines 51 `Death_*` names; 15 are stored. The rest (`Death_ReviveData`,
`Death_RevivePosition`, `Death_VitalPercentageData`, `Death_LastKillerIID`, …) are run-time
state.

> **OPEN (server):** The rally-point choice (nearest by place, by area, or as named by the
> area's defeat record), when the world-event conditions of the 4 conditional records apply,
> the revive vitals and the use of `Death_DefeatEffectProgression` (which does not occur as a
> literal in the class library's code, BC). Server rules; Tessera's sandbox follows Decision
> D8 (§16.4).

---

<a id="s9"></a>
## 9. Multi-user activity stations (Confirmed, I22)

<a id="s9-1"></a>
### 9.1 Activity collections

39 records (class 3789 here), named by `MultiUsage_ActivityCollection` on 143 mob templates
(29 distinct collections), 45 interactive objects of class 2097 (11) and 3 hotspots (1).
Property set:

| Property | Type | Records | Content |
|---|---|---|---|
| `MultiUsage_ActivityPatchBay` | array of `MultiUsage_ActivityGroup` | 39 | 126 groups; each: `MultiUsage_ActivitySocket_Array` (198 sockets: `MultiUsage_Activity` DID → activity, null in 9; `MultiUsage_ShowDependencyToRight` bool, 11), `MultiUsage_Group_IsSubmenu` (21, true 15), `MultiUsage_Group_IsRadio` (3, true), `MultiUsage_Group_Label` (StringInfo, 15), `MultiUsage_Group_IconMapping_Array` (15: `MultiUsage_Activity_IconUIState` → `MultiUsage_Activity_Icon` 0x41), `MultiUsage_ShowDependencyToRight` (6) |
| `MultiUsage_Mouseover` | enum (mouse-over type) | 39 | `Use` |
| `MultiUsage_ColumnCount` | int | 9 | 3 or 4 |
| `MultiUsage_TutorialHint` | enum | 9 | |

<a id="s9-2"></a>
### 9.2 Collection bundle

Root attributes: 0x004F39B3 → `refarray` of class-3795 group objects in patch-bay order;
0x01310460 → `refmap_u32` from **every** non-null socket's activity DID to its group object
(the first group when an activity appears twice). A group object has 0x0A2B74D3 → `refarray`
of class-3794 socket objects (attribute 0x0A0A48B9 = the activity DID, 0 for an empty
socket), 0x0D8E4CD5 = `MultiUsage_Group_IsSubmenu` and 0x008F4F8F = `MultiUsage_Group_IsRadio`
(0 or 1). Checked in all 39.

<a id="s9-3"></a>
### 9.3 Activities

81 records in three classes below 3790: 26 (class 3793), 48 (3811), 7 (3838); all named by
collections. Each imports and references the usage-permission record `UsagePermission_Default`
(a DID item). Property set (all three classes unless noted):

| Property | Type | Content |
|---|---|---|
| `MultiUsage_Activity_Name`, `…_Desc` | StringInfo | |
| `MultiUsage_Activity_Icon`, `…_IconMapping_Array` | 0x41 DID; 8 UI-state → icon pairs | |
| `MultiUsage_ActivityPolicy` | enum | 1 `Toggle` (9 in 3793), 2 `Instant` |
| `MultiUsage_Activity_MaxRange`, `…MaxRangeAdditiveModifierProperty` | float m (5, 8 or 15), PID | |
| `MultiUsage_HostPerms`, `MultiUsage_UserPerms` | struct `MultiUsage_PermissionBlob`: `Usage_RequiredEffect`, `Usage_RestrictedEffect`, `Usage_RequiredTrait`, and (host) `Usage_PropertyCondition_Array` of `PropertyCondition` (`PropertyCondition_FirstOperand` PID, `…_Operator`, a value under the operand's own PID, `…_ErrorType`, `…_Description`) | requirements on the station (host) and on the user |
| `MultiUsage_HostCooldownData`, `MultiUsage_UserCooldownData` | struct: `MultiUsage_CooldownChannel` enum, `MultiUsage_CooldownTime` float s, `MultiUsage_CooldownMod_Array` of PIDs | |
| `MultiUsage_AddEffect_EffectDID` | DID → effect | 3793 only |
| `MultiUsage_Activation_HostScript`, `…_WaitForHostScript` | enum, bool | |
| `MultiUsage_IsAutoRepeating`, `MultiUsage_Activity_ConsumeRequiredEffect_User` | bool | the first in 9 records of 3811 and 5 of 3838; the second in one record of 3811 |
| `DistributedCharging_MaxUsers` (1 or 3), `…_RequiredWorkUnits` (100–2,400), `…_UserRateMultiplierProperty`, `…_HostWorkUnitMultiplierProperty` (PIDs), `…_UserScript`, `…_InterruptedByMovement` (true) | | shared work: up to *MaxUsers* users add work units until the required amount is reached |
| `Mod_Array` (3811), `Clamp_Array` (7 records of 3811), `MultiUsage_VitalMod_ModArray` (3838: `…_VitalType` `Health`, `…_Value`, `…_ValueIsPercent` true) | | what the activity changes on the station |
| `UI_Examination_Tooltip_*` | | |

The property conditions and modifiers name `BigBattle_Interactive_*` and
`BigBattle_Commander_*` properties: by their names, these are the siege engines and command
stations of the big-battle content.

The bundles (9–25 items) mirror these structures (permission blobs as class-1602 objects with
native property sets, cooldown data as class-3841 objects, distributed charging as class
3825, modifiers as class-1339 objects with native property entries, the icon mapping as a
`map_u32` of UI state → icon DID bits, clamps as class 3812/3813). Readers use the property
set.

> **OPEN (server):** How work units accrue (rate per user per second, what the multiplier
> properties read), and how a toggle activity ends. `DistributedCharging_RequiredWorkUnits`
> does not occur as a literal in the class library's code (BC). Server rules.

---

<a id="s10"></a>
## 10. Plaques and level-up actions (Confirmed, I23)

<a id="s10-1"></a>
### 10.1 Plaques

30 usage-action records (class 3904 here), each named by one item. Property set:
`Mouseover_Type` = `Use` and `PlaqueDisplay_ServerInfo_Array`, 29 structs of
`PlaqueDisplay_Server` (an enum of world identifiers, 29 keys) and
`PlaqueDisplay_ServerString` (StringInfo): one text per world. Bundle: root attribute
0x002D0013 → `refmap_u32` keyed by `PlaqueDisplay_Server` → `string_info` objects (native 199)
equal to the texts. A reader shows the text for the world the player is on.

<a id="s10-2"></a>
### 10.2 Level-up actions

8 usage-action records (class 3859 here), named by 9 items:
`Usage_LevelupUsageAction_LevelToAdvance` (int: 50, 95, 105, 120, 130, 140, 150, 160),
`Mouseover_Type` = `Use`, and in 7 `Quest_QuestsToComplete` (2–4 quest DIDs; 21 in all). Bundle: root
attribute 0x0B8913D5 → `array_u32` of the quest DIDs (null in the record without quests).

---

<a id="s11"></a>
## 11. Target markings (Confirmed, I23)

WEENIECONTENT `TargetMarkControl` names one record (class 1844 here) with
`TargetMarking_ControlData_Array`: 10 DIDs of mark records (class 1845), mirrored in order by
an `array_u32` (root attribute 0x07A1414C). Each mark record has `TargetMarking_Name`
(StringInfo), `TargetMarking_Icon_DataID` (0x41) and `TargetMarking_ScriptID` (enum: one
"target mark script" key per mark). Players put these marks over targets; the icon and
script PIDs occur as constants in the client code (BIN e).

---

<a id="s12"></a>
## 12. Player trigger responses (Confirmed, I23)

WEENIECONTENT `PlayerTriggerResponseSet` names one record (class 3035 here) with
`TriggerResponse_Set`: 38 `TriggerResponse_Struct` entries, each with
`TriggerResponse_TriggerType` = 1 `PropertyChange`, `TriggerResponse_TriggerConfig`
(`Advancement_Level` = 15, 30, 39, 45, 58 or 65), an `EntityFilter_Array` of
`EntityFilter_Class` (character classes; absent in one entry), `TriggerResponse_ResponseType` =
1 `ApplyEffects` and `TriggerResponse_ResponseConfig` with one effect
(`TriggerResponse_ApplyEffects_Effect` → class 2080). Read by its labels: when a player of
one of the listed classes reaches the level, apply the effect. The bundle holds the same
structure as class-3034, 3033, 3039, 3040 and 3042 objects (values not compared).

---

<a id="s13"></a>
## 13. WEENIECONTENT labels (Confirmed, I24)

Read the labels at run time (`property-system.md` §8.1). Observed targets:

| Label | Key | Target | § |
|---|---|---|---|
| `ActivatorActionHandler`, `AdminActionHandler`, `ChestActionHandler`, `ConsumableActionHandler`, `DefaultActionHandler`, `DoorActionHandler`, `EquippableActionHandler`, `ExamineActionHandler`, `MonsterActionHandler`, `NPCActionHandler`, `PlayerActionHandler`, `TakeableActionHandler`, `TextItemActionHandler` | various | one handler each | §3.5 |
| `CorpseActionHandler`, `CSM_CorpseActionHandler` | 0x1000002D, 0x10000013 | the same handler record | §3.5 |
| `ContextMenuSystemGlobals` | 0x2 | context-menu globals | §3.5 |
| `UsageAction_DoNothing`, `UsageAction_Container`, `UsageAction_QuestNPC`, `UsageAction_Corpse`, `UsageAction_DecorationItem`, `UsageAction_PEObjective` | 0x10000033, 0x1000005E, 0x10000042, 0x10000174, 0x100000C1, 0x10000150 | usage-action records | §3.2 |
| `UsagePermission_Default`, `UsagePermission_AlwaysAllow`, `UsagePermission_HousingDecorationItem`, `TakePermission_CantTake` | | permission records | §3.6 |
| `UsagePluginDirectory` | 0x10000131 | plugin directory | §3.7 |
| `MouseoverDirectory` | 0x1000007E | mouse-over directory | §3.3 |
| `PickupInductionAction`, `TravelInductionAction`, `LayerTransferInductionAction` | 0x10000023, 0x1000000D, 0x10000124 | induction records | §3.8.3 |
| `TargetMarkControl` | 0x100000A0 | target-mark control | §11 |
| `CosmeticPetDirectory` | 0x10000197 | pet directory | §7.2 |
| `PlayerTriggerResponseSet` | 0x10000120 | trigger-response set | §12 |
| `FallingInjuryEffect`, `DefeatBlockSelfReviveEffect`, `SelfReviveToken` | 0x1000007C, 0x100000EE, 0x10000128 | effects, item | §8.3 |
| `Dialog_UsageConfirmation`, `Dialog_IATargetUsageConfirmation`, `Dialog_AbandonChanneling` | | dialog records (class below 1728, empty sets) | — |
| `PetChannelControl` | 0x10000008 | one record (class 1680) | — |

---

<a id="s14"></a>
## 14. Dictionary families (Confirmed, I25)

"Stored" = the name has a value in at least one 0x79, 0x78, landblock, static-entity,
instance or link property set. "Code" = number of names whose PID occurs as a 32-bit literal
in the class library's bytecode chunk (OBS; small PIDs below 0x10000000 excluded because
their byte patterns occur by chance).

| Prefix | Defined | Stored | Code | Where stored |
|---|---|---|---|---|
| `Usage` | 181 | 142 | 148 | items, templates, instance sets, plugin and activity records |
| `CSM` | 70 | 60 | 51 | handler records, context-menu globals; `CSM_ActionHandler` on items and templates |
| `Channeling` | 56 | 51 | 14 | channelling records |
| `Hotspot` / `HotSpot` | 51 / 3 | 10 / 2 | 12 / 1 | hotspots, decoration items, instance sets, creature templates (`Hotspot_OverrideMountedCombat_*`) |
| `Death` | 51 | 15 | 16 | §8 |
| `MultiUsage` | 44 | 42 | 9 | §9 |
| `Induction` | 37 | 30 | 9 | induction records; `Induction_ActionDID` everywhere |
| `Inventory_Cosmetic` | 36 | 23 | 30 (all `Inventory`) | loadouts, creature templates |
| `Interaction` | 36 | 1 | 36 | 9 instance sets; the rest is run-time state (`Interaction_Permission_*` are the members of `Hotspot_InteractionPerms`) |
| `CosmeticEntity` | 22 | 15 | 6 | §7 |
| `Tripwire` | 20 | 1 | 2 | instance sets (`Tripwire_Billboard_Text`) |
| `Volume` | 20 | 13 | — | 0x78 sets and private-encounter templates (liquid and volume settings; not part of this spec) |
| `RiddleDoor` | 17 | 0 | 0 | nowhere |
| `Mouseover` | 15 | 9 | 13 | §3.2, §3.3 |
| `Relay` | 13 | 0 | 0 | nowhere (available-list only) |
| `DistributedCharging` | 12 | 6 | 1 | §9.3 |
| `TriggerResponse` | 18 | 8 | 5 | §12 |
| `PatrolBox` | 6 | 5 | 0 | §5 |
| `UsagePlugin` / `UsagePluginDirectory` | 5 / 2 | 5 / 2 | 5 / 0 | §3.7 |
| `TargetMarking` | 5 | 5 | 3 | §11 |
| `PlaqueDisplay` | 4 | 4 | 0 | §10.1 |

In the native client code (BIN e), constants equal to these PIDs occur for 15 of the 22
`CosmeticEntity` names (§7.4), the induction-bar names of §3.8.4, `Usage_MinUsageDist`,
`Usage_UsageExpireDist`, `Mouseover_CursorOutOfRange`, `Mouseover_Range`,
`Mouseover_MouseoverList`, the `MultiUsage` patch-bay and icon names, the `TargetMarking`
icon and script names, `UsagePluginDirectory_UsagePluginArray` and many `Usage_*`
requirement names; **not** for `Hotspot_IsHotspot`, `Hotspot_EnableVisualization`,
`Hotspot_Box_*`, `SeeMeVisibilityMask` or `IsUsable`. (A constant equal to a PID in the
0x1000xxxx range is not proof of a property read: the same range holds user-interface
element IDs. Only the routines named in §3.4, §3.8.4 and §7.4 were read. Revision 3 read the
routines behind the hits for `Mouseover_CursorOutOfRange`, `Mouseover_MouseoverList`,
`Inventory_Cosmetic_LoadoutSet`, `…_Loadout` and `…_AppliedLoadout`: every one is a
user-interface element lookup, so none of these five is read as a property by native code;
`Mouseover_Range` is (§3.4.2).)

---

<a id="s15"></a>
## 15. Reference graph (Confirmed)

| From | Property | To |
|---|---|---|
| any object, template | `Usage_Action` | usage action (§3.2) |
| any object | `Usage_Permission`, `Usage_TakePermission` | permission records (§3.6) |
| any object | `CSM_ActionHandler` | handler (§3.5) |
| context-menu globals | `CSM_ActionHandler`, `CSM_PlayerActionHandler` | handlers |
| objects, templates, skills, recipes, private encounters, quests | `Induction_ActionDID`, `Skill_InductionAction`, … | induction (§3.8) |
| induction | `Induction_SetbackProgression` | progression (`combat-data.md` §4) |
| induction | `Induction_UserEffectList` | effects |
| items, objects, skills, effects, quests, instance sets | `…ChannelingState` properties | channelling (§3.9) |
| channelling | `Channeling_AdditionalEffectDID`, `Channeling_Player_Cosmetic_Slot_Entity` | effects; 0x47 |
| plugin directory | `UsagePluginDirectory_UsagePluginArray` | plugins (§3.7) |
| mouse-over directory | `Mouseover_Cursor`, `…OutOfRange` | 0x41 images |
| summoning effects, generator profiles | `Effect_Genesis_SummonedObject`, `GeneratorProfile_WSLEntity` | hotspots (§4.6) |
| hotspots | `EffectGenerator_HotspotEffectList` | effects |
| hotspots, mob templates, objects | `MultiUsage_ActivityCollection` | collections (§9) |
| collections | `MultiUsage_Activity` | activities |
| activities | (DID item) | `UsagePermission_Default` |
| mob templates, classes 1932, 763 | `Inventory_Cosmetic_LoadoutSet`, `…AppliedLoadout` | loadouts (§6) |
| loadouts, channelling | slot entities | 0x47 |
| pet directory | `CosmeticEntity_SkillList` | skills |
| effects (`Mod_Array`), pet-version records | `CosmeticEntity_CosmeticPet_ForwardSource`, `AI_Pet_Version_NewPetDID` | pets (§7) |
| pets | `CosmeticEntity_CosmeticEntity` | mob templates |
| landblocks, private-encounter templates, effects | `Death_DefeatInfo`, `…DefeatInfoOverride`, `Death_DefeatInfo_Override` | defeat records (§8) |
| defeat records | `Death_DefeatEffectProgression` | progression |
| target-mark control | `TargetMarking_ControlData_Array` | marks (§11) |
| trigger-response set | `TriggerResponse_ApplyEffects_Effect` | effects |

---

<a id="s16"></a>
## 16. Tessera rules where the data stop

<a id="s16-1"></a>
### 16.1 Use (sandbox server)

> **Decision D2 (dispatch):** A use request names an entity. The server resolves the entity's
> effective `Usage_Action` (instance set over template) and dispatches by the action
> **record** (DID, or WEENIECONTENT label where one exists) to Tessera's own behaviour table;
> unknown actions do nothing. `items.md` §5.10 states the same for items.

> **Decision D3 (permission):** `UsagePermission_AlwaysAllow` allows everyone;
> `TakePermission_CantTake` forbids taking; every other permission record allows the use when
> the requirements of D4 pass.

> **Decision D4 (requirements):** For each requirement property on the entity (its effective
> value, instance set over template, then each member of its `DefaultPermissionBlobStruct`),
> the server looks the PID up in the plugin directory (§3.7.2). Tessera has rules for
> **exactly** the requirements listed below. Every other requirement property (the other 63 of
> the 77 here) passes and is logged once per PID per server run. A failing requirement stops
> the use and reports the plugin's `UsagePlugin_ErrorType` (a generic failure when the plugin
> has none). "The user's value" of a property is the user's effective value when the request
> arrives, or the dictionary default when the user has none.
>
> 1. **Comparison plugins** (every plugin with `UsagePlugin_TargetPropertyName`; nine here). A
>    requirement equal to its null value (0, a null DID, false, an empty list, an empty bit
>    mask) passes. Otherwise it is compared with the user's value of the plugin's target
>    property: with the plugin's `Usage_Operator` when it has one (`EqualTo`, `NotEqualTo`,
>    `GreaterThanOrEqualTo` meaning the user's value is at least the requirement; any other
>    operator passes and is logged); without an operator, by the requirement's type: a list
>    passes when it contains the user's value, a bit mask when it shares at least one bit with
>    the user's value, a bool (true) when the user's value is absent, 0 or null, any other
>    value when the two are equal.
> 2. `Usage_MinLevel` (int): the user's `Advancement_Level` is at least the requirement.
> 3. `Usage_MaxLevel` (int): the requirement is below 0, or the user's `Advancement_Level` is
>    at most the requirement.
> 4. `Usage_AllowedInCombat` (bool): the requirement is true, or the user is not in combat
>    under Tessera's combat rules.
> 5. `Usage_RequiredEffect` (DID): 0, or the user has an active effect of that DID.
> 6. `Usage_RestrictedEffect` (DID): 0, or the user has no active effect of that DID.
>
> With this install's plugins, rule 1 gives: `Usage_RequiredChannelingState` and
> `Usage_BarringChannelingState` equal / not equal to the user's
> `Channeling_CurrentChannelingDID`; `Usage_MinMountLevel` at most the user's `Mount_Level`;
> `Usage_RequiredClassList` and `Usage_RequiredRaces` contain the user's `Agent_Class` and
> `Agent_Species`; `Usage_RequiredGenus` and `Usage_RequiredCraftTool` share a bit with the
> user's `Agent_Genus` and `Inventory_CraftToolCategory`; `Usage_BarShapeshiftedCharacters`
> and `Usage_BarrAdvancedStartCharacters`, when true, need the user's `Shapeshift_MeshIID` and
> `Character_LeveupUsageAction_Tier` absent or 0.

Then: range by §3.4.1 (the server applies the same formula with the client's constants); when
the entity has `Induction_ActionDID`, the induction of D9–D11; on completion, apply
`EffectGenerator_UsageEffectList` (and `Induction_UserEffectList`), enter
`Usage_AppliedChannelingState` if set (D12), and run the action of D2.

> **Decision D9 (induction duration):** When an induction starts, the server computes its
> duration *T* once (the arithmetic is OPEN, §3.8.5):
>
> 1. *T*₀ is the record's `Induction_Duration`, in seconds.
> 2. *A* is the sum, over the PIDs of `Induction_Duration_AdditiveModifierList`, of the user's
>    value of each (a PID listed twice counts twice).
> 3. *M* is 1 plus the sum, over the PIDs of `Induction_Duration_MultiplierModifierList`, of
>    the user's value of each. A PID in both lists counts in both.
> 4. *T* = max(0, *T*₀ + *A*) × max(0, *M*).
> 5. `Induction_Duration_AreaChallengeLevelOverride` is ignored.
>
> The server sets the user's `Skill_InductionAction` to the record,
> `Skill_InductionAction_StartTime` to the current time and `Skill_InductionAction_EndTime` to
> the start plus *T*; the client shows them as §3.8.4 describes. Later changes to the user's
> properties do not change a running induction; only setbacks (D10) do. *T* = 0 completes at
> the next server step.

> **Decision D10 (induction setbacks):** While an induction runs, every combat event of
> Tessera's combat rules whose subject is the user is matched against the record's
> `Induction_SetbackEventsArray` by `Induction_EventID` (event 0 and unlisted events do
> nothing; the first matching structure is used). For a match:
>
> 1. If the record's `Induction_SetbackIgnoreProp` is non-zero and the user's value of that
>    property is true (or non-zero), nothing happens.
> 2. Otherwise the induction's setback count *n* (0 at the start) goes up by one. The server
>    may expose it as `Induction_CurrentSetbackCount`.
> 3. The amount *s* is the structure's `Induction_SetbackTime`, or, for a structure with
>    `Induction_SetbackProgression`, that progression evaluated at index *n* (`combat-data.md`
>    §4 with its out-of-range rule). *s* is in seconds. When *s* is not above 0 or the
>    progression cannot be evaluated, nothing more happens.
> 4. Policy `ModifyStartTime` (also used for any other value or no policy): the start time
>    moves later by *s*, but not past the current time; the end time becomes the new start
>    plus *T*. The elapsed time shrinks by *s*, not below 0.
> 5. Policy `ModifyDuration`: *T* grows by *s*; the start stays; the end moves later by *s*.
> 6. The server sends the new start and end times.
>
> `Induction_Interruptable` does not gate setbacks (D11 covers interrupts).

> **Decision D11 (induction end, interruption, restart):** An induction **completes** when
> the server time reaches its end time first; the server then runs the completion steps above
> and shows `Induction_CompletionString`. It **fails** (shows `Induction_FailureString`, plays
> `Induction_FailureFXID`, applies nothing and runs no action) when, before that:
>
> 1. `Induction_AllowedWhileMoving` is not true and the user's position is more than 0.1 m
>    from where the induction started (a jump counts);
> 2. `Induction_AllowedInCombat` is false and the user is in combat: checked when the request
>    arrives (the request is then refused) and whenever the user enters combat;
> 3. `Induction_Interruptable` is true and Tessera's combat rules deliver an interrupt to the
>    user (when it is false, interrupts are ignored);
> 4. the user is defeated, teleports, changes region or instance, or disconnects;
> 5. the use target no longer exists or is out of range (§3.4.1, checked every server step);
> 6. the user cancels it;
> 7. the user starts another induction and the running record's `Induction_FailOnRestart` is
>    true; the new one then starts. When it is false or absent, the new request is refused
>    and the running induction continues.

> **Decision D12 (channelling states):** (the rules are OPEN, §3.9.3)
>
> 1. **Entry.** A user enters a channelling state through `Usage_AppliedChannelingState` on
>    use completion, an `Effect_EnterChannelingState_StateToEnter` effect, or a toggle skill's
>    `Skill_Toggle_ChannelingState`. The server sets the user's
>    `Channeling_CurrentChannelingDID` to the record. The duration *D* is
>    `Channeling_Duration` (for a toggle skill, its `Skill_Toggle_ChannelingDurationOverride`
>    instead when that is above 0), multiplied by the user's value of
>    `Channeling_Duration_Multiplier_PropertyName` when that PID is non-zero and the value is
>    above 0. Start and end times are sent as for an induction (§3.8.4). A user is in at most
>    one state; entering another ends the current one as abandoned, also when its
>    `Channeling_CanBeAbandoned` is false (that flag only governs the user's own cancel
>    request, step 6).
>    `Channeling_AdditionalEffectDID` (at `Channeling_AdditionalEffectSpellcraft` when given)
>    is applied on entry and removed on exit.
> 2. **Success.** At the end time the state ends as a success (`Channeling_Success_Script`,
>    `Channeling_CompletionString`). An `Effect_ExitChannelingState_StateToExit` effect naming
>    the current state ends it the same way. Durations of 9,999 s and more are not special.
> 3. **Setbacks.** Events are matched as in D10, with `Channeling_EventID` and
>    `Channeling_SetbackIgnoreProp`; a match moves the end time earlier by
>    `Channeling_SetbackTime` seconds. When the new end time is not after the current time,
>    the state ends as a failure (step 5).
> 4. **Barred actions.** While the state lasts, the server refuses, without ending the state,
>    user requests in each category whose bit is set in `Channeling_BarredActions`: `Emotes`
>    (emote commands), `Combat` (skill use and auto-attack), `Usage` (use requests of this
>    section, including the inductions they start), `Crafting` (crafting requests). Other bits
>    are ignored.
> 5. **Failure.** The state ends as a failure (`Channeling_Failure_Script`,
>    `Channeling_FailureString`) after a setback per step 3, or when
>    `Channeling_InterruptedByMovement` is true and the user is more than 0.1 m from where the
>    state began, `Channeling_InterruptedBySwimming` is true and the user starts swimming,
>    `Channeling_InterruptedByTeleport` is true and the user teleports or changes region or
>    instance, `Channeling_Interruptable` is true and the combat rules deliver an interrupt,
>    or (always) the user is defeated.
> 6. **Abandon.** A cancel request from the user ends the state as abandoned
>    (`Channeling_Abandoned_Script`, `Channeling_AbandonString`) when
>    `Channeling_CanBeAbandoned` is true, and is refused otherwise.
>
> On every exit the server clears `Channeling_CurrentChannelingDID` and sends the change.

<a id="s16-2"></a>
### 16.2 Visibility

> **Decision D5:** The Tessera client does not draw, and does not let the player select, an
> entity whose effective `SeeMeVisibilityMask` is `Hidden`; it draws `Obvious` and unset ones.
> The explorer shows hidden entities with a distinct marker.

<a id="s16-3"></a>
### 16.3 Loadouts and pets

> **Decision D6:** At spawn, a creature with `Inventory_Cosmetic_AppliedLoadout` takes that
> loadout; otherwise one of `Inventory_Cosmetic_LoadoutSet` chosen uniformly; the chosen
> loadout's properties overlay the creature's slot properties.

> **Decision D7:** A following pet runs the follow step of §7.4 and records its trail as
> §7.4 "Trail" describes (the client's rules; revision 4 replaced the earlier Tessera rule of
> a 0.5 m spacing tested oldest first). Tessera resets the trail (step 4) when the owner
> teleports, changes region or instance, or is placed by the server without moving through
> the world.

<a id="s16-4"></a>
### 16.4 Defeat

> **Decision D8:** When a player is defeated, the sandbox looks for a defeat record: the
> landblock's `Death_DefeatInfo` (if it names a defeat record), else a private-encounter or
> effect override, else none. With a record, it offers the record's `Death_RallyPoint`
> (resolved through the place table) as the revive location and applies the effect that the
> record's progression gives for the player's level, if any. Without one, it revives at the
> player's bound home location. Revive restores the vitals Tessera's combat rules choose.

---

<a id="s17"></a>
## 17. Explorer views (Decision)

- **Hotspot card:** name, description, `WeenieType`, visibility mask, selectable flag, the
  effect list (effect name, spellcraft, data-list entries), box extents, usage action and
  permission (by label), map-note data, the available-property list, and the routes of §4.6
  that name the template.
- **Channelling and induction cards:** the tables of §3.8.1 and §3.9.1 with labels, setback
  events, and the records that name the state.
- **Hotspot overlay**, next to the creature spawn overlay (`creatures.md` §10): markers at the
  positions of map-note and quest-marker rows that name class-1170 entities (`map-data.md`),
  and at generator positions whose profiles name hotspots; hidden hotspots drawn hollow.
- **Use-pipeline panel** for any object: the chain of §3.1 resolved to records and labels.

---

<a id="s18"></a>
## 18. Invariants (all Confirmed on this install unless marked)

| # | Invariant |
|---|---|
| IN1 | Every `Usage_Action` value names a record whose root class descends from one class (the usage-action root); that class has no records of its own. |
| IN2 | Every usage-action record's property set has at most four properties; `Mouseover_Type`, when present, is 0 (one record) or a key of the mouse-over directory's list. |
| IN3 | The mouse-over directory's list has no repeated `Mouseover_Type`, and its bundle map has exactly the list's keys. |
| IN4 | The plugin directory's array and its bundle refarray list the same DIDs in the same order; its bundle map's keys are exactly the plugins' `UsagePlugin_UsagePropertyName` values, each unique. |
| IN5 | Every plugin record has exactly one `UsagePlugin_UsagePropertyName`; plugin classes have no attributes. |
| IN6 | In every hotspot template with `EffectGenerator_HotspotEffectList` of *n* elements, the class-701 object's element map has keys 3 … *n* + 2, its multimap has *n* entries with key 64 and those values, and attribute 0x07BB0CF2 = *n* + 2; element *i* carries the *i*-th effect DID and spellcraft. Without the list, root attribute 0x04343517 is null and the bundle has one item. |
| IN7 | Every induction record's six root attributes equal the mirrors of §3.8.2; every channelling record's two equal §3.9.2. |
| IN8 | Every hotspot template has `WeenieType` `Hotspot`, `Hotspot_IsHotspot` true, `Entity_CreationType` 8, `Usage_Action` `UsageAction_DoNothing` and an available-property list. |
| IN9 | Every defeat record has `Death_RallyPoint` and `Death_DefeatEffectProgression`; every rally point resolves in the place table. |
| IN10 | Every activity-collection map key is the activity DID of a non-empty socket, and every non-empty socket's activity is a key. |
| IN11 | Every loadout is named by at least one `Inventory_Cosmetic_LoadoutSet` or `Inventory_Cosmetic_AppliedLoadout`; every loadout has `Inventory_Cosmetic_Slot1_Entity` and `…_HoldingLocation`. |
| IN12 | Every pet record's `CosmeticEntity_CosmeticEntity` names a mob template; its threshold mappings, when present, are stored with descending thresholds. |
| IN13 | No property set stores `Relay_*`, `RiddleDoor_*`, `Usage_UsageExpireDist`, `Mouseover_Range` or `Hotspot_Policy`. |
| IN14 | (BIN) The use-range of a target with `Usage_MinUsageDist` = *m* and no `Usage_UsageExpireDist` is *m* + 3.0 m of gap distance. |
| IN15 | Every `Physics_GameRadiusOverride` in a 0x79 set is a top-level float above 0 in a mob template's set; when the same set stores `Render_AdjustableScale`, its three components are equal. |
| IN16 | Every `Induction_SetbackStructure` holds `Induction_EventID` and exactly one of `Induction_SetbackTime` and `Induction_SetbackProgression`; within one record all structures hold the same one of the two; no `Induction_EventID` or `Channeling_EventID` repeats within a record. |
| IN17 | Every `Induction_Duration_AreaChallengeLevelOverride` value is a float; every PID in an induction modifier list names a float property with dictionary default 0; every non-zero `Induction_SetbackIgnoreProp` and `Channeling_SetbackIgnoreProp` names a bool property with default false. |
| IN18 | (BIN) In §19.2's synthetic setting, a target with `Physics_GameRadiusOverride` = 2.0 and `Render_AdjustableScale` (1.5, 1.5, 1.5) has cylinder radius 3.0 whatever its base radius. |
| IN19 | Every hotspot template's `PhysObj` resolves through its 0x47 chain to a setup with at least one mesh part. |
| IN20 | The player action handler's bundle map has exactly the `_GOP` PIDs of `CSM_Player_UIToggleOptions` as keys; each class-3427 value carries that struct's required type and ripcord DID (0 when absent) and a `string_info` equal to its string. |
| IN21 | (BIN) An entity whose setup has `physics_bounds` radius 0.5 and height 1.8, with `Physics_AdjustableScale` (2, 2, 2) and no `Model_Scale` or radius override, has a cylinder of radius 1.05 m and height 3.7 m (§19.2). |
| IN22 | (BIN) With threshold mappings [(−1 → `Super`), (6 → `Normal`)] and pace 20 m/s the movement state is `Normal`; with pace 4 m/s it is 0 (`Slow`). |
| IN23 | No property set in the install stores `Usage_MinUsageDist_ClientOverride`, `Mouseover_Entity`, `Mouseover_State` or `Mouseover_ActiveScriptID`; `Mouseover_Script` is stored only as a member of the mouse-over directory's structs (39 values, §3.3). |
| IN24 | (BC) After hovering a synthetic entity with a non-zero mouse-over type, the local player's `Mouseover_Range` is the entity's `Usage_MinUsageDist_ClientOverride` when stored, else its `Usage_MinUsageDist`, else 2.0 (§19.5). |
| IN25 | (BC) A synthetic directory struct for type *T* with `Mouseover_Cursor` *A* and no `Mouseover_CursorOutOfRange` gives cursor *A* with the flag set and with it clear, and hotspot (0, 0) when the hotspot members are absent. |
| IN26 | (BC) The NPC service rule of §3.4.5 gives: `Vendor_InventoryList` alone → 18; `Vendor_InventoryList` and `NPC_Is_Banker` → 10; `NPC_Is_Neighbor` = true alone → 10; `Combat_Violent` = true with `Vendor_InventoryList` → 0; nothing → 0 (with quest tests failing). |
| IN27 | (BIN) A trail fed owner positions 0.005 m apart records no new point; positions 1 m apart on ground (`Entity_Element` 5) are recorded newest first, and the 41st drops the oldest; a position whose element stays 4 because the ground probe fails is not recorded (§19.6). |

---

<a id="s19"></a>
## 19. Worked synthetic examples

<a id="s19-1"></a>
### 19.1 A hotspot with two effects

A synthetic template with `EffectGenerator_HotspotEffectList` = [(effect A, spellcraft −1),
(effect B, spellcraft 40.0)] compiles to: root (class *H*) with attribute 0x04343517 → item
1; item 1 a class-701 object with 0x07D7CE15 → item 2, 0x0C9D580F → item 3,
0x035D2793 = 0x00100010, 0x07BB0CF2 = 4; item 2 a `multimap_u32` {(64, 3), (64, 4)}; item 3
a `refmap_u32` {(3 → item 4), (4 → item 6)}; item 4 a class-700 object with effect A,
spellcraft −1, key 3, 0xCF31763 = 64, 0x35D2793 = 513, 0x53151DF → item 5 (class 1626, all
zero); item 6 likewise for effect B with key 4 and spellcraft 40.0, → item 7. A reader of
§2.4 ignores items 1–7 and reads the list from the property set; a writer produces them from
it. The multimap's and the element map's stored order follow `wstate.md` §5.3 (bucket order).

<a id="s19-2"></a>
### 19.2 Use range

Player cylinder: radius 0.4 m, height 1.8 m, at (10, 10, 0) in landblock (0x80, 0x40).
Target: radius 1.0, height 2.0, at (155, 12, 0) in landblock (0x7F, 0x40) of the same region
and instance; `Usage_MinUsageDist` = 2. Bring the target into the player's frame:
x = 155 − 160 = −5. Horizontal centre distance √(15² + 2²) = 15.13; minus 1.4 = 13.73.
Vertical gap 0 (spans overlap). *d* = 13.73 > *R* = 2 + 3 = 5: out of range. Moved to
x = 7 in the player's landblock: √(3² + 2²) − 1.4 = 2.21 ≤ 5: in range. (Both cylinders here
are final values with zero offsets and no override.)

Same target at x = 7, now with `Physics_GameRadiusOverride` = 2.0 and `Render_AdjustableScale`
(1.5, 1.5, 1.5): its radius becomes 2.0 × 1.5 = 3.0, so the horizontal gap is
√(3² + 2²) − (0.4 + 3.0) = 0.21. If instead the target's collision box spans z from 0 to 2.0
(offset z = 1.0) and it stands at z = 2.5 while the player stands at z = 0 with a zero offset:
moved heights 0 and 3.5, vertical gap 3.5 − (0 + 1.8) = 1.7, *d* = √(2.21² + 1.7²) = 2.78.

Base cylinder from data: a synthetic entity whose setup's `physics_bounds` give radius 0.5
and height 1.8, with `Physics_AdjustableScale` (2, 2, 2) and no `Model_Scale`, has *s* = 2 × 1
= 2, *r* = 0.5 × 2 + 0.05 = 1.05 m and *h* = 1.8 × 2 + 0.1 = 3.7 m. With `Model_Scale`
(1, 1, 0.5) instead, *s* = 2 × 0.5 = 1 (z components only), *r* = 0.55 m, *h* = 1.9 m.

<a id="s19-3"></a>
### 19.3 Pet movement state

Thresholds stored as (15 → `Super`), (10 → `Fast`), (6 → `Normal`); `CosmeticEntity_Following_Speed`
= 10. Owner running at 12 m/s: pace 12, the first entry with threshold ≤ 12 is 10 → `Fast`.
Owner walking at 4 m/s: pace 4, no entry → 0 (`Slow`). Owner standing still: pace = 10 →
`Fast`.

<a id="s19-4"></a>
### 19.4 Induction duration and setbacks (Decisions D9, D10)

Record: `Induction_Duration` 3.0, additive list [*P*], multiplier list [*Q*, *U*], policy
`ModifyStartTime`, one setback structure (`Combat_OnTargetOfHit`, time 1.0). User values:
*P* = −0.5, *Q* = −0.1, *U* absent (default 0). *A* = −0.5, *M* = 1 + (−0.1 + 0) = 0.9,
*T* = (3.0 − 0.5) × 0.9 = 2.25 s. Started at *t* = 100.0: end 102.25. Hit at *t* = 101.5
(elapsed 1.5): start moves to 101.0, end 103.25. Had the hit come at *t* = 100.2 instead
(elapsed 0.2): start moves to min(101.0, 100.2) = 100.2, end 102.45. With policy `ModifyDuration` and a progression whose
values at indices 1, 2, 3 are 0.4, 0.3, 0: the first hit adds 0.4 s to *T*, the second 0.3 s,
later hits nothing.

---

<a id="s19-5"></a>
### 19.5 Mouse-over type, range and cursor of an NPC

A synthetic NPC template *E* names the usage action of class 1580 (step 3 there gives 0: no
item waits for a target and the action record has no `Mouseover_Type`), stores `Usage_MinUsageDist` = 4,
`Vendor_InventoryList` and `Repair_CostModifier`, and nothing else of §3.4.5; the quest tests
fail. The NPC service rule matches row 2 once (*n* = 1), so *T* = 18 `NPC_Vendor`. Show runs
with *T* = 18: `Mouseover_Entity` := *E*, `Mouseover_Range` := 4.0, the directory struct for
18 gives the cursor. If *E* also stored `NPC_Is_Banker` (any value), row 16 would make *n* = 2
and *T* = 10 `NPC_Multiple`. On the first hover the flag was computed with the previous
entity's range; with the player 5 m (gap distance) from *E*, the next update computes
5 ≥ 4, sets the flag, hands off the change, and the struct's `Mouseover_CursorOutOfRange` is
shown. The use range of §3.4.1 for the same *E* is 4 + 3 = 7 m, so the player can still use
*E* while the out-of-range cursor shows.

<a id="s19-6"></a>
### 19.6 Pet trail

The owner stands at *p*₀ on ground (`Entity_Element` 5) when the pet is registered: the trail
is [*p*₀]. The owner moves to *p*₁, 0.005 m from *p*₀: nothing is recorded. It moves to *p*₂,
1 m away: the trail is [*p*₂, *p*₀] (newest first). After 40 recorded points, a further point
*p*ₙ makes the trail [*p*ₙ, …] with the oldest point gone, still 40 long. While the owner
jumps (element 4, `Earth_NoSurface`), each point is first dropped onto the ground by the probe
and recorded with element 5; if the probe finds nothing, the point keeps element 4 and is not
recorded. In the follow step, two points with equal scores resolve to the newer one.

<a id="s20"></a>
## 20. Open questions (summary)

- §3.2.3, §3.6, §3.7.3 (bytecode): use behaviour of usage-action, permission and plugin
  classes (class code with unspecified native functions).
- §2.4: byte-exact writer for the activity and trigger-response bundles.
- §3.4.1: whether the `field_b` = 4 property provider supplies dictionary defaults (Decision
  D14).
- §3.4.2: which relation the client's entity tree records, also §3.4.3 (Decision D15).
- §3.4.5: the quest-book, harmful-interaction, monster-play and house-keeper tests (Decisions
  D16, D17).
- §3.5: the handler classes' menu functions (options per object state, enabled state, the
  take handler's test, commands).
- §3.8.5 (server): induction duration arithmetic (Decision D9), setback unit and policies
  (D10), meaning of `Induction_Duration_AreaChallengeLevelOverride`.
- §3.9.3 (server): channelling setbacks, duration multiplier, barred actions and interrupts
  (D12).
- §4.6 (server): hotspot area when the box is zero.
- §5.3: effect of `SeeMeVisibilityMask` = `Hidden` (server or owner observation); meaning of
  `NeverHousekeep` (server).
- §6 (server): how a loadout is chosen.
- §7.4: what the liquid probe accepts, the event that resets the trail, use of the trail
  points' element, state-to-animation mapping.
- §8.3 (server): rally-point choice and revive rules.
- §9.3 (server): work-unit accrual and toggle activities.

---

<a id="s21"></a>
## 21. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-07 | all | First version (spec writer, dirty room; OBS over all records of each kind, LC for two facts, BIN questions (a)–(f)). Status Review. |
| 2026-10-07 | header, §1, §2.4, §3.1, §3.4.1, §3.4.3, §3.8.1, §3.8.5 (new), §3.9.3 (new), §5.3, §16.1, §18, §19.2, §19.4 (new), §20 | Revision 2, answering review 1 (spec writer). R1: `Induction_Duration_AreaChallengeLevelOverride` is a float (I11a). R2: cylinder construction with base constants, bounding-box offset and the `Physics_GameRadiusOverride` × `Render_AdjustableScale` override; gap distance measured from offset positions; OPEN narrowed to *r*₀, *h*₀, *s* (BIN g; I6, I6a, I6b). R3: data facts on modifiers and setbacks (I11a, I11b, I13a), OPEN callouts, Decisions D9–D12. R4: provenance row I27 for the §3.1 census; the two "Reported by name" uses (§3.8.1, §5.3) removed. R5: shortcut routine fact (BIN i) and OPEN. R6: §2.4 coverage table and OPEN. R7: setback structures hold exactly one of time or progression. R8: D4 lists the requirements Tessera evaluates. New invariants IN15–IN18. Status stays Review. |
| 2026-10-08 | header, §1, §2.4, §3.2.3, §3.4.1–§3.4.3, §3.4.4 (new), §3.5, §3.6, §3.7.3, §3.8.5, §3.9.3, §4.3, §4.6, §5.3, §6, §7.4, §8.3, §9.3, §14, §16.1, §16.3, §18, §19.2–§19.4, §20 | Revision 3 (spec writer). Review 2: Q1 outermost-parent rule and self case of the cursor distance (I7a, §3.4.2); Q2 `Mod_Entry` census corrected and re-run (§3.8.5 table); Q3 "no native routine refers to … as a constant" (§3.8.5, I11b); Q4 D12 entering a state ends the current one regardless of `Channeling_CanBeAbandoned`; Q5 *r*₀/*h*₀/*s* now facts, offset unscaled, Decision D13 for entities without a setup; Q6 synthetic progression values changed. Closed OPENs: source of *r*₀, *h*₀, *s* (setup `physics_bounds`, `Physics_AdjustableScale` × `Model_Scale`, BIN j; I6c); mouse-over type choice (hand-off and rules, BIN k + BC; new §3.4.4, I7b; narrower OPEN kept); shortcut list and resolution side effect (I8; relation OPEN merged into §3.4.2); hotspot descriptors and `Hotspot_EnableVisualization` (I14a); missing class-701, class-1626 and class-3427 attribute hashes (I3a; §2.4 OPEN narrowed to the last row); pet following details (BIN l; I20a; narrower OPEN). Corrected: the (e) constant hits for `Mouseover_CursorOutOfRange`, `Mouseover_MouseoverList` and the loadout PIDs are UI element IDs (§6, §14). New source BC (class-library bytecode read with `wlib-bytecode.md`). Remaining OPENs made precise; server-side ones marked "(server)". New invariants IN19–IN22. Status stays Review. |
| 2026-10-08 | header | Review 3 (spec reviewer, separate session): passed; status Review → Accepted. No content changed. |
| 2026-10-08 | header, §1, §3.4.1, §3.4.2, §3.4.4, §3.4.5 (new), §3.4.6 (new), §3.5, §7.4, §16.3, §18, §19.5 (new), §19.6 (new), §20 | Revision 4 (spec writer). No implementer OPEN notes had been filed against this spec; this revision closes or narrows the writer's own OPENs and the four minor notes of review 3. **Corrected (behaviour change for implementers):** `Mouseover_Range` is not server-supplied: the class code writes it onto the local player from the hovered entity (`Usage_MinUsageDist_ClientOverride`, else `Usage_MinUsageDist`, else 2.0; I7d, §3.4.2); the pet trail is kept newest first and recorded per owner position change of more than 0.01 m, filtered by element (I20b, §7.4), which replaces the old Decision D7 rule; `CSM_DisplayInactiveOptions` is read from the clicked object, not the handler (§3.5). **Closed:** the cursor and effect step (§3.4.6, I7f); the seven overriding usage-action classes, with the NPC service rule (§3.4.5, I7e); selectability and usage-action lookup are shared by all world-entity classes (§3.4.4); trail recording (§7.4); hover-target choice and flag recomputation (§3.4.2, I7c). **Narrowed:** §3.4.1 (lookup routes by `field_b`; Decision D14), §3.4.2 entity tree (Decision D15, review note m4), §3.4.5 tests of other systems (Decisions D16, D17), §3.5 menu assembly, §7.4. **Review 3 notes:** m1 (§3.4.1 warning), m2 (§3.4.4 no default), m3 (BC named in rows I9, I13a, I17, I21, I22 and in the BC read list), m4 (D15). New provenance rows I7c–I7f, I20b; new BIN questions (o)–(s); new invariants IN23–IN27. Status Accepted → Revised. |
| 2026-10-10 | §3.4 | Note only (no fact changed), for P3: the input that uses an entity is `npc-windows.md` §3.1. Status unchanged. |
