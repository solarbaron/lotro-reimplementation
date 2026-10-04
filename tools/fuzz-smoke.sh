#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Short fuzzing run of every target in fuzz/ (see fuzz/README.md).
#
# With cargo-fuzz and a nightly toolchain it uses `cargo +nightly fuzz run` (AddressSanitizer
# on). Without them it builds the same targets on the stable toolchain with libFuzzer's
# coverage instrumentation passed as plain codegen flags (no sanitizer), which is enough to
# find panics, assertion failures, overflows (overflow checks are on) and memory blow-ups.
#
# Usage: tools/fuzz-smoke.sh [-t SECONDS] [-j JOBS] [--stable] [TARGET...]
#   -t SECONDS  run time per target (default 30)
#   --stable    force the stable-toolchain path even if cargo-fuzz is installed
#   TARGET...   targets to run (default: all, from `[[bin]]` entries in fuzz/Cargo.toml)
# Corpora go to fuzz/corpus/<target> and crashes to fuzz/artifacts/<target>/ (both
# git-ignored). Never seed a corpus that will be committed with real game data.
# Exit status: 0 no crash, 1 a target crashed, 2 usage or build error.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fuzz="$root/fuzz"
secs=30
force_stable=0
targets=()
while [ $# -gt 0 ]; do
    case "$1" in
        -t) secs="${2:?-t needs seconds}"; shift 2 ;;
        --stable) force_stable=1; shift ;;
        -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "fuzz-smoke: unknown option $1" >&2; exit 2 ;;
        *) targets+=("$1"); shift ;;
    esac
done
if [ ${#targets[@]} -eq 0 ]; then
    while IFS= read -r t; do targets+=("$t"); done < <(
        awk '/^\[\[bin\]\]/{b=1;next} b&&/^name *=/{gsub(/.*= *"|".*/,"");print;b=0}' "$fuzz/Cargo.toml")
fi

limits=(-rss_limit_mb=2048 -malloc_limit_mb=1024)
failed=()

if [ "$force_stable" = 0 ] && command -v cargo-fuzz >/dev/null 2>&1 \
    && cargo +nightly --version >/dev/null 2>&1; then
    for t in "${targets[@]}"; do
        echo "== $t (cargo-fuzz, ${secs}s)"
        if ! (cd "$fuzz" && cargo +nightly fuzz run "$t" -- -max_total_time="$secs" "${limits[@]}"); then
            failed+=("$t")
        fi
    done
else
    host="$(rustc -vV | sed -n 's/^host: //p')"
    export CARGO_TARGET_DIR="$fuzz/target/stable-smoke"
    export RUSTFLAGS="-Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 \
-Cllvm-args=-sanitizer-coverage-inline-8bit-counters -Cllvm-args=-sanitizer-coverage-pc-table \
-Cllvm-args=-sanitizer-coverage-trace-compares --cfg fuzzing -Cdebug-assertions -Coverflow-checks"
    echo "fuzz-smoke: cargo-fuzz/nightly not found, building instrumented targets on stable ($host)"
    # --target keeps RUSTFLAGS away from build scripts.
    (cd "$fuzz" && cargo build --release --bins --target "$host") || exit 2
    for t in "${targets[@]}"; do
        echo "== $t (stable libFuzzer, ${secs}s)"
        mkdir -p "$fuzz/corpus/$t" "$fuzz/artifacts/$t"
        if ! "$CARGO_TARGET_DIR/$host/release/$t" "$fuzz/corpus/$t" \
            -max_total_time="$secs" "${limits[@]}" -print_final_stats=1 \
            -artifact_prefix="$fuzz/artifacts/$t/"; then
            failed+=("$t")
        fi
    done
fi

if [ ${#failed[@]} -gt 0 ]; then
    echo "fuzz-smoke: crashes in: ${failed[*]} (inputs under fuzz/artifacts/)" >&2
    exit 1
fi
echo "fuzz-smoke: ${#targets[@]} target(s) ran ${secs}s each without a crash"
