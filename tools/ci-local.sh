#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Runs locally the checks of .github/workflows/ci.yml, one "job" at a time, and prints a
# summary table (pass / fail / warn / skip per job). Read ci.yml for what each job means;
# this script mirrors its commands. Differences from CI, on purpose:
#   - tests run in release mode with --no-fail-fast (faster for real-data runs), and with
#     TESSERA_GAME_DIR when it is set (CI never has an install); unset = synthetic only
#   - TESSERA_REQUIRE_GPU is left as it is in your environment (CI sets 1 on Linux);
#     --require-gpu sets it
#   - TESSERA_PERF (frame-time assertions of real-data tests, docs/DEVELOPMENT.md §5) is
#     passed through as it is in your environment; --perf sets it to 1
#   - CARGO_INCREMENTAL, CARGO_PROFILE_DEV_DEBUG and CARGO_TERM_COLOR are not overridden,
#     so the local build cache stays valid for plain `cargo` runs
#   - jobs whose tools are missing (MSRV toolchains, cargo-deny, reuse, a DCO base) are
#     skipped with a note instead of installed; nothing is ever installed
#   - the fuzz smoke test runs here (CI skips it on pull requests)
#
# Jobs, in order:
#   content          tools/check-content.sh --self-test, then the tree scan
#   spdx             tools/check-spdx.sh
#   reuse            reuse lint (if `reuse` is installed and LICENSES/ exists)
#   dco              tools/check-dco.sh <base> HEAD (base: --base, default origin/main)
#   fmt              cargo fmt --check, workspace and fuzz/
#   clippy           clippy -D warnings: workspace, workspace --all-features, fuzz/ --bins
#   test             cargo build --all-targets --release, cargo test --release --no-fail-fast
#   doc              cargo doc --workspace --no-deps with RUSTDOCFLAGS=-D warnings
#   msrv             cargo check on rust-version from Cargo.toml (if that toolchain is installed)
#   msrv-explorer    cargo check -p tessera-explorer on its own rust-version (likewise)
#   deny             cargo deny check bans licenses sources, Cargo.toml and fuzz/Cargo.toml
#   advisories       cargo deny check advisories, both manifests (advisory: failure = warn)
#   fuzz             tools/fuzz-smoke.sh -t SECS -j 2
#
# Usage: tools/ci-local.sh [--quick] [--jobs N] [--skip JOB[,JOB...]]... [--base REF]
#                          [--fuzz-secs N] [--synthetic] [--require-gpu] [--perf] [--list]
#   --quick          only fmt, clippy (workspace, default features) and unit tests
#                    (cargo test --lib --bins, release, --no-fail-fast)
#   --jobs N         parallel compile jobs (exports CARGO_BUILD_JOBS=N)
#   --skip JOB       skip a job (repeatable, or comma-separated); see --list
#   --base REF       base for the DCO check (default origin/main: the unpushed commits)
#   --fuzz-secs N    run time per fuzz target (default 10, as CI on pushes)
#   --synthetic      unset TESSERA_GAME_DIR for this run (real-data tests skip)
#   --require-gpu    export TESSERA_REQUIRE_GPU=1 (a missing GPU adapter fails, as in CI)
#   --perf           export TESSERA_PERF=1 (assert frame-time budgets; needs an idle machine)
#   --list           print the job names and exit
# Exit status: 0 every job passed, warned or was skipped; 1 a job failed; 2 usage error.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2

all_jobs=(content spdx reuse dco fmt clippy test doc msrv msrv-explorer deny advisories fuzz)
quick_jobs=(fmt clippy test)

quick=0
base="origin/main"
fuzz_secs=10
skips=()

usage_error() {
    echo "ci-local: $1 (see --help)" >&2
    exit 2
}

is_job() {
    local j
    for j in "${all_jobs[@]}"; do [ "$j" = "$1" ] && return 0; done
    return 1
}

while [ $# -gt 0 ]; do
    case "$1" in
        --quick) quick=1; shift ;;
        --jobs|-j)
            [ $# -ge 2 ] || usage_error "--jobs needs a number"
            case "$2" in ''|*[!0-9]*|0) usage_error "--jobs needs a positive number" ;; esac
            export CARGO_BUILD_JOBS="$2"; shift 2 ;;
        --skip)
            [ $# -ge 2 ] || usage_error "--skip needs a job name"
            IFS=',' read -r -a names <<< "$2"
            for n in "${names[@]}"; do
                is_job "$n" || usage_error "unknown job '$n'"
                skips+=("$n")
            done
            shift 2 ;;
        --base)
            [ $# -ge 2 ] || usage_error "--base needs a git ref"
            base="$2"; shift 2 ;;
        --fuzz-secs)
            [ $# -ge 2 ] || usage_error "--fuzz-secs needs a number"
            case "$2" in ''|*[!0-9]*|0) usage_error "--fuzz-secs needs a positive number" ;; esac
            fuzz_secs="$2"; shift 2 ;;
        --synthetic) unset TESSERA_GAME_DIR; shift ;;
        --require-gpu) export TESSERA_REQUIRE_GPU=1; shift ;;
        --perf) export TESSERA_PERF=1; shift ;;
        --list) printf '%s\n' "${all_jobs[@]}"; exit 0 ;;
        -h|--help) sed -n '4,/^# Exit status/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) usage_error "unknown argument '$1'" ;;
    esac
done

export RUST_BACKTRACE=1

# ---------------------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------------------

if [ -t 1 ]; then
    c_bold=$'\e[1m'; c_red=$'\e[31m'; c_green=$'\e[32m'; c_yellow=$'\e[33m'; c_dim=$'\e[2m'
    c_off=$'\e[0m'
else
    c_bold=""; c_red=""; c_green=""; c_yellow=""; c_dim=""; c_off=""
fi

# A job function sets `note` (free text for the summary) and returns
#   0 pass, 1 fail, 3 skip, 4 warn (failure of an advisory job).
note=""
SKIP=3
WARN=4

# Echo a command, then run it.
run() {
    printf '%s+ %s%s\n' "$c_dim" "$*" "$c_off"
    "$@"
}

# Run a command inside a directory (a subshell, so the caller's directory is unchanged).
run_in() {
    local dir="$1"
    shift
    printf '%s+ (cd %s && %s)%s\n' "$c_dim" "$dir" "$*" "$c_off"
    (cd "$dir" && "$@")
}

have() { command -v "$1" >/dev/null 2>&1; }

rust_version_of() {
    sed -n 's/^rust-version *= *"\(.*\)"/\1/p' "$1" | head -n 1
}

# Name of an installed rustup toolchain for version $1 ("1.90" matches 1.90-<host> and
# 1.90.N-<host>). Prints nothing if there is none. Never installs anything.
installed_toolchain() {
    local v="$1" re
    have rustup || return 0
    re="^${v//./\\.}(\\.[0-9]+)?-"
    rustup toolchain list 2>/dev/null | awk '{print $1}' | grep -E "$re" | head -n 1
}

# ---------------------------------------------------------------------------------------
# Jobs
# ---------------------------------------------------------------------------------------

job_content() {
    run tools/check-content.sh --self-test || return 1
    run tools/check-content.sh
}

job_spdx() {
    run tools/check-spdx.sh
}

job_reuse() {
    if ! compgen -G "LICENSES/*" >/dev/null; then
        note="no LICENSES/ (CI skips it too)"
        return $SKIP
    fi
    if ! have reuse; then
        note="reuse not installed (pip install reuse)"
        return $SKIP
    fi
    run reuse lint
}

job_dco() {
    if ! git rev-parse -q --verify "$base^{commit}" >/dev/null; then
        note="base '$base' not found (fetch, or pass --base)"
        return $SKIP
    fi
    local n
    n="$(git rev-list --count "$base..HEAD" 2>/dev/null || echo "?")"
    note="$base..HEAD, $n commit(s)"
    run tools/check-dco.sh "$base" HEAD
}

job_fmt() {
    run cargo fmt --all -- --check || return 1
    run_in fuzz cargo fmt --all -- --check
}

job_clippy() {
    run cargo clippy --workspace --all-targets --locked -- -D warnings || return 1
    if [ "$quick" = 1 ]; then
        note="quick: default features, workspace only"
        return 0
    fi
    run cargo clippy --workspace --all-targets --all-features --locked -- -D warnings || return 1
    run_in fuzz cargo clippy --bins --locked -- -D warnings
}

job_test() {
    if [ -n "${TESSERA_GAME_DIR:-}" ]; then
        note="with game data"
        [ "${TESSERA_PERF:-}" = 1 ] && note="$note, timing asserted"
    else
        note="synthetic only"
    fi
    if [ "$quick" = 1 ]; then
        note="unit tests, $note"
        run cargo test --workspace --locked --release --no-fail-fast --lib --bins
        return
    fi
    run cargo build --workspace --all-targets --locked --release || return 1
    run cargo test --workspace --locked --release --no-fail-fast
}

job_doc() {
    RUSTDOCFLAGS="-D warnings" run cargo doc --workspace --no-deps --locked
}

job_msrv() {
    local v tc
    v="$(rust_version_of Cargo.toml)"
    [ -n "$v" ] || { note="no rust-version in Cargo.toml"; return 1; }
    tc="$(installed_toolchain "$v")"
    if [ -z "$tc" ]; then
        note="toolchain $v not installed (rustup toolchain install $v)"
        return $SKIP
    fi
    note="$tc"
    RUSTUP_TOOLCHAIN="$tc" run cargo check --workspace --exclude tessera-explorer \
        --all-targets --locked
}

job_msrv_explorer() {
    local v tc
    v="$(rust_version_of apps/explorer/Cargo.toml)"
    [ -n "$v" ] || { note="no rust-version in apps/explorer/Cargo.toml"; return 1; }
    tc="$(installed_toolchain "$v")"
    if [ -z "$tc" ]; then
        note="toolchain $v not installed (rustup toolchain install $v)"
        return $SKIP
    fi
    note="$tc"
    RUSTUP_TOOLCHAIN="$tc" run cargo check -p tessera-explorer --all-targets --locked
}

job_deny() {
    if ! have cargo-deny; then
        note="cargo-deny not installed (cargo install --locked cargo-deny)"
        return $SKIP
    fi
    local m rc=0
    for m in Cargo.toml fuzz/Cargo.toml; do
        run cargo deny --manifest-path "$m" --all-features check bans licenses sources || rc=1
    done
    return $rc
}

job_advisories() {
    if ! have cargo-deny; then
        note="cargo-deny not installed (cargo install --locked cargo-deny)"
        return $SKIP
    fi
    local m rc=0
    for m in Cargo.toml fuzz/Cargo.toml; do
        run cargo deny --manifest-path "$m" --all-features check advisories || rc=1
    done
    if [ "$rc" != 0 ]; then
        note="advisory in CI: triage, does not fail the run"
        return $WARN
    fi
}

job_fuzz() {
    if have cargo-fuzz; then
        note="${fuzz_secs}s per target, cargo-fuzz if nightly is installed"
    else
        note="${fuzz_secs}s per target, stable path (no cargo-fuzz)"
    fi
    run tools/fuzz-smoke.sh -t "$fuzz_secs" -j 2
}

# ---------------------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------------------

if [ "$quick" = 1 ]; then
    jobs=("${quick_jobs[@]}")
else
    jobs=("${all_jobs[@]}")
fi

pinned="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)"
active="$(rustc --version 2>/dev/null | awk '{print $2}')"
echo "${c_bold}ci-local${c_off}: $( [ "$quick" = 1 ] && echo quick || echo full ) run in $root"
echo "  toolchain: rustc ${active:-?} (pinned: ${pinned:-?})"
if [ -n "$pinned" ] && [ -n "$active" ] && [ "$pinned" != "$active" ]; then
    echo "  ${c_yellow}warning${c_off}: rustc is not the pinned version; is rustup's cargo first on PATH?"
fi
echo "  build jobs: ${CARGO_BUILD_JOBS:-cargo default}"
echo "  game data: ${TESSERA_GAME_DIR:-unset (real-data tests skip)}"
echo "  TESSERA_REQUIRE_GPU: ${TESSERA_REQUIRE_GPU:-unset}"
echo "  TESSERA_PERF: ${TESSERA_PERF:-unset (frame-time assertions off)}"

results=()
notes=()
times=()
failed=0
run_start=$SECONDS

for job in "${jobs[@]}"; do
    skipped=0
    for s in "${skips[@]}"; do [ "$s" = "$job" ] && skipped=1; done
    if [ "$skipped" = 1 ]; then
        results+=(skip); notes+=("skipped by --skip"); times+=("-")
        continue
    fi
    echo
    echo "${c_bold}==> $job${c_off}"
    note=""
    start=$SECONDS
    "job_${job//-/_}"
    rc=$?
    elapsed=$((SECONDS - start))
    case "$rc" in
        0) r=pass ;;
        "$SKIP") r=skip ;;
        "$WARN") r=warn ;;
        *) r=fail; failed=1 ;;
    esac
    [ "$r" = skip ] && echo "${c_yellow}skipped${c_off}: $note"
    results+=("$r"); notes+=("$note"); times+=("${elapsed}s")
done

echo
echo "${c_bold}Summary${c_off} ($((SECONDS - run_start))s)"
printf '  %-14s %-6s %7s  %s\n' JOB RESULT TIME NOTE
for i in "${!jobs[@]}"; do
    r="${results[$i]}"
    case "$r" in
        pass) c="$c_green" ;;
        fail) c="$c_red" ;;
        *) c="$c_yellow" ;;
    esac
    printf '  %-14s %s%-6s%s %7s  %s\n' "${jobs[$i]}" "$c" "$r" "$c_off" "${times[$i]}" "${notes[$i]}"
done

exit "$failed"
