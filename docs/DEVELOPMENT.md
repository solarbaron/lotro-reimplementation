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
   **Several worktrees** (parallel branches, each with its own target dir) build the same
   dependencies again and again. A shared compiler cache avoids that. Install sccache (a
   release binary in `~/.cargo/bin`), then add `[build] rustc-wrapper = "<path>/sccache"`
   to your **user** `~/.cargo/config.toml`, not the repository's. Set its size and
   location (on a fast disk) in `~/.config/sccache/config` (`[cache.disk]` `dir`, `size`).
   Dependencies and release builds are cached; incremental debug builds of the workspace's
   own crates are not. `sccache --show-stats` shows the hit rate.
3. **Game install**: install the game (e.g. Steam app 212500) and let it
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

### Release builds

Release artefacts of the client are built with a plain

```sh
cargo build --release -p tessera-client
```

never with `--all-targets`, `--tests`, `--benches`, `--examples`, `--workspace`,
`--all-features` or a `--features` list, and never through `cargo test`
(`docs/specs/official-entry.md` §6.3 "Release builds"). With resolver 3 a build that includes
the client's test targets turns on the features of its dev-dependencies in the binary as
well, and the client's tests enable `tessera-compat`'s `test-support` feature, which a release
build must never contain (§6.1). `tools/check-release-features.sh` checks the normal
dependency graph (`cargo tree -p tessera-client -e features,normal -i tessera-compat`, default
and all features) and fails if the feature is in it; `--self-test` checks that it catches the
feature on fixture workspaces (OE-T22). CI and `tools/ci-local.sh` run both as the job
`release-features`.

### Running the CI checks locally

`tools/ci-local.sh` runs the jobs of `.github/workflows/ci.yml` on this machine and ends
with a table of pass / fail / warn / skip per job; the exit status is non-zero if any job
failed.

```sh
tools/ci-local.sh --quick                # fmt, clippy, unit tests: before each commit
tools/ci-local.sh                        # everything: before pushing or leaving a machine
tools/ci-local.sh --jobs 4 --skip fuzz   # limit compile jobs, leave out a job (see --list)
tools/ci-local.sh --base @{push}         # DCO check against another base (default origin/main)
```

Differences from CI: tests run with `--release --no-fail-fast`, and with game data when
`TESSERA_GAME_DIR` is exported (`--synthetic` unsets it for the run); `--require-gpu` sets
`TESSERA_REQUIRE_GPU=1` as CI does on Linux. Jobs whose tools are missing are skipped with a
note, never installed: the MSRV toolchains (`rustup toolchain install 1.90 1.95`),
`cargo-deny`, `reuse`, and the DCO base. `advisories` is advisory, as in CI: a finding is a
warning to triage, not a failure. The fuzz smoke test (`--fuzz-secs`, default 10 s per
target) runs here although CI skips it on pull requests. `--help` lists every option.

### Fuzz targets and the coverage manifests

Every parser has a fuzz target (`CLAUDE.md`; P8 exit criterion 2,
[decision 0043](decisions/0043-p8-parity-audit.md) §2). A **parser** is every public entry point
in `crates/` and `apps/` that decodes bytes or text that Tessera did not write itself in the same
process: install records and files, network messages, settings, the keymap file, skins and UI
XML, plugin descriptors and plugin data, ABC files, scenario files and their input scripts, and
logs read back. A file Tessera wrote in an earlier run counts; in-memory data the same process
produced does not. `crates/compat` (M8) is covered too, by `fuzz/coverage/compat.toml`.

`fuzz/coverage/*.toml` records, per crate and app, each parser with the fuzz target that drives
it: `data.toml` for the data crates, `runtime.toml` for the runtime crates and the apps.
`tools/check-fuzz-coverage.sh` checks them offline, without a build (bash, awk and find), and
runs in CI (job `fuzz-coverage`) and in `tools/ci-local.sh`:

```sh
tools/check-fuzz-coverage.sh              # exit 0 covered, 1 a check failed, 2 usage or tree error
tools/check-fuzz-coverage.sh --list       # also every scanned function and what covers it
tools/check-fuzz-coverage.sh --self-test  # the checker's own fixture cases (CI runs this first)
```

Each failure names its manifest and line and a letter: (m) a malformed table, or a package that
no manifest covers; (a) a target that is not a `[[bin]]` of `fuzz/Cargo.toml` with a source
file; (b) a `[[bin]]` that no manifest names; (c) a parser-like `pub fn` in the sources that no
`[[parser]]` entry or `[[skip]]` covers; (g) a recorded gap. The script's header (and
`tools/README.md`) give the exact schema and the name rules of the scan.

**When you add a parser** (or a new public decoding function in a file that already has one):

1. Add a fuzz target `fuzz/fuzz_targets/<crate>_<thing>.rs` and register it as a `[[bin]]` in
   `fuzz/Cargo.toml` (`name`, `path`, `test = false`, `doc = false`, `bench = false`).
2. Add a `[[parser]]` table to the manifest of its half:

   ```toml
   [[parser]]
   crate = "tessera-foo"                    # the package name
   file = "crates/foo/src/bar.rs"           # the file that defines the entry point
   entry = "tessera_foo::bar::Bar::parse"   # or an array of paths in that file
   input = "install record"                 # install record | network | user file | scenario | settings | plugin | other
   target = "foo_bar"                       # the [[bin]] that drives it
   ```

   Exactly one of `target`, `exempt` and `gap`. Use `exempt = "..."` only for a thin wrapper whose
   whole input reaches a fuzzed parser; the reason must name that target (one of its words is
   the `[[bin]]` name). `gap = "..."` records a parser that has no target yet; the checker lists
   every gap as a failure, so a gap is a reminder, not a way through.
3. Run `tools/check-fuzz-coverage.sh` until it ends with `check-fuzz-coverage: ok`, then
   `tools/fuzz-smoke.sh -t 60 foo_bar`.

A target reaches the code under test through the crate's library (`fuzz/Cargo.toml` depends on
every data and runtime crate, and on the `tessera-client`, `tessera-explorer`,
`tessera-loadtest`, `tessera-dattool` and `tessera-server` libraries), or includes a small leaf
module by path when it should not link the whole app (`client_uiscale_command`,
`loadtest_text`). Code that lives only in an app's binary (`main.rs` and its modules) cannot be
reached: move the reader into the app's library first (as `tessera_dattool::hexarg`).

Other tables: a new crate or app with no parser of its own gets a `[[package]]` table
(`crate`, `reason`); it is still scanned. A function whose name looks like a parser but decodes
nothing hostile gets a `[[skip]]` with `path`, `fns = ["name"]` and a `reason`; a fuzz target
that drives a state machine or geometry rather than a parser is listed as a `[[skip]]` of its
source file, so that check (b) knows it. Test-only code (`tests/`, `#[cfg(test)]` items and the
like) is never scanned.

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
   tools/check-fuzz-coverage.sh               # every parser has a fuzz target (§1)
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
5. Never run `rea setup` or install the REA skill (decision 0033); `tools/dev-setup.sh`
   fails if an REA or Ghidra MCP server is registered, or if `$TESSERA_RE_DIR/.mcp.json`
   exists (dirty MCP servers are attached per session, `docs/TOOLING.md` §8 item 3).

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
2. The workflow plans the milestone and writes dirty-room specs. Each spec then gets a
   contamination screen (a clean auditor reading only the spec) and an independent review
   ([CLEAN_ROOM.md §5.3](CLEAN_ROOM.md#53-review-checklist)) by a session that wrote none of
   it; a failing spec is revised and reviewed again by fresh sessions, up to three rounds.
   Only an Accepted spec feeds the build. Tasks start as soon as their dependencies are
   done, with at most `max_builds` (default 3) builds or fixes at once; a task whose spec or
   dependency failed is skipped, not built without it. Each task gets reviewers and a fix
   pass; then every exit criterion is verified and the status tables are updated. It never
   commits. Its result carries a `provenance` record (which agent wrote, screened, reviewed
   and built what) for the commit messages.
3. Review the result (run the tests yourself, look at a screenshot or two), then commit and
   push. One commit per milestone keeps history readable.
4. Criteria that need the owner to observe the original client are collected in
   `docs/checklists/owner-observations.md`. Fill them in when convenient; they do not block
   the next milestone.
5. **M8 is in scope** by the owner's decision of 2026-10-11
   ([decision 0034](decisions/0034-official-server-compatibility.md)), but **paused** by
   [incident 0041](decisions/0041-incident-m8-spec-contamination.md): the workflow refuses
   `M8` until its specs are cleaned and independently re-accepted and the owner sets
   `M8_PAUSED = false` in the script. When it runs, it gives its spec writers the 0034 rules:
   - the repository content and boundary rules of `CLAUDE.md` and decision 0034 §1;
   - no key material, hostnames or account data in the repository or specs;
   - captures stay in the dirty room;
   - no anti-cheat or anti-tamper work.

   Live logins happen only on the owner's own account and never from CI. The workflow
   refuses `M9` until P8 and M8 are done.

Open items outside a milestone run go through the `tessera-followups` workflow
(`.claude/workflows/tessera-followups.js`). Scan and group them into packages with
non-overlapping paths, then give each package its own worktree and branch: a sparse checkout
without `research/` (`git sparse-checkout set --no-cone '/*' '!/research/'`) for code
packages, a full one for spec packages. Each code worktree's git-ignored `.cargo/config.toml`
points at a `target-dir` of its own on the main drive (`~/tessera-targets/<key>`), with
`incremental = false` and `debug = 0` for `[profile.dev]` and `[profile.dev.package."*"]`.
Never let worktrees share a target dir: cargo names workspace artifacts relative to the
workspace root and judges freshness by modification time, so a build in one worktree can be
taken as fresh by another and link the other worktree's code (seen on 2026-10-23). With
sccache (`incremental = false` is what lets it cache) the dependencies are not rebuilt per
worktree, and `debug = 0` keeps each dir near 20 GB; with full debug info a debug dir grew by
84 GB in half an hour. Set `max_clean` so that 25 GB per code package in flight still
leaves 40 GB free on `/`, and delete each dir after its branch is merged. Code packages are built, reviewed, fixed and audited, and spec
packages are revised, screened and independently reviewed. Then review, commit and merge each
branch and run the full gate once.

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
| `TESSERA_PERF=1` | Also assert frame-time budgets in real-data tests, and make the windowed timing runs of `p8_busiest_real_data` at all (see "Performance assertions" below) |
| `TESSERA_REQUIRE_GPU` | `1`: fail instead of skipping when no GPU adapter is available (CI). `0`: skip GPU tests even when an adapter exists but no device can be created |
| `TESSERA_BLESS=1` | Regenerate golden images (check them on lavapipe/llvmpipe first) |
| `TESSERA_GOLDEN_DIR` | Where local-only real-data goldens of panels live (decision 0031 tier 2; default `${XDG_CACHE_HOME:-~/.cache}/tessera/goldens`; never inside a work tree) |
| `TESSERA_GOLDEN_REQUIRE=1` | A missing tier-2 golden fails instead of being blessed (the second run of a verification) |
| `TESSERA_GOLDEN_BLESS_LOCAL=1` | Re-bless tier-2 goldens that differ (never together with `TESSERA_GOLDEN_REQUIRE`; `TESSERA_BLESS` does not touch them) |
| `TESSERA_SHEET_DIR` | Also copy the pictures a panel test takes to this folder, for judging them by eye |
| `TESSERA_SEED`, `TESSERA_SAMPLE` | Reproducible sampling in real-data tests |
| `TESSERA_TARGET_DIR` | Used by `tools/dev-setup.sh --fix` to place the build cache |
| `TESSERA_CLONES_DIR` | Where `tools/fetch-research-clones.sh` puts the research clones (default `${XDG_CACHE_HOME:-~/.cache}/tessera/clones`) |
| `TESSERA_RE_DIR` | Dirty-room data root for captures, snapshots, Wine prefixes, Ghidra projects (`docs/TOOLING.md` §5; default `~/.cache/tessera/re`). It also holds `tools/rea` and `tools/ghidra-shim` (decision 0033): machine-local, not carried by git; re-create them per `docs/TOOLING.md` §5.4 on a new machine |

Other `TESSERA_*` variables are documented next to the tests and apps that read them.

### Performance assertions (`TESSERA_PERF`)

Real-data tests that measure frame or main-thread update times (the 50 ms limit of ROADMAP
M3 exit criterion 2 in `apps/client/tests/walker_real_data.rs` and
`crates/world/tests/real_data.rs`) print their timings on every run but assert them only
with `TESSERA_PERF=1`, because any other build or test loading the machine makes them fail.
Their other checks (failures, near-ring holes, residency, counts) always run. To check the
criterion, use a release build on an otherwise idle machine:

```sh
TESSERA_PERF=1 cargo test --release -p tessera-client --test walker_real_data -- --nocapture
TESSERA_PERF=1 cargo test --release -p tessera-world --test real_data -- --nocapture
```

**The busiest scenario** (P8 criterion 3, [decision 0043](decisions/0043-p8-parity-audit.md)
§3): `apps/client/tests/p8_busiest_real_data.rs` always checks, without an install, that the
committed `busiest.scenario` has the decision's minimums and that `--scenario-timing` combines
with the other options as the client says, and with `TESSERA_GAME_DIR` that the scenario
resolves and plays to its end headless at the reference location, twice with the same log.
Only with `TESSERA_PERF=1` does it make the two windowed timing runs (paced, then
`--no-vsync`; 60 s each after the settle wait) and assert the budgets of decision 0043 §3
item 3 on their timing reports. Those runs need a display (`WAYLAND_DISPLAY` or `DISPLAY`), a
window of at least 1024 × 768 (give it a workspace of its own, so that the UI runs at
*Se* = 1), an idle machine, and a build with the audio device backend. Without that feature the test fails with
"(d) not measured", and at *Se* ≠ 1 it fails with "(c) not judged":

```sh
TESSERA_PERF=1 cargo test --release -p tessera-client --features audio-device \
    --test p8_busiest_real_data -- --nocapture
```

The location, the commands of the runs without the test and the measured figures are in
[`docs/perf/p8-busiest.md`](perf/p8-busiest.md); the scenario timing option is described in
[`USER_GUIDE.md` §3.19](USER_GUIDE.md#timing-runs). Runs on a loaded machine are indicative
only; the owner re-measures on the reference machine (decision 0010, owner row O200).

`tools/ci-local.sh` passes `TESSERA_PERF` through when it is set (or `--perf` sets it).
