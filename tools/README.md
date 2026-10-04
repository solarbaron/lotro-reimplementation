<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# tools/

Clean-room zone. Everything here is written from `docs/specs/` and general knowledge, like
the code in `crates/`. None of it may contain game data.

| Path | What it does | Needs |
|---|---|---|
| `check-content.sh` | Forbidden-content guard: game-data/capture/disassembler extensions, magic numbers (archive, DDS, Ogg, WAVE, Bink, fonts, PE, pcap), binary blobs > 1 MiB, Git LFS, trademark words in identifiers / file names / Cargo names under `crates/ apps/ fuzz/`, decompiler auto-names. `--self-test` plants violations and checks each is caught. | bash, python3 |
| `content-allowlist.txt` | Reviewed exceptions for `check-content.sh`, each with a reason. | |
| `check-spdx.sh` | SPDX header check: error for source files, warning for `.toml`/`.md` (`--strict` makes those errors). | bash, git |
| `check-dco.sh <base> [<head>]`, `check-dco.sh --all` | `Signed-off-by` matching the author on every commit (error); `Clean-Room-Role` and `Spec-Ref` reminders (warnings). `<base>` = `origin/main` or `@{push}` checks unpushed commits only; `--all` checks the whole history, root commit included. Commits up to the baseline in the script (eb01347) only get the sign-off check. | bash, git, GNU sed |
| `fuzz-smoke.sh [-t SECS] [TARGET...]` | Short run of every fuzz target (or only the named ones); uses cargo-fuzz + nightly when present, otherwise builds instrumented targets on stable. Build output and logs go under the configured cargo target dir. | cargo, a C++ compiler |
| `dev-setup.sh [--fix] [--fetch-research]` | Checks a machine (toolchain, git identity, game install, build cache and its free space, optional tools). `--fix` writes `.cargo/config.toml` from `TESSERA_TARGET_DIR`; `--fetch-research` downloads the dirty-room clones (about 6 GB). | bash |
| `fetch-research-clones.sh [DEST]` | Shallow dirty-room clones from `research/clones.txt` into `${TESSERA_CLONES_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/clones}`, never inside the repo. Spec writers only. | bash, git |
| `imhex/tessera_dat_container.hexpat` | ImHex pattern for the archive container: header, directory node, free-space table (`docs/specs/dat-container.md`). Settings: `inspect_node` (offset of another node), `show_free`. | ImHex (tested with 1.38.1) |
| `kaitai/tessera_dat_container.ksy` | Kaitai Struct description of the same container, including lazy B-tree descent through child pointers. | kaitai-struct-compiler (tested with 0.11) |

## Validating the format descriptions

Both descriptions are independent renderings of `docs/specs/dat-container.md`. When one of
them disagrees with `crates/dat` on the same bytes, the spec is ambiguous or one of the three
is wrong; file an `> **OPEN:**` question in the spec.

```sh
# Kaitai: compile (any target language works as a syntax and type check)
kaitai-struct-compiler -t python --outdir /tmp/ksy-out tools/kaitai/tessera_dat_container.ksy

# ImHex, headless (the AppImage and distro builds include the pattern-language CLI)
imhex --pl format -I /usr/share/imhex/includes \
      -i <archive> -p tools/imhex/tessera_dat_container.hexpat -o /tmp/out.json
```

Run them on synthetic archives or, read-only, on your own install. Output produced from real
archives is dirty-room material: keep it out of the repository.
