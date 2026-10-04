# Development guide (any machine)

How to set up a machine, hand work between machines, and continue the roadmap with Claude
Code. Git is the only shared state between machines ([decision 0007](decisions/0007-multi-device-workflow.md)).

## 1. New machine setup

```sh
git clone git@github.com:solarbaron/lotro-reimplementation.git
cd lotro-reimplementation
tools/dev-setup.sh            # reports what is present/missing
```

1. **Rust toolchain**: stable, with `rustfmt` and `clippy` (`rust-toolchain.toml` pins the
   channel). Builds need roughly 20 GB of cache.
2. **Build cache location** (optional, machine-local, git-ignored): to keep the cache off a
   small partition,
   `TESSERA_TARGET_DIR=/var/tmp/tessera-target tools/dev-setup.sh --fix`, which writes
   `.cargo/config.toml`.
3. **Game install**: install the game (any legal source, e.g. Steam app 212500) and let it
   finish patching. Then either `export TESSERA_GAME_DIR="/path/to/The Lord of the Rings Online"`
   or rely on auto-detection: `cargo run -p tessera-dattool -- detect`.
   Without an install everything builds and all synthetic tests run; real-data tests skip.
4. **Optional tools**: `cargo install cargo-deny --locked` (put `~/.cargo/bin` on `PATH`),
   cargo-fuzz + nightly, xvfb-run, a screenshot tool (grim/scrot), Mesa lavapipe. See
   `docs/TOOLING.md` for the full list by role.
5. **Research clones** (only for writing specs, about 6 GB): `tools/fetch-research-clones.sh`
   puts them in `~/.cache/tessera/clones`. Never inside the repo.

Smoke test:

```sh
cargo test --workspace
cargo run --release -p tessera-dattool -- validate "$TESSERA_GAME_DIR"
cargo run --release -p tessera-explorer
cargo run --release -p tessera-client
```

## 2. Handing over between machines

Only one machine works at a time.

**Before leaving a machine**
1. Let any running milestone workflow finish, or stop it.
2. Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` and `tools/check-content.sh`.
3. Commit (signed-off, with `Spec-Ref:` / `Assisted-by:` trailers where relevant) and
   `git push`.
4. If the work is unfinished, say where it stopped in the "Current state" section of
   `CLAUDE.md` and push that too.

**On arrival**
1. `git pull`, then `tools/dev-setup.sh`.
2. If the game patched since the other machine last ran real-data tests, record counts may
   have changed. Compare with `tessera-dattool snapshot <dir> -o a.idx` on each machine and
   `tessera-dattool diff a.idx b.idx`.
3. Golden images tolerate driver differences. A golden test that fails on only one machine
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
     cache: "<home>/.cache/tessera" } })
   ```

2. The workflow plans the milestone, writes dirty-room specs, builds each crate clean-room in
   dependency order, runs three reviewers and a fix pass per crate, then verifies every exit
   criterion and updates the status tables. It never commits.
3. Review the result (run the tests yourself, look at a screenshot or two), then commit and
   push. One commit per milestone keeps history readable.
4. Criteria that need the owner to observe the original client are collected in
   `docs/checklists/owner-observations.md`. Fill them in when convenient; they do not block
   the next milestone.
5. **M8 is gated** ([decision 0004](decisions/0004-server-strategy.md)). Do not run it until
   a legal review has been recorded as a new decision.

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
| `TESSERA_GAME_DIR` | Game install for real-data tests and the apps |
| `TESSERA_FULL=1` | Run exhaustive real-data tests (all records) instead of samples |
| `TESSERA_REQUIRE_GPU=1` | Fail instead of skipping when no GPU adapter is available (CI) |
| `TESSERA_BLESS=1` | Regenerate golden images (check them on lavapipe/llvmpipe first) |
| `TESSERA_SEED`, `TESSERA_SAMPLE` | Reproducible sampling in real-data tests |
| `TESSERA_TARGET_DIR` | Used by `tools/dev-setup.sh --fix` to place the build cache |

Other `TESSERA_*` variables are documented next to the tests and apps that read them.
