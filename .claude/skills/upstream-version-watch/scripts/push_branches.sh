#!/bin/bash
# Direct normal ELF startup; no install/build/download or Python business fallback.
# Ported: branch push for token-free remotes (ssh/scp/local). https remotes fail loud (HTTPS_NEEDS_HINTLIB); --self-test runs in Rust (tag.py scope args read as text; HintError mutation control N/A — hintlib is Python-owned).
set -euo pipefail
SCRIPT_PATH=${BASH_SOURCE[0]}
case "$SCRIPT_PATH" in */*) SCRIPT_DIR=${SCRIPT_PATH%/*}; [ -n "$SCRIPT_DIR" ] || SCRIPT_DIR=/ ;; *) SCRIPT_DIR=. ;; esac
SDIR="$(cd "$SCRIPT_DIR" && pwd -P)"
if [ "${EASY_VLLM_UPSTREAM_RUNTIME_ROOT+set}" = set ]; then
  runtime_root="$EASY_VLLM_UPSTREAM_RUNTIME_ROOT"
  if [ -z "$runtime_root" ]; then
    printf '[push_branches] ERROR explicit upstream runtime root is empty\n' >&2
    exit 2
  fi
else
  runtime_root="$SDIR/../rust/runtime"
fi
binary="$runtime_root/target/release/push_branches"
if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
  printf '[push_branches] ERROR normal Rust ELF missing/non-executable: %s\n' "$binary" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$binary" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[push_branches] ERROR normal Rust ELF invalid: %s\n' "$binary" >&2
  exit 2
fi
exec -a "$SDIR/push_branches.py" "$binary" "$@"
