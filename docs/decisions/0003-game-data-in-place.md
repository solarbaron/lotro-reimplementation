# 0003 — Read the user's install in place, read-only; no game data in the repo

**Status:** Accepted, 2026-10-03

## Decision
- Tessera reads `client_*.dat`/`.datx` from the user's own install, never copies, repacks or
  modifies them. Location: `TESSERA_GAME_DIR`, CLI flag, saved setting, or auto-discovery.
- The repo contains no game data: no archives, extracted assets, strings, screenshots of game
  content, binaries or keys. `tools/check-content.sh` enforces this in CI.
- Tests use synthetic fixtures; real-data tests are skipped unless `TESSERA_GAME_DIR` is set.

## Consequences
Machines without an install (CI, Claude Code on the web) can build and run all synthetic
tests but cannot verify real-data or visual criteria.
