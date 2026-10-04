#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Check (and optionally prepare) a development machine for Tessera. Safe to re-run.
#
# Usage: tools/dev-setup.sh [--fix]
#   (no args)  report what is present/missing; exit 1 if a REQUIRED item is missing
#   --fix      also do the safe local steps: create .cargo/config.toml if you pass
#              TESSERA_TARGET_DIR, and fetch research
#              clones into ~/.cache/tessera/clones
# Environment: TESSERA_GAME_DIR (game install), TESSERA_TARGET_DIR (cargo build cache dir).
# See docs/DEVELOPMENT.md.

set -uo pipefail

fix=0
[[ "${1:-}" == "--fix" ]] && fix=1
[[ "${1:-}" == "-h" || "${1:-}" == "--help" ]] && { sed -n '3,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0; }

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2

missing_required=0
ok()   { printf '  \033[32mok\033[0m       %s\n' "$*"; }
warn() { printf '  \033[33moptional\033[0m %s\n' "$*"; }
bad()  { printf '  \033[31mMISSING\033[0m  %s\n' "$*"; missing_required=1; }
have() { command -v "$1" >/dev/null 2>&1; }

echo "== Required"
if have cargo && have rustc; then
  ok "rust: $(rustc --version)"
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
  warn "TESSERA_GAME_DIR unset — real-data tests will be skipped (auto-detect: cargo run -p tessera-dattool -- detect)"
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
avail_kb=$(df -Pk . | awk 'NR==2{print $4}')
(( avail_kb > 30*1024*1024 )) && ok "free space here: $((avail_kb/1024/1024)) GB" || warn "low free space here: $((avail_kb/1024/1024)) GB (builds need ~20 GB)"

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
clones="${XDG_CACHE_HOME:-$HOME/.cache}/tessera/clones"
if [[ -d "$clones" ]] && [[ -n "$(ls -A "$clones" 2>/dev/null)" ]]; then
  ok "research clones: $(ls "$clones" | wc -l) in $clones"
elif [[ "$fix" == 1 ]]; then
  tools/fetch-research-clones.sh "$clones" | tail -1
else
  warn "research clones absent — tools/fetch-research-clones.sh (≈6 GB; only needed for spec writing)"
fi

echo "== Claude Code"
[[ -f .claude/workflows/tessera-milestone.js ]] && ok "milestone workflow (.claude/workflows/tessera-milestone.js)" || bad "milestone workflow missing"
[[ -d .claude/agents ]] && ok "agents: $(ls .claude/agents | tr '\n' ' ')" || warn "no .claude/agents"

echo
if [[ "$missing_required" == 1 ]]; then echo "Some REQUIRED items are missing."; exit 1; fi
echo "Machine is ready. Next: cargo test --workspace   (see docs/DEVELOPMENT.md)"
