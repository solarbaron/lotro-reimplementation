<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# research/ (dirty room)

**Spec writers only.** Implementers, reviewers of clean-room code and clean-room auditors
must not open anything in this directory ([CLAUDE.md](../CLAUDE.md),
[docs/CLEAN_ROOM.md](../docs/CLEAN_ROOM.md)). Facts reach the code only through
`docs/specs/`.

| Path | Contents |
|---|---|
| `notes/*.md` | Research notes per topic (sources, observations, open questions) |
| `resources.json` | Verified catalogue of external resources (URL, licence, what it is good for) |
| `clones.txt` | Third-party repositories studied, `<local-dir-name> <git-url>` |

## Where cited paths live on a machine

The notes were written on the primary workstation. Paths in them mean:

| Written as | Means | How to re-create it |
|---|---|---|
| `~/.cache/tessera/clones/<name>` | a research clone | `tools/fetch-research-clones.sh` (or `tools/dev-setup.sh --fetch-research`); the directory is `$TESSERA_CLONES_DIR`, else `${XDG_CACHE_HOME:-~/.cache}/tessera/clones`. `<name>` is the first column of `clones.txt` |
| `$TESSERA_RE_DIR/...` | dirty-room data root: downloads, decompiled jars, legal texts, captures, Ghidra projects ([docs/TOOLING.md §11](../docs/TOOLING.md)) | not kept in git; see below |
| `~/nvme/...` | the primary workstation's second disk | machine-specific; elsewhere use `$TESSERA_RE_DIR` and your own install path |
| `<local RE lab dir>`, "(a web download, not kept)" | material that existed only on the original machine | re-fetch from the source URL cited next to it in the note or in `resources.json` |

Clone names used in the notes that differ from `clones.txt`:

| Name in the notes | Directory per `clones.txt` |
|---|---|
| `mer-lotro-server` | `Middle-earth-Revenge_lotro-server` (github.com/Middle-earth-Revenge/lotro-server); not `lotro-server`, which is a different (GitLab) project |
| `lc` | the `lc_*` LotroCompanion repositories (`lc_lotro-companion-doc`, `lc_lotro-data-legacy`, ...) and `lotro-companion` |
| `lotro-maps` | `lotro-maps` |
| `lotrolinux` | not a git repository: the LotROLinux source tarballs (below) |

## Material that `fetch-research-clones.sh` cannot re-create

Download these by hand into `$TESSERA_RE_DIR` when a spec task needs them; never into the
repository:

- **LotROLinux source tarballs**: https://sourceforge.net/projects/lotrolauncher/ (unpack to
  `$TESSERA_RE_DIR/lotrolinux/`).
- **LotroCompanion binary distribution** (the notes cite its data-utilities jar, decompiled,
  and its bundled resources): https://sourceforge.net/projects/lotrocompanion/files/ or the
  `verify_lcdistrib_2248893` clone. Extract the jar that contains the data-file utilities
  and decompile it with a Java decompiler (e.g. Vineflower or CFR) into
  `$TESSERA_RE_DIR/jars/datutils-src/`; unpack its resources into `$TESSERA_RE_DIR/jars/res/`.
  Decompiled output is dirty material: facts only into specs, never code.
- **Legal source texts** (`notes/legal.md`): re-download from the URLs in that note's
  source list into `$TESSERA_RE_DIR/legal/`; the `dmca` clone covers the GitHub DMCA notices.
- **Web pages saved during research** (`$TESSERA_RE_DIR/web/...`): re-fetch from the URL
  cited next to each.
