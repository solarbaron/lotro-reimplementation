<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Game-control singletons: PvP and monster play, mounts and default attacks, account and store rules, UI and system rules, social managers

| | |
|---|---|
| **Status** | **Accepted** (review of the 2026-10-08 revision passed, 2026-10-08) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted), not the author |
| **Review record** | Reviews 2026-10-07: first and second changes requested; third passed (Accepted). Revision 2026-10-08: reviewed 2026-10-08 by a separate session (spec reviewer, AI-assisted), **passed**, Accepted. Details in the review log (decision 0018). |
| **Implements into** | `crates/props` (typed views of the singletons of §3–§8, the WEENIECONTENT label index of §2); `apps/explorer` (generic typed singleton views, the label index, the reference graph of §9); `apps/client` (the mounted gait multipliers of §5.3.2, duel timings of §5.7, the player-note limit of §8.1, consideration and vital-display lookups once their OPENs close); Tessera's own server (M7: PvP, monster-play, perk, collection, reset and social rules); `apps/dattool` (`info` for the generic forms of §3) |
| **Companion specs** | [`property-system.md`](property-system.md) §5 (property sets), §6.2 (the +0x09000000 rule), §7 (enum and bitfield labels, the bit-to-key rule §7.3), §8 (DID maps, the root key 0x0A WEENIECONTENT); [`wstate.md`](wstate.md) §3–§5 (object records, items, native classes; the small set of class 18 and the counted lists of classes 104 and 11, §5.3); [`wlib.md`](wlib.md) (class library, parent map); [`misc-records.md`](misc-records.md) §14 (the WEENIECONTENT map; this spec resolves the labels it hands on); [`combat-data.md`](combat-data.md) §4 (progressions, class 1385), §7 (skills, class 827), §8 (traits), §10.1 (the combat control); [`economy.md`](economy.md) §4.12 (loot singletons), §6.4 (perk vendors); [`items.md`](items.md) §6.4 (paper-item lists); [`map-data.md`](map-data.md) §9.7 (the housing control); [`quests.md`](quests.md) (reputation, reward-track experience); [`creatures.md`](creatures.md) §2.1 (classes 1653 and 1654); [`sound.md`](sound.md) §10.6 (combat music script); [`walker.md`](walker.md) (movement controller, for §5.3.2) |

---

<a id="purpose"></a>
## Purpose

The WEENIECONTENT DID map (0x2800000E, `misc-records.md` §14) names 316 entries. Before
this spec, 239 of its labels were not mentioned in any spec, and `misc-records.md` §14 hands
them to the specs of their subsystems. This spec is the sweep for the leftover **global rule
singletons and small system tables**: the PvP and monster-play controls, the mount and
mounted-combat controls with the per-weapon-style default attacks, the account and store
controls (perks, collections, paper-item services, login rewards, reward tracks), the UI
and system rules (vital-display overrides, confirmation-dialog markers, alerts, reset
schedules, consideration colours, formations, combat music, discovery distance, tracking,
pets, shapeshifting, open tapping), and the data side of the social managers (guild, mail,
auction, fellowship, housing).

With it an implementer can:

- resolve **every** WEENIECONTENT entry to one of ten structural kinds and read it with the
  generic reader of that kind (§2.1, §3); that makes the whole map resolvable by label
  instead of leaving labels as opaque DIDs;
- read each singleton of this spec's scope (119 labels, §2.2) and its member families (300
  further objects, §2.2) as typed values, with the census, value ranges and references of
  every property;
- follow the references between these records (§9) and check the invariants of §10;
- reproduce the few client behaviours that the client's own code settles: which of these
  singletons the client resolves by constant key, the mounted gait speed and turn-rate
  multipliers, the duel timing defaults, the player-note length default (§2.4, §5.3.2,
  §5.7, §8.1).

Most of these records are **server tuning tables**: the data says what the numbers are, not
how a server combines them. Those formulas are not in the data and are listed as OPEN
(§12); Tessera's own server (M7) defines its own rules from them. No network behaviour is
covered: the network protocol, login, keys and the launcher stay behind the M8 gate
(`docs/LEGAL.md` §9.3, decision 0004).

Game text is never reproduced: records are named by DID, fields by property label and
enum-mapper label. Those labels are interoperability identifiers (they are how a client
finds the records and fields at run time) and are quoted for that reason only.

---

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install with throwaway probes in the dirty-room
  scratch directory. Unless a row says otherwise, **every** record of the kind was read, and
  every property set and object state decoded completely with the decoders of
  `property-system.md` and `wstate.md`.
- **Reported**: from published sources only. No fact in this spec is Reported.
- **BIN**: read from the client executable under method M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)).

| Key | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-07 (Steam install, client data current on that date). Read: the WEENIECONTENT map (all 316 entries) and the root DID map; **all 383,343 object records** (383,332 of prefix 0x70 and 11 of prefix 0x76, all in `client_gamelogic.dat`: root class, size); the **383,332 object property sets** (0x79; one per 0x70 object; the 0x76 objects have none, because DID + 0x09000000 would be a 0x7F record and none exists), none failed; all **1,618 bare property sets** (0x78: 1,616 in `client_gamelogic.dat`, 2 in `client_local_English.dat`; no other archive holds records of prefixes 0x70–0x7F); and all 24 DID maps (all in `client_general.dat`), for the typed-reference census of §9 (5,915,209 non-null DID values in the 383,332 object sets and the 1,616 bare sets of `client_gamelogic.dat`, plus 3,666 non-null DID-map values; the two `client_local_English.dat` sets, one chat-font array with 6 non-null DIDs and one empty set, were checked separately in the revision and name none of the 119 labels); a byte scan for the raw 4-byte DIDs of the 119 labels of §2.2 over every record of prefixes 0x07, 0x0C, 0x22, 0x28, 0x47, 0x56, 0x70, 0x76, 0x78 and 0x79 in `client_gamelogic.dat` and `client_general.dat`, and over the two bare sets of `client_local_English.dat` (added in the revision of 2026-10-07); the full property trees of the 119 labelled objects and per-class censuses (every property path: presence, occurrences, type, range, distinct values, referenced classes) of 97 + 18 root classes; the object states of the class-3399, 849, 852 and 1490 records (§3.3, §4.2, §4.10); the enum mappers named by the properties of this spec (labels and key sets); the property dictionary (27,546 definitions) for the family counts of §4.9, §5.10 and §8.5. The probes used the project's own decoders (`tessera-dat`, `tessera-formats`, `tessera-props`, `tessera-wstate`) read-only plus small independent scripts. |
| BIN | **M4 (decision 0011).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (verified on 2026-10-07), read on 2026-10-07 with Ghidra (headless, on a private copy of the shared analysed project, deleted after use). **Questions:** (q1) which WEENIECONTENT singletons the client's native code resolves, and how; (q2) whether native code reads any property of this spec's singletons by a constant property ID; (q3) for each native reader found, what it reads, which defaults apply when a value is absent, and how the values are combined. **Method:** an instruction scan for call sites that pass the WEENIECONTENT root key together with a constant key of that map; an instruction scan for immediates equal to the 296 property IDs of this spec's controls; decompilation of the resolver's callers for this spec's labels and of the functions found by the property scan. **Why M1–M3 were not enough:** the data does not say which side (client or server) consumes a control, nor which value applies when a property is absent; a frame or log capture (M3) cannot separate the mounted gait multipliers from server-driven movement, and no capture of the running client is available to this session. **Not read:** anything that handles network traffic, login, keys, the launcher or anti-tamper. No symbol names, listings or addresses are carried into this spec; they stay in the dirty notes. **Second revision (2026-10-07):** (q4) whether native code could read GRSControl by a constant property ID (§7.16); method: a byte scan (no disassembly) of the same executable (SHA-256 prefix re-verified) for the 4-byte little-endian values of the 28 property IDs of GRSControl's property tree, sorted by PE section. **Third revision (2026-10-08):** (q5) what the guild reader does when `Guild_PlayerNoteMaxLength` is present with a type other than int; method: the decompilation of the guild reader made for q3 on 2026-10-07 was re-read (same executable, SHA-256 prefix re-verified on 2026-10-08); no new disassembly. |
| OBS2 | Observed in the user's install on 2026-10-08 (same data as OBS), with a new throwaway probe on the project's decoders, read-only: the class-table definition (defined or undefined, `wlib.md` §6.3) and parent of each role class of §2.1; every property of the 17 entities of §3.4 compared by value; the value types of every flank and trait-tree field of MountedCombatControl; the full property tree of RandomItemRewardControl with value types; the path from the top of the set to every `VitalDisplayOverride_UnlockedVDOs` value in all object property sets and bare sets of `client_gamelogic.dat`, and every occurrence of the array that holds them; every property set holding `RaceTable_IsTallRace`, with its `RaceTable_Race`; every array of the 387 objects of this spec's scope that have a property set (119 labels and 268 family members; the 32 compiled-only tables of §4.10 have none): lengths and null DID elements. |
| SPEC | Facts stated, with their own provenance, in the companion specs named in the header; used here as cross-references only. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| GC1 | Structural kinds of all 316 WEENIECONTENT entries: the ordered rule and the class indices of §2.1 (ancestry 484 ← 485 ← 89 of all 314 object entries; 286 under 486, 28 under 223; the rule gives the counts of the kind table) | OBS (316 of 316; rule re-run in the revision of 2026-10-07); OBS2 (which role classes are defined in the class table) | Confirmed |
| GC2 | This spec's scope: 119 labels (97 root classes, 338,903 bytes of object state and property set) and 300 member objects in 18 classes (61,067 bytes) (§2.2) | OBS | Confirmed |
| GC3 | No typed property value and no DID map other than WEENIECONTENT references any of the 119 labelled objects, except MOUNTED_1H (one monster-play table) and NewbieCinematicEncounter (8 references); raw 4-byte scans find no other occurrence at a reference position (§2.3) | OBS (all typed references of the 383,332 object sets, the 1,618 bare sets and the 24 DID maps; raw byte scan as in the OBS row) | Confirmed |
| GC4 | Native client code resolves WEENIECONTENT entries by **key**, through the general DID-map lookup with root key 0x0A; of this spec's labels only CombatControl, MountedCombatControl, RandomItemRewardControl and GuildControl are resolved that way (§2.4) | BIN (q1: every call site with a constant key of the map); OBS (keys) | BIN |
| GC5 | Of 296 property IDs of this spec's controls, native code reads by constant ID only the mounted gait multipliers and add-mod names, the mounted-combat unlock trait, two duel timings, the login-reward category name and tier fields, and one guild field; all other matches are user-interface element identifiers that share the numeric range (§2.4) | BIN (q2) | BIN |
| GC6 | Empty marker objects: 38 `Dialog_*` labels, 3 level directories and 11 other labels; one script object with no attributes, a 6-byte empty property set; one distinct root class each (§3.2) | OBS (52 of 52) | Confirmed |
| GC7 | Class-3399 DID sets: six labels; one reference attribute to a class-18 small set of object DIDs; empty property set (§3.3) | OBS (6 of 6) | Confirmed |
| GC8 | Manager entities: the 14 labels whose root class has the manager base (788 here) as parent; one 36-property entity template (RepositoryManager adds 12 repository version numbers, listed in §3.4); three other entities (WorldEventServer, PrivateEncounterServer, RipCordPersistentUnique) carry the same template except `PhysObj` (§3.4) | OBS (all 28 entity entries compared property by property); OBS2 (the name property is `Name`, the only property whose value differs between the 13 managers other than RepositoryManager) | Confirmed |
| GC9 | PvP controls: PVPControl, PVPVirtueControl (+ slot schedule), PVPCombatControl, PVPFeatControl, GloryControl (§4.1–§4.5) | OBS | Confirmed |
| GC10 | Monster-play controls and tables: MonsterPlayControl, MPAdvancementControl, MPLevelTableDirectory, the two empty level directories (§4.6–§4.8) | OBS | Confirmed |
| GC11 | Compiled-only tables referenced by these controls: class 849 (29 cumulative-value arrays of u64), class 852 (3 maps of u32 to float bits) (§4.10) | OBS (29 + 3 object states) | Confirmed (layout); meaning of the index OPEN |
| GC12 | Mount directory: 676 skill DIDs (347 tall, 329 short; disjoint); the per-species tall flag of the race table (§5.1) | OBS; OBS2 (`RaceTable_IsTallRace` in every property set of `client_gamelogic.dat`: 8 species objects) | Confirmed |
| GC13 | MountAppearanceTraits is the set of all 767 trait objects of class 3509. Each carries exactly one `Trait_Group`, nested as the single element of the top-level array `Trait_Groups` (none at top level); keys 8–14 of mapper 0x230002EE with the per-slot counts of §5.2; the seven keys are exactly the keys of `Mount_Control_Appearance_Group_To_Property_Hash`; the appearance fields and dye flags of §5.2; the seven slot-material properties named by the hash share the mapper of `Trait_Appearance_Material` and occur in no object property set and no bare set (§5.2) | OBS (767 of 767, full recursive walk of each property set; all 383,332 object sets and the 1,616 `client_gamelogic.dat` bare sets for the slot-material properties; second revision of 2026-10-07) | Confirmed |
| GC14 | MountedCombatControl layout (§5.3.1); the flank fields are floats; both trait-tree struct kinds carry `Mount_Control_Trait_Tree` | OBS; OBS2 (types and labels, 10 of 10 flank structs, 3 + 3 tree structs) | Confirmed |
| GC15 | Mounted gait multipliers: speed by gait (Walk/Trot/Canter/Gallop, else 1.0), turn rate by gait plus the actor's add-mod property, the locked-gear speed limit (§5.3.2) | BIN (q3) | BIN |
| GC16 | The 17 weapon-style default-attack skills: shared shape, distinguishing fields, mounted additions (§5.5) | OBS (17 of 17) | Confirmed (data); selection rule OPEN |
| GC17 | WeaponDamageVarianceTable, CombatStateControl, ResistControl layouts (§5.6, §5.8, §5.9) | OBS | Confirmed |
| GC18 | Duel timings read by the client, defaults 10 s and 600 s (§5.7) | BIN (q3); OBS | BIN |
| GC19 | Perks: PerkControl channels and the 47 perk objects (§6.1) | OBS (47 of 47) | Confirmed |
| GC20 | Collections: CollectionControl and 29 collection objects; collection data on 1,347 skills (§6.2) | OBS | Confirmed |
| GC21 | Paper-item services: 35 entries in 7 classes, one per service type (§6.3) | OBS (35 of 35) | Confirmed |
| GC22 | TurbinePointsControl, RewardTrackControl, RandomItemRewardControl, LoginRewards and the two login-reward objects (§6.4–§6.7) | OBS; OBS2 (the full tier-field labels and their types, 10 of 10 entries); BIN (RandomItemRewardControl reader) | Confirmed; reader BIN |
| GC23 | Vital-display overrides: 113 override objects, the control list, the 111 unlocking items; each unlock is the single element of the item's top-level `Usage_SetPropertyUsageData_Array` (§7.1) | OBS (113 + 111); OBS2 (the location, 111 of 111; every set of `client_gamelogic.dat` that holds the array) | Confirmed |
| GC24 | AlertsControl covers each non-zero alert type once (§7.3) | OBS | Confirmed |
| GC25 | ResetScheduleControl (§7.4) | OBS | Confirmed |
| GC26 | Consideration control and its 8 tables: 31 ranks per table, non-decreasing, index 15 = Even in all 8 (§7.5) | OBS (8 of 8) | Confirmed (data); index rule OPEN |
| GC27 | Formation control and the 30 formation-layout objects (§7.6) | OBS (30 of 30) | Confirmed |
| GC28 | Small UI and system controls: AggroPing, CombatMusic, DiscoveryDistance, TrackingState, Shapeshift, PetControl (2), PetChannel, PruneInfo_Skill, WhiteList (4), OpenTapping, GRS (§7.7–§7.16) | OBS; BIN (q4, GRSControl byte scan) | Confirmed; GRS scan BIN |
| GC29 | Social controls: GuildControl, MailControl, AuctionControl; player-note default 48, also for a value that is not an int (§8) | OBS; BIN (q3, the guild reader; q5, the type check) | Confirmed; default BIN |
| GC30 | Dictionary-only families: LFG (17 definitions) and Antispam (65) hold no value anywhere; used versus defined counts of the other families (§4.9, §5.10, §8.5) | OBS (all 383,332 object sets and all 1,618 bare sets; the two `client_local_English.dat` sets hold no property of these families) | Confirmed |
| GC31 | Reference graph of the singletons and their families (§9) | OBS (the typed-reference census of the OBS row) | Confirmed |
| GC32 | TravelMountSpeedBoostOverride is an effect object (class 734) with the values of §5.4 | OBS (full property tree, re-read in the revision of 2026-10-07) | Confirmed |
| GC33 | In the 387 scope objects with a property set, no array of DID elements holds a null DID (38 such arrays); the longest array is MountDirectory's `Mount_SkillList` (676) (§3.1) | OBS2 (387 of 387) | Confirmed |

---

<a id="s1"></a>
## 1. Conventions

- A **singleton** is an object record named by a WEENIECONTENT label. Its DID is found at run
  time: read the root DID map 0x28000000, take its entry for root key 0x0A (label
  `WEENIECONTENT`, DID 0x2800000E here), then the entry of the wanted label
  (`property-system.md` §8). DIDs, class indices and DID-map keys quoted in this spec are
  **observations of this install**, given for orientation; an implementation looks records
  up by label and properties by label through the dictionary.
- The **property half** of a singleton is its property set at DID + 0x09000000
  (`property-system.md` §6.2). It carries every value this spec describes, except for the
  compiled-only forms of §3.3, §4.2 and §4.10, whose values are in the object state
  (`wstate.md`). Where both halves exist, the object state holds the same data in compiled
  form (`combat-data.md` §3.2); a reader never needs it.
- Property tables give: the property label, its type (`property-system.md` §3), and what
  was observed. "PID" means a property-ID value (type 10): it names another property, which
  is read from an actor or from the world when the rule is applied. "→ class *n*" names the
  root class of the referenced objects (build-specific). Counts are over the whole install.
- Enum and bitfield values are shown by their mapper labels (`property-system.md` §7);
  bitfields use the bit-to-key rule (`property-system.md` §7.3): bit *i* stands for key
  *i* + 1.
- Units are stated where the data settles them; otherwise the unit is an OPEN or a reading
  marked as such. "Seconds" is used only where the values and the property label agree
  (for example 86,400 for a one-day interval).
- "Server rule" marks a value whose use is server-side gameplay. The data is Confirmed; how
  a server combines it is not in the data.

---

<a id="s2"></a>
## 2. The WEENIECONTENT map and this spec's share

<a id="s2-1"></a>
### 2.1 Structural kinds (Confirmed, GC1; 316 of 316 entries)

Every entry of the map falls into exactly one of ten kinds. The kind is recognised from
the record itself, not from the label: from the entry's DID, the root class *c* of the
object record (payload offset 4 of the 0x70 record, `wstate.md` §3.1), the ancestors of *c*
in the class library's parent map (`wlib.md`), and, for the empty forms, the record sizes.

**Rule.** Test the rows of this table in order; the first row that matches gives the kind.

| Order | Kind | Rule | Entries | Root classes | Reader |
|---|---|---|---|---|---|
| 1 | Null entry | the entry's DID is 0 (label `Undef`) | 1 | — | none |
| 2 | Script table | the DID has prefix 0x07 (`SharedUISelection_ScriptTable`) | 1 | — | `script-table.md` |
| 3 | DID set | *c* is the DID-set class | 6 | 1 | §3.3 |
| 4 | Dialog marker | the **parent** of *c* is the dialog base | 38 | 38 | §3.2 |
| 5 | Other empty marker | the object record is 36 bytes and decodes to one script object with no attributes, and the property set is 6 bytes with zero entries | 14 | 14 | §3.2 |
| 6 | Skill | *c* is the skill class (`combat-data.md` §7) | 23 | 1 | §5.5 and `combat-data.md` |
| 7 | Effect | the effect base is *c* or an ancestor of *c* (`combat-data.md` §2.1) | 12 | 4 | `combat-data.md` §6 |
| 8 | Manager entity | the **parent** of *c* is the manager base | 14 | 14 | §3.4 |
| 9 | Other entity or item | the entity base is an ancestor of *c* | 14 | 9 | their subsystem specs |
| 10 | Property control | the plain-object base is an ancestor of *c* | 193 | 182 | §3.1 |

**Class indices.** Class indices are build-specific (`wlib.md`). The values below are those
of this install. A reader keeps them in one table that a new build can replace, and checks
them with the cross-checks of the last column, which are in the data.

| Role | Class here | Cross-check (observed in every entry of the kind) |
|---|---|---|
| Common ancestors of all object entries | 484, 485, 89 | 484 is the root of the parent map, 485 its child and 89 the child of 485; all 314 object entries have the three among their ancestors |
| Plain-object base | 486 | 286 object entries descend from it: rows 3–7 and 10 |
| Entity base | 223 | 28 object entries descend from it: rows 8 and 9; no entry descends from both 486 and 223 |
| DID-set class | 3399 | empty property set; the two-item object state of §3.3 |
| Dialog base | 1728 | every entry whose root class has this parent is an empty marker (38 of 38) |
| Skill class | 827 | the skill signature properties (`combat-data.md` §2.1, §7.1) |
| Effect base | 698 | the effect signature properties (`combat-data.md` §2.1) |
| Manager base | 788 | the 36-property manager template of §3.4 (`PhysObj` 0x47000601 here); the template is necessary, not sufficient (§3.4) |

Notes on the rule:

- The order matters: the DID-set class, the dialog markers, the other empty markers, the
  skill class and the effect base all descend from the plain-object base, and so would
  otherwise fall into row 10.
- Row 8 needs the parent class, not the property template: three entities of row 9
  (WorldEventServer, PrivateEncounterServer and RipCordPersistentUnique, root classes 1811,
  1367 and 3168 here, whose parent is a different entity class, 1733 here) carry the
  same 36 properties with the same values as the managers, except `PhysObj` (§3.4).
- Of the 193 property controls, 187 hold their values in the property set and 6 only in the
  object state (RunicCombinationControl, QuestEventTargetLocationControl,
  BTSRequiredWebstoreItem, QuestCategoryLimiterControl, BattleStatControl,
  StorePropertyManipulatorDirectory; none in this spec's scope).
- The 14 entries of row 9 are Generator, Bank, HousingContainer, MountDecorationContainer,
  PetDecorationContainer, SelfReviveToken, TravelHorseTicketPaperItem,
  MithrilCoins_PaperItem, MordorAdvancementItemGrant, BankFillItem, GroupRefereeEntity and
  the three server entities named above.

**Role classes need not be defined** (Confirmed, GC1, OBS2). The rule and the cross-checks
use only the parent map, never the class table. A role class may be an undefined entry of
the class table (`wlib.md` §6.3) and still have a parent: in this install the **entity base
(223 here) is undefined**. It occurs only in the parent map, with parent 89 (`wlib.md` §9
notes the same index as the one undefined parent of a defined class). The other nine role
classes (484, 485, 89, 486, 3399, 1728, 827, 698, 788) are defined. The parent chain of
the cross-checks holds whether a role class is defined or not: 484 has no parent, 485's
parent is 484, 89's parent is 485, and 486 and 223 both have parent 89. A reader therefore
checks parentage only and does not require a class-table definition for any role class.
(Answers the implementer note of 2026-10-08.)

The 14 "other empty markers" are: DefaultGeneratorCondition, MonsterLevelDirectory,
UsagePermission_AlwaysAllow, UsagePermission_Default, UsagePermission_HousingDecorationItem,
TakePermission_CantTake, SummonFellowSelectCB, ConversationCallback, PerfSetup,
MonsterPlayLevelDirectory, SessionPlayLevelDirectory, PvPPlayLevelDirectory,
RemoveSocketOperation and RemoveAllSocketOperation. This spec describes the form (§3.2) and
the three level directories; the others belong to their subsystem specs.

<a id="s2-2"></a>
### 2.2 Scope (Confirmed, GC2)

This spec covers 119 labels (97 root classes, 338,903 bytes of object state and property
set together) and 300 member objects in 18 further classes (61,067 bytes):

| Group | Labels | Section |
|---|---|---|
| Generic forms | the 38 `Dialog_*` markers; the six DID sets (WebStore, MountAppearanceTraits, LoginRewards, BigBattles, StorePropertyManipulator, LootGenControl_RewardOnceTGPs); the 14 manager entities (PVPManager, GuildManager, FellowshipManager, FellowshipMakerManager, MailManager, AuctionManager, HousingAuthority, CharacterSheetManager, PopulationTrackingManager, RepositoryManager, WorldJoinManager, TrackingManager, ScriptManager, NeighborManager) | §3 |
| PvP and monster play | PVPControl, PVPVirtueControl, PVPCombatControl, PVPFeatControl, GloryControl, MonsterPlayControl, MPAdvancementControl, MPLevelTableDirectory, MonsterPlayLevelDirectory, PvPPlayLevelDirectory | §4 |
| Mounts and combat rules | MountDirectory, MountAppearanceTraits, MountedCombatControl, TravelMountSpeedBoostOverride, UNARMED, 1H, 1H_1H, 1H_Shield, 2H, BOW, CROSSBOW, JAVELIN, MOUNTED_UNARMED, MOUNTED_1H, MOUNTED_1H_1H, MOUNTED_1H_Shield, MOUNTED_2H, MOUNTED_BOW, MOUNTED_CROSSBOW, MOUNTED_JAVELIN, MOUNTED_RANGED_FALLBACK, WeaponDamageVarianceTable, CombatControl, CombatStateControl, ResistControl | §5 |
| Account and store | PerkControl, CollectionControl, PaperItemControl (services), TurbinePointsControl, RewardTrackControl, RandomItemRewardControl, LoginRewards | §6 |
| UI and system rules | gmVitalDisplayOverrideControl, AlertsControl, ResetScheduleControl, ConsiderationControl, DistantBattleUnitFormationControl, AggroPingControl, CombatMusicControl, DiscoveryDistanceControl, TrackingStateControl, ShapeshiftControl, PetControl, MonsterPetControl, PetChannelControl, PruneInfo_Skill, NewbieCinematicEncounter (for its whitelist), OpenTappingControl, GRSControl | §7 |
| Social managers (data side) | GuildControl, MailControl, AuctionControl, HousingControl (pointer only) | §8 |

Member families (not labelled, reached from the singletons): perks (47, class 1377),
collections (29, class 3996), paper-item service entries (35, 7 classes), vital-display
overrides (113, class 3463), consideration tables (6 + 2, classes 1694 and 1697),
formation layouts (10 + 20, classes 3757 and 3856), whitelists (4, class 3049), login
rewards (2, class 3731), advancement tables (29, class 849), modifier tables (3, class 852).

Already described elsewhere and only cross-referenced here: the combat control's tables
(`combat-data.md` §10.1), the paper-item lists (`items.md` §6.4), the housing control
(`map-data.md` §9.7), progressions (class 1385, `combat-data.md` §4), skills, traits and
effects as such (`combat-data.md` §6–§8).

<a id="s2-3"></a>
### 2.3 How the singletons are reached (Confirmed, GC3)

- **Through WEENIECONTENT only.** No typed DID value in any of the 383,332 object property
  sets, the 1,618 bare property sets or the 24 DID maps names any of the 119 labelled
  objects, with two exceptions: MOUNTED_1H is the `MonsterPlay_DefaultMeleeAttackSkill` of
  one monster-play table (class 1932), and NewbieCinematicEncounter (an encounter template,
  class 1368) is named by `Effect_Travel_PrivateEncounter` (4) and
  `QuestDispenser_PrivateEncounterTemplate` (4).
- **No raw occurrence.** A byte scan of every record of prefixes 0x07, 0x0C, 0x22, 0x28,
  0x47, 0x56, 0x70, 0x76, 0x78 and 0x79 in `client_gamelogic.dat` and `client_general.dat`,
  and of the two bare sets in `client_local_English.dat`, found each of the 119 DIDs once in
  WEENIECONTENT, once at payload offset 0 of its own object record (the header's first
  field, the record's own DID, `wstate.md` §3.1; 119 of 119, and no labelled record holds
  its own DID at any other offset), and otherwise only: the two typed references above,
  two 4-byte matches inside the 29.9 MB object state of QuestEventTargetLocationControl
  (not DID items of its decoded state) and one match inside the class library
  (0x56000000). None is a reference.
- Members of the families are reached from their controls (§9), and some also from other
  records (perks from perk vendors, VDO unlocks from items, formation layouts from
  distant-battle units, whitelists from encounter templates).

A client or server therefore finds each singleton by its label and nothing else.

<a id="s2-4"></a>
### 2.4 What the client's native code reads (BIN, GC4, GC5)

- The client's native code resolves WEENIECONTENT entries **by key**: it calls the general
  DID-map lookup with the root key 0x0A and the entry's key. No label of the map occurs as a
  string in the executable, apart from five generic words that are also used elsewhere
  (`Undef`, `Account`, `Bank`, `BigBattles`, `Character`). 66 call sites with a constant
  key were found (q1): 52 call the lookup directly and 14 go through two thin wrappers. They
  use 37 distinct keys, that is 37 labels. Of this spec's labels, only four occur:

  | Label | Map key (here) | What the native reader reads | Section |
  |---|---|---|---|
  | CombatControl | 0x10000037 | the two duel timings | §5.7 |
  | MountedCombatControl | 0x1000014F | gait speed and turn-rate multipliers, turn-rate add-mod names, the mounted-combat unlock trait | §5.3.2 |
  | RandomItemRewardControl | 0x1000017A | the tier list named by a login-reward record, and per tier the title, description, NPC, NPC icon and reward item | §6.6 |
  | GuildControl | 0x10000040 | `Guild_PlayerNoteMaxLength` | §8.1 |

  A Tessera reader resolves by label (§1); the key is the map's own key for the label and
  is an observation of this install.
- A scan for the 296 property IDs of this spec's controls as instruction immediates (q2)
  found native reads only for the properties in the table above. Every other match is an
  argument of the user-interface element lookup, a separate identifier space that shares
  the numeric range (the same overlap as in `combat-data.md`, row BIN (b1)).
- So the **consideration** colours, the **vital-display overrides**, **combat music**, the
  **aggro ping**, **alerts**, **discovery distance** and **tracking states** are not read by
  native code with constant IDs. Whether the client's script library reads them, or only
  the server, is OPEN (§12, Q1).

> **OPEN:** (Q1) Which code reads the controls that native code does not read by constant
> ID: the client's script library (class library 0x56000000, `wlib.md`) or only the server.
> The bytecode of the script library has not been read for these keys or property IDs.

---

<a id="s3"></a>
## 3. Generic forms

<a id="s3-1"></a>
### 3.1 Property controls

A property control is an ordinary object record whose values are in its property set. Read
it with the generic property-set reader; the per-control tables of §4–§8 give the shape.
No new byte decoder is needed. Each control in this spec has exactly one object in its root
class, except where a section says otherwise (PetControl's class holds two objects, §7.12).

**Typed views: what the client settles and what is a Tessera Decision** (answers the
implementer note of 2026-10-08; GC33, OBS2; BIN for the native reads):

| Choice | Client behaviour | Status for Tessera |
|---|---|---|
| A cap on the length of a list a view keeps (the implementation uses 65,536) | No native read of these lists was found (§2.4), so the client gives no limit. Longest property array in scope here: 676 (`Mount_SkillList`); largest DID set: 1,767 members (WebStore, §3.3) | **Tessera Decision.** Any cap well above the observed lengths is acceptable; 65,536 is kept |
| A present property whose value type differs from the type this spec states | A value's type comes from the dictionary (`property-system.md` §3), so within one install a property always has one type; a mismatch can only appear when another build's dictionary redefines the property. The client's native reads of §5.3.2, §5.7 and §8.1 treat a value of another type exactly as an absent value and use their stated fallback (BIN) | **Client behaviour** for the reads of §5.3.2, §5.7 and §8.1. **Tessera Decision** for every other field: report it as an error of the view (no native read was found to copy) |
| Null DIDs inside DID lists | Not observable here: none of the 38 arrays of DID elements in the 387 scope objects that have a property set holds a null DID (GC33). No native read of these lists was found (§2.4) | **Tessera Decision.** Dropping null elements is kept, except in the consideration lists of §7.5, which keep all 120 positions because the position is the level index (§7.5 reading) |

<a id="s3-2"></a>
### 3.2 Empty marker objects (Confirmed, GC6)

52 labels name objects with no data: the object state is 36 bytes and decodes
(`wstate.md` §3–§5) to a single script object of the root class with **no attributes**;
the property set is 6 bytes, the set's DID and a TSize with count 0 (hint 0)
(`property-system.md` §5.1). Each such label has its **own** root class; the class identity
is the only information.

- **Dialog markers.** 38 labels `Dialog_*` (AbandonChanneling, AbandonPrivateEncounter,
  AbandonQuest, CreateRelationship, DisenchantItemConfirmation, DuelingInvite,
  DyeItemConfirmation, FellowRecruitConfirmation,
  FellowshipMakerConfirmationDialogCallback, FellowshipVoteDialogCallback,
  FinishSessionQuest, GuildConfirmation, GuildCreationConfirmation,
  GuildThemeChangeConfirmation, HousingWritConfirmation, IATargetUsageConfirmation,
  ItemAdvancementBind, LeaveMP, LeavePrivateEncounter, Logout, MasterLootConfirmation,
  PerkConfirmation, PetRenameConfirmation, PrivateEncounter, ReadyCheck,
  RecallConfirmation, ReportSpammerConfirmation, SellAllConfirmation, ShareQuest,
  SkillConfirmationDialogCallback, SummonConfirmation, TraceryUpgradeItemConfirmation,
  UsageConfirmation, WardrobeAddDyeConfirmation, WardrobeItemRemovalConfirmation,
  WJDequeueDialogCallback, WJQueueDialogCallback, WJRequestDialogCallback). All 38 root
  classes have the same parent class (1728 here) and nothing else in common.
- **Level directories.** MonsterPlayLevelDirectory, PvPPlayLevelDirectory and
  SessionPlayLevelDirectory are empty markers whose root classes share one grandparent
  (3418 here); MonsterPlayLevelDirectory and PvPPlayLevelDirectory are in this spec's
  scope.

> **OPEN:** (Q2) What the marker classes are used for. Their names suggest
> confirmation-dialog callbacks and per-mode level directories whose behaviour is in code.
> No native resolver for any of them was found (§2.4); whatever uses them is either the
> client's script library or the server (whose traffic is gated). A Tessera client shows
> them as named markers and gives them no behaviour.

<a id="s3-3"></a>
### 3.3 DID sets (class 3399 here) (Confirmed, GC7; 6 of 6)

Six labels name objects whose **property set is empty** (6 bytes) and whose data is in the
object state only. The state has two items:

| Item | Content |
|---|---|
| 0 (root) | script object of the DID-set class with **one** attribute: a reference (attribute name hash 0x0DBF22C3) to item 1 |
| 1 | native object of class 18 (`set_u32`, `wstate.md` §5.3): a small set of object DIDs |

The set has no order of its own (bucket order, `wstate.md` §5.3) and no duplicates.

| Label | Set size | Members (root class here) | Notes |
|---|---|---|---|
| WebStore | 1,767 | 477 class 2532, 589 class 2541, 270 class 3052, 429 class 3054, 1 class 2776, 1 class 3109 | every object of classes 2532 (477) and 2541 (589) is a member |
| MountAppearanceTraits | 767 | 767 class 3509 | exactly the set of all class-3509 trait objects (`combat-data.md` §8) |
| LoginRewards | 2 | 2 class 3731 | the two login-reward objects of §6.7 |
| BigBattles | 14 | 14 class 2651 | 14 of the 552 class-2651 objects |
| StorePropertyManipulator | 2 | 2 class 3924 | |
| LootGenControl_RewardOnceTGPs | 0 | — | empty in this install |

This answers the class-3399 part of the OPEN in `economy.md` §4.12 (§11).

> **OPEN:** (Q3) What each set is for beyond being a membership list (for example whether
> WebStore lists the store's purchasable records). The member classes are store,
> big-battle and trait records described by other specs.

<a id="s3-4"></a>
### 3.4 Manager entities (Confirmed, GC8; 14 of 14)

Fourteen labels name **entity** objects whose root class has the **manager base** (class
788 here, §2.1 row 8) as its parent: PVPManager, GuildManager, FellowshipManager,
FellowshipMakerManager, MailManager, AuctionManager, HousingAuthority,
CharacterSheetManager, PopulationTrackingManager, RepositoryManager, WorldJoinManager,
TrackingManager, ScriptManager and NeighborManager. Each has its own root class.

- All 14 property sets hold the same 36 properties with the same values except the name,
  `Name` (whose text is not reproduced; see the last item): `Entity_CreationType` = UniqueDynamicImmobile,
  `WeenieType` = Entity, `SeeMeVisibilityMask` = Hidden, `NeverHousekeep` = true,
  `IsSelectable` = true, `PhysObj` = one entity descriptor (0x47000601 here, the same in
  all), `Entity_AdditionalScriptTables` = one entry naming 0x07000000, `Usage_Action` and
  `Usage_Action_Shadow` = 0x70000000 (the UsageAction_DoNothing record), usage distances 2,
  and zeroed copies of a handful of combat and big-battle properties.
- RepositoryManager carries 12 more properties, all ints, each the version of one server
  repository. Ten are labelled `<repository>_VersionNumber`: `PaperItemRepository` 1,
  `GuildEventRepository` 1, `TraitConfigurationRepository` 3,
  `TraitConfigurationCustomSlotNameRepository` 2, `TraitConfigurationQuickslotsRepository`
  1, `GuildRepository` 5, `GuildMemberRepository` 1, `GuildHistoricalRecordRepository` 1,
  `GuildCalendarRepository` 1 and `GuildCalendarRSVPRepository` 1. The other two are
  `PVP_Leaderboard_TableVersion` 1 and `NeighborhoodPersistentNPC_RepositoryVersion` 7.
- The object state is 63 bytes: one script object with three reference attributes, all
  null.
- **The template alone does not identify a manager.** Three other entities of the map,
  WorldEventServer, PrivateEncounterServer and RipCordPersistentUnique (root classes 1811,
  1367 and 3168 here; their parent is the entity class 1733 here, not the manager base),
  hold the same 36 properties with the same values and a 63-byte object state too;
  their only difference is `PhysObj` (0x47000687 here, the same in the three). They are
  "other entities" (§2.1 row 9) and are not covered by this spec. A reader that needs the
  manager kind uses the parent-class rule of §2.1.
- **The name property is `Name`** (StringInfo; PID 0x00000354 here; GC8, OBS2). Among the
  13 managers other than RepositoryManager it is the only property whose value differs (13
  distinct values); every one of the 17 entities of this section (the 14 managers and the
  three server entities) holds exactly one textual property, and it is `Name`. A reader
  looks the property up by the label `Name` through the dictionary, not by the PID.
  (Answers the implementer note of 2026-10-08.)

They are hidden, never-housekept server-side entities, one per service. A client has no
use for them. A Tessera server may treat each label as the identity of the matching
service (Decision for M7, not a fact about the original).

---

<a id="s4"></a>
## 4. PvP and monster play

All of §4 is server rule data unless marked otherwise.

<a id="s4-1"></a>
### 4.1 PVPControl (Confirmed, GC9)

| Property | Type | Observed |
|---|---|---|
| `PVPControl_Hero_MaxRank` | int | 10 |
| `PVPControl_Account_MaxRank` | int | 25 |
| `PVPControl_Hero_EarnedRankXP_Table` | DID → class 849 | §4.10 |
| `PVPControl_Account_EarnedRankXP_Table` | DID → class 849 | §4.10 |
| `PVPControl_QuestHeroXPRewardList` | array of `PVPControl_QuestXPRewardEntry` (5) | each: `PVPControl_QuestReward_Tier` (enum, mapper 0x23000107: Tier_1–Tier_5) and `PVPControl_QuestReward_XPAmount` (int; 100–500 in steps of 100) |
| `PVPControl_QuestAccountXPRewardList` | same (5) | amounts 10–50 in steps of 10 |
| `PVPControl_TeamScoreProperty_Array` | 2 structs | `PVPControl_Team` (mapper 0x23000378 PETeamNumber: Team1, Team2) and `PVPControl_TeamScoreProperty_ScoreProperty` (PID of a world property holding the team's score) |
| `PVPControl_MaxScoreProperty` | PID | the world property holding the score limit |
| `PVPControl_HeroSessionList` | array of `PVP_HeroDID` (1) | → class 1930 (an object with `PVP_AvailableForSelection` and `PVP_HeroAlignment`) |
| `PVPControl_Lobby_AdminAvatar`, `PVPControl_Lobby_PlayerAvatar` | DID → class 1653 / 1654 | the two avatar templates that carry `PVP_IsPVPLobbyPlayer` (`creatures.md` §2.1, classes 1653 and 1654) |

<a id="s4-2"></a>
### 4.2 PVPVirtueControl (Confirmed, GC9)

| Property | Type | Observed |
|---|---|---|
| `PVPVirtueControl_Virtue_Array` | array of `PVPVirtueControl_Virtue` (31) | DIDs → class 3564 (virtue traits, `combat-data.md` §8.1); 31 distinct of the 45 class-3564 objects |
| `PVPVirtueControl_PVPVirtueSlotSchedule` | DID → class 1490 | a slot schedule |

The slot-schedule objects (class 1490 here, 15 objects, also named by
`Trait_SlotSchedule_Schedule` 516 times) are **compiled-only**: their property sets are
empty (6 bytes) and the object state holds a root object with three reference attributes,
each to a native counted list of u32 (class 17). In the PvP schedule the three lists hold
0, 6 and 6 values (all 1).

> **OPEN:** (Q4) The meaning of the three lists of a slot schedule (attribute names are
> hashes only). The trait-slot rules belong to `combat-data.md` §8; this spec records the
> layout.

<a id="s4-3"></a>
### 4.3 PVPCombatControl (Confirmed, GC9)

Eight DIDs, each → a float progression (class 1385, `combat-data.md` §4):
`PVPCombatControl_FloatProgression_` + `BaseAvoidanceChance`, `BaseAvoidanceMitigation`,
`BaseCriticalHitChance`, `BaseCriticalHitMagnitude`, `MasteryMultiplier_Physical`,
`MasteryMultiplier_Tactical`, `Mitigation_Physical`, `Mitigation_Tactical`. The control's
root class has the same parent as the combat control (§5.7).

<a id="s4-4"></a>
### 4.4 PVPFeatControl (Confirmed, GC9)

| Property | Type | Observed |
|---|---|---|
| `PVPFeatControl_Feat_Array` | array of `PVPFeatControl_Feat` (10) | skill DIDs (class 827) |
| `PVPFeatControl_RequiredComponent` | DID → item (class 796) | |
| `PVPFeatControl_MaxComponentsPerMatch`, `PVPFeatControl_FreeSubscriberComponents`, `PVPFeatControl_FreeNonSubscriberComponents` | int | 6 each |

<a id="s4-5"></a>
### 4.5 GloryControl (Confirmed, GC9)

| Property | Type | Observed |
|---|---|---|
| `GloryControl_InitialPVPRating` | float | 1000 |
| `GloryControl_PVPMaxRatingChange` | float | 50 |
| `GloryControl_PVPSpread` | float | 500 |
| `GloryControl_MaxPVPRange` | int | 80 |
| `GloryControl_KillTrackerCap` | int | 24 |
| `GloryControl_KillTrackerDecayInterval` | int | 300 |
| `GloryControl_StalePVPDamageTimeThreshold` | int | 20 |
| `GloryControl_DamageRatioToRecieveCombatEnemyDefeatEvent` (label spelled so) | float | 0.2 |
| `GloryControl_GloryMaxBonusPercentage`, `GloryControl_GloryRewardMultiplier` | float | 0.1 each |
| `GloryControl_PVPPlayerSessionPointsMultiplier`, `GloryControl_PVPMonsterPlayerSessionPointsMultiplier` | float | 0.5 each |
| `GloryControl_RankAdvancementTable`, `GloryControl_PrestigeAdvancementTable` | DID → class 849 | §4.10 |
| `GloryControl_GloryModifierTable` | DID → class 852 | §4.10 |
| `GloryControl_DiminishingReturnsModifierProgression`, `GloryControl_Glory_ContributorModifierProgression`, `GloryControl_PVPPoints_RankMultiplierProgression`, `GloryControl_PVPPoints_RewardProgression` | DID → class 1385 | progressions |
| `GloryControl_RenownRankNameList`, `GloryControl_InfamyRankNameList` | array of `GloryControl_NameEntry` (16 each) | each: `GloryControl_Name` (StringInfo) and `GloryControl_Value` (int); values 0–15, each once |
| `GloryControl_PrestigeNameList` | same (11) | values 0–10, each once |

The name lists map a rank number to its display name: find the entry whose value is the
rank.

Effects carry `Glory_AwardMultiplier` (59 effect objects, class 734); that is a property of
effects, not of this control.

<a id="s4-6"></a>
### 4.6 MonsterPlayControl (Confirmed, GC10)

| Property | Type | Observed |
|---|---|---|
| `MonsterPlayControl_MPUnlockLevel` | int | 2 |
| `MonsterPlayControl_MPLevelCap` | int | 160 |
| `MonsterPlayControl_PlayerLeftMonsterPlayArea_Timeout` | int | 360 |
| `MonsterPlayControl_MonsterPlayCap_FreepCountWorldProperty`, `…_FreepCapWorldProperty`, `…_CreepCountWorldProperty`, `…_CreepCapWorldProperty` | PID | four world properties: the current count and the cap of each side's population |
| `MonsterPlay_MLT_MonsterPlaySessions`, `MonsterPlay_Default_Unlocked_MLT_MonsterPlayers` | array | empty |

<a id="s4-7"></a>
### 4.7 MPAdvancementControl (Confirmed, GC10)

| Property | Type | Observed |
|---|---|---|
| `MPAdvancementControl_TraitCostArray` | array of `MPAdvancementControl_TraitCostEntry` (331) | each: `MPAdvancementControl_Trait` (DID → class 1477 in 320, 1483 in 11; all distinct) and `MPAdvancementControl_TraitCost` (int 50–10,000; 17 distinct values, 1,000 in 140 entries and 500 in 91) |
| `MPAdvancementControl_VirtueInfoArray` | array of `MPAdvancementControl_VirtueInfoEntry` (13) | each: `MPAdvancementControl_Virtue` (DID → class 1494; 13 distinct), `MPAdvancementControl_VirtueCostProgression` and `MPAdvancementControl_VirtueRankRequirementProgression` (DID → class 1385; 4 distinct each) |
| `MPAdvancementControl_ModStatTypeArray` | 4 structs | `MPAdvancementControl_ModStatType` (mapper 0x23000070, ExaminationModStatType) → `MPAdvancementControl_AdvancementPoints` (int): Swarm 0, Normal 10, Signature 20, Elite 34 |
| `MPAdvancementControl_UnlockCharacteristicValue`, `…_UnlockTraitValue`, `…_UnlockVirtueValue`, `…_UnlockSkillValue` | int | 1, 1, 2, 5 |

<a id="s4-8"></a>
### 4.8 MPLevelTableDirectory and the level directories (Confirmed, GC10)

MPLevelTableDirectory holds three arrays of `AdvTable_MLT` DIDs, all → monster-play tables
(class 1932, `combat-data.md` §9.2): `AdvTable_EvilMLTList` (8), `AdvTable_NeutralMLTList`
(4) and `AdvTable_GoodMLTList` (1). The 13 are distinct; class 1932 has 67 objects.

MonsterPlayLevelDirectory and PvPPlayLevelDirectory are empty markers (§3.2).

<a id="s4-9"></a>
### 4.9 PvP and monster-play actor properties (Confirmed, GC30)

The dictionary families of these names are mostly **actor properties**, not control
fields: they are defined on creature and player templates (`creatures.md`) and changed by
effects. Counts of definitions whose label begins with the family name and an underscore,
and how many of them hold a value in at least one set of the install:

| Family | Defined | Used | Where the used ones live |
|---|---|---|---|
| `MonsterPlay_` | 108 | 73 | monster-play tables (1932), sessions (1930), the 38 class-1346 objects, one class-1351 record, MonsterPlayControl |
| `PVP_` | 37 | 15 | avatar templates 1653/1654 (event team, safe zone, lobby flag), PVPControl, the hero object (1930), RepositoryManager (a leaderboard version), effects, and two rank-XP mods on every creature template |
| `PVPCombat_` | 70 | 62 | every creature and player template (35 rating, penetration, multiplier and immunity pairs with `_Cached` twins), effects (class 734) and progressions (class 1389) |
| `PvP_` | 3 | 1 | classes 1653 and 1654 |

<a id="s4-10"></a>
### 4.10 Compiled-only tables used by these controls (Confirmed layout, GC11)

Two small classes hold their data only in the object state (empty property sets):

| Class (here) | Objects | Object state | Observed |
|---|---|---|---|
| 849 | 29 | root object with one reference attribute (hash 0x0FF518EC) to a native counted list of u64 (class 104, `array_u64`) | 11 to 201 values; the first value is 0 and the list is **non-decreasing** in all 29 |
| 852 | 3 | root object with one reference attribute (hash 0x0B4CCFD2) to a native hashed map of u32 keys to u32 values (class 11, `map_u32`) | the values are float bit patterns (for example 0x3F800000 = 1.0), all finite (0.28–24.0); each of the three maps has 24 keys |

Class 849 records are named by `PVPControl_*_EarnedRankXP_Table`,
`GloryControl_RankAdvancementTable`, `GloryControl_PrestigeAdvancementTable`,
`Mounted_Control_SteedXP_To_Level_Table` (§5.3.1), `Reputation_Faction_AdvancementTable`
(`quests.md`), `Allegiance_Advancement_Progression` and item-advancement level tables.
Class 852 records are named by `GloryControl_GloryModifierTable`,
`Reputation_GroupModifierTable` and `XPGlobals_GroupXPModifierTable`.

Reading of class 849 (consistent with every table; the index rule is OPEN): value *i* is the
cumulative amount needed to reach rank or level *i*. Reading of class 852: key → multiplier.

> **OPEN:** (Q5) Whether index 0 or index 1 of a class-849 list is the first rank (many lists
> start with two zeros), and what the keys of a class-852 map count (fellowship size, rank).

---

<a id="s5"></a>
## 5. Mounts, default attacks and combat rules

<a id="s5-1"></a>
### 5.1 MountDirectory (Confirmed, GC12)

One property, `Mount_SkillList`: an array of 676 elements whose element label says which of
two body sizes the skill is for: `Mount_SkillToGrantTall` (347) or `Mount_SkillToGrantShort`
(329). All are skill DIDs (class 827), all distinct, and no skill is in both groups. The
stored order interleaves the two kinds.

The same two properties also occur on 377 items each (mount items that grant one skill per
body size). 102 of the 131 collection pieces (§6.2) are skills of this directory.

**Tall and short species** (Confirmed, GC12, OBS2). The size is not in this record but in
the race table (`skeleton-anim.md` §2.2): each of the eight species objects (the objects
that hold `RaceTable_Race`; 8 distinct species) holds the bool `RaceTable_IsTallRace` at the
top level of its property set, and no other property set of `client_gamelogic.dat` holds
it. Here it is true for Human, Elf, Beorning and HighElf and false for Hobbit, Dwarf,
DwarfStoutAxe and RiverHobbit (`SpeciesType` labels, mapper 0x2300000B). A reader takes the
flag from the race table at run time.

> **OPEN:** (Q6, narrowed 2026-10-08) That `RaceTable_IsTallRace` = true selects the
> `Mount_SkillToGrantTall` group and false the `…Short` group is a reading from the labels;
> no client or data link between the flag and the two groups has been read.

<a id="s5-2"></a>
### 5.2 MountAppearanceTraits (Confirmed, GC13)

A DID set (§3.3) of all 767 trait objects of class 3509 here (`combat-data.md` §8). All
767 have `Trait_Nature` MountedAppearances (key 16 of mapper 0x23000057),
`Trait_Category` Appearance (key 54 of mapper 0x2300001A), `Trait_Tier` 1 and
`Trait_Cost_To_Slot` 0.

**The slot of a trait.** Each trait names the mount slot it fills, **one level down**: its
property set has the top-level array `Trait_Groups` with exactly one element, and that
element is a `Trait_Group` (enum, mapper 0x230002EE, the TraitGroup mapper). No trait
carries `Trait_Group` at the top level, so a reader that looks only at top-level properties
finds no slot; it must take the element of `Trait_Groups`. (The same holds for every other
object that carries `Trait_Group`: 509 objects of five other root classes, none at the top
level.) The keys used are 8 to 14, the mount-slot keys of the mapper:

| `Trait_Group` key (label) | Traits | `Trait_Appearance_IsDyable` true | `Trait_Appearance_Material` non-zero | Slot-material property named by the hash (§5.3.1) |
|---|---|---|---|---|
| 8 (`Mount_Slot_Head`) | 152 | 129 | 30 | `Mount_Appearance_Material_Slot_Head` |
| 9 (`Mount_Slot_Body`) | 175 | 166 | 70 | `Mount_Appearance_Material_Slot_Body` |
| 10 (`Mount_Slot_Legs`) | 105 | 82 | 0 | `Mount_Appearance_Material_Slot_Legs` |
| 11 (`Mount_Slot_Tail`) | 54 | 54 | 0 | `Mount_Appearance_Material_Slot_Tail` |
| 12 (`Mount_Slot_Saddle`) | 153 | 10 | 0 | `Mount_Appearance_Material_Slot_Saddle` |
| 13 (`Mount_Slot_Gear`) | 118 | 5 | 0 | `Mount_Appearance_Material_Slot_Gear` |
| 14 (`Mount_Slot_Hide`) | 10 | 10 | 0 | `Mount_Appearance_Material_Slot_Hide` |
| total | 767 | 456 | 100 | |

The seven keys used by the traits are exactly the seven keys of MountedCombatControl's
`Mount_Control_Appearance_Group_To_Property_Hash` (§5.3.1), each once. So the data links a
trait to its slot (through the nested `Trait_Group`) and the slot to an actor property
(through the hash).

**Appearance fields of a trait** (top level, all 767 unless stated):

| Property | Type | Observed |
|---|---|---|
| `Trait_Appearance_Material` | enum, mapper 0x230000F3 | 0 in 667; non-zero only in head and body traits (table above), 8 distinct keys |
| `Trait_Appearance_IsDyable` | bool | table above |
| `Trait_Appearance_DefaultColour` | enum, mapper 0x23000398 (AppearanceColor) | 0 in 761; non-zero in 6 body traits (2 distinct keys) |
| `Trait_Appearance_ScriptID` | enum, mapper 0x23000097 | 0 in 734; non-zero in 33 |
| `Trait_Appearance_List` | array | exactly 2 elements in all 767, each a `Trait_Appearance_Set` (array) holding exactly one `Trait_Appearance_Struct`; the struct's fields are `Trait_Appearance_APR_Key` and `Trait_Appearance_ColorAPR_Key` (enum, mapper 0x2300008A), `Trait_Appearance_APR` and `Trait_Appearance_ColorAPR` (DID of prefix 0x20, appearance records, `setup.md` §3.3) and `Trait_Appearance_APR_Value` (float); each field occurs in all 767 traits |
| `Trait_BundleSkill_Array` | array of `Trait_BundleSkill` | 369 traits |

The seven slot-material properties named by the hash are enums of the **same mapper**
(0x230000F3) as `Trait_Appearance_Material`, and none of them occurs in any object property
set or bare set of the install: they are values an actor holds at run time. Reading
(consistent with the shared mapper, not stated by the data): applying a trait sets the
actor's slot-material property of the trait's slot to the trait's
`Trait_Appearance_Material`.

> **OPEN:** (Q23, narrowed in the second revision) The slot link is in the data (above).
> What remains open is how a trait's appearance is applied to the mount: whether the
> reading above is the rule, what the material key 0 means, how the two appearance sets
> of `Trait_Appearance_List` and their keys and float values are combined with the mount's appearance (`appearance.md`), and what
> `Trait_Appearance_ScriptID` selects.

> **OPEN:** (Q29) Which dye rule wins. `Mount_Control_Slots_Allowing_Dye` (§5.3.1) lists the
> slots 8, 9, 10, 11 and 14, but 10 saddle traits and 5 gear traits (slots 12 and 13) have
> `Trait_Appearance_IsDyable` = true. Whether a trait in a slot that the control does not
> list can be dyed is not in the data.

<a id="s5-3"></a>
### 5.3 MountedCombatControl

<a id="s5-3-1"></a>
#### 5.3.1 Layout (Confirmed, GC14)

| Property | Type | Observed |
|---|---|---|
| `Mount_Control_Speed_Multiplier_Walk`, `…_Trot`, `…_Canter`, `…_Gallop` | float | 0.5, 0.7, 0.8, 1.0 |
| `Mount_Control_Turn_Rate_Multiplier_Walk`, `…_Trot`, `…_Canter`, `…_Gallop` | float | 1.0, 1.0, 0.55, 0.47 |
| `Mount_Control_Turn_Rate_AddMod_Property_Walk`, `…_Trot`, `…_Cantor` (label spelled so), `…_Gallop` | PID | each names an actor property (`Mount_Control_Turn_Rate_AddMod_<gait>_ForwardSource`) |
| `Mount_Control_NonMountedCombat_MaxSpeed_Mulitplier` (label spelled so) | float | 0.75 |
| `Mount_Control_LeashDistance`, `Mount_Control_MaxLeashDistance` | float | 250, 500 |
| `Mount_Control_WithinLeashDistance_Timeout`, `Mount_Control_OutsideLeashDistance_Timeout` | float | 120, 60 |
| `Mount_Control_SteedXP_Percent_Mounted`, `…_UnMounted` | float | 1.0, 0.3 |
| `Mounted_Control_SteedXP_To_Level_Table` | DID → class 849 | §4.10 |
| `MountedCombat_Control_MountLevelToTotalPointsProgression`, `MountedCombat_Control_TraitPointsToRefundCostProgression` | DID → class 1385 | progressions |
| `Mount_Control_Mounted_Combat_Unlocked_Trait` | DID → class 1477 | a trait |
| `Mount_Control_Tree_Array` | 3 `Mount_Control_Tree_Struct` | per `Mount_Control_Trait_Tree` (mapper 0x23000369: MountedCombat_Light, _Medium, _Heavy): `Mount_Control_Tree_Effect` (DID → class 718) and `Mount_WarSteed_SkillCombo` (bitfield32, mapper 0x23000387: Light, Heavy, or none for Medium) |
| `Mount_Control_Tree_To_Skill_Hash` | 3 `Mount_Control_Tree_To_Skill_Struct` | per trait tree (`Mount_Control_Trait_Tree`, as in the tree array): `Mount_Control_Mount_Skill_Hash`, 8 structs of `Mount_Control_Player_Race` (species mapper 0x2300000B) → `Mount_Control_Mount_Skill` (skill DID); 24 entries, 12 distinct skills |
| `Mount_Control_FlankConstraints_Array` | 10 structs | `Mount_Control_FlankPosition` (bitfield32, mapper 0x23000372: Right, Left, Back, Front, the four diagonals, Front180, Back180; each once), `Mount_Control_Flank_Heading` (0–315 degrees), `Mount_Control_Flank_LeftArc` (−90 to 0) and `…_RightArc` (0 to 90), all three floats |
| `Mount_Control_Slots_Allowing_Dye` | 5 structs | `Mount_Control_Slot_Allowed_to_be_Dyed` (mapper 0x230002EE, TraitGroup: mount slots) and `Mount_Control_Colors_Allowed_to_be_Dyed` (any-width bitfield, mapper 0x23000398 AppearanceColor; 3 distinct masks) |
| `Mount_Control_Appearance_Group_To_Property_Hash` | 7 structs | `Trait_Group` (mount slot) → `Mount_Control_Apperance_Trait_Material_PropertyName` (PID of the actor property that holds that slot's material) |

The flank constraints read as: a position covers the arc from heading + left arc to heading
+ right arc, in degrees relative to the steed's facing (reading consistent with the values).

**Field types and the tree key** (GC14, OBS2; answers the implementer note of 2026-10-08):

- `Mount_Control_Flank_Heading`, `Mount_Control_Flank_LeftArc` and
  `Mount_Control_Flank_RightArc` are all **floats** (dictionary type float, default 0.0;
  float in all 10 flank structs). `Mount_Control_FlankPosition` is a bitfield32. A reader
  needs no int form of the flank fields.
- Both struct kinds name their tree by the same field, **`Mount_Control_Trait_Tree`** (enum,
  mapper 0x23000369): each `Mount_Control_Tree_Struct` holds it with
  `Mount_Control_Tree_Effect` and `Mount_WarSteed_SkillCombo`, and each
  `Mount_Control_Tree_To_Skill_Struct` holds it with `Mount_Control_Mount_Skill_Hash`, whose
  elements are `Mount_Control_Mount_Skill_Struct` structs of `Mount_Control_Player_Race`
  (enum) and `Mount_Control_Mount_Skill` (DID). Here each of the three tree keys
  (MountedCombat_Light, _Medium, _Heavy) occurs once in each of the two arrays.

> **OPEN:** (Q7) How the flank constraints are used (which attacks or effects test a flank
> position). Server rule; no native reader was found (§2.4).

<a id="s5-3-2"></a>
#### 5.3.2 Gait multipliers in the client (BIN, GC15)

The client's mounted movement code reads this control (resolved by key, §2.4). The gait is
a key of the enum mapper `MountedMovementSpeed` (0x2300033B here): 1 Backwards, 2 Neutral,
3 Neutral_Forwards, 4 Walk, 5 Trot, 6 Canter, 7 Gallop.

1. **Speed multiplier of a gait.** Gait 4, 5, 6 or 7 selects
   `Mount_Control_Speed_Multiplier_Walk`, `…_Trot`, `…_Canter` or `…_Gallop`. Any other gait
   gives **1.0**. If the selected property is absent from the control, or is not a float,
   the result is also 1.0.
2. **Turn rate of a gait.** Gait 5, 6 or 7 selects the Trot, Canter or Gallop pair; **any
   other gait, including 4 and the neutral gaits, selects the Walk pair.** The result is the
   pair's `Mount_Control_Turn_Rate_Multiplier_*` (1.0 if absent or not a float) **plus** the
   actor's current value of the property named by the pair's
   `Mount_Control_Turn_Rate_AddMod_Property_*`. When that PID is absent or 0, nothing is
   added; when the actor has no float value of the named property, 0 is added.
3. **Which gait.** For the turn rate the client takes the actor's `MountMovement_CurrentSpeed`
   (same mapper; Neutral when absent). For the speed limit it takes the actor's
   `MountedCombat_Current_LockedGear` (same mapper; Gallop when absent).
4. **Forward speed limit.** When the actor has `MountedCombat_Current_LockedGear`, the
   forward speed limit is the multiplier of that gear (step 1) times the controller's base
   forward-speed limit. When it has none, the limit is the smaller of the Gallop multiplier
   times the base limit and a second limit whose source was not identified (Q8).

Worked example in §11.2.

> **OPEN:** (Q8) The source of the second forward-speed limit of step 4, and the rest of the
> mounted movement controller (backward and sideways limits, gait transitions). These belong
> to `walker.md`; this spec gives only what the control contributes.

<a id="s5-4"></a>
### 5.4 TravelMountSpeedBoostOverride (Confirmed, GC32)

An **effect** object (class 734, `combat-data.md` §6), not a control:
`Effect_Duration_ConstantInterval` 3600, `Effect_Harmful` false, `Effect_UIVisible` false,
`Effect_RemovalOnlyInCombat` true, and a single `Mod_Entry` whose `Mod_Modified` is
`ForwardSource_Movement_MountSpeedMultiplier`, `Mod_Op` Multiply and value 1 here
(`combat-data.md` §5). Its equivalence class is `MTX_Mount_Speed_Increase`
(mapper 0x23000005). Read it as any effect.

<a id="s5-5"></a>
### 5.5 Weapon-style default attacks (Confirmed data, GC16)

Seventeen labels name skills (class 827, `combat-data.md` §7), all with
`Skill_IsDefaultAttack` = true: UNARMED, 1H, 1H_1H, 1H_Shield, 2H, BOW, CROSSBOW, JAVELIN,
their eight MOUNTED_ twins, and MOUNTED_RANGED_FALLBACK. They are the automatic attack of
each weapon style. The fields that tell them apart:

| Label | `Skill_AllowedRHImplement` | `…LHImplement` | `…RangedImplement` | `Skill_AnimationMode` | `Skill_MaxRange` | `Skill_SkillRecoveryTime` | Self behaviours |
|---|---|---|---|---|---|---|---|
| UNARMED | NonWeapon | AnyAndNone | AnyAndNone | Melee | 2.5 | 1.5 | three melee swing behaviours |
| 1H, 2H | WeaponButNone | AnyAndNone | AnyAndNone | Melee | 2.5 | 1.5 | the same three |
| 1H_1H | WeaponButNone | WeaponButNone | AnyAndNone | Melee | 2.5 | 1.5 | the same three |
| 1H_Shield | WeaponButNone | Shield | AnyAndNone | Melee | 2.5 | 1.5 | the same three |
| BOW, CROSSBOW | AnyAndNone | AnyAndNone | WeaponButNone | Ranged | 25 | 1.5 | one missile behaviour three times |
| JAVELIN | AnyAndNone | AnyAndNone | WeaponButNone | Thrown | 25 | 1.5 | one thrown behaviour |
| MOUNTED_ twins | as their foot twins | | | as foot | 2.5 (melee) or 40 (ranged, thrown) | 0 | as foot, plus a mounted list |
| MOUNTED_RANGED_FALLBACK | WeaponButNone | AnyAndNone | AnyAndNone | Melee | 40 | 0 | one mounted ranged behaviour |

Behaviours are keys of the script-behaviour mapper (`Skill_Behavior`, read as any skill's).

- **1H and 2H differ only** in two critical-modifier PIDs of the attack hook; BOW and
  CROSSBOW differ in one critical-hit PID, the super-critical multiplier (1.3 and 1.6) and a
  block-chance modifier. So the **label** is what selects the weapon style; the
  implement fields are the same for 1H and 2H.
- **Mounted twins** add `Skill_MountedCombat_Aware`, `Skill_MountedBehaviorSelfList` and
  `…OtherList` (one mounted swing or shot), `Skill_OnTrained_ShortcutPage` = MountedCombat,
  `Skill_UserEffectList` (one effect), `Skill_Allowed_Combat_States_Override`,
  vital costs and requirements for MountEndurance and MountPower besides Health and
  Power, against-mounted and against-unmounted damage multipliers (0 and 0.25 in all nine), and
  `Skill_AllowedWhileMounted` = true; they set `Skill_RequiresFacing` = false.
- MOUNTED_1H is also the default melee attack of one monster-play table (§2.3).

> **OPEN:** (Q9) The rule that picks the default attack from the equipped items and the
> mounted state (for example what decides 1H against 2H, and when MOUNTED_RANGED_FALLBACK
> applies). It is not in the data and no native reader of these labels was found (§2.4);
> it affects the client's stance and attack animation choice. Until settled, a Tessera
> client may choose by equipment category (a Decision, not a fact).

<a id="s5-6"></a>
### 5.6 WeaponDamageVarianceTable (Confirmed, GC17)

One property, `Item_WeaponVarianceTable`: three `Item_WeaponVarianceTableEntry` structs,
each a 64-bit equipment-category bitfield `Item_WeaponVarianceEquipmentCategory` (mapper
0x2300004C, EquipmentCategory) and `Item_WeaponVarianceValue` (float, **0.25** in all
three). The three masks group the one-handed weapons and shields, the two-handed weapons
(with orbs, javelins and battle gauntlets), and bows with crossbows.

> **OPEN:** (Q10) How the variance is applied to a weapon's damage (a spread of ±25 % or a
> minimum of 75 % of the maximum). Server rule.

<a id="s5-7"></a>
### 5.7 CombatControl (SPEC; BIN, GC18)

The combat control's tables are described in `combat-data.md` §10.1; its label is
`CombatControl`. Additions:

- `Combat_Control_DuelingBeginDelay` and `Combat_Control_LengthOfTheDuel` (floats; 10 and
  600 here) are read by the client's duel display. If either property is absent or not a
  float, the client uses **10** and **600** (BIN). Units: seconds (the 600 is a ten-minute
  duel; reading).
- Other scalars: `Combat_Control_MaxCombatLevel` 160,
  `Combat_MinRangeForConjunctionSuccessNotification` 20,
  `Combat_Control_InductionInterruption_CooldownTime` 4, `Skill_Action_Queue_Duration` 0,
  `Skill_Mounted_Action_Queue_Duration` 240, `Skill_Mounted_Maximum_MoveTo_Range` 240,
  `Combat_Control_MountedPlayer_HealthToMountEndurance_Conversion` 0.4,
  `Combat_Control_MountedNonPlayer_HealthToMountEndurance_Conversion` 0.

<a id="s5-8"></a>
### 5.8 CombatStateControl (Confirmed, GC17)

| Property | Type | Observed |
|---|---|---|
| `CombatState_Control_StateDataList` | 17 `CombatState_StateData` | one per combat state (below) |
| `CombatState_Control_Mounted_StateDataList` | 9 `CombatState_StateData` | the same for mounted actors |
| `CombatState_Control_NoImmunityStates` | bitfield32, mapper 0x23000050 (CombatState) | Recovering, None |
| `CombatState_ChainProtected_States` | bitfield32 | KnockedOut, ConjunctionStunned, KnockedDown, Stunned |
| `CombatState_ChainProtection_Duration` | float | 10 |
| `CombatState_ExitOnVitalLoss_GracePeriod` | float | 1 |

Fields of a `CombatState_StateData` (counts in the 17 / 9 entries):

| Field | Type | Present | Observed |
|---|---|---|---|
| `CombatState_StateData_State` | bitfield32 (CombatState) | 17 / 9 | one bit each; distinct within each list |
| `CombatState_StateData_ValidExitStates`, `…_DefaultExitState` | bitfield32 | 17 / 9, 16 / 9 | default exit None or Recovering |
| `CombatState_StateData_DefaultDuration` | float | 16 / 9 | 0.1–30 / 3–25 |
| `CombatState_StateData_ExaminationName` | StringInfo | 17 / 9 | |
| `CombatState_StateData_AIHints_Mask`, `…_AIHints_Set` | any-width bitfield | 16 / 9 | |
| `CombatState_StateData_FlagEffect` | DID → class 718 or null | 12 / 9 | |
| `CombatState_StateData_StateEffectList` | array of `EffectGenerator_EffectStruct` | 14 / 1 | effect DID and spellcraft (`combat-data.md` §6.3) |
| `CombatState_StateData_OnEnterStateEffectList` | array of `CombatState_StateData_OnEnterStateEffectStruct` | 0 / 8 | |
| `CombatState_StateData_DiminishingReturns_Struct` | struct | 7 / 9 | counter and start-time PIDs, a duration-mod progression (class 1385), `…_ResetInterval` (20 / 45) |
| `CombatState_StateData_DurationMultModProperty` | PID | 7 / 9 | |
| `CombatState_StateData_ExitOnVitalLossProbability`, `…_ExitOnHarmfulSkillProbability`, `…_BreakOnVitalLossProb_Min/_Max`, `…_ExitOnHarmfulSkillProb_Min/_Max`, `…_BreakOutOfState_GracePeriod_Min/_Max`, `…_NoBreak_ExitOnVitalLoss_Delay` | float | 4–6 / 4 | probabilities 0–1, periods 0–10 |
| `CombatState_StateData_ScriptID` | enum (script key) | 12 / 1 | |
| `CombatState_StateData_Announcement_FirstPersonBreak`, `…_FirstPersonResponsible`, `…_ThirdPersonBreak` | StringInfo | 4 / 5 | |

Server rule data, except that the state's examination name, flag effect and announcements
are what a client shows when it learns of a state change.

<a id="s5-9"></a>
### 5.9 ResistControl (Confirmed, GC17)

`Resist_Control_Array`: 12 `Resist_Control_Data` structs, one per resist category
(`Resist_Category`, bitfield32, mapper 0x23000064; each once). Each names six actor
properties by PID: `Resist_Control_Additional_Resistance`,
`…_ClassPoints_Resistance`, `…_StatPoints_Resistance`, `…_AdditionalPoints_Resistance`,
`…_Cached_TotalPoints_Resistance`, `…_Cached_Total_Resistance`. Eleven of the twelve also
carry `Resist_Control_Dependent_Resist_Category` = TheOneResistance; the twelfth is that
category itself. It is the resist counterpart of the damage-type list of the combat control
(`combat-data.md` §10.1).

<a id="s5-10"></a>
### 5.10 Mount families in the dictionary (Confirmed, GC30)

`Mount_` has 149 definitions, of which 96 hold a value somewhere (the control fields of
§5.3, mount items and skills, and actor properties on every creature template).
`MountedCombat_` has 18, of which 13 are used (two control progressions, actor properties
on templates). `MountPower_` (7) are actor properties (`MountPower_MaxLevel` and its
`_Cached` twin on every creature and player template).

---

<a id="s6"></a>
## 6. Account and store controls

All of §6 is server rule data, except where marked.

<a id="s6-1"></a>
### 6.1 Perks (Confirmed, GC19)

**PerkControl:**

| Property | Type | Observed |
|---|---|---|
| `PerkControl_ChannelData` | 28 `PerkControl_ChannelDataEntry` | each: `PerkControl_Channel` (mapper 0x23000078 PerkChannel; 28 distinct), `PerkControl_UICategory` (mapper 0x230001D3 PerkUICategory; 28 distinct), `PerkControl_Channel_Interval` (int: 18,000 ×14, 9,000 ×9, 86,400 ×4, 2,700 ×1), `PerkControl_Channel_Cap` (int: 5 ×26, 1 ×2) |
| `Perk_AvailablePerks` | array of `Perk_DID` (2) | perk DIDs |

**Perk objects** (47, class 1377 here):

| Property | Type | Present | Observed |
|---|---|---|---|
| `Perk_Name`, `Perk_Desc` | StringInfo | 47 | |
| `Perk_Icon` | DID → 0x41 surface | 47 | 30 distinct |
| `Perk_Effect` | DID → effect | 47 | class 734 (44), 714, 715, 758 (1 each); 46 distinct |
| `Perk_Channel` | enum (PerkChannel) | 47 | 17 distinct; every one is a channel of the control |
| `Perk_UICategory` | enum (PerkUICategory) | 47 | 17 distinct; every one is a category of the control |
| `Perk_SessionPointCost` | int | 47 | 10–8,000 |
| `Perk_MinLevel` | int | 46 | 5–51 |
| `Perk_UsableAtLevelCap` | bool | 1 | false |
| `WebStoreInventoryItem_DataID` | DID → class 2541 | 44 | |
| examination tooltip fields | | 47 | the generic perk examination layout |

References: the perk vendors (`economy.md` §6.4, 72 references) and the control's
`Perk_AvailablePerks` (2). The perk confirmation is the marker `Dialog_PerkConfirmation`
(§3.2).

> **OPEN:** (Q24) How perks are bought and limited. A reading consistent with the labels
> and the values, not a fact: a perk is bought with session points, applies its effect, and
> is limited per channel to *cap* uses per *interval* seconds. Server rule.

<a id="s6-2"></a>
### 6.2 Collections (Confirmed, GC20)

**CollectionControl**: `CollectionControl_CollectionList`, 29 DIDs → class 3996 (every
object of the class, each once).

**Collection objects** (29):

| Property | Type | Observed |
|---|---|---|
| `Collection_Name` | StringInfo | 29 |
| `Collection_Category` | enum, mapper 0x230003DE | Mounts 22, CosmeticPets 7 |
| `Collection_DID_PieceList` | array of `Collection_DID_Piece` | 2–9 pieces; 131 in all, all distinct, all skills (class 827) |
| `Collection_TreasureDID` | DID → class 1406 | the reward; 18 distinct |
| `Usage_Permission` | DID → class 1603 | the same record in all 29 |
| `Usage_RequiredRaces` | array of 4 species keys | 22 (the mount collections) |

The skills themselves carry the display data of a piece: `Collection_Piece_CustomCamera`
(enum, mapper 0x230000B9, a UI-scene camera; 1,319 skills), `Collection_Piece_SourceDesc`
(StringInfo, 1,319), `Collection_Webstore_Item` (DID, 1,319), `Collection_Piece_DisplayEntDID`
(756), `Collection_Piece_DisplayDID` (563) and `Collection_Hide_Entry` (bool, 1,347).

The data lists the pieces and the reward of a collection, but not when a collection counts
as complete. Reading (not in the data): a collection is complete when the character has
every piece skill. The completion test and the reward rule are server rules.

> **OPEN:** (Q30) The completion rule of a collection and when `Collection_TreasureDID` is
> granted; the data gives only the piece list and the reward.

<a id="s6-3"></a>
### 6.3 Paper-item services (Confirmed, GC21)

The paper-item control's item lists and version upgrades are in `items.md` §6.4. Its other
fields:

| Property | Type | Observed |
|---|---|---|
| `PaperItemServicesArray` | array of `PaperItemServiceEntry_ServiceDID` (35) | DIDs → seven root classes: 3937 (28), 3940 (2), 3939, 3942, 4043, 4046, 4047 (1 each); the last six derive from 3937 |
| `PaperItemControl_PVPCurrency` | DID → item (class 796) | |
| `PaperItemControl_WalletUpgrade_WebstoreItem` | DID → class 2784 | |

Fields of a service entry (counts over the 35):

| Property | Type | Present | Observed |
|---|---|---|---|
| `PaperItemServiceEntry_Type` | enum, mapper 0x230003A6 (PaperItemServiceType) | 35 | **35 distinct** keys between 1 and 38: one entry per service type |
| `PaperItemServiceEntry_PaperItemDID` | DID → item (class 796) | 35 | the currency paid; 3 distinct in class 3937 |
| `PaperItemServiceEntry_Cost_RipCordID` | DID → class 2801 | 35 | the cost record (a RIPCORDS record) |
| `PaperItemServiceEntry_Enabled_RipCordID` | DID → class 2801 | 5 | |
| `PaperItemServiceEntry_PurchaseDescription`, `…_PurchasedMessage` | StringInfo | 30 | |
| `PaperItemService_TemporaryServiceEffect` | DID → class 3943 | 1 (class 3942) | |
| `PaperItemService_InductionActionDID` | DID → class 1195 | 1 (class 4046) | |

The subclasses differ only by these optional fields and by their class identity.

> **OPEN:** (Q11) What each service does when bought (presumably selected by the service
> type and the subclass). Server rule.

<a id="s6-4"></a>
### 6.4 TurbinePointsControl (Confirmed, GC22)

`TurbinePointsControl_TurbinePointEntryList`: 6 structs of
`TurbinePointsControl_TurbinePointTier` (mapper 0x23000107: Tier_0 … Tier_5) →
`TurbinePointsControl_TurbinePointAmount` (int): 25, 50, 100, 250, 500, 1,000. A reward of
store points by tier; server rule.

<a id="s6-5"></a>
### 6.5 RewardTrackControl (Confirmed, GC22)

`RewardTrackControl_RewardTrack_Array`: 27 DIDs → class 4224 (every object of the class).
A reward track (class 4224) holds: `Name`, `Description`; `RewardTrack_ExperienceType`
(ItemAdvancement 19, MonsterPlayRewards 8); `RewardTrack_Array` of 40, 51 or 101
`RewardTrack_Array_Entry` structs (2,466 in all; `…_Reward` → class 2814, `…_PipElement`,
`…_ExperienceCostMultiplier` 1–2.5); a reward-interval progression (class 1389); PIDs of the
character properties that hold its experience, milestone, goals and claimed milestones and
of the world property that switches it on; bonus-experience PIDs; a UI layout key; and a
usage-permission struct (minimum level 45, world-event condition). The reward items are
counted in `items.md` and the experience sources in `quests.md`.

<a id="s6-6"></a>
### 6.6 RandomItemRewardControl (Confirmed; reader BIN, GC22)

| Property | Type | Observed |
|---|---|---|
| `RandomItemReward_Silver_Category`, `RandomItemReward_Gold_Category` | array of 5 `RandomItemReward_Tier_Entry` (struct) | each: `RandomItemReward_Tier_Title` and `RandomItemReward_Tier_Description` (StringInfo), `RandomItemReward_Tier_NPC` (DID → class 1724, an NPC template), `RandomItemReward_Tier_NPC_Icon` (DID, 0x41 surface) and `RandomItemReward_Tier_VXP` (DID → item, class 796); all five fields in all 10 entries |
| `RandomItemRewardControl_ChanceToShowRarest` | float | 0.1 |

**Client use (BIN):** the login-reward window resolves this control by key (§2.4). It takes
the PID stored in a login-reward record's `LoginReward_ControlCategory_PropertyName`
(§6.7), reads that array from this control, and for each tier entry shows the title and
description and uses the NPC, NPC icon and reward item.

The tier-field labels are spelled out in the table above (GC22, OBS2). The prefix of the
tier fields is `RandomItemReward_`, not `RandomItemRewardControl_`; only the rarest-tier
chance carries the longer prefix. (Answers the implementer note of 2026-10-08.)

> **OPEN:** (Q12) The order in which the client lists the tiers and what
> `RandomItemRewardControl_ChanceToShowRarest` drives (a display choice or a server roll).

<a id="s6-7"></a>
### 6.7 LoginRewards (Confirmed, GC22)

LoginRewards is a DID set (§3.3) of the two login-reward objects (class 3731 here):

| Property | Type | Observed in the two |
|---|---|---|
| `LoginReward_ResetSchedule` | enum, mapper 0x230003AD | Daily; Weekly (see §7.4) |
| `LoginReward_AllowedAccountTypes` | any-width bitfield, mapper 0x23000307 | five account types; two subscriber types |
| `LoginReward_AlertState` | enum (UI state, mapper 0x230000D2) | two distinct alert states |
| `LoginReward_ControlCategory_PropertyName` | PID | `RandomItemReward_Silver_Category`; `RandomItemReward_Gold_Category` (§6.6) |
| `LoginReward_TreasureTableArray` | array of 3 `LoginReward_TreasureTable` | DIDs → weighted tables (class 3307, `economy.md`) |
| `LoginReward_TreasureTableIndex_WorldProperty` | PID | a world property selecting which of the three tables applies |
| `LoginReward_XPProgressionsProgression` | DID → class 3810 | a progression of progressions (`combat-data.md` §4.5) |
| `LoginReward_XPMultiplier_RipCord`, `LoginReward_RerollCostOverride_RipCord` | DID → class 2801 | |

---

<a id="s7"></a>
## 7. UI and system rules

<a id="s7-1"></a>
### 7.1 Vital-display overrides (Confirmed, GC23)

**gmVitalDisplayOverrideControl**: `VitalDisplayOverride_Array`, 113
`VitalDisplayOverride_Array_Entry` DIDs → class 3463 (every object of the class, each once).

**Override objects** (113):

| Property | Type | Observed |
|---|---|---|
| `VitalDisplayOverride_RequiredEnum` | enum, mapper 0x2300036A | **113 distinct** keys 1–113: every non-zero key of the mapper once |
| `VitalDisplayOverride_UIState` | enum, mapper 0x230000D2 (UIStateID) | 113 distinct; one is the plain `normal` state |
| `VitalDisplayOverride_CSM_Name` | StringInfo | the name shown in a chooser |
| `VitalDisplayOverride_IsForFreepCharacters`, `…_IsForMonsterCharacters`, `…_IsForPvPCharacters` | bool | (true, false, false) 73; (false, true, false) 28; (true, true, false) 12; PvP false in all |

**Unlocks**: 111 items (class 796) carry `VitalDisplayOverride_UnlockedVDOs`, an any-width
bitfield over the same mapper with **exactly one bit set** in each. By the bit-to-key rule
the bit names one override key; the 111 items unlock 111 distinct keys, every key except 1
and 2, which no item unlocks. Why keys 1 and 2 have no unlocking item is not in the data
(Q25).

Data navigation: an item's set bit *b* gives key *b* + 1; the override whose
`RequiredEnum` is that key gives the UI state to apply to the character's vital display,
for the character kinds whose flag is true.

**Where the unlock sits on an item** (GC23, OBS2; answers the implementer note of
2026-10-08). `VitalDisplayOverride_UnlockedVDOs` is never a top-level property. In each of
the 111 items it is the **single element** of the top-level array
`Usage_SetPropertyUsageData_Array` (depth 1, directly an element of the array, not inside a
struct). A client finds it by taking that array from the item's property set and the
element whose label is `VitalDisplayOverride_UnlockedVDOs`.

- In all of `client_gamelogic.dat`, 112 property sets hold
  `Usage_SetPropertyUsageData_Array`, always at the top level: the 111 unlock items (one
  element each) and one set whose single element is a quest property. So the array is a
  general list of property values, not a VDO-specific field; a reader takes the element by
  its label.
- The 111 items share their usage settings: the same usage-action record (as
  `Usage_Action` and `Usage_Action_Shadow`), `Usage_DestroyOnUse` = true,
  `Usage_InventoryRequired` = true, `Usage_AllowedInCombat` = false; 110 of them have
  `Usage_MonsterPlayerCanUse` = true.
- No property set of `client_gamelogic.dat` holds `VitalDisplayOverride_UnlockedVDOs` anywhere else
  (no creature or player template carries it). Reading (from the labels, not stated by the
  data): using such an item sets the array's property values on the user, so a character's
  unlocked overrides would be a run-time value of the same property (Q13, Q25).

> **OPEN:** (Q13) Where the chosen override key is stored on the character, and how the UI
> state changes the vitals display (frame art, colours). The overrides are not read by
> native code by constant ID (§2.4). Until settled, a Tessera client uses the plain state.

> **OPEN:** (Q25) How the overrides of keys 1 and 2 become available, since no item
> unlocks them (granted by default, granted by other means, or unused).

<a id="s7-2"></a>
### 7.2 Confirmation-dialog markers

The 38 `Dialog_*` labels are empty markers (§3.2). They carry no text, layout or button
data; the confirmation dialogs' layouts are UI layouts (`ui-layout.md`).

<a id="s7-3"></a>
### 7.3 AlertsControl (Confirmed, GC24)

| Property | Type | Observed |
|---|---|---|
| `AlertControl_MaxNumberOfBigAlerts` | int | 5 |
| `AlertControl_TypeToOverflow_Array` | 64 `AlertControl_TypeToOverflow_Struct` | `Alert_Type` (mapper 0x23000033, Alert) and `Alert_IsOverflow` (bool: true 59, false 5). The 64 types are **every non-zero key** of the mapper, each once |

Reading: at most five alerts are shown large; an alert type flagged as overflow goes to the
overflow list when that limit is reached. Not read by native code with constant IDs (§2.4).

> **OPEN:** (Q14) The exact overflow behaviour and what happens to the five non-overflow
> types when the limit is reached.

<a id="s7-4"></a>
### 7.4 ResetScheduleControl (Confirmed, GC25)

`ResetScheduleControl_ResetScheduleArray`: 3 `ResetScheduleControl_ResetScheduleEntry`
structs of `ResetScheduleControl_ResetSchedule` (mapper 0x230003AD: BiWeekly, Daily,
Weekly) and `ResetScheduleControl_ResetDays` (bitfield32, mapper 0x230003AE DayOfWeek:
1 Sunday … 7 Saturday):

| Schedule | Days bitfield | Days (bit-to-key rule) |
|---|---|---|
| Daily | 0x7F | all seven |
| BiWeekly | 0x12 | Monday, Thursday |
| Weekly | 0x10 | Thursday |

"BiWeekly" is therefore twice a week. The schedule names are also the values of
`LoginReward_ResetSchedule` (§6.7). Worked example in §11.3.

> **OPEN:** (Q15) The time of day and time zone of a reset. Not in the data.

<a id="s7-5"></a>
### 7.5 ConsiderationControl and the consideration tables (Confirmed data, GC26)

**ConsiderationControl:**

| Property | Type | Observed |
|---|---|---|
| `ConsiderationControl_RankToUIState` | 9 `ConsiderationControl_RankToUIStateEntry` | `ConsiderationControl_ConsiderRank` (mapper 0x23000072 ConsiderRank: Trivial 10, InsanelyEasy 20, VeryEasy 30, Easy 40, Even 50, Tough 60, VeryTough 70, InsanelyTough 80, Impossible 90) → `ConsiderationControl_UIStateID` (UIStateID mapper; the nine `Con<rank>_ColorName` states) |
| `ConsiderationControl_CombatTableData` | 6 `ConsiderationControl_CombatTableDataEntry` | `ConsiderationControl_ConsiderType` (mapper 0x2300000F: MonsterPlayer, Monster, Victim, Guard, Player, DelvingMonster; each once) and `ConsiderationControl_CombatTableList`, **120** DIDs → class 1694 |
| `ConsiderationControl_QuestTableList` | **120** DIDs → class 1697 | |

The 120-element lists are runs of a few tables: for Player and MonsterPlayer, elements 1–6,
7–39 and 40–120 name three tables; Monster, Victim, Guard and DelvingMonster share three
other tables with the same breaks. The quest list names one table for elements 1–39 and
another for 40–120.

**Consideration tables** (6 of class 1694, 2 of class 1697): one property,
`ConsiderationTable_RankPerDifferentialArray`, **31** `ConsiderationTable_ConsiderRank`
values (ConsiderRank keys). In all eight tables the sequence is **non-decreasing**, starts
at Trivial, ends at Impossible, and element 16 (index 15 counting from 0) is **Even**.

Data-consistent reading (OPEN, Q16): the list element is chosen by the viewer's level
(element *L* for level *L*), and the table element by the level difference *d* = target
level − viewer level as index *d* + 15, clamped to 0–30. The rank then maps to the
colour state through `RankToUIState`. Worked example in §11.1.

> **OPEN:** (Q16) The index rules: which level selects the list element (viewer's or
> target's), how levels above 120 are handled (the level cap is 160, §4.6), whether *d* is
> clamped, which consider type applies to which target, and where quest tables are used.
> Not read by native code with constant IDs (§2.4). This is visible behaviour (name
> colours); M3 observation could test the reading.

<a id="s7-6"></a>
### 7.6 Formations (Confirmed, GC27)

**DistantBattleUnitFormationControl:** `DistantBattleUnit_FormationControl_SpacingBetweenGroups`
(float, 2) and `DistantBattleUnit_FormationControl_Width_Lookup`, 7 structs of
`DistantBattleUnit_Formation_Width` (enum, each once) →
`DistantBattleUnit_FormationControl_ActualWidth` (float): Wide80 80, Wide 30, Normal 15,
Thin 10, Thin5 5, Fill 160, Scrum 10.

**Formation-layout objects** (not labelled; named by
`DistantBattleUnit_Formation_LayoutCalculator` in distant-battle unit records, 267 + 35
references to the grid kind and 109 + 18 to the scatter kind). Both classes derive from
one parent class (3756 here):

| Kind (class here) | Objects | Properties |
|---|---|---|
| Grid (3757) | 10 | `FormationLayout_Grid_DistanceBetweenUnits` (float 1.2–30), `FormationLayout_Grid_DistanceBetweenRanks` (float 1.5–30) |
| Scatter (3856) | 20 | `FormationLayout_Scatter_MinDistance` (float 0.7–25), `FormationLayout_Scatter_MaxDistance` (float 1.5–60), `FormationLayout_Scatter_ClusterConstant` (int 3–200) |

The NPC formation groups of `creatures.md` §5.6 are a different record family.

> **OPEN:** (Q17) The placement algorithms of the grid and scatter layouts and the unit of
> the distances (world units, `coordinates.md`, is the natural reading).

<a id="s7-7"></a>
### 7.7 AggroPingControl (Confirmed, GC28)

`AggroPingControl_AggroPingFrequency` (float, 17) and
`AggroPingControl_ChallengeLevelWindow` (int, 15).

> **OPEN:** (Q18) What an aggro ping is (a periodic check that wakes nearby creatures, read
> from the name) and the units (seconds and levels are the natural reading).

<a id="s7-8"></a>
### 7.8 CombatMusicControl (Confirmed, GC28)

`CombatMusicControl_Floor` (int, −5), `CombatMusicControl_Ceiling` (int, 15) and
`CombatMusicControl_ModArray`: 7 `CombatMusicControl_ModEntry` structs of
`CombatMusicControl_ModStatEnum` (mapper 0x23000070, ExaminationModStatType) →
`CombatMusicControl_ModStatAddModifier` (int): Swarm −1, Normal 0, Signature 2, Elite 4,
EliteMaster 6, Nemesis 9, ArchNemesis 11.

The client's combat-music switch itself is a script driven by `Combat_Violent`
(`sound.md` §10.6) and does not use these numbers.

> **OPEN:** (Q19) The formula: presumably an intensity summed from the opponents' ranks
> (with level differences) and clamped to floor–ceiling, selecting a music intensity. Who
> computes it is OPEN (§2.4).

<a id="s7-9"></a>
### 7.9 DiscoveryDistanceControl (Confirmed, GC28)

`DiscoveryDistanceControl_DistanceEnum_Array`: 4 structs of
`DiscoveryDistanceControl_DistanceEnum` (mapper 0x230001F4, DiscoveryDistance) →
`DiscoveryDistanceControl_DistanceThreshold` (float): VeryClose 20, Close 40, Distant 80,
VeryDistant 120. The mapper's fifth key, ExtremelyDistant, has no threshold (beyond 120).

> **OPEN:** (Q20) What is classified (distance to an undiscovered landmark or quest
> target is the natural reading) and the unit (world units).

<a id="s7-10"></a>
### 7.10 TrackingStateControl (Confirmed, GC28)

`TrackingStateControl_MapNoteToState_Array`: 17 structs of `TrackingStateControl_MapNote`
(bitfield64, mapper 0x230000D7 MapNoteType; one bit each, 15 distinct masks) →
`TrackingStateControl_TrackingState` (mapper 0x23000250 TrackingState; 17 distinct, every
non-zero key of the mapper once). The table maps a map note type to a tracking state.

> **OPEN:** (Q26) Which entities carry which map note type, and what a tracking state
> changes on the client (map or radar display). Not read by native code with constant IDs
> (§2.4).

<a id="s7-11"></a>
### 7.11 ShapeshiftControl (Confirmed, GC28)

`ShapeshiftControl_RedirectProperties`: 4 PIDs (`ShapeshiftControl_Redirect`): `Name`,
`AC_Color_Hair`, `AC_Texture_Scar` and one war-paint colour property.

> **OPEN:** (Q21) The redirect rule (presumably these properties are read from the
> original character instead of the shapeshifted form). Relevant to `appearance.md`.

<a id="s7-12"></a>
### 7.12 Pets (Confirmed, GC28)

**PetControl** and **MonsterPetControl** are the two objects of one class (3481 here). Each
holds `PetControl_ForwardedMasterProperty_Overrides`, an array of
`PetControl_ForwardedMasterProperty_Entry` structs with
`PetControl_ForwardedMasterProperty_MasterSource` and `…_PetTarget` (both PIDs): 47 entries
in PetControl, 1 in MonsterPetControl. In many entries source and target are the same
property.

**PetChannelControl**: `PetChannel_Control_ChannelList`, 29
`PetChannel_Control_ChannelStruct` structs of `PetChannel_Control_Channel` (mapper
0x23000058 PetChannel; 29 distinct including Undef), `PetChannel_Control_MaxCount` (int
1–50; 1 in 20), `PetChannel_Control_AlwaysSummon` (bool; true in 7) and, in 2,
`PetChannel_Control_AllowPetChannel` (PID of an actor property that enables the channel).
> **OPEN:** (Q27) How the pet tables are applied. Readings consistent with the labels and
> values, not facts: a pet's `…_PetTarget` property receives the master's `…_MasterSource`
> property; at most *MaxCount* pets are active per channel. When the forwarding happens
> (on summon, continuously) and what `AlwaysSummon` does are not in the data; which side
> applies the tables is part of Q1.

<a id="s7-13"></a>
### 7.13 PruneInfo_Skill (Confirmed, GC28)

`PruneInfo_SimplePropertyArray`: 55 PIDs (`PruneInfo_PropertyEntry`) of properties that
skills carry: 52 `Skill_*`, two `Pip_CashOut_*` and `Collection_Piece_CustomCamera`.
`PruneInfo_ArrayPropertyArray`: 42 `PruneInfo_ArrayPropertyEntry` structs naming a skill
array property (`PruneInfo_ArrayProperty`), its element property (`PruneInfo_ElementProperty`)
and in 11 a field of the element (`PruneInfo_SubElementProperty`).

> **OPEN:** (Q22) What pruning means here (properties dropped from skill records sent to or
> stored for a client is the natural reading). No client use is known.

<a id="s7-14"></a>
### 7.14 Whitelists (Confirmed, GC28)

The label NewbieCinematicEncounter names a private-encounter template (class 1368), which is
the subject of the encounter specs; its `PrivateEncounterTemplate_WhiteList` names one of
the four **whitelist** objects (class 3049 here; 17 references from encounter templates in
all). A whitelist holds `WhiteList_PermittedAlertArray` (`WhiteList_PermittedAlert`, Alert
keys; 0, 10, 11 or 20 entries) and `WhiteList_PermittedHintArray` (`WhiteList_PermittedHint`,
tutorial-hint type keys; 0, 5, 5 or 50 entries).

> **OPEN:** (Q28) What a whitelist does. A reading consistent with the labels, not a fact:
> inside an encounter that names a whitelist, only the listed alerts and tutorial hints are
> shown. Whether it filters on the client or the server, and what an empty list means, are
> not in the data.

<a id="s7-15"></a>
### 7.15 OpenTappingControl (Confirmed, GC28)

`OpenTapping_MaxCreditDistance` (float, 120): the distance within which a participant gets
credit for an open-tapped creature (`creatures.md`, `Tapping_OpenTapping`). Server rule.

<a id="s7-16"></a>
### 7.16 GRSControl (Confirmed, GC28)

Six tables that map gameplay keys to the PIDs of reporting properties (`GRS…`, `GRSEvent_…`):
`GRSDefensePoints_Array` (5: damage type → current and modifier PIDs),
`GRSResistancePoints_Array` (4: resist category → PIDs), `GRSItemSocketing_SocketDataProperties_Array`
(5 sockets: gem DID and gem level PIDs), `GRSLegacy_Array` (9 legacies: type, level, unlocks
PIDs), `GRSStaticEffect_Array` (14 PIDs), `GRSControl_TraitTreeBranchToGRSPropertyArray` (3
big-battle roles) and `GRSControl_QuestMedalToGRSPropertyArray` (4 medals). Reading (from
the property labels, not stated by the data): they feed server-side event reporting.

What the client does with the control is not settled. BIN: native code does not resolve
GRSControl by key (§2.4, GC4). Its 28 property IDs were not part of the property-ID scan of
§2.4; a byte scan of the executable (BIN q4) finds 4-byte matches of 9 of them, all in the
code section, which were not examined further, so a native reader by constant property ID
is neither shown nor excluded (Q1).

---

<a id="s8"></a>
## 8. Social managers (data side)

The manager entities of these services are §3.4. Social traffic is network behaviour and
out of scope.

<a id="s8-1"></a>
### 8.1 GuildControl (Confirmed; reader BIN, GC29)

| Property | Type | Observed |
|---|---|---|
| `Guild_MaxMembers` | int | 1,000 |
| `Guild_GuildNameMaxLength`, `Guild_RankNameMaxLength` | int | 32 each |
| `Guild_MotDMaxLength` | int | 250 |
| `Guild_MaxGuildCalendarEvents` | int | 20 |
| `Guild_RecruitmentTimeOut` | float | 30 |
| `Guild_DaysLeaderIsOfflineForSuccessorUsurp`, `…ForOfficerUsurp`, `…ForMemberUsurp` | int | 35, 45, 55 |
| `Guild_LeaderRankName`, `Guild_NewMemberRankName` | StringInfo | |
| `Guild_RankList` | 4 `Guild_RankEntry` | `Guild_RankID` 1–4, `Guild_RankLevel` 1, 5, 10, 20, `Guild_RankName`, `Guild_RankPermissions` (bitfield32, mapper 0x2300015D) |
| `Guild_DefaultTheme` | DID → class 370 | |
| `Guild_RecreateGuildCharterDID`, `Guild_RecreateMPGuildCharterDID` | DID → item (class 796) | |
| `Kinship_LifespanEpsilon` | int | 1,800 |
| `Kinship_LifespanRankArray` | 10 `Kinship_LifespanRankStruct` | `Kinship_LifespanRank` (mapper 0x2300007B, ranks 1–10), `…_RealTimeToGainRank` (int 0–360), `…_MaxMembers` (1,000–1,500), `…_Permissions` (7) and `…_MonsterPermissions` (1) (same permission mapper) |

**Client use (BIN):** the client resolves this control by key (§2.4) and reads
`Guild_PlayerNoteMaxLength` (an int). This install's control does **not** hold it, and the
client then uses **48**.

The client uses the stored value only when it is an **int** (property type 15,
`property-system.md` §3). When the control holds the property with any other type, the
client uses **48**, exactly as when the property is absent (BIN q5, GC29). This confirms
the implementation's choice. (Answers the implementer note of 2026-10-08.)

<a id="s8-2"></a>
### 8.2 MailControl (Confirmed, GC29)

| Property | Type | Observed |
|---|---|---|
| `Mail_MaxAllowed` | int | 50 |
| `Mail_BasePostage` | int | 50 |
| `Mail_ItemPostage_Common`, `…_Uncommon`, `…_Rare`, `…_Incomparable`, `…_Legendary` | float | 0.1, 0.2, 0.2, 0.3, 0.3 |
| `Mail_CurrencyCheckpointThreshold` | int | 200,000 |
| `Mail_ExpiredMailQuerySize` | int | 1,000 |
| `Mail_Failsafe` | int | 300 |
| `Mail_GRSAttachedItemPropertyNameArray` | 10 PIDs | reporting properties, one per attachment slot |

<a id="s8-3"></a>
### 8.3 AuctionControl (Confirmed, GC29)

| Property | Type | Observed |
|---|---|---|
| `Auction_MaxSubscriberAuctions`, `Auction_MaxCustomerAuctions`, `Auction_MaxFreeAuctions` | int | 30, 5, 0 |
| `Auction_MaxAuctions_WebstoreAccountItem_Array` | 5 structs | `Auction_WebStore_AccountItemDID` (→ class 2532) → `Auction_WebStore_NumAuctions` (5 each) |
| `Auction_MaxSubscriberBids` | int | 20 |
| `Auction_PostingFee_FourHours`, `…_EightHours`, `…_TwentyFourHours`, `…_FortyEightHours` | float | 0.03, 0.06, 0.2, 0.4 |
| `Auction_PostingFee_Minimum`, `Auction_PostingFee_KinshipMinimum` | int | 5, 300 |
| `Auction_WinningFee_*` (four durations) | float | 0.05 each |
| `Auction_Bid_MinimumIncrease`, `Auction_Expire_MaxDurationMultiplier` | float | 1.05 each |
| `Auction_Cancel_Window`, `Auction_Cleanup_Delay`, `Auction_Expire_PushoutWindow`, `…_PushoutMin`, `…_PushoutMax` | int | 3,600, 300, 300, 0, 300 |
| `Auction_Query_PageSize`, `Auction_Cleanup_QueryPageSize` | int | 20, 50 |
| `Auction_MinStringSearchLength`, `…_Asia` | int | 3, 1 |
| `Auction_CurrencyCheckpointThreshold` | int | 200,000 |
| `Auction_PropModCategory_PropertyArray` | 14 PIDs | the `Auction_PropModCategory1`… columns |
| `Auction_IAProgressionTierGroupToColumn_Array` | 6 structs | tier group (mapper 0x230002A5) → column PID |
| `Auction_ModsByClass_Array` | 13 structs | `Auction_ModsByClass_Class` (class mapper 0x2300000E; Undef and 12 classes) → `Auction_ModsByClass_Category_Array` (auction property-mod categories, mapper 0x230002A3; 39 for Undef, 1 for each class) |

The search-filter columns are what an auction browser offers. The fees are server rules.

<a id="s8-4"></a>
### 8.4 Housing and fellowship (SPEC)

HousingControl is described in `map-data.md` §9.7; HousingAuthority is a manager entity
(§3.4). FellowshipManager and FellowshipMakerManager are manager entities; the
fellowship-maker control and its dialogs are outside this spec (only the
`Dialog_Fellowship*` markers are covered, §3.2).

<a id="s8-5"></a>
### 8.5 Dictionary-only and lightly used families (Confirmed, GC30)

| Family | Defined | Used | Note |
|---|---|---|---|
| `LFG_` | 17 | **0** | no value in any property set |
| `Antispam_` | 65 | **0** | no value in any property set |
| `Mail_` | 40 | 12 | all 12 in MailControl |
| `Auction_` | 117 | 47 | AuctionControl and item and effect records (classes 795–805, 2814, 4178 and others) |
| `Guild_` | 58 | 20 | all 20 in GuildControl; the others (for example `Guild_PlayerNoteMaxLength`, §8.1) hold no value in the archives |

Defined-but-unused properties have no value to read; a reader shows them only as
dictionary entries.

---

<a id="s9"></a>
## 9. Reference graph (Confirmed, GC31)

Typed references from this spec's singletons (all DID values of their property sets) and
into the member families, found over all 383,332 object property sets and all 1,618 bare
property sets:

| From | Property | To (class here) | Count |
|---|---|---|---|
| PVPControl | `PVP_HeroDID`; `…_EarnedRankXP_Table`; lobby avatars | 1930; 849; 1653, 1654 | 1; 2; 1 + 1 |
| PVPVirtueControl | `PVPVirtueControl_Virtue`; slot schedule | 3564; 1490 | 31; 1 |
| PVPCombatControl, GloryControl, MPAdvancementControl, MountedCombatControl | progressions | 1385 | 8; 4; 26 (8 distinct); 2 |
| PVPFeatControl | feats; required component | 827; 796 | 10; 1 |
| GloryControl | rank and prestige tables; modifier table | 849; 852 | 2; 1 |
| MPAdvancementControl | traits; virtues | 1477 + 1483; 1494 | 331; 13 |
| MPLevelTableDirectory | `AdvTable_MLT` | 1932 | 13 |
| MountDirectory | `Mount_SkillToGrantTall` / `…Short` | 827 | 347 + 329 |
| MountedCombatControl | skills; tree effects; unlock trait; steed table | 827; 718; 1477; 849 | 24; 3; 1; 1 |
| CombatStateControl | flag effects (non-null); state and on-enter effects | 718; 734 + 764 | 13; 41 |
| PerkControl | `Perk_DID` | 1377 | 2 |
| perk vendors (`economy.md` §6.4) | `PerkVendor_PerkDID` | 1377 | 72 |
| CollectionControl | collections | 3996 | 29 |
| collections | pieces; treasure; permission | 827; 1406; 1603 | 131; 29; 29 |
| PaperItemControl | services; PvP currency; wallet upgrade | 3937 and 6 subclasses; 796; 2784 | 35; 1; 1 |
| service entries | currency; cost and enable records | 796; 2801 | 35; 40 |
| RewardTrackControl | tracks | 4224 | 27 |
| RandomItemRewardControl | NPCs; icons; reward items | 1724; 0x41; 796 | 10; 10; 10 |
| login rewards (3731) | treasure tables; progression; ripcords | 3307; 3810; 2801 | 6; 2; 4 |
| gmVitalDisplayOverrideControl | overrides | 3463 | 113 |
| items (796) | `VitalDisplayOverride_UnlockedVDOs` (a bitfield, not a DID) | override keys | 111 |
| ConsiderationControl | combat tables; quest tables | 1694; 1697 | 720 (6 distinct); 120 (2 distinct) |
| distant-battle units (3725, 4026) | `DistantBattleUnit_Formation_LayoutCalculator` | 3757; 3856 | 302; 127 |
| encounter templates (1368) | `PrivateEncounterTemplate_WhiteList` | 3049 | 17 |
| GuildControl | default theme; charters | 370; 796 | 1; 2 |
| AuctionControl | account items | 2532 | 5 |
| manager entities (14) | `PhysObj`; usage actions; script tables | 0x47; 1556; 0x07 | 14; 28; 14 |

No typed value of any record names one of the 119 labelled singletons, apart from the two
exceptions of §2.3: the singletons are reached from WEENIECONTENT and refer outward to
their families.

---

<a id="s10"></a>
## 10. Invariants

Checked on this install (all Confirmed, 2026-10-07). A reader treats a failed invariant as
a data error to report, never as a reason to panic.

1. **GCI1.** Every one of the 119 labels of §2.2 resolves through WEENIECONTENT to an object
   record that exists; its property set and object state decode completely.
2. **GCI2.** Each of the 52 empty markers (§3.2) decodes to one script object with zero
   attributes and a property set with zero entries; the 52 root classes are distinct; the
   38 dialog markers' root classes have one common parent.
3. **GCI3.** Each of the 6 DID sets (§3.3) has an empty property set and an object state of
   exactly two items: a root script object with one reference attribute, and a class-18
   set; every member is the DID of an existing object record.
4. **GCI4.** Exactly 14 entries of the map have a root class whose parent is the manager
   base (§2.1 row 8). Their property sets are identical apart from `Name`, except
   that RepositoryManager has the 12 extra version properties of §3.4 and no other
   difference. The three server entities named in §3.4 have the same property set as the
   managers apart from the name and `PhysObj`, and their root classes' parent is not the
   manager base (§3.4).
5. **GCI5.** Every class-849 list (§4.10) starts with 0 and is non-decreasing; every
   class-852 map's values decode as finite floats.
6. **GCI6.** MountDirectory's tall and short skill sets are disjoint and every element is a
   skill (§5.1). MountAppearanceTraits equals the set of all objects of the trait class it
   names (§5.2).
7. **GCI7.** The 17 weapon-style labels (§5.5) name skills with `Skill_IsDefaultAttack` =
   true.
8. **GCI8.** PerkControl's channels and UI categories are each distinct, and every perk's
   channel and category is one of them (§6.1).
9. **GCI9.** The paper-item service types (§6.3) are pairwise distinct.
10. **GCI10.** The vital-display override control lists every override object once; the
    overrides' `RequiredEnum` keys are distinct and non-zero; every
    `VitalDisplayOverride_UnlockedVDOs` value has exactly one bit set, and bit + 1 is the
    key of an override (§7.1).
11. **GCI11.** AlertsControl's 64 alert types are distinct and non-zero (§7.3).
12. **GCI12.** Every consideration table has 31 ranks, non-decreasing in rank value, with
    Even at index 15; every `ConsiderationControl_CombatTableList` and the
    `ConsiderationControl_QuestTableList` have 120 elements; the six consider types are
    distinct (§7.5).
13. **GCI13.** TrackingStateControl's 17 tracking states are distinct (§7.10).
14. **GCI14.** No property set holds an `LFG_` or `Antispam_` property (§8.5).
15. **GCI15.** The ordered rule of §2.1 assigns every one of the 316 entries exactly one
    kind, no entry reaches the end of the table without a match, and the per-kind counts
    are those of the §2.1 table (1, 1, 6, 38, 14, 23, 12, 14, 14, 193).
16. **GCI16.** Every member of MountAppearanceTraits has no top-level `Trait_Group` and a
    top-level `Trait_Groups` array of exactly one element, a `Trait_Group` whose key is a
    key of `Mount_Control_Appearance_Group_To_Property_Hash`; every key of that hash is used
    by at least one member (per-slot counts 152, 175, 105, 54, 153, 118, 10 for keys 8–14
    here); every member's `Trait_Appearance_List` has two `Trait_Appearance_Set` elements
    of one `Trait_Appearance_Struct` each (§5.2).
17. **GCI17.** Every `VitalDisplayOverride_UnlockedVDOs` value of the install is the single
    element of a top-level `Usage_SetPropertyUsageData_Array` of an item (111 of 111 here);
    none occurs at the top level or inside a struct (§7.1).
18. **GCI18.** MountedCombatControl's flank heading and arc fields are floats, and the
    `Mount_Control_Trait_Tree` keys of the tree array and of the tree-to-skill array are the
    same set, each key once per array (§5.3.1).
19. **GCI19.** Among the 13 managers other than RepositoryManager, `Name` is the only
    property whose value differs (§3.4).

Install-specific values (counts, DIDs, keys, class indices) are observations; tests compare
them only when `TESSERA_GAME_DIR` names an install of the same data.

---

<a id="s11"></a>
## 11. Worked synthetic examples

All values are invented for the example; none comes from the archives.

<a id="s11-1"></a>
### 11.1 Consideration lookup (data navigation; index rule per the reading of §7.5)

A synthetic control has `CombatTableData` entry (Monster, list *M*) where list *M* names
table A for elements 1–10 and table B for 11–120. Table B's 31 ranks are: 7 × Trivial,
3 × InsanelyEasy, 3 × VeryEasy, 2 × Easy, Even, 2 × Tough, 2 × VeryTough,
3 × InsanelyTough, 8 × Impossible. `RankToUIState` maps Tough to state *S*.

A level-20 viewer considers a level-22 monster. Element 20 of list *M* is table B. The
difference is +2, index 17 (counting from 0). Indices 0–6 are Trivial, 7–9 InsanelyEasy,
10–12 VeryEasy, 13–14 Easy, 15 Even, 16–17 Tough, so index 17 is Tough and the name colour
state is *S*. With a difference of −20 the index clamps to 0 (Trivial); with +20 to 30
(Impossible).

<a id="s11-2"></a>
### 11.2 Mounted gait multipliers (BIN, §5.3.2)

A synthetic control holds: speed Walk 0.4, Trot 0.6, Canter 0.9 and no Gallop property;
turn rate Walk 1.2, Canter 0.5; `Mount_Control_Turn_Rate_AddMod_Property_Canter` names
property *P*; no add-mod for Walk.

- Speed for gait Trot (5): 0.6. For gait Gallop (7): the property is absent, so **1.0**.
  For gait Neutral (2): **1.0**.
- Turn rate for gait Canter (6) on an actor whose *P* is 0.25: 0.5 + 0.25 = **0.75**; on an
  actor without *P*: 0.5. For gait Neutral (2): the Walk pair, **1.2**. For gait Trot (5):
  the Trot multiplier is absent (1.0) and no Trot add-mod, so **1.0**.
- Forward speed limit with `MountedCombat_Current_LockedGear` = Canter and a base forward
  limit of 10: 0.9 × 10 = **9**.

<a id="s11-3"></a>
### 11.3 Reset-day bitfield

A synthetic entry has `ResetScheduleControl_ResetDays` = 0x0A (bits 1 and 3). By the
bit-to-key rule these are keys 2 and 4 of the DayOfWeek mapper: Monday and Wednesday. The
value 0x7F (bits 0–6) is keys 1–7, every day.

<a id="s11-4"></a>
### 11.4 Vital-display unlock

A synthetic item has `VitalDisplayOverride_UnlockedVDOs` with only bit 9 set. Key = 10. The
override object whose `VitalDisplayOverride_RequiredEnum` is 10 has UI state *U* and
`…_IsForMonsterCharacters` = true, `…_IsForFreepCharacters` = false: the unlock offers state
*U* for monster-play characters only.

<a id="s11-5"></a>
### 11.5 A DID-set object (structure)

A synthetic object of the DID-set class has the empty property set (6 bytes: its DID, then
hint 0 and count 0) and an object state whose root script object has one reference
attribute pointing at item 1, a class-18 set with three members 0x70001000, 0x70001001 and
0x70002000. A reader reports "DID set, 3 members" and lists the members with their root
classes; the order shown should be sorted by DID, because the stored bucket order carries
no meaning.

---

<a id="s12"></a>
## 12. Open questions

| # | Question | Section | What would settle it |
|---|---|---|---|
| Q1 | Which code reads the controls that native code does not read (consideration, vital-display overrides, combat music, aggro ping, alerts, discovery distance, tracking states, perks, pets, GRS): the client's script library or only the server; for GRSControl also whether the code-section matches of its property IDs (§7.16) are reads | §2.4, §7.16 | reading the client script library's bytecode (`wlib.md`) for these keys or property IDs; M3 observation |
| Q2 | What the empty marker classes are used for | §3.2 | script-library reading; server traffic is gated |
| Q3 | Purpose of the DID sets beyond membership | §3.3 | the store and big-battle specs |
| Q4 | The three lists of a trait slot schedule (class 1490) | §4.2 | `combat-data.md` §8 work |
| Q5 | Index base of class-849 lists; key meaning of class-852 maps | §4.10 | comparison with displayed rank thresholds (M3) |
| Q6 | Whether the race table's `RaceTable_IsTallRace` is what selects the tall or short mount-skill group (the per-species flag itself is now in §5.1) | §5.1 | BIN (a native read of the flag); M3 (mount skills granted to a short and a tall character) |
| Q7 | Use of the flank constraints | §5.3.1 | server rule; Tessera decides for M7 |
| Q8 | Source of the second forward-speed limit; rest of the mounted controller | §5.3.2 | further BIN in the movement controller (`walker.md`) |
| Q9 | Default-attack selection rule | §5.5 | BIN or script-library reading; M3 observation of auto-attack animations |
| Q10 | Weapon damage variance formula | §5.6 | server rule |
| Q11 | Behaviour per paper-item service class | §6.3 | server rule |
| Q12 | Tier order in the login-reward window; meaning of the rarest-tier chance | §6.6 | further BIN on the reader; M3 |
| Q13 | Where the chosen vital-display override is stored; how a UI state changes the vitals display | §7.1 | script-library reading; M3 |
| Q14 | Alert overflow behaviour | §7.3 | M3 |
| Q15 | Reset time of day and time zone | §7.4 | not in the data; M3 |
| Q16 | Consideration index rules (viewer or target level; levels above 120; clamping; type selection; quest tables) | §7.5 | M3 observation of name colours at known level differences; script-library reading |
| Q17 | Grid and scatter placement algorithms | §7.6 | BIN of the distant-battle unit code, if client-side |
| Q18 | Aggro ping meaning and units | §7.7 | server rule |
| Q19 | Combat-music intensity formula | §7.8 | script-library reading |
| Q20 | What discovery distance classifies | §7.9 | script-library reading; M3 |
| Q21 | Shapeshift redirect rule | §7.11 | `appearance.md` work |
| Q22 | Meaning of skill pruning | §7.13 | server rule |
| Q23 | How a mount appearance trait's material and appearance sets are applied to the mount (the slot link itself is in the data: nested `Trait_Group`, §5.2) | §5.2 | trait and appearance code (`appearance.md`); BIN; M3 |
| Q24 | Perk purchase and per-channel limit rule | §6.1 | server rule |
| Q25 | How vital-display overrides 1 and 2 become available | §7.1 | script-library reading; M3 |
| Q26 | Which entities carry which map note type; what a tracking state changes | §7.10 | script-library reading; M3 |
| Q27 | Pet property forwarding and per-channel pet limits | §7.12 | script-library reading; server rule |
| Q28 | What an encounter whitelist filters, and where | §7.14 | M3 inside such an encounter; script-library reading |
| Q29 | Whether `Trait_Appearance_IsDyable` or `Mount_Control_Slots_Allowing_Dye` decides dyeing for saddle and gear traits | §5.2 | M3 (dye panel with such a trait); script-library reading |
| Q30 | Collection completion rule and reward grant | §6.2 | server rule; M3 |

---

<a id="s13"></a>
## 13. Corrections and additions to other specs (proposals)

This spec edits no other file. Proposed follow-ups for the owners of the other specs:

- **`misc-records.md` §14:** add that the labels of §2.2 are resolved in this spec, and the
  structural kinds of §2.1.
- **`economy.md` §4.12 (OPEN on `LootGenControl_RewardOnceTGPs`):** class 3399 is the
  DID-set class of §3.3. The record's property set is empty because its data is in the
  object state, and in this install the set has no members. The question of what
  `TreasureGroupProfile_RewardOnce` is for remains.
- **`combat-data.md` §10.1:** name the label `CombatControl`; add the client's duel-timing
  defaults (§5.7).
- **`items.md` §6.4:** point to §6.3 for the paper-item services.
- **`items.md`:** state that `Usage_SetPropertyUsageData_Array` is a top-level array of
  property values on usable items, and that the vital-display unlocks are its elements
  (§7.1).
- **`walker.md`:** reference §5.3.2 for the mounted gait speed and turn-rate multipliers if
  the walker gains mounted movement.

---

<a id="s14"></a>
## 14. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version, status Review. Census of all 316 WEENIECONTENT entries by kind; 119 labels and 300 member objects of this spec's scope decoded and described; BIN pass for native WEENIECONTENT resolution, native property reads, gait multipliers, duel and guild defaults. |
| 2026-10-07 | Revision for the first review (changes requested), status stays Review. §2.1: the kind rule is now an ordered table with a replaceable table of class indices (484, 485, 89 common root; 486 plain-object base; 223 entity base; 3399, 1728, 827, 698, 788) and cross-checks; managers are recognised by parent class 788, not by the property template; row 9 listed by label (GC1, GCI15 new). §3.4 and GCI4: three server entities share the manager template except `PhysObj`; RepositoryManager's 12 extra properties corrected (GC8). OBS row, §2.3, §9, GC3, GC30: 383,332 object property sets (the 11 0x76 objects have none); 1,618 bare sets = 1,616 in `client_gamelogic.dat` + 2 in `client_local_English.dat`, the 2 now covered by the typed and raw scans; scope of the 5,915,209 DID values stated. Provenance: §9 cites the new row GC31, §5.4 the new row GC32. §5.2: the slot examples removed (the 767 traits carry no `Trait_Group`, GC13), Q23. Readings in §6.1, §7.1, §7.10, §7.12 and §7.14 turned into OPEN Q24–Q28. |
| 2026-10-07 | Second revision, for the second review (changes requested), status stays Review. §5.2, GC13: the earlier statement that the 767 class-3509 traits carry no `Trait_Group` was wrong (a top-level-only check); each carries exactly one, nested as the single element of `Trait_Groups`, keys 8–14 with per-slot counts, equal to the keys of the appearance-group hash; the appearance fields, dye flags and the shared material mapper added; Q23 narrowed to how the appearance is applied; new Q29 (dye rule) and GCI16. §2.3: each labelled record's own DID at payload offset 0 named as the expected raw match. §6.2: the completion rule marked as a reading, Q30. §7.16: "no client use" replaced by the BIN facts (not resolved by key; byte scan, q4, finds unexamined code-section matches of 9 of 28 property IDs) and Q1. BIN row: q4. |
| 2026-10-07 | Third review passed; status Review → Accepted (reviewer). No content change. |
| 2026-10-08 | Revision answering the six implementer OPEN notes of 2026-10-08; status Accepted → Revised. No section renumbered. §2.1: role classes need not be defined in the class table; the entity base is undefined here and the cross-checks use the parent map only (GC1, OBS2). §3.1: the typed-view choices split into client behaviour (wrong type = absent for the native reads of §5.3.2, §5.7, §8.1) and Tessera Decisions (list cap, error on wrong type elsewhere, null DIDs dropped except in the consideration lists); no DID list in scope holds a null DID (new GC33). §3.4: the name property is `Name` (GC8, GCI4, new GCI19). §5.3.1: the flank fields are floats; both trait-tree struct kinds carry `Mount_Control_Trait_Tree` (GC14, new GCI18). §6.6: the tier-field labels spelled in full (GC22). §7.1: each unlock is the single element of the item's top-level `Usage_SetPropertyUsageData_Array` (GC23, new GCI17; reading added to Q13/Q25 context). §8.1: the client uses 48 also for a value that is not an int (BIN q5, GC29). §5.1: the per-species tall flag `RaceTable_IsTallRace` of the race table (GC12); Q6 narrowed to the link between flag and skill group. New provenance key OBS2; BIN row: q5. §13: an `items.md` proposal. |
