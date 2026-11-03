#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Census drift after an install update (decision 0045). Real-data tests record their counts
# with the census helper (crates/testkit, `tessera_testkit::Census`) against a local
# baseline per install version:
#   <base>/<package>/<test>/<function>/<version>.tsv     name, value, spec
# The first run on a new install version writes the baseline and, where counts moved, one
# drift file per test (written atomically, so parallel test binaries never share a file):
#   <base>/drift/<version>/<package>/<test>/<function>.tsv
#       package, test, function, name, against (previous version or `spec`), previous,
#       observed, spec   (`-` for none)
#   <base>/drift/last-version                             the version of the latest census
# This tool prints the drift rows of one install version. Drift is information, not a
# failure: nothing needs approval unless a rule check of a test failed.
#
# Usage: tools/census-drift.sh [--base DIR] [--version V] [--self-test]
#   --base DIR    the census base (default ${TESSERA_CENSUS_DIR:-${XDG_CACHE_HOME:-~/.cache}/
#                 tessera/census}, as the helper)
#   --version V   the install version; without it: the install of TESSERA_GAME_DIR, computed
#                 by tessera-dattool ($TESSERA_DATTOOL, else tessera-dattool on PATH) as the
#                 helper does (short level-1 content fingerprint, else schema-<short
#                 level-0>); without TESSERA_GAME_DIR or a dattool: <base>/drift/last-version
#   --self-test   plant baselines and drift files in a temporary base and check the output
# Needs bash, coreutils (find, sort, mktemp), awk.
# Exit status: 0 done or self-test passed; 1 no version found or self-test failed;
# 2 usage error.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/$(basename "${BASH_SOURCE[0]}")"

usage_error() {
    echo "census-drift: $1 (see the usage in the script's header)" >&2
    exit 2
}

default_base() {
    if [ -n "${TESSERA_CENSUS_DIR:-}" ]; then
        printf '%s\n' "$TESSERA_CENSUS_DIR"
    elif [ -n "${XDG_CACHE_HOME:-}" ]; then
        printf '%s\n' "$XDG_CACHE_HOME/tessera/census"
    else
        printf '%s\n' "${HOME:-/tmp}/.cache/tessera/census"
    fi
}

is_version() {
    [[ "$1" =~ ^(schema-)?[0-9a-f]{16}$ ]]
}

# The install version of $1 via the dattool (empty when it cannot be computed); the same
# rule as tools/golden-review.sh and the helper.
dattool_version() {
    local tool="${TESSERA_DATTOOL:-}" out
    if [ -z "$tool" ]; then
        tool="$(command -v tessera-dattool || true)"
    fi
    [ -n "$tool" ] || return 0
    out="$("$tool" --game-dir "$1" detect 2>/dev/null)" || return 0
    printf '%s\n' "$out" | awk '
        /^Install #/ { n++ }
        n == 1 && $1 == "schema" && $2 == "fingerprint" { schema = $3 }
        n == 1 && $1 == "content" && $2 == "fingerprint" {
            content = $3
            if (match($0, /records of [0-9]+\//)) {
                s = substr($0, RSTART + 11, RLENGTH - 12); with_iter = s + 0
            }
        }
        n == 1 && $1 == "install" && $2 == "stamp" { stamp = $3 }
        END {
            if (content != "" && (with_iter > 0 || (stamp != "" && stamp != "absent")))
                print content
            else if (schema != "")
                print "schema-" schema
        }'
}

report() {
    local base="$1" version="$2" source="$3"
    local ddir="$base/drift/$version" nbase=0 rows="" ntests=0 nrows=0
    if [ -d "$base" ]; then
        nbase="$(find "$base" -path "$base/drift" -prune -o -type f -name "$version.tsv" -print |
            wc -l | tr -d ' ')"
    fi
    if [ -d "$ddir" ]; then
        ntests="$(find "$ddir" -type f -name '*.tsv' | wc -l | tr -d ' ')"
        rows="$(find "$ddir" -type f -name '*.tsv' -print0 | sort -z |
            while IFS= read -r -d '' f; do grep -v '^#' "$f" || true; done)"
    fi
    [ -z "$rows" ] || nrows="$(printf '%s\n' "$rows" | wc -l | tr -d ' ')"
    echo "census drift: install version $version ($source)"
    echo "base: $base"
    echo "baselines: $nbase"
    echo "tests with drift: $ntests"
    echo "drifted counts: $nrows"
    if [ -n "$rows" ]; then
        printf '%s\n' "$rows" | awk -F'\t' '
            {
                key = $1 "/" $2 "/" $3
                if (key != last) { print key; last = key }
                if ($5 == "spec")
                    printf "  %s: %s (spec %s)\n", $4, $7, $8
                else
                    printf "  %s: %s -> %s since %s (spec %s)\n", $4, $6, $7, $5, $8
            }'
    fi
    echo "Drift is not a failure; only a failed rule check of a test needs work."
}

self_test() {
    local tmp fail=0 out
    tmp="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" RETURN
    local base="$tmp/census base" v=0123456789abcdef old=fedcba9876543210
    mkdir -p "$base/pkg/real_data/one" "$base/pkg/real_data/two" "$base/other/t/f"
    printf '# h\na\t1\t-\n' > "$base/pkg/real_data/one/$v.tsv"
    printf '# h\na\t1\t-\n' > "$base/pkg/real_data/one/$old.tsv"
    printf '# h\nb\t2\t3\n' > "$base/pkg/real_data/two/$v.tsv"
    printf '# h\nc\t5\t-\n' > "$base/other/t/f/$v.tsv"
    mkdir -p "$base/drift/$v/pkg/real_data" "$base/drift/$old/other/t"
    {
        printf '# Tessera census drift\n'
        printf 'pkg\treal_data\tone\ta\t%s\t0\t1\t-\n' "$old"
        printf 'pkg\treal_data\tone\tnew name\t%s\t-\t4\t-\n' "$old"
    } > "$base/drift/$v/pkg/real_data/one.tsv"
    printf 'pkg\treal_data\ttwo\tb\tspec\t-\t2\t3\n' > "$base/drift/$v/pkg/real_data/two.tsv"
    printf 'other\tt\tf\tz\tspec\t-\t1\t2\n' > "$base/drift/$old/other/t/f.tsv"
    printf '%s\n' "$v" > "$base/drift/last-version"

    check() {
        if ! grep -qF -- "$2" <<< "$1"; then
            echo "self-test: expected '$2' in the output of $3" >&2
            fail=1
        fi
    }
    out="$(env -u TESSERA_GAME_DIR "$script" --base "$base")" || {
        echo "self-test: the report failed" >&2
        fail=1
    }
    check "$out" "install version $v (last-version" "the report"
    check "$out" "baselines: 3" "the report"
    check "$out" "tests with drift: 2" "the report"
    check "$out" "drifted counts: 3" "the report"
    check "$out" "pkg/real_data/one" "the report"
    check "$out" "  a: 0 -> 1 since $old (spec -)" "the report"
    check "$out" "  new name: - -> 4 since $old (spec -)" "the report"
    check "$out" "  b: 2 (spec 3)" "the report"
    if grep -qF "z:" <<< "$out"; then
        echo "self-test: the report listed another version's drift" >&2
        fail=1
    fi
    out="$("$script" --base "$base" --version "$old" 2>/dev/null)" || fail=1
    check "$out" "baselines: 1" "the report of another version"
    check "$out" "drifted counts: 1" "the report of another version"
    check "$out" "  z: 1 (spec 2)" "the report of another version"
    out="$("$script" --base "$base" --version schema-00000000000000aa 2>/dev/null)" || fail=1
    check "$out" "drifted counts: 0" "the report of a version without drift"
    if "$script" --base "$base" --version not-a-version > /dev/null 2>&1; then
        echo "self-test: a malformed version was accepted" >&2
        fail=1
    fi
    if env -u TESSERA_GAME_DIR "$script" --base "$tmp/empty" > /dev/null 2>&1; then
        echo "self-test: a base without any census gave a version" >&2
        fail=1
    fi
    if [ "$fail" -ne 0 ]; then
        echo "census-drift: self-test FAILED" >&2
        return 1
    fi
    echo "census-drift: self-test passed"
}

base=""
version=""
run_self_test=0
while [ $# -gt 0 ]; do
    case "$1" in
        --base)
            [ $# -ge 2 ] || usage_error "--base needs a directory"
            base="$2"; shift 2 ;;
        --version)
            [ $# -ge 2 ] || usage_error "--version needs a version"
            version="$2"; shift 2 ;;
        --self-test) run_self_test=1; shift ;;
        -h|--help) sed -n '2,28p' "$script"; exit 0 ;;
        *) usage_error "unknown option $1" ;;
    esac
done

if [ "$run_self_test" -eq 1 ]; then
    self_test
    exit $?
fi

[ -n "$base" ] || base="$(default_base)"
source="--version"
if [ -z "$version" ] && [ -n "${TESSERA_GAME_DIR:-}" ]; then
    version="$(dattool_version "$TESSERA_GAME_DIR")"
    source="TESSERA_GAME_DIR via tessera-dattool"
fi
if [ -z "$version" ] && [ -f "$base/drift/last-version" ]; then
    version="$(head -n 1 "$base/drift/last-version")"
    source="last-version of the latest census"
    if [ -n "${TESSERA_GAME_DIR:-}" ]; then
        echo "census-drift: no tessera-dattool to compute the version of TESSERA_GAME_DIR;" \
            "using the latest census's (set TESSERA_DATTOOL or pass --version)" >&2
    fi
fi
[ -n "$version" ] || {
    echo "census-drift: no install version (pass --version, or run a census test first)" >&2
    exit 1
}
is_version "$version" || usage_error "$version is not an install version"
if [ -f "$base/drift/last-version" ]; then
    last="$(head -n 1 "$base/drift/last-version")"
    if [ "$last" != "$version" ]; then
        echo "note: the latest census ran on install version $last" >&2
    fi
fi
report "$base" "$version" "$source"
