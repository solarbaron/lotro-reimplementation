# 0007 — Multi-device workflow: git is the only shared state

**Status:** Accepted, 2026-10-04

## Context
The owner works with Claude on more than one machine, never at the same time. Claude's
per-machine memory, session transcripts, scratch directories and build caches do not travel.

## Decision
- Everything a new session needs lives in the repo: `CLAUDE.md` (rules + current state),
  `docs/decisions/`, `docs/ROADMAP.md` status tables, `docs/DEVELOPMENT.md` (machine setup and
  how to run a milestone), `.claude/workflows/tessera-milestone.js`, `.claude/agents/`.
- Third-party research clones are re-created on demand with `tools/fetch-research-clones.sh`
  into `~/.cache/tessera/clones` (never committed).
- Machine-specific settings stay out of git: `.cargo/config.toml` (build cache location),
  `TESSERA_GAME_DIR`.
- Hand-off rule: finish or stop the running milestone, commit, push; on the other machine
  pull before starting. Only one machine works at a time.
- Real-data counts in tests depend on the game version; compare installs with
  `tessera-dattool snapshot` + `diff` when switching machines after a game patch.
