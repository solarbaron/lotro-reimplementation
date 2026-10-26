#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Fuzz coverage guard (P8 exit criterion 2, decision 0024: "every new parser has a fuzz
# target"; CLAUDE.md: "every parser has a fuzz target in fuzz/"). A parser is every public
# entry point that decodes bytes or text Tessera did not write itself in the same process:
# install records, network input, user and third-party files, scenarios, settings, plugins.
#
# Manifests. Every fuzz/coverage/*.toml is read (a small TOML subset, see below). Each
# holds repeated tables:
#   [[parser]]
#   crate  = "tessera-foo"              package name of a workspace crate or app
#   file   = "crates/foo/src/bar.rs"    repo-relative source file of that package
#   entry  = "Bar::parse"               public path(s) of the decoding entry point(s) in the
#                                       file; a string or an array of strings
#   input  = "install record"           install record | network | user file | scenario |
#                                       settings | plugin | other
#   target = "foo_bar"                  a [[bin]] of fuzz/Cargo.toml ...
#   exempt = "reason"                   ... or why no own target; it must name the covering
#                                       target: one of its words is a [[bin]] name ...
#   gap    = "reason"                   ... or a known parser with no fuzz target (exactly one
#                                       of the three). Every gap is listed as a failure (g)
#   [[package]]
#   crate  = "tessera-foo"              a package with no parser of its own; it is still
#   reason = "why it decodes nothing"   scanned by (c)
#   [[skip]]
#   path   = "crates/foo/src/gen/**"    a file, a directory (everything under it) or a glob
#                                       (* within a path segment, ** across segments, ?)
#   fns    = ["load_max"]               optional: only these functions of the matched files
#   reason = "why nothing here is a parser"
#
# Checks (each failure is listed with the manifest and line it comes from):
#   (m) every manifest table is well formed: known keys, required keys, one of target,
#       exempt and gap, an exempt that names a [[bin]], a known input kind, the
#       crate is a workspace package whose directory holds the file, the file exists, the
#       last segment of every entry is a `fn` in the file, every skip matches some file and
#       every name in its fns is a `fn` of a matched file; and every package under crates/
#       and apps/ is in a manifest: a [[parser]] names it as crate, a
#       [[package]] names it, or a whole-file skip covers its directory
#   (a) every target names a [[bin]] of fuzz/Cargo.toml whose source file exists
#   (b) every [[bin]] of fuzz/Cargo.toml is named by some manifest target (or its source
#       lies under a whole-file skip)
#   (c) heuristic scan, per function: every plain `pub fn` (`pub(crate)` and the like are
#       not entry points) under crates/*/src and apps/*/src whose name looks like a parser
#       is the last segment of an entry of a [[parser]] for its file, or a skip covers it
#       (a whole-file skip, or one whose fns names it). Parser-like names: parse, decode,
#       load, from_bytes, from_slice, from_str (alone or followed by `_...`), read_...,
#       ..._from_payload; and, only when the generics, parameters or where clause (not the
#       return type) take `&[u8]`, a Reader, a PropertySet, PropertyStruct or
#       PropertyValue, a Path or PathBuf, or a Read/BufRead bound: read, open, open_...,
#       discover, discover_..., walk_... and available_... (`read` also with `&str`)
#   (g) every gap, with its reason: criterion 2 is not met while any remain
# Built-in skips, not read from manifests:
#   - test-only and bench code: files under tests/, benches/ or examples/ directories,
#     files named tests.rs, test.rs, *_tests.rs, test_*.rs or benches.rs, and every item
#     that follows a #[cfg(test)] attribute (its brace block is not scanned)
#   - cargo build directories: a directory named target that is not under a src/ tree
#
# TOML subset: comments (#), [[parser]], [[package]] and [[skip]] headers,
# `key = "basic string"` (escapes \" \\ \t \n), `key = 'literal string'`, and arrays of
# strings, which may span lines and may end with a comma. Anything else is a manifest error.
#
# Usage: tools/check-fuzz-coverage.sh [--root DIR] [--list] [--self-test]
#   (no option)   check this repository
#   --root DIR    check the tree at DIR (the self-test uses it on fixture trees)
#   --list        also print every scanned function and what covers it
#   --self-test   build fixture trees in a temporary directory, plant one violation in each
#                 (unknown target, missing bin source, orphan bin, uncovered parser, a new
#                 function in a covered file, an exemption naming no target, a gap, a
#                 package in no manifest, bad keys and values, stale entry, wrong crate,
#                 stale skip, syntax error, unreadable source tree) and accepted variants
#                 (skips, fns skips, [[package]], a src/target module, a tree without apps/,
#                 benign read/open signatures), and check that each is caught or passes
# Needs bash (no bash 4 features), awk, find, sort. Offline; no build.
# Exit status: 0 covered or self-test passed; 1 a check failed (gaps included) or
# self-test failed; 2 usage error, a tree without fuzz/Cargo.toml or an unreadable source
# tree.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/$(basename "${BASH_SOURCE[0]}")"
repo="$(cd "$here/.." && pwd)"

usage_error() {
    echo "check-fuzz-coverage: $1 (see --help)" >&2
    exit 2
}

ws="$repo"
self_test=0
list=0
while [ $# -gt 0 ]; do
    case "$1" in
        --root)
            [ $# -ge 2 ] || usage_error "--root needs a directory"
            [ -d "$2" ] || usage_error "--root: '$2' is not a directory"
            ws="$(cd "$2" && pwd)"; shift 2 ;;
        --list) list=1; shift ;;
        --self-test) self_test=1; shift ;;
        -h|--help) sed -n '4,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) usage_error "unknown argument '$1'" ;;
    esac
done

# ---- self-test --------------------------------------------------------------------------

# fixture DIR: a clean tree (two packages, test-only code, one manifest that covers
# everything). The compat cases add a tessera-compat crate and a compat_* bin, which are
# checked like every other package and bin.
fixture() {
    local d="$1"
    mkdir -p "$d/crates/alpha/src/nested" "$d/crates/alpha/tests" \
        "$d/apps/tool/src" "$d/fuzz/fuzz_targets" "$d/fuzz/coverage"
    printf '[package]\nname = "tessera-alpha"\nversion = "0.0.0"\n' > "$d/crates/alpha/Cargo.toml"
    printf '[workspace]\n\n[package]\nname = "tessera-tool"\n' > "$d/apps/tool/Cargo.toml"
    cat > "$d/crates/alpha/src/lib.rs" <<'EOF'
pub mod nested;
pub struct Rec;
impl Rec {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let _ = "pub fn decode(x) { in a string";
        data.first().map(|_| Rec)
    }
    pub fn parser_state(&self) {}
    pub(crate) fn decode_inner(&self) {}
    fn load_private(&self) {}
}
#[cfg(test)]
mod tests {
    pub fn parse_fixture() { let _ = '{'; }
    #[test]
    fn t() { let _ = r#"{ unbalanced"#; }
}
EOF
    cat > "$d/crates/alpha/src/nested/view.rs" <<'EOF'
pub fn shape() -> u32 { 1 }
pub const fn decoded_len() -> usize { 0 }
EOF
    printf 'pub fn parse_case() {}\n' > "$d/crates/alpha/src/tests.rs"
    printf 'pub fn decode_test() {}\n' > "$d/crates/alpha/tests/it.rs"
    printf 'pub fn read_config(p: &str) {}\npub fn from_str_lossy(s: &str) {}\n' > "$d/apps/tool/src/main.rs"
    cat > "$d/fuzz/Cargo.toml" <<'EOF'
[package]
name = "fixture-fuzz"

[[bin]]
name = "alpha_rec"
path = "fuzz_targets/alpha_rec.rs"
test = false
EOF
    : > "$d/fuzz/fuzz_targets/alpha_rec.rs"
    cat > "$d/fuzz/coverage/fixture.toml" <<'EOF'
# A comment.
[[parser]]
crate = "tessera-alpha"
file = "crates/alpha/src/lib.rs"
entry = "tessera_alpha::Rec::parse"   # trailing comment
input = "install record"
target = "alpha_rec"

[[parser]]
crate = "tessera-tool"
file = "apps/tool/src/main.rs"
entry = [
    "read_config",
    'from_str_lossy',
]
input = "user file"
exempt = "reads the file and hands it to Rec::parse (# alpha_rec)"
EOF
}

# plant CASE DIR: one violation (or one accepted variant) per case.
plant() {
    local c="$1" d="$2" m="$2/fuzz/coverage/fixture.toml"
    case "$c" in
        clean) ;;
        nobin) sed 's/^target = "alpha_rec"/target = "alpha_gone"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        nosource) rm "$d/fuzz/fuzz_targets/alpha_rec.rs" ;;
        orphan) printf '\n[[bin]]\nname = "alpha_extra"\npath = "fuzz_targets/alpha_extra.rs"\n' >> "$d/fuzz/Cargo.toml"
                : > "$d/fuzz/fuzz_targets/alpha_extra.rs" ;;
        uncovered) printf 'pub fn load_table(x: u8) {}\n' >> "$d/crates/alpha/src/nested/view.rs" ;;
        skipped) printf 'pub fn load_table(x: u8) {}\n' >> "$d/crates/alpha/src/nested/view.rs"
                 printf '\n[[skip]]\npath = "crates/alpha/src/nested"\nreason = "views only"\n' >> "$m" ;;
        globskip) printf 'pub fn read_rows<T>(x: T) {}\n' >> "$d/crates/alpha/src/nested/view.rs"
                  printf '\n[[skip]]\npath = "crates/*/src/**/view.rs"\nreason = "views only"\n' >> "$m" ;;
        both) printf 'target = "alpha_rec"\n' >> "$m" ;;
        neither) grep -v '^exempt' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        input) sed 's/^input = "user file"/input = "a file"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        unknownkey) printf 'targte = "alpha_rec"\n' >> "$m" ;;
        stale) sed 's/Rec::parse"/Rec::parse_all"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        nofile) sed 's|crates/alpha/src/lib.rs|crates/alpha/src/gone.rs|' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        wrongcrate) sed 's/^crate = "tessera-alpha"/crate = "tessera-tool"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        nocrate) sed 's/^crate = "tessera-alpha"/crate = "tessera-beta"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        staleskip) printf '\n[[skip]]\npath = "crates/alpha/src/old/**"\nreason = "gone"\n' >> "$m" ;;
        syntax) printf '\n[parser]\nfile = 3\n' >> "$m" ;;
        unterminated) printf '\n[[skip]]\npath = "crates/alpha\n' >> "$m" ;;
        cfgfn) cat >> "$d/crates/alpha/src/nested/view.rs" <<'EOF'
#[cfg(test)]
pub fn decode_fixture() {
    let _ = "}";
}
EOF
               ;;
        nomanifest) rm "$m" ;;
        compatsrc|compatbin|compatcovered)
            mkdir -p "$d/crates/compat/src"
            printf '[package]\nname = "tessera-compat"\n' > "$d/crates/compat/Cargo.toml"
            printf 'pub fn decode_wire(b: &[u8]) {}\n' > "$d/crates/compat/src/lib.rs"
            case "$c" in
                compatbin|compatcovered)
                    printf '\n[[bin]]\nname = "compat_wire"\npath = "fuzz_targets/compat_wire.rs"\n' >> "$d/fuzz/Cargo.toml"
                    : > "$d/fuzz/fuzz_targets/compat_wire.rs" ;;
            esac
            case "$c" in
                compatbin) printf '\n[[package]]\ncrate = "tessera-compat"\nreason = "x"\n' >> "$m"
                           : > "$d/crates/compat/src/lib.rs" ;;
                compatcovered)
                    printf '\n[[parser]]\ncrate = "tessera-compat"\nfile = "crates/compat/src/lib.rs"\nentry = "tessera_compat::decode_wire"\ninput = "network"\ntarget = "compat_wire"\n' >> "$m"
                    sed 's/^exempt = .*/exempt = "covered by compat_wire"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
            esac ;;
        exemptx) sed 's/^exempt = .*/exempt = "x"/' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        gap) sed 's/^exempt = /gap = /' "$m" > "$m.new" && mv "$m.new" "$m" ;;
        newfn) printf 'pub fn decode_hostile(b: &[u8]) {}\n' >> "$d/crates/alpha/src/lib.rs" ;;
        skipfns) printf 'pub fn decode_hostile(b: &[u8]) {}\n' >> "$d/crates/alpha/src/lib.rs"
                 printf '\n[[skip]]\npath = "crates/alpha/src/lib.rs"\nfns = ["decode_hostile"]\nreason = "not a decoder"\n' >> "$m" ;;
        staleskipfns) printf '\n[[skip]]\npath = "crates/alpha/src/lib.rs"\nfns = ["nothere"]\nreason = "gone"\n' >> "$m" ;;
        nopackage|packagetable|packageskip)
            mkdir -p "$d/crates/beta/src"
            printf '[package]\nname = "tessera-beta"\n' > "$d/crates/beta/Cargo.toml"
            printf 'pub fn unpack(b: &[u8]) {}\n' > "$d/crates/beta/src/lib.rs"
            case "$c" in
                packagetable) printf '\n[[package]]\ncrate = "tessera-beta"\nreason = "decodes nothing"\n' >> "$m" ;;
                packageskip) printf '\n[[skip]]\npath = "crates/beta"\nreason = "generated"\n' >> "$m" ;;
            esac ;;
        packagebad) printf '\n[[package]]\ncrate = "tessera-alpha"\nreason = "decodes nothing"\n' >> "$m" ;;
        targetdir) mkdir -p "$d/crates/alpha/src/target"
                   printf 'pub fn parse_target(b: &[u8]) {}\n' > "$d/crates/alpha/src/target/mod.rs"
                   printf '\n[[parser]]\ncrate = "tessera-alpha"\nfile = "crates/alpha/src/target/mod.rs"\nentry = "parse_target"\ninput = "other"\ntarget = "alpha_rec"\n' >> "$m" ;;
        noapps) rm -r "$d/apps"; head -n 7 "$m" > "$m.new" && mv "$m.new" "$m" ;;
        unreadable) mkdir -p "$d/crates/alpha/src/locked"; chmod 000 "$d/crates/alpha/src/locked" ;;
        readsig) cat >> "$d/crates/alpha/src/nested/view.rs" <<'EOF'
pub fn open(
    path: &std::path::Path,
) -> u8 { 0 }
pub fn read(r: &mut Reader<'_>) -> u8 { 0 }
EOF
                 ;;
        readbenign) cat >> "$d/crates/alpha/src/nested/view.rs" <<'EOF'
pub struct V;
impl V {
    pub fn read(&self) -> &str { "" }
    pub fn open(&mut self, name: Option<&str>) {}
    pub fn open_tab(
        &mut self,
        label: &str,
    ) -> Option<&[u8]> { None }
}
EOF
                    ;;
        frompayload) printf 'pub fn effect_from_payload(p: &[u8]) {}\n' >> "$d/crates/alpha/src/nested/view.rs" ;;
        *) echo "check-fuzz-coverage: unknown self-test case $c" >&2; return 2 ;;
    esac
}

if [ "$self_test" = 1 ]; then
    echo "check-fuzz-coverage: self-test (fixture trees)"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/check-fuzz-coverage.XXXXXX")"
    trap 'chmod -R u+rwx "$tmp" 2>/dev/null; rm -rf "$tmp"' EXIT
    failures=0
    # case | expected exit | pattern the output must contain ("" = none)
    while IFS='|' read -r case want pattern; do
        [ -n "$case" ] || continue
        if [ "$case" = unreadable ] && [ "$(id -u)" = 0 ]; then
            echo "  skip: $case (root reads every directory)"; continue
        fi
        d="$tmp/$case"
        fixture "$d"
        if ! plant "$case" "$d"; then failures=$((failures + 1)); continue; fi
        rc=0
        out="$(bash "$script" --root "$d" --list 2>&1)" || rc=$?
        if [ "$rc" = "$want" ] && { [ -z "$pattern" ] || grep -Eq -- "$pattern" <<< "$out"; }; then
            echo "  ok: $case (exit $rc)"
        else
            echo "  FAIL: $case: expected exit $want${pattern:+ and output matching '$pattern'}, got exit $rc:" >&2
            sed 's/^/  | /' <<< "$out" >&2
            failures=$((failures + 1))
        fi
    done <<'CASES'
clean|0|covered: crates/alpha/src/lib.rs parse \(line 4\) by fixture.toml:2 target alpha_rec
nobin|1|\(a\) .*fixture.toml:2: target 'alpha_gone' is not a \[\[bin\]\]
nosource|1|\(a\) .*target 'alpha_rec': source fuzz/fuzz_targets/alpha_rec.rs of the \[\[bin\]\] does not exist
orphan|1|\(b\) alpha_extra \(fuzz/fuzz_targets/alpha_extra.rs\)
uncovered|1|\(c\) crates/alpha/src/nested/view.rs: load_table \(line 3\)
skipped|0|covered: crates/alpha/src/nested/view.rs load_table \(line 3\) by skip
globskip|0|covered: crates/alpha/src/nested/view.rs read_rows \(line 3\) by skip
both|1|\(m\) .*fixture.toml:9: more than one of target, exempt and gap
neither|1|\(m\) .*fixture.toml:9: none of target, exempt and gap
input|1|\(m\) .*fixture.toml:9: input 'a file' is not one of
unknownkey|1|\(m\) .*fixture.toml:[0-9]+: unknown key 'targte'
stale|1|\(m\) .*fixture.toml:2: entry 'tessera_alpha::Rec::parse_all': no 'fn parse_all' in crates/alpha/src/lib.rs
nofile|1|\(m\) .*fixture.toml:2: file crates/alpha/src/gone.rs does not exist
wrongcrate|1|\(m\) .*fixture.toml:2: file crates/alpha/src/lib.rs is not in package tessera-tool \(apps/tool\)
nocrate|1|\(m\) .*fixture.toml:2: crate 'tessera-beta' is not a package under crates/ or apps/
staleskip|1|\(m\) .*fixture.toml:[0-9]+: skip 'crates/alpha/src/old/\*\*' matches no file
syntax|1|\(m\) .*fixture.toml:[0-9]+: table \[parser\]
unterminated|1|\(m\) .*fixture.toml:[0-9]+: unterminated string
cfgfn|0|
nomanifest|1|no manifests in fuzz/coverage
compatsrc|1|\(c\) crates/compat/src/lib.rs: decode_wire \(line 1\)
compatbin|1|\(b\) compat_wire \(fuzz/fuzz_targets/compat_wire.rs\)
compatcovered|0|covered: crates/compat/src/lib.rs decode_wire \(line 1\) by fixture.toml:[0-9]+ target compat_wire
exemptx|1|\(m\) .*fixture.toml:9: exempt names no covering fuzz target
gap|1|\(g\) .*fixture.toml:9: apps/tool/src/main.rs read_config
newfn|1|\(c\) crates/alpha/src/lib.rs: decode_hostile \(line 18\) \(the file has a \[\[parser\]\], fixture.toml:2
skipfns|0|covered: crates/alpha/src/lib.rs decode_hostile \(line 18\) by skip crates/alpha/src/lib.rs
staleskipfns|1|\(m\) .*fixture.toml:[0-9]+: skip fns 'nothere': no 'fn nothere'
nopackage|1|\(m\) package tessera-beta \(crates/beta\) is in no manifest
packagetable|0|
packageskip|0|
packagebad|1|\(m\) .*fixture.toml:[0-9]+: \[\[package\]\] tessera-alpha has \[\[parser\]\] tables too
targetdir|0|covered: crates/alpha/src/target/mod.rs parse_target \(line 1\) by fixture.toml:[0-9]+ target alpha_rec
noapps|0|covered: crates/alpha/src/lib.rs parse \(line 4\)
unreadable|2|cannot read the source tree
readsig|1|\(c\) crates/alpha/src/nested/view.rs: open \(line 3\), read \(line 6\)
readbenign|0|
frompayload|1|\(c\) crates/alpha/src/nested/view.rs: effect_from_payload \(line 3\)
CASES
    rc=0
    out="$(bash "$script" --root "$tmp/clean" --bogus 2>&1)" || rc=$?
    if [ "$rc" = 2 ]; then
        echo "  ok: unknown option (exit 2)"
    else
        echo "  FAIL: unknown option: expected exit 2, got $rc: $out" >&2
        failures=$((failures + 1))
    fi
    mkdir -p "$tmp/empty"
    rc=0
    out="$(bash "$script" --root "$tmp/empty" 2>&1)" || rc=$?
    if [ "$rc" = 2 ] && grep -q "no fuzz/Cargo.toml" <<< "$out"; then
        echo "  ok: tree without fuzz/Cargo.toml (exit 2)"
    else
        echo "  FAIL: tree without fuzz/Cargo.toml: expected exit 2, got $rc: $out" >&2
        failures=$((failures + 1))
    fi
    if [ "$failures" = 0 ]; then
        echo "check-fuzz-coverage: self-test passed"
        exit 0
    fi
    echo "check-fuzz-coverage: self-test FAILED ($failures case(s))" >&2
    exit 1
fi

# ---- main -------------------------------------------------------------------------------

cd "$ws"
[ -f fuzz/Cargo.toml ] || { echo "check-fuzz-coverage: no fuzz/Cargo.toml in $ws" >&2; exit 2; }

tmp="$(mktemp -d "${TMPDIR:-/tmp}/check-fuzz-coverage.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

# Packages: name<TAB>directory, for crates/* and apps/*.
for t in crates/*/Cargo.toml apps/*/Cargo.toml; do
    [ -f "$t" ] || continue
    awk -v dir="${t%/Cargo.toml}" '
        /^[ \t]*\[package\][ \t]*$/ { p = 1; next }
        /^[ \t]*\[/ { p = 0 }
        p && /^[ \t]*name[ \t]*=/ {
            v = $0; sub(/^[^=]*=[ \t]*/, "", v); gsub(/["\047 \t]/, "", v); sub(/#.*/, "", v)
            print v "\t" dir; exit
        }' "$t"
done > "$tmp/packages"

# Fuzz bins: name<TAB>repo-relative source path ("-" when the table has no path).
awk '
    function flush() { if (inbin) print (name == "" ? "-" : name) "\t" (path == "" ? "-" : "fuzz/" path) }
    /^[ \t]*\[\[bin\]\][ \t]*(#.*)?$/ { flush(); inbin = 1; name = ""; path = ""; next }
    /^[ \t]*\[/ { flush(); inbin = 0; next }
    inbin && /^[ \t]*name[ \t]*=/ { v = $0; sub(/^[^=]*=[ \t]*"/, "", v); sub(/".*/, "", v); name = v }
    inbin && /^[ \t]*path[ \t]*=/ { v = $0; sub(/^[^=]*=[ \t]*"/, "", v); sub(/".*/, "", v); path = v }
    END { flush() }' fuzz/Cargo.toml > "$tmp/bins"

# Every file of the tree that a manifest may name (for existence and skip matching).
# Prunes cargo build directories (a `target` directory that is not under a src/ tree; a
# module directory named `target` is source), dot entries, and the fuzz corpora and crashes.
find crates apps fuzz tools \( \( -type d -name target ! -path '*/src/*' \) -o -name '.?*' \
    -o -path fuzz/corpus -o -path fuzz/artifacts \) -prune -o -type f -print 2>/dev/null |
    LC_ALL=C sort > "$tmp/files" || true

# Manifests.
manifests=""
for m in fuzz/coverage/*.toml; do
    [ -f "$m" ] && manifests="$manifests $m"
done

# Manifest parser (TOML subset). Output, tab-separated:
#   T kind manifest line key=value...   one table (array values joined by \037)
#   E manifest line message             a syntax error
parse_manifests() {
    [ -n "$manifests" ] || return 0
    # shellcheck disable=SC2086
    awk '
        function err(msg) { print "E\t" FILENAME "\t" FNR "\t" msg }
        function flush() {
            if (kind != "") {
                o = "T\t" kind "\t" tfile "\t" tline
                for (i = 1; i <= nk; i++) o = o "\t" keys[i] "=" vals[i]
                print o
            }
            kind = ""; nk = 0; split("", seen)
        }
        function ws() { while (pos <= length(s) && substr(s, pos, 1) ~ /[ \t]/) pos++ }
        # Reads one string at pos into sval; returns 1, or 0 with errmsg set.
        function str(    q, c, out, n) {
            q = substr(s, pos, 1); out = ""; pos++
            n = length(s)
            while (pos <= n) {
                c = substr(s, pos, 1)
                if (c == q) { pos++; sval = out; return 1 }
                if (c == "\t") { errmsg = "tab in a string"; return 0 }
                if (q == "\"" && c == "\\") {
                    pos++; c = substr(s, pos, 1)
                    if (c == "\"" || c == "\\") out = out c
                    else if (c == "t" || c == "n") out = out " "
                    else { errmsg = "unsupported escape \\" c; return 0 }
                    pos++; continue
                }
                out = out c; pos++
            }
            errmsg = "unterminated string"; return 0
        }
        # The rest of the line must be blank or a comment.
        function rest_ok() { ws(); return pos > length(s) || substr(s, pos, 1) == "#" }
        # Parses array elements from pos; sets inarr = 0 at "]". Returns 0 on error.
        function arr_items(    c) {
            while (1) {
                ws()
                if (pos > length(s) || substr(s, pos, 1) == "#") return 1
                c = substr(s, pos, 1)
                if (c == "]") { pos++; inarr = 0; return 1 }
                if (c == ",") { if (!expect_comma) { errmsg = "unexpected comma in array"; return 0 }
                                expect_comma = 0; pos++; continue }
                if (c == "\"" || c == "\047") {
                    if (expect_comma) { errmsg = "missing comma in array"; return 0 }
                    if (!str()) return 0
                    if (sval ~ /\037/) { errmsg = "control character in a string"; return 0 }
                    acc = acc (acc == "" && !nacc ? "" : "\037") sval; nacc++
                    expect_comma = 1; continue
                }
                errmsg = "array elements must be strings"; return 0
            }
        }
        FNR == 1 { flush(); if (inarr) { print "E\t" pfile "\t" aline "\tunterminated array"; inarr = 0 } pfile = FILENAME }
        {
            s = $0; sub(/\r$/, "", s); pos = 1
            if (inarr) {
                if (!arr_items()) { err(errmsg); inarr = 0; next }
                if (!inarr) {
                    if (!rest_ok()) { err("text after the array"); next }
                    keys[++nk] = akey; vals[nk] = acc
                }
                next
            }
            ws()
            if (pos > length(s) || substr(s, pos, 1) == "#") next
            t = substr(s, pos)
            if (t ~ /^\[\[[ \t]*[A-Za-z0-9_-]+[ \t]*\]\][ \t]*(#.*)?$/) {
                flush()
                k = t; sub(/^\[\[[ \t]*/, "", k); sub(/[ \t]*\]\].*$/, "", k)
                if (k != "parser" && k != "skip" && k != "package") { err("unknown table [[" k "]] (only [[parser]], [[package]] and [[skip]])"); next }
                kind = k; tfile = FILENAME; tline = FNR
                next
            }
            if (t ~ /^\[/) { flush(); err("table " t ": only [[parser]], [[package]] and [[skip]] are allowed"); kind = ""; next }
            if (t !~ /^[A-Za-z_][A-Za-z0-9_-]*[ \t]*=/) { err("expected key = value"); next }
            key = t; sub(/[ \t]*=.*/, "", key)
            pos += length(key); ws(); pos++; ws()
            if (kind == "") { err("key " key " outside a [[parser]], [[package]] or [[skip]] table"); next }
            if (key in seen) { err("duplicate key " key); next }
            seen[key] = 1
            c = substr(s, pos, 1)
            if (c == "\"" || c == "\047") {
                if (!str()) { err(errmsg); next }
                if (!rest_ok()) { err("text after the value of " key); next }
                keys[++nk] = key; vals[nk] = sval
                next
            }
            if (c == "[") {
                pos++; inarr = 1; akey = key; acc = ""; nacc = 0; expect_comma = 0; aline = FNR
                if (!arr_items()) { err(errmsg); inarr = 0; next }
                if (!inarr) {
                    if (!rest_ok()) { err("text after the array"); next }
                    keys[++nk] = akey; vals[nk] = acc
                }
                next
            }
            err("value of " key " must be a string or an array of strings")
        }
        END {
            if (inarr) print "E\t" pfile "\t" aline "\tunterminated array"
            flush()
        }' $manifests
}
parse_manifests > "$tmp/records"

# Source scan (c): path<TAB>line<TAB>function, for every parser-like `pub fn`. Only the
# source roots that exist are scanned; a find error (an unreadable directory) is a tree
# error (exit 2) with a message, never a silent exit.
scan_dirs=()
for d in crates apps; do [ -d "$d" ] && scan_dirs+=("$d"); done
: > "$tmp/sources"
if [ ${#scan_dirs[@]} -gt 0 ]; then
    if ! find "${scan_dirs[@]}" -path '*/src/*' -type f -name '*.rs' > "$tmp/sources.raw" 2> "$tmp/find.err"; then
        echo "check-fuzz-coverage: cannot read the source tree under $ws:" >&2
        sed 's/^/  /' "$tmp/find.err" >&2
        exit 2
    fi
    LC_ALL=C sort "$tmp/sources.raw" > "$tmp/sources"
fi
scan_sources() {
    local f
    while IFS= read -r f; do
        case "$f" in
            */tests/*|*/benches/*|*/examples/*) continue ;;
            */tests.rs|*/test.rs|*_tests.rs|*/test_*.rs|*/benches.rs) continue ;;
        esac
        case "$f" in crates/*/src/*|apps/*/src/*) ;; *) continue ;; esac
        awk -v file="$f" '
            # Code of a line without comments, strings and char literals (for braces).
            function code_of(l,    out) {
                out = l
                gsub(/\\\\/, "", out)
                gsub(/\\"/, "", out)
                gsub(/r#*"[^"]*"#*/, "\"\"", out)
                gsub(/"[^"]*"/, "\"\"", out)
                gsub(/\047[^\047\\]\047/, "\047 \047", out)
                gsub(/\047\\.[^\047]*\047/, "\047 \047", out)
                sub(/\/\/.*/, "", out)
                return out
            }
            function braces(l,    o, c) {
                o = gsub(/\{/, "{", l); c = gsub(/\}/, "}", l)
                opened += o; depth += o - c
            }
            {
                line = $0
                if (inraw) { if (line ~ /"#*/ && index(line, rawend) > 0) inraw = 0; next }
                code = code_of(line)
                if (code ~ /r#*"/ ) {
                    # a raw string that does not end on this line
                    t = code; match(t, /r#*"/); h = substr(t, RSTART + 1, RLENGTH - 2)
                    rawend = "\"" h; inraw = 1
                    code = substr(code, 1, RSTART - 1)
                }
                if (skipping) {
                    braces(code)
                    if (opened > 0 && depth <= 0) skipping = 0
                    else if (opened == 0 && code ~ /;[ \t]*$/) skipping = 0
                    next
                }
                if (code ~ /^[ \t]*#\[cfg\(test\)\][ \t]*$/) { pending = 1; next }
                if (pending) {
                    if (code ~ /^[ \t]*(#\[.*\])?[ \t]*$/) next
                    pending = 0; depth = 0; opened = 0
                    braces(code)
                    if (opened > 0 && depth <= 0) next
                    if (opened == 0 && code ~ /;[ \t]*$/) next
                    skipping = 1; next
                }
                if (code ~ /^[ \t]*#\[cfg\(test\)\][ \t]*[^ \t]/) {
                    # attribute and item on one line
                    sub(/^[ \t]*#\[cfg\(test\)\]/, "", code)
                    depth = 0; opened = 0; braces(code)
                    if (opened > 0 && depth > 0) skipping = 1
                    next
                }
                if (sigpend) {
                    # the rest of a signature that started on an earlier line
                    sig = sig " " code; sigl++
                    if (sig ~ /[{;]/ || sigl > 40) {
                        if (hostile(sig, signame)) print file "\t" sigline "\t" signame
                        sigpend = 0
                    }
                    next
                }
                if (match(code, /^[ \t]*pub[ \t]+((const|async|unsafe|extern[ \t]*"[^"]*")[ \t]+)*fn[ \t]+[A-Za-z0-9_]+/)) {
                    n = substr(code, RSTART, RLENGTH); sub(/.*fn[ \t]+/, "", n)
                    if (n ~ /^read_/ || n ~ /^(parse|decode|load|from_bytes|from_slice|from_str)(_[A-Za-z0-9_]*)?$/ ||
                        n ~ /(^|_)from_payload$/)
                        print file "\t" FNR "\t" n
                    else if (n ~ /^(read|open|discover)$/ || n ~ /^(open|walk|discover|available)_[A-Za-z0-9_]+$/) {
                        # Common names that are parsers only when they take hostile input.
                        sig = substr(code, RSTART); signame = n; sigline = FNR; sigl = 0
                        if (sig ~ /[{;]/) { if (hostile(sig, n)) print file "\t" FNR "\t" n }
                        else sigpend = 1
                    }
                }
            }
            # Whether a signature (generics, parameters and where clause; the return type
            # does not count) takes bytes, a reader, a property set, struct or value, a path
            # or (for `read` only) text.
            function hostile(sig, name,    i, c, d, n, params, tail) {
                n = length(sig); d = 0; i = index(sig, "(")
                if (i == 0) return 0
                params = substr(sig, 1, i)
                for (; i <= n; i++) {
                    c = substr(sig, i, 1)
                    if (c == "(") d++
                    else if (c == ")") { d--; if (d == 0) break }
                }
                params = params substr(sig, index(sig, "(") + 1, i - index(sig, "(") - 1)
                tail = substr(sig, i + 1)
                if (match(tail, /(^|[^A-Za-z0-9_])where([^A-Za-z0-9_]|$)/)) params = params " " substr(tail, RSTART)
                if (params ~ /&(\047[A-Za-z_]+[ \t]+)?(mut[ \t]+)?\[u8\]/) return 1
                if (name == "read" && params ~ /&(\047[A-Za-z_]+[ \t]+)?(mut[ \t]+)?str([^A-Za-z0-9_]|$)/) return 1
                if (params ~ /(^|[^A-Za-z0-9_])(Reader|PropertySet|PropertyStruct|PropertyValue|Path|PathBuf)([^A-Za-z0-9_]|$)/) return 1
                if (params ~ /(impl|dyn|:)[ \t]*(std::io::|io::)?(Buf)?Read([^A-Za-z0-9_]|$)/) return 1
                return 0
            }' "$f"
    done < "$tmp/sources"
}
scan_sources > "$tmp/scan"

# All checks.
rc=0
awk -v list="$list" -v manifests="$manifests" '
    BEGIN {
        FS = "\t"
        split("install record|network|user file|scenario|settings|plugin|other", ik, "|")
        for (i in ik) kinds[ik[i]] = 1
        pkeys["crate"] = pkeys["file"] = pkeys["entry"] = pkeys["input"] = pkeys["target"] = pkeys["exempt"] = pkeys["gap"] = 1
        skeys["path"] = skeys["reason"] = skeys["fns"] = 1
        kkeys["crate"] = kkeys["reason"] = 1
    }
    function short(m) { sub(/^.*\//, "", m); return m }
    function problem(cat, msg) { probs[cat] = probs[cat] "  (" cat ") " msg "\n"; nprob++ }
    # glob -> anchored regex; a pattern without glob characters also matches below itself
    function glob_re(p,    r, i, c, n) {
        if (p !~ /[*?]/) { sub(/\/+$/, "", p); return "^" esc(p) "(/.*)?$" }
        r = "^"; n = length(p)
        for (i = 1; i <= n; i++) {
            c = substr(p, i, 1)
            if (c == "*" && substr(p, i + 1, 1) == "*") {
                if (substr(p, i + 2, 1) == "/") { r = r "(.*/)?"; i += 2 } else { r = r ".*"; i++ }
            } else if (c == "*") r = r "[^/]*"
            else if (c == "?") r = r "[^/]"
            else r = r esc(c)
        }
        return r "$"
    }
    function esc(s,    o, i, c) {
        o = ""
        for (i = 1; i <= length(s); i++) {
            c = substr(s, i, 1)
            if (c ~ /[][\\.^$+(){}|]/) o = o "\\" c; else o = o c
        }
        return o
    }
    # The first whole-file skip (one without fns) that covers path, or 0.
    function skipped(path,    i) {
        for (i = 1; i <= nskip; i++) if (skfns[i] == "" && path ~ skre[i]) return i
        return 0
    }
    # The first skip that covers function fn of path (a whole-file skip, or one whose fns
    # list names fn), or 0.
    function skipped_fn(path, fn,    i) {
        for (i = 1; i <= nskip; i++)
            if (path ~ skre[i] && (skfns[i] == "" || (i, fn) in skfn)) return i
        return 0
    }
    # Whether an exemption reason names a fuzz [[bin]] as one of its words.
    function names_bin(r,    n, ws, j) {
        n = split(r, ws, /[^A-Za-z0-9_]+/)
        for (j = 1; j <= n; j++) if (ws[j] != "" && (ws[j] in bin)) return 1
        return 0
    }
    # Names of the fns defined in a file (cached).
    function fns_of(path,    l, n) {
        if (path in fnloaded) return
        fnloaded[path] = 1
        while ((getline l < path) > 0) {
            while (match(l, /fn[ \t]+[A-Za-z0-9_]+/)) {
                n = substr(l, RSTART, RLENGTH); sub(/^fn[ \t]+/, "", n)
                if (RSTART == 1 || substr(l, RSTART - 1, 1) ~ /[^A-Za-z0-9_]/) fndef[path, n] = 1
                l = substr(l, RSTART + RLENGTH)
            }
        }
        close(path)
    }
    FILENAME == ARGV[1] { pkgdir[$1] = $2; next }
    FILENAME == ARGV[2] { nbin++; binname[nbin] = $1; binpath[nbin] = $2; bin[$1] = $2; next }
    FILENAME == ARGV[3] { exists[$0] = 1; allfiles[++nfiles] = $0; next }
    FILENAME == ARGV[4] {
        if ($1 == "E") { problem("m", short($2) ":" $3 ": " $4); next }
        nt++; tkind[nt] = $1 == "T" ? $2 : ""; twhere[nt] = short($3) ":" $4
        for (i = 5; i <= NF; i++) {
            k = $i; sub(/=.*/, "", k); v = substr($i, length(k) + 2)
            tval[nt, k] = v; thas[nt, k] = 1; tkeys[nt] = tkeys[nt] " " k
        }
        next
    }
    FILENAME == ARGV[5] { nscan++; sfile[nscan] = $1; sline[nscan] = $2; sfn[nscan] = $3; next }
    END {
        if (manifests == "") problem("m", "no manifests in fuzz/coverage (fuzz/coverage/*.toml)")
        # skips first (parsers may lie under them too)
        for (t = 1; t <= nt; t++) {
            if (tkind[t] != "skip") continue
            nk = split(tkeys[t], ks, " ")
            for (j = 1; j <= nk; j++) if (!(ks[j] in skeys)) problem("m", twhere[t] ": unknown key \047" ks[j] "\047 in [[skip]] (path, reason)")
            if (!thas[t, "path"] || tval[t, "path"] == "") { problem("m", twhere[t] ": [[skip]] without path"); continue }
            if (!thas[t, "reason"] || tval[t, "reason"] ~ /^[ \t]*$/) problem("m", twhere[t] ": [[skip]] " tval[t, "path"] " without a reason")
            if (tval[t, "path"] ~ /\037/) { problem("m", twhere[t] ": skip path must be one string"); continue }
            nskip++; skre[nskip] = glob_re(tval[t, "path"]); skpath[nskip] = tval[t, "path"]; skwhere[nskip] = twhere[t]
            skfns[nskip] = ""
            if (thas[t, "fns"]) {
                nf = split(tval[t, "fns"], fs, "\037")
                if (nf == 0) problem("m", twhere[t] ": [[skip]] " tval[t, "path"] " with an empty fns list")
                skfns[nskip] = tval[t, "fns"]
                for (j = 1; j <= nf; j++) {
                    if (fs[j] !~ /^[A-Za-z_][A-Za-z0-9_]*$/) problem("m", twhere[t] ": fns element \047" fs[j] "\047 is not a function name")
                    skfn[nskip, fs[j]] = 1
                }
            }
        }
        for (i = 1; i <= nskip; i++) {
            hit = 0
            for (f = 1; f <= nfiles; f++) {
                if (allfiles[f] !~ skre[i]) continue
                hit = 1
                if (skfns[i] == "") break
                fns_of(allfiles[f])
                nf = split(skfns[i], fs, "\037")
                for (j = 1; j <= nf; j++) if ((allfiles[f], fs[j]) in fndef) skfnseen[i, fs[j]] = 1
            }
            if (!hit) { problem("m", skwhere[i] ": skip \047" skpath[i] "\047 matches no file"); continue }
            if (skfns[i] != "") {
                nf = split(skfns[i], fs, "\037")
                for (j = 1; j <= nf; j++)
                    if (!((i, fs[j]) in skfnseen)) problem("m", skwhere[i] ": skip fns \047" fs[j] "\047: no \047fn " fs[j] "\047 in the files of " skpath[i])
            }
        }
        # [[package]] tables: a package with no parser of its own (still scanned by (c))
        for (t = 1; t <= nt; t++) {
            if (tkind[t] != "package") continue
            nk = split(tkeys[t], ks, " ")
            for (j = 1; j <= nk; j++) if (!(ks[j] in kkeys)) problem("m", twhere[t] ": unknown key \047" ks[j] "\047 in [[package]] (crate, reason)")
            if (!thas[t, "crate"]) { problem("m", twhere[t] ": [[package]] without crate"); continue }
            c = tval[t, "crate"]
            if (!thas[t, "reason"] || tval[t, "reason"] ~ /^[ \t]*$/) problem("m", twhere[t] ": [[package]] " c " without a reason")
            if (!(c in pkgdir)) { problem("m", twhere[t] ": crate \047" c "\047 is not a package under crates/ or apps/"); continue }
            pkgtable[c] = twhere[t]
        }
        for (t = 1; t <= nt; t++) {
            if (tkind[t] != "parser") continue
            w = twhere[t]
            nk = split(tkeys[t], ks, " ")
            for (j = 1; j <= nk; j++) if (!(ks[j] in pkeys)) problem("m", w ": unknown key \047" ks[j] "\047 in [[parser]] (crate, file, entry, input, target, exempt)")
            split("crate file entry input", req, " ")
            for (j = 1; j <= 4; j++) if (!thas[t, req[j]]) problem("m", w ": missing key " req[j])
            for (j = 1; j <= 4; j++) if (req[j] != "entry" && tval[t, req[j]] ~ /\037/) problem("m", w ": " req[j] " must be one string")
            ht = thas[t, "target"]; he = thas[t, "exempt"]; hg = thas[t, "gap"]
            if (ht + he + hg > 1) problem("m", w ": more than one of target, exempt and gap (exactly one)")
            if (ht + he + hg == 0) problem("m", w ": none of target, exempt and gap (exactly one)")
            if (he && tval[t, "exempt"] ~ /^[ \t]*$/) problem("m", w ": empty exempt reason")
            else if (he && !names_bin(tval[t, "exempt"]))
                problem("m", w ": exempt names no covering fuzz target (no word of it is a [[bin]] of fuzz/Cargo.toml; record a known gap with gap = \"...\")")
            if (hg && tval[t, "gap"] ~ /^[ \t]*$/) problem("m", w ": empty gap reason")
            else if (hg) { ge = tval[t, "entry"]; gsub(/\037/, ", ", ge); problem("g", w ": " tval[t, "file"] " " ge ": " tval[t, "gap"]) }
            if (thas[t, "crate"]) pcrate[tval[t, "crate"]] = 1
            if (thas[t, "input"] && !(tval[t, "input"] in kinds))
                problem("m", w ": input \047" tval[t, "input"] "\047 is not one of install record, network, user file, scenario, settings, plugin, other")
            file = tval[t, "file"]; crate = tval[t, "crate"]
            if (thas[t, "file"] && !(file in exists)) problem("m", w ": file " file " does not exist")
            else if (thas[t, "file"]) {
                how = w (ht ? " target " tval[t, "target"] : he ? " exempt" : " GAP")
                if (thas[t, "crate"]) {
                    if (!(crate in pkgdir)) problem("m", w ": crate \047" crate "\047 is not a package under crates/ or apps/")
                    else if (index(file, pkgdir[crate] "/") != 1) problem("m", w ": file " file " is not in package " crate " (" pkgdir[crate] ")")
                }
                if (thas[t, "entry"]) {
                    ne = split(tval[t, "entry"], es, "\037")
                    if (ne == 0) problem("m", w ": empty entry")
                    fns_of(file)
                    for (j = 1; j <= ne; j++) {
                        e = es[j]
                        if (e !~ /^[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*$/) { problem("m", w ": entry \047" e "\047 is not a path (a::B::c)"); continue }
                        last = e; sub(/^.*::/, "", last)
                        if (!((file, last) in fndef)) problem("m", w ": entry \047" e "\047: no \047fn " last "\047 in " file)
                        if (!((file, last) in pcover)) pcover[file, last] = how
                        else if (index(pcover[file, last], how) == 0) pcover[file, last] = pcover[file, last] ", " how
                    }
                }
            }
            if (ht) {
                b = tval[t, "target"]
                named[b] = 1
                if (!(b in bin)) problem("a", w ": target \047" b "\047 is not a [[bin]] of fuzz/Cargo.toml")
                else if (bin[b] == "-" || !(bin[b] in exists)) problem("a", w ": target \047" b "\047: source " bin[b] " of the [[bin]] does not exist")
            }
        }
        # (b) every bin is named
        nbins_ok = 0
        for (i = 1; i <= nbin; i++) {
            b = binname[i]; p = binpath[i]
            if (b == "-") { problem("b", "a [[bin]] without a name (path " p ")"); continue }
            if (b in named) { nbins_ok++; continue }
            if (p != "-" && skipped(p)) { nbins_ok++; continue }
            problem("b", b " (" p ")")
        }
        # every package is in some manifest: a [[parser]] crate, a [[package]]
        # table or a whole-file skip that covers the package directory
        for (p in pkgdir) {
            dir = pkgdir[p]
            if (p in pcrate) {
                if (p in pkgtable) problem("m", pkgtable[p] ": [[package]] " p " has [[parser]] tables too (drop the [[package]])")
                continue
            }
            if (p in pkgtable) continue
            if (skipped(dir) || skipped(dir "/")) continue
            problem("m", "package " p " (" dir ") is in no manifest: no [[parser]] has crate = \"" p "\", no [[package]] names it and no [[skip]] covers " dir)
        }
        # (c) scanned functions: each is the last segment of an entry of a [[parser]] for its
        # file, or a skip covers it
        nfl = 0; ncov = 0
        for (i = 1; i <= nscan; i++) {
            f = sfile[i]; n = sfn[i]
            if (!(f in fl)) { fl[f] = 1; order[++nfl] = f; fbad[f] = 0 }
            if ((f, n) in pcover) { ncov++; if (list) print "covered: " f " " n " (line " sline[i] ") by " pcover[f, n]; continue }
            k = skipped_fn(f, n)
            if (k) { ncov++; if (list) print "covered: " f " " n " (line " sline[i] ") by skip " skpath[k] " (" skwhere[k] ")"; continue }
            if (list) print "UNCOVERED: " f " " n " (line " sline[i] ")"
            fbad[f] = 1
            bad[f] = bad[f] (bad[f] == "" ? "" : ", ") n " (line " sline[i] ")"
        }
        for (i = 1; i <= nfl; i++) {
            f = order[i]
            if (!fbad[f]) continue
            hint = ""
            for (t = 1; t <= nt; t++) if (tkind[t] == "parser" && tval[t, "file"] == f) { hint = " (the file has a [[parser]], " twhere[t] ", but no entry names these)"; break }
            problem("c", f ": " bad[f] hint)
        }
        npar = 0; ntar = 0; ngap = 0; npkg = 0
        for (t = 1; t <= nt; t++) {
            if (tkind[t] == "parser") { npar++; if (thas[t, "target"]) ntar++; if (thas[t, "gap"]) ngap++ }
            if (tkind[t] == "package") npkg++
        }
        printf "check-fuzz-coverage: %d manifest(s), %d [[parser]] (%d target, %d exempt, %d gap), %d [[package]], %d [[skip]]; %d of %d fuzz bins named; %d of %d parser-like functions in %d scanned files covered\n", split(manifests, _m, " "), npar, ntar, npar - ntar - ngap, ngap, npkg, nskip, nbins_ok, nbin, ncov, nscan, nfl
        heads["m"] = "(m) manifest errors:"
        heads["a"] = "(a) targets that are not a [[bin]] with a source file:"
        heads["b"] = "(b) fuzz [[bin]]s named by no manifest target:"
        heads["c"] = "(c) parser-like public functions that no [[parser]] entry or [[skip]] covers:"
        heads["g"] = "(g) GAP: parsers recorded without a fuzz target (criterion 2 is not met while any remain):"
        split("m a b c g", cats, " ")
        for (j = 1; j <= 5; j++) {
            c = cats[j]
            if (!(c in probs)) continue
            print "FAIL " heads[c]
            printf "%s", probs[c]
        }
        if (nprob > 0) { print "check-fuzz-coverage: FAILED (" nprob " problem(s))"; exit 1 }
        print "check-fuzz-coverage: ok"
    }' "$tmp/packages" "$tmp/bins" "$tmp/files" "$tmp/records" "$tmp/scan" || rc=$?
exit "$rc"
