<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Combat UI behaviour: selection, player and target frames, quickslot activation and cooldowns, cast bars, floating text, effect bars, combat state and defeat

| | |
|---|---|
| **Status** | **Accepted** (review 4, 2026-10-14, of revision 3). Before: **Revised** (revision 3, 2026-10-14: non-blocking items (a)–(c) of review 1 and (e)–(g) of review 2). Before: **Accepted** (review 3, 2026-10-11, of revision 2) |
| **Last updated** | 2026-10-14 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewers, independent sessions that wrote no part of this spec (review 1, 2026-10-11, changes requested; review 2, 2026-10-11, another session, changes requested; review 3, 2026-10-11, a third session, passed; review 4, 2026-10-14, a fourth session, passed) |
| **Review record** | **Review 1, 2026-10-11** (text of commit 7b14f27 on branch p2, with the P2 notes in `keymap.md` and `game-controls.md`): **changes requested**. Checked (CLEAN_ROOM.md §5.3): functional facts only; no code or pseudo-code (the floating-text motion, anchor, stacking, effect-order and cooldown rules are plain-word steps and formulas; the §17.1 block is a synthetic scenario in Tessera's own format); no copied prose; identifiers are install labels or command names quoted for interoperability; no game text (texts by table and token only; the §17.1 names and the TR38 statement are Tessera's own), keys, hostnames or traffic; every fact group has a provenance row (the BIN row names decision 0011, questions (a)–(i) and why M1–M3 were not enough); Tessera choices are numbered TR rules; layouts give boxes in design pixels; CUI1–CUI10 are testable; unknowns are OPEN; nothing under a closed gate. Spot checks, own read-only probes of the install (M1): the ten `UILAYOUT` labels of CU1; both settings sets of §6.2 (16 of 16 values each); `UI_ShortcutActivationInputMap` (72 of 72 entries); every K47 binding and the K48 toggle types (key maps 0x1D000000, 0x1D000001, 0x1D000004, 0x1D000009; action map 0x51000000); the §5.2 decoration mapper; the border-state mapper and the 16 `CombatResultType` keys; nine `ColorNameText` colours and the floating-text fonts and colours; `EntitySelectionIndicator`, `TimedEffectTemplate` and its two fields, both progress meters, the 60-frame reset-timer meter, the P4/P5 roots, effect lists and `ViolentModeIndicator`; the selection script table and its three keys; all 21 tokens named in §5.1, §5.7, §6.6, §7.1 and §10.2 resolve; `Selection_Script` on 191,868 sets, all 0x10000818. Own M4 spot checks (decision 0011, private copy deleted after use), all agree with the text: the floating-text settings reader (the sixteen built-in defaults, the type check, release spacing 0.35, anchor lift 0.05, the three smoothing constants, the column list, travel and rise derived after reading), the effect-list set-up (icon size default 20; effect ID, start, timeout and combat-only written on the icon), the pulse update (both rows; combat-only icons steady while violent), the icon insert routine (§8.2 Order: an untimed, non-combat-only icon at position 0; the scan from the end; an icon with the same effect ID sends the new icon to position 0). **Blocking:** (1) §5.2 census paragraph disagrees with the install: `MonsterLevel_ExaminationModStatType` is stored at top level in 16,605 property sets of prefix 0x79 (5: 16,044; 9: 152; 8: 139; 12: 91; 10: 38; 11: 33; 7: 28; 3: 23; 6: 22; 4: 19; 2: 16), and 309 further values sit nested inside 68 of those sets (0: 226; 7: 52; 4: 21; 8: 2; 1: 1; 42–48: one each), so 16,914 counts values, not sets, and 0, 1 and 42–48 never occur at top level; `NPC_HideLevelInDisplay` is stored in 949 sets of 0x79 and one entity descriptor of 0x47 (950); `NPC_HideLevelAndVitalText` is stored (true) at top level in 8 entity descriptors of 0x47, not in none. Restate the paragraph (and say whether TR13's body-record reading includes 0x47 descriptors). No implemented behaviour changes (`tessera-gamedata` already reads both hide properties). **Non-blocking (for the revision):** (a) the §8.2 implementer OPEN on CUI7: the binary confirms the §8.2 same-ID rule (a new icon whose effect ID is already in the row goes to position 0, timed or not), so CUI7 holds only while no icon with the same effect ID is in the row; the ID compared is the value the icon also carries as `UI_Examination_Tooltip_EffectID`. With TR30 each `EffectApplied` carries a new instance id, so the case does not arise from the feed; state this in CUI7 and TR30 (no behaviour change: `tessera-ui` keeps CUI7). (b) §8.2 Order omits combat-only icons: in the original a combat-only icon passes over timed icons in the scan and is placed right after the last untimed, non-combat-only icon; the placement of an untimed combat-only icon is not covered by the text. With feed start times (arrival order) timed icons end up in the same order; say what Tessera does with untimed combat-only icons. (c) Answer the implementer and exit-verifier OPENs listed in §19 (14 items). (d) Duplicate anchor `s18` before §18. Full record in the review log (decision 0018). **Review 2, 2026-10-11** (revision 1, commit 1a941c1 on branch p2, against the text of 7b14f27; a session that wrote no part of the spec and did not perform review 1): **changes requested**. Checked (CLEAN_ROOM.md §5.3) for the whole text and in detail for revision 1: functional facts only; no code or pseudo-code; no copied prose; the census adds counts, values and interoperability property names only, no record DIDs, text or assets; CU26 gives the census a provenance row and the OBS row names its scope; the new TR13 text is a numbered Tessera rule and the new composition question is an OPEN listed in §19; nothing under a closed gate. Own read-only probes of the install (M1, a new scratch crate over `tessera-dat`/`tessera-formats`): every 0x47 entity descriptor (40,643), 0x78 set (1,618: 1,616 in `client_gamelogic.dat`, 2 in `client_local_English.dat`) and 0x79 set (383,332), 0 decode failures, each top-level property walked to every depth: the §5.2 census table agrees in every cell (16,605 top-level 0x79 sets with the eleven value counts; 309 nested values in 68 sets, disjoint from the 16,605, 264 under `SpeciesLevelLootTable` and 45 under `Mod_Array`, values 0: 226, 7: 52, 4: 21, 8: 2, 1: 1, 42–48: one each; none in 0x47 or 0x78; `NPC_HideLevelInDisplay` 949 sets + 1 descriptor, `NPC_HideLevelAndVitalText` 8 descriptors, all true, none nested); the `PhysObj` paragraph agrees (8 descriptors, 7 named by 12 sets, one named by none; 11 sets store `NPC_HideLevelInDisplay` and name an `NPC_HideLevelAndVitalText` descriptor; 1 set stores neither and names the descriptor that stores both). M1 also: the state counts of §4.1 and §5.1 (125, 12, 86, 13; 139, 102, 26, 27) and all 26 decoration states of the §5.2 table present on `OpponentPanel_BG`, `OpponentVitals_Animations` and `OpponentLevel_BG`; `ViolentModeIndicator` box and states. Own M4 spot check (decision 0011, private copy deleted after use), question (d): the target-frame update maps every key of the §5.2 table to the state given there and every other key to `monster_normal`; `OpponentLevel` is hidden when either hide property is true and `OpponentLevelHiddenSkull` is shown exactly when `NPC_HideLevelInDisplay` is true, so the skull case of TR13 for a set storing both matches the original. Implementation read only to compare: `tessera-gamedata` reads the three properties at top level of the body record, puts the skull first, and gives a 0x47 DID no set, as TR13 says. **Blocking:** (1) TR31 says the effect tooltip shows the stack count when `stacks` > 1; `skill-tooltips.md` §12.2 (which §0 makes the owner of effect tooltips) gives the opposite Tessera rule (no stack-count row, the original shows none) and asks for TR31 to be aligned, and the code shows no stack count. Align TR31 (and OC10’s wording) with `skill-tooltips.md` §12.2; no implemented behaviour changes. **Non-blocking (for the same revision):** (a)–(d) of review 1 are still open (CUI7 scope against the §8.2 same-ID rule, with the compared ID being the per-application runtime ID of `skill-tooltips.md` §8.2; combat-only placement in §8.2; the 14 implementer and verifier OPENs of §19; duplicate anchor `s18`); (e) the OBS row says "all 1,616 0x78" sets: the archive set holds 1,618 (2 in `client_local_English.dat`, neither stores the three properties), so name the archives or the count; (f) TR13 says the 0x70–0x77 object is read "through its property set (… prefix 0x79)": per `property-system.md` §6.2 the prefix is the object prefix + 9 (0x79 only for 0x70) and Tessera gives 0x77 no set; (g) TR13 makes a `Body::Object` DID of prefix 0x78–0x7F "no body record", while `tessera-gamedata` reads such a DID as the set itself (`property-system.md` §6.2 step 2); prefer stating the code’s reading (keeping TR13 as written would change implemented behaviour), and the revision-1 change-log claim "no implemented behaviour changes" holds only once this is settled; (h) the companion-spec cell cites `scenario-feed.md` as "Accepted, revision 4"; revision 6 is now Accepted. **Review 3, 2026-10-11** (revision 2, commit 7468b14 on branch p2, whole text; a session that wrote no part of the spec and performed neither review 1 nor review 2): **passed**; status Revised → Accepted. Checked (CLEAN_ROOM.md §5.3): functional facts only; no code or pseudo-code (§6.3–§6.5, §8.2 and §9.3 are plain steps and formulas; §17.1 is a synthetic scenario in Tessera's own format); no copied prose; identifiers are install labels, property and command names quoted for interoperability; no game text, keys, hostnames or traffic; every fact group has a provenance row and a mark; Tessera choices are numbered TR rules; boxes in design pixels, units stated; CUI1–CUI10 testable; unknowns are OPEN and listed in §19; nothing under a closed gate. Revision 2: TR31 now agrees with `skill-tooltips.md` §12.2 (no stack-count row; remaining time counts down) and OC10 with OS5; no other text mentions a stack count; the duplicate `s18` anchor is gone; the companion cell cites `scenario-feed.md` revision 6. No implemented behaviour changes: neither the icon nor the tooltip code shows `stacks`. Own read-only probes of the install (M1, a new scratch crate over `tessera-dat`/`tessera-props`/`tessera-formats`): `TimedEffectTemplate` of `effects` is a 36 × 36 field with exactly two field children, `TimedEffectBorderField` (36 × 36, z −1, the eleven category states) and `TimedEffectImageField` ((2, 2) 32 × 32, z −2, `Pulse`, `Pulse_Reset`), and no text element (§8.1, TR31); `PlayerProgressUIElement` (255 × 26, auto-fade 0.5, the five root states) and its meter (direction 4, `move_fill` false, `smooth_movement` true, image 0x41007E92); the §7.1 drag boxes and their bars at (32, 28), (15, 12) and (2, 28); `GamePlay_SelectionTarget` (200, 450) 140 × 71; the defeat panel's OK (321, 163) and revive (137, 163) buttons and the OK token; `QuickslotSkillResetTimer` a 60-frame meter from 0x41007E35 and `MinResetTimeForAvailabilityAlert` 2.0; the archive set holds 40,643 0x47, 1,618 0x78 (1,616 + 2 in a second archive) and 383,332 0x79 records. Own M4 spot check (decision 0011, private copy deleted after use), questions (a) and (c), parts not checked by reviews 1 and 2: the anchor routine (§6.4: lift from the settings, a 30-position smoothing ring, width × screen width ÷ 1024 × (4/3 ÷ aspect), height × screen height ÷ 768, the preference factor clamp(0.75 × *g* + 0.25, 0.25, 1.75), *q* = (1 − *u*)², the σ blend, offsets × σ, centred, bottom at the anchor) and the selection-indicator update (§3.5: state from `Skill_DefaultSkillEnabled` and the line-of-sight result, vertical offset −75, scales 0.25 and 0.2, top margin 0.75 × height): all agree with the text. **Non-blocking, for a later revision** (none changes implemented behaviour unless said): (a)–(c) of review 1 and (e)–(f) of review 2 still stand; (g) of review 2 still stands: `tessera-gamedata` reads a `Body::Object` DID of prefix 0x78–0x7F as the set itself, so restating TR13 to that reading changes nothing, while keeping TR13 as written needs a code change; the revision-1 change-log claim holds only for 0x70 bodies; `skill-tooltips.md` §12.2 keeps an OPEN asking for the TR31 alignment that revision 2 made (its writer may close it). **Review 4, 2026-10-14** (revision 3, commit 649bb7e on branch specfix: header, OBS and BIN rows, CU27–CU31, §3.3, §3.5, §4.4, §5.2, §5.3, §5.7, §6.5, §8.2, §10.2, §11, §13.1, §13.4, §13.7, §15, §16.1, §18, §19, change log; §4.1 excluded, it belongs to a parallel branch; a session that wrote no part of the spec and performed none of reviews 1–3): **passed**; status Revised → Accepted. Checked (CLEAN_ROOM.md §5.3): functional facts, numbered Tessera rules and OPENs only; no code or pseudo-code (the §8.2 Order steps and the §6.5 smoothing and release rules are plain-word steps and formulas); no copied prose; element, property and state names are install labels quoted for interoperability; no game text, keys, hostnames or traffic; every new fact has a row with its mark (CU27–CU29 Confirmed, CU30–CU31 BIN); BIN questions (j) and (k) name build, hash prefix, tool and why M1–M3 were not enough; nothing under a closed gate. The revision settles review 1 (a)–(c) and review 2 (e)–(g): CUI7 agrees with the same-ID rule because TR30 now replaces a repeated instance's icon; combat-only placement is §8.2 steps 1–4; the 14 implementer and verifier OPENs are answered as facts (CU27–CU31) or as Tessera rules (TR8a, TR14a, TR18a, TR19a, TR30a, TR38a, TR42–TR46), and §19 keeps owner and later-BIN items only; the OBS row names 1,618 0x78 sets; TR13 follows `property-system.md` §6.2. The behaviour changes on main are listed in the change log. Own read-only probes (M1, a new scratch crate over the repository decoders): the element trees in read order of `SelectionMiniVitals` (105 × 27; `OpponentHealthField` (2, 2) 100 × 9, `RageMeter` (2, 15), `OpponentPowerField` (2, 15), overlay 105 × 26), `MicroVitalFieldBase` (101 × 15; health (0, 0) and power (0, 7), 99 × 6), `VitalHealthFieldBase` and `VitalPowerFieldBase` (children and order as stated; the current meters based on `VitalColorMeter`), `SelectionTargetVitalField` based on `MicroVitalFieldBase`; the defeat box's nine own children in the stated order and boxes, and the ten `Base_Box_Silver_…` children of `Box_Silver_BlueGradient`; `pip_display`: 8 lists, 138 entries, every one a `UIComponent_PipDisplay_Entry` struct of exactly `UIComponent_PipDisplay_EntryState` (enum) and `UIComponent_PipDisplay_EntryValue` (int). Implementation read only to compare TR13: `tessera-gamedata` reads a 0x78–0x7F DID as the set itself, a 0x70–0x77 object through its set, gives a 0x47 DID none, gives players `normal` and puts the skull before the hidden case, as TR13 now states. Own M4 spot check (decision 0011; Ghidra 12.1.3 headless on a private project copy, deleted after use; client SHA-256 prefix d22363eb9aff0fb5): (j) the insert routine reads `UI_Effect_CombatOnly`, `UI_Effect_EffectID` and `UI_Effect_EffectDuration` (names resolved in the dictionary), puts an untimed non-combat-only icon at position 0, scans from the last icon passing over icons without an ID, sends a same-ID hit to position 0 and otherwise stops exactly as steps 3 and 4 state; (k) the smoothing helper starts with time constant 1 s, closes the gap by 1 − 2^(−Δ*t* ÷ τ) from the last set and snaps within 0.0002; the list update reads the smoothed speed before it sets the new target (base × (1 + min(*n* ÷ 10, 1)) for *n* > 2) and releases the first pending text when no text is active or the smallest rise (speed × elapsed, before the distance factor) of the non-critical texts drawn in the loop exceeds the pending text's height × (1 − the release spacing). **Non-blocking:** (a) TR38a's closing remark ("the result is the same only if the inherited art comes before that holder in child order") could be settled from the child-merge order of `ui-layout.md`. |
| **Implements into** | `crates/ui` (`game::combat`: target and player frames, cast bars, effect bars, floating-text layer, selection indicator and name block, defeat panel, quickslot cooldown and state display, and their fallback looks), `crates/input` (the targeting and quickslot actions of `keymap.md` §8.2), `crates/gamedata` (the data of the combat panels: frame-state, consideration and name-colour lookups, effect icon and category, skill icon, floating-text settings and tokens), `apps/client` (selection and targeting, the combat panels fed by the session, the floating-text and selection-indicator anchors, the defeat panel, the real-data test and the synthetic goldens of §16) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (layouts, states, media, §17 panels P3, P4, P5, P14), [`ui-widgets.md`](ui-widgets.md) (§4 Meter), [`ui-behaviour.md`](ui-behaviour.md) (tooltips §6, auto-fade §11.1), [`game-panels.md`](game-panels.md) (§4 drag boxes, §7 pip displays, §10 quickslot bars, §16 Escape), [`game-controls.md`](game-controls.md) (§7.1 vital-display overrides, §7.5 consideration), [`interaction.md`](interaction.md) (§3.4.4 eligibility, §3.4.6 mouse-over, §3.8.4 induction bar, §3.9 channelling, §5.3 selectable, §8 defeat), [`combat-data.md`](combat-data.md) (§6 effects, §7 skills, §11 enum mappers), [`player-character.md`](player-character.md) (§10 pips), [`script-table.md`](script-table.md) and [`decals.md`](decals.md) (the selection effect script), [`fonts.md`](fonts.md), [`keymap.md`](keymap.md) (§8.2, §12.14.2), [`scenario-feed.md`](scenario-feed.md) (§6.2, §9; Accepted, revision 6), `skill-tooltips.md` (P2, new: examination tooltips and icons of skills and effects), `combat-visuals.md` (P2, new: animation and particles of casts, hits, effects and defeat) |

---

<a id="s0"></a>
## 0. Purpose

Decision 0024 §5 P2 asks for a "combat UI behaviour" spec observed in the original client, so
that the client half can show targeting, skills and combat that the scenario feed (or a server
backend) reports. This spec gives, for every combat element on screen: which layout record and
element draws it, what the data says about its look, what the original's native code does with
it (method M4, decision 0011, rows marked BIN), and, where a behaviour was not traced or is game
logic that Tessera must not reproduce (decision 0024 §2.4, LEGAL.md §8), a numbered **Tessera
rule** (TR*n*). §11 maps every Combat-group event and command of `scenario-feed.md` §9 to the
elements it drives. §13 gives a look for every element when no install is present, so that
synthetic golden images can be committed (ARCHITECTURE §23.3).

All DIDs, element IDs, state IDs and counts are observations of one install (client build
4906.0071.1794.4035); implementations look labels up at run time (`ui-layout.md` §3, §17.1)
and treat the values here as examples. No game text appears in this spec: texts are referenced
by string table and token.

Out of scope: tooltips of skills and effects (`skill-tooltips.md`), animation and particles of
combat (`combat-visuals.md`), floating names of entities other than the selection (§5.5),
fellowship and raid frames (P4), the world map and minimap (P5), mounted combat (P5), monster
play and PvP frames.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install, read-only, 2026-10-10 (archives unchanged since 2026-10-04): every UI layout record named here, decoded per `ui-layout.md` §4 with the throwaway decoder of the M5 dump (all 302 layout records, cached 2026-10-06; the layouts named here were re-read and agree); the DID maps `UILAYOUT`, `UNIQUEDB`, `WEENIECONTENT`, `STRINGTABLE`, `KEYMAP`, `FONT` (`records.md` §8); the enum mappers `UIStateID`, `UIElementMessage`, `UIElementID`, `InputActions` (0x230000AF and base 0x23000176), 0x23000064, 0x23000070, 0x2300002A, 0x230003A8; the property sets 0x78000004, 0x780029BA, 0x7800000D; the script table 0x07000000; the key-map records 0x1D000000, 0x1D000001, 0x1D000009 and the action-map record 0x51000000 (`keymap.md` §12.9–§12.10); a census over every property set of prefixes 0x47, 0x78 and 0x79 of `client_gamelogic.dat` and `client_general.dat` for `Selection_Script`, `MonsterLevel_ExaminationModStatType` and `NPC_HideLevelAndVitalText`; for revision 1 (2026-10-11, archives unchanged) a fresh census of `MonsterLevel_ExaminationModStatType`, `NPC_HideLevelInDisplay` and `NPC_HideLevelAndVitalText` over all 40,643 0x47 entity descriptors (their embedded sets, `setup.md` §4), all 1,618 0x78 sets (1,616 in `client_gamelogic.dat` and 2 in `client_local_English.dat`, neither of which stores any of the three properties; recounted 2026-10-14) and all 383,332 0x79 sets (none failed to decode), every value walked at every depth and counted by prefix, depth and record, with the containing top-level property of each nested value and the `PhysObj` references of every 0x79 set to the 0x47 descriptors that store a hide property; the string table 0x250001BB (the `UI` table) only to confirm that the tokens named here exist and to tell which token of a pair is which (no text is reproduced); for revision 3 (2026-10-14, archives unchanged) the element trees of `SelectionMiniVitals`, `MicroVitalFieldBase`, `VitalHealthFieldBase` and `VitalPowerFieldBase` (0x2200007E), `SelectionTargetBase` (0x220000C4), `Awaken_DialogMessageField` (`Dialog`) with `Box_Silver_BlueGradient` (0x22000001), and every `UIComponent_PipDisplay_List` entry of `pip_display` (138 entries). Probes were throwaway scripts in the dirty scratch area. |
| **BC** | The class-library record 0x56000000 (`wlib.md`), searched as bytes for the tokens of §6.6 (M1; no bytecode was decoded into this spec). |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md): the offline client `x64/lotroclient64.exe`, file version **4906.0071.1794.4035**, read on 2026-10-10 with Ghidra (headless, on a private copy of the project's auto-analysed database, deleted after use). **Questions:** (a) how the floating-text system reads its settings and moves, scales, fades, stacks and removes a text; (b) which preference bits gate floating texts; (c) how the selection indicator chooses its state and placement; (d) how the target frame chooses between its vital and non-vital fields, its frame decoration, its health-meter colour and its level display; (e) how an effect icon is created, sized, ordered into rows, pulsed and given a border; (f) how the quickslot cooldown meter is started and ended; (g) how another entity's induction bar chooses its style; (h) how the toolbar shows the combat state; (i) where the four combat-related layout keys and the main element IDs are used. **Why M1–M3 were not enough:** the layout data gives elements, states and property values but not the rules that pick states, fill meters, order icons, move floating text or combine the settings; these rules exist only in code; black-box observation (M3) of the running client is not available to this session and could not show constants such as fade fractions, rise speeds or pulse lead times. **Read:** a scan of all instructions for the 32-bit IDs of the layouts, elements, states, properties and input actions named here (hits that turned out to be other enumerations with the same value were discarded after reading the routine); the floating-text settings reader, the text factory, the per-entity text list update, the anchor and distance-scale routine, the smoothing helpers it calls, the generic floating-text producers and the preference handler; the selection-indicator update; the target-frame update and its vital-colour and has-vitals helpers; the effect-icon creation, ordering and pulse routines and the effect-list set-up; the skill display's reset-timer start, end and goal handler; the induction-bar style setter; the toolbar combat-state handlers; the options panel's floating-text checkboxes. Nothing from network, login, launcher, keys or anti-tamper code was read (decision 0004). **Revision 3 pass, 2026-10-14** (same build, SHA-256 of the client file re-checked, prefix d22363eb9aff0fb5; Ghidra 12.1.3 headless on a private copy of the project's database, deleted after use): **questions** (j) where the effect-icon insert routine places a combat-only icon, and which effect ID its same-ID test compares; (k) the time constant of the floating-text rise-speed smoothing and which height the release of a pending text compares with. **Read:** the effect-icon insert routine; the smoothing helper's initialiser, read and set routines and their constants; the per-entity list's creation and update; the text factory's size fields. **Why M1–M3 were not enough:** the order and the smoothing are code rules; the data holds neither, and black-box observation (M3) is not available to this session. |
| **Project** | Other Tessera specs and decisions, as cited (read 2026-10-10). |
| **Design** | Tessera rules, marked **TR*n***. |

| # | Fact | Source | Mark |
|---|---|---|---|
| CU1 | The combat layouts and their `UILAYOUT` labels and keys: `selection` (0x10000009 → 0x22000013), `floatytext` (0x10000043 → 0x22000014), `effects` (0x10000018 → 0x22000016), `progress` (0x1000004C → 0x22000042), `toolbar` (0x10000042 → 0x22000033), `quickslots` (0x10000059 → 0x22000039), `gameplayactivatables` (→ 0x22000012), `pip_display` (→ 0x22000055), `text` (0x1000005F → 0x22000002), `Dialog` (0x2 → 0x2200007C); unlabelled base layouts 0x2200007E (mini vitals), 0x2200007F (vital fields), 0x220000C4 (selection-target display), 0x22000887 (awaken timer) (§2) | OBS | Confirmed 2026-10-10 |
| CU2 | Player vitals frame P4 `AvatarPanel`: tree, positions, properties and states (§4) | OBS | Confirmed 2026-10-10 |
| CU3 | Target frame P5 `OpponentPanel`: tree, the vital and non-vital fields, decoration states, level and hidden-level elements, effect list, portrait (§5) | OBS | Confirmed 2026-10-10 |
| CU4 | Vital fields of 0x2200007F: meters, fill directions, images by state, temporary and dread meters, vital texts hidden by default (§4.3, §5.3) | OBS | Confirmed 2026-10-10 |
| CU5 | `ColorNameText` (text layout, 0x10000D0D) holds 32 colour states: the nine consideration states, the player, NPC, object, corpse, tapped, PvP and item-quality states, and `Floaty_CriticalDamage` (§5.4, §6.3) | OBS | Confirmed 2026-10-10 |
| CU6 | Target-frame rules: field choice by "has vitals", decoration state from `MonsterLevel_ExaminationModStatType` by a fixed table, level hidden by `NPC_HideLevelInDisplay` / `NPC_HideLevelAndVitalText` with the hidden-level skull, health-meter colour state chosen among four (§5.2–§5.5) | BIN (d) | BIN |
| CU7 | Floating-text templates of `floatytext`, their fonts and colours (§6.1) | OBS | Confirmed 2026-10-10 |
| CU8 | The two floating-text settings sets `UI_FloatyText_Properties` (UNIQUEDB 0x10000001 → 0x78000004) and `UI_DistantBattle_FloatyText_Properties` (0x10000048 → 0x780029BA), 16 properties each (§6.2) | OBS | Confirmed 2026-10-10 |
| CU9 | Floating-text behaviour: built-in defaults, life, growth, critical placement and shrink, rise speed and crowding, fade, distance scale, release queue, cap, recycling (§6.3–§6.5) | BIN (a) | BIN |
| CU10 | Preference bits of `GOP_FloatyInformation_Bitfield` that gate floating texts and their options-panel names (§6.7) | BIN (b) | BIN |
| CU11 | The class library holds, together, UI-table tokens for the avoidance results (two forms for most); `CombatResultType` (0x2300002A) has 16 keys (§6.6) | BC; OBS | Confirmed (presence, keys); use Probable |
| CU12 | Selection indicator `EntitySelectionIndicator` and its four states; state choice by the local player's `Skill_DefaultSkillEnabled` and line of sight; placement through the floating-text anchor routine with fixed offsets and scales; `OffScreenIndicator`; `TargettingReticle` properties (§3.5) | OBS; BIN (c) | Confirmed (data), BIN |
| CU13 | Script table 0x07000000 (`WEENIECONTENT` `SharedUISelection_ScriptTable`) holds `ui_selected_fx` (key 0x10000818), `ui_mouseover_fx` (0x10000B31) and `ui_forwarded_selected_fx` (0x10000C3E); `ui_selected_fx` places decals by distance band and entity type; `Selection_Script` = 0x10000818 on all 191,868 property sets that store it (§3.5) | OBS | Confirmed 2026-10-10 |
| CU14 | Effect bars: `effects` templates, the `EffectDisplayList` properties of P4 and P5, icon size default 20, two rows and the row choice, order by start time, pulse at 15 s, combat-only pulsing, border state by category through the class library, tooltip properties set on the icon (§8) | OBS; BIN (e) | Confirmed (data), BIN |
| CU15 | Cast bars: `progress` elements, their states and media, their drag boxes in `gameplayactivatables`; style of another entity's bar by interruptibility; the floating mini induction bar (§7) | OBS; BIN (g) | Confirmed (data), BIN |
| CU16 | Quickslot cooldown: the 60-frame reset-timer meter, its start, its goal and the availability alert after cooldowns of at least 2 s (§9.3) | OBS; BIN (f) | Confirmed (data), BIN |
| CU17 | Skill quickslot overlays and their states (§9.4) | OBS | Confirmed 2026-10-10 |
| CU18 | `UI_ShortcutActivationInputMap` (UNIQUEDB 0x10000010 → 0x7800000D): 72 entries, shortcut index *i* → `QUICKSLOT_`(*i* + 1) (§9.2) | OBS | Confirmed 2026-10-10 |
| CU19 | Combat state: `Combat_Violent` sets the toolbar root state (`toolbar_combat` / `toolbar_noncombat`) and the `ViolentModeIndicator` state (`combat_violent` / `combat_normal`) (§10.1) | OBS; BIN (h) | Confirmed (data), BIN |
| CU20 | Defeat display: `Awaken_DialogMessageField` of `Dialog` and the awaken timer (§10.2) | OBS | Confirmed 2026-10-10 |
| CU21 | Selection's-target panel `GamePlay_SelectionTarget` based on `SelectionTargetBase` (§5.7) | OBS | Confirmed 2026-10-10 |
| CU22 | Default key bindings and toggle types of the targeting and quickslot commands (§3.3, `keymap.md` K47–K49) | OBS | Confirmed 2026-10-10 |
| CU23 | Class-resource pip displays (`PipDisplay` types per `PipType`) (§4.4) | OBS | Confirmed 2026-10-10 |
| CU24 | `UI_Selection_*` properties on `TargettingReticle` (§3.1) | OBS | Confirmed 2026-10-10 (values; use not traced) |
| CU25 | Every TR rule and the fallback looks of §13 | Design | Decision |
| CU26 | Where the three target-frame properties of §5.2 steps 3–4 are stored: by prefix (0x47, 0x78, 0x79), by depth (top level or nested in another property) and by value; the 0x79 sets whose `PhysObj` names a 0x47 descriptor that stores a hide property (§5.2 census) | OBS (revision 1 census) | Confirmed 2026-10-11 |
| CU27 | Element trees of the selection's mini vitals (`SelectionMiniVitals`), of `MicroVitalFieldBase` with `VitalHealthFieldBase` and `VitalPowerFieldBase`, and of `SelectionTargetBase` (§3.5, §5.7) | OBS (revision 3) | Confirmed 2026-10-14 |
| CU28 | Every `UIComponent_PipDisplay_List` entry is a `UIComponent_PipDisplay_Entry` struct of `UIComponent_PipDisplay_EntryState` (enum, a `UIStateID`) and `UIComponent_PipDisplay_EntryValue` (int) (§4.4) | OBS (revision 3; 138 of 138 entries) | Confirmed 2026-10-14 |
| CU29 | The children of the defeat panel's `DialogMessageBoxField`, among them `DialogMessage_Divider`, `SelfRevive_PaperItemTotal_NameLabel`, `SelfRevive_PaperItemBalance` and the box art inherited from `Box_Silver_BlueGradient` (§10.2) | OBS (revision 3) | Confirmed 2026-10-14 |
| CU30 | Effect-icon order with combat-only icons and the same-ID test (§8.2) | BIN (j) | BIN |
| CU31 | Rise-speed smoothing (base 2, time constant 1 s, snap within 0.0002) and the release comparison with the pending text's fitted height (§6.5) | BIN (k) | BIN |

---

<a id="s2"></a>
## 2. Terms and element index

| Term | Meaning |
|---|---|
| **Selection** | The entity the local player has selected (the "target"). One or none. |
| **Target frame** | P5 `OpponentPanel` (`ui-layout.md` §17.2), showing the selection. |
| **Player frame** | P4 `AvatarPanel`, showing the local player. |
| **Floating text** | Text drawn in screen space over an entity (an "floaty"), placed from the entity's position each frame. |
| **Cast bar** | The progress bar of a cast, induction, channel or craft (`CastKind`, `scenario-feed.md` §9.1). |
| **Design pixel** | A pixel of a layout's design size (`ui-layout.md` §5.1); layouts are placed per `ui-layout.md` §8. |
| **Layout key** | The `UILAYOUT` key of a layout (CU1); implementations find layouts by label. |

Element index (layout, element label, element ID here). "Panel" names a root opened per
`ui-layout.md` §17.1; "template" names an element the code creates per instance.

| # | Element | Layout | Element label (ID) | Kind |
|---|---|---|---|---|
| E1 | Selection indicator (screen) | `floatytext` | `EntitySelectionIndicator` (0x1000327F) | template, one instance |
| E2 | Off-screen indicator | `selection` | `OffScreenIndicator` (0x10000590) | template |
| E3 | Selection name block | `floatytext` | `SelectionNameTextTemplate` (0x10000374) with `EntityName`, `EntityTitleOrGuild`, `EntityMiniVitals`, `EntityMiniInduction` | template, one instance |
| E4 | Selection effect (world) | script table 0x07000000 | script `ui_selected_fx` | script on the entity |
| E5 | Player frame | `toolbar` | `AvatarPanel` (0x10000B46) | panel P4 |
| E6 | Target frame | `toolbar` | `OpponentPanel` (0x10000B56) | panel P5 |
| E7 | Selection's target | `gameplayactivatables` | `GamePlay_SelectionTarget` (0x1000109E) | panel |
| E8 | Player effect rows | `toolbar` | `PlayerEffectDisplay` (0x10000B18) in E5 | list |
| E9 | Target effect rows | `toolbar` | `OpponentEffectDisplay` (0x10000B5F) in E6 | list |
| E10 | Effect icon | `effects` | `TimedEffectTemplate` (0x10000360) | template |
| E11 | Player cast bar | `gameplayactivatables` | `GamePlay_ProgressBar_Dragbox` (0x10000385) with `GamePlay_ProgressBar` (0x10000386) | panel |
| E12 | Target cast bar | `gameplayactivatables` | `GamePlay_Opponent_ProgressBar_Dragbox` (0x1000461A) with `GamePlay_Opponent_ProgressBar` (0x1000461B) | panel |
| E13 | Floating combat text | `floatytext` | `DamageTextTemplate` (0x10000371) | template |
| E14 | Floating gain text | `floatytext` | `DamageTextTemplate` (same element; §6.8) | template |
| E15 | Main quickslot bar | `toolbar` | `QuickslotBar` under `ToolbarField` (P3) | panel P3 |
| E16 | Skill slot display | `quickslots` | `SkillQuickslotDisplayTemplate` (0x1000079F) | template |
| E17 | Combat-state indicator | `toolbar` | `ViolentModeIndicator` (0x10000B27) in P3 | element |
| E18 | Defeat panel | `Dialog` | `Awaken_DialogMessageField` (0x10001E0E) | panel |
| E19 | Awaken timer | `gameplayactivatables` | `GP_AwakenTimer` (0x1000300D) | panel |
| E20 | Class-resource display | `toolbar` (from `pip_display`) | the six `PipDisplay` elements of P3; the wrath meter `RageMeter` of E5 | elements |

---

<a id="s3"></a>
## 3. Selection and targeting

<a id="s3-1"></a>
### 3.1 What can be selected

An entity can be **selected** when it is **eligible** under `interaction.md` §3.4.4 step 1 (the
mouse-over eligibility: `Mouseover_AllowMouseoverOverride`, the phased-out and quest-item tests,
and the selectable test of §5.3: `IsSelectable` true and, when present,
`ClientIsSelectableOverride` true). The local player can always select itself
(`SELECTION_SELF`).

The `TargettingReticle` element of `selection` (CU24) carries `UI_Selection_MaxSelectionDistance`
400, `UI_Selection_AverageBBoxFrames` 4, `UI_Selection_FloatyTextTravel` 0 and
`UI_Selection_MaxNamePlacementAttempts` 50. Their readers were not identified (the scan of
question (i) found no use of these property IDs as constants).

> **TR1 (selectable entities in Tessera).** Scenario actors and test-server actors carry no
> property set. Tessera treats an actor as eligible when its `EntityKind` is `Player`,
> `NonPlayer` or `Creature`, or when it is an `Object` that has a body record (`Body::Object`)
> whose effective properties pass the §3.1 test; an `Object` without a body record is eligible
> (so that M7's test actors, which have none, can be selected: ROADMAP P2 exit criterion 4).
> Static world objects (`StaticSpawned`) are eligible by the §3.1 test on their records.

> **OPEN:** what `UI_Selection_MaxSelectionDistance` (400) limits (mouse picking, targeting
> keys, or keeping a selection) and its unit. **TR2:** Tessera does not limit mouse selection
> by distance; the targeting keys of §3.3 search within 50 m (Tessera's choice); a selection
> is kept at any distance until changed.

<a id="s3-2"></a>
### 3.2 Mouse selection

The pointer actions are those of `keymap.md` §8 (`select_left` = `SelectLeft`, …). The hover
and its mouse-over effect follow `interaction.md` §3.4.6 (BIN there).

> **TR3 (mouse).** A `select_left` press **that the UI declines** (`keymap.md` §7.8.3) and that
> is a click (press and release within the tap rule of `ui-behaviour.md` §3.3, pointer not
> captured by mouse-look) picks the entity under the pointer (the hovered entity of
> `interaction.md` §3.4.6, when eligible). With an entity, the client sends `SelectTarget {
> target: Some(id) }`; on empty ground or sky, nothing is sent (the selection stays).
> `select_double_left` on an eligible entity selects it as above and then uses it
> (`interaction.md` §16.1; `UseEntity`, scenario-feed §9.4). `select_right` on an eligible
> entity selects it; the context menu and "use" of a right click stay as specified elsewhere.
> The client shows the new selection only when the session reports it (`TargetChanged` for the
> player, §11), never before.

<a id="s3-3"></a>
### 3.3 Targeting actions (Confirmed bindings, CU22; behaviour TR4)

The original's targeting commands and their default bindings (section `Main` of the key-map
record 0x1D000000; toggle type 2, Impulse, in 0x51000000; `keymap.md` K47, K48):

| Command | Default | Tessera action (`keymap.md` §8.2) |
|---|---|---|
| `SELECTION_NEAREST_FOE` | Backspace | `target_nearest_enemy` |
| `SELECTION_NEXT_FOE` | Tab | `target_next_enemy` |
| `SELECTION_PREVIOUS_FOE` | Shift+Tab | `target_previous_enemy` |
| `SELECTION_SELF` | Backslash | `target_self` |
| `SELECTION_ASSIST` | F | `target_assist` |
| `SELECTION_NEAREST_PC` | Ctrl+Tab, F9 | `target_nearest_player` |
| `SELECTION_NEXT_PC` / `SELECTION_PREVIOUS_PC` | Shift+F9 / Ctrl+F9 | `target_next_player` / `target_previous_player` |
| `SELECTION_NEAREST_CREATURE` | F10 | `target_nearest_creature` |
| `SELECTION_NEXT_CREATURE` / `SELECTION_PREVIOUS_CREATURE` | Shift+F10 / Ctrl+F10 | `target_next_creature` / `target_previous_creature` |
| `SELECTION_NEAREST_ITEM` | Delete | `target_nearest_object` |
| `SELECTION_LAST`, `SELECTION_LAST_ATTACKER` | none | `target_last`, `target_last_attacker` |
| `SELECTION_OFF` | none (not in the action map) | `target_clear` |

`SELECTION_NEXT_ITEM`, `SELECTION_PREVIOUS_ITEM` and the tracked, fellow and mark commands are
not mapped in P2 (fellowship is P4).

> **OPEN:** the original's search rules (what counts as a foe, the range, the order of the
> cycle, how the "nearest" and "next" commands interact, what `SELECTION_LAST` remembers). They
> are game code that was not traced, and "foe" is a reaction rule (attackability) that the
> sandbox does not have (LEGAL.md §8). Owner check OC1.

> **TR4 (targeting rules).** Candidates are the eligible entities (§3.1) other than the player
> whose distance (3D, entity origin to player origin) is at most 50 m. The **foe** set is the
> candidates of kind `Creature` that are not defeated (§10.3). The **player** set is kind
> `Player`; the **creature** set is kind `Creature` (defeated ones included); the **object**
> set is kind `Object`.
> 1. *Nearest* of a set: the candidate with the smallest distance; ties broken by the smaller
>    entity ID.
> 2. *Next* of a set: order the set by distance (ties by entity ID); if the current selection
>    is in the set, take the element after it (wrapping to the first); otherwise the first.
>    *Previous*: the element before it (wrapping to the last); otherwise the last.
> 3. *Self*: the player. *Clear*: none. *Assist*: the current selection's own target, as last
>    reported by `TargetChanged { entity: selection, … }`; nothing happens when there is no
>    selection or it has no reported target.
> 4. *Last*: the selection before the current one (kept by the client, one deep), when it
>    still exists. *Last attacker*: the `source` of the most recent `VitalDelta` with
>    `target` = player and `kind` = `Damage` whose source still exists.
> 5. An empty result sends nothing. Otherwise the client sends `SelectTarget { target }` once
>    per press (Impulse; repeats are ignored), with `None` for *clear*.

> **Answered (revision 3, 2026-10-14; was OPEN, implementer, `apps/client`, 2026-10-11):** yes.
> TR4's candidates are the entities that pass TR1 in full, the same test as mouse picking
> (TR3): an `Object` with a body record is a candidate only when its effective properties pass
> the §3.1 test, so the targeting keys read the body records as picking does. (The client then
> needs a body record's effective properties for every candidate within 50 m, not only for the
> entity under the pointer.)

<a id="s3-4"></a>
### 3.4 Clearing with Escape

> **TR5 (Escape).** `game-panels.md` §16 handles a `menu` press that the UI declined. Before its
> step 2 (the main menu), and only when step 1 found no active on-demand panel: if the player
> has an active cast that can be cancelled (§7.4), the press sends `CancelCast` and stops;
> otherwise, if there is a selection, it sends `SelectTarget { target: None }` and stops. Only
> a press with neither opens or closes the main menu. (Owner check OC2: the original's order.)

<a id="s3-5"></a>
### 3.5 Selection display

**In the world (E4, CU13).** Every property set that stores `Selection_Script` (191,868 over
the 0x47, 0x78 and 0x79 sets) stores the script key 0x10000818, `ui_selected_fx`, a script of
the shared table 0x07000000. That script switches on `PrivateEncounter_AlwaysInAOI` and on
`WeenieType`, and its branches hold `EntityVFX` nodes and `DistanceCheck` nodes whose four bands
(0–2, 2–15, 15–35 and 35 m upward, the last written with −1) each start one Decal node
(`script-table.md` §5.1, `decals.md`). `ui_mouseover_fx` is the hover effect of
`interaction.md` §3.4.6.

> **TR6 (world indicator).** When the selection changes, the client stops the previous
> entity's selection script (if it still exists) and starts on the new selection the script
> whose key is the entity's effective `Selection_Script`, or `ui_selected_fx` when the entity
> has no property set (scenario and test-server actors), from the table of `WEENIECONTENT`
> `SharedUISelection_ScriptTable`. The script runs on the script-table runtime with the
> entity as owner. Clearing the selection stops it. (That the original starts the script on
> selection is Probable: the property name and its universal value; the starting code was not
> found. Owner check OC3.)

**On screen (E1, CU12, BIN c).** `EntitySelectionIndicator` (125 × 156, z 50) has four states
with one image each (draw mode 4): `floaty_peace`, `floaty_combat`, `floaty_peace_no_los`,
`floaty_combat_no_los`. Each update the client:

1. takes `floaty_combat` when the local player's `Skill_DefaultSkillEnabled` is true, otherwise
   `floaty_peace`, and the `_no_los` variant when the last line-of-sight test from the player
   to the selection failed; the test is repeated at an interval;
2. places the element with the anchor routine of §6.4 (anchor above the selection, settings of
   `UI_FloatyText_Properties`), with a vertical offset of −75 design pixels and the distance
   scales 0.25 (near) and 0.2 (far) instead of the settings' scales;
3. when the anchor is off screen, clamps the element to the screen edge (keeping a margin of
   three quarters of its height at the top) and turns it toward the anchor (`OffScreenIndicator`
   of `selection`, 64 × 64, carries the image for this case).

> **TR7.** Tessera has no `Skill_DefaultSkillEnabled` from the feed; it takes `floaty_combat`
> while the player's last `CombatState` is `in_combat`, `floaty_peace` otherwise. Tessera does
> not test line of sight (the `_no_los` states are never used) (OPEN: the test and its
> interval).

**Name block (E3).** `SelectionNameTextTemplate` (512 × 184, hidden by the layout until shown)
holds `EntityName` (font of `floatynamefont`, outline), `EntityTitleOrGuild` (font of
`floatynamefont_small`, markup on), `EntityMiniVitals` at (0, 160) (based on
`SelectionMiniVitals` of 0x2200007E: 105 × 27, health and power meters 100 × 9 and a frame
image) and `EntityMiniInduction` at (0, 187) (based on `MiniInduction`: 104 × 13, one meter
filling from the left, §7.3). The mini vitals are gated by the preference bit
`FloatingSelectionVitals` (0x8), the mini induction by `FloatingInductionBars` (0x2000000)
(§6.7).

> **TR8.** The name block is drawn over the selection with the §6.4 anchor (offset 0, the
> settings' scales); the name text takes the colour state of §5.4; the title line is empty
> (no title or guild in the feed); the mini vitals show the selection's health and power as in
> §5.3 when the selection has vitals; the mini induction shows the selection's cast (§7.3).
> Both preference bits are taken as set.

**Mini vitals (Confirmed, CU27; answers the implementer OPEN of 2026-10-10).**
`EntityMiniVitals` is based on `SelectionMiniVitals` (0x2200007E; widget type
`MultiVitalDisplay`, 105 × 27), whose children are, in read order: `OpponentHealthField` at
(2, 2) 100 × 9 (based on `VitalField` of 0x2200007F, four states) with
`CurrentOpponentHealthMeter` (based on `VitalColorMeter`, so it carries the §5.3 colour states)
and `TemporaryOpponentHealthMeter`; `RageMeter` at (2, 15) 100 × 9 with `CurrentRageMeter`;
`OpponentPowerField` at (2, 15) 100 × 9 with `CurrentOpponentPowerMeter` and
`TemporaryOpponentPowerMeter`; and `SelectionMiniVitals_Overlay` (105 × 26), the frame image.

> **TR8a (mini vitals).** Health sets `CurrentOpponentHealthMeter` and power
> `CurrentOpponentPowerMeter` (found by label); the temporary meters stay at 0 (TR10); the
> health meter takes the TR14 state; `RageMeter` and the overlay keep the layout's visibility
> (P2 has no wrath for other entities). The induction meter of `EntityMiniInduction` takes `vitalmeter_yellow` or
> `vitalmeter_grey` (§7.2).

---

<a id="s4"></a>
## 4. Player frame (E5, P4)

<a id="s4-1"></a>
### 4.1 Tree (Confirmed, CU2)

`AvatarPanel` (based on `HiddenDragBox_Template`; design rect (2, 2) 280 × 300; saves its
location; `game-panels.md` §4). Children of `VitalsParent` (0, 20), 300 × 300, in read order:

| Element | Type | Box | Content |
|---|---|---|---|
| `LoE_VitalsField` | based on `LoE_BaseVitalsField` of 0x2200007F | (64, 58) 224 × 40, z −5 | not used in P2 |
| `PlayerPortrait` | `TargetViewport` | (21, 21) 62 × 62 | the player's portrait (`ui-layout.md` §17.4) |
| `AvatarPanel_COVER_LOWRES` | Field, 13 states | (19, 19) 62 × 62 | portrait cover per frame state |
| `VitalsBg` | Field | (88, 37) 175 × 27, z 1 | meter background |
| `VitalsField` | based on `RealVitalsField` (§4.3) | (96, 33) 164 × 42 | health, power, wrath meters and texts |
| `AvatarPanel_BG` | Field, 125 states | (−1, 0) 273 × 124, z −10 | frame art: the 113 vital-display-override states (`normal` and 112 others, `game-controls.md` §7.1) and 12 `monster_…` states |
| `PlayerVitals_Animations` | Field, 12 states | (2, 0) 277 × 104 | animated frame overlays of some states (24-frame animations, 1 s) |
| `PlayerLevel_BG` | Field, 86 states | (19, 76) 65 × 32 | level plate per frame state |
| `PlayerLeaderIcon`, `PlayerAssistantIcon` | Field | (74, 68) 16 × 16 | fellowship roles (P4) |
| `PlayerVoiceChat`, `…Squelched`, `PlayerSendingData` | Field | (71, 11) 20 × 20 | not used |
| `PlayerName` | `PropertyDisplay` | (87, 3) 191 × 36 | `Name` of the player; font 0x4200000B, colour 0xFFF5DF92, outline, left aligned, truncated with a tooltip |
| `PlayerEffectDisplay` | `EffectDisplayList` | (92, 74) 169 × 200, z −10 | §8 |
| `PlayerLevel_HWField` / `PlayerLevel` | `HideWhenField` / `PropertyDisplay` | (32, 82) 38 × 17 | `Advancement_Level`; font 0x42000018, centred |
| `PlayerScaledLevel_HWField` | `HideWhenField` | same box | scaled level (not used in P2) |
| `PlayerPVP_Info` | Field | (−4, −3) 34 × 98 | PvP rank (not used) |

> **OPEN:** (implementer, `crates/ui` and `apps/client`, 2026-10-11) how a `PropertyDisplay`
> (`PlayerName`, `PlayerLevel`, and `OpponentLevel` of §5.1) draws its property, and what it
> shows before the property has a value, are not given. Tessera creates the game type
> `PropertyDisplay` as a Text element (registered before the HUD panels are built,
> `CombatVocab::register_classes`), so the layout's font, colour, outline and alignment apply,
> and the frames set the text themselves: the name of the frame's view, the level of TR16. Until
> a frame has a view, and while a level is unknown, the text is empty (never the layout's
> design-time text). The voice-chat and PvP rows above marked "not used" (`PlayerVoiceChat`,
> `…Squelched`, `PlayerSendingData`, `PlayerPVP_Info`) keep the layout's own visibility; whether
> the original's code hides the voice-chat icons when voice chat is off is not given.

<a id="s4-2"></a>
### 4.2 Frame state

The frame art states are applied to `AvatarPanel_BG`, `PlayerVitals_Animations`,
`PlayerLevel_BG` and `AvatarPanel_COVER_LOWRES` together (the same mechanism as the target
frame, §5.2; the player variant takes the vital-display-override state).

> **TR9.** The player frame takes the state `normal` (`game-controls.md` §7.1 Q13 Tessera
> rule). Monster-play states are not used.

<a id="s4-3"></a>
### 4.3 Vital fields (Confirmed, CU4)

`RealVitalsField` (0x2200007F; 168 × 42) holds:

| Part | Element | Box | Meter | Fill |
|---|---|---|---|---|
| Health | `HealthVitalField` › `HealthField` › `CurrentHealthMeter` | (0, 12) 164 × 9, z 10 | child meter, image 0x41007DF3, draw mode Normal | direction 4 (from the left edge, `ui-widgets.md` §4.3) |
| Temporary health | `TemporaryHealthMeter` | (0, 12) 164 × 9, z 9 | image 0x41007DF4, Alphablend | from the left |
| Dread clamp | `DreadField` › `DreadMeter` | (0, 12) 164 × 9 | image 0x410000E2 | from the left |
| Health text | `CurrentHealthText` | (0, 8) 162 × 16 | — | hidden by the layout; white, outline, centred; its design-time sample has the form *current*/*maximum* |
| Power | `PowerField` (at (0, 17)) › `CurrentPowerMeter` (at (0, 6) in it) | (0, 23) 164 × 9 | image 0x41007DF5 | from the left |
| Temporary power | `TemporaryPowerMeter` | same | 0x41007DF4 | from the left |
| Power text | `CurrentPowerText` | (3, 20) 162 × 16 | — | hidden by the layout |
| Wrath | `RageMeter` (at (1, 17)) › `CurrentRageMeter`, `RageText` | (1, 23) 164 × 9 | image 0x410000E2; `UIComponent_PipDisplay_Type` `Wrath` (26) | from the left |

Boxes in this table are relative to `RealVitalsField`.

`VitalHealthFieldBase` and `VitalPowerFieldBase` (the `MultiVitalDisplay` bases) name the parts
through `UI_VitalDisplay_Array`: per vital its meter, temporary meter, text, and for health the
clamped (dread) meters; the power entry is replaced by the wrath meter for characters whose pip
type is `Wrath` (`UI_VitalDisplay_ReplacementField` = `VitalPowerField`). The power and wrath
swap matches `character-panels.md` §3.4 (class key 214).

> **TR10 (vital display).** For each `VitalChanged { entity: player, vital, current, max }`:
> health sets `CurrentHealthMeter` to *p* = current ÷ max (0 when max is 0), clamped to [0, 1];
> power sets `CurrentPowerMeter` likewise. Temporary and dread meters stay at 0 (OPEN: the feed
> has no temporary vitals and no dread clamp). The vital texts stay hidden as in the layout;
> while the pointer is over the vital field, the client shows them with the text
> "*current*/*maximum*" in whole numbers (rounded half away from zero) (OPEN: when the original
> shows them; owner check OC4). A `Resource(ref)` vital whose ref resolves to a pip record of
> type `Wrath` replaces the power meter by the wrath meter (`RageMeter` shown, `PowerField`
> hidden) and fills it with current ÷ max.

<a id="s4-4"></a>
### 4.4 Class resources (Confirmed data, CU23)

P3 holds six `PipDisplay` elements stacked on one another (`game-panels.md` §7), each based on a
template of `pip_display` with `UIComponent_PipDisplay_Type` = a `PipType` key
(`player-character.md` §10.2: `Aim` 1, `Fervor` 2, `Rampage` 3, `Atunement` 4, `Momentum` 6,
`Venom` 7, `Mettle` 28, `Balance` 29 among the templates) and a `UIComponent_PipDisplay_List` of
{state, value} pairs.

> **TR11.** A `VitalChanged` with `Resource(ref)` whose ref resolves to a pip record of type
> *t* ≠ `Wrath` shows the P3 `PipDisplay` whose type is *t* (the others stay hidden, as
> `game-panels.md` §7 hides them) and sets the state of its list entry whose value equals
> round(current) (the nearest lower value when none is equal; the default state when none is
> lower). A literal ref, or a ref that resolves to no pip record, is shown only by the fallback
> look of §13.4.

**Entries (Confirmed, CU28; answers the implementer OPEN of 2026-10-10).** Each entry of a
`UIComponent_PipDisplay_List` is a `UIComponent_PipDisplay_Entry` struct of two members:
`UIComponent_PipDisplay_EntryState` (an enum key of `UIStateID`, the state to enter) and
`UIComponent_PipDisplay_EntryValue` (an int, the pip count it stands for). All 138 entries of
the `pip_display` templates have exactly these two members. TR11 reads them by name.

---

<a id="s5"></a>
## 5. Target frame (E6, P5)

<a id="s5-1"></a>
### 5.1 Tree (Confirmed, CU3)

`OpponentPanel` (based on `HiddenDragBox_Template`; design rect (279, 2) 305 × 300; title token
0x0B0D5E03 of table 0x250001BB; saves its location; UI-scaler group `OpponentVitals` (4)).
Children:

| Element | Box | Content |
|---|---|---|
| `LockOnOverlay` | (278, 33) 31 × 31, z −10 | lock-on marker with a tooltip (token 0x0591D2F9); not used in P2 |
| `VitalsParentOpponent` › `OpponentDisplayParent` | (0, 20) 302 × 300 | the **vital field** of a target with vitals |
| › `OpponentOpenTapping` | (12, 37) 36 × 36, 3 states | open-tapping marker; not used |
| › `OpponentEffectDisplay` (`EffectDisplayList`) | (37, 74) 170 × 200 | §8; `UIComponent_EffectDisplay_MaxIconColumns` 8 |
| › `Opponent_SelectionField` | (−8, 0) 304 × 120, z 1 | holder of the parts below |
| ›› `OpponentLevel_BG` | (220, 78) 65 × 32, 102 states | level plate per frame state |
| ›› `OpponentsVitals_BG` | (44, 37) 176 × 32 | meter background |
| ›› `OpponentVitalsField` (based on `RealOpponentVitalsField`) | (44, 33) 168 × 42 | §5.3 |
| ›› `OpponentPortrait` (`TargetViewport`) with `Target_Image` | (220, 19) 62 × 62 | portrait (UI scene 0x21000004, camera `Camera_1`, render target 62 × 62, opacity mask 0x40000E4C) |
| ›› `OpponentPanel_BG` | (31, 0) 273 × 124, 139 states | frame art |
| ›› `OpponentClass` (based on `ClassDisplay_20Template`) | (211, 72) 20 × 20 | class icon of a player target |
| ›› `OpponentPanel_COVER_LOWRES` | (221, 20) 62 × 62, 27 states, hidden | portrait cover |
| ›› `OpponentVitals_Animations` | (28, −1) 277 × 104, 26 states | animated frame overlays |
| ›› `OpponentLevelHiddenSkull` | (220, 79) 65 × 32 | shown instead of the level when the level is hidden |
| ›› `OpponentLevel_HWField` › `OpponentLevel` (`PropertyDisplay`) | (232, 82) 38 × 17 | `Apparent_Level` of the entity named by `Entity_Selection` |
| ›› `OpponentPVP_RankDisplay` | (274, −3) 32 × 32 | PvP rank; not used |
| ›› `OpponentName` (based on `ColorNameText`) | (2, 3) 216 × 36 | the target's name, truncated with a tooltip |
| `InanimateObject_SelectionField` | (40, 1) 228 × 84 | the **non-vital field**: `InanimateObjectPanel_BG` (7, 26) 215 × 51, `InanimateObjectForeground` (−2, 19) 231 × 66, `InanimateObjectName` (based on `ColorNameText`, (11, 29) 205 × 47) |

<a id="s5-2"></a>
### 5.2 Field choice and frame decoration (BIN d, CU6)

On a selection change and on changes of the target's watched properties, the client:

1. decides whether the target **has vitals** (an entity-type test of the client; pets are
   special-cased through `AI_Pet_Master`). Without vitals it shows
   `InanimateObject_SelectionField`, hides `OpponentDisplayParent`, and writes the name into
   `InanimateObjectName`. With vitals it shows `OpponentDisplayParent`, hides the non-vital
   field, and writes the name into `OpponentName`;
2. sets the health meter's colour state (§5.3);
3. for a non-player target, reads `MonsterLevel_ExaminationModStatType` (enum mapper
   0x23000070) and sets the **decoration state** by this table on `OpponentPanel_BG`,
   `OpponentVitals_Animations` and `OpponentLevel_BG`:

| Key (label) | State | Key (label) | State |
|---|---|---|---|
| 1 `Swarm` | `monster_swarm` | 34 `Rare_Signature` | `monster_signature_rare` |
| 2 `EliteMaster` | `monster_elitemaster` | 35 `Rare_Swarm` | `monster_swarm_rare` |
| 3 `Nemesis` | `monster_nemesis` | 36 `Rare_EliteMaster` | `monster_elitemaster_rare` |
| 4 `Signature` | `monster_signature` | 37 `Rare_Nemesis` | `monster_nemesis_rare` |
| 6 `ArchNemesis` | `monster_archnemesis` | 38 `Rare_Elite` | `monster_elite_rare` |
| 7 `Elite` | `monster_elite` | 39 `Rare_Normal` | `monster_normal_rare` |
| 8 `GoodElite` | `monster_goodelite` | 40 `Rare_ArchNemesis` | `monster_archnemesis_rare` |
| 9 `GoodEliteMaster` | `monster_goodelitemaster` | 42 `Scion_Swarm` | `monster_swarm_scion` |
| 10 `GoodArchNemesis` | `monster_goodarchnemesis` | 43 `Scion_ArchNemesis` | `monster_arch_scion` |
| 11 `GoodNemesis` | `monster_goodnemesis` | 44 `Scion_Signature` | `monster_signature_scion` |
| 12 `GoodSignature` | `monster_goodsignature` | 45 `Scion_Elite` | `monster_elite_scion` |
| | | 46 `Scion_Nemesis` | `monster_nemesis_scion` |
| | | 47 `Scion_EliteMaster` | `monster_elitem_scion` |
| | | 48 `Scion_Normal` | `monster_normal_scion` |

   Every other value (0, 5 `Normal`, the raid keys 25–32) and an absent property give
   `monster_normal`. A player target takes its vital-display-override state instead;
4. hides the level and shows `OpponentLevelHiddenSkull` when the target's
   `NPC_HideLevelInDisplay` is true; hides the level (without the skull) when
   `NPC_HideLevelAndVitalText` is true; otherwise shows `OpponentLevel`;
5. for a player target, sets `OpponentClass`'s `UI_ClassDisplay_Type` from `Agent_Class` and
   shows it; hides it otherwise;
6. points `OpponentEffectDisplay`'s `UIComponent_EffectDisplay_TargetIID` at the target (§8).

Census (OBS, CU26; every 0x47 entity descriptor, 0x78 and 0x79 property set, at every depth;
revision 1 restates the first version's paragraph, which added nested values to set counts and
missed the 0x47 descriptors):

| Property | Where stored | Count | Values |
|---|---|---|---|
| `MonsterLevel_ExaminationModStatType` | top level of a 0x79 set | 16,605 sets (one value each) | 5: 16,044; 9: 152; 8: 139; 12: 91; 10: 38; 11: 33; 7: 28; 3: 23; 6: 22; 4: 19; 2: 16 |
| `MonsterLevel_ExaminationModStatType` | nested inside another top-level property of a 0x79 set (`SpeciesLevelLootTable` 264 values, `Mod_Array` 45) | 309 values in 68 sets; none of the 68 stores it at top level | 0: 226; 7: 52; 4: 21; 8: 2; 1: 1; 42–48: one each |
| `MonsterLevel_ExaminationModStatType` | 0x47 or 0x78 | none | |
| `NPC_HideLevelInDisplay` | top level of a 0x79 set; top level of a 0x47 descriptor | 949 sets; 1 descriptor (950 in total) | true in all |
| `NPC_HideLevelAndVitalText` | top level of a 0x47 descriptor | 8 descriptors; no 0x78 or 0x79 set | true in all |

So 16,605 sets carry a decoration key that step 3 can read; 16,914 (16,605 + 309) counts
values, not sets. The values 0, 1 and 42–48 occur only nested, and no record stores 25–32 or
34–40 anywhere; the `Scion_*` and `Rare_*` rows of the table above therefore select no frame from
this install's templates. No hide property is nested anywhere.

The 0x47 descriptors with a hide property are reached from templates through `PhysObj`
(`creatures.md` §4.2): 12 0x79 sets name one of 7 of them (one descriptor is named by no 0x79
set). 11 of the 12 sets name a descriptor that stores `NPC_HideLevelAndVitalText` and store
`NPC_HideLevelInDisplay` (true) themselves; 1 set stores neither property and names the
descriptor that stores both. These counts are observations of this install.

> **TR12 (has vitals).** An entity has vitals for display when the session has reported at
> least one `VitalChanged` with `vital = Health` for it. M7's test actors and scenario actors
> without a health report get the non-vital field (ROADMAP P2 exit criterion 4).

> **TR13 (decoration).** The decoration key is read from the target's **body set**: the
> property set that `property-system.md` §6.2 gives for the `Body::Object` DID of
> `EntityCreated` (`scenario-feed.md` §5.4). For a game object of prefix 0x70–0x76 that is its
> set (the object's prefix + 9: 0x79 for a 0x70 object; on this install only 0x70 objects have
> a set, `property-system.md` §6.2); for a DID of prefix 0x78–0x7F it is that record itself; a DID of any other prefix
> (0x77 and a 0x47 entity descriptor included) has no body set. (Revision 3: the earlier wording
> named prefix 0x79 for every game object and made a 0x78–0x7F DID "no body record";
> `tessera-gamedata` already follows the reading above.) Steps 3 and 4 read
> `MonsterLevel_ExaminationModStatType`, `NPC_HideLevelInDisplay` and
> `NPC_HideLevelAndVitalText` at the **top level** of the body set only. They do **not** read
> values nested inside other properties, or the 0x47 entity descriptor that the set names in
> `PhysObj`. When one set stores both hide properties as true, the skull case of step 4
> applies. Decoration: a non-player target with a body set takes the state of step 3
> (`monster_normal` when the set stores no key); without a body set, `monster_normal`
> (creatures and NPCs) or `normal` (other objects). A **player** target takes `normal` (TR9)
> whether or not it has a body set; step 4 still reads the player's body set when there is
> one. On this install the reading leaves one template showing its level although its
> `PhysObj` descriptor stores both hide properties; the 11 others named in the census show the
> skull from their own `NPC_HideLevelInDisplay`.

> **OPEN:** (revision 1, 2026-10-11) how the original composes a live entity's properties from
> its template, its 0x47 descriptor and server state, and so whether a descriptor's
> `NPC_HideLevelAndVitalText` or `NPC_HideLevelInDisplay` reaches step 4, was not traced (BIN,
> later; owner check of the one template above when the owner meets it). TR13 stays as written
> until then.

> **Answered (revision 3, 2026-10-14; was OPEN, implementer, 2026-10-10):** a player target with
> a body set takes `normal` (TR13, last sentences), as `crates/gamedata` does.

<a id="s5-3"></a>
### 5.3 Target vitals (Confirmed data, CU4; colour BIN d)

`RealOpponentVitalsField` mirrors §4.3: `OpponentHealthField` with `CurrentOpponentHealthMeter`
(based on `VitalColorMeter`: child meter, **direction 3, from the right edge**, images by state
`vitalmeter_grey` 0x41007DF7, `vitalmeter_red` 0x410000E2, `vitalmeter_green` 0x41007DF3,
`vitalmeter_yellow` 0x41007DF8, `vitalmeter_blue` 0x41007DF5) and
`TemporaryOpponentHealthMeter`; `CurrentOpponentDreadField`; `OpponentPowerField` (state
`vitalmeter_blue`); `RageMeter`; texts `CurrentOpponentHealthText` and
`CurrentOpponentPowerText` (hidden by `RealVitalText`'s layout).

The health meter's **colour state** (BIN d) is one of `vitalmeter_red` (the default),
`vitalmeter_green`, `vitalmeter_grey` and `vitalmeter_yellow`, chosen from the entity's type
flags, its `AI_KungFu_IsNPC` property (true gives green), tapping and attackability tests and
`Combat_Violent`. Those tests are game rules (reaction and attackability).

> **TR14 (colour).** Tessera does not compute reactions. The health meter takes
> `vitalmeter_green` for targets of kind `Player` and `NonPlayer`, `vitalmeter_red` for
> `Creature`, `vitalmeter_grey` for a defeated target (§10.3); `vitalmeter_yellow` is not used
> (OPEN: owner check OC5 on the original's colours for neutral and tapped targets). Values follow
> TR10 with the target's reports.

> **TR14a (objects with vitals; answers the implementer OPEN of 2026-10-10).** A target of kind
> `Object` that has vitals (TR12) takes `vitalmeter_red`, the meter's default state (§5.3), or
> `vitalmeter_grey` when defeated.

<a id="s5-4"></a>
### 5.4 Name and name colour (Confirmed data, CU5; rule TR15)

`ColorNameText` (text layout element 0x10000D0D) has these colour states (ARGB):

| State | Colour | State | Colour |
|---|---|---|---|
| `ConTrivial_ColorName` | 0xFF7F7F7F | `PlayerNormal_ColorName` | 0xFFBCBC55 |
| `ConInsanelyEasy_ColorName` | 0xFF00D500 | `PlayerFellowship_ColorName` | 0xFF238623 |
| `ConVeryEasy_ColorName` | 0xFF00FFFF | `PlayerKinship_ColorName` | 0xFF06F8F8 |
| `ConEasy_ColorName` | 0xFF008BE9 | `PlayerRaid_ColorName` | 0xFF2A53D4 |
| `ConEven_ColorName` | 0xFFFFFFFF | `PlayerRP_ColorName` | 0xFFFFFFFF |
| `ConTough_ColorName` | 0xFFE5E500 | `NPC_ColorName` | 0xFFD9DE0B |
| `ConVeryTough_ColorName` | 0xFFFF7F00 | `Object_ColorName` | 0xFFFFFFFF |
| `ConInsanelyTough_ColorName` | 0xFFD00000 | `Corpse_ColorName` | 0xFFFFFFFF |
| `ConImpossible_ColorName` | 0xFFC200C7 | `Tapped_ColorName` | 0xFF7F7F7F |

(and PvP, item-quality, effect, skill, trait and `Floaty_CriticalDamage` states, §6.3). The
consideration states are the `ConsiderationControl_RankToUIState` targets of
`game-controls.md` §7.5.

> **TR15 (name colour).** The client sets the name element's state (in E3, E6, E7):
> 1. defeated target (§10.3): `Corpse_ColorName`;
> 2. kind `Creature` with a known level for both the player and the target: the consideration
>    state by the data-consistent reading of `game-controls.md` §7.5 (list element by the
>    **player's** level, table element *d* + 15 with *d* = target level − player level, clamped
>    to 0–30; consider type `Monster`), then `RankToUIState`; without both levels,
>    `ConEven_ColorName`;
> 3. kind `NonPlayer`: `NPC_ColorName`; kind `Player`: `PlayerNormal_ColorName`; kind `Object`:
>    `Object_ColorName`.
> Levels come from `LevelChanged` (`scenario-feed.md` §9.2) and `CharacterInfo` (player). This
> is a reading, not a traced rule (`game-controls.md` Q16; owner check OC6).

<a id="s5-5"></a>
### 5.5 Level

> **TR16.** `OpponentLevel` shows the target's last reported level (`LevelChanged`) as a decimal
> integer; without one it shows nothing. §5.2 step 4 applies to targets with a body record.

<a id="s5-6"></a>
### 5.6 Visibility of the target frame

> **TR17.** P5 is shown while there is a selection other than none (the player selected itself
> included) and hidden otherwise (`ui-layout.md` §17.4 P5 row). Its drag-box behaviour follows
> `game-panels.md` §4.

<a id="s5-7"></a>
### 5.7 Selection's target (E7, Confirmed data, CU21)

`GamePlay_SelectionTarget` (`gameplayactivatables`, created as a root; design rect (200, 450)
140 × 71; title token 0x0234CA6C; UI-scaler group `Vitals` (11); object mode CanvasResize)
holds `GamePlay_SelectionTargetDisplay` based on `SelectionTargetBase` (0x220000C4; widget type
`SelectionTargetDisplay`, 140 × 51): `SelectionTargetNameText` (based on `ColorNameText`,
(39, 0) 100 × 20, font 0x42000008), `SelectionTargetVitalField` (based on
`MicroVitalFieldBase`, (44, 22) 94 × 15), `SelectionTarget_Background` ((1, 7) 141 × 44, image
0x4101FD50) and `TargetNonVitalField` ((1, 6) 140 × 48) for targets without vitals. The type's
properties name these parts (`UI_SelectionTarget_*`).

> **TR18.** E7 shows the selection's own target (the last `TargetChanged { entity: selection }`)
> while there is one, with its name (TR15) and health (TR10 colouring by TR14), or the
> non-vital part (TR12); it is hidden otherwise. (OPEN: whether the original shows it by
> default or only by an option; owner check OC7.)

**Vital field (Confirmed, CU27; answers the implementer OPEN of 2026-10-10).**
`SelectionTargetVitalField` is based on `MicroVitalFieldBase` of 0x2200007E (101 × 15), which
holds `VitalHealthField` at (0, 0) 99 × 6 (based on `VitalHealthFieldBase`) and
`VitalPowerField` at (0, 7) 99 × 6 (based on `VitalPowerFieldBase`). `VitalHealthFieldBase`
(widget type `MultiVitalDisplay`) holds, in read order, `VitalHealthBackground` (a meter, z 10)
and `VitalHealthParentField` (a meter) with `VitalCurrentHealthField` ›
`VitalCurrentHealthMeter` (based on `VitalColorMeter`, so it carries the §5.3 colour states) and
`VitalTemporaryHealthMeter`, `VitalCurrentDreadField` › `VitalCurrentDreadMeter`, and the text
`VitalCurrentHealthText`. `VitalPowerFieldBase` holds `VitalPowerBackground`, `RageMeter` ›
`CurrentRageMeter` and `VitalPowerField` › `VitalCurrentPowerMeter`,
`VitalTemporaryPowerMeter` and `CurrentPowerText`.

> **TR18a (E7 health meter).** The health position and the TR14 state go to
> `VitalCurrentHealthMeter` (found by label); the temporary and dread meters stay at 0 (TR10);
> the power part keeps the layout's look (E7 shows health only, TR18).

---

<a id="s6"></a>
## 6. Floating text (E13, E14)

<a id="s6-1"></a>
### 6.1 Templates and fonts (Confirmed, CU7)

`floatytext` (design size 0 × 0; positions are set by code) holds:

| Template | Base | Box | Properties |
|---|---|---|---|
| `DamageTextTemplate` (0x10000371) | `FloatyDamageFont` of `text` | 512 × 80, z 100 | fit to text, centred both ways, margins 5, outline colour 0xFF000000, hidden; minimum 1 × 1, maximum 512 × 320 |
| `ExperienceTextTemplate` (0x10000372) | `FloatyExperienceFont` | 512 × 80 | as above with margins 4 |
| `DialogTextTemplate` (0x10000373) | `ColorFont` | 512 × 80 | chat bubbles (not used in P2) |
| `EntityNameTextTemplate` (0x1000036E) | — | 512 × 184 | `EntityName`, `EntityTitleOrGuild`, `EntityMiniInduction` |
| `SelectionNameTextTemplate` (0x10000374) | — | 512 × 184 | §3.5 |
| `EntitySelectionIndicator` (0x1000327F) | — | 125 × 156 | §3.5 |

Fonts and colours of the bases (`text` layout): `FloatyDamageFont` font 0x4200000F, colour
0xFFF7E4A9, tag colour 0xFFFFE566, outline; `FloatyExperienceFont` font 0x42000012, colour and
tag colour 0xFF1274EB; `floatynamefont` font 0x42000013 (based on `ColorNameText`, colour
0xFFF5DF92); `floatynamefont_small` font 0x4200002A; `ColorFont` font 0x42000010.

<a id="s6-2"></a>
### 6.2 Settings (Confirmed, CU8)

The floating-text settings are a property set found through `UNIQUEDB`: label
`UI_FloatyText_Properties` (key 0x10000001 → 0x78000004) for normal entities and
`UI_DistantBattle_FloatyText_Properties` (key 0x10000048 → 0x780029BA) for distant-battle
units. The client uses the second for an entity whose type flags mark it a distant-battle unit
(BIN a), the first otherwise.

| Property | Built-in default (BIN a) | 0x78000004 | 0x780029BA |
|---|---|---|---|
| `UI_FloatyText_LOSFailureFadeTime` | 2.5 | 2.5 | 2.5 |
| `UI_FloatyText_FadeTime` | 0.6 | 0.6 | 0.6 |
| `UI_FloatyText_NearRange` (m) | 4 | 4 | 20 |
| `UI_FloatyText_FarRange` (m) | 30 | 30 | 200 |
| `UI_FloatyText_NearRangeScale` | 1.2 | 1.2 | 1.2 |
| `UI_FloatyText_FarRangeScale` | 0.5 | 0.5 | 0.5 |
| `UI_FloatyText_DialogNearRangeScale` | 1.2 | 1.1 | 1.1 |
| `UI_FloatyText_DialogFarRangeScale` | 0.8 | 0.75 | 0.75 |
| `UI_FloatyText_DamageColumns` | 3 | 3 | 3 |
| `UI_FloatyText_DamageColumnSeparation` | 36 | 36 | 36 |
| `UI_FloatyText_CriticalDamage_SpeedMod` | 0.5 | 0.0 | 0.0 |
| `UI_FloatyText_CriticalDamage_InitialScale` | 1.0 | 1.7 | 1.7 |
| `UI_FloatyText_CriticalDamage_ShrinkTime` | 0.1 | 0.06 | 0.06 |
| `UI_FloatyText_CriticalDamage_HorizontalOffset` | 50 | 60 | 60 |
| `UI_FloatyText_CriticalDamage_VerticalOffset` | 50 | 50 | 50 |
| `UI_FloatyText_CheckDistance` | true | true | false |

A property is taken only when its stored type matches (float, int or bool); otherwise the
built-in default stays. Further built-in values without a property (BIN a): a release spacing
of 0.35 (§6.5), an anchor lift of 0.05 m above the entity's bounding box (§6.4), anchor-smoothing
constants (0.001, 0.005, 0.01), a list of three column positions (−1, 0, 1), and two values
derived when the settings are read, from a screen dimension *H* in pixels (Probable: the
vertical resolution): the **travel** *H* × 0.125 and the **base rise speed** *H* × 0.125 ÷ 1.5
pixels per second.

<a id="s6-3"></a>
### 6.3 Life of one text (BIN a, CU9)

A text is created from `DamageTextTemplate` (one pooled instance per text, reused after
removal) with: its text and colour (the colour replaces the template's), a **life** *L* in
seconds, a **scale** *s*₁, a **critical** flag, an initial horizontal offset *x*₀ = 0 and an
initial vertical offset *y*₀ = −(height of the entity's name block, when the entity shows one;
0 otherwise). The generic producers (gains of §6.8) pass *L* = 2.0 s and *s*₁ = 1.5.

A **critical** text on an entity other than the local player takes the state
`Floaty_CriticalDamage` (colour 0xFFFF0000, outline 0xFFFFFFFF, of `ColorNameText`) and the
first free of six **critical slots** of that entity; the slot moves the start by (*h*·*a*,
*v*·*b*) with *h* = `CriticalDamage_HorizontalOffset`, *v* = `CriticalDamage_VerticalOffset`
and (*a*, *b*) = (0, 0), (−1, 0), (1, 0), (−1, −1), (1, −1), (0, −1) for slots 0–5 (when all six
are taken, slot 0 is reused). A critical text on the local player is shown like a normal one.

Each frame, with *t* = clamp((now − start) ÷ *L*, 0, 1.1) (the start is taken at the first
update after creation):

1. **Visible** while *t* < 1, the entity is drawn, and (when `CheckDistance` is true) the
   entity passes a distance test against the camera; otherwise the element is hidden. At
   *t* ≥ 1.05 the text is removed, its critical slot freed and its element returned to the pool.
2. **Scale.** Normal text: *k* = 1 + 2*t*(*s*₁ − 1) while *t* < 0.5, then *k* = *s*₁ (it grows
   from 1 to *s*₁ over the first half). Critical text: *k* = *s*₁ × (1 + (*c* − *t*)/*c* ×
   (*i* − 1)) while *t* < *c*, then *k* = *s*₁, with *c* = `CriticalDamage_ShrinkTime` and *i* =
   `CriticalDamage_InitialScale` (it starts at *i*·*s*₁ and shrinks to *s*₁).
3. **Rise.** The text moves up by *r* = *v*ₛ × (now − start) × *m* pixels, where *v*ₛ is the
   list's smoothed rise speed (§6.5) and *m* = 1 for normal text and
   `CriticalDamage_SpeedMod` for critical text (0 here: criticals do not rise). The vertical
   offset is *y*₀ − *r* × *f*, where *f* (0–1) is a factor of the distance test (1 when the
   test is off).
4. **Opacity.** α = 1 while *t* ≤ 0.5, then 1 − 2(*t* − 0.5), times the distance-test fade
   factor, clamped to [0, 1]; set as the element's opacity.
5. **Place** with §6.4 using the element's design size × *k*, the offsets (*x*₀, *y*₀ − *r*·*f*),
   and the settings' near and far scales.

<a id="s6-4"></a>
### 6.4 Anchor and distance scale (BIN a)

The anchor is a point above the entity: the top of its bounding box plus 0.05 m. The routine has
several anchor modes; one of them smooths the point over a ring of the last 30 positions, moving
it only when the new position departs from the average by more than the smoothing constants
(which mode a text uses is taken from its element and was not read; Tessera uses the unsmoothed
point). It is projected to the screen; a point behind the camera hides the
element when the caller asks for it. With *d* = the camera's distance to the anchor, *n* =
`NearRange`, *f* = `FarRange`, *u* = clamp((*d* − *n*)/(*f* − *n*), 0, 1) and *q* = (1 − *u*)²,
the **distance scale** is *σ* = (1 − *q*) × `FarRangeScale` + *q* × `NearRangeScale` (*σ* =
`NearRangeScale` up to the near range, falling to `FarRangeScale` at the far range). The element's
width is *W* × *σ* × (screen width ÷ 1024) × (4/3 ÷ the screen's aspect ratio) and its height
*Hₑ* × *σ* × (screen height ÷ 768), both multiplied by the factor clamp(0.75 × *g* + 0.25, 0.25,
1.75) of the user's floating-text scale preference *g* (`UI.FloatyTextScale`; *g* = 1 gives 1);
the offsets are multiplied by *σ* likewise. The element is centred horizontally on the anchor and its bottom
edge placed at the anchor plus the vertical offset.

<a id="s6-5"></a>
### 6.5 Stacking, crowding and limits (BIN a)

Each entity keeps its own list: **active** texts and a **pending** queue.

- A new text goes to the pending queue. After the frame's update of the active texts, the first
  pending text is released (made active) when the list has no active text, or when every active
  non-critical text shown this frame has risen more than (1 − 0.35) × the **pending** text's
  height (BIN k, CU31): the smallest rise *r* (§6.3, before the distance factor *f*) among those
  texts is compared; with no such text the test passes. The height is that of the pending text's
  element right after its text was set (the template fits its box to the text, §6.1), in element
  pixels, before *k* and *σ*. Release happens at most once per frame. Texts thus leave one after
  another in a column instead of overlapping.
- The **rise speed** target is the base rise speed (§6.2); when active plus pending texts
  number *n* > 2, it is the base × (1 + min(*n* ÷ 10, 1)) (up to twice as fast). The smoothed
  speed *v*ₛ starts at the base rise speed when the list is created. Each frame the list first
  reads *v*ₛ (the value reached towards the previous target), uses it for all its texts, and
  then sets the new target (BIN k, CU31): between two frames Δ*t* apart, *v*ₛ moves by
  (1 − 2^(−Δ*t* ÷ 1 s)) of its distance to the target, so it closes half of the gap per second,
  and it takes the target's value once it is within 0.0002 of it. The 1 s time constant is the
  smoothing helper's initial value; neither the list's creation nor its update changes it.
- A new text is refused while more than 50 texts of the entity are **pending**; released
  (active) texts are not counted, and are bounded by the release rule.
- `UI_FloatyText_DamageColumns`, `UI_FloatyText_DamageColumnSeparation` and the built-in column
  list (−1, 0, 1) were not found in use in the traced path (OPEN); the texts of one entity share
  one column; Tessera does the same.

> **TR19a (figure box; answers the implementer OPEN of 2026-10-11).** For a skinned figure,
> "the top of its bounding box" (§6.4) and the box that the mouse pick of TR3 tests are the box
> of the joints of the figure's current pose (else its reference pose), turned and scaled by its
> placement, grown by 0.15 m on every side; a figure without a skeleton takes 0.7 × 0.7 × 1.8 m,
> scaled. The anchor is the top centre of that box plus 0.05 m. (Tessera's choice: the
> original's bounding-box source for figures was not traced.)

> **Answered (revision 3, 2026-10-14, BIN k; was OPEN, implementer, 2026-10-10):** the rise
> speed closes half of its gap to the target per second (above), not a 0.25 s time constant; the
> release compares the smallest rise of the shown active non-critical texts with (1 − 0.35) ×
> the pending text's fitted height, not with the layout height of the template (80) and not
> with the most recent text's own height. Without an install (§13.6) the fitted height is that
> of the fallback text's box.

<a id="s6-6"></a>
### 6.6 Combat wording and colours (OPEN; tokens CU11)

The routine that builds the text of a damage or heal report (number, wording, colour, life,
critical flag) and hands it to the per-entity list (an interface method shared by the list and
by the target portrait, §6.9) was not found. The class library holds UI-table tokens (table
key `UI` 0x10000065 → 0x250001BB) for the avoidance results, in one routine (BC):

| `HitResult` (feed) | `CombatResultType` key (0x2300002A) | Short-form token | Past-tense token |
|---|---|---|---|
| `Miss` | 12 `Miss` | 0x0D9CB273 | 0x0CB27424 |
| `Evade` | 1 `Evade` | 0x09B5EA75 | 0x0B5EA724 |
| `Block` | 3 `Block` | 0x09B2184B | 0x02185804 |
| `Parry` | 5 `Parry` | 0x09CFA449 | 0x0FA45674 |
| `Resist` | 7 `Resist` | 0x0C1E5D34 | 0x0E5D36A4 |
| `Immune` | 8 `Immune` | 0x0B7297D5 | — |
| (deflected) | 13 `Deflected` | 0x0392F214 | 0x02F21924 |
| (interrupt) | 15 `Interrupt` | — | 0x002B0DA4 |

`CombatResultType` also has `Hit` 9, `CriticalHit` 10, `SuperCriticalHit` 11, the partial
results 2, 4, 6 and `Failure` 14. Which form the floating text uses is not known.

> **TR19 (wording).** For a `VitalDelta` (`scenario-feed.md` §9.2), the floating text is:
> - `result` ∈ {`Normal`, `Critical`} and `amount` > 0: the amount rounded half away from zero,
>   as a decimal integer with no grouping, prefixed by "−" for `Damage` and "+" for `Heal`;
> - `result` ∈ {`Miss`, `Evade`, `Parry`, `Block`, `Resist`, `Immune`}: the short-form token of
>   the table above, resolved in the user's install (`string-table.md`); without an install,
>   or when the token does not resolve, the fallback word of §13.6;
> - amount 0 with `Normal` or `Critical`: no text.
> The critical flag is set for `Critical`. (Owner check OC8: wording, sign and form.)

> **TR20 (colours, life, scale).** Damage on another entity: the template colour (`FloatyDamageFont`,
> 0xFFF7E4A9). Damage on the local player: 0xFFFF4040. Heal on any entity: 0xFF40FF40.
> Avoidance words: 0xFFFFFFFF. Life 2.0 s and scale 1.5 for all (as the generic producers of
> §6.3). (Owner check OC8.)

> **TR21 (which entity).** The text of a `VitalDelta` is shown over its `target`. It is shown
> when `target` or `source` is the local player (the original's preference bits `My` and
> `Others`, §6.7, taken as set), and also for deltas between two other entities (`All`, taken as
> set). `vital` other than health is shown like health (OPEN: the original's handling of power
> deltas).

<a id="s6-7"></a>
### 6.7 Preference bits (BIN b, CU10)

`GOP_FloatyInformation_Bitfield` holds the floating-text options; the options panel's check
boxes map to bits: `MyFloatingDamage` 0x1, `AllFloatingDamage` 0x2, `PlayerFloatingGuildNames`
0x4, `FloatingSelectionVitals` 0x8, `OthersFloatingDamage` 0x10, `ItemFloatingNames` 0x40,
`FloatingChatBubbles` 0x80, `NPCFloatingNames` 0x200, `AllPlayerFloatingNames` 0x400,
`PlayerFloatingTitles` 0x800, `FloatingDestiny` 0x8000, `PlayerPetsFloatingName` 0x800000,
`MyPetsFloatingName` 0x1000000, `FloatingInductionBars` 0x2000000 (labels are the options'
string IDs without their prefix). The generic producers test their own bit: experience 0x20000,
glory 0x80000, marks 0x100000 (BIN; §6.8). A text whose bit is clear is not created. The user
preferences `UI.FloatyTextScale`, `UI.FloatyTextOpacity`, `UI.FloatyTextOnTop` also exist.

> **TR22.** Tessera keeps no such preference in P2: every bit is taken as set; scale and opacity
> preferences are 1; "on top" is true (floating text is drawn over the 3D scene, never hidden by
> it).

<a id="s6-8"></a>
### 6.8 Gains (Confirmed path, BIN a)

The generic producers make texts of the form "+*value* …" from UI-table tokens with a value
variable, in fixed colours (RGBA floats): experience gained (`XP_EarnedXP` increase; token
0x025DBC05; blue 0,0,1,1; bit 0x20000), glory (`Glory_GloryPoints` increase; token 0x055DC80E
or 0x02333A99 by faction; magenta 1,0,1,1; bit 0x80000), marks (token 0x07F7F5C8; yellow 1,1,0,1;
bit 0x100000) and destiny points (token 0x07B54379; cyan 0,1,1,1; bit 0x8000). They are drawn over
the local player.

> **TR23.** P2 shows no gain texts (the feed's `ExperienceChanged` belongs to P6); the
> `ExperienceTextTemplate` is not used.

<a id="s6-9"></a>
### 6.9 Portrait floaters (BIN a)

When the player's preference `GOP_Show_Selection_Portrait_FloatyDamage` is true, the target
frame also receives each damage text and shows it over `OpponentPortrait`: up to 10 at a time
(the oldest is dropped), each from `DamageTextTemplate`, drawn 10 levels above the panel, moving
with a random direction (horizontal −2 to −4, vertical 0 to 2, normalised and scaled by a random
1 to 3) and a random life of 0.45–0.65 s (the exact use of the four random ranges is Probable).

> **TR24.** Off in P2 (the preference is taken as false).

---

<a id="s7"></a>
## 7. Cast, induction and channel bars

<a id="s7-1"></a>
### 7.1 Elements (Confirmed, CU15)

`progress` holds `PlayerProgressUIElement` (widget type `PlayerProgress`, 255 × 26, auto-fade
0.5) and `OpponentProgressUIElement` (`OpponentProgress`, 255 × 26, auto-fade 0.5), each with:

| Child | Box | Content |
|---|---|---|
| `Progress_BaseBox` (based on `ProgressUIElement_Overlay_Background`) | (0, 0) 255 × 26, z 2 | frame |
| `ProgressUIElement_Meter` (Meter) | (12, 6) 234 × 13 | child meter, **direction 4 (fills from the left edge)**, draw mode Alphablend, `move_fill` false, `smooth_movement` true; image 0x41007E92 (player) or 0x411EA9D0 (opponent; 0x41007E93 in the opponent's `AltInit` state) |
| `BaseBox_Empty` | (0, 0) 255 × 26 | overlay |
| `ProgressUIElementTest_Label` (based on `verdana14`) | (11, 4) 238 × 16 | white, outline, centred, truncated with a tooltip |

`UI_Progress_Element_List` names the meter and the label. States of the root: `Init`,
`Succeeded`, `Interrupted`, `Abandoned`, `Failed` (and `AltInit` on the opponent's). Each end
state plays: a pause of 1 s, then the message `progress_stop` (UIElementMessage 0x1000000B) to
game code; `Interrupted` first shows the image 0x41007F68 (draw mode 2).

Mounted and music variants (`PlayerProgressUIElement_Mounted`, `…_Music`, 290 × 43) exist; the
induction record's `Channeling_ProgressBarElement` or `Induction_ProgressBarElement` chooses among
the three (`interaction.md` §3.9.1).

Placement (`gameplayactivatables`, created as roots, saved locations, edges (1, 4, 1, 5)):
`GamePlay_ProgressBar_Dragbox` at (380, 598) 320 × 66 (UI-scaler group `ProgressBar` (54)) holds
`GamePlay_ProgressBar` at (32, 28), `GamePlay_ProgressBar_Mounted` and `GamePlay_ProgressBar_Music`
at (15, 12); `GamePlay_Opponent_ProgressBar_Dragbox` at (0, 500) 260 × 66 (group
`TargetProgressBar` (65)) holds `GamePlay_Opponent_ProgressBar` at (2, 28). Their titles are
UI-table tokens 0x01F42612 and 0x0BEAECB4.

<a id="s7-2"></a>
### 7.2 Style and label (BIN g, interaction.md §3.8.4)

For another entity's induction (the target's bar and the floating mini bar), the client reads the
induction record named by `Skill_InductionAction`, the start and end times, and the
interruptible flag of `interaction.md` §3.8.4. Interruptible → the opponent bar's state
`AltInit` and the mini bar's meter state `vitalmeter_yellow`; not interruptible → `Init` and
`vitalmeter_grey`. The label is the title of §3.8.4.

> **TR25 (style and label for the feed).** The feed reports casts by `CastStarted { entity,
> cast, what, target, duration_ms }`. The bar's label is: for `Ref::Record` of a skill, its
> `Skill_Name`; of an induction or channelling record, the §3.8.4 title; for a literal ref, the
> literal; the label is cut to fit by the layout's truncation. Interruptible is: the induction or
> channelling record's flag when `what` is one or a skill whose `Skill_InductionAction` names one;
> otherwise true. `CastKind::Channel` uses the plain bar (TR27 for music and mounted bars).

<a id="s7-3"></a>
### 7.3 Fill and timing

> **TR26 (fill).** On `CastStarted` for the player (E11) or for the current selection (E12),
> the bar is shown in state `Init` (E12: `AltInit` when interruptible), its label set, and its
> meter position set each frame to *p* = clamp(elapsed ÷ total, 0, 1) with elapsed measured from
> the event's arrival and total = `duration_ms` (plus every `added_ms` of `CastDelayed` so far:
> the bar moves back). All four kinds fill from 0 to 1 (OPEN: whether channels drain; owner check
> OC9). `smooth_movement` is not used for this (the position is set, `ui-widgets.md` §4.6). The
> floating mini bar of E3 uses the same *p* for the selection's cast.

> **TR27.** The mounted and music variants are not used in P2: every cast uses the plain bar.

<a id="s7-4"></a>
### 7.4 End, interrupt and cancel

> **TR28.** On `CastEnded { outcome }`: `Completed` → state `Succeeded`, *p* = 1; `Interrupted`
> → `Interrupted`, *p* unchanged; `Cancelled` → `Abandoned`, *p* unchanged. The state's media
> then run (§7.1); on `progress_stop` the client hides the bar (auto-fade 0.5 s,
> `ui-behaviour.md` §11.1). A new `CastStarted` for the same entity replaces a bar still ending.
> E12 is hidden when the selection changes. The player can cancel an active cast with Escape
> (TR5) or by clicking the bar; both send `CancelCast`. A cast of kind `Craft` cannot be
> cancelled by Escape (OPEN; owner check OC9).

---

<a id="s8"></a>
## 8. Effect bars (E8–E10)

<a id="s8-1"></a>
### 8.1 Lists and icons (Confirmed data, CU14)

`PlayerEffectDisplay` (P4) and `OpponentEffectDisplay` (P5) are `EffectDisplayList` elements:
vertical list boxes (`horizontal` false) with `max_rows` 2 and `max_columns` 1, so they hold two
**rows** one under the other; `UIComponent_EffectDisplay_MaxIconColumns` 8; the player's has
`UIComponent_EffectDisplay_PlayerOnly` true. At creation (BIN e) each row is created from
`PlayerEffectDisplayChildTemplate` of `effects` (an `AutoResizeListBox`, horizontal, one row,
resizes to zero when empty), with `UICore_ListBox_max_columns` set to the list's
`MaxIconColumns` and `max_rows` to its `MaxIconRows` when those are positive.

An icon is created from `TimedEffectTemplate` (36 × 36) and resized to *e* × *e*, where *e* is
the list's `EffectDisplayList_EffectSize` (default **20** when absent, as in P4 and P5). Its
children: `TimedEffectBorderField` (36 × 36, z −1, stretched; states by category, below) and
`TimedEffectImageField` (at (2, 2) 32 × 32, z −2, stretched; states `Pulse` and `Pulse_Reset`).
The image field gets the effect's icon (the `Effect_Icon` DID, draw mode 2) and the tooltip
properties `UI_Examination_Tooltip_DID` (the effect's tooltip layout),
`UI_Examination_Tooltip_EffectID` and `UI_Examination_Tooltip_EffectInstanceID`
(`skill-tooltips.md`). The icon gets `UI_Effect_EffectID`, `UI_Effect_EffectDuration` (the
**start** time, or the largest float for an effect without a duration) and
`UI_Effect_EffectTimeout` (start + duration, or the largest float), and `UI_Effect_CombatOnly`
when the list tracks it. There is **no** timer text and **no** stack-count text in the template.

**Border (BIN e, BC).** For a harmful effect with a category, the border field takes the state
that a class-library function (ID 0x010B0021) returns for the category; for others the border is
hidden. The states are the labels of `Effect_Resist_Category`'s mapper 0x23000064: `FeignDeath`
(no image), `Disease` 0x410F25C3, `Physical` 0x410F25C4, `Wound` 0x410F25C4, `Cry` 0x410F25C6,
`Song` 0x410F25C3, `Fear` 0x410F25C5, `Poison` 0x410F25C6, `Magic` 0x410F25C5, `Elemental` (no
image), `Corruption` 0x410F25C2.

> **TR29 (border).** The border state is the label of the lowest set bit of the effect record's
> `Effect_Resist_Category` (bit *b* ↔ key *b* + 1, `property-system.md` §7.3) when that label is
> one of the eleven states and `Effect_Harmful` is true; otherwise the border is hidden. Literal
> refs have no border (fallback §13.5).

<a id="s8-2"></a>
### 8.2 Rows, order and removal (BIN e)

- **Rows.** With the list's split option off, every icon goes to row 1. With it on, harmful and
  beneficial effects go to different rows; a second option swaps which is first. (Both options
  are settings of the list's owner; not traced further.)
- **Order** (revision 3 re-read, BIN j, CU30). Each icon carries its effect ID
  (`UI_Effect_EffectID`; the same value as the image field's `UI_Examination_Tooltip_EffectID`,
  the per-application runtime ID of `skill-tooltips.md` §8.2), its start
  (`UI_Effect_EffectDuration`; the largest float for an icon without a duration) and its
  combat-only flag. A new icon is placed in its row thus:
  1. An icon without a duration that is not combat-only goes to position 0 (the list method's
     index argument 0, Probable).
  2. Otherwise the row is scanned from its last icon towards its first, passing over icons that
     carry no effect ID. The scan stops at the first icon that has the new icon's effect ID (the
     new icon then goes to position 0) or that passes the placement test of step 3 or 4 (the new
     icon then goes right after it). A scan that reaches the front without stopping puts the new
     icon at position 0.
  3. A new icon that is not combat-only (so it is timed) stops at an icon whose start is later
     than its own (an untimed icon counts as the latest).
  4. A combat-only new icon passes over every timed icon; it stops at an untimed icon that is
     not combat-only; at an untimed combat-only icon it stops when it is itself timed and passes
     over it when it is itself untimed.

  So the untimed, non-combat-only icons come first (most recent first) and an untimed
  combat-only icon goes right after the last of them. With starts in arrival order (TR30) every
  new timed icon, combat-only or not, goes right after the last untimed icon, so the timed
  icons follow by start, **most recent first**.
- **Pulse.** On each update, an icon whose timeout is less than **15 s** away starts pulsing: its
  image field enters `Pulse` (opacity 1 → 0.3 over 1 s, then 0.3 → 1 over 1 s, then `Pulse_Reset`,
  which restarts `Pulse`). A combat-only icon pulses while the local player's `Combat_Violent` is
  false and is steady (opacity 1) while it is true.
- **Removal.** Icons are removed when the effect is removed (session report); the client does not
  remove an icon when its timeout passes.

> **TR30 (feed).** `EffectApplied { entity, effect, what, duration_ms, stacks, source }` adds an
> icon to the effect list of every frame showing `entity` (E8 for the player, E9 when `entity` is
> the selection; E9 is rebuilt on a selection change by placing the client's kept effects in
> arrival order). Start = the event's arrival; timeout = start + `duration_ms`, or none. The
> icon is placed by the Order rule above, with the feed's instance `effect` as its effect ID. An
> `EffectApplied` for an instance that already has an icon in that list replaces it: the old
> icon is removed first. No row therefore holds two icons with the same effect ID, and the
> same-ID step of the Order rule never applies to feed effects. A literal ref is not
> combat-only. Rows: the split option is on;
> beneficial effects (`Effect_Harmful` false) in row 1, harmful in row 2; a literal ref counts
> as harmful when the event's `source` is an entity other than `entity`, beneficial otherwise
> (owner check OC10). `EffectRemoved { entity, effect }` removes the instance's icon. A row shows
> at most `MaxIconColumns` (8) icons; further icons are kept but not shown until earlier ones are
> removed (OPEN: the original's overflow). The pulse rule above applies; combat-only is
> `Effect_Duration_CombatOnly` of the record.

> **TR30a (row arrangement; answers the implementer OPEN of 2026-10-10).** Inside an
> `EffectDisplayList` the two rows are placed one under the other, *e* pixels apart, from the
> list's top-left corner, and the icons of a row side by side without spacing. Because an empty
> row resizes to zero (§8.1), the second row moves to the top while the first row shows no icon
> (install look only; the fallback rows keep §13.1's places). (Tessera's choice: the list's own
> layout of its rows was not traced.)

> **Answered (revision 3, 2026-10-14, BIN j; was OPEN, implementer, 2026-10-10):** the
> original does put a new icon at position 0 when the backward scan meets an icon with the same
> effect ID first, timed or not, so CUI7 holds in the original only while no icon with the same
> effect ID is in the row. The ID is the per-application runtime ID; under TR30 a repeated
> instance replaces its icon, so the case never arises from the feed and CUI7 (as restated in
> §15) holds. The original also places combat-only icons by steps 2 and 4 of the Order rule;
> Tessera follows them, so an untimed combat-only icon goes right after the last untimed,
> non-combat-only icon, not to position 0.

> **TR31 (stacks and timers).** The icon shows neither stacks nor a timer, as in the original's
> template. The effect tooltip is owned by `skill-tooltips.md` §12.2: it shows the remaining
> time (counting down while shown) and has **no** stack-count row, at any `stacks` value, as in
> the original. `EffectApplied.stacks` is therefore not shown anywhere in P2 (the client keeps
> it with the effect instance). (Owner checks OC10 for the icons and `skill-tooltips.md` OS5
> for the tooltip.)

---

<a id="s9"></a>
## 9. Quickslot bar beyond `game-panels.md` §10

<a id="s9-1"></a>
### 9.1 Slots and content

`game-panels.md` §10 builds 12 buttons per bar; the main bar (P3) has shortcut offset 0, P14's
bar 12. Skill content uses `SkillQuickslotDisplayTemplate` (E16): `RealQuickslotSkillDisplay`
(widget type `SkillDisplay`, 35 × 35, `UI_SkillDisplay_IconSize` `Small`, report usability and
availability true, `UI_SkillDisplay_MinResetTimeForAvailabilityAlert` 2.0) with
`QuickslotSkillIcon` (a Button at (3, 3) 32 × 32, rollover, states `normal`, `normal_rollover`,
`highlight`, `highlight_rollover`).

> **TR32 (feed slots).** The feed's slot *N* (1 … 1,024) is shortcut index *N* − 1. P3 shows
> indices 0–11 and P14 indices 12–23; other indices are kept but not shown. `QuickslotChanged {
> slot, content }` rebuilds that slot: `Skill(ref)` → the skill display with the skill's icon
> (`skill-tooltips.md`; a literal ref takes the fallback icon of §13.7); `Item(id)` → the item
> display of `character-panels.md` §5 for the item last reported with that id; none → empty
> (`game-panels.md` §10.2).

<a id="s9-2"></a>
### 9.2 Activation (Confirmed map, CU18; rules TR33)

`UI_ShortcutActivationInputMap` (0x7800000D) lists 72 `UI_Shortcut_InputMap_Entry` structs:
`UI_Shortcut_InputMap_ShortcutIndex` *i* → `UI_Shortcut_InputMap_InputAction` `QUICKSLOT_`(*i*+1),
for *i* = 0 … 71. The default bindings are in `keymap.md` K47 (`QUICKSLOT_1` … `_12` on 1 … 0,
Minus, Equal; 13–24 with Ctrl, 25–36 with Alt, 37–48 with Shift; 49–72 unbound).

> **TR33 (activation).** A click on a slot's button (`select_left` released over it, the UI's
> button click) and the press of the action `quickslot_`*n* (`keymap.md` §8.2; *n* = *i* + 1)
> **use** the slot with shortcut index *i*: `Skill(ref)` sends `UseSkill { what: ref, target:
> the current selection }`; `Item(id)` sends `UseItem { item: id, target: the selection }`; an
> empty slot sends nothing. Repeats are ignored (Impulse). A key press uses the slot even when its
> bar is not shown.

> **TR34 (labels).** `QuickslotLabelText` (`game-panels.md` §10.1 OPEN) shows the first default
> stroke bound to `quickslot_`*n* in Tessera's short form: the key's name for digits and Minus
> ("-") and Equal ("="), prefixed by "C" for Ctrl, "A" for Alt, "S" for Shift (so "C1", "S=");
> nothing when unbound.

<a id="s9-3"></a>
### 9.3 Cooldown display (BIN f, CU16)

`QuickslotSkillResetTimer` (a Meter at (3, 3) 32 × 32 inside the skill display) is a **frame
meter** (`ui-widgets.md` §4.4) with 60 frames, 0x41007E35 … 0x41007E70 in order, drawn with
draw mode Multiply over the icon. The skill display finds it through `UI_SkillDisplay_ResetTimerMeter`.

**Start** (with the end time *T* and the total *D* of the cooldown): when the meter exists, *D* > 0
and the display shows a skill, the client remembers *T* and (if unset) *D*, shows the meter, and
sets on it `UICore_Meter_smooth_movement` true, `UICore_Meter_smooth_movement_duration` = *R* =
*T* − now, `UICore_Meter_position` = clamp(*R* ÷ *D*, 0, 1), and `UICore_Meter_goal_position` 0.
The meter thus runs from the remaining fraction down to 0 in the remaining time; the frame shown
is trunc(59 × *p* + 0.5) (`ui-widgets.md` §4.4).

**End.** When the meter reports reaching its goal (`Meter_goal_position_reached`, 0x36): if the
remembered *D* is at least `MinResetTimeForAvailabilityAlert` (2.0 s), the availability overlay
plays its state (`skillavailable`: a 10-frame animation of 1 s, then back to no state); then the
meter is hidden and *T*, *D* reset. The same reset runs when the display is cleared.

There is no numeric countdown on the slot.

> **TR35 (feed).** `CooldownStarted { what, duration_ms }` starts the cooldown on every visible
> slot whose content is `Skill(what)` (equal refs), with *D* = `duration_ms` ÷ 1000 and *T* = the
> event's arrival + *D*. `CooldownEnded { what }` sets the goal reached at once (position 0, the
> end steps run). A slot that receives a skill while that skill's cooldown runs starts it with the
> remaining time.

<a id="s9-4"></a>
### 9.4 Slot states (Confirmed data, CU17)

The skill display names its overlays in `UI_SkillDisplay_ElementList`; each is a 32 × 32 field at
(3, 3), hidden by the layout:

| Overlay | State shown | Media of the state |
|---|---|---|
| `QuickslotSkillPlayingOverlay` | `playing` (`playing_Simple`) | 12-frame animation 0.5 s (9 frames 1.1 s) |
| `QuickslotSkillToggleOverlay` | `playing` (`playing_Simple`) | 12 frames 1 s (8 frames 1.1 s) |
| `QuickslotSkillComboMatchOverlay` | `skillcombomatch` | 9 frames 1 s |
| `QuickslotSkillOnDeckOverlay` | `skillondeck` (`_Simple`) | 12 frames 2 s (8 frames 1 s) |
| `QuickslotSkillErrorOverlay` | `skillerror` | 10 frames 1 s, then no state |
| `QuickslotSkillAvailableOverlay` | `skillavailable` | 10 frames 1 s, then no state (§9.3) |
| `QuickslotNewlyShortcutSkillOverlay` | `newlyshortcut` (`_Simple`) | 12 frames 1.1 s and an image |
| `QuickslotSkillRangeOverlay` (12 × 12 at (23, 4)) | `inrange` (no image), `outofrange` (0x41007F48), `mountedcombatoutofrange` | image |
| `QuickslotSkillGhostedOverlay` | shown, not a state | image 0x41007E33 and a coloured layer 0xBF000000 |
| `QuickslotSkillRecallOverlay` | shown | — |

> **TR36 (feed).** While the player's active cast has `what` equal to a slot's skill, that slot
> shows `playing` (non-`_Simple` variants throughout); `CastEnded` clears it. A `UseSkill` the
> session refuses with an error (the command result, `scenario-feed.md` §9.5) plays `skillerror`
> on the slot used. The range, ghosted, on-deck, combo, toggle and newly-shortcut overlays are not
> driven in P2 (no events; OPEN for later milestones). The cooldown follows TR35.

---

<a id="s10"></a>
## 10. Combat state and defeat

<a id="s10-1"></a>
### 10.1 Combat state (BIN h, CU19)

On a change of the local player's `Combat_Violent`, the client sets P3's root (`ToolbarField`)
to `toolbar_combat` (true) or `toolbar_noncombat` (false), and `ViolentModeIndicator` (P3, at
(246, 128) 63 × 63) to `combat_violent` (a 32-frame animation of 1.75 s) or `combat_normal` (no
image). `Combat_Violent` is also what the effect pulse (§8.2) and the selection indicator (via
`Skill_DefaultSkillEnabled`, §3.5) consult.

> **TR37.** `CombatState { entity: player, in_combat }` stands for `Combat_Violent`. For other
> entities `CombatState` is kept (TR14 does not use it) and shown nowhere in P2.

<a id="s10-2"></a>
### 10.2 Defeat panel (Confirmed data, CU20)

`Awaken_DialogMessageField` (`Dialog` 0x2200007C; widget type `UIScaler`, group `Menus` (55),
512 × 220, activatable) holds `DialogMessageBoxField` (based on `Box_Silver_BlueGradient`) with:
`DialogMessageStatementText` (493 × 122 at (10, 8), font colour 0xFFD2FF00), the OK button
`DialogMessageOkButton` (at (321, 163) 170 × 36, text token 0x0F10D163, activated when shown), the
revive button `DialogMessageReviveButton` (at (137, 163) 170 × 36, text token 0x07271745, sends
even when ghosted), the paid-revive and store buttons (`SelfRevive_PayForRevive`,
`SelfRevive_PaidCategory_BuyNowButton`, store category `WS_MithrilCoin`), a divider and a
paper-item balance (`UI_PaperItem_PaperItemService` `Revival`).
`UI_AwakenDialog_ReviveButtonGrayTimeout` is a dictionary property (no stored value found).

`AwakenTimer` (0x22000887; widget type `AwakenTimer`, 160 × 56; placed by `GP_AwakenTimer` of
`gameplayactivatables` at (432, 135), contextual root) holds a frame, a label (token 0x06404FBC
of table 0x25033EC5) and a `Timer` element (`UIComponent_Timer_*` settings).

> **OPEN:** the statement text the original shows, what the OK button does (return to the rally
> point of `interaction.md` §8 is the reading), when the revive button is ghosted, and when the
> awaken timer is shown. Game code and server rules. Owner check OC11.

> **TR38 (defeat panel, presentation only).** On `Defeated { entity: player }` the client opens
> E18 with the statement "Your character was defeated." (Tessera's own text), the OK button labelled
> by its token, and the revive button labelled by its token and **ghosted**; the paid and store
> buttons, the divider, the paper-item parts and E19 stay hidden (the parts by label: TR38a). The buttons send nothing (the feed
> has no release or revive command; OPEN for `scenario-feed.md`); OK closes the panel. On
> `Revived { entity: player }` the panel closes.

**Box children (Confirmed, CU29; answers the implementer OPEN of 2026-10-11).**
`DialogMessageBoxField` (based on `Box_Silver_BlueGradient` of 0x22000001) has these own
children, in read order: `DialogMessageBoxField_BG` (500 × 212 at (6, 4), hidden by the
layout); `DialogMessageOuterTextField` (493 × 122 at (10, 8)), which holds the statement text;
the OK button; `DialogMessageReviveButton`; `SelfRevive_PayForRevive` (the paid-revive button,
with `SelfRevive_PayForRevive_MithrilIcon`); `SelfRevive_PaidCategory_BuyNowButton` (hidden by
the layout); `DialogMessage_Divider` (400 × 26 at (57, 132)); `SelfRevive_PaperItemTotal_NameLabel`
(hidden by the layout); and `SelfRevive_PaperItemBalance` (based on `PaperItemServiceBalance_Base`
of 0x22000884; it carries `UI_PaperItem_PaperItemService` `Revival`). The box art is the ten
children inherited from `Box_Silver_BlueGradient` (its corner, edge, background and centre-overlay
fields, all labelled `Base_Box_Silver_…`).

> **TR38a (defeat panel parts).** TR38 hides exactly `SelfRevive_PayForRevive`,
> `SelfRevive_PaidCategory_BuyNowButton`, `DialogMessage_Divider`,
> `SelfRevive_PaperItemTotal_NameLabel` and `SelfRevive_PaperItemBalance` (found by label), and
> every other element below the panel that carries `UI_PaperItem_PaperItemService`. Every other
> element, the inherited box art and `DialogMessageBoxField_BG` included, keeps the layout's
> visibility. (Revision 3 replaces the implementation's position-based reading, which treated the
> box's children before the statement's holder as its art; the result is the same only if the
> inherited art comes before that holder in child order.)

<a id="s10-3"></a>
### 10.3 Defeated entities

> **TR39.** After `Defeated { entity }` and until `Revived { entity }`: the entity is "defeated"
> for TR4, TR14 and TR15; its effect icons stay until removed; its cast bar ends as
> `Interrupted` if one is active; frames showing it keep showing it. A defeated selection stays
> selected. The player's own defeat also hides E11 and opens E18 (TR38).

---

<a id="s11"></a>
## 11. Events and commands

Every Combat-group variant of `scenario-feed.md` §9.2 and §9.4, and what it drives. "Kept" means
the client stores the value for later use.

| Variant | Drives |
|---|---|
| `TargetChanged { entity: player, target }` | The selection (§3): E1, E3, E4, E6 (shown/hidden, TR17; refilled from kept vitals, level, effects and cast), E7, E12; TR4 *last* memory |
| `TargetChanged { entity: other, target }` | Kept (TR4 *assist*); E7 when `other` is the selection |
| `VitalChanged { entity, vital, current, max }` | Kept per entity and vital; health and power meters of E5 (player), E6 (selection), E3 mini vitals, E7; wrath (TR10) and pip displays (TR11, E20); TR12 has-vitals |
| `CombatState { entity, in_combat }` | Player: E17 and P3 root state (TR37), E1 state (TR7), effect pulse (§8.2); others: kept |
| `QuickslotChanged { slot, content }` | E15/E16 slot rebuild (TR32); running cooldowns and casts reapplied (TR35, TR36) |
| `CastStarted { entity, cast, what, target, duration_ms }` | Player: E11 (TR25–TR27), slot `playing` (TR36); selection: E12 and E3 mini induction; others: kept for when selected |
| `CastDelayed { entity, added_ms }` | Total of the active bar (TR26) |
| `CastEnded { entity, outcome }` | End states (TR28), slot `playing` cleared |
| `CooldownStarted { what, duration_ms }` | TR35 |
| `CooldownEnded { what }` | TR35 |
| `EffectApplied { … }` | Kept; icons in E8/E9 (TR30, TR29) |
| `EffectRemoved { entity, effect }` | Icon removal (TR30) |
| `VitalDelta { … }` | E13 (TR19–TR21); TR4 *last attacker* |
| `Defeated { entity, by }` | TR39; player: E18 (TR38) |
| `Revived { entity }` | TR39; player: E18 closed |
| `LevelChanged` (Character group) | Levels for E6 (TR16) and TR15 |
| `EntityDespawned` | A despawned selection is cleared locally (no command sent; the frames hide); kept values are dropped with the entity |
| Command `SelectTarget { target }` | Sent by TR3, TR4, TR5 |
| Command `UseSkill { what, target }` | Sent by TR33 |
| Command `CancelCast` | Sent by TR5 and TR28 |
| Command `SetQuickslot { slot, content }` | Sent when the player drops a skill or item on a slot or drags one off (`ui-behaviour.md` §7; drag-out sends `content: None`); the slot changes only on the answering `QuickslotChanged` |

> **OPEN (feed, for `scenario-feed.md`; not changed here):** (1) no temporary vitals or dread
> clamp (TR10); (2) no stack update of an existing effect instance (a new `EffectApplied` is
> needed); (3) no release or revive command for the defeat panel (TR38); (4) no command result
> distinguishing a refused `UseSkill` (TR36 needs one; until then no error is shown); (5) no
> entity title or guild for E3's second line.

> **TR45 (slot drags; answers the implementer OPEN of 2026-10-11).** A drop on bar slot *j*
> sends `SetQuickslot { slot: j, content }` with the source's content (a bar slot's, or a plugin
> shortcut's, `lua-plugin-api.md` §15.2) and leaves the source slot unchanged (a copy). Only a
> drop over the world (no UI element under the pointer) counts as "dragged off" and sends
> `content: None` for the source slot; a drop on any other UI sends nothing. (Tessera's choice:
> the original's move, copy or swap rule was not traced; owner check OC12.)

---

<a id="s12"></a>
## 12. Layers and order

> **TR40.** Screen-space floating elements (E1, E3, E13) are drawn after the 3D scene and before
> the UI panels, in this order: name blocks, then floating texts (older first), then the
> selection indicator. Panels follow the UI's own order (`ui-layout.md` §9.3).

---

<a id="s13"></a>
## 13. Fallback looks (no install)

Without an install (`--no-data`, `scenario-feed.md` §8.7) no layout, font, icon or string exists.
For the combat UI to render deterministically, Tessera draws each element with the fallback look
below. All geometry is in pixels of a 1024 × 768 reference, scaled uniformly to the screen height
(width scaled by the same factor, the whole HUD centred horizontally); colours are ARGB; text uses
the bundled UI font (decision 0022) at the given pixel size; "box" is a filled rectangle with a 1 px
border. These looks are Tessera's own design (TR41) and are used only when the install's element
is missing.

<a id="s13-1"></a>
### 13.1 Frames (E5, E6, E7)

| Part | Player frame E5 | Target frame E6 |
|---|---|---|
| Panel box | (8, 8) 240 × 64, fill 0xC0202020, border 0xFF808080 | (264, 8) 240 × 64, same |
| Level box | (12, 12) 28 × 28, border 0xFFC0A060, level text 14 px centred, 0xFFFFFFFF | (472, 12) 28 × 28 |
| Name | (46, 12), 14 px, 0xFFFFFFFF, cut with "..." at 190 px | (270, 12), 14 px, colour by §13.2 |
| Health bar | (46, 32) 190 × 10, back 0xFF401010, fill 0xFF30C030, fills from the left | (270, 32) 190 × 10, fill by §13.2, fills from the right |
| Power bar | (46, 46) 190 × 8, back 0xFF101040, fill 0xFF3060E0 | (270, 46) 190 × 8 |
| Resource bar | (46, 58) 190 × 4, back 0xFF303010, fill 0xFFE0C030 | — |
| Non-vital target | — | panel box only, name centred at (384, 32), 16 px |
| Effect rows | row 1 at (46, 76), row 2 at (46, 98), icons 20 × 20 with 2 px gaps | row 1 at (270, 76), row 2 at (270, 98) |

E7: (264, 124) 120 × 32 box, name 12 px at (268, 126), health bar (268, 144) 112 × 6.

> **TR42 (E7 without vitals; answers the implementer OPEN of 2026-10-10).** E7's fallback for a
> target without vitals is the same box and name, without the health bar.

<a id="s13-2"></a>
### 13.2 Colours by state

Name colour (TR15 states): consideration ranks Trivial 0xFF8C8C8C, InsanelyEasy 0xFF20C020,
VeryEasy 0xFF20D0D0, Easy 0xFF3090F0, Even 0xFFF0F0F0, Tough 0xFFE0D020, VeryTough 0xFFF08020,
InsanelyTough 0xFFE02020, Impossible 0xFFB030C0; NPC 0xFFE0E040; player 0xFFC0C060; object and
corpse 0xFFF0F0F0. Health fill (TR14): red 0xFFD03030, green 0xFF30C030, grey 0xFF808080,
yellow 0xFFD0C030.

<a id="s13-3"></a>
### 13.3 Cast bars (E11, E12)

E11: box (352, 640) 320 × 22, fill 0xC0202020; meter (356, 644) 312 × 14, back 0xFF303030, fill
0xFFE0B040 (interruptible E12: 0xFFE0C040; not interruptible: 0xFF909090); label 12 px centred,
0xFFFFFFFF. End states: `Succeeded` fill 0xFF40C040, `Interrupted` and `Abandoned` fill 0xFFC04040,
each held 1 s, then fading out over 0.5 s. E12: box (264, 168) 240 × 18, meter inset 3 px.

<a id="s13-4"></a>
### 13.4 Class resource (E20)

A literal resource ref (or one without a pip record): the player frame's resource bar of §13.1.

> **TR43 (the bar beside an install-look frame; answers the exit verifier's OPEN of
> 2026-10-11).** With an install, a resource without a pip record still draws the §13.1
> resource bar, at its §13.1 place on the fallback root over the install panels (TR41 applies
> element by element: the install has no element for it).

<a id="s13-5"></a>
### 13.5 Effect icons (E10)

20 × 20 box, fill 0xFF304030 (beneficial) or 0xFF403030 (harmful, by the literal rule of TR30);
the first character of the literal (or "?") in 12 px, centred, 0xFFFFFFFF. Pulse as
§8.2 (opacity).

<a id="s13-6"></a>
### 13.6 Floating text (E13)

Font 18 px with a 1 px black outline; colours by TR20; critical text 0xFFFF2020 with a white
outline. Motion, life, scale, criticals and stacking exactly as §6.3–§6.5 with the built-in
defaults of §6.2 taken from the column "0x78000004" (so the look is the same with and without an
install), *H* = the render height. Avoidance words (TR19 fallback): "Miss", "Evade", "Parry",
"Block", "Resist", "Immune" (Tessera's own words).

**Anchor without a world.** In a `--no-data` run there is no 3D scene. The client projects
entities with a fixed synthetic camera: at the player's position + (0, −6, 3) m (6 m south, 3 m
up) looking north and 15° down, vertical field of view 60°, the player's own anchor 2.0 m above
its position, every other entity's 2.0 m above its position (`scenario-feed.md` §5.4 positions).
Entities behind this camera have no floating elements.

<a id="s13-7"></a>
### 13.7 Quickslots, selection, combat state, defeat

- Quickslot bar (E15): 12 boxes 35 × 35 at (302 + 35 *i*, 728), border 0xFF808080; skill content:
  fill 0xFF404060 with the literal's first two characters (12 px); key label (TR34) 10 px at the
  slot's bottom left; cooldown: a dark layer 0xA0000000 covering a clockwise sector from 12
  o'clock of angle 360° × *f* ÷ 59, where *f* is the frame index of §9.3;
  `playing`: a 2 px border 0xFFFFE060; `skillerror`: a 2 px border 0xFFFF4040 for 1 s;
  `skillavailable`: a 2 px border 0xFFFFFFFF for 1 s.
- Selection indicator (E1): a filled downward triangle 16 × 12, 0xFFFFE060 (peace) or 0xFFFF6040
  (combat), centred 12 px above the name block's anchor; off screen, clamped as §3.5 step 3 and
  pointing toward the anchor. E4 is not drawn (no world).
- Name block (E3): name 14 px in the TR15 colour, mini health bar 60 × 5 under it, mini cast bar
  60 × 4 under that.
- Combat state (E17): a 24 × 24 box at (500, 700), border 0xFFFF4040 while in combat, absent
  otherwise.
- Defeat panel (E18): box (256, 274) 512 × 220, fill 0xE0101828; statement 16 px at (272, 300);
  buttons 170 × 36 at (393, 437) "Revive" (ghosted: text 0xFF808080) and (577, 437) "OK".

> **TR44 (fallback gaps; answers the implementer OPEN of 2026-10-10).** The fallback looks
> also follow these rules: (1) "box" border colours not given (cast bars, effect icons, E7,
> defeat panel and its buttons) take the panels' 0xFF808080, and the buttons a fill of
> 0xFF303848; (2) `Tapped_ColorName` takes the trivial grey 0xFF8C8C8C (§13.2 gives none);
> (3) the selection indicator's triangle is centred 12 px above the name block's **top** edge
> (§13.7 says "above the name block's anchor", but TR8 puts the block's bottom edge at that
> anchor, so the two would overlap); the name block is 120 × 29 (name line 16 px
> high, the 60 × 5 health bar at y 18, the 60 × 4 cast bar at y 25), centred on the anchor;
> (4) the level text of a hidden level (skull or not) is empty; (5) the text sizes need the
> client's text shaper to draw the bundled font at the given size: the fallback text elements
> carry the size in a Tessera property (an engineering detail of `tessera-ui`).

> **TR41.** The fallback looks above apply element by element whenever the install's element (or
> font, image, string) is missing; a run with an install never mixes them for the same element.
> Golden images of §16.2 are taken with `--no-data` only.

---

<a id="s14"></a>
## 14. Every element on screen (ROADMAP P2 exit criterion 1)

A combat scenario run "with every element on screen" means: at the checkpoint named in the
second column, the element of the first column is shown (visible, opacity > 0, non-empty box on
screen) in the screenshot. Element numbers are those of §2.

| Element | Shown at (the synthetic scenario of §17.1) |
|---|---|
| E5 player frame with name, level, health, power, resource | always after world entry |
| E6 target frame (vital field), name colour, level, health | after the selection of the creature |
| E6 non-vital field | after the selection of the object |
| E7 selection's target | while the creature targets the player |
| E1 selection indicator, E3 name block | while a selection exists |
| E8 player effect row (beneficial) | while the player's effect lasts |
| E9 target effect row (harmful) | while the creature's effect lasts |
| E10 pulsing icon | an effect within 15 s of its end |
| E11 player cast bar (fill, then end state) | during and 1 s after the player's cast |
| E12 target cast bar | during the creature's cast |
| E13 floating damage, critical, avoidance, heal | 0.1–1.0 s after each `VitalDelta` |
| E15/E16 skill slot with cooldown | during the cooldown |
| E17 combat state | while the player is in combat |
| E18 defeat panel | after the player's defeat |
| E20 class resource | after the resource report |

<a id="s15"></a>
## 15. Invariants

| # | Invariant |
|---|---|
| CUI1 | At most one selection; E6, E7, E12, E3, E1 always refer to it (or are hidden). |
| CUI2 | Every meter position shown is in [0, 1]; a vital with max 0 shows 0. |
| CUI3 | A per-entity floating-text list never holds more than 51 pending texts; at most six critical slots are taken. |
| CUI4 | A floating text is removed at *t* ≥ 1.05 and never shown at *t* ≥ 1. |
| CUI5 | The cooldown meter's frame index is trunc(59*p* + 0.5) ∈ [0, 59] and never increases during one cooldown. |
| CUI6 | An effect icon exists exactly while its instance is applied (between `EffectApplied` and `EffectRemoved`, or the entity's despawn). |
| CUI7 | Within a row: untimed icons that are not combat-only come first, then untimed combat-only icons, then timed icons; timed icons are in non-increasing order of start time; no two icons have the same effect ID (§8.2, TR30). |
| CUI8 | No command is sent by any display update; commands are sent only by TR3, TR4, TR5, TR28, TR33 and §11 `SetQuickslot`. |
| CUI9 | Running the same scenario twice headless gives the same element states at every tick (no wall-clock reads: elapsed times use the feed's virtual clock in headless runs). |
| CUI10 | With `--no-data`, no element draws an install resource; with an install, no element uses a fallback look unless its resource is missing. |

---

<a id="s16"></a>
## 16. Tests

<a id="s16-1"></a>
### 16.1 Real-data test (`apps/client/tests/combat_ui_real_data.rs`, with `TESSERA_GAME_DIR`)

1. **Elements resolve.** For every element of §2 (and every child named in §4.1, §5.1, §7.1,
   §8.1, §9.3, §9.4, §10.2), find its layout by `UILAYOUT` label (or its base layout through an
   element based on it) and the element by label; build it per `ui-layout.md` §9.2. A missing one
   is a failure listed by label.
2. **Data rows.** The two settings sets of §6.2 are found through `UNIQUEDB` and hold the 16
   properties; `UI_ShortcutActivationInputMap` has 72 entries mapping *i* to `QUICKSLOT_`(*i*+1);
   `ColorNameText` has the nine consideration states; script table `SharedUISelection_ScriptTable`
   holds `ui_selected_fx`; every token of §6.6 and §10.2 resolves in its table.
3. **Scenario.** Run the scenario of §17.1 headless with the install and `--screenshot` at each
   §14 checkpoint; at each, assert from the UI tree (not pixels) that the §14 elements are shown.
   Real-data screenshots and goldens stay local (ARCHITECTURE §23.3).
4. Report counts: elements resolved, data rows checked, checkpoints passed.

> **TR46 (real-data screenshots; answers the implementer OPEN of 2026-10-11).** The §16.1
> screenshots show the install's panels without a 3D scene: the floating elements use the
> synthetic camera of §13.6 and the portraits (`PlayerPortrait`, `OpponentPortrait`) stay
> empty. Quickslots holding items (TR32) show empty buttons in the install look until
> `game-panels.md` §10.3 names an item content template for quickslots (OPEN there).

<a id="s16-2"></a>
### 16.2 Synthetic goldens (`apps/client/tests/combat_ui_golden.rs`, committed)

Run the scenario of §17.1 with `--no-data`, take screenshots at the §14 checkpoints, compare with
committed goldens per ARCHITECTURE §23.3 (software adapter, per-pixel tolerance). The same run
writes the canonical log (`scenario-feed.md` §10), which must equal the committed log.

---

<a id="s17"></a>
## 17. Worked synthetic examples

<a id="s17-1"></a>
### 17.1 Reference scenario (synthetic; proposed `apps/client/tests/scenarios/combat-ui.scenario`)

```
tessera-scenario 1
title "Combat UI"
tick 100ms
player "Tamsin"
actor $boar kind=creature name="Grey boar" pos=0,8,0 level=7
actor $post kind=object name="Signpost" pos=3,4,0
at 0s spawn $boar
+0s spawn $post
+0s vital player health 900/900
+0s vital player power 400/400
+0s vital player resource="Focus" 2/5
+0s vital $boar health 1200/1200
+0s quickslot 1 "Jab"
+0s quickslot 2 "Mend"
at 1s target player $post
at 2s target player $boar
+0s combat player on
+0s target $boar player
+0.2s cast player "Jab" 800ms target=$boar
+0.8s damage player $boar 210 result=critical with="Jab" left=990
+0s cooldown "Jab" 3s
+0.2s effect player "Second wind" for=20s
+0s effect $boar "Bleeding" for=4s from=player
+0.3s damage $boar player 75 left=825
+0.3s damage player $boar 0 result=parry
+0.3s cast $boar "Gore" 1.5s target=player
+0.5s heal player player 60 with="Mend" left=885
at 8s damage $boar player 885 left=0
+0s defeat player by=$boar
at 10s revive player
+0s vital player health 900/900
+0s combat player off
```

Checkpoints (virtual time, s): 0.5 (E5, E20), 1.2 (E6 non-vital field, E1, E3), 2.1 (E6 vital,
E7, E17), 2.5 (E11 filling, *p* = 0.375), 3.1 (E11 `Succeeded`; E13 critical over the boar;
E16 cooldown), 3.4 (E8, E9, E10 pulsing: the boar's 4 s effect), 3.8 (E13 on the player), 4.0
(E13 avoidance word over the boar), 4.3 (E12), 4.7 (E13 heal), 8.1 (E18). Times follow
`scenario-feed.md` §6.2 and §7.4 with a 100 ms tick: the player's cast runs 2.2–3.0 s, the boar's
4.1–5.6 s; floating texts live 2 s from their event.

<a id="s17-2"></a>
### 17.2 Floating text timing

A normal text, *L* = 2 s, *s*₁ = 1.5, base rise speed at *H* = 768: 768 × 0.125 ÷ 1.5 = 64 px/s,
one text in the list (no crowding), the entity at 10 m (*n* = 4, *f* = 30: *u* = 6/26 ≈ 0.2308,
*q* ≈ 0.5917, *σ* ≈ 0.4083 × 0.5 + 0.5917 × 1.2 ≈ 0.9142). At 0.5 s: *t* = 0.25, *k* = 1 +
0.5 × 0.5 = 1.25, rise 32 px (before *σ*), α = 1. At 1.5 s: *t* = 0.75, *k* = 1.5, rise 96 px,
α = 0.5. At 2.0 s: *t* = 1, hidden. At 2.1 s: *t* = 1.05, removed.

A critical text on another entity (*c* = 0.06, *i* = 1.7): at *t* = 0.03, *k* = 1.5 × (1 + 0.5 ×
0.7) = 2.025; at *t* ≥ 0.06, *k* = 1.5; no rise; opacity as above; first critical at slot 0
(offset 0, 0), second simultaneous critical at slot 1 (−60, 0) design pixels.

<a id="s17-3"></a>
### 17.3 Cooldown display

`CooldownStarted { what: "Jab", duration_ms: 3000 }` at 3.0 s: *D* = 3, *T* = 6.0, at once *R* =
3, *p* = 1, frame 59; goal 0 over 3 s. At 4.5 s *p* = 0.5, frame trunc(29.5 + 0.5) = 30. At 6.0 s
the goal is reached: *D* = 3 ≥ 2, so `skillavailable` plays; meter hidden.

<a id="s17-4"></a>
### 17.4 Effect row order

Player effects applied at 3.4 s ("A", 20 s), 3.6 s ("B", 5 s) and 3.8 s ("C", no duration), all
beneficial, none combat-only: after A, row 1 is A; B starts later, so it goes before A: B, A; C has
no duration and goes to the front: C, B, A. B (timeout 8.6 s) has less than 15 s left from its
creation, so it pulses at once; A (timeout 23.4 s) starts pulsing at 23.4 − 15 = 8.4 s; C never
pulses. At 8.6 s the feed's `EffectRemoved` removes B: C, A.

<a id="s17-5"></a>
### 17.5 Targeting cycle

Creatures at 5 m (ID 7), 12 m (ID 4), 12 m (ID 9) and 60 m (ID 2), no selection: `target_next_enemy`
→ 7; again → 4 (tie at 12 m broken by ID); again → 9; again → 7 (wrap; ID 2 is beyond 50 m).
`target_previous_enemy` from 7 → 9. `target_nearest_enemy` → 7.

---

<a id="s18"></a>
## 18. Owner checks (proposed rows; O-numbers from O93 assigned at close-out)

| ID | Question | Sections |
|---|---|---|
| OC1 | Targeting keys: what Tab/Backspace select, range, order, wrap | §3.3 |
| OC2 | Escape with a target and an active cast: order of clearing, cancelling, main menu | §3.4 |
| OC3 | The world selection effect (ring decal) appears on selection and at which distances it changes | §3.5 |
| OC4 | When the vital texts (current/max) show on the player and target frames | §4.3 |
| OC5 | Target health-bar colours for neutral and tapped targets | §5.3 |
| OC6 | Name colours at level differences −10 … +10 against a creature | §5.4 |
| OC7 | Whether the selection's-target window is shown by default | §5.7 |
| OC8 | Floating damage wording, sign, colours for damage dealt, damage taken, heals and avoids | §6.6 |
| OC9 | Whether channel bars fill or drain; whether a craft can be cancelled with Escape | §7.3, §7.4 |
| OC10 | Effect rows: which row holds buffs and debuffs; that the icons show neither a timer nor a stack count (whether the effect tooltip shows a stack count is `skill-tooltips.md` OS5) | §8.2 |
| OC11 | Defeat panel: statement, buttons and the awaken timer | §10.2 |
| OC12 | Dragging a skill from one quickslot to another: move, copy or swap; dragging one onto a window | §11 (TR45) |

<a id="s19"></a>
## 19. Open questions (summary)

| Item | Section | Owner |
|---|---|---|
| `UI_Selection_*` properties and selection range | §3.1 | BIN (later) / owner OC1 |
| Targeting search rules | §3.3 | owner OC1 |
| Selection-script start, line-of-sight test | §3.5 | BIN (later) / owner OC3 |
| Temporary vitals, dread, stack update, defeat commands, command errors, titles | §4.3, §11 | `scenario-feed.md` owner |
| Combat wording routine; columns properties; power deltas | §6.5, §6.6 | BIN (later) / owner OC8 |
| Channel fill, craft cancel | §7.3, §7.4 | owner OC9 |
| Effect row options and overflow | §8.2 | owner OC10 |
| Slot overlays not driven | §9.4 | later milestones |
| Defeat panel behaviour | §10.2 | owner OC11 |
| Whether a 0x47 descriptor's hide properties reach the target frame (TR13 reads the body set only) | §5.2 | BIN (later) / owner |
| Quickslot drags in the original (TR45) | §11 | owner OC12 |

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-10 | First version (Draft), for decision 0024 P2: selection and targeting, player and target frames, selection's target, floating text (settings, motion, stacking, preference bits, tokens), cast bars, effect bars, quickslot activation and cooldown display, combat state, defeat panel, event and command map, fallback looks, exit-criterion element list, tests, invariants, worked examples, owner checks. Companion revision: `keymap.md` (targeting and quickslot actions, §8.2, §12.14.2, K47–K49). |
| 2026-10-10 | Implementer OPENs from `crates/gamedata` (no fact changed): §5.2 TR13 player targets with a body record, TR14 objects with vitals. |
| 2026-10-10 | Implementer OPENs from `crates/ui` (no fact changed): §4.4 pip list members, §6.5 rise smoothing and release height, §8.2 row and icon arrangement, §13.7 fallback gaps. |
| 2026-10-10 | Implementer OPENs from the `crates/ui` review fixes (no fact changed): §3.5 E3 mini-vitals meters, §5.7 E7 health meter, §8.2 empty first row and the same-ID rule against CUI7, §13.1 E7 without vitals; §19 lists every implementer OPEN. |
| 2026-10-11 | Review 1 (independent spec reviewer): changes requested; status Draft → Review, not Accepted. No content change (header review record only). |
| 2026-10-11 | Revision 1 (spec writer), blocking item (1) of review 1; status Review → Revised. §5.2 census restated from a fresh census (CU26, new; OBS row extended): `MonsterLevel_ExaminationModStatType` at top level in 16,605 0x79 sets, 309 further values nested in 68 other 0x79 sets (inside `SpeciesLevelLootTable` and `Mod_Array`; 0, 1 and 42–48 only there), none in 0x47 or 0x78; `NPC_HideLevelInDisplay` in 949 0x79 sets and 1 0x47 descriptor; `NPC_HideLevelAndVitalText` in 8 0x47 descriptors (the first version said none); the `PhysObj` links of 12 templates to those descriptors. TR13 now says what the body record is (the 0x70–0x77 game object through its 0x79 set), that steps 3–4 read the three properties at top level only, that 0x47 descriptors (named in `PhysObj` or given as the body) are not read, and that the skull case wins when one set stores both hide properties; new OPEN on how the original composes an entity's properties (§19). No implemented behaviour changes: this is what `tessera-gamedata` already does. Non-blocking review items (a)–(d) are not part of this revision. |
| 2026-10-11 | Review 2 (independent spec reviewer, of revision 1): changes requested; status Revised → Review, not Accepted. Revision 1’s census and TR13 confirmed by own probes and an M4 spot check; one blocking item (TR31 stack count against `skill-tooltips.md` §12.2). No content change (header review record only). |
| 2026-10-11 | Revision 2 (spec writer), blocking item (1) of review 2; status Review → Revised. §8.2 TR31 aligned with `skill-tooltips.md` §12.2: the effect tooltip has no stack-count row (the first version said it shows the count when `stacks` > 1); `EffectApplied.stacks` is not shown in P2. §18 OC10 reworded to match (the tooltip question is `skill-tooltips.md` OS5). Also non-blocking items (d) of review 1 (the duplicate `s18` anchor removed) and (h) of review 2 (companion cell: `scenario-feed.md` revision 6). No implemented behaviour changes: `tessera-ui` shows no stack count on the icon or in the tooltip. Non-blocking items (a)–(c) of review 1 and (e)–(g) of review 2 are not part of this revision and stay open. |
| 2026-10-11 | Review 3 (independent spec reviewer, of revision 2): **passed**; status Revised → Accepted. TR31 and OC10 agree with `skill-tooltips.md` §12.2; own M1 probes and an M4 spot check of §3.5 and §6.4 agree with the text. Non-blocking items carried (review 1 (a)–(c), review 2 (e)–(g)). No content change (header review record only). |
| 2026-10-14 | Revision 3 (spec writer), non-blocking items (a)–(c) of review 1 and (e)–(g) of review 2; status Accepted → Revised, awaiting independent review. Own read-only probes of the install (M1) and an M4 pass (BIN questions (j), (k)). **(a), (b)** §8.2 Order restated from the insert routine (CU30): the same-ID test compares the per-application runtime ID and sends a new icon to position 0 whatever its timing; combat-only icons pass over timed icons, an untimed one goes right after the last untimed non-combat-only icon; TR30 says a repeated instance replaces its icon (so the same-ID case never arises from the feed) and E9 is rebuilt in arrival order; CUI7 restated. **(c)** Every implementer and exit-verifier OPEN except §4.1 (left to a parallel branch) answered: §3.3 TR4 means the full TR1 test; §3.5 mini vitals named (CU27, TR8a); §4.4 pip entry members named (CU28); §5.2 player targets take `normal` (TR13); §5.3 objects with vitals red (TR14a); §5.7 E7 meter named (CU27, TR18a); §6.5 figure box (TR19a) and the rise smoothing and release height from the client (CU31); §8.2 row arrangement (TR30a); §10.2 box children named (CU29, TR38a); §11 slot drags (TR45, owner check OC12); §13.1 E7 without vitals (TR42); §13.4 bar place (TR43); §13.7 fallback gaps (TR44); §16.1 screenshots (TR46). §19 updated. **(e)** OBS row: 1,618 0x78 sets (1,616 + 2 in `client_local_English.dat`, neither stores the three properties). **(f), (g)** TR13 restated: the body set is the set `property-system.md` §6.2 gives (object prefix + 9 for 0x70–0x76; the record itself for 0x78–0x7F; none for 0x77, 0x47 and other prefixes). **Implemented behaviour that changes on main:** (i) an untimed combat-only effect icon goes right after the last untimed non-combat-only icon instead of position 0 (`tessera-ui` effects); (ii) the floating-text rise speed closes half its gap per second (was a 0.25 s time constant), and a pending text is released when the smallest rise of the shown active non-critical texts exceeds 0.65 × the pending text's fitted height (was the template's layout height compared with the most recent text) (`tessera-ui` floaty); (iii) the targeting keys apply the full TR1 test, reading body records (`apps/client` targeting); (iv) TR38a hides the defeat panel's parts by label, which changes the result only if the implemented position rule classified the inherited box art differently. Unchanged: TR13 (the code already reads a 0x78–0x7F DID as the set itself and gives 0x77 none), TR8a, TR14a, TR18a, TR19a, TR30a, TR42–TR46 (they record the implemented readings), CU28 (the code's first-Int / first-Enum reading gives the same members). |
| 2026-10-14 | Review 4 (independent spec reviewer, of revision 3, §4.1 excluded): **passed**; status Revised → Accepted. Own M1 probes of CU27–CU29 and an M4 spot check of BIN (j) and (k) agree with the text; TR13 matches `tessera-gamedata`. One non-blocking note (TR38a child order). No content change (header review record and status only); implemented behaviour is as revision 3's row says. |
