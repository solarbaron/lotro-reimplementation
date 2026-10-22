<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0022 — Bundle Noto Sans as the UI fallback font

**Status:** Accepted, 2026-10-08 (owner decision).

## Context
The M5 UI (`crates/ui`) needs a font that works without the game's own fonts, for example in
tests, before an install is found, or for Tessera's own text. M5 bundled
`crates/ui/assets/fonts/NotoSans-Regular.ttf`.

Noto Sans is published under the SIL Open Font License 1.1, which allows bundling with GPL
software. The repository rules (`REUSE.toml`, `tools/check-content.sh`) forbid committing
third-party material unless an exception is recorded, so `check-content.sh` refused the file.

The options put to the owner:
- load a system font;
- use only the game's fonts;
- bundle Noto Sans.

## Decision
Bundle Noto Sans Regular, unmodified, as the UI fallback font.
- `LICENSES/OFL-1.1.txt` holds the licence text.
- A REUSE annotation in `REUSE.toml` covers the font file: copyright "The Noto Project
  Authors", licence `OFL-1.1`.
- `tools/content-allowlist.txt` allows exactly that path.
- `CLAUDE.md` and `crates/ui/assets/fonts/README.md` record the exception and the font's
  licence. Game fonts are still never committed; those are loaded from the user's install
  (`fonts.md`).
- The font is not renamed and not modified, so the OFL's Reserved Font Name rules do not
  apply.

## Consequences
- The UI always has a usable font.
- This is the only third-party binary asset in the repository. Any other one needs its own
  decision.
