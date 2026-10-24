<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0043 — P8: what the parity audit's exit criteria mean

**Status:** Proposed, 2026-10-23 (waits for the owner); revised the same day after review (row state `differs`, owner rows renumbered O196–O200, criterion 3's run, budgets and scenario minimums, distinct tooltip sample).

The number 0043 follows [0042](0042-m8-remediation-decisions.md).

## Context
[Decision 0024](0024-parity-track.md#p8-parity-audit) §5 sets three exit criteria for P8
(parity audit), the last milestone of the parity track (P7 was dropped by the owner on
2026-10-16, so P8 depends on P1–P6):

1. `docs/checklists/parity.md`: every row passes or has an owner-accepted deviation
   (tooltips of 50 items and 50 skills included).
2. Every new parser has a fuzz target.
3. Frame-time budgets hold in the busiest scenario on the reference machine
   (`TESSERA_PERF=1`, decision 0010).

None of the three says enough to be checked. Criterion 1 does not say who writes the checklist,
which states a row can have, who may set them, or which 50 items and 50 skills are meant.
Criterion 2 does not say what a "parser" is, what "new" counts from, or how a reviewer can tell
that none is missing. Criterion 3 does not say which scenario is "the busiest", where and how
it runs, or which budgets are meant.

The milestones before P8 give the pieces this record builds on. P0–P6 each leave owner rows in
[`owner-observations.md`](../checklists/owner-observations.md) that "block nothing until P8"
(decision 0024 §2 principle 6). [Decision 0031](0031-parity-panel-goldens.md) (Accepted) gives
the local-only tier-2 directory for real-data images. The scenario feed
([`scenario-feed.md`](../specs/scenario-feed.md) §8) chooses install records by seeded
selectors, so a committed synthetic scenario can name a sample without naming a DID. The
frame-time and UI budgets already exist: M3 criterion 2, M4 criterion 4 and M4.5 criterion 5
([ROADMAP](../ROADMAP.md#m3), [M4.5](../ROADMAP.md#m4-5-exit)), the UI budgets U1–U5 of
[decision 0015](0015-ui-rendering.md) §6 and M6 criterion 4 (no audio underruns), all judged on
the reference machine of [decision 0010](0010-reference-machine-laptop.md) under the rules of
[`reference-machine.md`](../perf/reference-machine.md).

## Decision

### 1. Criterion 1: the parity checklist
1. **One checklist.** `docs/checklists/parity.md` is the only checklist criterion 1 is judged
   on. It lists every client system of the parity track (and the systems of M1–M7 the client
   shows), one row per thing the owner can compare in the original client. It is written by
   the dirty-room spec package (spec writer role) from the install's UI layouts and the
   Accepted specs, and it is **independently reviewed** (CLEAN_ROOM.md §5.3) before it counts
   for criterion 1. Its rows cite the specs and the existing owner rows they cover; they hold
   no game text (the `owner-observations.md` "Rules for recording" apply).
2. **Row states.** Each row has exactly one of these states:

   | State | Meaning | Who sets it |
   |---|---|---|
   | `pending` | Not yet compared | The spec writer (initial state) |
   | `pass` | The owner compared it in the original client and it matches | The owner only |
   | `deviation (proposed)` | Tessera differs on purpose, or cannot match; the row names the reason and the decision or spec rule | The spec writer, from a spec's Tessera rule, from a `differs` row whose difference is a Tessera rule or cannot be matched, or from a proposal an implementer records in `parity-tessera.md` |
   | `deviation (accepted by owner, DATE, REASON)` | The owner accepted the difference | The owner only |
   | `differs (owner, DATE, NOTE)` | The owner compared it in the original client and it does not match; NOTE says in the owner's words what differs, or that the owner rejected a proposed deviation and what Tessera should do instead | The owner only |
   | `not built` | Tessera does not have the system or part yet | The spec writer, from the Tessera-side evidence |

   Only the owner sets `pass`, `differs` or an accepted deviation. A `not built` row cannot
   pass: it is built and compared, or it becomes a proposed deviation that the owner accepts.
   A `differs` row leaves that state only by one of two moves: the spec writer turns it into
   `deviation (proposed)` when the difference is a Tessera rule or cannot be matched (the
   owner may say so in NOTE), or, once Tessera has been fixed and the fix is recorded in
   `parity-tessera.md`, the spec writer sets it back to `pending` for a new comparison. The
   defect itself goes to the roadmap. `docs/checklists/parity.md` §1.2 (Accepted 2026-10-23,
   written before this state was added) keeps such a row `pending` with a log note; its spec
   writer adds `differs` in the next revision, and until then the exit verifier counts a
   `pending` row with an owner log note of a difference as `differs`.
3. **Tessera-side evidence.** Agents never edit a row's state to `pass`. They record, per
   row ID of `parity.md`, the Tessera-side evidence in `docs/checklists/parity-tessera.md`:
   what is built (built, partly built, not built), the tests, scenarios, goldens and commands
   that show it, the commit and date, and the role that recorded it. The owner reads it
   before comparing a row; it decides nothing by itself.
4. **Deviations the owner has already accepted.** `parity.md` lists these as
   `deviation (accepted by owner, …)` with the date and reason of the owner's decision, and
   they need no new answer:
   - Tessera's own UI scaling instead of the original's: any resolution and aspect ratio, a
     user scale slider, crisp text and art, panels kept on screen
     ([decision 0040](0040-ui-scaling.md), owner 2026-10-16, answers 2026-10-23).
   - P7 (client-run game code) dropped: Tessera runs none of the class-library code the
     original client runs locally; with M8 the official server runs that code
     ([decision 0024](0024-parity-track.md) amendment, owner 2026-10-16).
   - The world walker's debug keys moved off the five keys that the `keymap.md` §8.2 defaults
     take (owner, 2026-10-16; `keymap.md` §8.2 and §8.3.1).
5. **Proposed deviations named here.** Unless the owner has accepted them as deviations by
   the time `parity.md` is written, these are listed as `deviation (proposed)`, each with its
   decision as the reason. Both decisions are Accepted as project decisions; neither record
   says that the owner accepted the resulting difference from the original client:
   - [Decision 0016](0016-plugin-class-helper.md): Tessera ships its own clean-room plugin
     `class` helper (the user's installed original package wins when present), so plugins that
     rely on behaviour outside the helper's spec may differ.
   - [Decision 0022](0022-bundled-ui-font.md): Tessera bundles Noto Sans as its UI fallback
     font, so text that falls back (Tessera's own text, text before an install is found) looks
     different from the original.

   Other proposed deviations (Tessera rules in the specs that knowingly differ from the
   original) are added row by row by the spec package, each with its reason; the owner
   answers all of them in one owner row (O199).
6. **The tooltip sample.** The "50 items and 50 skills" are chosen at run time from the
   install, never listed in the repository:
   - The committed synthetic scenario `apps/client/tests/scenarios/parity-tooltips.scenario`
     holds seeded `select` lines of the `item` and `skill` families (`scenario-feed.md` §8.2,
     §8.3, §8.5), with selectors only and no DID (`scenario-feed.md` §13). Its lines spread the
     item sample over the item kinds (the Tessera-defined keywords of `item kind=…`) and the
     skill sample over the skill family's yes/no attributes (`harmful=`, `induction=`,
     `toggle=`, `mount=`, `monster=`). Enum labels (for example the class mapper's labels of
     `skill class=@…`) appear in the file only where an Accepted spec already names them
     (`scenario-feed.md` §13 rule 2); no Accepted spec names the class mapper's labels today,
     so a spread over classes waits for the spec writer (Open, below). The scenario file is the
     authority for the exact lines, seeds and counts; this record does not repeat them.
   - **Distinct records.** Selector lines may overlap (one skill can be listed by several
     class tables, `combat-data.md` §9.1, and yes/no attributes overlap by nature), and a line
     whose candidates are fewer than its `n` gives all of them (`scenario-feed.md` §8.5). So
     the sample is counted in distinct DIDs per family: the records are taken in the
     scenario's line order and, within a line, in the selector's canonical order (§8.4); a DID
     an earlier line already gave is skipped; the first 50 distinct items and the first 50
     distinct skills are the sample, with ordinals 1–50 in that order. The scenario draws
     more than 50 per family so that overlaps leave enough; the test fails when fewer than 50
     distinct items or 50 distinct skills resolve. The index lists each DID once.
   - The real-data test `apps/client/tests/parity_tooltips_real_data.rs` resolves the sample
     against `TESSERA_GAME_DIR`, paints each tooltip with the client's UI host (the same path
     the client uses, at *Se* = 1, `ui-scaling.md` §4.3) and writes the images into the decision-0031 tier-2 local
     directory (`${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/…/<install
     version>/`), with a local index of ordinal, DID and name. The images and the index are
     never committed (decision 0031; the index holds game text). The test skips, reporting
     passed, when `TESSERA_GAME_DIR` is unset.
   - The sample depends on the install version: a patch can change the candidates, so the
     owner compares the sample of the install version the index directory names. The test
     asserts that the sample holds exactly 50 distinct items and 50 distinct skills and that
     each was painted without a failure; a record whose tooltip is categorised (P1 criterion 2, P2 criterion 2) is
     painted with its category and noted in the index.
   - The comparison in the original client is the owner's (O197 items, O198 skills); the
     tooltip rows of `parity.md` pass when the owner sets them.
7. **Earlier owner rows.** The owner rows of M1–P6 that compare Tessera with the original
   client are cited by the `parity.md` rows that cover them. An owner row is closed for P8
   when the parity row that cites it is `pass` or an accepted deviation. Owner rows that are
   not comparisons with the original (decisions, re-measurements on the reference machine)
   stay with their milestone and are not parity rows.
8. **Pass.** Criterion 1 passes when `parity.md` is Accepted after an independent review and
   every row is `pass` or `deviation (accepted by owner, …)`. No agent can pass it; the exit
   verifier reports the count of rows per state, `differs` included (so rows the owner
   compared and found different are never counted with rows nobody has compared).

### 2. Criterion 2: every new parser has a fuzz target
1. **Parser.** A parser is every public entry point in `crates/` and `apps/` that decodes
   bytes or text that Tessera did not write itself in the same process. That covers:
   - install records and files (archives, records, Havok data, string tables, UI layouts,
     `UserPreferences.ini`, Steam library files read for detection);
   - network messages (Tessera's own protocol, the test-server backend);
   - user or third-party files: settings, the keymap file, skins and UI XML, plugin
     descriptors and plugin Lua data, ABC files, scenario files and their input scripts, and
     logs read back (for example committed scenario logs compared on replay).

   A file Tessera wrote in an earlier run counts (the user can edit it). In-memory data that
   the same process produced and reads back does not. Third-party crates' own parsers (zlib,
   PNG, QUIC and TLS, the Lua compiler) are not Tessera's parsers; the Tessera entry point
   that hands hostile input to them is, and its fuzz target covers that hand-off.
2. **New.** "New" means every parser since M0, not only those of the P-track.
3. **Target or exemption.** Each parser has a fuzz target in `fuzz/` (a `[[bin]]` of
   `fuzz/Cargo.toml`), or a recorded exemption that names the target that covers it (for
   example a thin wrapper whose whole input reaches a fuzzed inner parser) and why.
4. **Manifests.** The coverage is recorded in `fuzz/coverage/*.toml`: for each crate and app,
   its parsers (entry point and input kind) with the target that fuzzes them, or the exemption
   with the covering target and the reason. `tools/check-fuzz-coverage.sh` checks them in CI.
   At least it fails when a crate or app has no manifest, when a named target is not a
   `[[bin]]` of `fuzz/Cargo.toml`, when an exemption names no covering target, and when a
   public decoding entry point it finds in the sources is in no manifest. How it finds them
   and the exact manifest schema are engineering choices of the package that builds the
   checker; they are written in the manifests' header and `tools/README.md`.
5. **Outside P8.** `crates/compat` is not part of criterion 2: it is M8 (decisions
   [0034](0034-official-server-compatibility.md) and [0042](0042-m8-remediation-decisions.md))
   and is audited by the M8 package. Its fuzz targets stay where they are.
6. **Pass.** Criterion 2 passes when the checker passes in CI on the exit commit, every named
   target builds, and every target added in P8 runs 60 s clean in `tools/fuzz-smoke.sh`.

### 3. Criterion 3: frame-time budgets in the busiest scenario
1. **The busiest scenario.** It is the committed synthetic
   `apps/client/tests/scenarios/busiest.scenario`. Within the feed's limits
   (`scenario-feed.md` §11.1) it plays at once, at least:
   - a crowd of at least 50 feed actors with figures around the player (the crowd of M4
     criterion 4);
   - combat: a selection and the target frame, casts and inductions, effects with icons and
     particles, floating text, and a defeat;
   - a full fellowship: the player and five members (`social-panels.md` SP12), with changing
     vitals;
   - chat lines at a steady 10 per second (the rate of decision 0015 §6);
   - the main panels open: bags, character, quest tracker and minimap;
   - synchronised music, if the feed allows it;
   - at least 60 s of this after the settle wait.

   The exact lines, counts above these minimums and timings are the scenario author's; they
   are written in the scenario file and summarised in `docs/perf/p8-busiest.md`.
2. **Where and how.** It runs at one reference location of
   [`m3-reference-locations.md`](../checklists/m3-reference-locations.md), chosen by the perf
   package and named in `docs/perf/p8-busiest.md`, with the M4.5 systems at their `walker.md`
   defaults (§11.8, §14.9), from a release build, as the **windowed** scenario run of
   `scenario-feed.md` §12.4, which already plays the scenario on the wall clock (`now` is the
   frame time) with the walker drawing every frame. The command requests `--size 1920x1080`
   and sets `--ui-scale 1`; the tiling compositor of the reference machine sizes windows
   itself, so the run records the surface it was given (`reference-machine.md`: windowed runs
   record the surface given, not the requested size), and decision 0015 §6 already judges U1–U5
   "at the window size that the compositor gives". It needs no change to `scenario-feed.md`:
   headless scenario runs keep their virtual clock and driver-only mode (§12.2), and
   `--scenario` keeps its conflict with `--duration` (§12.1). The exact command lines (paced and
   unpaced, the frame log and the UI statistics output) are named in `docs/perf/p8-busiest.md`.
   A real-data test checks with `TESSERA_GAME_DIR` that the scenario resolves and plays to its
   end on the install (headless, §12.2), so the measured run stays valid; the budgets of item 3
   are asserted only with `TESSERA_PERF=1`, from the windowed runs.
3. **Budgets.**
   - (a) Paced run (vsync, 60 Hz): 0 frames over 50 ms after the settle wait
     (M3 criterion 2).
   - (b) Unpaced run (`--no-vsync`), the same scenario: frame-time p99 at most 16.7 ms after
     the settle wait (M4.5 criterion 5's 60 fps bar, which also stands for M4 criterion 4).
     A paced run is not judged by p99: its frame intervals sit at the 16.7 ms vsync period, so
     any jitter puts its p99 above it (the paced M3 runs that pass show p99 of 16.7–24.8 ms,
     `reference-machine.md`).
   - (c) The UI budgets U1–U5 of [decision 0015](0015-ui-rendering.md) §6, from the paced
     run, with *Se* = 1 (`--ui-scale 1`, `ui-scaling.md` §4.3), so panels without node scales
     are painted at content scale 1 (§3.4). That section's split of frames applies:
     - **U1 and U3** are judged over **all steady-state frames of the run** after the settle
       wait (every frame outside the event windows and the content-event frames). The run
       has no separate walking phase and no 1000 Hz pointer sweep; those phases stay with
       decision 0015's own scripted run (M5 criterion 7, O49).
     - **U4 (a), (b)** are judged over the event frames the run has (a panel the scenario
       opens, such as the loot or defeat window), and reported "no event frames" when it has
       none; **U4 (f)** over the content-event frames (the chat lines).
     - **U4 (c)–(e)** are **not applicable to this run**: they need resize-bar drags, a plugin
       window resized every frame and a screen resize, which the scenario does not do. They
       stay with decision 0015's M5 run (O49).
     - **U2** is reported as "not measured" where the adapter has no timestamp queries or the
       windowed path does not feed it; **U5** over the whole run.
   - (d) No audio underruns when audio plays (M6 criterion 4).
4. **Judging.** The budgets are judged only with `TESSERA_PERF=1` on the idle reference
   machine (decision 0010), following the rules of `reference-machine.md`. Agent runs on a
   loaded machine are **indicative**: they are recorded in `docs/perf/p8-busiest.md` as such
   and can show a regression but cannot close the criterion. The owner's re-measure is O200.

## Consequences
- Criterion 1 is the owner's: it cannot be passed by an agent or an exit verifier, and P8
  stays open until the owner has walked `parity.md` (O196), compared the tooltip sample (O197,
  O198) and answered the proposed deviations (O199).
- The dirty-room spec package writes `parity.md` and gets it reviewed; clean agents write only
  `parity-tessera.md`. Only the owner sets `pass`, `differs` or an accepted deviation; the spec
  writer sets `pending`, `deviation (proposed)` and `not built` from the specs and the
  evidence, and moves `differs` rows on (§1 item 2).
- The tooltip sample is reproducible on one install version and needs no committed DID or
  text; a patch of the install gives a new sample and a new tier-2 directory.
- Every crate and app (except `crates/compat`) gets a fuzz coverage manifest, and CI fails
  when a parser has neither a target nor an exemption. Parsers added after P8 inherit the rule.
- The client gets a busiest scenario with stated minimums, measured in the existing windowed
  scenario run (wall clock, `scenario-feed.md` §12.4); no client timing mode and no
  `scenario-feed.md` change is needed, and criterion 3 has a fixed, repeatable run for the
  owner's measurement.
- `tessera-server` gains nothing: it runs no gameplay rules and links no data crate
  (`server-content.md` §7, `tools/check-server-deps.sh`).

## Open for the owner
- The owner accepts this record or asks for changes.
- Whether decisions 0016 and 0022 are accepted deviations (O199, with the others the spec
  package proposes).
- Whether owner rows of M1–P6 that no `parity.md` row cites must still be closed before P8
  exits (§1 item 7 says they stay with their milestone).
- U2 of decision 0015 §6 is set for 3840 × 2160; the busiest scenario runs at 1920 × 1080, so
  the same 0.3 ms is a looser bar there. Whether P8 also needs a 3840 × 2160 run of the
  scenario.
- The steady chat rate makes many frames content-event frames, which decision 0015 §6 judges
  by U4 (f) rather than by U1; the steady state of U1 is then the frames between chat lines.
  Whether the owner also wants a walking leg in the run (for example the player walking
  through the crowd), so that U1 sees the minimap scrolling as in decision 0015's run.
- Whether criterion 3 should instead use a headless rendered run on the wall clock at exactly
  1920 × 1080 (as M4.5 criterion 5 does for its walk spots). That run does not exist: headless
  scenario runs use the virtual clock and, with the default spawn, draw nothing
  (`scenario-feed.md` §12.2), and `--scenario` conflicts with `--duration` (§12.1). It would
  need a `scenario-feed.md` §12 revision by its spec writer first, so it is not part of this
  record.
- **For the spec writer of `scenario-feed.md`:** no Accepted spec names the labels of the
  class mapper, so committed scenarios cannot use `skill class=@…` or `trait class=@…`
  (§13 rule 2). Whether an Accepted spec should name them, so that the tooltip sample can be
  spread over classes. Until then the sample is spread over the yes/no attributes (§1 item 6).
  The `skill` family's `class` attribute also reaches only skills that a class table lists
  (§8.3); whether the sample must include skills outside every class table is left to the
  scenario's author and the owner.
- `docs/checklists/parity.md` (Accepted 2026-10-23) predates three changes of this record that
  its spec writer carries into the next revision: the `differs` state (§1 item 2), the owner
  row numbers O196–O200 (it cites O180–O184), and row PF1's budgets (§3 item 3: 0 frames over
  50 ms paced, p99 ≤ 16.7 ms unpaced).
