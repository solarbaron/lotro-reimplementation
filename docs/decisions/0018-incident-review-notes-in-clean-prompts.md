<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0018 — Incident: dirty-room reports in clean-room implementer prompts

**Status:** Accepted as an incident record, 2026-10-07. Recorded under `docs/CLEAN_ROOM.md` §10.
The owner decided both open points on 2026-10-07 (see "Owner decisions" below).

## Context
On 2026-10-07 the orchestrating session ran a hand-written workflow, "M4.5 close-out". Its
prompt for each clean-room implementer contained two things besides the task:
- the summaries returned by the dirty-room spec writers;
- the findings of the dirty-room spec reviewers.

Those reports described client behaviour read from the disassembled client (decision 0011)
for scenery generation, collision and movement, water, landblock cells, lighting and terrain
texturing. Some of those facts were not yet in any spec, or appeared only as requested
corrections. This breaks `CLEAN_ROOM.md` §7.2 and §6: implementers work only from
`docs/specs/`, and dirty-room output reaches them only through the spec lifecycle (§5).

The exposed implementer sessions recognised the problem and recorded it. Their draft record
is preserved with the held material (below).
- Most of them stopped before writing code.
- The first "world-scenery" run did not stop. Its code follows the review notes instead of
  the spec body in at least one place (failure handling under a frill layer).
- The first runs of "physics", "render", "client-walker" and "streaming" also wrote code
  under such prompts.
- The exit verifier received a reviewer report too, but implemented nothing.

**Not affected:**
- The reusable milestone workflow (`.claude/workflows/tessera-milestone.js`). Its
  implementer prompts contain only the planner's task, the spec paths and dependency status,
  so the M4.5 milestone run (`07ec7b2`) and the M5, M6 and M7 runs did not receive such
  reports.
- The audit-fix workflows. Their findings came from clean-room auditors.

**Earlier, smaller exposure of the same kind:** the River-hobbit workflow of 2026-10-05.
- Its implementer prompt contained the dirty-room spec writer's facts-only summary of the
  new rules, together with the list of the spec's remaining OPENs. The summary restated what
  the writer had put into `script-table.md`, `skeleton-anim.md` and `appearance.md`, but it
  reached the implementer before those revisions were reviewed.
- The resulting code is committed in `9d5eaed`.

## Decision
1. **Hold.** None of the close-out implementation is committed. Every uncommitted change
   written by the exposed sessions was saved as a patch and removed from the working tree:
   - the code in `crates/physics`, `crates/render` and `apps/client`;
   - in `crates/world`: `scenery/`, `frills/`, `streamer.rs`, `bundle.rs` and their tests;
   - the related fuzz targets;
   - the design sections those sessions added to `walker.md`, `render.md` and
     `ARCHITECTURE.md`;
   - the status text measured on that code (`ROADMAP.md`, `README.md`, `CLAUDE.md`,
     `docs/perf/reference-machine.md`).

   The patch lives in the dirty-room area of the owner's machine, under `$TESSERA_RE_DIR`,
   in `held/2026-10-07-closeout/`. It is not in the repository, and no clean-room session may
   open it.
2. **Pending owner decision A.** Discard the held material and have fresh, unexposed
   sessions re-implement the close-out from the specs once the revised specs are Accepted.
   This is `CLEAN_ROOM.md` §10 steps 3–4, and it is the recommended option.
3. **Pending owner decision B.** For `9d5eaed`, either:
   - (a) revert it forward and re-implement it from the Accepted specs in an unexposed
     session (strict reading of §10, recommended); or
   - (b) have a clean-room auditor confirm that the code implements nothing beyond the
     Accepted spec text, and keep it if so.
4. **Process rules, effective immediately:**
   - Orchestrators never put dirty-room session output into a clean-room prompt. That covers
     writer summaries, reviewer findings, OPEN lists copied from a report, and scratch paths.
     A clean prompt contains only the task, the spec paths, their review status, and
     findings from clean-room sessions.
   - A spec's header review record stays short: reviewer, date, result. Detailed review
     findings that cite the binary go to the dirty-room area, not into `docs/specs/`.
   - Implementers build only from Accepted specs (`CLEAN_ROOM.md` §5.2).

## Consequences
- M4.5's close-out items stay open until the re-implementation:
  - criterion 4 (walks), criterion 6 (rows S, Y5, F1, L5) and criterion 9;
  - streaming latency, interior cells, layer-less blocks, generated-scenery collision,
    steep slopes.
- The specs revised during the close-out need their reviews finished, and their long header
  review records moved out of `docs/specs/`:
  - `scenery.md`, `collision.md`, `water.md`, `landblock.md`, `lighting.md`,
    `terrain-texturing.md`;
  - the other specs whose headers hold detailed findings.
- The exposed sessions are tainted for those subsystems. They were transient and are not
  reused.

## Owner decisions (2026-10-07)
- **A: release.** The owner chose to release the held close-out work, against the
  recommendation to discard it and re-implement. The held patch was re-applied and
  committed. To bring code and specs back into line:
  - dirty-room spec writers add to the spec bodies every fact the released code relies on,
    and the specs go through review to Accepted (`scenery.md`, `collision.md`,
    `landblock.md`, `lighting.md`, `terrain-texturing.md`);
  - from then on, only fresh, unexposed clean-room sessions review and change that code,
    and they work from the Accepted specs alone.
- **B: audit `9d5eaed`.** A clean-room auditor checks that the River-hobbit code implements
  nothing beyond the Accepted text of `script-table.md`, `skeleton-anim.md` and
  `appearance.md`. The code is kept if it is clean. Anything that goes beyond the specs is
  either added to them through the spec lifecycle or removed.
- The process rules above stay in force.


## Correction: how the release landed in git (2026-10-07)
Applying the held patch with a three-way merge staged most of it in the index. As a result,
the released close-out work is split across two commits, and the first one is mislabelled:
- **`ecc814b`:** its message describes only the owner-checklist update and carries
  `Clean-Room-Role: spec`. It also contains 43 files of the released close-out:
  - code in `crates/physics`, `crates/render`, `crates/world` and `apps/client`, plus fuzz
    targets;
  - the `walker.md` and `render.md` design sections;
  - `ROADMAP.md`, `README.md`, `CLAUDE.md`, `ARCHITECTURE.md` and
    `docs/perf/reference-machine.md`.
- **`6607d43`:** contains the remaining new files and the one-line lint fix.

The tree after `6607d43` is the one the full checks ran on. Published history is not
rewritten (`docs/CLEAN_ROOM.md` §10 step 3), so this note stands as the correct description
of `ecc814b`.
