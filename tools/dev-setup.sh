#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Check (and optionally prepare) a development machine for Tessera. Safe to re-run.
#
# Usage: tools/dev-setup.sh [--fix] [--fetch-research]
#   (no args)         report what is present/missing; exit 1 if a REQUIRED item is missing
#   --fix             write the machine-local .cargo/config.toml (build cache location) from
#                     TESSERA_TARGET_DIR. Nothing else; never downloads anything.
#   --fetch-research  download the dirty-room research clones (about 6 GB; spec writers
#                     only) into $TESSERA_CLONES_DIR or ${XDG_CACHE_HOME:-~/.cache}/tessera/clones
# Environment: TESSERA_GAME_DIR (game install), TESSERA_TARGET_DIR (cargo build cache dir),
# TESSERA_CLONES_DIR (research clones). See docs/DEVELOPMENT.md.

set -uo pipefail

fix=0
fetch=0
for a in "$@"; do
  case "$a" in
    --fix) fix=1 ;;
    --fetch-research) fetch=1 ;;
    -h|--help) sed -n '3,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "dev-setup: unknown argument $a (see --help)" >&2; exit 2 ;;
  esac
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2

missing_required=0
if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then g=$'\033[32m' y=$'\033[33m' r=$'\033[31m' n=$'\033[0m'; else g='' y='' r='' n=''; fi
ok()   { printf '  %sok%s       %s\n' "$g" "$n" "$*"; }
warn() { printf '  %soptional%s %s\n' "$y" "$n" "$*"; }
bad()  { printf '  %sMISSING%s  %s\n' "$r" "$n" "$*"; missing_required=1; }
have() { command -v "$1" >/dev/null 2>&1; }

echo "== Required"
if have cargo && have rustc; then
  ok "rust: $(rustc --version)"
  pin="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml 2>/dev/null | head -n 1)"
  have_ver="$(rustc --version | awk '{print $2}')"
  if [[ -n "$pin" && "$pin" =~ ^[0-9] && "$have_ver" != "$pin" ]]; then
    if have rustup; then
      warn "rustc $have_ver differs from the pinned $pin (rust-toolchain.toml); in the repo rustup should switch to it automatically"
    else
      warn "rustc $have_ver differs from the pinned $pin (rust-toolchain.toml is ignored without rustup); CI uses $pin, so newer clippy lints may fail there"
    fi
  fi
  for c in rustfmt clippy-driver; do
    have "$c" && ok "$c" || bad "$c (install the rustfmt/clippy components)"
  done
else
  bad "rust toolchain (cargo, rustc) — https://rustup.rs or your distro package"
fi
have git && ok "git $(git --version | awk '{print $3}')" || bad "git"
if [[ -n "$(git config user.email)" ]]; then ok "git identity: $(git config user.name) <$(git config user.email)>"; else bad "git identity (git config user.name / user.email)"; fi
git remote get-url origin >/dev/null 2>&1 && ok "git remote origin: $(git remote get-url origin)" || warn "no git remote 'origin'"
have python3 && ok "python3 (spec probes)" || bad "python3"

echo "== Game data (real-data tests, visual checks)"
if [[ -n "${TESSERA_GAME_DIR:-}" ]]; then
  if [[ -f "$TESSERA_GAME_DIR/client_general.dat" ]]; then ok "TESSERA_GAME_DIR=$TESSERA_GAME_DIR"; else bad "TESSERA_GAME_DIR set but client_general.dat not found there"; fi
else
  warn "TESSERA_GAME_DIR unset — real-data tests will be SKIPPED (reported as passed). Export it: export TESSERA_GAME_DIR=\"\$(cargo run -q -p tessera-dattool -- detect --path)\""
fi

echo "== Build cache"
if [[ -f .cargo/config.toml ]]; then
  ok ".cargo/config.toml (machine-local): $(grep -E '^\s*target-dir' .cargo/config.toml | head -1)"
else
  if [[ "$fix" == 1 && -n "${TESSERA_TARGET_DIR:-}" ]]; then
    mkdir -p .cargo "$TESSERA_TARGET_DIR"
    printf '# Machine-local (git-ignored).\n[build]\ntarget-dir = "%s"\n' "$TESSERA_TARGET_DIR" > .cargo/config.toml
    ok "created .cargo/config.toml -> $TESSERA_TARGET_DIR"
  else
    warn "no .cargo/config.toml: builds go to ./target (needs ~20 GB). Set TESSERA_TARGET_DIR and run --fix to relocate"
  fi
fi
# Measure the disk the build cache will actually use.
cache_dir="$(sed -n 's/^[[:space:]]*target-dir[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' .cargo/config.toml 2>/dev/null | head -n 1)"
cache_dir="${CARGO_TARGET_DIR:-${cache_dir:-$root/target}}"
probe="$cache_dir"
while [[ ! -e "$probe" && "$probe" != "/" ]]; do probe="$(dirname "$probe")"; done
avail_kb=$(df -Pk "$probe" | awk 'NR==2{print $4}')
if (( avail_kb > 30*1024*1024 )); then
  ok "build cache $cache_dir: $((avail_kb/1024/1024)) GB free"
else
  warn "build cache $cache_dir: only $((avail_kb/1024/1024)) GB free (debug builds need ~20 GB, ~30 GB with release builds and fuzzing)"
fi

echo "== Quality tooling"
have cargo-deny && ok "cargo-deny" || warn "cargo-deny (cargo install cargo-deny --locked)"
have cargo-fuzz && ok "cargo-fuzz" || warn "cargo-fuzz (optional; tools/fuzz-smoke.sh falls back to stable libFuzzer builds)"
have cargo-nextest && ok "cargo-nextest" || warn "cargo-nextest (optional)"

echo "== Rendering / visual checks"
have xvfb-run && ok "xvfb-run (headless window tests)" || warn "xvfb-run"
{ have grim || have import || have scrot; } && ok "screenshot tool" || warn "screenshot tool (grim on Wayland, scrot/import on X11)"
if ls /usr/share/vulkan/icd.d/lvp_icd*.json >/dev/null 2>&1 || [[ -n "${VK_DRIVER_FILES:-}" ]]; then
  ok "Mesa lavapipe ICD (software Vulkan for golden images)"
else
  warn "Mesa lavapipe not found — software golden checks run via llvmpipe GL instead (crates/render/tests/golden/README.md)"
fi

echo "== Dirty-room research (spec writers only)"
clones="${TESSERA_CLONES_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/tessera/clones}"
if [[ "$fetch" == 1 ]]; then
  tools/fetch-research-clones.sh "$clones" | tail -1
elif [[ -d "$clones" ]] && [[ -n "$(ls -A "$clones" 2>/dev/null)" ]]; then
  ok "research clones: $(ls "$clones" | wc -l) in $clones"
else
  warn "research clones absent from $clones — only needed for spec writing (≈6 GB): tools/dev-setup.sh --fetch-research"
fi

echo "== Claude Code"
[[ -f .claude/workflows/tessera-milestone.js ]] && ok "milestone workflow (.claude/workflows/tessera-milestone.js)" || bad "milestone workflow missing"
[[ -d .claude/agents ]] && ok "agents: $(ls .claude/agents | tr '\n' ' ')" || warn "no .claude/agents"

echo
if [[ "$missing_required" == 1 ]]; then echo "Some REQUIRED items are missing."; exit 1; fi
echo "Machine is ready. Next: cargo test --workspace   (see docs/DEVELOPMENT.md)"
