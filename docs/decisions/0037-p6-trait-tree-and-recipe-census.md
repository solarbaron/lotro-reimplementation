<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0037 — P6: the trait-tree and recipe census and golden images of the P6 panels

**Status:** Proposed, 2026-10-16 (waits for the owner).

The number 0037 follows [0036](0036-p5-map-census-and-panel-goldens.md); P6 starts its records
there because a fix-up branch runs beside it and takes no decision numbers.

## Context
[Decision 0024](0024-parity-track.md#p6-progression-and-crafting-panels) §5 sets two exit
criteria for P6 (progression and crafting panels):

1. Every trait tree and recipe record renders (or is categorised).
2. A progression scenario (level up, train, craft) runs headless; golden images pass.

Criterion 1 does not say which records are "trait tree records" and "recipe records", nor what
"renders" means for a record that a panel shows only as a row or a page. The install has many
records that look like one or the other: trait trees and the point-based traits they place in
cells, other trait classes (virtues, racial, appearance, skirmish and granted traits), recipes,
recipe scrolls and the items that teach several recipes. Decision 0024 §5 already says how a
census treats a record that cannot be shown: it is listed with a reason (categorised), never a
crash.

Criterion 2 asks for golden images. [Decision 0031](0031-parity-panel-goldens.md) (Proposed)
names P6 among the milestones it serves and sets the rules: committed synthetic goldens (tier 1),
local-only real-data goldens (tier 2) and what "golden images pass" means for an exit verifier
(§3). It does not say which P6 panels need goldens.

The panel specs give the definitions. [`crafting-panels.md`](../specs/crafting-panels.md)
covers the crafting panel and recipe page, the crafting bar and results, the crafting trainers,
gathering nodes and facilities, with a census of recipes and the other crafting records (§10);
it is in Review. `progression-panels.md` is to cover the experience bar, levels and the
level-up presentation, the trait panel with its trait trees, and skill training, with a census
of the trait trees; it is being written and is **not yet in the tree** (see Open). The feed
forms both use are the [`scenario-feed.md`](../specs/scenario-feed.md) revision labelled "P6
progression" (§6.6, §18.P6; Review), numbered at the merge.

## Decision

### 1. "Every trait tree and recipe record" (criterion 1)
1. **Trait trees.** The census record is the trait-tree record: every record of the
   trait-tree class of [`combat-data.md`](../specs/combat-data.md) §8.5, each visited once. Its
   **placements**, the entries of its `Trait_TraitTree_TraitArray`, are parts of that record,
   not records of their own: each (tree, placement index) is visited exactly once, with its
   tree, and the placements decide the tree's category. The part is the placement, not the
   trait record, because placements outnumber the traits they name (`combat-data.md` §8.5: 1,216
   placements, all naming class-3438 traits; §8.1: 946 class-3438 records), so one trait DID can
   sit in more than one placement and could resolve in one and not in another. The tree is
   "rendered" when the trait panel builds its page with every cell, rank and prerequisite the
   progression spec names, and a placement is "rendered" when its cell and its trait's tooltip
   are built. Trait records that no tree places, of any class, are not part of criterion 1; the
   progression spec may count them in its census, but no criterion waits on them.
   **Held:** until `progression-panels.md` is committed, the trait half of criterion 1 has no
   census categories, no spec meaning of "rendered" per cell and rank and no expected counts,
   so it can be neither passed nor failed; only the unit above is fixed here (see Open).
2. **Recipes.** A recipe record is what `crafting-panels.md` §10.1 defines: every object record
   of the recipe class, found by its `CraftRecipe_Profession` property, without class indices.
   The recipe is "rendered" when the panel builds, for each of its tiers, its tree rows (§4.4)
   and its recipe page (§5), and the scroll rows of §7 when a scroll names it. The other
   families of `crafting-panels.md` §10 (professions, vocations, guilds and trainers §10.2,
   gathering nodes and facilities §10.3, recipe scrolls §10.4) run in the same census and are
   reported beside the recipes; criterion 1 names only the recipes.
3. **Census.** Both lists are built from the install at run time, never from counts or IDs
   written into a test. Each record, in its order, is built as above on the CPU painter at UI
   scale 1, headless, with no GPU, and is put in exactly one category of its spec's census: the
   first that applies, `Shown` when every part resolved (the recipe categories of
   `crafting-panels.md` §10.1 are `ProfessionMissing`, `TierTextMissing`,
   `CategoryTextMissing`, `NameMissing`, `IngredientMissing`, `TooManySlots`,
   `ResultImageMissing` and `Shown`; the trait-tree categories will be those of the census
   section of `progression-panels.md`, cited here by section and name once it is committed). A
   category other than `Shown` is a reason, not a failure (`crafting-panels.md` invariant CI9;
   the progression spec's census invariant, once it exists).
4. **Pass.** Criterion 1 passes when the census visits every record exactly once (and every
   placement of a tree exactly once, with its tree), every record lands in exactly one
   category, no record is a failure, and the counts per category and per
   family are recorded in the P6 status. A **failure** is a record whose building panics, logs
   an error, or leaves a part the spec requires unbuilt without a category that names the
   reason (for example a recipe in `Shown` whose page has an ingredient slot without its item,
   or a tree in `Shown` with a placement that has no cell).
5. **Counts are evidence, not constants.** The counts of the two specs (for recipes
   `crafting-panels.md` §10.1: 7,791 records, 7,782 `Shown`, 9 `ResultImageMissing` on the
   owner's install of 2026-10-16; for trait trees those `progression-panels.md` will give,
   which are to rest on the 17 trees and 1,216 placements of `combat-data.md` §8.5) are what the exit
   verifier expects on that install and compares with. A later install may differ: the test
   asserts the rules of item 4, and a count that differs from the spec is a warning (data
   change), not a failure (`crafting-panels.md` §10.4, Tessera rule).

### 2. Golden images of the P6 panels (criterion 2)
Decision 0031's two tiers apply unchanged to these P6 panels:

| Panel | Tier 1 (committed, synthetic) | Tier 2 (local only, install) |
|---|---|---|
| Experience bar | Tessera's fallback bar with literal values: a part-filled bar, and the state just after a level-up with the level-up presentation frozen at a fixed virtual time | The install's bar (the layout the progression spec names) with feed values |
| Trait panel | The fallback trait panel with a literal tree: tabs, cells with ranks, a met and an unmet prerequisite, available and spent points, a slotted trait | The install's trait panel showing one tree chosen by rule (the first tree of the player's class by DID, never a DID written into the test), with feed ranks and points |
| Trainer window | The fallback skill-training window with literal rows: an affordable row, a row above the player's level, a trained (ghosted) row, a selection; and the fallback crafting-trainer window of `crafting-panels.md` §11 | The install's trainer layouts with rows from install skills; the three crafting-trainer windows of `crafting-panels.md` §8.3 for the first profession, vocation and guild by DID |
| Crafting panel | The fallback panel of `crafting-panels.md` §11 in two states. (a) The synthetic content of §15.4: one profession tab, its tier-1 row with a proficiency meter (§4.7 rule 2), a category row and a craftable recipe row with its count, the recipe page with ingredients held and needed, the tool line, quantity and the make controls. (b) The tier rows of §15.2 (`profession P tier=3 proficiency=120/400 mastery=60/800 mastery-tier=2`, with recipes at tiers 1–3): proficiency icons, a mastery icon, a mastery meter and a proficiency meter, which §15.4 alone never reaches (there *a* = *b* = 0). State (b) adds feed lines beyond §15.4 and is built in-process (§3 item 5) | The install's `craft` layout with recipes chosen by rule (the first recipe by DID of the first profession, and the first one with optional ingredients and a critical result), with feed holdings and profession values |

The crafting bar (the cast bar of `combat-ui.md` §7 with kind `Craft`), the result lines in the
chat window, the gathering bar and the level-up chat lines should have goldens too; this record
does not require them for criterion 2. They are not protected by any committed image: no
committed golden shows the chat window (the P4 tier-1 goldens are the social panels only), so
until P6 or a later milestone adds one, these lines are checked only by the scenario log and
unit tests (see Open). The recipe page's facility line has no tier-1 golden either: a literal
recipe of the feed names no required facility (`scenario-feed.md` §6.6 `recipe` options), so a
synthetic run cannot show the line's met and unmet states; the tier-2 sample shows it when the
chosen recipe names a facility (see Open).

- For criterion 2, "golden images pass" requires the steps of decision 0031 §3 for the panels
  above: tier-1 goldens committed and passing in the workspace run; tier-2 goldens blessed once
  into a fresh golden directory of the verification, judged by eye against the panel's spec,
  and matched on a second run with `TESSERA_GOLDEN_REQUIRE=1`. The status records the install
  version and the verdict per image by test and name, never the picture or its text.
- The `--open-panel` names of the specs (`crafting`, `crafting-recipe`, `vocation-trainer`,
  `profession-trainer`, `craftguild-trainer` of `crafting-panels.md` §3, and the progression
  spec's names) serve inspection; goldens capture in-process (decision 0031 §1).
- Everything else of decision 0031 holds as written: inputs, capture path, painter,
  tolerances, blessing variables, locations, the refusal of directories inside a git work
  tree, atomic writes and the determinism rules (virtual clock, fixed fonts and viewport, no
  running animation, no capture after a settle timeout). A level-up presentation or meter fill
  that animates is captured after it has finished or at a fixed virtual time, never mid-run on
  the wall clock.
- The census of §1 checks every record by rules; tier-2 goldens cover only the sample. No
  golden of every tree or recipe is kept.

### 3. Criterion 2: the progression scenario runs headless
1. **Scenario.** A committed, original, synthetic scenario in `apps/client/tests/scenarios/`
   (name chosen by the implementer; `progression` exists from P0) that uses literal refs and
   runs with no install, and selectors only (no DID lists) where it adds install records. It
   has the three legs the criterion names, each scripted by the feed (values scripted, never
   derived; decision 0024 §2 principle 4):
   - **level up:** experience reports and a `LevelChanged` that raises the level and is not
     `quiet`, so the client runs its level-up presentation;
   - **train:** a trainer window (`trainer … offers=…`) or a trait tree (`trait-tree`,
     `trait-rank`, `trait-points … tree=`) where scripted input sends the train or trait
     command (`TrainSkill`, `SetTraitRank` or `ChooseTraitBranch`) and a trigger answers it, plus a
     crafting trainer (`craft-trainer`, `TrainCraft`) when the scenario covers it;
   - **craft:** the leg of `crafting-panels.md` §15.4: a profession, a learned recipe, held
     ingredients, scripted `craft …` input, `cast … kind=craft` bars, `craft-result` lines and
     a `profession` report that moves the proficiency meter.
2. **Runs.** `tessera-client --headless --no-data --scenario F --scenario-log L` on the virtual
   clock, release build, twice: exit code 0 both times, both logs byte-identical and equal to the
   committed `.log`. With `TESSERA_GAME_DIR` set, the same scenario (with its selectors resolved)
   runs headless twice with exit code 0 and byte-identical logs; that log is not committed.
3. **What the log must show.** In order: the not-quiet `LevelChanged` that raises the level,
   the train or trait command and the events that answer it, the `Craft` command with the
   scripted quantity and one `CraftResult` per craft, and the `ProfessionChanged` after them.
   A test in the client asserts these lines, and the panel state after the run (the level
   shown, the trained row ghosted or the rank shown, the recipe's makeable count and the meter
   position of `crafting-panels.md` §15.4). In the `--headless --no-data` run of item 2 the
   client runs only `HeadlessDriver` (`scenario-feed.md` §12.2): no window and no panel, so the
   commands in the log come from the scenario's input lines (`scenario-feed.md` §7.7), not from
   a panel control.
4. **The panel controls.** Because item 3 does not exercise them, a client test also drives the
   panels in-process on `HeadlessDriver`: it builds each panel from the feed state the
   scenario reached and asserts the command a control emits: Make and Make All (`Craft` with
   the quantity of `crafting-panels.md` §5.6 and its invariant CI6, and its tier, output,
   optional and pack fields), the skill trainer's train button (`TrainSkill`), the crafting
   trainer's train button (`TrainCraft`) and a trait cell's click and branch choice
   (`SetTraitRank`, `ChooseTraitBranch`; held with the trait half of §1 until
   `progression-panels.md` exists).
5. **Golden images.** Every tier-1 golden of §2 is required whatever legs the scenario has:
   the experience bar, the trait panel, the skill-trainer window, the crafting-trainer window
   and both states of the crafting panel. Each is captured from this scenario's end state or
   from a state of it reached in-process on `HeadlessDriver` when the scenario reaches that
   panel; a panel or state the scenario does not reach (for example the trait panel when the
   train leg uses a trainer window, the crafting trainer when the scenario has no
   `craft-trainer` step, or state (b) of the crafting panel) is captured from an in-process
   state built on `HeadlessDriver` with literal refs and the same feed forms. So the goldens
   and the scenario share one presentation path even where the scenario does not reach a
   panel.

## Consequences
- Criterion 1 has a fixed, countable meaning that a real-data test can check on any install;
  the exit verifier compares the counts with those of the two specs on the owner's install.
- The scenario feed needs the "P6 progression" forms (`scenario-feed.md` §6.6, §18.P6); a feed
  that lacks the level-up, trainer, trait or craft forms cannot pass criterion 2.
- `tessera-ui` gets committed synthetic goldens of the four P6 panels; their images stay small
  (decision 0031 tier 1, under 1 MiB each). The P6 tier-2 goldens live in the same local
  directory scheme as P2's, P3's and P5's; a patch of the install starts a new set.
- `tessera-server` gains nothing: it runs no gameplay rules and links no data crate
  (`server-content.md` §7, `tools/check-server-deps.sh`); no protocol minor is needed.
- This record stands on decision 0031. If the owner changes 0031, the P6 panels follow the
  changed rules; if the owner rejects it, §2 and item 5 of §3 fall with it and §1 and items 1–4
  of §3 stand alone.
- Comparison with the original client stays with the owner
  ([`owner-observations.md`](../checklists/owner-observations.md#p6-items), rows from O160)
  and is not part of any P6 criterion.

## Open
- The owner accepts this record or asks for changes.
- `progression-panels.md` is now in the tree and Accepted (2026-10-16), and the trait panel and
  census were conformed to it (fb83a61), so the trait half of criterion 1 is no longer held.
  Still to do: this record cites its census section and category names explicitly (as it does
  for `crafting-panels.md` §10.1) and its fallback and layout sections, and the owner rows
  O160–O165 and O175 get its section numbers.
- `crafting-panels.md` and the "P6 progression" revision of `scenario-feed.md` are Accepted
  (independent review, 2026-10-16).
- No committed golden shows the chat window, so the craft result lines and the level-up chat
  lines are protected only by the scenario log and unit tests; a chat-window golden (P6 or a
  later milestone) would cover them. The crafting bar and the gathering bar likewise have no P6
  golden.
- The recipe page's facility line has no tier-1 golden (literal feed recipes name no facility);
  whether the tier-2 sample's rule should add "the first recipe with a required facility" is
  left to the owner.
- `combat-data.md` §8.5 OPEN: which of the two numbers of a cell key is the row and which the
  column. Until it is settled, a trait-tree page's cell positions rest on the progression spec's
  rule, and the tier-2 trait-panel golden is judged against that rule, not against the original.
