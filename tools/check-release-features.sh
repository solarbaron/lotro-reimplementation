#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Release feature guard (docs/specs/official-entry.md §6.1 "Release builds", OE-T8 (e) and
# OE-T22). The test-support feature of tessera-compat exposes the record-building helpers of
# its consent module; only fuzz/ and dev-dependencies may enable it, and a release build of
# the client must never contain it.
#
# Graph. `cargo tree -p tessera-client -e features,normal --target all -i tessera-compat`
# (inverted: the forward tree does not show features that one of the client's own features
# turns on), with the client's default features and again with --all-features. Normal edges
# only: dev-dependencies (the client's tests enable test-support) and build-dependencies are
# not part of the binary.
# Release artefacts are built with a plain `cargo build --release -p tessera-client`, never
# with --all-targets or tests (with resolver 3 those turn the dev-dependency feature on in
# the binary too; docs/DEVELOPMENT.md "Release builds").
#
# Self-check. The same tree with dev edges must show the feature (else the pattern below
# no longer matches cargo's output and the guard would pass vacuously), and the forward
# normal tree must contain tessera-compat at all.
#
# Usage: tools/check-release-features.sh [--root DIR] [--self-test]
#   (no option)    check this repository's workspace
#   --root DIR     the workspace to check (default: this repository); the self-test uses it
#                  on throwaway fixture workspaces
#   --self-test    build fixture workspaces in a temporary directory: a clean one (the
#                  feature only through a dev-dependency) must pass; one where a normal
#                  dependency enables the feature, one where it comes through another crate
#                  and one where a client feature turns it on must fail; one without
#                  tessera-compat must fail the self-check (OE-T22)
# Needs bash (no bash 4 features), cargo, grep. In this repository cargo runs with --locked, as in CI (fixture
# workspaces of the self-test have no lockfile).
# Exit status: 0 clean or self-test passed; 1 the feature is in a normal graph, the
# self-check failed or the self-test failed; 2 usage or cargo error.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/$(basename "${BASH_SOURCE[0]}")"
repo="$(cd "$here/.." && pwd)"

client="tessera-client"
pattern='tessera-compat feature "test-support"'

usage_error() {
    echo "check-release-features: $1 (see the usage in the script's header)" >&2
    exit 2
}

ws="$repo"
self_test=0
while [ $# -gt 0 ]; do
    case "$1" in
        --root)
            [ $# -ge 2 ] || usage_error "--root needs a directory"
            ws="$(cd "$2" && pwd)" || usage_error "no directory $2"
            shift 2 ;;
        --self-test) self_test=1; shift ;;
        -h|--help) sed -n '2,32p' "$script"; exit 0 ;;
        *) usage_error "unknown option $1" ;;
    esac
done

# ---- self-test --------------------------------------------------------------------------

fixture_crate() { # DIR NAME [DEPENDENCY-TOML] [EXTRA-TOML]
    mkdir -p "$1/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n[dependencies]\n%s\n%s\n' \
        "$2" "${3:-}" "${4:-}" > "$1/Cargo.toml"
    : > "$1/src/lib.rs"
}

fixture() { # DIR CASE
    local d="$1" dep extra
    mkdir -p "$d"
    printf '[workspace]\nresolver = "3"\nmembers = ["client", "compat", "middle"]\n' > "$d/Cargo.toml"
    [ -f "$repo/rust-toolchain.toml" ] && cp "$repo/rust-toolchain.toml" "$d/"
    fixture_crate "$d/compat" tessera-compat "" "$(printf '[features]\ntest-support = []')"
    dep='tessera-compat = { path = "../compat" }'
    extra="$(printf '[dev-dependencies]\ntessera-compat = { path = "../compat", features = ["test-support"] }')"
    case "$2" in
        clean) ;;
        normal) dep='tessera-compat = { path = "../compat", features = ["test-support"] }' ;;
        indirect)
            fixture_crate "$d/middle" tessera-middle \
                'tessera-compat = { path = "../compat", features = ["test-support"] }'
            dep="$(printf '%s\ntessera-middle = { path = "../middle" }' "$dep")" ;;
        feature) extra="$(printf '%s\n\n[features]\nhelpers = ["tessera-compat/test-support"]' "$extra")" ;;
        nocompat) dep=''; extra='' ;;
        *) echo "check-release-features: unknown self-test case $2" >&2; return 2 ;;
    esac
    [ -d "$d/middle" ] || fixture_crate "$d/middle" tessera-middle
    fixture_crate "$d/client" "$client" "$dep" "$extra"
}

if [ "$self_test" = 1 ]; then
    echo "check-release-features: self-test (fixture workspaces)"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/check-release-features.XXXXXX")"
    trap 'rm -rf "$tmp"' EXIT
    failures=0
    # case | expected exit | pattern the output must contain
    while IFS='|' read -r case want out_pattern; do
        [ -n "$case" ] || continue
        d="$tmp/$case"
        if ! fixture "$d" "$case"; then failures=$((failures + 1)); continue; fi
        rc=0
        out="$(bash "$script" --root "$d" 2>&1)" || rc=$?
        if [ "$rc" = "$want" ] && grep -Eq -- "$out_pattern" <<<"$out"; then
            echo "  ok: $case (exit $rc)"
        else
            echo "  FAIL: $case: expected exit $want and output matching '$out_pattern', got exit $rc:" >&2
            sed 's/^/  | /' <<<"$out" >&2
            failures=$((failures + 1))
        fi
    done <<'CASES'
clean|0|\(--all-features\): test-support not in the normal graph
normal|1|\(default features\) enables tessera-compat's test-support
indirect|1|\(default features\) enables tessera-compat's test-support
feature|1|\(--all-features\) enables tessera-compat's test-support
nocompat|1|self-check failed
CASES
    if [ "$failures" = 0 ]; then
        echo "check-release-features: self-test passed"
        exit 0
    fi
    echo "check-release-features: self-test FAILED ($failures case(s))" >&2
    exit 1
fi

# ---- main -------------------------------------------------------------------------------

cd "$ws"
locked=()
[ "$ws" = "$repo" ] && locked=(--locked)

tree() {
    # --color never: CARGO_TERM_COLOR=always (as in CI) styles parts of the output with
    # escape codes, which the text matching below must not depend on.
    cargo tree --color never -p "$client" --target all ${locked[@]+"${locked[@]}"} "$@" 2>/dev/null || {
        echo "check-release-features: cargo tree $* failed in $ws" >&2
        exit 2
    }
}

failed=0

forward="$(tree -e features,normal)"
if ! grep -qF 'tessera-compat feature "default"' <<<"$forward"; then
    echo "check-release-features: self-check failed: tessera-compat is not in the normal" >&2
    echo "  graph of $client" >&2
    exit 1
fi

dev_tree="$(tree -e features,normal,dev -i tessera-compat)"
if ! grep -qF "$pattern" <<<"$dev_tree"; then
    echo "check-release-features: self-check failed: the dev graph of $client does not show" >&2
    echo "  '$pattern' (has cargo's output format changed?)" >&2
    failed=1
fi

for features in "" "--all-features"; do
    label="${features:-default features}"
    flags=(-e features,normal)
    [ -n "$features" ] && flags+=("$features")
    normal_tree="$(tree "${flags[@]}" -i tessera-compat)"
    if grep -qF "$pattern" <<<"$normal_tree"; then
        echo "check-release-features: $client ($label) enables tessera-compat's test-support" >&2
        echo "  feature through a normal dependency; the path to it:" >&2
        grep -A8 -F "$pattern" <<<"$normal_tree" >&2 || true
        failed=1
    else
        echo "check-release-features: $client ($label): test-support not in the normal graph"
    fi
done

exit "$failed"
