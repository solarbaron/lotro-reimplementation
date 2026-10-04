#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Re-create the dirty-room research clones listed in research/clones.txt.
# Shallow clones go OUTSIDE the repo: DEST, else $TESSERA_CLONES_DIR, else
# ${XDG_CACHE_HOME:-~/.cache}/tessera/clones (about 6 GB). Existing clones are
# fast-forwarded (--update) or skipped. Only spec writers (dirty room) may read them; see
# docs/CLEAN_ROOM.md. Never copy anything from them into the repository.
#
# Usage: tools/fetch-research-clones.sh [--update] [-j JOBS] [DEST]

set -euo pipefail

update=0
jobs=6
dest=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --update) update=1 ;;
    -j) jobs="$2"; shift ;;
    -h|--help) sed -n '3,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) dest="$1" ;;
  esac
  shift
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
list="$repo_root/research/clones.txt"
dest="${dest:-${TESSERA_CLONES_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/tessera/clones}}"

case "$(realpath -m "$dest")/" in
  "$repo_root"/*) echo "refusing to clone inside the repository: $dest" >&2; exit 2 ;;
esac
mkdir -p "$dest"

fetch_one() {
  local name="$1" url="$2" dir="$dest/$1"
  if [[ -d "$dir/.git" ]]; then
    if [[ "$update" == 1 ]]; then
      git -C "$dir" pull -q --ff-only && echo "updated  $name" || echo "FAILED   $name (update)"
    else
      echo "present  $name"
    fi
  elif GIT_TERMINAL_PROMPT=0 git clone -q --depth 1 "$url" "$dir" 2>/dev/null; then
    echo "cloned   $name"
  else
    echo "FAILED   $name ($url)"
  fi
}
export -f fetch_one
export dest update

grep -v -E '^\s*(#|$)' "$list" | xargs -P "$jobs" -L 1 bash -c 'fetch_one "$0" "$1"' \
  | tee >(grep -c '^FAILED' > "$dest/.failed-count")
sleep 0.2
failed="$(cat "$dest/.failed-count" 2>/dev/null || echo 0)"
rm -f "$dest/.failed-count"
echo "done: $(grep -c -v -E '^\s*(#|$)' "$list") listed, $failed failed, into $dest"
