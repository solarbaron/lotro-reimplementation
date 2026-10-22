<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0006 — Working codename "Tessera"; naming rules

**Status:** Accepted, 2026-10-03

## Decision
"Tessera" is a placeholder codename. Product, crate, repo and UI names must not use or allude
to the game's, Tolkien's, or the rights holders' marks (see `CLAUDE.md` Naming rules).
Plain-text compatibility statements ("reads data files from The Lord of the Rings Online™")
are fine.

## Consequences
The repository directory name on the owner's machines is not a product name; do not reuse it
in code or UI.

> **OPEN (owner decision, noted 2026-10-04):** the private GitHub repository that every
> machine clones (`solarbaron/lotro-reimplementation`, [DEVELOPMENT.md](../DEVELOPMENT.md))
> uses the game's mark in its name, which the rule above forbids for repo names. While the
> repository stays private this is a working name like the local directory name, but it
> must not become public under that name. Before any public visibility, either rename the
> GitHub repository (e.g. `tessera`; update the clone URL in `docs/DEVELOPMENT.md` and
> `repository` in `Cargo.toml`) or amend this record with a recorded exception.
