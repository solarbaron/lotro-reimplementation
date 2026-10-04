<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Development guide (any machine)

How to set up a machine, hand work between machines, and continue the roadmap with Claude
Code. Git is the only shared state between machines ([decision 0007](decisions/0007-multi-device-workflow.md)).

## 1. New machine setup

0. **Prerequisites**
   - git with an identity: `git config --global user.name "<name>"` and
     `git config --global user.email "<email>"`. The e-mail must match the
     `Signed-off-by:` line of your commits (`git commit -s`; `tools/check-dco.sh` checks
     it). `tools/dev-setup.sh` stops with "MISSING git identity" without it.
   - An SSH key registered with the GitHub account that can read the private repository
     (the clone URL below is SSH). Test with `ssh -T git@github.com`.
   - python3 (used by `tools/check-content.sh`, `tools/fuzz-smoke.sh` and spec probes).
   - A C/C++ compiler and the usual build tools (some dependencies build C code; Lua is
     vendored).

```sh
git clone git@github.com:solarbaron/lotro-reimplementation.git
cd lotro-reimplementation
tools/dev-setup.sh            # reports what is present/missing; changes nothing
```

1. **Rust toolchain**: with rustup, `rust-toolchain.toml` installs and selects the exact
   pinned version (with `rustfmt` and `clippy`) the first time you run `cargo` in the repo
   (or run `rustup toolchain install` there).
   CI uses the same version. Without rustup (a distribution `rustc`), the file is ignored:
   `tools/dev-setup.sh` warns when your version differs, and a newer or older clippy may
   report lints that CI does not (or the reverse). The MSRV is separate: `rust-version` in
   `Cargo.toml` (1.90; 1.95 for `apps/explorer`).
2. **Build cache location** (optional, machine-local, git-ignored). Debug builds, tests
   and clippy need about 20 GB; with release builds (smoke tests, heavy real-data tests)
   and fuzzing, plan for 25–30 GB. To keep the cache off a small partition:
   `TESSERA_TARGET_DIR=<big disk>/tessera-target tools/dev-setup.sh --fix`. `--fix` only
   writes `.cargo/config.toml`; it never downloads anything. `tools/dev-setup.sh` then
   reports the free space of the disk that holds the cache.
3. **Game install**: install the game (any legal source, e.g. Steam app 212500) and let it
   finish patching. **Real-data tests and the smoke commands below read only
   `TESSERA_GAME_DIR`**; auto-detection serves the apps (`tessera-client`,
   `tessera-explorer`, `tessera-dattool`), not the tests. Use detection to find the path,
   then export it (e.g. in your shell profile):

   ```sh
   export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
   ```

   Without the variable everything builds and all synthetic tests run, but every
   real-data test prints `TESSERA_GAME_DIR not set; skipping` and **counts as passed**.
   To see whether they really ran, this should print 0 on a machine with an install:
   `cargo test --workspace -- --nocapture 2>&1 | grep -cE 'TESSERA_GAME_DIR (not set|unset|is not set)'`.
4. **Optional tools**: `cargo install cargo-deny --locked` (put `~/.cargo/bin` on `PATH`),
   cargo-fuzz + a nightly toolchain, xvfb-run, a screenshot tool (grim/scrot), Mesa
   lavapipe. See `docs/TOOLING.md` for the full list by role.
5. **Research clones** (dirty room only, for writing specs; about 6 GB):
   `tools/dev-setup.sh --fetch-research` or `tools/fetch-research-clones.sh`. They go to
   `$TESSERA_CLONES_DIR`, else `${XDG_CACHE_HOME:-~/.cache}/tessera/clones`; never inside
   the repo. Implementation-only machines do not need them.

### Smoke test

Without game data (any machine; the client and explorer commands open no window):

```sh
cargo test --workspace                                  # synthetic tests; real-data tests skip
cargo run --release -p tessera-client -- --smoke        # offscreen test scene, exits 0
cargo run -q -p tessera-dattool -- detect               # lists installs, or says none found
```

With game data (`TESSERA_GAME_DIR` exported):

```sh
cargo test --workspace                                  # first run: about 30 min, see below
cargo run --release -p tessera-dattool -- validate "$TESSERA_GAME_DIR"
cargo run --release -p tessera-client -- --screenshot /tmp/walker.png   # offscreen walker
xvfb-run -a cargo run --release -p tessera-explorer -- --screenshot /tmp/explorer.png
```

Without options, `tessera-client` (the M3 world walker) and `tessera-explorer` open
interactive windows and run until closed; over SSH or in a Claude session use the
headless forms above (`tessera-client --smoke --smoke-out x.ppm`,
`--exit-after-frames N` under `xvfb-run`, `tessera-explorer --screenshot`).

**Expected durations.** The first `cargo test --workspace` with game data takes about half
an hour in a debug build on a 24-thread machine (about 12 minutes of it compiling); some
real-data binaries run for 2–3 minutes and print "has been running for over 60 seconds",
which is not a hang. `cargo test --workspace --release` is faster for real-data runs.

**Render tests.** Each GPU test binary uses the GPU one test at a time (a lock in
`crates/render/tests/support/gpu.rs`) and every GPU wait is bounded (30 s,
`tessera_render::GPU_WAIT_TIMEOUT`). If a render test still hangs or reports a lost
device on your driver, run `cargo test -p tessera-render -- --test-threads=1` and report
it. `TESSERA_REQUIRE_GPU=1` makes a missing adapter a failure (CI); a machine that has an
adapter but cannot create a device fails the GPU tests unless `TESSERA_REQUIRE_GPU=0`.

## 2. Handing over between machines

Only one machine works at a time.

**Before leaving a machine** (the same checks CI runs, see the header of
`.github/workflows/ci.yml`)
1. Let any running milestone workflow finish, or stop it.
2. Run:
   ```sh
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace                     # with TESSERA_GAME_DIR exported
   (cd fuzz && cargo fmt --all -- --check && cargo clippy --bins --locked -- -D warnings)
   cargo deny check                           # and: cargo deny --manifest-path fuzz/Cargo.toml check
   tools/check-content.sh
   tools/check-spdx.sh                        # 0 errors, 0 warnings on a clean tree
   ```
3. Commit with `git commit -s` and the trailers that apply (CONTRIBUTING.md "Commits"):
   `Clean-Room-Role: spec | review | implementation` for any change under `crates/`,
   `apps/`, `fuzz/`, `tools/` or `docs/specs/`; `Spec-Ref:` for format/protocol code;
   `Assisted-by:` for AI assistance. Check with `tools/check-dco.sh origin/main` (it checks
   only the commits not yet pushed; `tools/check-dco.sh --all` checks the whole history).
   Never rewrite pushed history to add trailers.
4. If the game install is used for real-data tests on the next machine, take a snapshot
   of this install and carry it across **by hand** (scp, USB). Never commit it (it is
   derived from game data; `*.idx` is git-ignored):
   ```sh
   mkdir -p ~/.cache/tessera/snapshots
   cargo run --release -p tessera-dattool -- snapshot "$TESSERA_GAME_DIR" \
       -o ~/.cache/tessera/snapshots/"$(hostname)-$(date +%F)".idx
   ```
5. `git push`. If the work is unfinished, say where it stopped in the "Current state"
   section of `CLAUDE.md` and push that too.

**On arrival**
1. `git pull`, then `tools/dev-setup.sh` (and `export TESSERA_GAME_DIR=...`, §1 step 3).
2. If the game patched since the other machine last ran real-data tests, record counts may
   have changed. Compare the snapshot you brought with this install:
   ```sh
   cargo run --release -p tessera-dattool -- diff <old>.idx "$TESSERA_GAME_DIR" --summary --exit-code
   ```
   Exit status 0 means no differences. Without a snapshot, run the real-data tests and
   compare their counts with the roadmap status tables.
3. `apps/client/tests/reference_locations.rs` checks 20 rows of
   `docs/checklists/m3-reference-locations.md` (heights, SED indices, area DIDs) taken from
   the 2026-10-04 data. If it fails after a patch, diff the snapshots for the affected
   landblock DIDs before suspecting code, and re-derive the expected values in the dirty
   room.
4. Golden images tolerate driver differences. A golden test that fails on only one machine
   is a real finding: investigate it, don't re-bless.

Claude's memory, transcripts and scratch files stay on the machine where they were made. A
new Claude session learns the project from `CLAUDE.md`, `docs/decisions/`, the roadmap status
tables and git history, so keep those current.

## 3. Continuing the roadmap with Claude Code

Each milestone in `docs/ROADMAP.md` runs as one workflow:

1. Start Claude Code in the repo root. With ultracode on (`/effort ultracode`), ask:
   *"Run the tessera-milestone workflow for M4"*, or call it explicitly:

   ```
   Workflow({ name: "tessera-milestone", args: {
     milestone: "M4",
     repo: "<absolute repo path>",
     game: "<absolute game install path, omit if none>",
     cache: "<${XDG_CACHE_HOME:-~/.cache}/tessera, as an absolute path>",
     notes: "<optional extra instructions, e.g. honour decision 0005's benchmark terms>" } })
   ```

   `cache` holds the research clones (`<cache>/clones`) and the run's scratch dirs
   (`<cache>/scratch/<M>/dirty` for spec writers, `<cache>/scratch/<M>/clean` for everyone
   else). Use the same directory `tools/fetch-research-clones.sh` uses, so the clones are
   not fetched twice.
2. The workflow plans the milestone, writes dirty-room specs, builds each crate clean-room in
   dependency order (a task whose spec or dependency failed is skipped, not built
   without it), runs three reviewers and a fix pass per crate, then verifies every exit
   criterion and updates the status tables. It never commits.
3. Review the result (run the tests yourself, look at a screenshot or two), then commit and
   push. One commit per milestone keeps history readable.
4. Criteria that need the owner to observe the original client are collected in
   `docs/checklists/owner-observations.md`. Fill them in when convenient; they do not block
   the next milestone.
5. **M8 is gated** ([decision 0004](decisions/0004-server-strategy.md)). Do not run it until
   a legal review has been recorded as a new decision. The workflow refuses `M8`.

Without ultracode, do the same steps by hand: write the specs with the `spec-writer` agent,
implement with `clean-implementer`, and audit with `clean-room-auditor` (`.claude/agents/`).

## 4. Claude Code on the web / machines without game data

The repo works on claude.ai/code once it is on GitHub. There is no game install there, so use
it for spec cleanup, refactors, synthetic tests, tooling and docs. Leave real-data
verification and visual checks for a machine with an install, and mark them as such in the
commit message.

## 5. Environment variables

| Variable | Purpose |
|---|---|
| `TESSERA_GAME_DIR` | Game install for real-data tests (the only way they find it) and the default for the apps |
| `TESSERA_FULL=1` | Run exhaustive real-data tests (all records) instead of samples |
| `TESSERA_REQUIRE_GPU` | `1`: fail instead of skipping when no GPU adapter is available (CI). `0`: skip GPU tests even when an adapter exists but no device can be created |
| `TESSERA_BLESS=1` | Regenerate golden images (check them on lavapipe/llvmpipe first) |
| `TESSERA_SEED`, `TESSERA_SAMPLE` | Reproducible sampling in real-data tests |
| `TESSERA_TARGET_DIR` | Used by `tools/dev-setup.sh --fix` to place the build cache |
| `TESSERA_CLONES_DIR` | Where `tools/fetch-research-clones.sh` puts the research clones (default `${XDG_CACHE_HOME:-~/.cache}/tessera/clones`) |
| `TESSERA_RE_DIR` | Dirty-room data root for captures, snapshots, Wine prefixes, Ghidra projects (`docs/TOOLING.md` §5; default `~/.cache/tessera/re`) |

Other `TESSERA_*` variables are documented next to the tests and apps that read them.
