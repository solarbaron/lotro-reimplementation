<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# P2 visual checklist (targeting, skills and combat presentation)

Pass/fail record for the visible parts of [ROADMAP P2](../ROADMAP.md#p2): one row per combat
element of [`combat-ui.md`](../specs/combat-ui.md) §2 and §14, per tooltip of
[`skill-tooltips.md`](../specs/skill-tooltips.md), per combat visual of
[`combat-visuals.md`](../specs/combat-visuals.md) and per test-server case of
[decision 0032](../decisions/0032-p2-targeting-on-test-server.md), each with the command line
that shows it. The exit criteria themselves are judged by the tests named in
[ROADMAP P2](../ROADMAP.md#p2-exit); this list is what a person looks at, in Tessera, to see
that the pictures agree with the specs. Comparisons with the original client are owner rows
O93–O109 and O126–O129 of [`owner-observations.md`](owner-observations.md#p2-items).

Text only: screenshots stay in a private scratch directory and are deleted after the check; no
game text (skill, effect or creature names, tooltip lines) is recorded here. Describe what you
see in your own words; name records by DID.

## Method

- **Install.** The owner's install, read in place: `export TESSERA_GAME_DIR="$(cargo run -q -p
  tessera-dattool -- detect --path)"`. Record the data date when the checks are run. Rows marked
  "no install" need none.
- **Clock.** The commands that play a scenario windowed run it on the wall clock from world
  entry; the times in the rows are seconds of the scenario (`at` and `+` steps). Headless test
  runs use the scenario's virtual clock (the checkpoints of `combat-ui.md` §17.1).
- **Results.** `pass`, `fail` (with a short note) or `n/j` (not judgeable from that run; say
  why). Fill in the date and the commit in the Result column.
- **Expected values** cite the spec rule (TR, CUI, CV numbers, sections). Where a spec rule is
  a Tessera decision (TR7, TR8, TR41, `combat-visuals.md` §11), the row checks Tessera's rule,
  not the original's; the owner rows check the original.

### Commands

| ID | What | Command |
|---|---|---|
| C1 | Reference scenario, windowed, install look | `cargo run --release -p tessera-client -- --scenario apps/client/tests/scenarios/combat-ui.scenario --ui` |
| C2 | Synthetic goldens (no install, fallback looks): checkpoints 1.2, 3.1, 3.4, 4.3 and 8.1, compared with the committed pictures `apps/client/tests/golden/combat_ui_<s>_<tenths>.png` | `cargo test -p tessera-client --test combat_ui_golden` (bless with `TESSERA_BLESS=1` only after a reviewed change) |
| C3 | Real-data screens: all eleven checkpoints of §17.1 with the install's panels, written on the first run and compared afterwards in `~/.cache/tessera/goldens/combat_ui/combat_ui_<ms>.png` (never committed, ARCHITECTURE §23.3) | `cargo test --release -p tessera-client --test combat_ui_real_data -- --nocapture` |
| C4 | Combat visuals, windowed: the scenario of [§ C4 scenario](#c4-scenario) saved outside the repository (for example `~/.cache/tessera/scratch/p2-visuals.scenario`) | `cargo run --release -p tessera-client -- --scenario ~/.cache/tessera/scratch/p2-visuals.scenario --ui --spawn 1,126,140,10,20,436 --heading 0` |
| C5 | Combat visuals, census and headless walker run (counts only) | `TESSERA_FULL=1 cargo test --release -p tessera-client --test combat_visuals_real_data -- --nocapture` |
| C6 | Skill and effect tooltips, every record (counts only) | `cargo test --release -p tessera-client --test combat_tooltips_real_data -- --nocapture` |
| C7 | One skill or effect record's card with its hover tooltip in the explorer | `cargo run --release -p tessera-explorer -- --select 0x7XXXXXXX --view gameplay` (add `--screenshot card.png` for a picture) |
| C8 | Test server with wanderers, two clients | Server: `tessera-server --data-dir ~/.cache/tessera/scratch/p2-server keygen`, then `account add wren --generate` and `account add finch --generate` (same `--data-dir`), then `tessera-server --data-dir ~/.cache/tessera/scratch/p2-server run --listen 127.0.0.1:7777`. Clients (two terminals, `TESSERA_PASSWORD` set to each printed password): `cargo run --release -p tessera-client -- --server tessera://127.0.0.1:7777 --account wren --ui --server-fingerprint FP` and the same with `--account finch`, where `FP` is the fingerprint `keygen` printed (`sha256:` and 64 hex digits; `tessera-server --data-dir … fingerprint` prints it again). A connect string `tessera://127.0.0.1:7777?pin=FP` does the same. Do not use `--trust-new-server` here: it trusts the key before it can be checked. |
| C9 | Selection in the synthetic server test (no install, no GPU) | `cargo test -p tessera-client --test combat_server` |

<a id="c4-scenario"></a>
### C4 scenario

Synthetic and selector-only; the records are picked from the install at run time, so the
file names no record and holds no game text. Replace `<INDUCTION>` and `<CHANNEL>` with the
DIDs of a skill whose explorer card (C7) shows an induction and of one that shows a
channelling state; if none is at hand, delete the lines that use them and mark rows V4 and V5
`n/j`. Change a `seed` to try other records.

```
tessera-scenario 1
title "Combat visuals check"
tick 100ms
player "Tamsin"
select $mob = creature kind=mob n=1 seed=3
select $sk = skill harmful=yes n=3 seed=7
select $fx = effect role=vital-over-time n=1 seed=5
select $ind = did <INDUCTION> all
select $chan = did <CHANNEL> all
actor $dummy kind=creature body=$mob[0] name="Dummy" pos=0,4,0 facing=player level=10
at 5s spawn $dummy
+0s vital $dummy health 1000/1000
+0s vital player health 900/900
+0s vital player power 400/400
+0s quickslot 1 $sk[0]
+0s quickslot 2 $sk[1]
at 10s target player $dummy
+0s combat player on
+0s combat $dummy on
+0s target $dummy player
at 12s cast player $sk[0] 1s target=$dummy
+0.5s damage player $dummy 50 result=normal with=$sk[0] left=950
at 15s cast player $sk[1] 1s target=$dummy
+0.5s damage player $dummy 80 result=critical with=$sk[1] left=870
at 18s cast $dummy $sk[2] 1s target=player
+0.5s damage $dummy player 40 with=$sk[2] left=860
at 21s cast player $ind[0] 3s target=$dummy kind=induction
at 22.5s interrupt player
at 25s effect $dummy $fx[0] for=10s from=player
+0s effect player $fx[0] for=10s
at 27s cast player $chan[0] 4s target=$dummy kind=channel
at 38s combat player off
+0s combat $dummy off
at 40s defeat $dummy by=player
at 44s revive $dummy
+0s vital $dummy health 1000/1000
at 46s defeat player by=$dummy
at 50s revive player
+0s vital player health 900/900
```

## 1. Combat elements (`combat-ui.md` §2, §14)

The headless checkpoints (C2, C3) are the exit-criterion evidence; C1 shows the same elements
moving. "Fallback" means the look of `combat-ui.md` §13 (C2); "install" the install's panels
(C1, C3).

| # | Element | Command | When | Expected | Result |
|---|---|---|---|---|---|
| E5 | Player frame: name, level, health, power | C1, C2, C3 | from 0.5 s | Name of the scenario's player; level 20; health 900 / 900 and power 400 / 400 full (§4.3; CUI2); install look in the toolbar's `AvatarPanel`, fallback box top left (§13.1) || C2 pass; C3 fail: bars shown, but the name and level text of the install frame are not drawn (tessera-ui `PropertyDisplay` text); verifier 2026-10-11, p2 tree |
| E20 | Class resource | C1, C2 (3.4 s), C3 (0.5 s) | from 0.5 s | The resource bar of §13.4 under the player frame (190 × 4, fill 0xFFE0C030) filled to 2 / 5 (0.4), in both looks: the scenario's resource is a literal ref, which TR11 shows only by the fallback look; no pip display is shown. Pips need a resource selector that resolves to a pip record; the reference scenario has none || C2 pass (3.4 s, 0.4); C3 pass after the verifier's fix (bar at its §13.1 place over the toolbar art, `combat-ui.md` §13.4 OPEN); 2026-10-11 |
| E6n | Target frame, non-vital field | C1, C2 (1.2 s), C3 | 1.0–2.0 s | The object selected: its name, no meters (§5.2 field choice, TR12); name in the object colour (TR15) || C2, C3 pass (1.2 s); 2026-10-11 |
| E6v | Target frame, vital field | C1, C3 (2.1 s) | from 2.0 s | The creature: name in the consideration colour of level 7 against 20 (TR15, §5.4), level 7 (TR16), health meter full then falling with each damage `left=` value (§5.3, TR14 colour) || C3 pass for name and health (2.1–4.7 s); the level text is not drawn (as E5); 2026-10-11 |
| E7 | Selection's target | C1, C3 (2.1 s) | 2.0–8.0 s | The small window shows the creature's target, the player, with its health meter (§5.7, TR18) || C2, C3 pass; 2026-10-11 |
| E1 | Selection indicator (screen) | C1, C2 (1.2 s), C3 | while a selection exists | Above the selection's anchor (§6.4 with the −75 offset and the 0.25 / 0.2 scales); peace image before 2.0 s, combat image from 2.0 s to 10.0 s (TR7); never a no-line-of-sight image || C2, C3 pass (peace at 1.2 s, combat at 3.4 s); since the verifier's TR40 fix drawn in front of the floating texts; 2026-10-11 |
| E3 | Selection name block | C1, C2 (1.2 s), C3 | while a selection exists | Name in its colour state, empty title line, mini vitals with the selection's health (TR8); the mini induction shows the creature's cast at 4.1–5.6 s || C2, C3 pass; 2026-10-11 |
| E17 | Combat state | C1, C3 (2.1 s) | 2.0–10.0 s | The toolbar's combat indicator on while the player's last `CombatState` is in combat, off after 10.0 s (§10.1, TR37) || C2 pass (box); C3 n/j (the toolbar's indicator change is too small to judge in the picture); 2026-10-11 |
| E11a | Player cast bar, filling | C1, C3 (2.5 s) | 2.2–3.0 s | Label of the cast, fill *p* = 0.375 at 2.5 s, rising linearly (§7.3, TR26) || C3 pass (2.5 s: fill a little under half, label); 2026-10-11 |
| E11b | Player cast bar, end state | C1, C2 (3.1 s), C3 | 3.0–4.0 s | Completed state (`Succeeded`) held for 1 s, then hidden (§7.4, TR28) || C2, C3 pass (3.1 s full bar); 2026-10-11 |
| E12 | Target cast bar | C1, C2 (4.3 s), C3 | 4.1–5.6 s | The creature's cast with its label, filling; hidden when the selection changes (CUI1) || C2, C3 pass (4.3 s); 2026-10-11 |
| E8 | Player effect row | C1, C2 (3.4 s), C3 | 3.2–23.2 s | Beneficial icons in the first row; the record effect (30 s) and the literal one (20 s), the literal one in the fallback box of §13.5 in both looks (TR41, TR29); untimed before timed, then newest first (CUI7, §8.2). || C2 pass; C3 pass after the verifier's fix (the literal effect takes the §13.5 box in the install look); 2026-10-11 |
| E9 | Target effect row | C1, C2 (3.4 s), C3 | 3.2–7.2 s | The harmful literal effect on the creature in the target frame's row, as the fallback box of §13.5 in both looks (TR41); gone at its end (CUI6). || C2, C3 pass (as E8); 2026-10-11 |
| E10 | Pulsing effect icon | C1, C2 (3.4 s), C3 | within 15 s of an effect's end | The creature's 4 s effect pulses at once; the player's 20 s effect starts pulsing at 8.2 s (§8.2). || C2 pass (4.3 s dimmed); C3 n/j (pulse phase not visible in one picture); 2026-10-11 |
| E13a | Floating damage, critical | C1, C2 (3.1 s), C3 | 3.0–5.0 s | −210 over the creature (TR19) in red with a white outline (`Floaty_CriticalDamage` 0xFFFF0000 in the install look, 0xFFFF2020 in the fallback, §6.3, §13.6); it starts at 1.7 × its final size (scale 2.55, then 1.5) and shrinks to it while *t* < 0.06 (the first 0.12 s of the 2 s life; *t* is the life fraction of §6.3), does not rise (`CriticalDamage_SpeedMod` 0 of column 0x78000004, §6.3 step 3, §17.2), fades from 1 s and is hidden at 2 s (TR20, CUI4) || C2 pass (3.1 s); C3: shown in the tree but drawn behind the target frame (the synthetic camera puts the creature's text under the panel; TR40 puts texts behind panels); 2026-10-11 |
| E13b | Floating damage taken | C1, C3 (3.8 s) | 3.5–5.5 s | −75 over the player in 0xFFFF4040, rising and fading within 2 s (TR19, TR20; Tessera's choice under the §6.6 OPEN) || C2 4.3 s and C3 4.7 s pass (rising, colour); 2026-10-11 |
| E13c | Floating avoidance | C1, C3 (4.0 s) | 3.8–5.8 s | The parry word over the creature, no number (§6.6 short-form token) || C2 4.3 s, C3 4.7 s pass; 2026-10-11 |
| E13d | Floating heal | C1, C3 (4.7 s) | 4.6–6.6 s | +60 over the player in 0xFF40FF40, rising and fading within 2 s (TR19, TR20) || C3 pass (4.7 s, green); 2026-10-11 |
| E15/E16 | Skill slot with cooldown | C1, C2 (3.1 s), C3 | 3.0–6.0 s | Slot 1 (a literal skill, the fallback icon of §13.7 in both looks, TR32) shows the cooldown sweep; frame index trunc(59*p* + 0.5) never increases (CUI5); the slot's casting frame during 2.2–3.0 s (§9.4, TR36). || C2 pass (sweep on slot 1); C3 pass after the verifier's fix (literal slots show the §13.7 content; sweep at 3.1–4.7 s, playing frame at 2.5 s); 2026-10-11 |
| E16i | Skill slot with a record icon | C1, C3 | from 0 s | Slot 3 shows the selected skill record's icon (`skill-tooltips.md` §11); slots 1 and 2 (literal skills) show the fallback icon of §13.7 (fill 0xFF404060, the literal's first two characters) even in the install look (TR32). || C3 pass after the verifier's fix (slot 3 record icon; slots 1 and 2 "Ja", "Me"); 2026-10-11 |
| E18 | Defeat panel | C1, C2 (8.1 s), C3 | 8.0–10.0 s | Shown: Tessera's own statement of TR38, the OK button labelled by its token, the revive button labelled by its token and ghosted. Hidden: the paid-revive and store buttons (`SelfRevive_PayForRevive`, `SelfRevive_PaidCategory_BuyNowButton`), the divider's paper-item parts and the awaken timer E19. Nothing else on the panel. OK closes it; the buttons send no command (§10.2, TR38); revived at 10.0 s, panel gone. **Expected to fail on C1 and C3** (2026-10-11): the install picture at 8.1 s shows a third, unlabelled button left of the revive button; which TR38-hidden part it is is not yet known (reported to the `tessera-ui` owner) || C2 pass; C3 fail: a third, unlabelled button left of the revive button and the divider are shown; the revive button's ghosted look is not distinct (owner O103); 2026-10-11 |
| E18v | Player frame at defeat | C1 | 8.0 s | Health 0 / 900 after `left=0`; the frame stays (§10.3) || C3 pass (8.1 s: health empty, frame kept); C1 n/j (not run windowed); 2026-10-11 |

## 2. Selection and targeting input (`combat-ui.md` §3)

| # | Check | Command | How | Expected | Result |
|---|---|---|---|---|---|
| T1 | Mouse selection | C4 | after 5 s, click the creature figure; then click the ground | The creature becomes the selection once the session reports it (TR1, TR3); a click on the ground or the sky changes nothing | |
| T2 | Double click uses | C4 | double-click the creature | Selection, then a use command; no error line | |
| T3 | Targeting keys | C4 | Tab, Shift+Tab, Backspace, F10 near the creature; Backslash; F | Tab, Shift+Tab and Backspace select the creature (the only creature within 50 m, TR4); Backslash selects the player; F selects the selection's own target (assist) once the creature targets the player (10 s) | |
| T4 | Delete selects the nearest object | C1 | press Delete after 1 s | The object actor becomes the selection (E6 non-vital field) | |
| T5 | Escape order | C4 | Escape during the induction at 21–22.5 s; again with no cast; again with no selection | First press cancels the cast (the bar ends cancelled); second clears the selection; third opens the main menu (TR5) | |
| T6 | Quickslot keys | C4 | press 1 and 2 with the creature selected | Each slot sends its skill use (no refusal flash in the sandbox); Ctrl+1 … Shift+= reach slots 13–48 (K47) | |
| T7 | Walker keys taken by the defaults | C4 | press Backslash, Minus, Equal, F9, F10 | They act as targeting and quickslot keys, not as the clock and switch keys (`keymap.md` §8.2 OPEN; USER_GUIDE §3.5) | |

## 3. Skill and effect tooltips (`skill-tooltips.md`)

| # | Check | Command | How | Expected | Result |
|---|---|---|---|---|---|
| TT1 | Skill tooltip on a quickslot | C1 | hover over slot 3 | The skill's examination tooltip in the examination panel: name, icon, rows in layout order with their colours, costs, ranges and times (§4, §7, §9); it does not follow the pointer | |
| TT2 | Effect tooltip with live timer | C1 | hover over the player's 30 s record effect icon from 4 s on | The effect's tooltip with its lines (§9) and the remaining time counting down while it shows; only the timer text changes (§10.2, §12.2) | |
| TT3 | Tooltip of a selected record in the explorer | C7 | `--select` a skill DID, then an effect DID | The card's tooltip preview has the same rows, order and colours as TT1 and TT2 for the same records. Values of effect lines evaluated at the viewer's level may differ: the explorer's viewer is level 1, C1's player level 20 (`skill-tooltips.md` §9.8 Tessera rule). The explorer shows no live timer row and no live values (§10.2, §12.2) | |
| TT4 | Every record (counts) | C6 | read the summary lines | Every skill and effect record ends in exactly one tooltip and one icon category (§14.1); 0 failures || pass (C6, 2026-10-11): 33,601 laid out (24,812 with an icon), 0 not laid out, 0 row failures; gamedata census as §14 (skills 5,201 / 8,046, effects 27,150 / 1,250 / 4,571) |
| TT5 | Skill-granting item | P1 tooltip path: `cargo run --release -p tessera-client -- --scenario FILE --ui --open-panel bags` with a scenario that gives one item whose record grants a skill | hover over the item in the bag | The item's tooltip comes with the granted skill's tooltip (`skill-tooltips.md` §12.4; `item-tooltips.md` §6.20, §8.3 `SkillGrantingItem`) | |

## 4. Combat visuals (`combat-visuals.md`)

All rows use C4 (install, windowed). The camera looks at the creature from behind the player;
turn it to see both figures. C5 gives the counts of exit criterion 3; these rows are what it
looks like.

| # | Visual | When | Expected | Result |
|---|---|---|---|---|
| V0 | World selection effect (E4) | from 10 s; and after T1 | The selection decal under the creature, its size band by distance (§3.5 TR6); stops when the selection clears (T5) or the creature is despawned | |
| V1 | Cast start: behaviour on the caster | 12 s, 15 s | The player plays the skill's behaviour clip; while walking (W held) the attack plays on the upper body over the walk (`skeleton-anim.md` §6.6, §3.3) | |
| V2 | Hook strike: hit reaction and hit particles | 12.5 s, 15.5 s | At the behaviour's attack hook the creature plays its hit reaction and the hit script's particles at the struck figure (§4.2–§4.5); the critical strike at 15.5 s uses its own result branch where the data has one | |
| V3 | Projectile or beam | a ranged skill (change `seed` until one appears) | The projectile or beam particles travel from the caster to the target, and the hit plays on arrival (§4.7). The beam ribbon and head entity are not drawn yet (known gap, ARCHITECTURE §3.1) | |
| V4 | Induction, interrupted | 21–22.5 s | The induction loop plays on the player while the bar fills; at the interrupt the loop and the induction's own effect stop (their never-ending particles may linger up to 6 s), the induction's failure effect (`Induction_FailureFXID`) plays on the player and its target counterpart (`Induction_TargetFailureFXID`) on the creature where the record names them, and no strike or skill behaviour is shown (§3.5, §9.1, worked example §13.3) | |
| V5 | Channel | 27–31 s | The channelling visuals play for the whole channel and stop at its end (§3.5) | |
| V6 | Creature's attack on the player | 18–18.5 s | The creature's behaviour plays, then the player's hit reaction (§4.4) | |
| V7 | Effect visuals | 25–35 s | The effect's particles attach to each figure, loop while the effect lasts and stop at 35 s, lingering particles fading out (§5.2) | |
| V8 | Combat stance | 10 s and 38 s | Combat on: the player takes the stance of its main-hand item (empty hand: unarmed, §11 rule 6) and the creature its monster stance; combat off: both back to the peace idle (§6) | |
| V9 | Defeat | 40 s, 46 s | The death clip plays once, then the death hold keeps the pose (§7); nothing else overrides it | |
| V10 | Revive | 44 s, 50 s | The hold stops and the recovery plays; the figure stands in its idle again (§7, invariant 8) | |
| V11 | Census (counts) | C5 | Every Particle node reachable from skill and effect records names a 0x39 or 0x44 record that exists and decodes (invariant 7); the walker run starts effects and draws particles on both figures || pass (C5, 2026-10-11): 20,552 Particle nodes, 6,580 effect DIDs, 0 missing; walker run through the headless scenario view: 158 combat events, 2 figures, 130 effects asked for, up to 43 systems / 6,102 vertices in a frame |

## 5. Test server (decision 0032)

| # | Check | Command | How | Expected | Result |
|---|---|---|---|---|---|
| S1 | Select a wanderer | C8, client wren | click a wanderer's figure | Target frame: the wanderer's name in the colour of its kind (TR15 step 3: the NPC colour), no level, no meters (the no-vitals form); E1 and E3 over its figure (D2) | |
| S2 | Select the other player | C8, both clients near each other | wren clicks finch's character | Same as S1, the name in the player colour (TR15 step 3); nothing new reaches the server (D1; C9 checks the counters) | |
| S3 | Despawn clears locally | C8 | finch quits while wren has finch selected | wren's target frame, E1, E3 and the selection effect go away; no command is sent (D2) | |
| S4 | Overlay files cannot script combat | `cargo run --release -p tessera-client -- --headless --scenario FILE --server tessera://127.0.0.1:7777 --account wren`, with two files saved outside the repository: (a) `tessera-scenario 1` / `at 1s combat player on`; (b) `tessera-scenario 1` / `input` / `at 1s target none` / `end`. Do not use `combat-ui.scenario`: it is refused at its `player` declaration, which is outside the overlay subset for another reason | run each | Exit status 3 before connecting, `NotAllowedInOverlay` at the Combat-group line: (a) 2:1, (b) 3:1 (D3; `scenario-feed.md` §12.5.1 and conformance cases O10–O12 of §14.9). Checked 2026-10-11 on the p2 tree: both as expected || pass (docs session, 2026-10-11, exit 3 at 2:1 and 3:1) |
| S5 | Synthetic server test | C9 | read the summary line | All cases pass (selection answered locally, target frame without level or vitals, local clear on disconnect, overlay selection) || pass (C9, 2026-10-11: 2 / 2) |
