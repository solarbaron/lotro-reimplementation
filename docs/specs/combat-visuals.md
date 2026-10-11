<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Combat visuals: skill animations, strikes and hooks, effect visuals, stance, defeat

| | |
|---|---|
| **Status** | Accepted (2026-10-11; independent review of commit 7b14f27 passed, see the review record). Before: Draft, awaiting independent review |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted, neither the author nor an implementer), 2026-10-11 |
| **Review record** | Independent review 2026-10-11 of the first version (commit 7b14f27 on branch p2), together with the P2 revisions of `script-table.md`, `particles.md` and `particle-runtime.md`, by a separate dirty-room session (AI-assisted; it wrote no part of these specs or of the P2 code), checked against CLEAN_ROOM.md §5.3: **passed**, Accepted. Every statement is a functional fact, a census, a Tessera rule (§9, §11) or an OPEN with its Decision (Q-CV1–Q-CV7); no code or pseudo-code (the §7 description of the death script follows the data's node structure, not a source listing); script-key labels, property names, callback labels and enum labels are data identifiers quoted for interoperability; no game text, assets, keys or hostnames; the BC and BIN tags name build, questions and why M1–M3 were not enough, and exclude the network decoder (decision 0004); every CV row has its mark; nothing falls under a closed gate. Spot-checks, own read-only probe with the p2 decoders on 2026-10-11, all exact: 13,247 skills; Self lists 1/2/3 non-zero keys in 13,221/3/23 skills, Other 1/3 in 13,228/19; lists equal in stored order in 13,024, different in 223; mounted lists 1,137/1,139 of one entry; `Skill_WarmUpBehavior` in 19 skills, 62 values, all 0; `Skill_AnimationMode`, `Skill_AnimationMayFail` and the duration-override histograms of §2.2; 13,733 hook infos, 4 skills without hooks, the six hook-index sequences of §2.3, 72 cosmetic hooks; every row of the §2.1 table (values, non-zero, keys); dictionary `field_b` 6 for `Combat_AnimationSpeedMultiplier` (default 1.0), the seven `CombatStrikeData_*`, both `ScriptSystem_Snd_*` and `SCP_Output_GetUpcomingTargets_TargetArray`, `field_b` 4 and the PIDs of `Entity_Mode`, `Combat_Violent`, `LifeState`, `Death_DeadScript`, `Death_DiedWhileKnockedDown`, `Mount_Is_Mounted`, `GOP_*` defaults true; mappers 0x23000123, 0x2300002A, 0x23000003, 0x23000092 (all §6.1 values), 0x2300013D and the categories of rule 6 in 0x2300004C; `Death@Agent::*Die` defined in 41 tables, `*Revive` in 5; Beam census of §4.7 and `particle-runtime.md` §11.1 (2,740 nodes; persistent 769; head entity 1,050; beam and head systems 571 and 45, 2 head-only; the ramp, head, rate, scale-source, detach, radius, port-children and target counts; hooks Attack only 945, both 2, Defend only 33); Execute callbacks and result ports of §4.6 and `script-table.md` §9.1; StoryHook 4,130 / 2,480; FireMissile 183; §10.1 categories of all 11,155 0x39 records. BIN spot-check (own headless decompilation, Ghidra 12.1.3, private project copy, deleted afterwards; constants read read-only from the client file): the beam draw step computes the head and tail fractions as `particle-runtime.md` §11.1 step 5 states, arrives at speed > 0.0002 and fraction > 0.9998, uses length 0.01 at or below 0.0002 and skips beams longer than 1,000 m (step 4), and detaches ends by the > 0.0002 / ≥ rule of step 3. The strike component (CV8–CV11, `script-table.md` T44) and the BC behaviour choice were not re-derived. **Non-blocking (wording; none changes implemented behaviour):** (1) §3.2 lists `StorySystem_StoryID` among the script-parameter properties (CV7), but its dictionary `field_b` is 4; say that the client keeps the story ID with the run (as a run parameter in Tessera) although its definition is not a script-parameter one; the implementation's run-parameter storage stays as is. (2) §6.3: the induced state bits listed sum to 209 of the 239 class-769 effects; 0x4000000 (23 effects), 0x2000 (5) and 0x8 (2) are missing. (3) §4.7 and the `particle-runtime.md` census say 4 hook-carrying Beam nodes have head speed 0: 2 have 0 and 2 have −25; all four never arrive (speed not above 0.0002). (4) The implementer OPENs of §2.1, §4.6, §4.7, §9, §9.3, §10.5 and rule 6 (two) record the readings the code follows; the spec writer should answer them as Decisions at the next revision. |
| **Implements into** | `crates/gamedata` (the skill, effect, induction, channelling and combat-state script references of §2, the census of §10), `crates/particles` (beam-driven particle systems, `particle-runtime.md` §11.1; lit systems, its rule 13), `crates/scriptgraph` (running a script key on a figure with a run-parameter set; StoryHook and Execute callbacks of §4; Beam nodes), `crates/anim` (no new rule: layers, ramps and track opacity as specified), `apps/client` walker (stories, strikes and hooks, the mapping of session events of §9, stance and defeat properties) |
| **Companion specs** | [`combat-data.md`](combat-data.md) (skill, effect, induction records; §7.7 attack hooks; §10.2 weapon timing), [`script-table.md`](script-table.md) (tables, nodes, lookup §8.2, animation node §6, StoryHook §9), [`script-runtime.md`](script-runtime.md) (runs, run parameters §5.1, starts §12), [`particles.md`](particles.md), [`particle-runtime.md`](particle-runtime.md) (effects; beams §11.1), [`skeleton-anim.md`](skeleton-anim.md) (layers, track opacity §6.6, holding locations §8), [`game-controls.md`](game-controls.md) §5.5 (default attacks), §5.8 (combat states), [`interaction.md`](interaction.md) §3.8–§3.9 (induction and channelling records), §8 (defeat records), [`scenario-feed.md`](scenario-feed.md) §9 (session events), [`combat-ui.md`](combat-ui.md) (panels, floating text) |

---

<a id="purpose"></a>
## Purpose

This spec says what a client shows on the figures in the world when combat happens: which
scripts it runs when a skill is used, cast, inducted or channelled; when and how a strike
shows on its target and on the attacker; how projectiles and beams travel; which visuals an
effect plays while it is applied; how the combat stance, the idle in combat, the combat
states, defeat and revive look; and how all of this is timed against the session events of
`scenario-feed.md` §9. It also gives the census that exit criterion 3 of P2 asks for: every
particle descriptor decodes or is categorised, and the Particle and Beam nodes that skill and
effect records reach.

Everything the client plays is a script of a script table (`script-table.md`): the data
names **script keys** (enum mapper 0x23000097) and the figure's tables define what each key
does. The client's own code decides only **which key** to run, **on which entity**, **with
which run parameters** and **when**. Those decisions are this spec. Outcomes (hit or miss,
damage, which effect is applied, when a cast ends) are never derived: they come from the
session (decision 0024, "values are scripted, never derived"). Where the original's timing
depends on data its server sends, §9 states the Tessera rule.

No game text appears. Script-key labels, property names and enum labels are identifiers
stored in the data and are quoted for interoperability only; an implementation reads them at
run time. String-valued identifiers inside nodes (scriptlet-parameter names) are referred to
by structure.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = observed in the user's install on 2026-10-10 over all records of the
kind (row OBS); **Confirmed (BIN)** = read from the client executable (row BIN);
**Confirmed (BC)** = read from the bytecode of the client's class library (row BC);
**Inferred** = follows from names or consistent data, not traced in code; **Decision** = a
Tessera rule (§11); **Reported** = published sources only (none used).

| Tag | Source |
|---|---|
| OBS | User install (Steam), read in place and read-only on 2026-10-10 with throwaway probes in the dirty-room scratch area (the repository's `tessera-formats`, `tessera-props` and `tessera-wstate` decoders used as tools): every property set of `client_gamelogic.dat` (0x79), classified by signature properties (skills: `Skill_SkillType`; effects: `Effect_Name`; induction objects: `Induction_Duration`; channelling objects: `Channeling_Duration`; the combat-state control); every property whose dictionary definition names the script-key mapper 0x23000097 or its base 0x230000F8; all 7,524 decodable script tables and all 28,992 scriptlets, walked through Scriptlet invocations (and, where stated, Run nodes); all 11,155 records with prefix 0x39 and all 254 with prefix 0x44 of `client_general.dat`; the master property dictionary 0x34000000 (types, defaults, the parameter flag `field_b`); the compiled objects of skill records (class 827) and effect records (class 698). |
| BC | The client's class library (the WLIB record of `wlib.md`), disassembled with the repository's `tessera-wvm` decoder in the dirty room on 2026-10-10: the methods of the skill class (827) that return the behaviour lists, the behaviour-entry class (3451), and the agent method that chooses and starts a skill behaviour; a literal search of every method body for the PIDs of §2. **Question:** how the client chooses the behaviour script of a skill. **Why M1–M3 were not enough:** the choice exists only as code; the data stores the lists. |
| BIN | **M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (re-checked 2026-10-10), analysed with Ghidra 12.1.3 in headless mode on a temporary copy of the analysis project (deleted afterwards), 2026-10-10, in the dirty room. **Questions:** (a) which entities and which scripts the client runs when a strike of a skill is shown, with which run parameters; (b) how strikes wait for the attacker's animation hooks, and what happens to strikes whose hook never fires; (c) how the beam renderer moves a beam's head, fires the Beam node's port and drives the beam's particle systems (answers `particle-runtime.md` Q-RT4); (d) whether native code reads the script-key properties of effects, inductions, channels and stances. **Why M1–M3 were not enough:** these are code paths; the data stores only the parameters; no published source describes them (M2); no running client or frame capture is available to this session (M3). Only the client's presentation routines that consume strike records already in memory were read; the code that receives and decodes the server's messages, login, keys, launcher and anti-tamper code were not examined (decision 0004). |
| CD | [`combat-data.md`](combat-data.md) (Accepted): skill and effect families §2, attack hooks §7.7, timing §7.9, weapon timing §10.2.1. |
| ST | [`script-table.md`](script-table.md) (Accepted): node layouts §4–§5, animation node §6, lookup §8.2, the locomotion template §8.3–§8.4, StoryHook §9. |
| SR | [`script-runtime.md`](script-runtime.md) (Accepted): runs and contexts §2, run parameters §5.1, starts and lookup §12. |
| PR | [`particle-runtime.md`](particle-runtime.md) (this revision): effect starts §5, stopping and lingering §7, beams §11.1. |
| GC | [`game-controls.md`](game-controls.md) (Accepted): §5.5 default attacks, §5.8 combat states. |
| IN | [`interaction.md`](interaction.md) (Accepted): §3.8 induction records, §3.9 channelling records, §8 defeat records. |
| SF | [`scenario-feed.md`](scenario-feed.md) §9 (session events and their fields). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| CV1 | Census of the properties of combat records that name script keys, their counts, distinct keys and keys no table defines (§2.1) | OBS | Confirmed |
| CV2 | Every skill has a `Skill_BehaviorSelfList` and a `Skill_BehaviorOtherList` with at least one non-zero key; the two lists differ in 223 skills; 1,137 / 1,139 skills have mounted lists of one entry; `Skill_WarmUpBehavior` holds only 0 (§2.2) | OBS (13,247 of 13,247 skills) | Confirmed |
| CV3 | Census of `Skill_AnimationMode`, `Skill_AnimationMayFail`, `Skill_AnimationDurationMultiplierModOverride` and of the attack-hook index sequences and hook script fields (§2.2, §2.3) | OBS | Confirmed |
| CV4 | Behaviour choice: the mounted lists when the performing entity has `Mount_Is_Mounted` true, else the plain lists; the Self list when the performing entity's ID equals the ID its caller passes, else the Other list; one entry uniformly at random when the list holds more than one; a key of 0 plays nothing (§3.1) | BC | Confirmed (BC); that the caller's ID is the local player's is Inferred |
| CV5 | The chosen key is started on the performing entity through the normal script start (`script-runtime.md` §12) with a run-parameter set built by the client (§3.2) | BC (the agent method's native calls); SR | Confirmed (BC); the content of the run-parameter set Inferred (§3.2) |
| CV6 | Census of the animation and Timer nodes reachable from behaviour keys: layers, `UseBoneOpacity`, driven speeds (§3.3) | OBS | Confirmed |
| CV7 | `Combat_AnimationSpeedMultiplier`, every `CombatStrikeData_*` property, `ScriptSystem_Snd_StrikingImplementType`, `ScriptSystem_Snd_StruckSurfaceType` and the `SCP_*` callback properties are script-parameter properties (dictionary `field_b` = 6, looked up in the run-parameter set); `Entity_Mode`, `Combat_Violent`, `LifeState`, `Death_DeadScript` and `Death_DiedWhileKnockedDown` are entity properties (`field_b` = 4); option defaults of `GOP_CombatHitFX`, `GOP_AvatarAttackFX`, `GOP_MonsterAttackFX`, `GOP_EnableGore` are true (§3.2, §4.5, §6, §7) | OBS (dictionary 0x34000000) | Confirmed |
| CV8 | Strikes are kept per attacker and per story (an action instance identified by `StorySystem_StoryID`), each with a hook number and a kind; the attacker's StoryHook events count per hook type; an event fires the story's strikes whose hook number equals the new count, every kind for an Attack hook (type 2), only kind 0x100 for a Defend hook (type 1); fired strikes are removed (§4.2) | BIN | Confirmed (BIN) |
| CV9 | When a story ends, its remaining strikes are fired at once, unless the end is of the discarding kind, which drops them (§4.3) | BIN | Confirmed (BIN); which ends discard Inferred |
| CV10 | Showing one strike: the hook data of (skill, hook number) is looked up in the class library; the attacker runs `Skill_AttackHook_fxAttackerHit`; the attacker's weapon runs `Skill_AttackHook_fxWeaponHit` and `Skill_AttackHook_BehaviorWeaponHit`; the target runs `Skill_AttackHook_BehaviorTargetHit` then `Skill_AttackHook_fxTargetHit`, both with the strike run parameters; a strike that names its own script runs that one instead of the two hook scripts; kind 1 always shows, kinds 0x100, 0x200 and 0x10000 only when the target's `LifeState` is Alive (§4.4) | BIN | Confirmed (BIN); the field-to-property mapping of the hook data Inferred from names |
| CV11 | The strike run parameters and how each is formed (§4.4) | BIN | Confirmed (BIN) |
| CV12 | Hit scripts: the reaction scripts choose evade, parry, block, resist, hit and knocked-down reactions from `CombatStrikeData_AttackResult`, the target's combat state and its stance (`Entity_Mode`, `Agent_Class`, off-hand implement); the impact scripts choose by `CombatStrikeData_AttackResult`, `CombatStrikeData_DamageType`, `WeenieType` and the options `GOP_CombatHitFX` and `GOP_EnableGore` (§4.5) | OBS | Confirmed |
| CV13 | Execute nodes of combat scripts call `GetUpcomingTargets` (625 nodes) and `GetUpcomingHitResult` (4) with result-type filters; the targets come back in `SCP_Output_GetUpcomingTargets_TargetArray`, which Beam, Particle and other nodes use as a Property target (§4.6) | OBS (callbacks from DID map 0x28000007, call properties decoded) | Confirmed; the callbacks' native rules not traced (OPEN, §14) |
| CV14 | Projectiles are Beam nodes with a head entity whose port fires an Attack StoryHook when the head arrives; 947 Beam nodes carry an Attack hook under their port (§4.7) | OBS; BIN (the port fires on arrival, `particle-runtime.md` §11.1) | Confirmed |
| CV15 | FireMissile nodes (183, 118 of them reachable from combat records) launch physical missile objects with gravity; none fires a StoryHook (§4.8) | OBS | Confirmed |
| CV16 | Census of the effect, induction, channelling and combat-state script keys and their contents (§2.1, §6) | OBS | Confirmed |
| CV17 | No native code and no class-library body names `Effect_FX`, `Effect_VitalOverTime_IntervalFX`, the induction or channelling script properties, `Entity_Mode`, `Combat_AnimationSpeedMultiplier` or the keys `Death@Agent::*Die` and `Death@Agent::*Revive` by literal (class-library bodies do name `Death@Agent::*Incapacitate` and `Dazed@Agent::*Dazed`); the client receives these choices with the session's data (Inferred) (§5, §6, §8) | BIN (immediate search of the whole executable); BC (literal search of every body) | Confirmed (negative search); the server origin Inferred |
| CV18 | The death script of the avatar closures picks its death clip by `Death_DeadScript` and a knocked-down variant by `Death_DiedWhileKnockedDown`, then holds a pose whose script stops itself when `LifeState` becomes Awakening, Reviving or Alive; a start-up script sets `Death_DiedWhileKnockedDown` when `LifeState` is Dead while the combat state is knocked down; the revive script stops the holds and plays a recovery (§7) | OBS | Confirmed |
| CV19 | Every avatar closure (14 of 14) defines the agent scripts of §7 and the ten combat-state scripts; coverage of behaviour, hit and effect keys per closure (§2.4) | OBS | Confirmed |
| CV20 | The locomotion templates (avatar 0x0C000FFE and creature 0x0C0000B9) and the weapon-style templates switch on `Entity_Mode` combat values; the avatar idle-emote switch is on `Combat_Violent` (§6.1) | OBS; ST §8.3–§8.4 | Confirmed |
| CV21 | Census of particle descriptors and of the Particle, Beam and FireMissile nodes that combat records reach (§10) | OBS | Confirmed |
| CV22 | Tessera rules (§11) | This spec | Decision |

---

<a id="s1a"></a>
## 1a. Terms

| Term | Meaning |
|---|---|
| **performer** | The entity that uses a skill: it runs the skill's behaviour. |
| **behaviour** | The script key a skill names for its performer (`Skill_Behavior`, §2.2). |
| **story** | One use of a skill by one performer, from the start of its behaviour to the end of that run; the client identifies it by `StorySystem_StoryID` (PID 0x48E, an integer). |
| **strike** | One result of a story on one target: an attack result, a damage type, a vital, a hook number and a kind (§4.1). |
| **hook** | A StoryHook node (`script-table.md` §9; kind 28) under an event port of the performer's behaviour: type 2 `Attack`, type 1 `Defend` (enum 0x2300013D). |
| **hook data** | The fields of one `Skill_AttackHookInfo` of the skill (`combat-data.md` §7.7). |
| **run parameter** | A value of the run-parameter set of a script run (`script-runtime.md` §5.1): properties whose dictionary definition has `field_b` = 6 are read there, not on the entity. |
| **Alive** | `LifeState` (PID 0x100005D6, mapper 0x23000123) = 4. Other values: 1 Awakening, 2 Reviving, 3 Incapacitated, 5 Dead. |

---

<a id="s2"></a>
## 2. The script keys combat records name (Confirmed)

<a id="s2-1"></a>
### 2.1 Census (CV1, CV16)

Every property below is an enum whose mapper is 0x23000097 (base 0x230000F8): a script key.
"Values" counts stored values, "non-zero" those that name a key, "keys" the distinct non-zero
keys, "undefined" the keys that no script table of the install defines (running such a key
plays nothing, `script-runtime.md` §12.1).

| Holder | Property | Values | Non-zero | Keys | Undefined |
|---|---|---|---|---|---|
| skill | `Skill_Behavior` (in `Skill_BehaviorSelfList`, `…OtherList`, `Skill_MountedBehaviorSelfList`, `…OtherList`) | 28,857 | 28,857 | 1,763 | 2 |
| skill | `Skill_WarmUpBehavior` | 62 | 0 | 0 | 0 |
| skill | `Skill_AttackHook_BehaviorTargetHit` | 13,733 | 7,170 | 31 | 1 |
| skill | `Skill_AttackHook_fxTargetHit` | 13,733 | 4,478 | 569 | 7 |
| skill | `Skill_AttackHook_fxAttackerHit` | 13,733 | 213 | 67 | 3 |
| skill | `Skill_AttackHook_fxWeaponHit` | 13,733 | 8 | 3 | 0 |
| skill | `Skill_AttackHook_BehaviorWeaponHit` | 13,733 | 3 | 1 | 0 |
| skill | `Skill_ExecutionFailureScript` | 13,247 | 561 | 11 | 0 |
| skill | `Skill_ConjunctionWindupScript` | 13,247 | 239 | 9 | 0 |
| skill | `Skill_GroundTargetPositionValidScript`, `…InvalidScript` | 4, 4 | 4, 4 | 4, 4 | 0 |
| skill | `Skill_Toggle_CasterOnlyTargetFX` | 1 | 1 | 1 | 0 |
| effect | `Effect_FX` | 16,187 | 8,489 | 1,083 | 56 |
| effect | `Effect_VitalOverTime_IntervalFX` | 2,438 | 2,123 | 46 | 1 |
| effect | `Effect_ThreatOverTime_IntervalFX` | 10 | 6 | 1 | 0 |
| effect (mounts) | `Mounted_StartScriptID`, `Mounted_AnimationScriptID`, `Dismounted_ByPlayer_Script`, `Dismounted_ByOther_Script` | 722 each | 722 each | 2 each | 0 |
| induction object | `Induction_AnimationScriptID` | 1,779 | 1,759 | 409 | 0 |
| induction object | `Induction_MountedAnimationScriptID` | 34 | 34 | 11 | 0 |
| induction object | `Induction_FXID`, `Induction_FailureFXID` | 333, 395 | 333, 395 | 44, 32 | 0, 0 |
| induction object | `Induction_TargetAnimationScriptID`, `Induction_TargetFXID`, `Induction_TargetFailureFXID` | 1,779, 89, 1 | 3, 89, 1 | 3, 32, 1 | 0, 5, 0 |
| channelling object | `Channeling_StartScriptID`, `Channeling_AnimationScriptID` | 401, 401 | 290, 401 | 64, 46 | 0, 0 |
| channelling object | `Channeling_Success_Script`, `Channeling_Failure_Script`, `Channeling_Abandoned_Script` | 401 each | 338, 348, 351 | 65, 60, 59 | 1, 1, 1 |
| channelling object | `Channeling_Target_Start_Script`, `…_Animation_Script`, `…_Success_Script`, `…_Failure_Script`, `…_Abandoned_Script` | 401 each | 7, 15, 9, 6, 8 | 4, 10, 5, 4, 5 | 0 |
| channelling object | the eight `Channeling_…Script_FXID` and `Channeling_Target…Script_FXID` properties | 401 each | 3–10 each | 1–4 each | 0 |
| combat-state control | `CombatState_StateData_ScriptID` | 13 | 13 | 10 | 0 |

Undefined keys are data facts, not errors to repair: 469 effects name one undefined key
(`Effect_FX`), which therefore shows nothing in the original either.

> **OPEN:** (implementer, 2026-10-10) the exact labels of the eight channelling FX properties
> are not given (§3.5 names `Channeling_StartScript_FXID`, `Channeling_AnimationScript_FXID`
> and their target counterparts; the success, failure and abandoned ones only as "their
> `…_FXID`"). `crates/gamedata` finds them by name: dictionary properties starting with
> `Channeling_` and ending with `_FXID`, target side when the name contains `Target`, the
> moment by `Start`, `Animation`, `Success`, `Failure` or `Abandon` in the name.

<a id="s2-2"></a>
### 2.2 Skills (CV2, CV3)

Of 13,247 skills (all class 827):

- **Behaviour lists.** `Skill_BehaviorSelfList` and `Skill_BehaviorOtherList` are arrays of
  `Skill_BehaviorListEntry` structs, each holding one `Skill_Behavior` key. Non-zero keys per
  list: Self 1 (13,221 skills), 2 (3), 3 (23); Other 1 (13,228), 3 (19). The lists hold the
  same keys in 13,024 skills and differ in 223 (different keys for the performer itself and
  for others, or a list of three swings against one). `Skill_MountedBehaviorSelfList` (1,137
  skills) and `…OtherList` (1,139) hold one entry each. `Skill_WarmUpBehavior` (19 skills)
  holds only 0.
- **`Skill_AnimationMode`** (mapper 0x2300002F): 1 Melee 7,300; 4 Unarmed 4,445; 2 Ranged
  1,152; 3 Thrown 346; 0 Undef 4.
- **`Skill_AnimationMayFail`**: true 12,921, false 326. **`Skill_AnimationDurationMultiplierModOverride`**:
  −1 (no override) 13,226; −0.2 in 14; 0.19 in 4; −0.01 in 3. Neither has a client-side reader
  that this pass found (§14 Q-CV3).
- `Skill_ActionDurationContribution` (`combat-data.md` §7.9) and the weapon timing of
  `combat-data.md` §10.2.1 do not enter the playback rate of a behaviour on the client; the
  rate comes from the run parameter `Combat_AnimationSpeedMultiplier` (§3.3).

<a id="s2-3"></a>
### 2.3 Attack hooks (CV3)

`Skill_AttackHookList` holds 13,733 `Skill_AttackHookInfo` structs in 13,243 skills (4 skills
have none). `Skill_AttackHook_AnimationHookIndex` sequences per skill, in stored order: (1)
12,878; (1, 2) 238; (1, 2, 3) 121; (1, 1) 3; (1, 1, 1) 2; (1, 2, 3, 4) 1. So the hook number of
a skill's *n*-th hook is *n*, except in five skills where several hooks share number 1.
`Skill_AttackHook_Cosmetic` is true in 72 hooks. The script fields of a hook are the five of
§2.1 (`…fxTargetHit`, `…fxAttackerHit`, `…fxWeaponHit`, `…BehaviorTargetHit`,
`…BehaviorWeaponHit`).

<a id="s2-4"></a>
### 2.4 Where the keys are defined (CV19)

Keys run through the closure of the entity's tables (`script-table.md` §8.2). In each of the
14 avatar closures (class tables of `script-table.md` §8.3, plus 0x07000000), the number of
distinct keys defined:

| Property | Keys defined in an avatar closure (of all keys) |
|---|---|
| `Skill_Behavior` | 1,089–1,100 of 1,763 |
| `Skill_AttackHook_BehaviorTargetHit` | 30 of 31 |
| `Skill_AttackHook_fxTargetHit` | 546 of 569 |
| `Effect_FX` | 879–882 of 1,083 |
| `Induction_AnimationScriptID` | 222–224 of 409 |
| `Channeling_AnimationScriptID` | 31 of 46 |
| `CombatState_StateData_ScriptID` | 10 of 10 |

The behaviour keys missing from the avatar closures belong to monster skills; creature tables
define them. Behaviour definitions are body-specific where the clips are: the default melee
swings of the Human male closure are defined in table 0x07000461, which only the Human,
Beorning and River-hobbit male closures include, and whose clips have the 89-bone track count;
17 other tables define the same key for the other bodies and for creatures. A clip whose track count differs from the skeleton's bone count
is refused when started (`script-table.md` §6.10 step 2), so the closure lookup is what picks
the right clip for a body; no further rule is needed.

---

<a id="s3"></a>
## 3. Skill behaviours

<a id="s3-1"></a>
### 3.1 Choosing the key (BC, CV4)

When the client shows a skill being used by a performer:

1. **List.** If the performer's `Mount_Is_Mounted` (PID 0x10000AAF) is true, the mounted lists
   (`Skill_MountedBehaviorSelfList`, `…OtherList`) are used, otherwise the plain ones.
2. **Self or Other.** The client compares the performer's instance ID with an ID its caller
   passes: equal selects the Self list, different the Other list. The path that plays a skill
   the local player has just used passes the local player, so the Self list is the local
   player's own view of its skills (Inferred from the list names; the comparison is BC).
3. **Entry.** With *n* entries: *n* = 0 plays nothing; *n* = 1 takes the entry; otherwise one
   entry chosen uniformly at random from the *n* (the class library's integer random draw over
   [0, *n* − 1]).
4. **Key.** The entry's `Skill_Behavior`; 0 plays nothing.

The three default-attack swing keys of `game-controls.md` §5.5 are therefore chosen at random
per swing.

<a id="s3-2"></a>
### 3.2 Starting the behaviour (BC, CV5, CV7)

The key is started on the performer like any script (`script-runtime.md` §12.1: lookup through
the performer's closure, instance limit, run). The client gives the run a run-parameter set.
Properties the behaviour scripts read from that set (script-parameter properties, CV7):

| Run parameter | Use in the data |
|---|---|
| `Combat_AnimationSpeedMultiplier` (PID 0x100008FA, float, dictionary default 1.0) | drives `AnimationScript_Speed` of 6,212 animation nodes and of 600 Timer nodes, and `TimerScript_SpeedMultiplier` of 156 Timers, in behaviour scripts (§3.3) |
| `StorySystem_StoryID` (0x48E) | the story the run belongs to; the hook events of §4.2 carry it |
| `SCP_Output_GetUpcomingTargets_TargetArray` (0x100011AC) and the other `SCP_*` outputs | written by the Execute callbacks of §4.6 |

When a driven source has no value, the driven field keeps its stored value
(`script-runtime.md` §5.2): without `Combat_AnimationSpeedMultiplier` a behaviour plays at its
stored speed. How the original forms the run-parameter set of a behaviour (which values it
holds besides these, and where `Combat_AnimationSpeedMultiplier` comes from) is not in the
client's code by literal (CV17): Q-CV1.

<a id="s3-3"></a>
### 3.3 What a behaviour plays (CV6)

Over the 1,763 behaviour keys, following Scriptlet invocations through every defining table
(not following Run nodes), the keys whose scripts contain: animation nodes 856; Attack hooks
889; Defend hooks 669; Particle nodes 1,248 (never-ending ones 92); Beam nodes 185; FireMissile
nodes 8; MeshFX 376; Sound 1,386; EntityVFX 367; Decal 187; Light 10; CameraShake 86;
after-effects 11.

Animation nodes reachable from behaviour keys (8,710):

| Layer (enum 0x230000D5) | Nodes |
|---|---|
| 40 `attack` | 6,384 |
| 30 `emote` | 848 |
| 58 `mounted_attack` | 542 |
| 15 `idle_emote` | 328 |
| 20 `movement` | 208 |
| 52, 35, 10, 37, 78, 76, others | 89, 88, 59, 46, 42, 23, 53 |

- **Upper body and full body.** 8,177 of the nodes have `UseBoneOpacity` = 1, and 5,853 drive it
  from `ScriptSystem_CurrentlyMoving`: while the performer moves, the clip's per-track weights
  apply and an attack clip with the upper-body weight pattern of `skeleton-anim.md` §6.6 leaves
  the legs to the locomotion layer; standing, the clip plays on the whole body. Nothing else
  selects upper-body or full-body playback; the layer budget of `skeleton-anim.md` §6.6 does
  the rest (layer 40 is above movement 20 and the idles 10 and 15).
- **Rate.** 6,212 animation nodes drive `AnimationScript_Speed` from
  `Combat_AnimationSpeedMultiplier`; 630 from `Movement_SpeedMultiplier`.
- **Hook timers.** 1,969 Timer nodes carry a clip in `TimerScript_ToolsAnimationOverride`
  (the clip their ports are timed against) and hold the StoryHook and sound events of an
  attack: for example the Human male one-handed swing plays its clip on layer 40 with a ramp-in
  of 0.2 s, and a separate Timer of the same rate fires a Defend hook at 0.371 s and an Attack
  hook at 0.416 s (clip seconds; event timing `script-table.md` §9).

<a id="s3-4"></a>
### 3.4 Weapon style and the clip for a body

Behaviour scripts pick their clips through the weapon-style templates (for example scriptlet
0x0C000021 for melee swings), which switch on `Entity_Mode` (the stance of §6), `Agent_Class`
and `Inventory_ImplementTypeSecondary` and bind one subtree per style (one-handed, with
shield, two-handed, two-handed axe, dual wield, spear, spear and shield, staff, staff-sword,
unarmed, bow, crossbow, rune-stone, battle gauntlets, a default, and class variants). The stance
therefore selects the swing; the closure selects the species (§2.4).

<a id="s3-5"></a>
### 3.5 Inductions and channels

The records are `interaction.md` §3.8 (induction, class 1195) and §3.9 (channelling, class
961); a skill names them by `Skill_InductionAction` and `Skill_Toggle_ChannelingState`
(`combat-data.md` §7.9). Their script properties (§2.1) name, by their labels:

| Moment | Performer | Target |
|---|---|---|
| induction running | `Induction_AnimationScriptID` (or `Induction_MountedAnimationScriptID` when mounted), `Induction_FXID` | `Induction_TargetAnimationScriptID`, `Induction_TargetFXID` |
| induction failed (interrupted or cancelled) | `Induction_FailureFXID` | `Induction_TargetFailureFXID` |
| induction completed | the skill's behaviour (§3.1); the induction keys are typically the `…_Start` half of a pair whose `…_End` half is the skill's behaviour | |
| channel started | `Channeling_StartScriptID`, `Channeling_StartScript_FXID` | `Channeling_Target_Start_Script`, `Channeling_TargetStartScript_FXID` |
| channel running | `Channeling_AnimationScriptID`, `Channeling_AnimationScript_FXID` | `Channeling_Target_Animation_Script`, `Channeling_TargetAnimationScript_FXID` |
| channel ended | `Channeling_Success_Script` / `…Failure_Script` / `…Abandoned_Script` and their `…_FXID` | the `Channeling_Target_…` and `Channeling_Target…Script_FXID` counterparts |

Content (Scriptlet invocations followed, Run nodes not): of the 409 induction animation keys,
294 hold animation nodes (1,087 of 4,098 nodes looping) and 163 never-ending Particle nodes;
of the 44 induction FX keys 33 hold never-ending particles. Induction and channel scripts
therefore run until they are stopped, as their roles imply. No native code or class-library
body names these properties by literal (CV17); which client path starts and stops them is
Q-CV2, and §9 gives the Tessera rule.

---

<a id="s4"></a>
## 4. Strikes, hooks and hit visuals

<a id="s4-1"></a>
### 4.1 Stories and strikes (BIN, CV8)

Each entity has a combat-presentation component holding, per story ID, the story's list of
**strikes**. A strike carries:

| Field | Meaning |
|---|---|
| kind | a code; values 1, 0x100, 0x200, 0x8000, 0x10000 are tested (§4.4); their meanings come with the session's data and were not traced |
| hook number | which hook of the performer's behaviour releases it |
| target | the entity that shows the strike |
| attack result | `CombatStrikeData_AttackResult` values (mapper 0x2300002A: 0 Undef, 1 Evade, 2 PartialEvade, 3 Block, 4 PartialBlock, 5 Parry, 6 PartialParry, 7 Resist, 8 Immune, 9 Hit, 10 CriticalHit, 11 SuperCriticalHit, 12 Miss, 13 Deflected, 14 Failure, 15 Interrupt) |
| damage type | mapper 0x23000028 (bit values; 0 Undef) |
| vital | mapper 0x23000003 (1 Health, 2 Power, 3 MountEndurance, 4 MountPower) |
| amount | for the intensity parameter (§4.4) |
| script | an optional script key that replaces the hook scripts (§4.4) |

The strikes reach the component with the session's data; how is out of scope (decision 0004).

<a id="s4-2"></a>
### 4.2 Hook events (BIN, CV8)

When a StoryHook node runs in a script on the performer, the component receives the hook type
(1 `Defend` or 2 `Attack`) and the run's `StorySystem_StoryID`. For that story:

1. The component keeps one counter per hook type, starting at 0. It increments the counter of
   the event's type; the new value *n* is the hook number of this event.
2. It takes the strikes of the story whose hook number is *n* and whose kind passes the type's
   mask: for an `Attack` event every kind passes; for a `Defend` event only kind 0x100.
3. It shows each taken strike (§4.4), in list order, and removes it from the story.

So the *n*-th Attack hook of a behaviour releases the strikes of hook number *n*
(`Skill_AttackHook_AnimationHookIndex`, §2.3), and a Defend hook releases only the strikes of
the defensive kind with the same number. Counting is per story: the hooks of a projectile's
arrival (§4.7) and of a swing count together if they run in the same story.

<a id="s4-3"></a>
### 4.3 End of a story (BIN, CV9)

When the story ends (its behaviour run ends), the component either shows every remaining
strike of the story at once (the normal end) or drops them (one end kind, which by its use is
the discarding of a story that did not happen; Inferred). Then the story is forgotten.

Strikes whose hook never fires are therefore shown when the behaviour ends. In the data this
is common: in the Human male closure, 3,728 skills with one hook have a behaviour that fires
no Attack hook at all (§10.4 lists the comparison).

<a id="s4-4"></a>
### 4.4 Showing one strike (BIN, CV10, CV11)

1. **Kind gate.** Kind 1 is shown. Kinds 0x100, 0x200 and 0x10000 are shown only when the
   target's `LifeState` is Alive; after showing such a strike the client also hands it to the
   class library (not traced; no visual found). Kind 0x8000 is not shown as a strike (it
   is handled only when the attacker is the local player, by interface code; Inferred).
2. **Hook data.** The client asks the class library for the hook data of (the story's skill,
   the strike's hook number); without hook data nothing is shown.
3. **Attacker.** If the hook's `Skill_AttackHook_fxAttackerHit` is not 0, it is run on the
   attacker.
4. **Weapon.** If the attacker wields a weapon object, its `Skill_AttackHook_fxWeaponHit` and
   `Skill_AttackHook_BehaviorWeaponHit` are run on that weapon (Q-CV4 on their order).
5. **Target.** If the strike names its own script, that key alone is run on the target;
   otherwise `Skill_AttackHook_BehaviorTargetHit` and then `Skill_AttackHook_fxTargetHit`,
   each when not 0. Both runs get the **strike run parameters**:

| Run parameter (PID) | Value |
|---|---|
| `CombatStrikeData_AttackerID` (0x1000193F) | the attacker's instance ID |
| `CombatStrikeData_AttackResult` (0x100006BD) | the strike's attack result |
| `CombatStrikeData_DamageType` (0x100006BC) | the strike's damage type |
| `CombatStrikeData_Vital` (0x1000078C) | 1 for kind 0x10000, else the strike's vital |
| `CombatStrikeData_TargetToAttackerAngle` (0x1000078B) | an angle computed from the target's and the attacker's positions (unit and reference not traced, Q-CV5) |
| `CombatStrikeData_Intensity` (0x10000879) | a float computed from the target and the strike's amount (formula not traced, Q-CV5) |
| `ScriptSystem_Snd_StrikingImplementType` (0x10000A4C) | from the attacker's `Combat_SkillMode` (mapper 0x2300002F): Melee, Ranged or Thrown take the attacker's `Combat_DefaultStrikingImplementType_Melee`, `…_Ranged` or `…_Thrown`; no mode or a value 0 gives 25 |
| `ScriptSystem_Snd_StruckSurfaceType` (0x10000211) | from the target's materials: for a Parry its `Material_WeaponMaterial`, for a Block its `Material_OffHandMaterial`; otherwise, or when that is absent, `Material_TorsoMaterial`, then `Content_IAmMadeOf` |

The sound parameters only select sounds; the visual rules depend on the attack result and the
damage type.

<a id="s4-5"></a>
### 4.5 What the hit scripts do (Confirmed, CV12)

- **Reactions** (`Skill_AttackHook_BehaviorTargetHit`; 31 keys; the common one is
  `Cbt_Melee_Hit`, named by 6,495 of the 7,170 non-zero values). In the Human male closure its body-specific
  definition (table 0x07000461) invokes a reaction template (scriptlet 0x0C00003F) that, unless a mounted channel is running, chooses
  by the target's `CombatState_CurrentState` (knocked-down states with Hit or CriticalHit play a
  knocked-down hit) and then by `CombatStrikeData_AttackResult`: {Evade, PartialEvade},
  {Parry, PartialParry}, {Resist}, {Hit, CriticalHit, SuperCriticalHit}, {Block, PartialBlock};
  other results (Miss, Immune, Deflected, …) play no reaction. Each group binds a stance
  template (0x0C000087, switching on `Entity_Mode`, `Agent_Class` and
  `Inventory_ImplementTypeSecondary`) to one reaction clip per stance. Reaction clips play on
  layers 40, 30, 15, 37 and 43 (§10.3).
- **Impacts** (`Skill_AttackHook_fxTargetHit`, 569 keys, and the shared definition of
  `Cbt_Melee_Hit` in a closure-wide table). They switch on `CombatStrikeData_AttackResult`
  (critical results add element or blood impacts), `CombatStrikeData_DamageType` (element
  variants), `WeenieType` (player or not) and, through a template (0x0C002B3A), on the player's
  options `GOP_CombatHitFX` and `GOP_EnableGore` (both default true; the template reads them on
  `ThePlayer`). Their content is mostly finite particles (455 of 569 keys), sounds (318) and
  entity colour effects (162).
- **Attacker effects** (`Skill_AttackHook_fxAttackerHit`, 67 keys): finite particles and sounds
  on the attacker at the moment of the hit.
- In the Human male closure 424 script definitions switch on `CombatStrikeData_AttackResult`;
  one also on `CombatStrikeData_DamageType` directly.

<a id="s4-6"></a>
### 4.6 Upcoming targets (Confirmed data, CV13)

Execute nodes (`script-table.md` §5.2) name a callback of DID map 0x28000007. In all tables and
scriptlets: `GetUpcomingTargets` 625 nodes, `ExecuteScript_AssertMsg` 125, none (0) 88,
`ExecuteScript_DistantGroup` 64, `ExecuteScript_UpdateBallisticTargetReticle` 10,
`GetUpcomingHitResult` 4, five others with 1–2 each.

A `GetUpcomingTargets` node passes, in its call properties, a filter
`SCP_GetUpcomingTargets_CombatResultType_Filter` (an array of attack results, mapper
0x2300002A) and has a result port (type 5) whose payload names `SCP_GetUpcomingTargets_Result`
with value 1. Its outputs are `SCP_Output_GetUpcomingTargets_TargetArray` (an array of
instance IDs) and the result flag. Combat scripts aim at the array through Property targets
(`script-table.md` §4.4, type 3) or a Random target over it (type 6 with a Property
sub-target). A typical thrown attack (`Cbt_Projectile_ThrowingAxe_A`): one Execute filtered to
{Hit, CriticalHit, Evade, Block}; on success a Beam whose head flies to one of the returned
targets; otherwise a second Execute filtered to {Miss} and, on success, one of four Beam
variants at random. So a projectile flies to each target that has an upcoming strike, and a
miss flies too.

Inferred reading (the native callback was not traced, Q-CV6): the "upcoming" targets are the
targets of the story's strikes not yet shown whose attack result passes the filter.
`GetUpcomingHitResult` (with `SCP_GetUpcomingHitResult_AnimHookType`) answers the attack
result of the next strike for a hook type in `SCP_GetUpcomingHitResult_Result`.

> **OPEN:** (implementer, 2026-10-11) two target rules the walker needs are not stated. (1) A
> Random target (type 6, `RandomTarget_Cap` *c*) over Property sub-targets: `tessera-scriptgraph`
> draws max(*c*, 1) distinct entities uniformly (the runtime's generator) from what the
> sub-targets resolve to. (2) A Particle node whose targets resolve to several entities:
> the effect starts on the first one only (its never-ending instance keeps one handle); when
> they resolve to none (an empty array, or the Default port after a failed
> `GetUpcomingTargets`), nothing starts, as for a Beam node without a target (see the OPEN of
> `particle-runtime.md` §5.5, which also limits this to Property and Random targets).

<a id="s4-7"></a>
### 4.7 Projectiles and beams (CV14)

A projectile is a Beam node with a head entity (`BeamScript_HeadEntity`, 1,050 of the 2,740
Beam nodes) that travels from the performer to the target at `BeamScript_HeadAnimSpeed` metres
per second (40, 50 and 30 m/s are the common values among head-entity beams). When the head
arrives the Beam node fires its Default port (`particle-runtime.md` §11.1 step 7), and 947 Beam
nodes have an Attack StoryHook under that port: the strike of the hit is released at the
impact, not at the throw. Beams with `BeamScript_HeadStartPos` = 1 (777 nodes) are drawn at
full length from their start (rays, chains). The beam renderer itself (envelope, endpoints,
head and tail motion, ribbon, particle systems, head entity) is specified in
`particle-runtime.md` §11.1.

> **OPEN:** (implementer, 2026-10-11) the frame of `BeamScript_TargetPosition` (a Position
> with optional landblock, cell and offset) is not stated for `UseTargetPosition` = 1 (one node
> in the data). The walker treats such a beam without a target entity as untargeted (source
> point + forward × `UntargetedBeamLength`).

Beam census (all 2,740 nodes; 1,496 reachable from skill and effect keys, §10.2):

| Field | Values |
|---|---|
| `BeamScript_Persistent` | 1 in 769 |
| `BeamScript_HeadEntity` | set in 1,050 |
| `BeamScript_BeamParticleSystem`, `…HeadParticleSystem` | set in 571 and 45 (2 of the 45 without a beam particle system) |
| ramp in, sustain, ramp out (s) | (0, 1, 0.25) 947; (0, 1, 0) 426; (0, 0.35, 0.25) 234; (0, 10, 0) 123; others |
| head start, start time, speed | (1, 0, 10) 410; (0, 0, 40) 328; (0, 0, 50) 296; (0, 0.01, 45) 228; (1, 0, 0) 220; others |
| hooks under the port | Attack only 945, Attack and Defend 2, Defend only 33; 4 of the hook-carrying nodes have a head speed of 0 and never arrive |
| targets | This 1,235; Random over a Property 981 (+47 with several sub-targets); Property 253; Link 147; Child 55; Initiator 2; none 20 |

<a id="s4-8"></a>
### 4.8 FireMissile (CV15)

The 183 FireMissile nodes (118 reachable from combat keys) launch one of 11 missile objects
(0x70 records) with `FireMissileScript_AffectedByGravity` in all, `…NeverFireAtTarget` in 157,
an initial speed of 35 m/s in 124 and a lifetime of 10 s in 163; none has a StoryHook under a
port. They are thrown props (siege and interactive objects), not skill projectiles. Tessera
does not simulate them in P2 (§11 rule 10).

---

<a id="s5"></a>
## 5. Effect visuals

<a id="s5-1"></a>
### 5.1 Data (Confirmed, CV16)

- `Effect_FX` (16,187 effects; 8,489 non-zero; 1,083 keys): following Scriptlet invocations,
  732 keys hold Particle nodes (382 with never-ending ones), 237 MeshFX, 223 EntityVFX, 135
  animation nodes (layers 30, 40, 78, 20 and 15 most), 60 Beam nodes, 35 CameraShake, 35 Decal,
  63 after-effects, 10 Light nodes, 419 sounds. Never-ending content means "for as long as the
  effect is applied"; finite content is a one-shot at application (for example the heal
  burst).
- `Effect_VitalOverTime_IntervalFX` (2,438 effects of class 755; 46 keys): finite particles
  (43 keys), EntityVFX (21), sounds (31); a per-pulse visual.
- The mount scripts (`Mounted_StartScriptID`, `Mounted_AnimationScriptID`,
  `Dismounted_ByPlayer_Script`, `Dismounted_ByOther_Script`) belong to the mount specs; P2
  plays none of them.
- Effects of class 769 (`Effect_InduceCombatState_StateToInduce`, 239 effects) name a combat
  state bit; the state's `CombatState_StateData_ScriptID` (§6.3) is the visual of that state.

<a id="s5-2"></a>
### 5.2 Attachment, start, loop, stop, linger

An effect's script runs on the affected entity; its Particle nodes attach to bones and holding
locations of that entity as `particle-runtime.md` §5–§6 states (no rule is specific to
effects). When the script's run is stopped, never-ending Particle nodes stop and linger
(`particle-runtime.md` §7.1–§7.3: at most 6 s), finite ones play out, other node kinds stop as
`script-runtime.md` §3.5 states. The client's own trigger for starting and stopping these
scripts is not in its code by literal (CV17; Q-CV2); §9 gives the Tessera rule.

---

<a id="s6"></a>
## 6. Combat stance, idle in combat, combat states

<a id="s6-1"></a>
### 6.1 Properties the tables read (Confirmed, CV20)

- **`Entity_Mode`** (PID 0x5A2, mapper 0x23000092, an entity property): the stance. Values the
  templates test: 0x10000003 `combat_monster`, 0x10000004 `combat_1hshield`, 0x10000005
  `combat_1h`, 0x10000006 `combat_2h`, 0x10000007 `combat_staff`, 0x1000000A `combat_magic`,
  0x1000000B `combat_spear`, 0x1000000C `combat_dualwield`, 0x10000011 `combat_throw`,
  0x10000012 `combat_staffsword`, 0x10000013 `combat_ua`, 0x10000014 `combat_2haxe`,
  0x10000015 `combat_bow`, 0x10000018 `combat_spearshield`, 0x1000001B `combat_gauntlets`
  (avatar locomotion template 0x0C000FFE; the creature template 0x0C0000B9 tests twelve of
  them, without `combat_magic`, `combat_staffsword`, `combat_2haxe` and `combat_gauntlets`);
  the weapon-style templates also test 0x1000000F `combat_xbow` and 0x10000019 `combat_rune`.
  Other values of the mapper: 0x10000001 `looted`, 0x10000002 `combat`, 0x10000008 `open`,
  0x10000009 `closed`, 0x1000000D `holding_container`, 0x1000000E `chargen_stand`, 0x10000010
  `dead`, 0x10000016 `chargen_sit`. With no value, every template takes its Default (peace)
  branch.
- **`Combat_Violent`** (PID 0x10000C6E, bool, entity): in combat. In 12 of the 14 avatar
  bindings it turns the idle emote off (`script-table.md` §8.3); in the Human male closure 267
  script definitions reach a Switch on it.
- **`Combat_AnimationSet`** (PID 0x10000469, mapper 0x2300010C): monster animation sets; avatar
  run bindings switch on it; players leave it unset.

The idle in combat is therefore the locomotion template's stance branch for the current
`Entity_Mode` (stance idle, walk and run), with the idle emote suppressed by `Combat_Violent`.

<a id="s6-2"></a>
### 6.2 Who sets them

No native code or class-library body sets `Entity_Mode` by literal (CV17); the stance arrives
as an entity property with the session's data (Inferred). The rule that maps equipment to a
stance is therefore not in the data or the client (`game-controls.md` §5.5 Q9); §11 rule 6
gives Tessera's.

<a id="s6-3"></a>
### 6.3 Combat states (Confirmed)

The combat-state control (`game-controls.md` §5.8) gives 13 `CombatState_StateData_ScriptID`
values (10 keys): `Effects@Agent::Fear` (two states), `Dazed@Agent::*Dazed`,
`Dazed@Agent::*Dazed_conj` (two), `Dazed@Agent::*Stunned`, `Knockdown@Agent::*Knockdown` (two),
`DuelingDefeat@Agent::*DuelingDefeat`, two emote keys (kneel, cower) and two combo-start keys.
A state's script is its visual while the state lasts. The state bits that effects induce
(`Effect_InduceCombatState_StateToInduce`) are, by count: 0x4000 (61 effects), 0x200 (42),
0x4 (38), 0x1000 (24), 0x100 (16), 0x2 (13), 0x1 (8), 0x40 (4), 0x1000000 (3).

---

<a id="s7"></a>
## 7. Defeat, death pose, revive (Confirmed, CV18)

The avatar closures define (in the shared table 0x07000021 and the species tables):

- **`Death@Agent::*Die`** (0x100007B0, defined in 41 tables, every avatar closure): in the
  shared avatar definition a Sequence of three steps. Step 1: when `Death_DiedWhileKnockedDown`
  is true, stop the dazed visual and start `die_knockdown`; otherwise a Switch on
  `Death_DeadScript` starts the death clip script matching its value (the case values are the
  keys `dead01` … `dead05` and `dead_knockdown`, the scripts started `die01` … `die05`; there is
  no default case, so with no value nothing plays); and start `Story@Agent::StopCurrentAttack`.
  Step 2: the same Switch, through a WhenRunning on the `Death_DeadScript` key, starts the
  matching hold (`die_hold01` … `die_hold05`, `die_knockdown_hold`) while that key is not
  running. Step 3: a Run whose key is driven from `Death_DeadScript` starts that key. 37 of the
  41 definitions (creature tables) play a death clip without the `Death_DeadScript` switch. In
  the Human male species table the `dieNN` scripts play one-shot clips on layer 78 `death` and
  the holds looping clips on layer 78 (or 76 in a water variant, by `Entity_Element`).
- **Holds** (`die_holdNN`): the pose is kept; a persistent When on `LifeState` stops the hold
  when `LifeState` becomes Awakening, Reviving or Alive.
- **Start-up** (0x0700003D, kind 1): when `LifeState` is Dead and `CombatState_CurrentState` is
  knocked down, sets `Death_DiedWhileKnockedDown`.
- **`Death@Agent::*Revive`** (0x10000A52, 5 tables, every avatar closure): stops the
  incapacitation and hold scripts and plays the recovery (`Incap_Recover`).
- Also in every avatar closure: `Death@Agent::FadeOut_` (0x10000949, the corpse fade), `Death@Agent::*Incapacitate` (0x100004AF),
  `Knockdown@Agent::*Knockdown`, `…*Knockback`, `Dazed@Agent::*Dazed`, `…*Stunned`.

`Death_DeadScript` (PID 0x10000135) and `Death_DiedWhileKnockedDown` have no stored value in
the archives; they are run-time entity state. Layer 78 is the highest regular layer, so the
death clips override everything below (`skeleton-anim.md` §6.6).

---

<a id="s8"></a>
## 8. What the original leaves to its session data

Because the client's code names none of them by literal (CV17), the following arrive with the
session's data in the original (Inferred) and are Tessera rules here (§9, §11): when induction,
channel, effect, combat-state, death and revive scripts start and stop; the stance
(`Entity_Mode`); the behaviour's `Combat_AnimationSpeedMultiplier`; the strikes and their hook
numbers. `LifeState`, `Death_DeadScript` and `Combat_Violent` are entity properties (CV7) whose
writer was not traced (native code reads `Combat_Violent` in 79 places, the interface's combat
indicators among them); Tessera sets them from the session events (§9.5).

---

<a id="s9"></a>
## 9. Timing against the session events (Decision)

The session events are those of `scenario-feed.md` §9.2 (fields in parentheses). "Run *K* on
*E*" means a script start of key *K* on entity *E* (`script-runtime.md` §12.1), skipped when
*K* is 0 or *E* is not drawn as a figure; "stop" stops every run of that start. Each row is a
Tessera rule; the facts it rests on are cited.

> **OPEN:** (implementer, 2026-10-11) when a figure becomes available after the events that
> concern it (its figure still building when a scenario's first steps run), the walker shows
> the persistent state on it at once (stance and `Combat_Violent`, applied effects' runs,
> `LifeState` and the death script of a defeated entity) and drops the one-shot visuals that
> passed (casts, strikes). The figures of session entities run no start-up scripts in the
> walker's effects runtime (the character layer animates them, `walker.md` §10), so a
> start-up script such as the knocked-down one of §7 does not run on them.

<a id="s9-1"></a>
### 9.1 Casts

`CastStarted (entity, cast, what, target, duration_ms)` with `what` a skill *S*:

| `cast` | At `CastStarted` | At `CastEnded (outcome)` |
|---|---|---|
| `Cast`, *S* without `Skill_InductionAction` | start the **story** of *S* on `entity` (§9.2) at once | completed: nothing more; interrupted or cancelled: the story's run is stopped and its strikes dropped (§4.3 discarding end) |
| `Induction`, or `Cast` with *S* having `Skill_InductionAction` | run the induction's caster keys (§3.5; the mounted animation key when `Mount_Is_Mounted`) on `entity` and its target keys on `target` | completed: stop the induction runs, then start the story of *S*; interrupted or cancelled: stop the induction runs, run `Induction_FailureFXID` on `entity` and `Induction_TargetFailureFXID` on `target` |
| `Channel` (*S* names `Skill_Toggle_ChannelingState`) | run the start and animation keys and their target counterparts (§3.5) | stop the start and animation runs; run the success (completed), failure (interrupted) or abandoned (cancelled) keys and their target counterparts |
| `Craft` | nothing (crafting animations are not in P2) | nothing |

`what` that is not a skill (an item, a recipe) plays nothing. A second `CastStarted` on an
entity with a running induction or channel stops those runs first (the feed sends
`CastEnded (interrupted)` before it, `scenario-feed.md` §9).

<a id="s9-2"></a>
### 9.2 Stories

Starting the story of skill *S* on performer *P* with optional target *T*:

1. Choose the behaviour key by §3.1 (Self list when *P* is the local player).
2. Allocate a story ID (a counter per client session) and an empty strike list with both hook
   counters at 0. Record *S*, *P*, *T* and the start time.
3. Set *P*'s `Combat_SkillMode` to *S*'s `Skill_AnimationMode` (used only by §4.4's sound
   parameter).
4. Run the behaviour on *P* with the run parameters `StorySystem_StoryID` = the story ID and
   `Combat_AnimationSpeedMultiplier` = 1.0 (rule 4).
5. The story ends when the behaviour's run ends, or 30 s after its start (an engineering
   limit for looping behaviours); then the remaining strikes are shown (§4.3 normal end).

At most one story per (performer, skill) is open: starting a new one ends the previous one
first.

<a id="s9-3"></a>
### 9.3 Strikes from `VitalDelta`

`VitalDelta (source, target, vital, kind, amount, result, what, damage_type)` with `what` a
skill *S* and `source` *P*:

1. **Strike.** Kind 1. Attack result from `result`: Normal → Hit (9), Critical → CriticalHit
   (10), Miss → Miss (12), Evade → Evade (1), Parry → Parry (5), Block → Block (3), Resist →
   Resist (7), Immune → Immune (8). Damage type: `damage_type` when present, otherwise the
   `Skill_AttackHook_DamageType` of the hook chosen in step 2. Vital: health 1, power 2, other
   resources 0. Amount: `amount`.
2. **Hook number.** If *P* has an open story of *S*, the strike gets hook number *i* = 1 + the
   number of strikes of that story for the same `target` received so far. If *S* has fewer than
   *i* hooks, the strike is shown at once with the skill's last hook number; if the story's
   Attack counter is already ≥ *i*, it is shown at once with number *i*; otherwise it is added
   to the story and waits for its hook (§4.2).
3. **No open story** (the scenario sends damage without a cast, or after the story ended): the
   strike is shown at once with hook number 1 (a skill without hooks shows nothing, §4.4 step
   2).
4. **Heals.** `kind` Heal is a strike like any other (its result is normally Normal); the hook
   scripts of heal skills are their heal impacts.
5. **Effect pulses.** `VitalDelta` whose `what` is an effect of class 755 runs that effect's
   `Effect_VitalOverTime_IntervalFX` on `target` at once (no story).
6. `VitalDelta` with no `what`, or with `what` naming something else, shows no strike visual.

> **OPEN:** (implementer, 2026-10-11) step 2's "fewer than *i* hooks" is read against the
> skill's **largest hook number**, not its number of hooks. The two differ only for the
> skills whose hook numbers repeat ((1, 1) or (1, 1, 1), §2.3): with the count, a second
> strike on a target would wait for hook number 2, which those skills do not have, so §4.4
> step 2 would find no hook data for it. The walker shows such a strike at once with the
> largest number (1). Which reading the original follows, and which hook data a repeated
> number uses, is not stated.

Showing a strike follows §4.4 steps 2–5 with the hook data of (*S*, hook number), the strike
run parameters of §4.4 (the angle and intensity by rule 5), on the entities the event names.
Defend hooks release nothing in Tessera (rule 3).

The floating text and the vitals of `combat-ui.md` are not delayed by this rule; whether the
original delays them until the hook is that spec's question.

<a id="s9-4"></a>
### 9.4 Effects

| Event | Visual |
|---|---|
| `EffectApplied (entity, effect, what, duration_ms, stacks, source)` | run `what`'s `Effect_FX` on `entity`, keyed by the instance id `effect`; for an effect of class 769, also run the induced state's `CombatState_StateData_ScriptID` (§6.3) on `entity` |
| `EffectRemoved (entity, effect)` | stop the runs started for that instance (never-ending particles linger, `particle-runtime.md` §7) |

Stacks do not change the visual. Instances that run the same key on the same entity share one
run: it starts with the first instance and stops when the last of them is removed (a second
start would be refused by `Script_MaxNumInstances`, `script-runtime.md` §12.3, unless the
table allows more).

<a id="s9-5"></a>
### 9.5 Combat state, defeat, revive

| Event | Visual |
|---|---|
| `CombatState (entity, in_combat = true)` | set `Combat_Violent` = true and `Entity_Mode` = the stance of rule 6 on `entity` |
| `CombatState (entity, in_combat = false)` | set `Combat_Violent` = false and clear `Entity_Mode` |
| `Defeated (entity, by)` | end every open story of `entity` (normal end); stop its induction, channel and effect runs; set `LifeState` = 5 (Dead); set `Death_DeadScript` to one of the case values of the Switch on `Death_DeadScript` in the entity's `Death@Agent::*Die` definitions, chosen uniformly at random (none when there is no such switch); run `Death@Agent::*Die` (0x100007B0); clear `Combat_Violent` and `Entity_Mode` |
| `Revived (entity)` | set `LifeState` = 4 (Alive); run `Death@Agent::*Revive` (0x10000A52); clear `Death_DeadScript` and `Death_DiedWhileKnockedDown` |

Property changes reach the running scripts as `script-runtime.md` §5.3 and §6 state (Switch
and When re-evaluation), so the stance, the idle emote and the death holds follow at once.

<a id="s9-6"></a>
### 9.6 Order within one instant

Events of one instant are handled in feed order. The usual scripted order — `CastEnded`, then
`VitalDelta`, then `EffectApplied` — starts the story first, queues the strikes for its hooks
and starts the effect visuals at once; the effect does not wait for the hook.

---

<a id="s10"></a>
## 10. Census for exit criterion 3 (Confirmed, CV21)

<a id="s10-1"></a>
### 10.1 Particle descriptors

Every record decodes: 11,155 of 11,155 with prefix 0x39 (`particles.md` §4) and 254 of 254 with
prefix 0x44 (outer layer, `particles.md` §6.1). By what Tessera simulates and draws:

| Category (rule) | 0x39 records | Simulated and drawn by Tessera |
|---|---|---|
| plain: surface, at least one emitter, `uses_video_post` 0, `use_lighting` 0, `inherit_opacity` 0 | 9,829 | yes |
| `use_lighting` 1 (`particles.md` §8.14) | 1,055 (+5 also post-pass, +1 also `inherit_opacity`) | yes, lit by `particle-runtime.md` rule 13 |
| `uses_video_post` 1: drawn in the pass after post-processing (`particles.md` §8.15) | 134 (+5 lit) | simulated; drawn in that pass when the renderer has it, otherwise in the main pass (rule 9) |
| `inherit_opacity` 1 | 128 (+1 lit) | yes, with the owner opacity of `particle-runtime.md` Q-RT7's Decision |
| no surface | 2 | simulated, never drawn (`particles.md` §8.11) |
| no emitter | 1 | nothing to simulate |
| 0x44 fork descriptors | 254 (flag 1 in 92) | lifetime only, not drawn (`particles.md` §10.2) |
| weather and water systems (UNIQUEDB, `particles.md` §7.1.3) | 10 | not reached by combat |

Never-ending by the rule of `particle-runtime.md` §5.3: 4,592 of the 0x39 records.

<a id="s10-2"></a>
### 10.2 Nodes reachable from skill and effect records

Reachable = in a definition of a key that a skill or effect property of §2.1 names, or in a
scriptlet such a definition invokes (transitively), in any table; Run nodes not followed.

| Population | Particle nodes | Of which never-ending / dropped | Distinct effect DIDs (0x39 / 0x44) | Beam nodes | Distinct beam systems | FireMissile nodes |
|---|---|---|---|---|---|---|
| skill and effect keys | 20,689 | 5,193 / 1,958 | 6,598 (6,456 / 142) | 1,496 | 66 | 118 |
| plus induction, channel and combat-state keys | 22,366 | 6,006 / 2,038 | 6,941 (6,779 / 162) | 1,577 | 71 | 118 |
| all these keys, Human male closure only, Run nodes followed within the closure | 6,932 | 978 / 930 | 2,736 (2,624 / 112) | 413 | 28 | 50 |

Categories of the 6,598 effect DIDs reached from skill and effect keys: plain 5,819; lit 522
(+3 post-pass and lit, +1 lit and `inherit_opacity`); fork 142; post-pass 68; `inherit_opacity`
42; no emitter 1. All 66 beam systems are plain. No node names an effect DID that does not exist
(every reachable DID decodes).

<a id="s10-3"></a>
### 10.3 Animation layers of hit reactions

Reachable from `Skill_AttackHook_BehaviorTargetHit` (31 keys): 619 animation nodes, on layers
40 (260), 15 (152), 30 (121), 37 (27), 58 (22), 20 (17), 25 (13), 43 (3), 52 (2), 0 (1), 54 (1);
597 of them with `UseBoneOpacity`, 11 looping.

<a id="s10-4"></a>
### 10.4 Hooks against behaviours (Human male closure)

For each skill, its number of attack hooks (§2.3) against the distinct numbers of Attack
StoryHooks found per timed unit (a Timer or animation node with hook ports) reachable from its
Self behaviour in the Human male closure (Run nodes followed in the closure); 10,831 skills
have a defined behaviour there, 2,416 do not (monster skills):

| Hooks | Attack hooks per unit | Skills |
|---|---|---|
| 1 | 1 | 4,678 |
| 1 | none | 3,728 |
| 1 | 1 or 2 (weapon-style variants) | 2,049 |
| 2 | 1 or 2 | 122 |
| 2 | none | 42 |
| 1 | 2 or 3 | 37 |
| 3 | 1 or 2 | 32 |
| 3 | none | 27 |
| others | | 116 |

A skill whose behaviour has no Attack hook shows its strikes at the end of the behaviour
(§4.3); a unit with more hooks than the skill has strikes releases nothing at the extra hooks.

<a id="s10-5"></a>
### 10.5 Real-data test expectations

A real-data census test over the install should find: every 0x39 and 0x44 record decodes and
falls into exactly one category of §10.1 (combinations counted under the first matching row);
every effect DID named by a Particle node reachable from skill and effect records exists; the
counts of §10.2 first row; every Beam node's tail decodes to the fields of
`particle-runtime.md` §11.1.

> **OPEN:** (implementer, 2026-10-11) the walker's census test
> (`apps/client/tests/combat_visuals_real_data.rs`) reaches from the skill keys it shows
> (behaviours, the five hook scripts) and the effects' `Effect_FX`, interval and threat
> interval FX, not from every skill and effect property of §2.1 (failure, conjunction,
> ground-target and mount scripts are left out, rule 10). On this install it finds 20,552
> Particle nodes (5,178 never-ending, 1,890 dropped), 6,580 effect DIDs (6,438 / 142),
> 1,496 Beam nodes and 61 beam systems, so it checks the first row as lower bounds (95 %)
> rather than exactly.

---

<a id="s11"></a>
## 11. Tessera rules (Decision)

1. **Mapping of session events** as §9.
2. **Behaviour choice** as §3.1, with the local player as the Self entity and the shared random
   source of `particle-runtime.md` §3.3 (tests seed it).
3. **Hooks.** Attack hooks release strikes as §4.2; Defend hooks release nothing, because every
   Tessera strike is of kind 1 (the defensive kind 0x100 of the original is session data whose
   meaning was not traced, Q-CV4).
4. **Animation speed.** `Combat_AnimationSpeedMultiplier` = 1.0 in every behaviour run; a
   scenario may set it on the performer as a property, which the start copies into the run
   parameters.
5. **Strike parameters.** `CombatStrikeData_TargetToAttackerAngle`: the attacker's direction in
   the target's horizontal frame, degrees in (−180, 180], 0 straight ahead, positive to the
   target's left; `CombatStrikeData_Intensity`: `amount` divided by the target's maximum of that
   vital from its last `VitalChanged`, clamped to [0, 1], 1 when unknown.
   `ScriptSystem_Snd_StruckSurfaceType`, `ScriptSystem_Snd_StrikingImplementType` as §4.4.
6. **Stance.** From the figure's worn items: main hand empty → `combat_ua`; main-hand category
   (mapper 0x2300004C) 1HSword, 1HAxe, 1HClub, 1HMace, 1HHammer or Dagger → `combat_1h`,
   `combat_1hshield` with an off-hand Shield, HeavyShield or MediumShield, `combat_dualwield`
   with an off-hand weapon; 2HSword, 2HClub, 2HMace, 2HHammer or Halberd → `combat_2h`; 2HAxe →
   `combat_2haxe`; Spear → `combat_spear`, `combat_spearshield` with a shield; 2HStaff →
   `combat_staff`; Bow → `combat_bow`; Crossbow → `combat_xbow`; Javelin or Thrown →
   `combat_throw`; BattleGauntlets → `combat_gauntlets`; any other main-hand item →
   `combat_1h`. Figures that are not player characters → `combat_monster`.

   > **OPEN:** (implementer, 2026-10-10) "an off-hand weapon" is not defined. `crates/gamedata`
   > (`stance_for`) reads it as an off-hand item of one of the one-handed categories of this
   > rule (1HSword, 1HAxe, 1HClub, 1HMace, 1HHammer, Dagger).

   > **OPEN:** (implementer, 2026-10-11) which worn item is the main hand and which the off
   > hand is not stated. The walker takes the items that are not stowed: an item with the
   > feed's `Equipped.slot` is in the hand of that slot (`Weapon_Primary`: main,
   > `Weapon_Secondary`: off; other slots, the ranged one included, hold no hand); an item
   > without it follows the hand rule of `character-panels.md` §7 (its `Inventory_DefaultSlot`
   > hand, the secondary hand when an earlier item took the primary one and it fits
   > `Weapon_Secondary`). A main-hand item outside this rule's categories gives `combat_1h`.
   > Only when the data lack the two slot labels, the first non-shield item of this rule's
   > categories is the main hand and the next item (or a shield) the off hand.
7. **Options.** `GOP_CombatHitFX`, `GOP_AvatarAttackFX`, `GOP_MonsterAttackFX` and
   `GOP_EnableGore` take their dictionary defaults (true) on the local player until an options
   panel sets them.
8. **Execute callbacks.** `GetUpcomingTargets`: the targets of the current story's strikes not
   yet shown whose attack result is in the filter (no filter: all), in strike order, without
   duplicates; when the story has no strikes yet and the filter contains Hit, the story's
   target *T* (§9.2) alone; the result flag is true when the array is not empty; the node fires
   its type-5 ports whose payload equals (`SCP_GetUpcomingTargets_Result`, the flag) and
   otherwise its Default port. `GetUpcomingHitResult`: the attack result of the first strike
   not yet shown, or Hit when there is none. Every other callback: the Decision of
   `script-runtime.md` §18 (fire the port matching `DefaultValue`).
9. **Post-processing pass.** Systems with `uses_video_post` = 1 are drawn after the main scene
   in the walker's last particle pass; a renderer without such a pass draws them with the
   others.
10. **Not simulated in P2:** FireMissile nodes (§4.8) complete at once; fork descriptors as
    `particles.md` §10.2; `Skill_ExecutionFailureScript`, `Skill_ConjunctionWindupScript`,
    the ground-target scripts and the mount scripts, which no P2 event names.
11. **Engineering limits:** at most 64 open stories per performer, 256 strikes per story,
    30 s per story; a further story or strike is shown at once.

---

<a id="s12"></a>
## 12. Invariants

1. Every skill has non-empty Self and Other lists; each mounted list has one entry (CV2).
2. Every hook number of the data is 1–4 and a skill's hook numbers start at 1 (CV3).
3. A strike is shown at most once: at its hook, at the end of its story, or at once.
4. Strikes of one story and one hook number are shown in the order they were received.
5. The behaviour run is the only script that releases a story's strikes; scripts on targets
   never do.
6. `Combat_AnimationSpeedMultiplier`, `CombatStrikeData_*`, `SCP_*` are only ever run
   parameters; `Entity_Mode`, `Combat_Violent`, `LifeState`, `Death_DeadScript` only entity
   properties (CV7).
7. Every Particle node reachable from combat records names an existing 0x39 or 0x44 record
   (§10.2).
8. Setting `LifeState` to Alive stops every death hold of the avatar closures (CV18).

---

<a id="s13"></a>
## 13. Worked synthetic examples

All records here are invented; DIDs and keys are placeholders.

<a id="s13-1"></a>
### 13.1 Two-hook melee skill

Skill *S* has two attack hooks (numbers 1 and 2), `Skill_AttackHook_BehaviorTargetHit` = *R*
in both and `Skill_AttackHook_fxTargetHit` = *F1*, *F2*. Its Self list has one key *B*, whose
definition on the performer's closure plays a 1.2 s clip and a Timer at rate 1 with Attack
hooks at 0.40 s and 0.80 s.

Feed: 0.000 `CastStarted (player, Cast, S, target = D, 0)`; 0.000 `VitalDelta (player, D, …,
result Normal)`; 0.000 `VitalDelta (player, D, …, result Critical)`; 0.000 `CastEnded`.

- 0.000: story 1 starts; strike A (Hit, hook 1) and strike B (CriticalHit, hook 2) are queued.
- 0.400: Attack counter 1 → A is shown: *R* then *F1* run on *D* with
  `CombatStrikeData_AttackResult` 9.
- 0.800: counter 2 → B is shown: *R* then *F2* with 10.
- 1.200: the run ends; the story ends with no strike left.

With a single hook in the behaviour instead, B waits until 1.200 (§4.3).

<a id="s13-2"></a>
### 13.2 Projectile

The behaviour runs a projectile key at 0.74 s: `GetUpcomingTargets` (filter Hit, CriticalHit,
Evade, Block) returns [*D*]; a Beam with head speed 40 m/s, head start 0, start time 0, from
the performer's right hand to *D*'s chest, 12 m apart. The head fraction reaches 0.9998 at
*t* = 0.9998 × 12 / 40 ≈ 0.300 s after the beam's start (`particle-runtime.md` §11.1 step 5),
so the Attack hook under the Beam's port fires at about 1.04 s and the strike is shown then.

<a id="s13-3"></a>
### 13.3 Induction interrupted

Skill *S* with induction *I* (`Induction_AnimationScriptID` = *A*, `Induction_FXID` = *X*,
`Induction_FailureFXID` = *Y*). Feed: 0.0 `CastStarted (player, Induction, S, D, 2000)`, 1.2
`CastEnded (interrupted)`. At 0.0 *A* and *X* run on the player; at 1.2 both are stopped (their
never-ending particles linger up to 6 s) and *Y* runs; no story starts, so *S*'s behaviour
never plays.

<a id="s13-4"></a>
### 13.4 Effect applied and removed

`EffectApplied (D, 7, E, 5000, 1, player)` with `Effect_FX` = *K* whose definition holds a
never-ending Particle node on a hand bone and a finite burst. At apply both start; the burst
plays out; at `EffectRemoved (D, 7)` the run stops, the never-ending system stops emitting and
is removed at most 6 s later (`particle-runtime.md` §7.3).

<a id="s13-5"></a>
### 13.5 Defeat and revive

`Defeated (D)`: `LifeState` 5, `Death_DeadScript` = one of the switch's case keys (say
`dead02`); `*Die` starts `die02` (a one-shot clip on layer 78) and the hold `die_hold02` (a
looping clip on layer 78) while `dead02` is not running, then runs `dead02`.
`Revived (D)`: `LifeState` 4 stops `die_hold02` (its When on `LifeState`); `*Revive` plays the
recovery.

---

<a id="s14"></a>
## 14. Open questions

> **OPEN (Q-CV1):** How the original forms the run-parameter set of a behaviour run (which
> values besides `StorySystem_StoryID`; where `Combat_AnimationSpeedMultiplier` comes from, for
> example the weapon's `Item_BaseAnimDurationMultiplierMod` or the skill's
> `Skill_AnimationDurationMultiplierModOverride`). Suggested method: M4 on the native functions
> of class-library package 3461 that the behaviour start calls. **Decision (Tessera):** rule 4.

> **OPEN (Q-CV2):** Which client path starts and stops the induction, channel, effect,
> combat-state and death scripts (no literal reader exists, CV17; presumably a generic "run
> script" message from the server). Suggested method: M4 on the callers of the entity's
> script-start routine outside the script runtime, excluding the network decoder (decision
> 0004). **Decision (Tessera):** §9.

> **OPEN (Q-CV3):** What `Skill_AnimationMayFail` and `Skill_AnimationDurationMultiplierModOverride`
> change on the client. **Decision (Tessera):** neither is read.

> **OPEN (Q-CV4):** The meaning of the strike kinds (1, 0x100, 0x200, 0x8000, 0x10000), which
> come with the session's data, and the order of the two weapon scripts of §4.4 step 4 (the
> listing calls the weapon behaviour script twice in one branch, which may be a decompiler
> artefact). **Decision (Tessera):** rule 3; weapon fx before weapon behaviour.

> **OPEN (Q-CV5):** The unit and reference of `CombatStrikeData_TargetToAttackerAngle` and the
> formula of `CombatStrikeData_Intensity`. No script definition of the avatar closures switches
> on either. **Decision (Tessera):** rule 5.

> **OPEN (Q-CV6):** The native rules of the `GetUpcomingTargets` and `GetUpcomingHitResult`
> callbacks (which strikes count as upcoming: the next hook only or all not yet shown).
> **Decision (Tessera):** rule 8.

> **OPEN (Q-CV7):** Whether a stance change while a behaviour plays restarts its weapon-style
> branch (the templates' Switches are not persistent, so the branch is chosen at start, Inferred).
> **Decision (Tessera):** no restart.

---

<a id="s15"></a>
## 15. Notes for other specs

- `combat-ui.md`: the strikes of §4 time the hit visuals only; whether floating text waits for
  the hook is that spec's decision. A Tessera client that delays floating text should use the
  moment §9.3 shows the strike.
- `scenario-feed.md`: no field is needed for P2. A scenario that wants a strike at a given hook
  orders its `VitalDelta` lines per target (§9.3 step 2). A later revision may add an optional
  hook number to `damage` and `heal`.
- `script-runtime.md` §18: StoryHook and Execute nodes now have a run-time meaning in combat
  scripts (`script-table.md` §9.1, this spec §4.2, §4.6, §11 rule 8).
- `game-controls.md` §5.5 Q9: §11 rule 6 is the stance Decision; the default attack's swing is
  §3.1.
- `particle-runtime.md`: Q-RT4 answered in its §11.1 (this revision); lit systems' lights as its
  rule 13.

---

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-10 | all | First version, status Draft, awaiting independent review. Census of the script keys of skills, hooks, effects, inductions, channels and combat states over all records (OBS); behaviour choice from the class library (BC); strikes, hook counting, story end and strike display from the client (BIN); hit, reaction and projectile scripts, stance, defeat and revive from the data; Tessera mapping of the session events; census for P2 exit criterion 3; Q-CV1–Q-CV7. Companion revisions: `particle-runtime.md` §11.1 (beams, answers Q-RT4) and rule 13 (lit systems), `particles.md` Q8 note, `script-table.md` §9.1. |
| 2026-10-10 | §2.1, §11 | Implementer OPENs from `crates/gamedata` (no fact changed): channelling FX labels found by name, "off-hand weapon" of rule 6. |
