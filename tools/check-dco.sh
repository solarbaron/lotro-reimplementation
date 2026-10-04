#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Commit-trailer check (CONTRIBUTING.md "Commits", docs/CLEAN_ROOM.md §8-9).
#
#   error    a non-merge commit without a `Signed-off-by:` trailer whose e-mail matches
#            the commit author (Developer Certificate of Origin 1.1 + clean-room
#            attestation)
#   warning  a commit that touches clean-room paths (crates/ apps/ fuzz/ tools/) or
#            docs/specs/ without a `Clean-Room-Role:` trailer, or with a role other than
#            spec | review | implementation
#   warning  a commit that touches crates/ or apps/ without a `Spec-Ref:` trailer
#            (only some such commits need one, so this is a reminder, not a failure)
#
# Usage: tools/check-dco.sh <base> [<head>]
#        tools/check-dco.sh --all [<head>]
#   <base> [<head>]  checks the commits in <base>..<head> (<head> defaults to HEAD). With
#                    `origin/main` or `@{push}` as <base> that is the UNPUSHED commits only;
#                    right after a clone or pull it checks nothing.
#   --all            checks every commit reachable from <head>, root commit included.
# Commits up to and including the baseline below were written before the trailer rules were
# enforced. They are still checked for Signed-off-by, but their trailer warnings are not
# reported. History is never rewritten to add trailers (docs/DEVELOPMENT.md §2).
# Exit status: 0 clean, 1 violations, 2 usage error.

set -euo pipefail

if [ $# -lt 1 ] || [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
    [ $# -ge 1 ] && exit 0 || exit 2
fi

# Last commit made before Clean-Room-Role/Spec-Ref trailers were enforced ("Multi-machine
# setup", 2026-10-04). Its ancestors only get the Signed-off-by check.
baseline=eb01347d726d2baa1cbdb16ceee5e0df7906825a

if [ "$1" = "--all" ]; then
    base=""
    head="${2:-HEAD}"
    range="$head"
else
    base="$1"
    head="${2:-HEAD}"
    if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
        echo "check-dco: unknown base revision: $base" >&2
        exit 2
    fi
    range="$base..$head"
fi
if ! git rev-parse --verify --quiet "$head^{commit}" >/dev/null; then
    echo "check-dco: unknown head revision: $head" >&2
    exit 2
fi
have_baseline=0
git cat-file -e "$baseline^{commit}" 2>/dev/null && have_baseline=1

errors=0
warnings=0
count=0
for c in $(git rev-list --no-merges --reverse "$range"); do
    count=$((count + 1))
    short="$(git rev-parse --short "$c")"
    subject="$(git log -1 --format=%s "$c")"
    author_email="$(git log -1 --format=%ae "$c" | tr '[:upper:]' '[:lower:]')"
    trailers="$(git log -1 --format=%B "$c" | git interpret-trailers --parse)"

    signoffs="$(printf '%s\n' "$trailers" | sed -n 's/^Signed-off-by: *//Ip')"
    if [ -z "$signoffs" ]; then
        echo "error: $short \"$subject\": missing Signed-off-by (use git commit -s)"
        errors=$((errors + 1))
    elif ! printf '%s\n' "$signoffs" | tr '[:upper:]' '[:lower:]' | grep -qF "<$author_email>"; then
        echo "error: $short \"$subject\": no Signed-off-by matches the author <$author_email>"
        errors=$((errors + 1))
    fi

    if [ "$have_baseline" = 1 ] && git merge-base --is-ancestor "$c" "$baseline"; then
        continue
    fi

    files="$(git diff-tree --no-commit-id --root --name-only -r "$c")"
    if printf '%s\n' "$files" | grep -qE '^(crates|apps|fuzz|tools|docs/specs)/'; then
        role="$(printf '%s\n' "$trailers" | sed -n 's/^Clean-Room-Role: *//Ip' | head -n 1)"
        case "$role" in
            spec|review|implementation) ;;
            "") echo "warning: $short \"$subject\": touches clean-room or spec paths without Clean-Room-Role"
                warnings=$((warnings + 1)) ;;
            *) echo "warning: $short \"$subject\": unknown Clean-Room-Role '$role'"
               warnings=$((warnings + 1)) ;;
        esac
    fi
    if printf '%s\n' "$files" | grep -qE '^(crates|apps)/' \
        && ! printf '%s\n' "$trailers" | grep -qi '^Spec-Ref:'; then
        echo "warning: $short \"$subject\": code change without Spec-Ref (needed for format/protocol code)"
        warnings=$((warnings + 1))
    fi
done

echo "check-dco: $count commit(s) checked, $errors error(s), $warnings warning(s)" >&2
[ "$errors" = 0 ]
