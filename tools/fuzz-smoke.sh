#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Short fuzzing run of every target in fuzz/ (see fuzz/README.md).
#
# With cargo-fuzz and a nightly toolchain it uses `cargo +nightly fuzz run` (AddressSanitizer
# on). Without them it builds the same targets on the stable toolchain with libFuzzer's
# coverage instrumentation passed as plain codegen flags (no sanitizer), which is enough to
# find panics, assertion failures, overflows (overflow checks are on) and memory blow-ups.
# The nightly toolchain is only detected (`rustup toolchain list`), never installed.
# cargo-fuzz is always given `--target <host>` (the host of the toolchain that builds):
# without it cargo-fuzz targets the triple it was itself compiled for, and the prebuilt
# release binaries (as installed by CI) are musl builds, which AddressSanitizer rejects
# ("incompatible with statically linked libc") and whose standard library is not installed.
#
# Memory limits: every run gets -rss_limit_mb=2048 (resident memory, polled by libFuzzer).
# libFuzzer enforces -malloc_limit_mb=1024 (one allocation too large) only through the
# sanitizer's malloc hooks, so only on the cargo-fuzz path. The stable path has no such
# hooks; it caps the address space instead (`ulimit -v`, 3 GiB = the two limits together),
# so that a large reservation that is never touched still fails, as an allocation failure
# (abort) that libFuzzer reports as a crash.
#
# Usage: tools/fuzz-smoke.sh [-t SECONDS] [-j JOBS] [--stable] [TARGET...]
#   -t SECONDS  run time per target, at least 1 (default 30)
#   -j JOBS     targets run at the same time (default 1)
#   --stable    force the stable-toolchain path even if cargo-fuzz is installed
#   TARGET...   targets to run (default: all, from `[[bin]]` entries in fuzz/Cargo.toml);
#               only the named targets are built
# A libFuzzer dictionary at fuzz/dict/<target>.dict is used when present (syntax tokens
# only, never strings taken from game data). Corpora go to fuzz/corpus/<target> and crashes
# to fuzz/artifacts/<target>/ (both git-ignored). Builds and logs go under the cargo target
# dir configured for fuzz/ (a machine-local .cargo/config.toml target-dir, CARGO_TARGET_DIR,
# else fuzz/target): stable builds in <target-dir>/fuzz-stable-smoke, logs in
# <target-dir>/fuzz-smoke-logs/<target>.log. A summary table (time, executions, result per
# target) is printed at the end. Never seed a corpus that will be committed with real game
# data.
# Exit status: 0 no crash, 1 a target crashed, 2 usage or build error.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fuzz="$root/fuzz"
secs=30
jobs=1
force_stable=0
targets=()
while [ $# -gt 0 ]; do
    case "$1" in
        -t) secs="${2:?-t needs seconds}"; shift 2 ;;
        -j) jobs="${2:?-j needs a job count}"; shift 2 ;;
        --stable) force_stable=1; shift ;;
        -h|--help) sed -n '2,/^# Exit status/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "fuzz-smoke: unknown option $1" >&2; exit 2 ;;
        *) targets+=("$1"); shift ;;
    esac
done
# Both must be positive: libFuzzer reads -max_total_time=0 as "no time limit".
case "$secs" in ''|*[!0-9]*) echo "fuzz-smoke: -t needs a positive whole number" >&2; exit 2 ;; esac
case "$jobs" in ''|*[!0-9]*) echo "fuzz-smoke: -j needs a positive number" >&2; exit 2 ;; esac
secs=$((10#$secs))
jobs=$((10#$jobs))
[ "$secs" -gt 0 ] || { echo "fuzz-smoke: -t needs a positive whole number" >&2; exit 2; }
[ "$jobs" -gt 0 ] || { echo "fuzz-smoke: -j needs a positive number" >&2; exit 2; }
named=${#targets[@]}
if [ "$named" -eq 0 ]; then
    while IFS= read -r t; do targets+=("$t"); done < <(
        awk '/^\[\[bin\]\]/{b=1;next} b&&/^name *=/{gsub(/.*= *"|".*/,"");print;b=0}' "$fuzz/Cargo.toml")
fi

rss_mb=2048
malloc_mb=1024
limits=(-rss_limit_mb="$rss_mb" -malloc_limit_mb="$malloc_mb")
# Honour the configured target dir instead of hard-coding one inside the checkout.
base="$(cd "$fuzz" && cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' 2>/dev/null)" \
    || base=""
base="${base:-$fuzz/target}"
logs="$base/fuzz-smoke-logs"
mkdir -p "$logs"
bin_args=()
if [ "$named" -gt 0 ]; then
    for t in "${targets[@]}"; do bin_args+=(--bin "$t"); done
else
    bin_args=(--bins)
fi

# Whether a nightly toolchain is installed, without side effects: `cargo +nightly` would
# make rustup download one when it is missing.
have_nightly() {
    if command -v rustup >/dev/null 2>&1; then
        # `+nightly` means the undated channel (`nightly-<host>`); dated ones do not count.
        rustup toolchain list 2>/dev/null | grep -Eq '^nightly-[a-z]'
    else
        RUSTUP_AUTO_INSTALL=0 cargo +nightly --version >/dev/null 2>&1
    fi
}

if [ "$force_stable" = 0 ] && command -v cargo-fuzz >/dev/null 2>&1 && have_nightly; then
    mode="cargo-fuzz"
    host="$(cd "$fuzz" && rustc +nightly -vV | sed -n 's/^host: //p')" || host=""
    [ -n "$host" ] || { echo "fuzz-smoke: cannot read the nightly host triple" >&2; exit 2; }
    if [ "$named" -gt 0 ]; then
        for t in "${targets[@]}"; do
            (cd "$fuzz" && cargo +nightly fuzz build --target "$host" "$t") || exit 2
        done
    else
        (cd "$fuzz" && cargo +nightly fuzz build --target "$host") || exit 2
    fi
else
    mode="stable libFuzzer"
    host="$(cd "$fuzz" && rustc -vV | sed -n 's/^host: //p')" || host=""
    [ -n "$host" ] || { echo "fuzz-smoke: cannot read the host triple" >&2; exit 2; }
    export CARGO_TARGET_DIR="$base/fuzz-stable-smoke"
    export RUSTFLAGS="-Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 \
-Cllvm-args=-sanitizer-coverage-inline-8bit-counters -Cllvm-args=-sanitizer-coverage-pc-table \
-Cllvm-args=-sanitizer-coverage-trace-compares --cfg fuzzing -Cdebug-assertions -Coverflow-checks"
    echo "fuzz-smoke: cargo-fuzz/nightly not found, building instrumented targets on stable ($host)"
    # --target keeps RUSTFLAGS away from build scripts.
    (cd "$fuzz" && cargo build --release "${bin_args[@]}" --target "$host") || exit 2
fi

# Run one target to completion; its output goes to its log and its exit status to
# <log>.status.
run_one() {
    local t="$1" log="$logs/$1.log" start end rc=0 extra=()
    mkdir -p "$fuzz/corpus/$t" "$fuzz/artifacts/$t"
    if [ -f "$fuzz/dict/$t.dict" ]; then extra+=(-dict="$fuzz/dict/$t.dict"); fi
    # mlua makes one-time allocations that LeakSanitizer cannot trace back to a long-lived
    # Lua state (the script state of fuzz_targets/script_pattern.rs, the plugin manager of
    # fuzz_targets/plugins_commands.rs). Only matters
    # with cargo-fuzz (AddressSanitizer); the stable path has no leak checker. Leak checks
    # are turned off in LeakSanitizer only: libFuzzer's own -detect_leaks=0 would also skip
    # installing the malloc hooks that enforce -malloc_limit_mb.
    local asan="${ASAN_OPTIONS:-}"
    case "$t" in
        script_pattern|plugins_commands)
            asan="${asan:+$asan:}detect_leaks=0" ;; # per-input and at-exit checks
    esac
    start=$(date +%s)
    if [ "$mode" = "cargo-fuzz" ]; then
        (cd "$fuzz" && { [ -z "$asan" ] || export ASAN_OPTIONS="$asan"; } \
            && cargo +nightly fuzz run --target "$host" "$t" -- -max_total_time="$secs" \
            "${limits[@]}" -print_final_stats=1 "${extra[@]}") >"$log" 2>&1 || rc=$?
    else
        # No malloc hooks here (see the header): cap the address space instead, in a
        # subshell so the cap stays with this run.
        (ulimit -v $(((rss_mb + malloc_mb) * 1024)) \
            && exec "$CARGO_TARGET_DIR/$host/release/$t" "$fuzz/corpus/$t" \
            -max_total_time="$secs" "${limits[@]}" -print_final_stats=1 \
            -artifact_prefix="$fuzz/artifacts/$t/" "${extra[@]}") >"$log" 2>&1 || rc=$?
    fi
    end=$(date +%s)
    echo "$rc $((end - start))" >"$log.status"
    if [ "$rc" = 0 ]; then
        echo "== $t: ok ($((end - start)) s)"
    else
        echo "== $t: FAILED (exit $rc, see $log)"
        grep -E '^(==[0-9]+==|thread .* panicked|SUMMARY|artifact_prefix|Test unit written)' \
            "$log" | head -n 20 || true
    fi
}

echo "fuzz-smoke: ${#targets[@]} target(s), ${secs}s each, $jobs at a time ($mode)"
running=0
for t in "${targets[@]}"; do
    rm -f "$logs/$t.log.status"
    if [ "$jobs" -le 1 ]; then
        run_one "$t"
    else
        run_one "$t" &
        running=$((running + 1))
        if [ "$running" -ge "$jobs" ]; then
            wait -n || true
            running=$((running - 1))
        fi
    fi
done
wait || true

failed=()
printf '\n%-22s %6s %14s %10s  %s\n' target time executions exec/s result
for t in "${targets[@]}"; do
    log="$logs/$t.log"
    read -r rc took 2>/dev/null <"$log.status" || { rc=2; took=0; }
    execs="$(sed -n 's/^stat::number_of_executed_units: *//p' "$log" | tail -n 1)"
    rate="$(sed -n 's/^stat::average_exec_per_sec: *//p' "$log" | tail -n 1)"
    if [ "$rc" = 0 ]; then
        result="ok"
    else
        result="CRASH (exit $rc)"
        failed+=("$t")
    fi
    printf '%-22s %5ss %14s %10s  %s\n' "$t" "$took" "${execs:--}" "${rate:--}" "$result"
done

if [ ${#failed[@]} -gt 0 ]; then
    echo "fuzz-smoke: crashes in: ${failed[*]} (inputs under fuzz/artifacts/)" >&2
    exit 1
fi
echo "fuzz-smoke: ${#targets[@]} target(s) ran ${secs}s each without a crash"
