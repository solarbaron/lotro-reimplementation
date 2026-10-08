<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0021 — Incident: spec writers' reports in the milestone exit verifier's prompt

**Status:** Accepted as an incident record, 2026-10-08. Recorded under `docs/CLEAN_ROOM.md` §10.
This is the same kind of exposure as decision 0018, found in a different place.

## Context
The reusable milestone workflow (`.claude/workflows/tessera-milestone.js`) ends with an exit
verifier. That verifier is a clean-room session: it checks the exit criteria and may fix code,
tests and tools.

Its prompt carried a summary of every node in the milestone plan. For spec nodes, that summary
was the text returned by the dirty-room spec writer: a report on what the writer confirmed and
corrected, sometimes from the disassembled client (decision 0011).

Decision 0018 had judged this workflow "not affected" because its implementer prompts are
clean. The implementer prompts are clean: they carry only the planner's task, the spec paths
and dependency status. The exit prompt was missed. The M5 exit verifier recognised the pattern
on 2026-10-08, limited itself to mechanical changes and reported it.

## Exposed sessions and what they changed
| Run | Spec reports in the prompt | Code the verifier changed |
|---|---|---|
| M4 (2026-10-05) | 4 | client app/cli, `crates/formats/src/item.rs` (stowed medium shield), `crates/mesh` assembly and its tests, `crates/render/src/renderer.rs`, a fuzz target, tools |
| M4.5 (2026-10-06) | 2 | walker terrain, objects, frills, jobs, mod, framelog; a test; `walker.md` text |
| M6 (2026-10-07) | 1 (`script-runtime`) | Vorbis setup-header check and placeholder codebook (`crates/audio/src/decode`), a test retry, fuzz targets, CI and setup wording |
| M7 (2026-10-07) | 2 (own protocol design, no client facts) | shared network thread in `crates/net/src/remote`, load-tool batch size, server tick schedule |
| M5 (2026-10-08, uncommitted) | 5 | mechanical only: renames for neutral names, lint, formatting, doc links |

## Decision
1. **Workflow fixed (2026-10-08):** the exit verifier now receives only each spec node's key,
   files and a pointer to read the specs themselves. It no longer receives the writer's text.
   Implementation results still reach it, because they come from clean sessions.
2. **Remedy, following owner decision 0018 B:** a clean-room auditor that was not exposed
   checks each verifier's code changes in the table above against the Accepted spec text and
   general engineering knowledge. Anything that goes beyond them is either moved into a spec
   through the spec lifecycle and re-implemented by an unexposed session, or removed. Where the
   audit finds nothing, the code is kept. The M5 changes are mechanical and are audited before
   the M5 commit.
3. **Later work:** much of the M4.5 walker code changed by that verifier has since been
   reviewed and changed by unexposed clean sessions (remediation rounds 1–3 after 0018). The
   audit records which of the verifier's lines are still present.

## Consequences
- The verifier sessions are tainted for the subsystems their prompts described. They were
  transient and are not reused.
- The audit results are appended to this record.
