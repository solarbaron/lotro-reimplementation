#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Review of local tier-2 goldens after an install update (decision 0031, owner decision
# 2026-10-31). The golden helper (apps/client/tests/ui_common/goldens.rs) carries an
# unchanged picture over from the previous install version by itself and never blesses a
# changed one; it leaves one record per picture under
#   <base>/review/<version>/changed/*.tsv   package, test, function, name, old version,
#                                          old golden, actual, diff, differing pixels
#   <base>/review/<version>/carried/*.tsv   package, test, function, name, from version,
#                                          old golden, new golden
#   <base>/review/<version>/new/*.tsv       package, test, function, name, version,
#                                          blessed|missing, golden
#   <base>/review/last-version              the version of the latest check
# This tool prints the counts and the review list of one install version, writes the list
# to <base>/review/<version>.tsv, and with --html writes one local page showing old | new |
# diff side by side through file:// links (no picture is copied or embedded).
#
# Usage: tools/golden-review.sh [--base DIR] [--version V] [--html FILE] [--self-test]
#   --base DIR    the golden base (default ${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/
#                 tessera/goldens}, as the helper)
#   --version V   the install version to review; without it: the install of
#                 TESSERA_GAME_DIR, computed by tessera-dattool ($TESSERA_DATTOOL, else
#                 tessera-dattool on PATH) as the helper does (short level-1 content
#                 fingerprint, else schema-<short level-0>); without TESSERA_GAME_DIR or a
#                 dattool: <base>/review/last-version
#   --html FILE   also write the review page to FILE; refused inside a git work tree
#   --self-test   plant records in a temporary base and check the counts, the list, the page
#                 and the work-tree refusal
# Needs bash, coreutils (find, sort, mktemp, realpath), awk.
# Exit status: 0 done or self-test passed; 1 refused, no version found or self-test failed;
# 2 usage error.

set -euo pipefail
# Bash 5.2: `&` in a ${var//pattern/replacement} replacement means the match; not here.
shopt -u patsub_replacement 2>/dev/null || true

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/$(basename "${BASH_SOURCE[0]}")"

usage_error() {
    echo "golden-review: $1 (see the usage in the script's header)" >&2
    exit 2
}

default_base() {
    if [ -n "${TESSERA_GOLDEN_DIR:-}" ]; then
        printf '%s\n' "$TESSERA_GOLDEN_DIR"
    elif [ -n "${XDG_CACHE_HOME:-}" ]; then
        printf '%s\n' "$XDG_CACHE_HOME/tessera/goldens"
    else
        printf '%s\n' "${HOME:-/tmp}/.cache/tessera/goldens"
    fi
}

# Fails when $1 (or the nearest existing ancestor of it) lies in a git work tree.
outside_work_trees() {
    local probe="$1" real a
    while [ ! -e "$probe" ]; do
        local up
        up="$(dirname "$probe")"
        [ "$up" != "$probe" ] || break
        probe="$up"
    done
    real="$(realpath "$probe")"
    a="$real"
    while :; do
        if [ -e "$a/.git" ]; then
            echo "golden-review: $1 is inside a git work tree ($a); refused (decision 0031)" >&2
            return 1
        fi
        [ "$a" != "/" ] || break
        a="$(dirname "$a")"
    done
}

is_version() {
    [[ "$1" =~ ^(schema-)?[0-9a-f]{16}$ ]]
}

# The install version of $1 via the dattool (empty when it cannot be computed).
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

# Percent-encodes a path for a file:// URL.
url_path() {
    local s="$1" out="" c i
    local LC_ALL=C
    for ((i = 0; i < ${#s}; i++)); do
        c="${s:i:1}"
        case "$c" in
            [A-Za-z0-9/._~-]) out+="$c" ;;
            *) out+="$(printf '%%%02X' "'$c")" ;;
        esac
    done
    printf 'file://%s' "$out"
}

html_escape() {
    local s="$1"
    s="${s//&/&amp;}"
    s="${s//</&lt;}"
    s="${s//>/&gt;}"
    s="${s//\"/&quot;}"
    printf '%s' "$s"
}

# Concatenates the records of one kind (sorted by file name).
records() {
    local dir="$1"
    [ -d "$dir" ] || return 0
    find "$dir" -maxdepth 1 -type f -name '*.tsv' -print0 | sort -z |
        while IFS= read -r -d '' f; do
            head -n 1 "$f"
        done
}

count_lines() {
    if [ -z "$1" ]; then echo 0; else printf '%s\n' "$1" | wc -l | tr -d ' '; fi
}

write_html() {
    local file="$1" version="$2" changed="$3" ncarried="$4" nnew="$5" tmp
    tmp="$(mktemp "$(dirname "$file")/.golden-review.XXXXXX")"
    {
        printf '<!DOCTYPE html>\n<html><head><meta charset="utf-8">\n'
        printf '<title>Golden review %s</title>\n' "$(html_escape "$version")"
        printf '<style>body{font-family:sans-serif;background:#222;color:#ddd}'
        printf 'table{border-collapse:collapse}td{vertical-align:top;padding:4px}'
        printf 'img{image-rendering:pixelated;max-width:100%%;border:1px solid #555}'
        printf 'h2{font-size:1em;margin-top:2em}</style>\n</head><body>\n'
        printf '<h1>Golden review, install %s</h1>\n' "$(html_escape "$version")"
        printf '<p>%s changed (need the owner'"'"'s review), %s carried over, %s new.</p>\n' \
            "$(count_lines "$changed")" "$ncarried" "$nnew"
        printf '<p>Approve a picture by running its test with TESSERA_GOLDEN_BLESS_LOCAL=1.</p>\n'
        if [ -n "$changed" ]; then
            while IFS=$'\t' read -r pkg test fn name oldv old actual diff pixels; do
                printf '<h2>%s/%s/%s: %s (since %s, %s pixels differ)</h2>\n' \
                    "$(html_escape "$pkg")" "$(html_escape "$test")" "$(html_escape "$fn")" \
                    "$(html_escape "$name")" "$(html_escape "$oldv")" \
                    "$(html_escape "$pixels")"
                printf '<table><tr><th>old (%s)</th><th>new</th><th>diff</th></tr><tr>\n' \
                    "$(html_escape "$oldv")"
                for p in "$old" "$actual" "$diff"; do
                    if [ "$p" = "-" ] || [ -z "$p" ]; then
                        printf '<td>(none: the sizes differ)</td>\n'
                    else
                        printf '<td><a href="%s"><img src="%s" alt="%s"></a></td>\n' \
                            "$(url_path "$p")" "$(url_path "$p")" "$(html_escape "$p")"
                    fi
                done
                printf '</tr></table>\n'
            done <<< "$changed"
        fi
        printf '</body></html>\n'
    } > "$tmp"
    mv -f "$tmp" "$file"
}

review() {
    local base="$1" version="$2" html="$3" source="$4"
    local rdir="$base/review/$version"
    local changed carried new nblessed nmissing list
    changed="$(records "$rdir/changed")"
    carried="$(records "$rdir/carried")"
    new="$(records "$rdir/new")"
    nblessed=0
    nmissing=0
    if [ -n "$new" ]; then
        nblessed="$(printf '%s\n' "$new" | awk -F'\t' '$6 == "blessed"' | wc -l | tr -d ' ')"
        nmissing="$(printf '%s\n' "$new" | awk -F'\t' '$6 == "missing"' | wc -l | tr -d ' ')"
    fi
    echo "golden review: install version $version ($source)"
    echo "base: $base"
    echo "carried over: $(count_lines "$carried")"
    echo "changed: $(count_lines "$changed")"
    echo "new: $(count_lines "$new") (blessed $nblessed, missing $nmissing)"
    if [ -d "$base/review" ]; then
        outside_work_trees "$base/review" || return 1
        list="$base/review/$version.tsv"
        local tmp
        tmp="$(mktemp "$base/review/.review.XXXXXX")"
        {
            printf 'package\ttest\tfunction\tname\told_version\told_golden\tactual\tdiff\tdiffering_pixels\n'
            [ -z "$changed" ] || printf '%s\n' "$changed"
        } > "$tmp"
        mv -f "$tmp" "$list"
        echo "review list: $list"
    fi
    if [ -n "$changed" ]; then
        while IFS=$'\t' read -r pkg test fn name oldv old actual diff pixels; do
            echo "changed: $pkg/$test/$fn/$name since $oldv ($pixels pixels differ)"
            echo "  old:  $old"
            echo "  new:  $actual"
            echo "  diff: $diff"
        done <<< "$changed"
    fi
    if [ -n "$html" ]; then
        write_html "$html" "$version" "$changed" "$(count_lines "$carried")" \
            "$(count_lines "$new")"
        echo "review page: $html"
    fi
}

self_test() {
    local tmp fail=0 out
    tmp="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" RETURN
    local base="$tmp/gold base" v=0123456789abcdef
    local d="$base/review/$v"
    mkdir -p "$d/changed" "$d/carried" "$d/new"
    local g="$base/pkg/test/fn"
    printf 'pkg\ttest\tfn\ta<b\tfedcba9876543210\t%s\t%s\t%s\t12\n' \
        "$g/fedcba9876543210/a<b.png" "$g/$v/a<b.actual.png" "$g/$v/a<b.diff.png" \
        > "$d/changed/one.tsv"
    printf 'pkg\ttest\tfn\tsized\tfedcba9876543210\t%s\t%s\t-\t40\n' \
        "$g/fedcba9876543210/sized.png" "$g/$v/sized.actual.png" > "$d/changed/two.tsv"
    for n in 1 2 3; do
        printf 'pkg\ttest\tfn\tc%s\tfedcba9876543210\told\tnew\n' "$n" > "$d/carried/$n.tsv"
    done
    printf 'pkg\ttest\tfn\tn1\t%s\tblessed\tp\n' "$v" > "$d/new/1.tsv"
    printf 'pkg\ttest\tfn\tn2\t%s\tmissing\tp\n' "$v" > "$d/new/2.tsv"
    printf '%s\n' "$v" > "$base/review/last-version"
    mkdir -p "$tmp/out" "$tmp/repo/.git"

    check() {
        if ! grep -qF -- "$2" <<< "$1"; then
            echo "self-test: expected '$2' in the output of $3" >&2
            fail=1
        fi
    }
    out="$(env -u TESSERA_GAME_DIR "$script" --base "$base" --html "$tmp/out/page.html")" || {
        echo "self-test: the review failed" >&2
        fail=1
    }
    check "$out" "install version $v (last-version" "the review"
    check "$out" "carried over: 3" "the review"
    check "$out" "changed: 2" "the review"
    check "$out" "new: 2 (blessed 1, missing 1)" "the review"
    check "$out" "changed: pkg/test/fn/a<b since fedcba9876543210 (12 pixels differ)" "the review"
    local list="$base/review/$v.tsv"
    if [ ! -f "$list" ] || [ "$(wc -l < "$list" | tr -d ' ')" != 3 ]; then
        echo "self-test: the review list $list does not hold a header and two lines" >&2
        fail=1
    fi
    local page="$tmp/out/page.html"
    if [ -f "$page" ]; then
        check "$(cat "$page")" 'file://' "the page"
        check "$(cat "$page")" 'gold%20base' "the page (encoded path)"
        check "$(cat "$page")" 'a&lt;b' "the page (escaped name)"
        check "$(cat "$page")" '(none: the sizes differ)' "the page"
    else
        echo "self-test: no page written" >&2
        fail=1
    fi
    if env -u TESSERA_GAME_DIR "$script" --base "$base" --html "$tmp/repo/sub/page.html" \
        > /dev/null 2>&1; then
        echo "self-test: a page inside a git work tree was not refused" >&2
        fail=1
    fi
    if [ -e "$tmp/repo/sub/page.html" ]; then
        echo "self-test: a page was written inside a git work tree" >&2
        fail=1
    fi
    out="$("$script" --base "$base" --version fedcba9876543210 2>/dev/null)" || fail=1
    check "$out" "changed: 0" "the review of another version"
    if "$script" --base "$base" --version not-a-version > /dev/null 2>&1; then
        echo "self-test: a malformed version was accepted" >&2
        fail=1
    fi
    if [ "$fail" -ne 0 ]; then
        echo "golden-review: self-test FAILED" >&2
        return 1
    fi
    echo "golden-review: self-test passed"
}

base=""
version=""
html=""
run_self_test=0
while [ $# -gt 0 ]; do
    case "$1" in
        --base)
            [ $# -ge 2 ] || usage_error "--base needs a directory"
            base="$2"; shift 2 ;;
        --version)
            [ $# -ge 2 ] || usage_error "--version needs a version"
            version="$2"; shift 2 ;;
        --html)
            [ $# -ge 2 ] || usage_error "--html needs a file"
            html="$2"; shift 2 ;;
        --self-test) run_self_test=1; shift ;;
        -h|--help) sed -n '2,32p' "$script"; exit 0 ;;
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
if [ -z "$version" ] && [ -f "$base/review/last-version" ]; then
    version="$(head -n 1 "$base/review/last-version")"
    source="last-version of the latest check"
    if [ -n "${TESSERA_GAME_DIR:-}" ]; then
        echo "golden-review: no tessera-dattool to compute the version of TESSERA_GAME_DIR;" \
            "using the latest check's (set TESSERA_DATTOOL or pass --version)" >&2
    fi
fi
[ -n "$version" ] || {
    echo "golden-review: no install version (pass --version, or run a golden test first)" >&2
    exit 1
}
is_version "$version" || usage_error "$version is not an install version"
if [ -f "$base/review/last-version" ]; then
    last="$(head -n 1 "$base/review/last-version")"
    if [ "$last" != "$version" ]; then
        echo "note: the latest check ran on install version $last" >&2
    fi
fi
if [ -n "$html" ]; then
    case "$html" in
        /*) ;;
        *) html="$PWD/$html" ;;
    esac
    outside_work_trees "$html" || exit 1
    mkdir -p "$(dirname "$html")"
fi
review "$base" "$version" "$html" "$source"
