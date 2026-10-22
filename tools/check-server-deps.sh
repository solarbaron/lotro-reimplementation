#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Server dependency guard (docs/specs/server-content.md §3.2 and §10 E1,
# docs/specs/scenario-feed.md §3.1, decision 0024 §2.3 and P0 exit criterion
# 4). tessera-server must never link a crate that decodes game data, tessera-gamedata or the
# scenario feed (tessera-net), and must contain no code taken from one.
#
# Allowlist. The only non-crates.io packages allowed in tessera-server's graph are
# tessera-server and tessera-protocol. Every other package that does not come from crates.io
# fails: any workspace crate (a new data crate is caught without editing this script), and
# any path or git package outside the workspace ([workspace] exclude, out-of-tree paths, git
# copies, [patch] replacements). Workspace crates are the roots of `cargo tree --workspace`.
#
# Graph. The union of three views, every target (--target all):
#   1. cargo tree -p tessera-server -e normal,build,dev, default features
#   2. the same with --all-features
#   3. tessera-server's part of `cargo tree --workspace --all-features -e normal,build`:
#      the features a workspace build turns on in tessera-protocol (or any dependency)
#      through another member (feature unification) are counted
# Dev-dependencies of tessera-server are checked too (decision 0024 §2.3 names plain
# `cargo tree -p tessera-server`, server-content.md §10 E1 says "MUST NOT depend on"):
# examples and tests of the server are server builds. In this repository cargo runs with
# --locked, as in CI (fixture workspaces of the self-test have no lockfile).
#
# Server hosts. Every other workspace crate whose normal or build graph (workspace build,
# all features) contains tessera-server runs a server in-process and is a "server build"
# (server-content.md, scope; today tessera-loadtest). Each one is checked with the
# allowlist tessera-server, tessera-protocol and tessera-net, the package itself. A crate
# that has tessera-server only as a dev-dependency (tessera-client's tests) is not a host.
#
# Source scan. In the allowed path packages of each graph (tessera-server, tessera-protocol,
# and for hosts tessera-net and the host), a `#[path = "..."]` or include!/include_str!/
# include_bytes! whose target leaves that crate's directory fails, and so does an include
# whose argument is not a plain string literal (except one built from env!("OUT_DIR")).
#
# tessera-net: scenario-feed.md §3.1 still lists `tessera-net` without the feature
# `scenario` as allowed in tessera-server's graph. This guard refuses tessera-net in any
# form, because the exception does not hold in a workspace build: tessera-client turns on
# tessera-net's default features (`scenario` included), and feature unification gives them
# to the server too. Raised as an OPEN in §3.1; the spec's independent review of revision 3
# (2026-10-09) agreed (drop the exception) and confirmed the dev-dependency and in-process
# host checks below; the spec text follows at its next revision.
#
# Usage: tools/check-server-deps.sh [-p PACKAGE] [--root DIR] [--self-test]
#   (no option)    check tessera-server and every server host
#   -p PACKAGE     check PACKAGE alone, as a server (allowlist: PACKAGE, tessera-protocol)
#   --root DIR     the workspace to check (default: this repository); the self-test uses it
#                  on throwaway fixture workspaces
#   --self-test    build fixture workspaces in a temporary directory, plant one violation
#                  in each (normal, feature-gated, build, dev and cfg(windows) dependency,
#                  unification through tessera-protocol, tessera-net, an unknown member, an
#                  excluded path crate, a git crate, a server host, #[path], include_bytes!)
#                  and check that each is caught and the clean tree passes
# Needs bash (no bash 4 features), cargo, awk; git for the git self-test case (else skipped).
# Exit status: 0 clean or self-test passed; 1 forbidden crate or self-test failed; 2 usage or cargo error.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/$(basename "${BASH_SOURCE[0]}")"
repo="$(cd "$here/.." && pwd)"

server="tessera-server"
server_allow="tessera-protocol"
host_allow="tessera-server tessera-protocol tessera-net"

usage_error() {
    echo "check-server-deps: $1 (see --help)" >&2
    exit 2
}

package=""
ws="$repo"
self_test=0
while [ $# -gt 0 ]; do
    case "$1" in
        -p|--package)
            [ $# -ge 2 ] || usage_error "-p needs a package name"
            package="$2"; shift 2 ;;
        --root)
            [ $# -ge 2 ] || usage_error "--root needs a directory"
            [ -f "$2/Cargo.toml" ] || usage_error "--root: no Cargo.toml in '$2'"
            ws="$(cd "$2" && pwd)"; shift 2 ;;
        --self-test) self_test=1; shift ;;
        -h|--help) sed -n '4,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) usage_error "unknown argument '$1'" ;;
    esac
done

# Why a package must not be in a server's graph. $1 = name, $2 = 1 if workspace member,
# $3 = source, $4 = the allowlist of this check.
reason_for() {
    if [ "$2" != 1 ]; then
        echo "not a workspace member and not from crates.io (source $3): a local path or git package can carry data-decoding code"
        return
    fi
    case "$1" in
        tessera-gamedata) echo "gameplay-data crate: typed stores over install records, scenario selector resolution (decision 0024 §3)" ;;
        tessera-net)      echo "holds the scenario feed and the sandbox backend (decision 0024 §2.3, §4); refused even without \`scenario\` (see --help, deviation from scenario-feed.md §3.1)" ;;
        tessera-dat)      echo "data-decoding crate: game archive container reader" ;;
        tessera-formats)  echo "data-decoding crate: record decoders over the archives" ;;
        tessera-havok)    echo "data-decoding crate: skeleton and clip tagfile reader" ;;
        tessera-props)    echo "data-decoding crate: property registry, property sets, DID maps" ;;
        tessera-text)     echo "data-decoding crate: string tables and text resolution" ;;
        tessera-wstate)   echo "data-decoding crate: class library and object graph" ;;
        tessera-wvm)      echo "data-decoding crate: class-library bytecode" ;;
        tessera-physics)  echo "data-decoding crate: collision built from decoded records (links tessera-formats)" ;;
        tessera-world)    echo "data-decoding crate: landblocks, scenery, water, sky from the archives" ;;
        tessera-terrain)  echo "data-decoding crate: terrain meshing from landblock records" ;;
        tessera-mesh)     echo "data-decoding crate: mesh records and character assembly" ;;
        tessera-material) echo "data-decoding crate: materials, surfaces, textures" ;;
        tessera-anim)     echo "data-decoding crate: skeletons and animation clips" ;;
        tessera-assets)   echo "data-decoding crate: decoded-asset cache and workers over the archives (links tessera-formats)" ;;
        tessera-audio)    echo "data-decoding crate: sound payloads" ;;
        tessera-install)  echo "reads game installations (rule 1: a server reads no install)" ;;
        tessera-scriptgraph) echo "runs decoded script tables and scriptlets (client runtime)" ;;
        *)                echo "workspace crate not on the allowlist of this check ($4)" ;;
    esac
}

cargo_tree() {
    local lock=""
    [ -f "$ws/Cargo.lock" ] && [ "$ws" = "$repo" ] && lock="--locked"
    # cargo's own error messages go to stderr unchanged.
    # shellcheck disable=SC2086
    cargo tree --manifest-path "$ws/Cargo.toml" $lock "$@" --prefix depth --format '{p}'
}

# stdin: `cargo tree --prefix depth --format '{p}'` output.
# Prints one "name<TAB>source" line per distinct package (source "-" = crates.io).
# mode=all: every package; mode=from ROOT: packages reachable from the depth-0 ROOT;
# mode=hosts ROOT: depth-0 packages other than ROOT from which ROOT is reachable;
# mode=roots: names of the depth-0 packages (the workspace members for --workspace).
graph() {
    awk -v mode="$1" -v root="${2:-}" '
        function src_of(k,    rest) {
            rest = k
            sub(/^[^ ]+ v[^ ]+/, "", rest)
            sub(/ \(proc-macro\)$/, "", rest)
            if (rest ~ /^ \(.*\)$/) return substr(rest, 3, length(rest) - 3)
            return "-"
        }
        /^[0-9]+/ {
            match($0, /^[0-9]+/)
            d = substr($0, 1, RLENGTH) + 0
            k = substr($0, RLENGTH + 1)
            sub(/ \(\*\)$/, "", k)
            if (k == "") next
            if (!(k in id)) { id[k] = ++n; key[n] = k; split(k, w, " "); name[n] = w[1] }
            i = id[k]
            st[d] = i
            if (d == 0) { isroot[i] = 1; if (name[i] == root) r = i }
            else {
                p = st[d - 1]
                if (!((p, i) in seen)) {
                    seen[p, i] = 1
                    adj[p] = adj[p] " " i
                    radj[i] = radj[i] " " p
                }
            }
        }
        function walk(start, a,    q, h, t, c, m, x, j) {
            h = 1; t = 1; q[1] = start; vis[start] = 1
            while (h <= t) {
                c = q[h++]
                m = split(a[c], x, " ")
                for (j = 1; j <= m; j++) if (!(x[j] in vis)) { vis[x[j]] = 1; q[++t] = x[j] }
            }
        }
        END {
            if (mode == "all") {
                for (i = 1; i <= n; i++) print name[i] "\t" src_of(key[i])
            } else if (mode == "roots") {
                for (i = 1; i <= n; i++) if (i in isroot) print name[i]
            } else {
                if (r == "") exit 3
                if (mode == "from") {
                    walk(r, adj)
                    for (i = 1; i <= n; i++) if (i in vis) print name[i] "\t" src_of(key[i])
                } else {
                    walk(r, radj)
                    for (i = 1; i <= n; i++) if ((i in vis) && (i in isroot) && i != r) print name[i]
                }
            }
        }'
}

# scan_sources CRATE_DIR: prints offending #[path] / include lines, one per line.
scan_sources() {
    local dir="$1" f
    [ -d "$dir" ] || return 0
    find "$dir" \( -path "$dir/target" -o -name '.?*' \) -prune -o -type f -name '*.rs' -print |
    LC_ALL=C sort | while IFS= read -r f; do
        awk -v file="$f" -v crate="$dir" '
            function norm(p,    n, x, i, out, s, k) {
                n = split(p, x, "/"); k = 0
                for (i = 1; i <= n; i++) {
                    if (x[i] == "" || x[i] == ".") continue
                    if (x[i] == "..") { if (k > 0) k--; continue }
                    s[++k] = x[i]
                }
                out = ""
                for (i = 1; i <= k; i++) out = out "/" s[i]
                return out == "" ? "/" : out
            }
            function report(what, lit,    target, dirn) {
                if (lit ~ /^\//) target = norm(lit)
                else { dirn = file; sub(/\/[^\/]*$/, "", dirn); target = norm(dirn "/" lit) }
                if (index(target "/", norm(crate) "/") != 1)
                    printf "%s:%d: %s \"%s\" leaves %s\n", file, NR0, what, lit, crate
            }
            { raw[NR] = $0; l = $0; sub(/\/\/.*/, "", l); code[NR] = l }
            END {
                for (NR0 = 1; NR0 <= NR; NR0++) {
                    s = code[NR0] " " code[NR0 + 1] " " code[NR0 + 2]
                    lim = length(code[NR0])
                    off = 0
                    while (match(substr(s, off + 1), /#\[ *path *=|include(_str|_bytes)? *!/)) {
                        start = off + RSTART
                        if (start > lim) break
                        rest = substr(s, start)
                        if (rest ~ /^#\[ *path *=/) {
                            if (match(rest, /^#\[ *path *= *"[^"]*"/)) {
                                lit = substr(rest, 1, RLENGTH); sub(/^[^"]*"/, "", lit); sub(/"$/, "", lit)
                                report("#[path]", lit)
                            } else printf "%s:%d: #[path] without a plain string literal\n", file, NR0
                        } else {
                            m = rest; sub(/^include(_str|_bytes)? *!/, "", m); mac = substr(rest, 1, length(rest) - length(m)); gsub(/ /, "", mac)
                            if (match(m, /^ *\( *"[^"]*" *\)/)) {
                                lit = substr(m, 1, RLENGTH); sub(/^[^"]*"/, "", lit); sub(/" *\)$/, "", lit)
                                report(mac, lit)
                            } else if (m ~ /^ *\(/ && m !~ /^ *\( *concat! *\( *env! *\( *"OUT_DIR" *\)/) {
                                printf "%s:%d: %s with a path that is not a plain string literal\n", file, NR0, mac
                            }
                        }
                        off = start + 1
                    }
                }
            }' "$f"
    done
}

members=""
unified=""

load_workspace() {
    if ! unified="$(cargo_tree --workspace --all-features -e normal,build --target all)"; then
        echo "check-server-deps: cargo tree --workspace failed in $ws" >&2
        return 2
    fi
    members="$(graph roots <<< "$unified")"
    if [ -z "$members" ]; then
        echo "check-server-deps: cargo tree --workspace printed no workspace crates (format changed?)" >&2
        return 2
    fi
}

is_member() { grep -qx -- "$1" <<< "$members"; }

# check ROOT "ALLOWED ...": prints findings; returns 0 clean, 1 forbidden packages, 2 failure.
check() {
    local root="$1" allow="$2" mode tree list rows="" bad dirs line name src label scan
    if ! is_member "$root"; then
        echo "check-server-deps: '$root' is not a workspace crate" >&2
        return 2
    fi
    for mode in default all; do
        if [ "$mode" = all ]; then
            tree="$(cargo_tree -p "$root" -e normal,build,dev --target all --all-features)" || {
                echo "check-server-deps: cargo tree failed for $root (all features)" >&2; return 2; }
            label="all features"
        else
            tree="$(cargo_tree -p "$root" -e normal,build,dev --target all)" || {
                echo "check-server-deps: cargo tree failed for $root (default features)" >&2; return 2; }
            label="default features"
        fi
        list="$(graph from "$root" <<< "$tree")" || {
            echo "check-server-deps: cargo tree output for $root has no root package (format changed?)" >&2
            return 2; }
        rows="$rows$(awk -v l="$label" '{ print $0 "\t" l }' <<< "$list")"$'\n'
    done
    list="$(graph from "$root" <<< "$unified")" || {
        echo "check-server-deps: $root missing from the workspace graph" >&2; return 2; }
    rows="$rows$(awk '{ print $0 "\tworkspace build" }' <<< "$list")"$'\n'

    # Forbidden: not the root, not allowlisted, not from crates.io.
    bad="$(awk -F'\t' -v root="$root" -v allow=" $allow " '
        NF >= 3 && $1 != root && index(allow, " " $1 " ") == 0 && $2 != "-" {
            k = $1 "\t" $2
            if (!(k in m)) { order[++n] = k; m[k] = $3 } else if (index(", " m[k] ", ", ", " $3 ", ") == 0) m[k] = m[k] ", " $3
        }
        END { for (i = 1; i <= n; i++) print order[i] "\t" m[order[i]] }' <<< "$rows" | LC_ALL=C sort)"

    # Source scan of the allowed path packages in the graph.
    dirs="$(awk -F'\t' -v root="$root" -v allow=" $allow " '
        NF >= 3 && ($1 == root || index(allow, " " $1 " ") > 0) && $2 ~ /^\// { print $2 }' <<< "$rows" | LC_ALL=C sort -u)"
    scan=""
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        scan="$scan$(scan_sources "$line")"
        [ -n "$scan" ] && scan="$scan"$'\n'
    done <<< "$dirs"
    scan="$(sed '/^$/d' <<< "$scan")"

    if [ -z "$bad" ] && [ -z "$scan" ]; then
        echo "check-server-deps: ok: $root links no package outside crates.io other than: ${allow// /, } (default and all features, workspace build; normal, build and dev edges; all targets; no #[path]/include! leaving its crates)"
        return 0
    fi
    echo "check-server-deps: FAIL: $root links packages that a server must not link" >&2
    echo "  (server-content.md §3.2 and §10 E1, decision 0024 §2.3):" >&2
    while IFS="$(printf '\t')" read -r name src label; do
        [ -n "$name" ] || continue
        if is_member "$name"; then
            echo "  $name [$label]: $(reason_for "$name" 1 "$src" "${allow// /, }")" >&2
        else
            echo "  $name [$label]: $(reason_for "$name" 0 "$src" "${allow// /, }")" >&2
        fi
    done <<< "$bad"
    if [ -n "$bad" ]; then
        echo "  Show the path with: cargo tree -p $root -e normal,build,dev --target all --all-features -i <crate>" >&2
        echo "  (workspace build: cargo tree --workspace --all-features -e normal,build --target all -i <crate>)" >&2
    fi
    if [ -n "$scan" ]; then
        echo "  Code taken from outside the allowed crates:" >&2
        sed 's/^/  /' <<< "$scan" >&2
    fi
    return 1
}

# ---- self-test --------------------------------------------------------------------------

fixture_crate() { # DIR NAME [DEPENDENCY-TOML]
    mkdir -p "$1/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n[dependencies]\n%s\n' \
        "$2" "${3:-}" > "$1/Cargo.toml"
    : > "$1/src/lib.rs"
}

fixture_base() { # DIR
    local d="$1"
    mkdir -p "$d"
    printf '[workspace]\nresolver = "3"\nmembers = ["server", "protocol", "formats", "net", "newcrate", "host"]\nexclude = ["outside"]\n' \
        > "$d/Cargo.toml"
    [ -f "$repo/rust-toolchain.toml" ] && cp "$repo/rust-toolchain.toml" "$d/"
    fixture_crate "$d/protocol" tessera-protocol
    fixture_crate "$d/server" tessera-server 'tessera-protocol = { path = "../protocol" }'
    fixture_crate "$d/formats" tessera-formats
    fixture_crate "$d/newcrate" tessera-newcrate
    fixture_crate "$d/net" tessera-net 'tessera-protocol = { path = "../protocol" }'
    fixture_crate "$d/host" tessera-loadhost \
        "$(printf 'tessera-server = { path = "../server" }\ntessera-net = { path = "../net" }')"
    fixture_crate "$d/outside/formatscopy" fmtcopy
    # An include inside the crate's own directory is fine.
    echo 'pub const D: &str = include_str!("../data.txt");' > "$d/server/src/lib.rs"
    echo fixture > "$d/server/data.txt"
    # A dev-dependency of an allowed crate is not linked into the server: must stay clean.
    printf '\n[dev-dependencies]\ntessera-formats = { path = "../formats" }\n' >> "$d/protocol/Cargo.toml"
}

append() { printf '%s\n' "$2" >> "$1"; }

# plant CASE DIR: apply one violation to a fresh fixture.
plant() {
    local d="$2" s="$2/server/Cargo.toml"
    case "$1" in
        clean) ;;
        normal) append "$s" 'tessera-formats = { path = "../formats" }' ;;
        feature) append "$s" "$(printf 'tessera-formats = { path = "../formats", optional = true }\n\n[features]\nnames = ["dep:tessera-formats"]')" ;;
        build) append "$s" "$(printf '\n[build-dependencies]\ntessera-formats = { path = "../formats" }')"
               echo 'fn main() {}' > "$d/server/build.rs" ;;
        dev) append "$s" "$(printf '\n[dev-dependencies]\ntessera-formats = { path = "../formats" }')" ;;
        target) append "$s" "$(printf '\n[target.'"'"'cfg(windows)'"'"'.dependencies]\ntessera-formats = { path = "../formats" }')" ;;
        unify) # tessera-protocol gets an optional data dependency; tessera-net turns it on.
               awk '{ print } /^\[dependencies\]$/ { print "tessera-formats = { path = \"../formats\", optional = true }" }' \
                   "$d/protocol/Cargo.toml" > "$d/protocol/Cargo.toml.new" && mv "$d/protocol/Cargo.toml.new" "$d/protocol/Cargo.toml"
               append "$d/protocol/Cargo.toml" "$(printf '\n[features]\nnames = ["dep:tessera-formats"]')"
               sed 's#^tessera-protocol = { path = "../protocol" }$#tessera-protocol = { path = "../protocol", features = ["names"] }#' \
                   "$d/net/Cargo.toml" > "$d/net/Cargo.toml.new" && mv "$d/net/Cargo.toml.new" "$d/net/Cargo.toml" ;;
        net) append "$s" 'tessera-net = { path = "../net", default-features = false }' ;;
        unknown) append "$s" 'tessera-newcrate = { path = "../newcrate" }' ;;
        excluded) append "$s" 'fmtcopy = { path = "../outside/formatscopy" }' ;;
        git) fixture_crate "$d/gitsrc" gitcopy
             git -C "$d/gitsrc" init -q && git -C "$d/gitsrc" add -A &&
                 git -C "$d/gitsrc" -c user.name=t -c user.email=t@example.invalid commit -q -m fixture
             append "$s" "gitcopy = { git = \"file://$d/gitsrc\" }" ;;
        host) append "$d/host/Cargo.toml" 'tessera-formats = { path = "../formats" }' ;;
        pathattr) printf '#[path = "../../formats/src/lib.rs"]\nmod copied;\n' > "$d/server/src/lib.rs" ;;
        include) printf 'pub const T: &[u8] =\n    include_bytes!(\n        "../../formats/src/lib.rs"\n    );\n' > "$d/server/src/lib.rs" ;;
        *) echo "check-server-deps: unknown self-test case $1" >&2; return 2 ;;
    esac
}

if [ "$self_test" = 1 ]; then
    echo "check-server-deps: self-test (fixture workspaces)"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/check-server-deps.XXXXXX")"
    trap 'rm -rf "$tmp"' EXIT
    failures=0
    # case | expected exit | pattern the output must contain ("" = none)
    while IFS='|' read -r case want pattern; do
        [ -n "$case" ] || continue
        if [ "$case" = git ] && ! command -v git >/dev/null 2>&1; then
            echo "  skip: git (git is not installed)"
            continue
        fi
        d="$tmp/$case"
        fixture_base "$d"
        if ! plant "$case" "$d"; then failures=$((failures + 1)); continue; fi
        rc=0
        if [ "$case" = git ]; then
            out="$(CARGO_HOME="$tmp/cargo-home" CARGO_NET_GIT_FETCH_WITH_CLI=true bash "$script" --root "$d" 2>&1)" || rc=$?
        else
            out="$(bash "$script" --root "$d" 2>&1)" || rc=$?
        fi
        if [ "$rc" = "$want" ] && { [ -z "$pattern" ] || grep -Eq -- "$pattern" <<< "$out"; }; then
            echo "  ok: $case (exit $rc)"
        else
            echo "  FAIL: $case: expected exit $want${pattern:+ and output matching '$pattern'}, got exit $rc:" >&2
            sed 's/^/  | /' <<< "$out" >&2
            failures=$((failures + 1))
        fi
    done <<'CASES'
clean|0|ok: tessera-loadhost links
normal|1|tessera-formats \[default features, all features, workspace build\]
feature|1|tessera-formats \[all features, workspace build\]
build|1|tessera-formats \[default features
dev|1|tessera-formats \[default features, all features\]: data-decoding
target|1|tessera-formats \[default features
unify|1|tessera-formats \[workspace build\]: data-decoding
net|1|tessera-net \[.*\]: holds the scenario feed
unknown|1|tessera-newcrate \[.*\]: workspace crate not on the allowlist
excluded|1|fmtcopy \[.*\]: not a workspace member
git|1|gitcopy \[.*\]: not a workspace member
host|1|FAIL: tessera-loadhost links
pathattr|1|#\[path\] "\.\./\.\./formats/src/lib\.rs" leaves
include|1|include_bytes! "\.\./\.\./formats/src/lib\.rs" leaves
CASES
    rc=0
    out="$(bash "$script" --root "$tmp/clean" -p nosuch 2>&1)" || rc=$?
    if [ "$rc" = 2 ] && grep -q "not a workspace crate" <<< "$out"; then
        echo "  ok: unknown -p package (exit 2)"
    else
        echo "  FAIL: unknown -p package: expected exit 2, got $rc: $out" >&2
        failures=$((failures + 1))
    fi
    if [ "$failures" = 0 ]; then
        echo "check-server-deps: self-test passed"
        exit 0
    fi
    echo "check-server-deps: self-test FAILED ($failures case(s))" >&2
    exit 1
fi

# ---- main -------------------------------------------------------------------------------

load_workspace || exit 2
if [ -n "$package" ]; then
    rc=0
    check "$package" "$server_allow" || rc=$?
    exit "$rc"
fi

if ! is_member "$server"; then
    echo "check-server-deps: '$server' is not a workspace crate in $ws" >&2
    exit 2
fi
status=0
rc=0
check "$server" "$server_allow" || rc=$?
[ "$rc" -gt "$status" ] && status=$rc
hosts="$(graph hosts "$server" <<< "$unified")" || {
    echo "check-server-deps: $server missing from the workspace graph" >&2; exit 2; }
if [ -n "$hosts" ]; then
    echo "check-server-deps: server hosts (run $server in-process): $(tr '\n' ' ' <<< "$hosts" | sed 's/ $//')"
fi
while IFS= read -r h; do
    [ -n "$h" ] || continue
    rc=0
    check "$h" "$host_allow" || rc=$?
    [ "$rc" -gt "$status" ] && status=$rc
done <<< "$hosts"
exit "$status"
