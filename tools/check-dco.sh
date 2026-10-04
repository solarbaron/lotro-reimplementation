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
#   Checks the commits in <base>..<head> (<head> defaults to HEAD).
#   Example: tools/check-dco.sh origin/main
# Exit status: 0 clean, 1 violations, 2 usage error.

set -euo pipefail

if [ $# -lt 1 ] || [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'
    [ $# -ge 1 ] && exit 0 || exit 2
fi
base="$1"
head="${2:-HEAD}"

if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
    echo "check-dco: unknown base revision: $base" >&2
    exit 2
fi

errors=0
warnings=0
count=0
for c in $(git rev-list --no-merges --reverse "$base..$head"); do
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

    files="$(git diff-tree --no-commit-id --name-only -r "$c")"
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
