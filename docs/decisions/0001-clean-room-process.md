<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0001 — Clean-room split: research → specs → code

**Status:** Accepted, 2026-10-03

## Context
Almost all published LOTRO tooling is unlicensed (all rights reserved) or GPL/AGPL. Functional
facts about file formats and protocols are distinct from source code implementation.

## Decision
Two zones (details in `docs/CLEAN_ROOM.md` and `CLAUDE.md`):
- **Dirty room** — `research/`, third-party clones (kept outside the repo, see
  `tools/fetch-research-clones.sh`), probes of the user's data files. Output: prose specs only.
- **Clean room** — `crates/`, `apps/`, `fuzz/`, `tools/`. Implemented from `docs/specs/` only.

Claude sessions follow the same split: spec-writing agents may read research material,
implementing agents may not (`.claude/agents/`, `.claude/workflows/tessera-milestone.js`).

## Consequences
Commits implementing formats carry `Spec-Ref:` trailers. Missing facts become
`> **OPEN:**` notes in specs rather than look-ups in dirty material.
