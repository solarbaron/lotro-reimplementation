#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# REUSE-IgnoreStart
# SPDX header check (CLAUDE.md: "Every source file starts with
# `// SPDX-License-Identifier: GPL-3.0-or-later`").
#
#   error    Rust sources whose first line is not exactly that header; other source files
#            (.wgsl .sh .py .lua .hexpat .ksy .yml .yaml) without
#            "SPDX-License-Identifier: GPL-3.0-or-later" in their first 5 lines
#   warning  .toml and .md files without any SPDX-License-Identifier in their first 5
#            lines (errors with --strict). Generated or comment-less files (Cargo.lock,
#            *.json, .gitignore, LICENSES/*) are skipped; REUSE.toml should cover them.
#
# The full REUSE 3.x check (`reuse lint`, using LICENSES/ and REUSE.toml at the
# repository root) runs in CI as well.
#
# Usage: tools/check-spdx.sh [--strict] [--root DIR]
# Exit status: 0 clean, 1 violations, 2 usage error.
# REUSE-IgnoreEnd

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
strict=0
while [ $# -gt 0 ]; do
    case "$1" in
        --strict) strict=1; shift ;;
        --root) root="$(cd "${2:?--root needs a directory}" && pwd)"; shift 2 ;;
        -h|--help) sed -n '2,20p' "$0" | grep -v REUSE-Ignore | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "check-spdx: unknown argument: $1" >&2; exit 2 ;;
    esac
done

cd "$root"
# REUSE-IgnoreStart (the patterns below are not license expressions)
want='SPDX-License-Identifier: GPL-3.0-or-later'
rust_first="// $want"

list_files() {
    if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        git ls-files -z --cached --others --exclude-standard
    else
        find . \( -name .git -o -name target -o -path ./fuzz/corpus -o -path ./fuzz/artifacts \) -prune \
            -o -type f -print0 | sed -z 's|^\./||'
    fi
}

errors=0
warnings=0
checked=0
while IFS= read -r -d '' f; do
    [ -f "$f" ] || continue
    case "$f" in
        LICENSES/*|*/Cargo.lock|Cargo.lock|*.json|.gitignore|*/.gitignore|.gitattributes) continue ;;
    esac
    case "$f" in
        *.rs)
            checked=$((checked + 1))
            first="$(head -n 1 "$f" | tr -d '\r')"
            if [ "$first" != "$rust_first" ]; then
                echo "error: $f:1: first line must be '$rust_first'"
                errors=$((errors + 1))
            fi
            ;;
        *.wgsl|*.sh|*.py|*.lua|*.hexpat|*.ksy|*.yml|*.yaml)
            checked=$((checked + 1))
            if ! head -n 5 "$f" | grep -qF "$want"; then
                echo "error: $f: no '$want' in the first 5 lines"
                errors=$((errors + 1))
            fi
            ;;
        *.toml|*.md)
            checked=$((checked + 1))
            if ! head -n 5 "$f" | grep -q 'SPDX-License-Identifier:'; then
                if [ "$strict" = 1 ]; then
                    echo "error: $f: no SPDX-License-Identifier in the first 5 lines"
                    errors=$((errors + 1))
                else
                    echo "warning: $f: no SPDX-License-Identifier in the first 5 lines"
                    warnings=$((warnings + 1))
                fi
            fi
            ;;
    esac
done < <(list_files)

echo "check-spdx: $checked files checked, $errors error(s), $warnings warning(s)" >&2
[ "$errors" = 0 ]
# REUSE-IgnoreEnd
