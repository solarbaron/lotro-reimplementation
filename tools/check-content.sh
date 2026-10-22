#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Forbidden-content guard (CLAUDE.md "Content rules" and "Naming",
# docs/CLEAN_ROOM.md §8). Rejects, anywhere in the tree:
#
#   ext       files with game-data, capture or disassembler extensions
#             (.dat .datx .dds .ogg .wav .bik .ttf, plus .otf .ttc .woff .woff2 .rdc
#             .pcap .pcapng .exe .dll .pdb .gzf .idb .i64 .bndb)
#   magic     files whose leading bytes identify one of those formats, whatever the name
#             (archive magic at 0x140, DDS, Ogg, RIFF/WAVE, Bink, TrueType/OpenType/WOFF,
#             PE executables, pcap)
#   size      binary blobs larger than 1 MiB (text files above 1 MiB are a warning)
#   lfs       Git LFS filters in .gitattributes (LFS is disabled for this project)
#   mcp       a .mcp.json file, or a .claude/settings*.json that declares an MCP server
#             or names rea or ghidra: a decompiler MCP registration must never reach a
#             checkout where workflows start clean agents (docs/decisions/0033)
#
# and, in the code under crates/ apps/ fuzz/:
#
#   mark      trademark / Tolkien words in identifiers, file names and Cargo names.
#             Comments and string literals are not checked: plain-prose compatibility
#             statements and interoperability file names ("...client64.exe") are allowed.
#   autoname  decompiler auto-generated names (FUN_00401000, DAT_..., sub_401000, ...),
#             which must never appear in clean-room code
#
# Exceptions live in tools/content-allowlist.txt, one per line, each with a reason.
#
# Usage: tools/check-content.sh [--root DIR] [--no-git] [--self-test] [-q]
#   --root DIR   scan DIR instead of the repository root
#   --no-git     walk the file system instead of asking git for the file list
#                (by default: tracked + untracked-but-not-ignored files)
#   --self-test  plant violations in a temporary tree and check that each is caught
#   -q           print violations only
# Exit status: 0 clean, 1 violations found, 2 usage or environment error.
#
# Needs bash and python3 (>= 3.8). Never modifies anything it scans.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
use_git=1
quiet=0
self_test=0

while [ $# -gt 0 ]; do
    case "$1" in
        --root) root="$(cd "${2:?--root needs a directory}" && pwd)"; shift 2 ;;
        --no-git) use_git=0; shift ;;
        --self-test) self_test=1; shift ;;
        -q) quiet=1; shift ;;
        -h|--help) sed -n '2,37p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "check-content: unknown argument: $1" >&2; exit 2 ;;
    esac
done

if ! command -v python3 >/dev/null 2>&1; then
    echo "check-content: python3 is required" >&2
    exit 2
fi

run_scan() {
    # $1 = root, $2 = use_git, $3 = quiet, $4 = allowlist path
    python3 - "$1" "$2" "$3" "$4" <<'PY'
import fnmatch, json, os, re, subprocess, sys

root, use_git, quiet, allow_path = sys.argv[1], sys.argv[2] == "1", sys.argv[3] == "1", sys.argv[4]

MIB = 1 << 20
BINARY_LIMIT = 1 * MIB
TEXT_WARN_LIMIT = 1 * MIB

FORBIDDEN_EXT = {
    # Required by the project rules.
    ".dat": "game archive", ".datx": "game archive", ".dds": "texture",
    ".ogg": "audio", ".wav": "audio", ".bik": "video", ".ttf": "font",
    # Same rationale, other spellings and neighbouring formats.
    ".otf": "font", ".ttc": "font", ".woff": "font", ".woff2": "font",
    ".rdc": "RenderDoc capture", ".pcap": "packet capture", ".pcapng": "packet capture",
    ".exe": "executable", ".dll": "executable", ".pdb": "debug symbols",
    ".gzf": "Ghidra database", ".idb": "IDA database", ".i64": "IDA database",
    ".bndb": "Binary Ninja database",
}

# Single-word marks matched anywhere inside one identifier word (distinctive words).
MARK_SUBSTR = [
    "lotro", "tolkien", "hobbit", "mordor", "angmar", "mirkwood", "isengard", "mithril",
    "daybreak", "middleearth", "lordoftherings", "standingstone", "helmsdeep",
    "rivendell", "lothlorien", "sauron", "gandalf", "saruman", "nazgul", "galadriel",
    "aragorn", "numenor", "silmaril", "hobbiton",
]
# Short or ambiguous marks matched only as a whole identifier word.
MARK_EXACT = {
    "lotr", "ssg", "shire", "moria", "rohan", "turbine", "gondor", "frodo", "bilbo",
    "legolas", "gimli", "elrond", "balrog", "erebor", "arnor", "eriador", "smaug",
}
# Multi-word marks, matched against runs of adjacent identifier words.
MARK_JOINED = {
    "middleearth", "lordoftherings", "lordoftheringsonline", "standingstone",
    "standingstonegames", "helmsdeep", "helmdeep", "lordrings",
}

AUTONAME = re.compile(
    r"^(?:(?:thunk_)?FUN|DAT|LAB|PTR|UNK|SUB|switchD|caseD|BYTE|WORD|DWORD|QWORD)_[0-9A-Fa-f]{6,16}$"
    r"|^(?:sub|loc|off|unk|byte|word|dword|qword|stru|asc|nullsub|locret)_[0-9A-Fa-f]{5,16}$"
)

CODE_DIRS = ("crates/", "apps/", "fuzz/")

# Decompiler MCP registrations (decision 0033). "rea" only as a whole word, so that
# "Read" or "area" do not match.
CLAUDE_SETTINGS = re.compile(r"(?:^|/)\.claude/settings[^/]*\.json$")
DECOMPILER_WORD = re.compile(r"(?<![A-Za-z0-9])rea(?![A-Za-z0-9])|rea-agents|ghidra", re.I)
MCP_KEYS = {"mcpServers", "enabledMcpjsonServers", "enableAllProjectMcpServers"}


def mcp_keys(node):
    found = set()
    if isinstance(node, dict):
        for k, v in node.items():
            if k in MCP_KEYS and v not in (None, False, {}, []):
                found.add(k)
            found |= mcp_keys(v)
    elif isinstance(node, list):
        for v in node:
            found |= mcp_keys(v)
    return found


def check_claude_settings(path, text):
    try:
        keys = mcp_keys(json.loads(text))
    except ValueError:
        keys = {k for k in MCP_KEYS if f'"{k}"' in text}
    for k in sorted(keys):
        report("mcp", path, 0, f"declares MCP servers ('{k}'); attach them per session instead")
    for n, l in enumerate(text.splitlines(), 1):
        m = DECOMPILER_WORD.search(l)
        if m:
            report("mcp", path, n, f"names the decompiler tool '{m.group(0)}'")
TOKEN_EXT = {".rs", ".wgsl"}

errors, warnings = [], []


def load_allowlist(path):
    rules = []
    if not os.path.isfile(path):
        return rules
    with open(path, encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            body = line.split("#", 1)[0].strip()
            if not body:
                continue
            parts = body.split()
            if len(parts) != 3 or "#" not in line:
                print(f"{path}:{n}: allowlist lines need '<check> <path-glob> <word|*> # reason'",
                      file=sys.stderr)
                sys.exit(2)
            rules.append((parts[0], parts[1], parts[2].lower()))
    return rules


ALLOW = load_allowlist(allow_path)


def allowed(check, path, word="*"):
    for c, glob, w in ALLOW:
        if c == check and fnmatch.fnmatchcase(path, glob) and (w == "*" or w == word.lower()):
            return True
    return False


def report(check, path, line, msg, word="*", warn=False):
    if allowed(check, path, word):
        return
    loc = f"{path}:{line}" if line else path
    (warnings if warn else errors).append(f"{loc}: [{check}] {msg}")


def list_files():
    if use_git:
        try:
            out = subprocess.run(
                ["git", "-C", root, "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                check=True, capture_output=True,
            ).stdout
            files = sorted({p for p in out.decode("utf-8", "surrogateescape").split("\0") if p})
            return [p for p in files if os.path.isfile(os.path.join(root, p))]
        except (OSError, subprocess.CalledProcessError):
            pass  # not a git work tree: fall back to walking
    skip_dirs = {".git", "target", "node_modules", ".jj"}
    skip_paths = {"fuzz/target", "fuzz/corpus", "fuzz/artifacts"}
    files = []
    for d, dirs, names in os.walk(root):
        rel_d = os.path.relpath(d, root).replace(os.sep, "/")
        rel_d = "" if rel_d == "." else rel_d + "/"
        dirs[:] = sorted(x for x in dirs
                         if x not in skip_dirs and (rel_d + x) not in skip_paths)
        for name in sorted(names):
            p = os.path.join(d, name)
            if os.path.isfile(p) and not os.path.islink(p):
                files.append(rel_d + name)
    return files


def sniff(head, size):
    """Returns a description if `head` (first 0x400 bytes) is a forbidden format."""
    if len(head) >= 0x144 and size >= 0x400 and head[0x140:0x144] == b"BT\x00\x00":
        return "data archive (magic at 0x140)"
    if head[:4] == b"DDS ":
        return "DDS texture"
    if head[:4] == b"OggS":
        return "Ogg stream"
    if head[:4] == b"RIFF" and head[8:12] == b"WAVE":
        return "RIFF/WAVE audio"
    if head[:3] in (b"BIK", b"KB2"):
        return "Bink video"
    if head[:4] in (b"\x00\x01\x00\x00", b"OTTO", b"true", b"ttcf", b"wOFF", b"wOF2"):
        # 00 01 00 00 is also the start of many unrelated binaries: require a table count.
        if head[:4] != b"\x00\x01\x00\x00" or (len(head) >= 6 and 0 < int.from_bytes(head[4:6], "big") < 64):
            return "font"
    if head[:2] == b"MZ" and len(head) >= 0x40:
        pe = int.from_bytes(head[0x3C:0x40], "little")
        if pe + 4 <= len(head) and head[pe:pe + 4] == b"PE\x00\x00":
            return "PE executable"
    if head[:4] in (b"\xd4\xc3\xb2\xa1", b"\xa1\xb2\xc3\xd4", b"\x0a\x0d\x0d\x0a"):
        return "packet capture"
    return None


def split_words(ident):
    out = []
    for part in re.split(r"[^A-Za-z0-9]+", ident):
        # CamelCase / ACRONYMCase / digits boundaries.
        out += re.findall(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z]+|[A-Z]+|[0-9]+", part)
    return [w.lower() for w in out if w]


def mark_hits(ident):
    words = split_words(ident)
    hits = set()
    for w in words:
        if w in MARK_EXACT:
            hits.add(w)
        for m in MARK_SUBSTR:
            if m in w:
                hits.add(m)
    alpha = [w for w in words if not w.isdigit()]
    for i in range(len(alpha)):
        joined = ""
        for j in range(i, min(i + 5, len(alpha))):
            joined += alpha[j]
            if j > i and joined in MARK_JOINED:
                hits.add(joined)
    return hits


RAW_STR = re.compile(r'(?:b|c)?r(#*)"')


def rust_identifiers(src):
    """Yields (line, identifier) for code outside comments, strings and char literals."""
    i, n, line = 0, len(src), 1
    ident = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1; i += 1; continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        if src.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if src.startswith("/*", i): depth += 1; i += 2
                elif src.startswith("*/", i): depth -= 1; i += 2
                else:
                    line += src[i] == "\n"; i += 1
            continue
        m = RAW_STR.match(src, i)
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            close = '"' + m.group(1)
            j = src.find(close, m.end())
            j = n if j < 0 else j + len(close)
            line += src.count("\n", i, j); i = j
            continue
        if c == '"' or (c in "bc" and i + 1 < n and src[i + 1] == '"'
                        and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_"))):
            i += 1 if c == '"' else 2
            while i < n and src[i] != '"':
                if src[i] == "\\": i += 1
                elif src[i] == "\n": line += 1
                i += 1
            i += 1
            continue
        if c == "'":
            if i + 1 < n and src[i + 1] == "\\":
                j = src.find("'", i + 2)
                i = n if j < 0 else j + 1
                continue
            if i + 2 < n and src[i + 2] == "'":
                i += 3; continue
            i += 1  # lifetime or label: the name after it is an identifier
            continue
        m = ident.match(src, i)
        if m:
            yield line, m.group(0)
            i = m.end(); continue
        i += 1


def check_code(path, text):
    if path.endswith(("Cargo.toml",)):
        for n, l in enumerate(text.splitlines(), 1):
            m = re.match(r'\s*name\s*=\s*"([^"]+)"', l)
            if m:
                for w in sorted(mark_hits(m.group(1))):
                    report("mark", path, n, f"trademark word '{w}' in Cargo name '{m.group(1)}'", w)
        return
    seen = set()
    for n, tok in rust_identifiers(text):
        if AUTONAME.match(tok):
            report("autoname", path, n, f"decompiler-style name '{tok}'", tok)
        if tok in seen:
            continue
        hits = mark_hits(tok)
        if hits:
            seen.add(tok)
            for w in sorted(hits):
                report("mark", path, n, f"trademark word '{w}' in identifier '{tok}'", w)


files = list_files()
for path in files:
    full = os.path.join(root, path)
    try:
        size = os.path.getsize(full)
        with open(full, "rb") as f:
            head = f.read(0x400)
    except OSError as e:
        report("io", path, 0, f"cannot read: {e}")
        continue
    base = os.path.basename(path).lower()
    ext = os.path.splitext(base)[1]
    if ext in FORBIDDEN_EXT:
        report("ext", path, 0, f"forbidden file type {ext} ({FORBIDDEN_EXT[ext]})")
    what = sniff(head, size)
    if what:
        report("magic", path, 0, f"content looks like a {what}")
    binary = b"\x00" in head
    if not binary and size > len(head):
        with open(full, "rb") as f:
            binary = b"\x00" in f.read(8192)
    if binary and size > BINARY_LIMIT:
        report("size", path, 0, f"binary file of {size} bytes exceeds the {BINARY_LIMIT}-byte limit")
    elif not binary and size > TEXT_WARN_LIMIT:
        report("size", path, 0, f"text file of {size} bytes is unusually large", warn=True)
    if base == ".gitattributes":
        with open(full, encoding="utf-8", errors="replace") as f:
            for n, l in enumerate(f, 1):
                if "filter=lfs" in l:
                    report("lfs", path, n, "Git LFS is disabled for this project")
    if base == ".mcp.json":
        report("mcp", path, 0, "MCP server registration file; attach servers per session "
               "with --strict-mcp-config (decision 0033)")
    if CLAUDE_SETTINGS.search(path):
        with open(full, encoding="utf-8", errors="replace") as f:
            check_claude_settings(path, f.read())
    if path.startswith(CODE_DIRS):
        for comp in path.split("/"):
            for w in sorted(mark_hits(os.path.splitext(comp)[0])):
                report("mark", path, 0, f"trademark word '{w}' in path component '{comp}'", w)
        if ext in TOKEN_EXT or base == "cargo.toml":
            with open(full, encoding="utf-8", errors="replace") as f:
                check_code(path, f.read())

for w in warnings:
    print(f"warning: {w}")
for e in errors:
    print(f"error: {e}")
if not quiet:
    print(f"check-content: {len(files)} files scanned, {len(errors)} error(s), "
          f"{len(warnings)} warning(s)", file=sys.stderr)
sys.exit(1 if errors else 0)
PY
}

allowlist="$root/tools/content-allowlist.txt"
[ -f "$allowlist" ] || allowlist="$here/content-allowlist.txt"

if [ "$self_test" = 1 ]; then
    t="$(mktemp -d)"
    trap 'rm -rf "$t"' EXIT
    mkdir -p "$t/crates/demo/src" "$t/docs" "$t/assets" "$t/.claude"
    printf '{"mcpServers":{"x":{"command":"rea","args":["--mcp"]}}}\n' > "$t/.mcp.json"
    printf '{\n  "mcpServers": {"dec": {}},\n  "permissions": {"allow": ["Read(*)", "mcp__ghidra__decompile"]}\n}\n' \
        > "$t/.claude/settings.json"
    printf '{\n  "permissions": {"allow": ["Read(*)", "Bash(cargo test:*)"]}\n}\n' \
        > "$t/.claude/settings.local.json"
    printf 'x' > "$t/assets/client_general.dat"
    printf 'DDS |\x7c\x00\x00\x00' > "$t/assets/renamed.bin"
    head -c 2097152 /dev/zero > "$t/assets/blob.bin"
    printf '*.png filter=lfs diff=lfs merge=lfs -text\n' > "$t/.gitattributes"
    cat > "$t/crates/demo/src/lib.rs" <<'RS'
// SPDX-License-Identifier: GPL-3.0-or-later
//! Reads LOTRO data files (prose in comments is fine).
pub fn open_lotro_archive() {}
pub struct MiddleEarthMap;
fn FUN_00401000() {}
const OK: &str = "lotroclient64.exe is an interoperability file name";
fn slot_range<'a>(x: &'a str) -> char { let _ = x; 'x' }
RS
    printf '[package]\nname = "hobbit-viewer"\n' > "$t/crates/demo/Cargo.toml"
    : > "$t/allow.txt"
    set +e
    out="$(run_scan "$t" 0 1 "$t/allow.txt")"
    status=$?
    set -e
    fail=0
    for want in \
        "client_general.dat: \[ext\]" \
        "renamed.bin: \[magic\]" \
        "blob.bin: \[size\]" \
        ".gitattributes:1: \[lfs\]" \
        "lib.rs:3: \[mark\] trademark word 'lotro'" \
        "lib.rs:4: \[mark\] trademark word 'middleearth'" \
        "lib.rs:5: \[autoname\]" \
        "Cargo.toml:2: \[mark\] trademark word 'hobbit'" \
        ".mcp.json: \[mcp\]" \
        "settings.json: \[mcp\] declares MCP servers" \
        "settings.json:3: \[mcp\] names the decompiler tool 'ghidra'"; do
        if ! printf '%s\n' "$out" | grep -q -- "$want"; then
            echo "self-test: expected a finding matching: $want" >&2
            fail=1
        fi
    done
    for unwanted in "lib.rs:2:" "lib.rs:6:" "lib.rs:7:" "settings.local.json"; do
        if printf '%s\n' "$out" | grep -q -- "$unwanted"; then
            echo "self-test: false positive at $unwanted" >&2
            fail=1
        fi
    done
    if [ "$status" != 1 ] || [ "$fail" != 0 ]; then
        printf '%s\n' "$out" >&2
        echo "check-content: self-test FAILED" >&2
        exit 1
    fi
    echo "check-content: self-test passed"
    exit 0
fi

run_scan "$root" "$use_git" "$quiet" "$allowlist"
